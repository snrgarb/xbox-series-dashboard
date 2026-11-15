//! The window: borderless full screen (or 1280x720 with `--windowed`), each decoded frame written to one texture
//! and drawn as a quad when the clock reaches its time. Any key ends it, as does the end of the file.
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use render::gpu::{self, Gpu};
use render::{Layer, Quad, Rect, Renderer, TexId};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

use super::audio::Playback;
use super::decode::{self, Event, Frame, Opened};
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
    playback: Arc<Playback>,
    events: Receiver<Event>,
    recycle: Sender<Vec<u8>>,
    screen: Option<Screen>,
    /// The frame decoded but not yet due.
    next: Option<Frame>,
    ended: bool,
    /// The first frame's time, once it has been written to the texture.
    first: Option<f64>,
    /// When the first frame was presented.
    started: Option<Instant>,
    presented: u64,
    dropped: u64,
    tally: Tally,
    last_stats: Instant,
    result: Result<(), String>,
}

pub fn run(args: &Play) -> Result<(), String> {
    let (recycle, recycled) = std::sync::mpsc::channel();
    // A full path: Media Foundation resolves the name as a URL, not against the working directory.
    let file = std::path::absolute(&args.file).map_err(|e| format!("{}: {e}", args.file.display()))?;
    if !file.is_file() {
        return Err(format!("{}: no such file", file.display()));
    }
    let options =
        decode::Options { file, audio: !args.no_audio, dxva: args.dxva, basic_processing: args.basic_processing };
    let events = decode::spawn(options, recycled);
    let Opened { size, pixel_aspect, playback } = match events.recv() {
        Ok(Event::Opened(opened)) => opened,
        Ok(Event::Failed(e)) => return Err(e),
        _ => return Err("the decoder stopped before opening the file".into()),
    };

    let event_loop = EventLoop::new().map_err(|e| format!("no display: {e}"))?;
    // Drawn every vertical blank, where each frame's time is checked against the clock.
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut player = Player {
        windowed: args.windowed,
        seconds: args.seconds,
        size,
        pixel_aspect,
        playback,
        events,
        recycle,
        screen: None,
        next: None,
        ended: false,
        first: None,
        started: None,
        presented: 0,
        dropped: 0,
        tally: Tally::default(),
        last_stats: Instant::now(),
        result: Ok(()),
    };
    let run = event_loop.run_app(&mut player);
    player.playback.close();
    let decoded = player.playback.decoded.load(Ordering::Relaxed);
    log!("video: done decoded={decoded} presented={} dropped={}", player.presented, player.dropped);
    run.map_err(|e| format!("event loop: {e}"))?;
    player.result
}

impl Player {
    /// The frame to show now, if one has come due: the latest whose time the clock has reached. Earlier ones that
    /// were never shown count as dropped.
    fn due_frame(&mut self, early: f64) -> Option<Frame> {
        let now = self.playback.now();
        let mut due: Option<Frame> = None;
        loop {
            if self.next.is_none() && !self.ended {
                match self.events.try_recv() {
                    Ok(Event::Frame(frame)) => {
                        self.tally.frames += 1;
                        self.tally.decode_ms += frame.decode_ms;
                        self.tally.copy_ms += frame.copy_ms;
                        self.next = Some(frame);
                    }
                    Ok(Event::End) | Err(TryRecvError::Disconnected) => self.ended = true,
                    Ok(Event::Failed(e)) => {
                        self.result = Err(e);
                        self.ended = true;
                    }
                    Ok(Event::Opened(_)) | Err(TryRecvError::Empty) => {}
                }
            }
            // The first frame goes up as soon as it is decoded; the clock starts from it.
            let first = self.first.is_none() && due.is_none();
            match self.next.take() {
                Some(frame) if first || (self.started.is_some() && frame.time <= now + early) => {
                    if let Some(skipped) = due.replace(frame) {
                        self.dropped += 1;
                        let _ = self.recycle.send(skipped.pixels);
                    }
                }
                waiting => {
                    self.next = waiting;
                    return due;
                }
            }
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(early) = self.screen.as_ref().map(|screen| screen.early) else { return };
        let due = self.due_frame(early);
        let Some(screen) = &mut self.screen else { return };
        if let Some(frame) = due {
            self.first.get_or_insert(frame.time);
            let before = Instant::now();
            screen.renderer.update_video_texture(&screen.gpu.queue, screen.texture, &frame.pixels, 4 * self.size.0);
            self.tally.uploads += 1;
            self.tally.upload_ms += before.elapsed().as_secs_f64() * 1000.0;
            self.presented += 1;
            let _ = self.recycle.send(frame.pixels);
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

        if let (None, Some(time)) = (self.started, self.first) {
            self.started = Some(Instant::now());
            self.last_stats = Instant::now();
            self.playback.start(time);
            log!("video: first frame presented at {:.0} ms", since_start());
        }
        if self.last_stats.elapsed() >= Duration::from_secs(1) && self.started.is_some() {
            self.stats();
        }
        let out_of_time = self.started.zip(self.seconds).is_some_and(|(at, s)| at.elapsed().as_secs_f64() >= s);
        if self.ended && self.next.is_none() {
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
            self.playback.decoded.load(Ordering::Relaxed),
            self.presented,
            self.dropped,
            per(tally.decode_ms, tally.frames),
            per(tally.upload_ms, tally.uploads),
            per(tally.copy_ms, tally.frames),
            self.playback.now(),
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
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat => {
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
