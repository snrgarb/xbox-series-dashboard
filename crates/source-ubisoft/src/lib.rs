//! Ubisoft Connect's games, installed and owned (docs/research/stores.md, "Ubisoft Connect"; docs/sources.md).
//!
//! - **Installed:** `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>`, value `InstallDir` (forward
//!   slashes), whose folder is there.
//! - **Owned:** the client's cache, `%LOCALAPPDATA%\Ubisoft Game Launcher\cache\configuration\configurations`, or
//!   the same under the client's folder (`cache`): every product the account has had, of which the games
//!   (`cache::games`). A game with no `Installs` key is owned and not installed. The cache is there only once the
//!   client has been signed in on this PC; without it the installed games are listed by their folders' names.
//! - **Which id:** a game is its `uplay_id`, its `source_id`. An `Installs` key is matched to it by either the
//!   `uplay_id` or the `install_id`: which of the two the key is named for is not settled.
//! - **Art:** the cover (`thumb_image`) and background, where the client has them in its `cache\assets` folder.
//! - **The client:** `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher`, value `InstallDir`. With neither it nor an
//!   `Installs` key, Ubisoft Connect is not installed.
//! - **Start, install, uninstall:** the client's links, `uplay://launch/<id>`, `uplay://install/<id>`,
//!   `uplay://uninstall/<id>`.
//!
//! `Ubisoft::at(Some(dir))` reads a fake client instead (`--ubisoft <dir>`, and `fixture`): its registry is
//! `<dir>/registry.json` (`source_kit::registry`), its cache where that file's client folder has it.
pub mod cache;
pub mod fixture;
pub mod yaml;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use model::{Ask, Found, Game, Library, Source, Start};
use source_kit::registry::{self, Registry};

/// Ubisoft, as a game's `source` names it.
pub const SOURCE: &str = "ubisoft";

/// The client's key; its games' are under `Installs`.
pub const LAUNCHER: &str = r"HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher";

/// The cache under the client's data folder, and under `%LOCALAPPDATA%` that folder's name.
const CACHE: [&str; 3] = ["cache", "configuration", "configurations"];
const DATA: &str = "Ubisoft Game Launcher";

/// Ubisoft Connect, as the library reads it and the shell starts its games.
#[derive(Debug, Clone, Default)]
pub struct Ubisoft {
    /// A fake client to read in place of this machine's.
    fake: Option<PathBuf>,
}

impl Ubisoft {
    /// This machine's Ubisoft Connect, or with `Some(dir)` the fake one at `dir` (`--ubisoft`).
    pub fn at(fake: Option<PathBuf>) -> Self {
        Self { fake }
    }

    fn registry(&self) -> Result<Registry> {
        match &self.fake {
            Some(dir) => Registry::load(&dir.join(fixture::REGISTRY)).with_context(|| dir.display().to_string()),
            None => Ok(Registry::Windows),
        }
    }

    /// Where the cache may be, in order: under `%LOCALAPPDATA%` (not for a fake), then under the client's folder.
    fn caches(&self, client: Option<&Path>) -> Vec<PathBuf> {
        let local = std::env::var_os("LOCALAPPDATA").filter(|_| self.fake.is_none() && cfg!(windows));
        let local = local.map(|local| PathBuf::from(local).join(DATA));
        let under = |dir: PathBuf| CACHE.iter().fold(dir, |path, part| path.join(part));
        local.into_iter().chain(client.map(Path::to_path_buf)).map(under).collect()
    }
}

/// An `Installs` key: its name and the game's folder.
struct Install {
    id: String,
    folder: PathBuf,
}

fn installs(registry: &Registry) -> Vec<Install> {
    registry
        .subkeys(&format!(r"{LAUNCHER}\Installs"))
        .into_iter()
        .filter_map(|(id, values)| {
            let folder = source_kit::native(registry::get(&values, "InstallDir")?.trim());
            folder.is_dir().then(|| Install { id: id.trim().to_string(), folder })
        })
        .collect()
}

/// The games: the cache's, each installed where a key matches it; then the keys that match no product of the
/// cache, by their folders' names. `assets` is the client's art cache.
fn games(products: &[cache::Product], installs: &[Install], assets: Option<&Path>) -> Vec<Game> {
    let named = |id: u32| move |install: &&Install| install.id == id.to_string();
    let art = |file: &Option<String>| {
        let path = assets?.join(file.as_deref()?);
        path.is_file().then_some(path)
    };
    let mut games: Vec<Game> = Vec::new();
    for product in cache::games(products) {
        let install = installs.iter().find(named(product.uplay_id)).or_else(|| {
            let by_install_id = installs.iter().find(named(product.install_id));
            by_install_id.filter(|_| product.install_id != 0)
        });
        let folder_name = install.and_then(|install| source_kit::folder_name(&install.folder));
        let Some(name) = product.name.clone().or(folder_name) else { continue };
        games.push(Game {
            source: SOURCE.into(),
            source_id: product.uplay_id.to_string(),
            name,
            installed: install.is_some(),
            install_dir: install.map(|install| install.folder.clone()),
            cover: art(&product.thumb),
            hero: art(&product.background),
            ..Game::default()
        });
    }
    let known = |install: &Install| {
        products.iter().any(|p| install.id == p.uplay_id.to_string() || install.id == p.install_id.to_string())
    };
    for install in installs.iter().filter(|install| !known(install)) {
        let Some(name) = source_kit::folder_name(&install.folder) else { continue };
        games.push(Game {
            source: SOURCE.into(),
            source_id: install.id.clone(),
            name,
            installed: true,
            install_dir: Some(install.folder.clone()),
            ..Game::default()
        });
    }
    games.sort_by_key(|game| game.name.to_lowercase());
    games
}

impl Source for Ubisoft {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "Ubisoft Connect"
    }

    fn read(&self) -> Result<Option<Found>> {
        // An empty folder named for the store is no store, and not the machine's: what a test gives.
        if self.fake.as_deref().is_some_and(source_kit::empty_folder) {
            return Ok(None);
        }
        let registry = self.registry()?;
        let client = registry.value(LAUNCHER, "InstallDir").map(|dir| source_kit::native(dir.trim()));
        let installs = installs(&registry);
        if self.fake.is_none() && client.is_none() && installs.is_empty() {
            return Ok(None);
        }
        let cache = self.caches(client.as_deref()).into_iter().find(|path| path.is_file());
        let products = match &cache {
            Some(path) => std::fs::read(path)
                .map_err(anyhow::Error::from)
                .and_then(|bytes| cache::parse(&bytes))
                .map_err(|e| eprintln!("ubisoft: {}: the owned games were not read: {e:#}", path.display()))
                .unwrap_or_default(),
            None => Vec::new(),
        };
        // cache\configuration\configurations: the art is in cache\assets.
        let assets = cache.as_deref().and_then(|path| Some(path.parent()?.parent()?.join("assets")));
        let library = Library { games: games(&products, &installs, assets.as_deref()), ..Library::default() };
        let from = match (&self.fake, client) {
            (Some(dir), _) => dir.display().to_string(),
            (None, Some(client)) => client.display().to_string(),
            (None, None) => LAUNCHER.to_string(),
        };
        Ok(Some(Found { library, from }))
    }

    fn start(&self, game: &Game) -> Option<Start> {
        id(game).map(|id| Start::Address(format!("uplay://launch/{id}")))
    }

    /// The client's own links: it shows its install dialog, or asks before it uninstalls.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        id(game).map(|id| Start::Address(format!("uplay://{}/{id}", ask.verb())))
    }
}

/// The game's id, if it is one of Ubisoft's.
fn id(game: &Game) -> Option<u32> {
    (game.source == SOURCE).then(|| game.source_id.parse().ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixture::Fixture;

    #[test]
    fn the_fake_reads_back_as_its_owned_games_installed_or_not() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ubisoft");
        let spec = Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let ubisoft = Ubisoft::at(Some(root.clone()));
        assert_eq!((ubisoft.key(), ubisoft.name()), ("ubisoft", "Ubisoft Connect"));
        let found = ubisoft.read().unwrap().expect("a fake Ubisoft Connect");
        assert_eq!(found.from, root.display().to_string());
        let games = &found.library.games;
        let seen: Vec<(&str, &str, bool)> =
            games.iter().map(|g| (g.name.as_str(), g.source_id.as_str(), g.installed)).collect();
        // Not the DLC, the copy from another store, nor the soundtrack that is Harborlight's addon. Oxbow Frontier's
        // key is named for its install_id; Kestrel Heights has no key; Driftglass has a key and no product.
        assert_eq!(
            seen,
            [
                ("Driftglass", "90610", true),
                ("Harborlight", "90210", true),
                ("Kestrel Heights", "90410", false),
                ("Oxbow Frontier", "90310", true),
            ]
        );
        assert_eq!(spec.expected(), games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
        let harborlight = &games[1];
        let folder = root.join("games").join("Harborlight");
        assert!(source_kit::same_folder(harborlight.install_dir.as_deref().unwrap(), &folder));
        let assets = root.join("client").join("cache").join("assets");
        assert_eq!(harborlight.cover.as_deref(), Some(assets.join("3f9a1c7e5b2d4f60.jpg").as_path()));
        assert_eq!(harborlight.hero, None);
        assert_eq!(games[2].install_dir, None);
    }

    #[test]
    fn games_start_install_and_uninstall_through_the_clients_links() {
        let game = Game { source: SOURCE.into(), source_id: "90310".into(), ..Game::default() };
        let ubisoft = Ubisoft::default();
        assert_eq!(ubisoft.start(&game), Some(Start::Address("uplay://launch/90310".into())));
        assert_eq!(ubisoft.ask(&game, Ask::Install), Some(Start::Address("uplay://install/90310".into())));
        assert_eq!(ubisoft.ask(&game, Ask::Uninstall), Some(Start::Address("uplay://uninstall/90310".into())));
        for other in [Game { source: "ea".into(), ..game.clone() }, Game { source_id: "x".into(), ..game }] {
            assert_eq!((ubisoft.start(&other), ubisoft.ask(&other, Ask::Install)), (None, None));
        }
    }

    #[test]
    fn with_no_cache_the_installed_games_go_by_their_folders_and_a_bad_cache_is_passed_over() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ubisoft");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let cache = CACHE.iter().fold(root.join("client"), |path, part| path.join(part));
        std::fs::write(&cache, b"\x0b not a cache").unwrap();
        let names = |ubisoft: &Ubisoft| -> Vec<(String, String)> {
            let games = ubisoft.read().unwrap().unwrap().library.games;
            games.into_iter().map(|g| (g.name, g.source_id)).collect()
        };
        let by_folder = [("Driftglass", "90610"), ("Harborlight", "90210"), ("Oxbow Frontier", "90311")];
        let by_folder: Vec<(String, String)> = by_folder.iter().map(|(n, i)| (n.to_string(), i.to_string())).collect();
        assert_eq!(names(&Ubisoft::at(Some(root.clone()))), by_folder);
        std::fs::remove_file(&cache).unwrap();
        assert_eq!(names(&Ubisoft::at(Some(root.clone()))), by_folder);
    }

    #[test]
    fn no_client_is_none_and_a_fake_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no ubisoft here");
        // An empty folder is no store, and never the machine's own.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(Ubisoft::at(Some(empty.path().to_path_buf())).read().unwrap(), None);
        let error = Ubisoft::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").starts_with(&missing.display().to_string()), "{error:#}");
        if cfg!(not(windows)) {
            assert_eq!(Ubisoft::at(None).read().unwrap(), None);
        }
    }
}
