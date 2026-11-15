//! The window: borderless fullscreen on the primary monitor, or 1280x720 with `--windowed`.
use std::sync::Arc;
use std::time::{Duration, Instant};

use model::Library;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use input::Pad;
use render::gpu::{self, Gpu};
use screens::scene::{GuideAction, RECENT_COUNT};
use screens::{guide_entries, Home};
use shell::{Launch, Showing};

use crate::{since_start, Args};

struct State {
    home: Home,
    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    gpu: Gpu,
    // Last, so it outlives the surface made from it.
    window: Arc<Window>,
}

struct App {
    windowed: bool,
    focus: usize,
    library: Library,
    pad: Pad,
    state: Option<State>,
    last_frame: Instant,
    presented: bool,
    launch: Option<Launch>,
}

pub fn run(args: &Args, library: Library) {
    let event_loop = EventLoop::new().unwrap_or_else(|e| crate::fail(&format!("no display: {e}")));
    // Drawn every vertical blank, which also polls the gamepad: a spike's loop, not one that idles.
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        windowed: args.windowed,
        focus: args.focus,
        library,
        pad: Pad::new(),
        state: None,
        last_frame: Instant::now(),
        presented: false,
        launch: None,
    };
    event_loop.run_app(&mut app).expect("event loop");
}

impl App {
    /// A on a Recent row game: starts it through Steam, Home becoming its Launch screen.
    fn launch_focused(&mut self) {
        let Some(state) = &mut self.state else { return };
        if self.launch.is_some() {
            return;
        }
        let Some(game) = self.library.recent(RECENT_COUNT).get(state.home.scene.focus()) else { return };
        let Some(dir) = &game.install_dir else {
            eprintln!("launch: {} isn't installed", game.name);
            return;
        };
        if let Some(launch) = Launch::start(game.appid, &game.name, dir, window_handle(&state.window)) {
            state.home.scene.set_launching(true);
            self.launch = Some(launch);
        }
    }

    fn open_guide(&mut self) {
        let Some(state) = &mut self.state else { return };
        let playing = self.launch.is_some().then(|| state.home.scene.focus());
        state.home.scene.open_guide(guide_entries(&self.library, playing));
    }

    /// Closes the Guide; over a game, the window hides at once and the game gets the foreground back.
    fn close_guide(&mut self) {
        let Some(state) = &mut self.state else { return };
        state.home.scene.close_guide();
        if let Some(launch) = &mut self.launch {
            launch.close_guide();
        }
    }

    fn guide_select(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = &mut self.state else { return };
        match state.home.scene.guide_action() {
            Some(GuideAction::Close) | None => self.close_guide(),
            Some(GuideAction::QuitGame) => {
                if let Some(launch) = &self.launch {
                    launch.quit_game();
                }
                self.close_guide();
            }
            Some(GuideAction::Launch(game)) => {
                while state.home.scene.focus() != game {
                    let by = if state.home.scene.focus() > game { -1 } else { 1 };
                    if !state.home.scene.move_focus(by) {
                        break;
                    }
                }
                self.close_guide();
                self.launch_focused();
            }
            Some(GuideAction::Exit) => event_loop.exit(),
        }
    }
}

impl State {
    fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("Home");
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
        // An sRGB target, as the offscreen one is, so both blend alike.
        let formats = surface.get_capabilities(&gpu.adapter).formats;
        config.format = formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);

        let home = Home::new(&gpu, config.format, (config.width, config.height), &self.library, self.focus, || {});
        self.state = Some(State { home, config, surface, gpu, window });
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = &mut self.state else { return };
        let moves = self.pad.poll();
        if let Some(launch) = &mut self.launch {
            match launch.tick() {
                // While a game starts, Home takes no input.
                Showing::LaunchScreen => {
                    state.window.request_redraw();
                    return;
                }
                // Nothing to draw behind a running game: sleep between looks at the pad, for the Guide button.
                Showing::Nothing => {
                    if moves.guide {
                        launch.open_guide();
                        self.last_frame = Instant::now();
                        event_loop.set_control_flow(ControlFlow::Poll);
                        self.open_guide();
                    } else if state.home.scene.guide_panel(state.size()) > 0 {
                        // Its Guide still sliding away behind the game: let it finish, hidden.
                        state.home.scene.update(1.0);
                    } else {
                        let next = Instant::now() + Duration::from_millis(30);
                        event_loop.set_control_flow(ControlFlow::WaitUntil(next));
                    }
                    return;
                }
                Showing::Guide => {
                    launch.clip_to_guide(state.home.scene.guide_panel(state.size()));
                }
                Showing::Home => {
                    self.launch = None;
                    state.home.scene.set_launching(false);
                    state.home.scene.close_guide();
                    self.last_frame = Instant::now();
                    event_loop.set_control_flow(ControlFlow::Poll);
                    state.window.request_redraw();
                    return;
                }
            }
        }
        if state.home.scene.guide_open() {
            if moves.guide || moves.back {
                self.close_guide();
            } else if moves.select {
                self.guide_select(event_loop);
            } else if moves.down != 0 {
                state.home.scene.guide_move(moves.down);
            }
        } else if moves.guide {
            self.open_guide();
        } else if moves.select {
            self.launch_focused();
        } else if moves.by != 0 {
            state.home.scene.move_focus(moves.by.signum());
        }
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && (self.launch.is_none() || state.home.scene.guide_open()) =>
            {
                let guide = state.home.scene.guide_open();
                match event.logical_key {
                    Key::Named(NamedKey::Escape) if guide => self.close_guide(),
                    Key::Named(NamedKey::Enter) if guide => self.guide_select(event_loop),
                    Key::Named(NamedKey::ArrowUp) if guide => state.home.scene.guide_move(-1),
                    Key::Named(NamedKey::ArrowDown) if guide => state.home.scene.guide_move(1),
                    _ if guide => {}
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Named(NamedKey::Enter) => self.launch_focused(),
                    Key::Named(NamedKey::ArrowLeft) => drop(state.home.scene.move_focus(-1)),
                    Key::Named(NamedKey::ArrowRight) => drop(state.home.scene.move_focus(1)),
                    _ => {}
                }
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                state.config.width = size.width;
                state.config.height = size.height;
                state.surface.configure(&state.gpu.device, &state.config);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.1);
                self.last_frame = now;
                state.home.take_ready_art(&state.gpu);
                state.home.scene.update(dt);

                let frame = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                        frame
                    }
                    wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        state.surface.configure(&state.gpu.device, &state.config);
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
                };
                let view = frame.texture.create_view(&Default::default());
                state.home.draw(&state.gpu, &view, (state.config.width, state.config.height));
                state.gpu.queue.present(frame);
                if !self.presented {
                    self.presented = true;
                    eprintln!("first frame presented at {:.0} ms since process start", since_start());
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
