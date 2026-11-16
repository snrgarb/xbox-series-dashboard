//! Where the Library the Dashboard shows comes from, and where what happens to it goes: the library database, with
//! every source (`crate::sources`) read into it.
//!
//! When the database has games the first frame is drawn from it, and the sources are read on a thread of their own
//! once that frame is out; if that changes anything the window is woken and takes a new snapshot. With an empty
//! database (a first run) or no window to wake (`--screenshot`), they are read before anything is drawn.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use library::sources::Read;
use library::Database;
use model::{FilterChange, Game, Library, Source};
use screens::settings::{Change, Options, Setting};

use crate::{fail, Args};

#[derive(Default)]
pub struct Store {
    database: Option<Database>,
    /// The database's file, which the thread that syncs opens for itself.
    path: PathBuf,
    /// Every source, in order.
    sources: Vec<Arc<dyn Source>>,
    /// The sources are still to be read, the Library on screen having come from the database.
    pending: bool,
    /// Hears from the thread that syncs when it has changed the database.
    synced: Option<Receiver<()>>,
    /// A game is running, and `game_booted` has been told so.
    booted: bool,
    /// The running game's Play session.
    session: Option<i64>,
}

/// The one place the Library comes from: `--library`'s JSON as it is; else the database, filled from every source
/// (Steam's from `--steam`'s install or the one the registry names); else, with no database and no source
/// installed, the built-in fake one. A source that can't be read is passed over; but with nothing else to show
/// the Dashboard does not start, and says why.
///
/// `--screenshot` uses a database only where `--data-dir` names one: drawing a picture leaves nothing behind, and
/// what the sources say goes to a database in memory.
///
/// A Library that came from no database has its filters, which are Home's Groups, worked out by one in memory.
pub fn open(args: &Args) -> (Library, Store) {
    let sources = crate::sources::all(args);
    if let Some(path) = &args.library {
        let json = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        let library = serde_json::from_str(&json).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        let (library, database) = with_filters(library);
        return (library, Store { database, sources, ..Store::default() });
    }
    let window = args.screenshot.is_none();
    let dir = args.data_dir.clone().or_else(|| if window { library::data_dir() } else { None });
    let path = dir.map(|dir| dir.join(library::FILE));
    let started = Instant::now();
    // Without a database the Dashboard still runs, on what the sources say now.
    let mut database = path
        .as_ref()
        .and_then(|path| Database::open(path).map_err(|e| eprintln!("library: {}: {e:#}", path.display())).ok());
    let stored = database.as_ref().and_then(|db| db.snapshot().ok()).filter(|library| !library.games.is_empty());
    if let Some(library) = &stored {
        let ms = started.elapsed().as_millis();
        eprintln!("library: {} games from the database in {ms} ms", library.games.len());
    }

    let mut pending = false;
    let library = match stored {
        Some(library) if window => {
            pending = true;
            library
        }
        stored => {
            let read = library::sources::read(&sources);
            match (read.found.is_empty(), stored, read.failed.first()) {
                (true, Some(library), _) => library,
                (true, None, Some((_, e))) => fail(&format!("{e:#}")),
                (true, None, None) => screens::fake_library(),
                (false, _, _) => {
                    // With no file to keep it in, the library is kept in memory for as long as the Dashboard runs.
                    if database.is_none() {
                        database = Database::in_memory().map_err(|e| eprintln!("library: {e:#}")).ok();
                    }
                    match database.as_mut().map(|db| sync(db, &read)) {
                        Some(Ok(library)) => library,
                        Some(Err(e)) => {
                            eprintln!("library: {e:#}");
                            merged(read)
                        }
                        None => merged(read),
                    }
                }
            }
        }
    };
    // The marks the user makes (a favourite, a hidden game) go to the database the Library on screen came from.
    let (library, memory) = with_filters(library);
    let database = memory.or(database);
    (library, Store { database, path: path.unwrap_or_default(), sources, pending, ..Store::default() })
}

/// What the sources read, put together with no database: every game of each in one Library.
fn merged(read: Read) -> Library {
    let mut library = Library::default();
    for (_, found) in read.found {
        library.user = library.user.or(found.user);
        library.games.extend(found.games);
    }
    library
}

/// The Library with the saved filters and their games, if it has none: one read from a file, the fake one, or the
/// sources' with no database to keep them in. A database in memory takes each game in as its source's and gives
/// its snapshot, so the built-in filters and the stores' collections are there as they would be from the real one.
/// A made-up game, of no source, is taken in as Steam's, whose mark the screens give it; the user and the
/// collections go with the first source's games. Its games are given ids, which last as long as that database,
/// which is given too: what the user marks while the Dashboard runs (a favourite, a hidden game) is kept there, and
/// gone with it.
fn with_filters(library: Library) -> (Library, Option<Database>) {
    if !library.filters.is_empty() {
        return (library, None);
    }
    let through = || -> Result<(Library, Option<Database>)> {
        let mut database = Database::in_memory()?;
        let key =
            |game: &Game| if game.source.is_empty() { source_steam::SOURCE.to_string() } else { game.source.clone() };
        let first = library.games.first().map_or(source_steam::SOURCE.to_string(), key);
        let mut by_source: BTreeMap<String, Library> = BTreeMap::new();
        by_source.insert(
            first,
            Library { user: library.user.clone(), collections: library.collections.clone(), ..Library::default() },
        );
        for game in &library.games {
            by_source.entry(key(game)).or_default().games.push(game.clone());
        }
        for (source, games) in &by_source {
            database.sync(source, games, library::now())?;
        }
        Ok((database.snapshot()?, Some(database)))
    };
    through().unwrap_or_else(|e| {
        eprintln!("library: no filters: {e:#}");
        (library, None)
    })
}

/// Merges what the sources were read as into the database; gives the snapshot after.
fn sync(database: &mut Database, read: &Read) -> Result<Library> {
    let started = Instant::now();
    let synced = database.sync_all(read, library::now())?;
    let ms = started.elapsed().as_millis();
    eprintln!("library: {} added, {} updated, {} gone, in {ms} ms", synced.added, synced.updated, synced.gone);
    database.snapshot()
}

impl Store {
    /// Reads the sources into the database on a thread of its own, if that is still to do. `shown` is the Library
    /// on screen; `wake` is called if the database ends up saying something else.
    pub fn sync_in_background(&mut self, shown: &Library, wake: impl FnOnce() + Send + 'static) {
        if !std::mem::take(&mut self.pending) {
            return;
        }
        let (path, shown, sources) = (self.path.clone(), shown.clone(), self.sources.clone());
        let (changed, synced) = mpsc::channel();
        self.synced = Some(synced);
        std::thread::spawn(move || {
            let read = library::sources::read(&sources);
            if read.found.is_empty() {
                return;
            }
            match Database::open(&path).and_then(|mut database| sync(&mut database, &read)) {
                Ok(library) if library != shown => {
                    let _ = changed.send(());
                    wake();
                }
                Ok(_) => {}
                Err(e) => eprintln!("library: {}: {e:#}", path.display()),
            }
        });
    }

    /// Every source, in order: what a game is started and asked of its store through.
    pub fn sources(&self) -> &[Arc<dyn Source>] {
        &self.sources
    }

    /// The new snapshot, once, if the background sync changed the database.
    pub fn changed(&mut self) -> Option<Library> {
        self.synced.as_ref()?.try_recv().ok()?;
        self.synced = None;
        self.snapshot()
    }

    /// The running game's window is up: its Play session starts. Asked every time the launch is looked at; only
    /// the first does anything.
    pub fn game_booted(&mut self, game: Option<&Game>) {
        if std::mem::replace(&mut self.booted, true) {
            return;
        }
        // A game with no id is in no database: a made-up one, or one the database could not take.
        let (Some(database), Some(game)) = (&mut self.database, game.filter(|game| game.id != 0)) else { return };
        match database.session_started(game.id, library::now()) {
            Ok(session) => {
                self.session = Some(session);
                eprintln!("library: {}'s Play session started", game.name);
            }
            Err(e) => eprintln!("library: {e:#}"),
        }
    }

    /// The game ended: its Play session does, and the snapshot that has it as the last game played is given.
    pub fn game_ended(&mut self) -> Option<Library> {
        self.booted = false;
        let session = self.session.take()?;
        match self.database.as_mut()?.session_ended(session, library::now()) {
            Ok(()) => eprintln!("library: the Play session ended"),
            Err(e) => eprintln!("library: {e:#}"),
        }
        self.snapshot()
    }

    /// Marks a game as a favourite, or as one no longer: the user's own mark, which no sync touches. Gives the
    /// snapshot after, in which the Favourites filter has the game or has lost it; None when nothing was changed.
    pub fn set_favourite(&mut self, game: i64, on: bool) -> Option<Library> {
        self.mark(game, if on { "a favourite" } else { "no longer a favourite" }, |db| db.set_favourite(game, on))
    }

    /// Hides a game from every list but the Hidden filter's, or shows it again. Gives the snapshot after.
    pub fn set_hidden(&mut self, game: i64, on: bool) -> Option<Library> {
        self.mark(game, if on { "hidden" } else { "shown again" }, |db| db.set_hidden(game, on))
    }

    fn mark(&mut self, game: i64, what: &str, set: impl FnOnce(&Database) -> Result<()>) -> Option<Library> {
        // A game with no id is in no database: there is nowhere to keep the mark.
        let database = self.database.as_ref().filter(|_| game != 0)?;
        let name = database.game(game).ok().flatten().map(|game| game.name)?;
        match set(database) {
            Ok(()) => eprintln!("library: {name} is {what}"),
            Err(e) => {
                eprintln!("library: {e:#}");
                return None;
            }
        }
        self.snapshot()
    }

    /// Carries out a change Settings' Groups page asked for: a filter shown or not, moved, made, changed or
    /// deleted. Gives the snapshot after, in which Home's Groups and Full library's tabs follow it; None when
    /// nothing was changed (no database, or one that refused it, which is logged).
    pub fn change_filter(&mut self, change: &FilterChange) -> Option<Library> {
        let database = self.database.as_mut()?;
        match database.change_filter(change) {
            Ok(made) => eprintln!("library: {}", describe(change, made)),
            Err(e) => {
                eprintln!("library: {e:#}");
                return None;
            }
        }
        self.snapshot()
    }

    /// A setting as the database has it; none with no database, or when it has never been set.
    pub fn setting(&self, key: &str) -> Option<String> {
        self.database.as_ref()?.setting(key).map_err(|e| eprintln!("library: {e:#}")).ok()?
    }

    /// Settings' options as the database has them: each its default until it has been set. Those that are set
    /// are said in the log, which is how a test knows a change outlived the Dashboard that made it.
    pub fn options(&self) -> Options {
        for setting in Setting::ALL {
            if let Some(value) = self.setting(setting.key()) {
                eprintln!("settings: {} is {value}", setting.key());
            }
        }
        Options::read(|key| self.setting(key))
    }

    /// Saves an option Settings changed. With no database it lasts as long as the Dashboard does.
    pub fn set(&self, change: Change) {
        let Some(database) = &self.database else { return };
        match database.set_setting(change.key(), change.value()) {
            Ok(()) => eprintln!("settings: {} = {}", change.key(), change.value()),
            Err(e) => eprintln!("settings: {} not saved: {e:#}", change.key()),
        }
    }

    fn snapshot(&self) -> Option<Library> {
        self.database.as_ref()?.snapshot().map_err(|e| eprintln!("library: {e:#}")).ok()
    }
}

/// What the log says of a change to the filters: `filter 3 is shown`, `filter 7 made: "New Group"`.
fn describe(change: &FilterChange, made: Option<i64>) -> String {
    match change {
        FilterChange::Shown { filter, shown } => {
            format!("filter {filter} is {}", if *shown { "shown" } else { "not shown" })
        }
        FilterChange::Move { filter, by } => format!("filter {filter} moved by {by}"),
        FilterChange::Add { name } => format!("filter {} made: {name:?}", made.unwrap_or_default()),
        FilterChange::Set { filter, name, .. } => format!("filter {filter} set: {name:?}"),
        FilterChange::Delete { filter } => format!("filter {filter} deleted"),
    }
}

#[cfg(test)]
mod tests {
    use super::{open, with_filters};
    use crate::Args;

    #[test]
    fn a_library_from_no_database_is_given_the_built_in_filters() {
        let fake = screens::fake_library();
        let (library, database) = with_filters(fake.clone());
        assert!(database.is_some());
        let names = |games: &[model::Game]| games.iter().map(|g| g.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&library.games), names(&fake.games));
        assert_eq!(library.user, fake.user);
        let groups: Vec<&str> = library.groups().map(|f| f.name.as_str()).collect();
        assert_eq!(groups, ["Most played", "Recently added"]);
        let most_played = library.groups().next().unwrap();
        assert_eq!(library.game(most_played.games[0]).unwrap().name, "Stardew Valley");
        // One that has its filters keeps them, and its games their ids.
        let (again, database) = with_filters(library.clone());
        assert!(again == library && database.is_none());
    }

    /// The fake library, as the Dashboard has it with no Steam and no database of its own.
    fn fake() -> (model::Library, super::Store) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("library.json");
        std::fs::write(&file, serde_json::to_string(&screens::fake_library()).unwrap()).unwrap();
        open(&Args { library: Some(file), ..Args::default() })
    }

    #[test]
    fn a_favourite_is_in_the_next_snapshot_and_in_the_favourites_filter() {
        let (library, mut store) = fake();
        let game = library.games[2].clone();
        let favourites = |library: &model::Library| {
            let filter = library.filters.iter().find(|f| f.key.as_deref() == Some("favourites")).unwrap();
            (filter.shown, filter.games.clone())
        };
        assert_eq!(favourites(&library), (true, vec![]));
        let after = store.set_favourite(game.id, true).expect("a new snapshot");
        assert!(after.game(game.id).unwrap().favourite);
        assert_eq!(favourites(&after), (true, vec![game.id]));
        let after = store.set_favourite(game.id, false).expect("a new snapshot");
        assert_eq!(favourites(&after), (true, vec![]));
    }

    #[test]
    fn a_hidden_game_leaves_the_games_for_the_hidden_ones_and_comes_back() {
        let (library, mut store) = fake();
        let game = library.games[0].clone();
        let after = store.set_hidden(game.id, true).expect("a new snapshot");
        assert!(after.games.iter().all(|g| g.id != game.id));
        assert_eq!(after.hidden.iter().map(|g| g.id).collect::<Vec<_>>(), [game.id]);
        let after = store.set_hidden(game.id, false).expect("a new snapshot");
        assert!(after.hidden.is_empty() && after.games.iter().any(|g| g.id == game.id));
    }

    #[test]
    fn a_change_to_the_filters_is_in_the_next_snapshot() {
        let (library, mut store) = fake();
        let groups = |library: &model::Library| library.groups().map(|f| f.name.clone()).collect::<Vec<_>>();
        assert_eq!(groups(&library), ["Most played", "Recently added"]);
        let recent = library.filters.iter().find(|f| f.name == "Recently added").unwrap().id;
        let after = store.change_filter(&model::FilterChange::Move { filter: recent, by: -9 }).expect("a snapshot");
        assert_eq!(groups(&after), ["Recently added", "Most played"]);
        let after = store.change_filter(&model::FilterChange::Shown { filter: recent, shown: false }).unwrap();
        assert_eq!(groups(&after), ["Most played"]);
        let after = store.change_filter(&model::FilterChange::Add { name: "Mine".into() }).unwrap();
        assert_eq!(groups(&after), ["Most played", "Mine"]);
        // What the database refuses changes nothing, and gives no snapshot.
        assert!(store.change_filter(&model::FilterChange::Delete { filter: recent }).is_none());
    }

    #[test]
    fn a_game_that_is_in_no_database_takes_no_mark() {
        let (_, mut store) = fake();
        assert!(store.set_favourite(0, true).is_none());
        assert!(store.set_hidden(987_654, true).is_none());
        assert!(super::Store::default().set_favourite(1, true).is_none());
    }
}
