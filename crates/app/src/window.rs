//! The window: borderless fullscreen on the primary monitor, or 1280x720 with `--windowed`. It feeds the Dashboard
//! the pad and the keyboard, draws it when something has changed, and carries out what it asks of the shell.
use std::sync::Arc;
use std::time::{Duration, Instant};

use input::Pad;
use model::Library;
use render::gpu::{self, Gpu};
use screens::{Dashboard, Effect};
use shell::{Launch, Showing};
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
/// How often a launch is looked at: the game's window appearing, booting, ending.
const LAUNCH_EVERY: Duration = Duration::from_millis(8);
/// The same behind a running game.
const LAUNCH_EVERY_HIDDEN: Duration = Duration::from_millis(30);

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
    /// Starts a game through Steam, the Dashboard showing its Launch screen.
    fn launch(&mut self, game: usize) {
        let Some(state) = &self.state else { return };
        if self.launch.is_some() {
            return;
        }
        let Some(found) = self.dashboard.library().games.get(game) else { return };
        let Some(dir) = &found.install_dir else {
            eprintln!("launch: {} isn't installed", found.name);
            return;
        };
        if let Some(launch) = Launch::start(found.appid, &found.name, dir, window_handle(&state.window)) {
            self.launch = Some(launch);
            self.dashboard.launch_started(game);
        }
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
                // Over a game, the window hides at once and the game gets the foreground back.
                Effect::GuideClosed => {
                    if let (Some(launch), Some(state)) = (&mut self.launch, &mut self.state) {
                        launch.close_guide();
                        state.host.store.clear();
                    }
                }
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
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = &mut self.state else { return };
        let mut presses = self.pad.poll();
        presses.append(&mut self.keys);
        if let Some(library) = self.store.changed() {
            self.dashboard.set_library(library);
            self.stale = true;
        }
        let mut wait = if self.launch.is_some() { LAUNCH_EVERY } else { IDLE };
        let mut hidden = false;

        if let Some(launch) = &mut self.launch {
            match launch.tick() {
                // The Launch screen, or its fade: keep drawing.
                Showing::LaunchScreen => self.stale = true,
                // Nothing to draw behind a running game: only the Guide button does anything.
                Showing::Nothing => {
                    self.dashboard.game_booted();
                    let playing = self.dashboard.playing();
                    self.store.game_booted(playing.and_then(|game| self.dashboard.library().games.get(game)));
                    presses.retain(|p| p.button == Button::Guide && !p.repeat);
                    if presses.is_empty() {
                        hidden = true;
                        wait = LAUNCH_EVERY_HIDDEN;
                    } else {
                        launch.open_guide();
                        self.last_frame = Instant::now();
                    }
                }
                Showing::Guide => {
                    let size = (state.config.width, state.config.height);
                    let panel = self.dashboard.guide_panel_width() * ui::scale_for(size);
                    launch.clip_to_guide(panel.round() as u32);
                }
                Showing::Home => {
                    self.launch = None;
                    self.dashboard.game_ended();
                    if let Some(library) = self.store.game_ended() {
                        self.dashboard.set_library(library);
                    }
                    state.host.store.clear();
                    self.last_frame = Instant::now();
                    self.stale = true;
                }
            }
        }

        let pressed = !presses.is_empty();
        for press in presses {
            self.dashboard.press(press);
        }
        self.carry_out(event_loop);

        let Some(state) = &self.state else { return };
        // Drawn when something changed or is moving, and once a second for the clock.
        let due = self.last_frame.elapsed() >= Duration::from_secs(1);
        if !hidden && (self.stale || pressed || due || state.host.animating()) {
            state.window.request_redraw();
        }
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
