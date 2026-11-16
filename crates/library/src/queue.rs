//! The Queue (`model::Queue`): every game being installed, kept in the library database across restarts in the
//! order each joined, and squared once a second with what the sources say they have under way
//! (`model::Source::installs`) while it holds anything.
//!
//! - A game joins as the Dashboard asks its store for it (`Follower::asked_install`), Queued, if its source keeps
//!   word of its installs; or as its source first reports it, which is how an install started in the store's own
//!   client joins.
//! - While its source reports it, it is where the source says: Queued, downloading so many bytes of so many, or
//!   paused.
//! - Once its source no longer reports it, the source is read again (`Source::read`, merged in as every read is):
//!   a game that now reads installed has finished and leaves for "recently installed", with when; one that does not
//!   was dropped (cancelled in the client, or at its install dialog) and simply leaves.
//! - One asked for that its source never reports leaves after a minute ([`GIVE_UP_AFTER`]), finished if a read then
//!   finds it installed (a small game can install between two looks), else dropped. Meanwhile its source is read
//!   every [`READ_EVERY`] seconds, so a game that finished that quickly does not wait the minute.
//!
//! An uninstall the Dashboard asked for is followed the same way, by reading its source again every few seconds
//! until the game reads uninstalled, for two minutes at most ([`UNINSTALL_FOR`]).
//!
//! [`spawn`] runs all this on a thread of its own, which polls only while there is something to follow and wakes
//! whoever started it when the Queue or the library has changed.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use model::{InstallState, Installed, Library, Queue, Queued, Source};
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
        -- queued, downloading or paused; with the bytes done and to do, as the source last said.
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
/// How long an uninstall is followed.
pub const UNINSTALL_FOR: u64 = 120;
/// How many finished installs a snapshot carries.
pub const RECENTLY_INSTALLED: usize = 8;
/// How often the Queue is looked at while there is something to follow.
pub const POLL: Duration = Duration::from_secs(1);

/// What happened to the Queue, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Joined: asked for here, or first seen in the client.
    Joined { game: i64, asked: bool },
    /// Moved on: another kind of state, or another whole percent.
    State { game: i64, state: InstallState },
    /// Finished installing.
    Installed(i64),
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
    started: u64,
    asked: Option<u64>,
    reported: bool,
    state: InstallState,
}

fn state_of(kind: &str, done: i64, total: i64) -> InstallState {
    let (done, total) = (done.max(0) as u64, total.max(0) as u64);
    match kind {
        "downloading" => InstallState::Downloading { done, total },
        "paused" => InstallState::Paused { done, total },
        _ => InstallState::Queued,
    }
}

fn columns(state: InstallState) -> (&'static str, i64, i64) {
    match state {
        InstallState::Queued => ("queued", 0, 0),
        InstallState::Downloading { done, total } => ("downloading", done as i64, total as i64),
        InstallState::Paused { done, total } => ("paused", done as i64, total as i64),
    }
}

/// Whether a change of state is worth a line in the log: another kind, or another whole percent.
fn moved(was: InstallState, now: InstallState) -> bool {
    std::mem::discriminant(&was) != std::mem::discriminant(&now) || was.percent() != now.percent()
}

impl Database {
    /// The Queue as the database has it: its games, the current download first, and the latest it saw finish.
    /// A game gone from its source is in neither.
    pub fn queue(&self) -> Result<Queue> {
        let entries = self
            .rows()?
            .into_iter()
            .filter(|row| !row.gone)
            .map(|row| Queued { game: row.game, state: row.state, started: row.started })
            .collect();
        let mut installed = self.conn.prepare(
            "SELECT game, finished FROM installed JOIN games ON games.id = installed.game
             WHERE games.gone = 0 ORDER BY finished DESC, game DESC LIMIT ?1",
        )?;
        let installed = installed
            .query_map([RECENTLY_INSTALLED as i64], |row| {
                Ok(Installed { game: row.get(0)?, finished: row.get::<_, i64>(1)?.max(0) as u64 })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Queue::new(entries, installed))
    }

    fn rows(&self) -> Result<Vec<Row>> {
        let mut rows = self.conn.prepare(
            "SELECT queue.game, source, gone, started, asked, reported, state, done, total
             FROM queue JOIN games ON games.id = queue.game",
        )?;
        let rows = rows.query_map([], |row| {
            Ok(Row {
                game: row.get(0)?,
                source: row.get(1)?,
                gone: row.get(2)?,
                started: row.get::<_, i64>(3)?.max(0) as u64,
                asked: row.get::<_, Option<i64>>(4)?.map(|asked| asked.max(0) as u64),
                reported: row.get(5)?,
                state: state_of(&row.get::<_, String>(6)?, row.get(7)?, row.get(8)?),
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

    /// Whether the game reads installed.
    fn installed(&self, game: i64) -> Result<bool> {
        let installed =
            self.conn.query_row("SELECT installed FROM games WHERE id = ?1", [game], |row| row.get(0)).optional()?;
        Ok(installed.unwrap_or(false))
    }

    fn join(&self, game: i64, now: u64, asked: bool, state: InstallState) -> Result<()> {
        let (kind, done, total) = columns(state);
        self.conn.execute(
            "INSERT INTO queue (game, started, asked, reported, state, done, total) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![game, now as i64, asked.then_some(now as i64), !asked, kind, done, total],
        )?;
        Ok(())
    }

    fn set_state(&self, game: i64, state: InstallState) -> Result<()> {
        let (kind, done, total) = columns(state);
        self.conn.execute(
            "UPDATE queue SET reported = 1, state = ?2, done = ?3, total = ?4 WHERE game = ?1",
            params![game, kind, done, total],
        )?;
        Ok(())
    }

    /// The game leaves the Queue: for Recently installed, at `now`, if it finished.
    fn leave(&self, game: i64, finished: Option<u64>) -> Result<()> {
        self.conn.execute("DELETE FROM queue WHERE game = ?1", [game])?;
        if let Some(at) = finished {
            self.conn.execute(
                "INSERT INTO installed (game, finished) VALUES (?1, ?2)
                 ON CONFLICT (game) DO UPDATE SET finished = ?2",
                params![game, at as i64],
            )?;
        }
        Ok(())
    }
}

/// What a look at the Queue found.
#[derive(Debug, Default)]
pub struct Polled {
    pub changes: Vec<Change>,
    /// A source was read again and changed the library: the screens want a new snapshot, not only the Queue.
    pub library_changed: bool,
}

/// Follows the Queue and the uninstalls asked for: what is kept between looks and is not the database's.
pub struct Follower {
    sources: Vec<Arc<dyn Source>>,
    /// The uninstalls asked for, by game, with when.
    uninstalls: HashMap<i64, u64>,
    /// When each source was last read again, by its key.
    read_at: HashMap<&'static str, u64>,
}

impl Follower {
    pub fn new(sources: Vec<Arc<dyn Source>>) -> Self {
        Self { sources, uninstalls: HashMap::new(), read_at: HashMap::new() }
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
        Ok(!self.uninstalls.is_empty() || !db.rows()?.is_empty())
    }

    /// Reads the source again and merges it in, as at start; whether that changed anything. One that can't be read
    /// is said in the log, as it is at start, and changes nothing.
    fn read_again(&mut self, db: &mut Database, key: &str, now: u64) -> Result<bool> {
        let Some(source) = self.source(key).cloned() else { return Ok(false) };
        self.read_at.insert(source.key(), now);
        let read = crate::sources::read(std::slice::from_ref(&source));
        let synced = db.sync_all(&read, now)?;
        Ok(synced != crate::Synced::default())
    }

    /// Whether the source was read longer ago than `every`, or never.
    fn due(&self, key: &str, now: u64, every: u64) -> bool {
        self.read_at.get(key).is_none_or(|&at| now.saturating_sub(at) >= every)
    }

    /// Squares the Queue with what the sources say they have under way, at `now`.
    pub fn poll(&mut self, db: &mut Database, now: u64) -> Result<Polled> {
        let mut polled = Polled::default();
        let mut reports: HashMap<&'static str, Vec<model::Install>> = HashMap::new();
        for source in &self.sources {
            if let Some(installs) = source.installs() {
                reports.insert(source.key(), installs);
            }
        }
        let rows = db.rows()?;
        let mut seen: Vec<i64> = Vec::new();
        for (&key, installs) in &reports {
            for install in installs {
                let Some(game) = db.game_of(key, &install.source_id)? else { continue };
                seen.push(game);
                match rows.iter().find(|row| row.game == game) {
                    Some(row) if row.reported && row.state == install.state => {}
                    Some(row) => {
                        db.set_state(game, install.state)?;
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

        // What its source no longer has, or never had: ended, or to be looked at again.
        let mut ended: Vec<&Row> = Vec::new();
        let mut waiting: Vec<&Row> = Vec::new();
        for row in rows.iter().filter(|row| !seen.contains(&row.game)) {
            if row.gone || !reports.contains_key(row.source.as_str()) {
                db.leave(row.game, None)?;
                polled.changes.push(Change::Dropped(row.game));
            } else if row.reported || row.asked.is_none_or(|asked| now.saturating_sub(asked) >= GIVE_UP_AFTER) {
                ended.push(row);
            } else {
                waiting.push(row);
            }
        }
        let mut to_read: Vec<String> = ended.iter().map(|row| row.source.clone()).collect();
        to_read.extend(waiting.iter().filter(|row| self.due(&row.source, now, READ_EVERY)).map(|r| r.source.clone()));
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

        // A source read again has said whether each finished.
        for row in &ended {
            let installed = db.installed(row.game)?;
            db.leave(row.game, installed.then_some(now))?;
            polled.changes.push(if installed { Change::Installed(row.game) } else { Change::Dropped(row.game) });
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
            } else if now.saturating_sub(asked) >= UNINSTALL_FOR {
                self.uninstalls.remove(&game);
                polled.changes.push(Change::StillInstalled(game));
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
                (state, Some((_, total)), Some(percent)) => {
                    let kind = if state.downloading() { "downloading" } else { "paused" };
                    format!("{kind}, {percent}% of {total} bytes")
                }
                (state, ..) => (if state.downloading() { "downloading" } else { "paused" }).to_string(),
            };
            format!("{} {how}", name(*game))
        }
        Change::Installed(game) => format!("{} installed", name(*game)),
        Change::Dropped(game) => format!("{} dropped", name(*game)),
        Change::Uninstalled(game) => format!("{} uninstalled", name(*game)),
        Change::StillInstalled(game) => {
            format!("{} still installed {UNINSTALL_FOR} s after it was asked to uninstall", name(*game))
        }
    };
    format!("queue: {says}")
}

/// What the thread was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// The game's store was asked to install it.
    Install(i64),
    /// The game's store was asked to uninstall it.
    Uninstall(i64),
    /// Look now: the sources have been read, and may have news.
    Look,
}

/// What the thread has for the screens: the Queue, and with it the library's snapshot when a source read again
/// changed it.
#[derive(Debug, Clone)]
pub struct Update {
    pub queue: Queue,
    pub library: Option<Library>,
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
            // A library snapshot is kept, should a later update carry only the Queue.
            let library = update.library.or(latest.and_then(|latest| latest.library));
            latest = Some(Update { queue: update.queue, library });
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
    loop {
        let Polled { changes, library_changed } = follower.poll(&mut db, crate::now())?;
        if !changes.is_empty() || library_changed {
            let name = |game: i64| db.game(game).ok().flatten().map_or_else(|| format!("game {game}"), |g| g.name);
            for change in &changes {
                eprintln!("{}", describe(change, &name));
            }
            let library = if library_changed { Some(db.snapshot()?) } else { None };
            if send.send(Update { queue: db.queue()?, library }).is_err() {
                return Ok(());
            }
            wake();
        }
        let next = if follower.busy(&db)? {
            asked.recv_timeout(POLL)
        } else {
            asked.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match next {
            Ok(Ask::Install(game)) => {
                if follower.asked_install(&mut db, game, crate::now())? {
                    let name = db.game(game)?.map_or_else(|| format!("game {game}"), |g| g.name);
                    eprintln!("{}", describe(&Change::Joined { game, asked: true }, &|_| name.clone()));
                    if send.send(Update { queue: db.queue()?, library: None }).is_err() {
                        return Ok(());
                    }
                    wake();
                }
            }
            Ok(Ask::Uninstall(game)) => follower.asked_uninstall(game, crate::now()),
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
            })
        }

        fn report(&self, installs: &[(&str, InstallState)]) {
            *self.installs.lock().unwrap() =
                installs.iter().map(|&(id, state)| Install { source_id: id.into(), state }).collect();
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
        // One that never goes is given up on.
        let kite = world.id("Kite");
        world.other.set_installed("9", true);
        world.follower.asked_uninstall(kite, NOW);
        assert_eq!(world.poll(UNINSTALL_FOR).changes, [Change::StillInstalled(kite)]);
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
        assert_eq!(wait(&handle).queue.status(moss), Some(model::queue::QUEUED));
        fake.report(&[("1", InstallState::Downloading { done: 1, total: 2 })]);
        assert_eq!(wait(&handle).queue.status(moss), Some(model::queue::INSTALLING));
        fake.report(&[]);
        fake.set_installed("1", true);
        let update = wait(&handle);
        assert!(update.queue.is_empty());
        assert!(update.library.expect("a snapshot").games[0].installed);
    }
}
