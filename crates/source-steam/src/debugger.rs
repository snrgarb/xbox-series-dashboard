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
//!   "goes through" with nothing done, which only the app's manifest tells.
//! - Steam's install wizard (`client::Wizard`, #230), a step a script, its install manager moving as the real one
//!   was seen to: 0; 5 (`WaitAppInfo`) at the first look after `OpenInstallWizard`, then 7 (`ShowConfig`), or 4
//!   (`ShowCDKey`) for an app set up to ask for a key; on `ContinueInstall` 8 (`ShowEULAs`) for an app with licence
//!   agreements, else 14 and the app queued in the library chosen; at 8, `LoadEula(appid)` gives the app's
//!   agreements as `[{id, url, version}]`, each url a page this debugger serves (`/eula/<appid>/<n>`), and
//!   `ContinueInstall` leaves 8 only once `MarkEulaAccepted(appid, id, version)` has been called for each of them
//!   with the id and version given; `CancelInstall` takes it back to 0 from anywhere. `GetInstallManagerInfo` says
//!   the state, what the app needs (`fixture::needed`) and what is free (`fixture::WizardSetUp`, which `standin
//!   steam-wizard` writes). `OpenInstallWizard` brings Steam's main window up, as the real one's does.
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

use crate::client::{self, Answer, Call, Shown, Step, Window, Wizard};
use crate::fixture::{self, Progress, WizardSetUp};
use crate::{cef, installs, overview};

/// The speed the stand-in says it downloads at, in bytes a second: 20 Mbps.
pub const SPEED: u64 = 2_500_000;

/// What the stand-in client keeps between scripts: how its main window stands, and its install manager.
#[derive(Debug)]
pub struct Client {
    pub main: Shown,
    pub manager: Manager,
}

/// The stand-in's install manager (#230).
#[derive(Debug, Default)]
pub struct Manager {
    /// `eInstallState`, as the real one's numbers go.
    pub state: i64,
    /// The install its wizard is open on.
    pub open: Option<Opened>,
    /// Looks left at 5 before it moves on.
    pub waiting: u8,
}

/// An install whose wizard is open.
#[derive(Debug, Clone)]
pub struct Opened {
    pub appid: u32,
    /// As the Dashboard asked it, which the stand-in's log line names (`Call::what`).
    pub folder: Option<String>,
    pub library: PathBuf,
    /// The agreements marked accepted, by id.
    pub accepted: Vec<String>,
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
        let client = Arc::new(Mutex::new(Client { main: Shown::Hidden, manager: Manager::default() }));
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

    /// The install manager's `eInstallState`.
    pub fn install_state(&self) -> i64 {
        self.client.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).manager.state
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
    if let Some(path) = line.strip_prefix("GET /eula/") {
        let path = path.split([' ', '?']).next().unwrap_or_default();
        let page = agreement_page(root, path);
        say(&format!("CEF: served the licence agreement page /eula/{path}"));
        let (status, page) = match page {
            Some(page) => ("200 OK", page),
            None => ("404 Not Found", String::new()),
        };
        let answer = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        return writer.write_all(answer.as_bytes()).map_err(|e| e.to_string());
    }
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
    let evaluated = if main {
        main_window(&mut client, expression, say)
    } else {
        evaluate(root, &mut client, port, expression, say)
    };
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
fn evaluate(
    root: &Path,
    client: &mut Client,
    port: u16,
    expression: &str,
    say: &dyn Fn(&str),
) -> Result<String, String> {
    if let Some(step) = Wizard::of_script(expression) {
        let answer = wizard(root, client, port, &step, expression, say);
        return Ok(serde_json::to_string(&answer).unwrap_or_default());
    }
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
        // An install comes a step at a time (`wizard_takes`).
        Call::Install { .. } => return Err(wizard),
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
fn act(root: &Path, _client: &mut Client, call: &Call, script: &str, say: &dyn Fn(&str)) -> Answer {
    let appid = call.appid();
    if let Err(missing) = takes(script, call) {
        say(&format!(
            "CEF: asked to {}: not done: the script does not call {missing}, so Steam's client returns undefined",
            call.what()
        ));
        let gone = |call: String| Step { call, ok: true, said: "undefined".into() };
        return Answer { done: true, steps: call.calls().into_iter().map(gone).collect(), value: None };
    }
    let done = match call {
        Call::Install { .. } => Err(io::Error::other("an install comes a step at a time, as Steam's wizard goes")),
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
        steps.extend(call.calls().into_iter().map(|call| Step { said: "undefined".into(), call, ok: true }));
    }
    let (ok, said) = match done {
        Ok(did) => (true, did),
        Err(e) => (false, e.to_string()),
    };
    steps.push(Step { call: "the stand-in Steam".into(), ok, said });
    Answer { done: ok, steps, value: None }
}

/// The calls the real client takes for a step of its install wizard, written out as `takes` writes them: the
/// first `script` does not make, if any.
pub fn wizard_takes(script: &str, step: &Wizard) -> Result<(), String> {
    let need = match step {
        Wizard::Open { appid, .. } => format!("SteamClient.Installs.OpenInstallWizard([{appid}], '0')"),
        Wizard::Look => "SteamClient.Installs.GetInstallManagerInfo()".into(),
        Wizard::Prepare { .. } => "SteamClient.Installs.SetCreateShortcuts(false, false)".into(),
        Wizard::Continue => "SteamClient.Installs.ContinueInstall()".into(),
        Wizard::Agreements { appid } => format!("SteamClient.Apps.LoadEula({appid})"),
        Wizard::Accept { appid, id, version } => format!("SteamClient.Apps.MarkEulaAccepted({appid}, {id}, {version})"),
        Wizard::Cancel => "SteamClient.Installs.CancelInstall()".into(),
    };
    if script.contains(&format!("=> {need})")) {
        Ok(())
    } else {
        Err(need)
    }
}

/// How many looks at 5 (`WaitAppInfo`) the stand-in's manager gives after `OpenInstallWizard` before it moves on.
const WAIT_APP_INFO: u8 = 1;

/// The id the stand-in gives an app's `n`th agreement (from 0), and its version.
fn agreement_id(appid: u32, n: u32) -> String {
    format!("{appid}_eula_{n}")
}
const AGREEMENT_VERSION: u64 = 3;

/// Does a step of the install wizard to the stand-in's manager, and answers as the real script would: each call
/// gone through, what the step gives as its value.
fn wizard(root: &Path, client: &mut Client, port: u16, step: &Wizard, script: &str, say: &dyn Fn(&str)) -> Answer {
    let gone = |call: String, said: &str| Step { call, ok: true, said: said.to_string() };
    if let Err(missing) = wizard_takes(script, step) {
        say(&format!(
            "CEF: install wizard: {step:?}: not done: the script does not call {missing}, so Steam's client returns undefined"
        ));
        return Answer {
            done: true,
            steps: step.calls().into_iter().map(|c| gone(c, "undefined")).collect(),
            value: None,
        };
    }
    let set_up = WizardSetUp::read(root);
    let manager = &mut client.manager;
    let was = manager.state;
    let mut value = serde_json::Value::Null;
    match step {
        Wizard::Open { appid, folder } => {
            let libraries: Vec<PathBuf> =
                crate::libraries(root).iter().filter_map(|dir| dir.parent().map(Path::to_path_buf)).collect();
            let chosen = match folder {
                Some(folder) => libraries.iter().position(|library| same(library, Path::new(folder))),
                None => libraries.len().checked_sub(1),
            };
            let Some(index) = chosen else {
                say(&format!("CEF: install wizard: no library at {}", folder.as_deref().unwrap_or("")));
                let mut steps = vec![gone(step.calls()[0].clone(), "[]")];
                steps.push(Step {
                    call: "choose the library".into(),
                    ok: false,
                    said: format!("Steam has no library at {}", folder.as_deref().unwrap_or("")),
                });
                return Answer { done: false, steps, value: None };
            };
            let library = libraries[index].clone();
            value = serde_json::json!({ "index": index, "path": library.display().to_string() });
            *manager = Manager {
                state: 5,
                open: Some(Opened { appid: *appid, folder: folder.clone(), library, accepted: Vec::new() }),
                waiting: WAIT_APP_INFO,
            };
            // The wizard brings the main window up.
            client.main = Shown::Showing;
        }
        Wizard::Look => {
            if manager.state == 5 {
                if manager.waiting > 0 {
                    manager.waiting -= 1;
                } else {
                    let key = manager.open.as_ref().is_some_and(|open| set_up.keys.contains(&open.appid));
                    manager.state = if key { 4 } else { 7 };
                }
            }
            let needed = manager.open.as_ref().map_or(0, |open| fixture::needed(open.appid));
            let free = if manager.open.is_some() { set_up.free() } else { 0 };
            value = serde_json::json!({ "eInstallState": manager.state, "nDiskSpaceRequired": needed,
                "nDiskSpaceAvailable": free });
        }
        Wizard::Prepare { .. } => {}
        Wizard::Continue => {
            let open = manager.open.clone();
            let count = |appid: u32| set_up.agreements.get(&appid).copied().unwrap_or(0);
            let all_accepted =
                |open: &Opened| (0..count(open.appid)).all(|n| open.accepted.contains(&agreement_id(open.appid, n)));
            match (manager.state, open) {
                (7, Some(open)) if count(open.appid) > 0 && !all_accepted(&open) => manager.state = 8,
                (7 | 8, Some(open)) if all_accepted(&open) => {
                    manager.state = 14;
                    let what = Call::Install { appid: open.appid, folder: open.folder.clone() }.what();
                    match fixture::write_progress(root, open.appid, Progress::Queued) {
                        Ok(()) => {
                            say(&format!("CEF: asked to {what}: {} queued in {}", open.appid, open.library.display()))
                        }
                        Err(e) => say(&format!("CEF: asked to {what}: not done: {e}")),
                    }
                }
                _ => {}
            }
        }
        Wizard::Agreements { appid } => {
            let count = set_up.agreements.get(appid).copied().unwrap_or(0);
            let shown = manager.state == 8 && manager.open.as_ref().is_some_and(|open| open.appid == *appid);
            let list: Vec<serde_json::Value> = (0..if shown { count } else { 0 })
                .map(|n| {
                    serde_json::json!({ "id": agreement_id(*appid, n),
                        "url": format!("http://127.0.0.1:{port}/eula/{appid}/{n}?l=english&cc=GB&client=1"),
                        "version": AGREEMENT_VERSION })
                })
                .collect();
            value = serde_json::Value::Array(list);
        }
        Wizard::Accept { appid, id, version } => {
            let known = (0..set_up.agreements.get(appid).copied().unwrap_or(0))
                .any(|n| id.as_str() == Some(agreement_id(*appid, n).as_str()))
                && version.as_u64() == Some(AGREEMENT_VERSION);
            match manager.open.as_mut().filter(|open| open.appid == *appid && known) {
                Some(open) => {
                    open.accepted.push(id.as_str().unwrap_or_default().to_string());
                    say(&format!("CEF: licence agreement {id} version {version} of {appid} marked accepted"));
                }
                None => say(&format!("CEF: licence agreement {id} version {version} of {appid}: not one it gave")),
            }
        }
        Wizard::Cancel => {
            *manager = Manager::default();
            say(&format!("CEF: install wizard cancelled: eInstallState {was} -> 0"));
        }
    }
    if manager.state != was && !matches!(step, Wizard::Cancel) {
        say(&format!("CEF: install wizard: eInstallState {was} -> {}", manager.state));
    }
    let said = |value: &serde_json::Value| if value.is_null() { "undefined".to_string() } else { value.to_string() };
    let mut steps: Vec<Step> = step.calls().into_iter().map(|call| gone(call, "undefined")).collect();
    if let Some(last) = steps.last_mut() {
        last.said = said(&value);
    }
    Answer { done: true, steps, value: (!value.is_null()).then_some(value) }
}

/// The stand-in's page for an agreement, `<appid>/<n>`: a whole page as a store's is, its frame, scripts and styles
/// round it, and enough paragraphs to scroll through. None for one no app of the fake's set-up has.
fn agreement_page(root: &Path, path: &str) -> Option<String> {
    let (appid, n) = path.split_once('/')?;
    let (appid, n): (u32, u32) = (appid.parse().ok()?, n.parse().ok()?);
    let count = WizardSetUp::read(root).agreements.get(&appid).copied().unwrap_or(0);
    if n >= count {
        return None;
    }
    let mut body = format!(
        "<h1>Stand-in Licence Agreement {} of {count}</h1>\n<p>For app {appid}, version {AGREEMENT_VERSION}. \
         This is the stand-in Steam&#39;s own made-up text: it binds nobody to anything.</p>\n",
        n + 1
    );
    // Long enough to be several pages of the prompt at any screen size: some 20 of its boxes at 1080p.
    for clause in 1..=60 {
        body.push_str(&format!(
            "<h2>{clause}. Clause {clause}</h2>\n<p>The stand-in's clause {clause} goes on for a while, so that \
             the agreement is long enough to scroll with the pad. It says nothing &amp; means nothing, \
             and is here only to be read through. It runs on over a second line and a third, as a real \
             agreement's clauses do, and then it ends.</p>\n<ul><li>Its first point.</li><li>Its second \
             point.</li></ul>\n"
        ));
    }
    body.push_str("<p>End of the stand-in licence agreement.</p>\n");
    Some(format!(
        "<!DOCTYPE html>\n<html><head><title>Licence agreement</title><style>body {{ color: #ccc }}</style>\
         <script>window.stand_in = '<p>never shown</p>';</script></head>\n<body><header>Stand-in store</header>\
         <nav>Store | Library</nav>\n<div class=\"eula\">{body}</div><footer>Stand-in footer</footer></body></html>"
    ))
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
        let old = Wizard::Open { appid: quiet, folder: Some(root.display().to_string()) }
            .script()
            .replace(&format!("OpenInstallWizard([{quiet}], '0')"), &format!("OpenInstallWizard([{quiet}])"));
        let answer = |script: &str| match cef::evaluate(&ports, cef::SHARED, script) {
            Ok(cef::Evaluated::Value(text)) => serde_json::from_str::<Answer>(&text).unwrap(),
            other => panic!("{other:?}"),
        };
        let wizard = answer(&old);
        assert!(wizard.done && wizard.value.is_none(), "returned undefined, as Steam's does");
        assert_eq!(debugger.install_state(), 0, "no wizard opened");
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

    /// #230: the wizard's states as the real client's went, walked through Steam's own calls: a licence agreement
    /// declined, then accepted (two of them, one after another), a product key asked for, and a game too big for
    /// the drive. Every stop cancels the wizard back to 0, and nothing but the user's own Accept marks one accepted.
    #[test]
    fn the_stand_ins_install_wizard_goes_as_steams_and_the_install_stops_where_it_must() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        fixture::write(&root, &fixture::Fixture::standard()).unwrap();
        std::fs::write(root.join(fixture::STANDIN_CLIENT), "").unwrap();
        let said = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let kept = said.clone();
        let debugger = Debugger::start(&root, move |line| kept.lock().unwrap().push(line.to_string())).unwrap();
        let ports = cef::ports(&root);
        let (agreed, keyed, plain) = (9_100_060, 9_100_100, 9_100_040);
        let set_up = |free: Option<u64>| {
            let set_up = WizardSetUp { agreements: [(agreed, 2)].into(), keys: vec![keyed], free };
            set_up.write(&root).unwrap();
        };
        set_up(None);
        let queued =
            |appid: u32| installs::under_way(&crate::libraries(&root), &|_| false).iter().any(|found| found.0 == appid);
        let install = |appid: u32| Call::Install { appid, folder: None };
        // What the install came to, and each agreement the user was shown.
        let ask = |appid: u32, replies: &[model::Reply]| {
            let mut seen: Vec<model::Agreement> = Vec::new();
            let mut replies = replies.iter().copied();
            let mut user = |agreement: &model::Agreement| {
                seen.push(agreement.clone());
                replies.next().unwrap_or(model::Reply::Decline)
            };
            let outcome = client::ask_user(&ports, Some(&root), &install(appid), &mut user);
            (outcome, seen)
        };
        use model::Reply::{Accept, Decline, OpenInStore};

        // Declined: the wizard cancelled, nothing queued, nothing marked accepted.
        let (outcome, seen) = ask(agreed, &[Decline]);
        assert_eq!(outcome, Outcome::Stopped(model::Stop::Declined));
        assert_eq!((debugger.install_state(), queued(agreed)), (0, false));
        assert_eq!(seen.len(), 1);
        let text = seen[0].text.as_ref().unwrap();
        assert!(text.starts_with("Stand-in Licence Agreement 1 of 2"), "{text}");
        assert!(!text.contains("never shown") && !text.contains("Stand-in footer") && !text.contains("<p>"));
        assert_eq!((seen[0].at, seen[0].of), (1, 2));

        // The first accepted and the second declined: still nothing.
        let (outcome, seen) = ask(agreed, &[Accept, Decline]);
        assert_eq!(outcome, Outcome::Stopped(model::Stop::Declined));
        assert_eq!((debugger.install_state(), queued(agreed), seen.len()), (0, false, 2));

        // Both accepted, one after another: the install goes on to 14 and is queued.
        let (outcome, seen) = ask(agreed, &[Accept, Accept]);
        assert_eq!(outcome, Outcome::Took);
        assert_eq!((debugger.install_state(), queued(agreed)), (14, true));
        assert_eq!(seen.iter().map(|agreement| agreement.at).collect::<Vec<_>>(), [1, 2]);
        assert!(seen[1].text.as_ref().unwrap().starts_with("Stand-in Licence Agreement 2 of 2"));
        fixture::write_progress(&root, agreed, Progress::Dropped).unwrap();

        // Left to Steam's own dialog: the wizard cancelled.
        assert_eq!(ask(agreed, &[OpenInStore]).0, Outcome::LeftToSteam);
        assert_eq!((debugger.install_state(), queued(agreed)), (0, false));

        // A product key: the wizard never gets to its config.
        assert_eq!(ask(keyed, &[]).0, Outcome::Stopped(model::Stop::StoreAsks("a product key".into())));
        assert_eq!((debugger.install_state(), queued(keyed)), (0, false));

        // Too big for what is free: refused with the manager's own figures, before ContinueInstall.
        let needed = fixture::needed(plain);
        set_up(Some(needed - 1));
        assert_eq!(ask(plain, &[]).0, Outcome::Stopped(model::Stop::NoSpace { needed, free: needed - 1 }));
        assert_eq!((debugger.install_state(), queued(plain)), (0, false));
        set_up(Some(needed));
        assert_eq!(ask(plain, &[]).0, Outcome::Stopped(model::Stop::NoSpace { needed, free: needed }), "as Steam: >=");
        // With room, a game with no agreement is taken without the user being asked.
        set_up(None);
        let (outcome, seen) = ask(plain, &[]);
        assert_eq!(outcome, Outcome::Took);
        assert!(seen.is_empty() && queued(plain));

        let said = said.lock().unwrap();
        let has = |line: &str| said.iter().any(|said| said == line);
        assert!(has("CEF: install wizard: eInstallState 0 -> 5"), "{said:#?}");
        assert!(has("CEF: install wizard: eInstallState 5 -> 7"), "{said:#?}");
        assert!(has("CEF: install wizard: eInstallState 7 -> 8"), "{said:#?}");
        assert!(has(&format!("CEF: licence agreement \"{agreed}_eula_1\" version 3 of {agreed} marked accepted")));
        assert!(has("CEF: install wizard cancelled: eInstallState 8 -> 0"), "{said:#?}");
        assert!(has("CEF: install wizard cancelled: eInstallState 4 -> 0"), "{said:#?}");
        assert!(has("CEF: install wizard cancelled: eInstallState 7 -> 0"), "{said:#?}");
        assert!(said.iter().any(|line| line.starts_with(&format!(
            "CEF: asked to install {agreed} into Steam's default library: {agreed} queued in "
        ))));
    }

    /// A script that makes Steam's calls otherwise than the real client takes them does nothing, as Steam's does.
    #[test]
    fn a_wizard_step_made_otherwise_than_steam_takes_it_is_not_taken() {
        let step = Wizard::Accept { appid: 7, id: serde_json::json!("7_eula_0"), version: serde_json::json!(3) };
        assert_eq!(wizard_takes(&step.script(), &step), Ok(()));
        let without = step.script().replace("MarkEulaAccepted(7, ", "MarkEulaAccepted(");
        assert!(wizard_takes(&without, &step).is_err());
        for step in [
            Wizard::Look,
            Wizard::Continue,
            Wizard::Cancel,
            Wizard::Agreements { appid: 7 },
            Wizard::Prepare { index: Some(1) },
        ] {
            assert_eq!(wizard_takes(&step.script(), &step), Ok(()), "{step:?}");
        }
        let open = Wizard::Open { appid: 7, folder: None };
        assert_eq!(wizard_takes(&open.script(), &open), Ok(()));
    }
}
