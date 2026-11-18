//! Battle.net's games (docs/research/stores.md, "Battle.net"; docs/research/installs.md 7; docs/sources.md).
//!
//! - **Installed:** `%PROGRAMDATA%\Battle.net\Agent\product.db` (`product`): each product its agent says is
//!   installed (`base_product_state.installed`), at its `install_path`, that is not one of the client's own parts
//!   and whose folder is there. A game is its product code, its `source_id`: `emb`.
//! - **Names:** `aggregate.json` beside it (`installed[].product_id` and `name`); else the game's Uninstall key's
//!   `DisplayName`; else the catalogue's; else its uid.
//! - **Not installed:** the products of the catalogue (`catalogue`, a built-in table; a fake's `catalogue.json`),
//!   while the client is installed, as available to install. Whether the account owns one is not known until the
//!   Agent answers an install of it (#172).
//! - **The client:** the Uninstall key whose `UninstallString` has `-uid=battle.net` in it: its `InstallLocation`
//!   holds `Battle.net.exe`. With neither it nor `product.db`, Battle.net is not installed.
//! - **Start:** `Battle.net.exe --exec="launch <code>"`, the catalogue's launch code (`Pro`, `Fen`) where it has
//!   one, else the product code.
//! - **Install, uninstall, progress** (#172): through the client's own Agent, its local HTTP API (`agent`), with no
//!   window: install into a folder, the Queue's bytes and speed, uninstall. Pause, resume and cancel only with
//!   `DASHBOARD_BATTLENET_STEER=1` (no tool knows them: `agent::calls`). Where the Agent does not answer or take it,
//!   as before: `Battle.net.exe --game=<uid>`, the game's page in the client; uninstall by the game's Uninstall key's
//!   own `UninstallString`, Blizzard's uninstaller. Each fallback says why in the log.
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
}

impl BattleNet {
    /// This machine's Battle.net, or with `Some(dir)` the fake one at `dir` (`--battlenet`).
    pub fn at(fake: Option<PathBuf>) -> Self {
        Self { fake, jobs: installing::Jobs::default(), steer: agent::calls::steer_on() }
    }

    /// With pause, resume and cancel asked of the Agent, or not, whatever the environment says.
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

    /// What the game is to the Agent: its product in `product.db`, else in the catalogue.
    fn target(&self, game: &Game) -> Option<Target> {
        let code = code(game)?;
        let products = self.products().unwrap_or_default();
        let listed = self.catalogue().into_iter().find(|p| p.code.eq_ignore_ascii_case(code));
        if let Some(product) = products.iter().find(|p| p.code.eq_ignore_ascii_case(code)) {
            let folder = source_kit::native(product.install_path.trim());
            let installed = product.installed && !product.install_path.trim().is_empty() && folder.is_dir();
            return Some(Target {
                code: product.code.clone(),
                uid: product.uid.clone(),
                name: listed.map_or_else(|| game.name.clone(), |p| p.name),
                installed: installed.then_some(folder),
            });
        }
        let listed = listed?;
        Some(Target { code: listed.code, uid: listed.uid, name: listed.name, installed: None })
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
        config_install_path().or_else(|| std::env::var_os("ProgramFiles(x86)").map(PathBuf::from))
    }
}

/// What an install or uninstall asks the Agent about.
#[derive(Debug, Clone)]
struct Target {
    code: String,
    uid: String,
    name: String,
    /// Its folder, where `product.db` says it is installed and it is there.
    installed: Option<PathBuf>,
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

/// `%APPDATA%\Battle.net\Battle.net.config`'s `Client.Install.DefaultInstallPath`: the folder the client offers
/// for a new install. Read as the config is commonly read; not seen on the TV PC yet.
fn config_install_path() -> Option<PathBuf> {
    let config = PathBuf::from(std::env::var_os("APPDATA")?).join("Battle.net").join("Battle.net.config");
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config).ok()?).ok()?;
    let path = json.get("Client")?.get("Install")?.get("DefaultInstallPath")?.as_str()?.trim();
    (!path.is_empty()).then(|| source_kit::native(path))
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
            let name = names.get(&product.code).cloned().or(display).or(listed).unwrap_or_else(|| product.uid.clone());
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
        Can { pause: self.steer, cancel: self.steer, choose_folder: self.folders().len() > 1, quiet_uninstall: true }
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
            log(&format!("{}: installed already", target.code));
            return Ok(());
        }
        if !self.jobs.begin(&target.code, &target.uid) {
            log(&format!("{}: under way already", target.code));
            return Ok(());
        }
        let base = folder.map(|f| f.path.clone()).or_else(|| self.default_folder()).ok_or(Refusal::Unsupported)?;
        let game_dir = base.join(folder_name(&target.name));
        let fallback = self.ask(game, Ask::Install);
        let jobs = self.jobs.clone();
        log(&format!(
            "{}: installing {} ({}) into {} through the Agent",
            target.code,
            target.uid,
            target.name,
            game_dir.display()
        ));
        std::thread::spawn(move || {
            let code = target.code.clone();
            match agent_install(&place, &target, &game_dir) {
                Ok(session) => {
                    let agent = place.dir.clone();
                    installing::follow(jobs, session, code.clone(), move || installed_in(&agent, &code));
                }
                Err(why) => {
                    jobs.end(&code);
                    fall_back(&code, &format!("the Agent did not take the install ({why})"), fallback, &mut *detached);
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
            let asked = agent::connect(&place).and_then(|mut session| {
                let (method, path, body) = agent::calls::uninstall(&target.uid);
                let answer = session.call(&format!("uninstall {code}"), method, &path, Some(&body))?;
                agent::calls::error(&answer).map_or(Ok(()), Err)
            });
            if let Err(why) = asked {
                fall_back(&code, &format!("the Agent did not take the uninstall ({why})"), fallback, &mut *detached);
                return;
            }
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

    /// Only with `DASHBOARD_BATTLENET_STEER=1`, and only an install the Dashboard asked for: the Agent's answer is
    /// logged, and a refusal said; nothing falls back (the client has no command for it).
    fn steer(&self, game: &Game, steer: Steer, _open: &mut dyn Opener) -> Result<(), Refusal> {
        let code = code(game).ok_or(Refusal::Unsupported)?.to_string();
        if !self.steer || !self.jobs.has(&code) {
            return Err(Refusal::Unsupported);
        }
        let place = self.place().ok_or(Refusal::Unsupported)?;
        let uid = self.jobs.uid(&code).ok_or(Refusal::Unsupported)?;
        if steer == Steer::Resume && !self.jobs.paused(&code) {
            return Ok(());
        }
        let jobs = self.jobs.clone();
        std::thread::spawn(move || {
            let what = format!("{} {code}", steer.verb());
            let asked = agent::connect(&place).and_then(|mut session| {
                let answer = match steer {
                    Steer::Pause | Steer::Resume => {
                        let (method, path, body) = agent::calls::pause(&uid, steer == Steer::Pause);
                        session.call(&what, method, &path, Some(&body))?
                    }
                    Steer::Cancel => {
                        let (method, path) = agent::calls::cancel(&uid);
                        session.call(&what, method, &path, None)?
                    }
                };
                agent::calls::error(&answer).map_or(Ok(()), Err)
            });
            match asked {
                Ok(()) if steer == Steer::Cancel => {
                    jobs.stop(&code);
                    log(&format!("{code}: cancelled: the Agent took it"));
                }
                Ok(()) => log(&format!("{code}: {}: the Agent took it", steer.verb())),
                Err(why) => log(&format!("{code}: {}: the Agent did not take it ({why})", steer.verb())),
            }
        });
        Ok(())
    }
}

/// Asks the Agent to install `target` into `game_dir`: `POST /install`, `POST /install/<uid>`, `POST /update`.
/// The session to follow it with, or why not.
fn agent_install(place: &agent::Place, target: &Target, game_dir: &Path) -> Result<agent::Session, String> {
    let mut session = agent::connect(place)?;
    let code = &target.code;
    let what = format!("install {code}");
    let (method, path, body) = agent::calls::install(&target.uid, &session.region, code);
    let answer = session.call(&what, method, &path, Some(&body))?;
    if let Some(error) = agent::calls::error(&answer) {
        return Err(said(&error));
    }
    let (method, path, body) = agent::calls::install_options(&target.uid, &game_dir.display().to_string());
    let answer = session.call(&what, method, &path, Some(&body))?;
    if let Some(error) = agent::calls::error(&answer) {
        return Err(said(&error));
    }
    let (method, path, body) = agent::calls::start(&target.uid);
    match session.call(&what, method, &path, Some(&body)).map(|answer| agent::calls::error(&answer)) {
        Ok(None) => {}
        Ok(Some(error)) => {
            log(&format!("{code}: POST /update said {}: following the install all the same", said(&error)))
        }
        Err(why) => log(&format!("{code}: POST /update had no answer ({why}): following the install all the same")),
    }
    log(&format!("{code}: the Agent took the install"));
    Ok(session)
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

    /// Keeps what it was asked to open, here and from its detached copies.
    #[derive(Clone, Default)]
    struct Kept(Arc<Mutex<Vec<String>>>);

    impl Opener for Kept {
        fn open(&mut self, start: &Start) -> Result<(), String> {
            self.0.lock().unwrap().push(start.to_string());
            Ok(())
        }

        fn detached(&self) -> Option<Box<dyn Opener + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl Kept {
        fn opened(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
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
        assert_eq!(larkspur.install_dir, Some(root.join("games").join("Larkspur Hollow")));
        assert!(battlenet.installed_now().unwrap().contains(&"lkh".to_string()));
        assert!(open.opened().is_empty(), "{:?}", open.opened());
        // Every request said, the token never.
        let lines = said.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("POST /install {")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("\"game_dir\"")), "{lines:?}");
        assert!(
            lines.iter().all(|l| !l.contains(&format!("standin-{}-{}", std::process::id(), served.port))),
            "{lines:?}"
        );

        // Uninstalled: gone from product.db and its folder, and still in the catalogue, not installed.
        installer.uninstall(&larkspur, &mut open).unwrap();
        wait("uninstalled", || !battlenet.installed_now().unwrap().contains(&"lkh".to_string()));
        assert!(!game(&battlenet, "lkh").installed);
        assert!(!root.join("games").join("Larkspur Hollow").exists());
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
        assert!(!root.join("games").join("Larkspur Hollow").exists());
        assert!(!game(&battlenet, "lkh").installed);
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
