//! Writes a fake Ubisoft Connect: the registry keys and the cache `Ubisoft` reads, with made-up products, for the
//! tests here and for the end-to-end tests' stand-in (`standin make-ubisoft <dir>`), read with `--ubisoft <dir>`.
//!
//! At `<dir>`: `registry.json` (the registry, as `source_kit::registry` reads a file); the client's folder,
//! `client/`, with its cache, `cache/configuration/configurations` (protobuf around YAML, as the client writes
//! it) and a cover in `cache/assets/`; and each installed game's folder under `games/`, with, given a program,
//! that program as `game.exe`. Every name, id and path is made up.
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use source_kit::proto::Writer;
use source_kit::registry::{Registry, Values};

use crate::LAUNCHER;

/// The fake's registry, under its folder.
pub const REGISTRY: &str = "registry.json";

/// Harborlight's cover in the client's art cache.
const COVER: &str = "3f9a1c7e5b2d4f60.jpg";

/// A fake Ubisoft Connect to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub products: Vec<FixtureProduct>,
    /// A program copied into every installed game's folder as `game.exe`. Without one the folders have none.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FixtureProduct {
    pub uplay_id: u32,
    pub install_id: u32,
    pub name: String,
    pub kind: Kind,
}

/// What the cache and the registry say of a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A game, installed: in the cache, with an `Installs` key named for its `install_id`.
    Installed,
    /// A game in the cache with no `Installs` key: owned, not installed.
    Owned,
    /// DLC (`is_ulc`).
    Dlc,
    /// A copy from another store (`third_party_platform`).
    ThirdParty,
    /// An addon of the product with this uplay_id, though it says it is a game of its own.
    AddonOf(u32),
    /// Nothing to start (`start_game` missing): a tool.
    NoStart,
    /// An `Installs` key and its folder, with nothing of it in the cache.
    KeyOnly,
}

impl Fixture {
    /// Harborlight (90210) and Oxbow Frontier (90310, its key named for its install_id 90311), installed; Kestrel
    /// Heights (90410), owned and not installed; Driftglass (90610), an installed game the cache does not know. And
    /// four products `Ubisoft` leaves out: Harborlight's DLC, its soundtrack (one of its addons), a copy of a game
    /// from another store, and a tool with nothing to start.
    pub fn standard() -> Fixture {
        let product =
            |uplay_id, install_id, name: &str, kind| FixtureProduct { uplay_id, install_id, name: name.into(), kind };
        Fixture {
            products: vec![
                product(90210, 90210, "Harborlight", Kind::Installed),
                product(90211, 90211, "Harborlight: Tidewater Pack", Kind::Dlc),
                product(90212, 90212, "Harborlight Original Soundtrack", Kind::AddonOf(90210)),
                product(90310, 90311, "Oxbow Frontier", Kind::Installed),
                product(90410, 90410, "Kestrel Heights", Kind::Owned),
                product(90510, 90510, "Gravel Saints", Kind::ThirdParty),
                product(90610, 90610, "Driftglass", Kind::KeyOnly),
                product(90710, 90710, "Made Up Benchmark Tool", Kind::NoStart),
            ],
            game_exe: None,
        }
    }

    /// The names of the games `Ubisoft` reads from it, as it lists them.
    pub fn expected(&self) -> Vec<&str> {
        let listed = |kind| matches!(kind, Kind::Installed | Kind::Owned | Kind::KeyOnly);
        let mut names: Vec<&str> = self.products.iter().filter(|p| listed(p.kind)).map(|p| p.name.as_str()).collect();
        names.sort_by_key(|name| name.to_lowercase());
        names
    }
}

/// A path as Ubisoft Connect writes one in the registry: forward slashes, one at the end.
fn forward(path: &Path) -> String {
    format!("{}/", path.display().to_string().replace('\\', "/").trim_end_matches('/'))
}

/// Writes `spec` at `root`.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    let client = root.join("client");
    let assets = client.join("cache").join("assets");
    let configuration = client.join("cache").join("configuration");
    for dir in [&assets, &configuration] {
        std::fs::create_dir_all(dir)?;
    }
    let mut keys: BTreeMap<String, Values> = BTreeMap::new();
    let install_dir = |path: &Path| Values::from([("InstallDir".to_string(), forward(path))]);
    keys.insert(LAUNCHER.to_string(), install_dir(&client));

    let mut cache = Writer::new();
    for product in &spec.products {
        if matches!(product.kind, Kind::Installed | Kind::KeyOnly) {
            let folder = root.join("games").join(&product.name);
            source_kit::write_game_folder(&folder, &product.name, spec.game_exe.as_deref())?;
            let key = if product.kind == Kind::KeyOnly { product.uplay_id } else { product.install_id };
            keys.insert(format!(r"{LAUNCHER}\Installs\{key}"), install_dir(&folder));
        }
        if product.kind != Kind::KeyOnly {
            let entry = Writer::new()
                .varint(1, product.uplay_id.into())
                .varint(2, product.install_id.into())
                .text(3, &game_info(product, spec));
            cache = cache.message(1, entry);
        }
    }
    std::fs::write(configuration.join("configurations"), cache.finish())?;
    // Generated, as the Steam fixture's art is: a few bytes are enough to be a file the source finds.
    std::fs::write(assets.join(COVER), b"not a real picture")?;
    Registry::write(&root.join(REGISTRY), &keys)
}

/// A product's configuration, as the client caches it: its name and cover by the names of strings in
/// `localizations`, as most are, except for a game whose name is written in place.
fn game_info(product: &FixtureProduct, spec: &Fixture) -> String {
    let addons: Vec<String> = spec
        .products
        .iter()
        .filter(|p| p.kind == Kind::AddonOf(product.uplay_id))
        .map(|p| format!("    - id: {}\n      name: {}\n", p.uplay_id, p.name))
        .collect();
    let mut root = String::new();
    if product.name == "Oxbow Frontier" {
        root.push_str(&format!("  name: \"{}\"\n", product.name));
    } else {
        root.push_str("  name: l1\n  thumb_image: t1\n  background_image: b1\n");
    }
    root.push_str(&format!("  is_ulc: {}\n", product.kind == Kind::Dlc));
    if product.kind == Kind::ThirdParty {
        root.push_str("  third_party_platform:\n    name: made-up store\n");
    }
    if !addons.is_empty() {
        root.push_str(&format!("  addons:\n{}", addons.concat()));
    }
    if product.kind != Kind::NoStart {
        root.push_str(&format!(
            "  start_game:\n    online:\n      executables:\n      - path:\n          relative: game.exe\n        working_directory:\n          register: HKEY_LOCAL_MACHINE\\SOFTWARE\\Ubisoft\\Launcher\\Installs\\{}\\InstallDir\n",
            product.install_id
        ));
    }
    let cover = if product.name == "Harborlight" { COVER } else { "0000000000000000.jpg" };
    format!(
        "version: 2.0\nroot:\n{root}localizations:\n  default:\n    l1: \"{}\"\n    t1: {cover}\n    b1: 8c2e4a6f0b1d3e5f.jpg\n",
        product.name.replace('"', "\\\"")
    )
}
