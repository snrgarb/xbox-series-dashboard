//! Where the Library the Dashboard shows comes from, and where what happens to it goes: the library database, with
//! Steam read into it.
//!
//! When the database has games the first frame is drawn from it, and Steam is read on a thread of its own once that
//! frame is out; if that changes anything the window is woken and takes a new snapshot. With an empty database (a
//! first run) or no window to wake (`--screenshot`), Steam is read before anything is drawn.
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use anyhow::Result;
use library::Database;
use model::{Game, Library};

use crate::{fail, Args};

#[derive(Default)]
pub struct Store {
    database: Option<Database>,
    /// The database's file, which the thread that syncs opens for itself.
    path: PathBuf,
    /// The Steam install still to be read, when the Library on screen came from the database.
    steam: Option<PathBuf>,
    /// Hears from the thread that syncs when it has changed the database.
    synced: Option<Receiver<()>>,
    /// A game is running, and `game_booted` has been told so.
    booted: bool,
    /// The running game's Play session.
    session: Option<i64>,
}

/// The one place the Library comes from: `--library`'s JSON as it is; else the database, filled from `--steam`'s
/// install or the one the registry names; else, with no database and no Steam, the built-in fake one.
///
/// `--screenshot` uses a database only where `--data-dir` names one: drawing a picture leaves nothing behind.
pub fn open(args: &Args) -> (Library, Store) {
    if let Some(path) = &args.library {
        let json = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        let library = serde_json::from_str(&json).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        return (library, Store::default());
    }
    let window = args.screenshot.is_none();
    let dir = args.data_dir.clone().or_else(|| if window { library::data_dir() } else { None });
    let path = dir.map(|dir| dir.join(library::FILE));
    let started = Instant::now();
    // Without a database the Dashboard still runs, on what Steam says now.
    let mut database = path
        .as_ref()
        .and_then(|path| Database::open(path).map_err(|e| eprintln!("library: {}: {e:#}", path.display())).ok());
    let stored = database.as_ref().and_then(|db| db.snapshot().ok()).filter(|library| !library.games.is_empty());
    if let Some(library) = &stored {
        let ms = started.elapsed().as_millis();
        eprintln!("library: {} games from the database in {ms} ms", library.games.len());
    }

    let steam = args.steam.clone().or_else(source_steam::find_steam_root);
    let mut later = None;
    let library = match (stored, steam) {
        (Some(library), Some(root)) if window => {
            later = Some(root);
            library
        }
        (stored, Some(root)) => match (read_steam(&root), stored) {
            (Ok(found), _) => match database.as_mut().map(|db| sync(db, &found)) {
                Some(Ok(library)) => library,
                Some(Err(e)) => {
                    eprintln!("library: {e:#}");
                    found
                }
                None => found,
            },
            (Err(e), Some(library)) => {
                eprintln!("steam: {}: {e:#}", root.display());
                library
            }
            (Err(e), None) => fail(&format!("{}: {e}", root.display())),
        },
        (Some(library), None) => library,
        (None, None) => screens::fake_library(),
    };
    (library, Store { database, path: path.unwrap_or_default(), steam: later, ..Store::default() })
}

fn read_steam(root: &Path) -> Result<Library> {
    let started = Instant::now();
    let library = source_steam::load(root)?;
    let ms = started.elapsed().as_millis();
    eprintln!("steam: {} games from {} in {ms} ms", library.games.len(), root.display());
    Ok(library)
}

/// Merges what Steam was read as into the database; gives the snapshot after.
fn sync(database: &mut Database, found: &Library) -> Result<Library> {
    let started = Instant::now();
    let synced = database.sync(library::STEAM, found, library::now())?;
    let ms = started.elapsed().as_millis();
    eprintln!("library: {} added, {} updated, {} gone, in {ms} ms", synced.added, synced.updated, synced.gone);
    database.snapshot()
}

impl Store {
    /// Reads Steam into the database on a thread of its own, if that is still to do. `shown` is the Library on
    /// screen; `wake` is called if the database ends up saying something else.
    pub fn sync_in_background(&mut self, shown: &Library, wake: impl FnOnce() + Send + 'static) {
        let Some(root) = self.steam.take() else { return };
        let (path, shown) = (self.path.clone(), shown.clone());
        let (changed, synced) = mpsc::channel();
        self.synced = Some(synced);
        std::thread::spawn(move || {
            let synced = read_steam(&root).and_then(|found| sync(&mut Database::open(&path)?, &found));
            match synced {
                Ok(library) if library != shown => {
                    let _ = changed.send(());
                    wake();
                }
                Ok(_) => {}
                Err(e) => eprintln!("library: {}: {e:#}", root.display()),
            }
        });
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

    fn snapshot(&self) -> Option<Library> {
        self.database.as_ref()?.snapshot().map_err(|e| eprintln!("library: {e:#}")).ok()
    }
}
