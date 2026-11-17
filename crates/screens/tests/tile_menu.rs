//! The Tile menu (docs/spec/tile-menu.md): what opens it, its rows and what each asks for, B at every level, its
//! sounds, the button held as it closes; then how it looks over Home and how it comes and goes, drawn offscreen
//! and set against the committed references (`SHOTS=bless cargo test -p screens --test tile_menu` accepts what is
//! drawn now) and against the Bundle's own numbers.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::home::Focus;
use screens::tile_menu::{self, Opener};
use screens::{hint_bar, Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Left, LeftBumper, Menu, Right, RightBumper, Up, View, A, B, X, Y};
use ui::{Host, PadEvent, Press};

// ---------------------------------------------------------------------------------------------------- the library

/// A library as the library database gives it once `seed` has had its say, its games Steam's.
fn library_seeded(library: Library, seed: impl FnOnce(&mut library::Database, &[Game])) -> Library {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &library, 1_780_000_000).unwrap();
    let games = database.snapshot().unwrap().games;
    seed(&mut database, &games);
    database.snapshot().unwrap()
}

/// The fake library: the Recent row's first game is installed (Halo Infinite), its sixth is not (Baldur's Gate 3).
fn library() -> Library {
    library_seeded(screens::fake_library(), |_, _| {})
}

/// Home with focus on the Recent row's tile `tile`.
fn on_tile(library: Library, tile: usize) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    press(&mut dashboard, &[Down]);
    press(&mut dashboard, &[Right].repeat(tile));
    assert_eq!(dashboard.home_focus(), Focus::Recent(tile));
    dashboard.take_effects();
    dashboard
}

/// The Tile menu of the Recent row's tile `tile`, just opened.
fn menu_on(tile: usize) -> Dashboard {
    let mut dashboard = on_tile(library(), tile);
    press(&mut dashboard, &[Menu]);
    dashboard.take_effects();
    dashboard
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Presses and gives everything the presses asked for, sounds first as they come.
fn pressed(dashboard: &mut Dashboard, buttons: &[Button]) -> Vec<Effect> {
    press(dashboard, buttons);
    dashboard.take_effects()
}

/// Presses Down until the menu's focus is on the row `label`.
#[track_caller]
fn down_to(dashboard: &mut Dashboard, label: &str) {
    for _ in 0..8 {
        if dashboard.focus_line().contains(&format!("({label})")) {
            return;
        }
        press(dashboard, &[Down]);
    }
    panic!("no row {label}: {}", dashboard.focus_line());
}

const OPEN: Effect = Effect::Sound(Sound::PromptOpen);
const MOVE: Effect = Effect::Sound(Sound::GuideMove);
const ENTER: Effect = Effect::Sound(Sound::GuideSelect);
const CLOSE: Effect = Effect::Sound(Sound::PromptClose);
/// The More options flyout's height: its title, then Hide this game and Refresh art.
const FLYOUT: f32 = 99.6 + 2.0 * 72.0;

// ------------------------------------------------------------------------------------------------- what opens it

#[test]
fn menu_on_a_recent_tile_opens_its_menu_over_home_with_focus_on_the_first_row() {
    let mut home = on_tile(library(), 0);
    assert_eq!(pressed(&mut home, &[Menu]), [OPEN], "PromptOpen.wav, and nothing else");
    assert_eq!(home.focus_line(), "TileMenu: Menu(Play game) of Halo Infinite");
    assert_eq!((home.page(), home.pages_up()), (Some("TileMenu"), 1));
    // Focus stays on what opened it, and the hint bar is gone while the menu is up.
    assert_eq!(home.home_focus(), Focus::Recent(0));
    assert_eq!(home.hints(), hint_bar::NONE);
    // In the menu a held direction repeats as in the Guide: 700 ms, then every 80.
    assert_eq!(home.repeat(), screens::GUIDE_REPEAT);
    press(&mut home, &[B]);
    assert_eq!(home.repeat(), ui::Repeat::default());
}

#[test]
fn menu_on_a_groups_game_card_opens_its_menu_and_the_hint_bar_comes_back_as_it_closes() {
    let mut home = on_tile(library(), 0);
    press(&mut home, &[Down, Down, Right]);
    let card = Focus::Group { group: 0, card: 1 };
    assert_eq!(home.home_focus(), card);
    assert_eq!(home.hints(), hint_bar::MORE_OPTIONS_AND_SEARCH);
    let game = home.focused_game().map(|game| home.library().games[game].name.clone()).unwrap();
    home.take_effects();
    assert_eq!(pressed(&mut home, &[Menu]), [OPEN]);
    assert!(home.focus_line().starts_with("TileMenu: Menu(") && home.focus_line().ends_with(&format!(" of {game}")));
    assert_eq!(home.hints(), hint_bar::NONE);
    assert_eq!(pressed(&mut home, &[B]), [CLOSE]);
    assert_eq!((home.page(), home.home_focus()), (None, card));
    assert_eq!(home.hints(), hint_bar::MORE_OPTIONS_AND_SEARCH);
}

#[test]
fn menu_opens_nothing_where_there_is_no_game() {
    // The profile, where Home starts; the Play history tile; a wide tile; See all.
    let mut home = Dashboard::new(library());
    assert_eq!(pressed(&mut home, &[Menu]), []);
    press(&mut home, &[Down]);
    press(&mut home, &[Right; 9]);
    let history = home.home_focus();
    assert!(home.focused_game().is_none(), "the Play history tile: {history:?}");
    home.take_effects();
    assert_eq!(pressed(&mut home, &[Menu]), []);
    press(&mut home, &[Down]);
    assert!(matches!(home.home_focus(), Focus::Wide(_)));
    home.take_effects();
    assert_eq!(pressed(&mut home, &[Menu]), []);
    press(&mut home, &[Down]);
    press(&mut home, &[Right; 9]);
    assert!(matches!(home.home_focus(), Focus::Group { .. }) && home.focused_game().is_none(), "See all");
    home.take_effects();
    assert_eq!(pressed(&mut home, &[Menu]), []);
    assert_eq!(home.pages_up(), 0);
}

#[test]
fn the_menu_opens_with_one_call_for_any_game_and_not_for_one_that_has_left_the_library() {
    let library = library();
    let (game, gone) =
        (library.games[11].clone(), Game { id: 999, source_id: "999".into(), ..library.games[0].clone() });
    let mut home = on_tile(library, 0);
    // From the Game hub there is no Manage game row: the first row, then Favourites, then Customise.
    home.open_page(tile_menu::entry(&game, Opener::GameHub));
    assert_eq!(home.focus_line(), format!("TileMenu: Menu(Install) of {}", game.name));
    press(&mut home, &[Down, Down]);
    assert_eq!(home.focus_line(), format!("TileMenu: Menu(Customise Home) of {}", game.name));
    press(&mut home, &[B]);
    home.open_page(tile_menu::entry(&game, Opener::MyGames));
    press(&mut home, &[Down, Down, Down]);
    assert_eq!(home.focus_line(), format!("TileMenu: Menu(Customise games & apps) of {}", game.name));
    press(&mut home, &[B]);
    home.open_page(tile_menu::entry(&gone, Opener::Home));
    assert_eq!((home.pages_up(), home.focus_line().as_str()), (0, "Recent(0)"));
}

// ---------------------------------------------------------------------------------------- moving, and what is deaf

#[test]
fn up_and_down_move_along_the_rows_and_stop_at_the_ends_in_silence() {
    let mut menu = menu_on(0);
    assert_eq!(pressed(&mut menu, &[Up]), [], "the first row: nowhere to go, no sound");
    let rows = ["Add to Favourites", "Manage game", "Uninstall", "Customise Home", "More options"];
    for row in rows {
        // The gap before Customise Home is not a stop.
        assert_eq!(pressed(&mut menu, &[Down]), [MOVE]);
        assert_eq!(menu.focus_line(), format!("TileMenu: Menu({row}) of Halo Infinite"));
    }
    assert_eq!(pressed(&mut menu, &[Down]), [], "the last row");
    // A held direction moves as a press does, and stops dead at the end.
    menu.press(Press { button: Up, repeat: true });
    assert_eq!(menu.take_effects(), [MOVE]);
    menu.press(Press { button: Down, repeat: true });
    menu.press(Press { button: Down, repeat: true });
    assert_eq!(menu.take_effects(), [MOVE]);
}

#[test]
fn every_other_button_does_nothing_while_the_menu_is_up() {
    let mut menu = menu_on(0);
    press(&mut menu, &[Down]);
    menu.take_effects();
    let line = menu.focus_line();
    // Right on More options does not open the flyout, Menu opens no second menu, Y no Search.
    assert_eq!(pressed(&mut menu, &[Left, Right, Menu, View, X, Y, LeftBumper, RightBumper]), []);
    assert_eq!((menu.focus_line(), menu.pages_up(), menu.home_focus()), (line, 1, Focus::Recent(0)));
    // A held A or B is no second press.
    menu.press(Press { button: A, repeat: true });
    menu.press(Press { button: B, repeat: true });
    assert_eq!((menu.take_effects(), menu.pages_up()), (vec![], 1));
}

// ------------------------------------------------------------------------------------------ what each row asks for

#[test]
fn play_game_closes_the_menu_and_launches_the_game() {
    let mut menu = menu_on(2);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Play game) of Sea of Thieves");
    assert_eq!(pressed(&mut menu, &[A]), [ENTER, Effect::Launch(2)]);
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Recent(2)));
}

#[test]
fn install_closes_the_menu_and_asks_the_games_store() {
    let mut menu = menu_on(5);
    let game = menu.library().games[5].clone();
    assert!(!game.installed);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Install) of Baldur's Gate 3");
    // A game that is not installed has no Uninstall row: five rows.
    press(&mut menu, &[Down; 4]);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(More options) of Baldur's Gate 3");
    press(&mut menu, &[Up; 4]);
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER, Effect::Install(game.id)]);
    assert_eq!(menu.page(), None);
}

#[test]
fn the_favourites_row_marks_the_game_and_reads_the_other_way_for_a_favourite() {
    let mut menu = menu_on(1);
    let game = menu.library().games[1].clone();
    press(&mut menu, &[Down]);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Add to Favourites) of Forza Horizon 5");
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER, Effect::Favourite { game: game.id, on: true }]);
    assert_eq!(menu.page(), None);

    // The shell marks it in the library database, and its new snapshot is what the screens show: Home has a
    // Favourites Group with the game, focus is on the tile still, and the row now takes the mark away.
    let marked = library_seeded(screens::fake_library(), |database, games| {
        database.set_favourite(games[1].id, true).unwrap();
    });
    menu.set_library(marked);
    let favourites = menu.library().groups().next().unwrap();
    assert_eq!((favourites.name.as_str(), favourites.games.as_slice()), ("Favourites", [game.id].as_slice()));
    assert_eq!(menu.home_focus(), Focus::Recent(1));
    press(&mut menu, &[Menu, Down]);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Remove from Favourites) of Forza Horizon 5");
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER, Effect::Favourite { game: game.id, on: false }]);
}

/// Manage game opens the game's Game hub in the menu's place, and B there is Home again, focus on the tile.
#[test]
fn manage_game_opens_the_game_hub_in_the_menus_place() {
    let mut menu = menu_on(0);
    down_to(&mut menu, "Manage game");
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER]);
    assert_eq!((menu.page(), menu.pages_up()), (Some("GameHub"), 1));
    assert!(menu.focus_line().starts_with("GameHub: "), "{}", menu.focus_line());
    // Its own Tile menu has no Manage game row: Favourites, then the next.
    press(&mut menu, &[Menu, Down, Down]);
    assert!(menu.focus_line().starts_with("TileMenu: Menu("), "{}", menu.focus_line());
    assert!(!menu.focus_line().contains("Manage game"), "{}", menu.focus_line());
    press(&mut menu, &[B, B]);
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Recent(0)));
}

#[test]
fn customise_home_opens_settings_on_personalisation_and_b_comes_back_to_the_tile() {
    let mut menu = menu_on(3);
    down_to(&mut menu, "Customise Home");
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER]);
    assert_eq!((menu.focus_line().as_str(), menu.pages_up()), ("Settings: Personalisation: Groups", 1));
    // Settings' own way back: Theme settings' boxes, its rail, then what the menu was over.
    press(&mut menu, &[B, B, B]);
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Recent(3)));

    // From a Group's card: back on the card.
    press(&mut menu, &[Down, Down, Right, Menu]);
    down_to(&mut menu, "Customise Home");
    press(&mut menu, &[A, B, B, B]);
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Group { group: 0, card: 1 }));
}

#[test]
fn uninstall_asks_first_and_only_yes_asks_the_store() {
    let mut menu = menu_on(0);
    let game = menu.library().games[0].clone();
    down_to(&mut menu, "Uninstall");
    menu.take_effects();
    // The question takes the menu's place, focus on the confirming row. Nothing is uninstalled yet.
    assert_eq!(pressed(&mut menu, &[A]), [ENTER], "Enter.wav, and no PromptOpen.wav of its own");
    assert_eq!(menu.focus_line(), "TileMenu: Prompt(Uninstall) of Halo Infinite");
    assert_eq!(pressed(&mut menu, &[Up]), []);
    assert_eq!(pressed(&mut menu, &[Down]), [MOVE]);
    assert_eq!(menu.focus_line(), "TileMenu: Prompt(Cancel) of Halo Infinite");
    assert_eq!(pressed(&mut menu, &[Down, Left, Right, Menu, Y]), []);

    // Cancel: back to the menu, focus on its Uninstall row. A prompt's answer sounds as its closing.
    assert_eq!(pressed(&mut menu, &[A]), [CLOSE]);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Uninstall) of Halo Infinite");
    // Down from there steps over the gap to Customise.
    press(&mut menu, &[Down]);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Customise Home) of Halo Infinite");
    press(&mut menu, &[Up]);
    menu.take_effects();

    // B is Cancel.
    press(&mut menu, &[A]);
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[B]), [CLOSE]);
    assert_eq!((menu.focus_line().as_str(), menu.pages_up()), ("TileMenu: Menu(Uninstall) of Halo Infinite", 1));

    // Yes: the whole menu closes, then the store is asked.
    press(&mut menu, &[A]);
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [CLOSE, Effect::Uninstall(game.id)]);
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Recent(0)));
}

#[test]
fn more_options_opens_the_flyout_and_hide_this_game_hides_it() {
    let mut menu = menu_on(4);
    let game = menu.library().games[4].clone();
    down_to(&mut menu, "More options");
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER]);
    assert_eq!(menu.focus_line(), "TileMenu: MoreOptions(Hide this game) of Hades II");
    // Two rows: Up from the first goes nowhere, Down to Refresh art and back. Left does not close it.
    assert_eq!(pressed(&mut menu, &[Up, Left, Right]), []);
    assert_eq!(pressed(&mut menu, &[Down]), [MOVE]);
    assert_eq!(menu.focus_line(), "TileMenu: MoreOptions(Refresh art) of Hades II");
    assert_eq!(pressed(&mut menu, &[Down]), []);
    assert_eq!(pressed(&mut menu, &[Up]), [MOVE]);
    assert_eq!(pressed(&mut menu, &[A]), [ENTER, Effect::Hidden { game: game.id, on: true }]);
    assert_eq!(menu.page(), None);

    // The snapshot without the game: the Recent row closes up, focus staying on the tile where it was.
    let hidden = library_seeded(screens::fake_library(), |database, games| {
        database.set_hidden(games[4].id, true).unwrap();
    });
    menu.set_library(hidden);
    assert_eq!(menu.home_focus(), Focus::Recent(4));
    assert_ne!(menu.focused_game().map(|at| menu.library().games[at].id), Some(game.id));
}

/// Refresh art forgets the art fetched for this one game and fetches it again, first in line: the app does that.
#[test]
fn more_options_refresh_art_asks_for_this_games_art_again() {
    let mut menu = menu_on(4);
    let game = menu.library().games[4].clone();
    down_to(&mut menu, "More options");
    press(&mut menu, &[A, Down]);
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER, Effect::RefreshArt(Some(game.key()))]);
    assert_eq!(menu.page(), None);
}

/// Unhide is reached where the hidden games are listed: the Hidden Group, once the user shows it.
#[test]
fn a_hidden_games_card_has_a_menu_whose_flyout_shows_it_again() {
    let library = library_seeded(screens::fake_library(), |database, games| {
        database.set_hidden(games[9].id, true).unwrap();
        for filter in database.filters().unwrap() {
            database.set_filter_shown(filter.id, filter.key.as_deref() == Some("hidden")).unwrap();
        }
    });
    let game = library.hidden[0].clone();
    let mut home = on_tile(library, 0);
    press(&mut home, &[Down, Down]);
    assert_eq!(home.home_focus(), Focus::Group { group: 0, card: 0 });
    home.take_effects();
    assert_eq!(pressed(&mut home, &[Menu]), [OPEN]);
    assert_eq!(home.focus_line(), format!("TileMenu: Menu(Play game) of {}", game.name));
    down_to(&mut home, "More options");
    press(&mut home, &[A]);
    assert_eq!(home.focus_line(), format!("TileMenu: MoreOptions(Unhide this game) of {}", game.name));
    home.take_effects();
    assert_eq!(pressed(&mut home, &[A]), [ENTER, Effect::Hidden { game: game.id, on: false }]);
}

// ------------------------------------------------------------------------------------------------ B, level by level

#[test]
fn b_is_one_level_at_a_time_and_each_level_keeps_its_focus() {
    let mut menu = menu_on(0);
    down_to(&mut menu, "More options");
    press(&mut menu, &[A]);
    menu.take_effects();
    // The flyout closes, the menu's focus still on More options.
    assert_eq!(pressed(&mut menu, &[B]), [CLOSE]);
    assert_eq!((menu.focus_line().as_str(), menu.pages_up()), ("TileMenu: Menu(More options) of Halo Infinite", 1));
    // Opened again it starts on its first row, as every opening does.
    press(&mut menu, &[A]);
    assert_eq!(menu.focus_line(), "TileMenu: MoreOptions(Hide this game) of Halo Infinite");
    press(&mut menu, &[B]);
    menu.take_effects();
    // The menu closes: Home has the pad, focus where it always was, and nothing was asked of the shell.
    assert_eq!(pressed(&mut menu, &[B]), [CLOSE]);
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Recent(0)));
    // And opened again the menu starts on its first row.
    press(&mut menu, &[Menu]);
    assert_eq!(menu.focus_line(), "TileMenu: Menu(Play game) of Halo Infinite");
}

#[test]
fn the_guide_button_closes_the_menu_and_its_question_with_no_answer() {
    let mut menu = menu_on(0);
    down_to(&mut menu, "Uninstall");
    press(&mut menu, &[A]);
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[Guide]), [Effect::Sound(Sound::GuideOpen)], "no sound of the menu's own");
    assert!(menu.guide_open());
    assert_eq!((menu.page(), menu.home_focus()), (None, Focus::Recent(0)));
}

#[test]
fn more_options_on_a_game_that_has_left_the_library_closes_the_menu() {
    let mut menu = menu_on(0);
    down_to(&mut menu, "More options");
    let mut without = library();
    without.games.remove(0);
    menu.set_library(without);
    menu.take_effects();
    assert_eq!(pressed(&mut menu, &[A]), [ENTER]);
    assert_eq!(menu.page(), None);
}

// ------------------------------------------------------------------------------------ the button that closed it

#[test]
fn the_a_that_chose_a_row_does_nothing_to_the_tile_under_the_menu_until_it_is_let_go() {
    let mut menu = menu_on(0);
    menu.pad_event(PadEvent::Press(Press { button: A, repeat: false }));
    assert_eq!(menu.take_effects(), [ENTER, Effect::Launch(0)]);
    // Still down, and repeating as a key does: the tile under the menu hears nothing.
    menu.pad_event(PadEvent::Press(Press { button: A, repeat: true }));
    assert_eq!(menu.take_effects(), []);
}

#[test]
fn the_b_that_closed_the_menu_does_not_also_leave_the_group() {
    let mut home = on_tile(library(), 0);
    press(&mut home, &[Down, Down, Menu]);
    let card = Focus::Group { group: 0, card: 0 };
    home.take_effects();
    home.pad_event(PadEvent::Press(Press { button: B, repeat: false }));
    assert_eq!((home.page(), home.home_focus()), (None, card));
    home.pad_event(PadEvent::Press(Press { button: B, repeat: true }));
    assert_eq!(home.home_focus(), card, "B is still down");
    // Let go and pressed again, it is Home's: B on a Group goes to the Recent row.
    home.pad_event(PadEvent::Release(B));
    home.pad_event(PadEvent::Press(Press { button: B, repeat: false }));
    assert_eq!(home.home_focus(), Focus::Recent(0));
}

#[test]
fn a_direction_held_as_the_menu_closes_does_not_move_focus_on_the_page() {
    let mut menu = menu_on(2);
    menu.pad_event(PadEvent::Press(Press { button: Down, repeat: false }));
    menu.pad_event(PadEvent::Press(Press { button: B, repeat: false }));
    assert_eq!(menu.page(), None);
    menu.take_effects();
    for _ in 0..3 {
        menu.pad_event(PadEvent::Press(Press { button: Down, repeat: true }));
    }
    assert_eq!((menu.home_focus(), menu.take_effects()), (Focus::Recent(2), vec![]));
    // Let go, it moves focus again; and so does a press that is not a repeat, with no release told (a key).
    menu.pad_event(PadEvent::Release(Down));
    menu.pad_event(PadEvent::Press(Press { button: Down, repeat: true }));
    assert!(matches!(menu.home_focus(), Focus::Wide(_)));
    let mut menu = menu_on(2);
    press(&mut menu, &[Right, B, Right]);
    assert_eq!(menu.home_focus(), Focus::Recent(3));
}

// ------------------------------------------------------------------------------------------------------ pictures

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// A cover and a wide background for each of the Recent row's games, each in its own colours with bands across.
fn library_with_art() -> Library {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("tile-menu-art");
        std::fs::create_dir_all(&dir).unwrap();
        for game in 0..7u32 {
            for (kind, w, h) in [("cover", 300, 450), ("hero", 960, 310)] {
                let hue = |shift: u32| (((game * 53 + shift * 85) % 200) + 40) as u8;
                let image = image::RgbImage::from_fn(w, h, |_, y| {
                    let dim = if (y * 8 / h) % 2 == 0 { 1 } else { 2 };
                    image::Rgb([hue(0) / dim, hue(1) / dim, hue(2) / dim])
                });
                shots::save_shared(&image, &dir.join(format!("{game}-{kind}.png")));
            }
        }
        dir
    });
    let mut library = screens::fake_library();
    for (i, game) in library.games.iter_mut().enumerate().take(7) {
        game.cover = Some(dir.join(format!("{i}-cover.png")));
        game.hero = Some(dir.join(format!("{i}-hero.png")));
    }
    library_seeded(library, |_, _| {})
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
static DRAWING: Mutex<()> = Mutex::new(());

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    /// Home, 1920x1080, focus on the Recent row's tile `tile`.
    fn on_tile(tile: usize) -> Self {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, (1920, 1080));
        let mut dashboard = on_tile(library_with_art(), tile);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn press(&mut self, buttons: &[Button]) {
        press(&mut self.dashboard, buttons);
    }

    /// The screen at rest.
    fn still(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }

    /// The screen `dt` seconds after the last frame: what is mid-flight stays mid-flight.
    fn frame_after(&mut self, mut dt: f32) -> image::RgbaImage {
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
        let pixels = read_back(gpu, &target.texture, target.size);
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }

    fn check(&mut self, name: &str) -> image::RgbaImage {
        let image = self.still();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
        image
    }
}

fn colour(image: &image::RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let pixel = image.get_pixel(x, y).0;
    [pixel[0], pixel[1], pixel[2]]
}

fn near(a: [u8; 3], b: [u8; 3], by: u8) -> bool {
    a.iter().zip(b).all(|(&a, b)| a.abs_diff(b) <= by)
}

const PANEL: [u8; 3] = [0x2B, 0x2B, 0x2B];
const FILL: [u8; 3] = [0x40, 0x40, 0x40];
const GREEN: [u8; 3] = [0x2C, 0xA2, 0x43];

/// The rows where the column of pixels at `x` is the panel's own colour: its top and its foot there.
fn panel_rows(image: &image::RgbaImage, x: u32) -> Option<(u32, u32)> {
    let rows: Vec<u32> = (0..image.height()).filter(|&y| colour(image, x, y) == PANEL).collect();
    Some((*rows.first()?, *rows.last()?))
}

/// Asserts the panel's rectangle: its left and right columns and, down the column 20 in from its left, its top
/// and foot, each to a pixel of where the Bundle's is (docs/spec/tile-menu.md, section 2.5). Not for a panel
/// whose last row has focus: the ring is on its foot.
#[track_caller]
fn assert_panel(image: &image::RgbaImage, left: u32, top: f32, height: f32) {
    let (first, last) = panel_rows(image, left + 20).expect("a panel");
    assert!((first as f32 - top).abs() <= 1.0, "the panel's top at {first}, not {top}");
    assert!((last as f32 + 1.0 - (top + height)).abs() <= 1.0, "the panel's foot at {last}, not {}", top + height);
    assert_sides(image, left, first);
}

#[track_caller]
fn assert_sides(image: &image::RgbaImage, left: u32, first: u32) {
    let y = first + 10;
    assert_eq!((colour(image, left, y), colour(image, left + 449, y)), (PANEL, PANEL), "450 wide from {left}");
    assert_ne!(colour(image, left - 1, y), PANEL);
    assert_ne!(colour(image, left + 450, y), PANEL);
}

/// The installed game's menu, six rows: the panel at 735, 263.7, 450 x 552.6 with focus on Play game, its ring 8
/// outside the panel's sides; Home under it at 0.4 of its light, the tile that opened it still focused.
#[test]
fn the_menu_of_an_installed_game_has_six_rows() {
    let mut stage = Stage::on_tile(0);
    let before = stage.still();
    stage.press(&[Menu]);
    let image = stage.check("tile-menu-installed");
    assert_panel(&image, 735, 263.7, 552.6);
    // The first row from y 363: its ring's strokes, and its fill between them.
    for (x, y) in [(728, 400), (1190, 400), (960, 364), (960, 433)] {
        assert!(near(colour(&image, x, y), GREEN, 8), "the ring at {x}, {y}: {:?}", colour(&image, x, y));
    }
    assert_eq!(colour(&image, 1100, 399), FILL);
    // The dim: `#99000000` over the page, the top bar and the wide tiles alike.
    for (x, y) in [(300, 940), (1600, 940), (60, 60), (1700, 300)] {
        let (was, is) = (colour(&before, x, y), colour(&image, x, y));
        let dimmed = was.map(|c| (c as f32 * 0.4).round() as u8);
        assert!(near(is, dimmed, 2), "at {x}, {y}: {was:?} dimmed is {is:?}");
    }
}

/// A game that is not installed: Install first and no Uninstall, five rows, as the Bundle's picture
/// (24-tile-menu-not-installed.jpg): the panel at y 299.7..780.3, the gap before Customise Home.
#[test]
fn the_menu_of_a_game_that_is_not_installed_has_five_rows() {
    let mut stage = Stage::on_tile(5);
    stage.press(&[Menu]);
    let image = stage.check("tile-menu-not-installed");
    assert_panel(&image, 735, 299.7, 480.6);
    // Customise Home's row is 21 further down than a fourth row would be: rows at 399, 471, 543, then 636, 708.
    stage.press(&[Down, Down, Down]);
    let image = stage.still();
    assert_eq!(colour(&image, 1100, 636 + 36), FILL);
    assert!(near(colour(&image, 1100, 543 + 72 + 3), PANEL, 3), "the gap, with nothing drawn in it");
}

/// From the Game hub there is no Manage game row: four rows for a game that is not installed, at y 335.7..744.3.
#[test]
fn the_menu_from_the_game_hub_has_four_rows() {
    let mut stage = Stage::on_tile(5);
    let game = stage.dashboard.library().games[5].clone();
    stage.dashboard.open_page(tile_menu::entry(&game, Opener::GameHub));
    let image = stage.check("tile-menu-from-the-hub");
    assert_panel(&image, 735, 335.7, 408.6);
}

/// More options (25-tile-menu-more-options.jpg): the flyout 77 in from the menu's left at the menu's top, the menu
/// behind it at 0.45 as one picture, its More options row still drawn focused.
#[test]
fn more_options_is_a_flyout_over_the_dimmed_menu() {
    let mut stage = Stage::on_tile(5);
    stage.press(&[Menu, Down, Down, Down, Down]);
    let menu = stage.still();
    stage.press(&[A]);
    let image = stage.check("tile-menu-more-options");
    // Two rows, Hide this game and Refresh art: 99.6 + 2 x 72 tall, from the menu's top.
    assert_panel(&image, 812, 299.7, FLYOUT);
    assert_eq!(colour(&image, 1100, 399 + 36), FILL, "focus on Hide this game");
    // The menu's left 77 shows, faded; its focused row's ring is there still, fainter than it was.
    assert_ne!(colour(&image, 760, 310), PANEL);
    let (was, is) = (colour(&menu, 728, 744), colour(&image, 728, 744));
    assert!(near(was, GREEN, 8) && is[1] > is[0] + 20 && is[1] < was[1], "the ring behind: {was:?}, {is:?}");
    // B: the flyout has gone at once and the menu is lit again.
    stage.press(&[B]);
    let back = stage.frame_after(0.0);
    assert_eq!(colour(&back, 760, 310), PANEL);
    assert_ne!(colour(&back, 1200, 310), PANEL);
}

/// "Uninstall X?" in the menu's place: one panel, centred by its own height (735, 418.2, 450 x 243.6), its
/// question as the title and two rows, focus on the confirming one. The panel is the one the Bundle's picture of
/// "Remove X?" shows (26-tile-menu-remove-prompt.jpg).
#[test]
fn the_uninstall_question_takes_the_menus_place() {
    let mut stage = Stage::on_tile(0);
    stage.press(&[Menu, Down, Down, Down, A]);
    assert_eq!(stage.dashboard.focus_line(), "TileMenu: Prompt(Uninstall) of Halo Infinite");
    let image = stage.check("tile-menu-uninstall-prompt");
    assert_panel(&image, 735, 418.2, 243.6);
    assert_eq!(colour(&image, 1100, 518 + 36), FILL);
    // A question too long for a line wraps, and the panel grows by a line: 283.2 tall.
    drop(stage);
    let mut stage = Stage::on_tile(6);
    stage.press(&[Menu, Down, Down, Down, A]);
    assert_eq!(stage.dashboard.focus_line(), "TileMenu: Prompt(Uninstall) of Hollow Knight: Silksong");
    let image = stage.check("tile-menu-uninstall-prompt-two-lines");
    assert_panel(&image, 735, 398.4, 283.2);
}

/// The menu on a Group's card: Home is risen to the Group under it, and the hint bar is not drawn.
#[test]
fn the_menu_over_a_group_hides_the_hint_bar() {
    let mut stage = Stage::on_tile(0);
    stage.press(&[Down, Down]);
    let before = stage.still();
    // The hint bar's fill, in the padding at its right end.
    let bar = (1850, 972);
    assert!(near(colour(&before, bar.0, bar.1), [3, 3, 3], 30), "{:?}", colour(&before, bar.0, bar.1));
    stage.press(&[Menu]);
    let image = stage.check("tile-menu-on-a-group");
    assert!(panel_rows(&image, 755).is_some());
    stage.press(&[B]);
    let after = stage.still();
    assert_eq!(colour(&after, bar.0, bar.1), colour(&before, bar.0, bar.1), "the bar is back as the menu closes");
}

/// The fly-up (section 7): the dim is whole from the first frame; the panel starts 80 under its place and unseen,
/// is part of the way and part faded mid-flight, and in its place 250 ms on.
#[test]
fn the_menu_flies_up_and_fades_in_under_a_dim_that_is_there_at_once() {
    let mut stage = Stage::on_tile(5);
    let before = stage.still();
    let dimmed = |x, y| colour(&before, x, y).map(|c| (c as f32 * 0.4).round() as u8);
    stage.press(&[Menu]);
    let first = stage.frame_after(0.0);
    assert!(near(colour(&first, 300, 940), dimmed(300, 940), 2), "the dim, at once");
    assert!(panel_rows(&first, 755).is_none(), "the panel is not seen yet");
    assert!(near(colour(&first, 960, 500), dimmed(960, 500), 2));

    // Half way through: cubic ease-out has it an eighth of 80 from its place, and it is half faded in.
    let middle = stage.frame_after(0.125);
    let (grey, under) = (colour(&middle, 755, 600), colour(&first, 755, 600));
    assert!(grey != PANEL && grey != under, "half faded: {grey:?} over {under:?}");
    // The first row's ring, which at rest starts at y 399: the first row of pixels plainly green down the middle.
    let green = |y: &u32| {
        let [r, g, b] = colour(&middle, 960, *y);
        g as i32 > r as i32 + 40 && g as i32 > b as i32 + 40
    };
    let ring = (380..440).find(green).expect("the ring");
    assert!((408..=411).contains(&ring), "the first row's ring mid-flight: {ring}");

    let last = stage.frame_after(0.13);
    assert_panel(&last, 735, 299.7, 480.6);
    assert!(!stage.host.animating(), "and it is at rest");
}

/// Closing: the dim goes at once and Home has the pad at once; the panel flies down and out over 150 ms, keeping
/// the rows it held, and is gone after.
#[test]
fn the_menu_flies_down_and_out_once_the_dim_has_gone() {
    let mut stage = Stage::on_tile(5);
    let before = stage.still();
    stage.press(&[Menu]);
    stage.still();
    stage.press(&[B]);
    assert_eq!(stage.dashboard.page(), None);
    stage.frame_after(0.0);
    let middle = stage.frame_after(0.1);
    assert_eq!(colour(&middle, 300, 940), colour(&before, 300, 940), "no dim");
    // 100 of 150 ms: cubic ease-in has it (2/3)^3 of 80 down, and two thirds faded out.
    let grey = colour(&middle, 755, 600);
    assert!(grey != PANEL && grey != colour(&before, 755, 600), "what is left of the panel: {grey:?}");
    let gone = stage.frame_after(0.06);
    assert_eq!(colour(&gone, 755, 600), colour(&before, 755, 600));
    assert_eq!(colour(&gone, 960, 440), colour(&before, 960, 440));
    assert!(!stage.host.animating());

    // Opened again while it flies out, the opening takes over from its own start.
    stage.press(&[Menu]);
    stage.frame_after(0.0);
    stage.frame_after(0.3);
    stage.press(&[B]);
    stage.frame_after(0.0);
    assert!(panel_rows(&stage.frame_after(0.05), 755).is_none(), "part faded, on its way out");
    stage.press(&[Menu]);
    let reopened = stage.frame_after(0.0);
    assert!(panel_rows(&reopened, 755).is_none(), "from its start: unseen");
    assert!(near(colour(&reopened, 300, 940), colour(&before, 300, 940).map(|c| (c as f32 * 0.4).round() as u8), 2));
}

/// Under Reduce motion everything is in its place, or gone, at once: the fades too.
#[test]
fn under_reduce_motion_the_menu_is_there_and_gone_at_once() {
    let mut stage = Stage::on_tile(5);
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
    // Home at rest first, so that what moves after is the menu's alone.
    let before = stage.still();
    assert!(!stage.host.animating());
    stage.press(&[Menu]);
    let first = stage.frame_after(0.0);
    assert_panel(&first, 735, 299.7, 480.6);
    stage.press(&[Down, Down, Down, Down, A]);
    let flyout = stage.frame_after(0.0);
    assert_panel(&flyout, 812, 299.7, FLYOUT);
    stage.press(&[B, B]);
    let gone = stage.frame_after(0.0);
    assert_eq!(colour(&gone, 755, 600), colour(&before, 755, 600));
    assert!(!stage.host.animating());
}

/// The question flies up as the menu did, and so does the menu coming back from it (section 7).
#[test]
fn the_question_and_the_menu_coming_back_fly_up_afresh() {
    let mut stage = Stage::on_tile(0);
    stage.press(&[Menu, Down, Down, Down]);
    stage.still();
    stage.press(&[A]);
    let first = stage.frame_after(0.0);
    assert!(panel_rows(&first, 755).is_none(), "the question starts unseen, and the menu has gone");
    let last = stage.frame_after(0.26);
    assert_panel(&last, 735, 418.2, 243.6);
    stage.press(&[B]);
    assert!(panel_rows(&stage.frame_after(0.0), 755).is_none(), "the menu starts unseen");
    let last = stage.frame_after(0.26);
    assert_panel(&last, 735, 263.7, 552.6);
    assert_eq!(colour(&last, 1100, 579 + 36), FILL, "focus on Uninstall still");
}

/// The flyout comes in from 48 to its left over 120 ms, fading in.
#[test]
fn the_flyout_comes_in_from_the_left() {
    let mut stage = Stage::on_tile(5);
    stage.press(&[Menu, Down, Down, Down, Down]);
    stage.still();
    stage.press(&[A]);
    let first = stage.frame_after(0.0);
    assert_ne!(colour(&first, 1200, 310), PANEL, "unseen at first");
    assert_ne!(colour(&first, 760, 310), PANEL, "the menu is behind it at once");
    let middle = stage.frame_after(0.06);
    let grey = colour(&middle, 1150, 310);
    assert!(grey != PANEL && grey[0] > 20, "half faded in: {grey:?}");
    // Cubic ease-out, half way: an eighth of 48 to go, so its right edge is 6 short of 1262.
    assert_ne!(colour(&middle, 1259, 310), grey);
    let last = stage.frame_after(0.07);
    assert_panel(&last, 812, 299.7, FLYOUT);
}
