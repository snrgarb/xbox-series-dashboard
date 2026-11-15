//! The domain: what a game is, the library of them, a Play session. No I/O.
pub mod sound;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use sound::Sound;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    /// The game's row in the library database, the same for as long as the database lives. 0 for a game that is in
    /// no database: one a source has just read, or a made-up one.
    #[serde(default)]
    pub id: i64,
    pub appid: u32,
    pub name: String,
    /// Unix seconds; 0 = never played.
    pub last_played: u64,
    pub playtime_minutes: u32,
    pub installed: bool,
    /// Where the game is installed, if it is.
    #[serde(default)]
    pub install_dir: Option<PathBuf>,
    /// Portrait cover (Steam's 600x900 library capsule), if on disk.
    pub cover: Option<PathBuf>,
    /// Wide background art (Steam's library hero), if on disk.
    pub hero: Option<PathBuf>,
    /// Landscape header (460x215), if on disk.
    pub header: Option<PathBuf>,
    /// The user's own mark, which no source knows of.
    #[serde(default)]
    pub favourite: bool,
}

/// One run of a game, from its window coming up to its ending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaySession {
    /// The game's id in the library database.
    pub game: i64,
    /// Unix seconds.
    pub started: u64,
    /// Unix seconds; `None` while the game runs, and for a session whose end the Dashboard never saw.
    pub ended: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    /// Steam persona name of the signed-in user, if found.
    pub user: Option<String>,
    /// Every owned game (no tools, DLC, or soundtracks), most recently played first.
    pub games: Vec<Game>,
}

impl Library {
    /// The Recent row: the `n` most recently played games.
    pub fn recent(&self, n: usize) -> &[Game] {
        let played = self.games.iter().take_while(|g| g.last_played > 0).count();
        &self.games[..played.min(n)]
    }
}
