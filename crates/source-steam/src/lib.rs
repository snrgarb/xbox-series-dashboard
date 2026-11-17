//! Reads a Steam install into a `model::Library`.
//!
//! Sources, all under the Steam root (same layout on Windows and in the fixture):
//! - `appcache/appinfo.vdf`: name, app type and art file names per app; and who made and published it, when it
//!   came out and its scores, where Steam has cached them (`credits`, `released`, `score`).
//! - `appcache/packageinfo.vdf`: which apps the account's packages grant (ownership).
//! - `userdata/<id>/config/localconfig.vdf`: persona name, last played, playtime.
//! - `steamapps/appmanifest_*.acf`: what is installed, where, and how much of the disk it takes.
//! - `appcache/librarycache/<appid>/`: cover, hero and header art.
//! - `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json`: the user's collections, Steam's Favourites
//!   and its hidden games (`collections`).
//!
//! [`Steam`] is the source the library reads (`model::Source`): it finds the install, reads it with [`load`],
//! starts a game through Steam's own `steam://` address, and installs, uninstalls, pauses, resumes and cancels
//! through its `model::Installer`: Steam's own client calls over its CEF debugger where the user has that on
//! ([`client`], #171), else as the Bundle does, without Steam's windows ([`command`]; the shell types it into
//! Steam's console over the debugger where it can, [`cef`]). It reports the installs Steam has under way from its
//! app manifests ([`installs`]), how far each has got from Steam's own download overview where the debugger is on
//! ([`overview`]) and else counted live ([`live`]), none said done while Steam is still at it ([`follow`]), and a game
//! installed whose Workshop items are still coming in as such, with how many are in ([`installs::workshop`],
//! #188). [`probe`] is `--log-queue-sources`' look at every candidate count. [`debugger`] is the end-to-end tests'
//! stand-in for the client's debugger.
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod achievements;
pub mod appcache;
pub mod cef;
pub mod client;
pub mod collections;
pub mod debugger;
pub mod eula;
pub mod fixture;
pub mod follow;
pub mod installs;
pub mod live;
pub mod overview;
pub mod probe;
pub mod vdf;

use anyhow::{Context, Result};
use model::{Ask, Can, Folder, Found, Game, Install, Installer, Library, Opener, Refusal, Source, Start, Steer};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use vdf::Value;

/// Steam, as a game's `source` names it.
pub const SOURCE: &str = "steam";

/// Steam, as the library reads it and the shell starts its games.
#[derive(Debug, Clone)]
pub struct Steam {
    /// The install to read; with none, the one the registry names.
    root: Option<PathBuf>,
    /// What is kept between looks at the installs under way ([`follow`]): the one thing a source keeps, as the
    /// Bundle's `SteamDownloads` keeps its baselines.
    follow: Arc<Mutex<follow::Follow>>,
    /// The clock `follow` goes by.
    started: Arc<std::time::Instant>,
}

impl Default for Steam {
    fn default() -> Self {
        Self::at(None)
    }
}

impl Steam {
    /// The Steam install at `root`, or with none the one the registry names (`--steam` on the command line).
    pub fn at(root: Option<PathBuf>) -> Self {
        Self { root, follow: Arc::default(), started: Arc::new(std::time::Instant::now()) }
    }

    fn root(&self) -> Option<PathBuf> {
        self.root.clone().or_else(find_steam_root)
    }

    /// The ports of the client's debugger, where the user has it on (`cef::enabled`) and there is a port to ask
    /// (`cef::ports`: the stand-in's only once its debugger is up): the route for Steam's own calls ([`client`]).
    /// Whether the client is running with it open is known only by asking.
    fn debugger(&self) -> Option<Vec<u16>> {
        let root = self.installed_root()?;
        let ports = cef::ports(&root);
        (cef::enabled(&root) && !ports.is_empty()).then_some(ports)
    }

    /// Asks Steam's client `call` on a thread of its own (the Dashboard's window never waits on its debugger), and
    /// where it is not taken, opens `fallback` with `open`: Steam's own program, as before #171. After an install,
    /// whose wizard brings Steam's main window up, the Dashboard is put in front again (`Opener::front`). An
    /// install's licence agreements are put before the user through `open` (`Opener::agreement`), and an install
    /// that stopped is told of there too (`Opener::stopped`, #230): neither falls back to `steam.exe`, which would go
    /// round them.
    fn ask_client(
        &self,
        ports: Vec<u16>,
        call: client::Call,
        game: Game,
        fallback: Option<(Start, Box<dyn Opener + Send>)>,
    ) {
        let root = self.installed_root();
        std::thread::spawn(move || {
            let install = matches!(call, client::Call::Install { .. });
            let mut fallback = fallback;
            let outcome = match &mut fallback {
                Some((_, open)) if install => {
                    let mut asked = |agreement: &model::Agreement| {
                        open.front();
                        open.agreement(&game, agreement)
                    };
                    client::ask_user(&ports, root.as_deref(), &call, &mut asked)
                }
                _ => client::ask(&ports, root.as_deref(), &call),
            };
            match (outcome, fallback) {
                (client::Outcome::NotTaken(_), Some((start, mut open))) => {
                    if install {
                        open.front();
                    }
                    client::log(&format!("{}: asking through steam.exe instead", call.what()));
                    if let Err(why) = open.open(&start) {
                        client::log(&format!("{}: steam.exe could not be asked either: {why}", call.what()));
                    }
                }
                (client::Outcome::NotTaken(_), None) => {
                    client::log(&format!("{}: nothing else can do it: the Queue stays as it is", call.what()));
                }
                (client::Outcome::Stopped(stop), Some((_, mut open))) => {
                    open.front();
                    open.stopped(&game, &stop);
                }
                (client::Outcome::LeftToSteam, Some((_, mut open))) => {
                    let address = Start::Address(format!("steam://install/{}", call.appid()));
                    client::log(&format!("{}: opening {address} for Steam's own install dialog", call.what()));
                    if let Err(why) = open.open(&address) {
                        client::log(&format!("{}: {address} could not be opened: {why}", call.what()));
                        open.front();
                        open.stopped(&game, &model::Stop::Failed("Steam's own install couldn't be opened.".into()));
                    }
                }
                (_, Some((_, mut open))) if install => open.front(),
                _ => {}
            }
        });
    }

    /// The install's root, unless it is a named folder left empty: a machine with no Steam, as a test names one so
    /// that the machine's own is never read.
    pub fn installed_root(&self) -> Option<PathBuf> {
        let root = self.root()?;
        let empty = self.root.is_some() && fs::read_dir(&root).is_ok_and(|mut entries| entries.next().is_none());
        (!empty).then_some(root)
    }

    /// Steam's own program: a named install's `steam.exe` and never another; else the one the registry records
    /// (`SteamExe`), else the one in the registry's install folder. None when there is none on disk.
    fn exe(&self) -> Option<PathBuf> {
        if let Some(root) = &self.root {
            return child(root, "steam.exe").filter(|exe| exe.is_file());
        }
        let recorded = registry::steam_exe().map(|exe| PathBuf::from(exe.replace('/', "\\")));
        recorded.filter(|exe| exe.is_file()).or_else(|| child(&find_steam_root()?, "steam.exe").filter(|e| e.is_file()))
    }
}

/// What `steam.exe` is given to ask the running client to install or uninstall an app, as the Bundle gives it when
/// Steam's CEF debugger is not on: `-silent +app_install <appid>`, which starts the download with no install dialog
/// and leaves Steam in the tray, and `-silent -console +app_uninstall <appid>`, which uninstalls with no
/// confirmation of Steam's (the Dashboard has asked "Uninstall X?" already) and no Console page. A Steam that is not
/// running is started by it, `-silent` keeping its main window down.
pub fn command(ask: Ask, appid: u32) -> Vec<String> {
    let verb: &[&str] = match ask {
        Ask::Install => &["-silent", "+app_install"],
        Ask::Uninstall => &["-silent", "-console", "+app_uninstall"],
    };
    verb.iter().map(|arg| arg.to_string()).chain(Some(appid.to_string())).collect()
}

impl Source for Steam {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "Steam"
    }

    fn read(&self) -> Result<Option<Found>> {
        let Some(root) = self.installed_root() else { return Ok(None) };
        let library = load(&root).with_context(|| root.display().to_string())?;
        Ok(Some(Found { library, from: root.display().to_string() }))
    }

    fn start(&self, game: &Game) -> Option<Start> {
        appid(game).map(|appid| Start::Address(format!("steam://rungameid/{appid}")))
    }

    /// Steam's own program given the Bundle's command ([`command`]), run in its folder; with no `steam.exe` to be
    /// found, `steam://install/<appid>` and `steam://uninstall/<appid>`, for which Steam shows its own window.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        let appid = appid(game)?;
        Some(match self.exe() {
            Some(exe) => {
                Start::Program { dir: exe.parent().map(Path::to_path_buf), path: exe, args: command(ask, appid) }
            }
            None => Start::Address(format!("steam://{}/{appid}", ask.verb())),
        })
    }

    /// Steam's own word: `HKCU\Software\Valve\Steam\Apps\<appid>\Running`.
    fn running(&self, game: &Game) -> bool {
        appid(game).is_some_and(registry::running)
    }

    /// From the app manifests in every library folder ([`installs::under_way`]), with the registry's `Updating` as
    /// a second word, for the install the registry names only; how far each has got and whether it is done as
    /// [`follow`] works it out, from Steam's download overview where its CEF debugger is on and Steam runs
    /// ([`overview`]), else from what Steam's client has written ([`live`]).
    fn installs(&self) -> Option<Vec<Install>> {
        let root = self.installed_root()?;
        let registry = self.root.is_none();
        let updating = |appid| registry && registry::updating(appid);
        let found = installs::under_way(&steamapps_dirs(&root), &updating);
        let mut follow = self.follow.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let busy = !found.is_empty() || follow.busy();
        let writes = if busy { live::writes(&root) } else { None };
        // Steam's own word, where it can be had: its debugger opened and its client running.
        let read_overview = writes.is_some() && cef::enabled(&root);
        let overview = read_overview.then(|| overview::read(&cef::ports(&root))).and_then(Result::ok);
        let dirs = steamapps_dirs(&root);
        let workshop = |appid| installs::workshop_pending(&dirs, appid);
        let at = self.started.elapsed().as_secs_f64();
        let mut found = follow.look_with_workshop(found, overview.as_ref(), writes, &updating, &workshop, at);
        // #188: an app held installing while Steam fetches the Workshop items the account subscribes to is
        // installed and playable; only its extras are still coming in, read from its Workshop record and their
        // folders. No speed: Steam's is not known to be theirs, and 0 would say nothing true.
        for install in &mut found {
            let held = install.source_id.parse().ok().filter(|&appid| follow.held(appid));
            let fetching = held.and_then(|appid| installs::workshop(&dirs, appid)).filter(|w| w.needs_download);
            if let Some(workshop) = fetching {
                install.state = workshop.state();
                install.speed = None;
            }
        }
        Some(found)
    }

    /// The apps whose manifest, in any library, says installed (`installs::installed`), as [`load`] reads them.
    fn installed_now(&self) -> Option<Vec<String>> {
        let root = self.installed_root()?;
        let mut ids: Vec<String> = installed_apps(&root)
            .into_iter()
            .filter(|(_, manifest)| manifest.installed)
            .map(|(appid, _)| appid.to_string())
            .collect();
        ids.sort();
        Some(ids)
    }

    /// Asking is all it takes: `+app_uninstall` asks the user nothing.
    fn uninstalls_by_itself(&self) -> bool {
        true
    }

    fn installer(&self) -> Option<&dyn Installer> {
        Some(self)
    }
}

/// Steam's installer (#171). Where the user has Steam's CEF debugger on, Steam's own client calls over it
/// ([`client`]): install into a library, uninstall with no prompt, pause, resume and cancel. Each is asked on a
/// thread of its own and says in the log what it asked and what came back; an install or uninstall the client does
/// not take is asked as before, through `steam.exe -silent` (what [`Source::ask`] gives, which the shell opens
/// without Steam's windows, typing it into the client's console first where it can). With the debugger off that is
/// the only route: into Steam's default library, and no Pause, Resume or Cancel. The stand-in Steam of the
/// end-to-end tests answers the same calls on a debugger of its own ([`debugger`]).
impl Installer for Steam {
    fn can(&self) -> Can {
        let client = self.debugger().is_some();
        Can { pause: client, cancel: client, choose_folder: client && self.folders().len() > 1, quiet_uninstall: true }
    }

    /// Steam's libraries, as `libraryfolders.vdf` names them, each by its root (the folder that holds its
    /// `steamapps`, as Steam's `GetInstallFolders` names it), the Steam folder's own last and taken as Steam's
    /// default: Steam's own default is its `bIsDefaultFolder`, which only the client says, and an install to the
    /// default asks the client for that one ([`client::Call::Install`]). Free space is not read.
    fn folders(&self) -> Vec<Folder> {
        let Some(root) = self.installed_root() else { return Vec::new() };
        let dirs = steamapps_dirs(&root);
        let last = dirs.len().saturating_sub(1);
        let library = |dir: &Path| dir.parent().map_or_else(|| dir.to_path_buf(), Path::to_path_buf);
        dirs.iter()
            .enumerate()
            .map(|(at, dir)| Folder { path: library(dir), free: None, default: at == last })
            .collect()
    }

    /// Into `folder` through the client where the debugger is on: Steam's own default for the default folder or
    /// none, the library named for any other. Else, or where the client does not take it, as the Bundle asks:
    /// Steam puts it in its default library.
    fn install(&self, game: &Game, folder: Option<&Folder>, open: &mut dyn Opener) -> Result<(), Refusal> {
        let start = self.ask(game, Ask::Install).ok_or(Refusal::Unsupported)?;
        let appid = appid(game).ok_or(Refusal::Unsupported)?;
        match (self.debugger(), open.detached()) {
            (Some(ports), Some(detached)) => {
                let folder = folder.filter(|folder| !folder.default).map(|folder| folder.path.display().to_string());
                self.ask_client(ports, client::Call::Install { appid, folder }, game.clone(), Some((start, detached)));
                Ok(())
            }
            _ => open.open(&start).map_err(Refusal::Failed),
        }
    }

    /// Through the client where the debugger is on, else as the Bundle asks: either way with no prompt of Steam's.
    fn uninstall(&self, game: &Game, open: &mut dyn Opener) -> Result<(), Refusal> {
        let start = self.ask(game, Ask::Uninstall).ok_or(Refusal::Unsupported)?;
        let appid = appid(game).ok_or(Refusal::Unsupported)?;
        match (self.debugger(), open.detached()) {
            (Some(ports), Some(detached)) => {
                self.ask_client(ports, client::Call::Uninstall { appid }, game.clone(), Some((start, detached)));
                Ok(())
            }
            _ => open.open(&start).map_err(Refusal::Failed),
        }
    }

    /// Only through the client: `steam.exe` has no word for it.
    fn steer(&self, game: &Game, steer: Steer, _open: &mut dyn Opener) -> Result<(), Refusal> {
        let appid = appid(game).ok_or(Refusal::Unsupported)?;
        let ports = self.debugger().filter(|_| steer.allowed(self.can())).ok_or(Refusal::Unsupported)?;
        self.ask_client(ports, client::Call::Steer { appid, steer }, game.clone(), None);
        Ok(())
    }
}

/// Whether `path` is under `folder`, by either kind of slash and in any case, as Windows compares paths.
pub fn under(folder: &Path, path: &Path) -> bool {
    let (folder, path) = (folder.to_string_lossy().to_lowercase(), path.to_string_lossy().to_lowercase());
    let (folder, path) = (folder.replace('/', "\\"), path.replace('/', "\\"));
    let folder = folder.trim_end_matches('\\');
    path.strip_prefix(folder).is_some_and(|rest| rest.starts_with('\\'))
}

/// Every `steamapps` folder of the install at `root`: its libraries' and its own.
pub fn libraries(root: &Path) -> Vec<PathBuf> {
    steamapps_dirs(root)
}

/// The game's appid, if it is one of Steam's.
fn appid(game: &Game) -> Option<u32> {
    (game.source == SOURCE).then(|| game.source_id.parse().ok()).flatten().filter(|&appid| appid != 0)
}

/// Read the Steam install at `steam_root`.
pub fn load(steam_root: &Path) -> Result<Library> {
    let appcache = child(steam_root, "appcache").context("no appcache folder under the Steam root")?;
    let appinfo_path = child(&appcache, "appinfo.vdf").context("no appcache/appinfo.vdf")?;
    let appinfo = appcache::parse_appinfo(&fs::read(&appinfo_path)?)?;

    let (local, config) = load_localconfig(steam_root);
    // With no file, or one that is not what is expected, there is no word on collections: which is not "none".
    let collections = config
        .as_deref()
        .map(collections::load)
        .and_then(|read| read.map_err(|e| eprintln!("steam: the collections were not read: {e:#}")).ok());
    let (favourites, hidden) = collections.as_ref().map(|c| (&c.favourites, &c.hidden)).unzip();
    let marked = |set: Option<&HashSet<u32>>, appid: u32| set.is_some_and(|set| set.contains(&appid));
    let installed = installed_apps(steam_root);

    // Ownership: apps granted by a cached package. Without packageinfo.vdf,
    // fall back to apps the user has local config for (i.e. has launched).
    let packaged = child(&appcache, "packageinfo.vdf")
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| appcache::parse_package_apps(&b).ok());
    let mut owned: HashSet<u32> = match packaged {
        Some(apps) => apps,
        None => local.apps.keys().copied().collect(),
    };
    owned.extend(installed.keys().copied());

    let art_root = child(&appcache, "librarycache");
    let mut games: Vec<(u32, Game)> = owned
        .into_iter()
        .filter_map(|appid| {
            let info = appinfo.get(&appid)?;
            let common = info.get("common")?;
            if !common.str_at(&["type"])?.eq_ignore_ascii_case("game") {
                return None;
            }
            let name = common.str_at(&["name"])?.to_string();
            let (played, minutes) = local.apps.get(&appid).copied().unwrap_or((0, 0));
            let manifest = installed.get(&appid);
            let manifest_played = manifest.map(|m| m.played);
            let art = art_root.as_deref().and_then(|r| child(r, &appid.to_string())).map(|d| ArtDir::scan(&d));
            let find =
                |asset: &str, legacy: Option<&str>, names: &[&str]| art.as_ref()?.find(common, asset, legacy, names);
            Some((
                appid,
                Game {
                    id: 0,
                    source: SOURCE.to_string(),
                    source_id: appid.to_string(),
                    name,
                    last_played: played.max(manifest_played.unwrap_or(0)),
                    playtime_minutes: minutes,
                    added: 0,
                    installed_at: None,
                    installed: manifest.is_some_and(|m| m.installed),
                    install_dir: manifest.filter(|m| m.installed).and_then(|m| m.folder.clone()),
                    cover: find("library_capsule", None, &["library_600x900.jpg", "library_capsule.jpg"]),
                    hero: find("library_hero", None, &["library_hero.jpg"]),
                    header: find("library_header", Some("header_image"), &["header.jpg", "library_header.jpg"]),
                    // Steam keeps nothing square but a 32 px icon: the art lookup finds one (docs/sources.md, "Art").
                    square: None,
                    // Its appid is all Steam's own art online needs.
                    online: Default::default(),
                    favourite: marked(favourites, appid),
                    hidden: marked(hidden, appid),
                    released: released(common),
                    developers: credits(info, "developer"),
                    publishers: credits(info, "publisher"),
                    size_on_disk: manifest.filter(|m| m.installed).map_or(0, |m| m.size),
                    critic_score: score(common, "metacritic_score"),
                    community_score: score(common, "review_percentage"),
                    play_count: 0,
                    art: Default::default(),
                },
            ))
        })
        .collect();
    games.sort_by(|(a_id, a), (b_id, b)| {
        b.last_played
            .cmp(&a.last_played)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a_id.cmp(b_id))
    });
    let games = games.into_iter().map(|(_, game)| game).collect();
    let collections = collections.map(|c| c.collections);
    Ok(Library { user: local.persona, games, collections, ..Library::default() })
}

/// The Steam install folder from the registry (Windows only).
pub fn find_steam_root() -> Option<PathBuf> {
    // The registry value uses forward slashes ("c:/program files (x86)/steam").
    registry::steam_path().map(|s| PathBuf::from(s.replace('/', "\\"))).filter(|p| p.is_dir())
}

#[derive(Default)]
struct LocalConfig {
    persona: Option<String>,
    /// appid -> (last played unix seconds, playtime minutes)
    apps: HashMap<u32, (u64, u32)>,
}

/// The most recently written `userdata/*/config/localconfig.vdf`, and the `config` folder it is in: the signed-in
/// user's.
fn load_localconfig(steam_root: &Path) -> (LocalConfig, Option<PathBuf>) {
    let newest = newest_localconfig(steam_root);
    let config = newest.as_deref().and_then(Path::parent).map(Path::to_path_buf);
    let parsed = newest.and_then(|p| fs::read(p).ok()).and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
    (parsed.map_or_else(LocalConfig::default, |v| parse_localconfig(&v)), config)
}

/// The most recently written `userdata/<id>/config/localconfig.vdf`: the signed-in user's.
fn newest_localconfig(steam_root: &Path) -> Option<PathBuf> {
    let userdata = child(steam_root, "userdata")?;
    fs::read_dir(userdata)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().parse::<u64>().is_ok_and(|id| id != 0))
        .filter_map(|e| child(&child(&e.path(), "config")?, "localconfig.vdf"))
        .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// The signed-in user's account id, their folder's name under `userdata` (the low 32 bits of their SteamID): that
/// of the most recently written `localconfig.vdf`.
pub fn signed_in_account(steam_root: &Path) -> Option<u32> {
    let config = newest_localconfig(steam_root)?;
    let user = config.parent()?.parent()?.file_name()?;
    user.to_str()?.parse().ok()
}

fn parse_localconfig(v: &Value) -> LocalConfig {
    let mut out = LocalConfig::default();
    let Some(root) = v.get("UserLocalConfigStore") else { return out };
    out.persona = root.str_at(&["friends", "PersonaName"]).filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(apps) = root.path(&["Software", "Valve", "Steam", "apps"]) {
        for (id, app) in apps.entries() {
            let Ok(id) = id.parse::<u32>() else { continue };
            let played = app.u64_at(&["LastPlayed"]).unwrap_or(0);
            let minutes = app.u64_at(&["Playtime"]).unwrap_or(0).min(u32::MAX as u64) as u32;
            out.apps.insert(id, (played, minutes));
        }
    }
    out
}

/// When the game came out, in Unix seconds, or 0: `common/original_release_date` where Steam has one (a game
/// that came to Steam after it came out), else `common/steam_release_date`.
fn released(common: &Value) -> u64 {
    let date = |key: &str| common.u64_at(&[key]).filter(|&date| date > 0);
    date("original_release_date").or_else(|| date("steam_release_date")).unwrap_or(0)
}

/// The game's developers or its publishers (`kind` is `developer` or `publisher`), in Steam's order: the names in
/// `common/associations` of that type, or with none there the one name in `extended/<kind>`.
fn credits(info: &Value, kind: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let associations = info.path(&["common", "associations"]).map_or(&[][..], Value::entries);
    for (_, association) in associations {
        let of_kind = association.str_at(&["type"]).is_some_and(|ty| ty.eq_ignore_ascii_case(kind));
        let name = association.str_at(&["name"]).map(str::trim).filter(|name| of_kind && !name.is_empty());
        if let Some(name) = name.filter(|name| !names.iter().any(|known| known == name)) {
            names.push(name.to_string());
        }
    }
    if names.is_empty() {
        let name = info.str_at(&["extended", kind]).map(str::trim).filter(|name| !name.is_empty());
        names.extend(name.map(str::to_string));
    }
    names
}

/// One of `common`'s scores, when it is there and is 0 to 100.
fn score(common: &Value, key: &str) -> Option<u8> {
    common.u64_at(&[key]).and_then(|score| u8::try_from(score).ok()).filter(|&score| score <= 100)
}

/// What an `appmanifest_*.acf` says of a game Steam has, installed or on its way.
struct Manifest {
    /// Its files are all on disk, and no uninstall is under way (`installs::installed`).
    installed: bool,
    /// Its LastPlayed, in Unix seconds.
    played: u64,
    /// The game's folder.
    folder: Option<PathBuf>,
    /// Its SizeOnDisk, in bytes.
    size: u64,
}

/// The `steamapps` folders of every library: those `libraryfolders.vdf` names that exist here, then the Steam
/// root's own.
fn steamapps_dirs(steam_root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(main) = child(steam_root, "steamapps") {
        let folders = child(&main, "libraryfolders.vdf")
            .and_then(|p| fs::read(p).ok())
            .and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
        if let Some(list) = folders.as_ref().and_then(|v| v.get("libraryfolders")) {
            for (_, folder) in list.entries() {
                let Some(path) = folder.str_at(&["path"]) else { continue };
                if let Some(dir) = child(Path::new(path), "steamapps") {
                    if !same_dir(&dir, &main) {
                        dirs.push(dir);
                    }
                }
            }
        }
        dirs.push(main);
    }
    dirs
}

/// appid -> its manifest, for every `appmanifest_*.acf` in the Steam root's `steamapps` and in any other library
/// folder that exists here.
fn installed_apps(steam_root: &Path) -> HashMap<u32, Manifest> {
    let mut out = HashMap::new();
    for dir in steamapps_dirs(steam_root) {
        for (id, file) in installs::manifests(&dir) {
            let manifest = fs::read(file).ok().and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
            let played = manifest.as_ref().and_then(|v| v.u64_at(&["AppState", "LastPlayed"])).unwrap_or(0);
            let folder = manifest
                .as_ref()
                .and_then(|v| v.str_at(&["AppState", "installdir"]))
                .map(|name| dir.join("common").join(name));
            let size = manifest.as_ref().and_then(|v| v.u64_at(&["AppState", "SizeOnDisk"])).unwrap_or(0);
            let flags = manifest.as_ref().and_then(|v| v.u64_at(&["AppState", "StateFlags"]));
            out.insert(id, Manifest { installed: installs::installed(flags), played, folder, size });
        }
    }
    out
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// `dir/name`, matching `name` case-insensitively if the exact name is absent
/// (Steam's own casing varies, and the Linux fixture is case-sensitive).
fn child(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

/// The files of one `librarycache/<appid>` folder: flat (older clients) and one
/// level of hash-named subfolders (newer clients).
struct ArtDir {
    dir: PathBuf,
    /// Paths relative to `dir`, '/'-separated, as named on disk.
    files: Vec<String>,
}

impl ArtDir {
    fn scan(dir: &Path) -> ArtDir {
        let mut files = Vec::new();
        for e in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                for sub in fs::read_dir(e.path()).into_iter().flatten().flatten() {
                    files.push(format!("{name}/{}", sub.file_name().to_string_lossy()));
                }
            } else {
                files.push(name);
            }
        }
        files.sort();
        ArtDir { dir: dir.to_path_buf(), files }
    }

    /// Prefer the file appinfo names for `asset` (English, else any language),
    /// then fall back to the well-known file names anywhere in the folder.
    fn find(&self, common: &Value, asset: &str, legacy: Option<&str>, names: &[&str]) -> Option<PathBuf> {
        let mut wanted: Vec<&str> = Vec::new();
        let by_lang = [common.path(&["library_assets_full", asset, "image"]), legacy.and_then(|k| common.get(k))];
        for langs in by_lang.into_iter().flatten() {
            wanted.extend(langs.str_at(&["english"]));
            wanted.extend(langs.entries().iter().filter_map(|(_, v)| v.as_str()));
        }
        let hit = wanted
            .iter()
            .find_map(|w| {
                // appinfo says "<hash>/<file>"; on disk that is either that
                // path or, in the older flat layout, just "<file>".
                let base = w.rsplit('/').next().unwrap_or(w);
                self.files.iter().find(|f| f.eq_ignore_ascii_case(w) || f.eq_ignore_ascii_case(base))
            })
            .or_else(|| {
                names.iter().find_map(|n| {
                    self.files.iter().find(|f| f.rsplit('/').next().is_some_and(|b| b.eq_ignore_ascii_case(n)))
                })
            })?;
        Some(hit.split('/').fold(self.dir.clone(), |p, part| p.join(part)))
    }
}

#[cfg(windows)]
mod registry {
    use std::ffi::c_void;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            ty: *mut u32,
            data: *mut c_void,
            len: *mut u32,
        ) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// `HKCU\Software\Valve\Steam\SteamPath`
    pub fn steam_path() -> Option<String> {
        string("SteamPath")
    }

    /// `HKCU\Software\Valve\Steam\SteamExe`: Steam's own program, as Steam records it.
    pub fn steam_exe() -> Option<String> {
        string("SteamExe")
    }

    /// A text value of `HKCU\Software\Valve\Steam`.
    fn string(name: &str) -> Option<String> {
        let (key, value) = (wide("Software\\Valve\\Steam"), wide(name));
        let mut buf = [0u16; 1024];
        let mut len = std::mem::size_of_val(&buf) as u32;
        // SAFETY: all pointers are valid for the call; `len` is the buffer size in bytes.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if rc != 0 {
            return None;
        }
        let chars = &buf[..(len as usize / 2).min(buf.len())];
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        Some(String::from_utf16_lossy(&chars[..end]))
    }

    /// `HKCU\Software\Valve\Steam\Apps\<appid>\Running`, which Steam keeps at 1 while the game runs.
    pub fn running(appid: u32) -> bool {
        flag(appid, "Running")
    }

    /// `HKCU\Software\Valve\Steam\Apps\<appid>\Updating`, which Steam sets as it works on the app's download,
    /// sooner than its manifest says so.
    pub fn updating(appid: u32) -> bool {
        flag(appid, "Updating")
    }

    /// Whether a number of `HKCU\Software\Valve\Steam\Apps\<appid>` is there and not 0.
    fn flag(appid: u32, name: &str) -> bool {
        let (key, value) = (wide(&format!("Software\\Valve\\Steam\\Apps\\{appid}")), wide(name));
        let (mut data, mut len) = (0u32, 4u32);
        // SAFETY: all pointers are valid for the call; `len` is the size of `data` in bytes.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut data as *mut u32).cast(),
                &mut len,
            )
        };
        rc == 0 && data != 0
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn steam_path() -> Option<String> {
        None
    }

    pub fn steam_exe() -> Option<String> {
        None
    }

    pub fn running(_appid: u32) -> bool {
        false
    }

    pub fn updating(_appid: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Kept(Vec<String>);

    impl Opener for Kept {
        fn open(&mut self, start: &Start) -> Result<(), String> {
            self.0.push(start.to_string());
            Ok(())
        }
    }

    /// An opener a thread can keep, as the shell's are: what it opened, shared.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<String>>>);

    impl Opener for Shared {
        fn open(&mut self, start: &Start) -> Result<(), String> {
            self.0.lock().unwrap().push(start.to_string());
            Ok(())
        }

        fn detached(&self) -> Option<Box<dyn Opener + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    fn steam_game(appid: &str) -> Game {
        Game { source: SOURCE.into(), source_id: appid.into(), ..Game::default() }
    }

    /// Waits up to five seconds for `done`: the client is asked on a thread of its own.
    fn soon(done: impl Fn() -> bool) -> bool {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done() {
            if std::time::Instant::now() > until {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        true
    }

    /// The fake Steam at `root`, as `standin make-steam` writes it: marked the stand-in, with a `steam.exe`.
    fn standin(root: &Path) -> Steam {
        fixture::write(root, &fixture::Fixture::standard()).unwrap();
        std::fs::write(root.join("steam.exe"), "").unwrap();
        std::fs::write(root.join(fixture::STANDIN_CLIENT), "").unwrap();
        Steam::at(Some(root.to_path_buf()))
    }

    #[test]
    fn with_the_debugger_on_steams_own_calls_install_steer_and_uninstall_and_nothing_is_opened() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let steam = standin(&root);
        let installer = steam.installer().unwrap();
        // The debugger's file, but no debugger up yet: no port to ask, so nothing but the old route.
        std::fs::write(root.join(cef::FILE), "").unwrap();
        assert_eq!(installer.can(), Can { quiet_uninstall: true, ..Can::NOTHING });
        let _debugger = debugger::Debugger::start(&root, |_| {}).unwrap();
        assert_eq!(installer.can(), Can { pause: true, cancel: true, quiet_uninstall: true, choose_folder: false });

        let quiet = "9100040";
        let game = steam_game(quiet);
        let state = || installs::under_way(&libraries(&root), &|_| false).first().map(|found| found.1);
        let opened = Shared::default();
        let mut open = opened.clone();
        let folders = installer.folders();
        assert_eq!(folders.len(), 1);
        assert!(folders[0].default && folders[0].path == root);
        installer.install(&game, model::installer::default_folder(&folders), &mut open).unwrap();
        assert!(soon(|| state() == Some(model::InstallState::Queued)), "{:?}", state());
        fixture::write_progress(&root, 9_100_040, fixture::Progress::Downloading { done: 25, total: 100 }).unwrap();
        installer.steer(&game, Steer::Pause, &mut open).unwrap();
        assert!(soon(|| state() == Some(model::InstallState::Paused { done: 25, total: 100 })));
        installer.steer(&game, Steer::Resume, &mut open).unwrap();
        assert!(soon(|| state() == Some(model::InstallState::Downloading { done: 25, total: 100 })));
        installer.steer(&game, Steer::Cancel, &mut open).unwrap();
        assert!(soon(|| state().is_none()));
        // An installed game, uninstalled with no prompt.
        let installed = steam_game("9100010");
        assert!(load(&root).unwrap().games.iter().any(|g| g.source_id == "9100010" && g.installed));
        installer.uninstall(&installed, &mut open).unwrap();
        assert!(soon(|| !load(&root).unwrap().games.iter().any(|g| g.source_id == "9100010" && g.installed)));
        assert!(opened.0.lock().unwrap().is_empty(), "steam.exe was asked: {:?}", opened.0.lock().unwrap());
        // Another store's game is none of Steam's.
        let other = Game { source: "gog".into(), ..steam_game(quiet) };
        assert_eq!(installer.steer(&other, Steer::Pause, &mut open), Err(Refusal::Unsupported));
    }

    #[test]
    fn a_client_that_does_not_answer_is_asked_through_steam_exe_and_cannot_be_steered() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let steam = standin(&root);
        std::fs::write(root.join(cef::FILE), "").unwrap();
        // A port its debugger wrote down, and nothing listening there.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        std::fs::write(root.join(cef::STANDIN_PORT), port.to_string()).unwrap();
        let installer = steam.installer().unwrap();
        let opened = Shared::default();
        installer.install(&steam_game("9100040"), None, &mut opened.clone()).unwrap();
        let exe = root.join("steam.exe").display().to_string();
        let wanted = format!("{exe} -silent +app_install 9100040");
        assert!(soon(|| opened.0.lock().unwrap().first() == Some(&wanted)), "{:?}", opened.0.lock().unwrap());
        // An opener no thread can keep: asked at once, the old way.
        let mut kept = Kept(Vec::new());
        installer.uninstall(&steam_game("9100010"), &mut kept).unwrap();
        assert_eq!(kept.0, [format!("{exe} -silent -console +app_uninstall 9100010")]);
    }

    /// Today's route behind the installer: what `ask` gives, opened, into Steam's own library whatever folder.
    #[test]
    fn steams_installer_asks_as_before_and_cannot_steer_a_real_steam() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("steam.exe"), "").unwrap();
        let steam = Steam::at(Some(dir.path().to_path_buf()));
        let installer = steam.installer().expect("Steam has an installer");
        let folders = installer.folders();
        assert_eq!(folders, []);
        let mut kept = Kept(Vec::new());
        installer.install(&steam_game("620"), folders.first(), &mut kept).unwrap();
        installer.uninstall(&steam_game("620"), &mut kept).unwrap();
        let exe = dir.path().join("steam.exe");
        assert_eq!(
            kept.0,
            [
                format!("{} -silent +app_install 620", exe.display()),
                format!("{} -silent -console +app_uninstall 620", exe.display())
            ]
        );
        assert_eq!(installer.can(), Can { quiet_uninstall: true, ..Can::NOTHING });
        for steer in [Steer::Pause, Steer::Resume, Steer::Cancel] {
            assert_eq!(installer.steer(&steam_game("620"), steer, &mut kept), Err(Refusal::Unsupported));
        }
        assert_eq!(kept.0.len(), 2);
    }

    #[test]
    fn a_game_installed_while_its_workshop_items_come_in_reads_so_with_how_many_are_in() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        let steam = standin(&root);
        let quiet = 9_100_040;
        fixture::write_progress(&root, quiet, fixture::Progress::Downloading { done: 50, total: 100 }).unwrap();
        let state = |steam: &Steam| steam.installs().unwrap().first().map(|install| (install.state, install.speed));
        assert_eq!(state(&steam), Some((model::InstallState::Downloading { done: 50, total: 100 }, None)));
        // The game's own files in, its Workshop items not: installed, two of six items in.
        fixture::write_workshop(&root, quiet, 2, 6).unwrap();
        fixture::write_progress(&root, quiet, fixture::Progress::Done).unwrap();
        let (got, speed) = state(&steam).unwrap();
        assert!(
            matches!(got, model::InstallState::Extras { items_done: 2, items: 6, done, total } if done > 0 && total > done)
        );
        assert_eq!(speed, None);
        fixture::write_workshop(&root, quiet, 5, 6).unwrap();
        assert!(matches!(state(&steam), Some((model::InstallState::Extras { items_done: 5, .. }, None))));
        // All in: done.
        fixture::write_workshop(&root, quiet, 6, 6).unwrap();
        assert_eq!(state(&steam), None);
    }

    #[test]
    fn localconfig_persona_and_apps() {
        let v = vdf::parse_text(
            r#""UserLocalConfigStore" {
                "friends" { "1" { "name" "Someone" } "PersonaName" "tester" }
                "Software" { "Valve" { "Steam" { "apps" {
                    "70" { "LastPlayed" "1700000000" "Playtime" "90" }
                    "80" { "cloud" { "last_sync_state" "synchronized" } }
                } } } }
            }"#,
        )
        .unwrap();
        let lc = parse_localconfig(&v);
        assert_eq!(lc.persona.as_deref(), Some("tester"));
        assert_eq!(lc.apps[&70], (1_700_000_000, 90));
        assert_eq!(lc.apps[&80], (0, 0));
    }

    #[test]
    fn art_prefers_appinfo_name_then_falls_back() {
        let art = ArtDir {
            dir: PathBuf::from("lc/70"),
            files: vec!["abc/library_600x900.jpg".into(), "header.jpg".into(), "Library_Hero.jpg".into()],
        };
        let common = vdf::parse_text(
            r#""library_assets_full" { "library_capsule" { "image" { "english" "abc/library_600x900.jpg" } }
                                       "library_hero" { "image" { "english" "gone/hero_v2.jpg" } } }
               "header_image" { "english" "header.jpg" }"#,
        )
        .unwrap();
        let cover = art.find(&common, "library_capsule", None, &["library_600x900.jpg"]);
        assert_eq!(cover, Some(PathBuf::from("lc/70").join("abc").join("library_600x900.jpg")));
        let hero = art.find(&common, "library_hero", None, &["library_hero.jpg"]);
        assert_eq!(hero, Some(PathBuf::from("lc/70").join("Library_Hero.jpg")));
        let header = art.find(&common, "library_header", Some("header_image"), &["header.jpg"]);
        assert_eq!(header, Some(PathBuf::from("lc/70").join("header.jpg")));
        assert_eq!(art.find(&common, "library_logo", None, &["logo.png"]), None);
    }

    #[test]
    fn credits_come_from_the_associations_and_else_from_extended() {
        let info = vdf::parse_text(
            r#""common" { "associations" {
                    "0" { "type" "developer" "name" "Tin Lantern" }
                    "1" { "type" "publisher" "name" "Harbour & Hull" }
                    "2" { "type" "Developer" "name" "Second Kettle" }
                    "3" { "type" "franchise" "name" "Lanterns" }
                    "4" { "type" "developer" "name" "Tin Lantern" }
                    "5" { "type" "developer" "name" " " } } }
               "extended" { "developer" "Ignored" "publisher" "Ignored too" }"#,
        )
        .unwrap();
        assert_eq!(credits(&info, "developer"), ["Tin Lantern", "Second Kettle"]);
        assert_eq!(credits(&info, "publisher"), ["Harbour & Hull"]);

        let old = r#""common" { "name" "Old" } "extended" { "developer" "Lone Dev" "publisher" "" }"#;
        let old = vdf::parse_text(old).unwrap();
        assert_eq!(credits(&old, "developer"), ["Lone Dev"]);
        assert!(credits(&old, "publisher").is_empty());
    }

    #[test]
    fn the_release_date_is_the_original_one_where_there_is_one_and_scores_are_out_of_a_hundred() {
        let common = |text: &str| vdf::parse_text(text).unwrap();
        let both = common(r#""steam_release_date" "1700000000" "original_release_date" "1200000000""#);
        assert_eq!(released(&both), 1_200_000_000);
        let steams = common(r#""steam_release_date" "1700000000" "original_release_date" "0""#);
        assert_eq!(released(&steams), 1_700_000_000);
        assert_eq!(released(&common(r#""name" "No date""#)), 0);

        let scores = common(r#""metacritic_score" "87" "review_percentage" "250""#);
        assert_eq!(score(&scores, "metacritic_score"), Some(87));
        assert_eq!(score(&scores, "review_percentage"), None);
        assert_eq!(score(&scores, "review_score"), None);
    }

    #[test]
    fn steam_as_a_source_reads_its_install_and_names_its_games_by_appid() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        fixture::write(&root, &fixture::Fixture::standard()).unwrap();
        let steam = Steam::at(Some(root.clone()));
        assert_eq!((steam.key(), steam.name()), ("steam", "Steam"));
        let found = steam.read().unwrap().expect("an install");
        assert_eq!(found.from, root.display().to_string());
        assert_eq!(found.library, load(&root).unwrap());

        let game = Game { source: SOURCE.into(), source_id: "9100040".into(), ..Game::default() };
        assert_eq!(steam.start(&game), Some(Start::Address("steam://rungameid/9100040".into())));
        // With no steam.exe in the install, Steam's own addresses, and its window.
        let ask = |steam: &Steam, ask| steam.ask(&game, ask).map(|start| start.to_string());
        assert_eq!(ask(&steam, Ask::Install).as_deref(), Some("steam://install/9100040"));
        assert_eq!(ask(&steam, Ask::Uninstall).as_deref(), Some("steam://uninstall/9100040"));
        // With one, the Bundle's command to it, in its folder.
        std::fs::write(root.join("steam.exe"), b"").unwrap();
        let exe = root.join("steam.exe").display().to_string();
        assert_eq!(ask(&steam, Ask::Install), Some(format!("{exe} -silent +app_install 9100040")));
        assert_eq!(ask(&steam, Ask::Uninstall), Some(format!("{exe} -silent -console +app_uninstall 9100040")));
        let asked = steam.ask(&game, Ask::Install).unwrap();
        assert_eq!(asked.folder(), Some(root.clone()));
        // Nothing is under way in the fixture.
        assert_eq!(steam.installs(), Some(Vec::new()));
        // Not Steam's, or no appid: nothing to ask Steam for.
        for other in [Game { source: "gog".into(), ..game.clone() }, Game { source_id: "x".into(), ..game.clone() }] {
            assert_eq!((steam.start(&other), steam.ask(&other, Ask::Install)), (None, None));
            assert!(!steam.running(&other));
        }
    }

    #[test]
    fn an_install_that_is_not_there_is_an_error_and_no_install_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no steam here");
        let error = Steam::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").starts_with(&missing.display().to_string()), "{error:#}");
        // An empty folder is no Steam, and not the machine's: what a test names so that the machine's is never read.
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(Steam::at(Some(empty.clone())).read().unwrap(), None);
        // A folder with something in it but no install is still an error.
        std::fs::write(empty.join("note.txt"), b"").unwrap();
        assert!(Steam::at(Some(empty)).read().is_err());
        // Off Windows no registry names one.
        if cfg!(not(windows)) {
            assert_eq!(Steam::at(None).read().unwrap(), None);
        }
    }
}
