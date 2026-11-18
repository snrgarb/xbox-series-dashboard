//! What the store sources other than Steam share (docs/sources.md): the Windows registry behind a seam that a file
//! can stand in for ([`registry`]), the few bytes of protobuf two stores keep their lists in ([`proto`]), and an
//! uninstaller's command line taken apart into a program to run ([`command`]); and the helper programs some stores
//! install through: running one ([`helper`]), a store's downloads through one ([`downloads`]), and where a store's
//! helper is ([`HelperRoute`]); and a store's list of owned games kept as a file ([`owned`]).
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod command;
pub mod downloads;
pub mod helper;
pub mod owned;
pub mod proto;
pub mod registry;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where a store's helper program is and keeps its records, as the executable gives it to the store's source
/// (docs/spec/helpers.md 5): legendary for Epic Games, gogdl for GOG.
#[derive(Clone)]
pub struct HelperRoute {
    /// The helper's program where the Dashboard keeps it, while it is added (kept, at the table's version); None
    /// while it is not, and the store is asked as before.
    pub program: Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>,
    /// The folder its sign-in and its records are in.
    pub home: PathBuf,
    /// What every run of it is given on top of the Dashboard's environment: a test's folder for its records, else
    /// nothing, and the helper keeps them where it always does.
    pub env: Vec<(String, OsString)>,
    /// Where a game goes with no folder chosen: a folder of the user's own (legendary's own default, `~\Games`), so
    /// no install asks Windows for an administrator.
    pub games: PathBuf,
}

impl HelperRoute {
    /// A route to a program that is always there: for the tests.
    pub fn fixed(program: PathBuf, home: PathBuf, env: Vec<(String, OsString)>, games: PathBuf) -> Self {
        Self { program: Arc::new(move || Some(program.clone()).filter(|p| p.is_file())), home, env, games }
    }
}

impl std::fmt::Debug for HelperRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelperRoute").field("home", &self.home).field("games", &self.games).finish_non_exhaustive()
    }
}

/// A path as a store writes it, with forward slashes or back, as this system writes paths: on Windows with back
/// slashes, elsewhere (the tests) as it is.
pub fn native(path: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.replace('/', "\\"))
    } else {
        PathBuf::from(path)
    }
}

/// Whether `dir` is a folder with nothing in it: a fake named for a store that is not there, which a source reads as
/// no store (and never as "the machine's own, then"). What the tests on the TV PC give every store they made no
/// fake of (e2e/README.md, safe mode).
pub fn empty_folder(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
}

/// Whether two folders are the same one by their paths: case, the kind of slash and a trailing one aside.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    let plain = |path: &Path| path.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase();
    plain(a) == plain(b)
}

/// The folder's last part, as a name: what a store with no name for a game has.
pub fn folder_name(path: &Path) -> Option<String> {
    let text = path.to_string_lossy().replace('\\', "/");
    text.trim_end_matches('/').rsplit('/').next().filter(|name| !name.is_empty()).map(str::to_string)
}

/// Writes a fake game's folder at `dir` for a fixture: with `exe`, a copy of it as `game.exe` and beside it
/// `game-args.txt` titling the stand-in game's window with `name` (crates/standin), so a test can tell which game is
/// in front; without, the folder alone.
pub fn write_game_folder(dir: &Path, name: &str, exe: Option<&Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    if let Some(exe) = exe {
        std::fs::copy(exe, dir.join("game.exe"))?;
        std::fs::write(dir.join("game-args.txt"), format!("--title\n{name}\n"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_the_same_whatever_their_slashes_and_case() {
        assert!(same_folder(Path::new("D:/Ubisoft/Harborlight/"), Path::new(r"d:\ubisoft\harborlight")));
        assert!(!same_folder(Path::new("D:/Ubisoft/Harborlight"), Path::new("D:/Ubisoft/Harbor")));
        assert_eq!(folder_name(Path::new("D:/Ubisoft/Harborlight/")).as_deref(), Some("Harborlight"));
        assert_eq!(folder_name(Path::new(r"D:\Games\Oxbow Frontier")).as_deref(), Some("Oxbow Frontier"));
        assert_eq!(folder_name(Path::new("/")), None);
    }

    #[test]
    fn a_folder_is_empty_only_when_it_is_there_with_nothing_in_it() {
        let dir = tempfile::tempdir().unwrap();
        assert!(empty_folder(dir.path()));
        assert!(!empty_folder(&dir.path().join("not there")));
        std::fs::write(dir.path().join("registry.json"), "{}").unwrap();
        assert!(!empty_folder(dir.path()));
    }
}
