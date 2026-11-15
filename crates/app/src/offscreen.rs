//! `--screenshot`: Home drawn to a texture with no window, read back and saved as a PNG.
use model::Library;

use render::gpu;
use render::offscreen::{read_back, wait, FORMAT};
use screens::Home;

use crate::{fail, since_start, Args};

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
        home.scene.open_guide(screens::guide_entries(library, None));
        home.scene.guide_move(1);
    }
    home.scene.settle();
    home.draw(&gpu, &view, size);
    let pixels = read_back(&gpu, &texture, size);
    image::save_buffer(out, &pixels, size.0, size.1, image::ColorType::Rgba8)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
    eprintln!("wrote {} ({}x{}) at {:.1} ms", out.display(), size.0, size.1, since_start());
}
