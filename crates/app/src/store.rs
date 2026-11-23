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
use library::art::{self, ArtDir, Event, Hosts, Lookup, Pace, Service, Services, StoreArt};
use library::sources::Read;
use library::Database;
use model::{ByHand, FilterChange, Game, Library, ProfileChange, Source, StoreApp};
use screens::settings::{Change, Options, Setting};

use crate::{fail, Args};

/// The folder in the data folder whose pictures a Profile may wear, as the Bundle's Gamerpics folder.
const GAMERPICS: &str = "Gamerpics";

#[derive(Default)]
pub struct Store {
    database: Option<Database>,
    /// The database's file, which the thread that syncs opens for itself.
    path: PathBuf,
    /// Every source, in order.
    sources: Vec<Arc<dyn Source>>,
    /// The sources are still to be read, the Library on screen having come from the database.
    pending: bool,
    /// Hears from the thread that syncs when it is done, and whether it changed the database.
    synced: Option<Receiver<bool>>,
    /// SteamGridDB's address: its own, or a test's (`--art-service`).
    art_service: String,
    /// The stores' own art online at a test's stand-in (`--store-art-service`), in place of their own.
    store_art_service: Option<String>,
    /// A game is running, and `game_booted` has been told so.
    booted: bool,
    /// The running game's Play session.
    session: Option<i64>,
    /// The thread that follows the Queue (`library::queue`), once the window's first frame is out. None with no
    /// database file for it to open.
    queue: Option<library::queue::Handle>,
    /// The Microsoft Store apps installed, once listed: every snapshot carries them (`Library::store_apps`).
    store_apps: Option<Vec<StoreApp>>,
    /// Hears from the thread that lists them.
    listed: Option<Receiver<Option<Vec<StoreApp>>>>,
    /// The Windows user's account picture, as it was when the Dashboard started: the first a Profile may wear.
    account_picture: Option<PathBuf>,
    /// Every store's achievements (`crate::sources::achievements`).
    achievement_sources: Vec<Arc<dyn model::AchievementSource>>,
    /// The thread that follows achievements (`library::achievements::spawn`), once the sources have been read. None
    /// with no database file for it to open.
    achievements: Option<library::achievements::Handle>,
    /// Settings > Launchers' update: how it goes, as every snapshot carries it (`Library::launchers`).
    update: model::launcher::Update,
    /// Hears from the thread that reads the launchers again, while it does.
    updating: Option<Receiver<Updated>>,
    /// Launchers switched on while an update was under way: read again once it is done.
    update_again: Vec<String>,
    /// Settings > Art's refresh of all art: how it goes, as every snapshot carries it (`Library::art_refresh`).
    art_refresh: model::ArtRefresh,
    /// The drives and each store's install folders, as last read: every snapshot carries them (`Library::storage`).
    storage: model::Storage,
    /// Hears from the thread that reads them, while it does.
    looking: Option<Receiver<model::Storage>>,
    /// Asked to read them again while a read was under way: once more when it is done.
    look_again: bool,
    /// Settings > Helpers: how each stands, as every snapshot carries them (`Library::helpers`).
    helpers: Vec<model::helper::Helper>,
    /// Settings > Set up this PC: how each launcher's app stands, as every snapshot carries them
    /// (`Library::clients`).
    clients: Vec<model::setup::Client>,
}

/// What an update of the launchers came to: what it changed, in a few words; none when it could not be done.
type Updated = Option<String>;

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
    let (library, mut store) = open_library(args);
    store.achievement_sources = crate::sources::achievements(args);
    (library, store)
}

fn open_library(args: &Args) -> (Library, Store) {
    let sources = crate::sources::all(args);
    let art_service = args.art_service.clone().unwrap_or_else(|| art::SERVICE.to_string());
    let store_art_service = args.store_art_service.clone();
    if let Some(path) = &args.library {
        let json = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        let library: Library =
            serde_json::from_str(&json).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        // With a data folder, the file's games go into its database as their sources' are: its settings and its
        // art cache are then theirs.
        if let Some(dir) = &args.data_dir {
            let path = dir.join(library::FILE);
            let into = |library: &Library| -> Result<(Library, Database)> {
                let mut database = Database::open(&path)?;
                first_profile(&mut database);
                game_pass_as_set(&database);
                let library = through(&mut database, library)?;
                Ok((library, database))
            };
            match into(&library) {
                Ok((library, database)) => {
                    let store = Store { database: Some(database), path, sources, art_service, ..Store::default() };
                    return dressed(library, Store { store_art_service, ..store });
                }
                Err(e) => eprintln!("library: {}: {e:#}", path.display()),
            }
        }
        let (library, database) = with_filters(library);
        return dressed(library, Store { database, sources, art_service, store_art_service, ..Store::default() });
    }
    let window = args.screenshot.is_none();
    let dir = args.data_dir.clone().or_else(|| if window { library::data_dir() } else { None });
    let path = dir.map(|dir| dir.join(library::FILE));
    let started = Instant::now();
    // Without a database the Dashboard still runs, on what the sources say now.
    let mut database = path
        .as_ref()
        .and_then(|path| Database::open(path).map_err(|e| eprintln!("library: {}: {e:#}", path.display())).ok());
    if let Some(database) = &mut database {
        first_profile(database);
        game_pass_as_set(database);
    }
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
            // A launcher switched off is not read (docs/spec/launchers.md).
            let off = database.as_ref().and_then(|db| db.launchers_off().ok()).unwrap_or_default();
            let read = library::sources::read(&library::launchers::switched_on(&sources, &off));
            match (read.found.is_empty(), stored, read.failed.first()) {
                (true, Some(library), _) => library,
                (true, None, Some((_, e))) => fail(&format!("{e:#}")),
                (true, None, None) => screens::fake_library(),
                (false, _, _) => {
                    // With no file to keep it in, the library is kept in memory for as long as the Dashboard runs.
                    if database.is_none() {
                        database = Database::in_memory().map_err(|e| eprintln!("library: {e:#}")).ok();
                        database.iter_mut().for_each(first_profile);
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
    let path = path.unwrap_or_default();
    dressed(library, Store { database, path, sources, pending, art_service, store_art_service, ..Store::default() })
}

/// The Store, with the Windows user's account picture read, and the Library as every snapshot it gives is: with
/// the pictures a Profile may wear. The Profiles are said in the log.
fn dressed(library: Library, mut store: Store) -> (Library, Store) {
    store.account_picture = shell::account::picture();
    let mut library = library;
    library.profiles.pictures = store.pictures();
    // A refresh of all art cut short at an earlier start: it goes on with the first lookup.
    if store.art_refreshing() {
        eprintln!("art: refresh: under way since an earlier start");
        store.art_refresh = model::ArtRefresh::Paused;
        library.art_refresh = store.art_refresh;
    }
    let profiles = &library.profiles;
    if let Some(signed_in) = profiles.signed_in() {
        eprintln!(
            "profiles: {} of them, {} {:?} signed in last",
            profiles.list.len(),
            signed_in.id,
            signed_in.gamertag
        );
    }
    (library, store)
}

/// With no Profile in the database yet, the first, made from the Windows user: their name, and their account
/// picture if Windows has one.
/// The Game Pass catalogue as the database's settings have it, before the sources are first read (#277).
fn game_pass_as_set(database: &Database) {
    let Some(game_pass) = crate::game_pass::get() else { return };
    let setting = |key: &str| database.setting(key).ok().flatten();
    let options = Options::read(setting);
    game_pass.set(options.show_game_pass, options.game_pass_region);
}

fn first_profile(database: &mut Database) {
    match database.first_profile(&shell::account::name(), shell::account::picture().as_deref()) {
        Ok(Some(made)) => eprintln!("profiles: {made}"),
        Ok(None) => {}
        Err(e) => eprintln!("profiles: {e:#}"),
    }
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
    let in_memory = || -> Result<(Library, Option<Database>)> {
        let mut database = Database::in_memory()?;
        first_profile(&mut database);
        let library = through(&mut database, &library)?;
        Ok((library, Some(database)))
    };
    in_memory().unwrap_or_else(|e| {
        eprintln!("library: no filters: {e:#}");
        (library, None)
    })
}

/// Takes a Library's games into a database, each as its source's (a made-up game, of none, as Steam's), and gives
/// its snapshot: what `with_filters` does in memory.
fn through(database: &mut Database, library: &Library) -> Result<Library> {
    {
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
        database.snapshot()
    }
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
        // What the database says is compared with what is shown, which has the Store apps and the pictures a
        // Profile may wear from elsewhere.
        let mut shown = Library { store_apps: None, ..shown.clone() };
        shown.profiles.pictures.clear();
        shown.launchers.update = model::launcher::Update::Idle;
        let (path, sources) = (self.path.clone(), self.sources.clone());
        let (changed, synced) = mpsc::channel();
        self.synced = Some(synced);
        std::thread::spawn(move || {
            // The launchers switched on, and what each was found to be kept for Settings > Launchers.
            let read_and_sync = |database: &mut Database| -> Result<bool> {
                let off = database.launchers_off()?;
                let read = library::sources::read(&library::launchers::switched_on(&sources, &off));
                let library = if read.found.is_empty() {
                    database.sync_all(&read, library::now())?;
                    database.snapshot()?
                } else {
                    sync(database, &read)?
                };
                // When each launcher was read is no news: this read is a moment after the one shown.
                Ok(unstamped(library) != unstamped(shown.clone()))
            };
            let news = match Database::open(&path).and_then(|mut database| read_and_sync(&mut database)) {
                Ok(news) => news,
                Err(e) => {
                    eprintln!("library: {}: {e:#}", path.display());
                    false
                }
            };
            // Done, with news or none: the art lookup waits for this.
            let _ = changed.send(news);
            wake();
        });
    }

    /// The sources have been read, or are not to be: what the art lookup waits for, so that it looks up the games
    /// the stores have now.
    pub fn sources_read(&self) -> bool {
        !self.pending && self.synced.is_none()
    }

    /// Every source, in order: what a game is started and asked of its store through.
    pub fn sources(&self) -> &[Arc<dyn Source>] {
        &self.sources
    }

    /// Starts following the Queue on a thread of its own (`library::queue::spawn`), if that is still to do and there
    /// is a database file: a Library from a JSON file, or one kept in memory, has no Queue. `wake` is called when
    /// it has news (`queue_update`).
    pub fn follow_queue(&mut self, wake: impl Fn() + Send + 'static) {
        if self.queue.is_some() || self.database.is_none() || self.path.as_os_str().is_empty() {
            return;
        }
        self.queue = Some(library::queue::spawn(self.path.clone(), self.sources.clone(), wake));
    }

    /// The Queue as the sources have it now, from one look at what they have under way: what `--screenshot` shows,
    /// having no thread to follow it. None with no database.
    pub fn queue_now(&mut self) -> Option<model::Queue> {
        let database = self.database.as_mut()?;
        let mut follower = library::queue::Follower::new(self.sources.clone());
        let looked = follower.poll(database, library::now()).and_then(|_| follower.queue(database));
        looked.map_err(|e| eprintln!("queue: {e:#}")).ok()
    }

    /// Tells the thread that follows the Queue what was asked of a store, or that it should look now.
    pub fn tell_queue(&self, ask: library::queue::Ask) {
        if let Some(queue) = &self.queue {
            queue.ask(ask);
        }
    }

    /// What the thread that follows the Queue has said since this was last asked: the Queue, and a new snapshot of
    /// the library when a source read again changed it.
    pub fn queue_update(&mut self) -> Option<library::queue::Update> {
        let mut update = self.queue.as_ref()?.take()?;
        // A snapshot the thread took carries what every snapshot here does: the Store apps listed, the pictures.
        if let Some(library) = &mut update.library {
            self.dress(library);
        }
        Some(update)
    }

    /// Starts following achievements on a thread of their own (docs/achievements.md), if that is still to do and
    /// there is a database file: every game's read and said in the log, how rare each is asked for online, and the
    /// running game's watched for new unlocks. Icons are fetched only with Settings' Download art online on; `wake`
    /// is called when there is news (`achievements_update`).
    pub fn follow_achievements(&mut self, wake: impl Fn() + Send + 'static) {
        if self.achievements.is_some() || self.database.is_none() || self.path.as_os_str().is_empty() {
            return;
        }
        let hosts = self
            .store_art_service
            .as_deref()
            .map_or_else(library::achievements::Hosts::default, library::achievements::Hosts::all_at);
        let fetch: Box<dyn library::achievements::Fetch> = Box::new(library::achievements::Online::new(hosts));
        let icons = self.art_online();
        let log: library::achievements::Log = Arc::new(|line| eprintln!("{line}"));
        let (path, sources) = (self.path.clone(), self.achievement_sources.clone());
        self.achievements = Some(library::achievements::spawn(path, sources, Some(fetch), icons, log, wake));
    }

    /// What the thread that follows achievements has said since this was last asked: unlocks to tell of, and a new
    /// snapshot of the library when the totals changed.
    pub fn achievements_update(&mut self) -> Option<library::achievements::Update> {
        let mut update = self.achievements.as_ref()?.take()?;
        if let Some(library) = &mut update.library {
            self.dress(library);
        }
        Some(update)
    }

    /// One game's whole list of achievements, in its store's order, with the icons the cache has: what its
    /// Achievements page shows. Empty with no database, or none for it.
    pub fn achievements_of(&self, game: i64) -> Vec<model::achievement::Achievement> {
        let Some(database) = &self.database else { return Vec::new() };
        database.achievements(game).unwrap_or_else(|e| {
            eprintln!("achievements: {e:#}");
            Vec::new()
        })
    }

    /// Every store's achievements read once into the database, as the thread that follows them reads them as it
    /// starts, with no rarity and no icons: what `--screenshot` shows, having no thread. What a snapshot carries of
    /// them after, or None with no database.
    pub fn achievements_now(&mut self) -> Option<model::Achievements> {
        let database = self.database.as_mut()?;
        let log: library::achievements::Log = Arc::new(|_| {});
        let mut follower = library::achievements::Follower::new(self.achievement_sources.clone(), None, false, log);
        follower.read_all(database, library::now()).map_err(|e| eprintln!("achievements: {e:#}")).ok()?;
        database.achievement_summary().map_err(|e| eprintln!("achievements: {e:#}")).ok()
    }

    /// Asks the thread that follows achievements: a game is running, or a game's icons are wanted.
    pub fn ask_achievements(&self, ask: library::achievements::Ask) {
        if let Some(achievements) = &self.achievements {
            achievements.ask(ask);
        }
    }

    /// Lists the Microsoft Store apps installed (`crate::store_apps`) on a thread of its own, for Add an app's
    /// list; `wake` is called once they are.
    pub fn list_store_apps(&mut self, wake: impl FnOnce() + Send + 'static) {
        let (listed, hear) = mpsc::channel();
        self.listed = Some(hear);
        std::thread::spawn(move || {
            let started = Instant::now();
            let apps = crate::store_apps::list();
            if let Some(apps) = &apps {
                eprintln!("apps: {} Microsoft Store apps listed in {} ms", apps.len(), started.elapsed().as_millis());
            }
            let _ = listed.send(apps);
            wake();
        });
    }

    /// Reads the drives and each store's install folders (Settings > Storage, the storage ring) on a thread of its
    /// own, from the library as it is now; `wake` is called once they are. One read at a time: asked while one is
    /// under way, it reads again after.
    pub fn look_at_storage(&mut self, library: &Library, wake: impl FnOnce() + Send + 'static) {
        if self.looking.is_some() {
            self.look_again = true;
            return;
        }
        let (looked, hear) = mpsc::channel();
        self.looking = Some(hear);
        let sources = self.sources.clone();
        let chosen = crate::storage::chosen(&sources, |key| self.setting(key));
        let installed: Vec<Game> = library.games.iter().filter(|game| game.installed).cloned().collect();
        let twins = crate::storage::twins(library);
        std::thread::spawn(move || {
            let storage = crate::storage::read(&sources, &shell::drives::SystemDrives, &chosen, &installed, &twins);
            let _ = looked.send(storage);
            wake();
        });
    }

    /// Whether Storage is to be read again, a read having been asked for while the last was under way.
    pub fn storage_wanted(&mut self) -> bool {
        self.looking.is_none() && std::mem::take(&mut self.look_again)
    }

    /// Settings > Storage chose the folder a store's installs go to: kept in the settings, and the store's default
    /// in every snapshot from now. The snapshot after.
    pub fn set_default_folder(&mut self, store: &str, folder: &std::path::Path) -> Option<Library> {
        self.set_text(&model::storage::setting_key(store), &folder.display().to_string());
        if let Some(folders) = self.storage.stores.iter_mut().find(|folders| folders.store == store) {
            let at = folders.folders.iter().position(|place| model::storage::same_folder(&place.path, folder));
            match at {
                Some(at) => folders.default = at,
                None => eprintln!("storage: {} is not one of {}'s folders", folder.display(), folders.name),
            }
        }
        self.snapshot()
    }

    /// The new snapshot, once, if the background sync changed the database, the Store apps have been listed, or the
    /// drives and folders read.
    pub fn changed(&mut self) -> Option<Library> {
        let mut changed = false;
        if let Some(storage) = self.looking.as_ref().and_then(|looking| looking.try_recv().ok()) {
            self.looking = None;
            if storage != self.storage {
                (self.storage, changed) = (storage, true);
            }
        }
        if let Some(apps) = self.listed.as_ref().and_then(|listed| listed.try_recv().ok()) {
            (self.store_apps, self.listed, changed) = (apps, None, true);
        }
        // The sync is done, with news or none: only news is a new snapshot (the art lookup waits for either).
        if let Some(news) = self.synced.as_ref().and_then(|synced| synced.try_recv().ok()) {
            self.synced = None;
            changed |= news;
        }
        changed.then(|| self.snapshot()).flatten()
    }

    /// Carries out a change Settings' Games & apps pages asked for. A program that is not there is refused, with
    /// why, for the user to be told; a picture that is not there is left out. Gives the snapshot after, or None
    /// with no database.
    pub fn by_hand(&mut self, change: ByHand) -> Result<Option<Library>, String> {
        let change = match change {
            ByHand::AddGame { path, .. } | ByHand::AddApp { path, .. } if !path.is_file() => {
                let why = format!("There's no program at {}", path.display());
                eprintln!("library: {why}: nothing added");
                return Err(why);
            }
            ByHand::AddGame { path, name, art } => ByHand::AddGame { path, name, art: there(art) },
            ByHand::AddApp { path, name, icon } => ByHand::AddApp { path, name, icon: there(icon) },
            change => change,
        };
        let Some(database) = self.database.as_mut() else { return Ok(None) };
        match database.by_hand(&change, library::now()) {
            Ok(done) => eprintln!("library: {done}"),
            Err(e) => {
                eprintln!("library: {e:#}");
                return Err(format!("{e:#}"));
            }
        }
        Ok(self.snapshot())
    }

    /// The running game's window is up: its Play session starts. Asked every time the launch is looked at; only
    /// the first does anything.
    pub fn game_booted(&mut self, game: Option<&Game>) {
        if std::mem::replace(&mut self.booted, true) {
            return;
        }
        // A game with no id is in no database: a made-up one, or one the database could not take.
        let Some(game) = game.filter(|game| game.id != 0) else { return };
        // Its achievements are watched from now on: a new unlock is known within a second or two.
        self.ask_achievements(library::achievements::Ask::Playing { game: game.id, since: library::now() });
        let Some(database) = &mut self.database else { return };
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
        let game_pass = crate::game_pass::get();
        let windows_region = game_pass.as_ref().and_then(|game_pass| game_pass.windows_region());
        let game_pass_subscription = game_pass.map(|game_pass| game_pass.subscription()).unwrap_or_default();
        Options { windows_region, game_pass_subscription, ..self.options_kept() }
    }

    /// The Game Pass catalogue's region, None for Windows' (Settings > Launchers): saved, and in force for the next
    /// read of Xbox.
    pub fn set_game_pass(&self, shown: bool, region: Option<model::game_pass::Region>) {
        if let Some(database) = &self.database {
            let key = model::game_pass::REGION_KEY;
            let saved = match region {
                Some(region) => database.set_setting(key, region.code()),
                None => database.delete_setting(key),
            };
            match saved {
                Ok(()) => eprintln!("settings: {key} = {}", region.map_or("Windows'".to_string(), |r| r.to_string())),
                Err(e) => eprintln!("settings: {key} not saved: {e:#}"),
            }
        }
        if let Some(game_pass) = crate::game_pass::get() {
            game_pass.set(shown, region);
        }
    }

    fn options_kept(&self) -> Options {
        let keys = Setting::ALL.iter().map(|setting| setting.key());
        for key in keys.chain([screens::idle::DIM_KEY, screens::idle::SHOW_KEY]) {
            if let Some(value) = self.setting(key) {
                eprintln!("settings: {key} is {value}");
            }
        }
        if let Some(value) = self.setting(screens::settings::BOOT_VIDEO) {
            eprintln!("settings: {} is {value}", screens::settings::BOOT_VIDEO);
        }
        Options { build: crate::build(), ..Options::read(|key| self.setting(key)) }
    }

    /// Saves the Boot video Settings chose (docs/spec/start-up.md): it plays from the next start.
    pub fn set_boot_video(&self, video: screens::BootVideo) {
        let Some(database) = &self.database else { return };
        let key = screens::settings::BOOT_VIDEO;
        match database.set_setting(key, video.key()) {
            Ok(()) => eprintln!("settings: {key} = {}", video.key()),
            Err(e) => eprintln!("settings: {key} not saved: {e:#}"),
        }
    }

    /// Saves an option Settings changed. With no database it lasts as long as the Dashboard does.
    pub fn set(&self, change: Change) {
        let Some(database) = &self.database else { return };
        match database.set_setting(change.key(), change.value()) {
            Ok(()) => eprintln!("settings: {} = {}", change.key(), change.value()),
            Err(e) => eprintln!("settings: {} not saved: {e:#}", change.key()),
        }
    }

    /// Saves a setting kept as text, as Settings > Guide's Now Playing source. With no database it lasts as long as
    /// the Dashboard does.
    pub fn set_text(&self, key: &str, value: &str) {
        let Some(database) = &self.database else { return };
        match database.set_setting(key, value) {
            Ok(()) => eprintln!("settings: {key} = {value}"),
            Err(e) => eprintln!("settings: {key} not saved: {e:#}"),
        }
    }

    /// Settings' Download art online: on unless turned off.
    fn art_online(&self) -> bool {
        self.setting(art::ONLINE_SETTING).is_none_or(|value| !matches!(value.trim(), "false" | "0" | "off" | "no"))
    }

    /// Our art cache, where there is a database in a data folder to keep it beside.
    fn art_dir(&self) -> Option<ArtDir> {
        self.database.as_ref()?.art_dir().cloned()
    }

    /// The user's SteamGridDB key, if one is kept and the service has not refused it.
    fn art_key(&self) -> Option<String> {
        if self.setting(art::REFUSED_SETTING).is_some() {
            return None;
        }
        self.setting(art::KEY_SETTING).filter(|key| !key.trim().is_empty())
    }

    /// Where art is asked for online, and the cache it goes into: the stores' own art, unless Settings' Download
    /// art online is off; SteamGridDB too with the user's key. None with neither, or no art cache to download into.
    fn art_services(&self) -> Option<(ArtDir, Services)> {
        if !self.art_online() {
            return None;
        }
        let hosts = self.store_art_service.as_deref().map_or_else(Hosts::default, Hosts::all_at);
        let order = model::ArtOrder::read(self.setting(model::art_order::KEY).as_deref()).on();
        let services = Services {
            stores: Some(StoreArt::new(hosts)),
            steamgriddb: self.art_key().map(|key| Service::new(&self.art_service, &key)),
            order,
        };
        Some((self.art_dir()?, services))
    }

    /// Looks up, on threads of their own, the art the stores' files on the PC gave none for: of the games of
    /// `library` that lack a picture, those our cache has not got, from the store's own art online, then from
    /// SteamGridDB with a key (docs/sources.md, "Art"). `tell` is told as pictures come in. None when there is
    /// nothing to do it with.
    /// `first`, if it is among them, is looked up before the rest (the Tile menu's Refresh art).
    pub fn look_up_art(
        &self,
        library: &Library,
        first: Option<&model::GameKey>,
        tell: impl Fn(Event) + Send + Sync + 'static,
    ) -> Option<Lookup> {
        let (dir, services) = self.art_services()?;
        let log: art::Log = std::sync::Arc::new(|line| eprintln!("{line}"));
        let mut wants = art::wanted(library);
        if let Some(first) = first {
            let is = |want: &art::Want| want.source == first.source && want.source_id == first.source_id;
            if let Some(at) = wants.iter().position(is) {
                let want = wants.remove(at);
                wants.insert(0, want);
            }
        }
        Some(Lookup::start(dir, services, wants, library::now(), Pace::default(), log, tell))
    }

    /// Settings > Art's Refresh all art and details (docs/spec/settings.md, 16.8): everything fetched from outside
    /// forgotten (`Database::forget_fetched`), and marked under way until a lookup has been through every game, at
    /// this start or a later one. Gives the snapshot after: no downloaded art in it.
    pub fn refresh_art(&mut self) -> Option<Library> {
        let database = self.database.as_ref()?;
        match database.forget_fetched() {
            Ok(forgot) => eprintln!("art: refresh: {forgot}"),
            Err(e) => eprintln!("art: refresh: {e:#}"),
        }
        let games = database.snapshot().map_or(0, |library| library.games.len());
        self.art_refresh = model::ArtRefresh::Working { done: 0, of: games };
        self.snapshot()
    }

    /// The Tile menu's Refresh art: the game's downloaded pictures forgotten, to be looked up again. The log names
    /// no game. Gives the snapshot after.
    pub fn refresh_art_of(&self, game: &model::GameKey) -> Option<Library> {
        let database = self.database.as_ref()?;
        match database.forget_art_of(&game.source, &game.source_id) {
            Ok(pictures) => eprintln!("art: refresh: one game's art: forgot {pictures} pictures"),
            Err(e) => eprintln!("art: refresh: {e:#}"),
        }
        self.snapshot()
    }

    /// Whether a refresh of all art was asked for and is not over: this start's or an earlier one's.
    pub fn art_refreshing(&self) -> bool {
        self.database.as_ref().is_some_and(|database| database.refreshing().unwrap_or(false))
    }

    pub fn set_helpers(&mut self, helpers: Vec<model::helper::Helper>) {
        self.helpers = helpers;
    }

    pub fn set_clients(&mut self, clients: Vec<model::setup::Client>) {
        self.clients = clients;
    }

    pub fn set_art_refresh(&mut self, refresh: model::ArtRefresh) {
        self.art_refresh = refresh;
    }

    /// A lookup that went on with a refresh is over: the refresh is done if it looked at every game, else it waits
    /// for the next (art online on again, the next start). Gives how it stands.
    pub fn art_refresh_over(&mut self, summary: &art::Summary) -> model::ArtRefresh {
        if summary.stopped {
            eprintln!("art: refresh: paused");
            return model::ArtRefresh::Paused;
        }
        if let Some(Err(e)) = self.database.as_ref().map(Database::refreshed) {
            eprintln!("art: refresh: {e:#}");
        }
        let pictures = summary.pictures.store_online + summary.pictures.steamgriddb;
        eprintln!("art: refresh: done, {pictures} pictures downloaded");
        model::ArtRefresh::Done { pictures }
    }

    /// The same here and now, for a picture drawn offscreen: gives the snapshot after, or `library` as it was.
    pub fn art_now(&self, library: Library) -> Library {
        let Some((dir, services)) = self.art_services() else { return library };
        let log: art::Log = std::sync::Arc::new(|line| eprintln!("{line}"));
        let summary = Lookup::run_now(dir, services, art::wanted(&library), library::now(), Pace::default(), log);
        if summary.found == 0 {
            return library;
        }
        self.snapshot().unwrap_or(library)
    }

    /// Keeps the user's SteamGridDB key, or forgets it; that the service refused the last one is forgotten with it.
    /// The log says it was set, never what it is.
    pub fn set_art_key(&self, key: Option<&str>) {
        let Some(database) = &self.database else { return };
        let saved = match key {
            Some(key) => database.set_setting(art::KEY_SETTING, key.trim()),
            None => database.delete_setting(art::KEY_SETTING),
        };
        let saved = saved.and_then(|()| database.delete_setting(art::REFUSED_SETTING));
        match (saved, key) {
            (Ok(()), Some(_)) => eprintln!("settings: {} set", art::KEY_SETTING),
            (Ok(()), None) => eprintln!("settings: {} forgotten", art::KEY_SETTING),
            (Err(e), _) => eprintln!("settings: {} not saved: {e:#}", art::KEY_SETTING),
        }
    }

    /// The service refused the key: no lookups until another is entered, this run or the next.
    pub fn art_key_refused(&self) {
        if let Some(database) = &self.database {
            if let Err(e) = database.set_setting(art::REFUSED_SETTING, "true") {
                eprintln!("settings: {e:#}");
            }
        }
    }

    /// The art picker's network (docs/spec/art-picker.md): SteamGridDB with the user's key while art online is on
    /// (none without: the picker then asks for nothing), its thumbnails in the data folder. None with no data folder.
    /// Match game's searches go through it too: the stores' while art online is on, SteamGridDB's with the key.
    pub fn art_picker(&self, tell: impl Fn(model::art_picker::Answer) + Send + Sync + 'static) -> Option<art::Picker> {
        let thumbs = self.art_dir()?.root().parent()?.join(art::picker::THUMBS);
        let service =
            self.art_online().then(|| self.art_key()).flatten().map(|key| Service::new(&self.art_service, &key));
        let hosts = self.store_art_service.as_deref().map_or_else(Hosts::default, Hosts::all_at);
        let stores = self.art_online().then(|| StoreArt::new(hosts));
        let log: art::Log = Arc::new(|line| eprintln!("{line}"));
        Some(art::Picker::start(service, stores, &thumbs, Pace::default(), log, tell))
    }

    /// Keeps Match game's choice (docs/spec/match-game.md): the game is that one of the source's games, or found by
    /// its name again (`None`). Its downloaded art is forgotten with it, to be looked up again by the match. The
    /// log names the source and the match's id, never a game. Gives the snapshot after.
    pub fn match_game(
        &self,
        game: &model::GameKey,
        source: model::ArtSource,
        matched: Option<&model::art_match::Match>,
    ) -> Option<Library> {
        let database = self.database.as_ref()?;
        match database.match_game(&game.source, &game.source_id, source, matched) {
            Ok(()) => match matched {
                Some(m) => eprintln!("art: match: {} is {}", source.name(), m.id),
                None => eprintln!("art: match: {} is Automatic", source.name()),
            },
            Err(e) => eprintln!("art: match: {} not kept: {e:#}", source.name()),
        }
        self.refresh_art_of(game)
    }

    /// Keeps the art picker's choice as the user's own (`picture`: a SteamGridDB picture's bytes, downloaded). The
    /// log names no game. Gives the snapshot after.
    pub fn choose_art(&self, choice: &model::art_picker::Choice, picture: Option<&[u8]>) -> Option<Library> {
        use model::art_picker::Chosen;
        let database = self.database.as_ref()?;
        let (game, tab) = (&choice.game, choice.tab);
        let what = match &choice.chosen {
            None => "Automatic".to_string(),
            Some(Chosen::Store) => "the store's own".to_string(),
            Some(Chosen::Picture(id)) => format!("SteamGridDB picture {id}"),
        };
        match database.choose_art(&game.source, &game.source_id, tab, choice.chosen.as_ref(), picture) {
            Ok(()) => eprintln!("art: picker: {} is {what}", tab.key()),
            Err(e) => eprintln!("art: picker: {} not kept: {e:#}", tab.key()),
        }
        self.snapshot()
    }

    /// Removes the art downloaded into the data folder, and nothing of a store's: gives the snapshot after.
    pub fn clear_art(&self) -> Option<Library> {
        let dir = self.art_dir()?;
        match dir.clear() {
            Ok(pictures) => eprintln!("art: cleared {pictures} pictures"),
            Err(e) => eprintln!("art: {e:#}"),
        }
        self.snapshot()
    }

    /// The library database's snapshot as it is now: what the screens show after art has come in.
    pub fn snapshot(&self) -> Option<Library> {
        let mut library = self.database.as_ref()?.snapshot().map_err(|e| eprintln!("library: {e:#}")).ok()?;
        self.dress(&mut library);
        Some(library)
    }

    /// What every snapshot carries that is not the database's: the Store apps listed, the pictures a Profile may
    /// wear, and how Settings > Launchers' update goes.
    fn dress(&self, library: &mut Library) {
        library.store_apps = self.store_apps.clone();
        library.profiles.pictures = self.pictures();
        library.launchers.update = self.update.clone();
        library.art_refresh = self.art_refresh;
        library.storage = self.storage.clone();
        library.helpers = self.helpers.clone();
        library.clients = self.clients.clone();
    }

    /// Switches a launcher on or off (Settings > Launchers): its games leave every list at once, or come back as
    /// they were. A launcher a game is playing from (`playing`) stays on. Gives the snapshot after, or None when
    /// nothing was changed.
    pub fn set_launcher(&mut self, key: &str, on: bool, playing: Option<&Game>) -> Option<Library> {
        if let Some(game) = playing.filter(|game| !on && game.source == key) {
            eprintln!("launchers: {key} stays on: {} is playing", game.name);
            return None;
        }
        let database = self.database.as_ref()?;
        match database.set_launcher(key, on) {
            Ok(games) if on => eprintln!("launchers: {key} on: {games} games back"),
            Ok(games) => eprintln!("launchers: {key} off: {games} games hidden"),
            Err(e) => {
                eprintln!("launchers: {key}: {e:#}");
                return None;
            }
        }
        self.snapshot()
    }

    /// Reads every launcher switched on again (`only`: these alone) on a thread of its own, and merges what each
    /// says into the database: Settings > Launchers' Update now, or a launcher just switched on. Whether one was
    /// started: none while one is under way (a launcher switched on meanwhile is read once it is done), or with no
    /// database file. `wake` is called once it is done (`launchers_updated`).
    pub fn update_launchers(&mut self, only: Option<Vec<String>>, wake: impl FnOnce() + Send + 'static) -> bool {
        if self.updating.is_some() {
            match only {
                Some(keys) => self.update_again.extend(keys),
                None => eprintln!("launchers: update: already under way"),
            }
            return false;
        }
        if self.database.is_none() || self.path.as_os_str().is_empty() {
            eprintln!("launchers: update: no library database to update");
            return false;
        }
        let (path, sources) = (self.path.clone(), self.sources.clone());
        let what = only.as_ref().map_or_else(|| "every launcher switched on".to_string(), |keys| keys.join(", "));
        eprintln!("launchers: update: reading {what}");
        let (done, updating) = mpsc::channel();
        self.updating = Some(updating);
        self.update = model::launcher::Update::Working;
        std::thread::spawn(move || {
            let _ = done.send(update(&path, &sources, only.as_deref()));
            wake();
        });
        true
    }

    /// The snapshot after an update of the launchers, once, when it is done (one that could not be done says so in
    /// it). Launchers switched on meanwhile are `take_update_again`'s.
    pub fn launchers_updated(&mut self) -> Option<Library> {
        let updated = self.updating.as_ref()?.try_recv().ok()?;
        self.updating = None;
        self.update = match updated {
            Some(changes) => model::launcher::Update::Done(changes),
            None => model::launcher::Update::Failed,
        };
        self.snapshot()
    }

    /// The launchers switched on while the last update was under way, which are to be read now.
    pub fn take_update_again(&mut self) -> Option<Vec<String>> {
        let mut keys = std::mem::take(&mut self.update_again);
        keys.sort();
        keys.dedup();
        (!keys.is_empty()).then_some(keys)
    }

    /// The pictures a Profile may wear besides the plain ones: the Windows account picture, then the pictures in
    /// the data folder's Gamerpics folder, by name. The folder is made if it is not there, for the user to find.
    pub fn pictures(&self) -> Vec<PathBuf> {
        let mut pictures: Vec<PathBuf> = self.account_picture.iter().cloned().collect();
        let Some(folder) = self.path.parent().filter(|_| !self.path.as_os_str().is_empty()) else { return pictures };
        let folder = folder.join(GAMERPICS);
        let _ = std::fs::create_dir_all(&folder);
        let mut found: Vec<PathBuf> = std::fs::read_dir(&folder)
            .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default();
        found.retain(|path| path.is_file() && model::profile::is_picture(path));
        found.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().to_lowercase()));
        pictures.extend(found);
        pictures
    }

    /// Carries out a change Settings' Profiles pages asked for. Gives the snapshot after; what the database
    /// refuses (a picture that is not there) is the user's to be told of.
    pub fn change_profile(&mut self, change: &ProfileChange) -> Result<Option<Library>, String> {
        let Some(database) = self.database.as_mut() else { return Ok(None) };
        match database.change_profile(change) {
            Ok(done) => eprintln!("profiles: {done}"),
            Err(e) => {
                eprintln!("profiles: {e:#}");
                return Err(match change {
                    ProfileChange::Picture { .. } => format!("{e:#}").replacen("there's", "There's", 1),
                    _ => format!("{e:#}"),
                });
            }
        }
        Ok(self.snapshot())
    }
}

/// `library` without when each launcher was last read.
fn unstamped(mut library: Library) -> Library {
    library.launchers.list.iter_mut().for_each(|launcher| launcher.read = None);
    library
}

/// Reads the launchers switched on (`only`: these alone, if they are on) into the database at `path`, and says what
/// that changed in the log: `launchers: update: steam, epic read in 12 ms: 1 new game, play time for 1`.
fn update(path: &std::path::Path, sources: &[Arc<dyn Source>], only: Option<&[String]>) -> Updated {
    let started = Instant::now();
    let run = || -> Result<String> {
        let mut database = Database::open(path)?;
        let before = database.snapshot()?;
        let mut on = library::launchers::switched_on(sources, &database.launchers_off()?);
        if let Some(only) = only {
            on.retain(|source| only.iter().any(|key| key == source.key()));
        }
        let read = library::sources::read(&on);
        database.sync_all(&read, library::now())?;
        let changes = library::launchers::Changes::between(&before, &database.snapshot()?);
        let mut said = changes.to_string();
        let name =
            |key: &str| on.iter().find(|source| source.key() == key).map_or("A launcher", |source| source.name());
        for (key, _) in &read.failed {
            said.push_str(&format!(", {} couldn't be read", name(key)));
        }
        let keys: Vec<&str> = read.found.iter().map(|(key, _)| *key).collect();
        let keys = if keys.is_empty() { "nothing".to_string() } else { keys.join(", ") };
        let ms = started.elapsed().as_millis();
        eprintln!("launchers: update: {keys} read in {ms} ms: {said}");
        Ok(said)
    };
    run().map_err(|e| eprintln!("launchers: update: {}: {e:#}", path.display())).ok()
}

/// A picture's path, if there is a file there; else it is said in the log and left out.
fn there(picture: Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    let picture = picture?;
    if picture.is_file() {
        Some(picture)
    } else {
        eprintln!("library: no picture at {}: left out", picture.display());
        None
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
        assert_eq!(library.game(most_played.games[0]).unwrap().name, "Starbeck Valley");
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
    fn a_launcher_read_again_a_moment_later_is_no_news_but_what_it_found_is() {
        use model::launcher::{Found, Launcher};
        let launcher = |read, games| Launcher { key: "steam".into(), on: true, found: Found::Installed, read, games };
        let at = |read, games| {
            let mut library = screens::fake_library();
            library.launchers.list = vec![launcher(read, games)];
            library
        };
        assert_eq!(super::unstamped(at(Some(100), 5)), super::unstamped(at(Some(101), 5)));
        assert_ne!(super::unstamped(at(Some(100), 5)), super::unstamped(at(Some(101), 6)));
    }

    #[test]
    fn a_launcher_switched_off_hides_its_games_unless_one_is_playing_and_on_brings_them_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("library.json");
        let mut steam = screens::fake_library();
        steam.games.iter_mut().for_each(|game| game.source = "steam".into());
        std::fs::write(&file, serde_json::to_string(&steam).unwrap()).unwrap();
        let (library, mut store) = open(&Args { library: Some(file), ..Args::default() });
        let all = library.games.len();
        assert!(all > 0);

        let playing = library.games[0].clone();
        assert_eq!(store.set_launcher("steam", false, Some(&playing)), None, "a game of its is playing");
        assert_eq!(store.snapshot().unwrap().games.len(), all);
        let off = store.set_launcher("steam", false, None).expect("a new snapshot");
        assert!(off.games.is_empty() && off.hidden.is_empty());
        assert!(!off.launchers.is_on("steam"));
        let on = store.set_launcher("steam", true, None).expect("a new snapshot");
        assert_eq!(on.games, library.games);
        // No database file: there is nothing to update.
        assert!(!store.update_launchers(None, || {}));
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
