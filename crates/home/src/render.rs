//! The renderer: a list of rounded, optionally textured quads, then text over them, in one pass.
use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glyphon::cosmic_text::Align;
use glyphon::{
    Attrs, Buffer, Cache, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport, Weight,
};
use image::RgbaImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexId(usize);
pub const WHITE: TexId = TexId(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn grow(self, by: f32) -> Self {
        Self::new(self.x - by, self.y - by, self.w + 2.0 * by, self.h + 2.0 * by)
    }
    pub fn lerp(self, to: Rect, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self::new(l(self.x, to.x), l(self.y, to.y), l(self.w, to.w), l(self.h, to.h))
    }
}

/// Linear RGB, straight alpha.
pub type Rgba = [f32; 4];

/// An sRGB colour as `0xRRGGBB` with an alpha.
pub fn rgba(hex: u32, a: f32) -> Rgba {
    let lin = |c: u32| {
        let c = (c & 0xff) as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    [lin(hex >> 16), lin(hex >> 8), lin(hex), a]
}

pub struct Quad {
    pub rect: Rect,
    pub top: Rgba,
    pub bottom: Rgba,
    pub radius: f32,
    /// Ring thickness; 0 fills the shape.
    pub ring: f32,
    pub tex: TexId,
}

impl Quad {
    pub fn fill(rect: Rect, color: Rgba, radius: f32) -> Self {
        Self { rect, top: color, bottom: color, radius, ring: 0.0, tex: WHITE }
    }
    pub fn gradient(rect: Rect, top: Rgba, bottom: Rgba, radius: f32) -> Self {
        Self { rect, top, bottom, radius, ring: 0.0, tex: WHITE }
    }
    pub fn image(rect: Rect, tex: TexId, alpha: f32, radius: f32) -> Self {
        Self { tex, ..Self::fill(rect, [1.0, 1.0, 1.0, alpha], radius) }
    }
    pub fn ring(rect: Rect, color: Rgba, radius: f32, thickness: f32) -> Self {
        Self { ring: thickness, ..Self::fill(rect, color, radius) }
    }
}

/// What a piece of text is laid out from; its shaped buffer is kept under this.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextKey {
    pub text: String,
    /// Font size in tenths of a pixel.
    pub size: u32,
    pub semibold: bool,
    /// Wrap width in pixels; 0 = one line.
    pub wrap: u32,
    pub centered: bool,
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
pub struct Frame {
    pub quads: Vec<Quad>,
    pub texts: Vec<Text>,
}

/// Fonts and shaped text. Segoe UI from the Windows fonts folder; elsewhere the first of a few common fonts, or
/// failing those whatever the system has.
pub struct TextSys {
    fonts: FontSystem,
    buffers: HashMap<TextKey, Buffer>,
}

impl TextSys {
    fn new() -> Self {
        let mut db = glyphon::fontdb::Database::new();
        let candidates: &[&str] = if cfg!(windows) {
            &["segoeui.ttf", "seguisb.ttf"]
        } else {
            &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            ]
        };
        let dir = std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts")).unwrap_or_default();
        for file in candidates {
            let _ = db.load_font_file(dir.join(file));
        }
        if db.is_empty() {
            db.load_system_fonts();
        }
        let family = db.faces().next().and_then(|f| f.families.first()).map(|f| f.0.clone());
        if let Some(family) = family {
            eprintln!("font: {family} ({} faces)", db.len());
            db.set_sans_serif_family(family);
        }
        Self { fonts: FontSystem::new_with_locale_and_db("en-US".into(), db), buffers: HashMap::new() }
    }

    fn buffer(&mut self, key: &TextKey) -> &Buffer {
        if !self.buffers.contains_key(key) {
            let size = key.size as f32 / 10.0;
            // Segoe UI's line is 1.33 of its size.
            let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, (size * 1.33).round()));
            buffer.set_size((key.wrap > 0).then_some(key.wrap as f32), None);
            let weight = if key.semibold { Weight::SEMIBOLD } else { Weight::NORMAL };
            let attrs = Attrs::new().family(Family::SansSerif).weight(weight);
            buffer.set_text(&key.text, &attrs, Shaping::Advanced, key.centered.then_some(Align::Center));
            buffer.shape_until_scroll(&mut self.fonts, false);
            self.buffers.insert(key.clone(), buffer);
        }
        &self.buffers[key]
    }

    /// The widest line's width, the line height and the number of lines.
    pub fn measure(&mut self, key: &TextKey) -> (f32, f32, usize) {
        let buffer = self.buffer(key);
        let width = buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max);
        (width, buffer.metrics().line_height, buffer.layout_runs().count())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    c0: Rgba,
    c1: Rgba,
    params: [f32; 4],
}

const MAX_QUADS: usize = 256;

pub struct Renderer {
    pub text: TextSys,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    textures: Vec<wgpu::BindGroup>,
    instances: wgpu::Buffer,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
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
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (MAX_QUADS * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer = TextRenderer::new(&mut atlas, device, Default::default(), None);

        let mut renderer = Self {
            text: TextSys::new(),
            pipeline,
            globals,
            globals_bind,
            texture_layout,
            textures: Vec::new(),
            instances,
            swash: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
        };
        renderer.add_texture(device, queue, &[RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]))]);
        renderer
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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, mip) in mips.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                mip,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * mip.width()), rows_per_image: None },
                wgpu::Extent3d { width: mip.width(), height: mip.height(), depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&Default::default());
        self.textures.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        }));
        TexId(self.textures.len() - 1)
    }

    /// Draws `frame` to `target`, which is `size` pixels, and submits it.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: (u32, u32),
        frame: &Frame,
    ) -> wgpu::SubmissionIndex {
        let quads = &frame.quads[..frame.quads.len().min(MAX_QUADS)];
        let instances: Vec<Instance> = quads
            .iter()
            .map(|q| Instance {
                rect: [q.rect.x, q.rect.y, q.rect.w, q.rect.h],
                c0: q.top,
                c1: q.bottom,
                params: [q.radius, q.ring, 0.0, 0.0],
            })
            .collect();
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&[size.0 as f32, size.1 as f32, 0.0, 0.0]));

        self.viewport.update(queue, Resolution { width: size.0, height: size.1 });
        for text in &frame.texts {
            self.text.buffer(&text.key);
        }
        let areas = frame.texts.iter().map(|t| TextArea {
            buffer: &self.text.buffers[&t.key],
            left: t.x,
            top: t.y,
            scale: 1.0,
            bounds: TextBounds {
                left: t.clip.x as i32,
                top: t.clip.y as i32,
                right: (t.clip.x + t.clip.w) as i32,
                bottom: (t.clip.y + t.clip.h) as i32,
            },
            default_color: glyphon::Color::rgba(t.color[0], t.color[1], t.color[2], t.color[3]),
            custom_glyphs: &[],
        });
        self.text_renderer
            .prepare(device, queue, &mut self.text.fonts, &mut self.atlas, &self.viewport, areas, &mut self.swash)
            .expect("prepare text");

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            // One draw per run of quads sharing a texture.
            let mut start = 0;
            while start < quads.len() {
                let tex = quads[start].tex;
                let end = start + quads[start..].iter().take_while(|q| q.tex == tex).count();
                pass.set_bind_group(1, &self.textures[tex.0], &[]);
                pass.draw(0..4, start as u32..end as u32);
                start = end;
            }
            self.text_renderer.render(&self.atlas, &self.viewport, &mut pass).expect("render text");
        }
        let index = queue.submit(Some(encoder.finish()));
        self.atlas.trim();
        index
    }
}
