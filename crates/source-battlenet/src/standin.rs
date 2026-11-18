//! The stand-in Agent, for the tests (`standin.exe` copied as a fake's `Agent/Agent.<build>/Agent.exe`,
//! crates/standin; and in-process in the tests here): an HTTP server on a free port of 127.0.0.1, written down in the
//! fake's `Agent/Agent.dat` as the real one's is, answering the calls of [`crate::agent::calls`] as the real Agent is
//! reported to (docs/research/installs.md 7), and doing to the fake's files what the real one does to the machine's:
//! an install finished is a game folder and an installed product in `product.db`, `aggregate.json` and the registry
//! (the fake rewritten from its `fixture.json`), an uninstall takes them away. Nothing is downloaded: an install's
//! bytes are counted out a step at a time, `STANDIN_HELPER_STEP_MS` apart (600 ms without), [`STEPS`] steps of
//! [`STEP_BYTES`].
//!
//! Like the real one it answers nothing without the `phoenix-agent/1.0` user agent, and nothing but `GET /agent`
//! without the token that gave. It works only in a fake's folder: one with a `fixture.json`.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::agent::PORT_FILE;
use crate::fixture::{self, Fixture, FixtureProduct, Kind};
use crate::http::{mask, USER_AGENT};

pub const STEPS: u64 = 8;
pub const STEP_BYTES: u64 = 6 * 1_048_576;
/// What a whole install is.
pub const WHOLE: u64 = STEPS * STEP_BYTES;
/// The error the stand-in gives for a product the account does not own: barncastle's README's "product invalid or
/// unavailable".
pub const NOT_OWNED: i64 = 2221;

#[derive(Debug, Clone)]
struct Install {
    code: String,
    name: String,
    dir: Option<PathBuf>,
    done: u64,
    paused: bool,
    playable: bool,
}

struct State {
    root: PathBuf,
    token: String,
    installs: HashMap<String, Install>,
    say: Box<dyn Fn(&str) + Send>,
}

/// A stand-in Agent serving: its port, and its state for a test to look at.
pub struct Served {
    pub port: u16,
    state: Arc<Mutex<State>>,
}

impl Served {
    /// The uids it has under way.
    pub fn under_way(&self) -> Vec<String> {
        let mut uids: Vec<String> = lock(&self.state).installs.keys().cloned().collect();
        uids.sort();
        uids
    }
}

fn lock(state: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The fake's folder of a stand-in Agent running as `<root>/Agent/Agent.<build>/Agent.exe`.
pub fn root_of(exe: &Path) -> Option<PathBuf> {
    let root = exe.parent()?.parent()?.parent()?.to_path_buf();
    root.join(fixture::SPEC).is_file().then_some(root)
}

/// Serves the fake at `root` on a thread of its own, for as long as the program runs, each request said with
/// `say`. The step between an install's counts is `step`.
pub fn serve(root: &Path, step: Duration, say: impl Fn(&str) + Send + 'static) -> std::io::Result<Served> {
    if !root.join(fixture::SPEC).is_file() {
        return Err(std::io::Error::other(format!("{} is no fake Battle.net", root.display())));
    }
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let token = format!("standin-{}-{}", std::process::id(), port);
    let state =
        Arc::new(Mutex::new(State { root: root.to_path_buf(), token, installs: HashMap::new(), say: Box::new(say) }));
    std::fs::write(root.join("Agent").join(PORT_FILE), port.to_string())?;
    let served = state.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = served.clone();
            std::thread::spawn(move || {
                let _ = answer(stream, &state);
            });
        }
    });
    let ticking = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(step);
        tick(&ticking);
    });
    Ok(Served { port, state })
}

/// Runs as the stand-in Agent from `exe`, for `seconds`: what `standin.exe` does named `Agent.exe`.
pub fn run(exe: &Path, args: &[String], say: impl Fn(&str) + Send + Clone + 'static) -> i32 {
    let Some(root) = root_of(exe) else {
        say(&format!("battlenet agent: {} is not in a fake Battle.net's Agent folder: not serving", exe.display()));
        return 2;
    };
    let step = std::env::var("STANDIN_HELPER_STEP_MS").ok().and_then(|ms| ms.parse().ok()).unwrap_or(600);
    let seconds =
        args.windows(2).find(|pair| pair[0] == "--seconds").and_then(|pair| pair[1].parse().ok()).unwrap_or(1800);
    match serve(&root, Duration::from_millis(step), say.clone()) {
        Ok(served) => {
            say(&format!(
                "battlenet agent: serving {} on 127.0.0.1:{} ({})",
                root.display(),
                served.port,
                args.join(" ")
            ));
            std::thread::sleep(Duration::from_secs(seconds));
            0
        }
        Err(e) => {
            say(&format!("battlenet agent: {e}"));
            1
        }
    }
}

/// Moves every install that is not paused on a step; one done is installed in the fake.
fn tick(state: &Mutex<State>) {
    let mut state = lock(state);
    let mut finished = Vec::new();
    for (uid, install) in state.installs.iter_mut() {
        if install.paused || install.playable || install.dir.is_none() {
            continue;
        }
        install.done = (install.done + STEP_BYTES).min(WHOLE);
        if install.done >= WHOLE {
            install.playable = true;
            finished.push(uid.clone());
        }
    }
    for uid in finished {
        let install = state.installs[&uid].clone();
        let result = change(&state.root, |spec| {
            spec.products.retain(|p| p.code != install.code);
            spec.products.push(FixtureProduct {
                uid: uid.clone(),
                code: install.code.clone(),
                name: install.name.clone(),
                kind: Kind::Installed,
                dir: install.dir.clone(),
            });
        });
        let _ = std::fs::remove_file(install.dir.clone().unwrap_or_default().join("standin-partial.txt"));
        (state.say)(&match result {
            Ok(()) => format!("battlenet agent: {uid} installed in {}", install.dir.unwrap_or_default().display()),
            Err(e) => format!("battlenet agent: {uid} could not be written as installed: {e}"),
        });
    }
}

/// The fake's spec changed by `how` and written again, the Agent's own program and every game's left in place.
fn change(root: &Path, how: impl FnOnce(&mut Fixture)) -> std::io::Result<()> {
    let mut spec = Fixture::load(root)?;
    how(&mut spec);
    fixture::write(root, &spec)
}

struct Request {
    method: String,
    path: String,
    user_agent: Option<String>,
    authorization: Option<String>,
    body: Value,
}

fn read(stream: &TcpStream) -> std::io::Result<Request> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let (mut user_agent, mut authorization, mut length) = (None, None, 0usize);
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else { continue };
        let value = value.trim().to_string();
        match name.trim().to_ascii_lowercase().as_str() {
            "user-agent" => user_agent = Some(value),
            "authorization" => authorization = Some(value),
            "content-length" => length = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    let mut body = vec![0; length.min(1 << 20)];
    reader.read_exact(&mut body)?;
    let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    Ok(Request { method, path, user_agent, authorization, body })
}

fn answer(mut stream: TcpStream, state: &Mutex<State>) -> std::io::Result<()> {
    let request = read(&stream)?;
    let (status, body) = route(state, &request);
    let text = body.to_string();
    {
        let state = lock(state);
        let sent = if request.body.is_null() { String::new() } else { format!(" {}", request.body) };
        (state.say)(&mask(
            &format!("battlenet agent: {} {}{sent} -> {status} {text}", request.method, request.path),
            Some(&state.token),
        ));
    }
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Bad Request",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        text.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(text.as_bytes())?;
    stream.flush()
}

fn route(state: &Mutex<State>, request: &Request) -> (u16, Value) {
    let mut state = lock(state);
    if request.user_agent.as_deref() != Some(USER_AGENT) {
        return (401, json!({ "error": "unknown client" }));
    }
    let path = request.path.split('?').next().unwrap_or("");
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    if let (["agent"], "GET") = (&parts[..], request.method.as_str()) {
        return (
            200,
            json!({ "pid": std::process::id(), "state": 1004, "version": "standin", "region": "eu", "authorization": state.token }),
        );
    }
    if request.authorization.as_deref() != Some(state.token.as_str()) {
        return (401, json!({ "error": "no session" }));
    }
    let catalogue: Vec<fixture::Available> = Fixture::load(&state.root).map(|spec| spec.available).unwrap_or_default();
    let uid_of = |body: &Value| body.get("uid").and_then(Value::as_str).unwrap_or("").to_string();
    match (request.method.as_str(), &parts[..]) {
        ("POST", ["install"]) => {
            let uid = uid_of(&request.body);
            let Some(product) = catalogue.iter().find(|a| a.uid == uid) else {
                return (404, json!({ "error": 2300, "uid": uid }));
            };
            if !product.owned {
                return (200, json!({ "error": NOT_OWNED, "uid": uid }));
            }
            let patch = request.body.get("instructions_patch_url").and_then(Value::as_str).unwrap_or("");
            if !patch.ends_with(&format!("/{}", product.code)) {
                return (200, json!({ "error": 2301, "uid": uid, "why": "patch url names another product" }));
            }
            state.installs.entry(uid.clone()).or_insert(Install {
                code: product.code.clone(),
                name: product.name.clone(),
                dir: None,
                done: 0,
                paused: false,
                playable: false,
            });
            (200, json!({ "response_uri": format!("/install/{uid}") }))
        }
        ("POST", ["install", uid]) => {
            let Some(install) = state.installs.get_mut(*uid) else { return (404, json!({ "error": 2300 })) };
            if let Some(paused) = request.body.get("paused").and_then(Value::as_bool) {
                install.paused = paused;
                return (200, json!({ "paused": paused }));
            }
            let Some(dir) = request.body.get("game_dir").and_then(Value::as_str).filter(|d| !d.is_empty()) else {
                return (200, json!({ "error": 2302, "why": "no game_dir" }));
            };
            // The folder as the Agent is given it, with a backslash at its end (`calls::install_options`).
            let dir = PathBuf::from(dir.trim_end_matches(['\\', '/']));
            if std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join("standin-partial.txt"), "")).is_err()
            {
                return (200, json!({ "error": 2303, "why": "game_dir could not be made" }));
            }
            install.dir = Some(dir);
            (200, json!({ "response_uri": format!("/install/{uid}") }))
        }
        ("GET", ["install", uid]) => match state.installs.get(*uid) {
            Some(install) => {
                let rate = if install.paused || install.playable || install.dir.is_none() { 0 } else { STEP_BYTES };
                (
                    200,
                    json!({
                        "progress": install.done as f64 / WHOLE as f64,
                        "download_rate": rate as f64,
                        "download_current": [install.done, 0, 0],
                        "download_total": [WHOLE, 0, 0],
                        "download_remaining": WHOLE - install.done,
                        "download_complete": install.playable,
                        "paused": install.paused,
                        "playable": install.playable,
                        "state": if install.playable { 1004 } else { 1003 },
                    }),
                )
            }
            None => (404, json!({ "error": 2300 })),
        },
        ("DELETE", ["install", uid]) => match state.installs.remove(*uid) {
            Some(install) => {
                if let Some(dir) = install.dir.filter(|_| !install.playable) {
                    let _ = std::fs::remove_dir_all(dir);
                }
                (200, json!({}))
            }
            None => (404, json!({ "error": 2300 })),
        },
        ("POST", ["uninstall"]) => {
            let uid = uid_of(&request.body);
            let spec = Fixture::load(&state.root).ok();
            let product =
                spec.as_ref().and_then(|spec| spec.products.iter().find(|p| p.uid == uid && p.kind != Kind::Client));
            let Some(product) = product.cloned() else { return (404, json!({ "error": 2300, "uid": uid })) };
            let dir = fixture::folder(&state.root, &product);
            let _ = std::fs::remove_dir_all(&dir);
            state.installs.remove(&uid);
            if dir.exists() {
                return (200, json!({ "error": 2305, "why": format!("{} could not be deleted", dir.display()) }));
            }
            match change(&state.root, |spec| spec.products.retain(|p| p.uid != uid)) {
                Ok(()) => (200, json!({ "response_uri": format!("/uninstall/{uid}") })),
                Err(e) => (200, json!({ "error": 2304, "why": e.to_string() })),
            }
        }
        ("POST", ["update"]) => match state.installs.get(&uid_of(&request.body)) {
            Some(_) => (200, json!({ "response_uri": format!("/update/{}", uid_of(&request.body)) })),
            None => (404, json!({ "error": 2300 })),
        },
        ("DELETE", ["agent"]) => (200, json!({})),
        _ => (404, json!({ "error": "unknown call" })),
    }
}
