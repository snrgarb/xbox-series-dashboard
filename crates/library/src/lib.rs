//! The library database: every game the user has, in one SQLite file, so the Dashboard's first frame comes from disk
//! without reading any store.
//!
//! Every source (a store, behind `model::Source`: docs/sources.md) is read into a `model::Library`, and
//! `Database::sync` merges each in under the source's key (`sources::refresh` does both for a list of them): what
//! the source knows (name, installed, art, its own play history, its collections) follows the source, and what
//! is the user's own (Play sessions, hidden, favourite, which filters show and in what order) is never touched by
//! a sync. A game a source no longer lists is kept, marked gone, and comes back with everything it had if the
//! source lists it again. The screens are given a snapshot (`Database::snapshot`, a `model::Library`) and never
//! see the database.
//!
//! The snapshot carries the saved filters, each with the games it picks: Home's Groups and Full library's tabs
//! (`filters`, and docs/spec/filters.md), and Play history's rows (`history`).
//!
//! The schema has a version (`PRAGMA user_version`) and grows by migrations: `MIGRATIONS[n]` takes a database from
//! version `n` to `n + 1`, and one is never edited once released. What is still to come has its place:
//! - More sources: a game is keyed by `(source, source_id)`, both text, so each store keeps its own ids, and a game
//!   owned in two stores is two games.
//! - Games added by hand and Apps (`by_hand`): the games a source of their own, `manual`, each a row of `games`
//!   keyed by its program's path; the Apps a table of their own, `apps`, as they are not games.
//! - Profiles (`profiles`): a table of their own, their pictures as files in the data folder.
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod achievements;
pub mod art;
pub mod by_hand;
pub mod choices;
pub mod filters;
pub mod game_pass;
pub mod history;
pub mod launchers;
pub mod profiles;
pub mod queue;
pub mod refresh;
pub mod sources;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use model::{Game, Library, PlaySession};
use rusqlite::types::ToSql;
use rusqlite::{params, Connection, OptionalExtension};

/// The database's file name in the data folder.
pub const FILE: &str = "library.db";

/// The setting that holds a source's signed-in user is `<source>.user`.
const USER: &str = "user";

/// `<source>.marks`, set once the source's favourites and hidden games have been taken for the games that were in
/// the database before it could say which they are.
const MARKS: &str = "marks";

/// `<source>.last_played`, set when the games' last played is to be taken again from the source at its next sync,
/// and not kept from before as it is otherwise: Steam's was the later of the user's record and the app manifest's,
/// and a new install's manifest can say the day it was installed, which a sync then kept for good (#237). The
/// Play sessions recorded here are kept: the later of those and the source's is taken.
const RETAKE: &str = "last_played";

const MIGRATIONS: &[&str] = &[
    // 1: games, their art, Play sessions and settings.
    "CREATE TABLE games (
        id INTEGER PRIMARY KEY,
        -- Which store the game is from ('steam'), and the store's own id for it (Steam's appid).
        source TEXT NOT NULL,
        source_id TEXT NOT NULL,
        name TEXT NOT NULL,
        -- What the game sorts by: lower case, without a leading article.
        sort_name TEXT NOT NULL,
        installed INTEGER NOT NULL DEFAULT 0,
        install_dir TEXT,
        -- Unix seconds; 0 = never played.
        last_played INTEGER NOT NULL DEFAULT 0,
        playtime_minutes INTEGER NOT NULL DEFAULT 0,
        -- Unix seconds: when the game first came into this database.
        added INTEGER NOT NULL,
        hidden INTEGER NOT NULL DEFAULT 0,
        favourite INTEGER NOT NULL DEFAULT 0,
        -- The source no longer lists the game. Kept for what the user did with it; not shown.
        gone INTEGER NOT NULL DEFAULT 0,
        UNIQUE (source, source_id)
    );
    CREATE TABLE art (
        game INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        -- cover, background, logo, icon, header or square; online:<part>, what the client says of its art online.
        kind TEXT NOT NULL,
        path TEXT NOT NULL,
        PRIMARY KEY (game, kind)
    ) WITHOUT ROWID;
    CREATE TABLE play_sessions (
        id INTEGER PRIMARY KEY,
        game INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        -- Unix seconds. No end: the game is running, or the Dashboard never saw it end.
        started INTEGER NOT NULL,
        ended INTEGER
    );
    CREATE INDEX play_sessions_game ON play_sessions (game, started);
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) WITHOUT ROWID;",
    // 2: a store's collections, and the saved filters.
    filters::MIGRATION,
    // 3: what the Game hub tells of a game, where its store's own files say (docs/spec/game-hub.md, 14.1).
    "-- Unix seconds; 0 = the store does not say.
    ALTER TABLE games ADD COLUMN released INTEGER NOT NULL DEFAULT 0;
    -- The names, one a line, in the store's order.
    ALTER TABLE games ADD COLUMN developers TEXT NOT NULL DEFAULT '';
    ALTER TABLE games ADD COLUMN publishers TEXT NOT NULL DEFAULT '';
    -- Bytes; 0 = not installed, or the store does not say.
    ALTER TABLE games ADD COLUMN size_on_disk INTEGER NOT NULL DEFAULT 0;
    -- 0 to 100, or none.
    ALTER TABLE games ADD COLUMN critic_score INTEGER;
    ALTER TABLE games ADD COLUMN community_score INTEGER;",
    // 4: the Apps (docs/spec/apps.md).
    by_hand::MIGRATION,
    // 5: the Queue, and the games it held that finished installing.
    queue::MIGRATION,
    // 6: the Profiles (docs/spec/profiles.md).
    profiles::MIGRATION,
    // 7: the games' achievements (docs/achievements.md).
    achievements::MIGRATION,
    // 8: the Queue's extras' items (#188).
    queue::ITEMS_MIGRATION,
    // 9: the user's choice of a game's pictures, in the art picker (#213).
    choices::MIGRATION,
    // 10: Steam's games' last played taken again from Steam at its next sync (`RETAKE`, #237).
    "INSERT OR REPLACE INTO settings (key, value) VALUES ('steam.last_played', 'retake');",
    // 11: the games of the PC Game Pass catalogue (`Game::game_pass`, #277).
    "ALTER TABLE games ADD COLUMN game_pass INTEGER NOT NULL DEFAULT 0;",
];

/// What a sync changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Synced {
    pub added: usize,
    /// Games the source says something new of: a name, installed or not, art, a later play.
    pub updated: usize,
    /// Games the source no longer lists.
    pub gone: usize,
}

pub struct Database {
    conn: Connection,
    /// Our own art cache, which a snapshot fills in from what a source gave no picture for. A database in a data
    /// folder has that folder's; one in memory has none unless it is given one.
    art: Option<art::ArtDir>,
    /// The data folder the database is in, which keeps the Profiles' pictures. None for one in memory.
    folder: Option<PathBuf>,
}

/// Where the database lives when nothing says otherwise: `%LOCALAPPDATA%\XboxSeriesDashboard` on Windows;
/// elsewhere `$XDG_DATA_HOME/xbox-series-dashboard`, else `~/.local/share/xbox-series-dashboard`.
pub fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        Some(var("LOCALAPPDATA")?.join("XboxSeriesDashboard"))
    } else {
        let data = var("XDG_DATA_HOME").or_else(|| Some(var("HOME")?.join(".local").join("share")))?;
        Some(data.join("xbox-series-dashboard"))
    }
}

/// The time now, in Unix seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Database {
    /// Opens the database at `path`, making it and its folder if they aren't there, and brings its schema up to
    /// date.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
        }
        let mut database = Self::set_up(Connection::open(path)?)?;
        database.art = path.parent().map(art::ArtDir::under);
        database.folder = path.parent().map(Path::to_path_buf);
        Ok(database)
    }

    /// The art cache a snapshot fills missing pictures from; none for no cache.
    pub fn set_art_dir(&mut self, art: Option<art::ArtDir>) {
        self.art = art;
    }

    /// The art cache this database's snapshots fill missing pictures from.
    pub fn art_dir(&self) -> Option<&art::ArtDir> {
        self.art.as_ref()
    }

    /// A database that lives in memory and goes with the value: for tests.
    pub fn in_memory() -> Result<Self> {
        Self::set_up(Connection::open_in_memory()?)
    }

    fn set_up(mut conn: Connection) -> Result<Self> {
        // The sync runs on its own connection on another thread: a write waits for the other's rather than failing.
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let version = version(&conn)?;
        if version > MIGRATIONS.len() {
            bail!("the database is version {version}, newer than this program's {}", MIGRATIONS.len());
        }
        for (done, migration) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = conn.transaction()?;
            tx.execute_batch(migration)?;
            tx.pragma_update(None, "user_version", done as i64 + 1)?;
            tx.commit()?;
        }
        // A database from before the launchers had filters gains them.
        filters::sync_launchers(&conn)?;
        Ok(Self { conn, art: None, folder: None })
    }

    /// The schema's version.
    pub fn version(&self) -> Result<usize> {
        version(&self.conn)
    }

    /// Merges what `source` has just been read as (`found`) into the database, at the time `now`:
    /// - a game not seen before is added;
    /// - a game seen before takes the source's name, installed state, art and facts (who made it, when it came
    ///   out, its size and scores); its last played and playtime only ever go up, so a Play session recorded here
    ///   is not undone by a source that has not caught up;
    /// - a game the source no longer lists is marked uninstalled and gone.
    ///
    /// Play sessions, hidden and favourite are the user's and are left as they are: what the source says of the two
    /// marks (Steam's Favourites, its hidden games) is taken only as a game is first seen. The source's
    /// collections, when it read any (`found.collections`), become the collection filters (`filters`). Syncing the
    /// same thing twice changes nothing the second time.
    pub fn sync(&mut self, source: &str, found: &Library, now: u64) -> Result<Synced> {
        struct Known {
            id: i64,
            /// Everything of the row a source can change.
            state: State,
            seen: bool,
        }
        #[derive(PartialEq)]
        struct State {
            name: String,
            installed: bool,
            install_dir: Option<String>,
            last_played: i64,
            playtime: i64,
            gone: bool,
            art: Vec<(String, String)>,
            facts: Facts,
        }
        /// What the source says of the game beyond those: the Game hub's facts.
        #[derive(PartialEq)]
        struct Facts {
            released: i64,
            developers: String,
            publishers: String,
            size_on_disk: i64,
            critic_score: Option<i64>,
            community_score: Option<i64>,
            game_pass: bool,
        }

        let tx = self.conn.transaction()?;
        let mut known: HashMap<String, Known> = HashMap::new();
        {
            let mut by_id: HashMap<i64, String> = HashMap::new();
            let mut rows = tx.prepare(
                "SELECT id, source_id, name, installed, install_dir, last_played, playtime_minutes, gone,
                        released, developers, publishers, size_on_disk, critic_score, community_score, game_pass
                 FROM games WHERE source = ?1",
            )?;
            let mut rows = rows.query([source])?;
            while let Some(row) = rows.next()? {
                let (id, source_id): (i64, String) = (row.get(0)?, row.get(1)?);
                let state = State {
                    name: row.get(2)?,
                    installed: row.get(3)?,
                    install_dir: row.get(4)?,
                    last_played: row.get(5)?,
                    playtime: row.get(6)?,
                    gone: row.get(7)?,
                    art: Vec::new(),
                    facts: Facts {
                        released: row.get(8)?,
                        developers: row.get(9)?,
                        publishers: row.get(10)?,
                        size_on_disk: row.get(11)?,
                        critic_score: row.get(12)?,
                        community_score: row.get(13)?,
                        game_pass: row.get(14)?,
                    },
                };
                by_id.insert(id, source_id.clone());
                known.insert(source_id, Known { id, state, seen: false });
            }
            let mut art = tx.prepare(
                "SELECT game, kind, path FROM art WHERE game IN (SELECT id FROM games WHERE source = ?1)
                 ORDER BY game, kind",
            )?;
            let mut art = art.query([source])?;
            while let Some(row) = art.next()? {
                let game: i64 = row.get(0)?;
                if let Some(known) = by_id.get(&game).and_then(|id| known.get_mut(id)) {
                    known.state.art.push((row.get(1)?, row.get(2)?));
                }
            }
        }

        let mut synced = Synced::default();
        let retake_key = format!("{source}.{RETAKE}");
        let retake =
            tx.query_row("SELECT 1 FROM settings WHERE key = ?1", [&retake_key], |_| Ok(())).optional()?.is_some();
        let set_art = |game: i64, art: &[(String, String)]| -> Result<()> {
            tx.execute("DELETE FROM art WHERE game = ?1", [game])?;
            for (kind, path) in art {
                tx.execute("INSERT INTO art (game, kind, path) VALUES (?1, ?2, ?3)", params![game, kind, path])?;
            }
            Ok(())
        };
        let set_facts = |game: i64, facts: &Facts| -> Result<()> {
            tx.execute(
                "UPDATE games
                 SET released = ?2, developers = ?3, publishers = ?4, size_on_disk = ?5, critic_score = ?6,
                     community_score = ?7, game_pass = ?8
                 WHERE id = ?1",
                params![
                    game,
                    facts.released,
                    facts.developers,
                    facts.publishers,
                    facts.size_on_disk,
                    facts.critic_score,
                    facts.community_score,
                    facts.game_pass
                ],
            )?;
            Ok(())
        };
        for game in &found.games {
            let source_id = game.source_id.clone();
            let pictures = [
                ("cover", &game.cover),
                ("background", &game.hero),
                ("header", &game.header),
                ("square", &game.square),
            ];
            let online = game.online.parts().into_iter();
            let mut art: Vec<(String, String)> = pictures
                .into_iter()
                .filter_map(|(kind, path)| Some((kind.to_string(), text(path.as_deref()?))))
                // What the client says of the game's art online, as `online:<part>`.
                .chain(online.filter_map(|(part, value)| Some((format!("online:{part}"), value.clone()?))))
                .collect();
            art.sort();
            let mut state = State {
                name: game.name.clone(),
                installed: game.installed,
                install_dir: game.install_dir.as_deref().map(text),
                last_played: game.last_played as i64,
                playtime: game.playtime_minutes as i64,
                gone: false,
                art,
                facts: Facts {
                    released: game.released as i64,
                    developers: game.developers.join("\n"),
                    publishers: game.publishers.join("\n"),
                    size_on_disk: game.size_on_disk as i64,
                    critic_score: game.critic_score.map(i64::from),
                    community_score: game.community_score.map(i64::from),
                    game_pass: game.game_pass,
                },
            };
            match known.get_mut(&source_id) {
                None => {
                    tx.execute(
                        "INSERT INTO games
                            (source, source_id, name, sort_name, installed, install_dir, last_played,
                             playtime_minutes, added, favourite, hidden)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                        params![
                            source,
                            source_id,
                            state.name,
                            sort_name(&state.name),
                            state.installed,
                            state.install_dir,
                            state.last_played,
                            state.playtime,
                            now as i64,
                            game.favourite,
                            game.hidden
                        ],
                    )?;
                    let id = tx.last_insert_rowid();
                    set_art(id, &state.art)?;
                    set_facts(id, &state.facts)?;
                    synced.added += 1;
                }
                Some(known) => {
                    known.seen = true;
                    let before = match retake {
                        false => known.state.last_played,
                        true => tx.query_row(
                            "SELECT COALESCE(MAX(MAX(started, COALESCE(ended, 0))), 0) FROM play_sessions
                             WHERE game = ?1",
                            [known.id],
                            |row| row.get(0),
                        )?,
                    };
                    state.last_played = state.last_played.max(before);
                    state.playtime = state.playtime.max(known.state.playtime);
                    if state == known.state {
                        continue;
                    }
                    tx.execute(
                        "UPDATE games
                         SET name = ?2, sort_name = ?3, installed = ?4, install_dir = ?5, last_played = ?6,
                             playtime_minutes = ?7, gone = 0
                         WHERE id = ?1",
                        params![
                            known.id,
                            state.name,
                            sort_name(&state.name),
                            state.installed,
                            state.install_dir,
                            state.last_played,
                            state.playtime
                        ],
                    )?;
                    if state.art != known.state.art {
                        set_art(known.id, &state.art)?;
                    }
                    if state.facts != known.state.facts {
                        set_facts(known.id, &state.facts)?;
                    }
                    synced.updated += 1;
                }
            }
        }
        for known in known.values().filter(|k| !k.seen && !k.state.gone) {
            tx.execute(
                "UPDATE games SET installed = 0, install_dir = NULL, size_on_disk = 0, game_pass = 0, gone = 1
                 WHERE id = ?1",
                [known.id],
            )?;
            synced.gone += 1;
        }
        tx.execute("DELETE FROM settings WHERE key = ?1", [&retake_key])?;
        let key = format!("{source}.{USER}");
        match &found.user {
            Some(user) => tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
                params![key, user],
            )?,
            None => tx.execute("DELETE FROM settings WHERE key = ?1", [&key])?,
        };
        if let Some(collections) = &found.collections {
            filters::sync_collections(&tx, source, collections)?;
            // The games that were here before the source could say which are favourites or hidden (a database
            // from before it was asked, or a first sync with no collections file) take its word now, once.
            let key = format!("{source}.{MARKS}");
            let taken: Option<String> =
                tx.query_row("SELECT value FROM settings WHERE key = ?1", [&key], |row| row.get(0)).optional()?;
            if taken.is_none() {
                for game in found.games.iter().filter(|g| g.favourite || g.hidden) {
                    tx.execute(
                        "UPDATE games SET favourite = MAX(favourite, ?3), hidden = MAX(hidden, ?4)
                         WHERE source = ?1 AND source_id = ?2",
                        params![source, game.source_id, game.favourite, game.hidden],
                    )?;
                }
                tx.execute("INSERT INTO settings (key, value) VALUES (?1, 'taken')", [&key])?;
            }
        }
        // A launcher that has its first game, or has lost its last, gains or loses its filter.
        filters::sync_launchers(&tx)?;
        tx.commit()?;
        Ok(synced)
    }

    /// What the screens are given: every game that is neither hidden nor gone, most recently played first and then
    /// by name; the hidden ones apart, in the same order; every filter with its games; Play history; and the
    /// user's name, as the source with the most games says it.
    pub fn snapshot(&self) -> Result<Library> {
        self.snapshot_at(now())
    }

    /// `snapshot`, with the filters that count days counting them back from `now`, and Play history's sections as
    /// of then.
    pub fn snapshot_at(&self, now: u64) -> Result<Library> {
        Ok(Library {
            user: self.user()?,
            games: self.games(&format!("hidden = 0 AND {}", launchers::shown("games")), &[], None)?,
            hidden: self.games(&format!("hidden = 1 AND {}", launchers::shown("games")), &[], None)?,
            filters: self.filters_at(now)?,
            history: history::rows(&self.conn, now)?,
            collections: None,
            queue: self.queue()?,
            apps: self.apps()?,
            store_apps: None,
            profiles: self.profiles()?,
            achievements: self.achievement_summary()?,
            launchers: self.launchers()?,
            art_refresh: Default::default(),
            storage: Default::default(),
            helpers: Vec::new(),
            clients: Vec::new(),
        })
    }

    /// The signed-in user's name, as a source last said it (`<source>.user`): that of the source with the most
    /// games the library shows, and of two with as many the first by name.
    fn user(&self) -> Result<Option<String>> {
        let user = self
            .conn
            .query_row(
                &format!(
                    "SELECT value FROM settings
                     WHERE key LIKE '%.{USER}' AND substr(key, 1, length(key) - {}) NOT IN {}
                     ORDER BY (SELECT COUNT(*) FROM games
                               WHERE {} AND source || '.{USER}' = settings.key) DESC, key
                     LIMIT 1",
                    USER.len() + 1,
                    launchers::OFF,
                    launchers::shown("games")
                ),
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(user)
    }

    /// The Recent row: the `n` most recently played games.
    pub fn recent(&self, n: usize) -> Result<Vec<Game>> {
        self.games(&format!("hidden = 0 AND last_played > 0 AND {}", launchers::shown("games")), &[], Some(n))
    }

    /// One game by its id, hidden or gone as it may be.
    pub fn game(&self, id: i64) -> Result<Option<Game>> {
        Ok(self.games("id = ?1", &[&id], None)?.pop())
    }

    /// The games `filter` (a `WHERE` clause on `games`) picks, in the snapshot's order, with their art: all of them,
    /// or the first `limit`.
    fn games(&self, filter: &str, values: &[&dyn ToSql], limit: Option<usize>) -> Result<Vec<Game>> {
        let limit = limit.map_or(String::new(), |n| format!("LIMIT {n}"));
        let mut games = Vec::new();
        let mut place: HashMap<i64, usize> = HashMap::new();
        let mut rows = self.conn.prepare(&format!(
            "SELECT id, source, source_id, name, installed, install_dir, last_played, playtime_minutes, favourite,
                    added, hidden, released, developers, publishers, size_on_disk, critic_score, community_score,
                    (SELECT COUNT(*) FROM play_sessions WHERE play_sessions.game = games.id), game_pass
             FROM games WHERE {filter} ORDER BY last_played DESC, sort_name, name, id {limit}"
        ))?;
        let mut rows = rows.query(values)?;
        while let Some(row) = rows.next()? {
            let (id, source, source_id): (i64, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            let names = |at: usize| -> rusqlite::Result<Vec<String>> {
                Ok(row.get::<_, String>(at)?.lines().map(str::to_string).collect())
            };
            let score = |at: usize| -> rusqlite::Result<Option<u8>> {
                Ok(row.get::<_, Option<i64>>(at)?.and_then(|score| u8::try_from(score).ok()))
            };
            let installed: bool = row.get(4)?;
            let install_dir = row.get::<_, Option<String>>(5)?.map(PathBuf::from);
            place.insert(id, games.len());
            games.push(Game {
                id,
                source,
                source_id,
                name: row.get(3)?,
                installed,
                installed_at: install_dir.as_deref().filter(|_| installed).and_then(installed_at),
                install_dir,
                last_played: row.get::<_, i64>(6)?.max(0) as u64,
                playtime_minutes: row.get::<_, i64>(7)?.clamp(0, u32::MAX as i64) as u32,
                favourite: row.get(8)?,
                added: row.get::<_, i64>(9)?.max(0) as u64,
                hidden: row.get(10)?,
                released: row.get::<_, i64>(11)?.max(0) as u64,
                developers: names(12)?,
                publishers: names(13)?,
                size_on_disk: row.get::<_, i64>(14)?.max(0) as u64,
                critic_score: score(15)?,
                community_score: score(16)?,
                play_count: row.get::<_, i64>(17)?.clamp(0, u32::MAX as i64) as u32,
                game_pass: row.get(18)?,
                cover: None,
                hero: None,
                header: None,
                square: None,
                online: Default::default(),
                art: Default::default(),
            });
        }
        // With no limit here: art for games past it is read and dropped.
        let mut art = self.conn.prepare(&format!(
            "SELECT game, kind, path FROM art WHERE game IN (SELECT id FROM games WHERE {filter})"
        ))?;
        let mut art = art.query(values)?;
        while let Some(row) = art.next()? {
            let (game, kind, path): (i64, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            let Some(game) = place.get(&game).map(|&at| &mut games[at]) else { continue };
            // The store's own, which the art picker offers as a choice.
            if let Some(tab) = model::art_picker::Tab::from_key(&kind) {
                game.art.store[tab.index()] = Some(path.clone().into());
            }
            match kind.as_str() {
                "cover" => game.cover = Some(path.into()),
                "background" => game.hero = Some(path.into()),
                "header" => game.header = Some(path.into()),
                "square" => game.square = Some(path.into()),
                other => {
                    // What the client says of its art online; a logo or an icon, which no screen draws yet, is not.
                    if let Some(part) = other.strip_prefix("online:").and_then(|part| game.online.part_mut(part)) {
                        *part = Some(path);
                    }
                }
            }
        }
        // What the source gave no picture for, from our own cache (docs/sources.md, "Art"): the source's own wins.
        if let Some(cached) = self.art.as_ref().map(art::ArtDir::index).filter(|index| !index.is_empty()) {
            for game in &mut games {
                let Some([cover, hero, header, square]) = cached.get(&(game.source.clone(), game.source_id.clone()))
                else {
                    continue;
                };
                let pictures = [
                    (&mut game.cover, cover),
                    (&mut game.hero, hero),
                    (&mut game.header, header),
                    (&mut game.square, square),
                ];
                for (mine, ours) in pictures {
                    if mine.is_none() {
                        mine.clone_from(ours);
                    }
                }
            }
        }
        // And over all of it, what the user chose.
        self.apply_choices(&mut games)?;
        Ok(games)
    }

    /// The game's window is up: a Play session starts at `now`, and the game was last played now. Gives the
    /// session's id, for `session_ended`.
    pub fn session_started(&mut self, game: i64, now: u64) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO play_sessions (game, started) VALUES (?1, ?2)", params![game, now as i64])?;
        let session = tx.last_insert_rowid();
        tx.execute("UPDATE games SET last_played = MAX(last_played, ?2) WHERE id = ?1", params![game, now as i64])?;
        tx.commit()?;
        Ok(session)
    }

    /// The game ended at `now`: the session gets its end, and the game its whole minutes of play. A session that
    /// has ended already is left alone.
    pub fn session_ended(&mut self, session: i64, now: u64) -> Result<()> {
        let tx = self.conn.transaction()?;
        let open: Option<(i64, i64)> = tx
            .query_row("SELECT game, started FROM play_sessions WHERE id = ?1 AND ended IS NULL", [session], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        if let Some((game, started)) = open {
            // Never before it started, whatever the clock did meanwhile.
            let ended = (now as i64).max(started);
            tx.execute("UPDATE play_sessions SET ended = ?2 WHERE id = ?1", params![session, ended])?;
            tx.execute(
                "UPDATE games
                 SET playtime_minutes = playtime_minutes + ?2, last_played = MAX(last_played, ?3)
                 WHERE id = ?1",
                params![game, (ended - started) / 60, ended],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A game's Play sessions, the oldest first.
    pub fn sessions(&self, game: i64) -> Result<Vec<PlaySession>> {
        let mut rows =
            self.conn.prepare("SELECT started, ended FROM play_sessions WHERE game = ?1 ORDER BY started, id")?;
        let rows = rows.query_map([game], |row| {
            let (started, ended): (i64, Option<i64>) = (row.get(0)?, row.get(1)?);
            Ok(PlaySession { game, started: started.max(0) as u64, ended: ended.map(|e| e.max(0) as u64) })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Hides a game from every list, or shows it again.
    pub fn set_hidden(&self, game: i64, hidden: bool) -> Result<()> {
        self.conn.execute("UPDATE games SET hidden = ?2 WHERE id = ?1", params![game, hidden])?;
        Ok(())
    }

    pub fn set_favourite(&self, game: i64, favourite: bool) -> Result<()> {
        self.conn.execute("UPDATE games SET favourite = ?2 WHERE id = ?1", params![game, favourite])?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0)).optional()?)
    }

    /// Forgets a setting: it is its default again.
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }
}

fn version(conn: &Connection) -> Result<usize> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    usize::try_from(version).context("the database's version is not one of ours")
}

/// A path as the database keeps it.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// What a name sorts by: lower case, without a leading article.
fn sort_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let rest = ["the ", "a ", "an "].iter().find_map(|article| lower.strip_prefix(article));
    rest.map_or(lower.clone(), |rest| rest.trim_start().to_string())
}

/// When the game in `dir` was installed, as near as can be told: when the folder was made, which its store does as
/// the install begins (the Bundle's `InstalledAt`), else when it last changed where the file system keeps no birth
/// time. None when the folder can't be read.
fn installed_at(dir: &std::path::Path) -> Option<u64> {
    let folder = std::fs::metadata(dir).ok()?;
    let at = folder.created().or_else(|_| folder.modified()).ok()?;
    Some(at.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_steam::fixture::{self, Fixture};

    const NOW: u64 = 1_800_000_000;
    pub(crate) const STEAM: &str = source_steam::SOURCE;

    fn game(appid: u32, name: &str, last_played: u64) -> Game {
        Game {
            id: 0,
            source: STEAM.into(),
            source_id: appid.to_string(),
            name: name.into(),
            last_played,
            playtime_minutes: if last_played > 0 { 30 } else { 0 },
            installed: true,
            install_dir: Some(PathBuf::from("games").join(name)),
            cover: Some(PathBuf::from("art").join(format!("{appid}-cover.jpg"))),
            hero: None,
            header: None,
            ..Game::default()
        }
    }

    fn found() -> Library {
        Library {
            user: Some("Tester".into()),
            games: vec![game(30, "Cinder", 300), game(10, "The Anchor", 100), game(20, "Bramble", 0)],
            ..Library::default()
        }
    }

    /// A snapshot's games as a source would have read them: no ids, no time they were added, and no filters.
    fn without_ids(mut library: Library) -> Library {
        library
            .games
            .iter_mut()
            .for_each(|g| (g.id, g.added, g.art, g.installed_at) = (0, 0, Default::default(), None));
        library.filters.clear();
        library.history.clear();
        library.launchers = Default::default();
        library
    }

    /// What a source read, less its collections: what `without_ids` makes of the snapshot it becomes.
    fn games_of(mut library: Library) -> Library {
        library.collections = None;
        library
    }

    fn names(games: &[Game]) -> Vec<&str> {
        games.iter().map(|g| g.name.as_str()).collect()
    }

    /// docs/spec/manage-queue.md, 7.3: an installed game's install date is its folder's, as the Bundle's; none for
    /// a folder that is not there or a game not installed.
    #[test]
    fn a_snapshot_dates_an_installed_game_by_its_install_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Cinder");
        std::fs::create_dir(&folder).unwrap();
        let mut found = found();
        found.games[0].install_dir = Some(folder.clone());
        found.games[2].installed = false;
        found.games[2].install_dir = Some(folder);
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &found, NOW).unwrap();
        let snapshot = db.snapshot().unwrap();
        let at = |name: &str| snapshot.games.iter().find(|g| g.name == name).unwrap().installed_at;
        let made = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        assert!(at("Cinder").is_some_and(|at| at.abs_diff(made) < 60), "{:?}", at("Cinder"));
        assert_eq!((at("The Anchor"), at("Bramble")), (None, None), "no folder there; not installed");
    }

    #[test]
    fn a_new_database_is_at_the_latest_version_and_empty() {
        let db = Database::in_memory().unwrap();
        assert_eq!(db.version().unwrap(), MIGRATIONS.len());
        let snapshot = db.snapshot().unwrap();
        assert!(snapshot.filters.iter().all(|f| f.games.is_empty()) && !snapshot.filters.is_empty());
        assert_eq!(without_ids(snapshot), Library::default());
    }

    #[test]
    fn a_sync_adds_what_the_source_has_and_a_second_changes_nothing() {
        let mut db = Database::in_memory().unwrap();
        assert_eq!(db.sync(STEAM, &found(), NOW).unwrap(), Synced { added: 3, updated: 0, gone: 0 });
        let first = db.snapshot().unwrap();
        assert_eq!(without_ids(first.clone()), found());
        assert!(first.games.iter().all(|g| g.id > 0));

        assert_eq!(db.sync(STEAM, &found(), NOW + 60).unwrap(), Synced::default());
        assert_eq!(db.snapshot().unwrap(), first);
    }

    #[test]
    fn games_sort_by_last_played_then_by_name_without_its_article() {
        let mut db = Database::in_memory().unwrap();
        let games = vec![game(1, "The Zephyr", 0), game(2, "an Orchard", 0), game(3, "Moss", 0), game(4, "Late", 50)];
        db.sync(STEAM, &Library { games, ..Library::default() }, NOW).unwrap();
        assert_eq!(names(&db.snapshot().unwrap().games), ["Late", "Moss", "an Orchard", "The Zephyr"]);
    }

    #[test]
    fn a_sync_updates_a_game_and_keeps_its_id() {
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &found(), NOW).unwrap();
        let before = db.snapshot().unwrap();
        let bramble = before.games.iter().find(|g| g.source_id == "20").unwrap().id;

        let mut next = found();
        let changed = next.games.iter_mut().find(|g| g.source_id == "20").unwrap();
        changed.name = "Bramble: Director's Cut".into();
        changed.installed = false;
        changed.install_dir = None;
        changed.cover = None;
        changed.hero = Some("art/20-hero.jpg".into());
        assert_eq!(db.sync(STEAM, &next, NOW).unwrap(), Synced { added: 0, updated: 1, gone: 0 });

        let after = db.game(bramble).unwrap().unwrap();
        assert_eq!(after.name, "Bramble: Director's Cut");
        assert!(!after.installed);
        assert_eq!((after.install_dir, after.cover), (None, None));
        assert_eq!(after.hero, Some("art/20-hero.jpg".into()));
    }

    #[test]
    fn a_sync_takes_what_the_source_says_of_a_game_and_follows_it() {
        let mut db = Database::in_memory().unwrap();
        let mut first = found();
        let told = first.games.iter_mut().find(|g| g.source_id == "20").unwrap();
        told.released = 1_552_348_800;
        told.developers = vec!["Tin Lantern".into(), "Second Kettle".into()];
        told.publishers = vec!["Harbour & Hull".into()];
        told.size_on_disk = 48_000_000_000;
        (told.critic_score, told.community_score) = (Some(87), Some(0));
        let told = told.clone();
        db.sync(STEAM, &first, NOW).unwrap();
        let read = |db: &Database| db.snapshot().unwrap().games.into_iter().find(|g| g.source_id == "20").unwrap();
        let game = read(&db);
        assert_eq!(Game { id: 0, added: 0, art: Default::default(), ..game.clone() }, told);
        // A game the source says nothing of has nothing.
        let untold = db.snapshot().unwrap().games.into_iter().find(|g| g.source_id == "10").unwrap();
        assert!(untold.released == 0 && untold.developers.is_empty() && untold.critic_score.is_none());
        assert_eq!(db.sync(STEAM, &first, NOW).unwrap(), Synced::default());

        // A fact that changes is an update; one the source no longer has is gone.
        let changed = first.games.iter_mut().find(|g| g.source_id == "20").unwrap();
        changed.developers.pop();
        changed.critic_score = None;
        changed.size_on_disk = 50_000_000_000;
        assert_eq!(db.sync(STEAM, &first, NOW).unwrap(), Synced { added: 0, updated: 1, gone: 0 });
        let after = read(&db);
        assert_eq!((after.id, after.developers.as_slice()), (game.id, &["Tin Lantern".to_string()][..]));
        assert_eq!((after.critic_score, after.community_score, after.size_on_disk), (None, Some(0), 50_000_000_000));

        // Each Play session is one more time played, and no source's to change.
        for start in [NOW, NOW + 600] {
            let session = db.session_started(game.id, start).unwrap();
            db.session_ended(session, start + 60).unwrap();
        }
        assert_eq!(read(&db).play_count, 2);
        db.sync(STEAM, &first, NOW).unwrap();
        assert_eq!(read(&db).play_count, 2);
        assert_eq!(db.game(game.id).unwrap().unwrap().play_count, 2);
    }

    #[test]
    fn a_database_from_before_the_facts_gains_them_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(MIGRATIONS[0]).unwrap();
            conn.execute_batch(MIGRATIONS[1]).unwrap();
            conn.pragma_update(None, "user_version", 2).unwrap();
            conn.execute(
                "INSERT INTO games (source, source_id, name, sort_name, installed, added)
                 VALUES ('steam', '20', 'Bramble', 'bramble', 1, 1600000000)",
                [],
            )
            .unwrap();
        }
        let mut db = Database::open(&path).unwrap();
        assert_eq!(db.version().unwrap(), MIGRATIONS.len());
        let before = db.snapshot().unwrap().games.remove(0);
        assert!(before.released == 0 && before.publishers.is_empty() && before.community_score.is_none());
        // The next sync fills them in, and the game is the same game.
        let mut next = found();
        next.games.iter_mut().for_each(|g| g.released = 1_552_348_800);
        assert_eq!(db.sync(STEAM, &next, NOW).unwrap(), Synced { added: 2, updated: 1, gone: 0 });
        let after = db.game(before.id).unwrap().unwrap();
        assert_eq!((after.released, after.added), (1_552_348_800, 1_600_000_000));
    }

    /// A Steam game's last played that a new install's manifest put at the day of the install is taken again from
    /// Steam's record of the user's play at the next sync, and the later Play session's kept; once (#237).
    #[test]
    fn a_steam_last_played_from_an_install_is_taken_again_from_steam_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let install_day = NOW + 86_400;
        {
            let conn = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..9] {
                conn.execute_batch(migration).unwrap();
            }
            conn.pragma_update(None, "user_version", 9).unwrap();
            conn.execute_batch(&format!(
                "INSERT INTO games (id, source, source_id, name, sort_name, installed, last_played, playtime_minutes,
                                    added)
                 VALUES (7, 'steam', '20', 'Bramble', 'bramble', 1, {install_day}, 17, 1600000000),
                        (8, 'steam', '30', 'Cinder', 'cinder', 1, {install_day}, 30, 1600000000),
                        (9, 'steam', '10', 'The Anchor', 'anchor', 1, {install_day}, 0, 1600000000);
                 INSERT INTO play_sessions (game, started, ended) VALUES (8, {NOW} - 1800, {NOW});"
            ))
            .unwrap();
        }
        let mut db = Database::open(&path).unwrap();
        let played = |db: &Database, id: i64| db.game(id).unwrap().unwrap().last_played;
        // Until Steam is read, as it was: the first frame keeps its Recent row.
        assert_eq!(played(&db, 7), install_day);
        let mut steam = found();
        steam.games.iter_mut().for_each(|g| g.last_played = if g.source_id == "20" { 1_500_000_000 } else { 0 });
        db.sync(STEAM, &steam, NOW).unwrap();
        assert_eq!((played(&db, 7), played(&db, 8), played(&db, 9)), (1_500_000_000, NOW, 0));
        // Once: from now on a Play session here is not undone by a source that has not caught up.
        let session = db.session_started(7, NOW + 60).unwrap();
        db.session_ended(session, NOW + 600).unwrap();
        db.sync(STEAM, &steam, NOW + 700).unwrap();
        assert_eq!(played(&db, 7), NOW + 600);
    }

    /// The rows a database had before there was more than one source were already keyed `(source, source_id)`:
    /// Steam's are taken in by the Steam source as the same games, with all the user did with them.
    #[test]
    fn a_database_from_before_the_sources_keeps_its_games_and_what_the_user_did() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        {
            let conn = Connection::open(&path).unwrap();
            for migration in MIGRATIONS {
                conn.execute_batch(migration).unwrap();
            }
            conn.pragma_update(None, "user_version", MIGRATIONS.len() as i64).unwrap();
            conn.execute_batch(
                "INSERT INTO games (id, source, source_id, name, sort_name, installed, last_played, playtime_minutes,
                                    added, hidden, favourite)
                 VALUES (7, 'steam', '20', 'Bramble', 'bramble', 1, 1700000000, 90, 1600000000, 0, 1),
                        (8, 'steam', '30', 'Cinder', 'cinder', 1, 0, 0, 1600000000, 1, 0);
                 INSERT INTO play_sessions (game, started, ended) VALUES (7, 1700000000 - 5400, 1700000000);
                 INSERT INTO settings (key, value) VALUES ('steam.user', 'Tester'), ('steam.marks', 'taken');",
            )
            .unwrap();
        }
        let mut db = Database::open(&path).unwrap();
        assert_eq!(db.version().unwrap(), MIGRATIONS.len());
        assert_eq!(db.sync(STEAM, &found(), NOW).unwrap(), Synced { added: 1, updated: 2, gone: 0 });
        let bramble = db.game(7).unwrap().unwrap();
        assert_eq!((bramble.source.as_str(), bramble.source_id.as_str()), (STEAM, "20"));
        assert!(bramble.favourite);
        assert_eq!((bramble.added, bramble.last_played, bramble.play_count), (1_600_000_000, 1_700_000_000, 1));
        assert!(db.game(8).unwrap().unwrap().hidden);
        let snapshot = db.snapshot().unwrap();
        assert_eq!(snapshot.user.as_deref(), Some("Tester"));
        assert_eq!(names(&snapshot.hidden), ["Cinder"]);
    }

    #[test]
    fn a_game_the_source_drops_is_kept_but_not_shown_and_comes_back_as_it_was() {
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &found(), NOW).unwrap();
        let anchor = db.snapshot().unwrap().games.iter().find(|g| g.source_id == "10").unwrap().id;
        db.set_favourite(anchor, true).unwrap();
        let session = db.session_started(anchor, NOW).unwrap();
        db.session_ended(session, NOW + 600).unwrap();

        let mut without = found();
        without.games.retain(|g| g.source_id != "10");
        assert_eq!(db.sync(STEAM, &without, NOW + 700).unwrap(), Synced { added: 0, updated: 0, gone: 1 });
        assert_eq!(db.sync(STEAM, &without, NOW + 800).unwrap(), Synced::default());
        assert_eq!(names(&db.snapshot().unwrap().games), ["Cinder", "Bramble"]);
        let kept = db.game(anchor).unwrap().unwrap();
        assert!(!kept.installed && kept.install_dir.is_none());

        assert_eq!(db.sync(STEAM, &found(), NOW + 900).unwrap(), Synced { added: 0, updated: 1, gone: 0 });
        let back = db.game(anchor).unwrap().unwrap();
        assert_eq!(back.id, anchor);
        assert!(back.favourite && back.installed);
        assert_eq!(back.last_played, NOW + 600);
        assert_eq!(db.sessions(anchor).unwrap().len(), 1);
        assert_eq!(names(&db.snapshot().unwrap().games), ["The Anchor", "Cinder", "Bramble"]);
    }

    #[test]
    fn a_sync_leaves_what_is_the_users_own() {
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &found(), NOW).unwrap();
        let id =
            |appid: u32| db.snapshot().unwrap().games.iter().find(|g| g.source_id == appid.to_string()).unwrap().id;
        let (cinder, bramble) = (id(30), id(20));
        db.set_hidden(cinder, true).unwrap();
        db.set_favourite(bramble, true).unwrap();
        let session = db.session_started(bramble, NOW).unwrap();
        db.session_ended(session, NOW + 45 * 60).unwrap();

        // The source has not heard of the session: its older last played and playtime do not undo it.
        assert_eq!(db.sync(STEAM, &found(), NOW + 3600).unwrap(), Synced::default());
        let snapshot = db.snapshot().unwrap();
        assert_eq!(names(&snapshot.games), ["Bramble", "The Anchor"]);
        let played = &snapshot.games[0];
        assert!(played.favourite);
        assert_eq!((played.last_played, played.playtime_minutes), (NOW + 45 * 60, 45));
        assert_eq!(db.sessions(bramble).unwrap().len(), 1);
        // Hidden, not gone.
        assert_eq!(db.game(cinder).unwrap().unwrap().name, "Cinder");

        // When the source has counted more, its count is taken.
        let mut later = found();
        later.games.iter_mut().find(|g| g.source_id == "20").unwrap().playtime_minutes = 50;
        assert_eq!(db.sync(STEAM, &later, NOW + 7200).unwrap(), Synced { added: 0, updated: 1, gone: 0 });
        assert_eq!(db.game(bramble).unwrap().unwrap().playtime_minutes, 50);
    }

    #[test]
    fn a_play_session_is_recorded_with_its_last_played_and_playtime() {
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &found(), NOW).unwrap();
        let bramble = db.snapshot().unwrap().games.iter().find(|g| g.source_id == "20").unwrap().id;

        let session = db.session_started(bramble, NOW).unwrap();
        assert_eq!(db.sessions(bramble).unwrap(), [PlaySession { game: bramble, started: NOW, ended: None }]);
        // Last played from the moment it booted: first in the Recent row.
        assert_eq!(names(&db.recent(2).unwrap()), ["Bramble", "Cinder"]);
        assert_eq!(db.game(bramble).unwrap().unwrap().playtime_minutes, 0);

        db.session_ended(session, NOW + 125).unwrap();
        let ended = PlaySession { game: bramble, started: NOW, ended: Some(NOW + 125) };
        assert_eq!(db.sessions(bramble).unwrap(), [ended]);
        let game = db.game(bramble).unwrap().unwrap();
        assert_eq!((game.last_played, game.playtime_minutes), (NOW + 125, 2));

        // Ending it again counts nothing twice.
        db.session_ended(session, NOW + 900).unwrap();
        assert_eq!(db.sessions(bramble).unwrap(), [ended]);
        assert_eq!(db.game(bramble).unwrap().unwrap().playtime_minutes, 2);
    }

    #[test]
    fn the_recent_row_is_the_played_games_newest_first() {
        let mut db = Database::in_memory().unwrap();
        db.sync(STEAM, &found(), NOW).unwrap();
        assert_eq!(names(&db.recent(8).unwrap()), ["Cinder", "The Anchor"]);
        assert_eq!(names(&db.recent(1).unwrap()), ["Cinder"]);
        assert!(db.recent(1).unwrap()[0].cover.is_some());
        assert_eq!(db.game(999).unwrap(), None);
    }

    #[test]
    fn settings_are_kept_by_key() {
        let db = Database::in_memory().unwrap();
        assert_eq!(db.setting("reduce motion").unwrap(), None);
        db.set_setting("reduce motion", "on").unwrap();
        db.set_setting("reduce motion", "off").unwrap();
        assert_eq!(db.setting("reduce motion").unwrap().as_deref(), Some("off"));
    }

    /// Settings' options are rows of the settings table (docs/spec/settings.md, 16.4): one is there for the next
    /// Dashboard that opens the file, and a sync, which keeps the source's own rows there, leaves it alone.
    #[test]
    fn a_setting_outlives_the_connection_and_a_sync() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        {
            let mut db = Database::open(&path).unwrap();
            db.set_setting("motion.reduce", "true").unwrap();
            let found =
                Library { user: Some("Player One".into()), collections: Some(Vec::new()), ..Library::default() };
            db.sync(STEAM, &found, 1_780_000_000).unwrap();
        }
        let db = Database::open(&path).unwrap();
        assert_eq!(db.setting("motion.reduce").unwrap().as_deref(), Some("true"));
        assert_eq!(db.setting("home.background_delay").unwrap(), None, "one never set has no row");
        assert_eq!(db.setting("steam.user").unwrap().as_deref(), Some("Player One"));
    }

    #[test]
    fn a_database_file_is_made_with_its_folder_and_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data").join(FILE);
        let first = {
            let mut db = Database::open(&path).unwrap();
            db.sync(STEAM, &found(), NOW).unwrap();
            db.snapshot().unwrap()
        };
        let db = Database::open(&path).unwrap();
        assert_eq!(db.version().unwrap(), MIGRATIONS.len());
        assert_eq!(db.snapshot().unwrap(), first);
    }

    #[test]
    fn a_database_from_a_newer_program_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        drop(Database::open(&path).unwrap());
        Connection::open(&path).unwrap().pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1).unwrap();
        assert!(Database::open(&path).is_err());
    }

    #[test]
    fn the_fixture_steam_install_goes_in_and_comes_out_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let mut spec = Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let read = source_steam::load(&root).unwrap();

        let mut db = Database::open(&dir.path().join(FILE)).unwrap();
        assert_eq!(db.sync(STEAM, &read, NOW).unwrap(), Synced { added: spec.games.len(), updated: 0, gone: 0 });
        let snapshot = db.snapshot().unwrap();
        assert_eq!(without_ids(snapshot.clone()), games_of(read));
        assert_eq!(snapshot.recent(8), db.recent(8).unwrap());
        assert_eq!(db.sync(STEAM, &source_steam::load(&root).unwrap(), NOW).unwrap(), Synced::default());

        // The user hides one game, marks another and plays it; then Steam uninstalls that one and plays a third.
        let (hidden, marked) = (snapshot.games[1].id, snapshot.games[0].id);
        db.set_hidden(hidden, true).unwrap();
        db.set_favourite(marked, true).unwrap();
        let session = db.session_started(marked, NOW).unwrap();
        db.session_ended(session, NOW + 3600).unwrap();
        spec.games[0].installed = false;
        spec.games[2].last_played = NOW + 7200;
        let again = dir.path().join("steam again");
        fixture::write(&again, &spec).unwrap();

        let synced = db.sync(STEAM, &source_steam::load(&again).unwrap(), NOW + 9000).unwrap();
        assert_eq!((synced.added, synced.gone), (0, 0));
        let after = db.snapshot().unwrap();
        assert_eq!(after.games.len(), spec.games.len() - 1);
        assert_eq!(names(&after.games[..2]), ["Brass Meridian", "Ashfall Courier"]);
        let played = &after.games[1];
        assert_eq!(played.id, marked);
        assert!(played.favourite && !played.installed && played.install_dir.is_none());
        assert_eq!(played.last_played, NOW + 3600);
        assert_eq!(played.playtime_minutes, spec.games[0].playtime_minutes + 60);
        assert_eq!(db.sessions(marked).unwrap().len(), 1);
        // Uninstalled, it takes no room; who made it and when is as Steam said.
        assert_eq!((played.play_count, played.size_on_disk), (1, 0));
        assert_eq!(played.developers, spec.games[0].facts.developers);
        assert_eq!(played.released, spec.games[0].facts.released);
        assert!(after.games.iter().all(|g| g.id != hidden));
    }
}
