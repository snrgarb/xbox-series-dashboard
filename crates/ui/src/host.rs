//! What a window, or an offscreen target, needs to show screens: the renderer, the art cache and the animated
//! values, and the one call that declares a frame and draws it.
use std::time::{Duration, Instant};

use render::gpu::Gpu;
use render::offscreen::{read_back, wait, FORMAT};
use render::{Frame, Renderer};

use crate::{ArtCache, Store, Ui};

pub struct Host {
    pub renderer: Renderer,
    pub art: ArtCache,
    pub store: Store,
    /// What the last frame drawn spent its time on.
    pub cost: FrameCost,
}

/// What a frame spent its time on, on the thread that draws, for a log that says why a frame was slow (the Game
/// hub's opening line, #214). Kept for every frame: a few clock readings.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameCost {
    /// Art decoded on the decoding threads, uploaded to the GPU: how many pieces, and the time.
    pub uploads: u32,
    pub upload: Duration,
    /// The screens declaring the frame (text measured in it included).
    pub declare: Duration,
    /// Pictures of the screen taken (`Ui::snapshot`): how many, and the time, drawing what they show included.
    pub snapshots: u32,
    pub snapshot: Duration,
    /// The frame drawn and submitted, text laid out included.
    pub render: Duration,
    /// Text shaped anew anywhere in the frame: how many pieces, and the time.
    pub shaped: u32,
    pub shaping: Duration,
    /// Text laid out for the GPU, glyphs not drawn before rasterised, new text shaped there.
    pub glyphs: Duration,
}

impl FrameCost {
    /// The parts that took any time, in a few words: `declare 2.1 ms, snapshot 1 in 4.0 ms, render 1.2 ms`.
    pub fn describe(&self) -> String {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut parts = Vec::new();
        if self.uploads > 0 {
            parts.push(format!("{} art uploaded in {:.1} ms", self.uploads, ms(self.upload)));
        }
        parts.push(format!("declare {:.1} ms", ms(self.declare)));
        if self.snapshots > 0 {
            parts.push(format!("picture of the screen behind {:.1} ms", ms(self.snapshot)));
        }
        parts.push(format!("render {:.1} ms", ms(self.render)));
        if self.shaped > 0 {
            parts.push(format!("text: {} shaped in {:.1} ms", self.shaped, ms(self.shaping)));
        }
        parts.push(format!("glyphs {:.1} ms", ms(self.glyphs)));
        parts.join(", ")
    }
}

impl Host {
    /// `wake` is called from another thread when a frame is wanted with no input: a piece of art is ready.
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            renderer: Renderer::new(&gpu.device, &gpu.queue, format),
            art: ArtCache::new(wake),
            store: Store::default(),
            cost: FrameCost::default(),
        }
    }

    /// Declares a frame `dt` seconds after the last and draws it to `target`.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        output: (u32, u32),
        dt: f32,
        clear: wgpu::Color,
        declare: impl FnOnce(&mut Ui),
    ) -> wgpu::SubmissionIndex {
        self.draw_part(gpu, target, output, (output, 0), dt, clear, declare)
    }

    /// As `draw`, to a target that is a part of a larger output: `part` is the whole output's size in pixels and
    /// how far down it the target's top is. The frame is declared as for the whole, and the target shows its part.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_part(
        &mut self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        output: (u32, u32),
        part: ((u32, u32), u32),
        dt: f32,
        clear: wgpu::Color,
        declare: impl FnOnce(&mut Ui),
    ) -> wgpu::SubmissionIndex {
        let (whole, top) = part;
        let mut cost = FrameCost::default();
        self.art.pump(gpu, &mut self.renderer);
        (cost.uploads, cost.upload) = self.art.take_uploads();
        // A place for a picture the frame may ask to have taken (`Ui::snapshot`), ready before it is declared.
        if self.store.snaps.wants_spare() {
            self.store.snaps.set_spare(self.renderer.reserve_texture(&gpu.device, &gpu.queue));
        }
        self.store.begin(dt);
        let mut frame = Frame::default();
        let began = Instant::now();
        declare(&mut Ui::part(&mut frame, &mut self.renderer.text, &mut self.store, &mut self.art, whole, top));
        self.store.end();
        cost.declare = began.elapsed();
        // The pictures it asked for are taken before it is drawn, so it shows them in this very frame; those it no
        // longer asks for are let go.
        let began = Instant::now();
        for job in self.store.snaps.take_jobs() {
            let (device, queue) = (&gpu.device, &gpu.queue);
            self.renderer.snapshot(device, queue, job.tex, &job.frame, job.output, job.shrink, job.blur);
            cost.snapshots += 1;
        }
        cost.snapshot = if cost.snapshots > 0 { began.elapsed() } else { Duration::ZERO };
        for tex in self.store.snaps.take_dropped() {
            self.renderer.remove_texture(tex);
        }
        let began = Instant::now();
        let index = self.renderer.render(&gpu.device, &gpu.queue, target, output, &frame, clear);
        cost.render = began.elapsed();
        (cost.shaped, cost.shaping) = self.renderer.text.take_shaped();
        cost.glyphs = self.renderer.take_glyph_time();
        self.cost = cost;
        index
    }

    /// Runs `body` with `store` in the place of the Host's own animated values: a frame for a second window. The
    /// renderer, the fonts and the art are the same, so nothing is decoded or uploaded twice; only what is moving
    /// is the window's own, since a value a frame doesn't ask for is forgotten.
    pub fn with_store<R>(&mut self, store: &mut Store, body: impl FnOnce(&mut Self) -> R) -> R {
        std::mem::swap(&mut self.store, store);
        let result = body(self);
        std::mem::swap(&mut self.store, store);
        result
    }

    /// How long after the last frame the next is wanted when nothing is moving and no input comes: a delay ends
    /// then, or something slow takes its next step.
    pub fn wake(&self) -> Option<f32> {
        self.store.wake()
    }

    /// Whether another frame is wanted without any input: something is moving, or art is on its way.
    pub fn animating(&self) -> bool {
        self.store.animating() || self.art.pending() > 0
    }
}

/// A target with no window, for screenshots.
pub struct Offscreen {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
}

impl Offscreen {
    pub fn new(gpu: &Gpu, size: (u32, u32)) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self { texture, view, size }
    }

    /// The frame `declare` describes as it looks at rest: every piece of art it asks for decoded, every animation
    /// at its end. RGBA pixels, row by row.
    pub fn still(&self, gpu: &Gpu, host: &mut Host, mut declare: impl FnMut(&mut Ui)) -> Vec<u8> {
        // A frame to find out what art is wanted, then frames until none is still on its way.
        loop {
            let index = host.draw(gpu, &self.view, self.size, 0.0, wgpu::Color::BLACK, &mut declare);
            wait(gpu, Some(index));
            if !host.art.wait(gpu, &mut host.renderer) {
                break;
            }
        }
        // Twice settled: the first lets state that follows the art (a background taking over) catch up.
        host.store.set_settle(true);
        for _ in 0..3 {
            let index = host.draw(gpu, &self.view, self.size, 0.0, wgpu::Color::BLACK, &mut declare);
            wait(gpu, Some(index));
        }
        host.store.set_settle(false);
        read_back(gpu, &self.texture, self.size)
    }
}
