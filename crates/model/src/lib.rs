//! The contract between the Steam reader and the Home renderer.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
