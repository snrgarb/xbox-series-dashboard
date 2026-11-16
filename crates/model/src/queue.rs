//! The Queue (the Bundle's glossary): every game being installed, the current download first, then those next up in
//! the order they started. A store's client says what it has under way (`Source::installs`, an [`Install`] each);
//! the library keeps the Queue across restarts and squares it with what the sources say (`library::queue`); the
//! screens read it from the snapshot (`Library::queue`).
use serde::{Deserialize, Serialize};

/// What a game's tiles read while it waits its turn, or is paused.
pub const QUEUED: &str = "Queued";
/// What they read once its download has started: three full stops, as the console has it.
pub const INSTALLING: &str = "Installing...";

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
    /// The user has paused it in the client.
    Paused { done: u64, total: u64 },
}

impl InstallState {
    /// What its tiles read: Installing... while it downloads, Queued while it waits or is paused, as the Bundle's
    /// read a download that is not active.
    pub fn status(self) -> &'static str {
        match self {
            InstallState::Downloading { .. } => INSTALLING,
            InstallState::Queued | InstallState::Paused { .. } => QUEUED,
        }
    }

    pub fn downloading(self) -> bool {
        matches!(self, InstallState::Downloading { .. })
    }

    /// The bytes done and to do, where the client says.
    pub fn bytes(self) -> Option<(u64, u64)> {
        match self {
            InstallState::Queued => None,
            InstallState::Downloading { done, total } | InstallState::Paused { done, total } => Some((done, total)),
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
        entries.sort_by_key(|entry| (!entry.state.downloading(), entry.started, entry.game));
        Self { entries, installed }
    }

    /// The game's place in the Queue, if it is in it.
    pub fn get(&self, game: i64) -> Option<&Queued> {
        self.entries.iter().find(|entry| entry.game == game)
    }

    /// What the game's tiles read of its install: Queued, Installing..., or nothing for a game not in the Queue.
    /// A game in no database (id 0) is in none.
    pub fn status(&self, game: i64) -> Option<&'static str> {
        self.get(game).filter(|_| game != 0).map(|entry| entry.state.status())
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
        Queued { game, state, started }
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
}
