//! Battle.net's games (docs/research/stores.md, "Battle.net"; docs/research/installs.md 7; docs/sources.md).
//!
//! - **Installed:** `%PROGRAMDATA%\Battle.net\Agent\product.db` (`product`): each product its agent says is
//!   installed (`base_product_state.installed`), at its `install_path`, that is not one of the client's own parts
//!   and whose folder is there. A game is its product code, its `source_id`: `emb`.
//! - **Names:** `aggregate.json` beside it (`installed[].product_id` and `name`); else the catalogue's; else the
//!   game's Uninstall key's `DisplayName` (the Agent writes the folder's name there, `Tony Hawks Pro Skater 3 + 4`,
//!   with no apostrophe); else its uid.
//! - **Uids** (#172): the client knows each game by a uid of its own, which an install must use, or the client shows
//!   Install for a game that is there and will not start it. Which it is: `catalogue` and [`resolve`]; a game
//!   `product.db` has under another uid is re-registered under the client's while Battle.net is closed
//!   (`BattleNet::reregister`), and until then Play says what to do instead of opening Battle.net (`cant_start`).
//! - **Not installed:** the products of the catalogue (`catalogue`, a built-in table; a fake's `catalogue.json`),
//!   while the client is installed, as available to install. Whether the account owns one is not known until the
//!   Agent answers an install of it (#172).
//! - **The client:** the Uninstall key whose `UninstallString` has `-uid=battle.net` in it: its `InstallLocation`
//!   holds `Battle.net.exe`. With neither it nor `product.db`, Battle.net is not installed.
//! - **Start:** `Battle.net.exe --exec="launch <code>"`, the catalogue's launch code (`Pro`, `Fen`) where it has
//!   one, else the product code.
//! - **Install, uninstall, progress** (#172): through the client's own Agent, its local HTTP API (`agent`), with no
//!   window: install into a folder (the Agent's form filled in: `agent::calls`), the Queue's bytes and speed,
//!   uninstall. Pause and resume only with `DASHBOARD_BATTLENET_STEER=1` (no tool knows them); Cancel also while the
//!   Agent is the Dashboard's, by stopping it. No Agent is started beside Battle.net's own, and one the Dashboard
//!   started is stopped once the install it took is over, never during it (`agent`, `installing`).
//!   Where the Agent does not answer or take it, as before: `Battle.net.exe --game=<uid>`, the game's page in the
//!   client; uninstall by the game's Uninstall key's own `UninstallString`, Blizzard's uninstaller. Each fallback
//!   says why in the log.
//!
//! `BattleNet::at(Some(dir))` reads a fake Battle.net instead (`--battlenet <dir>`, and `fixture`): its registry is
//! `<dir>/registry.json` (`source_kit::registry`), its agent's files are in `<dir>/Agent`, and its Agent is the
//! stand-in (`standin`), found only by `<dir>/Agent/Agent.dat`.
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod agent;
pub mod catalogue;
pub mod client;
pub mod fixture;
pub mod http;
pub mod installing;
pub mod listening;
pub mod product;
pub mod standin;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use model::{Ask, Can, Folder, Found, Game, Install, Installer, Library, Opener, Refusal, Source, Start, Steer};
use source_kit::registry::{self, Registry, Values, UNINSTALL};

/// Battle.net, as a game's `source` names it.
pub const SOURCE: &str = "battlenet";

/// The client's program, in its folder.
pub const CLIENT_EXE: &str = "Battle.net.exe";

/// A line of the log: `battlenet: <line>`.
pub fn log(line: &str) {
    eprintln!("battlenet: {line}");
}

/// Whether `path` is under `folder`, by either kind of slash and in any case, as Windows compares paths.
pub fn under(folder: &Path, path: &Path) -> bool {
    let plain = |p: &Path| p.to_string_lossy().replace('\\', "/").to_lowercase();
    let (folder, path) = (plain(folder), plain(path));
    path.strip_prefix(folder.trim_end_matches('/')).is_some_and(|rest| rest.starts_with('/'))
}

/// Battle.net, as the library reads it and the shell starts its games.
#[derive(Debug, Clone, Default)]
pub struct BattleNet {
    /// A fake Battle.net to read in place of this machine's.
    fake: Option<PathBuf>,
    /// The installs asked of the Agent, followed for the Queue.
    jobs: installing::Jobs,
    /// Whether pause, resume and cancel are asked of the Agent (`agent::calls::steer_on`).
    steer: bool,
    /// Whether an install stops before the download, its form filled in and abandoned
    /// (`agent::calls::no_download`).
    no_download: bool,
}

impl BattleNet {
    /// This machine's Battle.net, or with `Some(dir)` the fake one at `dir` (`--battlenet`).
    pub fn at(fake: Option<PathBuf>) -> Self {
        Self {
            fake,
            jobs: installing::Jobs::default(),
            steer: agent::calls::steer_on(),
            no_download: agent::calls::no_download(),
        }
    }

    /// With pause, resume and cancel asked of the Agent, or not, whatever the environment says.
    /// Stopping every install before its download, or not, whatever the environment says.
    pub fn with_no_download(mut self, no_download: bool) -> Self {
        self.no_download = no_download;
        self
    }

    pub fn with_steer(mut self, steer: bool) -> Self {
        self.steer = steer;
        self
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

    /// The Agent, where its program is.
    pub fn place(&self) -> Option<agent::Place> {
        let place = agent::Place { dir: self.agent()?, fake: self.fake.is_some() };
        place.there().then_some(place)
    }

    /// Every product in `product.db`; none when there is no such file.
    fn products(&self) -> Result<Vec<product::Product>> {
        products_in(self.agent().as_deref())
    }

    /// The products the Dashboard can ask for: a fake's own, else the built-in table.
    fn catalogue(&self) -> Vec<catalogue::Product> {
        match &self.fake {
            Some(dir) => catalogue::of_fake(dir),
            None => catalogue::built_in(),
        }
    }

    /// The client's `Battle.net.config`: a fake's in its folder, else `%APPDATA%\Battle.net\Battle.net.config`.
    fn config_path(&self) -> Option<PathBuf> {
        match &self.fake {
            Some(dir) => Some(dir.join(fixture::CONFIG)),
            None => Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Battle.net").join("Battle.net.config")),
        }
    }

    /// The uids the client's `Battle.net.config` names in `Games`.
    fn client_uids(&self) -> Vec<String> {
        self.config_path().and_then(|path| client::config(&path)).map(|c| client::games(&c)).unwrap_or_default()
    }

    /// The account's country from the client's cache, a fake's in its folder, else `%LOCALAPPDATA%\Battle.net\
    /// CachedData.db`; why not, as the log says it.
    fn country(&self) -> Result<client::Country, String> {
        let path = match &self.fake {
            Some(dir) => dir.join(fixture::CACHE),
            None => PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("no LOCALAPPDATA")?)
                .join("Battle.net")
                .join("CachedData.db"),
        };
        match client::country(&path) {
            Ok(Some(country)) => Ok(country),
            Ok(None) => Err(format!("{} has none", path.display())),
            Err(e) => Err(format!("{}: {e:#}", path.display())),
        }
    }

    /// What the game is to the Agent and the client (#172): its uid, the client's where that is known (the
    /// catalogue's row verified, or confirmed on this PC: [`resolve`]), else `product.db`'s; and its install in
    /// `product.db`, under that uid, else under another (`stale`: the TV PC's `libra`).
    fn target(&self, game: &Game) -> Option<Target> {
        let code = code(game)?;
        let products = self.products().unwrap_or_default();
        let listed = self.catalogue().into_iter().find(|p| p.code.eq_ignore_ascii_case(code));
        let mine: Vec<&product::Product> = products.iter().filter(|p| p.code.eq_ignore_ascii_case(code)).collect();
        let resolved = listed.as_ref().map(|listed| resolve(listed, &self.client_uids()));
        // The uid: the client's where that is known; else the one product.db has it under (written by the client's
        // own Agent, or an earlier install); else the table's, not verified.
        let (uid, verified) = match (&resolved, mine.first()) {
            (Some(Resolved { uid, verified: Some(how) }), _) => (uid.clone(), Some(how.clone())),
            (_, Some(product)) => (product.uid.clone(), Some("product.db has it under it".to_string())),
            (Some(Resolved { uid, verified: None }), None) => (uid.clone(), None),
            (None, None) => return None,
        };
        let under = mine.iter().find(|p| p.uid == uid).copied();
        let other = mine.iter().find(|p| p.uid != uid && !p.install_path.trim().is_empty()).copied();
        let record = under.or(other);
        let folder = record.map(|p| source_kit::native(p.install_path.trim()));
        let has_path = record.is_some_and(|p| !p.install_path.trim().is_empty());
        let installed = record.is_some_and(|p| p.installed) && has_path && folder.as_ref().is_some_and(|f| f.is_dir());
        let partial = record.is_some_and(|p| !p.installed) && has_path;
        Some(Target {
            code: record
                .map_or_else(|| listed.as_ref().map_or(code.to_string(), |l| l.code.clone()), |p| p.code.clone()),
            uid,
            verified,
            name: listed.map_or_else(|| game.name.clone(), |p| p.name),
            installed: installed.then(|| folder.clone()).flatten(),
            partial: partial.then_some(folder).flatten(),
            stale: under.is_none().then(|| other.map(|p| p.uid.clone())).flatten(),
        })
    }

    /// Re-registers, under the uid the client knows it by, each game `product.db` has installed under another
    /// (#172: the TV PC's Tony Hawk's Pro Skater 3 + 4, installed by the Dashboard as `libra`, which the client keeps
    /// as `lbra`, so showed Install and would not start). Only where the client's uid is verified ([`resolve`]),
    /// nothing is under it yet, the game's folder is there, and **nothing of Battle.net's runs** (the client and its
    /// Agent keep `product.db` and would write their own over it): `product.db`'s record and handshake renamed and
    /// nothing else changed (`product::reregister`, which refuses a file it would not write back byte for byte), a
    /// copy kept beside it (`product.db.dashboard-<old uid>.bak`), the file replaced whole. The game's files are never
    /// touched (an uninstall of the old uid would delete them). What is done, or waits on Battle.net closing, is said
    /// once. Gives the codes still under an old uid.
    fn reregister(&self) -> Vec<String> {
        let Some(agent) = self.agent() else { return Vec::new() };
        let Ok(products) = products_in(Some(&agent)) else { return Vec::new() };
        let catalogue = self.catalogue();
        let uids = self.client_uids();
        let mut left = Vec::new();
        for product in products.iter().filter(|p| p.installed && !product::clients_own(&p.uid)) {
            let Some(listed) = catalogue.iter().find(|l| l.code.eq_ignore_ascii_case(&product.code)) else { continue };
            let Resolved { uid, verified: Some(how) } = resolve(listed, &uids) else { continue };
            let folder = source_kit::native(product.install_path.trim());
            if uid == product.uid || products.iter().any(|p| p.uid == uid) || !folder.is_dir() {
                continue;
            }
            let code = &product.code;
            let what = format!(
                "{code}: product.db has it installed in {} under {}, a uid the client does not know it by ({uid}: {how})",
                folder.display(),
                product.uid
            );
            let place = agent::Place { dir: agent.clone(), fake: self.fake.is_some() };
            let running = place.others_running(agent::ours(&agent));
            if !running.is_empty() || agent::ours(&agent).is_some() {
                let mut why = running;
                why.extend(agent::ours(&agent).map(|pid| format!("the Agent the Dashboard started (pid {pid})")));
                once(&format!("{what}: re-registered once {} closed", why.join(", ")));
                left.push(code.clone());
                continue;
            }
            match rewrite(&agent, code, &product.uid, &uid) {
                Ok(backup) => log(&format!(
                    "{what}: re-registered as {uid}, its files left as they are ({} kept)",
                    backup.display()
                )),
                Err(e) => {
                    once(&format!("{what}: could not be re-registered ({e:#})"));
                    left.push(code.clone());
                }
            }
        }
        left
    }

    /// The client's program, when it is there.
    fn client(&self) -> Option<PathBuf> {
        client(&self.registry().ok()?)
    }

    /// The folder an install goes into by default: Battle.net's own (`Battle.net.config`'s `DefaultInstallPath`),
    /// else `Program Files (x86)`, as the client offers; a fake's `games`.
    fn default_folder(&self) -> Option<PathBuf> {
        if let Some(dir) = &self.fake {
            return Some(dir.join("games"));
        }
        let config = self.config_path().and_then(|path| client::config(&path));
        config
            .and_then(|config| client::default_install_path(&config))
            .or_else(|| std::env::var_os("ProgramFiles(x86)").map(PathBuf::from))
    }
}

/// What an install or uninstall asks the Agent about.
#[derive(Debug, Clone)]
struct Target {
    code: String,
    /// The uid the Agent is asked about: the client's own where it is known.
    uid: String,
    /// What confirmed `uid` as the client's, as the log says it; None where only the built-in table gives it, and
    /// then the Agent is not asked to install it (the client is opened on the game instead).
    verified: Option<String>,
    /// The uid `product.db` has it under where that is not `uid`: an install the client does not know (#172).
    stale: Option<String>,
    name: String,
    /// Its folder, where `product.db` says it is installed and it is there.
    installed: Option<PathBuf>,
    /// Its folder, where `product.db` has it and does not say installed: a download the Agent began and did not
    /// finish (the Dashboard ended mid-download, which ends the Agent it started). Installed again into it.
    partial: Option<PathBuf>,
}

/// Every product in the `product.db` of the agent's folder `agent`.
fn products_in(agent: Option<&Path>) -> Result<Vec<product::Product>> {
    let Some(path) = agent.map(|agent| agent.join("product.db")).filter(|path| path.is_file()) else {
        return Ok(Vec::new());
    };
    let bytes = std::fs::read(&path).with_context(|| path.display().to_string())?;
    product::parse(&bytes).with_context(|| path.display().to_string())
}

/// Whether `product.db` in `agent` says `code` is installed.
fn installed_in(agent: &Path, code: &str) -> bool {
    products_in(Some(agent)).unwrap_or_default().iter().any(|p| p.code.eq_ignore_ascii_case(code) && p.installed)
}

/// `code`'s product in `product.db` in `agent`, under `uid`.
fn product_in(agent: &Path, code: &str, uid: &str) -> Option<product::Product> {
    let products = products_in(Some(agent)).ok()?;
    products.into_iter().find(|p| p.code.eq_ignore_ascii_case(code) && p.uid == uid)
}

/// A catalogue row's uid, and what confirmed it as the client's, where anything has.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Resolved {
    uid: String,
    verified: Option<String>,
}

/// The uid the client knows a catalogue product by (#172): the row's, where the row is verified, or where the
/// client's `Battle.net.config` names it in `Games`; else the product code, where `Games` names that (the current
/// client keys the games of the last few years by their code: the TV PC's `lbra`); else the row's, not verified.
fn resolve(listed: &catalogue::Product, client_uids: &[String]) -> Resolved {
    let named = |uid: &str| client_uids.iter().any(|known| known.eq_ignore_ascii_case(uid));
    let (uid, verified) = match &listed.verified {
        Some(how) => (listed.uid.clone(), Some(how.clone())),
        None if named(&listed.uid) => (listed.uid.clone(), Some("Battle.net.config's Games names it".into())),
        None if named(&listed.code) => {
            (listed.code.clone(), Some("Battle.net.config's Games names the product code, not the table's uid".into()))
        }
        None => (listed.uid.clone(), None),
    };
    Resolved { uid, verified }
}

/// `product.db` in `agent` with `code` re-registered from `from` to `to` (`product::reregister`): a copy of it as
/// it was kept beside it (the first time only), the new one written beside it and put in its place. The copy's path.
fn rewrite(agent: &Path, code: &str, from: &str, to: &str) -> Result<PathBuf> {
    let db = agent.join("product.db");
    let bytes = std::fs::read(&db).context("not read")?;
    let again = product::reregister(&bytes, code, from, to)?.context("nothing under that uid any more")?;
    let backup = agent.join(format!("product.db.dashboard-{from}.bak"));
    if !backup.exists() {
        std::fs::write(&backup, &bytes).with_context(|| format!("{} not written", backup.display()))?;
    }
    let fresh = agent.join("product.db.dashboard-new");
    std::fs::write(&fresh, &again).with_context(|| format!("{} not written", fresh.display()))?;
    std::fs::rename(&fresh, &db).context("not put in place")?;
    Ok(backup)
}

/// A line of the log said once while the Dashboard runs: a state that holds until something else changes.
fn once(line: &str) {
    static SAID: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !said.iter().any(|s| s == line) {
        said.push(line.to_string());
        log(line);
    }
}

/// A game's name as a folder's: what Windows does not take in a name left out.
fn folder_name(name: &str) -> String {
    let plain: String = name.chars().filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control()).collect();
    plain.trim().trim_end_matches('.').to_string()
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
        // An empty folder named for the store is no store, and not the machine's: what a test gives.
        if self.fake.as_deref().is_some_and(source_kit::empty_folder) {
            return Ok(None);
        }
        let registry = self.registry()?;
        let client = client(&registry);
        let agent = self.agent();
        let database = agent.as_ref().map(|agent| agent.join("product.db")).filter(|db| db.is_file());
        if self.fake.is_none() && client.is_none() && database.is_none() {
            return Ok(None);
        }
        // A game under a uid the client does not know, re-registered under its own where that may be done now.
        self.reregister();
        let products = self.products()?;
        let names = agent.as_deref().map(names).unwrap_or_default();
        let keys = uninstall_keys(&registry);
        let catalogue = self.catalogue();
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
            let listed = catalogue.iter().find(|p| p.code.eq_ignore_ascii_case(&product.code)).map(|p| p.name.clone());
            let name = names.get(&product.code).cloned().or(listed).or(display).unwrap_or_else(|| product.uid.clone());
            games.push(Game {
                source: SOURCE.into(),
                source_id: product.code.clone(),
                name,
                installed: true,
                install_dir: Some(folder),
                ..Game::default()
            });
        }
        // The catalogue's other products, while the client is there to install them with: available to install.
        if client.is_some() || self.fake.is_some() {
            for product in &catalogue {
                let same = |game: &Game| {
                    game.source_id.eq_ignore_ascii_case(&product.code)
                        || products.iter().any(|p| {
                            p.installed && p.uid.eq_ignore_ascii_case(&product.uid) && p.code == game.source_id
                        })
                };
                if games.iter().any(same) {
                    continue;
                }
                games.push(Game {
                    source: SOURCE.into(),
                    source_id: product.code.clone(),
                    name: product.name.clone(),
                    installed: false,
                    ..Game::default()
                });
            }
        }
        games.sort_by_key(|game| game.name.to_lowercase());
        let place = match (&self.fake, database) {
            (Some(dir), _) => dir.display().to_string(),
            (None, Some(db)) => db.parent().map_or(String::new(), |agent| agent.display().to_string()),
            (None, None) => client.map_or(String::new(), |exe| exe.display().to_string()),
        };
        let from = place;
        Ok(Some(Found { library: Library { games, ..Library::default() }, from }))
    }

    /// `Battle.net.exe --exec="launch <code>"`, the catalogue's launch code where it has one.
    fn start(&self, game: &Game) -> Option<Start> {
        let code = code(game)?;
        let path = self.client()?;
        let listed = self.catalogue().into_iter().find(|p| p.code.eq_ignore_ascii_case(code));
        let launch = listed.and_then(|p| p.launch).unwrap_or_else(|| code.to_string());
        Some(Start::Program { path, args: vec![format!("--exec=launch {launch}")], dir: None })
    }

    /// A game `product.db` has installed only under a uid the client does not know it by (#172): Battle.net would
    /// show Install and start nothing, so Play says what to do instead. Re-registered first where that may be done
    /// now (Battle.net closed), and then started as any other.
    fn cant_start(&self, game: &Game) -> Option<String> {
        let target = self.target(game)?;
        let stale = target.stale.clone()?;
        target.installed.as_ref()?;
        if !self.reregister().contains(&target.code) {
            return None;
        }
        let place = self.place();
        let running = place.is_some_and(|place| !place.others_running(agent::ours(&place.dir)).is_empty());
        log(&format!(
            "{}: not started: product.db has it under {stale}, and Battle.net knows it as {}: {}",
            target.code,
            target.uid,
            if running { "re-registered once Battle.net is closed" } else { "it could not be re-registered" }
        ));
        Some(if running {
            "Battle.net doesn't show this game as installed yet. Close Battle.net, then play it here again.".into()
        } else {
            "Battle.net doesn't show this game as installed. In Battle.net, choose Install, then Locate the game."
                .into()
        })
    }

    /// Install: the game's page in the client, `--game=<uid>`. Uninstall: its Uninstall key's own command.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        let uid = self.target(game)?.uid;
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

    /// The installs asked of the Agent, while the Agent is there or one is under way.
    fn installs(&self) -> Option<Vec<Install>> {
        let installs = self.jobs.installs();
        (self.place().is_some() || !installs.is_empty()).then_some(installs)
    }

    /// The codes `product.db` says are installed, whose folders are there: as `read` lists them.
    fn installed_now(&self) -> Option<Vec<String>> {
        let products = self.products().ok()?;
        let mut codes: Vec<String> = products
            .iter()
            .filter(|p| p.installed && !product::clients_own(&p.uid))
            .filter(|p| !p.install_path.trim().is_empty() && source_kit::native(p.install_path.trim()).is_dir())
            .map(|p| p.code.clone())
            .collect();
        codes.sort();
        codes.dedup();
        Some(codes)
    }

    /// Through the Agent, asking is all an uninstall takes.
    fn uninstalls_by_itself(&self) -> bool {
        self.place().is_some()
    }

    fn installer(&self) -> Option<&dyn Installer> {
        Some(self)
    }
}

/// How long an uninstall is followed in `product.db`.
const UNINSTALL_WAIT: Duration = Duration::from_secs(600);

/// Battle.net's installer (#172): the Agent, where it is there. Each call is asked on a thread of its own (the
/// Dashboard's window never waits on the Agent) and falls back to the client's own way, said in the log, where the
/// Agent does not answer or take it: that is opened with the opener's `detached()` copy, and where there is none the
/// client's way is taken at once.
impl Installer for BattleNet {
    fn can(&self) -> Can {
        if self.place().is_none() {
            return Can::NOTHING;
        }
        // Cancel is sure with the Agent the Dashboard started (it is stopped, and its download with it); with another,
        // only the guess, with steering on.
        let place = self.place();
        let ours = place.as_ref().is_some_and(|place| agent::ours(&place.dir).is_some());
        Can {
            pause: self.steer,
            cancel: self.steer || ours,
            choose_folder: self.folders().len() > 1,
            quiet_uninstall: true,
        }
    }

    /// The default folder ([`BattleNet::default_folder`]) and the folders the installed games are in, each game's
    /// folder being `<folder>\<its name>`. Free space is not read.
    fn folders(&self) -> Vec<Folder> {
        if self.place().is_none() {
            return Vec::new();
        }
        let mut folders: Vec<Folder> =
            self.default_folder().into_iter().map(|path| Folder { path, free: None, default: true }).collect();
        let products = self.products().unwrap_or_default();
        for product in products.iter().filter(|p| p.installed && !product::clients_own(&p.uid)) {
            let Some(parent) = source_kit::native(product.install_path.trim()).parent().map(Path::to_path_buf) else {
                continue;
            };
            if parent.as_os_str().is_empty() || !parent.is_dir() {
                continue;
            }
            if !folders.iter().any(|f| source_kit::same_folder(&f.path, &parent)) {
                folders.push(Folder { path: parent, free: None, default: false });
            }
        }
        folders
    }

    fn install(&self, game: &Game, folder: Option<&Folder>, open: &mut dyn Opener) -> Result<(), Refusal> {
        let target = self.target(game).ok_or(Refusal::Unsupported)?;
        let Some(place) = self.place() else {
            log(&format!("{}: no Agent on this PC: opening Battle.net on the game instead", target.code));
            return Err(Refusal::Unsupported);
        };
        let Some(mut detached) = open.detached() else { return Err(Refusal::Unsupported) };
        if target.installed.is_some() {
            match &target.stale {
                Some(stale) => log(&format!(
                    "{}: installed already, under {stale}: re-registered as {} once Battle.net is closed",
                    target.code, target.uid
                )),
                None => log(&format!("{}: installed already", target.code)),
            }
            return Ok(());
        }
        // Never under a uid only a table gives: the client may know the game by another, and would not see it.
        let Some(verified) = target.verified.clone() else {
            log(&format!(
                "{}: its uid {} is the built-in table's, not confirmed on this PC (Battle.net.config's Games names \
                 neither it nor {}): opening Battle.net on the game instead",
                target.code, target.uid, target.code
            ));
            return Err(Refusal::Unsupported);
        };
        if !self.jobs.begin(&target.code, &target.uid) {
            log(&format!("{}: under way already", target.code));
            return Ok(());
        }
        let Some(base) = folder.map(|f| f.path.clone()).or_else(|| self.default_folder()) else {
            self.jobs.end(&target.code);
            return Err(Refusal::Unsupported);
        };
        let fallback = self.ask(game, Ask::Install);
        let jobs = self.jobs.clone();
        let no_download = self.no_download;
        log(&format!(
            "{}: installing {} ({}) under {} through the Agent",
            target.code,
            target.uid,
            target.name,
            base.display()
        ));
        log(&format!("{}: the client knows it as {} ({verified})", target.code, target.uid));
        let country = self.country();
        let game = game.clone();
        std::thread::spawn(move || {
            let code = target.code.clone();
            match agent_install(&place, &target, &base, no_download, country) {
                Asked::Taken(session, watch) => {
                    let (agent, uid) = (place.dir.clone(), target.uid.clone());
                    let product = move || product_in(&agent, &code, &uid);
                    installing::follow(jobs, session, place, *watch, game, detached, product);
                }
                Asked::Refused(why) => {
                    jobs.end(&code);
                    fall_back(&code, &format!("the Agent did not take the install ({why})"), fallback, &mut *detached);
                }
                Asked::Stopped(why) => {
                    jobs.end(&code);
                    log(&format!("{code}: {why}: dropped from the Queue, nothing downloaded"));
                }
            }
        });
        Ok(())
    }

    fn uninstall(&self, game: &Game, open: &mut dyn Opener) -> Result<(), Refusal> {
        let target = self.target(game).ok_or(Refusal::Unsupported)?;
        let place = self.place().ok_or(Refusal::Unsupported)?;
        if target.installed.is_none() {
            return Err(Refusal::Unsupported);
        }
        let Some(mut detached) = open.detached() else { return Err(Refusal::Unsupported) };
        let fallback = self.ask(game, Ask::Uninstall);
        log(&format!("{}: uninstalling {} through the Agent", target.code, target.uid));
        std::thread::spawn(move || {
            let code = target.code.clone();
            // The session is kept until the uninstall is seen done: an Agent the Dashboard started runs until then.
            let asked = agent::connect(&place).and_then(|mut session| {
                let (method, path, body) = agent::calls::uninstall(&target.uid);
                let answer = session.call(&format!("uninstall {code}"), method, &path, Some(&body))?;
                agent::calls::error(&answer).map_or(Ok(session), Err)
            });
            let _session = match asked {
                Ok(session) => session,
                Err(why) => {
                    let why = format!("the Agent did not take the uninstall ({why})");
                    fall_back(&code, &why, fallback, &mut *detached);
                    return;
                }
            };
            let begun = Instant::now();
            while begun.elapsed() < UNINSTALL_WAIT {
                if !installed_in(&place.dir, &code) {
                    log(&format!(
                        "{code}: uninstalled: product.db no longer says installed, {} ms on",
                        begun.elapsed().as_millis()
                    ));
                    return;
                }
                std::thread::sleep(installing::EVERY);
            }
            log(&format!(
                "{code}: product.db still says installed {} s after the Agent took the uninstall",
                UNINSTALL_WAIT.as_secs()
            ));
        });
        Ok(())
    }

    /// Only an install the Dashboard asked for. Pause and resume only with `DASHBOARD_BATTLENET_STEER=1`, asked on
    /// the path its progress is read on (guesses: `agent::calls::pause`). Cancel: with steering on, the guess
    /// (`DELETE` on that path) is asked first; then, always, the install is no longer followed, which lets the Agent
    /// the Dashboard started go: it is stopped, and its download with it, and the folder it made taken away
    /// (`installing`). With an Agent the Dashboard did not start and steering off, Cancel is not offered (`can`).
    /// Each answer is logged, and a refusal said; nothing falls back (the client has no command for it).
    fn steer(&self, game: &Game, steer: Steer, _open: &mut dyn Opener) -> Result<(), Refusal> {
        let code = code(game).ok_or(Refusal::Unsupported)?.to_string();
        if !self.jobs.has(&code) || !steer.allowed(self.can()) {
            return Err(Refusal::Unsupported);
        }
        let place = self.place().ok_or(Refusal::Unsupported)?;
        let uid = self.jobs.uid(&code).ok_or(Refusal::Unsupported)?;
        if steer == Steer::Resume && !self.jobs.paused(&code) {
            return Ok(());
        }
        let path = self.jobs.live(&code).unwrap_or_else(|| format!("/update/{uid}"));
        let jobs = self.jobs.clone();
        let guess = self.steer;
        std::thread::spawn(move || {
            let what = format!("{} {code}", steer.verb());
            if steer == Steer::Cancel && !guess {
                log(&format!(
                    "{code}: cancel: the Agent the Dashboard started is to be stopped, which ends its download"
                ));
                jobs.stop(&code);
                return;
            }
            let asked = agent::connect(&place).and_then(|mut session| {
                let answer = match steer {
                    Steer::Pause | Steer::Resume => {
                        let (method, path, body) = agent::calls::pause(&path, steer == Steer::Pause);
                        session.call(&what, method, &path, Some(&body))?
                    }
                    Steer::Cancel => {
                        let (method, path) = agent::calls::cancel(&path);
                        session.call(&what, method, &path, None)?
                    }
                };
                agent::calls::error(&answer).map_or(Ok(()), Err)
            });
            match (asked, steer) {
                (Ok(()), Steer::Cancel) => log(&format!("{code}: cancelled: the Agent took it")),
                (Ok(()), _) => log(&format!("{code}: {}: the Agent took it", steer.verb())),
                (Err(why), _) => log(&format!("{code}: {}: the Agent did not take it ({why})", steer.verb())),
            }
            // Cancelled here whatever the Agent made of the guess: the follower lets the Agent the Dashboard
            // started go.
            if steer == Steer::Cancel {
                jobs.stop(&code);
            }
        });
        Ok(())
    }
}

/// How long the Agent is given to work out a game's size before the form is finalized all the same.
const FORM_WAIT: Duration = Duration::from_secs(30);

/// How an install asked of the Agent came out.
enum Asked {
    /// Taken: the download under way, followed with this session, by what the Agent said of it.
    Taken(agent::Session, Box<installing::Watch>),
    /// Not taken: why, the client to be asked instead.
    Refused(String),
    /// Stopped before the download, as asked (`calls::no_download`): why, and nothing more to do.
    Stopped(String),
}

/// Asks the Agent to install `target` under `base`, filling in its form (`agent::calls`): `POST /install`, `POST
/// /install/<uid>` unfinalized until the size is known, then finalized, then `POST /update`. A form the Agent took
/// and the Dashboard does not finish is abandoned (`calls::abandon`).
fn agent_install(
    place: &agent::Place,
    target: &Target,
    base: &Path,
    no_download: bool,
    country: Result<client::Country, String>,
) -> Asked {
    let mut session = match agent::connect(place) {
        Ok(session) => session,
        Err(why) => return Asked::Refused(why),
    };
    let (code, uid) = (&target.code, &target.uid);
    let what = format!("install {code}");
    let (method, path, body) = agent::calls::install(uid, &session.region, code);
    let first = match session.call(&what, method, &path, Some(&body)) {
        Ok(answer) => answer,
        Err(why) => return Asked::Refused(why),
    };
    if let Some(why) = agent::calls::refusal(&first) {
        abandon(&mut session, code, uid, &said(&why));
        return Asked::Refused(said(&why));
    }
    let form = agent::calls::read_form(&first).unwrap_or_default();
    log(&format!("{code}: the Agent's form: {}", form.said()));
    let windows = listening::ui_language();
    let locale =
        agent::calls::choose_locale(&form.languages, windows.as_deref()).unwrap_or(agent::calls::LOCALE.to_string());
    let dirname = form.dirname.clone().unwrap_or_else(|| folder_name(&target.name));
    // A download the Agent began and did not finish goes on in its own folder (the Agent is taken to pick up from
    // the files there, as the client does: not known for sure, docs/sources.md).
    let game_dir = match &target.partial {
        Some(folder) => {
            log(&format!(
                "{code}: product.db has a download of it not finished in {}: installed again into that folder",
                folder.display()
            ));
            folder.display().to_string()
        }
        None => base.join(&dirname).display().to_string(),
    };
    let made_dir = (!Path::new(&game_dir).exists()).then(|| PathBuf::from(&game_dir));
    log(&format!(
        "{code}: filling it in: locale {locale} (Windows' {}), into {game_dir}",
        windows.as_deref().unwrap_or("not known")
    ));
    let country = match country {
        Ok(country) => {
            log(&format!(
                "{code}: the account's country {}, its address in {} ({})",
                country.account, country.geo_ip, country.from
            ));
            Some(country)
        }
        Err(why) => {
            log(&format!("{code}: no country sent: the client's cache gives none ({why})"));
            None
        }
    };
    let country = country.as_ref();
    // Unfinalized, again each second while the Agent works out the size.
    let begun = Instant::now();
    let filled = loop {
        let (method, path, body) = agent::calls::install_options(uid, &game_dir, &locale, false, country);
        let answer = match session.call(&what, method, &path, Some(&body)) {
            Ok(answer) => answer,
            Err(why) => {
                abandon(&mut session, code, uid, &why);
                return Asked::Refused(why);
            }
        };
        if let Some(why) = agent::calls::problem(&answer) {
            abandon(&mut session, code, uid, &said(&why));
            return Asked::Refused(said(&why));
        }
        let form = agent::calls::read_form(&answer).unwrap_or_default();
        if !form.sizing() || begun.elapsed() >= FORM_WAIT {
            break form;
        }
        std::thread::sleep(installing::EVERY);
    };
    log(&format!("{code}: the form filled in: {}", filled.said()));
    if no_download {
        let why = "stopped before the download (DASHBOARD_BATTLENET_NO_DOWNLOAD=1)";
        abandon(&mut session, code, uid, why);
        return Asked::Stopped(why.into());
    }
    let (method, path, body) = agent::calls::install_options(uid, &game_dir, &locale, true, country);
    let finalized = match session.call(&what, method, &path, Some(&body)) {
        Ok(answer) => {
            if let Some(why) = agent::calls::problem(&answer) {
                abandon(&mut session, code, uid, &said(&why));
                return Asked::Refused(said(&why));
            }
            answer
        }
        Err(why) => {
            abandon(&mut session, code, uid, &why);
            return Asked::Refused(why);
        }
    };
    let (method, path, body) = agent::calls::start(uid);
    let update = match session.call(&what, method, &path, Some(&body)) {
        Ok(answer) => {
            if let Some(error) = agent::calls::error(&answer) {
                log(&format!("{code}: POST /update said {}: following the install all the same", said(&error)));
            }
            Some(answer)
        }
        Err(why) => {
            log(&format!("{code}: POST /update had no answer ({why}): following the install all the same"));
            None
        }
    };
    let answers: Vec<&http::Answer> = update.iter().chain([&finalized]).collect();
    let watch = installing::Watch {
        code: code.clone(),
        paths: agent::calls::progress_paths(uid, &answers),
        game_path: agent::calls::game_path(uid, &answers),
        required: agent::calls::read_form(&finalized).and_then(|f| f.required_space).or(filled.required_space),
        made_dir,
    };
    log(&format!("{code}: the Agent took the install"));
    log(&format!("{code}: its progress read on {}, the game on {}", watch.paths.join(", "), watch.game_path));
    Asked::Taken(session, Box::new(watch))
}

/// Leaves nothing of an install the Dashboard will not finish in the Agent: its form taken back (`calls::abandon`, a
/// guess), said with why. An Agent the Dashboard started is then stopped as the session goes.
fn abandon(session: &mut agent::Session, code: &str, uid: &str, why: &str) {
    let (method, path) = agent::calls::abandon(uid);
    let answer = session.call(&format!("abandon {code}"), method, &path, None);
    let how = match answer {
        Ok(answer) if answer.ok() => format!("{method} {path} -> {}", answer.status),
        Ok(answer) => format!("{method} {path} -> {}: nothing of it held, or not taken back", answer.status),
        Err(e) => format!("{method} {path}: no answer ({e})"),
    };
    let after = if session.ours() { "; the Agent the Dashboard started is stopped" } else { "" };
    log(&format!("{code}: the install abandoned ({why}): {how}{after}"));
}

/// An error with what it is known to mean.
fn said(error: &str) -> String {
    match agent::calls::meaning(error) {
        Some(meaning) => format!("{error}: {meaning}"),
        None => error.to_string(),
    }
}

/// The client's own way, opened where the Agent's did not work: said in the log, with why.
fn fall_back(code: &str, why: &str, fallback: Option<Start>, open: &mut dyn Opener) {
    match fallback {
        Some(start) => {
            log(&format!("{code}: {why}: asking Battle.net instead: {start}"));
            if let Err(e) = open.open(&start) {
                log(&format!("{code}: Battle.net could not be asked either: {e}"));
            }
        }
        None => log(&format!("{code}: {why}, and Battle.net has no other way here")),
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn the_fake_reads_back_as_its_installed_games_and_its_catalogue() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        let spec = Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let battlenet = BattleNet::at(Some(root.clone()));
        assert_eq!((battlenet.key(), battlenet.name()), ("battlenet", "Battle.net"));
        let found = battlenet.read().unwrap().expect("a fake Battle.net");
        assert_eq!(found.from, root.display().to_string());
        let games = &found.library.games;
        let seen: Vec<(&str, &str, bool)> =
            games.iter().map(|g| (g.name.as_str(), g.source_id.as_str(), g.installed)).collect();
        // Not the one still downloading, nor the client's own agent. Juniper Vault has no name in aggregate.json:
        // its Uninstall key's. The catalogue's two, available to install.
        assert_eq!(
            seen,
            [
                ("Emberwatch", "emb", true),
                ("Juniper Vault", "jnv", true),
                ("Larkspur Hollow", "lkh", false),
                ("Quarry Bell", "qbl", false)
            ]
        );
        assert_eq!(spec.expected(), games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
        assert!(games.iter().filter(|g| g.installed).all(|g| g.install_dir.as_ref().is_some_and(|d| d.is_dir())));
        assert_eq!(battlenet.installed_now(), Some(vec!["emb".to_string(), "jnv".to_string()]));
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
        // A game of the catalogue, not installed: its page by the catalogue's uid.
        let larkspur = games.iter().find(|g| g.source_id == "lkh").unwrap();
        assert_eq!(battlenet.ask(larkspur, Ask::Install), Some(program(&["--game=larkspur"])));
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
    fn the_real_tables_launch_codes_are_used() {
        let battlenet = BattleNet::default();
        let catalogue = battlenet.catalogue();
        assert!(catalogue.iter().any(|p| p.code == "lbra" && p.launch.as_deref() == Some("LBRA")));
    }

    #[test]
    fn no_battlenet_is_none_and_a_fake_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no battle.net here");
        // An empty folder is no store, and never the machine's own.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(BattleNet::at(Some(empty.path().to_path_buf())).read().unwrap(), None);
        let error = BattleNet::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").starts_with(&missing.display().to_string()), "{error:#}");
        if cfg!(not(windows)) {
            assert_eq!(BattleNet::at(None).read().unwrap(), None);
        }
    }

    /// Keeps what it was asked to open, and what it was told stopped, here and from its detached copies.
    #[derive(Clone, Default)]
    struct Kept(Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<(String, model::Stop)>>>);

    impl Opener for Kept {
        fn open(&mut self, start: &Start) -> Result<(), String> {
            self.0.lock().unwrap().push(start.to_string());
            Ok(())
        }

        fn stopped(&mut self, game: &Game, stop: &model::Stop) {
            self.1.lock().unwrap().push((game.source_id.clone(), stop.clone()));
        }

        fn detached(&self) -> Option<Box<dyn Opener + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl Kept {
        fn opened(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }

        fn stops(&self) -> Vec<(String, model::Stop)> {
            self.1.lock().unwrap().clone()
        }
    }

    fn wait(what: &str, mut done: impl FnMut() -> bool) {
        let begun = Instant::now();
        while !done() {
            assert!(begun.elapsed() < Duration::from_secs(20), "never: {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn game(battlenet: &BattleNet, code: &str) -> Game {
        let games = battlenet.read().unwrap().unwrap().library.games;
        games.into_iter().find(|g| g.source_id == code).unwrap()
    }

    #[test]
    fn the_agent_installs_into_the_folder_follows_and_uninstalls_with_no_window() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let said = Arc::new(Mutex::new(Vec::<String>::new()));
        let kept = said.clone();
        let served =
            standin::serve(&root, Duration::from_millis(600), move |line| kept.lock().unwrap().push(line.into()))
                .unwrap();
        let battlenet = BattleNet::at(Some(root.clone())).with_steer(false);
        let installer = battlenet.installer().unwrap();
        // One folder: the installed games are in the default one too.
        assert_eq!(installer.can(), Can { pause: false, cancel: false, choose_folder: false, quiet_uninstall: true });
        let folders = installer.folders();
        assert_eq!(folders[0], Folder { path: root.join("games"), free: None, default: true });
        assert_eq!(battlenet.installs(), Some(vec![]));

        // Larkspur Hollow, owned: into the default folder, its bytes and speed in the Queue, then installed.
        let larkspur = game(&battlenet, "lkh");
        let mut open = Kept::default();
        installer.install(&larkspur, model::installer::default_folder(&folders), &mut open).unwrap();
        assert_eq!(battlenet.installs().unwrap()[0].source_id, "lkh");
        wait("downloading", || {
            battlenet.installs().unwrap().iter().any(|i| {
                matches!(i.state, model::InstallState::Downloading { done, .. } if done > 0) && i.speed.is_some()
            })
        });
        wait("installed", || battlenet.installs().unwrap().is_empty());
        let larkspur = game(&battlenet, "lkh");
        assert!(larkspur.installed, "{larkspur:?}");
        // Into the folder the Agent's form names, under the folder chosen: not the game's name.
        assert_eq!(larkspur.install_dir, Some(root.join("games").join("LarkspurHollow")));
        assert!(battlenet.installed_now().unwrap().contains(&"lkh".to_string()));
        assert!(open.opened().is_empty(), "{:?}", open.opened());
        assert!(open.stops().is_empty(), "{:?}", open.stops());
        // Every request said, the token never.
        let lines = said.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("POST /install {")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("\"game_dir\"")), "{lines:?}");
        // The form filled in: 704 on its first answer, the locale as its list writes it, unfinalized until sized.
        assert!(lines.iter().any(|l| l.contains("POST /install {") && l.contains("\"error\":704")), "{lines:?}");
        let posted = |finalized: bool| {
            lines.iter().any(|l| {
                l.contains("POST /install/larkspur {")
                    && l.contains("\"selected_locale\":\"enUS\"")
                    && l.contains(&format!("\"finalized\":{finalized}"))
            })
        };
        assert!(posted(false) && posted(true), "{lines:?}");
        // Once finalized, the progress where the Agent says it is: `/install/<uid>` is 404 and not the end.
        assert!(lines.iter().any(|l| l.contains("GET /install/larkspur -> 404 {}")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("GET /update/larkspur -> 200")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("GET /game/larkspur -> 200")), "{lines:?}");
        assert!(
            lines.iter().all(|l| !l.contains(&format!("standin-{}-{}", std::process::id(), served.port))),
            "{lines:?}"
        );
        for secret in ["48151623421234", "S-1-5-21", "standin-user"] {
            assert!(lines.iter().all(|l| !l.contains(secret)), "{secret}: {lines:?}");
        }

        // Uninstalled: gone from product.db and its folder, and still in the catalogue, not installed.
        installer.uninstall(&larkspur, &mut open).unwrap();
        wait("uninstalled", || !battlenet.installed_now().unwrap().contains(&"lkh".to_string()));
        assert!(!game(&battlenet, "lkh").installed);
        assert!(!root.join("games").join("LarkspurHollow").exists());
        assert!(open.opened().is_empty(), "{:?}", open.opened());

        // Quarry Bell, not owned: the Agent refuses, and the client is opened on its page.
        let quarry = game(&battlenet, "qbl");
        installer.install(&quarry, None, &mut open).unwrap();
        wait("the client asked", || !open.opened().is_empty());
        assert_eq!(open.opened(), [format!("{} --game=quarrybell", root.join(CLIENT_EXE).display())]);
        wait("dropped", || battlenet.installs().unwrap().is_empty());
        // Pause, with steering off: not offered.
        assert_eq!(installer.steer(&quarry, Steer::Pause, &mut open), Err(Refusal::Unsupported));
    }

    #[test]
    fn with_steering_on_an_install_pauses_resumes_and_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        // Slow enough for the follower, which asks once a second, to see each state.
        let _served = standin::serve(&root, Duration::from_millis(1500), |_| {}).unwrap();
        let battlenet = BattleNet::at(Some(root.clone())).with_steer(true);
        let installer = battlenet.installer().unwrap();
        assert!(installer.can().pause && installer.can().cancel);
        let larkspur = game(&battlenet, "lkh");
        let mut open = Kept::default();
        installer.install(&larkspur, None, &mut open).unwrap();
        let state = || battlenet.installs().unwrap().first().map(|i| i.state);
        wait("downloading", || state().is_some_and(|s| s.downloading()));
        installer.steer(&larkspur, Steer::Pause, &mut open).unwrap();
        wait("paused", || state().is_some_and(|s| s.paused()));
        let at = state().unwrap().bytes();
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(state().unwrap().bytes(), at, "paused, its bytes stand still");
        installer.steer(&larkspur, Steer::Resume, &mut open).unwrap();
        wait("downloading again", || state().is_some_and(|s| s.downloading()));
        installer.steer(&larkspur, Steer::Cancel, &mut open).unwrap();
        wait("cancelled", || state().is_none());
        wait("its folder taken away", || !root.join("games").join("LarkspurHollow").exists());
        assert!(!game(&battlenet, "lkh").installed);
        assert!(open.opened().is_empty() && open.stops().is_empty());
    }

    #[test]
    fn an_error_the_agent_names_mid_download_is_told_with_its_code() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let served = standin::serve(&root, Duration::from_millis(1500), |_| {}).unwrap();
        let battlenet = BattleNet::at(Some(root.clone())).with_steer(false);
        let installer = battlenet.installer().unwrap();
        // Steering off and the Agent not the Dashboard's: no Cancel to offer.
        assert!(!installer.can().cancel);
        let larkspur = game(&battlenet, "lkh");
        let mut open = Kept::default();
        installer.install(&larkspur, None, &mut open).unwrap();
        let state = || battlenet.installs().unwrap().first().map(|i| i.state);
        wait("downloading", || state().is_some_and(|s| s.downloading()));
        served.fail("larkspur", 2421);
        wait("dropped", || state().is_none());
        assert_eq!(
            open.stops(),
            [(
                "lkh".to_string(),
                model::Stop::Failed(
                    "Battle.net's Agent stopped it with error 2421: the PC does not meet its needs, or there is not \
                     enough space."
                        .into()
                )
            )]
        );
        assert!(open.opened().is_empty());
    }

    #[test]
    fn with_no_agent_answering_the_client_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        // The fake's Agent.exe is an empty file: started, it never answers.
        let battlenet = BattleNet::at(Some(root.clone()));
        let larkspur = game(&battlenet, "lkh");
        let mut open = Kept::default();
        battlenet.installer().unwrap().install(&larkspur, None, &mut open).unwrap();
        wait("the client asked", || !open.opened().is_empty());
        assert_eq!(open.opened(), [format!("{} --game=larkspur", root.join(CLIENT_EXE).display())]);
    }

    #[test]
    fn stopped_before_the_download_the_form_is_filled_in_and_taken_back() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let served = standin::serve(&root, Duration::from_millis(600), |_| {}).unwrap();
        let battlenet = BattleNet::at(Some(root.clone())).with_no_download(true);
        let larkspur = game(&battlenet, "lkh");
        let mut open = Kept::default();
        battlenet.installer().unwrap().install(&larkspur, None, &mut open).unwrap();
        wait("dropped", || battlenet.installs().unwrap().is_empty());
        wait("taken back", || served.under_way().is_empty());
        assert!(!root.join("games").join("LarkspurHollow").exists(), "nothing downloaded");
        assert!(open.opened().is_empty(), "stopped, not refused: the client is not asked");
        assert!(!game(&battlenet, "lkh").installed);
    }

    #[test]
    fn with_battlenets_own_agent_running_none_is_started_and_the_client_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let _client = standin::serve_as(&root, standin::Mode::Clients, Duration::from_millis(600), |_| {}).unwrap();
        let battlenet = BattleNet::at(Some(root.clone()));
        let larkspur = game(&battlenet, "lkh");
        let mut open = Kept::default();
        battlenet.installer().unwrap().install(&larkspur, None, &mut open).unwrap();
        wait("the client asked", || !open.opened().is_empty());
        assert_eq!(open.opened(), [format!("{} --game=larkspur", root.join(CLIENT_EXE).display())]);
        assert_eq!(agent::ours(&root.join("Agent")), None, "no Agent of the Dashboard's");
    }

    /// Adds `uids` to the fake client's `Battle.net.config` `Games`, as though the user had looked at those games.
    fn known(root: &Path, uids: &[&str]) {
        let path = root.join(fixture::CONFIG);
        let mut config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for uid in uids {
            config["Games"][*uid] = serde_json::json!({ "Resumable": "false" });
        }
        std::fs::write(&path, config.to_string()).unwrap();
    }

    /// The TV PC's case (#172), on the stand-ins: the table's uid for a game is not the client's.
    #[test]
    fn a_game_is_installed_only_under_the_uid_the_client_knows_it_by() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        // The table says `larkspur_old`, as Playnite's said `libra`; the client knows it by its code, `lkh`.
        let mut spec = Fixture::standard();
        spec.available[0].uid = "larkspur_old".into();
        spec.available[0].client_uid = Some("lkh".into());
        fixture::write(&root, &spec).unwrap();
        let said = Arc::new(Mutex::new(Vec::<String>::new()));
        let kept = said.clone();
        let _served =
            standin::serve(&root, Duration::from_millis(300), move |line| kept.lock().unwrap().push(line.into()))
                .unwrap();
        let battlenet = BattleNet::at(Some(root.clone())).with_steer(false);
        let larkspur = game(&battlenet, "lkh");
        let target = battlenet.target(&larkspur).unwrap();
        assert_eq!(target.uid, "lkh", "Battle.net.config names the code: that is the uid");
        assert!(target.verified.unwrap().contains("names the product code"));
        let mut open = Kept::default();
        battlenet.installer().unwrap().install(&larkspur, None, &mut open).unwrap();
        wait("installed", || battlenet.installs().unwrap().is_empty());
        assert!(game(&battlenet, "lkh").installed);
        // The stand-in client starts it: product.db has it under its uid.
        assert_eq!(fixture::client_sees(&root, "lkh").unwrap(), root.join("games").join("LarkspurHollow"));
        let lines = said.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("POST /install/lkh {")), "{lines:?}");
        assert!(lines.iter().all(|l| !l.contains("larkspur_old")), "{lines:?}");
        // The account's country, the client cache's, sent; never USA.
        assert!(lines
            .iter()
            .any(|l| l.contains("\"account_country\":\"NZL\"") && l.contains("\"geo_ip_country\":\"NZ\"")));
        assert!(lines.iter().all(|l| !l.contains("\"USA\"")), "{lines:?}");
        assert!(open.opened().is_empty() && open.stops().is_empty());

        // A uid only the table gives, which the client names nowhere: never asked of the Agent.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        let mut spec = Fixture::standard();
        spec.available[0].uid = "larkspur_old".into();
        spec.available[0].client_uid = Some("larkspur".into());
        fixture::write(&root, &spec).unwrap();
        let battlenet = BattleNet::at(Some(root.clone()));
        let larkspur = game(&battlenet, "lkh");
        assert_eq!(battlenet.target(&larkspur).unwrap().verified, None);
        assert_eq!(battlenet.installer().unwrap().install(&larkspur, None, &mut open), Err(Refusal::Unsupported));

        // One the client names, that the account does not own: the Agent refuses, and the client is asked.
        known(&root, &["quarrybell"]);
        assert!(battlenet.client_uids().contains(&"quarrybell".to_string()));
        let _served = standin::serve(&root, Duration::from_millis(300), |_| {}).unwrap();
        let quarry = game(&battlenet, "qbl");
        battlenet.installer().unwrap().install(&quarry, None, &mut open).unwrap();
        wait("the client asked", || !open.opened().is_empty());
        assert_eq!(open.opened(), [format!("{} --game=quarrybell", root.join(CLIENT_EXE).display())]);
    }

    /// The TV PC as the first install left it (#172): the game's files there, `product.db` naming it under the
    /// table's uid; the client shows Install. With Battle.net closed it is re-registered, nothing else touched.
    #[test]
    fn a_game_under_an_old_uid_is_re_registered_and_keeps_the_catalogues_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        let mut spec = Fixture::standard();
        spec.available[0].client_uid = Some("lkh".into());
        let folder = root.join("games").join("Larkspurs Hollow");
        // Installed by the Agent as `larkspur` (the table's), the Uninstall key named for the folder, not the game.
        spec.products.push(fixture::FixtureProduct {
            uid: "larkspur".into(),
            code: "lkh".into(),
            name: "Larkspurs Hollow".into(),
            kind: fixture::Kind::Unnamed,
            dir: Some(folder.clone()),
        });
        fixture::write(&root, &spec).unwrap();
        std::fs::write(folder.join(".build.info"), "made up").unwrap();
        let before = std::fs::read(root.join("Agent").join("product.db")).unwrap();
        let why = fixture::client_sees(&root, "lkh").unwrap_err();
        assert!(why.contains("shows Install") && why.contains("under larkspur"), "{why}");

        let battlenet = BattleNet::at(Some(root.clone()));
        let larkspur = game(&battlenet, "lkh");
        // The catalogue's name, not the Uninstall key's.
        assert_eq!((larkspur.name.as_str(), larkspur.installed), ("Larkspur Hollow", true));
        assert_eq!(fixture::client_sees(&root, "lkh").unwrap(), folder);
        let backup = root.join("Agent").join("product.db.dashboard-larkspur.bak");
        assert_eq!(std::fs::read(&backup).unwrap(), before);
        assert_eq!(
            std::fs::read_to_string(folder.join(".build.info")).unwrap(),
            "made up",
            "its files left as they are"
        );
        let products = products_in(Some(&root.join("Agent"))).unwrap();
        assert_eq!(products.iter().filter(|p| p.code == "lkh").map(|p| p.uid.as_str()).collect::<Vec<_>>(), ["lkh"]);
        assert_eq!(products.len(), parse_len(&before));
        // Started as any other now; and installing it again is nothing to do.
        assert_eq!(battlenet.cant_start(&larkspur), None);
        assert_eq!(battlenet.target(&larkspur).unwrap().stale, None);
    }

    fn parse_len(bytes: &[u8]) -> usize {
        product::parse(bytes).unwrap().len()
    }

    #[test]
    fn names_make_folders_and_paths_compare_as_windows_does() {
        assert_eq!(folder_name("Tony Hawk's Pro Skater 3 + 4"), "Tony Hawk's Pro Skater 3 + 4");
        assert_eq!(folder_name("DOOM: The Dark Ages"), "DOOM The Dark Ages");
        assert!(under(
            Path::new(r"C:\ProgramData\Battle.net\Agent"),
            Path::new("c:/programdata/battle.net/agent/Agent.9/Agent.exe")
        ));
        assert!(!under(
            Path::new(r"C:\ProgramData\Battle.net\Agent"),
            Path::new(r"C:\ProgramData\Battle.net\Agent2\Agent.exe")
        ));
    }
}
