//! Search (docs/spec/search.md, sections 1 to 6): every way in and where B goes, typing and what is found, the
//! keyboard put away and brought back, a result's Game hub. Judged by the focus log's line, as a test on the VM
//! judges; then the page drawn offscreen and compared with the committed reference images (`SHOTS=bless cargo test
//! -p screens` accepts what is drawn now). The Bundle's own pictures of these states are 15 to 18 of
//! docs/spec/pictures.md. What is found and in what order is unit-tested beside the page (`search/find.rs`).
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::home::{Focus, TopButton};
use screens::keyboard::Typed;
use screens::my_games;
use screens::pages::Open;
use screens::{Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Left, LeftTrigger, Menu, Right, Up, A, B, X, Y};
use ui::{Host, Press};

/// Art for the games, drawn once: a cover each, in its own colours with bands across it, so a crop shows.
fn art_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("search-art");
        std::fs::create_dir_all(&dir).unwrap();
        for game in 0..GAMES.len() as u32 {
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
            shots::save_shared(&image, &dir.join(format!("{game}.png")));
        }
        dir
    })
}

/// Fourteen made-up games and a fifteenth the user has hidden: name, whether it was played, whether it has a cover.
/// "Ha" finds two of them, as it finds two in the Bundle's picture, and the hidden one would be a third.
const GAMES: [(&str, bool, bool); 15] = [
    ("Ashfall Courier", true, true),
    ("Brass Meridian", true, true),
    ("Copperhead Station", false, true),
    ("Dune Cartographer", false, true),
    ("Ember Choir", true, true),
    ("Fathom Nine", false, true),
    ("Glasswing Atlas", false, false),
    ("Hadal Lantern", true, true),
    ("Halcyon Drift", false, true),
    ("Ironwood Pier", false, true),
    ("Juniper Static", false, true),
    ("Kestrel Run", false, false),
    ("Lanternfish Deep", false, true),
    ("Marrow Street", true, true),
    ("Harrow Moor", true, true),
];
/// The one that is hidden.
const HIDDEN: &str = "Harrow Moor";

/// The games as the library database has them, all added at the same moment, the hidden one hidden.
fn library() -> Library {
    let games = GAMES.iter().enumerate().map(|(i, &(name, played, cover))| Game {
        source_id: (3000 + i).to_string(),
        name: name.into(),
        last_played: if played { 1_790_000_000 - i as u64 * 86_400 } else { 0 },
        playtime_minutes: if played { 60 } else { 0 },
        installed: true,
        cover: cover.then(|| art_dir().join(format!("{i}.png"))),
        ..Game::default()
    });
    let found = Library { user: Some("Player One".into()), games: games.collect(), ..Library::default() };
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &found, 1_780_000_000).unwrap();
    let hidden = database.snapshot().unwrap().games.into_iter().find(|game| game.name == HIDDEN).unwrap();
    database.set_hidden(hidden.id, true).unwrap();
    database.snapshot().unwrap()
}

/// Search, opened from Home with Y.
fn search() -> Dashboard {
    let mut dashboard = Dashboard::new(library());
    press(&mut dashboard, &[Y]);
    dashboard.take_effects();
    dashboard
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
    line.strip_prefix("Search: ").unwrap_or_else(|| panic!("Search is not up: {line}")).to_string()
}

fn sounds(dashboard: &mut Dashboard) -> Vec<Sound> {
    let sound = |effect| match effect {
        Effect::Sound(sound) => Some(sound),
        _ => None,
    };
    dashboard.take_effects().into_iter().filter_map(sound).collect()
}

/// "Ha" typed by walking the keys from Q, as the Bundle's picture 16 has it: H is five along the second row, and A
/// is back at its start, where focus ends.
const HA: [Button; 13] = [Down, Right, Right, Right, Right, Right, A, Left, Left, Left, Left, Left, A];

/// Search as it opens on this library.
const OPENED: &str = "Key(Q); \"|\" upper letters, RecentlyAdded 12";

// --- Ways in, and where B goes ---

#[test]
fn search_opens_empty_on_q_with_recently_added() {
    assert_eq!(after(&mut search(), &[]), OPENED);
}

#[test]
fn b_puts_the_keyboard_away_first_then_leaves_for_home() {
    let mut dashboard = search();
    press(&mut dashboard, &HA);
    // The text stays, the caret goes, and focus is on the first game found.
    assert_eq!(after(&mut dashboard, &[B]), "Games(0) Hadal Lantern; \"Ha\" away, Games 2");
    assert_eq!(dashboard.pages_up(), 1);
    press(&mut dashboard, &[B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Recent(0)));
    // Nothing is kept: Search opens empty again.
    assert_eq!(after(&mut dashboard, &[Y]), OPENED);
    // With nothing found, the keyboard away leaves focus on the box.
    assert!(dashboard.typed(Typed::Text("zzz")));
    assert_eq!(after(&mut dashboard, &[B]), "Box; \"zzz\" away, no results");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), None);
}

#[test]
fn the_top_bars_search_button_opens_search_and_b_comes_back_to_it() {
    let mut dashboard = Dashboard::new(library());
    press(&mut dashboard, &[Right, Right]);
    assert_eq!(dashboard.home_focus(), Focus::TopBar(TopButton::Search));
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), format!("Search: {OPENED}"));
    press(&mut dashboard, &[B, B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::TopBar(TopButton::Search)));
}

#[test]
fn y_on_home_opens_search_and_b_gives_focus_back_to_homes_rows() {
    let mut dashboard = Dashboard::new(library());
    press(&mut dashboard, &[Down, Right, Right]);
    assert_eq!(after(&mut dashboard, &[Y]), OPENED);
    press(&mut dashboard, &[B, B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Recent(2)));
}

#[test]
fn y_on_a_game_in_my_games_opens_search_over_it_and_b_comes_back_to_the_game() {
    let mut dashboard = Dashboard::new(library());
    dashboard.open_page(Open::MyGames(my_games::Entry::Games));
    press(&mut dashboard, &[Right]);
    let on_game = dashboard.focus_line();
    assert_eq!(on_game, "MyGames: Games: Tile(1)");
    assert_eq!(after(&mut dashboard, &[Y]), OPENED);
    assert_eq!(dashboard.pages_up(), 2, "over My games & apps");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), Some("Search"));
    press(&mut dashboard, &[B]);
    assert_eq!((dashboard.focus_line(), dashboard.pages_up()), (on_game, 1));
}

#[test]
fn the_guides_search_utility_opens_search_and_b_is_home() {
    let mut dashboard = Dashboard::new(library());
    press(&mut dashboard, &[Down, Right, Guide]);
    // Search is the third Utility, under the landing tab's six entries.
    press(&mut dashboard, &[Down, Down, Down, Down, Down, Down, Right, Right, A]);
    assert!(!dashboard.guide_open());
    assert_eq!(after(&mut dashboard, &[]), OPENED);
    press(&mut dashboard, &[B, B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, Focus::Recent(1)));
}

#[test]
fn y_on_search_is_the_space_bar_and_never_opens_search_again() {
    let mut dashboard = search();
    assert_eq!(after(&mut dashboard, &[Y]), "Key(Q); \" |\" lower letters, RecentlyAdded 12");
    // With the keyboard away Y does nothing at all.
    assert_eq!(after(&mut dashboard, &[B, Y]), "RecentlyAdded(0) Ashfall Courier; \" \" away, RecentlyAdded 12");
    assert_eq!(dashboard.pages_up(), 1);
}

// --- Typing, and what is found ---

#[test]
fn typing_on_the_keys_finds_games_at_once_and_focus_stays_on_the_keys() {
    let mut dashboard = search();
    // "H" is in eight names; "Ha" in two, and in the hidden game's.
    assert_eq!(after(&mut dashboard, &HA[..7]), "Key(H); \"H|\" lower letters, Games 8");
    assert_eq!(after(&mut dashboard, &HA[7..]), "Key(A); \"Ha|\" lower letters, Games 2");
    // Backspace takes it back to one letter, then to nothing: Recently added again.
    assert_eq!(after(&mut dashboard, &[X]), "Key(A); \"H|\" lower letters, Games 8");
    assert_eq!(after(&mut dashboard, &[X]), "Key(A); \"|\" lower letters, RecentlyAdded 12");
    // Only spaces is nothing typed.
    assert_eq!(after(&mut dashboard, &[Y, Y]), "Key(A); \"  |\" lower letters, RecentlyAdded 12");
}

#[test]
fn the_name_starting_with_the_text_comes_first_and_hidden_games_are_never_found() {
    let mut dashboard = search();
    // "a": names starting with it, then a word starting with it, then the rest, each by name.
    assert!(dashboard.typed(Typed::Text("a")));
    press(&mut dashboard, &[B]);
    let mut found = Vec::new();
    for _ in 0..GAMES.len() {
        found.push(dashboard.focus_line());
        press(&mut dashboard, &[Right]);
    }
    found.dedup();
    let names: Vec<&str> =
        found.iter().map(|line| line.split_once(") ").unwrap().1.split(';').next().unwrap()).collect();
    assert_eq!(
        names,
        [
            "Ashfall Courier",
            "Glasswing Atlas",
            "Brass Meridian",
            "Copperhead Station",
            "Dune Cartographer",
            "Fathom Nine",
            "Hadal Lantern",
            "Halcyon Drift",
            "Juniper Static",
            "Lanternfish Deep",
            "Marrow Street",
        ]
    );
    assert!(!names.contains(&HIDDEN));
    // The hidden game is in no row, though its name starts with what is typed.
    press(&mut dashboard, &[Up, A, X]);
    assert!(dashboard.typed(Typed::Text("Harrow")));
    assert_eq!(after(&mut dashboard, &[]), "Key(Q); \"Harrow|\" lower letters, no results");
}

#[test]
fn recently_added_shows_while_nothing_is_typed_twelve_at_most() {
    let mut dashboard = search();
    press(&mut dashboard, &[B]);
    // All fourteen came at once, so by name; the hidden one is not among them; the row stops at twelve.
    let mut last = String::new();
    for _ in 0..15 {
        last = after(&mut dashboard, &[Right]);
    }
    assert_eq!(last, "RecentlyAdded(11) Kestrel Run; \"\" away, RecentlyAdded 12");
}

#[test]
fn a_physical_keyboard_types_with_the_keyboard_up_or_away() {
    let mut dashboard = search();
    assert!(dashboard.typed(Typed::Text("hal")));
    assert_eq!(after(&mut dashboard, &[]), "Key(Q); \"hal|\" lower letters, Games 1");
    press(&mut dashboard, &[B]);
    assert!(dashboard.typed(Typed::Backspace));
    assert_eq!(after(&mut dashboard, &[]), "Games(0) Hadal Lantern; \"ha\" away, Games 2");
}

// --- Moving about ---

#[test]
fn focus_goes_from_the_keys_up_to_the_results_and_the_box_and_down_again() {
    let mut dashboard = search();
    press(&mut dashboard, &HA);
    // Up from the keys' top row is the last row of results; from there Up is the box.
    assert_eq!(after(&mut dashboard, &[Up, Up]), "Key(1); \"Ha|\" lower letters, Games 2");
    assert_eq!(after(&mut dashboard, &[Up]), "Games(0) Hadal Lantern; \"Ha|\" lower letters, Games 2");
    assert_eq!(after(&mut dashboard, &[Right, Right]), "Games(1) Halcyon Drift; \"Ha|\" lower letters, Games 2");
    assert_eq!(after(&mut dashboard, &[Up]), "Box; \"Ha|\" lower letters, Games 2");
    assert_eq!(after(&mut dashboard, &[Left, Right, Up]), "Box; \"Ha|\" lower letters, Games 2");
    // Down is the tile the row last had, then the key the keys last had.
    assert_eq!(after(&mut dashboard, &[Down]), "Games(1) Halcyon Drift; \"Ha|\" lower letters, Games 2");
    assert_eq!(after(&mut dashboard, &[Down]), "Key(1); \"Ha|\" lower letters, Games 2");
    // The pad's shortcuts type wherever focus is while the keyboard is up; typing from a tile puts focus back on
    // the keys, as the rows are replaced under it.
    press(&mut dashboard, &[Up]);
    assert_eq!(after(&mut dashboard, &[X]), "Key(1); \"H|\" lower letters, Games 8");
}

#[test]
fn menu_and_the_enter_key_put_the_keyboard_away_and_a_on_the_box_brings_it_back_where_it_was() {
    let mut dashboard = search();
    press(&mut dashboard, &HA);
    assert_eq!(after(&mut dashboard, &[Menu]), "Games(0) Hadal Lantern; \"Ha\" away, Games 2");
    // With it away the shortcuts and LT do nothing.
    assert_eq!(after(&mut dashboard, &[X, Y, Menu, LeftTrigger]), "Games(0) Hadal Lantern; \"Ha\" away, Games 2");
    // Down from the last row is nothing with the keyboard away.
    assert_eq!(after(&mut dashboard, &[Down]), "Games(0) Hadal Lantern; \"Ha\" away, Games 2");
    assert_eq!(after(&mut dashboard, &[Up, A]), "Key(A); \"Ha|\" lower letters, Games 2");
    // The Enter key: Down to the third row, then along it past its end.
    press(&mut dashboard, &[Down]);
    press(&mut dashboard, &[Right; 10]);
    assert_eq!(after(&mut dashboard, &[]), "Key(Enter); \"Ha|\" lower letters, Games 2");
    assert_eq!(after(&mut dashboard, &[A]), "Games(0) Hadal Lantern; \"Ha\" away, Games 2");
}

#[test]
fn a_on_a_result_opens_its_game_hub_at_once_and_b_comes_back_to_it() {
    let mut dashboard = search();
    press(&mut dashboard, &HA);
    press(&mut dashboard, &[B, Right]);
    dashboard.take_effects();
    press(&mut dashboard, &[A]);
    // At once: no double tap's wait, and a second A is the hub's own Play.
    let hub = dashboard.focus_line();
    assert_eq!(hub, format!("GameHub: {} Hub Play", 3008));
    assert_eq!(dashboard.pages_up(), 2, "the hub is over Search");
    assert_eq!(dashboard.take_effects(), [Effect::Sound(Sound::Activate)]);
    assert_eq!(after(&mut dashboard, &[B]), "Games(1) Halcyon Drift; \"Ha\" away, Games 2");
    // With the keyboard up too, from a tile reached by Up from the keys.
    press(&mut dashboard, &[Up, A, Up, Up, Up]);
    assert_eq!(after(&mut dashboard, &[]), "Games(1) Halcyon Drift; \"Ha|\" lower letters, Games 2");
    press(&mut dashboard, &[A]);
    assert!(dashboard.focus_line().starts_with("GameHub: 3008 "));
    assert_eq!(after(&mut dashboard, &[B]), "Games(1) Halcyon Drift; \"Ha|\" lower letters, Games 2");
}

#[test]
fn moving_sounds_and_typing_is_silent() {
    let mut dashboard = search();
    press(&mut dashboard, &[Right]);
    assert_eq!(sounds(&mut dashboard), [Sound::Move]);
    press(&mut dashboard, &[A, X, Y]);
    assert_eq!(sounds(&mut dashboard), []);
    // An edge is silent too.
    press(&mut dashboard, &[Down, Down, Down, Down]);
    sounds(&mut dashboard);
    press(&mut dashboard, &[Down]);
    assert_eq!(sounds(&mut dashboard), []);
}

#[test]
fn a_new_library_while_the_hub_is_up_leaves_focus_on_the_same_game() {
    let mut dashboard = search();
    press(&mut dashboard, &HA);
    press(&mut dashboard, &[B, Right, A]);
    // The first game found leaves the library: the second is now the first.
    let mut library = dashboard.library().clone();
    library.games.retain(|game| game.name != "Hadal Lantern");
    dashboard.set_library(library);
    assert_eq!(after(&mut dashboard, &[B]), "Games(0) Halcyon Drift; \"Ha\" away, Games 1");
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
    /// Search over Home, opened with Y.
    fn new(size: (u32, u32)) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, size);
        let mut dashboard = search();
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn press(&mut self, buttons: &[Button]) -> &mut Self {
        press(&mut self.dashboard, buttons);
        self
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
}

/// The Bundle's 15: nothing typed, Recently added, the keys with Shift on and focus on Q.
#[test]
fn search_empty() {
    Stage::new(HD).check("search_empty");
}

/// The Bundle's 16: "Ha" typed on the keys, the two games found, focus on the a key.
#[test]
fn search_typed() {
    Stage::new(HD).press(&HA).check("search_typed");
}

/// The Bundle's 17: the symbols, focus on their key.
#[test]
fn search_symbols() {
    let mut stage = Stage::new(HD);
    stage.press(&HA).press(&[Up, Up, Left, A]);
    assert_eq!(stage.dashboard.focus_line(), "Search: Key(Symbols); \"Ha|\" lower symbols, Games 2");
    stage.check("search_symbols");
}

/// The Bundle's 18: the keyboard away, focus on the first game found.
#[test]
fn search_keyboard_away() {
    Stage::new(HD).press(&HA).press(&[B]).check("search_keyboard_away");
}

/// Nothing found: no row, and nothing in its place.
#[test]
fn search_no_results() {
    let mut stage = Stage::new(HD);
    assert!(stage.dashboard.typed(Typed::Text("Hazy")));
    stage.check("search_no_results");
}

/// The search box with focus, the keyboard away, and a result row that has slid along a tile.
#[test]
fn search_box_focused() {
    let mut stage = Stage::new(HD);
    stage.press(&[B, Right, Right, Right, Right, Right, Right, Right]);
    assert_eq!(stage.dashboard.focus_line(), "Search: RecentlyAdded(7) Hadal Lantern; \"\" away, RecentlyAdded 12");
    stage.check("search_row_slid");
    stage.press(&[Up]).check("search_box_focused");
}

/// A screen taller than 16:9: the keyboard stays 110 off the foot and the results take the height.
#[test]
fn search_16_10() {
    Stage::new((1920, 1200)).press(&HA).check("search_16_10");
}
