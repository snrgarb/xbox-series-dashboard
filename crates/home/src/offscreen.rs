//! `--screenshot`: Home drawn to a texture with no window, read back and saved as a PNG.
use model::Library;

use crate::gpu::{self, Gpu};
use crate::{fail, since_start, Args, Home};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub fn run(args: &Args, library: &Library) {
    let out = args.screenshot.as_ref().unwrap();
    let size = args.size.unwrap_or((1920, 1080));
    let gpu = gpu::init(gpu::instance(None), None);
    let device_ready = since_start();

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
    let mut home = Home::new(&gpu, FORMAT, size, library, args.focus, || {});

    // The first frame goes out before any art is in, as the window's would.
    home.draw(&gpu, &view, size);
    let first_frame = since_start();
    while home.scene.visible_art_pending() {
        home.wait_for_art(&gpu);
    }
    let visible_art = since_start();

    if args.bench {
        while home.scene.art_pending() {
            home.wait_for_art(&gpu);
        }
        let all_art = since_start();
        home.scene.settle();
        // Focus walks the row and back, a move every 20 frames at 60 a second, each frame waited for on the GPU.
        const FRAMES: u32 = 300;
        let mut by = 1;
        let started = std::time::Instant::now();
        for frame in 0..FRAMES {
            if frame % 20 == 0 && !home.scene.move_focus(by) {
                by = -by;
                home.scene.move_focus(by);
            }
            home.scene.update(1.0 / 60.0);
            let index = home.draw(&gpu, &view, size);
            wait(&gpu, Some(index));
        }
        let frame_ms = started.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
        eprintln!("bench: device ready at {device_ready:.1} ms since process start");
        eprintln!("bench: first frame submitted (placeholders) at {first_frame:.1} ms");
        eprintln!("bench: visible art loaded at {visible_art:.1} ms, all art at {all_art:.1} ms");
        eprintln!(
            "bench: {frame_ms:.3} ms average frame ({:.0} fps) over {FRAMES} offscreen {}x{} frames with focus animating",
            1000.0 / frame_ms,
            size.0,
            size.1
        );
        while home.scene.focus() != args.focus
            && home.scene.move_focus(if home.scene.focus() > args.focus { -1 } else { 1 })
        {}
    }

    // `--launch-screen`: the screenshot is of the focused game's Launch screen.
    home.scene.set_launching(args.launch_screen);
    if args.guide {
        home.scene.open_guide(crate::window::guide_entries(library, None));
        home.scene.guide_move(1);
    }
    home.scene.settle();
    home.draw(&gpu, &view, size);
    let pixels = read_back(&gpu, &texture, size);
    image::save_buffer(out, &pixels, size.0, size.1, image::ColorType::Rgba8)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
    eprintln!("wrote {} ({}x{}) at {:.1} ms", out.display(), size.0, size.1, since_start());
}

fn wait(gpu: &Gpu, submission_index: Option<wgpu::SubmissionIndex>) {
    gpu.device.poll(wgpu::PollType::Wait { submission_index, timeout: None }).expect("poll device");
}

fn read_back(gpu: &Gpu, texture: &wgpu::Texture, size: (u32, u32)) -> Vec<u8> {
    let row = 4 * size.0 as usize;
    let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("read back"),
        size: (padded * size.1 as usize) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded as u32), rows_per_image: None },
        },
        wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
    );
    gpu.queue.submit(Some(encoder.finish()));
    buffer.map_async(wgpu::MapMode::Read, .., |result| result.expect("map read-back buffer"));
    wait(gpu, None);
    let mapped = buffer.get_mapped_range(..).expect("mapped range");
    mapped.chunks(padded).flat_map(|line| &line[..row]).copied().collect()
}
