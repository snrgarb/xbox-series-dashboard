//! The windows: Home's, borderless fullscreen on the primary monitor or 1280x720 with `--windowed`, and the Overlay
//! above every other window, which shows what the Dashboard wants over a game (docs/ui.md). This feeds the Dashboard
//! the pad and the keyboard, draws each window when what it shows has changed, and carries out what the Dashboard
//! asks of the shell.
use std::sync::Arc;
use std::time::{Duration, Instant};

use input::Pad;
use model::Library;
use render::gpu::{self, Gpu};
use screens::{Dashboard, Effect, OverlayShows};
use shell::{Key as OverlayKey, Launch, Mode, Overlay, Step};
use ui::{Button, Host, Press};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::store::Store;
use crate::{since_start, Args};

/// How long the loop sleeps with nothing to draw: the pad's thread, the art cache and the window wake it sooner.
const IDLE: Duration = Duration::from_millis(250);
/// How soon the Overlay is tried again when it had no frame to draw on.
const OVERLAY_RETRY: Duration = Duration::from_millis(16);

struct State {
    host: Host,
    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    gpu: Gpu,
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
    state: Option<State>,
    proxy: EventLoopProxy<()>,
    last_frame: Instant,
    /// A frame is wanted whatever else: the first, or after a resize.
    stale: bool,
    presented: bool,
    launch: Option<Launch>,
    /// The library database: where a newer snapshot comes from, and where Play sessions go.
    store: Store,
}

pub fn run(args: &Args, library: Library, store: Store) {
    shell::log_wgpu();
    let event_loop = EventLoop::new().unwrap_or_else(|e| crate::fail(&format!("no display: {e}")));
    let mut dashboard = Dashboard::new(library);
    dashboard.set_clock(args.clock.clone());
    for button in args.focus_presses() {
        dashboard.press(Press { button, repeat: false });
    }
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
        state: None,
        proxy: event_loop.create_proxy(),
        last_frame: Instant::now(),
        stale: true,
        presented: false,
        launch: None,
        store,
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
            if cfg!(windows) {
                eprintln!("overlay: none: games start with no Launch screen and the Guide opens only at Home");
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
            }
            overlay.set(mode);
            self.overlay_stale = true;
        }
        mode?;
        let moving = self.overlay_store.animating() || state.host.art.pending() > 0;
        if !self.overlay_stale && !moving {
            return None;
        }
        let Some(frame) = overlay.frame(&state.gpu) else {
            if overlay.lost() {
                // It said why. From here on the Dashboard shows nothing that needs it.
                self.overlay = None;
                self.dashboard.set_overlay(false);
                return None;
            }
            return Some(OVERLAY_RETRY);
        };
        let now = Instant::now();
        let dt = (now - self.overlay_last_frame).as_secs_f32().min(0.1);
        self.overlay_last_frame = now;
        self.overlay_stale = false;
        let view = frame.texture.create_view(&Default::default());
        let (gpu, size, dashboard) = (&state.gpu, overlay.size(), &mut self.dashboard);
        // Cleared to nothing: where the Dashboard draws nothing, the game shows.
        state.host.with_store(&mut self.overlay_store, |host| {
            host.draw(gpu, &view, size, dt, wgpu::Color::TRANSPARENT, |ui| dashboard.draw_overlay(ui))
        });
        overlay.present(gpu, frame);
        // At once if something is moving, or if that frame was the last of something: the Overlay is then hidden
        // without waiting out the loop's sleep.
        let moving = self.overlay_store.animating() || state.host.art.pending() > 0;
        (moving || wanted(&self.dashboard) != mode).then_some(Duration::ZERO)
    }

    fn carry_out(&mut self, event_loop: &ActiveEventLoop) {
        for effect in self.dashboard.take_effects() {
            match effect {
                Effect::Launch(game) => self.launch(game),
                Effect::QuitGame => {
                    if let Some(launch) = &self.launch {
                        launch.quit_game();
                    }
                }
                // Over a game the Overlay follows what the Dashboard shows: it gives the foreground back as the
                // Guide closes, and hides once the panel has slid away.
                Effect::GuideClosed => {}
                Effect::Exit => event_loop.exit(),
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

        // A piece of art decoded on another thread asks for a frame.
        let proxy = self.proxy.clone();
        let host = Host::new(&gpu, config.format, move || {
            let _ = proxy.send_event(());
        });
        self.state = Some(State { host, config, surface, gpu, window });
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        self.stale = true;
        self.overlay_stale = true;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            return;
        }
        // After Home's first frame, so the Overlay costs the start-up nothing.
        if self.presented {
            self.make_overlay();
        }
        let mut presses = self.pad.poll();
        presses.append(&mut self.keys);
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
                    let playing = self.dashboard.playing();
                    self.store.game_booted(playing.and_then(|game| self.dashboard.library().games.get(game)));
                }
                Step::Ended => {
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

        let pressed = !presses.is_empty();
        for press in presses {
            self.dashboard.press(press);
        }
        self.overlay_stale |= pressed;
        self.carry_out(event_loop);
        let overlay_wants = self.follow_overlay();
        if let Some(launch) = &mut self.launch {
            // Said once the Overlay has done what was asked of it, so the log's order is what happened.
            if !self.dashboard.launch_screen_showing() {
                launch.launch_screen_gone();
            }
            launch.guide(self.dashboard.guide_over_game());
        }

        let Some(state) = &self.state else { return };
        // Home's window is drawn when something changed or is moving, and once a second for the clock; not at all
        // behind a game or under its Launch screen, where it keeps the last frame it showed.
        let due = self.last_frame.elapsed() >= Duration::from_secs(1);
        if !self.dashboard.home_covered() && (self.stale || pressed || due || state.host.animating()) {
            state.window.request_redraw();
        }
        // Woken sooner by the pad's thread, the art cache and the windows' messages. During a launch, in time for
        // its next look at the desktop.
        let launch = self.launch.as_ref().map(|launch| launch.wait());
        let wait = [Some(IDLE), launch, overlay_wants].into_iter().flatten().min().unwrap_or(IDLE);
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
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.1);
                self.last_frame = now;
                self.stale = false;

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
                if !self.presented {
                    self.presented = true;
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
