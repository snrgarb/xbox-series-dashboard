//! Reads the Epic Games Launcher's own files into a `model::Library` (docs/research/stores.md, "Epic Games Store").
//!
//! Launchers of different years leave different files. A game installed by a current launcher is recorded by Epic
//! Online Services' install helper (`install_helper`); the library itself comes from the eglv2 launcher's caches
//! (`modern`) where they give games, else from the older files:
//! - `%ProgramData%\Epic\EpicOnlineServicesShared\InstallHelper\InstalledItems\*.egi`: one JSON record per
//!   installation, the newest record, which wins over the others for the same `AppName` (seen on a real machine with
//!   launcher build EpicInstaller-20.3.3, which wrote neither manifests nor eglv2 files).
//! - `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item`: one JSON file per installed item; DLC and Unreal
//!   Engine content are left out.
//! - `%ProgramData%\Epic\UnrealEngineLauncher\LauncherInstalled.dat`: a second install list, which only puts right
//!   a manifest whose folder is not there.
//! - `%ProgramData%\Epic\EpicGamesLauncher\Data\Catalog\catcache.bin`: the account's library as the launcher last
//!   cached it, base64 of a JSON array, which is what lists the games that are owned and not installed. It is there
//!   only when the launcher was signed in on this PC once.
//! - `%LOCALAPPDATA%\com.epicgames.eglv2`: the eglv2 launcher's `installations.jsonl` and its library and catalogue
//!   caches (`modern`), known from one project only and absent on the real machine.
//!
//! No art on disk: the launcher caches its pictures' addresses, not the pictures. Those addresses (a catalogue
//! item's `keyImages`, on Epic's CDN) are the game's `OnlineArt`, which the art lookup downloads (#110). No play
//! history either: Epic keeps it on its servers.
//!
//! [`Epic`] is the source the library reads (`model::Source`): it starts a game, and asks for one to be installed,
//! through the launcher's `com.epicgames.launcher://` links, the launcher checking the game is the user's.
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

pub mod fixture;
pub mod install_helper;
mod modern;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use model::{Ask, Found, Game, Library, OnlineArt, Source, Start};
use serde_json::Value;

/// Epic, as a game's `source` names it.
pub const SOURCE: &str = "epic";

/// The scheme of the launcher's links.
const SCHEME: &str = "com.epicgames.launcher://";

/// Epic, as the library reads it and the shell starts its games.
#[derive(Debug, Clone, Default)]
pub struct Epic {
    /// A folder standing for the launcher's (`--epic`, `Places::under`); with none, the machine's own.
    root: Option<PathBuf>,
}

impl Epic {
    /// The launcher's files under `root` (`--epic` on the command line, `Places::under`), or with none the
    /// machine's own.
    pub fn at(root: Option<PathBuf>) -> Self {
        Self { root }
    }

    fn places(&self) -> Option<Places> {
        match &self.root {
            Some(root) => Some(Places::under(root)),
            None => Places::machine(),
        }
    }

    /// The item of the game, as the launcher's files have it now.
    fn item(&self, game: &Game) -> Option<Item> {
        let places = self.places()?;
        reading(&places).ok()?.items.into_iter().find(|item| item.app_name == game.source_id)
    }
}

/// Where the launcher keeps its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// `%ProgramData%\Epic`: the install helper's records, the manifests, the catalogue cache and
    /// `LauncherInstalled.dat`.
    pub data: PathBuf,
    /// `%LOCALAPPDATA%\com.epicgames.eglv2`: the new launcher's.
    pub modern: Option<PathBuf>,
}

impl Places {
    /// The machine's own, on Windows: `%ProgramData%\Epic` and `%LOCALAPPDATA%\com.epicgames.eglv2`.
    pub fn machine() -> Option<Places> {
        if !cfg!(windows) {
            return None;
        }
        let data = PathBuf::from(std::env::var_os("ProgramData")?).join("Epic");
        let modern = std::env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("com.epicgames.eglv2"));
        Some(Places { data, modern })
    }

    /// A folder standing for both, as the fixture writes one: `<root>` is `%ProgramData%\Epic`, and
    /// `<root>\com.epicgames.eglv2` the new launcher's folder.
    pub fn under(root: &Path) -> Places {
        Places { data: root.to_path_buf(), modern: Some(root.join("com.epicgames.eglv2")) }
    }

    /// `EpicGamesLauncher\Data\Manifests`.
    pub fn manifests(&self) -> PathBuf {
        self.data.join("EpicGamesLauncher").join("Data").join("Manifests")
    }

    /// `EpicGamesLauncher\Data\Catalog\catcache.bin`.
    pub fn catcache(&self) -> PathBuf {
        self.data.join("EpicGamesLauncher").join("Data").join("Catalog").join("catcache.bin")
    }

    /// `UnrealEngineLauncher\LauncherInstalled.dat`.
    pub fn launcher_installed(&self) -> PathBuf {
        self.data.join("UnrealEngineLauncher").join("LauncherInstalled.dat")
    }

    /// `EpicOnlineServicesShared\InstallHelper\InstalledItems`: the install helper's records of the games.
    pub fn installed_items(&self) -> PathBuf {
        install_helper::folder(&self.data)
    }

    /// Whether any launcher has left its lists here: with none, it is not installed.
    fn any(&self) -> bool {
        self.manifests().is_dir()
            || self.catcache().is_file()
            || self.installed_items().is_dir()
            || self.modern.as_deref().is_some_and(modern::any)
    }
}

impl Source for Epic {
    fn key(&self) -> &'static str {
        SOURCE
    }

    fn name(&self) -> &'static str {
        "Epic Games"
    }

    fn read(&self) -> Result<Option<Found>> {
        let Some(places) = self.places() else { return Ok(None) };
        if self.root.is_some() && !places.data.is_dir() {
            bail!("{}: no such folder", places.data.display());
        }
        if !places.any() {
            return Ok(None);
        }
        let reading = reading(&places).with_context(|| places.data.display().to_string())?;
        let from = match reading.parts.is_empty() {
            true => places.data.display().to_string(),
            false => format!("{} ({})", places.data.display(), reading.parts.join(", ")),
        };
        Ok(Some(Found { library: library(reading.items), from }))
    }

    /// The launcher's link, `com.epicgames.launcher://apps/<namespace>%3A<item>%3A<app>?action=launch&silent=true`:
    /// the launcher starts if it is not running, checks the game is the user's, and starts it.
    fn start(&self, game: &Game) -> Option<Start> {
        let app = app_name(game)?;
        let item = self.item(game).unwrap_or_else(|| Item { app_name: app.to_string(), ..Item::default() });
        Some(Start::Address(link(&item, "launch")))
    }

    /// Install: the launcher's link with `action=install`, which shows the launcher's own install dialog. Uninstall:
    /// nothing, as nothing outside the launcher can ask it to.
    fn ask(&self, game: &Game, ask: Ask) -> Option<Start> {
        let app = app_name(game)?;
        match ask {
            Ask::Install => {
                let item = self.item(game).unwrap_or_else(|| Item { app_name: app.to_string(), ..Item::default() });
                Some(Start::Address(link(&item, "install")))
            }
            Ask::Uninstall => None,
        }
    }
}

/// The game's `AppName`, if it is one of Epic's.
fn app_name(game: &Game) -> Option<&str> {
    (game.source == SOURCE && !game.source_id.is_empty()).then_some(game.source_id.as_str())
}

/// One item of the user's library, as either launcher's files tell it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Item {
    /// The id in every launcher link, and the game's `source_id`.
    pub app_name: String,
    /// `CatalogNamespace` and `CatalogItemId`: the link's other two parts.
    pub namespace: String,
    pub item_id: String,
    pub title: String,
    /// Who made it, as the catalogue names them; empty where it does not.
    pub developer: String,
    /// Where it is installed, and its program relative to there.
    pub install: Option<(PathBuf, String)>,
    /// What it takes on disk, where the manifest says.
    pub size: u64,
    /// The addresses of its key images on Epic's CDN, as its catalogue item names them.
    pub art: OnlineArt,
}

/// A catalogue item's `keyImages` (`[{"type", "url"}]`) as the store's own art online: the tall box art the cover,
/// the wide one the hero, the wide offer image (else the box art) the header. Only `https` addresses. Nothing is
/// square.
pub(crate) fn key_images(value: &Value) -> OnlineArt {
    let images = value.get("keyImages").and_then(Value::as_array);
    let image = |types: &[&str]| {
        types.iter().find_map(|wanted| {
            let found = images.into_iter().flatten().find(|i| i.get("type").and_then(Value::as_str) == Some(wanted))?;
            found.get("url")?.as_str().filter(|url| url.starts_with("https://")).map(str::to_string)
        })
    };
    OnlineArt {
        cover: image(&["DieselGameBoxTall", "OfferImageTall"]),
        hero: image(&["DieselGameBox", "OfferImageWide"]),
        header: image(&["OfferImageWide", "DieselStoreFrontWide", "DieselGameBox"]),
        ..OnlineArt::default()
    }
}

/// The launcher's link for the item: the three-part form where the item's namespace and catalogue id are known,
/// else the older short one.
pub(crate) fn link(item: &Item, action: &str) -> String {
    let silent = if action == "launch" { "&silent=true" } else { "" };
    if item.namespace.is_empty() || item.item_id.is_empty() {
        format!("{SCHEME}apps/{}?action={action}{silent}", item.app_name)
    } else {
        format!("{SCHEME}apps/{}%3A{}%3A{}?action={action}{silent}", item.namespace, item.item_id, item.app_name)
    }
}

/// What a launcher link asks and of which `AppName`: `("launch", "9c1f...")`. Either form; for the stand-in
/// launcher on a test machine.
pub fn parse_link(url: &str) -> Option<(String, String)> {
    let rest = url.trim().strip_prefix(SCHEME)?.strip_prefix("apps/")?;
    let (id, query) = rest.split_once('?').unwrap_or((rest, ""));
    let id = id.trim_end_matches('/');
    let app = id.replace("%3A", ":").replace("%3a", ":");
    let app = app.rsplit(':').next()?.to_string();
    let action = query.split('&').find_map(|pair| pair.strip_prefix("action=")).unwrap_or("launch");
    (!app.is_empty()).then(|| (action.to_string(), app))
}

/// The program of the installed game `app_name`: its install folder joined to its `LaunchExecutable`. For the
/// stand-in launcher on a test machine, which starts it as the launcher would.
pub fn executable(places: &Places, app_name: &str) -> Option<PathBuf> {
    let item = reading(places).ok()?.items.into_iter().find(|item| item.app_name == app_name)?;
    let (dir, exe) = item.install?;
    Some(exe.split(['/', '\\']).filter(|part| !part.is_empty()).fold(dir, |path, part| path.join(part)))
}

/// Reads the launcher's files at `places`.
pub fn load(places: &Places) -> Result<Library> {
    Ok(library(reading(places)?.items))
}

/// The library of `items`, by name.
fn library(items: Vec<Item>) -> Library {
    let mut games: Vec<Game> = items
        .into_iter()
        .map(|item| Game {
            source: SOURCE.to_string(),
            source_id: item.app_name,
            name: item.title,
            installed: item.install.is_some(),
            size_on_disk: item.size,
            install_dir: item.install.map(|(dir, _)| dir),
            online: item.art,
            developers: Some(item.developer).filter(|d| !d.is_empty()).into_iter().collect(),
            ..Game::default()
        })
        .collect();
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.source_id.cmp(&b.source_id)));
    Library { games, ..Library::default() }
}

/// What was read, and what each of the launcher's files gave for the log line (`InstallHelper 1 of 1, catcache 3 of
/// 152`: its games of its entries).
struct Reading {
    items: Vec<Item>,
    parts: Vec<String>,
}

/// Every game of the library: the eglv2 launcher's, else the old one's, with the install helper's records over
/// either.
fn reading(places: &Places) -> Result<Reading> {
    let mut parts = Vec::new();
    let catalogue = Catalogue::read(places, &mut parts);
    let mut items = match places.modern.as_deref().map(modern::items).transpose()?.flatten() {
        Some(items) if !items.is_empty() => {
            parts.insert(0, format!("eglv2 {}", items.len()));
            items
        }
        _ => legacy(places, &catalogue, &mut parts)?,
    };
    if let Some(records) = install_helper::records(&places.installed_items()) {
        let installed = records.iter().filter(|record| record.installed()).count();
        parts.insert(0, format!("InstallHelper {installed} of {}", records.len()));
        merge(&mut items, records, &catalogue);
    }
    Ok(Reading { items, parts })
}

/// The catalogue cache: its games, and every entry by its catalogue id, game or not, to name an installed game by.
#[derive(Debug, Default)]
struct Catalogue {
    games: Vec<Item>,
    by_id: HashMap<String, Item>,
}

impl Catalogue {
    /// `catcache.bin`, if it is there. One that cannot be read is said in the log, and is empty.
    fn read(places: &Places, parts: &mut Vec<String>) -> Catalogue {
        let file = places.catcache();
        let entries = match fs::read(&file) {
            Ok(bytes) => catcache(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Catalogue::default(),
            Err(e) => Err(e.into()),
        };
        let entries = entries.unwrap_or_else(|e| {
            eprintln!("epic: {}: {e:#}", file.display());
            parts.push("catcache unread".into());
            Vec::new()
        });
        if entries.is_empty() {
            return Catalogue::default();
        }
        let games: Vec<Item> = entries.iter().filter_map(catalogue_item).collect();
        parts.push(format!("catcache {} of {}", games.len(), entries.len()));
        let by_id = entries.iter().map(catalogue_entry).filter(|item| !item.item_id.is_empty());
        Catalogue { games, by_id: by_id.map(|item| (item.item_id.clone(), item)).collect() }
    }
}

/// The install helper's records over the games read: a game's newest record says whether it is installed, where and
/// how big. A record that is done names a game the others do not know, by the catalogue where it has the item, else
/// by its folder; one that is not done adds no game.
fn merge(items: &mut Vec<Item>, mut records: Vec<install_helper::Record>, catalogue: &Catalogue) {
    // Two records of one game: the done one counts.
    records.sort_by_key(install_helper::Record::installed);
    for record in records {
        let install = record.installed().then(|| (record.dir.clone(), record.exe.clone()));
        let size = if install.is_some() { record.size } else { 0 };
        if let Some(item) = items.iter_mut().find(|item| item.app_name == record.app_name) {
            item.install = install;
            item.size = size;
            if item.namespace.is_empty() || item.item_id.is_empty() {
                (item.namespace, item.item_id) = (record.namespace, record.item_id);
            }
            continue;
        }
        if install.is_none() {
            continue;
        }
        let known = catalogue.by_id.get(&record.item_id);
        let folder = record.dir.file_name().map(|name| name.to_string_lossy().into_owned());
        let title = known.map(|item| item.title.clone()).filter(|title| !title.is_empty());
        items.push(Item {
            title: title.or(folder).unwrap_or_else(|| record.app_name.clone()),
            developer: known.map(|item| item.developer.clone()).unwrap_or_default(),
            art: known.map(|item| item.art.clone()).unwrap_or_default(),
            app_name: record.app_name,
            namespace: record.namespace,
            item_id: record.item_id,
            install,
            size,
        });
    }
}

/// The old launcher's games: the installed ones from their manifests, and the rest of the account's from the
/// catalogue cache.
fn legacy(places: &Places, catalogue: &Catalogue, parts: &mut Vec<String>) -> Result<Vec<Item>> {
    let manifests = places.manifests();
    let mut files: Vec<PathBuf> = match fs::read_dir(&manifests) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("item")))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| manifests.display().to_string()),
    };
    files.sort();
    let listed = fs::read_to_string(places.launcher_installed()).ok().map(|text| launcher_installed(&text));
    let listed = listed.unwrap_or_default();

    let mut items: Vec<Item> = Vec::new();
    for file in &files {
        // A manifest can be empty or broken (people edit them when moving games): that one is said and passed
        // over. One of DLC or engine content is passed over in silence.
        let text = fs::read_to_string(file).unwrap_or_default();
        if serde_json::from_str::<Value>(&text).is_err() {
            eprintln!("epic: {}: not a manifest, passed over", file.display());
            continue;
        }
        let Some(manifest) = parse_manifest(&text) else { continue };
        let agreed = listed.get(&manifest.app_name);
        let dir = Some(manifest.dir.clone())
            .filter(|dir| dir.is_dir())
            .or_else(|| agreed.filter(|dir| dir.is_dir()).cloned());
        let Some(dir) = dir else { continue };
        let item = Item {
            app_name: manifest.app_name,
            namespace: manifest.namespace,
            item_id: manifest.item_id,
            title: manifest.title,
            developer: String::new(),
            install: Some((dir.clone(), manifest.exe)),
            size: manifest.size,
            art: OnlineArt::default(),
        };
        // Two manifests of one game: the one the install list agrees with.
        match items.iter_mut().find(|known| known.app_name == item.app_name) {
            Some(known) if agreed == Some(&dir) => *known = item,
            Some(_) => {}
            None => items.push(item),
        }
    }
    if manifests.is_dir() {
        parts.insert(0, format!("manifests {} of {}", items.len(), files.len()));
    }

    for owned in &catalogue.games {
        let installed = items.iter_mut().find(|item| {
            (!owned.item_id.is_empty() && item.item_id == owned.item_id) || item.app_name == owned.app_name
        });
        match installed {
            Some(item) => {
                if item.title.is_empty() {
                    item.title = owned.title.clone();
                }
                // The manifest names no pictures and no developer: the catalogue's.
                item.developer = owned.developer.clone();
                item.art = owned.art.clone();
            }
            None => items.push(owned.clone()),
        }
    }
    Ok(items)
}

/// What an installed item's manifest says, if it is a game.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Manifest {
    app_name: String,
    namespace: String,
    item_id: String,
    title: String,
    dir: PathBuf,
    exe: String,
    size: u64,
}

/// A manifest, if it is one and of a game: not DLC (`addons` without `addons/launchable`), not Unreal Engine
/// content (`plugins`, `plugins/engine`, a `UE_` compatible app, a `plugins/engine` technical type).
fn parse_manifest(text: &str) -> Option<Manifest> {
    let value: Value = serde_json::from_str(text).ok()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let strings = |key: &str| -> Vec<String> {
        let list = value.get(key).and_then(Value::as_array);
        list.into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
    };
    let categories = strings("AppCategories");
    let has = |category: &str| categories.iter().any(|c| c.eq_ignore_ascii_case(category));
    let dlc = has("addons") && !has("addons/launchable");
    let engine = has("plugins")
        || has("plugins/engine")
        || strings("CompatibleApps").iter().any(|app| app.starts_with("UE_"))
        || text("TechnicalType").contains("plugins/engine");
    let app_name = text("AppName");
    let dir = text("InstallLocation");
    if dlc || engine || app_name.is_empty() || dir.is_empty() {
        return None;
    }
    Some(Manifest {
        title: Some(text("DisplayName")).filter(|name| !name.is_empty()).unwrap_or_else(|| app_name.clone()),
        app_name,
        namespace: text("CatalogNamespace"),
        item_id: text("CatalogItemId"),
        dir: PathBuf::from(dir),
        exe: text("LaunchExecutable"),
        size: value.get("InstallSize").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// `LauncherInstalled.dat`: `AppName` -> its install folder.
fn launcher_installed(text: &str) -> HashMap<String, PathBuf> {
    let value: Value = serde_json::from_str(text).unwrap_or_default();
    let list = value.get("InstallationList").and_then(Value::as_array);
    list.into_iter()
        .flatten()
        .filter_map(|entry| {
            let app = entry.get("AppName")?.as_str()?;
            let dir = entry.get("InstallLocation")?.as_str()?;
            Some((app.to_string(), PathBuf::from(dir)))
        })
        .collect()
}

/// `catcache.bin`'s entries: base64 of a JSON array of catalogue items. A byte-order mark before the JSON and
/// nulls or white space after it are let be.
fn catcache(bytes: &[u8]) -> Result<Vec<Value>> {
    let json = base64(bytes).context("not base64")?;
    let json = json.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&json);
    let end = json.iter().rposition(|&b| b != 0 && !b.is_ascii_whitespace()).map_or(0, |last| last + 1);
    let value: Value = serde_json::from_slice(&json[..end]).context("not JSON")?;
    let Value::Array(list) = value else { bail!("not a list") };
    Ok(list)
}

/// A catalogue item, if it is a game: it has the `games` category, is not an extra or DLC, and names its
/// `AppName` (`releaseInfo[].appId`) and its title.
pub(crate) fn catalogue_item(value: &Value) -> Option<Item> {
    let categories = value.get("categories").and_then(Value::as_array);
    let paths: Vec<&str> =
        categories.into_iter().flatten().filter_map(|c| c.get("path").and_then(Value::as_str)).collect();
    let has = |path: &str| paths.iter().any(|p| p.eq_ignore_ascii_case(path));
    if !has("games") || has("digitalextras") || (has("addons") && !has("addons/launchable")) {
        return None;
    }
    // DLC names its game. A real launcher's cache gives every entry a `mainGameItem`, an empty object on a game
    // (seen 2026-11-16), so only one that names an id counts.
    let main = value.get("mainGameItem").and_then(|main| main.get("id")).and_then(Value::as_str);
    if main.is_some_and(|id| !id.is_empty()) {
        return None;
    }
    let item = catalogue_entry(value);
    (!item.app_name.is_empty() && !item.title.is_empty()).then_some(item)
}

/// What a catalogue entry says, game or not. Its `AppName` is the `appId` of its Windows release, else of its first
/// release that names one.
fn catalogue_entry(value: &Value) -> Item {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let releases = value.get("releaseInfo").and_then(Value::as_array).into_iter().flatten();
    fn app(release: &Value) -> Option<&str> {
        release.get("appId").and_then(Value::as_str).filter(|app| !app.is_empty())
    }
    let windows = |release: &&Value| {
        let platforms = release.get("platform").and_then(Value::as_array).into_iter().flatten();
        platforms.filter_map(Value::as_str).any(|platform| platform.eq_ignore_ascii_case("Windows"))
    };
    let app = releases.clone().filter(windows).find_map(app).or_else(|| releases.clone().find_map(app));
    Item {
        app_name: app.unwrap_or_default().to_string(),
        namespace: text("namespace"),
        item_id: text("id"),
        title: text("title"),
        developer: text("developer"),
        install: None,
        size: 0,
        art: key_images(value),
    }
}

/// Standard base64, as `catcache.bin` holds it: white space anywhere and the padding are let be.
fn base64(bytes: &[u8]) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let (mut bits, mut held) = (0u32, 0u32);
    for &c in bytes.iter().filter(|c| !c.is_ascii_whitespace() && **c != b'=') {
        bits = bits << 6 | value(c)? as u32;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
        }
    }
    Some(out)
}

/// Standard base64 of `bytes`, with padding: the fixture's `catcache.bin`.
pub(crate) fn to_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(app: &str, dir: &Path, categories: &[&str]) -> String {
        json!({
            "bIsApplication": true,
            "LaunchExecutable": "Binaries/Win64/Lanternfall.exe",
            "DisplayName": format!("Game {app}"),
            "AppName": app,
            "CatalogNamespace": format!("ns-{app}"),
            "CatalogItemId": format!("item-{app}"),
            "AppCategories": categories,
            "InstallLocation": dir.to_string_lossy(),
            "InstallSize": 4_200_000_000u64,
        })
        .to_string()
    }

    #[test]
    fn base64_reads_back_what_it_wrote_and_refuses_what_is_not() {
        for text in ["", "a", "ab", "abc", "abcd", "[{\"title\":\"Lanternfall\"}]"] {
            assert_eq!(base64(to_base64(text.as_bytes()).as_bytes()).unwrap(), text.as_bytes(), "{text}");
        }
        assert_eq!(to_base64(b"Lanternfall"), "TGFudGVybmZhbGw=");
        assert_eq!(base64(b"TGFudGVy\r\nbmZhbGw=").unwrap(), b"Lanternfall");
        assert_eq!(base64(b"not*base64"), None);
    }

    #[test]
    fn a_manifest_is_a_game_unless_it_is_dlc_or_engine_content() {
        let dir = Path::new("D:/Epic/Lanternfall");
        let game = parse_manifest(&manifest("lantern", dir, &["public", "games", "applications"])).unwrap();
        assert_eq!(
            game,
            Manifest {
                app_name: "lantern".into(),
                namespace: "ns-lantern".into(),
                item_id: "item-lantern".into(),
                title: "Game lantern".into(),
                dir: dir.into(),
                exe: "Binaries/Win64/Lanternfall.exe".into(),
                size: 4_200_000_000,
            }
        );
        assert!(parse_manifest(&manifest("x", dir, &["public", "addons"])).is_none());
        assert!(parse_manifest(&manifest("x", dir, &["addons", "addons/launchable"])).is_some());
        assert!(parse_manifest(&manifest("x", dir, &["plugins", "plugins/engine"])).is_none());
        let mut ue: Value = serde_json::from_str(&manifest("x", dir, &["games"])).unwrap();
        ue["CompatibleApps"] = json!(["UE_5.3"]);
        assert!(parse_manifest(&ue.to_string()).is_none());
        ue["CompatibleApps"] = json!([]);
        ue["TechnicalType"] = json!("plugins/engine");
        assert!(parse_manifest(&ue.to_string()).is_none());
        assert!(parse_manifest("").is_none());
        assert!(parse_manifest("{\"AppName\": \"x\"}").is_none());
    }

    #[test]
    fn a_catalogue_item_is_a_game_by_its_category_and_names_its_app() {
        let item = |categories: Value, extra: Value| {
            let mut value = json!({
                "id": "e2a4", "namespace": "4b7d", "title": "Lanternfall", "categories": categories,
                "releaseInfo": [{ "appId": "9c1f", "dateAdded": "2025-03-02T10:00:00.000Z" }],
            });
            if let (Some(value), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
                value.extend(extra.clone());
            }
            catalogue_item(&value)
        };
        let games = json!([{ "path": "games" }, { "path": "applications" }]);
        assert_eq!(
            item(games.clone(), json!({})),
            Some(Item {
                app_name: "9c1f".into(),
                namespace: "4b7d".into(),
                item_id: "e2a4".into(),
                title: "Lanternfall".into(),
                ..Item::default()
            })
        );
        assert_eq!(item(json!([{ "path": "applications" }]), json!({})), None);
        assert_eq!(item(json!([{ "path": "games" }, { "path": "digitalextras" }]), json!({})), None);
        assert_eq!(item(games.clone(), json!({ "mainGameItem": { "id": "e2a4" } })), None);
        // A real cache gives a game an empty one.
        assert!(item(games.clone(), json!({ "mainGameItem": { "namespace": "", "id": "" } })).is_some());
        assert_eq!(item(games.clone(), json!({ "releaseInfo": [] })), None);

        // Its key images, as the store's own art online: https only, nothing square.
        let images = json!({ "keyImages": [
            { "type": "Thumbnail", "url": "https://cdn1.epicgames.com/x/thumb.jpg" },
            { "type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/x/tall.jpg" },
            { "type": "DieselGameBox", "url": "https://cdn1.epicgames.com/x/wide.jpg" },
            { "type": "OfferImageWide", "url": "http://cdn1.epicgames.com/x/offer.jpg" },
        ]});
        let art = item(games, images).unwrap().art;
        assert_eq!(art.cover.as_deref(), Some("https://cdn1.epicgames.com/x/tall.jpg"));
        assert_eq!(art.hero.as_deref(), Some("https://cdn1.epicgames.com/x/wide.jpg"));
        assert_eq!(art.header.as_deref(), Some("https://cdn1.epicgames.com/x/wide.jpg"), "not the http one");
        assert_eq!((art.square, art.product), (None, None));
    }

    #[test]
    fn links_have_three_parts_where_the_item_is_known_and_read_back() {
        let known =
            Item { app_name: "9c1f".into(), namespace: "4b7d".into(), item_id: "e2a4".into(), ..Item::default() };
        let launch = link(&known, "launch");
        assert_eq!(launch, "com.epicgames.launcher://apps/4b7d%3Ae2a4%3A9c1f?action=launch&silent=true");
        let short = Item { app_name: "9c1f".into(), ..Item::default() };
        assert_eq!(link(&short, "install"), "com.epicgames.launcher://apps/9c1f?action=install");
        assert_eq!(parse_link(&launch), Some(("launch".into(), "9c1f".into())));
        assert_eq!(parse_link(&link(&short, "install")), Some(("install".into(), "9c1f".into())));
        assert_eq!(parse_link("com.epicgames.launcher://apps/9c1f"), Some(("launch".into(), "9c1f".into())));
        assert_eq!(parse_link("com.epicgames.launcher://store/library"), None);
        assert_eq!(parse_link("steam://rungameid/440"), None);
    }

    #[test]
    fn the_old_launcher_lists_installed_games_and_the_rest_of_the_catalogue() {
        let dir = tempfile::tempdir().unwrap();
        let places = Places::under(&dir.path().join("Epic"));
        let games = dir.path().join("Games");
        fs::create_dir_all(places.manifests()).unwrap();
        fs::create_dir_all(games.join("A")).unwrap();
        fs::create_dir_all(games.join("Moved")).unwrap();
        fs::create_dir_all(games.join("B")).unwrap();
        let write = |name: &str, text: String| fs::write(places.manifests().join(name), text).unwrap();
        write("1.item", manifest("a", &games.join("A"), &["games"]));
        // Two of one game: the one the install list agrees with.
        write("2.item", manifest("b", &games.join("A"), &["games"]));
        write("3.item", manifest("b", &games.join("B"), &["games"]));
        // Moved: its manifest's folder is gone, and the install list knows where it went.
        write("4.item", manifest("moved", &games.join("Gone"), &["games"]));
        // Uninstalled by hand: no folder anywhere.
        write("5.item", manifest("gone", &games.join("Gone"), &["games"]));
        write("6.item", manifest("dlc", &games.join("A"), &["addons"]));
        write("7.item", String::new());
        write("readme.txt", "not a manifest".into());
        let list = json!({ "InstallationList": [
            { "AppName": "b", "InstallLocation": games.join("B") },
            { "AppName": "moved", "InstallLocation": games.join("Moved") },
        ]});
        fs::create_dir_all(places.launcher_installed().parent().unwrap()).unwrap();
        fs::write(places.launcher_installed(), list.to_string()).unwrap();
        let catalogue = json!([
            { "id": "item-a", "title": "A from the catalogue", "categories": [{ "path": "games" }], "releaseInfo": [{ "appId": "a" }] },
            { "id": "item-c", "namespace": "ns-c", "title": "Owned", "categories": [{ "path": "games" }], "releaseInfo": [{ "appId": "c" }] },
            { "id": "item-d", "title": "Extra", "categories": [{ "path": "digitalextras" }], "releaseInfo": [{ "appId": "d" }] },
        ]);
        fs::create_dir_all(places.catcache().parent().unwrap()).unwrap();
        fs::write(places.catcache(), to_base64(catalogue.to_string().as_bytes())).unwrap();

        let library = load(&places).unwrap();
        let said: Vec<(&str, &str, bool, Option<PathBuf>)> = library
            .games
            .iter()
            .map(|g| (g.source_id.as_str(), g.name.as_str(), g.installed, g.install_dir.clone()))
            .collect();
        assert_eq!(
            said,
            [
                ("a", "Game a", true, Some(games.join("A"))),
                ("b", "Game b", true, Some(games.join("B"))),
                ("moved", "Game moved", true, Some(games.join("Moved"))),
                ("c", "Owned", false, None),
            ]
        );
        assert!(library.games.iter().all(|g| g.source == SOURCE && g.cover.is_none()));
        assert_eq!(library.games[0].size_on_disk, 4_200_000_000);
        assert_eq!(
            executable(&places, "b"),
            Some(games.join("B").join("Binaries").join("Win64").join("Lanternfall.exe"))
        );
        assert_eq!(executable(&places, "c"), None);
    }

    #[test]
    fn a_broken_catalogue_leaves_the_installed_games() {
        let dir = tempfile::tempdir().unwrap();
        let places = Places::under(dir.path());
        let game = dir.path().join("A");
        fs::create_dir_all(places.manifests()).unwrap();
        fs::create_dir_all(&game).unwrap();
        fs::write(places.manifests().join("a.item"), manifest("a", &game, &["games"])).unwrap();
        fs::create_dir_all(places.catcache().parent().unwrap()).unwrap();
        fs::write(places.catcache(), b"%%%").unwrap();
        assert_eq!(load(&places).unwrap().games.len(), 1);
    }

    /// Writes `records` as the install helper's, and `catalogue` as the catalogue cache, under `places`.
    fn helper(places: &Places, records: &[install_helper::Record], catalogue: Value) {
        fs::create_dir_all(places.installed_items()).unwrap();
        for record in records {
            let file = places.installed_items().join(format!("{}.egi", record.installation_id));
            fs::write(file, install_helper::write_record(record)).unwrap();
        }
        fs::create_dir_all(places.catcache().parent().unwrap()).unwrap();
        fs::write(places.catcache(), to_base64(catalogue.to_string().as_bytes())).unwrap();
    }

    fn catalogued(app: &str, title: &str) -> Value {
        json!({
            "id": format!("item-{app}"), "namespace": format!("ns-{app}"), "title": title, "developer": "Playtonic",
            "categories": [{ "path": "public" }, { "path": "games" }, { "path": "applications" }],
            "releaseInfo": [{ "appId": app, "platform": ["Windows"] }],
            "keyImages": [{ "type": "DieselGameBoxTall", "url": format!("https://cdn1.epicgames.com/{app}/tall.jpg") }],
        })
    }

    #[test]
    fn a_current_launchers_install_helper_says_what_is_installed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Epic");
        let places = Places::under(&root);
        let games = dir.path().join("Epic Games");
        for folder in ["Kiteborne", "Uncatalogued", "Pending", "Owned"] {
            fs::create_dir_all(games.join(folder)).unwrap();
        }
        use install_helper::record;
        helper(
            &places,
            &[
                record("kite", "Installed", &games.join("Kiteborne")),
                // Installed, and the catalogue cache does not know it: named by its folder.
                record("uncat", "Installed", &games.join("Uncatalogued")),
                // An install in progress of a game nothing else lists: no game.
                record("pending", "Incomplete", &games.join("Pending")),
                // An install in progress of an owned game: owned, not installed.
                record("owned", "Incomplete", &games.join("Owned")),
            ],
            json!([catalogued("kite", "Kiteborne"), catalogued("owned", "Wexford Hollow")]),
        );
        // The launcher's own components, done and with their folders there, are in the other folder: not games.
        let components = root.join("EpicOnlineServices").join("InstallHelper").join("InstalledItems");
        fs::create_dir_all(&components).unwrap();
        for name in ["overlay", "service", "support"] {
            let record = record(name, "Installed", &games);
            fs::write(components.join(format!("{name}.egi")), install_helper::write_record(&record)).unwrap();
        }

        let library = load(&places).unwrap();
        let said: Vec<(&str, &str, bool, Option<PathBuf>, u64)> = library
            .games
            .iter()
            .map(|g| (g.source_id.as_str(), g.name.as_str(), g.installed, g.install_dir.clone(), g.size_on_disk))
            .collect();
        assert_eq!(
            said,
            [
                ("kite", "Kiteborne", true, Some(games.join("Kiteborne")), 6_200_000_000),
                ("uncat", "Uncatalogued", true, Some(games.join("Uncatalogued")), 6_200_000_000),
                ("owned", "Wexford Hollow", false, None, 0),
            ]
        );
        let kite = &library.games[0];
        assert_eq!(kite.developers, ["Playtonic"]);
        assert_eq!(kite.online.cover.as_deref(), Some("https://cdn1.epicgames.com/kite/tall.jpg"));
        assert!(library.games[1].developers.is_empty());
        assert_eq!(executable(&places, "kite"), Some(games.join("Kiteborne").join("Game.exe")));

        // Started through the launcher's three-part link, from the record's sandbox and item.
        let epic = Epic::at(Some(root.clone()));
        let uncat = &library.games[1];
        let link = "com.epicgames.launcher://apps/ns-uncat%3Aitem-uncat%3Auncat?action=launch&silent=true";
        assert_eq!(epic.start(uncat), Some(Start::Address(link.into())));
        let found = epic.read().unwrap().unwrap();
        assert_eq!(found.from, format!("{} (InstallHelper 2 of 4, catcache 2 of 2)", root.display()));
    }

    #[test]
    fn an_install_helpers_record_wins_over_a_manifest_of_the_same_game() {
        let dir = tempfile::tempdir().unwrap();
        let places = Places::under(&dir.path().join("Epic"));
        let games = dir.path().join("Games");
        for folder in ["Old", "New", "B"] {
            fs::create_dir_all(games.join(folder)).unwrap();
        }
        fs::create_dir_all(places.manifests()).unwrap();
        fs::write(places.manifests().join("a.item"), manifest("a", &games.join("Old"), &["games"])).unwrap();
        fs::write(places.manifests().join("b.item"), manifest("b", &games.join("B"), &["games"])).unwrap();
        use install_helper::record;
        // `a` moved by a current launcher; `b` being installed again by one.
        let a = record("a", "Installed", &games.join("New"));
        let b = record("b", "Incomplete", &games.join("B"));
        // Two records of one game: the done one counts, whichever file comes first.
        let stale = install_helper::Record { installation_id: "0".into(), ..record("a", "Incomplete", &games) };
        helper(&places, &[a, b, stale], json!([catalogued("a", "Lanternfall")]));

        let library = load(&places).unwrap();
        let said: Vec<(&str, &str, bool, Option<PathBuf>, u64)> = library
            .games
            .iter()
            .map(|g| (g.source_id.as_str(), g.name.as_str(), g.installed, g.install_dir.clone(), g.size_on_disk))
            .collect();
        assert_eq!(
            said,
            [("a", "Game a", true, Some(games.join("New")), 6_200_000_000), ("b", "Game b", false, None, 0),]
        );
        assert_eq!(library.games[0].developers, ["Playtonic"], "the catalogue's, as for a manifest");
    }

    #[test]
    fn a_catalogue_cache_with_a_byte_order_mark_or_nulls_is_read_and_the_windows_release_names_the_app() {
        let entry = json!({
            "id": "i", "title": "Lanternfall", "categories": [{ "path": "games" }],
            "releaseInfo": [{ "appId": "", "platform": ["Mac"] }, { "appId": "mac", "platform": ["Mac"] }, { "appId": "win", "platform": ["Windows"] }],
        });
        let json = format!("\u{feff}[{entry}]\0\0\r\n");
        let read = catcache(to_base64(json.as_bytes()).as_bytes()).unwrap();
        assert_eq!(catalogue_item(&read[0]).unwrap().app_name, "win");
        let no_windows = json!({ "id": "i", "title": "T", "categories": [{ "path": "games" }], "releaseInfo": [{ "appId": "" }, { "appId": "mac" }] });
        assert_eq!(catalogue_item(&no_windows).unwrap().app_name, "mac");
    }

    #[test]
    fn epic_as_a_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("epic");
        let spec = fixture::Fixture::standard();
        fixture::write(&root, &spec).unwrap();
        let epic = Epic::at(Some(root.clone()));
        assert_eq!((epic.key(), epic.name()), ("epic", "Epic Games"));
        let found = epic.read().unwrap().expect("a launcher");
        // Where its games came from, each place's games of its entries.
        let from = format!("{} (InstallHelper 1 of 2, manifests 1 of 3, catcache 3 of 5)", root.display());
        assert_eq!(found.from, from);
        assert_eq!(found.library, load(&Places::under(&root)).unwrap());

        let installed = found.library.games.iter().find(|g| g.installed).unwrap();
        let fake = spec.games.iter().find(|g| g.app_name == installed.source_id).unwrap();
        let three = format!("com.epicgames.launcher://apps/{}%3A{}%3A{}", fake.namespace, fake.item_id, fake.app_name);
        assert_eq!(epic.start(installed), Some(Start::Address(format!("{three}?action=launch&silent=true"))));
        assert_eq!(epic.ask(installed, Ask::Install), Some(Start::Address(format!("{three}?action=install"))));
        assert_eq!(epic.ask(installed, Ask::Uninstall), None);
        assert!(!epic.running(installed));
        let other = Game { source: "steam".into(), ..installed.clone() };
        assert_eq!((epic.start(&other), epic.ask(&other, Ask::Install)), (None, None));
    }

    #[test]
    fn no_launcher_is_none_and_a_named_folder_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Epic::at(Some(dir.path().to_path_buf())).read().unwrap(), None);
        let missing = dir.path().join("no epic here");
        let error = Epic::at(Some(missing.clone())).read().unwrap_err();
        assert!(format!("{error:#}").contains(&missing.display().to_string()), "{error:#}");
        if cfg!(not(windows)) {
            assert_eq!(Epic::at(None).read().unwrap(), None);
        }
    }
}
