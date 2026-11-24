//! The launchers' marks (docs/spec/home.md 3.3.1), the stores' own and the project's few, drawn offscreen and
//! compared with the committed reference: on a tile's corner at the size a tile carries it, at the size that is in
//! pixels on a 4K screen, and enlarged with its name.
use std::path::{Path, PathBuf};

use render::gpu;
use render::offscreen::FORMAT;
use screens::drawn::Drawing;
use screens::home;
use screens::theme::BODY;
use ui::host::Offscreen;
use ui::{Color, Host, Rect};

fn fonts() {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let fonts = target.join("fonts");
    assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
    std::env::set_var("DASHBOARD_FONTS", fonts);
}

#[test]
#[ignore = "screenshot"]
fn launcher_marks() {
    fonts();
    let gpu = gpu::init(gpu::instance(None), None);
    let mut host = Host::new(&gpu, FORMAT, || {});
    let target = Offscreen::new(&gpu, (1920, 1080));
    let pixels = target.still(&gpu, &mut host, |ui| {
        ui.fill(ui.screen(), Color::rgb(0x101010), 0.0);
        for (i, &drawing) in Drawing::ALL.iter().enumerate() {
            let x = 40.0 + i as f32 * 155.0;
            // On a tile's corner, as Home's Recent row and My games & apps carry it: a light tile and a dark one.
            for (row, colour) in [(0.0, 0x6A8CAF), (1.0, 0x1A1A1A)] {
                let tile = Rect::new(x, 60.0 + row * 140.0, 120.0, 120.0);
                ui.fill(tile, Color::rgb(colour), 6.0);
                home::source_badge_at(ui, tile, drawing, 1.0);
            }
            // 40 across: what a tile's 20 is in pixels on a 4K screen.
            home::mark_drawn(ui, Rect::new(x + 40.0, 360.0, 40.0, 40.0), drawing);
            // Enlarged, with its name.
            home::mark_drawn(ui, Rect::new(x, 460.0, 120.0, 120.0), drawing);
            ui.label_centered(x + 60.0, 620.0, drawing.name(), &BODY);
        }
    });
    let image = image::RgbaImage::from_raw(1920, 1080, pixels).unwrap();
    let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("launcher-marks.png");
    image.save(&got).unwrap();
    shots::check("launcher-marks", &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}
