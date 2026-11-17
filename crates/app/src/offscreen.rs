//! `--screenshot`: the Dashboard drawn to a texture with no window, read back and saved as a PNG. What the Overlay
//! would show (`--launch-screen`, the Guide over a game) is drawn over Home's window's picture in the same frame,
//! and `--toast`'s Toast over both.
use model::Library;
use render::gpu;
use render::offscreen::{wait, FORMAT};
use screens::Dashboard;
use ui::host::Offscreen;
use ui::{Button, Host, Press};

use crate::{fail, since_start, Args};

/// `achievements_of` is one game's whole list of achievements, for an Achievements page a press opens; `fetch`
/// fetches what an art picker asks for, there and then.
pub fn run(
    args: &Args,
    library: Library,
    options: screens::Options,
    achievements_of: impl Fn(i64) -> Vec<model::achievement::Achievement>,
    fetch: impl Fn(Vec<model::art_picker::Ask>) -> Vec<model::art_picker::Answer>,
) {
    let out = args.screenshot.as_ref().unwrap();
    let size = args.size.unwrap_or((1920, 1080));
    let gpu = gpu::init(gpu::instance(None), None);
    let device_ready = since_start();

    let target = Offscreen::new(&gpu, size);
    let mut host = Host::new(&gpu, FORMAT, || {});
    let mut dashboard = Dashboard::new(library);
    // Settings > Helpers as they stand in `--data-dir`'s folder, if it names one.
    let helpers = crate::helper_programs::Helpers::for_args(args, args.data_dir.clone());
    dashboard.set_helpers(helpers.shown());
    dashboard.set_options(options);
    for text in &args.notify {
        dashboard.record(text);
    }
    dashboard.set_clock(args.clock.clone());
    // A made-up media session for the Audio Flyout's Now Playing card: Windows' are not read offscreen.
    if let Some(media) = args.fake_media().and_then(|mut fake| shell::media::MediaSessions::take(&mut fake)) {
        dashboard.set_media(media);
    }
    let press = |dashboard: &mut Dashboard, host: &mut Host, button| {
        dashboard.press(Press { button, repeat: false });
        // An A on a game's tile (or a Play history row): its Game hub opens when the double tap's time is up, over
        // the screen as it then looks, which the hub keeps a picture of. So that is drawn to rest first, and the
        // time then passed at once.
        if let Some(wait) = dashboard.timer() {
            target.still(&gpu, host, |ui| dashboard.draw(ui));
            dashboard.advance(wait);
        }
        // An Achievements page the press opened is given its game's list, as the window gives it.
        if let Some(game) = dashboard.achievements_wanted() {
            dashboard.set_achievements(game, achievements_of(game));
        }
        // An art picker is given its pages and their thumbnails, until it asks for nothing more.
        loop {
            let asks = dashboard.art_asks();
            if asks.is_empty() {
                break;
            }
            for answer in fetch(asks) {
                dashboard.art_answer(answer);
            }
        }
    };
    // Home starts with focus on the profile: a game to launch or a row to walk is on the Recent row.
    let on_row = args.focus.or((args.launch_screen || args.bench).then_some(0));
    for button in crate::focus_presses(on_row) {
        press(&mut dashboard, &mut host, button);
    }
    if let Some(page) = args.page.clone() {
        dashboard.open_page(page);
    }
    for &button in &args.press {
        press(&mut dashboard, &mut host, button);
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
                press(&mut dashboard, &mut host, if forward { Button::Right } else { Button::Left });
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
        press(&mut dashboard, &mut host, Button::Down);
    }
    // The first is the Toast in the picture: the rest wait behind it.
    for text in &args.toasts {
        dashboard.notify(text);
    }
    // Where the presses left focus, as the window's --log-focus says it.
    if args.log_focus {
        eprintln!("focus: {}", dashboard.focus_line());
    }
    // Endless animations stand where they do at this moment, the same every run.
    host.store.set_time(10.0);
    let pixels = target.still(&gpu, &mut host, |ui| dashboard.draw_all(ui));
    image::save_buffer(out, &pixels, size.0, size.1, image::ColorType::Rgba8)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
    eprintln!("wrote {} ({}x{}) at {:.1} ms", out.display(), size.0, size.1, since_start());
}
