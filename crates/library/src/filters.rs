//! Saved filters: which there are, the games each picks, and what the user and a sync may change of them.
//! docs/spec/filters.md is the design; sections 3, 5 and 7 are what is here.
//!
//! A filter's place is `filters.position`, lowest first and `id` between equals. Places are handed out `STEP` apart
//! so one can go between two others without moving them; when there is no room left between two, every filter is
//! given a new place, `STEP` apart again, in the order they were in.
use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use model::{Collection, Filter, FilterChange, FilterKind, Played, Rule, Sort};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};

use crate::{now, Database};

/// The built-in filters' keys, which `model::Filter::key` carries.
pub const FAVOURITES: &str = "favourites";
pub const INSTALLED: &str = "installed";
pub const MOST_PLAYED: &str = "most-played";
pub const RECENTLY_ADDED: &str = "recently-added";
pub const HIDDEN: &str = "hidden";

/// How far apart places are handed out.
const STEP: i64 = 1000;
const DAY: i64 = 86_400;

/// Migration 2: a store's collections, and the saved filters with the five built in.
pub(crate) const MIGRATION: &str = "
    -- A store's own grouping of games: a Steam collection.
    CREATE TABLE collections (
        id INTEGER PRIMARY KEY,
        source TEXT NOT NULL,
        -- The store's own id for it (Steam's 'uc-...').
        source_id TEXT NOT NULL,
        name TEXT NOT NULL,
        UNIQUE (source, source_id)
    );
    CREATE TABLE collection_games (
        collection INTEGER NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
        game INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        PRIMARY KEY (collection, game)
    ) WITHOUT ROWID;

    -- A saved filter: a Group on Home and a tab on Full library while it is shown.
    CREATE TABLE filters (
        id INTEGER PRIMARY KEY,
        -- builtin, collection or custom.
        kind TEXT NOT NULL,
        -- A builtin's key ('favourites', 'installed', 'most-played', 'recently-added', 'hidden'); NULL otherwise.
        key TEXT UNIQUE,
        -- A collection filter's collection; NULL otherwise. The filter goes with it.
        collection INTEGER UNIQUE REFERENCES collections (id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        -- The rule, as JSON: an object of the clauses that are set. '{}' matches every game.
        rule TEXT NOT NULL DEFAULT '{}',
        -- name, last_played, added or playtime.
        sort TEXT NOT NULL DEFAULT 'name',
        descending INTEGER NOT NULL DEFAULT 0,
        shown INTEGER NOT NULL DEFAULT 1,
        -- Its place among the filters, lowest first. Not unique: ties fall back to id.
        position INTEGER NOT NULL
    );

    INSERT INTO filters (kind, key, name, rule, sort, descending, shown, position) VALUES
        ('builtin', 'favourites',     'Favourites',     '{\"favourite\":true}',  'name',     0, 1, 1000),
        ('builtin', 'installed',      'Installed',      '{\"installed\":true}',  'name',     0, 0, 2000),
        ('builtin', 'most-played',    'Most played',    '{\"played\":\"played\"}', 'playtime', 1, 1, 3000),
        ('builtin', 'recently-added', 'Recently added', '{}',                  'added',    1, 1, 4000),
        ('builtin', 'hidden',         'Hidden',         '{\"hidden\":true}',     'name',     0, 0, 5000);";

fn kind_text(kind: FilterKind) -> &'static str {
    match kind {
        FilterKind::Builtin => "builtin",
        FilterKind::Collection => "collection",
        FilterKind::Custom => "custom",
    }
}

fn sort_text(sort: Sort) -> &'static str {
    match sort {
        Sort::Name => "name",
        Sort::LastPlayed => "last_played",
        Sort::Added => "added",
        Sort::Playtime => "playtime",
    }
}

/// A sort as the database names it. One it does not know sorts by name, as a preset with a sort the Bundle does
/// not support does there.
fn sort_of(text: &str) -> Sort {
    Sort::ALL.into_iter().find(|&sort| sort_text(sort) == text).unwrap_or_default()
}

/// The `ORDER BY` of a sort. A game without the field (never played) comes last in either direction, and games
/// level in it are in name order, upward.
fn order(sort: Sort, descending: bool) -> String {
    let dir = if descending { "DESC" } else { "ASC" };
    match sort {
        Sort::Name => format!("sort_name {dir}, name {dir}, id {dir}"),
        Sort::LastPlayed => format!("last_played = 0, last_played {dir}, sort_name, name, id"),
        // Every game has one.
        Sort::Added => format!("added {dir}, sort_name, name, id"),
        Sort::Playtime => format!("playtime_minutes = 0, playtime_minutes {dir}, sort_name, name, id"),
    }
}

/// The `WHERE` of a rule, with the values its `?`s stand for.
fn clauses(rule: &Rule, collection: Option<i64>, now: i64) -> (String, Vec<Value>) {
    let mut clauses = vec!["gone = 0".to_string(), format!("hidden = {}", i64::from(rule.hidden))];
    let mut values: Vec<Value> = Vec::new();
    let mut clause = |text: &str, value: i64| {
        clauses.push(text.to_string());
        values.push(Value::Integer(value));
    };
    if let Some(installed) = rule.installed {
        clause("installed = ?", installed.into());
    }
    if let Some(favourite) = rule.favourite {
        clause("favourite = ?", favourite.into());
    }
    match rule.played {
        Some(Played::Played) => clause("last_played > ?", 0),
        Some(Played::Never) => clause("last_played = ?", 0),
        None => {}
    }
    if let Some(days) = rule.played_within_days {
        // Never played is 0, which a far-off day could reach: it is no day at all.
        clause("last_played > 0 AND last_played >= ?", now - i64::from(days) * DAY);
    }
    if let Some(days) = rule.added_within_days {
        clause("added >= ?", now - i64::from(days) * DAY);
    }
    if let Some(hours) = rule.min_playtime_hours {
        clause("playtime_minutes >= ?", i64::from(hours) * 60);
    }
    if let Some(collection) = collection {
        clause("id IN (SELECT game FROM collection_games WHERE collection = ?)", collection);
    }
    if !rule.sources.is_empty() {
        clauses.push(format!("source IN ({})", vec!["?"; rule.sources.len()].join(", ")));
        values.extend(rule.sources.iter().cloned().map(Value::Text));
    }
    (clauses.join(" AND "), values)
}

/// Every filter in order, each with the games it picks at the time `now`.
pub(crate) fn all(conn: &Connection, now: u64) -> Result<Vec<Filter>> {
    let mut rows = conn.prepare(
        "SELECT id, kind, key, collection, name, rule, sort, descending, shown FROM filters ORDER BY position, id",
    )?;
    let rows = rows.query_map([], |row| {
        let kind = match row.get::<_, String>(1)?.as_str() {
            "builtin" => FilterKind::Builtin,
            "collection" => FilterKind::Collection,
            _ => FilterKind::Custom,
        };
        let filter = Filter {
            id: row.get(0)?,
            kind,
            key: row.get(2)?,
            name: row.get(4)?,
            shown: row.get(8)?,
            rule: Rule::default(),
            sort: sort_of(&row.get::<_, String>(6)?),
            descending: row.get(7)?,
            games: Vec::new(),
        };
        Ok((filter, row.get::<_, Option<i64>>(3)?, row.get::<_, String>(5)?))
    })?;
    let mut filters = Vec::new();
    for row in rows.collect::<rusqlite::Result<Vec<_>>>()? {
        let (mut filter, collection, rule) = row;
        match serde_json::from_str::<Rule>(&rule) {
            Ok(rule) => {
                let (clauses, values) = clauses(&rule, collection, now as i64);
                let order = order(filter.sort, filter.descending);
                let mut games = conn.prepare(&format!("SELECT id FROM games WHERE {clauses} ORDER BY {order}"))?;
                let games = games.query_map(params_from_iter(values), |row| row.get(0))?;
                filter.games = games.collect::<rusqlite::Result<_>>()?;
                filter.rule = rule;
            }
            // A clause from a newer program, or not a rule at all: it picks nothing rather than too much.
            Err(e) => eprintln!("library: filter {}'s rule is not one this program knows: {e}", filter.id),
        }
        filters.push(filter);
    }
    Ok(filters)
}

/// A filter's place.
struct Place {
    id: i64,
    kind: String,
    key: Option<String>,
    position: i64,
}

fn places(conn: &Connection) -> Result<Vec<Place>> {
    let mut rows = conn.prepare("SELECT id, kind, key, position FROM filters ORDER BY position, id")?;
    let rows = rows.query_map([], |row| {
        Ok(Place { id: row.get(0)?, kind: row.get(1)?, key: row.get(2)?, position: row.get(3)? })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Gives every filter a new place, `STEP` apart, in the order of `places`.
fn renumber(conn: &Connection, places: &mut [Place]) -> Result<()> {
    for (i, place) in places.iter_mut().enumerate() {
        place.position = (i as i64 + 1) * STEP;
        conn.execute("UPDATE filters SET position = ?2 WHERE id = ?1", params![place.id, place.position])?;
    }
    Ok(())
}

/// The place for a collection's filter seen for the first time: after the last collection's filter, or after
/// Favourites if there is none yet, and before whatever follows that.
fn place_for_collection(conn: &Connection) -> Result<i64> {
    let mut places = places(conn)?;
    let last_collection = places.iter().rposition(|p| p.kind == kind_text(FilterKind::Collection));
    let favourites = || places.iter().position(|p| p.key.as_deref() == Some(FAVOURITES));
    let Some(after) = last_collection.or_else(favourites) else {
        return Ok(places.last().map_or(STEP, |last| last.position + STEP));
    };
    if after + 1 == places.len() {
        return Ok(places[after].position + STEP);
    }
    if places[after + 1].position - places[after].position < 2 {
        renumber(conn, &mut places)?;
    }
    Ok((places[after].position + places[after + 1].position) / 2)
}

/// Takes in a source's collections as it has just been read (docs/spec/filters.md, section 5.2): one not seen
/// before gets a row and a filter, shown; one renamed renames both; one the source no longer has goes, and its
/// filter with it. Each collection's games are made the source's. Whether a collection's filter is shown and where
/// it is in the order are the user's, and are left alone.
pub(crate) fn sync_collections(conn: &Connection, source: &str, found: &[Collection]) -> Result<()> {
    let mut known: HashMap<String, (i64, String)> = HashMap::new();
    {
        let mut rows = conn.prepare("SELECT id, source_id, name FROM collections WHERE source = ?1")?;
        let mut rows = rows.query([source])?;
        while let Some(row) = rows.next()? {
            known.insert(row.get(1)?, (row.get(0)?, row.get(2)?));
        }
    }
    let mut game_of = conn.prepare("SELECT id FROM games WHERE source = ?1 AND source_id = ?2")?;
    let mut games_in = conn.prepare("SELECT game FROM collection_games WHERE collection = ?1 ORDER BY game")?;
    for collection in found {
        let id = match known.remove(&collection.source_id) {
            Some((id, name)) => {
                if name != collection.name {
                    conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, collection.name])?;
                    conn.execute("UPDATE filters SET name = ?2 WHERE collection = ?1", params![id, collection.name])?;
                }
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO collections (source, source_id, name) VALUES (?1, ?2, ?3)",
                    params![source, collection.source_id, collection.name],
                )?;
                let id = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO filters (kind, collection, name, position) VALUES ('collection', ?1, ?2, ?3)",
                    params![id, collection.name, place_for_collection(conn)?],
                )?;
                id
            }
        };
        // An id that is no game of the library's (a tool, DLC, a game no longer owned) is passed over.
        let mut games = Vec::new();
        for source_id in &collection.games {
            let game: Option<i64> = game_of.query_row(params![source, source_id], |row| row.get(0)).optional()?;
            games.extend(game);
        }
        games.sort_unstable();
        games.dedup();
        let had: Vec<i64> = games_in.query_map([id], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
        if had != games {
            conn.execute("DELETE FROM collection_games WHERE collection = ?1", [id])?;
            for game in games {
                conn.execute("INSERT INTO collection_games (collection, game) VALUES (?1, ?2)", params![id, game])?;
            }
        }
    }
    // What is left is no longer the source's: its filter and its games go with it.
    for (id, _) in known.values() {
        conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    }
    Ok(())
}

impl Database {
    /// Every filter, shown or not, in the user's order, each with the games it picks in its sort.
    pub fn filters(&self) -> Result<Vec<Filter>> {
        all(&self.conn, now())
    }

    /// `filters`, with the rules that count days counting them back from `now`.
    pub fn filters_at(&self, now: u64) -> Result<Vec<Filter>> {
        all(&self.conn, now)
    }

    /// Whether the filter is a Group on Home and a tab on Full library. Any filter's can be set.
    pub fn set_filter_shown(&self, filter: i64, shown: bool) -> Result<()> {
        self.conn.execute("UPDATE filters SET shown = ?2 WHERE id = ?1", params![filter, shown])?;
        Ok(())
    }

    /// Moves a filter `by` places among all the filters, shown or not: -1 is one up, +1 one down. It stops at the
    /// ends. The filters it passes each move one place the other way; no other filter's place changes.
    pub fn move_filter(&mut self, filter: i64, by: i32) -> Result<()> {
        let tx = self.conn.transaction()?;
        let mut places = places(&tx)?;
        let from = places.iter().position(|p| p.id == filter).with_context(|| format!("no filter {filter}"))?;
        let to = (from as i64 + i64::from(by)).clamp(0, places.len() as i64 - 1) as usize;
        if to != from {
            let (low, high) = (from.min(to), from.max(to));
            // Places that are not all different could not be told apart once handed round.
            if places[low..=high].windows(2).any(|pair| pair[0].position == pair[1].position) {
                renumber(&tx, &mut places)?;
            }
            let positions: Vec<i64> = places[low..=high].iter().map(|p| p.position).collect();
            if to > from {
                places[low..=high].rotate_left(1);
            } else {
                places[low..=high].rotate_right(1);
            }
            for (place, position) in places[low..=high].iter().zip(positions) {
                tx.execute("UPDATE filters SET position = ?2 WHERE id = ?1", params![place.id, position])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Makes a custom filter of this name: no clauses, so every game; by name; shown; last in the order. Gives its
    /// id.
    pub fn add_filter(&mut self, name: &str) -> Result<i64> {
        let name = filter_name(name)?;
        let tx = self.conn.transaction()?;
        let last: Option<i64> = tx.query_row("SELECT MAX(position) FROM filters", [], |row| row.get(0))?;
        tx.execute(
            "INSERT INTO filters (kind, name, position) VALUES ('custom', ?1, ?2)",
            params![name, last.unwrap_or(0) + STEP],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// Changes a custom filter: its name, what it picks and how it sorts. A built-in's and a collection's are not
    /// the user's to change, and asking is an error.
    pub fn set_filter(&self, filter: i64, name: &str, rule: &Rule, sort: Sort, descending: bool) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE filters SET name = ?2, rule = ?3, sort = ?4, descending = ?5 WHERE id = ?1 AND kind = 'custom'",
            params![filter, filter_name(name)?, serde_json::to_string(rule)?, sort_text(sort), descending],
        )?;
        if changed == 0 {
            bail!("filter {filter} is not a custom filter");
        }
        Ok(())
    }

    /// Deletes a custom filter. A built-in or a collection's cannot be deleted, and asking is an error.
    pub fn delete_filter(&self, filter: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM filters WHERE id = ?1 AND kind = 'custom'", [filter])? == 0 {
            bail!("filter {filter} is not a custom filter");
        }
        Ok(())
    }

    /// Carries out a change Settings asked for, by the setter that is its own. An `Add` gives the new filter's id.
    pub fn change_filter(&mut self, change: &FilterChange) -> Result<Option<i64>> {
        match change {
            FilterChange::Shown { filter, shown } => self.set_filter_shown(*filter, *shown)?,
            FilterChange::Move { filter, by } => self.move_filter(*filter, *by)?,
            FilterChange::Add { name } => return self.add_filter(name).map(Some),
            FilterChange::Set { filter, name, rule, sort, descending } => {
                self.set_filter(*filter, name, rule, *sort, *descending)?
            }
            FilterChange::Delete { filter } => self.delete_filter(*filter)?,
        }
        Ok(None)
    }
}

/// A filter's name as it is kept: without the spaces around it, and not empty.
fn filter_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        bail!("a filter needs a name");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::STEAM;
    use model::{Game, Library};
    use source_steam::fixture::{self, Fixture};

    const NOW: u64 = 1_800_000_000;
    const DAYS: u64 = 86_400;

    fn game(appid: u32, name: &str, played_days_ago: Option<u64>, playtime_minutes: u32, installed: bool) -> Game {
        Game {
            source_id: appid.to_string(),
            name: name.into(),
            last_played: played_days_ago.map_or(0, |days| NOW - days * DAYS),
            playtime_minutes,
            installed,
            ..Game::default()
        }
    }

    fn library(games: Vec<Game>) -> Library {
        Library { games, ..Library::default() }
    }

    /// Five games. Alder, Birch and Cedar came in a hundred days ago, The Dogwood and Elm two days ago. Elm is
    /// another store's. Cedar is a favourite.
    ///
    /// | game        | installed | last played  | played  |
    /// |-------------|-----------|--------------|---------|
    /// | Alder       | yes       | yesterday    | 10 h    |
    /// | Birch       | no        | 40 days ago  | 30 min  |
    /// | Cedar       | yes       | never        |         |
    /// | The Dogwood | yes       | 400 days ago | 100 h   |
    /// | Elm         | no        | never        |         |
    fn woods() -> Database {
        let mut db = Database::in_memory().unwrap();
        let mut steam = vec![
            game(1, "Alder", Some(1), 600, true),
            game(2, "Birch", Some(40), 30, false),
            game(3, "Cedar", None, 0, true),
        ];
        db.sync(STEAM, &library(steam.clone()), NOW - 100 * DAYS).unwrap();
        steam.push(game(4, "The Dogwood", Some(400), 6000, true));
        db.sync(STEAM, &library(steam), NOW - 2 * DAYS).unwrap();
        db.sync("gog", &library(vec![game(0, "Elm", None, 0, false)]), NOW - 2 * DAYS).unwrap();
        db.set_favourite(id_of(&db, "Cedar"), true).unwrap();
        db
    }

    fn id_of(db: &Database, name: &str) -> i64 {
        db.conn.query_row("SELECT id FROM games WHERE name = ?1", [name], |row| row.get(0)).unwrap()
    }

    /// The names of a filter's games, in its order.
    fn names(db: &Database, filter: &Filter) -> Vec<String> {
        filter.games.iter().map(|&id| db.game(id).unwrap().unwrap().name).collect()
    }

    fn filter(db: &Database, id: i64) -> Filter {
        db.filters_at(NOW).unwrap().into_iter().find(|f| f.id == id).unwrap()
    }

    fn keyed(db: &Database, key: &str) -> Filter {
        db.filters_at(NOW).unwrap().into_iter().find(|f| f.key.as_deref() == Some(key)).unwrap()
    }

    /// What a custom filter with this rule and sort picks.
    fn picks(db: &mut Database, rule: &Rule, sort: Sort, descending: bool) -> Vec<String> {
        let id = db.add_filter("Probe").unwrap();
        db.set_filter(id, "Probe", rule, sort, descending).unwrap();
        let picked = names(db, &filter(db, id));
        db.delete_filter(id).unwrap();
        picked
    }

    fn by_name(db: &mut Database, rule: Rule) -> Vec<String> {
        picks(db, &rule, Sort::Name, false)
    }

    /// The filters' names in order, a shown one's as it is and one not shown in brackets.
    fn order_of(db: &Database) -> Vec<String> {
        let filters = db.filters_at(NOW).unwrap();
        filters.iter().map(|f| if f.shown { f.name.clone() } else { format!("({})", f.name) }).collect()
    }

    fn collection(id: &str, name: &str, appids: &[u32]) -> Collection {
        Collection { source_id: id.into(), name: name.into(), games: appids.iter().map(u32::to_string).collect() }
    }

    /// The woods' Steam games as Steam would be read again, with these collections.
    fn steam_with(collections: Option<Vec<Collection>>) -> Library {
        let games = vec![
            game(1, "Alder", Some(1), 600, true),
            game(2, "Birch", Some(40), 30, false),
            game(3, "Cedar", None, 0, true),
            game(4, "The Dogwood", Some(400), 6000, true),
        ];
        Library { games, collections, ..Library::default() }
    }

    #[test]
    fn a_new_database_has_the_built_in_filters_in_their_order() {
        let db = Database::in_memory().unwrap();
        assert_eq!(order_of(&db), ["Favourites", "(Installed)", "Most played", "Recently added", "(Hidden)"]);
        let filters = db.filters().unwrap();
        assert!(filters.iter().all(|f| f.kind == FilterKind::Builtin && f.games.is_empty()));
        let keys: Vec<_> = filters.iter().map(|f| f.key.as_deref().unwrap()).collect();
        assert_eq!(keys, [FAVOURITES, INSTALLED, MOST_PLAYED, RECENTLY_ADDED, HIDDEN]);
        let sorts: Vec<_> = filters.iter().map(|f| (f.sort, f.descending)).collect();
        let name = (Sort::Name, false);
        assert_eq!(sorts, [name, name, (Sort::Playtime, true), (Sort::Added, true), name]);
        assert_eq!(filters[0].rule, Rule { favourite: Some(true), ..Rule::default() });
        assert_eq!(filters[4].rule, Rule { hidden: true, ..Rule::default() });
    }

    #[test]
    fn the_built_ins_pick_their_games() {
        let db = woods();
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Cedar"]);
        assert_eq!(names(&db, &keyed(&db, INSTALLED)), ["Alder", "Cedar", "The Dogwood"]);
        assert_eq!(names(&db, &keyed(&db, MOST_PLAYED)), ["The Dogwood", "Alder", "Birch"]);
        // The newest first; those that came in together by name.
        assert_eq!(names(&db, &keyed(&db, RECENTLY_ADDED)), ["The Dogwood", "Elm", "Alder", "Birch", "Cedar"]);
        assert!(keyed(&db, HIDDEN).games.is_empty());
        // The snapshot carries them, and every game they name.
        let snapshot = db.snapshot_at(NOW).unwrap();
        assert_eq!(snapshot.filters, db.filters_at(NOW).unwrap());
        assert!(snapshot.filters.iter().flat_map(|f| &f.games).all(|&id| snapshot.game(id).is_some()));
        let groups: Vec<&str> = snapshot.groups().map(|f| f.name.as_str()).collect();
        assert_eq!(groups, ["Favourites", "Most played", "Recently added"]);
        let dogwood = snapshot.game(id_of(&db, "The Dogwood")).unwrap();
        assert_eq!((dogwood.source.as_str(), dogwood.added), (STEAM, NOW - 2 * DAYS));
    }

    #[test]
    fn each_clause_of_a_rule_picks_its_games() {
        let mut db = woods();
        let db = &mut db;
        let all = ["Alder", "Birch", "Cedar", "The Dogwood", "Elm"];
        assert_eq!(by_name(db, Rule::default()), all);
        assert_eq!(by_name(db, Rule { installed: Some(true), ..Rule::default() }), ["Alder", "Cedar", "The Dogwood"]);
        assert_eq!(by_name(db, Rule { installed: Some(false), ..Rule::default() }), ["Birch", "Elm"]);
        assert_eq!(by_name(db, Rule { favourite: Some(true), ..Rule::default() }), ["Cedar"]);
        let played = |played| Rule { played: Some(played), ..Rule::default() };
        assert_eq!(by_name(db, played(Played::Played)), ["Alder", "Birch", "The Dogwood"]);
        assert_eq!(by_name(db, played(Played::Never)), ["Cedar", "Elm"]);
        let played_within = |days| Rule { played_within_days: Some(days), ..Rule::default() };
        assert_eq!(by_name(db, played_within(30)), ["Alder"]);
        assert_eq!(by_name(db, played_within(90)), ["Alder", "Birch"]);
        // However far back, a game never played was not played then.
        assert_eq!(by_name(db, played_within(100_000)), ["Alder", "Birch", "The Dogwood"]);
        let added_within = |days| Rule { added_within_days: Some(days), ..Rule::default() };
        assert_eq!(by_name(db, added_within(7)), ["The Dogwood", "Elm"]);
        assert_eq!(by_name(db, added_within(365)), all);
        let at_least = |hours| Rule { min_playtime_hours: Some(hours), ..Rule::default() };
        assert_eq!(by_name(db, at_least(1)), ["Alder", "The Dogwood"]);
        assert_eq!(by_name(db, at_least(10)), ["Alder", "The Dogwood"]);
        assert_eq!(by_name(db, at_least(50)), ["The Dogwood"]);
        let from =
            |sources: &[&str]| Rule { sources: sources.iter().map(|s| s.to_string()).collect(), ..Rule::default() };
        assert_eq!(by_name(db, from(&["gog"])), ["Elm"]);
        assert_eq!(by_name(db, from(&[STEAM, "gog"])), all);
        assert!(by_name(db, from(&["epic"])).is_empty());
    }

    #[test]
    fn a_rule_is_all_of_its_clauses() {
        let mut db = woods();
        let rule = Rule { installed: Some(true), played: Some(Played::Played), ..Rule::default() };
        assert_eq!(by_name(&mut db, rule.clone()), ["Alder", "The Dogwood"]);
        let rule = Rule { min_playtime_hours: Some(50), ..rule };
        assert_eq!(by_name(&mut db, rule.clone()), ["The Dogwood"]);
        assert!(by_name(&mut db, Rule { played_within_days: Some(30), ..rule }).is_empty());
    }

    #[test]
    fn a_collection_is_the_eighth_clause_and_is_its_filters_rule() {
        let mut db = woods();
        db.sync(STEAM, &steam_with(Some(vec![collection("uc-1", "Shade", &[4, 2])])), NOW).unwrap();
        let shade = db.filters_at(NOW).unwrap().into_iter().find(|f| f.name == "Shade").unwrap();
        assert_eq!((shade.kind, &shade.rule, shade.key.as_deref()), (FilterKind::Collection, &Rule::default(), None));
        assert_eq!(names(&db, &shade), ["Birch", "The Dogwood"]);
    }

    #[test]
    fn a_hidden_or_gone_game_is_in_no_filter_but_hidden() {
        let mut db = woods();
        db.set_hidden(id_of(&db, "Cedar"), true).unwrap();
        db.set_hidden(id_of(&db, "Alder"), true).unwrap();
        // Birch is no longer Steam's.
        let mut steam = steam_with(None);
        steam.games.retain(|g| g.name != "Birch");
        db.sync(STEAM, &steam, NOW).unwrap();

        assert_eq!(by_name(&mut db, Rule::default()), ["The Dogwood", "Elm"]);
        assert!(keyed(&db, FAVOURITES).games.is_empty());
        assert_eq!(names(&db, &keyed(&db, INSTALLED)), ["The Dogwood"]);
        assert_eq!(names(&db, &keyed(&db, HIDDEN)), ["Alder", "Cedar"]);
        // The snapshot keeps the hidden games apart, where the Hidden filter's ids find them.
        let snapshot = db.snapshot_at(NOW).unwrap();
        let listed = |games: &[Game]| games.iter().map(|g| g.name.clone()).collect::<Vec<_>>();
        assert_eq!(listed(&snapshot.games), ["The Dogwood", "Elm"]);
        assert_eq!(listed(&snapshot.hidden), ["Alder", "Cedar"]);
        assert!(snapshot.hidden.iter().all(|g| g.hidden) && snapshot.games.iter().all(|g| !g.hidden));
        assert_eq!(snapshot.game(id_of(&db, "Cedar")).map(|g| g.name.as_str()), Some("Cedar"));
        assert_eq!(snapshot.game(id_of(&db, "Birch")), None);

        db.set_hidden(id_of(&db, "Cedar"), false).unwrap();
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Cedar"]);
        assert_eq!(names(&db, &keyed(&db, HIDDEN)), ["Alder"]);
    }

    #[test]
    fn a_rule_with_a_clause_this_program_does_not_know_picks_nothing() {
        let mut db = woods();
        let id = db.add_filter("From the future").unwrap();
        assert_eq!(filter(&db, id).games.len(), 5);
        for rule in [r#"{"installed":true,"genre":"puzzle"}"#, r#"{"played":"sometimes"}"#, "not a rule"] {
            db.conn.execute("UPDATE filters SET rule = ?2 WHERE id = ?1", params![id, rule]).unwrap();
            let read = filter(&db, id);
            assert!(read.games.is_empty(), "{rule}");
            assert_eq!((read.name.as_str(), &read.rule), ("From the future", &Rule::default()));
        }
        // The others are none the worse for it.
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Cedar"]);
    }

    #[test]
    fn a_rule_is_kept_as_json_with_only_what_is_set() {
        let mut db = woods();
        let id = db.add_filter("Lately").unwrap();
        let stored = |db: &Database| -> String {
            db.conn.query_row("SELECT rule FROM filters WHERE id = ?1", [id], |row| row.get(0)).unwrap()
        };
        assert_eq!(stored(&db), "{}");
        let rule = Rule {
            installed: Some(true),
            favourite: Some(true),
            played: Some(Played::Played),
            played_within_days: Some(30),
            added_within_days: Some(90),
            min_playtime_hours: Some(10),
            sources: vec![STEAM.to_string()],
            hidden: false,
        };
        db.set_filter(id, "Lately", &rule, Sort::LastPlayed, true).unwrap();
        // docs/spec/filters.md, section 7.
        assert_eq!(
            stored(&db),
            r#"{"installed":true,"favourite":true,"played":"played","played_within_days":30,"added_within_days":90,"min_playtime_hours":10,"sources":["steam"]}"#
        );
        let read = filter(&db, id);
        assert_eq!((read.rule, read.sort, read.descending), (rule, Sort::LastPlayed, true));
    }

    #[test]
    fn each_sort_goes_both_ways_with_the_never_played_last_and_ties_by_name() {
        let mut db = woods();
        let db = &mut db;
        let all = Rule::default();
        assert_eq!(picks(db, &all, Sort::Name, false), ["Alder", "Birch", "Cedar", "The Dogwood", "Elm"]);
        assert_eq!(picks(db, &all, Sort::Name, true), ["Elm", "The Dogwood", "Cedar", "Birch", "Alder"]);
        // Cedar and Elm were never played: last whichever way, and by name between themselves.
        assert_eq!(picks(db, &all, Sort::LastPlayed, true), ["Alder", "Birch", "The Dogwood", "Cedar", "Elm"]);
        assert_eq!(picks(db, &all, Sort::LastPlayed, false), ["The Dogwood", "Birch", "Alder", "Cedar", "Elm"]);
        assert_eq!(picks(db, &all, Sort::Playtime, true), ["The Dogwood", "Alder", "Birch", "Cedar", "Elm"]);
        assert_eq!(picks(db, &all, Sort::Playtime, false), ["Birch", "Alder", "The Dogwood", "Cedar", "Elm"]);
        // Every game was added some day; those added together are by name, upward, whichever way the days go.
        assert_eq!(picks(db, &all, Sort::Added, true), ["The Dogwood", "Elm", "Alder", "Birch", "Cedar"]);
        assert_eq!(picks(db, &all, Sort::Added, false), ["Alder", "Birch", "Cedar", "The Dogwood", "Elm"]);
        // A sort the database does not name is by name.
        let id = db.add_filter("Odd").unwrap();
        db.conn.execute("UPDATE filters SET sort = 'release_date', descending = 0 WHERE id = ?1", [id]).unwrap();
        let odd = filter(db, id);
        assert_eq!((odd.sort, names(db, &odd)[0].as_str()), (Sort::Name, "Alder"));
        assert!(Sort::ALL.iter().all(|&sort| sort_of(sort_text(sort)) == sort));
        assert_eq!(Sort::ALL.map(Sort::default_descending), [false, true, true, true]);
    }

    #[test]
    fn steams_collections_become_filters_after_favourites() {
        let mut db = woods();
        let found = vec![collection("uc-1", "Shade", &[4, 2]), collection("uc-2", "Understorey", &[3, 999])];
        db.sync(STEAM, &steam_with(Some(found.clone())), NOW).unwrap();
        let order = ["Favourites", "Shade", "Understorey", "(Installed)", "Most played", "Recently added", "(Hidden)"];
        assert_eq!(order_of(&db), order);
        let named = |db: &Database, name: &str| db.filters_at(NOW).unwrap().into_iter().find(|f| f.name == name);
        // By name, as every collection's filter is; 999 is no game of the library's.
        assert_eq!(names(&db, &named(&db, "Shade").unwrap()), ["Birch", "The Dogwood"]);
        assert_eq!(names(&db, &named(&db, "Understorey").unwrap()), ["Cedar"]);
        let before = db.filters_at(NOW).unwrap();

        // The same again changes nothing.
        db.sync(STEAM, &steam_with(Some(found)), NOW).unwrap();
        assert_eq!(db.filters_at(NOW).unwrap(), before);

        // One renamed with other games, one new, one gone: the filters follow, the first keeping its id.
        let shade = named(&db, "Shade").unwrap().id;
        let found = vec![collection("uc-3", "Canopy", &[1]), collection("uc-1", "Deep shade", &[1, 2, 3])];
        db.sync(STEAM, &steam_with(Some(found)), NOW).unwrap();
        let order = ["Favourites", "Deep shade", "Canopy", "(Installed)", "Most played", "Recently added", "(Hidden)"];
        assert_eq!(order_of(&db), order);
        let renamed = named(&db, "Deep shade").unwrap();
        assert_eq!((renamed.id, names(&db, &renamed)), (shade, vec!["Alder".into(), "Birch".into(), "Cedar".into()]));
        assert_eq!(names(&db, &named(&db, "Canopy").unwrap()), ["Alder"]);
        let count = |table: &str| -> i64 {
            db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap()
        };
        assert_eq!((count("collections"), count("collection_games")), (2, 4), "Understorey went with its games");
    }

    #[test]
    fn a_sync_leaves_which_collections_show_and_their_order_to_the_user() {
        let mut db = woods();
        let found = vec![collection("uc-1", "Shade", &[4, 2]), collection("uc-2", "Understorey", &[3])];
        db.sync(STEAM, &steam_with(Some(found.clone())), NOW).unwrap();
        let id = |db: &Database, name: &str| db.filters_at(NOW).unwrap().iter().find(|f| f.name == name).unwrap().id;
        db.set_filter_shown(id(&db, "Shade"), false).unwrap();
        db.move_filter(id(&db, "Understorey"), 3).unwrap();
        let order =
            ["Favourites", "(Shade)", "(Installed)", "Most played", "Recently added", "Understorey", "(Hidden)"];
        assert_eq!(order_of(&db), order);

        // Synced again, renamed, and with a third: the two stay as the user left them, and the new one goes after
        // the last collection's filter, wherever that is now.
        let found = vec![found[0].clone(), collection("uc-2", "Floor", &[3]), collection("uc-3", "Canopy", &[1])];
        db.sync(STEAM, &steam_with(Some(found)), NOW).unwrap();
        let order =
            ["Favourites", "(Shade)", "(Installed)", "Most played", "Recently added", "Floor", "Canopy", "(Hidden)"];
        assert_eq!(order_of(&db), order);
    }

    #[test]
    fn with_no_word_on_collections_they_are_left_as_they_are_and_with_none_they_go() {
        let mut db = woods();
        db.sync(STEAM, &steam_with(Some(vec![collection("uc-1", "Shade", &[4, 2])])), NOW).unwrap();
        let before = db.filters_at(NOW).unwrap();
        // The file was missing or made no sense.
        db.sync(STEAM, &steam_with(None), NOW).unwrap();
        assert_eq!(db.filters_at(NOW).unwrap(), before);
        // Another store's sync is no word on Steam's.
        db.sync(
            "gog",
            &Library { collections: Some(Vec::new()), ..library(vec![game(0, "Elm", None, 0, false)]) },
            NOW,
        )
        .unwrap();
        assert_eq!(db.filters_at(NOW).unwrap(), before);
        // The file says there are none.
        db.sync(STEAM, &steam_with(Some(Vec::new())), NOW).unwrap();
        assert_eq!(order_of(&db), ["Favourites", "(Installed)", "Most played", "Recently added", "(Hidden)"]);
    }

    #[test]
    fn many_collections_still_find_a_place_each() {
        let mut db = woods();
        let found: Vec<Collection> =
            (0..40).map(|i| collection(&format!("uc-{i}"), &format!("Shelf {i:02}"), &[1 + i % 4])).collect();
        db.sync(STEAM, &steam_with(Some(found)), NOW).unwrap();
        let mut order = vec!["Favourites".to_string()];
        order.extend((0..40).map(|i| format!("Shelf {i:02}")));
        order.extend(["(Installed)", "Most played", "Recently added", "(Hidden)"].map(String::from));
        assert_eq!(order_of(&db), order);
        let mut positions = db.conn.prepare("SELECT position FROM filters ORDER BY position, id").unwrap();
        let positions: Vec<i64> = positions.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{positions:?}");
    }

    #[test]
    fn a_filter_is_shown_or_not_and_moved_among_the_others() {
        let mut db = woods();
        let installed = keyed(&db, INSTALLED).id;
        db.set_filter_shown(installed, true).unwrap();
        db.set_filter_shown(keyed(&db, FAVOURITES).id, false).unwrap();
        assert_eq!(order_of(&db), ["(Favourites)", "Installed", "Most played", "Recently added", "(Hidden)"]);

        db.move_filter(installed, 1).unwrap();
        assert_eq!(order_of(&db), ["(Favourites)", "Most played", "Installed", "Recently added", "(Hidden)"]);
        db.move_filter(installed, -1).unwrap();
        db.move_filter(installed, -1).unwrap();
        assert_eq!(order_of(&db), ["Installed", "(Favourites)", "Most played", "Recently added", "(Hidden)"]);
        // It stops at the ends.
        db.move_filter(installed, -1).unwrap();
        assert_eq!(order_of(&db), ["Installed", "(Favourites)", "Most played", "Recently added", "(Hidden)"]);
        db.move_filter(installed, 9).unwrap();
        assert_eq!(order_of(&db), ["(Favourites)", "Most played", "Recently added", "(Hidden)", "Installed"]);
        db.move_filter(installed, 1).unwrap();
        assert_eq!(order_of(&db), ["(Favourites)", "Most played", "Recently added", "(Hidden)", "Installed"]);
        assert!(db.move_filter(9999, 1).is_err());

        // Places that have come to be the same are told apart again before a move.
        db.conn.execute("UPDATE filters SET position = 7", []).unwrap();
        let first = db.filters_at(NOW).unwrap()[0].id;
        db.move_filter(first, 2).unwrap();
        let ids: Vec<i64> = db.filters_at(NOW).unwrap().iter().map(|f| f.id).collect();
        assert_eq!(ids, [2, 3, 1, 4, 5]);
    }

    #[test]
    fn a_custom_filter_is_added_changed_and_deleted_and_no_other_kind_is() {
        let mut db = woods();
        let id = db.add_filter("  Weeknights ").unwrap();
        let added = filter(&db, id);
        assert_eq!((added.kind, added.name.as_str(), added.shown), (FilterKind::Custom, "Weeknights", true));
        assert_eq!(
            (&added.rule, added.sort, added.descending, added.games.len()),
            (&Rule::default(), Sort::Name, false, 5)
        );
        assert_eq!(order_of(&db).last().unwrap(), "Weeknights");
        assert!(db.add_filter("   ").is_err());

        let rule = Rule { installed: Some(true), ..Rule::default() };
        db.set_filter(id, "Ready to play", &rule, Sort::Playtime, true).unwrap();
        let changed = filter(&db, id);
        assert_eq!(names(&db, &changed), ["The Dogwood", "Alder", "Cedar"]);
        assert_eq!((changed.name.as_str(), changed.shown), ("Ready to play", true));
        assert!(db.set_filter(id, " ", &rule, Sort::Name, false).is_err());

        // A second goes after the first; the first can go above the built-ins.
        let second = db.add_filter("Later").unwrap();
        db.move_filter(id, -9).unwrap();
        assert_eq!(order_of(&db)[..2], ["Ready to play", "Favourites"]);
        assert_eq!(order_of(&db).last().unwrap(), "Later");

        // A built-in and a collection's filter are not the user's to change or delete.
        db.sync(STEAM, &steam_with(Some(vec![collection("uc-1", "Shade", &[4])])), NOW).unwrap();
        let before = db.filters_at(NOW).unwrap();
        let shade = before.iter().find(|f| f.name == "Shade").unwrap().id;
        for fixed in [keyed(&db, FAVOURITES).id, shade, 9999] {
            assert!(db.set_filter(fixed, "Mine", &rule, Sort::Name, false).is_err());
            assert!(db.delete_filter(fixed).is_err());
        }
        assert_eq!(db.filters_at(NOW).unwrap(), before);

        db.delete_filter(id).unwrap();
        db.delete_filter(second).unwrap();
        assert!(db.filters_at(NOW).unwrap().iter().all(|f| f.kind != FilterKind::Custom));
    }

    /// What Settings' Groups page asks for, each change by the setter that is its own.
    #[test]
    fn a_change_settings_asks_for_is_carried_out() {
        let mut db = woods();
        let id = db.change_filter(&FilterChange::Add { name: "New Group".into() }).unwrap().expect("its id");
        assert_eq!(order_of(&db).last().unwrap(), "New Group");
        let rule = Rule { installed: Some(true), ..Rule::default() };
        let set = FilterChange::Set { filter: id, name: "Ready".into(), rule, sort: Sort::Playtime, descending: true };
        assert_eq!(db.change_filter(&set).unwrap(), None);
        assert_eq!(names(&db, &filter(&db, id)), ["The Dogwood", "Alder", "Cedar"]);
        db.change_filter(&FilterChange::Move { filter: id, by: -9 }).unwrap();
        assert_eq!(order_of(&db)[0], "Ready");
        db.change_filter(&FilterChange::Shown { filter: id, shown: false }).unwrap();
        assert!(!filter(&db, id).shown);
        db.change_filter(&FilterChange::Delete { filter: id }).unwrap();
        assert!(order_of(&db).iter().all(|name| name != "Ready"));
        // What a setter refuses is refused here.
        assert!(db.change_filter(&FilterChange::Delete { filter: keyed(&db, FAVOURITES).id }).is_err());
        assert!(db.change_filter(&FilterChange::Add { name: " ".into() }).is_err());
    }

    #[test]
    fn steams_favourites_and_hidden_games_are_taken_as_a_game_is_first_seen_and_never_again() {
        let mut db = Database::in_memory().unwrap();
        let marked = |favourites: &[u32], hidden: &[u32]| {
            let mut steam = steam_with(Some(Vec::new()));
            for game in &mut steam.games {
                let appid: u32 = game.source_id.parse().unwrap();
                (game.favourite, game.hidden) = (favourites.contains(&appid), hidden.contains(&appid));
            }
            steam
        };
        db.sync(STEAM, &marked(&[1, 2], &[3]), NOW).unwrap();
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Alder", "Birch"]);
        assert_eq!(names(&db, &keyed(&db, HIDDEN)), ["Cedar"]);

        // The marks are the user's from then on: Steam's later word does not reach them, either way.
        db.set_favourite(id_of(&db, "Alder"), false).unwrap();
        db.set_hidden(id_of(&db, "Cedar"), false).unwrap();
        db.sync(STEAM, &marked(&[1, 4], &[3, 2]), NOW).unwrap();
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Birch"]);
        assert!(keyed(&db, HIDDEN).games.is_empty());

        // A game that comes later brings its own.
        let mut later = marked(&[], &[]);
        later.games.push(Game { favourite: true, ..game(5, "Fir", None, 0, false) });
        db.sync(STEAM, &later, NOW).unwrap();
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Birch", "Fir"]);
    }

    #[test]
    fn games_that_were_here_before_steam_could_say_take_its_marks_once() {
        // The woods were synced with no word on collections, as a database from before this was.
        let mut db = woods();
        let mut steam = steam_with(Some(Vec::new()));
        steam.games[0].favourite = true;
        steam.games[1].hidden = true;
        db.sync(STEAM, &steam, NOW).unwrap();
        // Cedar was the user's favourite already, and stays one.
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Alder", "Cedar"]);
        assert_eq!(names(&db, &keyed(&db, HIDDEN)), ["Birch"]);

        db.set_favourite(id_of(&db, "Alder"), false).unwrap();
        steam.games[3].favourite = true;
        db.sync(STEAM, &steam, NOW).unwrap();
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Cedar"]);
    }

    #[test]
    fn a_database_of_the_first_version_gains_the_filters_and_keeps_its_games() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(crate::FILE);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(crate::MIGRATIONS[0]).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.execute(
                "INSERT INTO games (source, source_id, name, sort_name, installed, playtime_minutes, last_played, added)
                 VALUES ('steam', '1', 'Alder', 'alder', 1, 600, 1700000000, 1600000000)",
                [],
            )
            .unwrap();
        }
        let db = Database::open(&path).unwrap();
        assert_eq!(db.version().unwrap(), crate::MIGRATIONS.len());
        assert_eq!(names(&db, &keyed(&db, MOST_PLAYED)), ["Alder"]);
        assert_eq!(db.snapshot().unwrap().games[0].added, 1_600_000_000);
    }

    #[test]
    fn the_fixtures_collections_and_favourites_come_through_a_sync() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let mut spec = Fixture::standard();
        spec.games.iter_mut().for_each(|g| g.art = fixture::Art::None);
        fixture::write(&root, &spec).unwrap();
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &source_steam::load(&root).unwrap(), NOW).unwrap();

        let order =
            ["Favourites", "Couch co-op", "Short sessions", "(Installed)", "Most played", "Recently added", "(Hidden)"];
        assert_eq!(order_of(&db), order);
        let filters = db.filters_at(NOW).unwrap();
        let games = |name: &str| names(&db, filters.iter().find(|f| f.name == name).unwrap());
        assert_eq!(games("Favourites"), ["Brass Meridian", "Ember Choir"]);
        assert_eq!(games("Couch co-op"), ["Brass Meridian", "Glasswing Atlas", "Marrow Street", "Saltwind Rally"]);
        // The DLC in it is no game.
        assert_eq!(games("Short sessions"), ["Copperhead Station", "Lanternfish Deep", "Thimble & Thorn"]);
        assert_eq!(games("Most played")[..2], ["Ashfall Courier", "Lanternfish Deep"]);
        assert_eq!(games("Recently added").len(), spec.games.len());

        // In Steam the user renames one collection, deletes the other and hides a game. The hiding comes too late:
        // the game has been seen.
        let collections = spec.collections.as_mut().unwrap();
        collections.collections.retain(|c| c.name != "Short sessions");
        collections.collections.iter_mut().find(|c| c.name == "Couch co-op").unwrap().name = "Sofa".into();
        collections.hidden = vec![spec.games[2].appid];
        let again = dir.path().join("steam again");
        fixture::write(&again, &spec).unwrap();
        db.sync(STEAM, &source_steam::load(&again).unwrap(), NOW).unwrap();
        assert_eq!(order_of(&db), ["Favourites", "Sofa", "(Installed)", "Most played", "Recently added", "(Hidden)"]);
        assert_eq!(names(&db, &keyed(&db, FAVOURITES)), ["Brass Meridian", "Ember Choir"]);

        // And with the file gone the collections stay.
        spec.collections = None;
        let last = dir.path().join("steam at last");
        fixture::write(&last, &spec).unwrap();
        db.sync(STEAM, &source_steam::load(&last).unwrap(), NOW).unwrap();
        assert!(order_of(&db).contains(&"Sofa".to_string()));
    }
}
