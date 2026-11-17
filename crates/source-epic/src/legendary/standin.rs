//! The stand-in legendary, for the tests (`standin.exe` named `legendary.exe`, `crates/standin`): the commands the
//! Dashboard runs, answered as legendary 0.21.1 answers them, in its own words and line formats (`cli.py`,
//! `downloader/mp/manager.py`), and writing its own records (`user.json`, `metadata`, `installed.json`, the resume
//! file) as it writes them. Nothing is downloaded: a game's folder is a copy of the stand-in as its program, and its
//! bytes are counted out a step at a time (`STANDIN_HELPER_STEP_MS` apart, 600 ms without).
//!
//! It works only in a folder a test names (`LEGENDARY_CONFIG_PATH`), never in the user's own legendary folder.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The account the stand-in is signed in as.
pub const DISPLAY_NAME: &str = "Stand-in Player";

/// The one game of the stand-in account's library: owned, installed by no launcher. Its name starts with a letter no
/// other fixture's game has (Z).
pub const APP_NAME: &str = "ZinniaGate";
pub const TITLE: &str = "Zinnia Gate";
const NAMESPACE: &str = "5a696e6e6961476174654e616d657370";
const ITEM_ID: &str = "7a696e6e69616761746563617461";
/// Its program under its folder, as its record names it.
pub const EXECUTABLE: &str = "Binaries/Win64/game.exe";
/// The guid `egl-sync` gives its installation, and names the old launcher's record by.
pub const EGL_GUID: &str = "5354414E44494E4C4547454E44415259";
/// What it downloads, in steps.
const STEPS: u64 = 8;
const STEP_BYTES: u64 = 6 * 1_048_576;

/// Runs as legendary given `args`; its exit code.
pub fn run(args: &[String]) -> i32 {
    let Some(home) = std::env::var_os("LEGENDARY_CONFIG_PATH").filter(|p| !p.is_empty()).map(PathBuf::from) else {
        say("[cli] CRITICAL: the stand-in legendary works only in a folder a test names (LEGENDARY_CONFIG_PATH)");
        return 2;
    };
    let args: Vec<&str> = args.iter().map(String::as_str).filter(|a| !matches!(*a, "-y" | "--yes" | "-J")).collect();
    let result = match args.as_slice() {
        ["auth", "--code", _code] => auth(&home),
        ["auth", "--delete"] => {
            let _ = fs::remove_file(home.join("user.json"));
            say("[cli] INFO: User data deleted.");
            Ok(0)
        }
        ["status", rest @ ..] => status(&home, rest.contains(&"--json")),
        ["install", app, rest @ ..] => install(&home, app, rest),
        ["uninstall", app, ..] => uninstall(&home, app),
        ["launch", app, ..] => launch(&home, app),
        ["egl-sync", rest @ ..] => egl_sync(&home, rest),
        _ => {
            say(&format!("[cli] ERROR: the stand-in legendary does not know {}", args.join(" ")));
            Ok(1)
        }
    };
    result.unwrap_or_else(|e| {
        say(&format!("[cli] ERROR: stand-in: {e}"));
        1
    })
}

/// A line on standard error, as legendary's logging writes it, at once.
fn say(line: &str) {
    let mut err = std::io::stderr();
    let _ = writeln!(err, "{line}");
    let _ = err.flush();
}

/// Writes `user.json` as `legendary auth --code` leaves it.
pub fn write_signed_in(home: &Path) -> std::io::Result<()> {
    fs::create_dir_all(home)?;
    let user = json!({
        "access_token": "stand-in-access",
        "expires_at": "2099-01-01T00:00:00.000Z",
        "refresh_token": "stand-in-refresh",
        "refresh_expires_at": "2099-01-01T00:00:00.000Z",
        "account_id": "0000000000000000000000000000beef",
        "displayName": DISPLAY_NAME,
    });
    fs::write(home.join("user.json"), serde_json::to_string_pretty(&user)?)
}

/// Writes the stand-in account's library as legendary keeps it after listing it: `metadata/<AppName>.json` and
/// `assets.json`.
pub fn write_library(home: &Path) -> std::io::Result<()> {
    fs::create_dir_all(home.join("metadata"))?;
    let asset = json!({
        "app_name": APP_NAME, "asset_id": APP_NAME, "build_version": "1.0.0-standin",
        "catalog_item_id": ITEM_ID, "label_name": "Live", "namespace": NAMESPACE, "metadata": {}, "sidecar_rev": 0,
    });
    let image = |kind: &str| json!({"type": kind, "url": format!("https://cdn1.epicgames.com/standin/{kind}.jpg")});
    let game = json!({
        "app_name": APP_NAME,
        "app_title": TITLE,
        "asset_infos": {"Windows": asset},
        "base_urls": [],
        "metadata": {
            "id": ITEM_ID,
            "namespace": NAMESPACE,
            "title": TITLE,
            "developer": "Stand-in Studio",
            "keyImages": [image("DieselGameBoxTall"), image("DieselGameBox"), image("OfferImageWide")],
            "customAttributes": {"FolderName": {"type": "STRING", "value": TITLE}},
            "categories": [{"path": "games"}],
        },
        "sidecar": null,
        "achievements": null,
    });
    fs::write(home.join("metadata").join(format!("{APP_NAME}.json")), serde_json::to_string_pretty(&game)?)?;
    fs::write(home.join("assets.json"), serde_json::to_string(&json!({"Windows": [asset]}))?)
}

fn signed_in(home: &Path) -> bool {
    matches!(super::signed_in(home), super::SignIn::As(_))
}

fn auth(home: &Path) -> std::io::Result<i32> {
    say("[cli] INFO: Testing existing login data if present...");
    write_signed_in(home)?;
    say(&format!("[cli] INFO: Successfully logged in as \"{DISPLAY_NAME}\""));
    Ok(0)
}

fn status(home: &Path, json: bool) -> std::io::Result<i32> {
    if !signed_in(home) {
        say("[cli] ERROR: Log in failed!");
        return Ok(1);
    }
    write_library(home)?;
    let installed = super::installed(home).len();
    if json {
        let status = json!({"account": DISPLAY_NAME, "games_available": 1, "games_installed": installed,
                            "egl_sync_enabled": false, "config_directory": home.display().to_string()});
        println!("{status}");
    } else {
        println!("Epic account: {DISPLAY_NAME}\nGames available: 1\nGames installed: {installed}");
    }
    Ok(0)
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

fn install(home: &Path, app: &str, rest: &[&str]) -> std::io::Result<i32> {
    if !signed_in(home) {
        say("[cli] ERROR: Login failed! Cannot continue with download process.");
        return Ok(1);
    }
    if app != APP_NAME {
        say(&format!(
            "[cli] ERROR: Could not find \"{app}\" in list of available games, did you type the name correctly?"
        ));
        return Ok(1);
    }
    write_library(home)?;
    let base = rest.windows(2).find(|pair| pair[0] == "--base-path").map(|pair| PathBuf::from(pair[1]));
    let Some(base) = base else {
        say("[cli] ERROR: the stand-in legendary wants --base-path, as the Dashboard always gives it");
        return Ok(1);
    };
    let dir = base.join(TITLE);
    let resume = home.join("tmp").join(format!("{APP_NAME}.resume"));
    fs::create_dir_all(home.join("tmp"))?;
    let steps_done: u64 = fs::read_to_string(&resume).map(|text| text.lines().count() as u64).unwrap_or(0).min(STEPS);
    if steps_done > 0 {
        say("[DLM] INFO: Found previously interrupted download. Download will be resumed if possible.");
        say(&format!("[DLM] INFO: Skipping {steps_done} files based on resume data."));
    }
    let whole = STEPS * STEP_BYTES;
    let left = (STEPS - steps_done) * STEP_BYTES;
    say(&format!("[cli] INFO: Preparing download for \"{TITLE}\" ({APP_NAME})..."));
    say(&format!("[cli] INFO: Install size: {:.02} MiB", mib(whole)));
    say(&format!("[cli] INFO: Download size: {:.02} MiB (Compression savings: 0.0%)", mib(left)));
    say(&format!("[cli] INFO: Reusable size: 0.00 MiB (chunks) / {:.02} MiB (unchanged / skipped)", mib(whole - left)));
    say("[cli] INFO: Downloads are resumable, you can interrupt the download with CTRL-C and resume it using the same command later on.");
    fs::create_dir_all(&dir)?;
    let step = std::env::var("STANDIN_HELPER_STEP_MS").ok().and_then(|ms| ms.parse().ok()).unwrap_or(600u64);
    let began = Instant::now();
    let chunks = STEPS - steps_done;
    for n in 1..=chunks {
        std::thread::sleep(Duration::from_millis(step));
        let done = n * STEP_BYTES;
        let percent = n as f64 * 100.0 / chunks as f64;
        let secs = began.elapsed().as_secs();
        let eta = (chunks - n) * step / 1000;
        let speed = mib(STEP_BYTES) * 1000.0 / step as f64;
        say(&format!(
            "[DLManager] INFO: = Progress: {percent:.02}% ({n}/{chunks}), Running for {:02}:{:02}:{:02}, ETA: {:02}:{:02}:{:02}",
            secs / 3600, secs / 60 % 60, secs % 60, eta / 3600, eta / 60 % 60, eta % 60
        ));
        say(&format!("[DLManager] INFO:  - Downloaded: {:.02} MiB, Written: {:.02} MiB", mib(done), mib(done)));
        say("[DLManager] INFO:  - Cache usage: 6.00 MiB, active tasks: 1");
        say(&format!("[DLManager] INFO:  + Download\t- {speed:.02} MiB/s (raw) / {speed:.02} MiB/s (decompressed)"));
        say(&format!("[DLManager] INFO:  + Disk\t- {speed:.02} MiB/s (write) / 0.00 MiB/s (read)"));
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&resume)?;
        writeln!(file, "stand-in-hash:part-{}", steps_done + n)?;
    }
    let me = std::env::current_exe()?;
    source_kit::write_game_folder(&dir.join("Binaries").join("Win64"), TITLE, Some(&me))?;
    let mut installed: serde_json::Map<String, Value> = fs::read_to_string(home.join("installed.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    installed.insert(
        APP_NAME.into(),
        json!({
            "app_name": APP_NAME, "install_path": dir.display().to_string(), "title": TITLE, "version": "1.0.0-standin",
            "base_urls": [], "can_run_offline": true, "egl_guid": "", "executable": EXECUTABLE,
            "install_size": whole, "install_tags": [], "is_dlc": false, "launch_parameters": "",
            "manifest_path": "", "needs_verification": false, "platform": "Windows", "prereq_info": null,
            "uninstaller": null, "requires_ot": false, "save_path": null, "is_preloaded": false,
        }),
    );
    fs::write(home.join("installed.json"), serde_json::to_string_pretty(&installed)?)?;
    let _ = fs::remove_file(&resume);
    say(&format!("[cli] INFO: Finished installation process in {:.02} seconds.", began.elapsed().as_secs_f64()));
    Ok(0)
}

fn uninstall(home: &Path, app: &str) -> std::io::Result<i32> {
    let Some(game) = super::installed(home).into_iter().find(|game| game.app_name == app) else {
        say(&format!("[cli] ERROR: Game {app} not installed, cannot uninstall!"));
        return Ok(0);
    };
    say(&format!("[cli] INFO: Removing \"{}\" from \"{}\"...", game.title, game.install_path.display()));
    if game.install_path.is_dir() {
        fs::remove_dir_all(&game.install_path)?;
    }
    let mut installed: serde_json::Map<String, Value> =
        serde_json::from_str(&fs::read_to_string(home.join("installed.json"))?).unwrap_or_default();
    installed.remove(app);
    fs::write(home.join("installed.json"), serde_json::to_string_pretty(&installed)?)?;
    say("[cli] INFO: Game has been uninstalled.");
    Ok(0)
}

fn launch(home: &Path, app: &str) -> std::io::Result<i32> {
    let Some(game) = super::installed(home).into_iter().find(|game| game.app_name == app) else {
        say(&format!("[cli] ERROR: Game {app} is not currently installed!"));
        return Ok(1);
    };
    say("[cli] INFO: Logging in...");
    if !signed_in(home) {
        say("[cli] ERROR: Login failed, cannot continue!");
        return Ok(1);
    }
    let exe = game.executable.split(['/', '\\']).fold(game.install_path.clone(), |path, part| path.join(part));
    say(&format!("[cli] INFO: Launching {app}..."));
    std::process::Command::new(&exe).current_dir(exe.parent().unwrap_or(&game.install_path)).spawn()?;
    Ok(0)
}

/// `egl-sync --export-only`: each game it installed written into the old launcher's `Manifests` as the launcher's
/// own `.item` (`models/egl.py` `EGLManifest`, the fields the Epic source reads).
fn egl_sync(home: &Path, rest: &[&str]) -> std::io::Result<i32> {
    let Some(dir) = rest.windows(2).find(|pair| pair[0] == "--egl-manifest-path").map(|pair| PathBuf::from(pair[1]))
    else {
        say("[cli] ERROR: the stand-in legendary wants --egl-manifest-path");
        return Ok(1);
    };
    say("[cli] INFO: Reading EGL game manifests...");
    let mut records: serde_json::Map<String, Value> =
        serde_json::from_str(&fs::read_to_string(home.join("installed.json"))?).unwrap_or_default();
    for game in super::installed(home) {
        // The guid legendary gives the installation (`core.py` `egl_export`), kept in its record; the file named by it.
        let guid = EGL_GUID;
        let item = json!({
            "FormatVersion": 0, "AppName": game.app_name, "DisplayName": game.title,
            "InstallLocation": game.install_path.display().to_string(), "LaunchExecutable": game.executable,
            "CatalogNamespace": NAMESPACE, "CatalogItemId": ITEM_ID, "InstallSize": game.install_size,
            "AppCategories": ["public", "games", "applications"], "InstallationGuid": guid,
        });
        fs::write(dir.join(format!("{guid}.item")), serde_json::to_string_pretty(&item)?)?;
        if let Some(record) = records.get_mut(&game.app_name).and_then(Value::as_object_mut) {
            record.insert("egl_guid".into(), guid.into());
        }
        say(&format!("[cli] INFO: Exporting \"{}\"", game.title));
    }
    fs::write(home.join("installed.json"), serde_json::to_string_pretty(&records)?)?;
    Ok(0)
}
