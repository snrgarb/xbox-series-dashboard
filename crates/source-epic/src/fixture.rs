//! Writes a fake Epic Games Launcher's files: the old launcher's manifests, its install list and its catalogue
//! cache, as `load` reads them, with made-up games installed under the same folder. Test support, for the unit tests
//! here and for the end-to-end tests' stand-in launcher (`crates/standin`), which writes it on a test VM that has no
//! Epic and is registered there for the launcher's links.
//!
//! The folder is laid out as `Places::under` reads it: `<root>` is `%ProgramData%\Epic`, and the games are installed
//! under `<root>\Games`. Besides the games it writes a piece of DLC and an Unreal Engine plugin, which `load` leaves
//! out. No art: the launcher caches none.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::Places;

/// Every installed game's program, relative to its folder: what the manifest's `LaunchExecutable` says.
pub const EXE: &str = "Binaries/Win64/game.exe";

/// A fake launcher to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub games: Vec<FixtureGame>,
    /// A program copied into every installed item's folder as its `EXE`. Without one the folders are empty.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FixtureGame {
    /// `AppName`, the game's `source_id`.
    pub app_name: String,
    pub namespace: String,
    pub item_id: String,
    pub name: String,
    pub kind: Kind,
    /// With a manifest and a folder; else only in the catalogue cache, which is owned and not installed.
    pub installed: bool,
}

/// What an item is, which only a game is as `load` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Game,
    /// `AppCategories` has `addons`.
    Dlc,
    /// Unreal Engine content: `AppCategories` has `plugins`.
    Plugin,
}

impl FixtureGame {
    /// Where an installed item is: `<root>\Games\<name>`.
    pub fn install_dir(&self, root: &Path) -> PathBuf {
        root.join("Games").join(&self.name)
    }

    /// Its program.
    pub fn exe(&self, root: &Path) -> PathBuf {
        EXE.split('/').fold(self.install_dir(root), |path, part| path.join(part))
    }
}

impl Fixture {
    /// Three made-up games of the account's, and two items `load` leaves out:
    ///
    /// | item                     | installed | what            |
    /// |--------------------------|-----------|-----------------|
    /// | Kiteborne                | yes       | a game          |
    /// | Rusted Vigil             | yes       | a game          |
    /// | Wexford Hollow           | no        | a game, owned   |
    /// | Kiteborne: Night Market  | yes       | its DLC         |
    /// | Lantern Shader Pack      | yes       | an engine plugin|
    ///
    /// Every name starts with a letter no game of Steam's fixture has in it (K, R, W), so a test finds each by
    /// Search with one key.
    pub fn standard() -> Fixture {
        let games = [
            ("Kiteborne", Kind::Game, true),
            ("Rusted Vigil", Kind::Game, true),
            ("Wexford Hollow", Kind::Game, false),
            ("Kiteborne: Night Market", Kind::Dlc, true),
            ("Lantern Shader Pack", Kind::Plugin, true),
        ];
        let games = games.iter().enumerate().map(|(i, &(name, kind, installed))| FixtureGame {
            app_name: hex(i as u64 * 3 + 1),
            namespace: hex(i as u64 * 3 + 2),
            item_id: hex(i as u64 * 3 + 3),
            name: name.to_string(),
            kind,
            installed,
        });
        Fixture { games: games.collect(), game_exe: None }
    }
}

/// Thirty-two hex digits, as Epic's ids are: not real ones, only different for every `n`.
fn hex(n: u64) -> String {
    let a = (n + 0x51).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{a:016x}{:016x}", a.rotate_left(29) ^ 0x00C0_FFEE)
}

/// Writes `spec` as the launcher's files at `root`, making the folders. Files already there are written over.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    let places = Places::under(&root);
    let manifests = places.manifests();
    fs::create_dir_all(&manifests)?;
    let mut installed = Vec::new();
    for game in spec.games.iter().filter(|game| game.installed) {
        // DLC goes in its game's folder, as Epic installs it.
        let owner = spec.games.iter().find(|other| other.kind == Kind::Game && game.name.starts_with(&other.name));
        let dir = match (game.kind, owner) {
            (Kind::Dlc, Some(owner)) => owner.install_dir(&root),
            _ => game.install_dir(&root),
        };
        let guid = game.app_name.to_uppercase();
        fs::write(manifests.join(format!("{guid}.item")), manifest(game, &dir, &guid).to_string())?;
        installed.push(json!({ "InstallLocation": dir, "AppName": game.app_name, "AppVersion": "1.0.0" }));
        if game.kind == Kind::Game {
            let exe = game.exe(&root);
            fs::create_dir_all(exe.parent().expect("a program is in a folder"))?;
            if let Some(from) = &spec.game_exe {
                fs::copy(from, &exe)?;
            }
        }
    }
    let list = places.launcher_installed();
    fs::create_dir_all(list.parent().expect("the install list is in a folder"))?;
    fs::write(list, json!({ "InstallationList": installed }).to_string())?;
    let catalogue: Vec<Value> = spec.games.iter().map(catalogue_item).collect();
    let catcache = places.catcache();
    fs::create_dir_all(catcache.parent().expect("the catalogue cache is in a folder"))?;
    fs::write(catcache, crate::to_base64(Value::Array(catalogue).to_string().as_bytes()))
}

/// An installed item's manifest, as the sample in docs/research/stores.md.
fn manifest(game: &FixtureGame, dir: &Path, guid: &str) -> Value {
    let categories: &[&str] = match game.kind {
        Kind::Game => &["public", "games", "applications"],
        Kind::Dlc => &["public", "addons"],
        Kind::Plugin => &["public", "plugins", "plugins/engine"],
    };
    json!({
        "FormatVersion": 0,
        "bIsApplication": game.kind == Kind::Game,
        "bIsExecutable": game.kind == Kind::Game,
        "LaunchExecutable": if game.kind == Kind::Game { EXE } else { "" },
        "LaunchCommand": "",
        "DisplayName": game.name,
        "AppName": game.app_name,
        "CatalogNamespace": game.namespace,
        "CatalogItemId": game.item_id,
        "AppCategories": categories,
        "InstallLocation": dir,
        "InstallationGuid": guid,
        "MainGameAppName": game.app_name,
        "MainWindowProcessName": "",
        "ProcessNames": [],
        "TechnicalType": "",
    })
}

/// The item in the catalogue cache. It names no pictures (`keyImages`): an address the art lookup cannot reach is a
/// failed request, tried again every run, and a test's stand-ins serve none of Epic's CDN. Reading `keyImages` is
/// `key_images`' own test.
fn catalogue_item(game: &FixtureGame) -> Value {
    let categories = match game.kind {
        Kind::Game => json!([{ "path": "games" }, { "path": "applications" }]),
        Kind::Dlc => json!([{ "path": "addons" }]),
        Kind::Plugin => json!([{ "path": "plugins" }]),
    };
    json!({
        "id": game.item_id,
        "namespace": game.namespace,
        "title": game.name,
        "categories": categories,
        "releaseInfo": [{ "appId": game.app_name, "dateAdded": "2025-03-02T10:00:00.000Z" }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_fixture_reads_back_with_its_program_in_its_installed_games() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("standin");
        fs::write(&exe, b"not really a program").unwrap();
        let mut spec = Fixture::standard();
        spec.game_exe = Some(exe);
        let root = dir.path().join("epic");
        write(&root, &spec).unwrap();

        let library = crate::load(&Places::under(&root)).unwrap();
        let said: Vec<(&str, bool)> = library.games.iter().map(|g| (g.name.as_str(), g.installed)).collect();
        assert_eq!(said, [("Kiteborne", true), ("Rusted Vigil", true), ("Wexford Hollow", false)]);
        for (game, fake) in library.games.iter().zip(&spec.games) {
            assert_eq!(game.source_id, fake.app_name);
        }
        let kite = &spec.games[0];
        assert_eq!(library.games[0].install_dir.as_deref(), Some(kite.install_dir(&root).as_path()));
        assert_eq!(fs::read(kite.exe(&root)).unwrap(), b"not really a program");
        assert_eq!(crate::executable(&Places::under(&root), &kite.app_name), Some(kite.exe(&root)));
        assert!(!spec.games[2].install_dir(&root).exists());
        // Written again over itself, it reads the same.
        write(&root, &spec).unwrap();
        assert_eq!(crate::load(&Places::under(&root)).unwrap(), library);
    }

    #[test]
    fn ids_are_thirty_two_hex_digits_and_all_different() {
        let spec = Fixture::standard();
        let ids: Vec<&String> = spec.games.iter().flat_map(|g| [&g.app_name, &g.namespace, &g.item_id]).collect();
        assert!(ids.iter().all(|id| id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit())));
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
    }
}
