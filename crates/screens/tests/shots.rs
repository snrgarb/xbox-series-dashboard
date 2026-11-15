//! The screens drawn offscreen and compared with the committed reference images (crates/shots).
//! `SHOTS=bless cargo test -p screens` accepts what they draw now. The fonts are tools/test-fonts.sh's.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

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
fn guide() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.open_guide();
    stage.press(&[Button::Down]);
    stage.check("guide");
}

#[test]
fn launch_screen() {
    let mut stage = Stage::on_row(library_with_art(), HD);
    stage.dashboard.launch_started(0);
    stage.check("launch-screen");
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
        assert!(!home.guide_open() && home.take_effects().is_empty(), "{:?}", home.home_focus());
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
    assert_eq!(home.take_effects(), [Effect::Launch(9)]);
    press(&mut home, &[Right, Right, Right, Right, Right, Right, A]);
    assert_eq!(home.focused_game(), None);
    assert!(home.take_effects().is_empty(), "See all's page is yet to come");
    press(&mut home, &[B, Right, Right, Right, Right, Right, Right, Right, A]);
    assert_eq!(home.home_focus(), Focus::Recent(7));
    assert!(home.take_effects().is_empty(), "Play history's page is yet to come");
    press(&mut home, &[Down, A]);
    assert!(home.take_effects().is_empty(), "nor have the wide tiles'");
}

#[test]
fn a_second_a_within_the_double_tap_does_not_launch_twice() {
    let mut home = core();
    press(&mut home, &[Right, A, A, A]);
    assert_eq!(home.take_effects(), [Effect::Launch(1)]);
    // Another tile is another choice, however soon.
    press(&mut home, &[Right, A]);
    assert_eq!(home.take_effects(), [Effect::Launch(2)]);
    // And so is the same tile once the double tap's 300 ms are over.
    std::thread::sleep(std::time::Duration::from_millis(350));
    press(&mut home, &[A]);
    assert_eq!(home.take_effects(), [Effect::Launch(2)]);
}

#[test]
fn a_game_ending_brings_focus_back_to_its_recent_tile_from_the_groups() {
    let mut home = core();
    // Installed's second card is Forza Horizon 5, second in the Recent row.
    press(&mut home, &[Down, Down, Right, A]);
    assert_eq!(home.take_effects(), [Effect::Launch(1)]);
    home.launch_started(1);
    home.game_ended();
    assert_eq!(home.home_focus(), Focus::Recent(1));
}

#[test]
fn a_launch_asks_the_shell_and_the_game_ending_brings_home_back_on_its_tile() {
    let mut stage = Stage::on_row(screens::fake_library(), HD);
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
