//! What a window, or an offscreen target, needs to show screens: the renderer, the art cache and the animated
//! values, and the one call that declares a frame and draws it.
use render::gpu::Gpu;
use render::offscreen::{read_back, wait, FORMAT};
use render::{Frame, Renderer};

use crate::{ArtCache, Store, Ui};

pub struct Host {
    pub renderer: Renderer,
    pub art: ArtCache,
    pub store: Store,
}

impl Host {
    /// `wake` is called from another thread when a frame is wanted with no input: a piece of art is ready.
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            renderer: Renderer::new(&gpu.device, &gpu.queue, format),
            art: ArtCache::new(wake),
            store: Store::default(),
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
        self.art.pump(gpu, &mut self.renderer);
        self.store.begin(dt);
        let mut frame = Frame::default();
        declare(&mut Ui::new(&mut frame, &mut self.renderer.text, &mut self.store, &mut self.art, output));
        self.store.end();
        self.renderer.render(&gpu.device, &gpu.queue, target, output, &frame, clear)
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
