//! Discord's web session (#197; docs/discord-web.md): the Dashboard's own discord.com in a WebView2, on a thread of
//! its own, signed in by the user on Discord's own page and kept in the data folder, one session per Profile.
//!
//! Nothing here runs until Settings > Account > Discord messages is turned on ([`Session::set_on`]). On, the host
//! thread opens discord.com in a web view that is never shown, to learn whether the session is signed in; the sign-in
//! page shows it over the Dashboard ([`Session::show`]) and the pad drives it by keys. Sign out removes the session's
//! folder, and with it everything the web view kept.
//!
//! Direct Messages (#198): a page script (`js/stores.js`, `js/direct.js`) reads Discord's own in-page stores and pushes
//! what it reads; [`Session::direct`] holds it, in memory only, for the screens (docs/discord-web.md, "Reading").
//!
//! What this code never does: read, log or keep a token, a cookie, a password or anything typed. It asks the page
//! only for flags (`js/state.js`: which route, whether the app's shell shows, the kind of field focus is on); text the
//! Console keyboard kept goes into the focused field by DevTools' `Input.insertText` and is dropped. The log has
//! addresses as scheme, host and path only ([`redact::address`]).
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

pub use model::discord_dms::{Direct, DirectAsk, DirectEvent, DirectState};
pub use model::discord_web::{Web, WebAsk, WebField, WebKey, WebState};

pub(crate) mod act;
pub mod direct;
mod pictures;
pub mod redact;
#[cfg(windows)]
mod win;

/// Discord's own address. A test points the Dashboard at a stand-in (`--discord-web-url`, `standin serve-discord-web`).
pub const DISCORD: &str = "https://discord.com";

/// Discord's CDN, where the avatars and icons of the Direct Messages come from. A test points the Dashboard at a
/// stand-in ([`Session::set_cdn`], `--discord-cdn-url`).
pub const CDN: &str = "https://cdn.discordapp.com";

/// The script the host installs in every document of the web view (`AddScriptToExecuteOnDocumentCreated`): the store
/// search (`js/stores.js`), the reader that pushes what the stores hold (`js/direct.js`) and the actions it takes by
/// the host's ask (`js/act.js`), in one function of their own so that a second injection and the page's scripts cannot
/// meet. See docs/discord-web.md, "Reading" and "Acting".
pub fn direct_script() -> String {
    format!(
        "(() => {{\n{}\n{}\n{}\n}})();\n",
        include_str!("../js/stores.js"),
        include_str!("../js/direct.js"),
        include_str!("../js/act.js")
    )
}

/// Where a Profile's session is kept:`<data>\discord-web\profile-<id>`, the web view's whole user data folder. One
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

/// The smallest page, in CSS pixels, the sign-in page is zoomed to: wide enough that Discord keeps its QR panel beside
/// the form (it drops the panel on a narrow page), tall enough for the form. Within that, the page is zoomed as far as
/// it goes, so the QR code is as large as it can be from a sofa: on a 1080-line screen (970 lines above the band),
/// 2.2 times, Discord's QR panel about 390 pixels across. docs/discord-web.md, "The QR code first".
pub const VIEW_MIN: (f64, f64) = (860.0, 440.0);

/// The page zoom (WebView2's `ZoomFactor`) for `area` at the window's rasterization scale `raster` (its DPI scale):
/// the largest that leaves the page `VIEW_MIN` CSS pixels at least, never less than 1.
pub fn zoom(area: Area, raster: f64) -> f64 {
    let raster = if raster > 0.0 { raster } else { 1.0 };
    let fit = (area.width as f64 / VIEW_MIN.0).min(area.height as f64 / VIEW_MIN.1) / raster;
    (fit * 100.0).floor().max(100.0) / 100.0
}

/// What the Dashboard's thread asks of the host thread.
#[allow(dead_code)]
pub(crate) enum Command {
    Show(Area),
    Hide,
    Key(WebKey),
    /// Never logged.
    Text(String),
    /// Something for the page script: Discord's Direct Messages (docs/discord-web.md, "Reading").
    Page(PageOp),
    /// An action for the page script (docs/discord-web.md, "Acting").
    Act(act::ActOp),
    /// What the Console keyboard kept for a message: typed into the page's focused message box, then Enter after a
    /// moment. Never logged; `clear` first empties a draft the box held.
    Type {
        text: String,
        clear: bool,
    },
    /// Close the web view, remove the folder, end.
    SignOut,
    /// Close the web view and end; the folder stays.
    Stop,
}

/// What the page script is asked (`window.__xsdDirect`): the host calls the method of the same name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum PageOp {
    /// Route the hidden page to this DM (digits only: the host refuses anything else) and read its messages.
    Open(String),
    /// Route it back to `/channels/@me`.
    Close,
    /// Discord's own older-messages action for the open conversation.
    Older,
    /// Send everything again.
    Resend,
}

/// A web notification Discord's page raised, taken by the host (marked handled, so Windows shows nothing). Its words
/// are private: `Debug` says lengths only.
pub(crate) struct Notified {
    pub title: String,
    pub body: String,
    pub tag: String,
}

impl std::fmt::Debug for Notified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Notified(title {}, body {}, tag {} chars)",
            self.title.chars().count(),
            self.body.chars().count(),
            self.tag.chars().count()
        )
    }
}

/// What the host thread says back.
#[allow(dead_code)]
pub(crate) enum Report {
    State(WebState),
    /// One message the page script posted (`js/direct.js`), as JSON text: parsed by [`direct::Reader`].
    Page(String),
    Notified(Notified),
    Field(Option<WebField>),
    /// The host thread has ended, its window, web view and loader released; after a sign-out, whether the folder
    /// went. Always its last word.
    Ended {
        signed_out: Option<Result<(), String>>,
    },
}

/// How the host thread tells the Dashboard's thread: the report sent, and its event loop woken.
pub(crate) type Tell = Arc<dyn Fn(Report) + Send + Sync>;

/// A host thread just started: what wakes it to read its commands, and the thread, joined once it has ended.
pub(crate) struct Spawned {
    pub poke: Box<dyn Fn() + Send>,
    pub thread: std::thread::JoinHandle<()>,
}

/// What starts the web view: WebView2 on Windows, nothing anywhere else, a fake in the tests.
pub(crate) trait Backend {
    /// The runtime's version, asked with nothing started; `Err` with no loader or no runtime.
    fn runtime(&self) -> Result<String, String>;
    /// Starts a host thread on `folder`. `Err` when none could be started (then nothing is left running).
    fn spawn(&self, folder: PathBuf, base: String, commands: Receiver<Command>, tell: Tell) -> Result<Spawned, String>;
}

#[cfg(windows)]
struct WebView2;

#[cfg(windows)]
impl Backend for WebView2 {
    fn runtime(&self) -> Result<String, String> {
        win::runtime()
    }

    fn spawn(&self, folder: PathBuf, base: String, commands: Receiver<Command>, tell: Tell) -> Result<Spawned, String> {
        win::spawn(folder, base, commands, tell)
    }
}

#[cfg(not(windows))]
struct NoWebView;

#[cfg(not(windows))]
impl Backend for NoWebView {
    fn runtime(&self) -> Result<String, String> {
        Err("WebView2 is only on Windows".into())
    }

    fn spawn(&self, _: PathBuf, _: String, _: Receiver<Command>, _: Tell) -> Result<Spawned, String> {
        Err("WebView2 is only on Windows".into())
    }
}

/// The host thread as the Dashboard's thread holds it. There is never more than one.
struct Host {
    commands: Sender<Command>,
    /// Wakes the host thread's message loop to read `commands`.
    poke: Box<dyn Fn() + Send>,
    thread: Option<std::thread::JoinHandle<()>>,
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

/// How many web views may fail to open in one run of the Dashboard. Then no more are tried until it starts again.
pub const MOST_FAILURES: u32 = 3;

/// Who asked for the web view: the user (the switch turned on, Sign in), or the session following something else
/// (the Dashboard starting, another Profile, a host ended on purpose). After a failure only the user may try again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum By {
    User,
    Itself,
}

/// The session, as the Dashboard's thread drives it.
pub struct Session {
    base: String,
    data: Option<PathBuf>,
    profile: Option<i64>,
    on: bool,
    web: Web,
    host: Option<Host>,
    /// The host was asked to end (a sign-out, another Profile's, the switch off): another starts once it has.
    ending: bool,
    shown: Option<Area>,
    /// The sign-in page is up, shown or with text entry over it: the web view is wanted.
    signing_in: bool,
    /// Web views started this run, and how many of them failed to open since one last opened.
    attempts: u32,
    failures: u32,
    /// The log has said it gave up.
    gave_up: bool,
    /// What the runtime check found, asked once a run and again only when the user turns the switch on.
    runtime: Option<Result<String, String>>,
    backend: Box<dyn Backend>,
    reports: Receiver<Report>,
    report: Sender<Report>,
    wake: Wake,
    /// Discord's Direct Messages: what the page said, what the screens see of it, and what happened since they
    /// last asked. All of it memory only, and forgotten with the session ([`Session::forget_direct`]).
    reader: direct::Reader,
    pictures: pictures::Pictures,
    direct: Direct,
    /// The change of `direct` since [`Session::pump`] last said so.
    direct_changed: bool,
    events: Vec<DirectEvent>,
    /// The conversation the screens asked to open.
    open: Option<model::discord_dms::Id>,
    /// What is being done for the screens, one at a time, and what they sent that has not yet gone (#201).
    acts: act::Acts,
    /// Kinds of ask not built yet that the log has said so of.
    unbuilt: std::collections::BTreeSet<&'static str>,
    older_unavailable_said: bool,
    notifications: u32,
}

impl Session {
    /// `wake` is called from the host thread whenever it has something to say: [`Session::pump`] then.
    pub fn new(base: Option<&str>, data: Option<&Path>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        #[cfg(windows)]
        let backend: Box<dyn Backend> = Box::new(WebView2);
        #[cfg(not(windows))]
        let backend: Box<dyn Backend> = Box::new(NoWebView);
        Self::with(base, data, wake, backend)
    }

    fn with(
        base: Option<&str>,
        data: Option<&Path>,
        wake: impl Fn() + Send + Sync + 'static,
        backend: Box<dyn Backend>,
    ) -> Self {
        let (report, reports) = mpsc::channel();
        let wake: Wake = Arc::new(wake);
        // Discord's CDN, unless the session is pointed at a stand-in Discord: then a stand-in CDN or none.
        let cdn = base.is_none().then(|| CDN.to_string());
        Session {
            reader: direct::Reader::new(),
            pictures: pictures::Pictures::new(cdn, wake.clone()),
            direct: Direct::default(),
            direct_changed: false,
            events: Vec::new(),
            open: None,
            acts: act::Acts::new(),
            unbuilt: Default::default(),
            older_unavailable_said: false,
            notifications: 0,
            base: base.unwrap_or(DISCORD).trim_end_matches('/').to_string(),
            data: data.map(Path::to_path_buf),
            profile: None,
            on: false,
            web: Web::default(),
            host: None,
            ending: false,
            shown: None,
            signing_in: false,
            attempts: 0,
            failures: 0,
            gave_up: false,
            runtime: None,
            backend,
            reports,
            report,
            wake,
        }
    }

    /// Where the Direct Messages' avatars and icons are fetched from: a stand-in CDN in a test (`--discord-cdn-url`),
    /// None for no pictures. Unless set, Discord's own CDN, or none when the session is pointed at a stand-in Discord.
    pub fn set_cdn(&mut self, cdn: Option<&str>) {
        self.pictures.set_cdn(cdn.map(str::to_string));
    }

    /// Discord's Direct Messages as the web session last read them (docs/discord-web.md, "Reading").
    pub fn direct(&self) -> &Direct {
        &self.direct
    }

    /// What happened since this was last asked, once each: a message in a DM that is not open, a mention, a ring
    /// beginning or ending. Nothing is raised for what was unread when the session began.
    pub fn take_events(&mut self) -> Vec<DirectEvent> {
        std::mem::take(&mut self.events)
    }

    /// What the screens ask of the Direct Messages. Open, Close and Older are read (#198); Send, Forget, React and
    /// MarkRead are acted (#201: [`act::Acts`], one at a time, paced; docs/discord-web.md, "Acting"); Call, Answer and
    /// Decline are the voice ticket's: each says it is not built once in the log, by kind only.
    pub fn ask_direct(&mut self, ask: DirectAsk) {
        match ask {
            DirectAsk::Open(id) => {
                if !id.as_str().bytes().all(|b| b.is_ascii_digit()) || id.as_str().is_empty() {
                    return;
                }
                // Another conversation's messages are not this one's.
                if self.open.as_ref() != Some(&id) {
                    self.reader.conv = None;
                }
                self.open = Some(id.clone());
                if let Some(host) = &self.host {
                    host.send(Command::Page(PageOp::Open(id.as_str().to_string())));
                }
                self.refresh_direct();
            }
            DirectAsk::Close => {
                if self.open.take().is_some() {
                    self.reader.conv = None;
                    if let Some(host) = &self.host {
                        host.send(Command::Page(PageOp::Close));
                    }
                    self.refresh_direct();
                }
            }
            DirectAsk::Older => {
                if !self.direct.open.as_ref().is_some_and(|c| !c.at_start && !c.loading_older) {
                    return;
                }
                if !self.reader.older_found() {
                    if !std::mem::replace(&mut self.older_unavailable_said, true) {
                        say("discord web: older messages: Discord's action was not found; reading goes on without it");
                    }
                    return;
                }
                if let Some(host) = &self.host {
                    host.send(Command::Page(PageOp::Older));
                }
            }
            DirectAsk::Send { channel, text, reply, nonce } => {
                let at = now_ms();
                let outs = if self.acting() {
                    self.acts.send(&channel, text.as_str(), reply.as_ref(), nonce, at)
                } else {
                    self.acts.refuse(channel.as_str(), text.as_str(), nonce, at, "not signed in")
                };
                self.apply(outs);
                self.drive();
            }
            DirectAsk::Forget { channel, message } => {
                let outs = self.acts.forget(&channel, &message);
                self.apply(outs);
            }
            DirectAsk::React { channel, message, emoji, on } => {
                if self.acting() {
                    self.acts.react(&channel, &message, emoji.as_str(), on);
                    self.drive();
                } else {
                    say("discord web: reaction not done: not signed in");
                }
            }
            DirectAsk::MarkRead(channel) => {
                if self.acting() {
                    self.acts.mark_read(&channel);
                    self.drive();
                } else {
                    say("discord web: mark read not done: not signed in");
                }
            }
            other => {
                let kind = match other {
                    DirectAsk::Call(_) => "call",
                    DirectAsk::Answer(_) => "answer",
                    DirectAsk::Decline(_) => "decline",
                    _ => return,
                };
                if self.unbuilt.insert(kind) {
                    say(&format!("discord web: {kind} not built yet"));
                }
            }
        }
    }

    /// Whether the page can be acted through: a web view that is up, signed in, with its stores read.
    fn acting(&self) -> bool {
        self.host.is_some() && !self.ending && self.direct_state() == DirectState::Ready
    }

    /// Does what the queue says: asks the host, tells the screens, writes the log. Then what the screens show.
    fn apply(&mut self, outs: Vec<act::Out>) {
        let changed = !outs.is_empty();
        for out in outs {
            match out {
                act::Out::Op(op) => {
                    if let Some(host) = &self.host {
                        host.send(Command::Act(op));
                    }
                }
                act::Out::Type { text, clear } => {
                    if let Some(host) = &self.host {
                        host.send(Command::Type { text, clear });
                    }
                }
                act::Out::Event(event) => self.events.push(event),
                act::Out::Log(line) => say(&line),
                act::Out::Changed => {}
            }
        }
        if changed {
            self.refresh_direct();
        }
    }

    /// The queue's clock: a timeout, or the next action's turn. Each frame, from [`Session::pump`].
    fn drive(&mut self) {
        let outs = self.acts.tick(std::time::Instant::now());
        self.apply(outs);
    }

    /// How the screens' `Direct` follows the web session: Off, no runtime, loading while a web view checks or the
    /// list is not read, signed out, unreachable when it failed, the page changed when the health probe says so.
    fn direct_state(&self) -> DirectState {
        match self.web.state {
            WebState::Off => DirectState::Off,
            WebState::NoRuntime => DirectState::NoRuntime,
            WebState::Checking => DirectState::Loading,
            WebState::SignedOut | WebState::SigningIn => DirectState::SignedOut,
            WebState::Failed => DirectState::Unreachable,
            WebState::SignedIn if self.reader.page_changed() => DirectState::PageChanged,
            WebState::SignedIn if self.reader.dms.is_some() => DirectState::Ready,
            WebState::SignedIn => DirectState::Loading,
        }
    }

    /// Forgets every DM, message and picture held, and the open conversation. The page's script keeps nothing the
    /// Dashboard needs: a new session asks it to send everything again.
    fn forget_direct(&mut self) {
        self.reader.forget();
        self.pictures.forget();
        self.open = None;
        self.events.clear();
        // What was queued is not sent and what ran is let go; each send is told so (after the events were cleared: the
        // screens still hear of what did not go).
        let outs = self.acts.reset();
        self.apply(outs);
        self.refresh_direct();
    }

    /// Builds what the screens see from what the page said and the pictures that have come, asks for the pictures that
    /// are wanted (the first 40 DMs', the open conversation's, a ring's) and notes a change.
    fn refresh_direct(&mut self) {
        use model::discord_dms::{Conversation, Dm, Id, Private, Ring};
        const EAGER: usize = 40;
        let state = self.direct_state();
        let mut direct = Direct { state, ..Direct::default() };
        if state == DirectState::Ready {
            let dms = self.reader.dms.as_deref().unwrap_or(&[]);
            for dm in dms.iter().take(EAGER) {
                self.pictures.want(&dm.pic);
            }
            if let Some(conv) = self.reader.conv.as_ref().filter(|c| c.loaded) {
                if let Some(dm) = self.reader.dm(&conv.id) {
                    self.pictures.want(&dm.pic);
                }
                for m in &conv.msgs {
                    self.pictures.want(&m.author.pic);
                }
            }
            for r in &self.reader.rings {
                self.pictures.want(&r.pic);
            }
        }
        if state == DirectState::Ready {
            let pic = |path: &str| self.pictures.get(path);
            let dms = self.reader.dms.as_deref().unwrap_or(&[]);
            direct.dms = dms
                .iter()
                .map(|d| Dm {
                    id: Id(d.id.clone()),
                    name: Private(d.name.clone()),
                    group: d.group,
                    members: d.members,
                    picture: pic(&d.pic),
                    unread: d.unread,
                    last: d.last.as_ref().filter(|l| l.at > 0).map(|l| {
                        let line = l.text.as_deref().map(|t| direct::first_line(t, &l.names)).unwrap_or_default();
                        (Private(line), l.at)
                    }),
                })
                .collect();
            direct.rings = self
                .reader
                .rings
                .iter()
                .map(|r| Ring {
                    channel: Id(r.channel.clone()),
                    caller: Private(r.caller.clone()),
                    picture: pic(&r.pic),
                })
                .collect();
            if let Some(conv) =
                self.reader.conv.as_ref().filter(|c| c.loaded && self.open.as_ref().is_some_and(|o| o.as_str() == c.id))
            {
                let dm = self.reader.dm(&conv.id);
                let mut messages: Vec<_> = conv.msgs.iter().map(|m| direct::message(m, &pic)).collect();
                self.acts.overlay(&conv.id, &mut messages);
                direct.open = Some(Conversation {
                    id: Id(conv.id.clone()),
                    name: Private(dm.map(|d| d.name.clone()).unwrap_or_default()),
                    group: dm.is_some_and(|d| d.group),
                    picture: dm.and_then(|d| pic(&d.pic)),
                    messages,
                    loading_older: conv.older,
                    at_start: !conv.more,
                    unread_from: (!conv.unread_from.is_empty()).then(|| Id(conv.unread_from.clone())),
                });
            }
        }
        if direct != self.direct {
            self.direct = direct;
            self.direct_changed = true;
        }
    }

    /// A web notification the host took. Hook for the mention Toast (#202): a notification whose route is a server's
    /// channel is a `DirectEvent::Mention { name, channel_name }`. What Discord puts in a notification's title, body and
    /// tag has not been seen on the TV PC, so none is guessed at: nothing is raised until that is known. The host
    /// logs each one's lengths (below) so the first run shows what there is to go on.
    fn notification_event(&self, _notified: &Notified) -> Option<DirectEvent> {
        None
    }

    /// How the session stands, for the screens.
    pub fn web(&self) -> Web {
        self.web
    }

    /// Web views started this run.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    fn folder(&self) -> Option<PathBuf> {
        Some(folder(self.data.as_deref()?, self.profile?))
    }

    /// Settings' switch. Off ends the host thread; the session stays kept for when it is on again. On is the user's
    /// own: after a failure it may try once more, and the runtime is asked again.
    pub fn set_on(&mut self, on: bool) {
        if self.on == on {
            return;
        }
        self.on = on;
        say(&format!("discord web: switched {}", if on { "on" } else { "off" }));
        if on {
            self.runtime = None;
        }
        self.follow(By::User);
    }

    /// The Profile signed in: its own session, another's host ended first.
    pub fn set_profile(&mut self, profile: Option<i64>) {
        if self.profile == profile {
            return;
        }
        self.profile = profile;
        self.follow(By::Itself);
    }

    /// Ends a host that is no longer wanted, and starts one where there is something to do: a session is kept to
    /// check and follow, or the sign-in page is up. Signed out with the page closed, nothing runs and nothing is on
    /// the PC. Called only on a change (the switch, the Profile, the sign-in page, a host ended): never each frame.
    fn follow(&mut self, by: By) {
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
        // A failure rests until the user acts.
        if self.web.state == WebState::Failed && by == By::Itself {
            return;
        }
        if self.web.kept || self.signing_in {
            self.start(profile, folder, by);
            return;
        }
        let runtime = self.runtime.get_or_insert_with(|| self.backend.runtime()).clone();
        match runtime {
            Ok(_) => self.set_state(WebState::SignedOut),
            Err(e) => {
                if self.web.state != WebState::NoRuntime {
                    say(&format!("discord web: no runtime: {e}"));
                }
                self.set_state(WebState::NoRuntime);
            }
        }
    }

    /// Whether the sign-in page is up (shown, or under text entry). Its opening is the user's Sign in. Closed before
    /// the session signed in, what the web view made is removed: a sign-in given up keeps nothing.
    pub fn set_signing_in(&mut self, up: bool) {
        if self.signing_in == up {
            return;
        }
        self.signing_in = up;
        if up {
            self.follow(By::User);
        } else if self.web.state != WebState::SignedIn && self.host.is_some() {
            say("discord web: the sign-in page closed before signing in: nothing is kept");
            self.sign_out();
        }
    }

    /// Starts the one host thread, if none is up or ending and the failures allow it. One log line an attempt.
    fn start(&mut self, profile: i64, folder: PathBuf, by: By) {
        if self.host.is_some() || self.ending {
            return;
        }
        if self.failures >= MOST_FAILURES {
            if !self.gave_up {
                self.gave_up = true;
                say(&format!(
                    "discord web: gave up: the web view failed to open {MOST_FAILURES} times; it is tried again when the \
                     Dashboard starts again"
                ));
            }
            return;
        }
        if self.failures > 0 && by == By::Itself {
            self.set_state(WebState::Failed);
            return;
        }
        self.attempts += 1;
        let (commands, receive) = mpsc::channel();
        let report = self.report.clone();
        let wake = self.wake.clone();
        let report = std::sync::Mutex::new(report);
        let tell: Tell = Arc::new(move |r| {
            let _ = report.lock().unwrap_or_else(|p| p.into_inner()).send(r);
            wake();
        });
        say(&format!("discord web: opening the web view (attempt {})", self.attempts));
        match self.backend.spawn(folder, self.base.clone(), receive, tell) {
            Ok(spawned) => {
                self.host = Some(Host { commands, poke: spawned.poke, thread: Some(spawned.thread), profile });
                self.set_state(WebState::Checking);
            }
            Err(e) => {
                self.failures += 1;
                say(&format!("discord web: no runtime: {e}"));
                self.set_state(WebState::NoRuntime);
            }
        }
    }

    fn set_state(&mut self, state: WebState) {
        if self.web.state != state {
            say(&format!("discord web: {}", state.word()));
            self.web.state = state;
            match state {
                // Nothing of Discord's is kept past the session being up: forgotten on every way out of it.
                WebState::Off | WebState::NoRuntime | WebState::SignedOut | WebState::SigningIn | WebState::Failed => {
                    self.forget_direct();
                }
                // The page may have pushed before it was known to be signed in: asked again, so nothing is missed.
                WebState::SignedIn => {
                    if let Some(host) = &self.host {
                        host.send(Command::Page(PageOp::Resend));
                    }
                }
                WebState::Checking => {}
            }
        }
        if state != WebState::SigningIn {
            self.web.field = None;
        }
        self.refresh_direct();
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
            Some(_) => {}
            None => {
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

    /// Takes in what the host thread said. True when what the screens show changed. Nothing here starts a web view
    /// after a failure: only a host ended on purpose (another Profile's, a sign-out) is followed by another.
    pub fn pump(&mut self) -> bool {
        let before = self.web;
        while let Ok(report) = self.reports.try_recv() {
            match report {
                Report::State(WebState::Failed) => {
                    self.failures += 1;
                    self.set_state(WebState::Failed);
                }
                Report::State(state) => {
                    // It opened: the failures before it no longer count.
                    self.failures = 0;
                    // A page that is up is signing in until it is signed in.
                    let state =
                        if self.shown.is_some() && state == WebState::SignedOut { WebState::SigningIn } else { state };
                    self.set_state(state);
                    if state == WebState::SignedIn {
                        self.web.kept = true;
                    }
                }
                Report::Page(json) => {
                    if self.host.is_none() {
                        continue;
                    }
                    let taken = self.reader.take(&json, self.open.as_ref().map(|o| o.as_str()));
                    for line in &taken.log {
                        say(line);
                    }
                    if taken.reset {
                        self.forget_direct();
                    }
                    if let Some(page_act) = taken.act {
                        let outs = self.acts.page(&page_act, std::time::Instant::now());
                        self.apply(outs);
                    }
                    for event in taken.events {
                        let event = self.with_picture(event);
                        self.events.push(event);
                    }
                    if taken.changed {
                        self.refresh_direct();
                    }
                }
                Report::Notified(notified) => {
                    self.notifications += 1;
                    if self.notifications == 1 {
                        say(&format!(
                            "discord web: a web notification was taken ({notified:?}); Windows shows nothing"
                        ));
                    }
                    if let Some(event) = self.notification_event(&notified) {
                        self.events.push(event);
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
                    // Its last word: the thread is joined before anything else may start.
                    if let Some(thread) = self.host.take().and_then(|mut host| host.thread.take()) {
                        let _ = thread.join();
                    }
                    let on_purpose = std::mem::take(&mut self.ending);
                    if let (Some(removed), Some(folder)) = (signed_out, self.folder()) {
                        self.signed_out(&folder, removed);
                    }
                    if on_purpose {
                        let shown = self.shown.take();
                        self.follow(By::Itself);
                        if self.host.is_some() {
                            self.show(shown);
                        } else {
                            self.shown = shown;
                        }
                    } else if self.web.state != WebState::Failed {
                        // Ended of itself (its window closed): a failure, resting.
                        self.failures += 1;
                        self.set_state(WebState::Failed);
                    }
                }
            }
        }
        self.drive();
        if self.pictures.pump() {
            self.refresh_direct();
        }
        let failed = self.pictures.take_failures();
        if !failed.is_empty() {
            say(&format!("discord web: {} pictures could not be fetched ({})", failed.len(), failed[0]));
        }
        before != self.web || std::mem::take(&mut self.direct_changed)
    }

    /// The event with the picture it names, as far as it has come.
    fn with_picture(&self, event: DirectEvent) -> DirectEvent {
        match event {
            DirectEvent::Message { channel, name, group, line, .. } => {
                let picture = self.reader.dm(channel.as_str()).and_then(|d| self.pictures.get(&d.pic));
                DirectEvent::Message { channel, name, group, line, picture }
            }
            DirectEvent::Ring(mut ring) => {
                ring.picture = self
                    .reader
                    .rings
                    .iter()
                    .find(|r| r.channel == ring.channel.as_str())
                    .and_then(|r| self.pictures.get(&r.pic));
                DirectEvent::Ring(ring)
            }
            other => other,
        }
    }
}

/// The Dashboard is closing: the web view is closed properly, so what the browser has of the session is written out,
/// and the session stays kept. Waited for, a few seconds at most.
impl Drop for Session {
    fn drop(&mut self) {
        let Some(mut host) = self.host.take() else { return };
        if !self.ending {
            host.send(Command::Stop);
        }
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while let Some(left) = until.checked_duration_since(std::time::Instant::now()) {
            match self.reports.recv_timeout(left) {
                Ok(Report::Ended { .. }) => {
                    if let Some(thread) = host.thread.take() {
                        let _ = thread.join();
                    }
                    break;
                }
                Err(_) => break,
                Ok(_) => {}
            }
        }
    }
}

/// Unix milliseconds now: the time a message the user sent shows as, until Discord gives its own.
fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

pub(crate) fn say(line: &str) {
    #[cfg(test)]
    if tests::capture(line) {
        return;
    }
    log_line::write_line(format_args!("{line}"));
}

#[cfg(test)]
mod direct_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    thread_local! {
        /// The log lines this thread's `say` collects while a test captures them, instead of writing them.
        static LINES: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
    }

    /// Whether a test is capturing the log: then the line is kept for it.
    pub(crate) fn capture(line: &str) -> bool {
        LINES.with(|lines| match lines.borrow_mut().as_mut() {
            Some(lines) => {
                lines.push(line.to_string());
                true
            }
            None => false,
        })
    }

    /// Starts capturing this thread's log lines.
    pub(crate) fn capturing() {
        LINES.with(|lines| *lines.borrow_mut() = Some(Vec::new()));
    }

    /// The lines captured so far.
    pub(crate) fn captured() -> Vec<String> {
        LINES.with(|lines| lines.borrow().clone().unwrap_or_default())
    }

    #[test]
    fn the_sign_in_page_is_zoomed_as_far_as_keeps_the_qr_panel() {
        let tv = Area { x: 0, y: 0, width: 3840, height: 1940 };
        assert_eq!(zoom(tv, 2.0), 2.2, "a 4K TV at 200%");
        assert_eq!(zoom(Area { width: 1920, height: 970, ..tv }, 1.0), 2.2, "1080 at 100%: the same");
        assert_eq!(zoom(Area { width: 1920, height: 970, ..tv }, 1.5), 1.46);
        assert_eq!(zoom(Area { width: 800, height: 400, ..tv }, 1.0), 1.0, "never below 1");
        assert_eq!(zoom(tv, 0.0), 4.4, "no scale known: 1");
        for (area, raster) in [(tv, 2.0), (Area { width: 1920, height: 970, ..tv }, 1.25)] {
            let z = zoom(area, raster) * raster;
            assert!(area.width as f64 / z >= VIEW_MIN.0 && area.height as f64 / z >= VIEW_MIN.1);
        }
    }

    fn session(data: &Path) -> Session {
        let mut session = Session::new(None, Some(data), || {});
        session.set_profile(Some(7));
        session
    }

    /// A web view that never opens: each start reports failed and ends at once, as the TV PC's did without COM.
    struct Failing {
        spawned: Arc<std::sync::atomic::AtomicU32>,
    }

    impl Backend for Failing {
        fn runtime(&self) -> Result<String, String> {
            Ok("157.0.0.0 (fake)".into())
        }

        fn spawn(&self, _: PathBuf, _: String, _: Receiver<Command>, tell: Tell) -> Result<Spawned, String> {
            self.spawned.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tell(Report::State(WebState::Failed));
            tell(Report::Ended { signed_out: None });
            Ok(Spawned { poke: Box::new(|| {}), thread: std::thread::spawn(|| {}) })
        }
    }

    fn failing(data: &Path) -> (Session, Arc<std::sync::atomic::AtomicU32>) {
        let spawned = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let backend = Box::new(Failing { spawned: spawned.clone() });
        let mut session = Session::with(None, Some(data), || {}, backend);
        session.set_profile(Some(7));
        (session, spawned)
    }

    /// What the Dashboard does each frame (`App::follow_discord_web`), with the sign-in page up and shown.
    fn frames(session: &mut Session, n: usize, signing_in: bool) {
        for i in 0..n {
            session.set_profile(Some(7));
            session.set_signing_in(signing_in);
            session.show(signing_in.then_some(Area { x: 0, y: 0, width: 1920, height: 970 + (i % 2) as i32 }));
            session.pump();
        }
    }

    fn started(spawned: &std::sync::atomic::AtomicU32) -> u32 {
        spawned.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn a_failure_rests_however_many_frames_follow() {
        let data = tempfile::tempdir().unwrap();
        let (mut session, spawned) = failing(data.path());
        session.set_on(true);
        assert_eq!((started(&spawned), session.web().state), (0, WebState::SignedOut), "nothing kept: nothing runs");
        frames(&mut session, 1000, true);
        assert_eq!(started(&spawned), 1, "Sign in opened one web view, and its failure started no other");
        assert_eq!(session.web().state, WebState::Failed);
        assert!(session.host.is_none(), "its thread was joined and let go");
    }

    #[test]
    fn sign_in_pressed_again_is_one_more_attempt_until_the_cap() {
        let data = tempfile::tempdir().unwrap();
        let (mut session, spawned) = failing(data.path());
        session.set_on(true);
        for attempt in 1..=MOST_FAILURES {
            frames(&mut session, 200, true);
            assert_eq!(started(&spawned), attempt);
            frames(&mut session, 200, false);
            assert_eq!(started(&spawned), attempt, "the page closed starts nothing");
        }
        // The cap: no more this run, however the user asks.
        frames(&mut session, 200, true);
        frames(&mut session, 10, false);
        session.set_on(false);
        session.set_on(true);
        frames(&mut session, 200, true);
        assert_eq!(started(&spawned), MOST_FAILURES);
        assert!(session.gave_up);
    }

    #[test]
    fn a_kept_session_that_fails_is_not_tried_again_by_itself() {
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(folder(data.path(), 7)).unwrap();
        std::fs::write(folder(data.path(), 7).join("Local State"), b"made up").unwrap();
        let (mut session, spawned) = failing(data.path());
        session.set_on(true);
        frames(&mut session, 500, false);
        assert_eq!(started(&spawned), 1);
        // Another Profile and back: still resting.
        session.set_profile(Some(8));
        session.set_profile(Some(7));
        frames(&mut session, 500, false);
        assert_eq!(started(&spawned), 1);
        // The switch turned on again is the user's: one more.
        session.set_on(false);
        session.set_on(true);
        frames(&mut session, 500, false);
        assert_eq!(started(&spawned), 2);
    }

    #[test]
    fn the_runtime_is_asked_once_a_run_not_each_frame() {
        struct Counting(Arc<std::sync::atomic::AtomicU32>);
        impl Backend for Counting {
            fn runtime(&self) -> Result<String, String> {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err("no runtime (fake)".into())
            }
            fn spawn(&self, _: PathBuf, _: String, _: Receiver<Command>, _: Tell) -> Result<Spawned, String> {
                Err("no runtime (fake)".into())
            }
        }
        let data = tempfile::tempdir().unwrap();
        let asked = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut session = Session::with(None, Some(data.path()), || {}, Box::new(Counting(asked.clone())));
        session.set_profile(Some(7));
        session.set_on(true);
        frames(&mut session, 500, false);
        assert_eq!((started(&asked), session.web().state), (1, WebState::NoRuntime));
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
