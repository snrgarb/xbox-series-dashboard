//! What the user adds by hand (docs/spec/apps.md): games of the source `manual`, and Apps.
//!
//! A game added by hand is a row of `games` like any store's, `source` [`MANUAL`] and `source_id` the program's full
//! path, so the same program is never two games; its install folder is the program's, its art a `cover` row of
//! `art`. Its source, [`Manual`], reads nothing: what there is of it is what the user put in the database, which no
//! sync touches, and it starts the program.
//!
//! An App is a row of `apps` (migration 4): a program by its path, or a Microsoft Store app by its AUMID, its name
//! and its picture. It is not a game, and so is in no list of games: the snapshot has the Apps apart
//! (`Library::apps`), in the order they were added.
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use model::by_hand::{self, MANUAL};
use model::{Added, App, AppKind, Ask, ByHand, Found, Game, Source, Start};
use rusqlite::{params, OptionalExtension};

use crate::{sort_name, text, Database};

/// Migration 4: the Apps.
pub const MIGRATION: &str = "CREATE TABLE apps (
        id INTEGER PRIMARY KEY,
        -- 'program': target is its full path. 'store': a Microsoft Store app, target its AUMID.
        kind TEXT NOT NULL,
        target TEXT NOT NULL,
        name TEXT NOT NULL,
        -- Its picture's file, if it has one.
        icon TEXT,
        -- Unix seconds: when it was added.
        added INTEGER NOT NULL,
        UNIQUE (kind, target)
    );";

/// The games added by hand, as a source: it reads nothing, and starts the program a game is.
#[derive(Debug, Default, Clone, Copy)]
pub struct Manual;

impl Source for Manual {
    fn key(&self) -> &'static str {
        MANUAL
    }

    fn name(&self) -> &'static str {
        "Added by hand"
    }

    /// Nothing to read: the games are the database's own, and stay as they are.
    fn read(&self) -> Result<Option<Found>> {
        Ok(None)
    }

    fn start(&self, game: &Game) -> Option<Start> {
        (game.source == MANUAL && !game.source_id.is_empty()).then(|| by_hand::program(Path::new(&game.source_id)))
    }

    /// No store to ask: a game added by hand is removed in Settings, and never installed or uninstalled.
    fn ask(&self, _game: &Game, _ask: Ask) -> Option<Start> {
        None
    }
}

impl Database {
    /// Carries out a change Settings' Games & apps pages asked for, at the time `now`. Gives what the log says of
    /// it: `added the game Moss`, `removed the App Notepad`.
    pub fn by_hand(&mut self, change: &ByHand, now: u64) -> Result<String> {
        match change {
            ByHand::AddGame { path, name, art } => {
                let name = named(name)?;
                let source_id = text(path);
                let known: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT name FROM games WHERE source = ?1 AND source_id = ?2",
                        [MANUAL, &source_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(known) = known {
                    bail!("{} is in the library already, as {known}", path.display());
                }
                let tx = self.conn.transaction()?;
                tx.execute(
                    "INSERT INTO games (source, source_id, name, sort_name, installed, install_dir, added)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                    params![MANUAL, source_id, name, sort_name(name), path.parent().map(text), now as i64],
                )?;
                let id = tx.last_insert_rowid();
                if let Some(art) = art {
                    tx.execute("INSERT INTO art (game, kind, path) VALUES (?1, 'cover', ?2)", params![id, text(art)])?;
                }
                tx.commit()?;
                Ok(format!("added the game {name}"))
            }
            ByHand::AddApp { path, name, icon } => {
                self.add_app(AppKind::Program, &text(path), named(name)?, icon.as_deref(), now)
            }
            ByHand::AddStoreApp(app) => {
                self.add_app(AppKind::Store, &app.aumid, named(&app.name)?, app.logo.as_deref(), now)
            }
            ByHand::Rename { item, name } => {
                let name = named(name)?;
                let was = self.name_of(*item)?;
                match *item {
                    Added::Game(id) => self.conn.execute(
                        "UPDATE games SET name = ?2, sort_name = ?3 WHERE id = ?1 AND source = ?4",
                        params![id, name, sort_name(name), MANUAL],
                    )?,
                    Added::App(id) => {
                        self.conn.execute("UPDATE apps SET name = ?2 WHERE id = ?1", params![id, name])?
                    }
                };
                Ok(format!("renamed {was} {name}"))
            }
            ByHand::Remove(item) => {
                let was = self.name_of(*item)?;
                match *item {
                    // Its art and Play sessions go with it.
                    Added::Game(id) => {
                        self.conn.execute("DELETE FROM games WHERE id = ?1 AND source = ?2", params![id, MANUAL])?
                    }
                    Added::App(id) => self.conn.execute("DELETE FROM apps WHERE id = ?1", [id])?,
                };
                Ok(match item {
                    Added::Game(_) => format!("removed the game {was}"),
                    Added::App(_) => format!("removed the App {was}"),
                })
            }
        }
    }

    fn add_app(&mut self, kind: AppKind, target: &str, name: &str, icon: Option<&Path>, now: u64) -> Result<String> {
        let known: Option<String> = self
            .conn
            .query_row("SELECT name FROM apps WHERE kind = ?1 AND target = ?2", [kind.key(), target], |row| row.get(0))
            .optional()?;
        if let Some(known) = known {
            bail!("{target} is an App already, as {known}");
        }
        self.conn.execute(
            "INSERT INTO apps (kind, target, name, icon, added) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind.key(), target, name, icon.map(text), now as i64],
        )?;
        Ok(format!("added the App {name}"))
    }

    /// The name of a game added by hand or an App; an error if there is no such one.
    fn name_of(&self, item: Added) -> Result<String> {
        let name: Option<String> = match item {
            Added::Game(id) => self
                .conn
                .query_row("SELECT name FROM games WHERE id = ?1 AND source = ?2", params![id, MANUAL], |row| {
                    row.get(0)
                })
                .optional()?,
            Added::App(id) => {
                self.conn.query_row("SELECT name FROM apps WHERE id = ?1", [id], |row| row.get(0)).optional()?
            }
        };
        name.with_context(|| format!("no {item:?} added by hand"))
    }

    /// Every App, in the order they were added.
    pub fn apps(&self) -> Result<Vec<App>> {
        let mut rows = self.conn.prepare("SELECT id, kind, target, name, icon FROM apps ORDER BY added, id")?;
        let rows = rows.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        let mut apps = Vec::new();
        for row in rows {
            let (id, kind, target, name, icon) = row?;
            // A kind this program does not know is passed over.
            let Some(kind) = AppKind::from_key(&kind) else { continue };
            apps.push(App { id, name, kind, target, icon: icon.map(PathBuf::from) });
        }
        Ok(apps)
    }
}

/// A name with the spaces round it gone; an error for one with nothing left.
fn named(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        bail!("a name is needed");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use model::StoreApp;

    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn add_game(db: &mut Database, path: &str, name: &str) -> i64 {
        let change = ByHand::AddGame { path: path.into(), name: name.into(), art: Some("art/moss.png".into()) };
        assert_eq!(db.by_hand(&change, NOW).unwrap(), format!("added the game {}", name.trim()));
        db.snapshot().unwrap().games.iter().find(|g| g.source_id == path).unwrap().id
    }

    #[test]
    fn a_game_added_by_hand_is_a_game_of_its_own_source_started_as_its_program() {
        let mut db = Database::in_memory().unwrap();
        let path = r"C:\Games\Moss\moss.exe";
        let id = add_game(&mut db, path, "  Moss ");
        let game = db.game(id).unwrap().unwrap();
        assert_eq!((game.source.as_str(), game.name.as_str(), game.added), (MANUAL, "Moss", NOW));
        assert!(game.installed);
        assert_eq!(game.install_dir.as_deref(), Some(Path::new(r"C:\Games\Moss\moss.exe").parent().unwrap()));
        assert_eq!(game.cover.as_deref(), Some(Path::new("art/moss.png")));
        assert_eq!(Manual.start(&game), Some(by_hand::program(Path::new(path))));
        assert_eq!((Manual.ask(&game, Ask::Uninstall), Manual.read().unwrap()), (None, None));
        // Not another source's game.
        assert_eq!(Manual.start(&Game { source: "steam".into(), ..game }), None);
        // The same program twice is refused; a name of nothing too.
        let again = ByHand::AddGame { path: path.into(), name: "Moss again".into(), art: None };
        assert!(db.by_hand(&again, NOW).is_err());
        let unnamed = ByHand::AddGame { path: r"C:\b.exe".into(), name: "  ".into(), art: None };
        assert!(db.by_hand(&unnamed, NOW).is_err());
        assert_eq!(db.snapshot().unwrap().games.len(), 1);
    }

    #[test]
    fn a_game_added_by_hand_is_renamed_played_and_removed_with_its_sessions() {
        let mut db = Database::in_memory().unwrap();
        let id = add_game(&mut db, r"C:\Games\Moss\moss.exe", "Moss");
        let rename = ByHand::Rename { item: Added::Game(id), name: "The Moss".into() };
        assert_eq!(db.by_hand(&rename, NOW).unwrap(), "renamed Moss The Moss");
        let session = db.session_started(id, NOW + 10).unwrap();
        db.session_ended(session, NOW + 130).unwrap();
        let played = db.recent(8).unwrap();
        assert_eq!(played.iter().map(|g| (g.id, g.name.as_str())).collect::<Vec<_>>(), [(id, "The Moss")]);
        assert_eq!(played[0].playtime_minutes, 2);

        // A sync of every source (Manual reads nothing) leaves it as it is.
        let read = crate::sources::read(&[std::sync::Arc::new(Manual) as std::sync::Arc<dyn Source>]);
        assert!(read.found.is_empty() && read.failed.is_empty());
        db.sync_all(&read, NOW + 200).unwrap();
        assert_eq!(db.game(id).unwrap().unwrap().name, "The Moss");

        assert_eq!(db.by_hand(&ByHand::Remove(Added::Game(id)), NOW).unwrap(), "removed the game The Moss");
        assert_eq!(db.game(id).unwrap(), None);
        assert!(db.sessions(id).unwrap().is_empty());
        let snapshot = db.snapshot().unwrap();
        assert!(snapshot.games.is_empty() && snapshot.history.is_empty());
        // Gone, it can be neither renamed nor removed again.
        assert!(db.by_hand(&ByHand::Remove(Added::Game(id)), NOW).is_err());
    }

    #[test]
    fn a_stores_game_is_never_renamed_or_removed_as_one_added_by_hand() {
        let mut db = Database::in_memory().unwrap();
        let steam = Game { source_id: "440".into(), name: "Tern".into(), installed: true, ..Game::default() };
        db.sync("steam", &model::Library { games: vec![steam], ..Default::default() }, NOW).unwrap();
        let id = db.snapshot().unwrap().games[0].id;
        assert!(db.by_hand(&ByHand::Rename { item: Added::Game(id), name: "Mine".into() }, NOW).is_err());
        assert!(db.by_hand(&ByHand::Remove(Added::Game(id)), NOW).is_err());
        assert_eq!(db.game(id).unwrap().unwrap().name, "Tern");
    }

    #[test]
    fn apps_are_kept_apart_from_the_games_in_the_order_added() {
        let mut db = Database::in_memory().unwrap();
        let notepad =
            ByHand::AddApp { path: r"C:\Windows\System32\notepad.exe".into(), name: "Notepad".into(), icon: None };
        assert_eq!(db.by_hand(&notepad, NOW).unwrap(), "added the App Notepad");
        let calculator = StoreApp {
            name: "Calculator".into(),
            aumid: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(),
            logo: Some("logo.png".into()),
        };
        db.by_hand(&ByHand::AddStoreApp(calculator.clone()), NOW + 1).unwrap();
        assert!(db.by_hand(&ByHand::AddStoreApp(calculator), NOW + 2).is_err(), "ticked twice is one App");
        let snapshot = db.snapshot().unwrap();
        assert!(snapshot.games.is_empty() && snapshot.history.is_empty());
        assert!(snapshot.filters.iter().all(|filter| filter.games.is_empty()));
        let apps: Vec<(AppKind, &str, Option<&Path>)> =
            snapshot.apps.iter().map(|app| (app.kind, app.name.as_str(), app.icon.as_deref())).collect();
        assert_eq!(
            apps,
            [(AppKind::Program, "Notepad", None), (AppKind::Store, "Calculator", Some(Path::new("logo.png")))]
        );
        let (first, second) = (snapshot.apps[0].id, snapshot.apps[1].id);
        db.by_hand(&ByHand::Rename { item: Added::App(first), name: "Notes".into() }, NOW).unwrap();
        assert_eq!(db.by_hand(&ByHand::Remove(Added::App(second)), NOW).unwrap(), "removed the App Calculator");
        let names: Vec<String> = db.apps().unwrap().into_iter().map(|app| app.name).collect();
        assert_eq!(names, ["Notes"]);
    }

    /// What is added by hand is in the database's file: the next Dashboard to open it has it all.
    #[test]
    fn what_is_added_by_hand_outlives_the_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(crate::FILE);
        let before = {
            let mut db = Database::open(&path).unwrap();
            add_game(&mut db, r"C:\Games\Moss\moss.exe", "Moss");
            let app =
                ByHand::AddApp { path: r"C:\Tools\tool.exe".into(), name: "Tool".into(), icon: Some("t.png".into()) };
            db.by_hand(&app, NOW).unwrap();
            db.snapshot_at(NOW).unwrap()
        };
        let db = Database::open(&path).unwrap();
        let after = db.snapshot_at(NOW).unwrap();
        assert_eq!(after, before);
        assert_eq!((after.games.len(), after.apps.len()), (1, 1));
    }
}
