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
                shots::save_shared(&image, &dir.join(format!("{game}-{kind}.png")));
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

/// When the tests' libraries came into their database.
const ADDED: u64 = 1_780_000_000;

/// A library that has no filters yet (the fake one, as a test changed it) as the library database gives it once
/// `seed` has had its say, with the saved filters that are Home's Groups. One that has its filters is left alone.
fn from_database(library: Library, seed: impl FnOnce(&mut library::Database)) -> Library {
    if !library.filters.is_empty() {
        return library;
    }
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &library, ADDED).unwrap();
    seed(&mut database);
    database.snapshot().unwrap()
}

/// The library with the two Groups most of these tests were written on: Installed, by name, and Most played, the
/// longest played first. The other filters are there and not shown.
fn two_groups(library: Library) -> Library {
    from_database(library, |database| {
        for filter in database.filters().unwrap() {
            let shown = matches!(filter.key.as_deref(), Some("installed" | "most-played"));
            database.set_filter_shown(filter.id, shown).unwrap();
        }
    })
}

/// The library as a first run has it, but for two favourites and a Steam collection of five games: Favourites, the
/// collection, Most played and Recently added are the Groups.
fn library_with_a_collection() -> Library {
    from_database(library_with_art(), |database| {
        let games = database.snapshot().unwrap().games;
        for game in [&games[3], &games[11]] {
            database.set_favourite(game.id, true).unwrap();
        }
        let appids = [0, 2, 5, 8, 12].map(|i| games[i].source_id.clone()).to_vec();
        let collection =
            model::Collection { source_id: "uc-Vj9aG3fT6uXo".into(), name: "Couch co-op".into(), games: appids };
        let mut found = Library { user: Some("Player One".into()), games, ..Library::default() };
        found.collections = Some(vec![collection]);
        database.sync("steam", &found, ADDED).unwrap();
    })
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
        let mut dashboard = Dashboard::new(two_groups(library));
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
    /// the same on any machine. Home's window and the Overlay over it, as `still` draws them: the Guide is on the
    /// Overlay at Home too.
    fn step(&mut self, mut dt: f32) {
        let Self { gpu, host, target, dashboard, .. } = self;
        loop {
            let draw = |ui: &mut ui::Ui| dashboard.draw_all(ui);
            let index = host.draw(gpu, &target.view, target.size, dt, wgpu::Color::BLACK, draw);
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
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
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

/// The Groups a first run shows, from the saved filters: Favourites, a Steam collection after it, then Most played,
/// each in its own sort. The page is at rest on the first.
#[test]
fn home_groups_from_the_saved_filters() {
    let mut stage = Stage::on_row(library_with_a_collection(), HD);
    stage.press(&[Button::Down, Button::Down, Button::Right]);
    stage.check("home-groups-filters");
}

/// With no Group to show, Favourites shows empty: its header over a See all card alone.
#[test]
fn home_favourites_empty() {
    let library = from_database(library_with_art(), |database| {
        for filter in database.filters().unwrap() {
            database.set_filter_shown(filter.id, false).unwrap();
        }
    });
    let mut stage = Stage::on_row(library, HD);
    stage.press(&[Button::Down, Button::Down]);
    stage.check("home-favourites-empty");
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

/// A slow renderer gives a fade two faint frames and a long wait for the third, the game's launcher in plain sight
/// meanwhile: there the Launch screen is whole in its first frame.
#[test]
fn on_a_slow_renderer_the_launch_screen_comes_up_whole() {
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.dashboard.set_slow_renderer(true);
    stage.dashboard.launch_started(2);
    stage.frame();
    assert!(stage.dashboard.overlay_at_rest(), "whole in one frame");
    assert!(stage.dashboard.home_covered());
    // Booted: gone in one frame too.
    stage.dashboard.game_booted();
    stage.frame();
    assert!(!stage.dashboard.launch_screen_showing());
}

/// What the Overlay is wanted for through a launch, and when Home's window has nothing to show.
#[test]
fn the_overlay_shows_the_launch_screen_then_the_guide_over_the_game() {
    use screens::OverlayShows;
    let mut stage = Stage::new(screens::fake_library(), HD);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    // The Guide over Home is on the Overlay too, so there is one Guide: it takes the foreground from Home's window,
    // slides away passive, and Home is never covered. No game is held under it.
    stage.press(&[Button::Guide]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Guide);
    stage.still();
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Guide);
    assert!(!stage.dashboard.guide_over_game());
    assert!(!stage.dashboard.home_covered());
    stage.press(&[Button::B]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Passive);
    stage.still();
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);

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

/// The Guide at Home outlives the Overlay it was on: Home's window can show it, and does.
#[test]
fn an_overlay_lost_at_home_leaves_the_guide_in_homes_window() {
    use screens::OverlayShows;
    let mut stage = Stage::new(screens::fake_library(), HD);
    stage.press(&[Button::Guide, Button::Down]);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Guide);
    let on_the_overlay = stage.still();
    stage.dashboard.set_overlay(false);
    assert_eq!(stage.dashboard.overlay(), OverlayShows::Nothing);
    assert!(stage.dashboard.guide_open());
    // The same picture, now all of it Home's window's: the Overlay has nothing to show.
    assert_eq!(shots::difference(&stage.still(), &on_the_overlay).share, 0.0);
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

/// The hint bar on a Group's game card is in the screen's bottom right corner, 60 in and 52 up, 63 tall, a
/// near-black a little of the page shows through (E/HintBarControl.xaml:17-19): a white ring for Menu and "More
/// options", then the yellow ring for Y and "Search". It is not there on See all, and it does not move with the
/// page.
#[test]
fn the_hint_bar_stands_in_the_bottom_right_corner_on_a_groups_game_card() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Down, Button::Down]);
    let image = stage.still();
    let dark = |image: &image::RgbaImage, x: u32, y: u32| image.get_pixel(x, y).0[..3].iter().all(|&c| c < 45);
    let yellow = |x: u32, y: u32| {
        let [r, g, b, _] = image.get_pixel(x, y).0;
        r > 200 && (150..215).contains(&g) && b < 90
    };
    // The bar's back, just inside its right edge at 1860 and its top and foot at 965 and 1028.
    for (x, y) in [(1856, 996), (1700, 968), (1700, 1025), (1480, 996)] {
        assert!(dark(&image, x, y), "the bar's back at {x}, {y}: {:?}", image.get_pixel(x, y));
    }
    // Its hints, right to left from the padding: "Search", 12, the Y's ring 36 across, 26, "More options", 12,
    // Menu's ring. Selawik is as wide as Segoe UI, so the rings are where the Bundle's picture has them: Y's
    // from 1715 and Menu's from 1486, both on the bar's middle, 996.5.
    let search = ink_rows(&image, (1764, 1840), (965, 1028)).expect("Search");
    assert!((975..990).contains(&search.0) && (1000..1012).contains(&search.1), "Search's ink: {search:?}");
    let about = |x: u32, is: &dyn Fn(u32, u32) -> bool| (x - 2..=x + 2).any(|x| is(x, 996));
    assert!(about(1716, &yellow) && about(1750, &yellow), "the Y's ring is 36 across from 1715");
    assert!(!about(1705, &yellow), "the bar is not yellow beside the ring");
    let white = |x: u32, y: u32| ink(&image, x, y);
    assert!(about(1487, &white) && about(1521, &white), "Menu's ring is 36 across from 1486");
    assert!(ink_rows(&image, (1535, 1690), (965, 1028)).is_some(), "More options");

    // On the second Group the page has risen by a Group and the bar has not moved.
    stage.press(&[Button::Down]);
    let image = stage.still();
    assert!(dark(&image, 1856, 996) && dark(&image, 1480, 996), "the bar stays in its corner");
    // See all has no menu: no bar.
    stage.press(&[Button::Right; 6]);
    let image = stage.still();
    assert!(ink_rows(&image, (1764, 1840), (975, 1012)).is_none(), "no hint on See all");
}

/// A page is there at once in Home's place, top bar and all, and at once gone: Home is drawn as it was left, the
/// page at the Group's rest and nothing sliding there (docs/spec/search.md, 6).
#[test]
fn a_page_takes_homes_place_at_once_and_b_shows_home_at_the_rest_it_was_left_at() {
    // Search as it first draws over another Home, one not on the row and with the swirl still behind it.
    let points = [(120, 100), (960, 114), (1700, 114), (300, 700), (960, 950), (1800, 1000)];
    let elsewhere = {
        let mut stage = Stage::new(library_with_art(), HD);
        stage.press(&[Button::Y]);
        let image = stage.frame();
        points.map(|(x, y)| image.get_pixel(x, y).0)
    };
    let mut stage = Stage::on_row(library_with_art(), HD);
    let pale = |image: &image::RgbaImage| image.get_pixel(120, 100).0[..3].iter().all(|&c| c > 200);
    stage.run(1.0, TICK);
    assert!(pale(&stage.frame()), "the profile's picture is at the top left of Home");
    // The first frame after Y is the page alone: the same where the top bar and the rows were, whatever Home was.
    stage.press(&[Button::Y]);
    let image = stage.frame();
    assert!(!pale(&image), "the top bar hides while a page is up");
    for ((x, y), page) in points.into_iter().zip(elsewhere) {
        assert_eq!(image.get_pixel(x, y).0, page, "nothing of Home shows at {x}, {y}");
    }
    // And the first frame after B (Search's second: the first puts its keyboard away) is Home at rest, the top bar
    // back.
    stage.press(&[Button::B, Button::B]);
    assert!(pale(&stage.frame()), "Home is back in the next frame");

    // From a Group: Home comes back risen to the Group's rest in the first frame, its header 64 down.
    stage.press(&[Button::Down, Button::Down]);
    stage.run(1.0, TICK);
    let header = |image: &image::RgbaImage| ink_rows(image, (105, 300), (0, 110)).expect("the Group's header");
    let at_rest = header(&stage.frame());
    assert!((64..80).contains(&at_rest.0), "the header's ink starts at {}", at_rest.0);
    stage.press(&[Button::Y]);
    stage.run(0.2, TICK);
    stage.press(&[Button::B, Button::B]);
    let back = stage.frame();
    assert_eq!(header(&back), at_rest, "Home is at the Group's rest in the first frame back");
    assert!(!pale(&back), "and its top bar is still up off the screen");
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
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
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
    let mut dashboard = Dashboard::new(two_groups(library));
    press(&mut dashboard, &[Down]);
    dashboard
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// A twice at one moment: a double tap, which plays the game focus is on.
fn double_tap(dashboard: &mut Dashboard) {
    press(dashboard, &[A, A]);
}

/// A on a game's tile, and the double tap's time gone by with no second A: the game's hub opens.
fn a_alone(dashboard: &mut Dashboard) {
    press(dashboard, &[A]);
    dashboard.advance(screens::home::DOUBLE_TAP.as_secs_f32());
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
fn a_on_the_profile_opens_the_guide_and_on_notifications_does_nothing() {
    let mut home = core();
    // What the three round buttons open is tests/pages.rs.
    press(&mut home, &[Up, Right, Right, Right, Right, A]);
    assert_eq!(home.home_focus(), Focus::TopBar(TopButton::Notifications));
    assert!(!home.guide_open() && home.page().is_none() && asked(&mut home).is_empty());
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

/// docs/spec/filters.md, section 4.1.
#[test]
fn with_no_group_to_show_favourites_is_there_empty_and_down_from_the_wide_tiles_is_its_see_all() {
    // Nothing installed and nothing played: both shown filters are empty. And a library that is in no database
    // has no filters at all.
    let mut nothing = screens::fake_library();
    for game in &mut nothing.games {
        (game.installed, game.last_played, game.playtime_minutes) = (false, 0, 0);
    }
    let libraries = [two_groups(nothing), screens::fake_library(), Library::default()];
    for library in libraries {
        let mut home = Dashboard::new(library);
        press(&mut home, &[Down]);
        assert_eq!(after(&mut home, &[Down, Down]), card(0, 0));
        assert_eq!(home.focused_game(), None);
        assert_eq!(after(&mut home, &[Right, Down]), card(0, 0), "See all is its only card, and the only Group");
        // A on it is Full library, and B there is See all again.
        press(&mut home, &[A]);
        assert!(asked(&mut home).is_empty() && home.page() == Some("MyGames"));
        assert_eq!(after(&mut home, &[B, Up]), Focus::Wide(0));
    }
}

/// The games of the Group's cards, by their places in the library, as focus is walked along it from its start.
fn cards_games(home: &mut Dashboard, group: usize) -> Vec<usize> {
    let mut games = Vec::new();
    while let (Focus::Group { group: at, .. }, Some(game)) = (home.home_focus(), home.focused_game()) {
        assert_eq!(at, group);
        games.push(game);
        press(home, &[Right]);
    }
    games
}

#[test]
fn the_groups_are_the_shown_filters_that_have_a_game_in_the_users_order() {
    // As a first run has them: Favourites is shown and empty, so Most played is first and Recently added next.
    let library = from_database(screens::fake_library(), |_| {});
    let mut home = Dashboard::new(library);
    press(&mut home, &[Down, Down, Down]);
    assert_eq!(cards_games(&mut home, 0), [9, 3, 7, 1, 6, 0], "the longest played first");
    press(&mut home, &[Down]);
    assert_eq!(after(&mut home, &[Left; 6]), card(1, 0));
    assert_eq!(cards_games(&mut home, 1), [10, 5, 11, 8, 1, 4], "all added at once: by name");
    assert_eq!(after(&mut home, &[Down]), card(1, 6), "two Groups, and no third");

    // The user marks two favourites, shows Installed and moves Most played last: a new snapshot, and Home follows.
    let library = from_database(screens::fake_library(), |database| {
        let filters = database.filters().unwrap();
        let keyed = |key: &str| filters.iter().find(|f| f.key.as_deref() == Some(key)).unwrap().id;
        let games = database.snapshot().unwrap().games;
        database.set_favourite(games[12].id, true).unwrap();
        database.set_favourite(games[4].id, true).unwrap();
        database.set_filter_shown(keyed("installed"), true).unwrap();
        database.set_filter_shown(keyed("recently-added"), false).unwrap();
        database.move_filter(keyed("most-played"), 2).unwrap();
        let weeknights = database.add_filter("Never played").unwrap();
        let rule = model::Rule { played: Some(model::Played::Never), ..model::Rule::default() };
        database.set_filter(weeknights, "Never played", &rule, model::Sort::Name, true).unwrap();
        database.move_filter(weeknights, -9).unwrap();
    });
    home.set_library(library);
    press(&mut home, &[B, Down, Down]);
    assert_eq!(cards_games(&mut home, 0), [13, 12, 11, 10], "the user's own Group, first, names downward");
    press(&mut home, &[Down]);
    assert_eq!(after(&mut home, &[Left; 6]), card(1, 0));
    assert_eq!(cards_games(&mut home, 1), [4, 12], "Favourites: Hades II, Ori and the Will of the Wisps");
    press(&mut home, &[Down]);
    assert_eq!(after(&mut home, &[Left; 6]), card(2, 0));
    assert_eq!(cards_games(&mut home, 2), [10, 1, 4, 0, 6, 7], "Installed, by name");
    press(&mut home, &[Down]);
    assert_eq!(after(&mut home, &[Left; 6]), card(3, 0));
    assert_eq!(cards_games(&mut home, 3)[0], 9, "Most played, last now");
    assert_eq!(after(&mut home, &[Down]), card(3, 6));
}

/// The Hidden filter is a Group when the user shows it (docs/spec/filters.md, section 4): the hidden games' cards,
/// which are in no other list.
#[test]
fn the_hidden_group_shows_the_hidden_games_when_the_user_shows_it() {
    let library = from_database(screens::fake_library(), |database| {
        let games = database.snapshot().unwrap().games;
        database.set_hidden(games[9].id, true).unwrap();
        database.set_hidden(games[2].id, true).unwrap();
        for filter in database.filters().unwrap() {
            let shown = matches!(filter.key.as_deref(), Some("hidden" | "most-played"));
            database.set_filter_shown(filter.id, shown).unwrap();
        }
    });
    assert_eq!(library.games.len(), 12);
    let mut home = Dashboard::new(library);
    press(&mut home, &[Down, Down, Down]);
    // Stardew Valley is hidden, so Starfield is the most played of what is left: third in the library now.
    assert_eq!(home.focused_game(), Some(2));
    // The last Group: two cards, Sea of Thieves and Stardew Valley, and See all.
    assert_eq!(after(&mut home, &[Down, Right, Right, Right]), card(1, 2));
    // A hidden game has no place among the library's games: its card launches nothing until the Game hub opens on it.
    assert_eq!(after(&mut home, &[Left, A]), card(1, 1));
    assert_eq!(home.focused_game(), None);
    assert!(asked(&mut home).is_empty());
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
fn a_double_tap_on_a_groups_card_launches_its_game_and_a_on_see_all_or_play_history_none() {
    let mut home = core();
    // Most played's first card is the game played longest, Stardew Valley, tenth in the library.
    press(&mut home, &[Down, Down, Down]);
    assert_eq!(home.focused_game(), Some(9));
    double_tap(&mut home);
    assert_eq!(asked(&mut home), [Effect::Launch(9)]);
    // See all and the Play history tile open pages (tests/pages.rs) and launch nothing.
    press(&mut home, &[Right, Right, Right, Right, Right, Right, A]);
    assert_eq!(home.focused_game(), None);
    assert!(asked(&mut home).is_empty() && home.page().is_some());
    press(&mut home, &[B, B, Right, Right, Right, Right, Right, Right, Right, A]);
    assert_eq!(home.home_focus(), Focus::Recent(7));
    assert!(asked(&mut home).is_empty() && home.page().is_some());
}

#[test]
fn a_on_a_game_opens_its_hub_once_the_double_taps_time_is_up_and_a_second_a_within_it_plays() {
    let mut home = core();
    press(&mut home, &[Right]);
    // The Dashboard's clock, in ms from here: the test moves it on, and nothing waits.
    let mut clock = 0;
    let mut to = |home: &mut Dashboard, ms: u64| {
        home.advance((ms - clock) as f32 / 1000.0);
        clock = ms;
    };
    let left = |home: &Dashboard| home.timer().map(|left| (left * 1000.0).round() as u64);
    let a = Press { button: A, repeat: false };
    let button = |button: Button| Press { button, repeat: false };

    // One A: nothing yet, for as long as a second may still come.
    home.press(a);
    assert_eq!(left(&home), Some(300));
    to(&mut home, 299);
    assert_eq!(left(&home), Some(1));
    assert!(asked(&mut home).is_empty() && home.page().is_none());
    // None came: the game's hub, focus on Play. Nothing was launched.
    to(&mut home, 300);
    assert_eq!(left(&home), None);
    assert_eq!(home.focus_line(), "GameHub: 1001 Hub Play");
    assert!(asked(&mut home).is_empty());
    // B: Home again, focus on the tile, and nothing more waiting.
    to(&mut home, 400);
    home.press(button(B));
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(1)));
    to(&mut home, 1000);
    assert_eq!(left(&home), None);

    // A second A within the time plays the game, once, and no hub opens.
    to(&mut home, 2000);
    home.press(a);
    to(&mut home, 2299);
    home.press(a);
    assert_eq!(asked(&mut home), [Effect::Launch(1)]);
    assert_eq!(left(&home), None);
    to(&mut home, 5000);
    assert_eq!(home.page(), None);

    // One that comes after it finds the hub, which is up by then: it is A on Play.
    to(&mut home, 6000);
    home.press(a);
    to(&mut home, 6300);
    home.press(a);
    assert_eq!(home.focus_line(), "GameHub: 1001 Hub Play");
    assert_eq!(asked(&mut home), [Effect::Launch(1)]);
    to(&mut home, 6400);
    home.press(button(B));

    // A on another tile is another choice, however soon: the first tile's is forgotten.
    to(&mut home, 7000);
    home.press(a);
    to(&mut home, 7100);
    home.press(button(Right));
    to(&mut home, 7200);
    home.press(a);
    to(&mut home, 7300);
    assert_eq!(left(&home), Some(200));
    to(&mut home, 7500);
    assert_eq!(left(&home), None);
    assert_eq!(home.focus_line(), "GameHub: 1002 Hub Play");
    assert!(asked(&mut home).is_empty());
    to(&mut home, 7600);
    home.press(button(B));
    assert_eq!(home.home_focus(), Focus::Recent(2));

    // Focus moving on does not undo the choice: the hub of the tile that was chosen opens, and B is on the tile
    // focus went to.
    to(&mut home, 8000);
    home.press(a);
    to(&mut home, 8100);
    home.press(button(Left));
    to(&mut home, 8300);
    assert_eq!(home.focus_line(), "GameHub: 1002 Hub Play");
    to(&mut home, 8400);
    home.press(button(B));
    assert_eq!(home.home_focus(), Focus::Recent(1));

    // Menu does, and so does anything that takes the pad from Home: the Guide, a page.
    for (at, taken) in [Button::Menu, Button::Guide, Button::Y].into_iter().enumerate() {
        let ms = 9000 + 1000 * at as u64;
        to(&mut home, ms);
        home.press(a);
        to(&mut home, ms + 100);
        home.press(button(taken));
        to(&mut home, ms + 300);
        assert_eq!(left(&home), None, "{taken:?}");
        assert_ne!(home.page(), Some("GameHub"), "{taken:?}");
        home.close_guide();
        to(&mut home, ms + 400);
        home.press(button(B));
    }
    assert!(asked(&mut home).iter().all(|effect| *effect == Effect::GuideClosed));
}

#[test]
fn a_game_played_from_its_hub_brings_home_back_on_its_tile_when_it_ends() {
    let mut home = core();
    press(&mut home, &[Right, Right]);
    a_alone(&mut home);
    // The hub is over Home; A on Play asks the shell for the game, and the hub stays under its Launch screen.
    press(&mut home, &[A]);
    assert_eq!(asked(&mut home), [Effect::Launch(2)]);
    home.launch_started(2);
    assert_eq!(home.focus_line(), "GameHub: 1002 Hub Play");
    home.game_ended();
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(2)));
}

#[test]
fn a_hidden_games_card_opens_its_hub_and_a_double_tap_on_it_plays_nothing() {
    // The Hidden Group shown, with one game in it.
    let library = from_database(screens::fake_library(), |database| {
        let games = database.snapshot().unwrap().games;
        database.set_hidden(games[4].id, true).unwrap();
        for filter in database.filters().unwrap() {
            database.set_filter_shown(filter.id, filter.key.as_deref() == Some("hidden")).unwrap();
        }
    });
    let hidden = library.hidden[0].source_id.clone();
    let mut home = on_row(library);
    press(&mut home, &[Down, Down]);
    assert_eq!((home.home_focus(), home.focused_game()), (card(0, 0), None));
    double_tap(&mut home);
    assert!(asked(&mut home).is_empty());
    a_alone(&mut home);
    assert_eq!(home.focus_line(), format!("GameHub: {hidden} Hub Play"));
    press(&mut home, &[A, B]);
    assert!(asked(&mut home).is_empty());
    assert_eq!(home.home_focus(), card(0, 0));
}

#[test]
fn a_game_ending_brings_focus_back_to_its_recent_tile_from_the_groups() {
    let mut home = core();
    // Installed's second card is Forza Horizon 5, second in the Recent row.
    press(&mut home, &[Down, Down, Right]);
    double_tap(&mut home);
    assert_eq!(asked(&mut home), [Effect::Launch(1)]);
    home.launch_started(1);
    home.game_ended();
    assert_eq!(home.home_focus(), Focus::Recent(1));
}

#[test]
fn a_launch_asks_the_shell_and_the_game_ending_brings_home_back_on_its_tile() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
    stage.press(&[Button::Right, Button::Right]);
    double_tap(&mut stage.dashboard);
    assert_eq!(asked(&mut stage.dashboard), [Effect::Launch(2)]);
    stage.dashboard.launch_started(2);
    // While it starts, and behind it once it runs, Home takes no input.
    stage.press(&[Button::Left, Button::A]);
    stage.dashboard.game_booted();
    stage.press(&[Button::Left, Button::A]);
    assert!(stage.dashboard.take_effects().is_empty(), "not so much as a sound");
    // The Guide over the game: Quit in the running game's menu quits it, and the Guide goes half a second after
    // the game has.
    stage.press(&[Button::Guide, Button::Down, Button::Down, Button::Menu, Button::A]);
    assert_eq!(asked(&mut stage.dashboard), [Effect::QuitGame]);
    assert!(stage.dashboard.guide_open());
    stage.dashboard.game_ended();
    assert!(stage.dashboard.guide_open());
    stage.dashboard.advance(screens::CLOSE_AFTER_QUIT);
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
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
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

// The Guide's dialogs (docs/spec/guide.md, section 6), and the menu panel by itself as the Tile menu will use it
// (docs/spec/tile-menu.md).

use screens::menu_panel::{icon, MenuPanel, Row};

/// Profile & system with focus on Power, and Power opened from it.
fn power_from_its_entry(mut stage: Stage) -> Stage {
    stage.press(&[Button::RightBumper, Button::Down, Button::Down, Button::Down, Button::A]);
    stage
}

fn guide_at_home() -> Stage {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.press(&[Button::Guide]);
    stage
}

/// Power from its entry: the band over the Guide's panel, which is out of focus under the dialog's dim, with
/// focus on Close.
#[test]
fn guide_power() {
    let mut stage = power_from_its_entry(guide_at_home());
    let image = stage.still();
    // The band is opaque and as wide as the screen; over and under it the dim is over everything.
    assert_eq!(colour(&image, 4, 500), [45, 45, 45]);
    assert_eq!(colour(&image, 1916, 500), [45, 45, 45]);
    assert_ne!(colour(&image, 1916, 133), [45, 45, 45]);
    assert_eq!(colour(&image, 1916, 134), [45, 45, 45]);
    assert_eq!(colour(&image, 1000, 941), [45, 45, 45]);
    assert_ne!(colour(&image, 1000, 942), [45, 45, 45]);
    stage.check_image("guide-power", image);
}

/// Focus on a tile: Restart PC, the middle of the first row.
#[test]
fn guide_power_tile_focused() {
    let mut stage = power_from_its_entry(guide_at_home());
    stage.press(&[Button::Up, Button::Up, Button::Right]);
    stage.check("guide-power-restart");
}

/// Power by a hold of the Guide button at Home: no panel, no hints, no Status area; Home under the two dims.
#[test]
fn guide_power_held() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.pad_event(ui::PadEvent::GuideHeld);
    stage.check("guide-power-held");
}

/// Power by a hold over a game, as the Overlay shows it.
#[test]
fn guide_power_held_over_a_game() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.launch_started(1);
    stage.dashboard.game_booted();
    stage.still();
    stage.dashboard.pad_event(ui::PadEvent::GuideHeld);
    let image = stage.still_over(GAME);
    stage.check_image("guide-power-over-game", image);
}

#[test]
fn guide_power_720p() {
    let mut stage = Stage::on_row(library_with_art(), (1280, 720));
    stage.press(&[Button::Guide]);
    let mut stage = power_from_its_entry(stage);
    stage.press(&[Button::Up]);
    stage.check("guide-power-720p");
}

/// Wider than 16:9: the band to both edges, its contents where they are on the canvas.
#[test]
fn guide_power_ultrawide() {
    let mut stage = Stage::on_row(library_with_art(), (2560, 1080));
    stage.press(&[Button::Guide]);
    power_from_its_entry(stage).check("guide-power-ultrawide");
}

/// The running game's menu over the Guide: the menu panel in the middle of the screen, the panel out of focus
/// behind the dim, the hints still beside it.
#[test]
fn guide_running_game_menu() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::Down, Button::Down, Button::Menu]);
    let image = stage.still_over(GAME);
    // The menu panel: x 735..1185, its fill, and its one row's ring 8 outside it.
    assert_eq!(colour(&image, 737, 500), [43, 43, 43]);
    assert_eq!(colour(&image, 1183, 500), [43, 43, 43]);
    assert!(within(colour(&image, 728, 590), GREEN, 8), "{:?}", colour(&image, 728, 590));
    stage.check_image("guide-game-menu", image);
}

// --- Games from the Guide (docs/spec/guide.md, 6.4, 7.3, 7.4). What the presses ask for is tests/guide_games.rs.
// No picture of the Bundle shows a running game in the Guide: these are from its XAML and C#.

/// "Quit X and start Y?": A on a game played before, while another runs. The prompt's title wraps; its first
/// answer has focus.
#[test]
fn guide_quit_and_start_prompt() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::Down, Button::Down, Button::Down, Button::A]);
    let image = stage.still_over(GAME);
    // The points the end-to-end test looks at (e2e/payloads/guide-games/run.ps1): the menu panel beside its title,
    // and the game under the Guide's dim and the dialog's.
    assert_eq!(colour(&image, 745, 420), [43, 43, 43], "the menu panel");
    assert!(within(colour(&image, 1230, 900), [2, 4, 11], 3), "the game: {:?}", colour(&image, 1230, 900));
    stage.check_image("guide-quit-and-start", image);
}

/// "Force quit X?", ten seconds after Quit: the prompt with its message, focus on Force quit.
#[test]
fn guide_force_quit_prompt() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::Down, Button::Down, Button::Menu, Button::A]);
    stage.dashboard.advance(screens::QUIT_TIMEOUT);
    let image = stage.still_over(GAME);
    assert_eq!(colour(&image, 745, 420), [43, 43, 43], "the menu panel");
    stage.check_image("guide-force-quit", image);
}

/// The same with the Guide setting off, the game having been quit some other way: the prompt over the dim alone,
/// with no panel, no hints and no Status area.
#[test]
fn guide_force_quit_prompt_with_the_guide_off() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::Down, Button::Down, Button::Menu, Button::A, Button::B]);
    stage.still();
    stage.dashboard.set_guide_enabled(false);
    stage.dashboard.advance(screens::QUIT_TIMEOUT);
    let image = stage.still_over(GAME);
    // Where the panel would be there is the game, under the Guide's dim and the dialog's.
    assert!(within(colour(&image, 270, 900), [2, 4, 11], 3), "no panel: {:?}", colour(&image, 270, 900));
    stage.check_image("guide-force-quit-alone", image);
}

/// A game that ends by itself under the open Guide leaves the Guide open, over Home now, its list without the
/// game and focus where it was.
#[test]
fn guide_after_the_game_under_it_ended() {
    let mut stage = guide_over_game(library_with_art(), 1);
    stage.press(&[Button::Down, Button::Down]);
    stage.still();
    stage.dashboard.game_ended();
    assert!(stage.dashboard.guide_open());
    stage.check("guide-game-ended");
}

/// Home chosen in the Guide over a game: Home's window draws again, in front of the game, and takes the pad.
#[test]
fn home_from_the_guide_over_a_game_draws_home_again() {
    let mut stage = guide_over_game(library_with_art(), 1);
    assert!(stage.dashboard.home_covered());
    stage.press(&[Button::A]);
    assert!(!stage.dashboard.home_covered() && stage.dashboard.in_front_of_game());
    // Home as `home_with_art` has it, to the pixel.
    stage.press(&[Button::Down, Button::Right]);
    stage.check("home-art");
}

/// The panel behind a dialog is out of focus: an edge of it that is sharp with no dialog is soft with one, and
/// nothing beside the panel is touched.
#[test]
fn behind_a_dialog_the_panel_is_out_of_focus_and_what_is_beside_it_is_not() {
    let mut stage = guide_at_home();
    stage.press(&[Button::RightBumper, Button::Down, Button::Down, Button::Down]);
    let sharp = stage.still();
    stage.press(&[Button::A]);
    let soft = stage.still();
    let luma = |image: &image::RgbaImage, x: u32, y: u32| colour(image, x, y).iter().map(|&c| c as i32).sum::<i32>();
    // Down the panel's top edge, above the band (y 33), and across its right edge under the band (x 614): the
    // light changes in one step when sharp, and over the blur's reach either side when not.
    let rise = |image: &image::RgbaImage, down: bool, from: u32| {
        let at = |i: u32| if down { (300, i) } else { (i, 1000) };
        let lumas: Vec<i32> = (from..from + 28).map(|i| luma(image, at(i).0, at(i).1)).collect();
        let whole = (lumas[27] - lumas[0]).abs();
        let steepest = lumas.windows(2).map(|pair| (pair[1] - pair[0]).abs()).max().unwrap();
        (whole, steepest)
    };
    for (edge, down, from) in [("top", true, 19), ("right", false, 600)] {
        let ((whole, steepest), (soft_whole, soft_steepest)) = (rise(&sharp, down, from), rise(&soft, down, from));
        assert!(whole > 20 && steepest * 2 > whole, "the {edge} edge, sharp: {steepest} of {whole}");
        assert!(
            soft_whole > 6 && soft_steepest * 4 < soft_whole,
            "the {edge} edge, soft: {soft_steepest} of {soft_whole}"
        );
    }
    // What is beside the panel is under the dialog's dim and otherwise as it was: the words on Home's Achievements
    // tile, under the band, are as sharp as they were, their ink where it was.
    let ink = |image: &image::RgbaImage| {
        let lit = |x: u32, y: u32| luma(image, x, y) > luma(image, 1350, 950) + 30;
        (985..1170u32).flat_map(|x| (960..996u32).map(move |y| (x, y))).filter(|&(x, y)| lit(x, y)).count()
    };
    assert!(ink(&sharp) > 200, "the tile's title is drawn: {}", ink(&sharp));
    assert!(ink(&soft) * 3 > ink(&sharp) * 2, "and still sharp under the dim: {} of {}", ink(&soft), ink(&sharp));
}

/// Power's contents come in from the left after 40 ms, over 120 ms, fading in; the band and the dim are there at
/// once.
#[test]
fn powers_contents_come_in_from_the_left_behind_the_band() {
    const TILE: [u8; 3] = [59, 59, 59];
    const BAND: [u8; 3] = [45, 45, 45];
    let mut stage = guide_at_home();
    stage.still();
    let mut stage = power_from_its_entry(stage);
    let image = stage.frame_after(0.0);
    assert_eq!(colour(&image, 1900, 500), BAND, "the band is there in the first frame");
    // A point of the first tile, which is x 132..636, y 356..485 at rest.
    assert_eq!(colour(&image, 600, 380), BAND, "its contents are not");
    let image = stage.frame_after(0.03);
    assert_eq!(colour(&image, 600, 380), BAND, "nor 30 ms in");
    // 100 ms in, halfway through settling: 25 short of its place and half there.
    let image = stage.frame_after(0.07);
    let tile = colour(&image, 500, 380);
    assert!(tile[0] > BAND[0] + 3 && tile[0] < TILE[0] - 3, "half faded in: {tile:?}");
    assert_eq!(colour(&image, 625, 380), BAND, "and not yet as far right as it will be");
    let image = stage.frame_after(0.07);
    assert_eq!(colour(&image, 500, 380), TILE);
    assert_eq!(colour(&image, 625, 380), TILE);
    // Moving focus brings nothing in again.
    stage.press(&[Button::Up]);
    assert_eq!(colour(&stage.frame_after(0.0), 625, 380), TILE);
}

/// Under Reduce motion Power is whole at once.
#[test]
fn under_reduce_motion_power_is_there_at_once() {
    let mut stage = guide_at_home();
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
    stage.frame_after(0.0);
    let mut stage = power_from_its_entry(stage);
    assert_eq!(colour(&stage.frame_after(0.0), 625, 380), [59, 59, 59]);
}

/// A hold with the Guide open: the panel slides away under Power, and the dim stays.
#[test]
fn a_hold_slides_the_panel_away_under_power() {
    let mut stage = guide_at_home();
    stage.still();
    stage.dashboard.pad_event(ui::PadEvent::GuideHeld);
    stage.frame_after(0.0);
    stage.frame_after(0.2);
    let image = stage.frame_after(0.2);
    assert!(stage.dashboard.guide_open());
    // Under the band, where the panel was: no panel, and Home's wide tile under the two dims.
    assert!(!is_panel(&image, 300));
    let tile = colour(&image, 300, 1010);
    assert!(tile[2] > tile[1] + 8, "the Discord tile's blue, dimmed: {tile:?}");
    // Closing Power closes it all, and Home is as it was.
    stage.press(&[Button::B]);
    stage.frame_after(0.0);
    stage.frame_after(0.2);
    assert!(!stage.dashboard.guide().showing());
}

impl Stage {
    /// Home at rest with `over` drawn over it.
    fn still_with(&mut self, mut over: impl FnMut(&mut ui::Ui)) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| {
            dashboard.draw_all(ui);
            over(ui);
        });
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }
}

/// The Tile menu's dim, which the Tile menu will draw under its panel.
const TILE_MENU_DIM: ui::Color = ui::Color::argb(0x99000000);

fn tile_menu() -> MenuPanel<u8> {
    let rows = vec![
        Row::new(icon::PLAY, "Play game", 0),
        Row::new(icon::UNFAVOURITE, "Remove from Favourites", 1),
        Row::new(icon::MANAGE, "Manage game", 2),
        Row::new(icon::CUSTOMISE, "Customise Home", 3).after_a_gap(),
        Row::new(icon::MORE_OPTIONS, "More options", 4),
    ];
    MenuPanel::menu("Halo Infinite", rows)
}

/// A menu of five rows as the Bundle's picture of the Tile menu has it (23-tile-menu-installed.jpg): the panel at
/// x 735..1185, y 300..780, the first row's ring from y 399, the gap before Customise Home.
#[test]
fn menu_panel_as_the_tile_menu() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    let menu = tile_menu();
    let image = stage.still_with(|ui| {
        ui.layer();
        ui.fill(ui.screen(), TILE_MENU_DIM, 0.0);
        let at = menu.centred(ui);
        assert!((menu.height(ui) - 480.6).abs() < 0.01, "{}", menu.height(ui));
        menu.draw(ui, at);
    });
    const PANEL: [u8; 3] = [43, 43, 43];
    for (x, y) in [(735, 310), (1184, 310), (960, 300), (960, 779), (960, 640)] {
        assert_eq!(colour(&image, x, y), PANEL, "the panel at {x}, {y}");
    }
    for (x, y) in [(734, 500), (1185, 500), (960, 299), (960, 781)] {
        assert_ne!(colour(&image, x, y), PANEL, "outside the panel at {x}, {y}");
    }
    // The focused row: its ring's strokes at x 727..730 and 1189..1192, y 399..402 and 467..470; its fill between.
    for (x, y) in [(728, 435), (1190, 435), (960, 400), (960, 468)] {
        assert!(within(colour(&image, x, y), GREEN, 6), "the ring at {x}, {y}: {:?}", colour(&image, x, y));
    }
    assert_eq!(colour(&image, 1100, 435), FILL);
    stage.check_image("menu-panel", image);
}

/// A prompt in the flyout's place over its menu, which is behind it at 0.45 as one picture
/// (26-tile-menu-remove-prompt.jpg).
#[test]
fn menu_panel_prompt_over_its_menu() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    let mut menu = tile_menu();
    menu.set_focus(4);
    let rows = vec![Row::new(icon::REMOVE_GAME, "Remove", true), Row::new(icon::CANCEL, "Cancel", false)];
    let prompt = MenuPanel::prompt("Remove Halo Infinite?", None, rows);
    let image = stage.still_with(|ui| {
        ui.layer();
        ui.fill(ui.screen(), TILE_MENU_DIM, 0.0);
        let at = menu.centred(ui);
        menu.draw_faded(ui, at, 0.45);
        prompt.draw(ui, (at.0 + 77.0, at.1));
    });
    assert_eq!(colour(&image, 820, 310), [43, 43, 43], "the prompt, opaque, from x 812");
    assert_ne!(colour(&image, 800, 310), [43, 43, 43], "the menu behind it, faded");
    stage.check_image("menu-panel-prompt", image);
}

/// A prompt whose question wraps, with a message under it: the Guide's "Force quit X?" (#40).
#[test]
fn menu_panel_prompt_with_a_message() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    let rows = vec![Row::new(icon::QUIT, "Force quit", true), Row::new(icon::CANCEL, "Cancel", false)];
    let message = Some("It hasn't closed. Anything unsaved will be lost.");
    let mut prompt = MenuPanel::prompt("Force quit Microsoft Flight Simulator?", message, rows);
    prompt.set_focus(1);
    let image = stage.still_with(|ui| {
        ui.layer();
        ui.fill(ui.screen(), ui::Color::argb(0xB3000000), 0.0);
        let at = prompt.centred(ui);
        prompt.draw(ui, at);
    });
    stage.check_image("menu-panel-prompt-message", image);
}

// --- The Console keyboard and text entry (docs/spec/search.md, sections 7 to 10) ---

use screens::keyboard::Typed;
use screens::pages::Open;

/// Text entry over the stand-in page that opens it (`screens::keyboard::stand_in`), on its Name box: "Filter by
/// name" over an empty name, focus on Q.
fn text_entry(size: (u32, u32)) -> Stage {
    let mut stage = Stage::new(library_with_art(), size);
    stage.dashboard.open_page(Open::KeyboardStandIn);
    stage.press(&[A]);
    stage
}

/// "Ha" typed by walking, as the Bundle's picture of text entry has it: H, then back along the row to A.
const HA: [Button; 13] = [Down, Right, Right, Right, Right, Right, A, Left, Left, Left, Left, Left, A];

/// The keys' greys, and the field's.
const KEY: [u8; 3] = [0x2F, 0x32, 0x38];
const SIDE_KEY: [u8; 3] = [0x3A, 0x3D, 0x44];
const FOCUSED_KEY: [u8; 3] = [0x55, 0x59, 0x63];
const HELD_KEY: [u8; 3] = [0x7A, 0x7E, 0x88];
const FIELD: [u8; 3] = [0x2E, 0x31, 0x36];

/// The letters with Shift on, as the keyboard opens over nothing: the Bundle's picture 15, with the Shift key lit.
#[test]
fn keyboard_letters_with_shift_on() {
    text_entry(HD).check("keyboard-shift");
}

/// "Ha" typed, focus on A, the letters in lower case: the Bundle's picture 13.
#[test]
fn keyboard_letters_after_typing() {
    let mut stage = text_entry(HD);
    stage.press(&HA);
    stage.check("keyboard-letters");
}

/// The symbols, focus on the symbols key, which reads "abc": the Bundle's picture 17.
#[test]
fn keyboard_symbols() {
    let mut stage = text_entry(HD);
    stage.press(&HA);
    stage.press(&[Up, Up, Left, A]);
    assert_eq!(stage.dashboard.focus_line(), "TextEntry: Symbols; \"Ha|\" lower symbols");
    stage.check("keyboard-symbols");
}

#[test]
fn text_entry_of_a_secret() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.open_page(Open::KeyboardStandIn);
    stage.press(&[Down, Down, A, A, A, A, A, Button::LeftBumper]);
    stage.check("text-entry-secret");
}

#[test]
fn text_entry_16_10() {
    let mut stage = text_entry((1920, 1200));
    stage.press(&HA);
    stage.check("text-entry-16-10");
}

#[test]
fn text_entry_720p() {
    let mut stage = text_entry((1280, 720));
    stage.press(&HA);
    stage.check("text-entry-720p");
}

/// The keys are opaque blocks 4 apart in the Bundle's four greys: the letters', the side keys' a shade lighter,
/// the focused key's under the one ring, 3 thick and touching the key, and the lightest while a pad button holds a
/// key down (E/ConsoleKeyboardControl.xaml:19-63, 233). Shift is lit while it is on (#54).
#[test]
fn the_keys_are_the_bundles_greys_and_the_ring_hugs_the_focused_key() {
    let mut stage = text_entry(HD);
    let image = stage.still();
    // R's face is x 822..888, y 764..812, clear of the glow round Q; the caret's left key x 498..608, y 764..864; backspace x 1312..1422,
    // y 712..760. Each is sampled in a corner, clear of what it shows.
    assert_eq!(colour(&image, 826, 768), KEY, "a letter key");
    assert_eq!(colour(&image, 616, 924), KEY, "the space bar");
    assert_eq!(colour(&image, 502, 768), SIDE_KEY, "a side key");
    assert_eq!(colour(&image, 1316, 716), SIDE_KEY, "backspace");
    // Between two keys the page shows, dimmed: there is no plate behind the keys.
    assert!(colour(&image, 750, 790).iter().all(|&c| c < 0x20), "between keys: {:?}", colour(&image, 750, 790));
    // Q has focus: its face x 612..678, y 764..812, lighter; the ring's 3 lie just outside it, and 5 outside
    // there is no ring.
    assert!(within(colour(&image, 645, 772), FOCUSED_KEY, 12), "the focused key: {:?}", colour(&image, 645, 772));
    for (x, y) in [(610, 788), (680, 788), (645, 762), (645, 814)] {
        assert!(
            within(colour(&image, x, y), [0x2C, 0xA2, 0x43], 24),
            "the ring at {x}, {y}: {:?}",
            colour(&image, x, y)
        );
    }
    assert!(!within(colour(&image, 606, 788), [0x2C, 0xA2, 0x43], 60), "the ring is 3 thick");
    assert!(within(colour(&image, 645, 766), FOCUSED_KEY, 40), "and touches the key: {:?}", colour(&image, 645, 766));
    // Shift is on as the keyboard opens over nothing, and its key is lit; typing turns it off.
    assert_eq!(colour(&image, 502, 872), FOCUSED_KEY, "Shift while it is on");
    stage.press(&[A]);
    let image = stage.still();
    assert_eq!(colour(&image, 502, 872), SIDE_KEY, "Shift while it is off");

    // X held down on the pad lights backspace until it is let go, wherever focus is; A lights the focused key.
    let down = |button| ui::PadEvent::Press(Press { button, repeat: false });
    stage.dashboard.pad_event(down(Button::X));
    stage.dashboard.pad_event(down(A));
    let image = stage.still();
    assert_eq!(colour(&image, 1316, 716), HELD_KEY, "backspace while X is down");
    assert!(within(colour(&image, 645, 772), HELD_KEY, 12), "Q while A is down: {:?}", colour(&image, 645, 772));
    stage.dashboard.pad_event(ui::PadEvent::Release(Button::X));
    stage.dashboard.pad_event(ui::PadEvent::Release(A));
    let image = stage.still();
    assert_eq!(colour(&image, 1316, 716), SIDE_KEY, "backspace once X is up");
    assert!(within(colour(&image, 645, 772), FOCUSED_KEY, 12));
}

/// Where the caret is in text entry's field: the rightmost white in the line just over the letters' tops, which
/// only the caret, 30 tall, reaches. The field's text area is x 524..1396, its middle y 647.
fn caret_x(image: &image::RgbaImage) -> Option<u32> {
    (500..1420).rev().find(|&x| colour(image, x, 634).iter().all(|&c| c > 200))
}

/// The page dims, lighter above and darker at the foot; the field is Search's pill, the keyboard's width, with
/// the caret after the text; and a space typed moves the caret as far as any character
/// (E/TextEntryControl.xaml:51-75).
#[test]
fn text_entry_dims_the_page_and_the_caret_follows_the_text() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.open_page(Open::KeyboardStandIn);
    let page = stage.still();
    stage.press(&[A]);
    let image = stage.still();
    // The page's fill, #1F1F1F, under 35% black at the top and 78% from y 670 down.
    assert_eq!(colour(&page, 710, 60), [0x1F; 3]);
    assert!(within(colour(&image, 710, 60), [20; 3], 1), "the dim at the top: {:?}", colour(&image, 710, 60));
    assert!(within(colour(&image, 710, 430), [20; 3], 1), "at four tenths: {:?}", colour(&image, 710, 430));
    assert!(within(colour(&image, 300, 700), [7; 3], 1), "from 62 hundredths: {:?}", colour(&image, 300, 700));
    assert!(within(colour(&image, 300, 1070), [7; 3], 1), "at the foot: {:?}", colour(&image, 300, 1070));
    let between = colour(&image, 300, 550)[0];
    assert!((8..20).contains(&between), "darkening between the two: {between}");
    // The field, x 496..1424, y 620..674, a pill: its ends are round.
    assert_eq!(colour(&image, 960, 624), FIELD);
    assert_eq!(colour(&image, 1400, 647), FIELD);
    assert_ne!(colour(&image, 498, 622), FIELD, "the pill's corner is cut");
    // Empty, the caret stands at the text's start: 1 in from x 524, 2 wide.
    assert_eq!(caret_x(&image), Some(526));
    // The caption's ink is over the field, from the text's left edge.
    let caption = ink_rows(&image, (524, 760), (560, 612)).expect("the caption");
    assert!((570..585).contains(&caption.0) && (596..610).contains(&caption.1), "the caption's ink: {caption:?}");
    assert!(ink_rows(&image, (500, 522), (560, 612)).is_none(), "the caption starts at the field's text");

    stage.dashboard.typed(Typed::Text("a"));
    let after_a = caret_x(&stage.still()).expect("the caret");
    stage.dashboard.typed(Typed::Text(" "));
    let after_space = caret_x(&stage.still()).expect("the caret");
    stage.dashboard.typed(Typed::Text("a"));
    let after_both = caret_x(&stage.still()).expect("the caret");
    assert!(after_a > 530 && after_space >= after_a + 5, "a space moves the caret: {after_a}, {after_space}");
    assert_eq!(after_both - after_space, after_a - 526, "and the next character is as far on as the first");
}

/// A text longer than the field scrolls, the caret kept in view at the field's right edge, and nothing of it is
/// drawn outside the text's area (#54; the Bundle cuts it and loses the caret).
#[test]
fn a_text_longer_than_the_field_scrolls_to_keep_the_caret_in_view() {
    let mut stage = text_entry(HD);
    stage.dashboard.typed(Typed::Text(&"Wide text ".repeat(12)));
    let image = stage.still();
    assert_eq!(caret_x(&image), Some(1394), "the caret stands at the area's right edge");
    assert_eq!(colour(&image, 1400, 647), FIELD, "nothing is drawn past the text's area");
    assert_eq!(colour(&image, 520, 647), FIELD);
    assert!(ink_rows(&image, (524, 1390), (630, 664)).is_some(), "the text's end shows");
    // The caret back at the start: the text is at rest again, its start showing.
    stage.press(&[Button::LeftBumper; 120]);
    let image = stage.still();
    assert_eq!(caret_x(&image), Some(526));
    stage.check("text-entry-long");
}

/// A secret is a dot for each character, 18 apart, and the caret among them where the cursor is.
#[test]
fn a_secret_is_drawn_as_dots_with_the_caret_among_them() {
    let mut stage = Stage::new(library_with_art(), HD);
    stage.dashboard.open_page(Open::KeyboardStandIn);
    stage.press(&[Down, Down, A, A, A, A]);
    let image = stage.still();
    let white = |image: &image::RgbaImage, x: u32| colour(image, x, 647).iter().all(|&c| c > 200);
    assert!(white(&image, 533) && white(&image, 551) && white(&image, 569), "three dots");
    assert!(!white(&image, 542) && !white(&image, 587), "and nothing between or after them");
    assert_eq!(caret_x(&image), Some(580), "the caret after the third");
    // The caret between the second and third: the third dot stands past it.
    stage.press(&[Button::LeftBumper]);
    let image = stage.still();
    assert_eq!(caret_x(&image), Some(562));
    assert!(white(&image, 533) && white(&image, 551) && white(&image, 573));
}

/// The system's sound in the Bundle's picture of the Audio Flyout (54-guide-audio-flyout.jpg): one output and one
/// input, their names too long for the box, volume 67. A second output for the list.
fn bundle_audio() -> model::Audio {
    use model::AudioDevice;
    model::Audio {
        outputs: vec![
            AudioDevice::new("out-1", "Speakers (2- High Definition Audio Device)"),
            AudioDevice::new("out-2", "TV (HD Audio Driver for Display Audio)"),
        ],
        output: Some(0),
        inputs: vec![AudioDevice::new("in-1", "Microphone (2- High Definition Audio Device)")],
        input: Some(0),
        volume: 67,
        muted: false,
    }
}

/// The Guide at Home with focus on a Utility: `place` along the row.
fn on_utility(place: usize) -> Stage {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.set_pad_connected(true);
    stage.dashboard.set_audio(bundle_audio());
    stage.press(&[Button::Guide]);
    stage.press(&[Button::Down; 6]);
    stage.press(&vec![Button::Right; place]);
    stage
}

/// The Bundle's 53-guide-notifications-flyout.jpg: the Flyout over the dimmed panel, with nothing in it; the hints
/// beside the panel hidden.
#[test]
fn guide_notifications_flyout_with_none() {
    let mut stage = on_utility(0);
    stage.press(&[Button::A]);
    stage.check("guide-notifications-empty");
}

/// Notifications, newest first: a card as tall as its text, at least 87 and three lines at most.
#[test]
fn guide_notifications_flyout() {
    let mut stage = on_utility(0);
    stage.dashboard.record("Saltwind Rally is ready to play");
    stage.dashboard.record(
        "A long one, which says a good deal more than fits on two lines of the card and so is cut short at the end \
         of its third line, where an ellipsis says that there was more of it to read",
    );
    stage.dashboard.record("Controller connected");
    stage.press(&[Button::A, Button::Down]);
    stage.check("guide-notifications");
}

/// The Bundle's 54-guide-audio-flyout.jpg: Output with focus, Input, Volume at 67, Mute off, and under a divider
/// the line about music.
#[test]
fn guide_audio_flyout() {
    let mut stage = on_utility(1);
    stage.press(&[Button::A]);
    let image = stage.still();
    // The Flyout's flat grey at its foot, to the screen's last row, and no further right than x 691.
    assert_eq!(image.get_pixel(400, 1079).0[..3], [0x1F, 0x1F, 0x1F]);
    assert_ne!(image.get_pixel(700, 1079).0[..3], [0x1F, 0x1F, 0x1F], "the Flyout ends at x 691");
    stage.check_image("guide-audio", image);
}

/// Output's list open: the dropdown ringed in the accent, its chevron up, and the list over the entries below, the
/// default device ticked and focused.
#[test]
fn guide_audio_output_list() {
    let mut stage = on_utility(1);
    stage.press(&[Button::A, Button::A]);
    stage.check("guide-audio-output-list");
}

/// Mute on, with focus: the switch filled in the accent, its knob at the right.
#[test]
fn guide_audio_muted() {
    let mut stage = on_utility(1);
    stage.dashboard.set_audio(model::Audio { muted: true, volume: 0, ..bundle_audio() });
    stage.press(&[Button::A, Button::Down, Button::Down, Button::Down]);
    stage.check("guide-audio-muted");
}

/// With no devices at all: "None" in both dropdowns, the volume at 0.
#[test]
fn guide_audio_with_no_devices() {
    let mut stage = on_utility(1);
    stage.dashboard.set_audio(model::Audio::default());
    stage.press(&[Button::A]);
    stage.check("guide-audio-none");
}

/// A Flyout comes in from 48 to the left, fading in, over 120 ms; closing it is at once.
#[test]
fn a_flyout_slides_in_from_the_left() {
    let mut stage = on_utility(1);
    stage.run(1.0, 0.05);
    let lit = |image: &image::RgbaImage, x: u32| image.get_pixel(x, 900).0[..3] == [0x1F, 0x1F, 0x1F];
    stage.press(&[Button::A]);
    let early = stage.frame_after(0.03);
    assert!(!lit(&early, 680), "its right edge has not arrived, and it is not yet opaque");
    let settled = stage.frame_after(0.3);
    assert!(lit(&settled, 680) && lit(&settled, 160) && !lit(&settled, 695));
    stage.press(&[Button::B]);
    let closed = stage.frame_after(0.01);
    assert!(!lit(&closed, 680) && !lit(&closed, 400));
}

// --- The Game hub (docs/spec/game-hub.md). Where its focus goes is the page's own tests', in src/game_hub. ---

/// The library with art, and with what Steam's files tell of three of its games: the first, which has all its art,
/// everything; the fourth, which has a cover alone, nothing; the sixth, which has no art and is not installed, who
/// made it and published it.
fn library_for_the_hub() -> Library {
    let mut library = library_with_art();
    for game in &mut library.games {
        // Noon in Greenwich: the same day wherever the test runs.
        game.last_played = game.last_played / 86_400 * 86_400 + if game.last_played > 0 { 43_200 } else { 0 };
    }
    let told = &mut library.games[0];
    told.publishers = vec!["Harbour & Hull".into()];
    told.developers = vec!["Tin Lantern".into(), "Second Kettle".into()];
    told.released = 1_698_364_800;
    told.size_on_disk = 50_067 * (1 << 20);
    (told.critic_score, told.community_score) = (Some(87), Some(91));
    let uninstalled = &mut library.games[5];
    uninstalled.publishers = vec!["Ninth Bell Games".into(), "Oddfellow Interactive".into()];
    uninstalled.developers = vec!["Mossback Works".into()];
    uninstalled.released = 1_552_348_800;
    // Not installed, it takes no room, whatever the number says.
    uninstalled.size_on_disk = 1 << 30;
    library
}

impl Stage {
    /// Home with focus on the Recent row's tile `tile`, at rest as the player sees it by then; A on the tile; and
    /// the double tap's time gone by with no second A. The game's hub is up, over Home as it was.
    fn hub(tile: usize) -> Self {
        let mut stage = Stage::on_row(library_for_the_hub(), HD);
        stage.press(&vec![Button::Right; tile]);
        stage.still();
        a_alone(&mut stage.dashboard);
        assert_eq!(stage.dashboard.page(), Some("GameHub"));
        stage
    }
}

/// The Bundle's `19-game-hub-play-focused.jpg`: a game with background art, which is behind the band, blurred;
/// Home in the strips above and below it; focus on Play.
#[test]
fn game_hub() {
    let mut stage = Stage::hub(0);
    assert_eq!(stage.dashboard.focus_line(), "GameHub: 1000 Hub Play");
    assert_eq!(stage.dashboard.hints(), screens::hint_bar::MORE_OPTIONS);
    stage.check("game-hub");
}

/// The Bundle's `20-game-hub-manage-focused.jpg`: Manage says its name over itself.
#[test]
fn game_hub_manage() {
    let mut stage = Stage::hub(0);
    stage.press(&[Button::Right]);
    stage.check("game-hub-manage");
}

/// The Bundle's `22-game-hub-details.jpg`: the other page, its wide card showing the game's art.
#[test]
fn game_hub_details() {
    let mut stage = Stage::hub(0);
    stage.press(&[Button::Right, Button::Right, Button::A]);
    assert_eq!(stage.dashboard.focus_line(), "GameHub: 1000 Details Toggle");
    stage.check("game-hub-details");
}

/// A game with a cover and no background art: the cover behind the band, blurred more and overhanging; no Info
/// card, as the library knows nothing to put in it. Focus on a card: the ring, and nothing else changes.
#[test]
fn game_hub_cover_only() {
    let mut stage = Stage::hub(3);
    stage.press(&[Button::Right, Button::Up]);
    assert_eq!(stage.dashboard.focus_line(), "GameHub: 1003 Hub Stats");
    stage.check("game-hub-cover-only");
}

/// A game with no art at all, not installed: the theme's green behind the band, the game's name where its cover
/// would be, and Install with the store's name under it.
#[test]
fn game_hub_no_art() {
    let mut stage = Stage::hub(5);
    assert_eq!(stage.dashboard.focus_line(), "GameHub: 1005 Hub Install");
    stage.check("game-hub-no-art");
}

/// The strips show the screen the hub was opened over as it was then, and keep showing it: Home is not drawn
/// again under the hub, whatever becomes of it. B, and the next hub has a picture of its own.
#[test]
fn the_hubs_strips_show_home_as_it_was_when_the_hub_opened() {
    let mut stage = Stage::hub(0);
    let first = stage.still();
    // Points in the strip above the band and in the one below it, clear of the hint bar.
    let strips = [(200, 60), (960, 60), (1700, 60), (300, 1030), (1100, 1030)];
    let in_strips = |image: &image::RgbaImage| strips.map(|(x, y)| colour(image, x, y));
    // Home's background was the game's art, which is in the strip above. In the band it is blurred.
    assert_ne!(colour(&first, 960, 60), colour(&first, 960, 110));

    // The game loses its background art under the hub: the band has its cover now, and the strips what they had.
    let mut library = stage.dashboard.library().clone();
    library.games[0].hero = None;
    stage.dashboard.set_library(library);
    let later = stage.still();
    assert!(later != first, "the band's art is the game's as it is now");
    assert_eq!(in_strips(&later), in_strips(&first), "the picture of the screen behind is kept as it was taken");

    // B, the next game's tile, and its hub: Home's background is that game's by now, and so is the strip.
    stage.press(&[Button::B, Button::Right]);
    assert_eq!(stage.dashboard.home_focus(), Focus::Recent(1));
    // Long enough on the tile for the background to follow focus there.
    stage.run(2.5, 0.1);
    a_alone(&mut stage.dashboard);
    let next = stage.still();
    assert_ne!(colour(&next, 960, 60), colour(&first, 960, 60), "the next hub has a picture of its own");
}
