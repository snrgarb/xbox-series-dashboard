//! What the store sources other than Steam share (docs/sources.md): the Windows registry behind a seam that a file
//! can stand in for ([`registry`]), the few bytes of protobuf two stores keep their lists in ([`proto`]), and an
//! uninstaller's command line taken apart into a program to run ([`command`]).
pub mod command;
pub mod proto;
pub mod registry;

use std::path::{Path, PathBuf};

/// A path as a store writes it, with forward slashes or back, as this system writes paths: on Windows with back
/// slashes, elsewhere (the tests) as it is.
pub fn native(path: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.replace('/', "\\"))
    } else {
        PathBuf::from(path)
    }
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
}
