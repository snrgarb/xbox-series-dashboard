//! Writes a fake Battle.net: the registry keys and the agent's files `BattleNet` reads, with made-up products, for
//! the tests here and for the end-to-end tests' stand-in (`standin make-battlenet <dir>`), read with
//! `--battlenet <dir>`.
//!
//! At `<dir>`: `registry.json` (the registry, as `source_kit::registry` reads a file), with the client's Uninstall
//! key naming `<dir>` as its folder and each game's; the client, `Battle.net.exe`; the agent's `Agent/product.db`
//! (protobuf) and `Agent/aggregate.json`; and each installed game's folder under `games/`. Given a program, it is
//! the client (the stand-in Battle.net, crates/standin) and every game's `game.exe`; without one the client is an
//! empty file. Every name, code and path is made up.
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use source_kit::proto::Writer;
use source_kit::registry::{Registry, Values};

use crate::CLIENT_EXE;

/// The fake's registry, under its folder.
pub const REGISTRY: &str = "registry.json";

const UNINSTALL: &str = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

/// A fake Battle.net to write.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub products: Vec<FixtureProduct>,
    /// A program copied as the client and as every installed game's `game.exe`.
    pub game_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct FixtureProduct {
    pub uid: String,
    pub code: String,
    pub name: String,
    pub kind: Kind,
}

/// What the agent's files and the registry say of a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl Fixture {
    /// Emberwatch (`emb`) and Juniper Vault (`jnv`, named by its Uninstall key alone), installed; Glimmerfall
    /// (`glm`), still downloading; and the client's agent, which `BattleNet` leaves out.
    pub fn standard() -> Fixture {
        let product = |uid: &str, code: &str, name: &str, kind| FixtureProduct {
            uid: uid.into(),
            code: code.into(),
            name: name.into(),
            kind,
        };
        Fixture {
            products: vec![
                product("emberwatch", "emb", "Emberwatch", Kind::Installed),
                product("junipervault", "jnv", "Juniper Vault", Kind::Unnamed),
                product("glimmerfall", "glm", "Glimmerfall", Kind::Downloading),
                product("agent", "agent", "Agent", Kind::Client),
            ],
            game_exe: None,
        }
    }

    /// The names of the games `BattleNet` reads from it, as it lists them.
    pub fn expected(&self) -> Vec<&str> {
        let listed = |kind| matches!(kind, Kind::Installed | Kind::Unnamed);
        let mut names: Vec<&str> = self.products.iter().filter(|p| listed(p.kind)).map(|p| p.name.as_str()).collect();
        names.sort_by_key(|name| name.to_lowercase());
        names
    }
}

/// A path as Battle.net writes one in `product.db`: forward slashes.
fn forward(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Writes `spec` at `root`.
pub fn write(root: &Path, spec: &Fixture) -> io::Result<()> {
    let root = std::path::absolute(root)?;
    let agent = root.join("Agent");
    std::fs::create_dir_all(&agent)?;
    let client = root.join(CLIENT_EXE);
    match &spec.game_exe {
        Some(exe) => std::fs::copy(exe, &client).map(drop)?,
        None => std::fs::write(&client, b"")?,
    }

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
        let folder = match product.kind {
            Kind::Client => agent.clone(),
            _ => root.join("games").join(&product.name),
        };
        let installed = product.kind != Kind::Downloading;
        if matches!(product.kind, Kind::Installed | Kind::Unnamed) {
            source_kit::write_game_folder(&folder, &product.name, spec.game_exe.as_deref())?;
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
    std::fs::write(agent.join("aggregate.json"), serde_json::to_string_pretty(&aggregate).map_err(io::Error::other)?)?;
    Registry::write(&root.join(REGISTRY), &keys)
}
