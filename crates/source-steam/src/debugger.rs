//! The stand-in Steam's CEF debugger, for the end-to-end tests and this crate's own (#171): what the real client's
//! debugger is to the Dashboard (`crate::cef`), on a port of its own on 127.0.0.1 that it writes down beside the
//! fake's `steam.exe` (`cef::STANDIN_PORT`), never 8080 or 8081. It lists one page, `SharedJSContext`, and answers a
//! script run there the way the real client would, by doing to the fake Steam's files what Steam would do:
//!
//! - Steam's own calls (`crate::client`): what each asks is read from the script's first line (`Call::of_script`),
//!   done to the fake (an install queued in its library, an uninstall, a download paused, resumed or cancelled at
//!   the bytes it has), and answered as the script answers, each call it names "gone through".
//! - The download overview (`crate::overview::SCRIPT`): the app downloading, at a made-up speed of [`SPEED`] bytes a
//!   second; else the app whose Workshop items are still coming in, as the real client named Garry's Mod (#188,
//!   "Starting"); else none.
//! - The console's command (`cef::console_script`), as `steam.exe +app_install` and `+app_uninstall` are taken.
//!
//! Anything else it answers as a script that threw. Each thing it is asked is said through `say`, which the stand-in
//! program writes to its log (`steam: CEF: ...`), so a test sees both ends.
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use model::Steer;

use crate::client::{Answer, Call, Step};
use crate::fixture::{self, Progress};
use crate::{cef, installs, overview};

/// The speed the stand-in says it downloads at, in bytes a second: 20 Mbps.
pub const SPEED: u64 = 2_500_000;

/// The stand-in debugger, serving on a thread of its own until it is stopped or dropped.
pub struct Debugger {
    port: u16,
    root: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Debugger {
    /// Opens the stand-in's debugger for the fake Steam at `root` on a free port of 127.0.0.1, and writes the port
    /// down beside its `steam.exe`.
    pub fn start(root: &Path, say: impl Fn(&str) + Send + 'static) -> io::Result<Debugger> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        std::fs::write(root.join(cef::STANDIN_PORT), format!("{port}\n"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let (served, stopped) = (root.to_path_buf(), stop.clone());
        let thread = std::thread::spawn(move || serve(&listener, &served, port, &stopped, &say));
        Ok(Debugger { port, root: root.to_path_buf(), stop, thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Debugger {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(self.root.join(cef::STANDIN_PORT));
    }
}

fn serve(listener: &TcpListener, root: &Path, port: u16, stop: &AtomicBool, say: &dyn Fn(&str)) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(why) = connection(stream, root, port, say) {
                    say(&format!("CEF: a connection failed: {why}"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => {
                say(&format!("CEF: stopped listening: {e}"));
                return;
            }
        }
    }
}

/// One connection: the list of pages, or the page's WebSocket and one script run in it.
fn connection(stream: TcpStream, root: &Path, port: u16, say: &dyn Fn(&str)) -> Result<(), String> {
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let request = cef::headers(&mut reader)?;
    let line = request.first().map_or("", String::as_str);
    if line.starts_with("GET /json") {
        let pages = serde_json::json!([
            { "title": "Steam", "type": "page", "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/MAIN") },
            { "title": cef::SHARED, "type": "page", "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/SHARED") },
        ])
        .to_string();
        let answer = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{pages}",
            pages.len()
        );
        return writer.write_all(answer.as_bytes()).map_err(|e| e.to_string());
    }
    if !line.starts_with("GET /devtools/page/SHARED ") {
        let _ = writer.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return Err(format!("asked {line:?}"));
    }
    writer
        .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")
        .map_err(|e| e.to_string())?;
    let message: serde_json::Value =
        serde_json::from_str(&cef::message_text(&mut reader)?).map_err(|e| format!("not JSON: {e}"))?;
    let id = message.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let expression = message["params"]["expression"].as_str().unwrap_or_default();
    let result = match evaluate(root, expression, say) {
        Ok(value) => serde_json::json!({ "result": { "type": "string", "value": value } }),
        Err(thrown) => serde_json::json!({ "result": {}, "exceptionDetails": {
            "text": "Uncaught", "exception": { "description": thrown } } }),
    };
    let reply = serde_json::json!({ "id": id, "result": result }).to_string();
    writer.write_all(&cef::server_frame(reply.as_bytes())).map_err(|e| e.to_string())
}

/// What a script run in the stand-in's page comes to: its value, or what it threw.
fn evaluate(root: &Path, expression: &str, say: &dyn Fn(&str)) -> Result<String, String> {
    if let Some(call) = Call::of_script(expression) {
        let answer = act(root, &call, say);
        return Ok(serde_json::to_string(&answer).unwrap_or_default());
    }
    if expression == overview::SCRIPT {
        return Ok(overview_of(root).to_string());
    }
    if let Some(command) = console_command(expression) {
        say(&format!("CEF: asked through the console: {command}"));
        let (verb, appid) = command.split_once(' ').ok_or("no appid")?;
        let appid: u32 = appid.parse().map_err(|_| "no appid")?;
        let progress = match verb {
            "app_install" => Progress::Queued,
            "app_uninstall" => Progress::Dropped,
            _ => return Err(format!("the stand-in Steam's console knows no {verb}")),
        };
        fixture::write_progress(root, appid, progress).map_err(|e| e.to_string())?;
        return Ok("asked".to_string());
    }
    say(&format!("CEF: a script the stand-in does not know: {:.80}", expression.replace('\n', " ")));
    Err("ReferenceError: the stand-in Steam does not know this script".to_string())
}

/// `app_install 620` from `SteamClient.Console.ExecCommand('app_install 620'); 'asked'`.
fn console_command(expression: &str) -> Option<&str> {
    let rest = expression.strip_prefix("SteamClient.Console.ExecCommand('")?;
    rest.strip_suffix("'); 'asked'")
}

/// Does what `call` asks to the fake at `root`, and answers as the real script would: each of its calls gone
/// through, and a last step that says what the stand-in did; or, where it could not, that step failed.
fn act(root: &Path, call: &Call, say: &dyn Fn(&str)) -> Answer {
    let appid = call.appid();
    let done = match call {
        Call::Install { folder, .. } => {
            let libraries: Vec<PathBuf> =
                crate::libraries(root).iter().filter_map(|dir| dir.parent().map(Path::to_path_buf)).collect();
            let chosen = match folder {
                Some(folder) => libraries.iter().find(|library| same(library, Path::new(folder))).cloned(),
                None => libraries.last().cloned(),
            };
            match chosen {
                Some(library) => fixture::write_progress(root, appid, Progress::Queued)
                    .map(|()| format!("{appid} queued in {}", library.display())),
                None => Err(io::Error::other(format!("no library at {}", folder.as_deref().unwrap_or("")))),
            }
        }
        Call::Uninstall { .. } => fixture::write_progress(root, appid, Progress::Dropped)
            .and_then(|()| fixture::remove_workshop(root, appid))
            .map(|()| format!("{appid} uninstalled")),
        Call::Steer { steer, .. } => fixture::steered(root, appid, *steer).and_then(|progress| {
            fixture::write_progress(root, appid, progress)?;
            if *steer == Steer::Cancel {
                fixture::remove_workshop(root, appid)?;
            }
            Ok(format!("{appid} is {progress:?}"))
        }),
    };
    say(&format!(
        "CEF: asked to {}: {}",
        call.what(),
        match &done {
            Ok(did) => did.clone(),
            Err(e) => format!("not done: {e}"),
        }
    ));
    let mut steps: Vec<Step> = Vec::new();
    if done.is_ok() {
        steps.extend(call.calls().into_iter().map(|call| Step { call, ok: true, said: "undefined".into() }));
    }
    let (ok, said) = match done {
        Ok(did) => (true, did),
        Err(e) => (false, e.to_string()),
    };
    steps.push(Step { call: "the stand-in Steam".into(), ok, said });
    Answer { done: ok, steps }
}

/// Whether two paths name the same folder, as Windows compares them.
fn same(a: &Path, b: &Path) -> bool {
    let norm = |path: &Path| path.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

/// The download overview as the fake's files have it.
fn overview_of(root: &Path) -> serde_json::Value {
    let dirs = crate::libraries(root);
    let found = installs::under_way(&dirs, &|_| false);
    // The bytes named plainly, as older clients gave them: the fake's client writes nothing, and a count of its
    // writes would count its log.
    if let Some((appid, _, counts)) = found.iter().find(|(_, state, _)| state.downloading()) {
        return serde_json::json!({ "update_appid": appid, "update_state": "Downloading",
            "update_network_bytes_per_second": SPEED, "update_bytes_downloaded": counts.downloaded,
            "update_bytes_to_download": counts.to_download, "progress": [] });
    }
    let fetching = installs::manifests(&root.join("steamapps"))
        .into_iter()
        .map(|(appid, _)| appid)
        .find(|&appid| installs::workshop_pending(&dirs, appid));
    match fetching {
        Some(appid) => serde_json::json!({ "update_appid": appid, "update_state": "Starting",
            "update_network_bytes_per_second": 0, "progress": [] }),
        None => serde_json::json!({ "update_appid": 0, "update_state": "None", "progress": [] }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{self, Outcome};

    #[test]
    fn the_stand_in_debugger_takes_steams_own_calls_and_does_them_to_the_fake() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        fixture::write(&root, &fixture::Fixture::standard()).unwrap();
        std::fs::write(root.join(fixture::STANDIN_CLIENT), "").unwrap();
        let said = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let kept = said.clone();
        let debugger = Debugger::start(&root, move |line| kept.lock().unwrap().push(line.to_string())).unwrap();
        let ports = cef::ports(&root);
        assert_eq!(ports, [debugger.port()]);
        let quiet = 9_100_040;
        let state = || installs::under_way(&crate::libraries(&root), &|_| false).first().map(|found| found.1);

        let install = Call::Install { appid: quiet, folder: Some(root.display().to_string()) };
        assert_eq!(client::ask(&ports, &install), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Queued));
        fixture::write_progress(&root, quiet, Progress::Downloading { done: 30, total: 100 }).unwrap();
        let overview = overview::read(&ports).unwrap();
        assert_eq!((overview.appid, overview.speed(quiet)), (quiet, Some(SPEED * 8)));
        let steer = |steer| Call::Steer { appid: quiet, steer };
        assert_eq!(client::ask(&ports, &steer(Steer::Pause)), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Paused { done: 30, total: 100 }));
        assert!(!overview::read(&ports).unwrap().working_on(quiet));
        assert_eq!(client::ask(&ports, &steer(Steer::Resume)), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Downloading { done: 30, total: 100 }));
        assert_eq!(client::ask(&ports, &steer(Steer::Cancel)), Outcome::Took);
        assert_eq!(state(), None);
        // Nothing under way: nothing to pause, and the outcome says so.
        assert!(matches!(client::ask(&ports, &steer(Steer::Pause)), Outcome::NotTaken(_)));
        // A library the fake has not.
        let elsewhere = Call::Install { appid: quiet, folder: Some(r"Z:\Nowhere".into()) };
        assert!(matches!(client::ask(&ports, &elsewhere), Outcome::NotTaken(why) if why.contains("no library")));

        // Installed, its Workshop items coming in: the overview names it, as the real one named Garry's Mod.
        fixture::write_workshop(&root, quiet, 1, 3).unwrap();
        fixture::write_progress(&root, quiet, Progress::Done).unwrap();
        let overview = overview::read(&ports).unwrap();
        assert!(overview.working_on(quiet) && overview.state == "Starting");
        assert_eq!(client::ask(&ports, &Call::Uninstall { appid: quiet }), Outcome::Took);
        assert!(installs::workshop(&crate::libraries(&root), quiet).is_none());
        assert!(!crate::load(&root).unwrap().games.iter().any(|g| g.source_id == quiet.to_string() && g.installed));

        // The console, as before #171; and a script it does not know.
        let console = cef::evaluate(&ports, cef::SHARED, &cef::console_script(&format!("app_install {quiet}")));
        assert_eq!(console, Ok(cef::Evaluated::Value("asked".into())));
        assert_eq!(state(), Some(model::InstallState::Queued));
        assert!(matches!(cef::evaluate(&ports, cef::SHARED, "1 + 1"), Ok(cef::Evaluated::Threw(_))));

        drop(debugger);
        assert!(cef::ports(&root).is_empty(), "the port is no longer written down");
        let said = said.lock().unwrap();
        assert!(said.iter().any(|line| line == &format!("CEF: asked to pause {quiet}: {quiet} is Paused {{ done: 30, total: 100 }}")), "{said:#?}");
    }
}
