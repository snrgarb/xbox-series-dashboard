//! The Queue (the Bundle's glossary): every game being installed, the current download first, then those next up in
//! the order they started. A store's client says what it has under way (`Source::installs`, an [`Install`] each);
//! the library keeps the Queue across restarts and squares it with what the sources say (`library::queue`); the
//! screens read it from the snapshot (`Library::queue`).
use serde::{Deserialize, Serialize};

use crate::Can;

/// What a game's tiles read while it waits its turn, or is paused.
pub const QUEUED: &str = "Queued";
/// What they read once its download has started: three full stops, as the console has it.
pub const INSTALLING: &str = "Installing...";
/// What Manage Queue's row reads of an install paused, by the user here or in the store's client (#170): the
/// console's word. Tiles read it Queued, as the Bundle's do.
pub const PAUSED: &str = "Paused";

/// Where an install a store's client has under way is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum InstallState {
    /// Waiting its turn: the client has it, and has not started on it, or the Dashboard has asked for it and the
    /// client has not said so yet.
    Queued,
    /// The client is downloading it: `done` of `total` bytes, as far as the client has written them down (`total`
    /// 0 while it does not know).
    Downloading { done: u64, total: u64 },
    /// The user has paused it, here or in the client.
    Paused { done: u64, total: u64 },
    /// The game itself is installed and playable, and its store is still fetching more for it: Steam's Workshop
    /// items the account subscribes to (#188), `done` of `total` bytes of that where the store says (0 of 0 while
    /// it does not). Play is offered and the game's Ready to play Toast has been told; the Queue keeps its row
    /// until the store is done, and it then leaves with nothing more said.
    Extras { done: u64, total: u64 },
}

impl InstallState {
    /// What its tiles read: Installing... while it downloads, Queued while it waits or is paused, as the Bundle's
    /// read a download that is not active.
    /// A game whose extras are still coming in reads as an installing one in the Queue's own row; its tiles read
    /// it installed (`Queue::status`).
    pub fn status(self) -> &'static str {
        match self {
            InstallState::Downloading { .. } | InstallState::Extras { .. } => INSTALLING,
            InstallState::Queued | InstallState::Paused { .. } => QUEUED,
        }
    }

    pub fn downloading(self) -> bool {
        matches!(self, InstallState::Downloading { .. })
    }

    /// The game is installed, and only extras are still coming in.
    pub fn extras(self) -> bool {
        matches!(self, InstallState::Extras { .. })
    }

    pub fn paused(self) -> bool {
        matches!(self, InstallState::Paused { .. })
    }

    /// The bytes done and to do, where the client says.
    pub fn bytes(self) -> Option<(u64, u64)> {
        match self {
            InstallState::Queued => None,
            InstallState::Downloading { done, total }
            | InstallState::Paused { done, total }
            | InstallState::Extras { done, total } => Some((done, total)),
        }
    }

    /// The whole percent done, for the log: none without a total.
    pub fn percent(self) -> Option<u64> {
        let (done, total) = self.bytes().filter(|&(_, total)| total > 0)?;
        Some(done.min(total) * 100 / total)
    }
}

/// An install a store's client has under way, as its source reports it: the game by the store's own id for it
/// (`Game::source_id`), and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub source_id: String,
    pub state: InstallState,
    /// How fast it downloads, in bits a second, where the store says so itself (Steam's download overview, its
    /// Downloads page's number): the Queue shows it in place of the speed it works out from the bytes.
    pub speed: Option<u64>,
}

impl Install {
    /// An install with no speed of the store's own.
    pub fn new(source_id: impl Into<String>, state: InstallState) -> Self {
        Self { source_id: source_id.into(), state, speed: None }
    }
}

/// A game in the Queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queued {
    /// The game's id in the library database, found with `Library::game`.
    pub game: i64,
    pub state: InstallState,
    /// Unix seconds: when it joined the Queue, asked for here or first seen in the client. What "next up" is ordered
    /// by, the client's own order not being readable.
    pub started: u64,
    /// How fast it is downloading, in bits a second, as the library's follower works it out from the bytes done
    /// at successive looks (`library::queue`): none while it is not downloading, or until its bytes have first
    /// moved. The database keeps no speed: a Queue read from it has none.
    #[serde(default)]
    pub speed: Option<u64>,
    /// What its store's installer can do with it (`Source::installer`): which of Pause, Resume and Cancel its menu
    /// in the Queue offers. Given by the library's follower, as the speed is; nothing in a Queue read from the
    /// database alone.
    #[serde(default)]
    pub can: Can,
}

impl Queued {
    /// A game in the Queue with no speed and nothing its store can be asked of it.
    pub fn new(game: i64, state: InstallState, started: u64) -> Self {
        Self { game, state, started, speed: None, can: Can::NOTHING }
    }
}

/// A game the Queue held that finished installing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub game: i64,
    /// Unix seconds.
    pub finished: u64,
}

/// The Queue as a snapshot has it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    /// The games being installed: the current download first, then the rest in the order they started.
    pub entries: Vec<Queued>,
    /// The games the Queue held that have finished installing, the latest first (Manage Queue's Recently
    /// installed, #79).
    pub installed: Vec<Installed>,
}

impl Queue {
    /// The Queue's entries in its order: those downloading first, then by when they started.
    pub fn new(mut entries: Vec<Queued>, installed: Vec<Installed>) -> Self {
        entries.sort_by_key(|entry| (!(entry.state.downloading() || entry.state.extras()), entry.started, entry.game));
        Self { entries, installed }
    }

    /// The game's place in the Queue, if it is in it.
    pub fn get(&self, game: i64) -> Option<&Queued> {
        self.entries.iter().find(|entry| entry.game == game)
    }

    /// What the game's tiles read of its install: Queued, Installing..., or nothing for a game not in the Queue, or
    /// one installed whose extras are still coming in (#188: it is played as any installed game). A game in no
    /// database (id 0) is in none.
    pub fn status(&self, game: i64) -> Option<&'static str> {
        self.installing(game).map(|entry| entry.state.status())
    }

    /// The game's place in the Queue while the game itself is not installed yet: not one whose extras alone are
    /// still coming in. A game in no database (id 0) is in none.
    pub fn installing(&self, game: i64) -> Option<&Queued> {
        self.get(game).filter(|entry| game != 0 && !entry.state.extras())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(game: i64, state: InstallState, started: u64) -> Queued {
        Queued::new(game, state, started)
    }

    #[test]
    fn the_current_download_comes_first_then_the_rest_in_the_order_they_started() {
        let downloading = InstallState::Downloading { done: 5, total: 10 };
        let paused = InstallState::Paused { done: 1, total: 10 };
        let queue = Queue::new(
            vec![entry(1, InstallState::Queued, 100), entry(2, paused, 50), entry(3, downloading, 300)],
            Vec::new(),
        );
        assert_eq!(queue.entries.iter().map(|e| e.game).collect::<Vec<_>>(), [3, 2, 1]);
        assert_eq!((queue.status(3), queue.status(2), queue.status(1)), (Some(INSTALLING), Some(QUEUED), Some(QUEUED)));
        assert_eq!((queue.status(4), queue.len()), (None, 3));
        assert_eq!(
            (downloading.percent(), paused.bytes(), InstallState::Queued.percent()),
            (Some(50), Some((1, 10)), None)
        );
    }

    #[test]
    fn a_game_whose_extras_alone_are_coming_in_reads_installed_on_its_tiles_and_stays_in_the_queue() {
        let extras = InstallState::Extras { done: 0, total: 0 };
        let queue = Queue::new(vec![entry(1, InstallState::Queued, 100), entry(2, extras, 300)], Vec::new());
        // Under way, so first; but its tiles read it as any installed game.
        assert_eq!(queue.entries.iter().map(|e| e.game).collect::<Vec<_>>(), [2, 1]);
        assert_eq!((queue.status(2), queue.installing(2), queue.get(2).map(|e| e.state)), (None, None, Some(extras)));
        assert_eq!(queue.status(1), Some(QUEUED));
        assert!(extras.extras() && !extras.downloading() && !extras.paused());
    }
}
