//! The host thread on Windows: WebView2Loader.dll loaded from beside dashboard.exe, one window of its own that is
//! shown only for the sign-in page, and a loop that runs the thread's messages (WebView2 answers through them), takes
//! the Dashboard's commands and looks at the page now and then.
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use model::discord_web::{WebField, WebKey, WebState};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

use crate::{say, Command, Report, Spawned, Tell};

mod host;

/// Starts the host thread. `Err` (no runtime) when the loader is not beside the exe or finds no WebView2 runtime:
/// then nothing is started and the loader is let go. The thread initialises COM for itself (WebView2 needs a
/// single-threaded apartment on the thread that makes it) and uninitialises it, its window, web view and loader all
/// released, before its last report, `Ended`.
pub(crate) fn spawn(folder: PathBuf, base: String, commands: Receiver<Command>, tell: Tell) -> Result<Spawned, String> {
    let loader = host::Loader::load()?;
    let version = loader.runtime_version()?;
    say(&format!("discord web: WebView2 runtime {version}, loader {}", loader.path.display()));
    let (id_tx, id_rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("discord web".into())
        .spawn(move || {
            let _ = id_tx.send(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() });
            let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            let signed_out = if com.is_err() {
                say(&format!("discord web: the web view could not be opened: COM: {com:?}"));
                tell(Report::State(WebState::Failed));
                drop(loader);
                None
            } else {
                let signed_out = run(loader, folder, base, commands, &tell);
                unsafe { CoUninitialize() };
                signed_out
            };
            tell(Report::Ended { signed_out });
        })
        .map_err(|e| format!("the host thread: {e}"))?;
    let id = id_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| format!("the host thread: {e}"))?;
    Ok(Spawned {
        poke: Box::new(move || unsafe {
            let _ = PostThreadMessageW(id, WM_APP, WPARAM(0), LPARAM(0));
        }),
        thread,
    })
}

/// The runtime the loader finds, with nothing started: Settings' no-runtime before any web view is wanted.
pub fn runtime() -> Result<String, String> {
    host::Loader::load()?.runtime_version()
}

/// How often the page is looked at: while the sign-in page is up, and while not.
const LOOK_SHOWN: Duration = Duration::from_millis(250);
const LOOK_HIDDEN: Duration = Duration::from_millis(1500);

/// The web view's life on this thread; what a sign-out removed, if it was one. Everything it made is released by
/// the time it returns.
fn run(
    loader: host::Loader,
    folder: PathBuf,
    base: String,
    commands: Receiver<Command>,
    tell: &Tell,
) -> Option<Result<(), String>> {
    let host = match host::Host::open(&loader, &folder, &base) {
        Ok(host) => host,
        Err(e) => {
            say(&format!("discord web: the web view could not be opened: {e}"));
            tell(Report::State(WebState::Failed));
            return None;
        }
    };
    // The page is told it has focus whether or not its window has (the spike's lesson: keys and Enter reach a
    // field only so while hidden or never activated).
    host.cdp_send("Emulation.setFocusEmulationEnabled", serde_json::json!({ "enabled": true }));
    // The page script (Direct Messages) is installed before the first document loads.
    host.reading(tell.clone(), &base);
    host.navigate(&format!("{base}/app"));
    let mut state = WebState::Checking;
    let mut field: Option<WebField> = None;
    let mut shown = false;
    // The QR code first: until the pad sends a key, a field the page focused by itself is let go, so the page opens
    // with no field waiting for the keyboard. The first Tab then goes to the form's first field.
    let mut hands_off = true;
    let mut qr: Option<bool> = None;
    let mut next_look = Instant::now() + Duration::from_millis(500);
    loop {
        host::pump(Duration::from_millis(if shown { 100 } else { 500 }));
        if host::closed() {
            say("discord web: the web view's window was closed");
            host.close();
            tell(Report::State(WebState::Failed));
            return None;
        }
        loop {
            let command = match commands.try_recv() {
                Ok(command) => command,
                Err(mpsc::TryRecvError::Empty) => break,
                // The Dashboard is closing: the session stays kept.
                Err(mpsc::TryRecvError::Disconnected) => Command::Stop,
            };
            match command {
                Command::Show(area) => {
                    shown = true;
                    host.show(area);
                    // Signed out is the login page already, as it was left (text typed in it kept); not yet known,
                    // the login page is asked for.
                    if state == WebState::Checking {
                        host.navigate(&format!("{base}/login"));
                        hands_off = true;
                    }
                    next_look = Instant::now();
                }
                Command::Hide => {
                    shown = false;
                    host.hide();
                    if field.take().is_some() {
                        tell(Report::Field(None));
                    }
                }
                Command::Key(key) if hands_off && key == WebKey::Tab => {
                    hands_off = false;
                    host.first_field();
                    next_look = Instant::now();
                }
                Command::Key(key) => {
                    hands_off = false;
                    host.key(key);
                }
                Command::Text(text) => {
                    hands_off = false;
                    host.insert(&text);
                }
                Command::Page(op) => host.page(&op),
                Command::Stop => {
                    host.close();
                    say("discord web: the web view is closed; the session is kept");
                    return None;
                }
                Command::SignOut => {
                    host.close();
                    // The browser processes let go of the folder once they have ended.
                    let removed = crate::remove(&folder, 10, Duration::from_millis(500));
                    return Some(removed);
                }
            }
        }
        if Instant::now() < next_look {
            continue;
        }
        next_look = Instant::now() + if shown { LOOK_SHOWN } else { LOOK_HIDDEN };
        let Ok(page) = host.state() else { continue };
        let now = if page["app"].as_bool() == Some(true) {
            WebState::SignedIn
        } else if page["login"].as_bool() == Some(true) {
            WebState::SignedOut
        } else {
            state
        };
        if now != state {
            state = now;
            tell(Report::State(state));
        }
        let login = page["login"].as_bool() == Some(true);
        if shown && login {
            // Whether Discord shows its QR code: presence only, never what it holds.
            let now = page["qr"].as_bool() == Some(true);
            if qr != Some(now) {
                qr = Some(now);
                say(&format!("discord web: QR code shown {}", if now { "yes" } else { "no" }));
            }
        }
        let focused = !page["field"].as_str().unwrap_or("").is_empty();
        if shown && login && hands_off && focused {
            host.unfocus();
            next_look = Instant::now() + Duration::from_millis(50);
            continue;
        }
        let seen = match page["field"].as_str() {
            Some("password") if shown => Some(WebField::Password),
            Some("text") if shown => Some(WebField::Text),
            _ => None,
        };
        if seen != field {
            field = seen;
            tell(Report::Field(field));
        }
    }
}
