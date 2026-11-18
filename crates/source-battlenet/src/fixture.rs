//! Writes a fake Battle.net: the registry keys and the agent's files `BattleNet` reads, with made-up products, for
//! the tests here and for the end-to-end tests' stand-in (`standin make-battlenet <dir>`), read with
//! `--battlenet <dir>`.
//!
//! At `<dir>`: `registry.json` (the registry, as `source_kit::registry` reads a file), with the client's Uninstall
//! key naming `<dir>` as its folder and each game's; the client, `Battle.net.exe`; the agent's `Agent/product.db`
//! (protobuf) and `Agent/aggregate.json`; the Agent itself, `Agent/Agent.9999/Agent.exe` (#172); each installed
//! game's folder under `games/` (or where the stand-in Agent installed it); `catalogue.json`, the products the fake
//! offers to install, which the source reads in place of its built-in table ([`crate::catalogue`]); and
//! `fixture.json`, this spec, which the stand-in Agent reads and writes back as it installs and uninstalls. Given a
//! program, it is the client and the Agent (the stand-in Battle.net and the stand-in Agent, crates/standin) and every
//! game's `game.exe`; without one those are empty files. Every name, code and path is made up.
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use source_kit::proto::Writer;
use source_kit::registry::{Registry, Values};

use crate::CLIENT_EXE;

/// The fake's registry, under its folder.
pub const REGISTRY: &str = "registry.json";

/// The products the fake offers, under its folder: what a fake reads in place of the built-in table.
pub const CATALOGUE: &str = "catalogue.json";

/// This spec, under its folder, for the stand-in Agent.
pub const SPEC: &str = "fixture.json";

/// The fake Agent's build: its program is `Agent/Agent.<build>/Agent.exe`.
pub const AGENT_BUILD: &str = "9999";

const UNINSTALL: &str = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

/// A fake Battle.net to write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    pub products: Vec<FixtureProduct>,
    /// What the catalogue offers to install (`catalogue.json`), and whether the stand-in Agent takes each.
    #[serde(default)]
    pub available: Vec<Available>,
    /// A program copied as the client, the Agent and every installed game's `game.exe`.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureProduct {
    pub uid: String,
    pub code: String,
    pub name: String,
    pub kind: Kind,
    /// Its folder, where it is not `games/<name>`: where the stand-in Agent installed it.
    #[serde(default)]
    pub dir: Option<PathBuf>,
}

/// A product the catalogue names, not installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Available {
    pub uid: String,
    pub code: String,
    pub name: String,
    /// Whether the account owns it: the stand-in Agent installs it, else refuses it as the real one refuses what it
    /// may not install.
    pub owned: bool,
}

/// What the agent's files and the registry say of a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Installed, named in `aggregate.json`, with an Uninstall key.
    Installed,
    /// Installed, with an Uninstall key, and no name in `aggregate.json`.
    Unnamed,
    /// Still downloading: in `aggregate.json` already, not installed in `product.db`.
    Downloading,
    /// One of the client's own parts, installed.
    Client,
}

/// Larkspur Hollow: offered, owned, installed by the stand-in Agent in the end-to-end test (#172).
pub const LARKSPUR: (&str, &str, &str) = ("larkspur", "lkh", "Larkspur Hollow");
/// Quarry Bell: offered, not owned, which the stand-in Agent refuses.
pub const QUARRY: (&str, &str, &str) = ("quarrybell", "qbl", "Quarry Bell");

impl Fixture {
    /// Emberwatch (`emb`) and Juniper Vault (`jnv`, named by its Uninstall key alone), installed; Glimmerfall
    /// (`glm`), still downloading; the client's agent, which `BattleNet` leaves out; and in the catalogue Larkspur
    /// Hollow (`lkh`, owned) and Quarry Bell (`qbl`, not owned), neither installed.
    pub fn standard() -> Fixture {
        let product = |uid: &str, code: &str, name: &str, kind| FixtureProduct {
            uid: uid.into(),
            code: code.into(),
            name: name.into(),
            kind,
            dir: None,
        };
        let available = |(uid, code, name): (&str, &str, &str), owned| Available {
            uid: uid.into(),
            code: code.into(),
            name: name.into(),
            owned,
        };
        Fixture {
            products: vec![
                product("emberwatch", "emb", "Emberwatch", Kind::Installed),
                product("junipervault", "jnv", "Juniper Vault", Kind::Unnamed),
                product("glimmerfall", "glm", "Glimmerfall", Kind::Downloading),
                product("agent", "agent", "Agent", Kind::Client),
            ],
            available: vec![available(LARKSPUR, true), available(QUARRY, false)],
            game_exe: None,
        }
    }

    /// The names of the games `BattleNet` reads from it, as it lists them: those installed, and the catalogue's
    /// others.
    pub fn expected(&self) -> Vec<&str> {
        let listed = |kind| matches!(kind, Kind::Installed | Kind::Unnamed);
        let installed = self.products.iter().filter(|p| listed(p.kind));
        let mut names: Vec<&str> = installed.clone().map(|p| p.name.as_str()).collect();
        let other = |a: &&Available| !installed.clone().any(|p| p.code == a.code);
        names.extend(self.available.iter().filter(other).map(|a| a.name.as_str()));
        names.sort_by_key(|name| name.to_lowercase());
        names
    }

    /// The spec at `root`, as `write` left it.
    pub fn load(root: &Path) -> io::Result<Fixture> {
        let text = std::fs::read_to_string(root.join(SPEC))?;
        serde_json::from_str(&text).map_err(io::Error::other)
    }
}

/// A path as Battle.net writes one in `product.db`: forward slashes.
fn forward(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// The folder of a product of the fake at `root`.
pub fn folder(root: &Path, product: &FixtureProduct) -> PathBuf {
    match (&product.dir, product.kind) {
        (Some(dir), _) => dir.clone(),
        (None, Kind::Client) => root.join("Agent"),
        (None, _) => root.join("games").join(&product.name),
    }
}

/// `exe` copied to `to`, or an empty file without one; left as it is where it is there already (it may be running:
/// the stand-in Agent rewrites the fake while it runs from it).
fn program(exe: Option<&Path>, to: &Path) -> io::Result<()> {
    if to.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(to.parent().unwrap_or(Path::new(".")))?;
    match exe {
        Some(exe) => std::fs::copy(exe, to).map(drop),
        None => std::fs::write(to, b""),
    }
}

/// Writes `spec` at `root`.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    let agent = root.join("Agent");
    std::fs::create_dir_all(&agent)?;
    let client = root.join(CLIENT_EXE);
    program(spec.game_exe.as_deref(), &client)?;
    program(spec.game_exe.as_deref(), &agent.join(format!("Agent.{AGENT_BUILD}")).join("Agent.exe"))?;

    let mut keys: BTreeMap<String, Values> = BTreeMap::new();
    let mut key = |name: &str, values: &[(&str, String)]| {
        let values = values.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        keys.insert(format!(r"{UNINSTALL}\{name}"), values);
    };
    key(
        "Battle.net",
        &[
            ("DisplayName", "Battle.net".into()),
            ("InstallLocation", root.display().to_string()),
            (
                "UninstallString",
                format!("\"{}\" --lang=enUS --uid=battle.net", root.join("Battle.net Uninstaller.exe").display()),
            ),
        ],
    );

    let mut database = Writer::new();
    let mut aggregate = Vec::new();
    for product in &spec.products {
        let folder = folder(&root, product);
        let installed = product.kind != Kind::Downloading;
        if matches!(product.kind, Kind::Installed | Kind::Unnamed) {
            if !folder.join("game.exe").is_file() {
                source_kit::write_game_folder(&folder, &product.name, spec.game_exe.as_deref())?;
            }
            key(
                &product.name,
                &[
                    ("DisplayName", product.name.clone()),
                    ("Publisher", "Made Up Entertainment".into()),
                    ("InstallLocation", folder.display().to_string()),
                    (
                        "UninstallString",
                        format!("\"{}\" --uid={} --lang=enUS --displaylanguage=enUS", client.display(), product.uid),
                    ),
                ],
            );
        }
        let base = Writer::new()
            .varint(1, installed.into())
            .varint(2, installed.into())
            .varint(3, installed.into())
            .text(7, "1.0.0.100");
        let install = Writer::new()
            .text(1, &product.uid)
            .text(2, &product.code)
            .message(3, Writer::new().text(1, &forward(&folder)).text(2, "eu"))
            .message(4, Writer::new().message(1, base))
            .text(6, &product.code.to_uppercase());
        database = database.message(1, install);
        if matches!(product.kind, Kind::Installed | Kind::Downloading) {
            aggregate.push(serde_json::json!({
                "product_id": product.code,
                "name": product.name,
                "icon_path": forward(&folder.join("game.exe")),
                "launch_uri": format!("battlenet://game/{}", product.code),
                "last_played_timestamp": 0,
                "icon_index": 0,
            }));
        }
    }
    std::fs::write(agent.join("product.db"), database.finish())?;
    let aggregate = serde_json::json!({ "installed": aggregate });
    let json = |value: &serde_json::Value| serde_json::to_string_pretty(value).map_err(io::Error::other);
    std::fs::write(agent.join("aggregate.json"), json(&aggregate)?)?;
    let catalogue: Vec<serde_json::Value> =
        spec.available.iter().map(|a| serde_json::json!({ "uid": a.uid, "code": a.code, "name": a.name })).collect();
    std::fs::write(root.join(CATALOGUE), json(&serde_json::Value::Array(catalogue))?)?;
    std::fs::write(root.join(SPEC), serde_json::to_string_pretty(spec).map_err(io::Error::other)?)?;
    Registry::write(&root.join(REGISTRY), &keys)
}
