//! Settings > Storage and Manage's storage ring (#179; docs/spec/storage.md): the drives and each store's install
//! folders, read off the UI thread through the stores' installers and Windows' volumes (`shell::drives`), and the
//! folder an install goes to.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use model::storage::{self, Drives, Storage, StoreInstaller};
use model::{Game, Library, Source};

/// Reads Storage now: what the thread that looks runs. Says in the log what it found:
/// `storage: 2 drives; Steam: 2 folders, installs into D:\SteamLibrary unless asked; ring on C:\`.
pub fn read(
    sources: &[Arc<dyn Source>],
    drives: &dyn Drives,
    chosen: &HashMap<String, PathBuf>,
    installed: &[Game],
) -> Storage {
    let stores: Vec<StoreInstaller> = sources
        .iter()
        .filter_map(|source| {
            Some(StoreInstaller { key: source.key(), name: source.name(), installer: source.installer()? })
        })
        .collect();
    let read = storage::read(&stores, drives, chosen, installed);
    eprintln!("{}", line(&read));
    read
}

/// What the log says of Storage as read.
pub fn line(storage: &Storage) -> String {
    let mut said = match storage.drives.len() {
        1 => "storage: 1 drive".to_string(),
        n => format!("storage: {n} drives"),
    };
    for store in &storage.stores {
        let folders =
            if store.folders.len() == 1 { "1 folder".into() } else { format!("{} folders", store.folders.len()) };
        let into = store.folders.get(store.default).map(|place| place.path.display().to_string()).unwrap_or_default();
        let asks = if store.asks() { " unless asked" } else { "" };
        said += &format!("; {}: {folders}, installs into {into}{asks}", store.name);
    }
    match &storage.ring {
        Some(drive) => said += &format!("; ring on {}", drive.root),
        None => said += "; no ring",
    }
    said
}

/// The folder an Install with no question goes to: the store's default as Storage has it (the user's choice in
/// Settings > Storage, where the store can be told), none where Storage names none for the game's store.
pub fn default_for(library: &Library, game: i64) -> Option<PathBuf> {
    let game = library.game(game)?;
    let store = library.storage.of(&game.source).filter(|store| store.choose)?;
    store.folders.get(store.default).map(|place| place.path.clone())
}

/// The folders the user chose, by store, as the settings keep them.
pub fn chosen(sources: &[Arc<dyn Source>], setting: impl Fn(&str) -> Option<String>) -> HashMap<String, PathBuf> {
    sources
        .iter()
        .filter_map(|source| {
            let path = setting(&storage::setting_key(source.key())).filter(|path| !path.trim().is_empty())?;
            Some((source.key().to_string(), PathBuf::from(path)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use model::storage::{Drive, Place, StoreFolders};
    use model::Storage;

    use super::*;

    fn drive(root: &str) -> Drive {
        Drive { root: root.into(), label: String::new(), free: 1, total: 2 }
    }

    #[test]
    fn the_log_says_where_each_store_installs_and_which_drive_the_ring_shows() {
        let place = |path: &str| Place { path: path.into(), drive: None };
        let steam = StoreFolders {
            store: "steam".into(),
            name: "Steam".into(),
            folders: vec![place(r"D:\SteamLibrary"), place(r"C:\Steam")],
            choose: true,
            default: 0,
        };
        let storage =
            Storage { drives: vec![drive("C:\\"), drive("D:\\")], stores: vec![steam], ring: Some(drive("C:\\")) };
        assert_eq!(
            line(&storage),
            r"storage: 2 drives; Steam: 2 folders, installs into D:\SteamLibrary unless asked; ring on C:\"
        );
        assert_eq!(line(&Storage::default()), "storage: 0 drives; no ring");

        let game = Game { id: 4, source: "steam".into(), ..Game::default() };
        let library = Library { games: vec![game], storage, ..Library::default() };
        assert_eq!(default_for(&library, 4).as_deref(), Some(Path::new(r"D:\SteamLibrary")));
        assert_eq!(default_for(&library, 5), None);
    }
}
