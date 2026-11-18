//! Discord's web session (#197; docs/discord-web.md): the Dashboard's own discord.com in a WebView2, on a thread of
//! its own, signed in by the user on Discord's own page and kept in the data folder, one session per Profile.
//!
//! Nothing here runs until Settings > Account > Discord messages is turned on ([`Session::set_on`]). On, the host
//! thread opens discord.com in a web view that is never shown, to learn whether the session is signed in; the sign-in
//! page shows it over the Dashboard ([`Session::show`]) and the pad drives it by keys. Sign out removes the session's
//! folder, and with it everything the web view kept.
//!
//! What this code never does: read, log or keep a token, a cookie, a password or anything typed. It asks the page
//! only for flags (`js/state.js`: which route, whether the app's shell shows, the kind of field focus is on); text the
//! Console keyboard kept goes into the focused field by DevTools' `Input.insertText` and is dropped. The log has
//! addresses as scheme, host and path only ([`redact::address`]).
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

pub use model::discord_web::{Web, WebAsk, WebField, WebKey, WebState};

pub mod redact;
#[cfg(windows)]
mod win;

/// Discord's own address. A test points the Dashboard at a stand-in (`--discord-web-url`, `standin serve-discord-web`).
pub const DISCORD: &str = "https://discord.com";

/// Where a Profile's session is kept: `<data>\discord-web\profile-<id>`, the web view's whole user data folder. One
/// per Profile, as the account-switching research expects (docs/research/account-switching.md, Summary 1):
/// a Profile signs in to its own person's Discord, and another Profile's sign-in never shows it.
pub fn folder(data: &Path, profile: i64) -> PathBuf {
    data.join("discord-web").join(format!("profile-{profile}"))
}

/// Whether a session is kept in `folder`: anything at all in it.
pub fn kept(folder: &Path) -> bool {
    std::fs::read_dir(folder).is_ok_and(|mut entries| entries.next().is_some())
}

/// Removes a session's folder, trying a few times while the web view's browser processes let go of it. How many
/// files were left, and why, if any were.
pub fn remove(folder: &Path, tries: u32, pause: std::time::Duration) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 1..=tries.max(1) {
        match std::fs::remove_dir_all(folder) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                last = format!("attempt {attempt}: {e}");
                std::thread::sleep(pause);
            }
        }
    }
    let left = count(folder);
    Err(format!("{left} files are left ({last})"))
}

/// Files under a folder, all the way down.
pub fn count(folder: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(folder) else { return 0 };
    entries.flatten().map(|entry| if entry.path().is_dir() { count(&entry.path()) } else { 1 }).sum()
}

/// The sign-in page's area on the screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// What the Dashboard's thread asks of the host thread.
#[allow(dead_code)]
enum Command {
    Show(Area),
    Hide,
    Key(WebKey),
    /// Never logged.
    Text(String),
    /// Close the web view, remove the folder, end.
    SignOut,
    /// Close the web view and end; the folder stays.
    Stop,
}

/// What the host thread says back.
#[allow(dead_code)]
enum Report {
    State(WebState),
    Field(Option<WebField>),
    /// The host thread has ended; after a sign-out, whether the folder went.
    Ended {
        signed_out: Option<Result<(), String>>,
    },
}

/// The host thread as the Dashboard's thread holds it.
struct Host {
    commands: Sender<Command>,
    /// Wakes the host thread's message loop to read `commands`.
    poke: Box<dyn Fn() + Send>,
    profile: i64,
}

impl Host {
    fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            (self.poke)();
        }
    }
}

type Wake = Arc<dyn Fn() + Send + Sync>;

/// The session, as the Dashboard's thread drives it.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Session {
    base: String,
    data: Option<PathBuf>,
    profile: Option<i64>,
    on: bool,
    web: Web,
    host: Option<Host>,
    /// A host is ending (a sign-out, or another Profile's): a new one starts once it has.
    ending: bool,
    shown: Option<Area>,
    /// The sign-in page is up, shown or with text entry over it: the web view is wanted.
    signing_in: bool,
    reports: Receiver<Report>,
    report: Sender<Report>,
    wake: Wake,
}

impl Session {
    /// `wake` is called from the host thread whenever it has something to say: [`Session::pump`] then.
    pub fn new(base: Option<&str>, data: Option<&Path>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (report, reports) = mpsc::channel();
        Session {
            base: base.unwrap_or(DISCORD).trim_end_matches('/').to_string(),
            data: data.map(Path::to_path_buf),
            profile: None,
            on: false,
            web: Web::default(),
            host: None,
            ending: false,
            shown: None,
            signing_in: false,
            reports,
            report,
            wake: Arc::new(wake),
        }
    }

    /// How the session stands, for the screens.
    pub fn web(&self) -> Web {
        self.web
    }

    fn folder(&self) -> Option<PathBuf> {
        Some(folder(self.data.as_deref()?, self.profile?))
    }

    /// Settings' switch. Off ends the host thread; the session stays kept for when it is on again.
    pub fn set_on(&mut self, on: bool) {
        if self.on == on {
            return;
        }
        self.on = on;
        say(&format!("discord web: switched {}", if on { "on" } else { "off" }));
        self.follow();
    }

    /// The Profile signed in: its own session, another's host ended first.
    pub fn set_profile(&mut self, profile: Option<i64>) {
        if self.profile == profile {
            return;
        }
        self.profile = profile;
        self.follow();
    }

    /// Starts or ends the host thread as the switch, the Profile and a host still ending allow. The web view runs
    /// only while it has something to do: a session is kept to check and follow, or the sign-in page is up. Signed
    /// out with the page closed, nothing runs and nothing is on the PC.
    fn follow(&mut self) {
        let wanted = self.on.then_some(()).and(self.profile);
        if let Some(host) = &self.host {
            if wanted != Some(host.profile) && !self.ending {
                host.send(Command::Stop);
                self.ending = true;
            }
        }
        self.web.kept = self.folder().is_some_and(|f| kept(&f));
        if !self.on {
            self.set_state(WebState::Off);
            return;
        }
        if self.host.is_some() || self.ending {
            return;
        }
        let (Some(profile), Some(folder)) = (self.profile, self.folder()) else {
            self.set_state(WebState::Failed);
            return;
        };
        if self.web.kept || self.signing_in {
            self.start(profile, folder);
            return;
        }
        match runtime() {
            Ok(_) => self.set_state(WebState::SignedOut),
            Err(e) => {
                say(&format!("discord web: no runtime: {e}"));
                self.set_state(WebState::NoRuntime);
            }
        }
    }

    /// Whether the sign-in page is up (shown, or under text entry). Closed before the session signed in, what the web
    /// view made is removed: a sign-in given up keeps nothing.
    pub fn set_signing_in(&mut self, up: bool) {
        if self.signing_in == up {
            return;
        }
        self.signing_in = up;
        if up {
            self.follow();
        } else if self.web.state != WebState::SignedIn && self.host.is_some() {
            say("discord web: the sign-in page closed before signing in: nothing is kept");
            self.sign_out();
        }
    }

    #[cfg(windows)]
    fn start(&mut self, profile: i64, folder: PathBuf) {
        let (commands, receive) = mpsc::channel();
        let report = self.report.clone();
        let wake = self.wake.clone();
        let tell: win::Tell = Box::new(move |r| {
            let _ = report.send(r);
            wake();
        });
        match win::spawn(folder, self.base.clone(), receive, tell) {
            Ok(poke) => {
                self.host = Some(Host { commands, poke, profile });
                self.set_state(WebState::Checking);
            }
            Err(e) => {
                say(&format!("discord web: no runtime: {e}"));
                self.set_state(WebState::NoRuntime);
            }
        }
    }

    /// Anywhere but Windows there is no WebView2.
    #[cfg(not(windows))]
    fn start(&mut self, _profile: i64, _folder: PathBuf) {
        say("discord web: no runtime: WebView2 is only on Windows");
        self.set_state(WebState::NoRuntime);
    }

    fn set_state(&mut self, state: WebState) {
        if self.web.state != state {
            say(&format!("discord web: {}", state.word()));
            self.web.state = state;
        }
        if state != WebState::SigningIn {
            self.web.field = None;
        }
    }

    /// Shows Discord's sign-in page over `area`, or puts it away (None). Only a change is sent.
    pub fn show(&mut self, area: Option<Area>) {
        if self.shown == area {
            return;
        }
        self.shown = area;
        let Some(host) = &self.host else { return };
        match area {
            Some(area) => {
                host.send(Command::Show(area));
                if self.web.state.can_sign_in() {
                    self.set_state(WebState::SigningIn);
                }
            }
            None => {
                host.send(Command::Hide);
                if self.web.state == WebState::SigningIn {
                    self.set_state(WebState::SignedOut);
                }
            }
        }
    }

    /// What Settings and the sign-in page ask.
    pub fn ask(&mut self, ask: WebAsk) {
        match ask {
            WebAsk::ShowSignIn | WebAsk::HideSignIn => {}
            WebAsk::Key(key) => {
                if let Some(host) = &self.host {
                    host.send(Command::Key(key));
                }
            }
            WebAsk::Text(text) => {
                if let Some(host) = &self.host {
                    host.send(Command::Text(text));
                }
            }
            WebAsk::SignOut => self.sign_out(),
        }
    }

    fn sign_out(&mut self) {
        let Some(folder) = self.folder() else { return };
        say(&format!("discord web: signing out: {} files in the session's folder", count(&folder)));
        match &self.host {
            Some(host) if !self.ending => {
                host.send(Command::SignOut);
                self.ending = true;
            }
            _ => {
                // No web view holds it: removed here and now.
                let removed = remove(&folder, 5, std::time::Duration::from_millis(300));
                self.signed_out(&folder, removed);
            }
        }
    }

    fn signed_out(&mut self, folder: &Path, removed: Result<(), String>) {
        match removed {
            Ok(()) => say(&format!("discord web: signed out: nothing is left, {} is gone", folder.display())),
            Err(e) => say(&format!("discord web: signed out, but {e} in {}", folder.display())),
        }
        self.web.kept = kept(folder);
        if self.on {
            self.set_state(WebState::SignedOut);
        }
    }

    /// Takes in what the host thread said. True when what the screens show changed.
    pub fn pump(&mut self) -> bool {
        let before = self.web;
        while let Ok(report) = self.reports.try_recv() {
            match report {
                Report::State(state) => {
                    // A page that is up is signing in until it is signed in.
                    let state =
                        if self.shown.is_some() && state == WebState::SignedOut { WebState::SigningIn } else { state };
                    self.set_state(state);
                    if state == WebState::SignedIn {
                        self.web.kept = true;
                    }
                }
                Report::Field(field) => {
                    if self.shown.is_some() && self.web.field != field {
                        // The kind of field only: what is in it is never asked.
                        let kind = match field {
                            Some(WebField::Text) => "a text field",
                            Some(WebField::Password) => "a password field",
                            None => "no field",
                        };
                        say(&format!("discord web: focus on {kind}"));
                        self.web.field = field;
                    }
                }
                Report::Ended { signed_out } => {
                    self.host = None;
                    self.ending = false;
                    if let (Some(removed), Some(folder)) = (signed_out, self.folder()) {
                        self.signed_out(&folder, removed);
                    }
                    let shown = self.shown.take();
                    self.follow();
                    if self.host.is_some() {
                        self.show(shown);
                    }
                }
            }
        }
        before != self.web
    }
}

/// The Dashboard is closing: the web view is closed properly, so what the browser has of the session is written out,
/// and the session stays kept. Waited for, a few seconds at most.
impl Drop for Session {
    fn drop(&mut self) {
        let Some(host) = self.host.take() else { return };
        if !self.ending {
            host.send(Command::Stop);
        }
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while let Some(left) = until.checked_duration_since(std::time::Instant::now()) {
            match self.reports.recv_timeout(left) {
                Ok(Report::Ended { .. }) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }
}

/// The WebView2 runtime the loader beside the exe finds; `Err` with no loader or no runtime.
#[cfg(windows)]
fn runtime() -> Result<String, String> {
    win::runtime()
}

#[cfg(not(windows))]
fn runtime() -> Result<String, String> {
    Err("WebView2 is only on Windows".into())
}

pub(crate) fn say(line: &str) {
    log_line::write_line(format_args!("{line}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(data: &Path) -> Session {
        let mut session = Session::new(None, Some(data), || {});
        session.set_profile(Some(7));
        session
    }

    #[test]
    fn each_profile_has_its_own_folder_in_the_data_folder() {
        let data = Path::new("data");
        assert_eq!(folder(data, 7), data.join("discord-web").join("profile-7"));
        assert_ne!(folder(data, 7), folder(data, 8));
    }

    #[test]
    fn nothing_starts_until_the_switch_is_on() {
        let data = tempfile::tempdir().unwrap();
        let session = session(data.path());
        assert_eq!(session.web().state, WebState::Off);
        assert!(session.host.is_none());
        assert!(!data.path().join("discord-web").exists(), "nothing is made while off");
    }

    #[cfg(not(windows))]
    #[test]
    fn on_without_webview2_is_no_runtime_and_off_again_is_off() {
        let data = tempfile::tempdir().unwrap();
        let mut session = session(data.path());
        session.set_on(true);
        assert_eq!(session.web().state, WebState::NoRuntime);
        session.set_on(false);
        assert_eq!(session.web().state, WebState::Off);
    }

    #[test]
    fn sign_out_leaves_nothing_of_the_session() {
        let data = tempfile::tempdir().unwrap();
        let mut session = session(data.path());
        let kept = folder(data.path(), 7);
        std::fs::create_dir_all(kept.join("EBWebView").join("Default")).unwrap();
        std::fs::write(kept.join("EBWebView").join("Default").join("Cookies"), b"made up").unwrap();
        session.set_on(true);
        assert!(session.web().kept && session.web().can_sign_out());
        // Another Profile's session is not touched.
        let other = folder(data.path(), 8);
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("Local State"), b"made up").unwrap();
        session.host = None;
        session.ask(WebAsk::SignOut);
        assert!(!kept.exists(), "the Profile's folder is gone");
        assert!(!session.web().kept);
        assert!(other.join("Local State").exists());
    }

    #[test]
    fn a_kept_folder_is_one_with_anything_in_it() {
        let data = tempfile::tempdir().unwrap();
        let empty = data.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(!kept(&empty) && !kept(&data.path().join("missing")));
        std::fs::write(empty.join("x"), b"").unwrap();
        assert!(kept(&empty));
        assert_eq!(count(data.path()), 1);
    }
}
