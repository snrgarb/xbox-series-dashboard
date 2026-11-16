//! Import from Playnite, once (docs/import.md): a plan read from Playnite's files (`playnite::look`), measured
//! against the library and carried out on it.
//!
//! Everything it does only adds, so it can be run again and doubles nothing:
//! - a game is the library's game of the same store and store id, matched without regard to case, never by name;
//! - its play time and last played become Playnite's where Playnite's are more (Playnite's play time for a store's
//!   game is mostly the store's own, which the library may have already: they are never added together);
//! - it becomes a favourite or hidden where Playnite says so, and nothing that is one stops being one;
//! - a Group comes over once, remembered by Playnite's id for its preset: a rule's as a custom filter
//!   (`playnite.group.<id>` in the settings table names it), a list of games' as a collection of the source
//!   [`SOURCE`], whose games a later import puts right. One whose name a Group here has already is left.
use std::collections::HashSet;

use anyhow::Result;
use model::playnite::{Group, Picks, Plan, Preview};
use rusqlite::{params, OptionalExtension, Transaction};

use crate::filters::{sort_text, STEP};
use crate::Database;

/// The source a Group of Playnite's games is a collection of.
pub const SOURCE: &str = "playnite";
/// `playnite.imported`: when the last import was, in Unix seconds.
pub const IMPORTED: &str = "playnite.imported";
/// `playnite.offered`: the Toast that says there is a Playnite library has been shown.
pub const OFFERED: &str = "playnite.offered";
/// `playnite.group.<preset id>`: the custom filter a rule's Group became.
const GROUP: &str = "playnite.group.";

/// What an import did: what it brought, and a line for the log for each thing it changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imported {
    pub preview: Preview,
    pub lines: Vec<String>,
}

impl Database {
    /// What importing `plan` would bring to the library as it is now. Nothing is changed.
    pub fn import_preview(&mut self, plan: &Plan) -> Result<Preview> {
        let tx = self.conn.transaction()?;
        // Worked out as the import itself would be, and then undone.
        let imported = work(&tx, plan)?;
        tx.rollback()?;
        Ok(imported.preview)
    }

    /// Imports `plan` at the time `now`. Running it again brings only what has changed since in Playnite.
    pub fn import(&mut self, plan: &Plan, now: u64) -> Result<Imported> {
        let tx = self.conn.transaction()?;
        let imported = work(&tx, plan)?;
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
            params![IMPORTED, now.to_string()],
        )?;
        tx.commit()?;
        Ok(imported)
    }
}

struct Known {
    id: i64,
    name: String,
    source: String,
    source_id: String,
    playtime: i64,
    last_played: i64,
    favourite: bool,
    hidden: bool,
}

/// The library's game of this store and store id.
fn game(tx: &Transaction, source: &str, source_id: &str) -> Result<Option<Known>> {
    let known = tx
        .query_row(
            "SELECT id, name, source, source_id, playtime_minutes, last_played, favourite, hidden FROM games
             WHERE source = ?1 AND source_id = ?2 COLLATE NOCASE
             ORDER BY source_id = ?2 DESC, gone, id LIMIT 1",
            params![source, source_id],
            |row| {
                Ok(Known {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    source: row.get(2)?,
                    source_id: row.get(3)?,
                    playtime: row.get(4)?,
                    last_played: row.get(5)?,
                    favourite: row.get(6)?,
                    hidden: row.get(7)?,
                })
            },
        )
        .optional()?;
    Ok(known)
}

fn work(tx: &Transaction, plan: &Plan) -> Result<Imported> {
    let mut preview = Preview {
        from: plan.from.display().to_string(),
        games_read: plan.games_read,
        other_stores: plan.other_stores,
        skipped: plan.skipped.clone(),
        ..Preview::default()
    };
    let mut lines = Vec::new();
    let mut matched = HashSet::new();
    for marks in &plan.games {
        let Some(game) = game(tx, &marks.source, &marks.source_id)? else {
            preview.not_here += 1;
            continue;
        };
        if !matched.insert(game.id) {
            // Playnite has the same game twice: the second only adds what the first did not.
            preview.matched -= 1;
        }
        preview.matched += 1;
        let who = format!("{} ({} {})", game.name, game.source, game.source_id);
        let playtime = i64::from(marks.playtime_minutes);
        if playtime > game.playtime {
            tx.execute("UPDATE games SET playtime_minutes = ?2 WHERE id = ?1", params![game.id, playtime])?;
            preview.playtime += 1;
            lines.push(format!("import: {who}: play time {playtime} min, was {} min", game.playtime));
        }
        let last_played = marks.last_played as i64;
        if last_played > game.last_played {
            tx.execute("UPDATE games SET last_played = ?2 WHERE id = ?1", params![game.id, last_played])?;
            preview.last_played += 1;
            lines.push(format!("import: {who}: last played {last_played}, was {}", game.last_played));
        }
        if marks.favourite && !game.favourite {
            tx.execute("UPDATE games SET favourite = 1 WHERE id = ?1", [game.id])?;
            preview.favourites += 1;
            lines.push(format!("import: {who}: a favourite"));
        }
        if marks.hidden && !game.hidden {
            tx.execute("UPDATE games SET hidden = 1 WHERE id = ?1", [game.id])?;
            preview.hidden += 1;
            lines.push(format!("import: {who}: hidden"));
        }
    }
    for group in &plan.groups {
        match bring(tx, group)? {
            Brought::Made(line) | Brought::Changed(line) => {
                preview.groups.push(group.name.clone());
                lines.push(line);
            }
            Brought::Same => {}
            Brought::Left(why) => {
                lines.push(format!("import: Group \"{}\" not brought: {why}", group.name));
                preview.skipped.push((group.name.clone(), why));
            }
        }
    }
    Ok(Imported { preview, lines })
}

enum Brought {
    Made(String),
    Changed(String),
    /// Brought before, and as it was.
    Same,
    Left(String),
}

/// The place after every filter there is.
fn last_place(tx: &Transaction) -> Result<i64> {
    let last: Option<i64> = tx.query_row("SELECT MAX(position) FROM filters", [], |row| row.get(0))?;
    Ok(last.unwrap_or(0) + STEP)
}

/// Whether a filter of this name is here, whatever its case.
fn name_taken(tx: &Transaction, name: &str) -> Result<bool> {
    let taken: Option<i64> = tx
        .query_row("SELECT id FROM filters WHERE name = ?1 COLLATE NOCASE LIMIT 1", [name], |row| row.get(0))
        .optional()?;
    Ok(taken.is_some())
}

fn bring(tx: &Transaction, group: &Group) -> Result<Brought> {
    let taken = "a Group of that name is here already".to_string();
    match &group.picks {
        Picks::Rule(rule) => {
            let key = format!("{GROUP}{}", group.id);
            let brought: Option<String> =
                tx.query_row("SELECT value FROM settings WHERE key = ?1", [&key], |row| row.get(0)).optional()?;
            // Brought once: the filter is the user's now, to change or delete.
            if brought.is_some() {
                return Ok(Brought::Same);
            }
            if name_taken(tx, &group.name)? {
                return Ok(Brought::Left(taken));
            }
            tx.execute(
                "INSERT INTO filters (kind, name, rule, sort, descending, shown, position)
                 VALUES ('custom', ?1, ?2, ?3, ?4, 1, ?5)",
                params![
                    group.name,
                    serde_json::to_string(rule)?,
                    sort_text(group.sort),
                    group.descending,
                    last_place(tx)?
                ],
            )?;
            let filter = tx.last_insert_rowid();
            tx.execute("INSERT INTO settings (key, value) VALUES (?1, ?2)", params![key, filter.to_string()])?;
            Ok(Brought::Made(format!("import: Group \"{}\" made: filter {filter}, by its rule", group.name)))
        }
        Picks::Games(keys) => {
            let mut games = Vec::new();
            for (source, source_id) in keys {
                games.extend(game(tx, source, source_id)?.map(|g| g.id));
            }
            games.sort_unstable();
            games.dedup();
            let known: Option<i64> = tx
                .query_row(
                    "SELECT id FROM collections WHERE source = ?1 AND source_id = ?2",
                    params![SOURCE, group.id],
                    |row| row.get(0),
                )
                .optional()?;
            let (collection, made) = match known {
                Some(collection) => (collection, false),
                None => {
                    if name_taken(tx, &group.name)? {
                        return Ok(Brought::Left(taken));
                    }
                    tx.execute(
                        "INSERT INTO collections (source, source_id, name) VALUES (?1, ?2, ?3)",
                        params![SOURCE, group.id, group.name],
                    )?;
                    let collection = tx.last_insert_rowid();
                    tx.execute(
                        "INSERT INTO filters (kind, collection, name, sort, descending, shown, position)
                         VALUES ('collection', ?1, ?2, ?3, ?4, 1, ?5)",
                        params![collection, group.name, sort_text(group.sort), group.descending, last_place(tx)?],
                    )?;
                    (collection, true)
                }
            };
            let mut had = tx.prepare("SELECT game FROM collection_games WHERE collection = ?1 ORDER BY game")?;
            let had: Vec<i64> = had.query_map([collection], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
            if !made && had == games {
                return Ok(Brought::Same);
            }
            tx.execute("DELETE FROM collection_games WHERE collection = ?1", [collection])?;
            for game in &games {
                tx.execute("INSERT INTO collection_games (collection, game) VALUES (?1, ?2)", params![collection, game])?;
            }
            let (count, name) = (games.len(), &group.name);
            Ok(if made {
                Brought::Made(format!("import: Group \"{name}\" made: {count} games as Playnite picked them"))
            } else {
                Brought::Changed(format!("import: Group \"{name}\" now has {count} games, as Playnite picks them"))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::STEAM;
    use model::playnite::Marks;
    use model::{Game, Library, Rule, Sort};
    use std::path::PathBuf;

    const NOW: u64 = 1_800_000_000;

    fn steam_game(appid: &str, name: &str, playtime: u32, last_played: u64) -> Game {
        Game {
            source: STEAM.into(),
            source_id: appid.into(),
            name: name.into(),
            playtime_minutes: playtime,
            last_played,
            installed: true,
            ..Game::default()
        }
    }

    fn library() -> Database {
        let mut db = Database::in_memory().unwrap();
        let games = vec![
            steam_game("10", "Alder", 600, NOW - 100),
            steam_game("20", "Birch", 0, 0),
            steam_game("30", "Cedar", 0, 0),
        ];
        db.sync(STEAM, &Library { games, ..Library::default() }, NOW - 1000).unwrap();
        db.sync("epic", &Library { games: vec![Game { source_id: "Elm".into(), name: "Elm".into(), ..Game::default() }], ..Library::default() }, NOW - 1000).unwrap();
        db
    }

    fn marks(source: &str, id: &str, name: &str) -> Marks {
        Marks { source: source.into(), source_id: id.into(), name: name.into(), ..Marks::default() }
    }

    fn plan() -> Plan {
        Plan {
            from: PathBuf::from("Playnite").join("library"),
            games_read: 7,
            games: vec![
                // Less than the library has: nothing.
                Marks { playtime_minutes: 300, last_played: NOW - 5000, ..marks(STEAM, "10", "Alder") },
                Marks { playtime_minutes: 754, last_played: NOW - 50, ..marks(STEAM, "20", "Birch") },
                Marks { favourite: true, hidden: true, ..marks(STEAM, "30", "Cedar") },
                // The same name in another store is another game; one the library has not, left.
                Marks { favourite: true, ..marks("epic", "alder", "Alder") },
                Marks { playtime_minutes: 60, ..marks(STEAM, "99", "Ghost") },
                // The store's id, in another case.
                Marks { favourite: true, ..marks("epic", "elm", "Elm") },
            ],
            other_stores: 1,
            groups: vec![
                Group {
                    id: "rule-1".into(),
                    name: "Installed on Steam".into(),
                    picks: Picks::Rule(Rule { installed: Some(true), sources: vec![STEAM.into()], ..Rule::default() }),
                    sort: Sort::Name,
                    descending: false,
                },
                Group {
                    id: "games-2".into(),
                    name: "Couch picks".into(),
                    picks: Picks::Games(vec![(STEAM.into(), "20".into()), (STEAM.into(), "99".into())]),
                    sort: Sort::LastPlayed,
                    descending: true,
                },
                Group {
                    id: "games-3".into(),
                    name: "most played".into(),
                    picks: Picks::Games(Vec::new()),
                    sort: Sort::Name,
                    descending: false,
                },
            ],
            skipped: vec![("Released in 2019".into(), "it filters by release year".into())],
        }
    }

    fn shown(db: &Database) -> Vec<(String, Vec<String>)> {
        let filters = db.filters_at(NOW).unwrap();
        let name = |id: i64| db.game(id).unwrap().unwrap().name;
        filters.iter().filter(|f| f.shown).map(|f| (f.name.clone(), f.games.iter().map(|&g| name(g)).collect())).collect()
    }

    #[test]
    fn the_preview_is_what_the_import_brings_and_changes_nothing() {
        let mut db = library();
        let before = db.snapshot_at(NOW).unwrap();
        let preview = db.import_preview(&plan()).unwrap();
        assert_eq!(db.snapshot_at(NOW).unwrap(), before);
        let wanted = Preview {
            from: PathBuf::from("Playnite").join("library").display().to_string(),
            games_read: 7,
            matched: 4,
            playtime: 1,
            last_played: 1,
            favourites: 2,
            hidden: 1,
            groups: vec!["Installed on Steam".into(), "Couch picks".into()],
            not_here: 2,
            other_stores: 1,
            skipped: vec![
                ("Released in 2019".into(), "it filters by release year".into()),
                ("most played".into(), "a Group of that name is here already".into()),
            ],
        };
        assert_eq!(preview, wanted);

        let imported = db.import(&plan(), NOW).unwrap();
        assert_eq!(imported.preview, wanted);
        assert!(imported.lines.contains(&"import: Birch (steam 20): play time 754 min, was 0 min".to_string()));
        assert!(imported.lines.contains(&"import: Cedar (steam 30): a favourite".to_string()));
        assert_eq!(db.setting(IMPORTED).unwrap(), Some(NOW.to_string()));
    }

    #[test]
    fn an_import_brings_its_marks_by_store_and_id_and_its_groups() {
        let mut db = library();
        db.import(&plan(), NOW).unwrap();
        let snapshot = db.snapshot_at(NOW).unwrap();
        let get = |name: &str| snapshot.games.iter().chain(&snapshot.hidden).find(|g| g.name == name).unwrap().clone();
        let (alder, birch, cedar, elm) = (get("Alder"), get("Birch"), get("Cedar"), get("Elm"));
        assert_eq!((alder.playtime_minutes, alder.last_played, alder.favourite), (600, NOW - 100, false));
        assert_eq!((birch.playtime_minutes, birch.last_played), (754, NOW - 50));
        assert!(cedar.favourite && cedar.hidden && elm.favourite);
        // Birch was played last: first in the Recent row.
        assert_eq!(db.recent(1).unwrap()[0].name, "Birch");

        let groups = shown(&db);
        let last_two = &groups[groups.len() - 2..];
        assert_eq!(last_two[0], ("Installed on Steam".to_string(), vec!["Alder".to_string(), "Birch".to_string()]));
        assert_eq!(last_two[1], ("Couch picks".to_string(), vec!["Birch".to_string()]));
    }

    #[test]
    fn an_import_run_again_doubles_nothing() {
        let mut db = library();
        db.import(&plan(), NOW).unwrap();
        let once = db.snapshot_at(NOW).unwrap();
        let again = db.import(&plan(), NOW + 60).unwrap();
        assert!(again.preview.is_empty(), "{:?}", again.preview);
        assert_eq!(again.lines, ["import: Group \"most played\" not brought: a Group of that name is here already"]);
        assert_eq!(db.snapshot_at(NOW).unwrap(), once);

        // What the user did since stays: a Group from a rule deleted is not brought back; a favourite taken away
        // in the Dashboard that Playnite still has is one again, as the import says it brings.
        let installed = db.filters().unwrap().into_iter().find(|f| f.name == "Installed on Steam").unwrap();
        db.delete_filter(installed.id).unwrap();
        let mut later = plan();
        // Playnite has since put another game in Couch picks, and counted more of Birch.
        let Picks::Games(couch) = &mut later.groups[1].picks else { panic!() };
        couch.push((STEAM.into(), "10".into()));
        later.games[1].playtime_minutes = 800;
        let third = db.import(&later, NOW + 120).unwrap();
        assert_eq!((third.preview.playtime, third.preview.groups.as_slice()), (1, &["Couch picks".to_string()][..]));
        let groups = shown(&db);
        assert!(groups.iter().all(|(name, _)| name != "Installed on Steam"));
        assert_eq!(groups.last().unwrap().1, ["Birch", "Alder"]);
    }
}
