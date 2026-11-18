//! The host thread on Windows: WebView2Loader.dll loaded from beside dashboard.exe, one window of its own that is
//! shown only for the sign-in page, and a loop that runs the thread's messages (WebView2 answers through them), takes
//! the Dashboard's commands and looks at the page now and then.
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use model::discord_web::{WebField, WebState};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

use crate::{say, Command, Report};

mod host;

pub type Tell = Box<dyn Fn(Report) + Send>;

/// Starts the host thread. `Err` (no runtime) when the loader is not beside the exe or finds no WebView2 runtime:
/// then nothing is started. What comes back wakes the thread to read its commands.
pub fn spawn(
    folder: PathBuf,
    base: String,
    commands: Receiver<Command>,
    tell: Tell,
) -> Result<Box<dyn Fn() + Send>, String> {
    let loader = host::Loader::load()?;
    let version = loader.runtime_version()?;
    say(&format!("discord web: WebView2 runtime {version}, loader {}", loader.path.display()));
    let (id_tx, id_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("discord web".into())
        .spawn(move || {
            let _ = id_tx.send(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() });
            run(loader, folder, base, commands, tell);
        })
        .map_err(|e| format!("the host thread: {e}"))?;
    let id = id_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| format!("the host thread: {e}"))?;
    Ok(Box::new(move || unsafe {
        let _ = PostThreadMessageW(id, WM_APP, WPARAM(0), LPARAM(0));
    }))
}

/// The runtime the loader finds, with nothing started: Settings' no-runtime before any web view is wanted.
pub fn runtime() -> Result<String, String> {
    host::Loader::load()?.runtime_version()
}

/// How often the page is looked at: while the sign-in page is up, and while not.
const LOOK_SHOWN: Duration = Duration::from_millis(250);
const LOOK_HIDDEN: Duration = Duration::from_millis(1500);

fn run(loader: host::Loader, folder: PathBuf, base: String, commands: Receiver<Command>, tell: Tell) {
    let host = match host::Host::open(&loader, &folder) {
        Ok(host) => host,
        Err(e) => {
            say(&format!("discord web: the web view could not be opened: {e}"));
            tell(Report::State(WebState::Failed));
            tell(Report::Ended { signed_out: None });
            return;
        }
    };
    // The page is told it has focus whether or not its window has (the spike's lesson: keys and Enter reach a
    // field only so while hidden or never activated).
    host.cdp_send("Emulation.setFocusEmulationEnabled", serde_json::json!({ "enabled": true }));
    host.navigate(&format!("{base}/app"));
    let mut state = WebState::Checking;
    let mut field: Option<WebField> = None;
    let mut shown = false;
    let mut next_look = Instant::now() + Duration::from_millis(500);
    loop {
        host::pump(Duration::from_millis(if shown { 100 } else { 500 }));
        if host::closed() {
            say("discord web: the web view's window was closed");
            tell(Report::Ended { signed_out: None });
            return;
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
                Command::Key(key) => host.key(key),
                Command::Text(text) => host.insert(&text),
                Command::Stop => {
                    host.close();
                    say("discord web: the web view is closed; the session is kept");
                    tell(Report::Ended { signed_out: None });
                    return;
                }
                Command::SignOut => {
                    host.close();
                    // The browser processes let go of the folder once they have ended.
                    let removed = crate::remove(&folder, 10, Duration::from_millis(500));
                    tell(Report::Ended { signed_out: Some(removed) });
                    return;
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
