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
            let read = library::sources::read(&sources);
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
        let (path, sources) = (self.path.clone(), self.sources.clone());
        let (changed, synced) = mpsc::channel();
        self.synced = Some(synced);
        std::thread::spawn(move || {
            let read = library::sources::read(&sources);
            let news = !read.found.is_empty()
                && match Database::open(&path).and_then(|mut database| sync(&mut database, &read)) {
                    Ok(library) => library != shown,
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
            library.store_apps = self.store_apps.clone();
            library.profiles.pictures = self.pictures();
        }
        Some(update)
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

    /// The new snapshot, once, if the background sync changed the database, or the Store apps have been listed.
    pub fn changed(&mut self) -> Option<Library> {
        let mut changed = false;
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

    /// Saves a setting kept as text, as Settings > Guide's Now Playing source. With no database it lasts as long as
    /// the Dashboard does.
    pub fn set_text(&self, key: &str, value: &str) {
        let Some(database) = &self.database else { return };
        match database.set_setting(key, value) {
            Ok(()) => eprintln!("settings: {key} = {value}"),
            Err(e) => eprintln!("settings: {key} not saved: {e:#}"),
        }
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
        let online =
            self.setting(art::ONLINE_SETTING).is_none_or(|value| !matches!(value.trim(), "false" | "0" | "off" | "no"));
        if !online {
            return None;
        }
        let hosts = self.store_art_service.as_deref().map_or_else(Hosts::default, Hosts::all_at);
        let services = Services {
            stores: Some(StoreArt::new(hosts)),
            steamgriddb: self.art_key().map(|key| Service::new(&self.art_service, &key)),
        };
        Some((self.art_dir()?, services))
    }

    /// Looks up, on threads of their own, the art the stores' files on the PC gave none for: of the games of
    /// `library` that lack a picture, those our cache has not got, from the store's own art online, then from
    /// SteamGridDB with a key (docs/sources.md, "Art"). `tell` is told as pictures come in. None when there is
    /// nothing to do it with.
    pub fn look_up_art(&self, library: &Library, tell: impl Fn(Event) + Send + Sync + 'static) -> Option<Lookup> {
        let (dir, services) = self.art_services()?;
        let log: art::Log = std::sync::Arc::new(|line| eprintln!("{line}"));
        Some(Lookup::start(dir, services, art::wanted(library), library::now(), Pace::default(), log, tell))
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
        library.profiles.pictures = self.pictures();
        Some(Library { store_apps: self.store_apps.clone(), ..library })
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
