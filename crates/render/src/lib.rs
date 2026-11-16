//! The renderer: a frame is a stack of layers, each a list of rounded, optionally textured quads with its text drawn
//! over them. A later layer covers an earlier one's text, which is what a panel over a page needs.
//!
//! Colours are sRGB with straight alpha and blended in gamma space on a non-sRGB target, as WPF blends, so the
//! theme's colours and alphas mean here what they mean there.
//!
//! A layer can be drawn as one picture ([`Group`]): to a texture of its own, which is then laid over the layers
//! under it, out of focus or faded as a whole. That is how the Bundle shows the Guide's panel behind a dialog, and
//! a menu behind its flyout.
//!
//! A whole frame can be kept as a small picture ([`Renderer::snapshot`]): drawn once to a texture, shrunk and
//! blurred, and from then on a texture like any other. That is how the Game hub shows the page it was opened over.
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

/// The shader's kind for an arc, after the gradients'.
const ARC: f32 = 4.0;

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
    /// The part of a ring that is drawn, its ends round: the angle of its middle and half its length, in radians
    /// clockwise from the top. None draws the whole shape.
    pub arc: Option<(f32, f32)>,
    /// An arc's ends are cut square, along the radius, rather than round.
    pub square_ends: bool,
    /// How far the rectangle is turned about its middle, in radians clockwise.
    pub turn: f32,
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
            arc: None,
            square_ends: false,
            turn: 0.0,
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
    /// A stroke `thickness` wide just inside the circle `rect` holds, from `from` to `to` radians clockwise from the
    /// top, with round ends.
    pub fn arc(rect: Rect, color: Rgba, thickness: f32, from: f32, to: f32) -> Self {
        let half = ((to - from) / 2.0).clamp(0.0, std::f32::consts::PI);
        Self { arc: Some(((from + to) / 2.0, half)), ..Self::ring(rect, color, 0.0, thickness) }
    }
    /// The same with its ends cut square, along the radius: the Spinner round a Profile's picture.
    pub fn arc_square(rect: Rect, color: Rgba, thickness: f32, from: f32, to: f32) -> Self {
        Self { square_ends: true, ..Self::arc(rect, color, thickness, from, to) }
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

/// A layer drawn as one picture, quads and text together, and that picture blurred, faded, or both, as WPF blurs
/// and fades an element: what overlaps within the layer does not show through itself as it fades. What is under the
/// layer is untouched, and shows through wherever the layer's picture is not opaque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Group {
    /// The part of the target the layer is drawn in, in pixels: everything it draws and as far round that as the
    /// blur reaches. Nothing of the layer shows outside it.
    pub bounds: Rect,
    /// The blur's radius in pixels, as WPF's `BlurEffect` counts it: how far the blur reaches, which is three times
    /// the Gaussian's deviation. 0 leaves the picture sharp.
    pub blur: f32,
    /// What the picture's alpha is multiplied by, 0 to 1.
    pub opacity: f32,
}

#[derive(Default)]
pub struct Layer {
    pub quads: Vec<Quad>,
    /// Drawn over the layer's quads.
    pub texts: Vec<Text>,
    /// Draws the layer as one picture, blurred or faded.
    pub group: Option<Group>,
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
    /// The turn's cosine and sine; two to spare.
    turn: [f32; 4],
}

/// What one grouped layer of a frame is drawn through: the texture it is drawn to, the one it is blurred across
/// into, and what the passes between them read. Kept from frame to frame while the layer's size stays the same.
struct BlurSlot {
    size: (u32, u32),
    drawn: wgpu::TextureView,
    across: wgpu::TextureView,
    /// The quad shader's globals for drawing to `drawn`: its size, and where in the frame it is.
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    /// The same for the text.
    viewport: Viewport,
    /// The pass across, which reads `drawn`, and the pass down, which reads `across`.
    params: [wgpu::Buffer; 2],
    binds: [wgpu::BindGroup; 2],
}

/// A grouped layer of the frame being drawn: which slot it is drawn through, and where in the frame it is, in whole
/// pixels.
#[derive(Clone, Copy)]
struct Blurred {
    slot: usize,
    origin: (f32, f32),
}

struct Texture {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    /// A video texture: its fourth byte is not alpha, and it is drawn opaque.
    video: bool,
}

pub struct Renderer {
    pub text: TextSys,
    pipeline: wgpu::RenderPipeline,
    /// The same, for video textures: the texture's alpha is taken as 1.
    opaque_pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    globals_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    format: wgpu::TextureFormat,
    /// The blur's pass across, which replaces what is in its target, and its pass down, which is blended onto the
    /// frame.
    blur_across: wgpu::RenderPipeline,
    blur_down: wgpu::RenderPipeline,
    blur_layout: wgpu::BindGroupLayout,
    blur_slots: Vec<BlurSlot>,
    texture_layout: wgpu::BindGroupLayout,
    textures: Vec<Option<Texture>>,
    free: Vec<usize>,
    instances: wgpu::Buffer,
    capacity: usize,
    swash: SwashCache,
    cache: Cache,
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
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let pipeline = |fragment: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
                        6 => Float32x4
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
        };
        let (pipeline, opaque_pipeline) = (pipeline("fs"), pipeline("fs_opaque"));

        let blur_shader = device.create_shader_module(wgpu::include_wgsl!("blur.wgsl"));
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&blur_layout)],
            immediate_size: 0,
        });
        let blur_pipeline = |blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("blur"),
                layout: Some(&blur_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &blur_shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &blur_shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let blur_across = blur_pipeline(None);
        let blur_down = blur_pipeline(Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
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
            opaque_pipeline,
            globals,
            globals_bind,
            globals_layout,
            sampler,
            format,
            blur_across,
            blur_down,
            blur_layout,
            blur_slots: Vec::new(),
            texture_layout,
            textures: Vec::new(),
            free: Vec::new(),
            instances,
            capacity,
            swash: SwashCache::new(),
            cache,
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
        self.keep_texture(device, texture, false)
    }

    /// A texture for video: `width` x `height`, black until its first frame, filled by
    /// [`Self::update_video_texture`]. Its pixels are B, G, R, X bytes as Media Foundation's RGB32 has them; the
    /// fourth byte is ignored and the picture drawn opaque. One mip level, so it is for drawing at about its own
    /// size or larger.
    pub fn add_video_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) -> TexId {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Not an sRGB format, as the other textures aren't.
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.keep_texture(device, texture, true)
    }

    /// Replaces a video texture's pixels with one frame: rows of B, G, R, X bytes from the top, each `stride` bytes
    /// apart (at least four times the width; a decoder may pad its rows). It shows from the next [`Self::render`].
    ///
    /// Panics if `id` did not come from [`Self::add_video_texture`] or `pixels` is too short for the texture.
    pub fn update_video_texture(&mut self, queue: &wgpu::Queue, id: TexId, pixels: &[u8], stride: u32) {
        let texture = self.textures.get(id.0).and_then(Option::as_ref).filter(|texture| texture.video);
        let texture = &texture.expect("update_video_texture on a texture that isn't for video").texture;
        let size = texture.size();
        let needed = stride as usize * (size.height as usize - 1) + 4 * size.width as usize;
        assert!(stride >= 4 * size.width && pixels.len() >= needed, "a video frame too small for its texture");
        queue.write_texture(
            texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None },
            size,
        );
    }

    /// A texture as a quad reads it.
    fn bound(&self, device: &wgpu::Device, texture: wgpu::Texture, video: bool) -> Texture {
        let view = texture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        Texture { texture, bind, video }
    }

    /// A texture a frame can be drawn to and a quad can then read, in the target's format.
    fn drawn_texture(&self, device: &wgpu::Device, size: (u32, u32)) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("snapshot"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    /// A place for a picture that is drawn later, by [`Self::snapshot`]: black until then. Whoever declares frames
    /// can hold one ready, so a quad can name the picture in the very frame it is taken.
    pub fn reserve_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> TexId {
        self.add_texture(device, queue, &[RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]))])
    }

    /// Takes a picture of `frame`, which is declared for a target of `size` pixels, and keeps it as the texture
    /// `into` (one from [`Self::reserve_texture`]): `shrink` times smaller each way, and blurred there by `blur` of
    /// its own pixels, as WPF's `BlurEffect` blurs (0 for none). The frame is drawn over black. Returns the
    /// picture's size.
    ///
    /// It is drawn at its own size and halved down to the small one, so every pixel of the frame is in the picture
    /// and none is skipped; the blur is the last step, at the small size, which is what makes it cheap. The edges
    /// are held, not faded. Nothing of the frame being declared is touched: this submits passes of its own, before
    /// the frame's.
    #[allow(clippy::too_many_arguments)]
    pub fn snapshot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        into: TexId,
        frame: &Frame,
        size: (u32, u32),
        shrink: u32,
        blur: f32,
    ) -> (u32, u32) {
        let small = ((size.0 / shrink.max(1)).max(1), (size.1 / shrink.max(1)).max(1));
        let texture = self.drawn_texture(device, size);
        self.render(device, queue, &texture.create_view(&Default::default()), size, frame, wgpu::Color::BLACK);
        let mut held = self.keep_texture(device, texture, false);
        let mut at = size;
        loop {
            let next = if at.0 / 2 > small.0 && at.1 / 2 > small.1 { (at.0 / 2, at.1 / 2) } else { small };
            let last = next == small;
            let whole = Rect::new(0.0, 0.0, next.0 as f32, next.1 as f32);
            let layer = Layer {
                quads: vec![Quad::image(whole, held, [0.0, 0.0, 1.0, 1.0], 1.0, 0.0)],
                texts: Vec::new(),
                group: (last && blur > 0.0).then_some(Group { bounds: whole, blur, opacity: 1.0 }),
            };
            let texture = self.drawn_texture(device, next);
            let view = texture.create_view(&Default::default());
            self.render(device, queue, &view, next, &Frame { layers: vec![layer] }, wgpu::Color::BLACK);
            self.remove_texture(held);
            if last {
                if into != WHITE && into.0 < self.textures.len() {
                    self.textures[into.0] = Some(self.bound(device, texture, false));
                }
                return next;
            }
            held = self.keep_texture(device, texture, false);
            at = next;
        }
    }

    fn keep_texture(&mut self, device: &wgpu::Device, texture: wgpu::Texture, video: bool) -> TexId {
        let entry = Some(self.bound(device, texture, video));
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

    /// One layer's quads, then its text over them, into `pass`: `place` is where its quads start among the
    /// instances and which text renderer is its own.
    fn draw_layer(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        layer: &Layer,
        (first, text): (usize, Option<usize>),
        globals: &wgpu::BindGroup,
        viewport: &Viewport,
    ) {
        if !layer.quads.is_empty() {
            pass.set_bind_group(0, globals, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
        }
        // One draw per run of quads sharing a texture.
        let mut start = 0;
        while start < layer.quads.len() {
            let tex = layer.quads[start].tex;
            let end = start + layer.quads[start..].iter().take_while(|q| q.tex == tex).count();
            // A texture freed while something still draws with it shows as plain white, not a crash.
            let bound = self.textures.get(tex.0).and_then(Option::as_ref).or(self.textures[0].as_ref());
            let bound = bound.expect("the white texture");
            pass.set_pipeline(if bound.video { &self.opaque_pipeline } else { &self.pipeline });
            pass.set_bind_group(1, &bound.bind, &[]);
            pass.draw(0..4, (first + start) as u32..(first + end) as u32);
            start = end;
        }
        if let Some(text) = text {
            self.text_renderers[text].render(&self.atlas, viewport, pass).expect("render text");
        }
    }

    /// Makes slot `slot` ready for a layer `size` pixels at `origin` in a frame of `target` pixels, blurred and
    /// faded as `group` says: its textures, made again only when the size has changed, and what its passes read.
    #[allow(clippy::too_many_arguments)]
    fn prepare_group(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slot: usize,
        origin: (f32, f32),
        size: (u32, u32),
        group: Group,
        target: (u32, u32),
    ) {
        if self.blur_slots.get(slot).is_none_or(|made| made.size != size) {
            let texture = || {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("grouped layer"),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                texture.create_view(&Default::default())
            };
            let (drawn, across) = (texture(), texture());
            let buffer = |size| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            };
            let globals = buffer(16);
            let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.globals_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            let params = [buffer(48), buffer(48)];
            let bind = |params: &wgpu::Buffer, view: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.blur_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(view) },
                    ],
                })
            };
            let binds = [bind(&params[0], &drawn), bind(&params[1], &across)];
            let viewport = Viewport::new(device, &self.cache);
            let made = BlurSlot { size, drawn, across, globals, globals_bind, viewport, params, binds };
            if slot < self.blur_slots.len() {
                self.blur_slots[slot] = made;
            } else {
                self.blur_slots.push(made);
            }
        }
        let made = &mut self.blur_slots[slot];
        let (w, h) = (size.0 as f32, size.1 as f32);
        queue.write_buffer(&made.globals, 0, bytemuck::cast_slice(&[w, h, origin.0, origin.1]));
        made.viewport.update(queue, Resolution { width: size.0, height: size.1 });
        // WPF's Gaussian reaches its radius at three deviations. With no radius each pass reads its one texel, and
        // the picture stays sharp.
        let radius = group.blur.max(0.0);
        let (sigma, taps) = ((radius / 3.0).max(0.01), radius.ceil());
        let opacity = group.opacity.clamp(0.0, 1.0);
        let across = [0.0, 0.0, w, h, w, h, 1.0 / w, 0.0, sigma, taps, 1.0, 0.0];
        let (width, height) = (target.0 as f32, target.1 as f32);
        let down = [origin.0, origin.1, w, h, width, height, 0.0, 1.0 / h, sigma, taps, opacity, 0.0];
        queue.write_buffer(&made.params[0], 0, bytemuck::cast_slice(&across));
        queue.write_buffer(&made.params[1], 0, bytemuck::cast_slice(&down));
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
                // An arc is a kind of its own, its angles where a texture's corners would be: it has no texture.
                params: [q.radius, q.ring, if q.arc.is_some() { ARC } else { q.gradient as u32 as f32 }, q.feather],
                uv: q.arc.map_or(q.uv, |(middle, half)| [middle, half, if q.square_ends { 1.0 } else { 0.0 }, 0.0]),
                clip: q.clip.map_or(everything, |c| [c.x, c.y, c.right(), c.bottom()]),
                turn: [q.turn.cos(), q.turn.sin(), 0.0, 0.0],
            })
            .collect();
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&[size.0 as f32, size.1 as f32, 0.0, 0.0]));

        // The layers drawn as one picture, each through a slot of its own. One whose bounds are off the target, or
        // that draws nothing or is faded to nothing, is not drawn at all.
        let limit = device.limits().max_texture_dimension_2d as f32;
        let mut slots = 0;
        let mut blurred: Vec<Option<Blurred>> = Vec::with_capacity(frame.layers.len());
        for layer in &frame.layers {
            let placed = layer.group.and_then(|group| {
                let (x, y) = (group.bounds.x.floor(), group.bounds.y.floor());
                let w = (group.bounds.right().ceil() - x).min(limit);
                let h = (group.bounds.bottom().ceil() - y).min(limit);
                let on_target = x < size.0 as f32 && y < size.1 as f32 && x + w > 0.0 && y + h > 0.0;
                let empty = (layer.quads.is_empty() && layer.texts.is_empty()) || group.opacity <= 0.0;
                (on_target && w >= 1.0 && h >= 1.0 && !empty).then_some(((x, y), (w as u32, h as u32), group))
            });
            blurred.push(placed.map(|(origin, extent, group)| {
                self.prepare_group(device, queue, slots, origin, extent, group, size);
                slots += 1;
                Blurred { slot: slots - 1, origin }
            }));
        }
        self.blur_slots.truncate(slots);
        // A layer that was to be drawn as one picture and has nowhere to be drawn.
        let hidden = |at: usize| frame.layers[at].group.is_some() && blurred[at].is_none();

        self.viewport.update(queue, Resolution { width: size.0, height: size.1 });
        let has_text = |at: usize| !frame.layers[at].texts.is_empty() && !hidden(at);
        let with_text = (0..frame.layers.len()).filter(|&at| has_text(at)).count();
        while self.text_renderers.len() < with_text {
            self.text_renderers.push(TextRenderer::new(&mut self.atlas, device, Default::default(), None));
        }
        for (next, at) in (0..frame.layers.len()).filter(|&at| has_text(at)).enumerate() {
            let layer = &frame.layers[at];
            for text in &layer.texts {
                self.text.buffer(&text.key);
            }
            // A grouped layer's text is drawn to the layer's own texture, which holds a part of the frame.
            let (dx, dy) = blurred[at].map_or((0.0, 0.0), |blurred| blurred.origin);
            let areas = layer.texts.iter().map(|t| TextArea {
                buffer: &self.text.buffers[&t.key],
                left: t.x - dx,
                top: t.y - dy,
                scale: 1.0,
                bounds: TextBounds {
                    left: (t.clip.x - dx).floor() as i32,
                    top: (t.clip.y - dy).floor() as i32,
                    right: (t.clip.right() - dx).ceil() as i32,
                    bottom: (t.clip.bottom() - dy).ceil() as i32,
                },
                default_color: glyphon::Color::rgba(t.color[0], t.color[1], t.color[2], t.color[3]),
                custom_glyphs: &[],
            });
            let viewport = blurred[at].map_or(&self.viewport, |blurred| &self.blur_slots[blurred.slot].viewport);
            self.text_renderers[next]
                .prepare(device, queue, &mut self.text.fonts, &mut self.atlas, viewport, areas, &mut self.swash)
                .expect("prepare text");
        }

        // Where each layer's quads start among the instances, and which text renderer is the layer's.
        let (mut first, mut text) = (0, 0);
        let mut places: Vec<(usize, Option<usize>)> = Vec::with_capacity(frame.layers.len());
        for (at, layer) in frame.layers.iter().enumerate() {
            places.push((first, has_text(at).then_some(text)));
            first += layer.quads.len();
            text += has_text(at) as usize;
        }

        let mut encoder = device.create_command_encoder(&Default::default());
        fn attachment(view: &wgpu::TextureView, clear: wgpu::Color) -> Option<wgpu::RenderPassColorAttachment<'_>> {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
            })
        }
        // Each grouped layer to its own texture, then blurred across into its second.
        for (at, layer) in frame.layers.iter().enumerate() {
            let Some(blurred) = blurred[at] else { continue };
            let slot = &self.blur_slots[blurred.slot];
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("grouped layer"),
                    color_attachments: &[attachment(&slot.drawn, wgpu::Color::TRANSPARENT)],
                    ..Default::default()
                });
                self.draw_layer(&mut pass, layer, places[at], &slot.globals_bind, &slot.viewport);
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur across"),
                color_attachments: &[attachment(&slot.across, wgpu::Color::TRANSPARENT)],
                ..Default::default()
            });
            pass.set_pipeline(&self.blur_across);
            pass.set_bind_group(0, &slot.binds[0], &[]);
            pass.draw(0..4, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[attachment(target, clear)],
                ..Default::default()
            });
            for (at, layer) in frame.layers.iter().enumerate() {
                match blurred[at] {
                    // Blurred down and faded as it is laid over what is under it.
                    Some(blurred) => {
                        pass.set_pipeline(&self.blur_down);
                        pass.set_bind_group(0, &self.blur_slots[blurred.slot].binds[1], &[]);
                        pass.draw(0..4, 0..1);
                    }
                    None if hidden(at) => {}
                    None => self.draw_layer(&mut pass, layer, places[at], &self.globals_bind, &self.viewport),
                }
            }
        }
        let index = queue.submit(Some(encoder.finish()));
        self.atlas.trim();
        self.text.trim();
        index
    }
}
