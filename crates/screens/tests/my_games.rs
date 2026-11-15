//! My games & apps (docs/spec/my-games.md): the shell, Games, Play history and Manage. Where focus goes and what a
//! press asks for, with no picture; then the page drawn offscreen and compared with the committed reference images,
//! as tests/shots.rs does for Home (`SHOTS=bless cargo test -p screens` accepts what is drawn now). The Bundle's own
//! pictures of these states are 01 to 08 of docs/spec/pictures.md.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use chrono::TimeZone;
use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::home::Focus;
use screens::my_games::Entry;
use screens::pages::Open;
use screens::{hint_bar, Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Left, Menu, Right, Up, A, B, X, Y};
use ui::{Host, Press};

const DAY: u64 = 86_400;

/// Noon on Sunday 15 November 2026, the day the Bundle's pictures were taken, where the tests run: Play history's
/// days are the calendar's there.
fn now() -> u64 {
    chrono::Local.with_ymd_and_hms(2026, 11, 15, 12, 0, 0).unwrap().timestamp() as u64
}

/// Art for the first games, drawn once: a cover each, in its own colours with bands across it, so a crop shows.
fn art_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("my-games-art");
        std::fs::create_dir_all(&dir).unwrap();
        for game in 0..12u32 {
            let hue = |shift: u32| (((game * 67 + shift * 85) % 200) + 40) as u8;
            let image = image::RgbImage::from_fn(300, 450, |x, y| {
                let edge = x < 6 || y < 6 || x >= 294 || y >= 444;
                let level = |c: u8| match (edge, (y * 8 / 450) % 2 == 0) {
                    (true, _) => 255,
                    (false, true) => c,
                    (false, false) => c / 2,
                };
                image::Rgb([level(hue(0)), level(hue(1)), level(hue(2))])
            });
            image.save(dir.join(format!("{game}.png"))).unwrap();
        }
        dir
    })
}

/// Twenty-two made-up games, as a source reads them: name, how many days ago it was last played (none: never), and
/// whether it is installed. Eighteen are installed, which is four rows of the grid, the last of three. Seven were
/// played, on days that fall in six of Play history's sections; one of them is not installed.
const GAMES: [(&str, Option<u64>, bool); 22] = [
    ("Ashfall Courier", Some(0), true),
    ("Brass Meridian", Some(1), true),
    ("Copperhead Station", Some(2), true),
    ("Dune Cartographer", Some(3), false),
    ("Ember Choir", Some(12), true),
    ("Fathom Nine", Some(40), true),
    ("Glasswing Atlas", Some(400), true),
    ("Hollow Pines Motel", None, true),
    ("The Iron Orchard", None, true),
    ("Juniper Static", None, false),
    ("Kestrel Run", None, true),
    ("Lanternfish Deep", None, true),
    ("Marrow Street", None, true),
    ("Nightjar Protocol", None, false),
    ("An Open Tide", None, true),
    ("Quiet Orbit", None, true),
    ("Saltwind Rally", None, true),
    ("Thimble & Thorn", None, true),
    ("Umbra Garden", None, false),
    ("Vesper Line", None, true),
    ("Wren and the Long Winter of the Northern Reach", None, true),
    ("Zenith Yard", None, true),
];

/// Those games as a source reads them, every second of the first twelve with a cover.
fn source() -> Library {
    let games = GAMES.iter().enumerate().map(|(i, &(name, played, installed))| Game {
        appid: 2000 + i as u32,
        name: name.into(),
        last_played: played.map_or(0, |days| now() - 3600 - days * DAY),
        playtime_minutes: if played.is_some() { 120 } else { 0 },
        installed,
        cover: (i < 12 && i.is_multiple_of(2)).then(|| art_dir().join(format!("{i}.png"))),
        ..Game::default()
    });
    Library { user: Some("Player One".into()), games: games.collect(), ..Library::default() }
}

/// What the screens are given once a library database has taken in what its source read, one reading after
/// another: the games keep their ids from one to the next, as they do in the Dashboard's own database.
fn through(readings: &[Library]) -> Library {
    let mut database = library::Database::in_memory().unwrap();
    for found in readings {
        database.sync(library::STEAM, found, now() - 500 * DAY).unwrap();
    }
    database.snapshot_at(now()).unwrap()
}

/// The library with only the games `keep` picks.
fn library_of(keep: impl Fn(&Game) -> bool) -> Library {
    let mut found = source();
    found.games.retain(keep);
    through(&[found])
}

/// The library after its source has read it a second time, with `change` made to its games.
fn library_after(change: impl Fn(&mut Game)) -> Library {
    let mut later = source();
    later.games.iter_mut().for_each(change);
    through(&[source(), later])
}

fn library() -> Library {
    library_of(|_| true)
}

/// The page over Home, opened as `entry` opens it.
fn page_on(library: Library, entry: Entry) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    dashboard.open_page(Open::MyGames(entry));
    dashboard
}

fn page(entry: Entry) -> Dashboard {
    page_on(library(), entry)
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// The focus line after these presses, without the page's name.
#[track_caller]
fn after(dashboard: &mut Dashboard, buttons: &[Button]) -> String {
    press(dashboard, buttons);
    let line = dashboard.focus_line();
    line.strip_prefix("MyGames: ").unwrap_or_else(|| panic!("the page is not up: {line}")).to_string()
}

/// Longer than the nav's settle wait: a step along the nav after it is a step on its own.
fn rest() {
    std::thread::sleep(Duration::from_millis(170));
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

fn sounds(dashboard: &mut Dashboard) -> Vec<Sound> {
    let effects = dashboard.take_effects().into_iter();
    effects.filter_map(|effect| if let Effect::Sound(sound) = effect { Some(sound) } else { None }).collect()
}

// --- Where focus goes ---

#[test]
fn every_way_in_gives_focus_to_the_content() {
    assert_eq!(after(&mut page(Entry::Games), &[]), "Games: Tile(0)");
    assert_eq!(after(&mut page(Entry::PlayHistory), &[]), "PlayHistory: Row(0)");
    assert_eq!(after(&mut page(Entry::Queue), &[]), "Manage: Queue");
    // Full library's content is #53's: until then it has nothing to focus, and says which filter it was handed.
    let see_all = Entry::FullLibrary { filter: Some(7) };
    assert_eq!(after(&mut page(see_all), &[]), "FullLibrary { filter: Some(7) }: Nav(FullLibrary)");
}

#[test]
fn with_nothing_to_focus_in_the_content_focus_is_on_the_nav_and_stays_there() {
    // No game is installed, and none was ever played.
    let empty = || library_of(|game| !game.installed && game.last_played == 0);
    let mut games = page_on(empty(), Entry::Games);
    assert_eq!(after(&mut games, &[Right]), "Games: Nav(Games)");
    assert_eq!(after(&mut games, &[A, Left, Y, Menu]), "Games: Nav(Games)");
    assert_eq!(games.hints(), hint_bar::NONE);
    let mut history = page_on(empty(), Entry::PlayHistory);
    assert_eq!(after(&mut history, &[Right, A]), "PlayHistory: Nav(PlayHistory)");
    // The nav still leads everywhere, and B is Home.
    assert_eq!(after(&mut history, &[Down, Down, Right]), "Manage: Queue");
    press(&mut history, &[B]);
    assert_eq!(history.page(), None);
}

#[test]
fn the_nav_shows_the_item_focus_steps_onto_and_stops_at_its_ends() {
    let mut page = page(Entry::Games);
    assert_eq!(after(&mut page, &[Left]), "Games: Nav(Games)");
    assert_eq!(after(&mut page, &[Up, Left]), "Games: Nav(Games)", "the first item, and Left is nothing");
    let mut lines = Vec::new();
    for _ in 0..5 {
        rest();
        lines.push(after(&mut page, &[Down]));
    }
    // Each step, taken on its own, shows its item at once. Down from Full library crosses the line to Manage, and
    // Manage is the end.
    let full_library = "FullLibrary { filter: None }: Nav(FullLibrary)";
    assert_eq!(
        lines,
        [
            "Apps: Nav(Apps)",
            "PlayHistory: Nav(PlayHistory)",
            full_library,
            "Manage: Nav(Manage)",
            "Manage: Nav(Manage)"
        ]
    );
    rest();
    assert_eq!(after(&mut page, &[Up]), full_library);
    // Apps and Full library have nothing to focus yet: Right and A leave focus on the nav.
    assert_eq!(after(&mut page, &[Right, A]), full_library);
    rest();
    assert_eq!(after(&mut page, &[Up, Right]), "PlayHistory: Row(0)");
}

#[test]
fn focus_passing_along_the_nav_shows_nothing_until_it_rests_or_right_or_a_lands() {
    let mut page = page(Entry::Games);
    // Left, then three steps with no rest between them: Games is still up, and the items passed over never show.
    assert_eq!(after(&mut page, &[Left, Down, Down, Down]), "Games: Nav(FullLibrary)");
    assert_eq!(after(&mut page, &[Up]), "Games: Nav(PlayHistory)");
    // Right shows the focused item at once and lands in it, never in what was showing before.
    assert_eq!(after(&mut page, &[Right]), "PlayHistory: Row(0)");
    // And A the same.
    assert_eq!(after(&mut page, &[Left, Down, Down, A]), "Manage: Queue");
    // Back on the item that is showing, nothing waits: resting changes nothing.
    assert_eq!(after(&mut page, &[Left, Up, Down]), "Manage: Nav(Manage)");
    rest();
    assert_eq!(after(&mut page, &[]), "Manage: Nav(Manage)");
    // An item focus has rested on is showing by the next press, even with no frame drawn between.
    assert_eq!(after(&mut page, &[Up, Up]), "FullLibrary { filter: None }: Nav(PlayHistory)");
    rest();
    assert_eq!(after(&mut page, &[Left]), "PlayHistory: Nav(PlayHistory)");
}

#[test]
fn the_grids_edges_stop_and_left_from_the_first_column_is_the_nav() {
    let mut page = page(Entry::Games);
    page.take_effects();
    // Up from the first row is nothing, and silent.
    assert_eq!(after(&mut page, &[Up]), "Games: Tile(0)");
    assert!(page.take_effects().is_empty());
    // Along the row to its fifth tile, where Right stops: nothing wraps.
    assert_eq!(after(&mut page, &[Right, Right, Right, Right]), "Games: Tile(4)");
    assert_eq!(sounds(&mut page), [Sound::Move; 4]);
    assert_eq!(after(&mut page, &[Right]), "Games: Tile(4)");
    assert!(page.take_effects().is_empty());
    // Down the column; the last row has three games, so Down onto it lands on its last.
    assert_eq!(after(&mut page, &[Down, Down]), "Games: Tile(14)");
    assert_eq!(after(&mut page, &[Down]), "Games: Tile(17)");
    assert_eq!(after(&mut page, &[Right]), "Games: Tile(17)");
    assert_eq!(after(&mut page, &[Left, Left]), "Games: Tile(15)");
    // Left from the first column is the Games nav item, and Right from there the game it left.
    assert_eq!(after(&mut page, &[Left]), "Games: Nav(Games)");
    assert_eq!(after(&mut page, &[Left]), "Games: Nav(Games)");
    assert_eq!(after(&mut page, &[Right]), "Games: Tile(15)");
    // A does the same from the nav.
    assert_eq!(after(&mut page, &[Up, Left, A]), "Games: Tile(10)");
}

#[test]
fn down_from_the_last_row_is_see_full_library_which_leads_to_full_library() {
    let mut page = page(Entry::Games);
    assert_eq!(after(&mut page, &[Right, Down, Down, Down]), "Games: Tile(16)");
    assert_eq!(page.hints(), hint_bar::MORE_OPTIONS_AND_SEARCH);
    assert_eq!(after(&mut page, &[Down]), "Games: SeeFullLibrary");
    assert_eq!(page.hints(), hint_bar::NONE);
    // Right and Down are nothing there; Up is the game Down came from.
    assert_eq!(after(&mut page, &[Right, Down, Y, Menu]), "Games: SeeFullLibrary");
    assert_eq!(after(&mut page, &[Up]), "Games: Tile(16)");
    // Left is the nav, and Right from the nav is the game again, not the button.
    assert_eq!(after(&mut page, &[Down, Left]), "Games: Nav(Games)");
    assert_eq!(after(&mut page, &[Right]), "Games: Tile(16)");
    // A opens Full library by way of its nav item. It has nothing to focus yet, so focus is on the item.
    page.take_effects();
    assert_eq!(after(&mut page, &[Down, A]), "FullLibrary { filter: None }: Nav(FullLibrary)");
    assert!(asked(&mut page).is_empty());
    // Games forgot its place when the page moved on: back on it, Right is the first game.
    rest();
    assert_eq!(after(&mut page, &[Up, Up, Up, Right]), "Games: Tile(0)");
}

#[test]
fn games_lists_the_installed_games_by_name_and_counts_them() {
    let page = page(Entry::Games);
    let library = page.library();
    assert_eq!(library.games.iter().filter(|game| game.installed).count(), 18);
    // Play history counts every played game, installed or not.
    assert_eq!(library.history.len(), 7);
}

#[test]
fn y_on_a_game_opens_search_over_the_page_and_b_comes_back_to_the_game() {
    let mut page = page(Entry::Games);
    press(&mut page, &[Right, Down]);
    page.take_effects();
    press(&mut page, &[Y]);
    assert_eq!((page.focus_line().as_str(), page.pages_up()), ("Search: Open", 2));
    assert!(page.take_effects().is_empty(), "Y is silent");
    press(&mut page, &[B]);
    assert_eq!((page.focus_line().as_str(), page.pages_up()), ("MyGames: Games: Tile(6)", 1));
    // Not from the nav, nor from Play history or Manage.
    assert_eq!(after(&mut page, &[Left, Left, Y]), "Games: Nav(Games)");
    assert_eq!(after(&mut page, &[Down, Down, Right, Y]), "PlayHistory: Row(0)");
    assert_eq!(after(&mut page, &[Left, Down, Down, Right, Y]), "Manage: Queue");
    assert_eq!(page.pages_up(), 1);
}

#[test]
fn a_on_a_game_asks_nothing_until_the_game_hub_is_there_and_menu_and_x_open_its_tile_menu() {
    let mut page = page(Entry::Games);
    page.take_effects();
    assert_eq!(after(&mut page, &[A]), "Games: Tile(0)");
    assert!(page.take_effects().is_empty());
    assert_eq!(page.pages_up(), 1);
    // The Tile menu over the page, which says "Customise games & apps" where Home's says "Customise Home"; B
    // comes back to the tile.
    for button in [Menu, X] {
        press(&mut page, &[button]);
        assert!(page.focus_line().starts_with("TileMenu: Menu("), "{}", page.focus_line());
        assert_eq!(page.pages_up(), 2);
        press(&mut page, &[Down, Down, Down, Down, Down, Down, Up]);
        let line = page.focus_line();
        assert!(line.starts_with("TileMenu: Menu(Customise games & apps) of "), "{line}");
        assert_eq!(after(&mut page, &[B]), "Games: Tile(0)");
        assert_eq!(page.pages_up(), 1);
    }
}

#[test]
fn play_history_moves_across_its_sections_stops_at_its_ends_and_remembers_its_row() {
    let mut page = page(Entry::PlayHistory);
    assert_eq!(page.hints(), hint_bar::MORE_OPTIONS);
    assert_eq!(after(&mut page, &[Up, Right, Y, X]), "PlayHistory: Row(0)");
    assert_eq!(after(&mut page, &[Down, Down, Down]), "PlayHistory: Row(3)");
    // Left is the nav, with no hints; Right is the row it left.
    assert_eq!(after(&mut page, &[Left]), "PlayHistory: Nav(PlayHistory)");
    assert_eq!(page.hints(), hint_bar::NONE);
    assert_eq!(after(&mut page, &[Right]), "PlayHistory: Row(3)");
    assert_eq!(after(&mut page, &[Down, Down, Down, Down, Down]), "PlayHistory: Row(6)");
    // And still the row after the page has shown something else.
    assert_eq!(after(&mut page, &[Left, Down, Down, Right]), "Manage: Queue");
    assert_eq!(after(&mut page, &[Left, Up, Up, Right]), "PlayHistory: Row(6)");
}

#[test]
fn a_second_a_on_a_play_history_row_within_the_double_tap_plays_its_game() {
    let mut page = page(Entry::PlayHistory);
    press(&mut page, &[Down, Down]);
    page.take_effects();
    // One A waits for the Game hub, which is not there yet: nothing.
    press(&mut page, &[A]);
    assert!(asked(&mut page).is_empty());
    press(&mut page, &[A]);
    let row = page.library().history[2];
    let place = page.library().games.iter().position(|game| game.id == row.game).unwrap();
    assert_eq!(page.library().games[place].name, "Copperhead Station");
    assert_eq!(asked(&mut page), [Effect::Launch(place)]);

    // Not once the 300 ms are over, and not across two rows.
    let mut page = self::page(Entry::PlayHistory);
    press(&mut page, &[A]);
    std::thread::sleep(Duration::from_millis(350));
    press(&mut page, &[A, Down, A]);
    assert!(asked(&mut page).is_empty());
    press(&mut page, &[A]);
    assert_eq!(asked(&mut page).len(), 1);
}

#[test]
fn manage_has_its_queue_box_which_does_nothing_yet() {
    let mut page = page(Entry::Queue);
    page.take_effects();
    assert_eq!(after(&mut page, &[A, Right, Up, Down]), "Manage: Queue");
    assert!(page.take_effects().is_empty());
    assert_eq!(page.hints(), hint_bar::NONE);
    assert_eq!(after(&mut page, &[Left]), "Manage: Nav(Manage)");
    assert_eq!(after(&mut page, &[A]), "Manage: Queue");
}

#[test]
fn b_is_home_from_anywhere_on_the_page() {
    let reach: [(Entry, &[Button]); 7] = [
        (Entry::Games, &[]),
        (Entry::Games, &[Left]),
        (Entry::Games, &[Down, Down, Down, Down]),
        (Entry::Games, &[Left, Down, Down]),
        (Entry::PlayHistory, &[Down]),
        (Entry::FullLibrary { filter: None }, &[]),
        (Entry::Queue, &[]),
    ];
    for (entry, presses) in reach {
        let mut page = page(entry);
        press(&mut page, presses);
        page.take_effects();
        press(&mut page, &[B]);
        assert_eq!(page.page(), None, "B from {entry:?} after {presses:?}");
        assert!(page.take_effects().is_empty(), "B is silent, and asks nothing");
        assert_eq!(page.home_focus(), Focus::Recent(0), "Home's rows have focus again");
    }
    // A held B closes nothing.
    let mut page = page(Entry::Games);
    page.press(Press { button: B, repeat: true });
    assert_eq!(page.page(), Some("MyGames"));
}

#[test]
fn a_held_direction_moves_and_a_held_button_does_nothing() {
    let mut page = page(Entry::PlayHistory);
    let held = |button| Press { button, repeat: true };
    page.press(held(Down));
    page.press(held(Down));
    assert_eq!(page.focus_line(), "MyGames: PlayHistory: Row(2)");
    page.press(Press { button: A, repeat: false });
    page.press(held(A));
    assert!(asked(&mut page).is_empty(), "a held A is not a second A");
    let mut page = self::page(Entry::Games);
    page.press(held(Y));
    page.press(held(Right));
    assert_eq!((page.focus_line().as_str(), page.pages_up()), ("MyGames: Games: Tile(1)", 1));
}

#[test]
fn a_new_snapshot_keeps_focus_on_its_game_and_falls_back_when_the_game_has_gone() {
    let mut page = page(Entry::Games);
    assert_eq!(after(&mut page, &[Down, Right, Right]), "Games: Tile(7)");
    // Two of the games before it have been uninstalled: it is the sixth now, and Right is the seventh.
    let uninstalled = ["Ashfall Courier", "Ember Choir"];
    page.set_library(library_after(|game| game.installed &= !uninstalled.contains(&game.name.as_str())));
    assert_eq!(after(&mut page, &[Right]), "Games: Tile(6)");
    // It has been uninstalled itself: the first game.
    page.set_library(library_after(|game| game.installed &= game.name != "Kestrel Run"));
    assert_eq!(after(&mut page, &[Right]), "Games: Tile(1)");
    // With no installed game left, focus is on the nav.
    page.set_library(library_after(|game| game.installed = false));
    assert_eq!(after(&mut page, &[Right]), "Games: Nav(Games)");

    // Play history: focus stays on its game's row when another game has been played since.
    let mut page = self::page(Entry::PlayHistory);
    assert_eq!(after(&mut page, &[Down]), "PlayHistory: Row(1)");
    page.set_library(library_after(|game| {
        if game.name == "Kestrel Run" {
            game.last_played = now();
        }
    }));
    assert_eq!(page.library().history.len(), 8);
    assert_eq!(after(&mut page, &[Up]), "PlayHistory: Row(1)", "its row is the third now");
}

// --- What it looks like ---

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

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(library: Library, entry: Entry, size: (u32, u32)) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, size);
        let mut dashboard = page_on(library, entry);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn press(&mut self, buttons: &[Button]) {
        press(&mut self.dashboard, buttons);
    }

    /// The screen at rest, compared with its reference.
    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }

    /// The screen one frame on, `dt` seconds after the last on the ui's clock: what is mid-flight stays so.
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
        let pixels = read_back(&self.gpu, &self.target.texture, self.target.size);
        image::RgbaImage::from_raw(self.target.size.0, self.target.size.1, pixels).unwrap()
    }
}

/// The Bundle's 02: Games as it opens, focus on the grid's first game, the hint bar up.
#[test]
fn my_games_games() {
    Stage::new(library(), Entry::Games, HD).check("my_games_games");
}

/// The Bundle's 01: focus on the Games nav item, the grid at its top.
#[test]
fn my_games_games_nav_focused() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.press(&[Left]);
    stage.check("my_games_games_nav_focused");
}

/// The Bundle's 03: three rows down, the grid a row up, its last row and "Looking for more?" under it.
#[test]
fn my_games_games_scrolled() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.press(&[Down, Down, Down]);
    stage.check("my_games_games_scrolled");
}

/// The Bundle's 04: focus on SEE FULL LIBRARY.
#[test]
fn my_games_games_see_full_library() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.press(&[Down, Down, Down, Down]);
    stage.check("my_games_games_see_full_library");
}

/// A focused tile with no cover and a title of two lines; a not-installed game is not in Games.
#[test]
fn my_games_games_long_title_focused() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.press(&[Down, Down, Down, Right]);
    stage.check("my_games_games_long_title_focused");
}

#[test]
fn my_games_games_empty() {
    let mut stage = Stage::new(library_of(|game| !game.installed), Entry::Games, HD);
    stage.check("my_games_games_empty");
}

/// The Bundle's 05: Apps, focus on its nav item.
#[test]
fn my_games_apps() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.press(&[Left, Down]);
    stage.check("my_games_apps");
}

/// The Bundle's 06: Play history, focus on its first row, six sections under it.
#[test]
fn my_games_play_history() {
    Stage::new(library(), Entry::PlayHistory, HD).check("my_games_play_history");
}

/// Focus on the last row: the list has come up to its end, the end's padding under it.
#[test]
fn my_games_play_history_scrolled() {
    let mut stage = Stage::new(library(), Entry::PlayHistory, HD);
    stage.press(&[Down; 6]);
    stage.check("my_games_play_history_scrolled");
}

#[test]
fn my_games_play_history_empty() {
    let mut stage = Stage::new(library_of(|game| game.last_played == 0), Entry::PlayHistory, HD);
    stage.check("my_games_play_history_empty");
}

/// The Bundle's 07, less Full library's content, which is #53's: focus on its nav item.
#[test]
fn my_games_full_library_from_the_nav() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.press(&[Left, Down, Down, Down]);
    stage.check("my_games_full_library_from_the_nav");
}

/// The Bundle's 08: Manage, focus on its Queue box.
#[test]
fn my_games_manage() {
    Stage::new(library(), Entry::Queue, HD).check("my_games_manage");
}

#[test]
fn my_games_manage_nav_focused() {
    let mut stage = Stage::new(library(), Entry::Queue, HD);
    stage.press(&[Left]);
    stage.check("my_games_manage_nav_focused");
}

/// On a screen wider than the canvas the page's fill and the nav's fills run to the screen's left edge.
#[test]
fn my_games_ultrawide() {
    let mut stage = Stage::new(library(), Entry::Games, (2560, 1080));
    stage.press(&[Left]);
    stage.check("my_games_ultrawide");
}

/// The rows of the screen, top to bottom, that have anything bright in them between two columns.
fn ink_rows(image: &image::RgbaImage, (left, right): (u32, u32), (top, bottom): (u32, u32)) -> Option<(u32, u32)> {
    let bright = |y: u32| (left..right).any(|x| image.get_pixel(x, y).0[..3].iter().all(|&c| c > 100));
    let rows: Vec<u32> = (top..bottom).filter(|&y| bright(y)).collect();
    Some((*rows.first()?, *rows.last()?))
}

/// docs/spec/my-games.md, 3.3: a pane opened by a step along the nav comes up from 50 below over 120 ms, fading
/// in; one opened by a step up comes down from above; the page opening has no fly-in at all.
#[test]
fn a_pane_opened_from_the_nav_flies_in_and_the_page_itself_does_not() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    let header = |image: &image::RgbaImage| ink_rows(image, (520, 900), (20, 190));
    // The page as it opens: the header at its rest in the first frame.
    let games = header(&stage.frame_after(0.0)).expect("Games' header");
    assert!((72..90).contains(&games.0), "the header's ink starts at {}", games.0);

    stage.press(&[Left]);
    stage.frame_after(0.2);
    rest();
    stage.press(&[Down]);
    // The first frame holds the pane at its start, clear.
    assert_eq!(header(&stage.frame_after(0.0)), None, "Apps' header is not to be seen yet");
    let mid = header(&stage.frame_after(0.04)).expect("Apps' header, on its way");
    let at_rest = header(&stage.frame_after(0.2)).expect("Apps' header, at rest");
    assert!(mid.0 > at_rest.0 + 5 && mid.0 < at_rest.0 + 50, "it comes up from below: {mid:?}, then {at_rest:?}");
    assert!(at_rest.0.abs_diff(games.0) <= 2, "and rests where Games' header did: {at_rest:?}, {games:?}");

    // A step up: from above.
    rest();
    stage.press(&[Up]);
    stage.frame_after(0.0);
    let mid = header(&stage.frame_after(0.04)).expect("Games' header, on its way");
    assert!(mid.0 + 5 < games.0, "it comes down from above: {mid:?}");
    assert_eq!(header(&stage.frame_after(0.2)), Some(games));
}

#[test]
fn under_reduce_motion_a_pane_is_simply_there() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
    let header = |image: &image::RgbaImage| ink_rows(image, (520, 900), (20, 190));
    let games = header(&stage.frame_after(0.0));
    stage.press(&[Left]);
    stage.frame_after(0.2);
    rest();
    stage.press(&[Down]);
    assert_eq!(stage.dashboard.focus_line(), "MyGames: Apps: Nav(Apps)");
    let (apps, games) = (header(&stage.frame_after(0.0)).expect("Apps' header"), games.expect("Games' header"));
    assert!(apps.0.abs_diff(games.0) <= 2, "at its rest in the first frame: {apps:?}, {games:?}");
}

/// The grid slides a row at a time over 300 ms, and jumps under Reduce motion.
#[test]
fn the_grid_slides_to_its_row() {
    let mut stage = Stage::new(library(), Entry::Games, HD);
    // The first game's cover is bright along its left edge: where that edge ends is the foot of its tile, which
    // is the first row's (195.29 to 427.43 with the grid at its top).
    let foot = |image: &image::RgbaImage| {
        let bright = |y: u32| image.get_pixel(528, y).0[..3].iter().all(|&c| c > 100);
        let top = (120..1000).find(|&y| bright(y)).expect("the first tile");
        (top, (top..1000).find(|&y| !bright(y)).expect("its foot"))
    };
    stage.press(&[Right]);
    stage.frame_after(0.0);
    stage.frame_after(0.0);
    let (top, at_top) = foot(&stage.frame_after(0.5));
    assert!((195..=198).contains(&top) && (425..=428).contains(&at_top), "{top} to {at_top}");
    stage.press(&[Down, Down, Down]);
    stage.frame_after(0.0);
    let (top, mid) = foot(&stage.frame_after(0.1));
    assert!(top == 126 && mid > 185 && mid < 420, "the first row is on its way under the list's top: {mid}");
    // At rest a row up: the list's top edge cuts it at 126, and its foot is 247.14 higher than it was.
    let (top, a_row_up) = foot(&stage.frame_after(0.5));
    assert!(top == 126 && (178..=181).contains(&a_row_up), "{top} to {a_row_up}");

    // Under Reduce motion it is there in the next frame.
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
    stage.press(&[Up, Up, Up]);
    assert!((425..=428).contains(&foot(&stage.frame_after(0.0)).1));
}
