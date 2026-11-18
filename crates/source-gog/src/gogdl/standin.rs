//! The stand-in gogdl, for the tests (`standin.exe` named `gogdl.exe`, `crates/standin`): the commands the GOG
//! source runs, answered as gogdl 1.4.0 answers them, in its own line formats (`dl/progressbar.py`, `cli.py`) and
//! writing its own files (`auth.json`, the manifest, `.gogdl-resume`); and the programs a GOG build brings, the script
//! interpreter (`scriptinterpreter.exe`) and the game's uninstaller (`unins000.exe`), both copies of the stand-in.
//! Nothing is downloaded: a game's folder holds a copy of the stand-in as its program, its `goggame-<id>.info` and
//! its uninstaller, and its bytes are counted out a step at a time (`STANDIN_HELPER_STEP_MS` apart, 600 ms without).
//!
//! It works only with `GOGDL_CONFIG_PATH` naming a test's folder, as the GOG source always gives it.
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// What it downloads, in steps.
const STEPS: u64 = 8;
const STEP_BYTES: u64 = 6 * 1_048_576;
/// The build and version `info` says.
pub const BUILD_ID: &str = "57000000000000001";
pub const VERSION_NAME: &str = "1.0 (stand-in)";
/// What the script interpreter writes in the game's folder: the arguments it was given, a line each.
pub const SET_UP_FILE: &str = "standin-set-up.txt";

/// Writes `auth.json` as `gogdl auth --code` leaves it (`auth.py` 140-143), with made-up tokens.
pub fn write_auth(path: &Path) -> std::io::Result<()> {
    let data = json!({
        super::CLIENT_ID: {
            "expires_in": 3600, "access_token": "stand-in-access", "refresh_token": "stand-in-refresh",
            "user_id": "48151623420001", "session_id": "stand-in-session", "token_type": "bearer",
            "loginTime": 4_102_444_800.0,
        }
    });
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(path, data.to_string())
}

/// The game's name: the GOG fixture's, by its product id, else made up from it.
fn title(id: &str) -> String {
    let fixture = crate::fixture::Fixture::standard();
    fixture.games.into_iter().find(|game| game.id == id).map_or_else(|| format!("Game {id}"), |game| game.name)
}

fn say(line: &str) {
    let mut err = std::io::stderr();
    let _ = writeln!(err, "{line}");
    let _ = err.flush();
}

fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2).find(|pair| pair[0] == flag).map(|pair| pair[1].as_str())
}

/// Runs as gogdl given `args`; its exit code.
pub fn run(args: &[String]) -> i32 {
    let Some(config) = std::env::var_os("GOGDL_CONFIG_PATH").filter(|p| !p.is_empty()).map(PathBuf::from) else {
        say("[GOGDL] ERROR: the stand-in gogdl works only in a folder a test names (GOGDL_CONFIG_PATH)");
        return 2;
    };
    let auth = value(args, "--auth-config-path").map(PathBuf::from);
    let command: Vec<&str> = {
        let mut rest = args.iter().map(String::as_str);
        let mut left = Vec::new();
        while let Some(arg) = rest.next() {
            if arg == "--auth-config-path" {
                rest.next();
            } else {
                left.push(arg);
            }
        }
        left
    };
    let Some(auth) = auth else {
        say("[GOGDL] ERROR: the stand-in gogdl wants --auth-config-path, as the GOG source always gives it");
        return 2;
    };
    let result = match command.as_slice() {
        ["auth", "--code", _] => write_auth(&auth).map(|()| {
            println!(
                "{}",
                fs::read_to_string(&auth)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v.get(super::CLIENT_ID).cloned())
                    .unwrap_or_default()
            );
            0
        }),
        _ if !super::signed_in(&auth) => {
            // gogdl's own way with no credentials: it cannot reach GOG's API (`api.py`).
            say("[API] ERROR: No credentials, run gogdl auth first");
            Ok(1)
        }
        // `auth` signed in: gogdl renews a token that has run out and prints the credentials (`auth.py` 42-122); the
        // stand-in's run out in 2100, so it prints them as they are.
        ["auth"] => {
            println!("{}", super::read_json(&auth).and_then(|v| v.get(super::CLIENT_ID).cloned()).unwrap_or_default());
            Ok(0)
        }
        ["download", id, rest @ ..] => download(&config, id, rest),
        ["info", id, ..] => {
            let info = json!({"size": {}, "dlcs": [], "buildId": BUILD_ID, "languages": ["en-US"],
                              "folder_name": title(id), "dependencies": [], "versionEtag": "", "versionName": VERSION_NAME,
                              "available_branches": [null]});
            println!("{info}");
            Ok(0)
        }
        ["redist", rest @ ..] if rest.contains(&"ISI") => redist(rest),
        _ => {
            say(&format!("[GOGDL] ERROR: the stand-in gogdl does not know {}", command.join(" ")));
            Ok(1)
        }
    };
    result.unwrap_or_else(|e| {
        say(&format!("[GOGDL] ERROR: stand-in: {e}"));
        1
    })
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

fn download(config: &Path, id: &str, rest: &[&str]) -> std::io::Result<i32> {
    let Some(base) = rest.windows(2).find(|pair| pair[0] == "--path").map(|pair| PathBuf::from(pair[1])) else {
        say("[GOGDL] ERROR: the stand-in gogdl wants --path");
        return Ok(1);
    };
    let support = rest.windows(2).find(|pair| pair[0] == "--support").map(|pair| PathBuf::from(pair[1]));
    let name = title(id);
    let dir = base.join(&name);
    fs::create_dir_all(&dir)?;
    say("[V2] INFO: Initialized V2 Download Manager");
    // The resume file, a line a step done (`dl/managers/task_executor.py` 27).
    let resume = dir.join(".gogdl-resume");
    let steps_done = fs::read_to_string(&resume).map(|text| text.lines().count() as u64).unwrap_or(0).min(STEPS);
    let total = (STEPS - steps_done) * STEP_BYTES;
    let step = std::env::var("STANDIN_HELPER_STEP_MS").ok().and_then(|ms| ms.parse().ok()).unwrap_or(600u64);
    let began = Instant::now();
    let chunks = STEPS - steps_done;
    for n in 1..=chunks {
        std::thread::sleep(Duration::from_millis(step));
        let written = n * STEP_BYTES;
        let percent = written as f64 * 100.0 / total as f64;
        let secs = began.elapsed().as_secs();
        let eta = (chunks - n) * step / 1000;
        let speed = mib(STEP_BYTES) * 1000.0 / step as f64;
        say(&format!(
            "[PROGRESS] INFO: = Progress: {percent:.02} {written}/{total}, Running for: {:02}:{:02}:{:02}, ETA: {:02}:{:02}:{:02}",
            secs / 3600, secs / 60 % 60, secs % 60, eta / 3600, eta / 60 % 60, eta % 60
        ));
        say(&format!("[PROGRESS] INFO: = Downloaded: {:.02} MiB, Written: {:.02} MiB", mib(written), mib(written)));
        say(&format!("[PROGRESS] INFO:  + Download\t- {speed:.02} MiB/s (raw) / {speed:.02} MiB/s (decompressed)"));
        say(&format!("[PROGRESS] INFO:  + Disk\t- {speed:.02} MiB/s (write) / 0.00 MiB/s (read)"));
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&resume)?;
        writeln!(file, "stand-in-chunk-{}", steps_done + n)?;
    }
    // The build's files: the game's program, its .info, its uninstaller.
    let me = std::env::current_exe()?;
    source_kit::write_game_folder(&dir, &name, Some(&me))?;
    fs::copy(&me, dir.join("unins000.exe"))?;
    let info = json!({"gameId": id, "rootGameId": id, "name": name, "version": 1,
                      "playTasks": [{"isPrimary": true, "type": "FileTask", "path": "game.exe", "name": name}]});
    fs::write(dir.join(format!("goggame-{id}.info")), info.to_string())?;
    if let Some(support) = support {
        fs::create_dir_all(support.join(id))?;
        fs::write(support.join(id).join("readme.txt"), "stand-in support files\n")?;
    }
    // The manifest, as GOG's content system gives it (`dl/managers/v2.py` 278): the set-up step's script interpreter.
    let manifest = json!({"version": 2, "baseProductId": id, "installDirectory": name, "scriptInterpreter": true,
                          "dependencies": [], "depots": [], "products": [{"productId": id, "name": name}]});
    let manifests = config.join("heroic_gogdl").join("manifests");
    fs::create_dir_all(&manifests)?;
    fs::write(manifests.join(id), manifest.to_string())?;
    let _ = fs::remove_file(&resume);
    Ok(0)
}

/// `redist --ids ISI --path <dir>`: the script interpreter at `<dir>\__redist\ISI\scriptinterpreter.exe`, a copy of
/// the stand-in.
fn redist(rest: &[&str]) -> std::io::Result<i32> {
    let Some(path) = rest.windows(2).find(|pair| pair[0] == "--path").map(|pair| PathBuf::from(pair[1])) else {
        say("[GOGDL] ERROR: the stand-in gogdl wants --path");
        return Ok(1);
    };
    let isi = path.join("__redist").join("ISI");
    fs::create_dir_all(&isi)?;
    fs::copy(std::env::current_exe()?, isi.join("scriptinterpreter.exe"))?;
    say("[REDIST] INFO: Downloaded ISI");
    Ok(0)
}

/// As the script interpreter: the arguments written to the game's folder (`/DIR=`), a line each.
pub fn script_interpreter(args: &[String]) -> i32 {
    let Some(dir) = args.iter().find_map(|arg| arg.strip_prefix("/DIR=")) else { return 1 };
    match fs::write(Path::new(dir).join(SET_UP_FILE), args.join("\n") + "\n") {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// As the game's uninstaller, `unins000.exe /VERYSILENT /dir=<folder>`: every file of the folder deleted but itself,
/// which a running program cannot delete.
pub fn uninstaller(args: &[String]) -> i32 {
    let me = std::env::current_exe().ok();
    let dir = args
        .iter()
        .find_map(|arg| arg.strip_prefix("/dir=").or_else(|| arg.strip_prefix("/DIR=")))
        .map(PathBuf::from)
        .or_else(|| me.as_ref().and_then(|me| me.parent().map(Path::to_path_buf)));
    let Some(dir) = dir else { return 1 };
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if me.as_ref().is_some_and(|me| me.file_name() == path.file_name()) {
            continue;
        }
        let _ = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
    }
    0
}

/// A game in the stand-in account's library that is in neither the GOG fixture's folders nor its database: named
/// only by the product API, gamesdb not knowing it.
pub const LIBRARY_ONLY: (&str, &str) = ("1900000104", "Quarrel Fen");

/// A stand-in of GOG's web services on 127.0.0.1 (`standin serve-gog-web`), as `library` asks them, for the tests:
/// `/galaxy-library/users/<id>/releases` (two entries a page, `next_page_token` to the next), which answers 401 to any
/// sign-in but the stand-in gogdl's; `/gamesdb/platforms/gog/external_releases/<id>`; `/api/products/<id>`. It keeps
/// the path of every request and whether it came with the stand-in's sign-in.
pub struct Web {
    pub addr: std::net::SocketAddr,
    pub asked: Arc<Mutex<Vec<(String, bool)>>>,
}

impl Web {
    /// Listens at `port` (0: any free one), answering on a thread of its own for as long as the program runs.
    pub fn start(port: u16) -> std::io::Result<Web> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let asked = Arc::new(Mutex::new(Vec::new()));
        let kept = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let kept = kept.clone();
                std::thread::spawn(move || {
                    let _ = answer_web(stream, &kept);
                });
            }
        });
        Ok(Web { addr, asked })
    }

    /// Its address, as `DASHBOARD_STORE_WEB` takes it.
    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }
}

/// The library's entries, in its order: the GOG fixture's games, a Steam game among them, and [`LIBRARY_ONLY`].
fn library_items() -> Vec<Value> {
    let mut items: Vec<Value> = crate::fixture::Fixture::standard()
        .games
        .iter()
        .map(|game| json!({"platform_id": "gog", "external_id": game.id, "owned": true}))
        .collect();
    items.insert(1, json!({"platform_id": "steam", "external_id": "48151", "owned": true}));
    items.push(json!({"platform_id": "gog", "external_id": LIBRARY_ONLY.0, "owned": true}));
    items
}

fn answer_web(stream: std::net::TcpStream, kept: &Mutex<Vec<(String, bool)>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut signed = false;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("authorization") && value.trim() == "Bearer stand-in-access" {
                signed = true;
            }
        }
    }
    kept.lock().unwrap_or_else(|p| p.into_inner()).push((path.clone(), signed));
    let (route, query) = path.split_once('?').unwrap_or((&path, ""));
    let parts: Vec<&str> = route.trim_matches('/').split('/').collect();
    let named = |id: &str| -> Option<String> {
        let fixture = crate::fixture::Fixture::standard();
        fixture.games.into_iter().find(|game| game.id == id).map(|game| game.name)
    };
    let (status, body) = match parts.as_slice() {
        ["galaxy-library", "users", _, "releases"] if !signed => ("401 Unauthorized", json!({"error": "unauthorized"})),
        ["galaxy-library", "users", _, "releases"] => {
            let items = library_items();
            let page: usize = query.strip_prefix("page_token=page").and_then(|n| n.parse().ok()).unwrap_or(0);
            let these: Vec<Value> = items.iter().skip(page * 2).take(2).cloned().collect();
            let next = (page * 2 + 2 < items.len()).then(|| format!("page{}", page + 1));
            ("200 OK", json!({"total_count": items.len(), "limit": 2, "items": these, "next_page_token": next}))
        }
        ["gamesdb", "platforms", "gog", "external_releases", id] => match named(id) {
            Some(name) => ("200 OK", json!({"id": "1", "external_id": id, "title": {"*": name, "en-US": name}})),
            None => ("404 Not Found", json!({"error": "not found"})),
        },
        ["api", "products", id] => match named(id).or_else(|| (*id == LIBRARY_ONLY.0).then(|| LIBRARY_ONLY.1.into())) {
            Some(name) => ("200 OK", json!({"id": id.parse::<u64>().unwrap_or(0), "title": name})),
            None => ("404 Not Found", json!({"error": "not found"})),
        },
        _ => ("404 Not Found", json!({"error": "not found"})),
    };
    let body = body.to_string();
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}
