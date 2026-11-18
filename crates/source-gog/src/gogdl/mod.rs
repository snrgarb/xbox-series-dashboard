//! GOG through gogdl (#176; docs/research/installs.md 4; docs/spec/helpers.md 5), the open GOG downloader Heroic
//! uses, which the user adds in Settings > Helpers. Everything here is read in gogdl's own source at the version the
//! Dashboard fetches, 1.4.0 (`Heroic-Games-Launcher/heroic-gogdl` at `6c6ef53`), and the set-up and uninstall steps in
//! Heroic's own, which drives gogdl the same way (`src/backend/storeManagers/gog/setup.ts` and `games.ts` at
//! `4b55a4cb70`); the file each fact comes from is named beside it.
//!
//! **Its folder** is the Dashboard's (`<data>\gogdl`, [`Gogdl::home`]): gogdl keeps no records of what it installed,
//! so the Dashboard keeps them there.
//! - `auth.json`: the sign-in, as `gogdl --auth-config-path <it> auth --code <code>` writes it (`auth.py`): GOG's
//!   token response under Galaxy's client id ([`signed_in`]).
//! - `heroic_gogdl\manifests\<id>`: the build's manifest gogdl writes after a download (`dl/managers/v2.py` 278), there
//!   because every run is given `GOGDL_CONFIG_PATH=<home>` (`constants.py`), so none goes to Heroic's own folder.
//! - `support\<id>`, `redist`: the game's support files (`--support`) and the script interpreter (`redist --ids ISI`).
//! - `installed.json` and `pending\<id>.json`: the Dashboard's own record of what it installed through gogdl, and of
//!   an install under way.
//!
//! **What is run**: `--auth-config-path <auth.json> download <id> --platform windows --path <folder> --support
//! <support\id> --skip-dlcs --lang en-US` (`args.py` 55-95, as Heroic runs it, `games.ts` 360), which makes the game's
//! folder in `<folder>` (its `installDirectory`, `v2.py` 290); then the game's set-up step: where the manifest says
//! `scriptInterpreter`, the script interpreter (`redist --ids ISI --path <redist>` fetches it; `info <id>` gives the
//! build and version it is told), else each product's `temp_executable` from its support folder (Heroic's
//! `setup.ts`). Uninstall is the game's own `unins000.exe /VERYSILENT /dir=<folder>` (Heroic's `games.ts` 980), then
//! what is left deleted.
//!
//! **Its progress** is on its standard error, through Python's logging as `[PROGRESS] INFO: <message>` (`cli.py` 25,
//! `dl/progressbar.py`): `= Progress: <percent> <written>/<total>, Running for: ..., ETA: ...` once a second, its
//! bytes counted after what its resume file (`.gogdl-resume` in the game's folder) skips; [`parse`].
//!
//! gogdl lists no library of its own: the games owned and not installed are Galaxy's database's (`galaxy`), and where
//! that is not on the PC, GOG's library service's, asked with gogdl's sign-in ([`library`], #276).
pub mod library;
pub mod standin;

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use model::{Can, Folder, Install, Refusal, Steer};
use serde_json::{json, Value};
use source_kit::downloads::{self, Downloads, Line, Run};
use source_kit::HelperRoute;

use crate::Record;

/// What the log lines start with.
pub const LOG: &str = "gog: gogdl";

/// GOG Galaxy's client id, under which `auth.json` keeps the sign-in (`auth.py`, Heroic's `gog/user.ts`).
pub const CLIENT_ID: &str = "46899977096215655";

/// GOG's own sign-in page for Galaxy's client, which ends on gogdl's redirect address (`auth.py` 12) with the code
/// `auth --code` takes in it. Only ever said to the user: the address is the one Heroic opens, not checked here
/// against a fixed version of Heroic's source.
pub const SIGN_IN_PAGE: &str = "https://login.gog.com/auth?client_id=46899977096215655&layout=galaxy&redirect_uri=\
                                https%3A%2F%2Fembed.gog.com%2Fon_login_success%3Forigin%3Dclient&response_type=code";

/// Whether `auth.json` holds a sign-in: Galaxy's client's tokens.
pub fn signed_in(auth: &Path) -> bool {
    let Some(value) = read_json(auth) else { return false };
    value.get(CLIENT_ID).and_then(|client| client.get("refresh_token")).and_then(Value::as_str).is_some()
}

/// What a line gogdl writes says of its download: its logger's name and level stripped, `[PROGRESS] INFO: `.
pub fn parse(line: &str) -> Line {
    let message = match line.split_once("] ") {
        Some((logger, rest)) if logger.starts_with('[') => rest,
        _ => line,
    };
    let (level, message) = message.split_once(": ").unwrap_or(("", message));
    if matches!(level, "ERROR" | "CRITICAL") {
        return Line::Error(message.trim().to_string());
    }
    let body = message.trim_start();
    // `= Progress: 12.34 1234/5678, Running for: 00:00:05, ETA: 00:00:40`
    if let Some(rest) = body.strip_prefix("= Progress: ") {
        let counts = rest.split_whitespace().nth(1).unwrap_or("").trim_end_matches(',');
        return match counts.split_once('/').map(|(done, total)| (done.parse(), total.parse())) {
            Some((Ok(done), Ok(total))) => Line::Progress { done, total },
            _ => Line::Quiet,
        };
    }
    // ` + Download\t- 25.00 MiB/s (raw) / 40.00 MiB/s (decompressed)`
    if let Some(rest) = body.strip_prefix("+ Download") {
        let rate = rest.trim_start_matches(['\t', ' ', '-']);
        return mib(rate).map_or(Line::Quiet, Line::Speed);
    }
    if body.starts_with("= Downloaded:") || body.starts_with("+ Disk") {
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

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(fs::read_to_string(path).ok()?.trim_start_matches('\u{feff}')).ok()
}

fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(path, serde_json::to_string_pretty(value)?)
}

/// The games the Dashboard installed through gogdl, as `installed.json` keeps them: each a record of the source's,
/// its folder still there.
pub fn installed(home: &Path) -> Vec<Record> {
    let Some(Value::Object(map)) = read_json(&home.join("installed.json")) else { return Vec::new() };
    let mut records: Vec<Record> = map
        .iter()
        .filter_map(|(id, value)| {
            let path = source_kit::native(value.get("path")?.as_str()?);
            path.is_dir().then(|| Record { id: id.clone(), path, ..Record::default() })
        })
        .collect();
    records.sort_by(|a, b| a.id.cmp(&b.id));
    records
}

fn set_installed(home: &Path, id: &str, path: Option<&Path>) -> std::io::Result<()> {
    let file = home.join("installed.json");
    let mut map = match read_json(&file) {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    match path {
        Some(path) => map.insert(id.into(), json!({ "path": path.display().to_string() })),
        None => map.remove(id),
    };
    write_json(&file, &Value::Object(map))
}

/// The game's folder gogdl made under `base`: the one holding its `goggame-<id>.info` (written by the build, as every
/// GOG game's folder has it).
fn game_folder(base: &Path, id: &str) -> Option<PathBuf> {
    let info = format!("goggame-{id}.info");
    let mut dirs: Vec<PathBuf> =
        fs::read_dir(base).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.into_iter().find(|dir| dir.join(&info).is_file())
}

fn subfolders(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// GOG's route through gogdl: where it is and what it has under way.
#[derive(Debug, Clone)]
pub struct Gogdl {
    pub route: HelperRoute,
    downloads: Downloads,
}

impl Gogdl {
    pub fn new(route: HelperRoute) -> Self {
        Self { route, downloads: Downloads::new(LOG) }
    }

    pub fn program(&self) -> Option<PathBuf> {
        (self.route.program)()
    }

    /// The Dashboard's folder for gogdl: the sign-in, the manifests, the support files and its records.
    pub fn home(&self) -> &Path {
        &self.route.home
    }

    pub fn auth(&self) -> PathBuf {
        self.home().join("auth.json")
    }

    /// Every run is given `GOGDL_CONFIG_PATH`, so gogdl's manifests are in the Dashboard's folder.
    fn env(&self) -> Vec<(String, OsString)> {
        let mut env = vec![("GOGDL_CONFIG_PATH".to_string(), self.home().as_os_str().to_owned())];
        env.extend(self.route.env.iter().cloned());
        env
    }

    fn ready(&self) -> Result<PathBuf, String> {
        let program = self.program().ok_or("gogdl is not added (Settings > Helpers)")?;
        if !signed_in(&self.auth()) {
            return Err("gogdl is not signed in to GOG (docs/spec/helpers.md 5)".into());
        }
        Ok(program)
    }

    pub fn usable(&self) -> bool {
        self.ready().is_ok()
    }

    pub fn can(&self) -> Can {
        let usable = self.usable();
        Can { pause: usable, cancel: usable, choose_folder: false, quiet_uninstall: usable }
    }

    pub fn folders(&self) -> Vec<Folder> {
        vec![Folder { path: self.route.games.clone(), free: None, default: true }]
    }

    pub fn installs(&self) -> Vec<Install> {
        self.downloads.installs()
    }

    fn pending_file(&self, id: &str) -> PathBuf {
        self.home().join("pending").join(format!("{id}.json"))
    }

    /// Downloads the game into `folder` (gogdl makes the game's own folder in it), its progress followed, then its
    /// set-up step. Unsupported where gogdl cannot be asked: Galaxy is, as before.
    pub fn install(&self, id: &str, folder: Option<&Folder>) -> Result<(), Refusal> {
        let program = match self.ready() {
            Ok(program) => program,
            Err(why) => {
                eprintln!("{LOG}: {id}: {why}: asking GOG Galaxy instead");
                return Err(Refusal::Unsupported);
            }
        };
        let base = folder.map_or_else(|| self.route.games.clone(), |folder| folder.path.clone());
        if let Err(e) = fs::create_dir_all(&base) {
            return Err(Refusal::Failed(format!("{} could not be made: {e}", base.display())));
        }
        // What was in the folder before: what Cancel may delete is only what this install made.
        let pending = json!({ "base": base.display().to_string(), "before": subfolders(&base) });
        if let Err(e) = write_json(&self.pending_file(id), &pending) {
            return Err(Refusal::Failed(format!("{} could not be written: {e}", self.pending_file(id).display())));
        }
        eprintln!("{LOG}: {id}: installing into {}", base.display());
        self.run_download(program, id, &base)
    }

    fn run_download(&self, program: PathBuf, id: &str, base: &Path) -> Result<(), Refusal> {
        let support = self.home().join("support").join(id);
        let args = vec![
            "--auth-config-path".into(),
            self.auth().display().to_string(),
            "download".into(),
            id.into(),
            "--platform".into(),
            "windows".into(),
            "--path".into(),
            base.display().to_string(),
            "--support".into(),
            support.display().to_string(),
            "--skip-dlcs".into(),
            "--lang".into(),
            "en-US".into(),
        ];
        let me = self.clone();
        let (game, base_dir) = (id.to_string(), base.to_path_buf());
        let finish = move || me.finished(&game, &base_dir);
        let run = Run { program, args, env: self.env(), parse, finish: Box::new(finish) };
        self.downloads.start(id, run).map_err(Refusal::Failed)
    }

    /// The download ended well: its folder found, its set-up step run, and the game recorded as the Dashboard's.
    fn finished(&self, id: &str, base: &Path) -> Result<(), String> {
        let dir = game_folder(base, id).ok_or(format!("no folder under {} holds goggame-{id}.info", base.display()))?;
        self.set_up(id, &dir);
        set_installed(self.home(), id, Some(&dir)).map_err(|e| format!("installed.json: {e}"))?;
        let _ = fs::remove_file(self.pending_file(id));
        eprintln!("{LOG}: {id}: installed in {}", dir.display());
        Ok(())
    }

    /// The game's set-up step, as Heroic runs it on Windows (`setup.ts`): what the build's manifest asks for. Each
    /// program is started as it is; one Windows refuses without an administrator's say (error 740) is started again
    /// through PowerShell's `Start-Process -Verb RunAs`, which puts Windows' permission prompt up. A step that fails
    /// is said, and the game is installed all the same, as Heroic does.
    fn set_up(&self, id: &str, dir: &Path) {
        let manifest = self.home().join("heroic_gogdl").join("manifests").join(id);
        let Some(manifest) = read_json(&manifest) else {
            eprintln!("{LOG}: {id}: set-up: no manifest at {}, so no set-up step", manifest.display());
            return;
        };
        let products: Vec<&Value> = manifest
            .get("products")
            .and_then(Value::as_array)
            .map(|products| {
                products.iter().filter(|p| p.get("productId").and_then(Value::as_str) == Some(id)).collect()
            })
            .unwrap_or_default();
        let support = self.home().join("support").join(id);
        if manifest.get("scriptInterpreter").and_then(Value::as_bool) == Some(true) {
            let Some(isi) = self.script_interpreter() else { return };
            let info = self.info(id);
            let text = |key: &str| info.as_ref().and_then(|info| info.get(key)?.as_str()).unwrap_or("").to_string();
            for product in products {
                let product = product.get("productId").and_then(Value::as_str).unwrap_or(id);
                let args = vec![
                    "/VERYSILENT".to_string(),
                    format!("/DIR={}", dir.display()),
                    "/Language=English".into(),
                    "/LANG=English".into(),
                    format!("/ProductId={product}"),
                    "/galaxyclient".into(),
                    format!("/buildId={}", text("buildId")),
                    format!("/versionName={}", text("versionName")),
                    "/lang-code=en-US".into(),
                    format!("/supportDir={}", support.display()),
                    "/nodesktopshortcut".into(),
                ];
                run_set_up(id, &isi, &args, dir);
            }
            return;
        }
        for product in products {
            let Some(exe) = product.get("temp_executable").and_then(Value::as_str).filter(|exe| !exe.is_empty()) else {
                continue;
            };
            let product_id = product.get("productId").and_then(Value::as_str).unwrap_or(id);
            let program = support.join(product_id).join(source_kit::native(exe));
            let args = vec![
                "/VERYSILENT".to_string(),
                format!("/DIR={}", dir.display()),
                "/Language=english".into(),
                "/LANG=english".into(),
                "/lang-code=en-US".into(),
                format!("/ProductId={product_id}"),
                "/galaxyclient".into(),
                "/nodesktopshortcut".into(),
            ];
            run_set_up(id, &program, &args, dir);
        }
    }

    /// The script interpreter, fetched by gogdl into the Dashboard's `redist` folder where it is not there yet.
    fn script_interpreter(&self) -> Option<PathBuf> {
        let redist = self.home().join("redist");
        let isi = redist.join("__redist").join("ISI").join("scriptinterpreter.exe");
        if !isi.is_file() {
            let program = self.program()?;
            let args = vec![
                "--auth-config-path".into(),
                self.auth().display().to_string(),
                "redist".into(),
                "--ids".into(),
                "ISI".into(),
                "--path".into(),
                redist.display().to_string(),
            ];
            let _ = downloads::run_to_end(LOG, "redist", &program, &args, &self.env(), Duration::from_secs(600));
        }
        if !isi.is_file() {
            eprintln!("{LOG}: set-up: no script interpreter at {}: the set-up step is not run", isi.display());
            return None;
        }
        Some(isi)
    }

    /// `info <id> --platform windows`: the build to install, as JSON on its standard output (`cli.py` 18).
    fn info(&self, id: &str) -> Option<Value> {
        let program = self.program()?;
        let args = vec![
            "--auth-config-path".into(),
            self.auth().display().to_string(),
            "info".into(),
            id.into(),
            "--platform".into(),
            "windows".into(),
        ];
        let what = format!("{id}: info");
        let (_, lines) =
            downloads::run_to_end(LOG, &what, &program, &args, &self.env(), Duration::from_secs(120)).ok()?;
        lines.iter().rev().find_map(|line| serde_json::from_str::<Value>(line).ok().filter(Value::is_object))
    }

    /// Pause stops gogdl, which keeps `.gogdl-resume` in the game's folder; Resume runs the same download again;
    /// Cancel stops it and deletes what it made.
    pub fn steer(&self, id: &str, steer: Steer) -> Result<(), Refusal> {
        if !self.downloads.has(id) {
            return Err(Refusal::Unsupported);
        }
        let pending = read_json(&self.pending_file(id));
        let base = pending
            .as_ref()
            .and_then(|pending| pending.get("base")?.as_str().map(PathBuf::from))
            .unwrap_or_else(|| self.route.games.clone());
        match steer {
            Steer::Pause => self.downloads.pause(id).map_err(Refusal::Failed),
            Steer::Resume => {
                if !self.downloads.paused(id) {
                    return Ok(());
                }
                let program = self.ready().map_err(Refusal::Failed)?;
                self.run_download(program, id, &base)
            }
            Steer::Cancel => {
                let before: HashSet<String> = pending
                    .as_ref()
                    .and_then(|pending| pending.get("before")?.as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_string))
                    .collect();
                let (home, game, pending_file) = (self.home().to_path_buf(), id.to_string(), self.pending_file(id));
                let cleanup = move || {
                    for name in subfolders(&base).into_iter().filter(|name| !before.contains(name)) {
                        let dir = base.join(name);
                        if let Err(e) = fs::remove_dir_all(&dir) {
                            eprintln!("{LOG}: {game}: {} could not be deleted: {e}", dir.display());
                        }
                    }
                    forget(&home, &game);
                    let _ = fs::remove_file(pending_file);
                };
                self.downloads.cancel(id, Box::new(cleanup)).map_err(Refusal::Failed)
            }
        }
    }

    /// The game's own `unins000.exe /VERYSILENT /dir=<folder>` on a thread of its own, then what is left of its
    /// folder, its manifest, its support files and its record deleted. A game the Dashboard did not install through
    /// gogdl is not its to uninstall this way: its uninstaller is asked as before.
    pub fn uninstall(&self, id: &str) -> Result<(), Refusal> {
        let Some(record) = installed(self.home()).into_iter().find(|record| record.id == id) else {
            return Err(Refusal::Unsupported);
        };
        let (home, game) = (self.home().to_path_buf(), id.to_string());
        std::thread::spawn(move || {
            let dir = record.path;
            let uninstaller = dir.join("unins000.exe");
            if uninstaller.is_file() {
                let args = vec!["/VERYSILENT".to_string(), format!("/dir={}", dir.display())];
                run_set_up(&game, &uninstaller, &args, &dir);
            }
            if dir.exists() {
                if let Err(e) = fs::remove_dir_all(&dir) {
                    eprintln!("{LOG}: {game}: {} could not be deleted: {e}", dir.display());
                }
            }
            forget(&home, &game);
            let _ = set_installed(&home, &game, None);
            eprintln!("{LOG}: {game}: uninstalled from {}", dir.display());
        });
        Ok(())
    }
}

/// gogdl's manifest of the game and its support files: gone, so a later download starts afresh (Heroic does the
/// same, `games.ts` 990).
fn forget(home: &Path, id: &str) {
    let _ = fs::remove_file(home.join("heroic_gogdl").join("manifests").join(id));
    let _ = fs::remove_dir_all(home.join("support").join(id));
}

/// Runs a set-up or uninstall program to its end, its window its own: started as it is, and where Windows wants an
/// administrator for it (error 740, `ERROR_ELEVATION_REQUIRED`) through `powershell Start-Process -Verb RunAs
/// -Wait`, as Heroic always does (`setup.ts` `runSetupCommand`). Windows' permission prompt is on its secure desktop,
/// where no program's input reaches: the user answers it.
fn run_set_up(id: &str, program: &Path, args: &[String], dir: &Path) {
    let name = program.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    eprintln!("{LOG}: {id}: running {name} {}", args.join(" "));
    let started = Instant::now();
    let child = Command::new(program).args(args).current_dir(dir).spawn();
    let status = match child {
        Ok(mut child) => child.wait(),
        Err(e) if e.raw_os_error() == Some(740) => {
            eprintln!("{LOG}: {id}: {name} wants an administrator: asking Windows");
            let quoted: Vec<String> = args.iter().map(|arg| format!("'{}'", arg.replace('\'', "''"))).collect();
            let command = format!(
                "Start-Process -FilePath '{}' -Verb RunAs -Wait -ArgumentList {}",
                program.display().to_string().replace('\'', "''"),
                quoted.join(",")
            );
            Command::new("powershell").args(["-NoProfile", "-Command", &command]).current_dir(dir).status()
        }
        Err(e) => Err(e),
    };
    let secs = started.elapsed().as_secs();
    match status {
        Ok(status) => eprintln!("{LOG}: {id}: {name} ended with {} after {secs} s", status.code().unwrap_or(-1)),
        Err(e) => eprintln!("{LOG}: {id}: {name} did not run: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines as gogdl 1.4.0 prints them: its format strings (`dl/progressbar.py` 94-112) under its logging format
    /// (`cli.py` 25), with made-up numbers.
    const DOWNLOAD: &str = "\
[V2] INFO: Initialized V2 Download Manager
[PROGRESS] INFO: = Progress: 12.50 6291456/50331648, Running for: 00:00:03, ETA: 00:00:21
[PROGRESS] INFO: = Downloaded: 6.00 MiB, Written: 6.00 MiB
[PROGRESS] INFO:  + Download\t- 2.00 MiB/s (raw) / 2.50 MiB/s (decompressed)
[PROGRESS] INFO:  + Disk\t- 2.00 MiB/s (write) / 0.00 MiB/s (read)
[V2] ERROR: Unable to proceed, Not enough disk space";

    #[test]
    fn gogdls_lines_give_the_bytes_the_whole_and_the_speed() {
        let read: Vec<Line> = DOWNLOAD.lines().map(parse).collect();
        assert_eq!(
            read,
            [
                Line::Other,
                Line::Progress { done: 6_291_456, total: 50_331_648 },
                Line::Quiet,
                Line::Speed(2_097_152),
                Line::Quiet,
                Line::Error("Unable to proceed, Not enough disk space".into()),
            ]
        );
    }

    #[test]
    fn the_sign_in_and_the_records_are_read_from_its_folder() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        assert!(!signed_in(&home.join("auth.json")));
        standin::write_auth(&home.join("auth.json")).unwrap();
        assert!(signed_in(&home.join("auth.json")));
        fs::write(home.join("other.json"), r#"{"1": {"refresh_token": "x"}}"#).unwrap();
        assert!(!signed_in(&home.join("other.json")));

        assert!(installed(home).is_empty());
        let dir = home.join("games").join("Yarrow");
        fs::create_dir_all(&dir).unwrap();
        set_installed(home, "1900000777", Some(&dir)).unwrap();
        set_installed(home, "1900000778", Some(&home.join("gone"))).unwrap();
        let records = installed(home);
        assert_eq!(records.iter().map(|r| (r.id.as_str(), r.path.clone())).collect::<Vec<_>>(), [("1900000777", dir)]);
        set_installed(home, "1900000777", None).unwrap();
        assert!(installed(home).is_empty());
    }
}
