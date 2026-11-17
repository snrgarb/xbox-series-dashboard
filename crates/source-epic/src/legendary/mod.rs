//! Epic Games through legendary (#175; docs/research/installs.md 3; docs/sources.md "Epic Games"), the open
//! command-line Epic client the user adds in Settings > Helpers. Everything here is read in legendary's own source
//! at the version the Dashboard fetches, 0.21.1 (`legendary-gl/legendary` at `42f6bde`); the file and line each
//! fact comes from is named beside it.
//!
//! **Its records** are in its own folder (`lfs/lgndry.py` `LGDLFS.__init__`): `LEGENDARY_CONFIG_PATH`, else
//! `$XDG_CONFIG_HOME/legendary`, else `~/.config/legendary` ([`default_home`]). The Dashboard leaves them there, so a
//! sign-in made with legendary itself is the Dashboard's too. In it:
//! - `user.json`: the sign-in, Epic's token response (`userdata`), with `displayName` ([`signed_in`]).
//! - `installed.json`: what legendary has installed, by `AppName` (`models/game.py` `InstalledGame`; [`installed`]).
//! - `metadata/<AppName>.json`: every game of the account's library legendary has seen (`models/game.py` `Game`),
//!   written by any command that lists the library (`core.py` `get_game_and_dlc_list`, which `status` runs):
//!   the games owned and not installed ([`owned`]).
//!
//! **What is run** (`cli.py`, the parser from line 2726): `-y install <AppName> --base-path <folder> --skip-sdl
//! --skip-dlcs` (`-y` answers every question yes, so nothing waits on a keyboard; `--skip-sdl` takes only the
//! required parts of a game with selective downloads; DLC is left for later), `-y uninstall <AppName>`, `launch
//! <AppName>` (the game started with Epic's exchange code, so its ownership check passes), `status --json` (the
//! library brought up to date), and after an install, where the old launcher's `Data\Manifests` is there, `-y
//! egl-sync --export-only --one-shot --egl-manifest-path <it>` (the old launcher's record of the game, written by
//! legendary; the current launcher no longer reads it, legendary#797).
//!
//! **Its progress** is on its standard error, through Python's logging as `[<logger>] <LEVEL>: <message>` (`cli.py`
//! line 54): `[cli] INFO: Download size: <MiB> MiB (Compression savings: <n>%)` (`cli.py` 1044), then once a second
//! `[DLManager] INFO:  - Downloaded: <MiB> MiB, Written: <MiB> MiB` and ` + Download\t- <MiB> MiB/s (raw) / <MiB>
//! MiB/s (decompressed)` (`downloader/mp/manager.py` 766-775); [`parse`].
//!
//! **Signing in** is legendary's own (`legendary auth`, which on Windows shows Epic's sign-in page in a window of its
//! own, else asks for the code from `https://legendary.gl/epiclogin`): the Dashboard asks for no password and keeps
//! none, and says plainly where it stands (docs/spec/helpers.md 5).
pub mod standin;

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use model::{Can, Folder, Install, OnlineArt, Refusal, Start, Steer};
use serde_json::Value;
use source_kit::downloads::{self, Downloads, Line, Run};
use source_kit::HelperRoute;

/// What the log lines start with.
pub const LOG: &str = "epic: legendary";

/// legendary's own folder for its records, as it finds it itself (`lfs/lgndry.py`): `LEGENDARY_CONFIG_PATH`, else
/// `$XDG_CONFIG_HOME\legendary`, else `~\.config\legendary` (on Windows, `%USERPROFILE%\.config\legendary`).
pub fn default_home() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LEGENDARY_CONFIG_PATH").filter(|p| !p.is_empty()) {
        return Some(path.into());
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path).join("legendary"));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".config").join("legendary"))
}

/// Where legendary installs with no folder named (`core.py` `get_default_install_dir`): its config's `install_dir`,
/// else `~\Games`. The Dashboard names one always: this, the user's own folder, unless Settings > Storage says.
pub fn default_games() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join("Games"))
}

/// How the sign-in stands in `home`'s `user.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignIn {
    /// Signed in, as this account's display name.
    As(String),
    /// No sign-in kept.
    No,
}

/// `user.json`: signed in where it holds a token, as its `displayName`. Whether the token is still good only Epic
/// says, at legendary's next run: a run that is refused says so in the log.
pub fn signed_in(home: &Path) -> SignIn {
    let Some(value) = read_json(&home.join("user.json")) else { return SignIn::No };
    let token = ["refresh_token", "access_token"].iter().any(|key| value.get(*key).and_then(Value::as_str).is_some());
    if !token {
        return SignIn::No;
    }
    SignIn::As(value.get("displayName").and_then(Value::as_str).unwrap_or("").to_string())
}

/// A game legendary has installed (`installed.json`, one `InstalledGame` an `AppName`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub app_name: String,
    pub title: String,
    pub install_path: PathBuf,
    /// Its program, relative to `install_path`.
    pub executable: String,
    pub install_size: u64,
    pub is_dlc: bool,
}

/// What legendary has installed, by name; nothing where it has no `installed.json`.
pub fn installed(home: &Path) -> Vec<Installed> {
    let Some(Value::Object(map)) = read_json(&home.join("installed.json")) else { return Vec::new() };
    let mut games: Vec<Installed> = map
        .iter()
        .filter_map(|(key, value)| {
            let text = |name: &str| value.get(name).and_then(Value::as_str).unwrap_or("").to_string();
            let app_name = Some(text("app_name")).filter(|name| !name.is_empty()).unwrap_or_else(|| key.clone());
            let install_path = text("install_path");
            (!install_path.is_empty()).then(|| Installed {
                title: text("title"),
                install_path: source_kit::native(&install_path),
                executable: text("executable"),
                install_size: value.get("install_size").and_then(Value::as_u64).unwrap_or(0),
                is_dlc: value.get("is_dlc").and_then(Value::as_bool).unwrap_or(false),
                app_name,
            })
        })
        .collect();
    games.sort_by(|a, b| a.app_name.cmp(&b.app_name));
    games
}

/// A game of the account's library, as legendary last listed it (`metadata/<AppName>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Owned {
    pub app_name: String,
    pub title: String,
    pub namespace: String,
    pub item_id: String,
    pub developer: String,
    pub art: OnlineArt,
}

/// The games of the account's library legendary has listed: those it could install on Windows (an asset for
/// `Windows`), less DLC (`mainGameItem`, `models/game.py` `is_dlc`) and Unreal Engine content (namespace `ue`,
/// which `get_game_and_dlc_list` skips too).
pub fn owned(home: &Path) -> Vec<Owned> {
    let mut files: Vec<PathBuf> = fs::read_dir(home.join("metadata"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")))
        .collect();
    files.sort();
    files.iter().filter_map(|file| owned_game(&read_json(file)?)).collect()
}

fn owned_game(game: &Value) -> Option<Owned> {
    let meta = game.get("metadata").unwrap_or(&Value::Null);
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let windows = game.get("asset_infos").and_then(|assets| assets.get("Windows")).is_some();
    let dlc = meta.get("mainGameItem").is_some();
    let namespace = text(meta, "namespace");
    if !windows || dlc || namespace == "ue" {
        return None;
    }
    let app_name = text(game, "app_name");
    let title = Some(text(game, "app_title")).filter(|t| !t.is_empty()).unwrap_or_else(|| text(meta, "title"));
    (!app_name.is_empty()).then(|| Owned {
        title: Some(title).filter(|t| !t.is_empty()).unwrap_or_else(|| app_name.clone()),
        app_name,
        namespace,
        item_id: text(meta, "id"),
        developer: text(meta, "developer"),
        art: crate::key_images(meta),
    })
}

/// The folder a game goes into under `base`, as legendary names it (`core.py` `prepare_download`): its
/// `customAttributes.FolderName`, else its `AppName`.
pub fn folder_name(home: &Path, app_name: &str) -> String {
    let meta = read_json(&home.join("metadata").join(format!("{app_name}.json")));
    let name = meta.as_ref().and_then(|game| {
        let attributes = game.get("metadata")?.get("customAttributes")?;
        attributes.get("FolderName")?.get("value")?.as_str().map(str::to_string)
    });
    name.filter(|name| !name.is_empty()).unwrap_or_else(|| app_name.to_string())
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// What a line legendary writes says of its download: its logger's name and level stripped, `[DLManager] INFO: `.
pub fn parse(line: &str) -> Line {
    let message = match line.split_once("] ") {
        Some((logger, rest)) if logger.starts_with('[') => rest,
        _ => line,
    };
    let (level, message) = message.split_once(": ").unwrap_or(("", message));
    if matches!(level, "ERROR" | "CRITICAL" | "FATAL") {
        return Line::Error(message.trim().to_string());
    }
    // `Download size: 1234.56 MiB (Compression savings: 12.3%)`; not "Download size is 0, ...".
    if let Some(rest) = message.strip_prefix("Download size: ") {
        return mib(rest).map_or(Line::Other, Line::Total);
    }
    let body = message.trim_start();
    // ` - Downloaded: 512.00 MiB, Written: 600.00 MiB`
    if let Some(rest) = body.strip_prefix("- Downloaded: ") {
        return mib(rest).map_or(Line::Quiet, Line::Done);
    }
    // ` + Download\t- 25.00 MiB/s (raw) / 40.00 MiB/s (decompressed)`
    if let Some(rest) = body.strip_prefix("+ Download") {
        let rate = rest.trim_start_matches(['\t', ' ', '-']);
        return mib(rate).map_or(Line::Quiet, Line::Speed);
    }
    if body.starts_with("= Progress:") || body.starts_with("- Cache usage:") || body.starts_with("+ Disk") {
        return Line::Quiet;
    }
    Line::Other
}

/// The bytes of `<number> MiB...` at the start of `text`.
fn mib(text: &str) -> Option<u64> {
    let number: String = text.trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let value: f64 = number.parse().ok()?;
    text.trim_start()[number.len()..].trim_start().starts_with("MiB").then(|| (value * 1_048_576.0).round() as u64)
}

/// Epic's route through legendary: where it is and what it has under way.
#[derive(Debug, Clone)]
pub struct Legendary {
    pub route: HelperRoute,
    downloads: Downloads,
    /// The old launcher's `Data\Manifests`, for `egl-sync`: the machine's, or a fake's.
    egl_manifests: Option<PathBuf>,
}

impl Legendary {
    pub fn new(route: HelperRoute, egl_manifests: Option<PathBuf>) -> Self {
        Self { route, downloads: Downloads::new(LOG), egl_manifests }
    }

    /// legendary's program, while it is added.
    pub fn program(&self) -> Option<PathBuf> {
        (self.route.program)()
    }

    pub fn home(&self) -> &Path {
        &self.route.home
    }

    /// The program, where it is added and signed in: the only state anything is asked of it in. Why not, else.
    fn ready(&self) -> Result<PathBuf, String> {
        let program = self.program().ok_or("legendary is not added (Settings > Helpers)")?;
        match signed_in(self.home()) {
            SignIn::As(_) => Ok(program),
            SignIn::No => Err("legendary is not signed in to Epic Games (docs/spec/helpers.md 5)".into()),
        }
    }

    /// Whether it can be asked now.
    pub fn usable(&self) -> bool {
        self.ready().is_ok()
    }

    pub fn can(&self) -> Can {
        let usable = self.usable();
        Can { pause: usable, cancel: usable, choose_folder: false, quiet_uninstall: usable }
    }

    /// The one folder: Settings > Storage's choice where the Dashboard passes one, else the user's `Games`. Free
    /// space is not read.
    pub fn folders(&self) -> Vec<Folder> {
        vec![Folder { path: self.route.games.clone(), free: None, default: true }]
    }

    /// What it has under way, for the Queue.
    pub fn installs(&self) -> Vec<Install> {
        self.downloads.installs()
    }

    /// Whether legendary installed the game.
    pub fn has_installed(&self, app_name: &str) -> bool {
        installed(self.home()).iter().any(|game| game.app_name == app_name)
    }

    /// `legendary launch <AppName>`, for a game legendary installed, while it is added.
    pub fn start(&self, app_name: &str) -> Option<Start> {
        let program = self.program()?;
        self.has_installed(app_name).then(|| Start::Program {
            dir: program.parent().map(Path::to_path_buf),
            args: vec!["launch".into(), app_name.into()],
            path: program,
        })
    }

    fn base(&self, folder: Option<&Folder>) -> PathBuf {
        folder.map_or_else(|| self.route.games.clone(), |folder| folder.path.clone())
    }

    /// Installs the game into `folder` (legendary makes the game's own folder in it), its progress followed.
    /// Unsupported where legendary cannot be asked: the launcher is, as before.
    pub fn install(&self, app_name: &str, folder: Option<&Folder>) -> Result<(), Refusal> {
        let program = match self.ready() {
            Ok(program) => program,
            Err(why) => {
                eprintln!("{LOG}: {app_name}: {why}: asking the Epic Games Launcher instead");
                return Err(Refusal::Unsupported);
            }
        };
        let base = self.base(folder);
        if let Err(e) = fs::create_dir_all(&base) {
            return Err(Refusal::Failed(format!("{} could not be made: {e}", base.display())));
        }
        eprintln!("{LOG}: {app_name}: installing into {}", base.display());
        self.remember_base(app_name, folder);
        self.run_install(program, app_name, &base)
    }

    fn run_install(&self, program: PathBuf, app_name: &str, base: &Path) -> Result<(), Refusal> {
        let args = vec![
            "-y".into(),
            "install".into(),
            app_name.into(),
            "--base-path".into(),
            base.display().to_string(),
            "--skip-sdl".into(),
            "--skip-dlcs".into(),
        ];
        let (home, env, app) = (self.home().to_path_buf(), self.route.env.clone(), app_name.to_string());
        let (sync_program, egl) = (program.clone(), self.egl_manifests.clone().filter(|dir| dir.is_dir()));
        let finish = move || {
            let _ = fs::remove_file(base_file(&home, &app));
            if !installed(&home).iter().any(|game| game.app_name == app) {
                return Err("legendary's installed.json does not have it".to_string());
            }
            if let Some(egl) = egl {
                egl_sync(&sync_program, &env, &app, &egl);
            }
            Ok(())
        };
        let run = Run { program, args, env: self.route.env.clone(), parse, finish: Box::new(finish) };
        self.downloads.start(app_name, run).map_err(Refusal::Failed)
    }

    /// Pause stops legendary, which keeps its resume file (`<home>\tmp\<AppName>.resume`); Resume runs the same
    /// install again, into the folder the paused one was going to; Cancel stops it and deletes what it fetched.
    pub fn steer(&self, app_name: &str, steer: Steer) -> Result<(), Refusal> {
        if !self.downloads.has(app_name) {
            return Err(Refusal::Unsupported);
        }
        match steer {
            Steer::Pause => self.downloads.pause(app_name).map_err(Refusal::Failed),
            Steer::Resume => {
                if !self.downloads.paused(app_name) {
                    return Ok(());
                }
                let program = self.ready().map_err(Refusal::Failed)?;
                let base = self.paused_base(app_name).unwrap_or_else(|| self.route.games.clone());
                self.run_install(program, app_name, &base)
            }
            Steer::Cancel => {
                let (home, app) = (self.home().to_path_buf(), app_name.to_string());
                let base = self.paused_base(app_name).unwrap_or_else(|| self.route.games.clone());
                let cleanup = move || cancelled(&home, &app, &base);
                self.downloads.cancel(app_name, Box::new(cleanup)).map_err(Refusal::Failed)
            }
        }
    }

    /// The folder a game's install went to: the one it was started into, kept beside its resume file.
    fn paused_base(&self, app_name: &str) -> Option<PathBuf> {
        let text = fs::read_to_string(base_file(self.home(), app_name)).ok()?;
        Some(PathBuf::from(text.trim())).filter(|path| !path.as_os_str().is_empty())
    }

    /// `-y uninstall <AppName>`, on a thread of its own, for a game legendary installed: legendary deletes its folder
    /// and its record (and runs the game's own uninstaller where its record names one, `cli.py` 1205). The old
    /// launcher's record `egl-sync --one-shot` wrote is not legendary's to remove (only with its sync switched on,
    /// `core.py` 1825): the Dashboard removes it, once the game's folder is gone.
    pub fn uninstall(&self, app_name: &str) -> Result<(), Refusal> {
        let Some(game) = installed(self.home()).into_iter().find(|game| game.app_name == app_name) else {
            return Err(Refusal::Unsupported);
        };
        let program = self.program().ok_or_else(|| {
            Refusal::Failed("legendary installed it, and is not added now (Settings > Helpers)".into())
        })?;
        let (env, app, egl) = (self.route.env.clone(), app_name.to_string(), self.egl_manifests.clone());
        std::thread::spawn(move || {
            let args = vec!["-y".into(), "uninstall".into(), app.clone()];
            let _ = downloads::run_to_end(LOG, &app, &program, &args, &env, Duration::from_secs(600));
            if let Some(egl) = egl.filter(|_| !game.install_path.exists()) {
                forget_egl(&egl, &app, &game.install_path);
            }
        });
        Ok(())
    }

    /// `status --json`, which lists the account's library again into `metadata` (and checks for a legendary
    /// update, as it always does): blocking. Whether it ran well.
    pub fn refresh(&self) -> bool {
        let Ok(program) = self.ready() else { return false };
        let args = vec!["status".into(), "--json".into()];
        let ran = downloads::run_to_end(LOG, "library", &program, &args, &self.route.env, Duration::from_secs(300));
        matches!(ran, Ok((Some(0), _)))
    }

    /// Records where the game is being installed, so a Resume after the Dashboard has started again goes there too.
    pub fn remember_base(&self, app_name: &str, folder: Option<&Folder>) {
        let file = base_file(self.home(), app_name);
        let _ = fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
        let _ = fs::write(file, self.base(folder).display().to_string());
    }
}

/// Where the Dashboard keeps the folder a game is being installed into: beside legendary's resume file.
fn base_file(home: &Path, app_name: &str) -> PathBuf {
    home.join("tmp").join(format!("{app_name}.dashboard-base"))
}

/// What an install cancelled leaves deleted: its folder (made by legendary under `base`, named as
/// [`folder_name`] says), its resume file (`core.py` 1614), and where it was going. legendary keeps no record of an
/// install that never finished, so there is none to remove.
fn cancelled(home: &Path, app_name: &str, base: &Path) {
    let dir = base.join(folder_name(home, app_name));
    if dir.is_dir() && !installed(home).iter().any(|game| game.app_name == app_name) {
        if let Err(e) = fs::remove_dir_all(&dir) {
            eprintln!("{LOG}: {app_name}: {} could not be deleted: {e}", dir.display());
        }
    }
    let _ = fs::remove_file(home.join("tmp").join(format!("{app_name}.resume")));
    let _ = fs::remove_file(base_file(home, app_name));
}

/// Removes the old launcher's record of a game legendary uninstalled: the `.item` (named by its installation's guid,
/// `lfs/egl.py` 86) whose `AppName` is the game's and whose `InstallLocation` its folder.
fn forget_egl(manifests: &Path, app_name: &str, dir: &Path) {
    let items = fs::read_dir(manifests).into_iter().flatten().flatten().map(|entry| entry.path());
    for item in items.filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("item"))) {
        let Some(manifest) = read_json(&item) else { continue };
        let text = |key: &str| manifest.get(key).and_then(Value::as_str).unwrap_or("").to_string();
        if text("AppName") == app_name && source_kit::same_folder(&source_kit::native(&text("InstallLocation")), dir) {
            match fs::remove_file(&item) {
                Ok(()) => eprintln!("{LOG}: {app_name}: the old launcher's record {} removed", item.display()),
                Err(e) => eprintln!("{LOG}: {app_name}: {} could not be removed: {e}", item.display()),
            }
        }
    }
}

/// The old launcher's record of the games legendary installed, written by legendary (`cli.py` `egs_sync`).
fn egl_sync(program: &Path, env: &[(String, OsString)], app: &str, manifests: &Path) {
    let args = vec![
        "-y".into(),
        "egl-sync".into(),
        "--export-only".into(),
        "--one-shot".into(),
        "--egl-manifest-path".into(),
        manifests.display().to_string(),
    ];
    let _ = downloads::run_to_end(LOG, &format!("{app}: egl-sync"), program, &args, env, Duration::from_secs(120));
}

/// legendary's records merged into the launchers' items: each owned game it has listed added where no launcher
/// lists it (its ids and art filling what a launcher's item lacks), and each game it installed marked installed
/// there, with its folder and size.
pub(crate) fn merge(items: &mut Vec<crate::Item>, home: &Path) -> (usize, usize) {
    let (owned, installed) = (owned(home), installed(home));
    let mut by_name: HashMap<String, usize> =
        items.iter().enumerate().map(|(i, item)| (item.app_name.clone(), i)).collect();
    for game in &owned {
        match by_name.get(&game.app_name) {
            Some(&i) => {
                let item = &mut items[i];
                if item.namespace.is_empty() || item.item_id.is_empty() {
                    (item.namespace, item.item_id) = (game.namespace.clone(), game.item_id.clone());
                }
                if item.developer.is_empty() {
                    item.developer = game.developer.clone();
                }
                if item.art == OnlineArt::default() {
                    item.art = game.art.clone();
                }
            }
            None => {
                by_name.insert(game.app_name.clone(), items.len());
                items.push(crate::Item {
                    app_name: game.app_name.clone(),
                    namespace: game.namespace.clone(),
                    item_id: game.item_id.clone(),
                    title: game.title.clone(),
                    developer: game.developer.clone(),
                    install: None,
                    size: 0,
                    art: game.art.clone(),
                });
            }
        }
    }
    let mut found = 0;
    for game in installed.iter().filter(|game| !game.is_dlc && game.install_path.is_dir()) {
        found += 1;
        let install = Some((game.install_path.clone(), game.executable.clone()));
        match by_name.get(&game.app_name) {
            Some(&i) => {
                items[i].install = install;
                items[i].size = game.install_size;
            }
            None => {
                by_name.insert(game.app_name.clone(), items.len());
                items.push(crate::Item {
                    app_name: game.app_name.clone(),
                    title: Some(game.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| game.app_name.clone()),
                    install,
                    size: game.install_size,
                    ..crate::Item::default()
                });
            }
        }
    }
    (found, owned.len())
}

/// Whether legendary has left any record in `home`: then Epic is there, launcher or not.
pub(crate) fn any(home: &Path) -> bool {
    home.join("installed.json").is_file() || home.join("metadata").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines as legendary 0.21.1 prints them: its format strings (`cli.py` 1042-1049, `downloader/mp/manager.py`
    /// 766-775) under its logging format (`cli.py` 54), with made-up numbers.
    const INSTALL: &str = "\
[cli] INFO: Preparing download for \"Zinnia Gate\" (ZinniaGate)...
[cli] INFO: Install size: 1210.00 MiB
[cli] INFO: Download size: 1024.50 MiB (Compression savings: 15.3%)
[cli] INFO: Reusable size: 0.00 MiB (chunks) / 0.00 MiB (unchanged / skipped)
[cli] INFO: Downloads are resumable, you can interrupt the download with CTRL-C and resume it using the same command later on.
[DLManager] INFO: = Progress: 12.34% (123/999), Running for 00:01:02, ETA: 00:08:00
[DLManager] INFO:  - Downloaded: 512.00 MiB, Written: 600.00 MiB
[DLManager] INFO:  - Cache usage: 128.00 MiB, active tasks: 16
[DLManager] INFO:  + Download\t- 25.00 MiB/s (raw) / 40.00 MiB/s (decompressed)
[DLManager] INFO:  + Disk\t- 38.00 MiB/s (write) / 0.00 MiB/s (read)
[cli] INFO: Finished installation process in 92.04 seconds.
[cli] ERROR: Login failed! Cannot continue with download process.
[cli] CRITICAL: Installation cannot proceed, exiting.";

    #[test]
    fn legendarys_lines_give_the_whole_the_bytes_and_the_speed() {
        let read: Vec<Line> = INSTALL.lines().map(parse).collect();
        assert_eq!(
            read,
            [
                Line::Other,
                Line::Other,
                Line::Total(1_074_266_112),
                Line::Other,
                Line::Other,
                Line::Quiet,
                Line::Done(536_870_912),
                Line::Quiet,
                Line::Speed(26_214_400),
                Line::Quiet,
                Line::Other,
                Line::Error("Login failed! Cannot continue with download process.".into()),
                Line::Error("Installation cannot proceed, exiting.".into()),
            ]
        );
        // Nothing to download: not a whole.
        let none =
            "[cli] INFO: Download size is 0, the game is either already up to date or has not changed. Exiting...";
        assert_eq!(parse(none), Line::Other);
    }

    #[test]
    fn its_records_say_the_sign_in_the_installed_games_and_the_library() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        assert_eq!(signed_in(home), SignIn::No);
        assert!(installed(home).is_empty() && owned(home).is_empty() && !any(home));
        standin::write_signed_in(home).unwrap();
        assert_eq!(signed_in(home), SignIn::As(standin::DISPLAY_NAME.into()));
        standin::write_library(home).unwrap();
        let owned = owned(home);
        assert_eq!(owned.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), ["Zinnia Gate"]);
        assert_eq!(owned[0].developer, "Stand-in Studio");
        assert!(owned[0].art.cover.as_deref().is_some_and(|url| url.starts_with("https://")));
        assert_eq!(folder_name(home, "ZinniaGate"), "Zinnia Gate");
        assert_eq!(folder_name(home, "Unknown"), "Unknown");
        assert!(any(home));
    }
}
