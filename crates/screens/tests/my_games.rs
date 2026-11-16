//! My games & apps (docs/spec/my-games.md): the shell, Games, Play history, Full library and Manage. Where focus goes and what a
//! press asks for, with no picture; then the page drawn offscreen and compared with the committed reference images,
//! as tests/shots.rs does for Home (`SHOTS=bless cargo test -p screens` accepts what is drawn now). The Bundle's own
//! pictures of these states are 01 to 14 of docs/spec/pictures.md.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use chrono::TimeZone;
use model::{Game, Library};
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::home::Focus;
use screens::keyboard::Typed;
use screens::my_games::Entry;
use screens::pages::Open;
use screens::{hint_bar, Dashboard, Effect, Sound};
use ui::host::Offscreen;
use ui::Button::{self, Down, Left, LeftBumper, Menu, Right, RightBumper, Up, A, B, X, Y};
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
        source_id: (2000 + i).to_string(),
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
        database.sync("steam", found, now() - 500 * DAY).unwrap();
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

/// Where focus is, without what the page says of its state after it.
fn place(line: &str) -> String {
    line.split("; ").next().unwrap_or_default().to_string()
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
    assert_eq!(place(&after(&mut page(Entry::FullLibrary { filter: None }), &[])), "FullLibrary: Tile(0)");
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
    let full_library = "FullLibrary: Nav(FullLibrary)";
    let lines: Vec<String> = lines.iter().map(|line| place(line)).collect();
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
    assert_eq!(place(&after(&mut page, &[Up])), full_library);
    // Full library's content takes focus; Apps has nothing to focus yet, so Right and A leave focus on its item.
    assert_eq!(place(&after(&mut page, &[Right])), "FullLibrary: Tile(0)");
    assert_eq!(place(&after(&mut page, &[Left])), full_library);
    rest();
    assert_eq!(after(&mut page, &[Up, Up, Right, A]), "Apps: Nav(Apps)");
    rest();
    assert_eq!(after(&mut page, &[Down, Right]), "PlayHistory: Row(0)");
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
    assert_eq!(place(&after(&mut page, &[Up, Up])), "FullLibrary: Nav(PlayHistory)");
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
    // A opens Full library by way of its nav item, focus on its first game.
    page.take_effects();
    assert_eq!(place(&after(&mut page, &[Down, A])), "FullLibrary: Tile(0)");
    assert!(asked(&mut page).is_empty());
    // Games forgot its place when the page moved on: back on it, Right is the first game.
    rest();
    assert_eq!(after(&mut page, &[Left, Up, Up, Up, Right]), "Games: Tile(0)");
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
    let line = page.focus_line();
    assert!(line.starts_with("Search: Key(Q); \"|\" upper letters, RecentlyAdded "), "{line}");
    assert_eq!(page.pages_up(), 2);
    assert!(page.take_effects().is_empty(), "Y is silent");
    // B twice: Search's keyboard goes, then Search.
    press(&mut page, &[B, B]);
    assert_eq!((page.focus_line().as_str(), page.pages_up()), ("MyGames: Games: Tile(6)", 1));
    // Not from the nav, nor from Play history or Manage.
    assert_eq!(after(&mut page, &[Left, Left, Y]), "Games: Nav(Games)");
    assert_eq!(after(&mut page, &[Down, Down, Right, Y]), "PlayHistory: Row(0)");
    assert_eq!(after(&mut page, &[Left, Down, Down, Right, Y]), "Manage: Queue");
    assert_eq!(page.pages_up(), 1);
}

#[test]
fn a_on_a_game_opens_its_game_hub_at_once_and_menu_and_x_open_its_tile_menu() {
    let mut page = page(Entry::Games);
    page.take_effects();
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
    page.take_effects();
    // A: the hub of the game on the tile, over the page; B is the page again, focus on the tile.
    press(&mut page, &[Right, A]);
    let line = page.focus_line();
    assert!(line.starts_with("GameHub: ") && line.ends_with(" Hub Play"), "{line}");
    assert_eq!(page.pages_up(), 2);
    assert!(asked(&mut page).is_empty(), "the grid has no double tap: A opens, and plays nothing");
    assert_eq!(after(&mut page, &[B]), "Games: Tile(1)");
    assert_eq!(page.pages_up(), 1);
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
    // One A waits out the double tap's time before the game's hub opens: nothing yet.
    press(&mut page, &[A]);
    assert!(asked(&mut page).is_empty());
    assert_eq!(page.pages_up(), 1);
    press(&mut page, &[A]);
    assert_eq!(page.pages_up(), 1, "a double tap opens no hub");
    let row = page.library().history[2];
    let place = page.library().games.iter().position(|game| game.id == row.game).unwrap();
    assert_eq!(page.library().games[place].name, "Copperhead Station");
    assert_eq!(asked(&mut page), [Effect::Launch(place)]);

    // Not across two rows.
    let mut page = self::page(Entry::PlayHistory);
    press(&mut page, &[A, Down, A]);
    assert!(asked(&mut page).is_empty());
    press(&mut page, &[A]);
    assert_eq!(asked(&mut page).len(), 1);
}

#[test]
fn one_a_on_a_play_history_row_opens_its_game_hub_when_the_double_taps_time_is_over() {
    let mut page = page(Entry::PlayHistory);
    press(&mut page, &[Down, Down]);
    let row = page.library().history[2];
    let appid = page.library().games.iter().find(|game| game.id == row.game).unwrap().source_id.clone();
    page.take_effects();
    // On the Dashboard's clock, which the test moves on: nothing waits.
    press(&mut page, &[A]);
    page.advance(0.1);
    let wait = page.timer().expect("the hub is 200 ms off");
    assert!((wait - 0.2).abs() < 0.001, "{wait}");
    assert_eq!(page.pages_up(), 1);
    page.advance(0.2);
    assert_eq!(page.timer(), None);
    assert_eq!((page.pages_up(), page.focus_line()), (2, format!("GameHub: {appid} Hub Play")));
    assert!(asked(&mut page).is_empty());
    // A there is the hub's Play; B is Play history again, focus on the row.
    page.advance(1.6);
    press(&mut page, &[B]);
    assert_eq!((page.pages_up(), page.focus_line().as_str()), (1, "MyGames: PlayHistory: Row(2)"));
    // And nothing opens a second time.
    page.advance(1.0);
    assert_eq!((page.timer(), page.pages_up()), (None, 1));
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

// --- Full library ---

/// The library with Groups to make tabs of: three favourites, a Steam collection "My Picks" of six games, then the
/// built-ins Most played (the seven played) and Recently added (every game). Four tabs, in that order.
fn tabbed() -> Library {
    let mut found = source();
    let favourites = ["Brass Meridian", "Kestrel Run", "Quiet Orbit"];
    found.games.iter_mut().for_each(|game| game.favourite = favourites.contains(&game.name.as_str()));
    let picks = [3u32, 0, 7, 12, 2, 18].map(|i| (2000 + i).to_string()).to_vec();
    let collection = model::Collection { source_id: "uc-picks".into(), name: "My Picks".into(), games: picks };
    found.collections = Some(vec![collection]);
    through(&[found])
}

/// A saved filter's id, by its name.
fn filter(library: &Library, name: &str) -> i64 {
    library.filters.iter().find(|filter| filter.name == name).unwrap_or_else(|| panic!("no filter {name}")).id
}

/// Full library on the tab of the filter named so, as See all opens it; or with no tab lit, as the nav does.
fn full_library(tab: Option<&str>) -> Dashboard {
    let library = tabbed();
    let filter = tab.map(|name| self::filter(&library, name));
    let mut dashboard = page_on(library, Entry::FullLibrary { filter });
    dashboard.take_effects();
    dashboard
}

/// The focus line after these presses, without the page's and the item's names.
#[track_caller]
fn on(dashboard: &mut Dashboard, buttons: &[Button]) -> String {
    let line = after(dashboard, buttons);
    line.strip_prefix("FullLibrary: ").unwrap_or_else(|| panic!("Full library is not up: {line}")).to_string()
}

/// What the focus log says of Full library's state: the lit tab of the four, the sort, the name and the count.
fn state(tab: usize, name: &str, sort: &str, typed: &str, games: usize) -> String {
    format!("tab {tab}/4 {name:?}, sort {sort}, name {typed:?}, {games} games")
}

#[test]
fn see_all_opens_full_library_on_its_tab_in_the_filters_own_order() {
    assert_eq!(
        full_library(Some("Favourites")).focus_line(),
        format!("MyGames: FullLibrary: Tile(0); {}", state(1, "Favourites", "Name", "", 3))
    );
    assert_eq!(
        on(&mut full_library(Some("My Picks")), &[]),
        format!("Tile(0); {}", state(2, "My Picks", "Name", "", 6))
    );
    assert_eq!(
        on(&mut full_library(Some("Most played")), &[]),
        format!("Tile(0); {}", state(3, "Most played", "Playtime", "", 7))
    );
    // The nav opens it with no tab lit: every game that is not hidden, by name.
    assert_eq!(on(&mut full_library(None), &[]), format!("Tile(0); {}", state(0, "", "Name", "", 22)));
    // A filter that is not a tab (the empty Favourites Home shows when there is no Group) lights none.
    let mut page = page_on(library(), Entry::FullLibrary { filter: Some(filter(&library(), "Favourites")) });
    assert!(after(&mut page, &[]).ends_with("; tab 0/2 \"\", sort Name, name \"\", 22 games"));
}

#[test]
fn the_tabs_step_with_left_and_right_and_the_bumpers_and_stop_at_their_ends() {
    let mut page = full_library(Some("My Picks"));
    // Up from the grid's first row is the row, on Sort; Up again the tabs, which show nothing of it.
    assert_eq!(place(&on(&mut page, &[Right, Up])), "Sort");
    assert_eq!(page.hints(), hint_bar::NONE);
    assert_eq!(on(&mut page, &[Up]), format!("Tabs; {}", state(2, "My Picks", "Name", "", 6)));
    assert_eq!(on(&mut page, &[Up, A]), format!("Tabs; {}", state(2, "My Picks", "Name", "", 6)));
    // Right lights the next, its games in its own order, silently: focus did not move.
    page.take_effects();
    assert_eq!(on(&mut page, &[Right]), format!("Tabs; {}", state(3, "Most played", "Playtime", "", 7)));
    assert!(page.take_effects().is_empty(), "a tab switch is silent");
    assert_eq!(on(&mut page, &[Right, Right]), format!("Tabs; {}", state(4, "Recently added", "Added", "", 22)));
    // Left back to the first, and Left there is the nav.
    assert_eq!(on(&mut page, &[Left, Left, Left]), format!("Tabs; {}", state(1, "Favourites", "Name", "", 3)));
    assert_eq!(place(&on(&mut page, &[Left])), "Nav(FullLibrary)");
    // Right from the nav is the grid's first game: another tab's grid starts again at its top.
    assert_eq!(place(&on(&mut page, &[Right])), "Tile(0)");
    // LB and RB from anywhere on the page, the nav included; they stop at the ends.
    assert_eq!(
        on(&mut page, &[RightBumper, RightBumper]),
        format!("Tile(0); {}", state(3, "Most played", "Playtime", "", 7))
    );
    assert_eq!(
        on(&mut page, &[Left, LeftBumper]),
        format!("Nav(FullLibrary); {}", state(2, "My Picks", "Name", "", 6))
    );
    assert_eq!(
        on(&mut page, &[LeftBumper, LeftBumper]),
        format!("Nav(FullLibrary); {}", state(1, "Favourites", "Name", "", 3))
    );
    assert_eq!(
        on(&mut page, &[Right, RightBumper, RightBumper, RightBumper, RightBumper]),
        format!("Tile(0); {}", state(4, "Recently added", "Added", "", 22))
    );

    // With no tab lit, Right on the tabs is nothing and Left the nav; RB lights the first.
    let mut page = full_library(None);
    assert_eq!(on(&mut page, &[Up, Up, Right]), format!("Tabs; {}", state(0, "", "Name", "", 22)));
    assert_eq!(on(&mut page, &[LeftBumper]), format!("Tabs; {}", state(0, "", "Name", "", 22)));
    assert_eq!(on(&mut page, &[RightBumper]), format!("Tabs; {}", state(1, "Favourites", "Name", "", 3)));
    let mut page = full_library(None);
    assert_eq!(place(&on(&mut page, &[Up, Up, Left])), "Nav(FullLibrary)");
}

#[test]
fn the_row_leads_to_the_tabs_the_grid_and_the_nav_and_remembers_where_it_was() {
    let mut page = full_library(Some("Recently added"));
    assert_eq!(place(&on(&mut page, &[Down, Right, Up])), "Tile(1)");
    assert_eq!(place(&on(&mut page, &[Up])), "Sort");
    assert_eq!(place(&on(&mut page, &[Right])), "Filter");
    assert_eq!(place(&on(&mut page, &[Right])), "Filter", "Right at Filter is nothing");
    // Down is the game the grid was left from; Up from its first row is Filter again, where the row was left.
    assert_eq!(place(&on(&mut page, &[Down])), "Tile(1)");
    assert_eq!(place(&on(&mut page, &[Up])), "Filter");
    assert_eq!(place(&on(&mut page, &[Up, Down])), "Filter", "down from the tabs is the row as it was left");
    assert_eq!(place(&on(&mut page, &[Left])), "Sort");
    assert_eq!(place(&on(&mut page, &[Left])), "Nav(FullLibrary)");
    // Right from the nav is the game it was left from. Down onto a shorter last row is its last game, and nothing
    // is under that.
    assert_eq!(place(&on(&mut page, &[Right])), "Tile(1)");
    assert_eq!(place(&on(&mut page, &[Down, Down, Down])), "Tile(16)");
    assert_eq!(place(&on(&mut page, &[Down, Down])), "Tile(21)");
    assert_eq!(page.hints(), hint_bar::MORE_OPTIONS, "Y opens the name box, which is not hinted");
    // Menu and X open the game's Tile menu; B comes back to it.
    press(&mut page, &[Menu]);
    assert!(page.focus_line().starts_with("TileMenu: Menu("), "{}", page.focus_line());
    assert_eq!(place(&on(&mut page, &[B])), "Tile(21)");
}

#[test]
fn the_sort_list_orders_every_tab_until_the_page_closes() {
    let mut page = full_library(Some("Most played"));
    // It opens on the option the grid is in, takes every button, and stops at its ends.
    assert_eq!(place(&on(&mut page, &[Up, A])), "SortList(Playtime)");
    assert_eq!(page.hints(), hint_bar::NONE);
    assert_eq!(place(&on(&mut page, &[Down, Left, Right, Y, RightBumper])), "SortList(Playtime)");
    assert_eq!(on(&mut page, &[]), format!("SortList(Playtime); {}", state(3, "Most played", "Playtime", "", 7)));
    // B closes it, focus on Sort, nothing chosen.
    assert_eq!(place(&on(&mut page, &[B])), "Sort");
    assert_eq!(page.page(), Some("MyGames"));
    // A chooses: the grid is in that order, and focus is on Sort.
    assert_eq!(place(&on(&mut page, &[A, Up, Up, Up, Up])), "SortList(Name)");
    assert_eq!(on(&mut page, &[Down, A]), format!("Sort; {}", state(3, "Most played", "LastPlayed", "", 7)));
    // It orders the other tabs too, and the nav's way there and back keeps it.
    assert_eq!(on(&mut page, &[RightBumper]), format!("Sort; {}", state(4, "Recently added", "LastPlayed", "", 22)));
    rest();
    press(&mut page, &[Left, Up]);
    rest();
    assert_eq!(on(&mut page, &[Down, Right]), format!("Tile(0); {}", state(4, "Recently added", "LastPlayed", "", 22)));
    // The page closed and opened again has forgotten it.
    press(&mut page, &[B]);
    assert_eq!(page.page(), None);
    page.open_page(Open::MyGames(Entry::FullLibrary { filter: Some(filter(page.library(), "Recently added")) }));
    assert_eq!(on(&mut page, &[]), format!("Tile(0); {}", state(4, "Recently added", "Added", "", 22)));
}

#[test]
fn the_name_box_opens_on_y_and_narrows_the_grid_through_text_entry() {
    let mut page = full_library(Some("Recently added"));
    // Y on a game opens the box, empty: nothing is narrowed yet.
    page.take_effects();
    assert_eq!(on(&mut page, &[Right, Y]), format!("Name; {}", state(4, "Recently added", "Added", "", 22)));
    assert_eq!(page.pages_up(), 1);
    // A on it: text entry over the page, from the name as it is.
    press(&mut page, &[A]);
    assert_eq!(page.pages_up(), 2);
    assert!(page.focus_line().starts_with("TextEntry: "), "{}", page.focus_line());
    // The grid narrows as it is typed, whatever the case.
    page.typed(Typed::Text("IN"));
    press(&mut page, &[Menu]);
    // Kept, a new name that finds games: focus on the first of them.
    assert_eq!(on(&mut page, &[]), format!("Tile(0); {}", state(4, "Recently added", "Added", "IN", 6)));
    // The tab and the sort leave the name as it is.
    assert_eq!(on(&mut page, &[LeftBumper]), format!("Tile(0); {}", state(3, "Most played", "Playtime", "IN", 2)));
    // B with a name set clears it; focus stays on its game, Fathom Nine, the sixth of the whole tab. B again is
    // Home.
    assert_eq!(on(&mut page, &[B]), format!("Tile(5); {}", state(3, "Most played", "Playtime", "", 7)));
    press(&mut page, &[B]);
    assert_eq!(page.page(), None);
}

#[test]
fn the_name_box_keeps_focus_when_text_entry_finds_nothing_and_b_there_clears_it() {
    let mut page = full_library(None);
    // A on Filter opens the box too.
    assert_eq!(place(&on(&mut page, &[Up, Right, A])), "Name");
    // Dropped: the name as it was, focus on the box still.
    press(&mut page, &[Y]);
    page.typed(Typed::Text("orbit"));
    assert!(page.focus_line().starts_with("TextEntry: "), "{}", page.focus_line());
    press(&mut page, &[B]);
    assert_eq!(on(&mut page, &[]), format!("Name; {}", state(0, "", "Name", "", 22)));
    // Kept with a name that finds nothing: focus on the box, the grid empty, and Down goes nowhere.
    press(&mut page, &[A]);
    page.typed(Typed::Text("zzz"));
    press(&mut page, &[Menu]);
    assert_eq!(on(&mut page, &[Down]), format!("Name; {}", state(0, "", "Name", "zzz", 0)));
    // The box holds its name as focus leaves it, and Right from Sort comes back to it.
    assert_eq!(place(&on(&mut page, &[Left, Right])), "Name");
    // B in the box clears the name and closes it, focus on Filter; Left from Filter is then Sort.
    assert_eq!(on(&mut page, &[B]), format!("Filter; {}", state(0, "", "Name", "", 22)));
    assert_eq!(place(&on(&mut page, &[Left])), "Sort");
    // Left empty, the box closes as focus leaves it.
    assert_eq!(place(&on(&mut page, &[Y, Left, Right])), "Filter");
    // The name goes when the page moves to another item.
    press(&mut page, &[Y, A]);
    page.typed(Typed::Text("orbit"));
    press(&mut page, &[Menu]);
    assert_eq!(on(&mut page, &[]), format!("Tile(0); {}", state(0, "", "Name", "orbit", 1)));
    press(&mut page, &[Left, Up]);
    rest();
    press(&mut page, &[Down]);
    rest();
    assert_eq!(on(&mut page, &[Right]), format!("Tile(0); {}", state(0, "", "Name", "", 22)));
}

#[test]
fn full_librarys_focus_follows_its_game_when_the_library_changes_and_a_tab_that_goes_goes_dark() {
    let mut page = full_library(Some("Favourites"));
    assert_eq!(place(&on(&mut page, &[Right, Right])), "Tile(2)");
    // Quiet Orbit is still a favourite, the others not: the tab has one game, focus on it.
    let mut found = source();
    found.games.iter_mut().for_each(|game| game.favourite = game.name == "Quiet Orbit");
    page.set_library(through(&[found]));
    // (A opens the game's hub and B closes it: focus is back where it was.)
    assert_eq!(on(&mut page, &[A, B]), "Tile(0); tab 1/3 \"Favourites\", sort Name, name \"\", 1 games");
    // No favourite at all: the tab is gone, no tab is lit, and the grid is every game, focus on the same game.
    page.set_library(library());
    assert_eq!(on(&mut page, &[A, B]), "Tile(15); tab 0/2 \"\", sort Name, name \"\", 22 games");
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

/// The Bundle's 07: Full library as the nav opens it, focus on its nav item, no tab lit, every game by name.
#[test]
fn my_games_full_library_from_the_nav() {
    let mut stage = Stage::new(tabbed(), Entry::Games, HD);
    stage.press(&[Left, Down, Down, Down]);
    stage.check("my_games_full_library_from_the_nav");
}

/// Full library as See all on a Group opens it, on that Group's tab.
fn see_all(tab: &str) -> Stage {
    let library = tabbed();
    let filter = Some(filter(&library, tab));
    Stage::new(library, Entry::FullLibrary { filter }, HD)
}

/// The Bundle's 09: See all on "My Picks", focus on the tabs, which shows nothing.
#[test]
fn my_games_full_library_tabs() {
    let mut stage = see_all("My Picks");
    stage.press(&[Up, Up]);
    stage.check("my_games_full_library_tabs");
}

/// The Bundle's 10: Right on the tabs, the next tab lit, a sort the Sort list does not have ("Sort").
#[test]
fn my_games_full_library_another_tab() {
    let mut stage = see_all("My Picks");
    stage.press(&[Up, Up, Right]);
    stage.check("my_games_full_library_another_tab");
}

/// The Bundle's 11: the Sort list open over the grid, on the option the grid is in.
#[test]
fn my_games_full_library_sort_list() {
    let mut stage = see_all("Recently added");
    stage.press(&[Up, A, Up]);
    stage.check("my_games_full_library_sort_list");
}

/// A sort chosen: "Sorted by" it, and the grid in its order.
#[test]
fn my_games_full_library_sorted() {
    let mut stage = see_all("Recently added");
    stage.press(&[Up, A, Up, A, Down]);
    stage.check("my_games_full_library_sorted");
}

/// The Bundle's 12: the name box open and empty, focus on it.
#[test]
fn my_games_full_library_name_box() {
    let mut stage = see_all("Recently added");
    stage.press(&[Y]);
    stage.check("my_games_full_library_name_box");
}

/// The Bundle's 13: text entry over the page, "In" typed, the grid narrowed behind it.
#[test]
fn my_games_full_library_name_entry() {
    let mut stage = see_all("Recently added");
    stage.press(&[Y, A]);
    stage.dashboard.typed(Typed::Text("In"));
    stage.check("my_games_full_library_name_entry");
}

/// The Bundle's 14: the name kept, focus on the narrowed grid's first game.
#[test]
fn my_games_full_library_name_kept() {
    let mut stage = see_all("Recently added");
    stage.press(&[Y, A]);
    stage.dashboard.typed(Typed::Text("In"));
    stage.press(&[Menu]);
    stage.check("my_games_full_library_name_kept");
}

/// A name that finds nothing: the grid's empty line, focus on the box, "0 games".
#[test]
fn my_games_full_library_nothing_found() {
    let mut stage = see_all("Recently added");
    stage.press(&[Y, A]);
    stage.dashboard.typed(Typed::Text("Quiet Orbit of the Long Winter"));
    stage.press(&[Menu]);
    stage.check("my_games_full_library_nothing_found");
}

/// Scrolled to the grid's last row: the rows above pass under the fade and are cut under the Sort row.
#[test]
fn my_games_full_library_scrolled() {
    let mut stage = see_all("Recently added");
    stage.press(&[Down, Down, Down, Right]);
    stage.check("my_games_full_library_scrolled");
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

/// Art downloaded while the Dashboard runs (docs/sources.md, "Art"): a game given a cover in a new snapshot shows it
/// fading in over a quarter of a second, over its name; a cover it had from the first is simply there.
#[test]
fn a_cover_given_while_on_screen_fades_in_over_the_name() {
    let differ = |a: &image::RgbaImage, b: &image::RgbaImage| {
        a.pixels().zip(b.pixels()).filter(|(a, b)| a.0.iter().zip(b.0).any(|(&x, y)| x.abs_diff(y) > 8)).count()
    };
    let mut bare = source();
    bare.games.iter_mut().for_each(|game| game.cover = None);
    let mut stage = Stage::new(through(&[bare]), Entry::Games, HD);
    let without = stage.frame_after(0.5);
    stage.dashboard.set_library(library());
    let given = stage.frame_after(0.0);
    assert!(differ(&given, &without) < 500, "nothing yet in the frame it is given: {}", differ(&given, &without));
    let mid = stage.frame_after(0.1);
    let whole = stage.frame_after(0.3);
    assert_eq!(differ(&whole, &stage.frame_after(0.3)), 0, "whole after a quarter of a second");
    let (from_without, from_whole) = (differ(&mid, &without), differ(&mid, &whole));
    assert!(from_without > 5000 && from_whole > 5000, "on its way: {from_without} from none, {from_whole} from whole");
    // The covers a page opens with are there in its first frame (once decoded), with no fade.
    drop(stage);
    let mut fresh = Stage::new(library(), Entry::Games, HD);
    assert_eq!(differ(&fresh.frame_after(0.0), &whole), 0, "a cover there from the first is whole at once");
}

/// docs/spec/my-games.md, 3.4: across the tabs only the grid moves, in from 50 on the side of the tab lit, over
/// 120 ms; the tabs and the row stay put. Under Reduce motion it is simply there.
#[test]
fn a_tab_switch_flies_the_grid_in_from_its_side() {
    // The first tile's bright left edge, found along the first row's middle.
    let edge = |image: &image::RgbaImage| (450..700).find(|&x| image.get_pixel(x, 466).0[..3].iter().all(|&c| c > 200));
    // The Sort box's left edge, which does not move.
    let sort = |image: &image::RgbaImage| (480..700).find(|&x| image.get_pixel(x, 292).0[0] > 0x28);
    // My Picks, then Most played: both start with Ashfall Courier, which has a cover.
    let mut stage = see_all("My Picks");
    let at_rest = stage.frame_after(0.5);
    let (rest_edge, rest_sort) = (edge(&at_rest).expect("the first tile"), sort(&at_rest).expect("Sort"));
    stage.press(&[RightBumper]);
    let start = stage.frame_after(0.0);
    let start_edge = edge(&start).expect("the first tile, starting out");
    assert!(start_edge.abs_diff(rest_edge + 50) <= 2, "from 50 right: {start_edge}, at rest {rest_edge}");
    assert_eq!(sort(&start), Some(rest_sort), "the row stays put");
    let mid = edge(&stage.frame_after(0.04)).expect("on its way");
    assert!(mid > rest_edge + 2 && mid < start_edge, "on its way: {mid}");
    assert_eq!(edge(&stage.frame_after(0.2)), Some(rest_edge));
    // Back a tab: from the left.
    stage.press(&[LeftBumper]);
    let start_edge = edge(&stage.frame_after(0.0)).expect("the first tile, from the left");
    assert!(start_edge.abs_diff(rest_edge - 50) <= 2, "from 50 left: {start_edge}");
    stage.frame_after(0.2);
    // Under Reduce motion it is there at once.
    stage.dashboard.set_options(screens::Options { reduce_motion: true, ..Default::default() });
    stage.press(&[RightBumper]);
    assert_eq!(edge(&stage.frame_after(0.0)), Some(rest_edge));
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
