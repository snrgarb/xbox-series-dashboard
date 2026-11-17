//! Achievements in the library database (docs/achievements.md; decided in #7): each store's, read through
//! `model::AchievementSource` from its client's own files, kept per game (migration 7) with how rare each is, and
//! followed on a thread of their own ([`spawn`]):
//!
//! 1. As it starts, every game's achievements are read and kept, and said in the log (`achievements: <appid> <n> of
//!    <m>, score <s>`, then the totals). What was already unlocked then is never told of as new.
//! 2. How rare each is, the share of the store's players who have it, from the store's own public figures (Steam's
//!    keyless `GetGlobalAchievementPercentagesForApp`), asked once a week at most per game and kept with the date it
//!    was asked: offline, what was kept stands, and nothing waits on it.
//! 3. While a game runs, and after it until another starts, as the Bundle tells of the game that just exited, its
//!    files are looked at every second ([`WATCH`]): an achievement newly unlocked since the game started is an
//!    [`Update::unlocked`], for the Achievement unlocked Toast (#142).
//! 4. Icons are fetched into our own cache ([`IconDir`]) as art is, off the frame and only with Settings' Download
//!    art online on: those of each new unlock before it is told of, those of the game that is running, the last
//!    unlocks', and any game's on request ([`Ask::Icons`], for its Achievements page).
//!
//! Totals are this PC's, as the Bundle's are, not a Profile's own: every Profile shows the same.
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use model::achievement::{
    nearest_locked, Achievement, AchievementSource, Achievements, GameProgress, Totals, Unlock, RECENT,
};
use model::Library;
use rusqlite::{params, OptionalExtension};

use crate::art::cache::{escape, extension};
use crate::art::steamgriddb::{self, retry_after, transport, Error};
use crate::launchers::shown;
use crate::Database;

/// Migration 7: every game's achievements, and when the store's figures of how rare they are were last asked for.
pub const MIGRATION: &str = "CREATE TABLE achievements (
        game INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        -- The store's own name for it (Steam's API name).
        key TEXT NOT NULL,
        -- Its place in the store's list.
        position INTEGER NOT NULL,
        name TEXT NOT NULL,
        description TEXT,
        hidden INTEGER NOT NULL DEFAULT 0,
        -- Where its icons are online.
        icon TEXT,
        icon_locked TEXT,
        -- Its points on its own store, where it has them.
        points INTEGER,
        -- The share of the store's players who have it, 0 to 100; none while not known.
        rarity REAL,
        -- Unix seconds; none while locked; 0 when unlocked at a time not known.
        unlocked INTEGER,
        PRIMARY KEY (game, key)
    ) WITHOUT ROWID;
    CREATE TABLE achievement_rarity (
        game INTEGER PRIMARY KEY REFERENCES games (id) ON DELETE CASCADE,
        -- Unix seconds: when the store was last asked how rare the game's achievements are.
        asked INTEGER NOT NULL,
        -- Whether it said.
        found INTEGER NOT NULL
    );";

/// The folder in the data folder that keeps the icons.
pub const FOLDER: &str = "achievements";

/// How long the store's figures of how rare a game's achievements are stand before they are asked for again.
pub const RARITY_FOR: u64 = 7 * 86_400;

/// How often the running game's files are looked at.
pub const WATCH: Duration = Duration::from_secs(1);

/// The most an icon may be: Steam's are a few kilobytes.
const ICON_BYTES: u64 = 4 << 20;

/// The gap between two requests online.
const GAP: Duration = Duration::from_millis(100);

/// How long before the game was seen running an unlock may be and still be its session's: the Launch screen sees
/// a game running a few seconds after it has started.
const SESSION_SLACK: u64 = 60;

/// How often, at most, the thread sends a snapshot while it has more to fetch.
const SNAPSHOT_EVERY: Duration = Duration::from_secs(5);

/// Where a line of the log goes.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

// ----------------------------------------------------------------------------- icons

/// Our own cache of achievement icons: `achievements/<source>/<source_id>/<file>`, the file the icon's own file
/// name, each a folder's name as the art cache writes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconDir {
    root: PathBuf,
}

impl IconDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The cache in a data folder.
    pub fn under(data_dir: &Path) -> Self {
        Self::new(data_dir.join(FOLDER))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the icon at `url` of a game is kept.
    pub fn path(&self, source: &str, source_id: &str, url: &str) -> PathBuf {
        let file = url.split(['?', '#']).next().unwrap_or(url).rsplit('/').next().unwrap_or(url);
        self.root.join(escape(source)).join(escape(source_id)).join(escape(file))
    }

    /// The icon, if the cache has it.
    pub fn find(&self, source: &str, source_id: &str, url: &str) -> Option<PathBuf> {
        Some(self.path(source, source_id, url)).filter(|path| path.is_file())
    }

    fn save(&self, source: &str, source_id: &str, url: &str, bytes: &[u8]) -> Result<PathBuf> {
        let path = self.path(source, source_id, url);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let part = path.with_extension("part");
        std::fs::write(&part, bytes)?;
        std::fs::rename(&part, &path)?;
        Ok(path)
    }
}

// ----------------------------------------------------------------------------- the database

/// A row of `achievements` with its game's source and id, as `ROW` selects it.
const ROW: &str = "a.game, g.source, g.source_id, a.key, a.name, a.description, a.hidden, a.icon, a.icon_locked,
                   a.points, a.rarity, a.unlocked
                   FROM achievements a JOIN games g ON g.id = a.game";

impl Database {
    /// The game's id, by its source and its id there, if it is in the library, not gone and its launcher switched on.
    pub fn game_id(&self, source: &str, source_id: &str) -> Result<Option<i64>> {
        let id = self.conn.query_row(
            &format!(
                "SELECT id FROM games WHERE source = ?1 AND source_id = ?2 AND {}",
                crate::launchers::shown("games")
            ),
            params![source, source_id],
            |row| row.get(0),
        );
        Ok(id.optional()?)
    }

    /// The icon cache beside this database; none for one in memory.
    pub fn achievement_icons(&self) -> Option<IconDir> {
        self.folder.as_deref().map(IconDir::under)
    }

    /// Keeps a game's achievements as its store has just read them: each takes the store's name, description,
    /// icons and whether it is unlocked and when, keeping the rarity it had; one the store no longer lists goes.
    /// Gives those newly unlocked, as kept, rarity and all.
    pub fn put_achievements(&mut self, game: i64, read: &[Achievement]) -> Result<Vec<Achievement>> {
        let tx = self.conn.transaction()?;
        let before: HashMap<String, bool> = {
            let mut rows = tx.prepare("SELECT key, unlocked IS NOT NULL FROM achievements WHERE game = ?1")?;
            let rows = rows.query_map([game], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        {
            let mut put = tx.prepare(
                "INSERT INTO achievements
                     (game, key, position, name, description, hidden, icon, icon_locked, points, unlocked)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT (game, key) DO UPDATE SET
                     position = excluded.position, name = excluded.name, description = excluded.description,
                     hidden = excluded.hidden, icon = excluded.icon, icon_locked = excluded.icon_locked,
                     points = excluded.points, unlocked = excluded.unlocked",
            )?;
            for (place, a) in read.iter().enumerate() {
                put.execute(params![
                    game,
                    a.key,
                    place as i64,
                    a.name,
                    a.description,
                    a.hidden,
                    a.icon,
                    a.icon_locked,
                    a.points,
                    a.unlocked.map(|at| at as i64)
                ])?;
            }
            let listed: HashSet<&str> = read.iter().map(|a| a.key.as_str()).collect();
            let mut drop = tx.prepare("DELETE FROM achievements WHERE game = ?1 AND key = ?2")?;
            for key in before.keys().filter(|key| !listed.contains(key.as_str())) {
                drop.execute(params![game, key])?;
            }
        }
        tx.commit()?;
        let newly: HashSet<&str> = read
            .iter()
            .filter(|a| a.is_unlocked() && !before.get(&a.key).copied().unwrap_or(false))
            .map(|a| a.key.as_str())
            .collect();
        Ok(self.achievements(game)?.into_iter().filter(|a| newly.contains(a.key.as_str())).collect())
    }

    /// One game's achievements, in its store's order, with their rarity and the icons the cache has: what its
    /// Achievements page shows (#141).
    pub fn achievements(&self, game: i64) -> Result<Vec<Achievement>> {
        let icons = self.achievement_icons();
        let mut rows = self.conn.prepare(&format!("SELECT {ROW} WHERE a.game = ?1 ORDER BY a.position"))?;
        let rows = rows.query_map([game], |row| row_achievement(row, icons.as_ref()))?;
        Ok(rows.map(|row| row.map(|(_, a)| a)).collect::<rusqlite::Result<_>>()?)
    }

    /// Keeps how rare the game's achievements are, as its store has just said (by each achievement's key, any case),
    /// and that it was asked at `now`; with `None`, the store had no figures for it: only that it was asked.
    pub fn set_rarity(&mut self, game: i64, rarity: Option<&HashMap<String, f64>>, now: u64) -> Result<()> {
        let tx = self.conn.transaction()?;
        if let Some(rarity) = rarity {
            let lower: HashMap<String, f64> = rarity.iter().map(|(k, v)| (k.to_lowercase(), *v)).collect();
            let keys: Vec<String> = {
                let mut rows = tx.prepare("SELECT key FROM achievements WHERE game = ?1")?;
                let rows = rows.query_map([game], |row| row.get(0))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            let mut set = tx.prepare("UPDATE achievements SET rarity = ?3 WHERE game = ?1 AND key = ?2")?;
            for key in keys {
                let found = rarity.get(&key).or_else(|| lower.get(&key.to_lowercase()));
                set.execute(params![game, key, found])?;
            }
        }
        tx.execute(
            "INSERT INTO achievement_rarity (game, asked, found) VALUES (?1, ?2, ?3)
             ON CONFLICT (game) DO UPDATE SET asked = excluded.asked, found = excluded.found",
            params![game, now as i64, rarity.is_some()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// When the store was last asked how rare the game's achievements are.
    pub fn rarity_asked(&self, game: i64) -> Result<Option<u64>> {
        let asked: Option<i64> = self
            .conn
            .query_row("SELECT asked FROM achievement_rarity WHERE game = ?1", [game], |row| row.get(0))
            .optional()?;
        Ok(asked.map(|at| at.max(0) as u64))
    }

    /// The games that have achievements, by id, with their source and id there.
    fn achievement_games(&self) -> Result<Vec<(i64, String, String)>> {
        let mut rows = self.conn.prepare(&format!(
            "SELECT g.id, g.source, g.source_id FROM games g
             WHERE {} AND EXISTS (SELECT 1 FROM achievements a WHERE a.game = g.id) ORDER BY g.id",
            crate::launchers::shown("g")
        ))?;
        let rows = rows.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// What a snapshot carries: the totals over every game that is not gone, how far each has got, the locked one
    /// nearest to hand of each game part done, and the last unlocks, newest first.
    pub fn achievement_summary(&self) -> Result<Achievements> {
        let icons = self.achievement_icons();
        let mut games: Vec<GameProgress> = Vec::new();
        // Each game's list, in its store's order, while it is read.
        let mut lists: Vec<Vec<Achievement>> = Vec::new();
        {
            let mut rows =
                self.conn.prepare(&format!("SELECT {ROW} WHERE {} ORDER BY a.game, a.position", shown("g")))?;
            for row in rows.query_map([], |row| row_achievement(row, None))? {
                let (game, a) = row?;
                if games.last().is_none_or(|last| last.game != game) {
                    games.push(GameProgress { game, ..GameProgress::default() });
                    lists.push(Vec::new());
                }
                let progress = games.last_mut().expect("just pushed");
                progress.of += 1;
                if a.is_unlocked() {
                    progress.unlocked += 1;
                    progress.score += u64::from(a.score());
                }
                lists.last_mut().expect("just pushed").push(a);
            }
        }
        let next = games
            .iter()
            .zip(&lists)
            .filter(|(progress, _)| progress.unlocked > 0 && progress.unlocked < progress.of)
            .filter_map(|(progress, list)| {
                let achievement = nearest_locked(list)?.clone();
                Some(Unlock { game: progress.game, achievement })
            })
            .collect();
        if games.is_empty() {
            return Ok(Achievements::default());
        }
        let totals = Totals {
            score: games.iter().map(|g| g.score).sum(),
            unlocked: games.iter().map(|g| g.unlocked).sum(),
            of: games.iter().map(|g| g.of).sum(),
            games: games.len() as u32,
        };
        let mut rows = self.conn.prepare(&format!(
            "SELECT {ROW} WHERE {} AND a.unlocked IS NOT NULL ORDER BY a.unlocked DESC, a.game, a.position
             LIMIT {RECENT}",
            shown("g")
        ))?;
        let recent = rows
            .query_map([], |row| row_achievement(row, icons.as_ref()))?
            .map(|row| row.map(|(game, achievement)| Unlock { game, achievement }))
            .collect::<rusqlite::Result<_>>()?;
        Ok(Achievements { totals: Some(totals), recent, games, next })
    }
}

/// An achievement from a row `ROW` selects, with its game's id, and its icons from `icons` where it has them.
fn row_achievement(row: &rusqlite::Row, icons: Option<&IconDir>) -> rusqlite::Result<(i64, Achievement)> {
    let (source, source_id): (String, String) = (row.get(1)?, row.get(2)?);
    let icon: Option<String> = row.get(7)?;
    let icon_locked: Option<String> = row.get(8)?;
    let cached = |url: &Option<String>| icons.and_then(|dir| dir.find(&source, &source_id, url.as_deref()?));
    let achievement = Achievement {
        key: row.get(3)?,
        name: row.get(4)?,
        description: row.get(5)?,
        hidden: row.get(6)?,
        icon_file: cached(&icon),
        icon_locked_file: cached(&icon_locked),
        icon,
        icon_locked,
        points: row.get::<_, Option<i64>>(9)?.map(|p| p.clamp(0, u32::MAX as i64) as u32),
        rarity: row.get(10)?,
        unlocked: row.get::<_, Option<i64>>(11)?.map(|at| at.max(0) as u64),
    };
    Ok((row.get(0)?, achievement))
}

// ----------------------------------------------------------------------------- online

/// Where the stores' figures and the icons are asked for: their own services, or a test's stand-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosts {
    /// Steam's Web API.
    pub steam_api: String,
    /// With a stand-in, every icon's address goes to it: `<icons>/<host>/<path>`. None: the icon's own address.
    pub icons: Option<String>,
}

impl Default for Hosts {
    fn default() -> Self {
        Self { steam_api: "https://api.steampowered.com".into(), icons: None }
    }
}

impl Hosts {
    /// Everything at one stand-in (`library::art::fake`, `--store-art-service`): `<base>/steam-api`, `<base>/icons`.
    pub fn all_at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self { steam_api: format!("{base}/steam-api"), icons: Some(format!("{base}/icons")) }
    }

    fn icon(&self, url: &str) -> String {
        match (&self.icons, url.split_once("://")) {
            (Some(base), Some((_, rest))) => format!("{base}/{rest}"),
            _ => url.to_string(),
        }
    }
}

/// What is asked online: how rare a game's achievements are, and an icon. Behind a trait so the tests never touch
/// the network.
pub trait Fetch: Send {
    /// The share of players who have each of the game's achievements, by key; Ok(None) when the store has no
    /// figures for the game (Steam answers 403 for an app with none public); an error when it can't be asked now.
    fn rarity(&self, source: &str, source_id: &str) -> Result<Option<HashMap<String, f64>>, Error>;

    /// An icon's bytes; Ok(None) when the server says it has no such icon (400, 403, 404, 410); an error when it
    /// can't be asked now (offline, a 429 or 5xx).
    fn icon(&self, url: &str) -> Result<Option<Vec<u8>>, Error>;
}

/// The stores' own services.
pub struct Online {
    hosts: Hosts,
    agent: ureq::Agent,
}

impl Online {
    pub fn new(hosts: Hosts) -> Self {
        Self { hosts, agent: steamgriddb::agent() }
    }
}

impl Fetch for Online {
    /// Steam's only: `ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2/?gameid=<appid>`, no key. Checked on
    /// 2026-11-17 with 620 (Portal 2): `{"achievementpercentages":{"achievements":[{"name":"ACH.WAKE_UP",
    /// "percent":"64.7"},...]}}`, each percent a string; an unknown app is `403 {}`.
    fn rarity(&self, source: &str, source_id: &str) -> Result<Option<HashMap<String, f64>>, Error> {
        if source != "steam" || source_id.is_empty() || !source_id.bytes().all(|b| b.is_ascii_digit()) {
            return Ok(None);
        }
        let url = format!(
            "{}/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2/?gameid={source_id}",
            self.hosts.steam_api
        );
        let mut response = self.agent.get(&url).call().map_err(|e| transport(e, "Steam"))?;
        match response.status().as_u16() {
            200..=299 => {}
            400 | 403 | 404 => return Ok(None),
            429 | 503 => return Err(Error::Busy(retry_after(&response))),
            status => return Err(Error::Failed(format!("Steam answered {status}"))),
        }
        let text = response.body_mut().read_to_string().map_err(|e| transport(e, "Steam"))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| Error::Failed("Steam's answer is not JSON".into()))?;
        Ok(percentages(&json))
    }

    fn icon(&self, url: &str) -> Result<Option<Vec<u8>>, Error> {
        let url = self.hosts.icon(url);
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Ok(None);
        }
        let mut response = self.agent.get(steamgriddb::tidy(&url)).call().map_err(|e| transport(e, "Steam's CDN"))?;
        match response.status().as_u16() {
            200..=299 => {}
            400 | 403 | 404 | 410 => return Ok(None),
            status @ (429 | 503) => {
                return Err(Error::Failed(format!(
                    "Steam's CDN answered {status}, retry-after {:?}",
                    retry_after(&response)
                )))
            }
            status => return Err(Error::Failed(format!("Steam's CDN answered {status}"))),
        }
        let body = response.body_mut().with_config().limit(ICON_BYTES).read_to_vec();
        body.map(Some).map_err(|e| transport(e, "Steam's CDN"))
    }
}

/// The percentages in Steam's answer, by API name: each a string or a number, kept to 0..100.
pub fn percentages(json: &serde_json::Value) -> Option<HashMap<String, f64>> {
    let list = json.pointer("/achievementpercentages/achievements")?.as_array()?;
    let percent = |v: &serde_json::Value| v.as_f64().or_else(|| v.as_str()?.trim().parse().ok());
    let found = list.iter().filter_map(|entry| {
        let name = entry.get("name")?.as_str()?.to_string();
        Some((name, percent(entry.get("percent")?)?.clamp(0.0, 100.0)))
    });
    Some(found.collect())
}

/// Why a request failed, as the log says it.
fn why(error: &Error) -> String {
    match error {
        Error::KeyRefused => "it was refused".into(),
        Error::Busy(_) => "the service asked to wait".into(),
        Error::Failed(why) => why.clone(),
    }
}

// ----------------------------------------------------------------------------- following

/// What the thread is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// A game is running, since `since` (Unix seconds): its files are watched from now on, and after it ends, until
    /// another starts. Its rarity is asked for first if it is due, and its icons fetched.
    Playing { game: i64, since: u64 },
    /// Fetch every icon of this game: its Achievements page is showing (#141).
    Icons(i64),
    /// Read every game's achievements again, and ask for the rarity that is due: Settings > Launchers' Update now
    /// (docs/spec/launchers.md). A new snapshot follows.
    ReadAll,
}

/// What the thread has to say.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Update {
    /// Achievements newly unlocked in the game being watched, since it started: the Achievement unlocked Toast's
    /// (#142). Several in one update were found together: "<n> achievements unlocked".
    pub unlocked: Vec<Unlock>,
    /// A new snapshot of the library, when what it carries of achievements changed.
    pub library: Option<Library>,
    /// The games some icons came in for.
    pub icons: Vec<i64>,
}

/// The thread following achievements.
pub struct Handle {
    asks: Sender<Ask>,
    updates: Receiver<Update>,
}

impl Handle {
    pub fn ask(&self, ask: Ask) {
        let _ = self.asks.send(ask);
    }

    /// Everything the thread has said since this was last asked, as one: every unlock in turn, the latest snapshot.
    pub fn take(&self) -> Option<Update> {
        let mut all: Option<Update> = None;
        while let Ok(update) = self.updates.try_recv() {
            let mut so_far = all.take().unwrap_or_default();
            so_far.unlocked.extend(update.unlocked);
            so_far.library = update.library.or(so_far.library);
            for game in update.icons {
                if !so_far.icons.contains(&game) {
                    so_far.icons.push(game);
                }
            }
            all = Some(so_far);
        }
        all
    }
}

/// Follows achievements on a thread of its own, with its own connection to the database at `path`: the read as
/// it starts, the rarity and icons online through `fetch` (none: nothing is asked online; `icons` false: no icon
/// is), and the running game's files once it is asked to. Each change is sent and followed by `wake`.
pub fn spawn(
    path: PathBuf,
    sources: Vec<Arc<dyn AchievementSource>>,
    fetch: Option<Box<dyn Fetch>>,
    icons: bool,
    log: Log,
    wake: impl Fn() + Send + 'static,
) -> Handle {
    let (asks, asked) = mpsc::channel();
    let (send, updates) = mpsc::channel();
    std::thread::spawn(move || {
        let run = || -> Result<()> {
            let mut db = Database::open(&path)?;
            let mut follower = Follower::new(sources, fetch, icons, log.clone());
            follower.read_all(&mut db, crate::now())?;
            if send.send(Update { library: Some(db.snapshot()?), ..Update::default() }).is_err() {
                return Ok(());
            }
            wake();
            follow(&mut db, &mut follower, &asked, &send, &wake)
        };
        if let Err(e) = run() {
            log(&format!("achievements: {}: {e:#}", path.display()));
        }
    });
    Handle { asks, updates }
}

fn follow(
    db: &mut Database,
    follower: &mut Follower,
    asked: &Receiver<Ask>,
    send: &Sender<Update>,
    wake: &dyn Fn(),
) -> Result<()> {
    let mut dirty = false;
    let mut icons: Vec<i64> = Vec::new();
    let mut sent = Instant::now();
    loop {
        let wait = if follower.has_work() {
            GAP
        } else if follower.watching() {
            WATCH
        } else {
            Duration::from_secs(3600)
        };
        match asked.recv_timeout(wait) {
            Ok(ask) => {
                dirty |= ask == Ask::ReadAll;
                follower.ask(db, ask, crate::now())?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        while let Ok(ask) = asked.try_recv() {
            dirty |= ask == Ask::ReadAll;
            follower.ask(db, ask, crate::now())?;
        }
        let unlocked = follower.look(db)?;
        let worked = follower.work(db, crate::now())?;
        dirty |= worked.changed;
        for game in worked.icons {
            if !icons.contains(&game) {
                icons.push(game);
            }
        }
        let due = !follower.has_work() || sent.elapsed() >= SNAPSHOT_EVERY;
        if !unlocked.is_empty() || (due && (dirty || !icons.is_empty())) {
            let library = if !unlocked.is_empty() || dirty { Some(db.snapshot()?) } else { None };
            let update = Update { unlocked, library, icons: std::mem::take(&mut icons) };
            if send.send(update).is_err() {
                return Ok(());
            }
            dirty = false;
            sent = Instant::now();
            wake();
        }
    }
}

/// What one piece of work online did.
#[derive(Debug, Default)]
pub struct Worked {
    /// What a snapshot carries changed: a rarity, which changes a Score.
    pub changed: bool,
    /// The games an icon came in for.
    pub icons: Vec<i64>,
}

/// The game being watched.
struct Watched {
    game: i64,
    source: Arc<dyn AchievementSource>,
    source_id: String,
    /// Unlocks from before this are an older session's.
    since: u64,
    /// The files as last read: a change to any is a new read.
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
}

/// What follows achievements: the thread's state, without the thread, so the tests can drive it.
pub struct Follower {
    sources: Vec<Arc<dyn AchievementSource>>,
    fetch: Option<Box<dyn Fetch>>,
    icons_online: bool,
    log: Log,
    watched: Option<Watched>,
    /// Games whose rarity is to be asked for, in turn.
    rarity: VecDeque<i64>,
    /// Icons to fetch, in turn: the game, and the icon's address.
    icons: VecDeque<(i64, String)>,
    /// The icons the server did not have this run, by address: not asked for again.
    missing: HashSet<String>,
    /// Offline, or the icons' server in trouble: no icon is asked for again this run.
    icons_stopped: bool,
}

impl Follower {
    pub fn new(sources: Vec<Arc<dyn AchievementSource>>, fetch: Option<Box<dyn Fetch>>, icons: bool, log: Log) -> Self {
        Self {
            sources,
            fetch,
            icons_online: icons,
            log,
            watched: None,
            rarity: VecDeque::new(),
            icons: VecDeque::new(),
            missing: HashSet::new(),
            icons_stopped: false,
        }
    }

    fn say(&self, line: &str) {
        (self.log)(line)
    }

    /// Reads every source's achievements into the database at `now`, telling of none as new, and says each game's
    /// and the totals in the log. The games whose rarity is due, and the last unlocks' icons, are queued.
    pub fn read_all(&mut self, db: &mut Database, now: u64) -> Result<()> {
        let started = Instant::now();
        for source in self.sources.clone() {
            let Some(ids) = source.games() else { continue };
            for source_id in ids {
                let Some(game) = db.game_id(source.key(), &source_id)? else { continue };
                match source.read(&source_id) {
                    Ok(Some(read)) => {
                        db.put_achievements(game, &read)?;
                        self.say_game(db, game, &source_id)?;
                    }
                    Ok(None) => {}
                    Err(e) => self.say(&format!("achievements: {source_id}: {e:#}")),
                }
            }
        }
        let summary = db.achievement_summary()?;
        if let Some(t) = summary.totals {
            let ms = started.elapsed().as_millis();
            self.say(&format!(
                "achievements: {} games, {} of {} unlocked, score {}, read in {ms} ms",
                t.games, t.unlocked, t.of, t.score
            ));
        }
        for (game, _, _) in db.achievement_games()? {
            if rarity_due(db, game, now)? && !self.rarity.contains(&game) {
                self.rarity.push_back(game);
            }
        }
        for unlock in &summary.recent {
            self.want_icon(unlock.game, &unlock.achievement, false);
        }
        Ok(())
    }

    /// `achievements: <id> <n> of <m>, score <s>`: how far the game has got.
    fn say_game(&self, db: &Database, game: i64, source_id: &str) -> Result<()> {
        let all = db.achievements(game)?;
        let unlocked = all.iter().filter(|a| a.is_unlocked());
        let score: u64 = unlocked.clone().map(|a| u64::from(a.score())).sum();
        self.say(&format!("achievements: {source_id} {} of {}, score {score}", unlocked.count(), all.len()));
        Ok(())
    }

    fn want_icon(&mut self, game: i64, a: &Achievement, locked_too: bool) {
        if !self.icons_online || self.fetch.is_none() || self.icons_stopped {
            return;
        }
        let mut want = |url: &Option<String>, file: &Option<PathBuf>| {
            let url = url.as_deref().filter(|url| file.is_none() && !url.trim().is_empty())?;
            let queued = self.icons.iter().any(|(g, u)| *g == game && u == url);
            (!queued && !self.missing.contains(url)).then(|| self.icons.push_back((game, url.to_string())))
        };
        want(&a.icon, &a.icon_file);
        if locked_too {
            want(&a.icon_locked, &a.icon_locked_file);
        }
    }

    /// Whether there is something still to fetch online.
    pub fn has_work(&self) -> bool {
        self.fetch.is_some() && (!self.rarity.is_empty() || !self.icons.is_empty())
    }

    /// Whether a game's files are being watched.
    pub fn watching(&self) -> bool {
        self.watched.is_some()
    }

    /// Takes in what was asked, at `now`.
    pub fn ask(&mut self, db: &mut Database, ask: Ask, now: u64) -> Result<()> {
        match ask {
            Ask::Playing { game, since } => self.watch(db, game, since, now),
            Ask::Icons(game) => {
                for a in db.achievements(game)? {
                    self.want_icon(game, &a, true);
                }
                Ok(())
            }
            Ask::ReadAll => self.read_all(db, now),
        }
    }

    /// Watches the game's files from now on, the game started at `since`: they are read now, telling of an unlock
    /// only if it is from since then. A game of no source that has achievements is not watched, and the one
    /// watched before stays so.
    fn watch(&mut self, db: &mut Database, game: i64, since: u64, now: u64) -> Result<()> {
        let Some(found) = db.game(game)? else { return Ok(()) };
        let Some(source) = self.sources.iter().find(|s| s.key() == found.source).cloned() else { return Ok(()) };
        let files = source.files(&found.source_id).into_iter().map(|path| (path, None)).collect();
        self.say(&format!("achievements: watching {} ({})", found.source_id, found.name));
        self.watched = Some(Watched { game, source, source_id: found.source_id, since, files });
        if rarity_due(db, game, now)? && !self.rarity.contains(&game) {
            self.rarity.push_front(game);
        }
        for a in db.achievements(game)? {
            self.want_icon(game, &a, false);
        }
        Ok(())
    }

    /// Looks at the watched game's files: when any has changed, they are read again, kept, and the unlocks of this
    /// session that are new are given, each said in the log with its icon fetched first where it can be.
    pub fn look(&mut self, db: &mut Database) -> Result<Vec<Unlock>> {
        let Some(watched) = &mut self.watched else { return Ok(Vec::new()) };
        let mut changed = false;
        for (path, last) in &mut watched.files {
            let now = std::fs::read(path).ok();
            if now != *last {
                (*last, changed) = (now, true);
            }
        }
        if !changed {
            return Ok(Vec::new());
        }
        let (game, since, source_id, source) =
            (watched.game, watched.since, watched.source_id.clone(), watched.source.clone());
        let read = match source.read(&source_id) {
            Ok(Some(read)) => read,
            Ok(None) => return Ok(Vec::new()),
            Err(e) => {
                self.say(&format!("achievements: {source_id}: {e:#}"));
                return Ok(Vec::new());
            }
        };
        let newly = db.put_achievements(game, &read)?;
        let of_this_session = |a: &Achievement| a.unlocked.is_some_and(|at| at == 0 || at + SESSION_SLACK >= since);
        let mut told = Vec::new();
        for a in newly.into_iter().filter(of_this_session) {
            let a = self.icon_now(db, game, a);
            let rarity = a.rarity.map_or("rarity not known".to_string(), |r| format!("{r}% of players"));
            let rare = if a.is_rare() { ", Rare" } else { "" };
            self.say(&format!(
                "achievements: {source_id} unlocked {} {:?}: score {}, {rarity}{rare}",
                a.key,
                a.name,
                a.score()
            ));
            told.push(Unlock { game, achievement: a });
        }
        self.say_game(db, game, &source_id)?;
        Ok(told)
    }

    /// The achievement with its icon, fetched now if it is not in the cache and icons may be fetched.
    fn icon_now(&mut self, db: &Database, game: i64, mut a: Achievement) -> Achievement {
        if let (Some(url), None, true) = (&a.icon, &a.icon_file, self.icons_online) {
            if let Some(path) = self.fetch_icon(db, game, url) {
                a.icon_file = Some(path);
            }
        }
        a
    }

    /// Fetches one icon into the cache; None when it could not be. One the server does not have, or that is no
    /// picture, is said once and not asked for again this run; the others go on. Offline, or the server in
    /// trouble (a 429 or 5xx), no more icons are asked for this run.
    fn fetch_icon(&mut self, db: &Database, game: i64, url: &str) -> Option<PathBuf> {
        let dir = db.achievement_icons()?;
        let found = db.game(game).ok()??;
        if let Some(path) = dir.find(&found.source, &found.source_id, url) {
            return Some(path);
        }
        if self.icons_stopped || self.missing.contains(url) {
            return None;
        }
        let file = url.split(['?', '#']).next().unwrap_or(url).rsplit('/').next().unwrap_or(url).to_string();
        match self.fetch.as_ref()?.icon(url) {
            Ok(Some(bytes)) if extension(&bytes).is_some() => {
                match dir.save(&found.source, &found.source_id, url, &bytes) {
                    Ok(path) => Some(path),
                    Err(e) => {
                        self.say(&format!("achievements: an icon of {} not kept: {e:#}", found.source_id));
                        None
                    }
                }
            }
            Ok(Some(_)) => {
                self.missing.insert(url.to_string());
                self.say(&format!("achievements: an icon of {} is not a picture: {file}", found.source_id));
                None
            }
            Ok(None) => {
                self.missing.insert(url.to_string());
                self.say(&format!("achievements: an icon of {} not found: {file}", found.source_id));
                None
            }
            Err(e) => {
                self.say(&format!(
                    "achievements: icons not fetched: {} (asking for {}'s {file})",
                    why(&e),
                    found.source_id
                ));
                self.icons_stopped = true;
                self.icons.clear();
                None
            }
        }
    }

    /// One piece of work online, at `now`: the next game's rarity, else the next icon.
    pub fn work(&mut self, db: &mut Database, now: u64) -> Result<Worked> {
        let mut worked = Worked::default();
        if self.fetch.is_none() {
            return Ok(worked);
        }
        if let Some(game) = self.rarity.pop_front() {
            let Some(found) = db.game(game)? else { return Ok(worked) };
            let fetch = self.fetch.as_ref().expect("checked above");
            match fetch.rarity(&found.source, &found.source_id) {
                Ok(rarity) => {
                    let before = db.achievements(game)?;
                    db.set_rarity(game, rarity.as_ref(), now)?;
                    let after = db.achievements(game)?;
                    let known = after.iter().filter(|a| a.rarity.is_some()).count();
                    self.say(&format!("achievements: rarity of {}: {known} of {} known", found.source_id, after.len()));
                    if before != after {
                        worked.changed = true;
                        self.say_game(db, game, &found.source_id)?;
                    }
                }
                Err(e) => {
                    // Offline, or the service in trouble: what was kept stands, and nothing more is asked this time.
                    self.say(&format!(
                        "achievements: rarity not fetched ({}): kept as it was for {} games",
                        why(&e),
                        self.rarity.len() + 1
                    ));
                    self.rarity.clear();
                }
            }
        } else if let Some((game, url)) = self.icons.pop_front() {
            if self.fetch_icon(db, game, &url).is_some() {
                worked.icons.push(game);
            }
        }
        Ok(worked)
    }
}

/// Whether the store is to be asked how rare the game's achievements are: never asked, or not for [`RARITY_FOR`].
fn rarity_due(db: &Database, game: i64, now: u64) -> Result<bool> {
    Ok(db.rarity_asked(game)?.is_none_or(|asked| now.saturating_sub(asked) >= RARITY_FOR))
}

#[cfg(test)]
mod tests;
