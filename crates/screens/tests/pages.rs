//! The pages over Home: every way into one, and where B on it leaves focus (docs/spec/home.md, 7.4;
//! docs/spec/my-games.md, 3; docs/spec/search.md, 4; docs/spec/settings.md, 8). What each page does once it is
//! open is its own file's: tests/my_games.rs, tests/search.rs, tests/settings.rs. No picture, so no GPU; what the hint bar looks like is tests/shots.rs.
use model::Library;
use screens::home::{rest, Focus, TopButton};
use screens::{hint_bar, Dashboard, Effect, Sound};
use ui::Button::{self, Down, Guide, Left, Right, RightBumper, Up, A, B, Y};
use ui::Press;

/// The fake library with two Groups, Installed and Most played, each six games and See all: seven games on the
/// Recent row and the Play history tile. Focus starts on the profile.
fn library() -> Library {
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &screens::fake_library(), 1_780_000_000).unwrap();
    for filter in database.filters().unwrap() {
        let shown = matches!(filter.key.as_deref(), Some("installed" | "most-played"));
        database.set_filter_shown(filter.id, shown).unwrap();
    }
    database.snapshot().unwrap()
}

/// Search as it opens on this library: empty, on Q, Recently added under the box.
const SEARCH: &str = "Search: Key(Q); \"|\" upper letters, RecentlyAdded 12";

/// Home with focus on the Recent row's first tile.
fn home() -> Dashboard {
    let mut dashboard = Dashboard::new(library());
    press(&mut dashboard, &[Down]);
    dashboard.take_effects();
    dashboard
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

/// Where the page rests with focus where it is, on a 1080 screen with the two Groups.
fn rests(dashboard: &Dashboard) -> screens::home::Rest {
    rest(dashboard.home_focus(), 2, 1080.0)
}

/// Presses `open` on Home as it is, expects the page that says `line` of itself and nothing of Home changed under
/// it; then B, and Home again with focus on `back`. On Search B is two steps: the keyboard goes, then the page.
#[track_caller]
fn there_and_back(dashboard: &mut Dashboard, open: Button, line: &str, back: Focus) {
    let before = dashboard.home_focus();
    press(dashboard, &[open]);
    assert_eq!(dashboard.focus_line(), line);
    assert_eq!(dashboard.pages_up(), 1);
    assert_eq!(dashboard.home_focus(), before, "Home keeps its focus while it is covered");
    // The page has the pad: nothing reaches Home under it. (No A: on a game of My games & apps it is a page of
    // its own, the game's hub.)
    press(dashboard, &[Down, Right, Up, Left]);
    assert_eq!((dashboard.pages_up(), dashboard.home_focus()), (1, before));
    if line.starts_with("Search: ") {
        press(dashboard, &[B]);
        assert_eq!(dashboard.page(), Some("Search"), "B puts Search's keyboard away first");
    }
    press(dashboard, &[B]);
    assert_eq!(dashboard.page(), None, "B closes the page");
    assert_eq!(dashboard.home_focus(), back);
    assert_eq!(dashboard.focus_line(), format!("{back:?}"));
    assert!(asked(dashboard).is_empty(), "nothing is asked of the shell");
}

/// Settings > Discord as Home's Discord tile opens it, nothing set up.
const DISCORD_PAGE: &str = "Settings: Discord: Direct Messages from discord.com; dms off, off";

/// Presses A on Home as it is, expects Settings with focus where `line` says and Home unchanged under it; then B
/// `levels` times, one level each, and Home again with focus on `back`.
#[track_caller]
fn settings_and_back(dashboard: &mut Dashboard, line: &str, levels: usize, back: Focus) {
    let before = dashboard.home_focus();
    press(dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), line);
    assert_eq!((dashboard.pages_up(), dashboard.home_focus()), (1, before));
    for level in 1..levels {
        press(dashboard, &[B]);
        assert_eq!(dashboard.page(), Some("Settings"), "B climbs a level at a time: {level} of {levels}");
    }
    assert!(
        dashboard.focus_line().starts_with("Settings: Rail: "),
        "the last level is the rail: {}",
        dashboard.focus_line()
    );
    press(dashboard, &[B]);
    assert_eq!((dashboard.page(), dashboard.home_focus()), (None, back));
    assert!(asked(dashboard).is_empty(), "nothing is asked of the shell");
}

#[test]
fn the_top_bars_my_games_button_opens_games_and_b_comes_back_on_the_recent_row() {
    let mut home = home();
    // From the third tile up to the button over it, My games & apps.
    press(&mut home, &[Right, Right, Up]);
    assert_eq!(home.home_focus(), Focus::TopBar(TopButton::MyGames));
    // Nothing gives the top bar focus back: the Recent row takes it, on the game last focused.
    there_and_back(&mut home, A, "MyGames: Games: Tile(0)", Focus::Recent(2));
}

#[test]
fn the_top_bars_search_and_settings_buttons_open_their_pages_and_get_focus_back() {
    let mut home = home();
    press(&mut home, &[Right, Right, Up, Right]);
    there_and_back(&mut home, A, SEARCH, Focus::TopBar(TopButton::Search));
    press(&mut home, &[Right]);
    settings_and_back(&mut home, "Settings: Rail: General", 1, Focus::TopBar(TopButton::Settings));
    // Notifications has no page.
    press(&mut home, &[Right, A]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::TopBar(TopButton::Notifications)));
}

#[test]
fn y_opens_search_from_anywhere_on_home_and_b_gives_focus_back_to_homes_rows() {
    // From the top bar, where Home starts: back on the Recent row, not on the button.
    let mut home = Dashboard::new(library());
    there_and_back(&mut home, Y, SEARCH, Focus::Recent(0));
    // Even from the Search button itself: only A on it gives it focus back.
    press(&mut home, &[Right, Right, Up, Right]);
    assert_eq!(home.home_focus(), Focus::TopBar(TopButton::Search));
    there_and_back(&mut home, Y, SEARCH, Focus::Recent(2));

    // From a game's tile, and from the Play history tile, which gives focus to the game's tile before it.
    press(&mut home, &[Right]);
    there_and_back(&mut home, Y, SEARCH, Focus::Recent(3));
    press(&mut home, &[Right, Right, Right, Right]);
    assert_eq!(home.home_focus(), Focus::Recent(7));
    there_and_back(&mut home, Y, SEARCH, Focus::Recent(6));

    // From a wide tile, a Group's card and See all: back where it was, the page at the same rest.
    press(&mut home, &[Down, Left]);
    let wide = home.home_focus();
    assert!(matches!(wide, Focus::Wide(_)));
    there_and_back(&mut home, Y, SEARCH, wide);
    press(&mut home, &[Down, Down, Right, Right]);
    let card = Focus::Group { group: 1, card: 2 };
    let at_rest = rests(&home);
    there_and_back(&mut home, Y, SEARCH, card);
    assert_eq!(rests(&home), at_rest);
    press(&mut home, &[Right, Right, Right, Right]);
    there_and_back(&mut home, Y, SEARCH, Focus::Group { group: 1, card: 6 });
}

#[test]
fn the_play_history_tile_opens_play_history_and_b_comes_back_on_the_game_before_it() {
    let mut home = home();
    press(&mut home, &[Right, Right, Right, Right, Right, Right, Right]);
    assert_eq!(home.home_focus(), Focus::Recent(7));
    there_and_back(&mut home, A, "MyGames: PlayHistory: Row(0)", Focus::Recent(6));
}

#[test]
fn see_all_opens_full_library_on_its_groups_filter_and_b_comes_back_on_see_all_at_the_groups_rest() {
    let mut home = home();
    let filters = home.library().filters.clone();
    let shown: Vec<_> = filters.iter().filter(|filter| filter.shown).collect();
    assert_eq!(shown.len(), 2);
    for (group, filter) in shown.iter().enumerate() {
        press(&mut home, &[Down, Down]);
        press(&mut home, &[Down].repeat(group));
        press(&mut home, &[Right; 6]);
        let see_all = Focus::Group { group, card: 6 };
        assert_eq!(home.home_focus(), see_all);
        let at_rest = rests(&home);
        // Full library on that filter's tab, focus on its first game, the grid in the filter's own order.
        // The tabs are the Groups and, after them, each launcher's whole library.
        let (tab, tabs, games) = (group + 1, home.library().tabs().count(), filter.games.len());
        let sort = format!(
            "{:?}{}",
            filter.sort,
            if filter.descending == filter.sort.default_descending() { "" } else { " reversed" }
        );
        let line = format!(
            "MyGames: FullLibrary: Tile(0); tab {tab}/{tabs} {:?}, sort {sort}, name \"\", {games} games",
            filter.name
        );
        there_and_back(&mut home, A, &line, see_all);
        assert_eq!(rests(&home), at_rest, "Home is still at the Group's rest");
        // B again is the Group's own: back to the Recent row.
        press(&mut home, &[B]);
        assert_eq!(home.home_focus(), Focus::Recent(0));
    }
}

#[test]
fn the_wide_tiles_open_their_pages_and_b_comes_back_on_the_tile() {
    let mut home = home();
    press(&mut home, &[Down]);
    assert_eq!(home.home_focus(), Focus::Wide(0));
    let at_rest = rests(&home);
    // Discord, with no Discord tab in the Guide: Settings > Account > Discord, on its switch. B is Account's
    // boxes, the rail, then the tile.
    settings_and_back(&mut home, DISCORD_PAGE, 3, Focus::Wide(0));
    press(&mut home, &[Right]);
    settings_and_back(&mut home, "Settings: General: Personalisation", 2, Focus::Wide(1));
    press(&mut home, &[Right, Right]);
    let queue = "ManageQueue: Cover(0) of Aeon of Embers IV; 0 installing, 8 recently installed";
    there_and_back(&mut home, A, queue, Focus::Wide(3));
    assert_eq!(rests(&home), at_rest);

    // Achievements is the Guide's, as the profile is: no page.
    press(&mut home, &[Left, A]);
    assert!(home.guide_open() && home.page().is_none());
    press(&mut home, &[B]);
    assert_eq!(home.home_focus(), Focus::Wide(2));
    assert_eq!(asked(&mut home), [Effect::GuideClosed]);
    // And not with the Guide setting off.
    home.set_guide_enabled(false);
    press(&mut home, &[A]);
    assert!(!home.guide_open() && home.page().is_none());
    // Nor Discord's Guide: its tile opens Settings then, connected or not.
    let connected = model::discord::Discord { connected: true, ..Default::default() };
    home.set_discord(&connected);
    press(&mut home, &[Left, Left]);
    settings_and_back(&mut home, DISCORD_PAGE, 3, Focus::Wide(0));
    home.set_guide_enabled(true);
    press(&mut home, &[A]);
    assert!(home.guide_open() && home.page().is_none(), "with a Discord tab the tile opens the Guide");
}

/// The Guide open at Home with focus on the Recent row's third tile.
fn guide_open() -> Dashboard {
    let mut home = home();
    press(&mut home, &[Right, Right, Guide]);
    home.take_effects();
    home
}

#[test]
fn the_guides_entries_open_their_pages_and_b_gives_focus_back_to_homes_rows() {
    // My games & apps is the landing tab's second entry.
    let mut home = guide_open();
    press(&mut home, &[Down, A]);
    assert!(!home.guide_open());
    assert_eq!(asked(&mut home), [Effect::GuideClosed, Effect::ShowHome]);
    assert_eq!(home.focus_line(), "MyGames: Games: Tile(0)");
    press(&mut home, &[B]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(2)));

    // Settings is Profile & system's third.
    let mut home = guide_open();
    press(&mut home, &[RightBumper, Down, Down, A]);
    assert_eq!(asked(&mut home), [Effect::GuideClosed, Effect::ShowHome]);
    assert_eq!(home.focus_line(), "Settings: Rail: General");
    press(&mut home, &[B]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(2)));

    // Search is the third Utility, under the landing tab's six entries.
    let mut home = guide_open();
    press(&mut home, &[Down, Down, Down, Down, Down, Down, Right, Right, A]);
    assert_eq!(asked(&mut home), [Effect::GuideClosed, Effect::ShowHome]);
    assert_eq!(home.focus_line(), SEARCH);
    press(&mut home, &[B, B]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(2)));

    // Downloads is the fourth, after Search: Manage Queue, as Home's Downloads tile opens it.
    let mut home = guide_open();
    press(&mut home, &[Down, Down, Down, Down, Down, Down, Right, Right, Right, A]);
    assert_eq!(asked(&mut home), [Effect::GuideClosed, Effect::ShowHome]);
    assert_eq!(home.page(), Some("ManageQueue"));
    press(&mut home, &[B]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(2)));
}

#[test]
fn a_page_opened_from_the_guide_takes_the_place_of_the_one_that_was_up() {
    let mut home = home();
    press(&mut home, &[Up, Right, Right, Right, A]);
    assert_eq!(home.focus_line(), "Settings: Rail: General");
    // The Guide opens over the page, and its My games & apps closes Settings for it.
    press(&mut home, &[Guide, Down, A]);
    assert_eq!((home.focus_line().as_str(), home.pages_up()), ("MyGames: Games: Tile(0)", 1));
    // B is Home, on its rows: the Settings button that opened the page before is not remembered.
    press(&mut home, &[B]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(0)));
}

#[test]
fn the_guides_home_closes_the_page_that_is_up() {
    let mut home = home();
    press(&mut home, &[Down, Right, A]);
    assert_eq!(home.focus_line(), "Settings: General: Personalisation");
    press(&mut home, &[Guide, A]);
    assert!(!home.guide_open());
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Wide(1)));
    // With no page up it closes the Guide and no more.
    press(&mut home, &[Guide, A]);
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Wide(1)));
}

/// Over a running game the Guide's entries open their pages all the same, the Dashboard coming in front of the
/// game (docs/spec/guide.md, 3.3; tests/guide_games.rs has what is asked of the shell).
#[test]
fn over_a_game_the_guides_entries_open_their_pages_in_front_of_it() {
    let over_game = || {
        let mut home = home();
        home.launch_started(2);
        home.game_booted();
        press(&mut home, &[Guide]);
        home
    };
    let mut home = over_game();
    press(&mut home, &[Down, A]);
    assert_eq!((home.guide_open(), home.page()), (false, Some("MyGames")));
    assert!(home.in_front_of_game() && !home.home_covered());
    // B on the page is Home, still in front of the game.
    press(&mut home, &[B]);
    assert_eq!(home.page(), None);
    assert!(home.in_front_of_game());

    let mut home = over_game();
    press(&mut home, &[Down, Down, Down, Down, Down, Down, Right, Right, A]);
    assert_eq!((home.guide_open(), home.page()), (false, Some("Search")));

    let mut home = over_game();
    press(&mut home, &[RightBumper, Down, Down, A]);
    assert_eq!((home.guide_open(), home.page()), (false, Some("Settings")));
}

#[test]
fn a_game_ending_closes_the_page_and_puts_focus_on_its_tile() {
    let mut home = home();
    press(&mut home, &[Up, Right, Right, Right, A]);
    assert_eq!(home.page(), Some("Settings"));
    home.launch_started(3);
    home.game_booted();
    home.game_ended();
    assert_eq!((home.page(), home.home_focus()), (None, Focus::Recent(3)));
}

#[test]
fn a_held_button_opens_and_closes_nothing() {
    let mut home = home();
    home.press(Press { button: Y, repeat: true });
    assert_eq!(home.page(), None);
    press(&mut home, &[Y]);
    home.press(Press { button: B, repeat: true });
    assert_eq!(home.page(), Some("Search"));
}

#[test]
fn the_hint_bar_shows_on_a_groups_game_card_and_nowhere_else_on_home() {
    let mut home = Dashboard::new(library());
    let none = hint_bar::NONE;
    assert_eq!(home.hints(), none, "the top bar");
    press(&mut home, &[Down]);
    assert_eq!(home.hints(), none, "a Recent tile");
    press(&mut home, &[Right, Right, Right, Right, Right, Right, Right]);
    assert_eq!(home.hints(), none, "the Play history tile");
    press(&mut home, &[Down]);
    assert_eq!(home.hints(), none, "a wide tile");
    press(&mut home, &[Down]);
    assert_eq!(home.hints(), hint_bar::MORE_OPTIONS_AND_SEARCH);
    let labels: Vec<_> = home.hints().iter().map(|hint| hint.label).collect();
    assert_eq!(labels, ["More options", "Search"]);
    // Not under the Guide, nor under a page that has none of its own.
    press(&mut home, &[Guide]);
    assert_eq!(home.hints(), none, "under the Guide");
    press(&mut home, &[B, Y]);
    assert_eq!(home.hints(), none, "on Search");
    press(&mut home, &[B, B, Right, Right, Right, Right, Right, Right]);
    assert_eq!(home.home_focus(), Focus::Group { group: 0, card: 6 });
    assert_eq!(home.hints(), none, "See all");
    press(&mut home, &[Left]);
    assert_eq!(home.hints(), hint_bar::MORE_OPTIONS_AND_SEARCH);
}

#[test]
fn a_that_opens_a_page_sounds_and_y_and_b_are_silent() {
    let mut home = home();
    press(&mut home, &[Up, Right]);
    home.take_effects();
    press(&mut home, &[A]);
    assert_eq!(home.take_effects(), [Effect::Sound(Sound::Activate)]);
    // B closes it in silence, though Home's focus is on the Recent row now and was on the top bar.
    press(&mut home, &[B]);
    assert_eq!(home.home_focus(), Focus::Recent(0));
    assert!(home.take_effects().is_empty());
    press(&mut home, &[Y]);
    assert_eq!(home.page(), Some("Search"));
    // Its first B puts the keyboard away, focus landing on a result; the second closes it, in silence.
    press(&mut home, &[Down, B]);
    home.take_effects();
    press(&mut home, &[B]);
    assert_eq!(home.page(), None);
    assert!(home.take_effects().is_empty());
    // From the Guide it is the Guide's own sound.
    press(&mut home, &[Guide, Down]);
    home.take_effects();
    press(&mut home, &[A]);
    assert_eq!(home.take_effects(), [Effect::Sound(Sound::GuideSelect), Effect::GuideClosed, Effect::ShowHome]);
}
