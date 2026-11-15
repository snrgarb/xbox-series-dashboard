//! A video texture drawn offscreen: its frames replace one another, padded rows and all, and it is opaque whatever
//! the decoder left in the fourth byte.
use render::gpu::{self, Gpu};
use render::{offscreen, Frame, Layer, Quad, Rect, Renderer};

const SIZE: (u32, u32) = (64, 32);

fn draw(gpu: &Gpu, renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: SIZE.0, height: SIZE.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: offscreen::FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let index = renderer.render(&gpu.device, &gpu.queue, &view, SIZE, frame, wgpu::Color::BLACK);
    offscreen::wait(gpu, Some(index));
    offscreen::read_back(gpu, &target, SIZE)
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = 4 * (y * SIZE.0 + x) as usize;
    pixels[at..at + 4].try_into().unwrap()
}

/// A 4x2 frame of one colour, each row followed by `stride - 16` bytes of padding that must never be drawn.
fn frame_of(bgrx: [u8; 4], stride: usize) -> Vec<u8> {
    let mut row = bgrx.repeat(4);
    row.resize(stride, 0x55);
    row.repeat(2)
}

#[test]
fn a_video_texture_shows_its_latest_frame_opaque() {
    let gpu = gpu::init(gpu::instance(None), None);
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, offscreen::FORMAT);
    let video = renderer.add_video_texture(&gpu.device, 4, 2);
    // The left half of the target; the right half stays the clear colour.
    let quad = Quad::image(Rect::new(0.0, 0.0, 32.0, 32.0), video, [0.0, 0.0, 1.0, 1.0], 1.0, 0.0);
    let frame = Frame { layers: vec![Layer { quads: vec![quad], texts: Vec::new() }] };

    // Blue 255, green 128, red 0, and a fourth byte of 0 that would hide the picture were it taken as alpha.
    renderer.update_video_texture(&gpu.queue, video, &frame_of([255, 128, 0, 0], 16), 16);
    let pixels = draw(&gpu, &mut renderer, &frame);
    assert_eq!(pixel(&pixels, 8, 16), [0, 128, 255, 255]);
    assert_eq!(pixel(&pixels, 48, 16), [0, 0, 0, 255]);

    // The next frame, from a decoder that pads its rows.
    renderer.update_video_texture(&gpu.queue, video, &frame_of([0, 0, 255, 7], 24), 24);
    let pixels = draw(&gpu, &mut renderer, &frame);
    assert_eq!(pixel(&pixels, 8, 16), [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 24, 8), [255, 0, 0, 255]);
}
