//! Where installs may go, and how full the drives are (Settings > Storage, #179; docs/spec/storage.md): each store's
//! install folders as its installer gives them (`Installer::folders`), each on its drive with its free and total
//! space, the folder an install goes to unless the user says otherwise, and the drive Manage's storage ring shows
//! (docs/spec/my-games.md, 2.6).
//!
//! The drives are read behind [`Drives`], the seam between the Dashboard and Windows' volumes: the executable's
//! reads them (`shell::drives`), a test's is made up. What is read is put together here, with no I/O, by [`read`].
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Folder, Game, Installer};

/// A drive: its root as Windows writes it (`C:\`), its name where it has one, and its space in bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drive {
    pub root: String,
    /// The volume's label (`Games`), empty where it has none.
    pub label: String,
    pub free: u64,
    pub total: u64,
}

impl Drive {
    /// How the page names it: `Games (D:)`, or `Local Disk (C:)` as Explorer names a drive with no label.
    pub fn name(&self) -> String {
        let letter = self.root.trim_end_matches(['\\', '/']);
        let label = if self.label.is_empty() { "Local Disk" } else { &self.label };
        format!("{label} ({letter})")
    }

    /// The share of it in use, 0 to 1. None for a drive that says it holds nothing.
    pub fn used(&self) -> Option<f64> {
        (self.total > 0).then(|| self.total.saturating_sub(self.free) as f64 / self.total as f64)
    }

    /// `354.6 GB free`, as the Bundle's ring words it.
    pub fn free_text(&self) -> String {
        format!("{} free", size(self.free))
    }

    /// `354.6 GB free of 931.5 GB`.
    pub fn space_text(&self) -> String {
        format!("{} free of {}", size(self.free), size(self.total))
    }
}

/// A size as the Bundle writes one (C/MyGamesState.cs:283-309): to one decimal, in GB, MB below 1 GB, TB from
/// 1024 GB; 1024 to a step.
pub fn size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let gb = bytes as f64 / (MB * 1024.0);
    if gb >= 1024.0 {
        format!("{:.1} TB", gb / 1024.0)
    } else if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.1} MB", bytes as f64 / MB)
    }
}

/// The used share as the ring writes it: `55.8%`.
pub fn percent(used: f64) -> String {
    format!("{:.1}%", used * 100.0)
}

/// Windows' volumes, as the Dashboard reads them. Only the executable's reads anything; a test's is made up.
pub trait Drives: Send + Sync {
    /// Every fixed drive, in order of their roots.
    fn all(&self) -> Vec<Drive>;

    /// The drive `path` is on, read now. None where it cannot be read (no such drive, a network path).
    fn of(&self, path: &Path) -> Option<Drive>;
}

/// No drives at all: anywhere a drive cannot be read.
pub struct NoDrives;

impl Drives for NoDrives {
    fn all(&self) -> Vec<Drive> {
        Vec::new()
    }

    fn of(&self, _path: &Path) -> Option<Drive> {
        None
    }
}

/// One of a store's install folders, on its drive where that could be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub path: PathBuf,
    pub drive: Option<Drive>,
}

impl Place {
    /// How the question and the page name it: the folder, then its free space where known.
    pub fn line(&self) -> String {
        match &self.drive {
            Some(drive) => format!("{}, {}", self.path.display(), drive.free_text()),
            None => self.path.display().to_string(),
        }
    }
}

/// A store's install folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreFolders {
    /// The store, by its source's key (`Game::source`): `steam`.
    pub store: String,
    /// Its name, as the page writes it: `Steam`.
    pub name: String,
    pub folders: Vec<Place>,
    /// Whether an install can be put in a folder of the user's choice: the installer's `Can::choose_folder`.
    pub choose: bool,
    /// The folder installs go to unless the user says otherwise, by its place in `folders`: the one the user chose
    /// in Settings > Storage while it is still there, else the store's own default, else the first.
    pub default: usize,
}

impl StoreFolders {
    /// Whether an install asks which folder: the store can be told, and there is more than one.
    pub fn asks(&self) -> bool {
        self.choose && self.folders.len() > 1
    }
}

/// Everything Settings > Storage and Manage's storage ring show, as read when last looked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Storage {
    /// Every fixed drive.
    pub drives: Vec<Drive>,
    /// Each store with an installer that names its folders, in the sources' order.
    pub stores: Vec<StoreFolders>,
    /// The drive Manage's ring shows: the one holding most installed games that are not hidden; none where no
    /// installed game's drive could be read (docs/spec/my-games.md, 2.6).
    pub ring: Option<Drive>,
}

impl Storage {
    /// The folders of the store with this key, if it names any.
    pub fn of(&self, store: &str) -> Option<&StoreFolders> {
        self.stores.iter().find(|folders| folders.store == store)
    }
}

/// A store to read the folders of: its key, its name, and its installer.
pub struct StoreInstaller<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub installer: &'a dyn Installer,
}

/// The settings key that keeps the folder the user chose for a store's installs.
pub fn setting_key(store: &str) -> String {
    format!("storage.default.{store}")
}

/// Whether two paths name the same folder, as Windows compares them: either slash, any case, no trailing slash.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    let norm = |path: &Path| path.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

/// Puts Storage together: every fixed drive; each store's folders with their drives, the user's choice (`chosen`,
/// by store key) as the default where it is still one of them and the store lets one be chosen; and the ring's
/// drive, from where the installed games (`installed`, not hidden) are.
pub fn read<'a>(
    stores: &[StoreInstaller<'_>],
    drives: &dyn Drives,
    chosen: &HashMap<String, PathBuf>,
    installed: impl IntoIterator<Item = &'a Game>,
) -> Storage {
    let stores = stores
        .iter()
        .filter_map(|store| {
            let folders: Vec<Folder> = store.installer.folders();
            if folders.is_empty() {
                return None;
            }
            let choose = store.installer.can().choose_folder;
            let own = folders.iter().position(|folder| folder.default).unwrap_or(0);
            let picked = chosen
                .get(store.key)
                .filter(|_| choose)
                .and_then(|path| folders.iter().position(|folder| same_folder(&folder.path, path)));
            let folders = folders
                .into_iter()
                .map(|folder| {
                    let drive = drives.of(&folder.path);
                    Place { path: folder.path, drive }
                })
                .collect();
            Some(StoreFolders {
                store: store.key.to_string(),
                name: store.name.to_string(),
                folders,
                choose,
                default: picked.unwrap_or(own),
            })
        })
        .collect();
    Storage { drives: drives.all(), stores, ring: ring(drives, installed) }
}

/// The drive holding most of these games, ties to the first by its root; none where no game's drive is known.
fn ring<'a>(drives: &dyn Drives, installed: impl IntoIterator<Item = &'a Game>) -> Option<Drive> {
    let mut counts: Vec<(Drive, usize)> = Vec::new();
    for game in installed.into_iter().filter(|game| game.installed && !game.hidden) {
        let Some(drive) = game.install_dir.as_deref().and_then(|dir| drives.of(dir)) else { continue };
        match counts.iter_mut().find(|(seen, _)| seen.root.eq_ignore_ascii_case(&drive.root)) {
            Some((_, count)) => *count += 1,
            None => counts.push((drive, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.root.to_lowercase().cmp(&b.0.root.to_lowercase())));
    counts.into_iter().next().map(|(drive, _)| drive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Can;

    /// Drives by the letter a path starts with.
    pub struct Letters(Vec<Drive>);

    impl Drives for Letters {
        fn all(&self) -> Vec<Drive> {
            self.0.clone()
        }

        fn of(&self, path: &Path) -> Option<Drive> {
            let letter = path.to_string_lossy().chars().next()?.to_ascii_uppercase();
            self.0.iter().find(|drive| drive.root.starts_with(letter)).cloned()
        }
    }

    fn drive(root: &str, free: u64, total: u64) -> Drive {
        Drive { root: root.into(), label: String::new(), free, total }
    }

    struct Folders(Vec<Folder>, bool);

    impl Installer for Folders {
        fn can(&self) -> Can {
            Can { choose_folder: self.1, ..Can::NOTHING }
        }

        fn folders(&self) -> Vec<Folder> {
            self.0.clone()
        }
    }

    fn folder(path: &str, default: bool) -> Folder {
        Folder { path: path.into(), free: None, default }
    }

    fn game(dir: &str, hidden: bool) -> Game {
        Game { installed: true, hidden, install_dir: Some(dir.into()), ..Game::default() }
    }

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn sizes_are_written_as_the_bundle_writes_them() {
        assert_eq!(size(354 * GB + GB / 2 + GB / 10), "354.6 GB");
        assert_eq!(size(512 * 1024 * 1024), "512.0 MB");
        assert_eq!(size(2048 * GB), "2.0 TB");
        assert_eq!(percent(0.558), "55.8%");
        let d = Drive { root: "D:\\".into(), label: "Games".into(), free: GB, total: 4 * GB };
        assert_eq!((d.name(), d.free_text(), d.used()), ("Games (D:)".into(), "1.0 GB free".into(), Some(0.75)));
        assert_eq!(drive("C:\\", 0, 0).name(), "Local Disk (C:)");
        assert_eq!(drive("C:\\", 0, 0).used(), None);
    }

    #[test]
    fn each_folder_is_on_its_drive_and_the_users_choice_is_the_default_while_it_is_there() {
        let drives = Letters(vec![drive("C:\\", 10 * GB, 100 * GB), drive("D:\\", 50 * GB, 200 * GB)]);
        let steam = Folders(vec![folder("D:\\SteamLibrary", false), folder("C:\\Steam", true)], true);
        let stores = [StoreInstaller { key: "steam", name: "Steam", installer: &steam }];
        let none = HashMap::new();
        let storage = read(&stores, &drives, &none, []);
        let steam_folders = storage.of("steam").unwrap();
        assert_eq!((steam_folders.default, steam_folders.asks()), (1, true));
        assert_eq!(steam_folders.folders[0].drive.as_ref().map(|d| d.root.as_str()), Some("D:\\"));
        assert_eq!(steam_folders.folders[0].line(), "D:\\SteamLibrary, 50.0 GB free");
        assert_eq!(storage.drives.len(), 2);

        let chosen = HashMap::from([("steam".to_string(), PathBuf::from("d:/steamlibrary/"))]);
        assert_eq!(read(&stores, &drives, &chosen, []).of("steam").unwrap().default, 0);
        // A folder that is gone is no choice: the store's own default again.
        let gone = HashMap::from([("steam".to_string(), PathBuf::from("E:\\Gone"))]);
        assert_eq!(read(&stores, &drives, &gone, []).of("steam").unwrap().default, 1);
        // A store that cannot be told keeps its own, and never asks.
        let fixed = Folders(steam.0.clone(), false);
        let stores = [StoreInstaller { key: "steam", name: "Steam", installer: &fixed }];
        let read_fixed = read(&stores, &drives, &chosen, []);
        assert_eq!((read_fixed.of("steam").unwrap().default, read_fixed.of("steam").unwrap().asks()), (1, false));
    }

    #[test]
    fn a_store_with_no_folders_is_left_out() {
        let none = Folders(Vec::new(), true);
        let stores = [StoreInstaller { key: "epic", name: "Epic Games", installer: &none }];
        assert!(read(&stores, &NoDrives, &HashMap::new(), []).stores.is_empty());
    }

    #[test]
    fn the_ring_shows_the_drive_with_most_installed_games_that_are_not_hidden() {
        let drives = Letters(vec![drive("C:\\", 1, 2), drive("D:\\", 1, 2)]);
        let games = [game("D:\\a", false), game("C:\\b", false), game("C:\\c", true), game("Q:\\d", false)];
        assert_eq!(read(&[], &drives, &HashMap::new(), &games).ring.map(|d| d.root), Some("C:\\".into()));
        let more = [game("D:\\a", false), game("D:\\e", false), game("C:\\b", false)];
        assert_eq!(read(&[], &drives, &HashMap::new(), &more).ring.map(|d| d.root), Some("D:\\".into()));
        // None installed, or none on a drive that can be read: no ring.
        assert_eq!(read(&[], &drives, &HashMap::new(), &[game("Q:\\x", false)]).ring, None);
        assert_eq!(read(&[], &NoDrives, &HashMap::new(), &games).ring, None);
    }
}
