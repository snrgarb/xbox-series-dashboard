//! The screens drawn offscreen and compared with the committed reference images (crates/shots).
//! `SHOTS=bless cargo test -p screens` accepts what they draw now. The fonts are tools/test-fonts.sh's.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
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
        for game in 0..13u32 {
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

/// The fake library, some of its games given art: the first three all of it, the fourth a cover alone, the fifth a
/// header alone, and a cover alone for a few further down, which the Play history and See all tiles show; the rest
/// none.
fn library_with_art() -> Library {
    let mut library = screens::fake_library();
    let art = |game: usize, kind: &str| Some(art_dir().join(format!("{game}-{kind}.png")));
    for (i, game) in library.games.iter_mut().enumerate() {
        let game: &mut Game = game;
        match i {
            0..=2 => (game.cover, game.hero, game.header) = (art(i, "cover"), art(i, "hero"), art(i, "header")),
            3 => game.cover = art(i, "cover"),
            4 => game.header = art(i, "header"),
            7..=9 | 12 => game.cover = art(i, "cover"),
            _ => {}
        }
    }
    library
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once on as many threads.
static DRAWING: Mutex<()> = Mutex::new(());

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(library: Library, size: (u32, u32)) -> Self {
        fonts();
        // A test that failed while drawing leaves nothing behind that the next would trip on.
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, size);
        let mut dashboard = Dashboard::new(library);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    /// Home starts with focus on the profile; most pictures are of it once Down has put focus on the Recent row's
    /// first tile.
    fn on_row(library: Library, size: (u32, u32)) -> Self {
        let mut stage = Self::new(library, size);
        stage.press(&[Button::Down]);
        stage
    }

    fn press(&mut self, buttons: &[Button]) {
        for &button in buttons {
            self.dashboard.press(Press { button, repeat: false });
        }
    }

    /// The screen at rest, as an image.
    fn still(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        // Home's window and the Overlay over it, in one picture.
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }

    /// The Guide alone, at rest, over a screen of one colour: what is seen of it over a game.
    fn still_over(&mut self, game: ui::Color) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| {
            ui.fill(ui.screen(), game, 0.0);
            dashboard.draw_guide(ui);
        });
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }

    fn check(&mut self, name: &str) {
        let image = self.still();
        compare(name, &image);
    }

    /// One frame, `dt` seconds after the last on the ui's clock: what is mid-flight stays mid-flight. Art the frame
    /// asks for is decoded and the frame drawn again at the same moment, however long that takes, so a frame shows
    /// the same on any machine.
    fn step(&mut self, mut dt: f32) {
        let Self { gpu, host, target, dashboard, .. } = self;
        loop {
            let index = host.draw(gpu, &target.view, target.size, dt, wgpu::Color::BLACK, |ui| dashboard.draw(ui));
            wait(gpu, Some(index));
            if !host.art.wait(gpu, &mut host.renderer) {
                break;
            }
            while host.art.wait(gpu, &mut host.renderer) {}
            dt = 0.0;
        }
    }

    /// Frames `step` seconds apart until `seconds` have gone by.
    fn run(&mut self, seconds: f32, step: f32) {
        for _ in 0..(seconds / step).round() as u32 {
            self.step(step);
        }
    }

    /// This many frames a `TICK` apart.
    fn ticks(&mut self, frames: u32) {
        for _ in 0..frames {
            self.step(TICK);
        }
    }

    /// The screen as it is at this moment, as an image.
    fn frame(&mut self) -> image::RgbaImage {
        self.step(0.0);
        let pixels = read_back(&self.gpu, &self.target.texture, self.target.size);
        image::RgbaImage::from_raw(self.target.size.0, self.target.size.1, pixels).unwrap()
    }

    /// The background's colour in the clear between the top bar and the rows, where the scrim leaves it as it is.
    fn background(&mut self) -> [u8; 3] {
        let pixel = self.frame().get_pixel(960, 300).0;
        [pixel[0], pixel[1], pixel[2]]
    }

    fn check_frame(&mut self, name: &str) {
        let image = self.frame();
        compare(name, &image);
    }

    /// The screen `dt` seconds after the last frame, as an image.
    fn frame_after(&mut self, dt: f32) -> image::RgbaImage {
        self.step(dt);
        self.frame()
    }

    fn check_image(&mut self, name: &str, image: image::RgbaImage) {
        compare(name, &image);
    }
}

fn compare(name: &str, image: &image::RgbaImage) {
    let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
    image.save(&got).unwrap();
    shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}

const HD: (u32, u32) = (1920, 1080);

/// The step of the tests that follow the background over time: 25 frames a second. None of the background's waits
/// (1.5 s, 150 ms, 100 ms) is a whole number of them, so no frame falls on the moment a wait ends. The first frame
/// after a press is the one a wait is timed from: the wait of 1.5 s is over in the 39th, 1.52 s later.
const TICK: f32 = 0.04;

#[test]
fn home() {
    Stage::on_row(screens::fake_library(), HD).check("home");
}

#[test]
fn home_with_art() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Right]);
    stage.check("home-art");
}

#[test]
fn home_blurred_cover_behind_a_game_with_no_background() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Right, Button::Right, Button::Right]);
    stage.check("home-blurred-cover");
}

#[test]
fn home_wide_tile_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Right, Button::Right, Button::Down]);
    stage.check("home-wide-tile");
}

#[test]
fn home_720p() {
    Stage::on_row(library_with_art(), (1280, 720)).check("home-720p");
}

/// Taller than 16:9: the canvas at the top, the background to the foot.
#[test]
fn home_16_10() {
    Stage::on_row(library_with_art(), (1280, 800)).check("home-16-10");
}

#[test]
fn home_ultrawide() {
    // Wider than 16:9: the canvas centred, the background to both edges.
    Stage::on_row(library_with_art(), (2560, 1080)).check("home-ultrawide");
}

/// Home as it starts: focus on the profile, no Recent tile grown, the base behind. With a pad connected, its glyph
/// by the clock.
#[test]
fn home_as_it_starts() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.set_pad_connected(true);
    stage.check("home-start");
}

/// The swirl some twelve seconds into its movement: turned most of the way back to level, its bands drifted across.
#[test]
fn home_swirl_later() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.run(12.0, 0.1);
    stage.check_frame("home-swirl-later");
}

/// Under Reduce motion the swirl stands as it is authored, however long it shows.
#[test]
fn home_swirl_under_reduce_motion() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.host.store.reduce_motion = true;
    stage.run(12.0, 0.1);
    stage.check_frame("home-swirl-reduce-motion");
}

/// The swirl fills a screen of another shape: scaled until it covers it, centred.
#[test]
fn home_swirl_16_10() {
    Stage::new(library_with_art(), (1280, 800)).check("home-swirl-16-10");
}

#[test]
fn home_swirl_ultrawide() {
    Stage::new(library_with_art(), (2560, 1080)).check("home-swirl-ultrawide");
}

/// Focus has rested on the row's second game: its art has taken the first's place.
#[test]
fn home_background_follows_focus_after_the_delay() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.ticks(15);
    stage.press(&[Button::Right]);
    stage.ticks(55);
    stage.check_frame("home-background-followed");
}

/// Half way through the crossfade from the first game's art to the second's.
#[test]
fn home_background_mid_crossfade() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.ticks(15);
    stage.press(&[Button::Right]);
    // The wait, in whose last frame the art sets off; then half the fade's 0.4 s.
    stage.ticks(39);
    stage.ticks(5);
    stage.check_frame("home-background-crossfade");
}

/// Half way through a blurred cover's fade in over the swirl: it comes darkened, as one with its black sheet.
#[test]
fn home_blurred_cover_fading_in_over_the_swirl() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.ticks(10);
    stage.press(&[Button::Down, Button::Right, Button::Right, Button::Right]);
    // Over the swirl there is no wait: the cover sets off in the first frame.
    stage.ticks(1);
    stage.ticks(5);
    stage.check_frame("home-blurred-cover-fading");
}

/// A round button with focus: its ring, its label under it, its icon larger.
#[test]
fn home_top_bar_button_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Up, Button::Right, Button::Right]);
    stage.check("home-top-bar-button");
}

#[test]
fn home_play_history_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Right; 7]);
    stage.check("home-play-history");
}

/// The page at rest on the first Group: a game's card with focus.
#[test]
fn home_group_card_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Down, Button::Down, Button::Right]);
    stage.check("home-group");
}

/// The page at rest on the last Group: its See all card with focus.
#[test]
fn home_see_all_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Down, Button::Down, Button::Down]);
    stage.press(&[Button::Right; 6]);
    stage.check("home-see-all");
}

/// The scrolled rests on a taller screen: there is more of the page under the wide tiles, so it rises less.
#[test]
fn home_wide_tile_focused_16_10() {
    let mut stage = Stage::on_row(library_with_art(), (1280, 800));
    stage.press(&[Button::Down]);
    stage.check("home-wide-tile-16-10");
}

#[test]
fn home_group_card_focused_16_10() {
    let mut stage = Stage::on_row(library_with_art(), (1280, 800));
    stage.press(&[Button::Down, Button::Down, Button::Right, Button::Right, Button::Right]);
    stage.check("home-group-16-10");
}

#[test]
fn launch_screen() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.launch_started(0);
    stage.check("launch-screen");
}

/// A game with a cover and no background art: the cover, blurred and darkened.
#[test]
fn launch_screen_with_a_cover_alone() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.launch_started(3);
    stage.check("launch-screen-cover");
}

/// A game with no art at all: black, the Spinner and its name, here long enough to wrap.
#[test]
fn launch_screen_with_no_art() {
    let mut library = screens::fake_library();
    library.games[0].name = "The Legend of the Seven Lanterns: Definitive Anniversary Edition".into();
    let mut stage = Stage::new(library, HD);
    stage.dashboard.launch_started(0);
    stage.check("launch-screen-no-art");
}

/// What the Overlay is wanted for through a launch, and when Home's window has nothing to show.
#[test]
fn the_overlay_shows_the_launch_screen_then_the_guide_over_the_game() {
    use screens::OverlayShows;
    let mut stage = Stage::new(screens::fake_library(), HD);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    // The Guide over Home is in Home's window, not on the Overlay.
    stage.press(&[Button::Guide]);
    stage.still();
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    assert!(!stage.dashboard.guide_over_game());
    stage.press(&[Button::B]);
    stage.still();

    // The Launch screen: passive, and Home is covered once it has faded in.
    stage.dashboard.launch_started(2);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Passive);
    assert!(!stage.dashboard.home_covered(), "Home shows until the Launch screen has faded in over it");
    assert!(!stage.dashboard.overlay_at_rest(), "fading in");
    stage.still();
    assert!(stage.dashboard.home_covered());
    assert!(stage.dashboard.launch_screen_showing());
    assert!(stage.dashboard.overlay_at_rest(), "only the Spinner turns");

    // Booted: it fades away, and then the Overlay has nothing to show.
    stage.dashboard.game_booted();
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Passive, "still fading");
    assert!(!stage.dashboard.overlay_at_rest(), "fading away");
    stage.still();
    assert!(!stage.dashboard.launch_screen_showing());
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    assert!(stage.dashboard.home_covered(), "the game is in front");

    // The Guide over the game takes input; closed, it slides away passive, then the Overlay is done.
    stage.press(&[Button::Guide]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Guide);
    assert!(stage.dashboard.guide_over_game());
    stage.still();
    stage.press(&[Button::B]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Passive);
    assert!(!stage.dashboard.guide_over_game());
    stage.still();
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);

    stage.dashboard.game_ended();
    assert!(!stage.dashboard.home_covered());
}

/// Where the Overlay can't be made, nothing that needs it is shown, and nothing waits for it.
#[test]
fn with_no_overlay_there_is_no_launch_screen_and_no_guide_over_a_game() {
    use screens::OverlayShows;
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.dashboard.set_overlay(false);
    stage.dashboard.launch_started(2);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    assert!(!stage.dashboard.launch_screen_showing());
    assert!(stage.dashboard.home_covered());
    stage.dashboard.game_booted();
    stage.press(&[Button::Guide]);
    assert!(!stage.dashboard.guide_open());
    stage.dashboard.game_ended();
    stage.press(&[Button::Guide]);
    assert!(stage.dashboard.guide_open(), "at Home the Guide is in Home's window");
}

/// Losing the Overlay mid-launch drops what it showed.
#[test]
fn an_overlay_lost_takes_what_it_showed_with_it() {
    use screens::OverlayShows;
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.dashboard.launch_started(2);
    stage.dashboard.game_booted();
    stage.press(&[Button::Guide]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Guide);
    stage.dashboard.set_overlay(false);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    assert!(!stage.dashboard.guide_open() && !stage.dashboard.launch_screen_showing());
}

/// The Launch screen is the whole screen's: nothing of Home shows through it, corner to corner.
#[test]
fn the_launch_screen_with_no_art_is_black_to_the_corners() {
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.dashboard.launch_started(0);
    let image = stage.still();
    for (x, y) in [(0, 0), (1919, 0), (0, 1079), (1919, 1079), (400, 300), (1500, 900)] {
        assert_eq!(image.get_pixel(x, y).0, [0, 0, 0, 255], "at ({x}, {y})");
    }
    // The Spinner is there: something white on its ring, round the screen's middle.
    let on_ring = (0..360).any(|degrees| {
        let (sin, cos) = (degrees as f32).to_radians().sin_cos();
        let pixel = image.get_pixel((960.0 + 29.4 * sin) as u32, (540.0 - 29.4 * cos) as u32).0;
        pixel[0] > 200 && pixel[1] > 200 && pixel[2] > 200
    });
    assert!(on_ring, "no white on the Spinner's ring");
}

/// The focused tile's title band is the theme's 90% black: over a white-edged cover its top edge is nearly black.
#[test]
fn the_title_band_is_nearly_opaque() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    let image = stage.still();
    // The first tile, focused: 105..377 across, standing on 805.9. Its cover's edge is white for 6 of its pixels.
    let in_band = image.get_pixel(108, 790).0;
    assert!(in_band[0] < 40 && in_band[1] < 40 && in_band[2] < 40, "the band lets the cover through: {in_band:?}");
    let above_band = image.get_pixel(108, 600).0;
    assert!(above_band[0] > 200, "the cover's white edge should show above the band: {above_band:?}");
}

// --- Measures the Bundle's XAML sets, looked for in the picture. The comparison with a reference lets a few
// thousand pixels differ, which a line of text, or a band 8 taller, is within. docs/spec/home.md says which Bundle:
// its pictures are of an older one, with smaller type and the Groups under the top bar. ---

/// Whether a pixel is text's white, or the dimmer white of a subtitle, and not what is under it.
fn ink(image: &image::RgbaImage, x: u32, y: u32) -> bool {
    image.get_pixel(x, y).0[..3].iter().all(|&c| c > 150)
}

/// The rows from `top` to `bottom` with ink between `left` and `right`: the first and the last, if any has.
fn ink_rows(image: &image::RgbaImage, (left, right): (u32, u32), (top, bottom): (u32, u32)) -> Option<(u32, u32)> {
    let mut rows = (top..bottom).filter(|&y| (left..right).any(|x| ink(image, x, y)));
    let first = rows.next()?;
    Some((first, rows.next_back().unwrap_or(first)))
}

/// A focused card's title band holds a line of Segoe UI 26 with 8 (of the rows' units) round it: 52.4 tall, not
/// the 44.5 of the type the Bundle had before (E/HomeTiles.xaml:41, 88-91).
#[test]
fn a_title_band_of_one_line_is_52_tall() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Down, Button::Down, Button::Right]);
    let image = stage.still();
    // The first Group's second card: 352.3..568.4 across, 120.2..450 down, so its band starts at 397.6, over a
    // cover that is nowhere near black.
    let above = image.get_pixel(360, 393).0;
    assert!(above[..3].iter().any(|&c| c > 60), "the cover should show above the band: {above:?}");
    let within = image.get_pixel(360, 402).0;
    assert!(within[..3].iter().all(|&c| c < 40), "the band should have started by 402 down: {within:?}");
}

/// With focus on a Group the page has risen until the Group's header is 64 from the top, and the top bar has gone
/// up off the screen with it (C/HomeRests.cs:105, 140-143). The Bundle's pictures, older, have it under the bar.
#[test]
fn on_a_group_its_header_is_64_down_and_the_top_bar_has_gone() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    let pale = |image: &image::RgbaImage| image.get_pixel(120, 100).0[..3].iter().all(|&c| c > 200);
    assert!(pale(&stage.still()), "the profile's picture should be at the top left with focus on the Recent row");
    stage.press(&[Button::Down, Button::Down]);
    let image = stage.still();
    assert!(!pale(&image), "the top bar should have gone with focus on a Group");
    // The header is a line of Bahnschrift 32, 38.4 tall from 64: its capital starts a little under the line's top.
    let (first, last) = ink_rows(&image, (105, 300), (0, 110)).expect("the Group's header");
    assert!((64..80).contains(&first) && (88..103).contains(&last), "the header's ink is at {first}..{last}");
}

/// A wide tile's caption stands 14 (of the rows' units) off the tile's foot, in lines of Segoe UI 26: a lone title
/// in 994.2..1028.7, and with a subtitle under it the title in 959.6..994.2 (E/PromoRow.xaml:44-58). Achievements
/// and Downloads have the second line.
#[test]
fn the_wide_tiles_captions_stand_on_their_foot_in_one_line_or_two() {
    let mut stage = Stage::new(screens::fake_library(), HD);
    let image = stage.still();
    // Each tile's caption starts 20 in from its left edge; 150 across is short of the glyph in the tile's middle.
    for (tile, left, two_lines) in [(0, 125, false), (1, 558, false), (2, 992, true), (3, 1425, true)] {
        let across = (left, left + 150);
        let foot = ink_rows(&image, across, (994, 1029));
        assert!(foot.is_some(), "wide tile {tile} should have a line on its foot");
        assert_eq!(ink_rows(&image, across, (1029, 1044)), None, "wide tile {tile} has text under its caption");
        let over = ink_rows(&image, across, (960, 994));
        assert_eq!(over.is_some(), two_lines, "wide tile {tile}'s line over the foot: {over:?}");
    }
}

// --- The background's timing, on the ui's clock. docs/spec/home.md, section 8, has the rules. ---

/// The colours of the fake library's background art where `Stage::background` looks: the first game's, the
/// second's.
const FIRST_ART: [u8; 3] = [40, 125, 210];
const SECOND_ART: [u8; 3] = [93, 178, 63];

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&a, b)| a.abs_diff(b) <= 6)
}

#[test]
fn the_background_follows_focus_only_once_it_has_rested_on_a_tile() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    // Coming onto the row over the swirl there is no wait: only the fade.
    stage.ticks(15);
    assert!(near(stage.background(), FIRST_ART), "{:?}", stage.background());

    stage.press(&[Button::Right]);
    stage.ticks(25);
    assert!(near(stage.background(), FIRST_ART), "the art changed before the delay: {:?}", stage.background());
    // Nothing is moving meanwhile: the window sleeps until the delay's end, 0.54 s on.
    assert!(!stage.host.animating(), "frames are drawn all through the delay");
    let wake = stage.host.wake().expect("the delay's end asks for a frame");
    assert!((0.5..=0.58).contains(&wake), "{wake}");

    // Another move starts the wait again, even one that comes straight back.
    stage.ticks(10);
    stage.press(&[Button::Left, Button::Right]);
    stage.ticks(35);
    assert!(near(stage.background(), FIRST_ART), "a move did not restart the delay: {:?}", stage.background());

    // The delay over, the art decoded, the crossfade done.
    stage.ticks(20);
    assert!(near(stage.background(), SECOND_ART), "{:?}", stage.background());
    assert!(!stage.host.animating() && stage.host.wake().is_none(), "at rest under art, Home asks for no frame");
}

#[test]
fn with_background_delay_off_the_wait_is_short() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.set_background_delay(false);
    stage.ticks(15);
    stage.press(&[Button::Right]);
    stage.ticks(4);
    assert!(near(stage.background(), FIRST_ART), "the art changed at once: {:?}", stage.background());
    // 150 ms from the first frame is over in the fifth; then the fade.
    stage.ticks(18);
    assert!(near(stage.background(), SECOND_ART), "{:?}", stage.background());
}

#[test]
fn the_swirl_comes_back_a_moment_after_focus_leaves_the_rows_games() {
    let green = |colour: [u8; 3]| colour[1] > colour[0] && colour[1] > colour[2];
    let mut stage = Stage::new(library_with_art(), HD);
    let swirl = stage.background();
    assert!(green(swirl), "{swirl:?}");
    stage.press(&[Button::Down]);
    stage.ticks(15);
    assert!(near(stage.background(), FIRST_ART));

    // Up to the top bar: the art stays for a tenth of a second, then fades out over the swirl.
    stage.press(&[Button::Up]);
    stage.ticks(2);
    assert!(near(stage.background(), FIRST_ART), "{:?}", stage.background());
    stage.ticks(15);
    let back = stage.background();
    assert!(green(back) && !near(back, FIRST_ART), "{back:?}");

    // A game with no art at all leaves up what is up: the sixth has none, after the fifth's blurred header.
    stage.press(&[Button::Down, Button::Right, Button::Right, Button::Right, Button::Right]);
    stage.ticks(15);
    let fifth = stage.background();
    assert!(!near(fifth, back), "the fifth game's header is not up: {fifth:?}");
    stage.press(&[Button::Right]);
    stage.ticks(60);
    assert!(near(stage.background(), fifth), "{:?} became {:?}", fifth, stage.background());
}

#[test]
fn the_swirl_moves_at_thirty_frames_a_second_and_not_at_all_under_reduce_motion() {
    let mut stage = Stage::new(library_with_art(), HD);
    let start = stage.frame();
    assert!(!stage.host.animating(), "the swirl alone does not ask for every frame");
    assert_eq!(stage.host.wake(), Some(1.0 / 30.0));
    stage.run(3.0, 0.1);
    assert!(shots::difference(&stage.frame(), &start).share > 0.01, "the swirl has not moved");

    // Under art the swirl is not drawn, and nothing asks for a frame.
    stage.press(&[Button::Down]);
    stage.ticks(15);
    assert!(!stage.host.animating() && stage.host.wake().is_none());

    // Under the Guide the swirl holds still, as the Bundle's does behind another window.
    stage.press(&[Button::Up]);
    stage.dashboard.open_guide();
    stage.ticks(25);
    assert!(!stage.host.animating() && stage.host.wake().is_none(), "the swirl moves under the Guide");

    // One stage at a time draws.
    drop(stage);
    let mut stage = Stage::new(library_with_art(), HD);
    stage.host.store.reduce_motion = true;
    let start = stage.frame();
    assert!(!stage.host.animating() && stage.host.wake().is_none(), "a swirl standing still asks for no frame");
    stage.run(3.0, 0.1);
    assert_eq!(shots::difference(&stage.frame(), &start).share, 0.0);
}

// --- Where focus goes and what a press asks for: no picture, so no GPU. docs/spec/home.md has the rules. ---

use screens::home::{Focus, TopButton};
use screens::Effect;
use Button::{Down, Left, Right, Up, A, B};

/// A Dashboard on the fake library: seven games in the Recent row and the Play history tile; two Groups, Installed
/// and Most played, each six games and See all. Focus is on the Recent row's first tile.
fn core() -> Dashboard {
    on_row(screens::fake_library())
}

/// A Dashboard with focus brought down from the profile, where Home starts, to the Recent row's first tile.
fn on_row(library: Library) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    press(&mut dashboard, &[Down]);
    dashboard
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Presses, then says where focus is.
fn after(dashboard: &mut Dashboard, buttons: &[Button]) -> Focus {
    press(dashboard, buttons);
    dashboard.home_focus()
}

/// What has been asked of the shell, but for the sounds, which tests/sounds.rs is about.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

fn card(group: usize, card: usize) -> Focus {
    Focus::Group { group, card }
}

#[test]
fn home_starts_on_the_profile_and_down_is_the_recent_rows_first_tile() {
    let mut home = Dashboard::new(screens::fake_library());
    assert_eq!(home.home_focus(), Focus::TopBar(TopButton::Profile));
    assert_eq!(home.focused_game(), None);
    assert_eq!(after(&mut home, &[Down]), Focus::Recent(0));
}

#[test]
fn focus_moves_along_the_recent_row_and_stops_at_its_ends() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Left]), Focus::Recent(0));
    assert_eq!(after(&mut home, &[Right; 6]), Focus::Recent(6));
    assert_eq!(after(&mut home, &[Right]), Focus::Recent(7), "the Play history tile is the row's last");
    assert_eq!(after(&mut home, &[Right]), Focus::Recent(7));
    assert_eq!(after(&mut home, &[B]), Focus::Recent(7), "B does nothing on the Recent row");
}

#[test]
fn down_from_the_recent_row_goes_to_the_nearest_wide_tile_and_up_comes_back_to_the_tile_it_left() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Down]), Focus::Wide(0));
    assert_eq!(after(&mut home, &[Up]), Focus::Recent(0));
    assert_eq!(after(&mut home, &[Right, Right, Right, Down]), Focus::Wide(1), "the fourth tile is over the second");
    assert_eq!(after(&mut home, &[Up, Right, Right, Right, Down]), Focus::Wide(3));
    // Along the wide tiles and back up: to the tile Down left, not the one over this wide tile.
    assert_eq!(after(&mut home, &[Left, Left, Left]), Focus::Wide(0));
    assert_eq!(after(&mut home, &[Left]), Focus::Wide(0), "the wide tiles stop at their ends");
    assert_eq!(after(&mut home, &[Up]), Focus::Recent(6));
    // From the Play history tile too.
    assert_eq!(after(&mut home, &[Right, Down]), Focus::Wide(3));
    assert_eq!(after(&mut home, &[Right]), Focus::Wide(3));
    assert_eq!(after(&mut home, &[Up]), Focus::Recent(7));
}

#[test]
fn up_from_the_recent_row_goes_to_the_top_bar_button_over_the_tile_and_down_comes_back_under_it() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Up]), Focus::TopBar(TopButton::Profile));
    assert_eq!(after(&mut home, &[Up]), Focus::TopBar(TopButton::Profile), "there is nothing over the top bar");
    assert_eq!(after(&mut home, &[Down]), Focus::Recent(0));
    assert_eq!(after(&mut home, &[Right, Right, Right, Up]), Focus::TopBar(TopButton::MyGames));
    assert_eq!(after(&mut home, &[Down]), Focus::Recent(3));
    assert_eq!(after(&mut home, &[Right, Right, Right, Right, Up]), Focus::TopBar(TopButton::Notifications));
    assert_eq!(after(&mut home, &[Down]), Focus::Recent(7));
}

#[test]
fn focus_moves_along_the_top_bar_and_stops_at_its_ends() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Up, Left]), Focus::TopBar(TopButton::Profile));
    assert_eq!(after(&mut home, &[Right]), Focus::TopBar(TopButton::MyGames));
    assert_eq!(after(&mut home, &[Right]), Focus::TopBar(TopButton::Search));
    assert_eq!(after(&mut home, &[Right]), Focus::TopBar(TopButton::Settings));
    assert_eq!(after(&mut home, &[Right]), Focus::TopBar(TopButton::Notifications));
    assert_eq!(after(&mut home, &[Right]), Focus::TopBar(TopButton::Notifications));
    assert_eq!(after(&mut home, &[B]), Focus::TopBar(TopButton::Notifications), "B does nothing on the top bar");
    assert_eq!(after(&mut home, &[Left, Left, Left, Left, Left]), Focus::TopBar(TopButton::Profile));
}

#[test]
fn a_on_the_profile_opens_the_guide_and_on_a_button_with_no_page_yet_does_nothing() {
    let mut home = core();
    press(&mut home, &[Up]);
    for _ in 0..4 {
        press(&mut home, &[Right, A]);
        assert!(!home.guide_open() && asked(&mut home).is_empty(), "{:?}", home.home_focus());
    }
    assert_eq!(after(&mut home, &[Left, Left, Left, Left, A]), Focus::TopBar(TopButton::Profile));
    assert!(home.guide_open());
}

#[test]
fn down_from_the_wide_tiles_enters_the_first_group_at_its_start_and_up_comes_back_to_the_wide_tile() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Right, Right, Right, Down, Down]), card(0, 0));
    assert_eq!(after(&mut home, &[Right, Right, Up]), Focus::Wide(1), "the wide tile last focused");
    assert_eq!(after(&mut home, &[Right, Down]), card(0, 0), "always the first card, wherever focus was before");
    assert_eq!(after(&mut home, &[Up]), Focus::Wide(2));
    assert_eq!(after(&mut home, &[Up]), Focus::Recent(3), "the Recent tile Down left, after a trip through the Groups");
}

#[test]
fn focus_moves_along_a_group_and_keeps_its_place_between_groups() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Down, Down, Left]), card(0, 0));
    assert_eq!(after(&mut home, &[Right; 4]), card(0, 4));
    assert_eq!(after(&mut home, &[Down]), card(1, 4));
    assert_eq!(after(&mut home, &[Down]), card(1, 4), "there is nothing under the last Group");
    assert_eq!(after(&mut home, &[Right, Right]), card(1, 6), "See all is the Group's last card");
    assert_eq!(after(&mut home, &[Right]), card(1, 6));
    assert_eq!(after(&mut home, &[Up]), card(0, 6));
}

/// A library whose second Group is shorter than its first: four games installed, two of them played.
fn short_library() -> Library {
    let mut library = screens::fake_library();
    library.games.truncate(4);
    for game in &mut library.games[2..] {
        (game.last_played, game.playtime_minutes) = (0, 0);
    }
    library
}

#[test]
fn a_shorter_group_takes_focus_on_its_last_card() {
    let mut home = on_row(short_library());
    // Installed: four games and See all. Most played: two and See all.
    assert_eq!(after(&mut home, &[Down, Down, Right, Right, Right, Right]), card(0, 4));
    assert_eq!(after(&mut home, &[Right]), card(0, 4));
    assert_eq!(after(&mut home, &[Down]), card(1, 2));
    assert_eq!(after(&mut home, &[Up]), card(0, 2), "the place it has now, not the one it had before");
}

#[test]
fn with_no_groups_down_from_the_wide_tiles_does_nothing() {
    let mut library = screens::fake_library();
    for game in &mut library.games {
        (game.installed, game.playtime_minutes) = (false, 0);
    }
    let mut home = on_row(library);
    assert_eq!(after(&mut home, &[Down, Down]), Focus::Wide(0));
}

#[test]
fn b_on_a_group_goes_back_to_the_recent_rows_game_last_focused() {
    let mut home = core();
    assert_eq!(after(&mut home, &[Right, Right, Down, Down, Down, Right, B]), Focus::Recent(2));
    // The Play history tile is not a game: B goes back to the game's tile focused before it.
    assert_eq!(after(&mut home, &[Right; 5]), Focus::Recent(7));
    assert_eq!(after(&mut home, &[Down, Down, B]), Focus::Recent(6));
    // B is only the Groups' way back.
    assert_eq!(after(&mut home, &[Down, B]), Focus::Wide(3));
}

#[test]
fn a_on_a_groups_card_launches_its_game_and_on_see_all_or_play_history_nothing() {
    let mut home = core();
    // Most played's first card is the game played longest, Stardew Valley, tenth in the library.
    press(&mut home, &[Down, Down, Down]);
    assert_eq!(home.focused_game(), Some(9));
    press(&mut home, &[A]);
    assert_eq!(asked(&mut home), [Effect::Launch(9)]);
    press(&mut home, &[Right, Right, Right, Right, Right, Right, A]);
    assert_eq!(home.focused_game(), None);
    assert!(asked(&mut home).is_empty(), "See all's page is yet to come");
    press(&mut home, &[B, Right, Right, Right, Right, Right, Right, Right, A]);
    assert_eq!(home.home_focus(), Focus::Recent(7));
    assert!(asked(&mut home).is_empty(), "Play history's page is yet to come");
    press(&mut home, &[Down, A]);
    assert!(asked(&mut home).is_empty(), "nor have the wide tiles'");
}

#[test]
fn a_second_a_within_the_double_tap_does_not_launch_twice() {
    let mut home = core();
    press(&mut home, &[Right, A, A, A]);
    assert_eq!(asked(&mut home), [Effect::Launch(1)]);
    // Another tile is another choice, however soon.
    press(&mut home, &[Right, A]);
    assert_eq!(asked(&mut home), [Effect::Launch(2)]);
    // And so is the same tile once the double tap's 300 ms are over.
    std::thread::sleep(std::time::Duration::from_millis(350));
    press(&mut home, &[A]);
    assert_eq!(asked(&mut home), [Effect::Launch(2)]);
}

#[test]
fn a_game_ending_brings_focus_back_to_its_recent_tile_from_the_groups() {
    let mut home = core();
    // Installed's second card is Forza Horizon 5, second in the Recent row.
    press(&mut home, &[Down, Down, Right, A]);
    assert_eq!(asked(&mut home), [Effect::Launch(1)]);
    home.launch_started(1);
    home.game_ended();
    assert_eq!(home.home_focus(), Focus::Recent(1));
}

#[test]
fn a_launch_asks_the_shell_and_the_game_ending_brings_home_back_on_its_tile() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
    stage.press(&[Button::Right, Button::Right, Button::A]);
    assert_eq!(asked(&mut stage.dashboard), [Effect::Launch(2)]);
    stage.dashboard.launch_started(2);
    // While it starts, and behind it once it runs, Home takes no input.
    stage.press(&[Button::Left, Button::A]);
    stage.dashboard.game_booted();
    stage.press(&[Button::Left, Button::A]);
    assert!(stage.dashboard.take_effects().is_empty(), "not so much as a sound");
    // The Guide over the game: Menu on the running game quits it, and the Guide goes when the game has.
    stage.press(&[Button::Guide, Button::Down, Button::Down, Button::Menu]);
    assert_eq!(asked(&mut stage.dashboard), [Effect::QuitGame]);
    assert!(stage.dashboard.guide_open());
    stage.dashboard.game_ended();
    assert!(!stage.dashboard.guide_open());
    assert_eq!(stage.dashboard.playing(), None);
    assert_eq!(stage.dashboard.home_focus(), Focus::Recent(2));
}

// --- The Guide (docs/spec/guide.md). Where its focus goes and what a press asks for is tests/guide.rs. ---

use screens::guide::{PcBattery, Status};

/// The colour of the stand-in game the end-to-end tests run, and of the Bundle's pictures of the Guide over a game.
const GAME: ui::Color = ui::Color::rgb(0x102050);

/// A Dashboard with a game running, its Launch screen gone, and the Guide open over it.
fn guide_over_game(library: Library, game: usize) -> Stage {
    let mut stage = Stage::new(library, HD);
    stage.dashboard.launch_started(game);
    stage.dashboard.game_booted();
    stage.still();
    stage.press(&[Button::Guide]);
    stage
}

fn colour(image: &image::RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let pixel = image.get_pixel(x, y).0;
    [pixel[0], pixel[1], pixel[2]]
}

fn within(a: [u8; 3], b: [u8; 3], by: u8) -> bool {
    a.iter().zip(b).all(|(&a, b)| a.abs_diff(b) <= by)
}

/// The landing tab over Home as it opens: focus on Home, four games under the divider, the Utilities at the foot.
#[test]
fn guide() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Guide]);
    stage.check("guide");
}

/// A game's entry with focus: its ring over its cover, and the neighbours' covers under its glow.
#[test]
fn guide_recent_game_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Guide, Button::Down, Button::Down, Button::Down]);
    stage.check("guide-recent-game");
}

/// A Utility with focus, the Notifications badge beside it, and the Status area with everything it can show: the
/// PC's battery, the pad's with its percent, the clock.
#[test]
fn guide_utility_focused_with_a_badge_and_batteries() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.set_pad_connected(true);
    stage.dashboard.set_status(Status {
        pad_battery: Some(60),
        pc_battery: Some(PcBattery { percent: Some(57), charging: false }),
        notifications: 3,
        ..Status::default()
    });
    stage.press(&[Button::Guide]);
    stage.press(&[Button::Down; 6]);
    stage.press(&[Button::Right]);
    stage.check("guide-utility");
}

/// The Bundle's own picture of the Guide (guide-over-game's guide-open.jpg): over the stand-in game, which is none
/// of the library's, with nothing played yet and a pad on a cable.
#[test]
fn guide_over_a_game_with_no_games_played() {
    let mut library = screens::fake_library();
    library.games.iter_mut().for_each(|game| game.last_played = 0);
    let mut stage = Stage::new(library, HD);
    stage.dashboard.set_clock(Some("4:53 PM".into()));
    stage.dashboard.set_pad_connected(true);
    stage.press(&[Button::Guide]);
    let image = stage.still_over(GAME);
    stage.check_image("guide-no-games", image);
}

/// Over a running game, as the Overlay shows it: the game first in the list, "Running", with focus and so with the
/// Menu button's hint; the dim over the game's own colour.
#[test]
fn guide_over_a_game() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::Down, Button::Down]);
    let image = stage.still_over(GAME);
    // The colours the end-to-end smoke test looks for on the real screen (e2e/payloads/smoke/run.ps1).
    assert!(within(colour(&image, 270, 900), [21, 38, 17], 3), "the panel: {:?}", colour(&image, 270, 900));
    assert!(within(colour(&image, 604, 900), [21, 38, 17], 3), "the panel's edge: {:?}", colour(&image, 604, 900));
    assert!(within(colour(&image, 634, 900), [7, 14, 35], 3), "beside the panel: {:?}", colour(&image, 634, 900));
    assert!(within(colour(&image, 1230, 700), [7, 14, 35], 3), "the game dimmed: {:?}", colour(&image, 1230, 700));
    stage.check_image("guide-over-game", image);
}

/// A running game with no cover: the console's own tile for it, a pad on the accent.
#[test]
fn guide_over_a_game_with_no_cover() {
    let mut stage = guide_over_game(screens::fake_library(), 3);
    let image = stage.still_over(GAME);
    stage.check_image("guide-over-game-no-cover", image);
}

#[test]
fn guide_profile_and_system() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Guide, Button::RightBumper]);
    stage.check("guide-profile");
}

/// Profile & system over a game: Switch profile greyed out, here with focus, which it still takes.
#[test]
fn guide_profile_and_system_over_a_game() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::RightBumper, Button::Down]);
    let image = stage.still_over(GAME);
    stage.check_image("guide-profile-over-game", image);
}

#[test]
fn guide_power_focused() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Guide, Button::RightBumper, Button::Down, Button::Down, Button::Down]);
    stage.check("guide-profile-power");
}

/// Names too long for their line: a game's wraps to two lines and ends in an ellipsis, with "Running" under it on
/// the running game; the user's is one line.
#[test]
fn guide_long_names() {
    let mut library = library_with_art();
    library.user = Some("A Gamertag That Is Far Too Long For The Panel".into());
    library.games[0].name = "The Legend of the Seven Lanterns: Definitive Anniversary Edition of the Year".into();
    library.games[1].name = "Microsoft Flight Simulator 40th Anniversary Deluxe Edition".into();
    library.games[2].name = "Supercalifragilisticexpialidocious".into();
    let mut stage = guide_over_game(library, 0);
    let image = stage.still_over(GAME);
    stage.check_image("guide-long-names", image);
    stage.press(&[Button::RightBumper]);
    let image = stage.still_over(GAME);
    stage.check_image("guide-long-names-profile", image);
}

/// With Discord connected its tab stands between the two; its entries come with Discord.
#[test]
fn guide_discord_tab() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.set_status(Status { discord: true, ..Status::default() });
    stage.press(&[Button::Guide, Button::RightBumper]);
    stage.check("guide-discord");
}

#[test]
fn guide_720p() {
    let mut stage = Stage::on_row(library_with_art(), (1280, 720));
    stage.dashboard.set_pad_connected(true);
    stage.press(&[Button::Guide, Button::Down]);
    stage.check("guide-720p");
}

/// Wider than 16:9: the panel at the screen's left, the Status area at its right, the dim to both edges.
#[test]
fn guide_ultrawide() {
    let mut stage = Stage::on_row(library_with_art(), (2560, 1080));
    stage.dashboard.set_pad_connected(true);
    stage.press(&[Button::Guide]);
    stage.check("guide-ultrawide");
}

/// The tab's underline: there once the panel's content is in.
const UNDERLINE: (u32, u32) = (140, 135);
const GREEN: [u8; 3] = [44, 162, 67];
/// A point of the panel under every entry and over the Utilities, and its colour there.
const PANEL: (u32, u32) = (300, 900);
const PANEL_COLOUR: [u8; 3] = [21, 38, 17];
/// The fill of an entry with focus, over the panel's grey.
const FILL: [u8; 3] = [64, 64, 64];

fn is_panel(image: &image::RgbaImage, x: u32) -> bool {
    within(colour(image, x, PANEL.1), PANEL_COLOUR, 3)
}

/// The panel slides in over 120 ms, and what is on it comes 50 ms behind.
#[test]
fn the_guide_slides_in_and_its_content_follows_the_panel() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
    stage.frame_after(0.0);
    stage.press(&[Button::Guide]);
    let image = stage.frame_after(0.0);
    assert!(!is_panel(&image, PANEL.0), "the panel starts off the screen");
    // Halfway through its time the panel is seven eighths in: its right edge at x 526.
    let image = stage.frame_after(0.06);
    assert!(is_panel(&image, PANEL.0), "{:?}", colour(&image, PANEL.0, PANEL.1));
    assert!(is_panel(&image, 520) && !is_panel(&image, 532));
    assert!(!within(colour(&image, UNDERLINE.0, UNDERLINE.1), GREEN, 8), "its content has only just set off");
    let image = stage.frame_after(0.06);
    assert!(is_panel(&image, 608), "the panel is in after 120 ms");
    assert!(!within(colour(&image, UNDERLINE.0, UNDERLINE.1), GREEN, 8), "its content is still on its way");
    let image = stage.frame_after(0.06);
    assert!(within(colour(&image, UNDERLINE.0, UNDERLINE.1), GREEN, 8), "and in 50 ms after it");
}

/// It slides out over 150 ms, slowly at first, and is gone then.
#[test]
fn the_guide_slides_out_and_is_gone() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
    stage.press(&[Button::Guide]);
    stage.still();
    stage.press(&[Button::B]);
    // It sets off in the frame after the press. Two thirds through, it has gone under a third of its way: its
    // right edge is at x 406.
    stage.frame_after(0.0);
    let image = stage.frame_after(0.1);
    assert!(stage.dashboard.guide().showing());
    assert!(is_panel(&image, 400) && !is_panel(&image, 412));
    // Its content goes with it: Home's fill, x 87..605 at rest, ends at 398.
    assert!(within(colour(&image, 380, 235), FILL, 3), "{:?}", colour(&image, 380, 235));
    assert!(!within(colour(&image, 420, 235), FILL, 12), "{:?}", colour(&image, 420, 235));
    stage.frame_after(0.06);
    assert!(!stage.dashboard.guide().showing());
}

/// Under Reduce motion the Guide is there at once and gone at once, the dim with it.
#[test]
fn under_reduce_motion_the_guide_appears_and_goes_at_once() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
    stage.host.store.reduce_motion = true;
    let home = stage.frame_after(0.0);
    stage.press(&[Button::Guide]);
    let image = stage.frame_after(0.0);
    assert!(is_panel(&image, 608));
    assert!(within(colour(&image, UNDERLINE.0, UNDERLINE.1), GREEN, 8));
    // A new tab's content too: Profile & system's first entry has focus, and its fill is there.
    stage.press(&[Button::RightBumper]);
    let image = stage.frame_after(0.0);
    assert!(within(colour(&image, 430, 225), FILL, 3), "{:?}", colour(&image, 430, 225));
    stage.press(&[Button::B]);
    let image = stage.frame_after(0.0);
    assert!(!stage.dashboard.guide().showing());
    assert!(image == home, "Home as it was, with no dim left over it");
}

/// A new tab's content comes in from the side the bumper went toward, fading in over 120 ms; the tab strip does
/// not move.
#[test]
fn a_new_tabs_content_flies_in_from_the_side() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
    stage.press(&[Button::Guide]);
    stage.still();
    stage.press(&[Button::RightBumper]);
    let image = stage.frame_after(0.0);
    // The underline is under the second tab at once; the first entry's fill is yet to show.
    assert!(within(colour(&image, UNDERLINE.0 + 84, UNDERLINE.1), GREEN, 8));
    assert!(!within(colour(&image, 430, 225), FILL, 12), "{:?}", colour(&image, 430, 225));
    // Halfway it is still 6 to the right of its place: the profile's picture, x 105..165 at rest, starts at 111.
    const PICTURE: [u8; 3] = [242, 242, 242];
    let image = stage.frame_after(0.06);
    assert!(!within(colour(&image, 107, 260), PICTURE, 40), "{:?}", colour(&image, 107, 260));
    assert!(colour(&image, 135, 250)[0] > 160, "three quarters faded in: {:?}", colour(&image, 135, 250));
    let image = stage.frame_after(0.06);
    assert!(within(colour(&image, 430, 225), FILL, 3));
    assert!(within(colour(&image, 107, 260), PICTURE, 12), "{:?}", colour(&image, 107, 260));

    // Back with the left bumper: from the left, so Home's fill, x 87..605 at rest, ends 6 short on the way.
    stage.press(&[Button::LeftBumper]);
    stage.frame_after(0.0);
    let image = stage.frame_after(0.06);
    let short = colour(&image, 601, 235);
    let image = stage.frame_after(0.06);
    assert!(within(colour(&image, 430, 235), FILL, 3));
    assert!(
        !within(short, colour(&image, 601, 235), 12),
        "{short:?} on the way, {:?} at rest",
        colour(&image, 601, 235)
    );
}
