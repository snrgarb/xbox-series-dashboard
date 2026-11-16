//! The window: borderless full screen (or 1280x720 with `--windowed`), each decoded frame written to one texture
//! and drawn as a quad when the clock reaches its time. Any key ends it, as does the end of the file.
use std::sync::Arc;
use std::time::{Duration, Instant};

use render::gpu::{self, Gpu};
use render::{Layer, Quad, Rect, Renderer, TexId};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

use media::{Status, Video};

use crate::fit::letterbox;
use crate::{log, since_start, Play};

struct Screen {
    renderer: Renderer,
    texture: TexId,
    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    gpu: Gpu,
    /// A frame is shown this much before its time: half the display's refresh interval, so it lands on the refresh
    /// nearest its time.
    early: f64,
    // Last, so it outlives the surface made from it.
    window: Arc<Window>,
}

/// One second's worth of the figures the stats line averages.
#[derive(Default)]
struct Tally {
    frames: u32,
    decode_ms: f64,
    copy_ms: f64,
    uploads: u32,
    upload_ms: f64,
}

struct Player {
    windowed: bool,
    seconds: Option<f64>,
    size: (u32, u32),
    pixel_aspect: (u32, u32),
    video: Video,
    screen: Option<Screen>,
    ended: bool,
    /// When the first frame was presented.
    started: Option<Instant>,
    presented: u64,
    tally: Tally,
    last_stats: Instant,
    result: Result<(), String>,
}

pub fn run(args: &Play) -> Result<(), String> {
    if !args.file.is_file() {
        return Err(format!("{}: no such file", args.file.display()));
    }
    let options =
        media::Options { audio: !args.no_audio, volume: 1.0, dxva: args.dxva, basic_processing: args.basic_processing };
    let mut video = Video::open(&args.file, options);
    let media::Picture { size, pixel_aspect } = video.wait_open()?;

    let event_loop = EventLoop::new().map_err(|e| format!("no display: {e}"))?;
    // Drawn every vertical blank, where each frame's time is checked against the clock.
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut player = Player {
        windowed: args.windowed,
        seconds: args.seconds,
        size,
        pixel_aspect,
        video,
        screen: None,
        ended: false,
        started: None,
        presented: 0,
        tally: Tally::default(),
        last_stats: Instant::now(),
        result: Ok(()),
    };
    let run = event_loop.run_app(&mut player);
    let decoded = player.video.decoded();
    log!("video: done decoded={decoded} presented={} dropped={}", player.presented, player.video.dropped);
    run.map_err(|e| format!("event loop: {e}"))?;
    player.result
}

impl Player {
    /// The frame to show now, if one has come due: the latest whose time the clock has reached (`media::Video::due`).
    /// Notes whether the video has ended.
    fn due_frame(&mut self, early: f64) -> Option<media::Frame> {
        let due = self.video.due(early);
        if let Some(frame) = &due {
            self.tally.frames += 1;
            self.tally.decode_ms += frame.decode_ms;
            self.tally.copy_ms += frame.copy_ms;
        }
        match self.video.status() {
            Status::Ended => self.ended = true,
            Status::Failed(e) => {
                self.result = Err(e);
                self.ended = true;
            }
            Status::Opening | Status::Playing => {}
        }
        due
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(early) = self.screen.as_ref().map(|screen| screen.early) else { return };
        let due = self.due_frame(early);
        let Some(screen) = &mut self.screen else { return };
        if let Some(frame) = due {
            let before = Instant::now();
            screen.renderer.update_video_texture(&screen.gpu.queue, screen.texture, &frame.pixels, 4 * self.size.0);
            self.tally.uploads += 1;
            self.tally.upload_ms += before.elapsed().as_secs_f64() * 1000.0;
            self.presented += 1;
            self.video.recycle(frame.pixels);
        }

        let target = match screen.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(target) | wgpu::CurrentSurfaceTexture::Suboptimal(target) => target,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                screen.surface.configure(&screen.gpu.device, &screen.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
        };
        let screen_size = (screen.config.width, screen.config.height);
        let [x, y, w, h] = letterbox(screen_size, self.size, self.pixel_aspect);
        let mut layer = Layer::default();
        if self.presented > 0 {
            layer.quads.push(Quad::image(Rect::new(x, y, w, h), screen.texture, [0.0, 0.0, 1.0, 1.0], 1.0, 0.0));
        }
        let frame = render::Frame { layers: vec![layer] };
        let view = target.texture.create_view(&Default::default());
        let black = wgpu::Color::BLACK;
        screen.renderer.render(&screen.gpu.device, &screen.gpu.queue, &view, screen_size, &frame, black);
        screen.gpu.queue.present(target);

        if self.started.is_none() && self.presented > 0 {
            self.started = Some(Instant::now());
            self.last_stats = Instant::now();
            self.video.presented();
            log!("video: first frame presented at {:.0} ms", since_start());
        }
        if self.last_stats.elapsed() >= Duration::from_secs(1) && self.started.is_some() {
            self.stats();
        }
        let out_of_time = self.started.zip(self.seconds).is_some_and(|(at, s)| at.elapsed().as_secs_f64() >= s);
        if self.ended {
            if self.result.is_ok() {
                log!("video: end of stream");
            }
            event_loop.exit();
        } else if out_of_time {
            log!("video: stopped after --seconds");
            event_loop.exit();
        }
    }

    /// Totals so far, and this last second's averages for a frame.
    fn stats(&mut self) {
        let tally = std::mem::take(&mut self.tally);
        let per = |sum: f64, count: u32| if count > 0 { sum / count as f64 } else { 0.0 };
        log!(
            "video: stats decoded={} presented={} dropped={} avg_decode_ms={:.2} avg_upload_ms={:.2} avg_copy_ms={:.2} clock={:.3}",
            self.video.decoded(),
            self.presented,
            self.video.dropped,
            per(tally.decode_ms, tally.frames),
            per(tally.upload_ms, tally.uploads),
            per(tally.copy_ms, tally.frames),
            self.video.clock(),
        );
        self.last_stats = Instant::now();
    }
}

impl ApplicationHandler for Player {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.screen.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("video-spike");
        let attributes = if self.windowed {
            attributes.with_inner_size(PhysicalSize::new(1280, 720))
        } else {
            attributes.with_fullscreen(Some(Fullscreen::Borderless(event_loop.primary_monitor())))
        };
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        window.set_cursor_visible(self.windowed);

        // As crates/app/src/window.rs sets the Dashboard's window up.
        let instance = gpu::instance(Some(Box::new(event_loop.owned_display_handle())));
        let surface = instance.create_surface(window.clone()).expect("create surface");
        let gpu = gpu::init(instance, Some(&surface));
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .expect("the adapter can't draw to the window");
        let formats = surface.get_capabilities(&gpu.adapter).formats;
        // Not an sRGB format: the renderer blends in gamma space, and a video frame's bytes go to the screen as they are.
        config.format = formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);

        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, config.format);
        let texture = renderer.add_video_texture(&gpu.device, self.size.0, self.size.1);
        let refresh = window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).unwrap_or(60_000);
        let early = 500.0 / refresh.max(1) as f64;
        log!(
            "video: window {}x{} at {:.2} Hz, texture {}x{}",
            config.width,
            config.height,
            refresh as f64 / 1000.0,
            self.size.0,
            self.size.1
        );
        self.screen = Some(Screen { renderer, texture, config, surface, gpu, early, window });
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(screen) = &self.screen {
            screen.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(screen) = &mut self.screen else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            // A key pressed for something else as the window opens is not a skip: only one pressed once the picture
            // has been up a moment counts. On the VM a stray key press arrived with the window.
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && self.started.is_some_and(|at| at.elapsed() > Duration::from_millis(300)) =>
            {
                log!("video: skipped by a key");
                event_loop.exit();
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                screen.config.width = size.width;
                screen.config.height = size.height;
                screen.surface.configure(&screen.gpu.device, &screen.config);
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}
