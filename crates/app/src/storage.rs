//! Settings > Storage and Manage's storage ring (#179; docs/spec/storage.md): the drives and each store's install
//! folders, read off the UI thread through the stores' installers and Windows' volumes (`shell::drives`), and the
//! folder an install goes to.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use model::storage::{self, Drives, Storage, StoreInstaller};
use model::{Game, Library, Source};

/// Reads Storage now: what the thread that looks runs. Says in the log what it found:
/// `storage: 2 drives; Steam: 2 folders, installs into D:\SteamLibrary unless asked; ring on C:\`; and, of the games
/// owned in more than one launcher (`twins`, from [`twins`]), which stores can install each:
/// `storage: Quiet Orbit can be installed from Steam, Epic Games`.
pub fn read(
    sources: &[Arc<dyn Source>],
    drives: &dyn Drives,
    chosen: &HashMap<String, PathBuf>,
    installed: &[Game],
    twins: &[Game],
) -> Storage {
    let stores: Vec<StoreInstaller> = sources
        .iter()
        .filter_map(|source| {
            Some(StoreInstaller { key: source.key(), name: source.name(), installer: source.installer()? })
        })
        .collect();
    let mut read = storage::read(&stores, drives, chosen, installed);
    eprintln!("{}", line(&read));
    let can = |game: &Game| crate::sources::of(sources, game).filter(|source| source.can_install(game));
    let mut said: Vec<(String, Vec<&str>)> = Vec::new();
    for game in twins {
        let Some(source) = can(game) else { continue };
        read.installable.push(game.id);
        let name = model::names::normalised_name(&game.name);
        match said.iter_mut().find(|(seen, _)| *seen == name) {
            Some((_, stores)) => stores.push(source.name()),
            None => said.push((name, vec![source.name()])),
        }
    }
    for (name, stores) in said.iter().filter(|(_, stores)| stores.len() > 1) {
        let shown = twins.iter().find(|game| model::names::normalised_name(&game.name) == *name);
        let shown = shown.map_or(name.as_str(), |game| game.name.as_str());
        eprintln!("storage: {shown} can be installed from {}", stores.join(", "));
    }
    read
}

/// The games, not installed and in no Queue, that the library has from more than one launcher under one name
/// (`model::names`): those a store may be asked whether it can install, for Install's "Install from" question.
/// Each launcher's games once, the library's order kept; grouped by the normalised name, so a library of thousands
/// is gone through once.
pub fn twins(library: &Library) -> Vec<Game> {
    let store = |game: &&Game| !game.source.is_empty() && game.source != model::by_hand::MANUAL;
    let mut by_name: HashMap<String, Vec<&Game>> = HashMap::new();
    for game in library.games.iter().filter(store) {
        let name = model::names::normalised_name(&game.name);
        if !name.is_empty() {
            by_name.entry(name).or_default().push(game);
        }
    }
    let mut twins: Vec<Game> = Vec::new();
    for game in library.games.iter().filter(store) {
        let Some(same) = by_name.get(&model::names::normalised_name(&game.name)) else { continue };
        let elsewhere = same.iter().any(|other| other.source != game.source);
        if elsewhere && !game.installed && game.id != 0 && library.queue.get(game.id).is_none() {
            twins.push(game.clone());
        }
    }
    twins
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
        let storage = Storage {
            drives: vec![drive("C:\\"), drive("D:\\")],
            stores: vec![steam],
            ring: Some(drive("C:\\")),
            installable: Vec::new(),
        };
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

    /// A store that can install a game where its function says.
    struct Store(&'static str, &'static str, fn(&Game) -> bool);

    impl Source for Store {
        fn key(&self) -> &'static str {
            self.0
        }
        fn name(&self) -> &'static str {
            self.1
        }
        fn read(&self) -> anyhow::Result<Option<model::source::Found>> {
            Ok(None)
        }
        fn start(&self, _game: &Game) -> Option<model::Start> {
            None
        }
        fn ask(&self, game: &Game, _ask: model::Ask) -> Option<model::Start> {
            (self.2)(game).then(|| model::Start::Address(format!("{}://install/{}", self.0, game.source_id)))
        }
    }

    fn game(id: i64, source: &str, name: &str, installed: bool) -> Game {
        Game { id, source: source.into(), source_id: id.to_string(), name: name.into(), installed, ..Game::default() }
    }

    #[test]
    fn twins_are_the_games_not_installed_owned_in_another_launcher_under_the_same_name() {
        let games = vec![
            game(1, "steam", "Quiet Orbit", false),
            game(2, "epic", "QUIET ORBIT™", false),
            game(3, "ea", "Quiet Orbit", true),
            game(4, "steam", "Fathom Nine", false),
            game(5, "epic", "Fathom Nine Deluxe Edition", false),
            game(6, "steam", "Saltwind Rally", true),
            game(7, "gog", "Saltwind Rally", false),
            game(8, model::by_hand::MANUAL, "Fathom Nine", false),
        ];
        let library = Library { games, ..Library::default() };
        let ids: Vec<i64> = twins(&library).iter().map(|game| game.id).collect();
        // An installed copy (3, 6) is not asked about, but makes the others twins; an edition is another game; a game
        // added by hand is no store's.
        assert_eq!(ids, [1, 2, 7]);
    }

    #[test]
    fn of_the_twins_those_whose_store_can_install_them_are_installable() {
        let sources: Vec<Arc<dyn Source>> = vec![
            Arc::new(Store("steam", "Steam", |_| true)),
            Arc::new(Store("epic", "Epic Games", |_| true)),
            Arc::new(Store("ea", "EA app", |_| false)),
        ];
        let quiet = |id, source| game(id, source, "Quiet Orbit", false);
        let twins = [quiet(1, "steam"), quiet(2, "epic"), quiet(3, "ea")];
        let read = read(&sources, &model::storage::NoDrives, &HashMap::new(), &[], &twins);
        assert_eq!(read.installable, [1, 2], "the EA app installs nothing from here");
    }
}
