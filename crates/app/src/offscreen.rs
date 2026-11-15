//! `--screenshot`: the Dashboard drawn to a texture with no window, read back and saved as a PNG.
use model::Library;
use render::gpu;
use render::offscreen::{wait, FORMAT};
use screens::Dashboard;
use ui::host::Offscreen;
use ui::{Button, Host, Press};

use crate::{fail, since_start, Args};

pub fn run(args: &Args, library: Library) {
    let out = args.screenshot.as_ref().unwrap();
    let size = args.size.unwrap_or((1920, 1080));
    let gpu = gpu::init(gpu::instance(None), None);
    let device_ready = since_start();

    let target = Offscreen::new(&gpu, size);
    let mut host = Host::new(&gpu, FORMAT, || {});
    let mut dashboard = Dashboard::new(library);
    dashboard.set_clock(args.clock.clone());
    let press = |dashboard: &mut Dashboard, button| dashboard.press(Press { button, repeat: false });
    // Home starts with focus on the profile: a game to launch or a row to walk is on the Recent row.
    let on_row = args.focus.or((args.launch_screen || args.bench).then_some(0));
    for button in crate::focus_presses(on_row) {
        press(&mut dashboard, button);
    }
    for &button in &args.press {
        press(&mut dashboard, button);
    }

    // The first frame goes out before any art is in, as the window's would.
    let index = host.draw(&gpu, &target.view, size, 0.0, wgpu::Color::BLACK, |ui| dashboard.draw(ui));
    wait(&gpu, Some(index));
    let first_frame = since_start();
    target.still(&gpu, &mut host, |ui| dashboard.draw(ui));
    let art = since_start();

    if args.bench {
        // Focus walks the row and back, a move every 20 frames at 60 a second, each frame waited for on the GPU.
        const FRAMES: u32 = 300;
        let count = dashboard.library().recent(screens::home::RECENT_GAMES).len().max(1) as u32;
        let started = std::time::Instant::now();
        for frame in 0..FRAMES {
            if frame % 20 == 0 {
                let step = frame / 20;
                let forward = (step / (count - 1).max(1)).is_multiple_of(2);
                press(&mut dashboard, if forward { Button::Right } else { Button::Left });
            }
            let index = host.draw(&gpu, &target.view, size, 1.0 / 60.0, wgpu::Color::BLACK, |ui| dashboard.draw(ui));
            wait(&gpu, Some(index));
        }
        let frame_ms = started.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
        eprintln!("bench: device ready at {device_ready:.1} ms since process start");
        eprintln!("bench: first frame submitted (placeholders) at {first_frame:.1} ms, art loaded at {art:.1} ms");
        eprintln!(
            "bench: {frame_ms:.3} ms average frame ({:.0} fps) over {FRAMES} offscreen {}x{} frames with focus animating",
            1000.0 / frame_ms,
            size.0,
            size.1
        );
    }

    if args.launch_screen {
        if let Some(game) = dashboard.focused_game() {
            dashboard.launch_started(game);
        }
    }
    if args.guide {
        dashboard.open_guide();
        press(&mut dashboard, Button::Down);
    }
    // Endless animations stand where they do at this moment, the same every run.
    host.store.set_time(10.0);
    let pixels = target.still(&gpu, &mut host, |ui| dashboard.draw(ui));
    image::save_buffer(out, &pixels, size.0, size.1, image::ColorType::Rgba8)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
    eprintln!("wrote {} ({}x{}) at {:.1} ms", out.display(), size.0, size.1, since_start());
}
