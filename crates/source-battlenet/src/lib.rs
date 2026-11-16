//! Battle.net's installed games (docs/research/stores.md, "Battle.net"; docs/sources.md).
//!
//! - **Installed:** `%PROGRAMDATA%\Battle.net\Agent\product.db` (`product`): each product its agent says is
//!   installed (`base_product_state.installed`), at its `install_path`, that is not one of the client's own parts
//!   and whose folder is there. A game is its product code, its `source_id`: `emb`.
//! - **Names:** `aggregate.json` beside it (`installed[].product_id` and `name`); else the game's Uninstall key's
//!   `DisplayName`; else its uid.
//! - **Owned but not installed:** none. No local file lists them without Blizzard's servers.
//! - **The client:** the Uninstall key whose `UninstallString` has `-uid=battle.net` in it: its `InstallLocation`
//!   holds `Battle.net.exe`. With neither it nor `product.db`, Battle.net is not installed.
//! - **Start:** `Battle.net.exe --exec="launch <code>"`. Whether the client starts the game on that, or only shows
//!   it, and whether it must be up first, only a signed-in PC can say (the research's open question).
//! - **Install:** `Battle.net.exe --game=<uid>`, which opens the game's page there. **Uninstall:** the game's
//!   Uninstall key's own `UninstallString` (the one with `--uid=<uid>` in it), Blizzard's uninstaller.
//!
//! `BattleNet::at(Some(dir))` reads a fake Battle.net instead (`--battlenet <dir>`, and `fixture`): its registry is
//! `<dir>/registry.json` (`source_kit::registry`), its agent's files are in `<dir>/Agent`.
pub mod fixture;
pub mod product;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use model::{Ask, Found, Game, Library, Source, Start};
use source_kit::registry::{self, Registry, Values, UNINSTALL};

/// Battle.net, as a game's `source` names it.
pub const SOURCE: &str = "battlenet";

/// The client's program, in its folder.
pub const CLIENT_EXE: &str = "Battle.net.exe";

/// Battle.net, as the library reads it and the shell starts its games.
#[derive(Debug, Clone, Default)]
pub struct BattleNet {
    /// A fake Battle.net to read in place of this machine's.
    fake: Option<PathBuf>,
}

impl BattleNet {
    /// This machine's Battle.net, or with `Some(dir)` the fake one at `dir` (`--battlenet`).
    pub fn at(fake: Option<PathBuf>) -> Self {
        Self { fake }
    }

    fn registry(&self) -> Result<Registry> {
        match &self.fake {
            Some(dir) => Registry::load(&dir.join(fixture::REGISTRY)).with_context(|| dir.display().to_string()),
            None => Ok(Registry::Windows),
        }
    }

    /// The agent's folder, where `product.db` and `aggregate.json` are.
    fn agent(&self) -> Option<PathBuf> {
        match &self.fake {
            Some(dir) => Some(dir.join("Agent")),
            None if cfg!(windows) => {
                std::env::var_os("PROGRAMDATA").map(|data| PathBuf::from(data).join("Battle.net").join("Agent"))
            }
            None => None,
        }
    }

    /// Every product in `product.db`; none when there is no such file.
    fn products(&self) -> Result<Vec<product::Product>> {
        let Some(path) = self.agent().map(|agent| agent.join("product.db")).filter(|path| path.is_file()) else {
            return Ok(Vec::new());
        };
        let bytes = std::fs::read(&path).with_context(|| path.display().to_string())?;
        product::parse(&bytes).with_context(|| path.display().to_string())
    }

    /// The product whose code is the game's `source_id`.
    fn product(&self, game: &Game) -> Option<product::Product> {
        let code = code(game)?;
        self.products().ok()?.into_iter().find(|product| product.code == code)
    }

    /// The client's program, when it is there.
    fn client(&self) -> Option<PathBuf> {
        client(&self.registry().ok()?)
    }
}

/// `Battle.net.exe` in the folder of the Uninstall key that uninstalls the client, when it is there.
fn client(registry: &Registry) -> Option<PathBuf> {
    UNINSTALL.iter().flat_map(|key| registry.subkeys(key)).find_map(|(_, values)| {
        let uninstall = registry::get(&values, "UninstallString")?.to_ascii_lowercase();
        if !uninstall.contains("-uid=battle.net") {
            return None;
        }
        let exe =
            source_kit::native(registry::get(&values, "InstallLocation")?.trim().trim_matches('"')).join(CLIENT_EXE);
        exe.is_file().then_some(exe)
    })
}

/// The games' Uninstall keys, by the uid in their `UninstallString` (`Battle.net.exe ... --uid=<uid> ...`).
fn uninstall_keys(registry: &Registry) -> HashMap<String, Values> {
    let mut keys = HashMap::new();
    for (_, values) in UNINSTALL.iter().flat_map(|key| registry.subkeys(key)) {
        let Some(line) = registry::get(&values, "UninstallString") else { continue };
        let lower = line.to_ascii_lowercase();
        let Some(at) = lower.find("battle.net") else { continue };
        let Some(uid) = lower[at..].find("--uid=").map(|i| &line[at + i + "--uid=".len()..]) else { continue };
        let uid = uid.split(|c: char| c.is_whitespace() || c == '"').next().unwrap_or("");
        if !uid.is_empty() && !product::clients_own(uid) {
            keys.insert(uid.to_lowercase(), values);
        }
    }
    keys
}

/// `aggregate.json`'s names, by product code.
fn names(agent: &Path) -> HashMap<String, String> {
    let path = agent.join("aggregate.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return HashMap::new() };
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("battlenet: {}: no names read: {e}", path.display());
            return HashMap::new();
        }
    };
    let installed = json.get("installed").and_then(|i| i.as_array()).map_or(&[][..], Vec::as_slice);
    installed
        .iter()
        .filter_map(|entry| {
            let code = entry.get("product_id")?.as_str()?;
            let name = entry.get("name")?.as_str()?.trim();
            (!name.is_empty()).then(|| (code.to_string(), name.to_string()))
        })
        .collect()
}

impl Source for BattleNet {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "Battle.net"
    }

    fn read(&self) -> Result<Option<Found>> {
        let registry = self.registry()?;
        let client = client(&registry);
        let agent = self.agent();
        let database = agent.as_ref().map(|agent| agent.join("product.db")).filter(|db| db.is_file());
        if self.fake.is_none() && client.is_none() && database.is_none() {
            return Ok(None);
        }
        let products = self.products()?;
        let names = agent.as_deref().map(names).unwrap_or_default();
        let keys = uninstall_keys(&registry);
        let mut games: Vec<Game> = Vec::new();
        for product in products.iter().filter(|p| p.installed && !product::clients_own(&p.uid)) {
            let folder = source_kit::native(product.install_path.trim());
            if product.install_path.trim().is_empty() || !folder.is_dir() {
                continue;
            }
            if games.iter().any(|game| game.source_id == product.code) {
                continue;
            }
            let key = keys.get(&product.uid.to_lowercase());
            let display =
                key.and_then(|values| registry::get(values, "DisplayName")).map(|name| name.trim().to_string());
            let name = names.get(&product.code).cloned().or(display).unwrap_or_else(|| product.uid.clone());
            games.push(Game {
                source: SOURCE.into(),
                source_id: product.code.clone(),
                name,
                installed: true,
                install_dir: Some(folder),
                ..Game::default()
            });
        }
        games.sort_by_key(|game| game.name.to_lowercase());
        let from = match (&self.fake, database) {
            (Some(dir), _) => dir.display().to_string(),
            (None, Some(db)) => db.parent().map_or(String::new(), |agent| agent.display().to_string()),
            (None, None) => client.map_or(String::new(), |exe| exe.display().to_string()),
        };
        Ok(Some(Found { library: Library { games, ..Library::default() }, from }))
    }

    /// `Battle.net.exe --exec="launch <code>"`.
    fn start(&self, game: &Game) -> Option<Start> {
        let code = code(game)?;
        let path = self.client()?;
        Some(Start::Program { path, args: vec![format!("--exec=launch {code}")], dir: None })
    }

    /// Install: the game's page in the client, `--game=<uid>`. Uninstall: its Uninstall key's own command.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        let uid = self.product(game)?.uid;
        match ask {
            Ask::Install => {
                Some(Start::Program { path: self.client()?, args: vec![format!("--game={uid}")], dir: None })
            }
            Ask::Uninstall => {
                let keys = uninstall_keys(&self.registry().ok()?);
                source_kit::command::program(registry::get(keys.get(&uid.to_lowercase())?, "UninstallString")?)
            }
        }
    }
}

/// The game's product code, if it is one of Battle.net's.
fn code(game: &Game) -> Option<&str> {
    (game.source == SOURCE && !game.source_id.is_empty()).then_some(game.source_id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixture::Fixture;

    #[test]
    fn the_fake_reads_back_as_its_installed_games() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        let spec = Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let battlenet = BattleNet::at(Some(root.clone()));
        assert_eq!((battlenet.key(), battlenet.name()), ("battlenet", "Battle.net"));
        let found = battlenet.read().unwrap().expect("a fake Battle.net");
        assert_eq!(found.from, root.display().to_string());
        let games = &found.library.games;
        let seen: Vec<(&str, &str)> = games.iter().map(|g| (g.name.as_str(), g.source_id.as_str())).collect();
        // Not the one still downloading, nor the client's own agent. Juniper Vault has no name in aggregate.json:
        // its Uninstall key's.
        assert_eq!(seen, [("Emberwatch", "emb"), ("Juniper Vault", "jnv")]);
        assert_eq!(spec.expected(), games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
        assert!(games.iter().all(|g| g.installed && g.install_dir.as_ref().is_some_and(|d| d.is_dir())));
    }

    #[test]
    fn games_start_and_are_installed_through_the_client_and_uninstalled_by_their_keys() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        let mut spec = Fixture::standard();
        let exe = dir.path().join("stand-in.exe");
        std::fs::write(&exe, b"").unwrap();
        spec.game_exe = Some(exe);
        fixture::write(&root, &spec).unwrap();
        let battlenet = BattleNet::at(Some(root.clone()));
        let games = battlenet.read().unwrap().unwrap().library.games;
        let emberwatch = &games[0];
        let client = root.join(CLIENT_EXE);
        let program = |args: &[&str]| Start::Program {
            path: client.clone(),
            args: args.iter().map(|a| a.to_string()).collect(),
            dir: None,
        };
        assert_eq!(battlenet.start(emberwatch), Some(program(&["--exec=launch emb"])));
        assert_eq!(battlenet.start(emberwatch).unwrap().parameters().as_deref(), Some("\"--exec=launch emb\""));
        assert_eq!(battlenet.ask(emberwatch, Ask::Install), Some(program(&["--game=emberwatch"])));
        let uninstall = battlenet.ask(emberwatch, Ask::Uninstall).expect("its key's UninstallString");
        assert_eq!(uninstall.target(), client.display().to_string());
        assert_eq!(uninstall.parameters().as_deref(), Some("--uid=emberwatch --lang=enUS --displaylanguage=enUS"));

        // No client: nothing to start the game with.
        std::fs::remove_file(&client).unwrap();
        assert_eq!(battlenet.start(emberwatch), None);
        let other = Game { source: "ubisoft".into(), ..emberwatch.clone() };
        assert_eq!((battlenet.start(&other), battlenet.ask(&other, Ask::Uninstall)), (None, None));
    }

    #[test]
    fn no_battlenet_is_none_and_a_fake_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no battle.net here");
        let error = BattleNet::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").starts_with(&missing.display().to_string()), "{error:#}");
        if cfg!(not(windows)) {
            assert_eq!(BattleNet::at(None).read().unwrap(), None);
        }
    }
}
