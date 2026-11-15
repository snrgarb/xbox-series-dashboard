//! The screens drawn offscreen and compared with the committed reference images (crates/shots).
//! `SHOTS=bless cargo test -p screens` accepts what they draw now. The fonts are tools/test-fonts.sh's.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::Dashboard;
use ui::host::Offscreen;
use ui::{Button, Host, Press};

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// The test fonts, before anything loads a font.
fn fonts() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
    });
}

/// Art for the fake library, drawn once: a cover, a wide background and a header for each of the first games,
/// each in its own colours with bands across it, so a crop or a stretch shows.
fn art_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("art");
        std::fs::create_dir_all(&dir).unwrap();
        for game in 0..6u32 {
            for (kind, w, h) in [("cover", 300, 450), ("hero", 960, 310), ("header", 460, 215)] {
                let hue = |shift: u32| (((game * 53 + shift * 85) % 200) + 40) as u8;
                let image = image::RgbImage::from_fn(w, h, |x, y| {
                    let band = (y * 8 / h) % 2 == 0;
                    let edge = x < 6 || y < 6 || x >= w - 6 || y >= h - 6;
                    let level = |c: u8| {
                        if edge {
                            255
                        } else if band {
                            c
                        } else {
                            c / 2
                        }
                    };
                    image::Rgb([level(hue(0)), level(hue(1)), level(hue(2))])
                });
                image.save(dir.join(format!("{game}-{kind}.png"))).unwrap();
            }
        }
        dir
    })
}

/// The fake library, its first games given art: the first three all of it, the fourth a cover alone, the fifth a
/// header alone, the rest none.
fn library_with_art() -> Library {
    let mut library = screens::fake_library();
    let art = |game: usize, kind: &str| Some(art_dir().join(format!("{game}-{kind}.png")));
    for (i, game) in library.games.iter_mut().enumerate() {
        let game: &mut Game = game;
        match i {
            0..=2 => (game.cover, game.hero, game.header) = (art(i, "cover"), art(i, "hero"), art(i, "header")),
            3 => game.cover = art(i, "cover"),
            4 => game.header = art(i, "header"),
            _ => {}
        }
    }
    library
}

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
}

impl Stage {
    fn new(library: Library, size: (u32, u32)) -> Self {
        fonts();
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, size);
        let mut dashboard = Dashboard::new(library);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard }
    }

    fn press(&mut self, buttons: &[Button]) {
        for &button in buttons {
            self.dashboard.press(Press { button, repeat: false });
        }
    }

    /// The screen at rest, as an image.
    fn still(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw(ui));
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }

    fn check(&mut self, name: &str) {
        let image = self.still();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }
}

const HD: (u32, u32) = (1920, 1080);

#[test]
fn home() {
    Stage::new(screens::fake_library(), HD).check("home");
}

#[test]
fn home_with_art() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.press(&[Button::Right]);
    stage.check("home-art");
}

#[test]
fn home_blurred_cover_behind_a_game_with_no_background() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.press(&[Button::Right, Button::Right, Button::Right]);
    stage.check("home-blurred-cover");
}

#[test]
fn home_wide_tile_focused() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.press(&[Button::Right, Button::Right, Button::Down]);
    stage.check("home-wide-tile");
}

#[test]
fn home_720p() {
    Stage::new(library_with_art(), (1280, 720)).check("home-720p");
}

/// Taller than 16:9: the canvas at the top, the background to the foot.
#[test]
fn home_16_10() {
    Stage::new(library_with_art(), (1280, 800)).check("home-16-10");
}

#[test]
fn home_ultrawide() {
    // Wider than 16:9: the canvas centred, the background to both edges.
    Stage::new(library_with_art(), (2560, 1080)).check("home-ultrawide");
}

#[test]
fn guide() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.open_guide();
    stage.press(&[Button::Down]);
    stage.check("guide");
}

#[test]
fn launch_screen() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.launch_started(0);
    stage.check("launch-screen");
}

/// The focused tile's title band is the theme's 90% black: over a white-edged cover its top edge is nearly black.
#[test]
fn the_title_band_is_nearly_opaque() {
    let mut stage = Stage::new(library_with_art(), HD);
    let image = stage.still();
    // The first tile, focused: 105..377 across, standing on 805.9. Its cover's edge is white for 6 of its pixels.
    let in_band = image.get_pixel(108, 790).0;
    assert!(in_band[0] < 40 && in_band[1] < 40 && in_band[2] < 40, "the band lets the cover through: {in_band:?}");
    let above_band = image.get_pixel(108, 600).0;
    assert!(above_band[0] > 200, "the cover's white edge should show above the band: {above_band:?}");
}

#[test]
fn focus_moves_along_the_recent_row_and_down_to_the_nearest_wide_tile() {
    use screens::home::Focus;
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.press(&[Button::Left]);
    assert_eq!(stage.dashboard.home_focus(), Focus::Recent(0));
    stage.press(&[Button::Right, Button::Right, Button::Right, Button::Right, Button::Right, Button::Right]);
    assert_eq!(stage.dashboard.home_focus(), Focus::Recent(6));
    stage.press(&[Button::Down]);
    assert_eq!(stage.dashboard.home_focus(), Focus::Wide(3));
    stage.press(&[Button::Left, Button::Up]);
    assert_eq!(stage.dashboard.home_focus(), Focus::Recent(6), "Up goes back to the tile Down left");
}

#[test]
fn a_launch_asks_the_shell_and_the_game_ending_brings_home_back_on_its_tile() {
    use screens::home::Focus;
    use screens::Effect;
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.press(&[Button::Right, Button::Right, Button::A]);
    assert_eq!(stage.dashboard.take_effects(), [Effect::Launch(2)]);
    stage.dashboard.launch_started(2);
    // While it starts, and behind it once it runs, Home takes no input.
    stage.press(&[Button::Left, Button::A]);
    stage.dashboard.game_booted();
    stage.press(&[Button::Left, Button::A]);
    assert!(stage.dashboard.take_effects().is_empty());
    // The Guide over the game offers Quit.
    stage.press(&[Button::Guide, Button::Down, Button::A]);
    assert_eq!(stage.dashboard.take_effects(), [Effect::QuitGame, Effect::GuideClosed]);
    stage.dashboard.game_ended();
    assert_eq!(stage.dashboard.playing(), None);
    assert_eq!(stage.dashboard.home_focus(), Focus::Recent(2));
}
