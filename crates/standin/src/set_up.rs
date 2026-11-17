//! The stand-in launchers' installers and uninstallers (Settings > Set up this PC, #178; docs/spec/set-up.md 6):
//! this program named `standin-setup-<key>.exe`, as `serve-helpers --clients` serves it, or
//! `standin-uninstall-<key>.exe`, as the stand-in installer leaves it in the app's folder.
//!
//! They touch nothing of the machine's: only the test's own folder that `STANDIN_SETUP` names, which holds the
//! registry file standing in for Windows' list of installed programs (`registry.json`, read by the Dashboard with
//! `--setup-registry`), what each run was given (`runs.txt`), and each app's folder. Without `STANDIN_SETUP` they
//! write nothing and end with 3.
//!
//! - The installer writes `runs.txt`'s `install <key> <its switches>`, waits `STANDIN_SETUP_MS` (1500 without), makes
//!   the app's folder (`--installpath=`'s where it is given one, else `<STANDIN_SETUP>\<key>`) with the uninstaller in
//!   it, and the app's Uninstall key in the registry file, named and shown as the real app's (`helpers::clients`).
//!   Battle.net's folder is a whole fake Battle.net too (`source_battlenet::fixture`, its games' `game.exe` this
//!   program), so `--battlenet <folder>` reads its games once it is installed. With `STANDIN_SETUP_EXIT` set, it ends
//!   with that code having installed nothing.
//! - The uninstaller writes `uninstall <key> <its switches>` and hands over to a copy of itself outside the app's
//!   folder (`<STANDIN_SETUP>\handover`, told the folder with `_?=`, as an NSIS uninstaller does) and ends at once;
//!   the copy waits as long, empties the app's folder and then removes its key from the registry file.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use source_kit::registry::{Registry, Values};

use crate::note;

/// The environment variable naming the test's own folder.
pub const SANDBOX: &str = "STANDIN_SETUP";
/// The registry file in it.
pub const REGISTRY: &str = "registry.json";
/// What each run was given.
pub const RUNS: &str = "runs.txt";

/// The argument an uninstaller's copy is told the app's folder by, as NSIS's are.
const HANDED: &str = "_?=";

const UNINSTALL: &str = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

/// The key this program stands in for, by its own name: `standin-setup-<key>` or `standin-uninstall-<key>`.
pub fn named() -> Option<(bool, String)> {
    let exe = std::env::current_exe().ok()?;
    let stem = exe.file_stem()?.to_string_lossy().to_lowercase();
    if let Some(key) = stem.strip_prefix("standin-setup-") {
        return Some((true, key.to_string()));
    }
    stem.strip_prefix("standin-uninstall-").map(|key| (false, key.to_string()))
}

fn sandbox() -> Option<PathBuf> {
    std::env::var_os(SANDBOX).map(PathBuf::from).filter(|dir| dir.is_dir())
}

fn wait() {
    let ms = std::env::var("STANDIN_SETUP_MS").ok().and_then(|ms| ms.parse().ok()).unwrap_or(1500);
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn record(sandbox: &Path, line: &str) {
    use std::io::Write;
    let file = std::fs::OpenOptions::new().create(true).append(true).open(sandbox.join(RUNS));
    if let Ok(mut file) = file {
        let _ = writeln!(file, "{line}");
    }
}

fn keys(sandbox: &Path) -> BTreeMap<String, Values> {
    match Registry::load(&sandbox.join(REGISTRY)) {
        Ok(Registry::File(keys)) => keys,
        _ => BTreeMap::new(),
    }
}

/// The app's key's name in the registry file: its first product code, else one made up from its key.
fn key_name(client: &helpers::clients::Client) -> String {
    client.entries.first().cloned().unwrap_or_else(|| format!("{{STANDIN-{}}}", client.key.to_uppercase()))
}

/// Runs as the installer (`install`) or the uninstaller of `key`'s app, with its switches `args`.
pub fn run(install: bool, key: &str, args: &[String]) -> i32 {
    let what = if install { "install" } else { "uninstall" };
    let line = format!("{what} {key} {}", args.join(" "));
    note(&format!("set-up: {}", line.trim_end()));
    let Some(sandbox) = sandbox() else {
        note(&format!("set-up: {SANDBOX} names no test's folder: nothing written"));
        return 3;
    };
    let Some(client) = helpers::clients::builtin().into_iter().find(|client| client.key == key) else {
        note(&format!("set-up: no launcher {key}"));
        return 2;
    };
    // The uninstaller's copy it handed over to says nothing more of its run.
    if !args.iter().any(|arg| arg.starts_with(HANDED)) {
        record(&sandbox, line.trim_end());
    }
    wait();
    let done = if install { installed(&sandbox, &client, args) } else { removed(&sandbox, &client, args) };
    match done {
        Ok(code) => code,
        Err(e) => {
            note(&format!("set-up: {what} {key}: {e}"));
            1
        }
    }
}

fn installed(sandbox: &Path, client: &helpers::clients::Client, args: &[String]) -> Result<i32, String> {
    if let Some(code) = std::env::var("STANDIN_SETUP_EXIT").ok().and_then(|code| code.parse().ok()) {
        note(&format!("set-up: install {}: ending with {code}, nothing installed", client.key));
        return Ok(code);
    }
    let given =
        args.iter().find_map(|arg| arg.strip_prefix("--installpath=")).map(|p| PathBuf::from(p.trim_matches('"')));
    let folder = std::path::absolute(given.unwrap_or_else(|| sandbox.join(&client.key))).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let failed = |e: std::io::Error| format!("{}: {e}", folder.display());
    if client.key == "battlenet" {
        let spec = source_battlenet::fixture::Fixture {
            game_exe: Some(exe.clone()),
            ..source_battlenet::fixture::Fixture::standard()
        };
        source_battlenet::fixture::write(&folder, &spec).map_err(failed)?;
    }
    std::fs::create_dir_all(&folder).map_err(failed)?;
    let uninstaller = folder.join(format!("standin-uninstall-{}.exe", client.key));
    std::fs::copy(&exe, &uninstaller).map_err(failed)?;
    let mut keys = keys(sandbox);
    let values: Values = [
        ("DisplayName", client.display.clone()),
        ("DisplayVersion", client.version.clone()),
        ("Publisher", client.vendor.clone()),
        ("InstallLocation", folder.display().to_string()),
        ("UninstallString", format!("\"{}\"", uninstaller.display())),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    keys.insert(format!(r"{UNINSTALL}\{}", key_name(client)), values);
    Registry::write(&sandbox.join(REGISTRY), &keys).map_err(|e| e.to_string())?;
    note(&format!("set-up: installed {} at {}", client.key, folder.display()));
    Ok(0)
}

fn removed(sandbox: &Path, client: &helpers::clients::Client, args: &[String]) -> Result<i32, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let Some(folder) = args.iter().find_map(|arg| arg.strip_prefix(HANDED)).map(PathBuf::from) else {
        // Run from the app's folder: handed over to a copy of itself outside it, which can empty the folder, as an
        // NSIS uninstaller hands over to one in the temporary folder. This one ends at once.
        let handover = sandbox.join("handover");
        std::fs::create_dir_all(&handover).map_err(|e| e.to_string())?;
        let copy = handover.join(me.file_name().unwrap_or_default());
        std::fs::copy(&me, &copy).map_err(|e| format!("{}: {e}", copy.display()))?;
        let folder = me.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut given = args.to_vec();
        given.push(format!("{HANDED}{}", folder.display()));
        std::process::Command::new(&copy).args(&given).spawn().map_err(|e| format!("{}: {e}", copy.display()))?;
        note(&format!("set-up: uninstall {}: handed over to {}", client.key, copy.display()));
        return Ok(0);
    };
    // The app's folder emptied, and left: an empty folder named for a store is no store (`source_kit::empty_folder`).
    // Then its key, last, as a real uninstaller's: once the list no longer has it, the app is gone.
    for item in std::fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))?.flatten() {
        let path = item.path();
        let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    }
    let mut keys = keys(sandbox);
    keys.remove(&format!(r"{UNINSTALL}\{}", key_name(client)));
    Registry::write(&sandbox.join(REGISTRY), &keys).map_err(|e| e.to_string())?;
    note(&format!("set-up: removed {} from {}", client.key, folder.display()));
    Ok(0)
}
