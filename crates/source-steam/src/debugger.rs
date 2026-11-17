//! The stand-in Steam's CEF debugger, for the end-to-end tests and this crate's own (#171): what the real client's
//! debugger is to the Dashboard (`crate::cef`), on a port of its own on 127.0.0.1 that it writes down beside the
//! fake's `steam.exe` (`cef::STANDIN_PORT`), never 8080 or 8081. It lists one page, `SharedJSContext`, and answers a
//! script run there the way the real client would, by doing to the fake Steam's files what Steam would do:
//!
//! - Steam's own calls (`crate::client`): what each asks is read from the script's first line (`Call::of_script`),
//!   done to the fake (an install queued in its library, an uninstall, a download paused, resumed or cancelled at
//!   the bytes it has), and answered as the script answers, each call it names "gone through". As the real client
//!   does (seen on the TV PC, 2026-11-17, [`takes`]), a call the script makes without the client id `'0'`, or an
//!   uninstall without its three arguments, returns `undefined` and does nothing: the install's wait on Steam's
//!   install manager then runs out at `eInstallState` 0, and it is not taken; an uninstall or a download's call
//!   "goes through" with nothing done, which only the app's manifest tells. Taken, the install manager goes 0, 7 on
//!   the wizard, 14 on `ContinueInstall`, and Steam's main window comes up, as the real one's does.
//! - Steam's main window (`client::Window`), on a page of its own titled "Steam": how it stands, and hiding or
//!   minimizing it.
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
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use model::Steer;

use crate::client::{self, Answer, Call, Shown, Step, Window};
use crate::fixture::{self, Progress};
use crate::{cef, installs, overview};

/// The speed the stand-in says it downloads at, in bytes a second: 20 Mbps.
pub const SPEED: u64 = 2_500_000;

/// What the stand-in client keeps between scripts: how its main window stands.
#[derive(Debug)]
pub struct Client {
    pub main: Shown,
}

/// The stand-in debugger, serving on a thread of its own until it is stopped or dropped.
pub struct Debugger {
    port: u16,
    root: PathBuf,
    client: Arc<Mutex<Client>>,
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
        let client = Arc::new(Mutex::new(Client { main: Shown::Hidden }));
        let (served, stopped, kept) = (root.to_path_buf(), stop.clone(), client.clone());
        let thread = std::thread::spawn(move || serve(&listener, &served, &kept, port, &stopped, &say));
        Ok(Debugger { port, root: root.to_path_buf(), client, stop, thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// How the stand-in client's main window stands.
    pub fn main_window(&self) -> Shown {
        self.client.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).main
    }

    /// Puts the stand-in client's main window as the user might have left it.
    pub fn set_main_window(&self, shown: Shown) {
        self.client.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).main = shown;
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

fn serve(
    listener: &TcpListener,
    root: &Path,
    client: &Mutex<Client>,
    port: u16,
    stop: &AtomicBool,
    say: &dyn Fn(&str),
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(why) = connection(stream, root, client, port, say) {
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

/// One connection: the list of pages, or a page's WebSocket and one script run in it.
fn connection(
    stream: TcpStream,
    root: &Path,
    client: &Mutex<Client>,
    port: u16,
    say: &dyn Fn(&str),
) -> Result<(), String> {
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let request = cef::headers(&mut reader)?;
    let line = request.first().map_or("", String::as_str);
    if line.starts_with("GET /json") {
        let pages = serde_json::json!([
            { "title": client::MAIN, "type": "page", "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/MAIN") },
            { "title": cef::SHARED, "type": "page", "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/SHARED") },
        ])
        .to_string();
        let answer = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{pages}",
            pages.len()
        );
        return writer.write_all(answer.as_bytes()).map_err(|e| e.to_string());
    }
    let main = line.starts_with("GET /devtools/page/MAIN ");
    if !main && !line.starts_with("GET /devtools/page/SHARED ") {
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
    let mut client = client.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let evaluated =
        if main { main_window(&mut client, expression, say) } else { evaluate(root, &mut client, expression, say) };
    drop(client);
    let result = match evaluated {
        Ok(value) => serde_json::json!({ "result": { "type": "string", "value": value } }),
        Err(thrown) => serde_json::json!({ "result": {}, "exceptionDetails": {
            "text": "Uncaught", "exception": { "description": thrown } } }),
    };
    let reply = serde_json::json!({ "id": id, "result": result }).to_string();
    writer.write_all(&cef::server_frame(reply.as_bytes())).map_err(|e| e.to_string())
}

/// What a script run in the page of the stand-in's main window comes to.
fn main_window(client: &mut Client, expression: &str, say: &dyn Fn(&str)) -> Result<String, String> {
    let Some(window) = Window::of_script(expression) else {
        say(&format!(
            "CEF: a script for the main window the stand-in does not know: {:.80}",
            expression.replace('\n', " ")
        ));
        return Err("ReferenceError: the stand-in Steam does not know this script".to_string());
    };
    let was = client.main;
    let value = match window {
        Window::Look => serde_json::json!({
            "visibility": if was == Shown::Showing { "visible" } else { "hidden" },
            "focused": was == Shown::Showing,
            "minimized": was == Shown::Minimized,
        })
        .to_string(),
        Window::Hide => {
            client.main = Shown::Hidden;
            "undefined".to_string()
        }
        Window::Minimize => {
            client.main = Shown::Minimized;
            "undefined".to_string()
        }
    };
    say(&format!("CEF: the main window, asked to {window:?}: it was {was:?}, it is {:?}", client.main));
    Ok(value)
}

/// What a script run in the stand-in's windowless page comes to: its value, or what it threw.
fn evaluate(root: &Path, client: &mut Client, expression: &str, say: &dyn Fn(&str)) -> Result<String, String> {
    if let Some(call) = Call::of_script(expression) {
        let answer = act(root, client, &call, expression, say);
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

/// The calls the real client takes for `call`, as a script makes them (seen on the TV PC, 2026-11-17: each with the
/// client id `'0'`, the uninstall with three arguments), written out here rather than taken from `crate::client`, so
/// that a script that strays from them is caught. The first one `script` does not make, if any.
pub fn takes(script: &str, call: &Call) -> Result<(), String> {
    let appid = call.appid();
    let wizard = format!("SteamClient.Installs.OpenInstallWizard([{appid}], '0')");
    let uninstall = format!("SteamClient.Installs.OpenUninstallWizard([{appid}], '0', true)");
    let pause = format!("SteamClient.Downloads.PauseAppUpdate({appid}, '0')");
    let needs = match call {
        Call::Install { .. } => vec![wizard, "SteamClient.Installs.ContinueInstall()".to_string()],
        Call::Uninstall { .. } => vec![uninstall],
        Call::Steer { steer: Steer::Pause, .. } => vec![pause],
        Call::Steer { steer: Steer::Resume, .. } => {
            vec![format!("SteamClient.Downloads.ResumeAppUpdate({appid}, '0')")]
        }
        // The real one's cancel is the pause, then these; the pause comes as a script of its own.
        Call::Steer { steer: Steer::Cancel, .. } => {
            vec![format!("SteamClient.Downloads.RemoveFromDownloadList({appid}, '0')"), uninstall]
        }
    };
    match needs.into_iter().find(|need| !script.contains(&format!("=> {need}"))) {
        Some(missing) => Err(missing),
        None => Ok(()),
    }
}

/// Does what `call` asks to the fake at `root`, and answers as the real script would: each of its calls gone
/// through, and a last step that says what the stand-in did; or, where it could not, that step failed. A script
/// that does not make the calls as the real client takes them ([`takes`]) is answered as the real client answers
/// it, and nothing is done.
fn act(root: &Path, client: &mut Client, call: &Call, script: &str, say: &dyn Fn(&str)) -> Answer {
    let appid = call.appid();
    if let Err(missing) = takes(script, call) {
        say(&format!(
            "CEF: asked to {}: not done: the script does not call {missing}, so Steam's client returns undefined",
            call.what()
        ));
        let gone = |call: String| Step { call, ok: true, said: "undefined".into() };
        return match call {
            // The wizard never opens: the install manager stays at 0, and the script's wait runs out.
            Call::Install { .. } => {
                let mut steps: Vec<Step> = call.calls().into_iter().take(2).map(gone).collect();
                steps.push(Step {
                    call: client::WIZARD_OPEN.into(),
                    ok: false,
                    said: format!(
                        "eInstallState still 0 after {} ms: Steam did not take it; {{\"eInstallState\":0}}",
                        client::WIZARD_WAIT.as_millis()
                    ),
                });
                Answer { done: false, steps }
            }
            _ => Answer { done: true, steps: call.calls().into_iter().map(gone).collect() },
        };
    }
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
            // Cancelled while its download still runs (not paused first and let stop), the files go but the
            // manifest is left behind at StateFlags 514, as the real Steam's was on the TV PC (2026-11-17).
            let file = root.join("steamapps").join(format!("appmanifest_{appid}.acf"));
            let running = installs::counts(&file).filter(|counts| counts.flags & installs::UPDATE_RUNNING != 0);
            let progress = match (steer, running) {
                (Steer::Cancel, Some(counts)) => {
                    Progress::Abandoned { done: counts.downloaded, total: counts.to_download }
                }
                _ => progress,
            };
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
        // The install manager as the real one went: 7 with the wizard open, 14 once the install goes on.
        let said = |name: &str| match name {
            client::WIZARD_OPEN => r#"{"eInstallState":7}"#.to_string(),
            client::INSTALL_GOING => r#"{"eInstallState":14}"#.to_string(),
            _ => "undefined".to_string(),
        };
        steps.extend(call.calls().into_iter().map(|call| Step { said: said(&call), call, ok: true }));
        // And the wizard brings the main window up.
        if matches!(call, Call::Install { .. }) {
            client.main = Shown::Showing;
        }
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
    use crate::client::Outcome;

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

        let ask = |call: &Call| client::ask(&ports, Some(&root), call);

        // As before the fix (#171, 2026-11-17): the wizard without the client id does nothing, and the install's
        // wait on Steam's install manager runs out at 0; the two-argument uninstall returns with nothing done.
        let install = Call::Install { appid: quiet, folder: Some(root.display().to_string()) };
        let old = install
            .script()
            .replace(&format!("OpenInstallWizard([{quiet}], '0')"), &format!("OpenInstallWizard([{quiet}])"));
        let answer = |script: &str| match cef::evaluate(&ports, cef::SHARED, script) {
            Ok(cef::Evaluated::Value(text)) => serde_json::from_str::<Answer>(&text).unwrap(),
            other => panic!("{other:?}"),
        };
        let wizard = answer(&old);
        assert!(!wizard.done && wizard.steps.last().is_some_and(|step| step.call == client::WIZARD_OPEN && !step.ok));
        assert_eq!(state(), None, "nothing queued");
        assert_eq!(debugger.main_window(), Shown::Hidden, "no wizard, no main window");

        // Steam's main window in the tray: the install puts it back there.
        assert_eq!(ask(&install), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Queued));
        assert_eq!(debugger.main_window(), Shown::Hidden);
        // Minimized, it is minimized again; showing, it is left showing.
        debugger.set_main_window(Shown::Minimized);
        assert_eq!(ask(&install), Outcome::Took);
        assert_eq!(debugger.main_window(), Shown::Minimized);
        debugger.set_main_window(Shown::Showing);
        assert_eq!(ask(&install), Outcome::Took);
        assert_eq!(debugger.main_window(), Shown::Showing);
        debugger.set_main_window(Shown::Hidden);

        fixture::write_progress(&root, quiet, Progress::Downloading { done: 30, total: 100 }).unwrap();
        let overview = overview::read(&ports).unwrap();
        assert_eq!((overview.appid, overview.speed(quiet)), (quiet, Some(SPEED * 8)));
        let steer = |steer| Call::Steer { appid: quiet, steer };
        assert_eq!(ask(&steer(Steer::Pause)), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Paused { done: 30, total: 100 }));
        assert!(!overview::read(&ports).unwrap().working_on(quiet));
        assert_eq!(ask(&steer(Steer::Resume)), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Downloading { done: 30, total: 100 }));
        // A resume without the client id: undefined, and the download stays where it was.
        fixture::write_progress(&root, quiet, Progress::Paused { done: 30, total: 100 }).unwrap();
        let resume = steer(Steer::Resume).script().replace("(9100040, '0')", "(9100040)");
        assert!(answer(&resume).done);
        assert_eq!(state(), Some(model::InstallState::Paused { done: 30, total: 100 }));
        assert_eq!(ask(&steer(Steer::Cancel)), Outcome::Took);
        assert_eq!(state(), None);
        let manifest = root.join("steamapps").join(format!("appmanifest_{quiet}.acf"));
        assert!(!manifest.exists(), "paused first and let stop: the manifest goes");

        // Cancelled while the download still runs, with no pause first (#171, the TV PC): the manifest is left at
        // 514. That reads as nothing under way and not installed, the cancel is said as what was seen, and Install
        // on it works.
        fixture::write_progress(&root, quiet, Progress::Downloading { done: 30, total: 100 }).unwrap();
        assert!(answer(&steer(Steer::Cancel).script()).done);
        assert_eq!(installs::counts(&manifest).map(|counts| counts.flags), Some(514));
        assert_eq!(state(), None, "not shown as a paused download");
        assert!(!crate::load(&root).unwrap().games.iter().any(|g| g.source_id == quiet.to_string() && g.installed));
        let (done, seen) = client::on_disk(&crate::libraries(&root), &steer(Steer::Cancel)).unwrap();
        assert!(done && seen.contains("left behind at StateFlags 514"), "{seen}");
        assert_eq!(ask(&install), Outcome::Took);
        assert_eq!(state(), Some(model::InstallState::Queued));
        fixture::write_progress(&root, quiet, Progress::Dropped).unwrap();
        // Nothing under way: nothing to pause, and the outcome says so.
        assert!(matches!(ask(&steer(Steer::Pause)), Outcome::NotTaken(_)));
        // A library the fake has not.
        let elsewhere = Call::Install { appid: quiet, folder: Some(r"Z:\Nowhere".into()) };
        assert!(matches!(ask(&elsewhere), Outcome::NotTaken(why) if why.contains("no library")));

        // Installed, its Workshop items coming in: the overview names it, as the real one named Garry's Mod.
        fixture::write_workshop(&root, quiet, 1, 3).unwrap();
        fixture::write_progress(&root, quiet, Progress::Done).unwrap();
        let overview = overview::read(&ports).unwrap();
        assert!(overview.working_on(quiet) && overview.state == "Starting");
        let uninstall = Call::Uninstall { appid: quiet };
        let two = uninstall.script().replace("'0', true", "true");
        assert!(answer(&two).done, "returned, as Steam's does");
        assert!(crate::load(&root).unwrap().games.iter().any(|g| g.source_id == quiet.to_string() && g.installed));
        assert_eq!(ask(&uninstall), Outcome::Took);
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
        assert!(
            said.iter()
                .any(|line| line
                    == &format!("CEF: asked to pause {quiet}: {quiet} is Paused {{ done: 30, total: 100 }}")),
            "{said:#?}"
        );
    }
}
