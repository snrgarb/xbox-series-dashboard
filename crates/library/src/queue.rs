//! The Queue (`model::Queue`): every game being installed, kept in the library database across restarts in the
//! order each joined, and squared once a second with what the sources say they have under way
//! (`model::Source::installs`) while it holds anything.
//!
//! - A game joins as the Dashboard asks its store for it (`Follower::asked_install`), Queued, if its source keeps
//!   word of its installs; or as its source first reports it, which is how an install started in the store's own
//!   client joins.
//! - While its source reports it, it is where the source says: Queued, downloading so many bytes of so many, or
//!   paused.
//! - Once the game itself is installed while its store is still fetching more for it (Steam's Workshop items,
//!   `InstallState::Extras`, #188), its source is read again so that it reads installed, Play is offered, and it is
//!   told of as finished (`Change::Playable`); its row stays until the store is done, then leaves with nothing more
//!   said (`Change::ExtrasDone`).
//! - Once its source no longer reports it, the source is read again (`Source::read`, merged in as every read is):
//!   a game that now reads installed has finished and leaves for "recently installed", with when; one that does not
//!   was dropped (cancelled in the client, or at its install dialog) and simply leaves.
//! - One asked for that its source never reports leaves after a minute ([`GIVE_UP_AFTER`]), finished if a read then
//!   finds it installed (a small game can install between two looks), else dropped. Meanwhile its source is read
//!   every [`READ_EVERY`] seconds, so a game that finished that quickly does not wait the minute.
//!
//! An uninstall the Dashboard asked for is followed the same way, by reading its source again every few seconds
//! until the game reads uninstalled: for two minutes at most ([`UNINSTALL_FOR`]) where asking is all it takes
//! (`Source::uninstalls_by_itself`, Steam's), after which the log says it is still there; for ten
//! ([`USER_UNINSTALL_FOR`]) where the uninstall is the user's to finish in a window of its own (Settings' installed
//! apps, a game's uninstaller), after which it is no longer followed, and nothing is said.
//!
//! What the screens tell the user of in a Toast ([`Tell`]) goes with each update: a game joining the Queue (but for
//! those already in it as the Dashboard starts), one finishing installing, one seen uninstalled, and one whose
//! install stopped (#230). A game never just leaves the Queue: one that leaves without installing keeps a row in
//! Manage Queue saying why (`model::Stopped`, held here and not in the database), told by its store's installer
//! (`Ask::Stopped`: not enough space, a licence agreement declined, what the store asked for) or worked out here
//! (cancelled here, stopped in the store's own client, never started, no longer listed). The row goes once the game
//! is asked for again or installed.
//!
//! While a game downloads, its speed is worked out from its bytes at successive looks ([`Follower::queue`]): the
//! bytes gained over the time since they last moved, smoothed, and 0 once they have stood still for
//! [`STALLED_AFTER`] seconds, as Steam writes its progress only every few seconds. None until they first move.
//!
//! What is done in the store's own client while the Dashboard runs shows too (#171): the thread looks every
//! [`WATCH_EVERY`] seconds when there is nothing else to follow, so an install started there joins the Queue as its
//! source reports it; and at that pace each source's own cheap word of what is installed (`Source::installed_now`,
//! Steam's app manifests) is held to what it said last, the source read again when it changes, so a game installed
//! or uninstalled there reads so without a restart.
//!
//! [`spawn`] runs all this on a thread of its own, which polls once a second while there is something to follow,
//! every [`WATCH_EVERY`] seconds otherwise, and wakes whoever started it when the Queue or the library has changed.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use model::{InstallState, Installed, Library, Queue, Queued, Source, Stop, Stopped};
use rusqlite::{params, OptionalExtension};

use crate::Database;

pub const MIGRATION: &str = "CREATE TABLE queue (
        game INTEGER PRIMARY KEY REFERENCES games (id) ON DELETE CASCADE,
        -- Unix seconds: when it joined, which orders what is next up.
        started INTEGER NOT NULL,
        -- Unix seconds: when the Dashboard asked its store for it. None: first seen in the store's own client.
        asked INTEGER,
        -- Its source has reported it: once it no longer does, the install has ended.
        reported INTEGER NOT NULL DEFAULT 0,
        -- queued, downloading, paused or extras; with the bytes done and to do, as the source last said.
        state TEXT NOT NULL DEFAULT 'queued',
        done INTEGER NOT NULL DEFAULT 0,
        total INTEGER NOT NULL DEFAULT 0
    );
    -- The games the Queue held that finished installing: Manage Queue's Recently installed.
    CREATE TABLE installed (
        game INTEGER PRIMARY KEY REFERENCES games (id) ON DELETE CASCADE,
        -- Unix seconds.
        finished INTEGER NOT NULL
    );";

/// How long an install asked for may go unreported by its source before it leaves the Queue, in seconds.
pub const GIVE_UP_AFTER: u64 = 60;
/// How often a source is read again while it has an install asked for and not reported, or an uninstall followed.
pub const READ_EVERY: u64 = 10;
/// How soon after an uninstall is asked for its source is first read again.
pub const UNINSTALL_FIRST_READ: u64 = 2;
/// How long an uninstall is followed where asking is all it takes, in seconds.
pub const UNINSTALL_FOR: u64 = 120;
/// How long an uninstall is followed where it is the user's to finish in a window of its own, in seconds: as long
/// as the Bundle follows one before giving up.
pub const USER_UNINSTALL_FOR: u64 = 600;
/// How often, in seconds, each source's own word of what is installed (`Source::installed_now`) is looked at, and
/// how long the Queue's thread waits between looks when there is nothing else to follow: a game installed,
/// uninstalled or started downloading in the store's own client shows within this.
pub const WATCH_EVERY: u64 = 5;
/// How many finished installs a snapshot carries.
pub const RECENTLY_INSTALLED: usize = 8;
/// How often the Queue is looked at while there is something to follow.
pub const POLL: Duration = Duration::from_secs(1);
/// How long a download's bytes may stand still, in seconds, before its speed reads 0. Steam writes its progress
/// every few seconds, so a look that finds them where they were keeps the last speed until then.
pub const STALLED_AFTER: f64 = 10.0;
/// How much of the speed between two looks a download's speed takes on: the rest is the speed it had, so that one
/// burst of Steam's writing does not make it jump.
pub const SMOOTHING: f64 = 0.5;

/// What happened to the Queue, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Joined: asked for here, or first seen in the client.
    Joined { game: i64, asked: bool },
    /// Moved on: another kind of state, or another whole percent.
    State { game: i64, state: InstallState },
    /// Finished installing.
    Installed(i64),
    /// The game itself is installed and playable, and its store is still fetching extras for it: told of as
    /// finished, its row kept.
    Playable(i64),
    /// The extras of a game that was playable already are in too: it leaves the Queue.
    ExtrasDone(i64),
    /// Left without installing.
    Dropped(i64),
    /// An uninstall asked for here has been seen done.
    Uninstalled(i64),
    /// An uninstall asked for here was not seen done in time.
    StillInstalled(i64),
}

/// A row of the queue table, with what the games table says of its game.
#[derive(Debug, Clone)]
struct Row {
    game: i64,
    source: String,
    gone: bool,
    /// Its launcher is switched off: the row is kept as it is, neither shown nor followed, until it is on again.
    off: bool,
    started: u64,
    asked: Option<u64>,
    reported: bool,
    state: InstallState,
}

/// The state a row's columns say: its kind, its bytes, and an extras' items.
fn state_of(kind: &str, done: i64, total: i64, items_done: i64, items: i64) -> InstallState {
    let (done, total) = (done.max(0) as u64, total.max(0) as u64);
    let count = |n: i64| n.clamp(0, u32::MAX as i64) as u32;
    match kind {
        "downloading" => InstallState::Downloading { done, total },
        "paused" => InstallState::Paused { done, total },
        "extras" => InstallState::Extras { items_done: count(items_done), items: count(items), done, total },
        _ => InstallState::Queued,
    }
}

/// The columns a state is kept in: its kind, its bytes, and an extras' items (0 for the rest).
fn columns(state: InstallState) -> (&'static str, i64, i64, i64, i64) {
    match state {
        InstallState::Queued => ("queued", 0, 0, 0, 0),
        InstallState::Downloading { done, total } => ("downloading", done as i64, total as i64, 0, 0),
        InstallState::Paused { done, total } => ("paused", done as i64, total as i64, 0, 0),
        InstallState::Extras { items_done, items, done, total } => {
            ("extras", done as i64, total as i64, items_done as i64, items as i64)
        }
    }
}

/// Migration 8: an extras' items, done and in all (#188).
pub const ITEMS_MIGRATION: &str = "-- Of a game installed whose extras are coming in: its items in, and in all.
    ALTER TABLE queue ADD COLUMN items_done INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE queue ADD COLUMN items INTEGER NOT NULL DEFAULT 0;";

/// Whether a change of state is worth a line in the log: another kind, or another whole percent.
fn moved(was: InstallState, now: InstallState) -> bool {
    std::mem::discriminant(&was) != std::mem::discriminant(&now) || was.percent() != now.percent()
}

impl Database {
    /// The Queue as the database has it: its games, the current download first, and the latest it saw finish.
    /// A game gone from its source, or of a launcher switched off, is in neither.
    pub fn queue(&self) -> Result<Queue> {
        let entries = self
            .rows()?
            .into_iter()
            .filter(|row| !row.gone && !row.off)
            .map(|row| Queued::new(row.game, row.state, row.started))
            .collect();
        let mut installed = self.conn.prepare(&format!(
            "SELECT game, finished FROM installed JOIN games ON games.id = installed.game
             WHERE {} ORDER BY finished DESC, game DESC LIMIT ?1",
            crate::launchers::shown("games")
        ))?;
        let installed = installed
            .query_map([RECENTLY_INSTALLED as i64], |row| {
                Ok(Installed { game: row.get(0)?, finished: row.get::<_, i64>(1)?.max(0) as u64 })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Queue::new(entries, installed))
    }

    fn rows(&self) -> Result<Vec<Row>> {
        let mut rows = self.conn.prepare(&format!(
            "SELECT queue.game, source, gone, started, asked, reported, state, done, total, source IN {},
                    items_done, items
             FROM queue JOIN games ON games.id = queue.game",
            crate::launchers::OFF
        ))?;
        let rows = rows.query_map([], |row| {
            Ok(Row {
                game: row.get(0)?,
                source: row.get(1)?,
                gone: row.get(2)?,
                off: row.get(9)?,
                started: row.get::<_, i64>(3)?.max(0) as u64,
                asked: row.get::<_, Option<i64>>(4)?.map(|asked| asked.max(0) as u64),
                reported: row.get(5)?,
                state: state_of(&row.get::<_, String>(6)?, row.get(7)?, row.get(8)?, row.get(10)?, row.get(11)?),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The game of this store with this id, if the database has it and its source still lists it.
    fn game_of(&self, source: &str, source_id: &str) -> Result<Option<i64>> {
        let id = self
            .conn
            .query_row(
                "SELECT id FROM games WHERE source = ?1 AND source_id = ?2 AND gone = 0",
                [source, source_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id)
    }

    /// The `source_id`s of the source's games that read installed, sorted: what `Source::installed_now` is held to.
    fn installed_ids(&self, source: &str) -> Result<Vec<String>> {
        let mut ids =
            self.conn.prepare("SELECT source_id FROM games WHERE source = ?1 AND installed = 1 AND gone = 0")?;
        let mut ids: Vec<String> = ids.query_map([source], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
        ids.sort();
        Ok(ids)
    }

    /// Whether the game reads installed.
    fn installed(&self, game: i64) -> Result<bool> {
        let installed =
            self.conn.query_row("SELECT installed FROM games WHERE id = ?1", [game], |row| row.get(0)).optional()?;
        Ok(installed.unwrap_or(false))
    }

    fn join(&self, game: i64, now: u64, asked: bool, state: InstallState) -> Result<()> {
        let (kind, done, total, items_done, items) = columns(state);
        self.conn.execute(
            "INSERT INTO queue (game, started, asked, reported, state, done, total, items_done, items)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![game, now as i64, asked.then_some(now as i64), !asked, kind, done, total, items_done, items],
        )?;
        Ok(())
    }

    fn set_state(&self, game: i64, state: InstallState) -> Result<()> {
        let (kind, done, total, items_done, items) = columns(state);
        self.conn.execute(
            "UPDATE queue SET reported = 1, state = ?2, done = ?3, total = ?4, items_done = ?5, items = ?6
             WHERE game = ?1",
            params![game, kind, done, total, items_done, items],
        )?;
        Ok(())
    }

    /// The game leaves the Queue: for Recently installed, at `now`, if it finished.
    fn leave(&self, game: i64, finished: Option<u64>) -> Result<()> {
        self.conn.execute("DELETE FROM queue WHERE game = ?1", [game])?;
        if let Some(at) = finished {
            self.finished(game, at)?;
        }
        Ok(())
    }

    /// The game finished installing at `at`: it heads Recently installed.
    fn finished(&self, game: i64, at: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO installed (game, finished) VALUES (?1, ?2) ON CONFLICT (game) DO UPDATE SET finished = ?2",
            params![game, at as i64],
        )?;
        Ok(())
    }
}

/// What a look at the Queue found.
#[derive(Debug, Default)]
pub struct Polled {
    pub changes: Vec<Change>,
    /// A source was read again and changed the library: the screens want a new snapshot, not only the Queue.
    pub library_changed: bool,
    /// The Queue changed in what the log does not tell of: a download's bytes, or its speed.
    pub moved: bool,
}

/// A downloading game's bytes when they last moved, when that was (seconds on the follower's own clock), and the
/// speed it has been given, in bits a second.
#[derive(Debug, Clone, Copy)]
struct Sample {
    bytes: u64,
    at: f64,
    speed: Option<f64>,
}

impl Sample {
    /// The sample after a look at `at` finds `bytes` done.
    fn next(self, bytes: u64, at: f64) -> Sample {
        let elapsed = at - self.at;
        if bytes == self.bytes {
            // Standing still: the last speed, until it has stood still too long. Then 0, and a speed it picks up
            // again is counted from there, not over the whole time it stood.
            return match self.speed {
                Some(speed) if speed > 0.0 && elapsed > STALLED_AFTER => Sample { at, speed: Some(0.0), ..self },
                _ => self,
            };
        }
        // Fewer than before is a download started again: counted from here.
        if bytes < self.bytes || elapsed <= 0.0 {
            return Sample { bytes, at, speed: self.speed.filter(|_| bytes > self.bytes) };
        }
        let now = (bytes - self.bytes) as f64 * 8.0 / elapsed;
        let speed = match self.speed {
            Some(was) if was > 0.0 => was + SMOOTHING * (now - was),
            _ => now,
        };
        Sample { bytes, at, speed: Some(speed) }
    }
}

/// Follows the Queue and the uninstalls asked for: what is kept between looks and is not the database's.
pub struct Follower {
    sources: Vec<Arc<dyn Source>>,
    /// The uninstalls asked for, by game, with when.
    uninstalls: HashMap<i64, u64>,
    /// When each source was last read again, by its key.
    read_at: HashMap<&'static str, u64>,
    /// Each downloading game's bytes and speed, by game.
    samples: HashMap<i64, Sample>,
    /// Each downloading game's speed as its store says it, by game: shown in place of the sample's.
    told: HashMap<i64, u64>,
    /// What each source's client said was installed at its last look (`Source::installed_now`), by its key.
    installed: HashMap<&'static str, Vec<String>>,
    /// When the sources' installed games were last looked at.
    watched_at: Option<u64>,
    /// The games whose install the user cancelled here, until they leave the Queue.
    cancels: HashSet<i64>,
    /// The games whose install stopped and why, the latest first: their rows in Manage Queue.
    stopped: Vec<Stopped>,
}

impl Follower {
    pub fn new(sources: Vec<Arc<dyn Source>>) -> Self {
        Self {
            sources,
            uninstalls: HashMap::new(),
            read_at: HashMap::new(),
            samples: HashMap::new(),
            told: HashMap::new(),
            installed: HashMap::new(),
            watched_at: None,
            cancels: HashSet::new(),
            stopped: Vec::new(),
        }
    }

    /// The game's install stopped: its row says why, in the place of any it had, first. Said in the log.
    fn stop(&mut self, db: &Database, game: i64, stop: Stop) -> Result<Stopped> {
        let found = db.game(game)?;
        let key = found.as_ref().map(|game| game.source.clone()).unwrap_or_default();
        let store = self.source(&key).map_or_else(|| key.clone(), |source| source.name().to_string());
        let stopped = Stopped { game, store, stop };
        let name = found.map_or_else(|| format!("game {game}"), |game| game.name);
        eprintln!("queue: {name} stopped: {}: {}", stopped.headline(), stopped.line());
        self.stopped.retain(|was| was.game != game);
        self.stopped.insert(0, stopped.clone());
        Ok(stopped)
    }

    /// Why a game left without installing, where its store's installer has not said: cancelled here, no longer
    /// listed, never started, or stopped in the store's own client.
    fn reason(&mut self, row: &Row) -> Stop {
        if self.cancels.remove(&row.game) {
            Stop::Cancelled
        } else if row.gone {
            Stop::Gone
        } else if !row.reported {
            Stop::NeverStarted
        } else {
            Stop::InStore
        }
    }

    /// The user cancelled the game's install here: told as such when it leaves.
    pub fn asked_cancel(&mut self, game: i64) {
        self.cancels.insert(game);
    }

    /// The game's store's installer said its install stopped, and why (`model::Opener::stopped`): its row leaves the
    /// Queue now, if it has one, and its stopped row says why. What to tell of it.
    pub fn stopped_by_store(&mut self, db: &mut Database, game: i64, stop: Stop) -> Result<(Vec<Change>, Stopped)> {
        let mut changes = Vec::new();
        if db.rows()?.iter().any(|row| row.game == game) {
            db.leave(game, None)?;
            changes.push(Change::Dropped(game));
        }
        self.cancels.remove(&game);
        let stopped = self.stop(db, game, stop)?;
        Ok((changes, stopped))
    }

    /// The stopped row of a game, as last said.
    pub fn stopped(&self, game: i64) -> Option<&Stopped> {
        self.stopped.iter().find(|stopped| stopped.game == game)
    }

    /// The sources whose client's word of what is installed has changed since the last look, every
    /// [`WATCH_EVERY`] seconds: to be read again, so that a game installed or uninstalled in the store's own client
    /// shows as such. At the first look, a source whose word differs from the database's.
    fn watch_installed(
        &mut self,
        db: &Database,
        off: &std::collections::HashSet<String>,
        now: u64,
    ) -> Result<Vec<String>> {
        if self.watched_at.is_some_and(|at| now.saturating_sub(at) < WATCH_EVERY) {
            return Ok(Vec::new());
        }
        self.watched_at = Some(now);
        let mut changed = Vec::new();
        for source in self.sources.iter().filter(|source| !off.contains(source.key())) {
            let Some(now_installed) = source.installed_now() else { continue };
            let before = match self.installed.get(source.key()) {
                Some(before) => before.clone(),
                None => db.installed_ids(source.key())?,
            };
            if before != now_installed {
                eprintln!(
                    "queue: {}'s installed games changed in its own client ({} then, {} now): reading it again",
                    source.key(),
                    before.len(),
                    now_installed.len()
                );
                changed.push(source.key().to_string());
            }
            self.installed.insert(source.key(), now_installed);
        }
        Ok(changed)
    }

    /// The Queue as the database has it, each download with the speed it has been given and what its store's
    /// installer can do with it (`Source::installer`).
    pub fn queue(&self, db: &Database) -> Result<Queue> {
        let mut queue = db.queue()?;
        // A stopped game that is in the Queue again, or installed after all, has no stopped row.
        let mut stopped = Vec::new();
        for was in &self.stopped {
            if queue.get(was.game).is_none() && !db.installed(was.game)? {
                stopped.push(was.clone());
            }
        }
        queue.stopped = stopped;
        let sources: HashMap<i64, String> = db.rows()?.into_iter().map(|row| (row.game, row.source)).collect();
        for entry in &mut queue.entries {
            entry.speed = self.speed(entry.game);
            let installer = sources.get(&entry.game).and_then(|key| self.source(key)).and_then(|s| s.installer());
            entry.can = installer.map_or(model::Can::NOTHING, |installer| installer.can());
        }
        Ok(queue)
    }

    /// The game's download speed in bits a second, to the whole bit: its store's, where it says, else the one
    /// worked out from its bytes.
    fn speed(&self, game: i64) -> Option<u64> {
        let worked_out = || self.samples.get(&game).and_then(|sample| sample.speed).map(|speed| speed.round() as u64);
        self.told.get(&game).copied().or_else(worked_out)
    }

    /// Each download's bytes as they are now, at `at`: its speed worked out again. Whether any speed changed.
    fn sample(&mut self, db: &Database, at: f64) -> Result<bool> {
        let rows = db.rows()?;
        let downloading: HashMap<i64, u64> = rows
            .iter()
            .filter_map(|row| match row.state {
                InstallState::Downloading { done, .. } if !row.gone => Some((row.game, done)),
                _ => None,
            })
            .collect();
        let speeds = |follower: &Self| -> HashMap<i64, u64> {
            follower.samples.keys().filter_map(|&game| Some((game, follower.speed(game)?))).collect()
        };
        let before = speeds(self);
        self.samples.retain(|game, _| downloading.contains_key(game));
        for (&game, &bytes) in &downloading {
            let next = match self.samples.get(&game) {
                Some(&sample) => sample.next(bytes, at),
                None => Sample { bytes, at, speed: None },
            };
            self.samples.insert(game, next);
        }
        Ok(speeds(self) != before)
    }

    fn source(&self, key: &str) -> Option<&Arc<dyn Source>> {
        self.sources.iter().find(|source| source.key() == key)
    }

    /// The Dashboard has just asked the game's store to install it: it joins the Queue, Queued, unless it is
    /// installed, in the Queue already, or of a store that keeps no word of its installs (whose own window shows
    /// how it goes). Whether it joined.
    pub fn asked_install(&mut self, db: &mut Database, game: i64, now: u64) -> Result<bool> {
        let found: Option<(String, bool, bool)> = db
            .conn
            .query_row("SELECT source, installed, gone FROM games WHERE id = ?1", [game], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?;
        let Some((source, installed, gone)) = found else { return Ok(false) };
        let follows = self.source(&source).is_some_and(|source| source.installs().is_some());
        let queued = db.rows()?.iter().any(|row| row.game == game);
        if installed || gone || queued || !follows {
            return Ok(false);
        }
        // Asked for again: its stopped row goes.
        self.stopped.retain(|was| was.game != game);
        self.cancels.remove(&game);
        db.join(game, now, true, InstallState::Queued)?;
        Ok(true)
    }

    /// The Dashboard has just asked the game's store to uninstall it: its source is read again until it reads
    /// uninstalled.
    pub fn asked_uninstall(&mut self, game: i64, now: u64) {
        self.uninstalls.insert(game, now);
    }

    /// Whether there is anything to follow: the Queue holds a game, or an uninstall is under way.
    pub fn busy(&self, db: &Database) -> Result<bool> {
        Ok(!self.uninstalls.is_empty() || db.rows()?.iter().any(|row| !row.off))
    }

    /// Reads the source again and merges it in, as at start; whether that changed anything. One that can't be read
    /// is said in the log, as it is at start, and changes nothing.
    fn read_again(&mut self, db: &mut Database, key: &str, now: u64) -> Result<bool> {
        let Some(source) = self.source(key).cloned() else { return Ok(false) };
        // A launcher switched off is not read: its games are as it last left them.
        if db.launchers_off()?.contains(key) {
            return Ok(false);
        }
        self.read_at.insert(source.key(), now);
        let read = crate::sources::read(std::slice::from_ref(&source));
        let synced = db.sync_all(&read, now)?;
        Ok(synced != crate::Synced::default())
    }

    /// Whether the source was read longer ago than `every`, or never.
    fn due(&self, key: &str, now: u64, every: u64) -> bool {
        self.read_at.get(key).is_none_or(|&at| now.saturating_sub(at) >= every)
    }

    /// Squares the Queue with what the sources say they have under way, at `now`, a speed's clock reading the same.
    pub fn poll(&mut self, db: &mut Database, now: u64) -> Result<Polled> {
        self.poll_at(db, now, now as f64)
    }

    /// [`poll`](Self::poll), with a finer clock for the speeds: `at`, in seconds, from wherever the caller's
    /// clock starts.
    pub fn poll_at(&mut self, db: &mut Database, now: u64, at: f64) -> Result<Polled> {
        let mut polled = self.square(db, now)?;
        polled.moved |= self.sample(db, at)?;
        Ok(polled)
    }

    fn square(&mut self, db: &mut Database, now: u64) -> Result<Polled> {
        let mut polled = Polled::default();
        let mut reports: HashMap<&'static str, Vec<model::Install>> = HashMap::new();
        // A launcher switched off is not asked, and its games' rows are left as they are until it is on again.
        let off = db.launchers_off()?;
        for source in self.sources.iter().filter(|source| !off.contains(source.key())) {
            if let Some(installs) = source.installs() {
                reports.insert(source.key(), installs);
            }
        }
        let mut rows = db.rows()?;
        rows.retain(|row| !row.off);
        let mut seen: Vec<i64> = Vec::new();
        // Games that have just become playable while their extras come in: their source is read again, so that
        // they read installed.
        let mut playable: Vec<(i64, String)> = Vec::new();
        let told_before = std::mem::take(&mut self.told);
        for (&key, installs) in &reports {
            for install in installs {
                let Some(game) = db.game_of(key, &install.source_id)? else { continue };
                seen.push(game);
                if let Some(speed) = install.speed.filter(|_| install.state.downloading()) {
                    self.told.insert(game, speed);
                }
                let was = rows.iter().find(|row| row.game == game);
                if install.state.extras() && !was.is_some_and(|row| row.state.extras()) {
                    playable.push((game, key.to_string()));
                }
                match was {
                    Some(row) if row.reported && row.state == install.state => {}
                    Some(row) => {
                        db.set_state(game, install.state)?;
                        polled.moved = true;
                        // Its first report is told of even as it stays Queued: from here, the store has taken
                        // it on, and the moment it reports it no more it has ended.
                        if !row.reported || moved(row.state, install.state) {
                            polled.changes.push(Change::State { game, state: install.state });
                        }
                    }
                    None => {
                        db.join(game, now, false, install.state)?;
                        polled.changes.push(Change::Joined { game, asked: false });
                        if install.state != InstallState::Queued {
                            polled.changes.push(Change::State { game, state: install.state });
                        }
                    }
                }
            }
        }
        // A store's own speed that changed is the Queue moving, as bytes are.
        polled.moved |= self.told != told_before;

        // What its source no longer has, or never had: ended, or to be looked at again.
        let mut ended: Vec<&Row> = Vec::new();
        let mut waiting: Vec<&Row> = Vec::new();
        for row in rows.iter().filter(|row| !seen.contains(&row.game)) {
            if row.gone || !reports.contains_key(row.source.as_str()) {
                db.leave(row.game, None)?;
                polled.changes.push(Change::Dropped(row.game));
                let stop = self.reason(row);
                self.stop(db, row.game, stop)?;
            } else if row.reported || row.asked.is_none_or(|asked| now.saturating_sub(asked) >= GIVE_UP_AFTER) {
                ended.push(row);
            } else {
                waiting.push(row);
            }
        }
        let mut to_read: Vec<String> = ended.iter().map(|row| row.source.clone()).collect();
        to_read.extend(playable.iter().map(|(_, key)| key.clone()));
        to_read.extend(waiting.iter().filter(|row| self.due(&row.source, now, READ_EVERY)).map(|r| r.source.clone()));
        to_read.extend(self.watch_installed(db, &off, now)?);
        let uninstalls: Vec<(i64, u64)> = self.uninstalls.iter().map(|(&game, &asked)| (game, asked)).collect();
        for &(game, asked) in &uninstalls {
            let Some(source) = db.game(game)?.map(|game| game.source) else {
                self.uninstalls.remove(&game);
                continue;
            };
            let since = now.saturating_sub(asked);
            let first = since >= UNINSTALL_FIRST_READ && self.read_at.get(source.as_str()).is_none_or(|&at| at < asked);
            if first || (since >= UNINSTALL_FIRST_READ && self.due(&source, now, READ_EVERY)) {
                to_read.push(source);
            }
        }
        to_read.sort();
        to_read.dedup();
        for key in &to_read {
            polled.library_changed |= self.read_again(db, key, now)?;
        }

        // A source read again has said whether each finished. One that was playable already, its extras coming
        // in, finished then.
        for row in &ended {
            let installed = db.installed(row.game)?;
            let extras = row.state.extras();
            db.leave(row.game, installed.then_some(now).filter(|_| !extras))?;
            polled.changes.push(match (installed, extras) {
                (true, false) => Change::Installed(row.game),
                (true, true) => Change::ExtrasDone(row.game),
                (false, _) => Change::Dropped(row.game),
            });
            if installed {
                self.cancels.remove(&row.game);
            } else {
                let stop = self.reason(row);
                self.stop(db, row.game, stop)?;
            }
        }
        for &(game, _) in &playable {
            db.finished(game, now)?;
            polled.changes.push(Change::Playable(game));
        }
        for row in &waiting {
            if to_read.contains(&row.source) && db.installed(row.game)? {
                db.leave(row.game, Some(now))?;
                polled.changes.push(Change::Installed(row.game));
            }
        }
        for (game, asked) in uninstalls {
            if !self.uninstalls.contains_key(&game) {
                continue;
            }
            if !db.installed(game)? {
                self.uninstalls.remove(&game);
                polled.changes.push(Change::Uninstalled(game));
                continue;
            }
            // Where the uninstall is the user's to finish, a game still there is their choice: nothing is said.
            let by_itself = match db.game(game)? {
                Some(found) => self.source(&found.source).is_some_and(|source| source.uninstalls_by_itself()),
                None => false,
            };
            let follow_for = if by_itself { UNINSTALL_FOR } else { USER_UNINSTALL_FOR };
            if now.saturating_sub(asked) >= follow_for {
                self.uninstalls.remove(&game);
                if by_itself {
                    polled.changes.push(Change::StillInstalled(game));
                }
            }
        }
        Ok(polled)
    }
}

/// What the log says of a change: `queue: Quiet Orbit joined, asked for`, `queue: Quiet Orbit downloading, 25% of
/// 1000 bytes`, `queue: Quiet Orbit installed`. `name` finds a game's name.
pub fn describe(change: &Change, name: &dyn Fn(i64) -> String) -> String {
    let says = match change {
        Change::Joined { game, asked: true } => format!("{} joined, asked for", name(*game)),
        Change::Joined { game, asked: false } => format!("{} joined, from its store's client", name(*game)),
        Change::State { game, state } => {
            let how = match (state, state.bytes(), state.percent()) {
                (InstallState::Queued, ..) => "queued".to_string(),
                (&InstallState::Extras { items_done, items, done, total }, ..) => {
                    let bytes = (total > 0).then(|| format!(", {} of {total} bytes", done.min(total)));
                    format!("{}, {items_done} of {items} items{}", kind(*state), bytes.unwrap_or_default())
                }
                (state, Some((_, total)), Some(percent)) => format!("{}, {percent}% of {total} bytes", kind(*state)),
                (state, ..) => kind(*state).to_string(),
            };
            format!("{} {how}", name(*game))
        }
        Change::Installed(game) => format!("{} installed", name(*game)),
        Change::Playable(game) => format!("{} installed; its store is still fetching more for it", name(*game)),
        Change::ExtrasDone(game) => format!("{} has all its store was fetching for it", name(*game)),
        Change::Dropped(game) => format!("{} dropped", name(*game)),
        Change::Uninstalled(game) => format!("{} uninstalled", name(*game)),
        Change::StillInstalled(game) => {
            format!("{} still installed {UNINSTALL_FOR} s after it was asked to uninstall", name(*game))
        }
    };
    format!("queue: {says}")
}

/// How the log names a state other than Queued.
fn kind(state: InstallState) -> &'static str {
    match state {
        InstallState::Paused { .. } => "paused",
        InstallState::Extras { .. } => "downloading Workshop content",
        _ => "downloading",
    }
}

/// What the thread was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// The game's store was asked to install it.
    Install(i64),
    /// The game's store was asked to uninstall it.
    Uninstall(i64),
    /// The game's store was asked to cancel its install: told as cancelled when it leaves.
    Cancel(i64),
    /// The game's store's installer stopped its install before it started, and says why (#230).
    Stopped { game: i64, stop: Stop },
    /// Look now: the sources have been read, and may have news.
    Look,
}

/// What the screens tell the user of, in a Toast, as the Bundle does (its glossary's Toast).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tell {
    /// The game joined the Queue: "Queued". Not for those already in it as the Dashboard starts.
    Queued(i64),
    /// The game the Queue held has finished installing: "Ready to play".
    Ready(i64),
    /// An uninstall the Dashboard asked for is done: "<game> was uninstalled".
    Uninstalled(i64),
    /// The game's install stopped, or it left the Queue without installing: why (#230).
    Stopped(Stopped),
}

impl Tell {
    /// What a change is told as, if anything. `first`: the change was found by the look as the thread started,
    /// whose games had joined before the Dashboard was there to see. A game dropped is told of with its stopped
    /// row ([`Follower::stopped`]), which the change does not carry ([`tells`]).
    pub fn of(change: &Change, first: bool) -> Option<Tell> {
        match *change {
            Change::Joined { game, .. } if !first => Some(Tell::Queued(game)),
            Change::Installed(game) | Change::Playable(game) => Some(Tell::Ready(game)),
            Change::Uninstalled(game) => Some(Tell::Uninstalled(game)),
            _ => None,
        }
    }
}

/// What the changes are told as: [`Tell::of`] each, and a game dropped with its stopped row.
pub fn tells(changes: &[Change], first: bool, follower: &Follower) -> Vec<Tell> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Dropped(game) => follower.stopped(*game).cloned().map(Tell::Stopped),
            change => Tell::of(change, first),
        })
        .collect()
}

/// What the thread has for the screens: the Queue, with it the library's snapshot when a source read again
/// changed it, and what to tell the user of.
#[derive(Debug, Clone)]
pub struct Update {
    pub queue: Queue,
    pub library: Option<Library>,
    pub tell: Vec<Tell>,
}

/// The thread that follows the Queue, as its starter holds it.
pub struct Handle {
    asks: Sender<Ask>,
    updates: Receiver<Update>,
}

impl Handle {
    pub fn ask(&self, ask: Ask) {
        let _ = self.asks.send(ask);
    }

    /// The latest the thread has to say, if it has said anything since this was last asked.
    pub fn take(&self) -> Option<Update> {
        let mut latest: Option<Update> = None;
        while let Ok(update) = self.updates.try_recv() {
            // A library snapshot is kept, should a later update carry only the Queue, and every Toast.
            let (library, mut tell) = match latest {
                Some(latest) => (update.library.or(latest.library), latest.tell),
                None => (update.library, Vec::new()),
            };
            tell.extend(update.tell);
            latest = Some(Update { queue: update.queue, library, tell });
        }
        latest
    }
}

/// Follows the Queue on a thread of its own, with its own connection to the database at `path`: one look as it
/// starts (an install started in a client while the Dashboard was not running joins then), another each time it is
/// asked, and one a second while there is something to follow ([`POLL`]); none while there is nothing. Each look
/// that changed something is said in the log, sent, and followed by `wake`.
pub fn spawn(path: PathBuf, sources: Vec<Arc<dyn Source>>, wake: impl Fn() + Send + 'static) -> Handle {
    let (asks, asked) = mpsc::channel();
    let (send, updates) = mpsc::channel();
    std::thread::spawn(move || {
        if let Err(e) = follow(&path, sources, &asked, &send, &wake) {
            eprintln!("queue: {}: {e:#}", path.display());
        }
    });
    Handle { asks, updates }
}

fn follow(
    path: &Path,
    sources: Vec<Arc<dyn Source>>,
    asked: &Receiver<Ask>,
    send: &Sender<Update>,
    wake: &dyn Fn(),
) -> Result<()> {
    let mut db = Database::open(path)?;
    let mut follower = Follower::new(sources);
    let clock = std::time::Instant::now();
    let mut first = true;
    loop {
        let Polled { changes, library_changed, moved } =
            follower.poll_at(&mut db, crate::now(), clock.elapsed().as_secs_f64())?;
        // The first look says what it found whenever the Queue holds anything, so that the screens have what each
        // store can be asked of its games (`Queued::can`), which no Queue read from the database carries.
        let holds = first && !db.queue()?.entries.is_empty();
        if !changes.is_empty() || library_changed || moved || holds {
            let name = |game: i64| db.game(game).ok().flatten().map_or_else(|| format!("game {game}"), |g| g.name);
            for change in &changes {
                eprintln!("{}", describe(change, &name));
            }
            let library = if library_changed { Some(db.snapshot()?) } else { None };
            let tell = tells(&changes, first, &follower);
            if send.send(Update { queue: follower.queue(&db)?, library, tell }).is_err() {
                return Ok(());
            }
            wake();
        }
        first = false;
        // With nothing to follow, it still looks every few seconds: an install started in the store's own client
        // joins the Queue, and a game installed or uninstalled there reads so (`Follower::watch_installed`).
        let wait = if follower.busy(&db)? { POLL } else { Duration::from_secs(WATCH_EVERY) };
        let next = asked.recv_timeout(wait);
        match next {
            Ok(Ask::Install(game)) => {
                if follower.asked_install(&mut db, game, crate::now())? {
                    let name = db.game(game)?.map_or_else(|| format!("game {game}"), |g| g.name);
                    eprintln!("{}", describe(&Change::Joined { game, asked: true }, &|_| name.clone()));
                    let update = Update { queue: follower.queue(&db)?, library: None, tell: vec![Tell::Queued(game)] };
                    if send.send(update).is_err() {
                        return Ok(());
                    }
                    wake();
                }
            }
            Ok(Ask::Uninstall(game)) => follower.asked_uninstall(game, crate::now()),
            Ok(Ask::Cancel(game)) => follower.asked_cancel(game),
            Ok(Ask::Stopped { game, stop }) => {
                let (changes, stopped) = follower.stopped_by_store(&mut db, game, stop)?;
                let name = db.game(game)?.map_or_else(|| format!("game {game}"), |g| g.name);
                for change in &changes {
                    eprintln!("{}", describe(change, &|_| name.clone()));
                }
                let update = Update { queue: follower.queue(&db)?, library: None, tell: vec![Tell::Stopped(stopped)] };
                if send.send(update).is_err() {
                    return Ok(());
                }
                wake();
            }
            Ok(Ask::Look) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use model::{Ask as Asked, Found, Game, Install, Start};

    use super::*;

    const NOW: u64 = 1_800_000_000;
    const QUEUED: InstallState = InstallState::Queued;

    /// A store whose games and installs under way a test sets, and which counts how often it was read.
    struct Fake {
        key: &'static str,
        follows: bool,
        games: Mutex<Vec<Game>>,
        installs: Mutex<Vec<Install>>,
        reads: Mutex<usize>,
        /// What its installer can do; none: it has no installer.
        can: Mutex<Option<model::Can>>,
        /// It gives its own word of what is installed (`Source::installed_now`): its games' as they are set.
        watched: Mutex<bool>,
    }

    impl model::Installer for Fake {
        fn can(&self) -> model::Can {
            self.can.lock().unwrap().unwrap_or_default()
        }
    }

    impl Fake {
        fn new(key: &'static str, follows: bool, games: &[(&str, &str, bool)]) -> Arc<Self> {
            let games = games
                .iter()
                .map(|&(id, name, installed)| Game {
                    source: key.into(),
                    source_id: id.into(),
                    name: name.into(),
                    installed,
                    ..Game::default()
                })
                .collect();
            Arc::new(Self {
                key,
                follows,
                games: Mutex::new(games),
                installs: Mutex::default(),
                reads: Mutex::default(),
                can: Mutex::default(),
                watched: Mutex::default(),
            })
        }

        fn report(&self, installs: &[(&str, InstallState)]) {
            *self.installs.lock().unwrap() = installs.iter().map(|&(id, state)| Install::new(id, state)).collect();
        }

        fn set_installed(&self, id: &str, installed: bool) {
            self.games.lock().unwrap().iter_mut().filter(|g| g.source_id == id).for_each(|g| g.installed = installed);
        }

        fn reads(&self) -> usize {
            *self.reads.lock().unwrap()
        }
    }

    impl Source for Fake {
        fn key(&self) -> &'static str {
            self.key
        }

        fn name(&self) -> &'static str {
            "Fake"
        }

        fn read(&self) -> Result<Option<Found>> {
            *self.reads.lock().unwrap() += 1;
            let library = Library { games: self.games.lock().unwrap().clone(), ..Library::default() };
            Ok(Some(Found { library, from: self.key.into() }))
        }

        fn start(&self, _: &Game) -> Option<Start> {
            None
        }

        fn ask(&self, _: &Game, _: Asked) -> Option<Start> {
            None
        }

        fn installs(&self) -> Option<Vec<Install>> {
            self.follows.then(|| self.installs.lock().unwrap().clone())
        }

        fn installed_now(&self) -> Option<Vec<String>> {
            let games = self.games.lock().unwrap();
            let mut ids: Vec<String> = games.iter().filter(|g| g.installed).map(|g| g.source_id.clone()).collect();
            ids.sort();
            self.watched.lock().unwrap().then_some(ids)
        }

        /// The store that reports its installs uninstalls by itself too, as Steam does; the other is the user's.
        fn uninstalls_by_itself(&self) -> bool {
            self.follows
        }

        fn installer(&self) -> Option<&dyn model::Installer> {
            self.can.lock().unwrap().is_some().then_some(self as &dyn model::Installer)
        }
    }

    struct World {
        db: Database,
        follower: Follower,
        fake: Arc<Fake>,
        other: Arc<Fake>,
    }

    impl World {
        /// A store that reports its installs, with Moss and Tern not installed and Wren installed; and one that
        /// does not, with Kite.
        fn new() -> Self {
            let fake = Fake::new("fake", true, &[("1", "Moss", false), ("2", "Tern", false), ("3", "Wren", true)]);
            let other = Fake::new("other", false, &[("9", "Kite", false)]);
            let sources: Vec<Arc<dyn Source>> = vec![fake.clone(), other.clone()];
            let mut db = Database::in_memory().unwrap();
            db.sync_all(&crate::sources::read(&sources), NOW).unwrap();
            Self { db, follower: Follower::new(sources), fake, other }
        }

        fn id(&self, name: &str) -> i64 {
            self.db.snapshot().unwrap().games.iter().find(|g| g.name == name).unwrap().id
        }

        fn poll(&mut self, at: u64) -> Polled {
            self.follower.poll(&mut self.db, NOW + at).unwrap()
        }

        fn ask(&mut self, name: &str, at: u64) -> bool {
            let game = self.id(name);
            self.follower.asked_install(&mut self.db, game, NOW + at).unwrap()
        }

        /// The Queue by name and state, in its order.
        fn queue(&self) -> Vec<(String, InstallState)> {
            let snapshot = self.db.snapshot().unwrap();
            let name = |game| snapshot.game(game).unwrap().name.clone();
            snapshot.queue.entries.iter().map(|entry| (name(entry.game), entry.state)).collect()
        }

        fn installed(&self, name: &str) -> bool {
            self.db.game(self.id(name)).unwrap().unwrap().installed
        }
    }

    fn named(names: &[(&str, InstallState)]) -> Vec<(String, InstallState)> {
        names.iter().map(|&(name, state)| (name.to_string(), state)).collect()
    }

    /// #171: a game uninstalled or installed in the store's own client while the Dashboard runs reads so at the next
    /// look, the source read again only when its own word of what is installed changes.
    #[test]
    fn a_game_installed_or_uninstalled_in_its_stores_own_client_reads_so_within_a_few_seconds() {
        let mut world = World::new();
        *world.fake.watched.lock().unwrap() = true;
        // The first look: the client says what the database says, so nothing is read.
        let reads = world.fake.reads();
        assert!(!world.poll(0).library_changed);
        assert_eq!(world.fake.reads(), reads);
        // Wren uninstalled in the client, Moss installed there: not looked at before the next look is due.
        world.fake.set_installed("3", false);
        world.fake.set_installed("1", true);
        world.poll(1);
        assert_eq!(world.fake.reads(), reads);
        assert!(world.installed("Wren"));
        assert!(world.poll(WATCH_EVERY).library_changed);
        assert!(!world.installed("Wren") && world.installed("Moss"));
        assert!(world.queue().is_empty(), "nothing joins the Queue for it");
        // Nothing changed since: not read again.
        let reads = world.fake.reads();
        world.poll(2 * WATCH_EVERY);
        assert_eq!(world.fake.reads(), reads);
        // A source that gives no such word is never read for it.
        world.other.set_installed("9", true);
        world.poll(3 * WATCH_EVERY);
        assert!(!world.installed("Kite"));
    }

    #[test]
    fn each_entry_carries_what_its_stores_installer_can_do_and_nothing_without_one() {
        let mut world = World::new();
        world.ask("Moss", 0);
        let can = |world: &World| world.follower.queue(&world.db).unwrap().entries[0].can;
        assert_eq!(can(&world), model::Can::NOTHING);
        let pause_and_cancel = model::Can { pause: true, cancel: true, ..model::Can::NOTHING };
        *world.fake.can.lock().unwrap() = Some(pause_and_cancel);
        assert_eq!(can(&world), pause_and_cancel);
        // The database alone knows nothing of stores.
        assert_eq!(world.db.queue().unwrap().entries[0].can, model::Can::NOTHING);
    }

    /// #188: the game itself installed while its store fetches more for it is told of as finished at once, reads
    /// installed and is played as any installed game; its row stays until the store is done, and then leaves with
    /// nothing more told.
    #[test]
    fn a_game_playable_while_its_extras_come_in_is_told_of_then_and_leaves_quietly_later() {
        let mut world = World::new();
        let moss = world.id("Moss");
        world.ask("Moss", 0);
        world.fake.report(&[("1", InstallState::Downloading { done: 10, total: 10 })]);
        world.poll(1);
        // Steam's manifest says installed; its Workshop items are still coming.
        let extras = InstallState::Extras { items_done: 1, items: 4, done: 0, total: 0 };
        world.fake.set_installed("1", true);
        world.fake.report(&[("1", extras)]);
        let polled = world.poll(2);
        assert_eq!(polled.changes, [Change::State { game: moss, state: extras }, Change::Playable(moss)]);
        assert!(polled.library_changed && world.installed("Moss"));
        let tell: Vec<Tell> = polled.changes.iter().filter_map(|change| Tell::of(change, false)).collect();
        assert_eq!(tell, [Tell::Ready(moss)]);
        let snapshot = world.db.snapshot().unwrap();
        assert_eq!((snapshot.queue.status(moss), snapshot.queue.get(moss).map(|e| e.state)), (None, Some(extras)));
        assert_eq!(snapshot.queue.installed.first().map(|done| done.game), Some(moss));
        // Still fetching, as it was: nothing more, its items kept as they were said.
        assert!(world.poll(3).changes.is_empty());
        // More items in: the row moves on, and the log says so.
        let more = InstallState::Extras { items_done: 3, items: 4, done: 300, total: 400 };
        world.fake.report(&[("1", more)]);
        assert_eq!(world.poll(3).changes, [Change::State { game: moss, state: more }]);
        assert_eq!(world.db.snapshot().unwrap().queue.get(moss).map(|e| e.state), Some(more));
        let name = |_| "Moss".to_string();
        assert_eq!(
            describe(&Change::State { game: moss, state: more }, &name),
            "queue: Moss downloading Workshop content, 3 of 4 items, 300 of 400 bytes"
        );
        // Done: it leaves, with no second Toast.
        world.fake.report(&[]);
        let polled = world.poll(4);
        assert_eq!(polled.changes, [Change::ExtrasDone(moss)]);
        assert!(polled.changes.iter().all(|change| Tell::of(change, false).is_none()));
        let snapshot = world.db.snapshot().unwrap();
        assert!(snapshot.queue.is_empty());
        assert_eq!(snapshot.queue.installed.first().map(|done| done.game), Some(moss));
        assert_eq!(
            describe(&Change::Playable(moss), &name),
            "queue: Moss installed; its store is still fetching more for it"
        );
        assert_eq!(
            describe(&Change::State { game: moss, state: extras }, &name),
            "queue: Moss downloading Workshop content, 1 of 4 items"
        );
    }

    #[test]
    fn an_install_asked_for_goes_from_queued_to_downloading_to_installed() {
        let mut world = World::new();
        let moss = world.id("Moss");
        assert!(world.ask("Moss", 0));
        // Asked again, for one installed, or of a store that says nothing of its installs: nothing more.
        assert!(!world.ask("Moss", 0) && !world.ask("Wren", 0) && !world.ask("Kite", 0));
        assert_eq!(world.queue(), named(&[("Moss", QUEUED)]));
        assert!(world.follower.busy(&world.db).unwrap());

        // The client takes it on, then downloads it.
        // Its first report says the store has it, Queued.
        world.fake.report(&[("1", QUEUED)]);
        assert_eq!(world.poll(1).changes, [Change::State { game: moss, state: QUEUED }]);
        assert!(world.poll(1).changes.is_empty());
        let downloading = InstallState::Downloading { done: 250, total: 1000 };
        world.fake.report(&[("1", downloading)]);
        assert_eq!(world.poll(2).changes, [Change::State { game: moss, state: downloading }]);
        assert_eq!(world.queue(), named(&[("Moss", downloading)]));
        // Bytes that do not move it a whole percent are kept, and not told of.
        let more = InstallState::Downloading { done: 255, total: 1000 };
        world.fake.report(&[("1", more)]);
        assert!(world.poll(3).changes.is_empty());
        assert_eq!(world.queue(), named(&[("Moss", more)]));

        // Done: the client reports it no more, its source is read again and has it installed.
        world.fake.report(&[]);
        world.fake.set_installed("1", true);
        let polled = world.poll(4);
        assert_eq!(polled.changes, [Change::Installed(moss)]);
        assert!(polled.library_changed && world.installed("Moss"));
        let snapshot = world.db.snapshot().unwrap();
        assert!(snapshot.queue.is_empty());
        assert_eq!(snapshot.queue.installed, [Installed { game: moss, finished: NOW + 4 }]);
        assert!(!world.follower.busy(&world.db).unwrap());
    }

    #[test]
    fn an_install_the_client_drops_leaves_the_queue_at_once() {
        let mut world = World::new();
        let tern = world.id("Tern");
        // Started in the client itself: it joins as it is first reported, paused here.
        let paused = InstallState::Paused { done: 5, total: 10 };
        world.fake.report(&[("2", paused)]);
        assert_eq!(
            world.poll(0).changes,
            [Change::Joined { game: tern, asked: false }, Change::State { game: tern, state: paused }]
        );
        assert_eq!(world.db.queue().unwrap().status(tern), Some(model::queue::QUEUED));
        world.fake.report(&[]);
        assert_eq!(world.poll(1).changes, [Change::Dropped(tern)]);
        assert!(world.queue().is_empty() && !world.installed("Tern"));
        assert!(world.db.queue().unwrap().installed.is_empty());
    }

    #[test]
    fn the_current_download_comes_first_and_the_rest_in_the_order_they_joined() {
        let mut world = World::new();
        world.ask("Moss", 0);
        world.ask("Tern", 5);
        assert_eq!(world.queue(), named(&[("Moss", QUEUED), ("Tern", QUEUED)]));
        let downloading = InstallState::Downloading { done: 1, total: 2 };
        world.fake.report(&[("1", QUEUED), ("2", downloading)]);
        world.poll(6);
        assert_eq!(world.queue(), named(&[("Tern", downloading), ("Moss", QUEUED)]));
    }

    #[test]
    fn the_queue_is_kept_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(crate::FILE);
        let fake = Fake::new("fake", true, &[("1", "Moss", false)]);
        let sources: Vec<Arc<dyn Source>> = vec![fake.clone()];
        let downloading = InstallState::Downloading { done: 1, total: 4 };
        {
            let mut db = Database::open(&path).unwrap();
            db.sync_all(&crate::sources::read(&sources), NOW).unwrap();
            let mut follower = Follower::new(sources.clone());
            let moss = db.snapshot().unwrap().games[0].id;
            follower.asked_install(&mut db, moss, NOW).unwrap();
            fake.report(&[("1", downloading)]);
            follower.poll(&mut db, NOW + 1).unwrap();
        }
        let mut db = Database::open(&path).unwrap();
        let queue = db.snapshot().unwrap().queue;
        assert_eq!(queue.entries.iter().map(|e| (e.state, e.started)).collect::<Vec<_>>(), [(downloading, NOW)]);
        // Still reported after the restart: where it was, and nothing to tell.
        let mut follower = Follower::new(sources);
        assert!(follower.poll(&mut db, NOW + 100).unwrap().changes.is_empty());
        assert_eq!(db.queue().unwrap().len(), 1);
    }

    #[test]
    fn an_install_asked_for_that_its_client_never_reports_leaves_after_a_minute() {
        let mut world = World::new();
        let (moss, tern) = (world.id("Moss"), world.id("Tern"));
        world.ask("Moss", 0);
        world.ask("Tern", 0);
        // Its source is read again every few seconds meanwhile: one that installed between two looks is found.
        let reads = world.fake.reads();
        assert!(world.poll(1).changes.is_empty());
        assert_eq!(world.fake.reads(), reads + 1);
        assert!(world.poll(2).changes.is_empty());
        assert_eq!(world.fake.reads(), reads + 1, "not again so soon");
        world.fake.set_installed("2", true);
        assert_eq!(world.poll(1 + READ_EVERY).changes, [Change::Installed(tern)]);
        assert_eq!(world.queue(), named(&[("Moss", QUEUED)]));
        assert!(world.poll(GIVE_UP_AFTER - 1).changes.is_empty());
        assert_eq!(world.poll(GIVE_UP_AFTER).changes, [Change::Dropped(moss)]);
        assert!(world.queue().is_empty());
    }

    #[test]
    fn an_install_asked_for_that_its_store_takes_on_then_drops_leaves_at_once() {
        let mut world = World::new();
        let moss = world.id("Moss");
        world.ask("Moss", 0);
        world.fake.report(&[("1", QUEUED)]);
        assert_eq!(world.poll(1).changes, [Change::State { game: moss, state: QUEUED }]);
        // Dropped in the client: gone at the next look, not a minute after it was asked for.
        world.fake.report(&[]);
        assert_eq!(world.poll(2).changes, [Change::Dropped(moss)]);
        assert!(world.queue().is_empty());
    }

    /// #230: a game never just leaves the Queue. Each that leaves without installing has a stopped row saying why,
    /// told in a Toast: as its store's installer said, cancelled here, stopped in the store, never started. The row
    /// goes once it is asked for again or installed.
    #[test]
    fn a_game_that_leaves_without_installing_says_why_until_it_is_asked_for_again() {
        let mut world = World::new();
        let (moss, tern) = (world.id("Moss"), world.id("Tern"));
        let stop_of = |world: &World, game: i64| world.follower.stopped(game).map(|stopped| stopped.stop.clone());

        // Stopped by its store's installer: told at once, out of the Queue.
        world.ask("Moss", 0);
        let no_room = Stop::NoSpace { needed: 20 << 30, free: 1 << 30 };
        let (changes, stopped) = world.follower.stopped_by_store(&mut world.db, moss, no_room.clone()).unwrap();
        assert_eq!(changes, [Change::Dropped(moss)]);
        assert_eq!((stopped.store.as_str(), stopped.headline()), ("Fake", "Not enough space".to_string()));
        assert!(world.queue().is_empty());
        let queue = world.follower.queue(&world.db).unwrap();
        assert_eq!(queue.stopped.iter().map(|stopped| stopped.game).collect::<Vec<_>>(), [moss]);

        // Asked for again: its row goes.
        world.ask("Moss", 1);
        assert!(world.follower.queue(&world.db).unwrap().stopped.is_empty());
        assert_eq!(stop_of(&world, moss), None);

        // Taken on by its store, then cancelled here: "Installation cancelled", told with the drop.
        world.fake.report(&[("1", QUEUED)]);
        world.poll(2);
        world.follower.asked_cancel(moss);
        world.fake.report(&[]);
        let polled = world.poll(3);
        assert_eq!(polled.changes, [Change::Dropped(moss)]);
        assert_eq!(stop_of(&world, moss), Some(Stop::Cancelled));
        assert_eq!(tells(&polled.changes, false, &world.follower), [Tell::Stopped(world.follower.stopped(moss).unwrap().clone())]);

        // Taken on, then dropped in the store's own client: "Stopped in <store>".
        world.ask("Tern", 4);
        world.fake.report(&[("2", QUEUED)]);
        world.poll(5);
        world.fake.report(&[]);
        world.poll(6);
        assert_eq!(stop_of(&world, tern), Some(Stop::InStore));

        // Never reported by a store that reports its installs: "<store> didn't start it".
        world.ask("Tern", 7);
        world.poll(7 + GIVE_UP_AFTER);
        assert_eq!(stop_of(&world, tern), Some(Stop::NeverStarted));
        // The latest first.
        let queue = world.follower.queue(&world.db).unwrap();
        assert_eq!(queue.stopped.iter().map(|stopped| stopped.game).collect::<Vec<_>>(), [tern, moss]);

        // Installed after all: its row goes.
        world.fake.set_installed("2", true);
        let sources: Vec<Arc<dyn Source>> = vec![world.fake.clone(), world.other.clone()];
        world.db.sync_all(&crate::sources::read(&sources), NOW).unwrap();
        assert!(world.follower.queue(&world.db).unwrap().stopped.iter().all(|stopped| stopped.game != tern));
    }

    #[test]
    fn an_uninstall_is_followed_until_the_game_reads_uninstalled() {
        let mut world = World::new();
        let wren = world.id("Wren");
        world.follower.asked_uninstall(wren, NOW);
        assert!(world.follower.busy(&world.db).unwrap());
        assert!(world.poll(1).changes.is_empty());
        // Read once, the game still installed.
        assert!(world.poll(UNINSTALL_FIRST_READ).changes.is_empty());
        world.fake.set_installed("3", false);
        assert!(world.poll(UNINSTALL_FIRST_READ + 1).changes.is_empty(), "not read again so soon");
        let polled = world.poll(UNINSTALL_FIRST_READ + READ_EVERY);
        assert_eq!(polled.changes, [Change::Uninstalled(wren)]);
        assert!(polled.library_changed && !world.installed("Wren"));
        assert!(!world.follower.busy(&world.db).unwrap());
        // One that never goes is given up on, and said to be still there: asking was all it took.
        world.fake.set_installed("3", true);
        world.follower.asked_uninstall(wren, NOW + 1000);
        assert!(world.poll(1000 + UNINSTALL_FOR - 1).changes.is_empty());
        assert_eq!(world.poll(1000 + UNINSTALL_FOR).changes, [Change::StillInstalled(wren)]);
        assert!(!world.follower.busy(&world.db).unwrap());
    }

    #[test]
    fn an_uninstall_that_is_the_users_to_finish_is_followed_longer_and_its_wait_ends_quietly() {
        let mut world = World::new();
        let kite = world.id("Kite");
        world.other.set_installed("9", true);
        world.follower.asked_uninstall(kite, NOW);
        // Still there after two minutes: the user may not have got to it yet.
        assert!(world.poll(UNINSTALL_FOR).changes.is_empty());
        assert!(world.follower.busy(&world.db).unwrap());
        // Ten minutes on it is no longer followed, and nothing is said of it.
        assert!(world.poll(USER_UNINSTALL_FOR).changes.is_empty());
        assert!(!world.follower.busy(&world.db).unwrap());
        // One the user does uninstall meanwhile is seen to go, and told of.
        world.follower.asked_uninstall(kite, NOW + 1000);
        world.other.set_installed("9", false);
        assert_eq!(world.poll(1000 + 300).changes, [Change::Uninstalled(kite)]);
    }

    #[test]
    fn what_is_told_in_a_toast() {
        let joined = Change::Joined { game: 1, asked: false };
        assert_eq!(Tell::of(&joined, false), Some(Tell::Queued(1)));
        // Already in the Queue as the Dashboard starts: it joined before anyone was there to see.
        assert_eq!(Tell::of(&joined, true), None);
        assert_eq!(Tell::of(&Change::Installed(2), true), Some(Tell::Ready(2)));
        assert_eq!(Tell::of(&Change::Uninstalled(3), false), Some(Tell::Uninstalled(3)));
        let state = Change::State { game: 1, state: QUEUED };
        assert_eq!((Tell::of(&state, false), Tell::of(&Change::Dropped(1), false)), (None, None));
        assert_eq!(Tell::of(&Change::StillInstalled(1), false), None);
    }

    #[test]
    fn a_downloads_speed_is_worked_out_from_its_bytes_smoothed_and_falls_to_nothing_once_they_stand_still() {
        let mut world = World::new();
        let moss = world.id("Moss");
        world.ask("Moss", 0);
        let speed = |world: &World| world.follower.queue(&world.db).unwrap().get(moss).unwrap().speed;
        let at = |world: &mut World, done: u64, at: u64| {
            world.fake.report(&[("1", InstallState::Downloading { done, total: 10_000 })]);
            world.poll(at)
        };
        // Queued, and the first look at its bytes: no speed yet.
        assert!(!world.poll(1).moved);
        assert!(at(&mut world, 0, 2).moved);
        assert_eq!(speed(&world), None);
        // 250 bytes in a second: 2 kilobits a second.
        assert!(at(&mut world, 250, 3).moved);
        assert_eq!(speed(&world), Some(2000));
        // The bytes where they were: the speed stays, and nothing has moved.
        assert!(!at(&mut world, 250, 4).moved);
        assert_eq!(speed(&world), Some(2000));
        // A thousand more over the two seconds since they last moved: 4 kilobits a second, of which half is taken.
        at(&mut world, 1250, 5);
        assert_eq!(speed(&world), Some(3000));
        // Standing still for longer than Steam goes between writes: 0.
        at(&mut world, 1250, 5 + STALLED_AFTER as u64);
        assert_eq!(speed(&world), Some(3000));
        assert!(at(&mut world, 1250, 6 + STALLED_AFTER as u64).moved);
        assert_eq!(speed(&world), Some(0));
        // Moving again, it starts from what it moves at; paused, it has none.
        at(&mut world, 2250, 7 + STALLED_AFTER as u64);
        assert_eq!(speed(&world), Some(8000));
        world.fake.report(&[("1", InstallState::Paused { done: 2250, total: 10_000 })]);
        world.poll(8 + STALLED_AFTER as u64);
        assert_eq!(speed(&world), None);
        // The database keeps no speed.
        assert!(world.db.queue().unwrap().entries.iter().all(|entry| entry.speed.is_none()));
    }

    #[test]
    fn the_log_says_what_happened() {
        let name = |_| "Quiet Orbit".to_string();
        let say = |change| describe(&change, &name);
        assert_eq!(say(Change::Joined { game: 1, asked: true }), "queue: Quiet Orbit joined, asked for");
        assert_eq!(say(Change::Joined { game: 1, asked: false }), "queue: Quiet Orbit joined, from its store's client");
        assert_eq!(say(Change::State { game: 1, state: QUEUED }), "queue: Quiet Orbit queued");
        let state = InstallState::Downloading { done: 250, total: 1000 };
        assert_eq!(say(Change::State { game: 1, state }), "queue: Quiet Orbit downloading, 25% of 1000 bytes");
        let state = InstallState::Paused { done: 0, total: 0 };
        assert_eq!(say(Change::State { game: 1, state }), "queue: Quiet Orbit paused");
        assert_eq!(say(Change::Installed(1)), "queue: Quiet Orbit installed");
        assert_eq!(say(Change::Dropped(1)), "queue: Quiet Orbit dropped");
        assert_eq!(say(Change::Uninstalled(1)), "queue: Quiet Orbit uninstalled");
    }

    #[test]
    fn the_thread_tells_of_an_install_asked_for_and_of_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(crate::FILE);
        let fake = Fake::new("fake", true, &[("1", "Moss", false)]);
        let sources: Vec<Arc<dyn Source>> = vec![fake.clone()];
        let moss = {
            let mut db = Database::open(&path).unwrap();
            db.sync_all(&crate::sources::read(&sources), NOW).unwrap();
            db.snapshot().unwrap().games[0].id
        };
        let (woken, wakes) = mpsc::channel();
        let handle = spawn(path, sources, move || {
            let _ = woken.send(());
        });
        let wait = |handle: &Handle| {
            wakes.recv_timeout(Duration::from_secs(10)).expect("woken");
            handle.take().expect("an update")
        };
        handle.ask(Ask::Install(moss));
        let update = wait(&handle);
        assert_eq!((update.queue.status(moss), update.tell), (Some(model::queue::QUEUED), vec![Tell::Queued(moss)]));
        fake.report(&[("1", InstallState::Downloading { done: 1, total: 2 })]);
        let update = wait(&handle);
        assert_eq!((update.queue.status(moss), update.tell), (Some(model::queue::INSTALLING), vec![]));
        fake.report(&[]);
        fake.set_installed("1", true);
        let update = wait(&handle);
        assert!(update.queue.is_empty());
        assert_eq!(update.tell, [Tell::Ready(moss)]);
        assert!(update.library.expect("a snapshot").games[0].installed);
    }
}
