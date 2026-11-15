//! The renderer: a frame is a stack of layers, each a list of rounded, optionally textured quads with its text drawn
//! over them. A later layer covers an earlier one's text, which is what a panel over a page needs.
//!
//! Colours are sRGB with straight alpha and blended in gamma space on a non-sRGB target, as WPF blends, so the
//! theme's colours and alphas mean here what they mean there.
pub mod gpu;
pub mod offscreen;
mod text;

use bytemuck::{Pod, Zeroable};
use glyphon::{Cache, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use image::RgbaImage;

pub use text::{Font, HAlign, Measured, TextKey, TextSys};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TexId(usize);
pub const WHITE: TexId = TexId(0);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn grow(self, by: f32) -> Self {
        Self::new(self.x - by, self.y - by, self.w + 2.0 * by, self.h + 2.0 * by)
    }
    pub fn lerp(self, to: Rect, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self::new(l(self.x, to.x), l(self.y, to.y), l(self.w, to.w), l(self.h, to.h))
    }
    pub fn right(self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
    pub fn center(self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn scale(self, by: f32) -> Self {
        Self::new(self.x * by, self.y * by, self.w * by, self.h * by)
    }
    /// The part of this inside `other`; empty (zero size) when they don't meet.
    pub fn intersect(self, other: Rect) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = (self.right().min(other.right()) - x).max(0.0);
        let h = (self.bottom().min(other.bottom()) - y).max(0.0);
        Self::new(x, y, w, h)
    }
}

/// sRGB, straight alpha, each 0 to 1.
pub type Rgba = [f32; 4];

/// A colour as `0xRRGGBB` with an alpha.
pub fn rgba(hex: u32, a: f32) -> Rgba {
    let c = |shift: u32| ((hex >> shift) & 0xff) as f32 / 255.0;
    [c(16), c(8), c(0), a]
}

/// How a quad's two colours run across it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Gradient {
    /// Top to bottom.
    #[default]
    Vertical,
    /// Left to right.
    Horizontal,
    /// Top left to bottom right.
    Diagonal,
    /// The centre to the edge.
    Radial,
}

#[derive(Debug, Clone, Copy)]
pub struct Quad {
    pub rect: Rect,
    pub c0: Rgba,
    pub c1: Rgba,
    pub gradient: Gradient,
    pub radius: f32,
    /// Ring thickness; 0 fills the shape.
    pub ring: f32,
    /// How soft the edge is, in pixels; 0 is a crisp, antialiased edge.
    pub feather: f32,
    pub tex: TexId,
    /// The part of the texture shown: u0, v0, u1, v1.
    pub uv: [f32; 4],
    /// Nothing is drawn outside this.
    pub clip: Option<Rect>,
}

impl Quad {
    pub fn fill(rect: Rect, color: Rgba, radius: f32) -> Self {
        Self {
            rect,
            c0: color,
            c1: color,
            gradient: Gradient::Vertical,
            radius,
            ring: 0.0,
            feather: 0.0,
            tex: WHITE,
            uv: [0.0, 0.0, 1.0, 1.0],
            clip: None,
        }
    }
    pub fn gradient(rect: Rect, c0: Rgba, c1: Rgba, gradient: Gradient, radius: f32) -> Self {
        Self { c1, gradient, ..Self::fill(rect, c0, radius) }
    }
    pub fn image(rect: Rect, tex: TexId, uv: [f32; 4], alpha: f32, radius: f32) -> Self {
        Self { tex, uv, ..Self::fill(rect, [1.0, 1.0, 1.0, alpha], radius) }
    }
    pub fn ring(rect: Rect, color: Rgba, radius: f32, thickness: f32) -> Self {
        Self { ring: thickness, ..Self::fill(rect, color, radius) }
    }
}

pub struct Text {
    pub key: TextKey,
    pub x: f32,
    pub y: f32,
    /// Nothing is drawn outside this.
    pub clip: Rect,
    /// sRGB, 0-255.
    pub color: [u8; 4],
}

#[derive(Default)]
pub struct Layer {
    pub quads: Vec<Quad>,
    /// Drawn over the layer's quads.
    pub texts: Vec<Text>,
}

/// What to draw, bottom layer first.
#[derive(Default)]
pub struct Frame {
    pub layers: Vec<Layer>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    c0: Rgba,
    c1: Rgba,
    params: [f32; 4],
    uv: [f32; 4],
    clip: [f32; 4],
}

struct Texture {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
}

pub struct Renderer {
    pub text: TextSys,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    textures: Vec<Option<Texture>>,
    free: Vec<usize>,
    instances: wgpu::Buffer,
    capacity: usize,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    /// One per layer that has text.
    text_renderers: Vec<TextRenderer>,
}

impl Renderer {
    /// `format` is the target's, and is not an sRGB one: see the module's note on blending.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        assert!(!format.is_srgb(), "the renderer blends in gamma space: give it a non-sRGB target");
        let shader = device.create_shader_module(wgpu::include_wgsl!("quad.wgsl"));
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quads"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &globals_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let capacity = 512;
        let instances = Self::instance_buffer(device, capacity);

        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        // Text is blended as the quads are: its colours taken as they are, not as linear light.
        let atlas = TextAtlas::with_color_mode(device, queue, &cache, format, ColorMode::Web);

        let mut renderer = Self {
            text: TextSys::new(),
            pipeline,
            globals,
            globals_bind,
            texture_layout,
            textures: Vec::new(),
            free: Vec::new(),
            instances,
            capacity,
            swash: SwashCache::new(),
            viewport,
            atlas,
            text_renderers: Vec::new(),
        };
        renderer.add_texture(device, queue, &[RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]))]);
        renderer
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad instances"),
            size: (capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Uploads a mip chain, largest first.
    pub fn add_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mips: &[RgbaImage]) -> TexId {
        let (width, height) = mips[0].dimensions();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Not an sRGB format: the image's values are blended as they are.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, mip) in mips.iter().enumerate() {
            Self::write(queue, &texture, level as u32, mip);
        }
        let view = texture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        let entry = Some(Texture { texture, bind });
        match self.free.pop() {
            Some(slot) => {
                self.textures[slot] = entry;
                TexId(slot)
            }
            None => {
                self.textures.push(entry);
                TexId(self.textures.len() - 1)
            }
        }
    }

    fn write(queue: &wgpu::Queue, texture: &wgpu::Texture, level: u32, pixels: &RgbaImage) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * pixels.width()), rows_per_image: None },
            wgpu::Extent3d { width: pixels.width(), height: pixels.height(), depth_or_array_layers: 1 },
        );
    }

    /// Overwrites a texture's largest level with pixels of the same size: a video's next frame.
    pub fn update_texture(&mut self, queue: &wgpu::Queue, id: TexId, pixels: &RgbaImage) {
        if let Some(Some(texture)) = self.textures.get(id.0) {
            Self::write(queue, &texture.texture, 0, pixels);
        }
    }

    /// Frees a texture. Its id may be given to a later one, so nothing may draw with it again.
    pub fn remove_texture(&mut self, id: TexId) {
        if id != WHITE && self.textures.get(id.0).is_some_and(Option::is_some) {
            self.textures[id.0] = None;
            self.free.push(id.0);
        }
    }

    /// Draws `frame` to `target`, which is `size` pixels and is first cleared to `clear`, and submits it.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: (u32, u32),
        frame: &Frame,
        clear: wgpu::Color,
    ) -> wgpu::SubmissionIndex {
        let everything = [0.0, 0.0, size.0 as f32, size.1 as f32];
        let instances: Vec<Instance> = frame
            .layers
            .iter()
            .flat_map(|layer| &layer.quads)
            .map(|q| Instance {
                rect: [q.rect.x, q.rect.y, q.rect.w, q.rect.h],
                c0: q.c0,
                c1: q.c1,
                params: [q.radius, q.ring, q.gradient as u32 as f32, q.feather],
                uv: q.uv,
                clip: q.clip.map_or(everything, |c| [c.x, c.y, c.right(), c.bottom()]),
            })
            .collect();
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&[size.0 as f32, size.1 as f32, 0.0, 0.0]));

        self.viewport.update(queue, Resolution { width: size.0, height: size.1 });
        let with_text = frame.layers.iter().filter(|l| !l.texts.is_empty()).count();
        while self.text_renderers.len() < with_text {
            self.text_renderers.push(TextRenderer::new(&mut self.atlas, device, Default::default(), None));
        }
        for (next, layer) in frame.layers.iter().filter(|l| !l.texts.is_empty()).enumerate() {
            for text in &layer.texts {
                self.text.buffer(&text.key);
            }
            let areas = layer.texts.iter().map(|t| TextArea {
                buffer: &self.text.buffers[&t.key],
                left: t.x,
                top: t.y,
                scale: 1.0,
                bounds: TextBounds {
                    left: t.clip.x.floor() as i32,
                    top: t.clip.y.floor() as i32,
                    right: t.clip.right().ceil() as i32,
                    bottom: t.clip.bottom().ceil() as i32,
                },
                default_color: glyphon::Color::rgba(t.color[0], t.color[1], t.color[2], t.color[3]),
                custom_glyphs: &[],
            });
            self.text_renderers[next]
                .prepare(device, queue, &mut self.text.fonts, &mut self.atlas, &self.viewport, areas, &mut self.swash)
                .expect("prepare text");
        }

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut first = 0usize;
            let mut text = 0;
            for layer in &frame.layers {
                if !layer.quads.is_empty() {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.globals_bind, &[]);
                    pass.set_vertex_buffer(0, self.instances.slice(..));
                }
                // One draw per run of quads sharing a texture.
                let mut start = 0;
                while start < layer.quads.len() {
                    let tex = layer.quads[start].tex;
                    let end = start + layer.quads[start..].iter().take_while(|q| q.tex == tex).count();
                    // A texture freed while something still draws with it shows as plain white, not a crash.
                    let bound = self.textures.get(tex.0).and_then(Option::as_ref).or(self.textures[0].as_ref());
                    pass.set_bind_group(1, &bound.expect("the white texture").bind, &[]);
                    pass.draw(0..4, (first + start) as u32..(first + end) as u32);
                    start = end;
                }
                first += layer.quads.len();
                if !layer.texts.is_empty() {
                    self.text_renderers[text].render(&self.atlas, &self.viewport, &mut pass).expect("render text");
                    text += 1;
                }
            }
        }
        let index = queue.submit(Some(encoder.finish()));
        self.atlas.trim();
        self.text.trim();
        index
    }
}
