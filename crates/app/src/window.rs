//! The windows: Home's, borderless fullscreen on the primary monitor or 1280x720 with `--windowed`, and the Overlay
//! above every other window, which shows what the Dashboard wants over a game (docs/ui.md). This feeds the Dashboard
//! the pad and the keyboard, draws each window when what it shows has changed, and carries out what the Dashboard
//! asks of the shell.
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use input::Pad;
use model::{Library, Sound};
use render::gpu::{self, Gpu};
use screens::{Dashboard, Effect, OverlayShows, Setting};
use shell::power::{DryPower, Power, SystemPower};
use shell::{Freezer, Key as OverlayKey, Launch, Mode, Overlay, Step};
use ui::{Button, Host, Press};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::sounds::Sounds;
use crate::store::Store;
use crate::{since_start, Args};

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
    dashboard: Dashboard,
    pad: Pad,
    /// Keys pressed since the pad was last looked at, as the buttons they stand for.
    keys: Vec<Press>,
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
    state: Option<State>,
    proxy: EventLoopProxy<()>,
    last_frame: Instant,
    /// A frame is wanted whatever else: the first, or after a resize.
    stale: bool,
    /// When Home's first frame was presented.
    presented: Option<Instant>,
    launch: Option<Launch>,
    /// Holds the running game still while the Guide is open over it, and lets it go as the Guide hands the
    /// foreground back (docs/spec/guide.md, 7.2). After `launch` here, so it outlives it.
    freezer: Freezer,
    /// Since when the Guide has been closed with the button that closed it still down.
    release_wait: Option<Instant>,
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
    /// What turns the machine off, restarts it and puts it to sleep: for real, or with --dry-power in the log only.
    power: Box<dyn Power>,
    /// How long the sound of leaving has still to play: Exit waits for it.
    leaving: Duration,
    /// The minute of the day Home's last frame showed on its clock.
    minute: u64,
    /// With --log-frames: the frames Home's window and the Overlay drew since they were last said, and when.
    frames: Option<(u32, u32, Instant)>,
}

/// The minute it is, counted from 1970: what the clock's next change is told by.
fn minute() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() / 60)
}

/// The longest Exit waits for the sound of leaving.
const LEAVING_AT_MOST: Duration = Duration::from_secs(4);

/// How often --log-frames says what was drawn.
const FRAMES_EVERY: Duration = Duration::from_secs(5);

pub fn run(args: &Args, library: Library, store: Store) {
    shell::log_wgpu();
    let event_loop = EventLoop::new().unwrap_or_else(|e| crate::fail(&format!("no display: {e}")));
    let mut dashboard = Dashboard::new(library);
    dashboard.set_clock(args.clock.clone());
    // Settings' options, as they were left (docs/spec/settings.md, section 7).
    let options = store.options();
    if !options.use_guide {
        eprintln!("guide: off ({}): the Guide button does nothing", Setting::UseGuide.key());
    }
    dashboard.set_options(options);
    // A Dashboard that was killed with the Guide open over a game left the game frozen: it is let go first of all.
    let data_dir = args.data_dir.clone().or_else(library::data_dir);
    let mut freezer = Freezer::new(data_dir.as_deref());
    freezer.thaw_leftovers();
    for button in args.focus_presses() {
        dashboard.press(Press { button, repeat: false });
    }
    // Where --focus put focus is where the Dashboard starts: nothing is heard of getting there.
    dashboard.take_effects();
    let mut app = App {
        windowed: args.windowed,
        dashboard,
        pad: Pad::start({
            let proxy = event_loop.create_proxy();
            move || {
                let _ = proxy.send_event(());
            }
        }),
        keys: Vec::new(),
        overlay: None,
        overlay_tried: false,
        overlay_store: ui::Store::default(),
        overlay_last_frame: Instant::now(),
        overlay_stale: false,
        overlay_moving: false,
        overlay_next: Instant::now(),
        state: None,
        proxy: event_loop.create_proxy(),
        last_frame: Instant::now(),
        stale: true,
        presented: None,
        launch: None,
        freezer,
        release_wait: None,
        sounds: Sounds::open(args, &store),
        store,
        library_ready: since_start(),
        moving: false,
        log_focus: args.log_focus,
        // The one place the machine's own power is in reach: a test or an offscreen frame never comes here.
        power: if args.dry_power { Box::new(DryPower::default()) } else { Box::new(SystemPower) },
        leaving: Duration::ZERO,
        minute: minute(),
        frames: args.log_frames.then(|| (0, 0, Instant::now())),
    };
    event_loop.run_app(&mut app).expect("event loop");
}

impl App {
    /// Makes the Overlay, the first time it is called. Where there can't be one the Dashboard is told, and shows
    /// nothing that would need it.
    fn make_overlay(&mut self) {
        let Some(state) = &self.state else { return };
        if self.overlay_tried {
            return;
        }
        self.overlay_tried = true;
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
    }

    /// Starts a game through Steam, the Dashboard showing its Launch screen.
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
        let home = window_handle(&state.window);
        if let Some(launch) = Launch::start(found.appid, &found.name, dir, home, self.overlay.is_some()) {
            self.launch = Some(launch);
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
            overlay.set(mode);
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
        let Some(frame) = overlay.frame(&state.gpu) else {
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
        // Something that was moving has moved on by as long as this frame is after the last, so a fade takes its
        // time by the clock however few frames a slow renderer gives it; from rest, a frame starts what it starts.
        let dt = (now - self.overlay_last_frame).as_secs_f32().min(if self.overlay_moving { 1.0 } else { 0.1 });
        self.overlay_last_frame = now;
        self.overlay_stale = false;
        if let Some((_, overlay, _)) = &mut self.frames {
            *overlay += 1;
        }
        let view = frame.texture.create_view(&Default::default());
        let (gpu, size, dashboard) = (&state.gpu, overlay.size(), &mut self.dashboard);
        // Cleared to nothing: where the Dashboard draws nothing, the game shows.
        state.host.with_store(&mut self.overlay_store, |host| {
            host.draw(gpu, &view, size, dt, wgpu::Color::TRANSPARENT, |ui| dashboard.draw_overlay(ui))
        });
        overlay.present(gpu, frame);
        let took = now.elapsed();
        self.overlay_next = now + if took > SLOW_FRAME { took * (1 + SLOW_FRAME_REST) } else { Duration::ZERO };
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

    fn carry_out(&mut self, event_loop: &ActiveEventLoop) {
        for effect in self.dashboard.take_effects() {
            match effect {
                Effect::Launch(game) => self.launch(game),
                Effect::QuitGame => {
                    if let Some(launch) = &mut self.launch {
                        launch.quit_game(&mut self.freezer);
                    }
                }
                // The Overlay follows what the Dashboard shows: it gives the foreground back as the Guide closes,
                // once the button that closed it is up, and hides once the panel has slid away.
                Effect::GuideClosed => {}
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
                // In force already: saved, so that it is still so the next time.
                Effect::Set(change) => self.store.set(change),
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Xbox Series Dashboard");
        let attributes = if self.windowed {
            attributes.with_inner_size(PhysicalSize::new(1280, 720))
        } else {
            attributes.with_fullscreen(Some(Fullscreen::Borderless(event_loop.primary_monitor())))
        };
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        window.set_cursor_visible(self.windowed);

        let window_ready = since_start();
        let instance = gpu::instance(Some(Box::new(event_loop.owned_display_handle())));
        let surface = instance.create_surface(window.clone()).expect("create surface");
        let gpu = gpu::init(instance, Some(&surface));
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .expect("the adapter can't draw to the window");
        // Not an sRGB target, as the offscreen one isn't: the renderer blends in gamma space.
        let formats = surface.get_capabilities(&gpu.adapter).formats;
        config.format = formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);

        let gpu_ready = since_start();

        // A piece of art decoded on another thread asks for a frame.
        let proxy = self.proxy.clone();
        let host = Host::new(&gpu, config.format, move || {
            let _ = proxy.send_event(());
        });
        let ready = [self.library_ready, window_ready, gpu_ready, since_start()];
        self.state = Some(State { host, config, surface, gpu, ready, window });
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        self.stale = true;
        self.overlay_stale = true;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            return;
        }
        // After Home's first frame, and once Home has come to rest or a while has passed, so the Overlay costs
        // neither the start-up nor the first art its time. A launch that comes sooner makes it then.
        if let (Some(presented), Some(state)) = (self.presented, &self.state) {
            let at_rest = !self.stale && !state.host.animating() && state.host.art.pending() == 0;
            if at_rest || presented.elapsed() >= OVERLAY_AFTER {
                self.make_overlay();
            }
        }
        let events = self.pad.poll();
        let mut presses = std::mem::take(&mut self.keys);
        if self.dashboard.set_pad(self.pad.status()) {
            self.stale = true;
            self.overlay_stale = true;
        }
        if let Some(library) = self.store.changed() {
            self.dashboard.set_library(library);
            self.stale = true;
            self.overlay_stale = true;
        }
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
                    self.dashboard.game_ended();
                    if let Some(library) = self.store.game_ended() {
                        self.dashboard.set_library(library);
                    }
                    self.last_frame = Instant::now();
                    self.stale = true;
                }
            }
        }

        let pressed = !presses.is_empty() || !events.is_empty();
        let focus = self.dashboard.focus_line();
        let guide = self.dashboard.guide_line();
        for event in events {
            self.dashboard.pad_event(event);
        }
        for press in presses {
            self.dashboard.press(press);
        }
        if self.log_focus && self.dashboard.focus_line() != focus {
            eprintln!("focus: {}", self.dashboard.focus_line());
        }
        if self.log_focus && self.dashboard.guide_line() != guide {
            eprintln!("guide: {}", self.dashboard.guide_line());
        }
        // A held direction repeats at the pace of whatever has the pad now.
        self.pad.set_repeat(self.dashboard.repeat());
        self.overlay_stale |= pressed;
        self.carry_out(event_loop);
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
        let overlay_wants = self.follow_overlay();
        if let Some(launch) = &mut self.launch {
            // Said once the Overlay has done what was asked of it, so the log's order is what happened.
            if !self.dashboard.launch_screen_showing() {
                launch.launch_screen_gone();
            }
            let over_game = self.dashboard.guide_over_game();
            launch.guide(over_game);
            if over_game && self.overlay.as_ref().is_some_and(|overlay| overlay.mode() == Some(Mode::Guide)) {
                launch.freeze(&mut self.freezer);
            }
        }

        let Some(state) = &self.state else { return };
        if let Some((home, overlay, since)) = &mut self.frames {
            if since.elapsed() >= FRAMES_EVERY {
                eprintln!("frames: home {home}, overlay {overlay} in {:.1} s", since.elapsed().as_secs_f32());
                (*home, *overlay, *since) = (0, 0, Instant::now());
            }
        }
        // Home's window is drawn when something changed or is moving, and when the minute changes for the clock; not
        // at all behind a game or under its Launch screen, where it keeps the last frame it showed. The loop looks
        // every IDLE at most, which is how late the clock can be.
        let due = minute() != self.minute;
        // And when what was drawn last asked to be drawn again: a delay's end, the swirl's next step.
        let covered = self.dashboard.home_covered();
        let wake = state.host.wake().filter(|_| !covered).map(Duration::from_secs_f32);
        let woken = wake.is_some_and(|wake| self.last_frame.elapsed() >= wake);
        let mut home_wants = None;
        if !covered && (self.stale || pressed || due || woken || state.host.animating()) {
            state.window.request_redraw();
        } else if let Some(wake) = wake {
            home_wants = Some(wake.saturating_sub(self.last_frame.elapsed()));
        }
        // Woken sooner by the pad's thread, the art cache and the windows' messages. During a launch, in time for
        // its next look at the desktop.
        let launch = self.launch.as_ref().map(|launch| launch.wait());
        let wants = [Some(IDLE), launch, overlay_wants, home_wants, release_wants];
        let wait = wants.into_iter().flatten().min().unwrap_or(IDLE);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + wait));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let button = match event.logical_key {
                    Key::Named(NamedKey::ArrowLeft) => Button::Left,
                    Key::Named(NamedKey::ArrowRight) => Button::Right,
                    Key::Named(NamedKey::ArrowUp) => Button::Up,
                    Key::Named(NamedKey::ArrowDown) => Button::Down,
                    Key::Named(NamedKey::Enter) => Button::A,
                    Key::Named(NamedKey::Backspace) => Button::B,
                    // Esc is B, and at Home with nothing open it is the way out.
                    Key::Named(NamedKey::Escape) => {
                        if !self.dashboard.guide_open() && self.dashboard.playing().is_none() {
                            event_loop.exit();
                        }
                        Button::B
                    }
                    _ => return,
                };
                self.keys.push(Press { button, repeat: event.repeat });
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state.config.width = size.width;
                state.config.height = size.height;
                state.surface.configure(&state.gpu.device, &state.config);
                self.stale = true;
            }
            // Behind a game or under its Launch screen nothing of Home's shows: nothing is drawn, whoever asked.
            WindowEvent::RedrawRequested if self.dashboard.home_covered() => {}
            WindowEvent::RedrawRequested => {
                let began = since_start();
                let now = Instant::now();
                // From rest an animation starts with a tenth of a second at most, however long Home sat still. One
                // that was moving has moved on by as long as this frame is after the last, as the Overlay's does: a
                // fade takes its time by the clock however few frames a slow renderer gives it.
                let passed = (now - self.last_frame).as_secs_f32();
                let dt = passed.min(if self.moving { 1.0 } else { 0.1 });
                state.host.store.slept(passed - dt);
                self.last_frame = now;
                self.stale = false;
                self.minute = minute();
                if let Some((home, ..)) = &mut self.frames {
                    *home += 1;
                }

                let frame = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                        frame
                    }
                    wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        state.surface.configure(&state.gpu.device, &state.config);
                        self.stale = true;
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
                };
                let view = frame.texture.create_view(&Default::default());
                let size = (state.config.width, state.config.height);
                let dashboard = &mut self.dashboard;
                state.host.draw(&state.gpu, &view, size, dt, wgpu::Color::BLACK, |ui| dashboard.draw(ui));
                state.gpu.queue.present(frame);
                self.moving = state.host.animating();
                if self.presented.is_none() {
                    self.presented = Some(Instant::now());
                    let [library, window, gpu, renderer] = state.ready;
                    eprintln!(
                        "start-up: library at {library:.0} ms, window at {window:.0} ms, GPU at {gpu:.0} ms, renderer \
                         at {renderer:.0} ms, first frame begun at {began:.0} ms"
                    );
                    eprintln!("first frame presented at {:.0} ms since process start", since_start());
                    // Steam is read once the first frame is out, and wakes the window if it has news.
                    let proxy = self.proxy.clone();
                    self.store.sync_in_background(self.dashboard.library(), move || {
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
