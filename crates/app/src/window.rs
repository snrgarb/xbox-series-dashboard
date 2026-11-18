//! The windows: Home's, borderless fullscreen on the primary monitor or 1280x720 with `--windowed`, the Overlay
//! above every other window, which shows what the Dashboard wants over a game, and the Toast strip above that
//! (docs/ui.md). This feeds the Dashboard the pad, the keyboard and the time, draws each window when what it shows
//! has changed, and carries out what the Dashboard asks of the shell.
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use input::Pad;
use library::art::Event as ArtEvent;
use model::{ArtRefresh, GameKey, Library, Sound};
use render::gpu::{self, Gpu};
use screens::keyboard::Typed;
use screens::toasts::Seen;
use screens::{Dashboard, Effect, Options, OverlayShows, Phase, Setting};
use shell::audio::{self, AudioControl, SystemAudio};
use shell::idle::LastInput;
use shell::media::{self, MediaSessions};
use shell::power::{DryPower, Power, SystemPower};
use shell::store::{Addresses, Ask, Asking, ProgramAddresses, SystemAddresses, Word};
use shell::wake::{SystemWakes, Wakes};
use shell::{Freezer, Key as OverlayKey, Launch, Mode, Overlay, Step, ToastStrip};
use ui::{Button, Host, Press};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::sounds::Sounds;
use crate::startup::Playing;
use crate::store::Store;
use crate::turns::{LateFrames, Ledger, Part, Totals};
use crate::{since_start, Args};

/// What an art lookup says, with the number of the run that said it.
type RunEvent = (u64, ArtEvent);

/// What the art picker's network said (docs/spec/art-picker.md), with the number of the picker it was for: one that
/// has closed says nothing to the next. A chosen picture's download is told whatever picker it came from.
enum PickerEvent {
    Answer(u64, model::art_picker::Answer),
    Downloaded(model::art_picker::Choice, Result<Vec<u8>, String>),
}

/// How long the loop sleeps with nothing to draw: the pad's thread, the art cache and the window wake it sooner.
const IDLE: Duration = Duration::from_millis(250);
/// How soon the Overlay is tried again when it had no frame to draw on.
const OVERLAY_RETRY: Duration = Duration::from_millis(16);
/// The Overlay is made once Home has come to rest after its first frame, or this long after it, whichever is first.
const OVERLAY_AFTER: Duration = Duration::from_secs(2);
/// A frame of the Overlay that took longer than this was drawn by a slow renderer: the processor, where there is no
/// GPU. With vsync a GPU's takes a refresh at most.
const SLOW_FRAME: Duration = Duration::from_millis(50);
/// What a slow renderer is held to while only the Spinner turns: a frame, then this many frames' time left to the
/// game that is starting.
const SLOW_FRAME_REST: u32 = 3;

/// How long Home's window is kept waiting, at most, by a Toast on a slow renderer.
const HOME_HELD: Duration = Duration::from_millis(1500);

/// The seconds since the Dashboard was last told the time, which it is being told now.
fn told(last: &mut Instant) -> f32 {
    let now = Instant::now();
    (now - std::mem::replace(last, now)).as_secs_f32()
}

/// When the Overlay may next be drawn while only the Spinner turns, after a frame the loop began at `began` and
/// was `took` over, all of it: the wait for the swap chain to have a frame to draw on, the drawing, the present.
/// The wait is where a slow renderer's time shows: the frame is drawn after it is handed over, and the next cannot
/// be had until it is, so the loop stands still there and not in the drawing. A frame timed without it looks quick
/// however slow it was, the next follows at once, and the loop does nothing but frames: a launch then looks at the
/// desktop once a frame, every look late (`shell::launch`, LATE), and a window that wants the user is never seen
/// to. Timed with it, a slow frame is followed by its rest, in which the loop goes round as often as it is asked.
fn overlay_due(began: Instant, took: Duration) -> Instant {
    began + if took > SLOW_FRAME { took * (1 + SLOW_FRAME_REST) } else { took }
}

struct State {
    host: Host,
    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    gpu: Gpu,
    /// When the library, the window, the GPU and the renderer were each ready, in ms since the process started.
    ready: [f64; 4],
    // Last, so it outlives the surface made from it.
    window: Arc<Window>,
}

struct App {
    windowed: bool,
    /// Shell mode, the Dashboard's side of it (docs/spec/shell-mode.md, 6).
    as_shell: crate::as_shell::AsShell,
    dashboard: Dashboard,
    pad: Pad,
    /// Keys pressed since the pad was last looked at, as the buttons they stand for, or as the text they type.
    keys: Vec<KeyPress>,
    /// The Overlay, made once Home's first frame is out. None before, anywhere but Windows, and where it could not
    /// be made. Before `state`, so it is dropped before the device it draws with.
    overlay: Option<Overlay>,
    overlay_tried: bool,
    /// The Overlay's own animated values: its frames are drawn by Home's renderer from Home's art.
    overlay_store: ui::Store,
    overlay_last_frame: Instant,
    /// The Overlay wants a frame whatever else: it was just shown, a button was pressed, art came in.
    overlay_stale: bool,
    /// Whether the Overlay's last frame left something moving: the next one is then that much later in its
    /// animations, however long it took.
    overlay_moving: bool,
    /// The soonest the Overlay is drawn again for the Spinner alone: at once, but on a slow renderer.
    overlay_next: Instant,
    /// The Toast strip, made with the Overlay. None before, anywhere but Windows, and where it could not be made:
    /// Toasts are then drawn in Home's window. Before `state`, as the Overlay is.
    toast_strip: Option<ToastStrip>,
    /// The strip's own animated values, as the Overlay has its own.
    toast_store: ui::Store,
    toast_last_frame: Instant,
    /// The strip wants a frame whatever else: it was just shown, the Toast on it changed.
    toasts_stale: bool,
    /// When the Dashboard was last told how much time had passed.
    told_time: Instant,
    /// `--toast`'s notifications, until Toasts have somewhere to show and the greeting has been asked for.
    toasts_to_post: Option<Vec<String>>,
    /// What the log last said of the Toasts: how many had been seen, and what the screen had of them.
    toasts_said: (u64, Seen),
    /// The renderer is the processor: there is no GPU. Its frames are slow whatever the last one took.
    cpu_renderer: bool,
    /// With --allow-toast-posts: other programs may post notifications to the strip's window.
    toast_posts: bool,
    /// How long Home's last frame took to draw, and when it was done: a slow one waits while a Toast moves.
    home_took: Duration,
    home_done: Instant,
    state: Option<State>,
    proxy: EventLoopProxy<()>,
    last_frame: Instant,
    /// A frame is wanted whatever else: the first, or after a resize.
    stale: bool,
    /// When the window's first frame was presented: Start-up's, or Home's with none. The sources are read from
    /// then on, behind the Boot video.
    presented: Option<Instant>,
    /// When Home's first frame was presented, once Start-up is over: the Overlay, the greeting and the art
    /// lookup wait for it.
    home_presented: Option<Instant>,
    /// The Boot video, while it plays (docs/spec/start-up.md).
    boot_video: Option<Playing>,
    /// Where Start-up was as the log last said it (`startup: who's playing shown ...`, `signing in`, `signed in`),
    /// by the frames presented.
    startup_said: Option<Phase>,
    /// What says the machine has woken from sleep: Who's playing today? asks again (docs/spec/start-up.md, 8).
    wakes: Box<dyn Wakes>,
    launch: Option<Launch>,
    /// When Home's window is drawn while it is a game's backdrop (docs/spec/launch-screen.md).
    backdrop: crate::backdrop::Frames,
    /// Holds the running game still while the Guide is open over it, and lets it go as the Guide hands the
    /// foreground back (docs/spec/guide.md, 7.2). After `launch` here, so it outlives it.
    freezer: Freezer,
    /// Since when the Guide has been closed with the button that closed it still down.
    release_wait: Option<Instant>,
    /// Who gets the foreground once the Guide has let go of it, when that is not the window that had it as the
    /// Guide opened (docs/spec/guide.md, 3.3).
    hand_back: Option<HandBack>,
    /// The library database: where a newer snapshot comes from, and where Play sessions go.
    store: Store,
    /// When the library had been read, in ms since the process started.
    library_ready: f64,
    sounds: Sounds,
    /// Whether Home's last frame left something moving.
    moving: bool,
    /// With --log-focus: say where focus went, on Home or on the page over it (`Dashboard::focus_line`) and in the
    /// Guide (`Dashboard::guide_line`), for a test that must know before it presses on.
    log_focus: bool,
    /// The Profile signed in as the log last said it (`profile: ...`), so that it is said again only as it changes.
    profile_said: String,
    /// What turns the machine off, restarts it and puts it to sleep: for real, or with --dry-power in the log only.
    power: Box<dyn Power>,
    /// What opens a store's address for a game, to install or uninstall it.
    addresses: Box<dyn Addresses>,
    /// What a store's installer, on its own thread, has for the user (#230): a licence agreement to answer, or an
    /// install stopped. Each agreement asked waits on its reply here, by the number its prompt was opened with.
    words: Receiver<Word>,
    agreements: std::collections::HashMap<u64, Sender<model::Reply>>,
    agreements_asked: u64,
    /// The system's sound, which the Guide's Audio Flyout shows and changes.
    audio: Box<dyn AudioControl>,
    /// When it was last read while the Audio Flyout is open: it is read as the Flyout opens and every `AUDIO_EVERY`
    /// after, so that the Flyout follows what is changed elsewhere.
    audio_read: Option<Instant>,
    /// Windows' media sessions, which the Audio Flyout's Now Playing card shows, followed on a thread of their own.
    media: Box<dyn MediaSessions>,
    /// What --log-focus last said of the card (`now playing: ...`): said again only as it changes.
    now_playing_said: Option<String>,
    /// How long the sound of leaving has still to play: Exit waits for it.
    leaving: Duration,
    /// The minute of the day Home's last frame showed on its clock.
    minute: u64,
    /// With --log-frames: the frames Home's window, the Overlay and the Toast strip drew since they were last
    /// said, and when.
    frames: Option<([u32; 3], Instant)>,
    /// With --log-frames: how long Home's frames and the Overlay's took, since they were last said.
    frame_times: [FrameTimes; 2],
    /// Where the loop's thread spends its time, part by part (#99); what had been spent when Home's last frame and
    /// the Overlay's had their buffers; what each one's last draw spent its time on; and the late frames said.
    ledger: Ledger,
    frame_spent: [Totals; 2],
    draw_cost: [ui::FrameCost; 2],
    late: LateFrames,
    /// A Game hub's opening, timed from A to its first frame and its last moving one, always (#214).
    hub_open: crate::hub_open::HubOpen,
    /// The art lookup under way, if one is (docs/sources.md, "Art"): dropped, it stops.
    art: Option<library::art::Lookup>,
    /// It has been started once the sources were read: a new key starts it again, nothing else does.
    art_started: bool,
    /// What it says as it goes, for the loop to take in, with the number of the run that said it: a run stopped
    /// for another (a new key, a refresh) still says it is done, and that is not the new run's end.
    art_events: (Sender<RunEvent>, Receiver<RunEvent>),
    /// The number of the run under way, or of the last one.
    art_run: u64,
    /// The game whose whole list of achievements an Achievements page was last given, and whether it should be
    /// read again (new icons for it, or a new snapshot).
    achievements_listed: (Option<i64>, bool),
    /// The art picker's network while a picker is up, and the number of the picker it is for.
    picker: Option<(u64, library::art::Picker)>,
    picker_run: u64,
    picker_events: (Sender<PickerEvent>, Receiver<PickerEvent>),
    /// When the keyboard or the mouse was last used anywhere in Windows: the Idle screen counts from it, and from
    /// the pads' own (docs/spec/idle-screen.md, 2).
    input_clock: Box<dyn LastInput>,
    /// `--idle-after`'s times: in force in place of Settings' whatever Settings says, and never saved.
    idle_after: Option<screens::idle::Times>,
    /// The Discord connection (docs/discord.md): Discord's own pipe, or with --discord-pipe a test's.
    discord: discord::Connection,
    /// Settings > About's updates and the daily check (docs/spec/updates.md).
    updater: crate::updates::Updater,
    /// Settings > Helpers, carried out.
    helpers: crate::helper_programs::Helpers,
    /// Settings > Set up this PC, carried out (#178).
    set_up: crate::set_up::SetUp,
}

/// What a Toast says when SteamGridDB refuses the key (a 401): once, and no lookups until another key is entered.
const KEY_REFUSED: &str =
    "SteamGridDB didn't accept your key, so no art is being downloaded. Check it in Settings, Games & apps, Art.";

/// How long a window's frames took, for --log-frames: the wait for the swap chain to give a frame to draw on, the
/// drawing and the present, and the time from one frame to the next while something moved.
///
/// A frame's time is when it got its buffer (`acquired`): its picture is fixed then, by the clock the animations
/// read (`overlay_last_frame`, `last_frame`), and presented a draw later. The gap to the frame before is counted
/// from that frame's own buffer, not from the start of its turn: the wait before the first frame of a stretch (the
/// Overlay just shown, Home starting to move from rest) is how late the stretch starts, not a gap in its movement,
/// and is said apart, under `from rest`. A frame that follows one that left nothing moving comes after a rest, and
/// is not late.
#[derive(Default)]
struct FrameTimes {
    frames: u32,
    acquire: Duration,
    acquire_most: Duration,
    draw: Duration,
    draw_most: Duration,
    /// The frames that followed one that left something moving, and the time they took in all.
    moving: u32,
    moving_time: Duration,
    gap_most: Duration,
    /// How much of the longest gap was that frame's own wait for its buffer.
    gap_most_wait: Duration,
    /// Of those, the frames that got their buffer more than 12 ms after the one before: short of 90 a second.
    late: u32,
    /// The frames that came after a rest (the first of a stretch, or one alone), and the longest of their waits.
    from_rest: u32,
    from_rest_wait_most: Duration,
    /// When the last frame got its buffer.
    last: Option<Instant>,
}

impl FrameTimes {
    /// Ignoring 12 ms or less, at 120 Hz a refresh and a half: a gap over it is a refresh missed.
    const LATE: Duration = Duration::from_millis(12);

    /// Counts a frame; the gap before it, if it was late.
    fn frame(&mut self, began: Instant, acquired: Instant, done: Instant, was_moving: bool) -> Option<Duration> {
        self.frames += 1;
        let mut late = None;
        let (acquire, draw) = (acquired - began, done - acquired);
        self.acquire += acquire;
        self.acquire_most = self.acquire_most.max(acquire);
        self.draw += draw;
        self.draw_most = self.draw_most.max(draw);
        match self.last.filter(|_| was_moving) {
            Some(last) => {
                let gap = acquired - last;
                self.moving += 1;
                self.moving_time += gap;
                if gap > self.gap_most {
                    (self.gap_most, self.gap_most_wait) = (gap, acquire);
                }
                if gap > Self::LATE {
                    self.late += 1;
                    late = Some(gap);
                }
            }
            None => {
                self.from_rest += 1;
                self.from_rest_wait_most = self.from_rest_wait_most.max(acquire);
            }
        }
        self.last = Some(acquired);
        late
    }

    /// What they were, and forgets them.
    fn said(&mut self) -> String {
        let ms = |time: Duration| time.as_secs_f32() * 1000.0;
        let each = self.frames.max(1);
        let said = format!(
            "wait {:.1} ms (most {:.1}), draw {:.1} ms (most {:.1}); moving: {} frames at {:.0} a second, longest gap \
             {:.1} ms ({:.1} of it its wait), {} late; from rest: {} frames, their wait most {:.1} ms",
            ms(self.acquire / each),
            ms(self.acquire_most),
            ms(self.draw / each),
            ms(self.draw_most),
            self.moving,
            self.moving as f32 / self.moving_time.as_secs_f32().max(f32::EPSILON),
            ms(self.gap_most),
            ms(self.gap_most_wait),
            self.late,
            self.from_rest,
            ms(self.from_rest_wait_most)
        );
        *self = FrameTimes { last: self.last, ..FrameTimes::default() };
        said
    }
}

/// A key of the keyboard, as the Dashboard is given it.
enum KeyPress {
    /// A key that stands for a button of the pad: the arrows, Enter, Escape.
    Button(Press),
    /// Backspace: it deletes where text is being typed, and is B anywhere else.
    Backspace { repeat: bool },
    /// What a key types, for a page that takes text (text entry). Nothing anywhere else.
    Text(String),
}

/// Who the Guide hands the foreground to as it closes over a running game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandBack {
    /// Home's window, in front of the game: Home or a page was chosen.
    Home,
    /// The game's own window: the running game was chosen.
    Game,
}

/// The minute it is, counted from 1970: what the clock's next change is told by.
fn minute() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() / 60)
}

/// The longest Exit waits for the sound of leaving.
const LEAVING_AT_MOST: Duration = Duration::from_secs(4);

/// How often the system's sound is read while the Audio Flyout is open.
const AUDIO_EVERY: Duration = Duration::from_millis(250);

/// What Home's last draw and the Overlay's spent their time on, by the part each one's drawing is billed to: for a
/// late frame's line.
fn draws(cost: &[ui::FrameCost; 2]) -> [(&'static str, Part, &ui::FrameCost); 2] {
    [("home", Part::HomeDraw, &cost[0]), ("overlay", Part::OverlayDraw, &cost[1])]
}

/// How often --log-frames says what was drawn.
const FRAMES_EVERY: Duration = Duration::from_secs(5);

/// Configures `surface` for `window`: its size, a format that isn't sRGB, vsync. Windows' refusal of the swap chain
/// comes back as an error, caught by an error scope, where wgpu would otherwise panic.
fn configure(
    gpu: &gpu::Gpu,
    surface: &wgpu::Surface<'static>,
    window: &Window,
) -> Result<wgpu::SurfaceConfiguration, String> {
    let size = window.inner_size();
    let mut config = surface
        .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
        .ok_or("the adapter can't draw to the window")?;
    // Not an sRGB target, as the offscreen one isn't: the renderer blends in gamma space.
    let formats = surface.get_capabilities(&gpu.adapter).formats;
    config.format = formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(config.format);
    config.present_mode = wgpu::PresentMode::AutoVsync;
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    surface.configure(&gpu.device, &config);
    match pollster::block_on(scope.pop()) {
        None => Ok(config),
        Some(error) => {
            Err(format!("{error} ({}x{}, {:?}, {:?})", config.width, config.height, config.format, config.alpha_mode))
        }
    }
}

/// What the window is, as Windows has it, for the log.
fn describe(window: &Window) -> String {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => shell_mode::dashboard::describe_window(handle.hwnd.get()),
        _ => format!("window of {:?}", window.inner_size()),
    }
}

pub fn run(
    args: &Args,
    started_with: &[String],
    library: Library,
    mut store: Store,
    as_shell: crate::as_shell::AsShell,
) {
    shell::log_wgpu();
    let event_loop = EventLoop::new().unwrap_or_else(|e| crate::fail(&format!("no display: {e}")));
    let mut dashboard = Dashboard::new(library);
    dashboard.set_clock(args.clock.clone());
    // Settings' options, as they were left (docs/spec/settings.md, section 7).
    let mut options = store.options();
    // The Boot video is opened before the window, so it decodes while the window and the GPU are made.
    let boot_video = Playing::start(args, &store, &mut options);
    if !options.use_guide {
        eprintln!("guide: off ({}): the Guide button does nothing", Setting::UseGuide.key());
    }
    // The Idle screen's times: Settings', or a test's in their place (docs/spec/idle-screen.md).
    let options = Options { idle: args.idle_after.unwrap_or(options.idle), ..options };
    let given = if args.idle_after.is_some() { "--idle-after" } else { "Settings" };
    eprintln!("idle: {} ({given})", options.idle.describe());
    dashboard.set_options(options);
    // The cards come in a different order each time the Dashboard starts.
    dashboard.seed_idle(SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |since| since.as_nanos() as u64));
    // Settings > Guide's Now Playing source, as it was left: Spotify until one is chosen.
    dashboard.set_now_playing_source(store.setting(screens::now_playing::SOURCE_KEY).as_deref());
    for text in &args.notify {
        dashboard.record(text);
    }
    // A Dashboard that was killed with the Guide open over a game left the game frozen: it is let go first of all.
    let data_dir = args.data_dir.clone().or_else(library::data_dir);
    let mut freezer = Freezer::new(data_dir.as_deref());
    freezer.thaw_leftovers();
    // Discord, signed in with the tokens saved before, if any: its secrets are kept in the data folder.
    let mut discord = discord::Connection::new(&args.discord, data_dir.as_deref(), {
        let wake = std::sync::Mutex::new(event_loop.create_proxy());
        move || {
            let _ = wake.lock().map(|proxy| proxy.send_event(()));
        }
    });
    discord.start();
    dashboard.set_discord(discord.discord());
    for button in args.focus_presses() {
        dashboard.press(Press { button, repeat: false });
    }
    if let Some(page) = args.page.clone() {
        dashboard.open_page(page);
    }
    // Where --focus put focus is where the Dashboard starts: nothing is heard of getting there.
    dashboard.take_effects();
    // Start-up, over all of that until it is done: the Boot video, if one plays, then Who's playing today? and
    // Signing you in. With --no-startup, Home at once, the Profile signed in last signed in.
    if !args.no_startup {
        dashboard.start_up(boot_video.is_some());
    }
    let current = model::update::Version::parse(crate::build().version).unwrap_or_default();
    let last_check = store.setting(screens::settings::LAST_UPDATE_CHECK);
    let updater = crate::updates::Updater::new(args.update_source.clone(), current, started_with, last_check);
    let helpers = crate::helper_programs::Helpers::for_args(args, data_dir.clone());
    helpers.refresh(store.sources(), crate::helper_programs::wake(event_loop.create_proxy()));
    store.set_helpers(helpers.shown());
    dashboard.set_helpers(helpers.shown());
    let set_up = crate::set_up::SetUp::for_args(args, data_dir.clone());
    store.set_clients(set_up.shown());
    dashboard.set_clients(set_up.shown());
    // What a store's installer's thread has for the user (#230), to the window.
    let words = mpsc::channel();
    let mut app = App {
        windowed: args.windowed,
        as_shell,
        dashboard,
        pad: Pad::start(
            {
                let proxy = event_loop.create_proxy();
                move || {
                    let _ = proxy.send_event(());
                }
            },
            args.fake_pad_battery,
        ),
        keys: Vec::new(),
        overlay: None,
        overlay_tried: false,
        overlay_store: ui::Store::default(),
        overlay_last_frame: Instant::now(),
        overlay_stale: false,
        overlay_moving: false,
        overlay_next: Instant::now(),
        toast_strip: None,
        toast_store: ui::Store::default(),
        toast_last_frame: Instant::now(),
        toasts_stale: false,
        told_time: Instant::now(),
        toasts_to_post: Some(args.toasts.clone()),
        toasts_said: (0, Seen::Nothing),
        cpu_renderer: false,
        hub_open: Default::default(),
        toast_posts: args.allow_toast_posts,
        home_took: Duration::ZERO,
        home_done: Instant::now(),
        state: None,
        proxy: event_loop.create_proxy(),
        last_frame: Instant::now(),
        stale: true,
        presented: None,
        home_presented: None,
        boot_video,
        startup_said: None,
        wakes: Box::new(SystemWakes::new({
            let proxy = event_loop.create_proxy();
            let wake = std::sync::Mutex::new(proxy);
            move || {
                let _ = wake.lock().map(|proxy| proxy.send_event(()));
            }
        })),
        launch: None,
        backdrop: Default::default(),
        freezer,
        release_wait: None,
        hand_back: None,
        sounds: Sounds::open(args, &store),
        store,
        library_ready: since_start(),
        moving: false,
        log_focus: args.log_focus,
        profile_said: String::new(),
        // The one place the machine's own power is in reach: a test or an offscreen frame never comes here.
        power: if args.dry_power { Box::new(DryPower::default()) } else { Box::new(SystemPower) },
        // A store's address goes to the program Windows has for it, or with --open-addresses-with to that program.
        addresses: {
            let addresses: Box<dyn Addresses> = match &args.open_addresses_with {
                Some(program) => {
                    eprintln!("store: every address is opened with {}", program.display());
                    Box::new(ProgramAddresses::new(program))
                }
                None => Box::new(SystemAddresses),
            };
            let wake = std::sync::Mutex::new(event_loop.create_proxy());
            Box::new(Asking::new(addresses, words.0, move || {
                let _ = wake.lock().map(|proxy| proxy.send_event(()));
            }))
        },
        words: words.1,
        agreements: Default::default(),
        agreements_asked: 0,
        audio: Box::new(SystemAudio),
        audio_read: None,
        // Windows' media sessions, or with --fake-media one made up; their album art is kept in the data folder.
        media: match args.fake_media() {
            Some(fake) => Box::new(fake),
            None => {
                let proxy = event_loop.create_proxy();
                let wake = std::sync::Mutex::new(proxy);
                media::system(data_dir.as_ref().map(|dir| dir.join("now-playing")), move || {
                    let _ = wake.lock().map(|proxy| proxy.send_event(()));
                })
            }
        },
        now_playing_said: None,
        leaving: Duration::ZERO,
        minute: minute(),
        frames: args.log_frames.then(|| ([0; 3], Instant::now())),
        frame_times: Default::default(),
        ledger: Ledger::new(Instant::now()),
        frame_spent: Default::default(),
        draw_cost: Default::default(),
        late: Default::default(),
        art: None,
        art_started: false,
        art_events: mpsc::channel(),
        art_run: 0,
        achievements_listed: (None, false),
        picker: None,
        picker_run: 0,
        picker_events: mpsc::channel(),
        input_clock: shell::idle::system(),
        idle_after: args.idle_after,
        discord,
        updater,
        helpers,
        set_up,
    };
    // Where shell mode stands, for Settings > System: asked off the frame's thread, as schtasks is slow.
    let proxy = event_loop.create_proxy();
    app.as_shell.ask(None, move || {
        let _ = proxy.send_event(());
    });
    event_loop.run_app(&mut app).expect("event loop");
}

impl App {
    /// Makes the Overlay and the Toast strip, the first time it is called. Where there can't be an Overlay the
    /// Dashboard is told, and shows nothing that would need it; with no strip it draws Toasts in Home's window.
    /// Toasts have somewhere to show from here on: the greeting is asked for (once Start-up is over), and `--toast`'s
    /// notifications posted.
    fn make_overlay(&mut self) {
        let Some(state) = &self.state else { return };
        if self.overlay_tried {
            return;
        }
        self.overlay_tried = true;
        let was = self.ledger.enter(Part::MakeOverlay);
        self.overlay = Overlay::new(&state.gpu, state.config.format);
        if self.overlay.is_none() {
            self.dashboard.set_overlay(false);
            // The Guide, if it is open at Home, is in Home's window from here on.
            self.stale = true;
            if cfg!(windows) {
                eprintln!(
                    "overlay: none: games start with no Launch screen, and the Guide opens only at Home, in Home's \
                     window"
                );
            }
        }
        self.toast_strip = ToastStrip::new(&state.gpu, state.config.format, self.toast_posts);
        self.dashboard.set_toast_strip(self.toast_strip.is_some());
        if self.toast_strip.is_none() && cfg!(windows) {
            eprintln!("toasts: no strip: Toasts show in Home's window, and not over a game");
        }
        self.ledger.enter(was);
        self.dashboard.toasts_ready();
        for text in self.toasts_to_post.take().unwrap_or_default() {
            self.dashboard.notify(&text);
        }
        self.toasts_changed();
    }

    /// The Toast on screen may have changed: whichever window shows Toasts wants a frame.
    fn toasts_changed(&mut self) {
        self.toasts_stale = true;
        self.stale |= self.toast_strip.is_none();
    }

    /// Says in the log what became of the Toasts since it last did: one shown, going, gone. Each is said of what
    /// the screen has, by the frames presented: shown at a Toast's first frame, going at the first frame of its
    /// going, gone at the first without it or when the strip is hidden. So the times are the screen's.
    fn say_toasts(&mut self) {
        let toasts = self.dashboard.toasts();
        let now = (toasts.seen_count(), toasts.seen());
        let (count, seen) = std::mem::replace(&mut self.toasts_said, now);
        let at = since_start();
        if seen == Seen::Showing && now.1 != Seen::Showing {
            eprintln!("toasts: going at {at:.0} ms");
        }
        if seen != Seen::Nothing && (now.1 == Seen::Nothing || now.0 != count) {
            eprintln!("toasts: gone at {at:.0} ms");
        }
        if now.0 != count {
            if let Some(toast) = toasts.on_screen() {
                let waiting = toasts.waiting();
                let (text, kind, told) = (&toast.text, toast.kind, toast.told_line());
                eprintln!("toasts: shown {text:?} ({kind:?}){told}, {waiting} waiting at {at:.0} ms");
            }
        }
    }

    /// Shows, hides and draws the Toast strip as the Dashboard's Toasts come and go, and tells the Dashboard what
    /// has reached the screen, which its queue goes by. Returns how soon it wants another turn, None when it has
    /// nothing more to draw.
    fn follow_toasts(&mut self) -> Option<Duration> {
        let (Some(strip), Some(state)) = (&mut self.toast_strip, &mut self.state) else {
            // Toasts are in Home's window, which says what it presented. Behind a game it presents nothing: they
            // pass unseen, in their time.
            if self.toast_strip.is_none() && self.dashboard.home_covered() {
                self.dashboard.toasts_presented(told(&mut self.told_time));
            }
            return None;
        };
        let wanted = self.dashboard.toasts().on_screen().is_some();
        if wanted != strip.shown() {
            if wanted {
                self.toast_store.clear();
                self.toast_last_frame = Instant::now();
            }
            strip.set(wanted);
            self.toasts_stale = true;
        }
        if !wanted {
            // Hidden: the last Toast is off the screen.
            self.dashboard.toasts_presented(told(&mut self.told_time));
            return None;
        }
        // The Overlay comes to be above it whenever it is shown, raised or given the foreground.
        strip.keep_above();
        // A Toast at rest between its turns of text asked to be woken, and draws nothing meanwhile.
        let wake = self.toast_store.wake().map(Duration::from_secs_f32);
        let rest = wake.map(|wake| wake.saturating_sub(self.toast_last_frame.elapsed()));
        if !(self.toasts_stale || self.toast_store.animating() || rest == Some(Duration::ZERO)) {
            return rest;
        }
        let Some(frame) = strip.frame(&state.gpu) else {
            if strip.lost() {
                // It said why. From here on Toasts are drawn in Home's window.
                self.toast_strip = None;
                self.dashboard.set_toast_strip(false);
                self.stale = true;
                return None;
            }
            return Some(OVERLAY_RETRY);
        };
        let now = Instant::now();
        // The Toast is where its age puts it: only what the store itself times moves by this.
        let dt = (now - self.toast_last_frame).as_secs_f32().min(1.0);
        self.toast_last_frame = now;
        self.toasts_stale = false;
        if let Some((frames, _)) = &mut self.frames {
            frames[2] += 1;
        }
        let view = frame.texture.create_view(&Default::default());
        let (gpu, dashboard) = (&state.gpu, &mut self.dashboard);
        // The strip is the foot of the screen: the frame is laid out for the whole screen, and cleared to nothing.
        let (size, part) = (strip.size(), (strip.screen(), strip.top()));
        state.host.with_store(&mut self.toast_store, |host| {
            host.draw_part(gpu, &view, size, part, dt, wgpu::Color::TRANSPARENT, |ui| dashboard.draw_toasts(ui))
        });
        strip.present(gpu, frame);
        self.dashboard.toasts_presented(told(&mut self.told_time));
        if self.toast_store.animating() {
            return Some(Duration::ZERO);
        }
        self.toast_store.wake().map(Duration::from_secs_f32)
    }

    /// Starts a game through its source, the Dashboard showing its Launch screen.
    fn launch(&mut self, game: usize) {
        if self.launch.is_some() {
            return;
        }
        self.make_overlay();
        let Some(state) = &self.state else { return };
        let Some(found) = self.dashboard.library().games.get(game) else { return };
        let Some(dir) = &found.install_dir else {
            eprintln!("launch: {} isn't installed", found.name);
            return;
        };
        let Some(source) = crate::sources::of(self.store.sources(), found) else {
            eprintln!("launch: {} is from no source the Dashboard has", found.name);
            return;
        };
        let home = window_handle(&state.window);
        let launch_screen = self.overlay.is_some();
        if let Some(launch) = Launch::start(found, source.clone(), dir, home, launch_screen, &mut *self.addresses) {
            self.launch = Some(launch);
            self.backdrop.launch_started();
            self.dashboard.launch_started(game);
        }
    }

    /// Shows, hides and draws the Overlay as the Dashboard wants it. Returns how soon it wants another turn, None
    /// when it has nothing more to draw.
    fn follow_overlay(&mut self) -> Option<Duration> {
        // Wanted before Home came to rest: the Guide opened in the Dashboard's first moments.
        if !self.overlay_tried && self.dashboard.overlay() != OverlayShows::Nothing {
            self.make_overlay();
        }
        let (Some(overlay), Some(state)) = (&mut self.overlay, &mut self.state) else { return None };
        let wanted = |dashboard: &Dashboard| match dashboard.overlay() {
            OverlayShows::Nothing => None,
            OverlayShows::Passive => Some(Mode::Passive),
            OverlayShows::Guide => Some(Mode::Guide),
        };
        let mode = wanted(&self.dashboard);
        if mode != overlay.mode() {
            if overlay.mode().is_none() {
                // Shown afresh: whatever was moving when it was last hidden starts again from its start.
                self.overlay_store.clear();
                self.overlay_last_frame = Instant::now();
                self.overlay_moving = false;
            }
            let was = self.ledger.enter(Part::OverlayShown);
            overlay.set(mode);
            self.ledger.enter(was);
            self.overlay_stale = true;
        }
        mode?;
        let moving = self.overlay_store.animating() || state.host.art.pending() > 0;
        if !self.overlay_stale {
            if !moving {
                return None;
            }
            // Only the Spinner turning: a slow renderer waits its turn, and the loop goes on meanwhile.
            let wait = self.overlay_next.saturating_duration_since(Instant::now());
            if self.dashboard.overlay_at_rest() && !wait.is_zero() {
                return Some(wait);
            }
        }
        let began = Instant::now();
        let was = self.ledger.enter_at(Part::OverlayWait, began);
        let Some(frame) = overlay.frame(&state.gpu) else {
            self.ledger.enter(was);
            if overlay.lost() {
                // It said why. From here on the Dashboard shows nothing that needs it: the game is let go, and
                // the Guide at Home is in Home's window.
                self.overlay = None;
                self.dashboard.set_overlay(false);
                self.freezer.thaw();
                self.stale = true;
                return None;
            }
            return Some(OVERLAY_RETRY);
        };
        let now = Instant::now();
        self.ledger.enter_at(Part::OverlayDraw, now);
        let spent = self.frames.is_some().then(|| self.ledger.totals_at(now));
        // Something that was moving has moved on by as long as this frame is after the last, so a fade takes its
        // time by the clock however few frames a slow renderer gives it; from rest, a frame starts what it starts.
        let dt = (now - self.overlay_last_frame).as_secs_f32().min(if self.overlay_moving { 1.0 } else { 0.1 });
        self.overlay_last_frame = now;
        self.overlay_stale = false;
        if let Some((frames, _)) = &mut self.frames {
            frames[1] += 1;
        }
        let view = frame.texture.create_view(&Default::default());
        let (gpu, size, dashboard) = (&state.gpu, overlay.size(), &mut self.dashboard);
        // Cleared to nothing: where the Dashboard draws nothing, the game shows.
        state.host.with_store(&mut self.overlay_store, |host| {
            host.draw(gpu, &view, size, dt, wgpu::Color::TRANSPARENT, |ui| dashboard.draw_overlay(ui))
        });
        self.draw_cost[1] = state.host.cost;
        self.ledger.enter(Part::OverlayPresent);
        overlay.present(gpu, frame);
        self.ledger.enter(was);
        if let Some(totals) = spent {
            let spent = totals.since(&std::mem::replace(&mut self.frame_spent[1], totals));
            if let Some(gap) = self.frame_times[1].frame(began, now, Instant::now(), self.overlay_moving) {
                self.late.say("overlay", gap, &spent, draws(&self.draw_cost));
            }
        }
        self.overlay_next = overlay_due(began, began.elapsed());
        // At once if something is moving, or if that frame was the last of something: the Overlay is then hidden
        // without waiting out the loop's sleep.
        self.overlay_moving = self.overlay_store.animating() || state.host.art.pending() > 0;
        if wanted(&self.dashboard) != mode {
            return Some(Duration::ZERO);
        }
        let at_rest = self.dashboard.overlay_at_rest();
        let rest = if at_rest { self.overlay_next.saturating_duration_since(Instant::now()) } else { Duration::ZERO };
        self.overlay_moving.then_some(rest)
    }

    /// Puts in front what the Guide said should be, once the Guide has let go of the foreground (the button that
    /// closed it is up): Home's window in front of the running game, or the game's own window. Where the Overlay
    /// has the foreground it is told who to give it to, and gives it as it stops being the Guide; where it has
    /// not (the Guide in Home's window, or no Guide at all: A on the running game's tile) Home's window gives or
    /// takes it itself. True when Home's window is to be hidden behind the game once the Overlay has handed over.
    fn hand_back(&mut self) -> bool {
        if self.dashboard.overlay() == OverlayShows::Guide {
            return false;
        }
        let (Some(to), Some(state)) = (self.hand_back.take(), &self.state) else { return false };
        let Some(launch) = &mut self.launch else { return false };
        let overlay = self.overlay.as_mut().filter(|overlay| overlay.mode() == Some(Mode::Guide));
        match to {
            HandBack::Home => {
                launch.show_home(overlay.is_none());
                if let Some(overlay) = overlay {
                    overlay.give_back_to(window_handle(&state.window));
                }
                // Home's window has been hidden since the game came up: it draws afresh.
                self.last_frame = Instant::now();
                self.stale = true;
                false
            }
            HandBack::Game => match overlay {
                Some(overlay) if !launch.game_window().is_null() => {
                    overlay.give_back_to(launch.game_window());
                    true
                }
                _ => {
                    // A game with no window to go back to: the Dashboard stays in front of it.
                    if !launch.show_game(true) {
                        self.dashboard.show_dashboard();
                    }
                    false
                }
            },
        }
    }

    /// Reads the system's sound and tells the Dashboard, if it is not as the Dashboard has it. With --log-focus,
    /// what was read is said whenever it is news: `audio: volume 67, muted false, output "Speakers" of 2, ...`.
    fn read_audio(&mut self) {
        let read = self.audio.read();
        self.audio_read = Some(Instant::now());
        if read != *self.dashboard.audio() {
            self.dashboard.set_audio(read);
            self.stale = true;
            self.overlay_stale = true;
        }
    }

    /// Reads the system's sound as the Audio Flyout opens and every `AUDIO_EVERY` while it is open. How soon it
    /// is to be read again.
    fn follow_audio(&mut self) -> Option<Duration> {
        if !self.dashboard.audio_shown() {
            self.audio_read = None;
            return None;
        }
        let opened = self.audio_read.is_none();
        if self.audio_read.is_none_or(|read| read.elapsed() >= AUDIO_EVERY) {
            let was = self.dashboard.audio().clone();
            self.read_audio();
            if self.log_focus && (opened || was != *self.dashboard.audio()) {
                eprintln!("audio: {}", audio::describe(self.dashboard.audio()));
            }
        }
        self.audio_read.map(|read| AUDIO_EVERY.saturating_sub(read.elapsed()))
    }

    /// Gives the Dashboard Windows' media sessions as each new reading comes; with --log-focus says what the Audio
    /// Flyout's Now Playing card shows as the Flyout opens and whenever that changes (`now playing: ...`).
    fn follow_media(&mut self) {
        if let Some(media) = self.media.take() {
            self.dashboard.set_media(media);
            self.stale = true;
            self.overlay_stale = true;
        }
        let line = self.dashboard.now_playing_line();
        if self.log_focus && line.is_some() && line != self.now_playing_said {
            eprintln!("now playing: {}", line.as_deref().unwrap_or_default());
        }
        self.now_playing_said = line;
    }

    /// Takes in what Discord said and the time, and has the Dashboard show how the connection is now. How soon it
    /// wants another turn: its next timer (a try again, a refresh), None with none.
    fn follow_discord(&mut self) -> Option<Duration> {
        let (focus, guide) = (self.dashboard.focus_line(), self.dashboard.guide_line());
        if self.discord.pump(SystemTime::now()) {
            self.dashboard.set_discord(self.discord.discord());
            self.stale = true;
            // The Discord tab on the Overlay follows the Call too.
            self.overlay_stale = true;
        }
        if self.log_focus && self.dashboard.focus_line() != focus {
            eprintln!("focus: {}", self.dashboard.focus_line());
        }
        if self.log_focus && self.dashboard.guide_line() != guide {
            eprintln!("guide: {}", self.dashboard.guide_line());
        }
        let next = self.discord.next_timer()?;
        Some(next.duration_since(SystemTime::now()).unwrap_or(Duration::ZERO))
    }

    fn carry_out(&mut self, event_loop: &ActiveEventLoop) {
        for effect in self.dashboard.take_effects() {
            match effect {
                Effect::Launch(game) => self.launch(game),
                Effect::QuitGame => {
                    if let Some(launch) = &mut self.launch {
                        launch.quit_game(&mut self.freezer);
                    }
                }
                // With nothing of the game's to end, asking again would never help.
                Effect::ForceQuitGame => {
                    let ended = self.launch.as_mut().is_some_and(|launch| launch.force_quit(&mut self.freezer));
                    if !ended {
                        self.dashboard.quit_given_up();
                    }
                }
                // The Overlay follows what the Dashboard shows: it gives the foreground back as the Guide closes,
                // once the button that closed it is up, and hides once the panel has slid away.
                Effect::GuideClosed => {}
                // Who it gives the foreground to then: carried out as it lets go (`hand_back`).
                Effect::ShowHome => self.hand_back = Some(HandBack::Home),
                Effect::ShowGame => self.hand_back = Some(HandBack::Game),
                // The sound of leaving is played to its end first, as the Bundle plays it.
                Effect::Exit => {
                    eprintln!("power: exit to desktop at {:.0} ms", since_start());
                    std::thread::sleep(self.leaving.min(LEAVING_AT_MOST));
                    event_loop.exit();
                }
                Effect::Sound(sound) => {
                    let seconds = self.sounds.play(sound);
                    if sound == Sound::PowerOff {
                        self.leaving = Duration::from_secs_f32(seconds.max(0.0));
                    }
                }
                Effect::Power(command) => self.power.carry_out(command),
                // The Audio Flyout's: done, and read back at once, so the Flyout shows what the system made of it.
                Effect::Volume(level) => {
                    self.audio.set_volume(level);
                    self.audio_read = None;
                }
                Effect::Mute(muted) => {
                    self.audio.set_muted(muted);
                    self.audio_read = None;
                }
                Effect::DefaultDevice(flow, device) => {
                    let id = self.dashboard.audio().devices(flow).0.get(device).map(|device| device.id.clone());
                    if let Some(id) = id {
                        self.audio.set_default(flow, &id);
                    }
                    self.audio_read = None;
                }
                // The Now Playing card's controls: the app is asked, and its session's next reading shows the rest.
                Effect::Media(command, app_id) => self.media.send(command, &app_id),
                Effect::NowPlayingSource(source) => {
                    self.store.set_text(screens::now_playing::SOURCE_KEY, &source);
                    self.stale = true;
                    self.overlay_stale = true;
                }
                // Settings > Idle's two times: in force already, from now, and saved.
                Effect::IdleTimes(times) => {
                    for (key, value) in times.saved() {
                        self.store.set_text(key, &value);
                    }
                    eprintln!("idle: {}", times.describe());
                }
                // In force already: saved, so that it is still so the next time.
                Effect::Set(change) => {
                    self.store.set(change);
                    // Settings > Art's Download art online: off stops a lookup under way; on starts one.
                    if change.setting == Setting::DownloadArtOnline {
                        self.art = None;
                        if change.on && self.art_started {
                            self.start_art();
                        }
                    }
                }
                // Plays from the next start.
                Effect::BootVideo(video) => self.store.set_boot_video(video),
                // The user's marks go to the library database, and what it says then is what the screens show.
                Effect::Favourite { game, on } => {
                    let marked = self.store.set_favourite(game, on);
                    self.show(marked);
                }
                Effect::Hidden { game, on } => {
                    let marked = self.store.set_hidden(game, on);
                    self.show(marked);
                }
                // Settings' Groups: the database changes, and Home and Full library follow its next snapshot.
                Effect::Filter(change) => {
                    let changed = self.store.change_filter(&change);
                    self.show(changed);
                }
                // Settings > Art: the key kept or forgotten, and a lookup with the new one; the art downloaded gone.
                Effect::ArtKey(key) => {
                    self.art = None;
                    self.store.set_art_key(key.as_ref().map(|secret| secret.0.as_str()));
                    if key.is_some() && self.art_started {
                        self.start_art();
                    }
                }
                // Settings > Discord: the connection does it, and the page shows how it is after.
                Effect::Discord(ask) => {
                    self.discord.ask(ask, shell::clipboard::text);
                    self.dashboard.set_discord(self.discord.discord());
                    self.stale = true;
                }
                // Settings > System: done off the frame's thread, and the page shows how it stands after.
                Effect::ShellMode(on) => {
                    let proxy = self.proxy.clone();
                    self.as_shell.ask(Some(on), move || {
                        let _ = proxy.send_event(());
                    });
                }
                // Settings > About: asked and downloaded off the frame's thread; the page follows how it goes.
                Effect::CheckForUpdates => self.check_for_updates("Check for updates"),
                // Settings > Helpers: a fetch goes off the frame's thread; the page follows how it goes.
                Effect::Helper(ask) => {
                    let proxy = self.proxy.clone();
                    self.helpers.ask(ask, move || {
                        let _ = proxy.send_event(());
                    });
                    self.show_helpers();
                }
                // Settings > Set up this PC: a launcher's app installed or removed off the frame's thread, one at a
                // time; the page follows how it goes. Never while a game of that launcher plays.
                Effect::SetUp(ask) => {
                    let key = match &ask {
                        model::setup::SetUpAsk::Install(key) | model::setup::SetUpAsk::Remove(key) => key.clone(),
                    };
                    let library = self.dashboard.library();
                    let playing = self.dashboard.playing().and_then(|game| library.games.get(game));
                    let playing = playing.filter(|game| game.source == key).map(|game| game.name.clone());
                    let proxy = self.proxy.clone();
                    self.set_up.ask(ask, playing.as_deref(), move || {
                        let _ = proxy.send_event(());
                    });
                    self.show_clients();
                }
                Effect::InstallUpdate => {
                    if self.dashboard.playing().is_some() {
                        eprintln!("update: Install refused while a game runs");
                    } else {
                        let proxy = self.proxy.clone();
                        self.updater.install(move || {
                            let _ = proxy.send_event(());
                        });
                        self.show_updates();
                    }
                }
                Effect::ClearArt => {
                    self.art = None;
                    let cleared = self.store.clear_art();
                    self.show(cleared);
                }
                // Settings > Art's Refresh all art and details: everything downloaded forgotten, then fetched again
                // in the background, the achievements' rarity and icons with it. Before the stores have been read,
                // the first lookup is the refresh's.
                Effect::RefreshArt(None) => {
                    self.art = None;
                    let forgotten = self.store.refresh_art();
                    self.show(forgotten);
                    self.store.ask_achievements(library::achievements::Ask::ReadAll);
                    if self.art_started {
                        self.start_art();
                    } else {
                        self.set_art_refresh(ArtRefresh::Working { done: 0, of: self.dashboard.library().games.len() });
                    }
                }
                // The Tile menu's Refresh art: that game's downloaded art forgotten, and it looked up first.
                // The art picker's choice (docs/spec/art-picker.md): kept at once, or once the chosen picture has
                // downloaded, off the frame's thread.
                Effect::ChooseArt(choice) => match choice.url.clone().filter(|_| choice.chosen.is_some()) {
                    Some(url) => {
                        let (sender, proxy) = (self.picker_events.0.clone(), self.proxy.clone());
                        let log: library::art::Log = Arc::new(|line| eprintln!("{line}"));
                        library::art::download_full(url, library::art::Pace::default(), log, move |got| {
                            let _ = sender.send(PickerEvent::Downloaded(choice, got));
                            let _ = proxy.send_event(());
                        });
                    }
                    None => self.keep_art_choice(choice, None),
                },
                Effect::RefreshArt(Some(game)) => {
                    let forgotten = self.store.refresh_art_of(&game);
                    self.show(forgotten);
                    if self.art_started {
                        self.art = None;
                        self.start_art_with(Some(game));
                    }
                }
                // Settings > Launchers (docs/spec/launchers.md): a launcher's games leave every list or come back at
                // once; one switched on is read again, as Update now reads them all.
                Effect::Launcher { key, on } => {
                    let library = self.dashboard.library();
                    let playing = self.dashboard.playing().and_then(|game| library.games.get(game)).cloned();
                    let switched = self.store.set_launcher(&key, on, playing.as_ref());
                    if switched.is_some() {
                        self.store.tell_queue(::library::queue::Ask::Look);
                    }
                    self.show(switched);
                    if on {
                        self.update_launchers(Some(vec![key]));
                    }
                }
                Effect::UpdateLaunchers => self.update_launchers(None),
                // Settings' Games & apps: the database changes, and the screens follow its next snapshot. What it
                // refuses (a program that is not there) the user is told of.
                Effect::ByHand(change) => match self.store.by_hand(change) {
                    Ok(changed) => self.show(changed),
                    Err(why) => {
                        self.dashboard.notify(&why);
                    }
                },
                // Settings' Profiles pages, or a Profile signing in: the database changes, and every place a Profile
                // shows follows its next snapshot. What it refuses (a picture that is not there) the user is told of.
                Effect::Profile(change) => match self.store.change_profile(&change) {
                    Ok(changed) => self.show(changed),
                    Err(why) => {
                        self.dashboard.notify(&why);
                    }
                },
                Effect::StartApp(app) => crate::games::start_app(self.dashboard.library(), app, &mut *self.addresses),
                // The store is asked, and the Queue told: the game joins it, or its uninstall is followed.
                // Into the folder chosen in the question, else the store's default as Settings > Storage has it.
                Effect::Install(game) | Effect::InstallTo { game, .. } => {
                    let folder = match &effect {
                        Effect::InstallTo { folder, .. } => Some(folder.clone()),
                        _ => crate::storage::default_for(self.dashboard.library(), game),
                    };
                    let (library, sources) = (self.dashboard.library(), self.store.sources());
                    let addresses = &mut *self.addresses;
                    if crate::games::ask_store(library, sources, game, Ask::Install, folder.as_deref(), addresses) {
                        self.store.tell_queue(::library::queue::Ask::Install(game));
                    }
                }
                Effect::DefaultFolder { store, folder } => {
                    let changed = self.store.set_default_folder(&store, &folder);
                    self.show(changed);
                }
                Effect::LookAtStorage => self.look_at_storage(),
                Effect::Uninstall(game) => {
                    let (library, sources) = (self.dashboard.library(), self.store.sources());
                    let addresses = &mut *self.addresses;
                    if crate::games::ask_store(library, sources, game, Ask::Uninstall, None, addresses) {
                        self.store.tell_queue(::library::queue::Ask::Uninstall(game));
                    }
                }
                // A queued game's menu in the Queue (#170): the store is asked through its installer, and the
                // Queue looks at once at what it then says.
                Effect::Steer { game, steer } => {
                    let (library, sources) = (self.dashboard.library(), self.store.sources());
                    if crate::games::steer(library, sources, game, steer, &mut *self.addresses) {
                        if steer == model::Steer::Cancel {
                            self.store.tell_queue(::library::queue::Ask::Cancel(game));
                        }
                        self.store.tell_queue(::library::queue::Ask::Look);
                    }
                }
                // The user's answer to a licence agreement (#230), for the installer that waits on it.
                Effect::Agreement { id, reply } => {
                    eprintln!("store: licence agreement {id} answered: {reply:?}");
                    if let Some(waiting) = self.agreements.remove(&id) {
                        let _ = waiting.send(reply);
                    }
                }
            }
        }
        // An agreement's prompt gone with no answer (the Guide's page over it, a game ended) is declined.
        let up = self.dashboard.agreements_up();
        let gone: Vec<u64> = self.agreements.keys().copied().filter(|id| !up.contains(id)).collect();
        for id in gone {
            eprintln!("store: licence agreement {id} gone unanswered: Decline");
            if let Some(waiting) = self.agreements.remove(&id) {
                let _ = waiting.send(model::Reply::Decline);
            }
        }
        // A Profile signed in by the core itself (`Dashboard::sign_in`) changed before its effect was carried out.
        self.log_profile();
    }

    /// Reads the drives and the stores' install folders again off the frame's thread (Settings > Storage, the
    /// storage ring): a new snapshot follows once they are read.
    fn look_at_storage(&mut self) {
        let proxy = self.proxy.clone();
        self.store.look_at_storage(self.dashboard.library(), move || {
            let _ = proxy.send_event(());
        });
    }

    /// Asks for updates off the frame's thread (`why`, for the log), and keeps when.
    fn check_for_updates(&mut self, why: &str) {
        let proxy = self.proxy.clone();
        let checked = self.updater.check(why, move || {
            let _ = proxy.send_event(());
        });
        if let Some(at) = checked {
            self.store.set_text(screens::settings::LAST_UPDATE_CHECK, &at.to_string());
        }
        self.show_updates();
    }

    /// Settings > About follows where updates stand.
    fn show_updates(&mut self) {
        let updates = self.updater.state();
        if updates != self.dashboard.options().updates {
            self.dashboard.set_options(screens::Options { updates, ..self.dashboard.options() });
            self.stale = true;
            self.overlay_stale = true;
        }
    }

    /// The Boot video while it plays: drawn every refresh, and let go as it ends, fails, runs too long or a press
    /// skips it, when Start-up goes on and Home is drawn.
    fn follow_boot_video(&mut self) {
        let Some(playing) = &mut self.boot_video else { return };
        if playing.ended(self.dashboard.boot_video_playing()).is_some() {
            self.boot_video = None;
            self.dashboard.boot_video_ended();
            self.as_shell.boot_video_ended();
        }
        self.stale = true;
        self.overlay_stale = true;
    }

    /// Reads the launchers switched on again (`only`: these alone) off this thread: the page shows it working.
    fn update_launchers(&mut self, only: Option<Vec<String>>) {
        let proxy = self.proxy.clone();
        if self.store.update_launchers(only, move || {
            let _ = proxy.send_event(());
        }) {
            let working = self.store.snapshot();
            self.show(working);
        }
    }

    /// An update of the launchers is done: its snapshot shown, then what it does not read itself fetched again in
    /// the background, art for the games that lack it and every game's achievements and rarity; and the launchers
    /// switched on meanwhile read.
    fn take_launchers_update(&mut self) {
        let Some(snapshot) = self.store.launchers_updated() else { return };
        self.show(Some(snapshot));
        self.store.tell_queue(library::queue::Ask::Look);
        // Games a launcher brought in may be owned in another too: which stores can install them is read again.
        self.look_at_storage();
        self.store.ask_achievements(library::achievements::Ask::ReadAll);
        if self.art_started {
            self.art = None;
            self.start_art();
        }
        if let Some(again) = self.store.take_update_again() {
            self.update_launchers(Some(again));
        }
    }

    /// Looks up the art the stores gave none for, with the key that is kept: what comes in wakes the loop.
    fn start_art(&mut self) {
        self.start_art_with(None);
    }

    /// The same, `first` looked up before every other game: the Tile menu's Refresh art. A refresh of all that was
    /// asked for and is not over (Settings > Art) goes on in it, and says how far it has got.
    fn start_art_with(&mut self, first: Option<GameKey>) {
        self.art_started = true;
        self.art_run += 1;
        let run = self.art_run;
        let (sender, proxy) = (self.art_events.0.clone(), self.proxy.clone());
        self.art = self.store.look_up_art(self.dashboard.library(), first.as_ref(), move |event| {
            let _ = sender.send((run, event));
            let _ = proxy.send_event(());
        });
        if self.store.art_refreshing() {
            let of = self.dashboard.library().games.len();
            let state = if self.art.is_some() { ArtRefresh::Working { done: 0, of } } else { ArtRefresh::Paused };
            self.set_art_refresh(state);
        }
    }

    /// How the launchers' apps stand, on Settings > Set up this PC, without a new snapshot.
    fn show_clients(&mut self) {
        let shown = self.set_up.shown();
        self.store.set_clients(shown.clone());
        if self.dashboard.set_clients(shown) {
            self.stale = true;
        }
    }

    /// How the helpers stand, on Settings > Helpers, without a new snapshot.
    fn show_helpers(&mut self) {
        let shown = self.helpers.shown();
        self.store.set_helpers(shown.clone());
        if self.dashboard.set_helpers(shown) {
            self.stale = true;
        }
    }

    /// How a refresh of all art goes, on Settings > Art, without a new snapshot.
    fn set_art_refresh(&mut self, state: ArtRefresh) {
        self.store.set_art_refresh(state);
        if self.dashboard.set_art_refresh(state) {
            self.stale = true;
        }
    }

    /// Takes in what the art lookup has said: a game's new picture is in the next snapshot, how far a refresh has
    /// got, and a refused key is said once in a Toast. What a run stopped for another says of its end is not the
    /// new one's. True if a Toast was posted.
    fn take_art(&mut self) -> bool {
        let (mut found, mut toast) = (false, false);
        while let Ok((run, event)) = self.art_events.1.try_recv() {
            let current = run == self.art_run;
            match event {
                ArtEvent::Found { .. } => found = true,
                ArtEvent::KeyRefused => {
                    self.store.art_key_refused();
                    self.dashboard.notify(KEY_REFUSED);
                    toast = true;
                }
                ArtEvent::Looked { done, of } if current && self.store.art_refreshing() => {
                    self.set_art_refresh(ArtRefresh::Working { done, of });
                }
                ArtEvent::Looked { .. } => {}
                ArtEvent::Done(summary) if current => {
                    self.art = None;
                    if self.store.art_refreshing() {
                        let state = self.store.art_refresh_over(&summary);
                        self.set_art_refresh(state);
                    }
                }
                ArtEvent::Done(_) => {}
            }
        }
        if found {
            let was = self.ledger.enter(Part::Library);
            let library = self.store.snapshot();
            self.show(library);
            self.ledger.enter(was);
        }
        toast
    }

    /// The art picker (docs/spec/art-picker.md): what was fetched for it goes to it, and what it asks for is
    /// fetched, by a network of its own made as a picker first asks and dropped once none is up. A page coming
    /// changes the focus line, which the turn then says with --log-focus.
    fn follow_art_picker(&mut self) {
        while let Ok(event) = self.picker_events.1.try_recv() {
            match event {
                PickerEvent::Answer(run, answer) => {
                    if self.picker.as_ref().is_some_and(|(current, _)| *current == run) {
                        self.dashboard.art_answer(answer);
                        self.stale = true;
                    }
                }
                PickerEvent::Downloaded(choice, Ok(bytes)) => self.keep_art_choice(choice, Some(&bytes)),
                PickerEvent::Downloaded(choice, Err(why)) => {
                    eprintln!("art: picker: the chosen {} could not be downloaded: {why}", choice.tab.key());
                    self.dashboard.notify("The picture you chose couldn't be downloaded. Try again in a moment.");
                    self.overlay_stale = true;
                }
            }
        }
        if !self.dashboard.art_picker_up() {
            self.picker = None;
        } else {
            let asks = self.dashboard.art_asks();
            if !asks.is_empty() && self.picker.is_none() {
                self.picker_run += 1;
                let run = self.picker_run;
                let (sender, proxy) = (self.picker_events.0.clone(), self.proxy.clone());
                self.picker = self
                    .store
                    .art_picker(move |answer| {
                        let _ = sender.send(PickerEvent::Answer(run, answer));
                        let _ = proxy.send_event(());
                    })
                    .map(|picker| (run, picker));
            }
            if let Some((_, picker)) = &self.picker {
                for ask in asks {
                    picker.ask(ask);
                }
            }
        }
    }

    /// Keeps the art picker's choice, and shows the snapshot after. Automatic looks the game's art up again, first,
    /// for what the choice stood in for.
    fn keep_art_choice(&mut self, choice: model::art_picker::Choice, picture: Option<&[u8]>) {
        let shown = self.store.choose_art(&choice, picture);
        self.show(shown);
        if choice.chosen.is_none() && self.art_started {
            self.art = None;
            self.start_art_with(Some(choice.game));
        }
    }

    /// An Achievements page's game's whole list (docs/spec/achievements-page.md): read from the database as the
    /// page opens, its icons asked for then, and read again as they come in or the snapshot changes.
    fn follow_achievement_list(&mut self) {
        let wanted = self.dashboard.achievements_wanted();
        let (listed, stale) = self.achievements_listed;
        if wanted == listed && !stale {
            return;
        }
        if let Some(game) = wanted {
            if wanted != listed {
                self.store.ask_achievements(library::achievements::Ask::Icons(game));
            }
            let list = self.store.achievements_of(game);
            let unlocked = list.iter().filter(|a| a.is_unlocked()).count();
            let icons = list.iter().filter(|a| a.icon_file.is_some() || a.icon_locked_file.is_some()).count();
            eprintln!("achievements: the page lists {} ({unlocked} unlocked, {icons} with icons)", list.len());
            self.dashboard.set_achievements(game, list);
            self.stale = true;
        }
        self.achievements_listed = (wanted, false);
    }

    /// A newer snapshot of the library, if there is one, takes the place of the one on screen.
    fn show(&mut self, library: Option<Library>) {
        if let Some(library) = library {
            let was = self.ledger.enter(Part::Library);
            let groups = self.dashboard.home_groups();
            self.dashboard.set_library(library);
            self.stale = true;
            self.overlay_stale = true;
            if self.log_focus && self.dashboard.home_groups() != groups {
                self.log_groups();
            }
            self.log_profile();
            self.ledger.enter(was);
        }
    }

    /// Says in the log where Start-up has got to, by the frame just presented: Who's playing today? shown, a
    /// Profile signing in, signed in and Home shown. The `whos-playing` e2e payload judges by these.
    fn say_startup(dashboard: &Dashboard, startup_said: &mut Option<Phase>) {
        let phase = dashboard.startup_phase();
        let said = std::mem::replace(startup_said, phase);
        if phase == said {
            return;
        }
        let at = since_start();
        let gamertag = dashboard.startup_profile().map(|profile| profile.gamertag.clone()).unwrap_or_default();
        match phase {
            Some(Phase::WhosPlaying) => {
                let profiles = dashboard.startup_profiles();
                eprintln!("startup: who's playing shown at {at:.0} ms, {profiles} profiles, focus on {gamertag:?}");
            }
            Some(Phase::SigningIn) => eprintln!("startup: signing in {gamertag:?} at {at:.0} ms"),
            None if said == Some(Phase::SigningIn) => eprintln!("startup: signed in at {at:.0} ms"),
            _ => {}
        }
    }

    /// The Profile signed in, as Home's top bar, the Guide and the greeting show it
    /// (`profile: "Alpha", real name "", shape 0`). Said with the first frame and again whenever it changes.
    fn log_profile(&mut self) {
        let line = self.dashboard.profile_line();
        if line != self.profile_said {
            eprintln!("profile: {line}");
            self.profile_said = line;
        }
    }

    /// With --log-focus: Home's Groups in order, as Home shows them now (`groups: Favourites, Most played`). Said
    /// with the first frame and again whenever they change.
    fn log_groups(&self) {
        eprintln!("groups: {}", self.dashboard.home_groups().join(", "));
    }

    /// What the Idle screen did this turn, in the log whatever the flags (`idle: dimmed at 4012 ms`, `idle:
    /// showing things at 7003 ms`, `idle: woken by A at 9120 ms`): both windows draw it afresh.
    fn say_idle(&mut self) {
        for news in self.dashboard.take_idle_news() {
            eprintln!("idle: {news} at {:.0} ms", since_start());
            self.stale = true;
            self.overlay_stale = true;
        }
    }

    /// With --idle-after, its times stay in force: a change in Settings is saved as ever, but counted by only from
    /// the Dashboard's next start.
    fn keep_idle_after(&mut self) {
        let Some(times) = self.idle_after else { return };
        if self.dashboard.idle_times() != times {
            eprintln!("idle: {} (--idle-after, in place of Settings')", times.describe());
            self.dashboard.set_options(screens::Options { idle: times, ..self.dashboard.options() });
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        // The window is made plain, and its swap chain on it, before anything else is asked of it: under shell mode
        // Start-up is made topmost only once the surface is configured (docs/spec/shell-mode.md, 6).
        let windowed = self.windowed;
        let make_window = |event_loop: &ActiveEventLoop| {
            let attributes = Window::default_attributes().with_title(shell_mode::dashboard::WINDOW_TITLE);
            let attributes = if windowed {
                attributes.with_inner_size(PhysicalSize::new(1280, 720))
            } else {
                attributes.with_fullscreen(Some(Fullscreen::Borderless(event_loop.primary_monitor())))
            };
            let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
            window.set_cursor_visible(windowed);
            window
        };
        let window = make_window(event_loop);

        let window_ready = since_start();
        let instance = gpu::instance(Some(Box::new(event_loop.owned_display_handle())));
        let surface = instance.create_surface(window.clone()).expect("create surface");
        let gpu = gpu::init(instance, Some(&surface));
        self.cpu_renderer = gpu.adapter.get_info().device_type == wgpu::DeviceType::Cpu;
        self.hub_open.set_processor(self.cpu_renderer);
        // A swap chain Windows refuses is never a panic: said, with what the window was, and tried once more on a
        // new window with nothing of shell mode's on it (topmost dropped for good); a second refusal ends it.
        let mut topmost = self.as_shell.topmost_start_up() && self.dashboard.starting_up();
        let (window, surface, config) = match configure(&gpu, &surface, &window) {
            Ok(config) => (window, surface, config),
            Err(why) => {
                eprintln!("window: ERROR: the swap chain couldn't be made: {why}; {}", describe(&window));
                drop(surface);
                drop(window);
                let dropped = if topmost { "topmost for Start-up" } else { "nothing but the window itself" };
                topmost = false;
                eprintln!("window: trying once more on a new plain window; dropped: {dropped}");
                let window = make_window(event_loop);
                let surface = gpu.instance.create_surface(window.clone()).expect("create surface");
                match configure(&gpu, &surface, &window) {
                    Ok(config) => {
                        eprintln!("window: the plain window's swap chain was made");
                        (window, surface, config)
                    }
                    Err(why) => {
                        eprintln!("window: ERROR: again: {why}; {}", describe(&window));
                        crate::fail("no swap chain for the window, twice");
                    }
                }
            }
        };
        // A request to close (taskkill without /f) is heard from any program, one with fewer rights included: a
        // Dashboard Winlogon started as the shell may run elevated, and Windows would drop it unsaid (#99).
        if cfg!(windows) {
            let heard = shell::hear_close_from_anyone(window_handle(&window));
            let elevated = shell::elevated().map_or("unknown", |elevated| if elevated { "yes" } else { "no" });
            eprintln!("window: runs elevated: {elevated}; a request to close is heard from any program: {heard}");
        }
        if topmost {
            window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
            eprintln!("window: topmost while Start-up shows; {}", describe(&window));
        }
        if self.frames.is_some() {
            let hertz = window.current_monitor().and_then(|monitor| monitor.refresh_rate_millihertz());
            let hertz = hertz.map_or("unknown".to_string(), |rate| format!("{:.1} Hz", f64::from(rate) / 1000.0));
            eprintln!(
                "display: {}x{} at {hertz}, Home presents {:?}, at most {} frames queued",
                config.width, config.height, config.present_mode, config.desired_maximum_frame_latency
            );
        }

        let gpu_ready = since_start();

        // A piece of art decoded on another thread asks for a frame.
        let proxy = self.proxy.clone();
        let host = Host::new(&gpu, config.format, move || {
            let _ = proxy.send_event(());
        });
        let ready = [self.library_ready, window_ready, gpu_ready, since_start()];
        self.state = Some(State { host, config, surface, gpu, ready, window });
        // The Dashboard's clock starts with the window: the time the window and the GPU took to be made is not
        // Signing you in's.
        self.told_time = Instant::now();
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        self.ledger.back(Part::Events);
        self.backdrop.art_came();
        self.stale = true;
        self.overlay_stale = true;
        self.ledger.away();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.ledger.back(Part::Updates);
        // Another program asked the Dashboard to close through a window of the shell's, as taskkill does.
        if shell::close_asked() {
            event_loop.exit();
            return;
        }
        if self.state.is_none() {
            return;
        }
        self.hub_open.turn();
        // Settings > System: where shell mode stands, as a thread found it. Under `shell:`, not `settings:`, which is
        // only ever an option set or saved.
        if let Some(shell) = self.as_shell.answer() {
            eprintln!("shell: Settings > System shows {}", screens::settings::system::state(shell));
            self.dashboard.set_options(screens::Options { shell, ..self.dashboard.options() });
            self.stale = true;
        }
        // Where focus and the Guide are before anything this turn changes them: a press, a game ending, time, an
        // update's check or install going on (Settings > About's line says where updates stand).
        let focus = self.dashboard.focus_line();
        let guide = self.dashboard.guide_line();
        // Updates: how a check or an install goes, as a thread says; the daily check when it is due; and a Toast
        // for a version ready, never while a game runs.
        for event in self.updater.poll() {
            match event {
                crate::updates::Event::Changed(_) => self.show_updates(),
                crate::updates::Event::HandedOver => {
                    eprintln!("update: handed over at {:.0} ms; exiting 0", since_start());
                    event_loop.exit();
                    return;
                }
            }
        }
        if self.updater.due(self.dashboard.options().daily_update_check) {
            self.check_for_updates("the daily check");
        }
        if self.updater.owes_toast() && self.home_presented.is_some() && self.dashboard.playing().is_none() {
            if let Some(version) = self.updater.untold() {
                eprintln!("update: {version} is ready: a Toast tells of it");
                self.dashboard.notify(&format!(
                    "Version {version} of the Dashboard is ready. Install it in Settings > System > About."
                ));
                self.overlay_stale = true;
            }
        }
        // The last input anywhere, read before the pad's events are taken (`pad.poll`): a press among them that
        // made it is then still to come, and is the Idle screen's wake press (docs/spec/idle-screen.md, 5).
        if let Some(at) = shell::idle::later(self.input_clock.last_input(), self.pad.last_used()) {
            self.dashboard.input_seen(at);
        }
        // Toasts are timed by the time the Dashboard is told has passed: told first, so that one posted this turn
        // starts from its start.
        let began = Instant::now();
        let mut toasts_changed = self.dashboard.advance(told(&mut self.told_time));
        self.hub_open.spent(began.elapsed());
        // A slow renderer, by the Overlay's test or by its kind: Home gives way to a Toast on one.
        self.dashboard.set_slow_renderer(self.cpu_renderer || self.home_took > SLOW_FRAME);
        // After Home's first frame, and once Home has come to rest or a while has passed, so the Overlay costs
        // neither the start-up nor the first art its time. A launch that comes sooner makes it then.
        if let (Some(presented), Some(state)) = (self.home_presented, &self.state) {
            let at_rest = !self.stale && !state.host.animating() && state.host.art.pending() == 0;
            if at_rest || presented.elapsed() >= OVERLAY_AFTER {
                self.make_overlay();
            }
        }
        if let Some(strip) = &mut self.toast_strip {
            for text in strip.take_posts() {
                eprintln!("toasts: posted {text:?} at {:.0} ms", since_start());
                self.dashboard.notify(&text);
                toasts_changed = true;
                // The open Guide counts it, on its Notifications Utility.
                self.overlay_stale = true;
            }
        }
        // What the Dashboard times itself: a game that was asked to quit and has not, the Guide closing after one.
        if self.dashboard.take_timed() {
            self.stale = true;
            self.overlay_stale = true;
        }
        let events = self.pad.poll();
        let mut presses: Vec<Press> = Vec::new();
        let keys = std::mem::take(&mut self.keys);
        let typed = !keys.is_empty();
        if self.dashboard.set_pad(self.pad.status()) {
            self.stale = true;
            self.overlay_stale = true;
        }
        self.ledger.enter(Part::Sources);
        let read = self.store.sources_read();
        let was = self.ledger.enter(Part::Library);
        let changed = self.store.changed();
        self.ledger.enter(was);
        if let Some(library) = changed {
            self.show(Some(library));
            // The sources have been read: an install started in a client meanwhile joins the Queue now.
            self.store.tell_queue(library::queue::Ask::Look);
            // And their libraries' folders may have changed with them.
            if !read && self.store.sources_read() {
                self.look_at_storage();
            }
        }
        if self.store.storage_wanted() {
            self.look_at_storage();
        }
        // What a store's installer has for the user (#230): an agreement's prompt over whatever is up, or the Queue
        // told why an install stopped (its Toast follows from the Queue, with its row).
        while let Ok(word) = self.words.try_recv() {
            match word {
                Word::Agreement { game, name, agreement, reply } => {
                    self.agreements_asked += 1;
                    let id = self.agreements_asked;
                    eprintln!(
                        "store: licence agreement {id} for {name} ({} of {}){}",
                        agreement.at,
                        agreement.of,
                        if agreement.text.is_ok() { "" } else { ", its text not had" }
                    );
                    let _ = game;
                    self.agreements.insert(id, reply);
                    let focus = self.dashboard.focus_line();
                    self.dashboard.ask_agreement(screens::agreement::Entry { id, game: name, agreement });
                    // Opened with no press: said as a press's would be.
                    if self.log_focus && self.dashboard.focus_line() != focus {
                        eprintln!("focus: {}", self.dashboard.focus_line());
                    }
                    self.stale = true;
                    self.overlay_stale = true;
                }
                Word::Stopped { game, stop } => self.store.tell_queue(library::queue::Ask::Stopped { game, stop }),
            }
        }
        // The Queue moved on: a new snapshot when a source read again changed the library, else the Queue alone.
        if let Some(update) = self.store.queue_update() {
            self.show(update.library);
            // A download's bytes and speed alone draw nothing new on Home.
            if self.dashboard.set_queue(update.queue) {
                self.stale = true;
                self.overlay_stale = true;
            }
            // A game installed or uninstalled has filled or freed its drive: the ring and Storage read again.
            if update
                .tell
                .iter()
                .any(|tell| matches!(tell, library::queue::Tell::Ready(_) | library::queue::Tell::Uninstalled(_)))
            {
                self.look_at_storage();
            }
            // A game queued, ready to play, uninstalled, or stopped: a Toast each, with its cover (#132, #230).
            for tell in update.tell {
                let (kind, game) = match tell {
                    library::queue::Tell::Queued(game) => (screens::toasts::ToastKind::Queued, game),
                    library::queue::Tell::Ready(game) => (screens::toasts::ToastKind::Installed, game),
                    library::queue::Tell::Uninstalled(game) => (screens::toasts::ToastKind::Uninstalled, game),
                    library::queue::Tell::Stopped(stopped) => {
                        eprintln!("toast: {}: {}", stopped.headline(), stopped.line());
                        self.dashboard.tell_stopped(&stopped);
                        toasts_changed = true;
                        continue;
                    }
                };
                self.dashboard.tell(kind, game);
                toasts_changed = true;
            }
        }
        // Art for the games a store gave none for, once the stores have been read and the first frame is out.
        if !self.art_started && self.home_presented.is_some() && self.store.sources_read() {
            self.start_art();
        }
        // Achievements, once the stores' games are in the database (docs/achievements.md): the totals in a new
        // snapshot, and what was unlocked in the Achievement unlocked Toast (docs/spec/achievement-toast.md).
        if self.store.sources_read() {
            let proxy = self.proxy.clone();
            self.store.follow_achievements(move || {
                let _ = proxy.send_event(());
            });
        }
        self.take_launchers_update();
        if let Some(update) = self.store.achievements_update() {
            let listed = self.achievements_listed.0;
            self.achievements_listed.1 |= update.library.is_some() || listed.is_some_and(|g| update.icons.contains(&g));
            self.show(update.library);
            self.dashboard.unlocked(update.unlocked);
        }
        if self.helpers.take() {
            self.show_helpers();
            self.helpers.refresh(self.store.sources(), crate::helper_programs::wake(self.proxy.clone()));
        }
        let refreshed = self.helpers.refreshed();
        if !refreshed.is_empty() {
            self.update_launchers(Some(refreshed));
        }
        // A launcher's app installed or removed: its launcher read again, so its games come in or show as gone.
        // The page's line follows the step without a press, and is said as a press's would be.
        let (changed, over) = self.set_up.take();
        if changed {
            let focus = self.dashboard.focus_line();
            self.show_clients();
            if self.log_focus && self.dashboard.focus_line() != focus {
                eprintln!("focus: {}", self.dashboard.focus_line());
            }
        }
        if !over.is_empty() {
            self.update_launchers(Some(over));
        }
        if self.take_art() {
            toasts_changed = true;
            self.overlay_stale = true;
        }
        self.ledger.enter(Part::Launch);
        // The keyboard is the Overlay's while it is the Guide over a game.
        if let Some(overlay) = &mut self.overlay {
            presses.extend(overlay.take_keys().into_iter().map(|(key, repeat)| {
                let button = match key {
                    OverlayKey::Left => Button::Left,
                    OverlayKey::Right => Button::Right,
                    OverlayKey::Up => Button::Up,
                    OverlayKey::Down => Button::Down,
                    OverlayKey::Enter => Button::A,
                    OverlayKey::Back | OverlayKey::Escape => Button::B,
                };
                Press { button, repeat }
            }));
        }

        if let Some(launch) = &mut self.launch {
            match launch.tick(self.overlay.as_ref()) {
                Step::Same => {}
                Step::LaunchScreenGoes => {
                    self.dashboard.game_booted();
                    self.overlay_stale = true;
                    let playing = self.dashboard.playing();
                    self.store.game_booted(playing.and_then(|game| self.dashboard.library().games.get(game)));
                }
                Step::Ended => {
                    // Whatever of it was frozen under the Guide has gone with it.
                    self.freezer.thaw();
                    self.launch = None;
                    self.hand_back = None;
                    // A Guide that is open stays open, and closing it lands on Home, whose window is back behind it.
                    if let (Some(overlay), Some(state)) = (&mut self.overlay, &self.state) {
                        overlay.give_back_to(window_handle(&state.window));
                    }
                    // The library first, with the Play session that has just ended: a game that is to start now
                    // is named by its place in the library as it is.
                    if let Some(library) = self.store.game_ended() {
                        self.dashboard.set_library(library);
                    }
                    self.dashboard.game_ended();
                    self.backdrop.game_ended(self.dashboard.backdrop_fading());
                    self.last_frame = Instant::now();
                    self.stale = true;
                    self.overlay_stale = true;
                }
            }
        }

        // Woken from sleep: Who's playing today? asks again, if it asks on waking.
        if self.wakes.woke() {
            let asked = self.dashboard.woke_from_sleep();
            let what = if asked { "Who's playing today? asks" } else { "nothing asked" };
            eprintln!("startup: woke from sleep at {:.0} ms: {what}", since_start());
            self.stale = true;
            self.overlay_stale = true;
        }
        self.ledger.enter(Part::Input);
        let pressed = typed || !presses.is_empty() || !events.is_empty();
        let began = Instant::now();
        for event in events {
            if let ui::PadEvent::Press(press) = &event {
                self.hub_open.pressed(*press, began);
            }
            self.dashboard.pad_event(event);
        }
        // The keyboard's keys, in the order they were pressed. Text goes to a page that takes it; Backspace
        // deletes there, and is B where nothing does.
        for key in keys {
            match key {
                KeyPress::Button(press) => {
                    self.hub_open.pressed(press, began);
                    self.dashboard.press(press)
                }
                KeyPress::Backspace { repeat } => {
                    if !self.dashboard.typed(Typed::Backspace) {
                        self.dashboard.press(Press { button: Button::B, repeat });
                    }
                }
                KeyPress::Text(text) => {
                    self.dashboard.typed(Typed::Text(&text));
                }
            }
        }
        for press in presses {
            self.hub_open.pressed(press, began);
            self.dashboard.press(press);
        }
        self.hub_open.spent(began.elapsed());
        // An Achievements page a press opened has its list before its focus is said.
        self.follow_achievement_list();
        // An art picker a press opened asks for its first page at once.
        self.follow_art_picker();
        self.say_idle();
        if self.log_focus && self.dashboard.focus_line() != focus {
            eprintln!("focus: {}", self.dashboard.focus_line());
        }
        if self.log_focus && self.dashboard.guide_line() != guide {
            eprintln!("guide: {}", self.dashboard.guide_line());
        }
        // A held direction repeats at the pace of whatever has the pad now.
        self.pad.set_repeat(self.dashboard.repeat());
        self.overlay_stale |= pressed;
        // A press that opens the Guide sends the Toast away.
        if toasts_changed || pressed {
            self.toasts_changed();
        }
        self.stale |= self.dashboard.focus_line() != focus;
        let said = (self.dashboard.focus_line(), self.dashboard.guide_line());
        let began = Instant::now();
        self.ledger.enter_at(Part::Effects, began);
        self.carry_out(event_loop);
        self.hub_open.spent(began.elapsed());
        self.ledger.enter(Part::Follow);
        // A Game hub that this turn put up starts being timed (#214).
        if let Some(state) = &self.state {
            let px = ui::scale_for((state.config.width, state.config.height));
            let hub = self
                .dashboard
                .game_hub_up()
                .map(|(serial, game)| (serial, game.name.clone(), screens::game_hub::art_keys(game, px)));
            self.hub_open.look(hub, &state.host.art, Instant::now());
        }
        // What was carried out may have changed what the page says (a Group shown or not, from the snapshot after),
        // or the Guide (Discord asked: a Flyout loading, a Shortcut removed).
        if self.log_focus && self.dashboard.focus_line() != said.0 {
            eprintln!("focus: {}", self.dashboard.focus_line());
        }
        if self.log_focus && self.dashboard.guide_line() != said.1 {
            eprintln!("guide: {}", self.dashboard.guide_line());
        }
        // Settings > Idle's times, which an --idle-after run keeps in place of them.
        self.keep_idle_after();
        let audio_wants = self.follow_audio();
        self.follow_media();
        let discord_wants = self.follow_discord();
        // The Guide closed by a press keeps the foreground until the button is up, so the window that gets it back
        // never sees the press; or for a second, if no release is seen (docs/spec/guide.md, 3.5).
        let mut release_wants = None;
        if self.dashboard.awaiting_release() {
            let waited = self.release_wait.get_or_insert_with(Instant::now).elapsed();
            if waited >= screens::RELEASE_FALLBACK {
                eprintln!("guide: the button that closed it is still down after a second: handing back all the same");
                self.dashboard.release_wait_over();
            } else {
                release_wants = Some(screens::RELEASE_FALLBACK - waited);
            }
        }
        if !self.dashboard.awaiting_release() {
            self.release_wait = None;
        }
        // The game is let go before it is handed the foreground, so it can take it; and held only once the Overlay
        // has taken the foreground from it, as the Bundle orders the two.
        if self.freezer.is_frozen() && self.dashboard.overlay() != OverlayShows::Guide {
            self.freezer.thaw();
        }
        let home_hides = self.hand_back();
        let overlay_wants = self.follow_overlay();
        self.ledger.enter(Part::Toasts);
        let toasts_want = self.follow_toasts();
        self.say_toasts();
        self.ledger.enter(Part::Launch);
        if let Some(launch) = &mut self.launch {
            // The game has the foreground again, from the Overlay: Home's window goes behind it as it was.
            if home_hides {
                launch.show_game(false);
            }
            // Said once the Overlay has done what was asked of it, so the log's order is what happened.
            if !self.dashboard.launch_screen_showing() {
                launch.launch_screen_gone();
            }
            // A game being quit is not held still, so it is free to close; one the user gave up quitting is again.
            launch.quitting(self.dashboard.quitting());
            let over_game = self.dashboard.guide_over_game();
            launch.guide(over_game);
            if over_game && self.overlay.as_ref().is_some_and(|overlay| overlay.mode() == Some(Mode::Guide)) {
                launch.freeze(&mut self.freezer);
            }
        }

        self.ledger.enter(Part::End);
        self.follow_boot_video();
        let Some(state) = &self.state else { return };
        if let Some((frames, since)) = &mut self.frames {
            if since.elapsed() >= FRAMES_EVERY {
                let [home, overlay, toasts] = *frames;
                let seconds = since.elapsed().as_secs_f32();
                eprintln!("frames: home {home}, overlay {overlay}, toasts {toasts} in {seconds:.1} s");
                for (name, times) in ["home", "overlay"].iter().zip(&mut self.frame_times) {
                    if times.frames > 0 {
                        eprintln!("frame times: {name}: {}", times.said());
                    }
                }
                let unsaid = self.late.take_unsaid();
                if unsaid > 0 {
                    eprintln!("frame late: {unsaid} more, not said one by one");
                }
                (*frames, *since) = ([0; 3], Instant::now());
            }
        }
        // Home's window is drawn when something changed or is moving, and when the minute changes for the clock; behind
        // a game or under its Launch screen only as the game's backdrop begins to cover it. The loop looks
        // every IDLE at most, which is how late the clock can be.
        let due = minute() != self.minute;
        // The Idle screen's time is on the Overlay.
        if due && self.dashboard.idle_stage() == screens::idle::Stage::ShowingThings {
            self.overlay_stale = true;
        }
        // And when what was drawn last asked to be drawn again: a delay's end, the swirl's next step.
        let covered = self.dashboard.home_covered();
        let wake = state.host.wake().filter(|_| !covered).map(Duration::from_secs_f32);
        let woken = wake.is_some_and(|wake| self.last_frame.elapsed() >= wake);
        let mut home_wants = None;
        // The strip is drawn before Home, above. On a slow renderer Home also gives way to a Toast (its swirl holds
        // still, which the Dashboard sees to), and a frame of Home that does come is not begun while the Toast is
        // moving, nor when the Toast would move, or the queue change, before that frame was done: a frame of Home
        // keeps the renderer from the strip for as long as it takes. Home is not kept longer than HOME_HELD after
        // its last frame.
        let frame_takes = self.home_took.mul_f32(1.25);
        let queue = self.dashboard.toasts().wake().map(Duration::from_secs_f32);
        let toast_soon = [toasts_want, queue].into_iter().flatten().any(|wait| wait < frame_takes);
        let toast_soon = toast_soon || self.dashboard.toasts().awaits_frame();
        let held = HOME_HELD.saturating_sub(self.home_done.elapsed());
        // Behind a game Home's window is its backdrop: drawn when it begins to cover, and left.
        if self.backdrop.due(covered) {
            state.window.request_redraw();
        } else if !covered && (self.stale || pressed || due || woken || state.host.animating()) {
            if self.dashboard.home_gives_way() && toast_soon && !held.is_zero() {
                self.stale |= pressed || due;
                home_wants = Some(held);
            } else {
                state.window.request_redraw();
            }
        } else if let Some(wake) = wake {
            home_wants = Some(wake.saturating_sub(self.last_frame.elapsed()));
        }
        // Woken sooner by the pad's thread, the art cache and the windows' messages. During a launch, in time for
        // its next look at the desktop.
        let launch = self.launch.as_ref().map(|launch| launch.wait());
        // And in time for the Toasts' next change: one goes, the next shows.
        let toasts = queue;
        let timer = self.dashboard.timer().map(Duration::from_secs_f32);
        // And on the moment the Idle screen dims, shows things or turns a card.
        let idle = self.dashboard.idle_timer().map(Duration::from_secs_f32);
        let wants = [
            Some(IDLE),
            launch,
            overlay_wants,
            toasts_want,
            toasts,
            home_wants,
            release_wants,
            timer,
            audio_wants,
            idle,
            discord_wants,
        ];
        let wait = wants.into_iter().flatten().min().unwrap_or(IDLE);
        let wake = Instant::now() + wait;
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
        self.ledger.wake_at(wake);
        self.ledger.away();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        self.ledger.back(Part::Events);
        self.window_event_billed(event_loop, event);
        self.ledger.away();
    }
}

impl App {
    /// A window event, its time billed to `Part::Events`, Home's frames' to their parts.
    fn window_event_billed(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => {
                eprintln!("window: asked to close (WM_CLOSE to Home's window) at {:.0} ms", since_start());
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let button = match event.logical_key {
                    Key::Named(NamedKey::ArrowLeft) => Button::Left,
                    Key::Named(NamedKey::ArrowRight) => Button::Right,
                    Key::Named(NamedKey::ArrowUp) => Button::Up,
                    Key::Named(NamedKey::ArrowDown) => Button::Down,
                    Key::Named(NamedKey::Enter) => Button::A,
                    Key::Named(NamedKey::Backspace) => {
                        self.keys.push(KeyPress::Backspace { repeat: event.repeat });
                        return;
                    }
                    // Esc is B, and at Home with nothing open it is the way out. On a page it closes what B
                    // closes: text entry first, dropping the text (docs/spec/search.md, 8.4).
                    Key::Named(NamedKey::Escape) => {
                        let nothing_open = !self.dashboard.guide_open()
                            && self.dashboard.pages_up() == 0
                            && !self.dashboard.starting_up();
                        if nothing_open && self.dashboard.playing().is_none() {
                            event_loop.exit();
                        }
                        Button::B
                    }
                    // Any other key is what it types, Space among them: a page that takes text gets it as typed.
                    _ => {
                        if let Some(text) = event.text {
                            self.keys.push(KeyPress::Text(text.to_string()));
                        }
                        return;
                    }
                };
                self.keys.push(KeyPress::Button(Press { button, repeat: event.repeat }));
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state.config.width = size.width;
                state.config.height = size.height;
                state.surface.configure(&state.gpu.device, &state.config);
                self.stale = true;
                self.backdrop.again(self.dashboard.home_covered(), "Home's window resized");
            }
            // Alt+Tab to the Dashboard while the game is played: Home in front of it, as Home chosen in the Guide.
            WindowEvent::Focused(true) if self.launch.as_ref().is_some_and(Launch::game_in_front) => {
                if !self.dashboard.guide_open() {
                    eprintln!("launch: Home's window chosen over the game at {:.0} ms", since_start());
                    self.dashboard.show_dashboard();
                    self.stale = true;
                }
            }
            // What the backdrop drew may be gone: drawn again when Home's window is activated (as the game's goes),
            // shown again or rescaled.
            WindowEvent::Focused(true) => self.backdrop.again(self.dashboard.home_covered(), "Home's window activated"),
            WindowEvent::Occluded(false) => self.backdrop.again(self.dashboard.home_covered(), "Home's window shown"),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.backdrop.again(self.dashboard.home_covered(), "Home's window rescaled")
            }
            // Behind a game or under its Launch screen Home's window is the game's backdrop, drawn once.
            WindowEvent::RedrawRequested if self.backdrop.skip(self.dashboard.home_covered()) => {}
            WindowEvent::RedrawRequested => {
                let began = since_start();
                let turn = Instant::now();
                self.ledger.enter_at(Part::HomeWait, turn);
                self.stale = false;
                self.minute = minute();
                if let Some((frames, _)) = &mut self.frames {
                    frames[0] += 1;
                }

                let frame = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                        frame
                    }
                    wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                        // A turn all the same: what asked for it (a wake) is answered, and not asked again at once.
                        self.last_frame = Instant::now();
                        // Nothing of the window shows: a Toast drawn in it passes unseen, in its time.
                        if self.toast_strip.is_none() {
                            self.dashboard.toasts_presented(told(&mut self.told_time));
                        }
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        self.last_frame = Instant::now();
                        state.surface.configure(&state.gpu.device, &state.config);
                        self.stale = true;
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
                };
                // The picture is of the moment the frame had its buffer, as the Overlay's is (#99): read before the
                // wait, the clock would put a long wait (the first frame of a stretch) into the next frame's step,
                // and the movement would jump by it between two frames a refresh apart.
                let now = Instant::now();
                let acquired = now;
                self.ledger.enter_at(Part::HomeDraw, acquired);
                let spent = self.frames.is_some().then(|| self.ledger.totals_at(acquired));
                // From rest an animation starts with a tenth of a second at most, however long Home sat still. One
                // that was moving has moved on by as long as this frame is after the last, as the Overlay's does: a
                // fade takes its time by the clock however few frames a slow renderer gives it.
                let passed = (now - self.last_frame).as_secs_f32();
                let dt = passed.min(if self.moving { 1.0 } else { 0.1 });
                state.host.store.slept(passed - dt);
                self.last_frame = now;
                let view = frame.texture.create_view(&Default::default());
                let size = (state.config.width, state.config.height);
                if let Some(playing) = &mut self.boot_video {
                    let refresh = state.window.current_monitor().and_then(|m| m.refresh_rate_millihertz());
                    let refresh = refresh.map_or(1.0 / 60.0, |rate| 1000.0 / f64::from(rate.max(1)));
                    playing.before_frame(&mut state.host, &state.gpu, &mut self.dashboard, refresh);
                }
                let dashboard = &mut self.dashboard;
                state.host.draw(&state.gpu, &view, size, dt, wgpu::Color::BLACK, |ui| dashboard.draw(ui));
                self.draw_cost[0] = state.host.cost;
                self.ledger.enter(Part::HomePresent);
                state.gpu.queue.present(frame);
                self.ledger.enter(Part::HomeAfter);
                if let Some(playing) = &mut self.boot_video {
                    playing.after_present();
                }
                Self::say_startup(&self.dashboard, &mut self.startup_said);
                if self.home_presented.is_none() && !self.dashboard.starting_up() {
                    self.home_presented = Some(Instant::now());
                    eprintln!("startup: Home's first frame at {:.0} ms", since_start());
                    // Start-up is over: the Dashboard is a window among others again.
                    if self.as_shell.topmost_start_up() {
                        state.window.set_window_level(winit::window::WindowLevel::Normal);
                    }
                }
                if let Some(totals) = spent {
                    let spent = totals.since(&std::mem::replace(&mut self.frame_spent[0], totals));
                    if let Some(gap) = self.frame_times[0].frame(turn, acquired, Instant::now(), self.moving) {
                        self.late.say("home", gap, &spent, draws(&self.draw_cost));
                    }
                }
                (self.home_took, self.home_done) = (turn.elapsed(), Instant::now());
                // Without a strip the Toast was in that frame.
                if self.toast_strip.is_none() {
                    self.dashboard.toasts_presented(told(&mut self.told_time));
                }
                self.moving = state.host.animating();
                let art = &state.host.art;
                self.hub_open.frame(turn, acquired, Instant::now(), state.host.cost, self.moving, art);
                let (covered, showing) = (self.dashboard.home_covered(), self.dashboard.backdrop_showing());
                self.backdrop.presented(covered, showing, self.moving, self.dashboard.backdrop_whole(), size);
                if self.presented.is_none() {
                    self.presented = Some(Instant::now());
                    let [library, window, gpu, renderer] = state.ready;
                    eprintln!(
                        "start-up: library at {library:.0} ms, window at {window:.0} ms, GPU at {gpu:.0} ms, renderer \
                         at {renderer:.0} ms, first frame begun at {began:.0} ms"
                    );
                    eprintln!("first frame presented at {:.0} ms since process start", since_start());
                    // Shell mode's --shell process takes its black down; with no Boot video playing, explorer may
                    // start behind the Dashboard now.
                    self.as_shell.first_frame();
                    if self.boot_video.is_none() {
                        self.as_shell.boot_video_ended();
                    }
                    self.log_profile();
                    if self.log_focus {
                        self.log_groups();
                    }
                    // The sources are read once the first frame is out, and wake the window if they have news.
                    let proxy = self.proxy.clone();
                    self.store.sync_in_background(self.dashboard.library(), move || {
                        let _ = proxy.send_event(());
                    });
                    let proxy = self.proxy.clone();
                    self.store.list_store_apps(move || {
                        let _ = proxy.send_event(());
                    });
                    // And the drives and the stores' install folders, for the storage ring and Settings > Storage.
                    self.look_at_storage();
                    // And the Queue is followed from then on, waking the window as it changes.
                    let proxy = self.proxy.clone();
                    self.store.follow_queue(move || {
                        let _ = proxy.send_event(());
                    });
                }
            }
            _ => {}
        }
    }
}

/// The window's HWND; null anywhere but Windows.
fn window_handle(window: &Window) -> *mut std::ffi::c_void {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as *mut std::ffi::c_void,
        _ => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A launch's looks at the desktop (`shell::launch`): one a turn of the loop, 200 ms after the last at the
    /// soonest, and one that comes 500 ms or more after the last counts for nothing.
    const LOOK_EVERY: Duration = Duration::from_millis(200);
    const LATE: Duration = Duration::from_millis(500);

    /// The loop while the Launch screen is up and only its Spinner turns, on a renderer whose every frame holds
    /// the loop for `frame`: how many looks came on time, of how many, in `run`. `timed` is what the frame is
    /// said to have taken.
    fn looks(frame: Duration, timed: impl Fn(Duration) -> Duration, run: Duration) -> (u32, u32) {
        let start = Instant::now();
        let (mut now, mut looked, mut next_look, mut due) = (start, start, start, start);
        let (mut on_time, mut all) = (0, 0);
        while now - start < run {
            // The turn: the look, if it is time for one, then the frame, if it is time for one.
            if now >= next_look {
                all += 1;
                on_time += (now - looked < LATE) as u32;
                (looked, next_look) = (now, now + LOOK_EVERY);
            }
            if now >= due {
                let began = now;
                now += frame;
                due = overlay_due(began, timed(frame));
            }
            // Asleep until the next of the two.
            now = now.max(next_look.min(due));
        }
        (on_time, all)
    }

    #[test]
    fn a_slow_renderers_frames_leave_the_loop_its_looks_at_the_desktop() {
        // The software renderer on the test VM: 600 ms a frame, nearly all of it the wait for the swap chain.
        let frame = Duration::from_millis(600);
        let (on_time, all) = looks(frame, |took| took, Duration::from_secs(12));
        // A frame, late, then its rest: nine looks of every ten are on time, and 5 s of them (twenty-five) come
        // in well under ten.
        assert!(on_time >= 40 && on_time * 10 >= all * 8, "{on_time} of {all} looks on time");
        // Timed by its drawing alone, as it was: the frames follow one another, and no look is on time but the
        // first, which nothing came before.
        let (on_time, all) = looks(frame, |_| Duration::from_millis(5), Duration::from_secs(12));
        assert_eq!((on_time, all), (1, 20));
    }

    #[test]
    fn a_quick_frame_is_followed_at_once_and_a_slow_one_by_its_rest() {
        let began = Instant::now();
        let quick = Duration::from_millis(16);
        assert_eq!(overlay_due(began, quick), began + quick);
        let slow = Duration::from_millis(600);
        assert_eq!(overlay_due(began, slow), began + slow * 4);
    }

    /// A window's frames, through `FrameTimes`: each `(wait, draw, between)` in ms, the first after a rest, every
    /// other after one that left something moving. `between` is the loop's time from a frame's present to the next
    /// frame's turn.
    fn timed(frames: &[(u64, u64, u64)]) -> FrameTimes {
        let ms = Duration::from_millis;
        let mut times = FrameTimes::default();
        let mut at = Instant::now();
        for (n, &(wait, draw, between)) in frames.iter().enumerate() {
            at += ms(between);
            let (began, acquired) = (at, at + ms(wait));
            at = acquired + ms(draw);
            times.frame(began, acquired, at, n > 0);
        }
        times
    }

    #[test]
    fn the_wait_before_a_stretchs_first_frame_is_said_apart_and_is_no_late_frame() {
        // The Guide as it was measured on the TV PC (#99): its first frame waits two refreshes for its buffer, then
        // every frame one. Counted from each turn's start, as it was, the second frame's gap took the first one's
        // wait in and was late, though the two pictures were a refresh apart.
        let mut frames = vec![(16, 0, 0)];
        frames.extend([(8, 0, 0); 30]);
        let mut times = timed(&frames);
        assert_eq!((times.moving, times.late, times.from_rest), (30, 0, 1));
        assert_eq!((times.gap_most, times.from_rest_wait_most), (Duration::from_millis(8), Duration::from_millis(16)));
        let said = times.said();
        assert!(said.contains("moving: 30 frames at 125 a second, longest gap 8.0 ms (8.0 of it its wait), 0 late"));
        assert!(said.ends_with("; from rest: 1 frames, their wait most 16.0 ms"), "{said}");
        // Said and forgotten, all but when the last frame was.
        assert_eq!((times.frames, times.moving, times.from_rest), (0, 0, 0));
        assert!(times.last.is_some());
    }

    #[test]
    fn a_refresh_missed_in_a_stretch_is_late_and_says_where_its_time_went() {
        // The loop slow to come back once (another window's frame, a sleep too long): late, its wait a refresh.
        let mut times = timed(&[(0, 1, 0), (7, 1, 0), (7, 1, 15), (7, 1, 0), (21, 1, 0), (7, 1, 0)]);
        assert_eq!((times.moving, times.late), (5, 2));
        assert!(times.said().contains("longest gap 23.0 ms (7.0 of it its wait), 2 late"));
        // A wait of over two refreshes in the middle of a stretch: late, nearly all of it the wait.
        let mut times = timed(&[(0, 1, 0), (7, 1, 0), (21, 1, 0), (7, 1, 0)]);
        assert_eq!((times.moving, times.late), (3, 1));
        assert!(times.said().contains("longest gap 22.0 ms (21.0 of it its wait), 1 late"));
    }

    #[test]
    fn a_frame_after_a_rest_starts_a_stretch_however_long_the_rest() {
        let ms = Duration::from_millis;
        let mut times = FrameTimes::default();
        let at = Instant::now();
        // A stretch, a second's rest, another stretch: the rest is neither a gap nor late.
        times.frame(at, at, at + ms(1), false);
        times.frame(at + ms(1), at + ms(8), at + ms(9), true);
        let later = at + Duration::from_secs(1);
        times.frame(later, later + ms(3), later + ms(4), false);
        times.frame(later + ms(4), later + ms(11), later + ms(12), true);
        assert_eq!((times.moving, times.late, times.from_rest), (2, 0, 2));
        assert_eq!((times.gap_most, times.from_rest_wait_most), (ms(8), ms(3)));
    }
}
