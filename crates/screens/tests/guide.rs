//! The Guide: where its focus goes and what a press asks for (docs/spec/guide.md, section 3). No picture, so no
//! GPU; what it looks like is tests/shots.rs, and what it sounds like tests/sounds.rs.
use model::Library;
use screens::guide::{Focus, Kind, Tab};
use screens::{Dashboard, Effect};
use ui::Button::{self, Down, Guide, Left, LeftBumper, Menu, Right, RightBumper, Up, A, B};
use ui::Press;

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Presses, then says which tab shows and where focus is on it.
fn after(dashboard: &mut Dashboard, buttons: &[Button]) -> (Tab, Focus) {
    press(dashboard, buttons);
    dashboard.guide().focus()
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

fn kinds(dashboard: &Dashboard) -> Vec<Kind> {
    dashboard.guide().entries().iter().map(|entry| entry.kind).collect()
}

/// The Guide open at Home on the fake library: Home, My games & apps and four games.
fn at_home() -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    press(&mut dashboard, &[Guide]);
    dashboard.take_effects();
    dashboard
}

/// The Guide open over the library's game `game`, running.
fn over_game(library: Library, game: usize) -> Dashboard {
    let mut dashboard = Dashboard::new(library);
    dashboard.launch_started(game);
    dashboard.game_booted();
    press(&mut dashboard, &[Guide]);
    dashboard.take_effects();
    dashboard
}

const LANDING: Tab = Tab::Landing;
const PROFILE: Tab = Tab::ProfileAndSystem;

fn entry(at: usize) -> Focus {
    Focus::Entry(at)
}

fn utility(at: usize) -> Focus {
    Focus::Utility(at)
}

#[test]
fn the_guide_opens_on_the_landing_tab_with_focus_on_home() {
    let dashboard = at_home();
    assert_eq!(dashboard.guide().focus(), (LANDING, entry(0)));
    assert_eq!(dashboard.guide().tabs(), [LANDING, PROFILE]);
    let games = [Kind::Recent(0), Kind::Recent(1), Kind::Recent(2), Kind::Recent(3)];
    assert_eq!(kinds(&dashboard), [&[Kind::Home, Kind::MyGames][..], &games[..]].concat());
    // The first game stands under a divider, and nothing else does.
    let dividers: Vec<bool> = dashboard.guide().entries().iter().map(|entry| entry.divider).collect();
    assert_eq!(dividers, [false, false, true, false, false, false]);
    assert_eq!(dashboard.guide().entries()[2].title, "Halyard Point");
}

#[test]
fn up_and_down_move_along_the_entries_and_down_from_the_last_is_the_first_utility() {
    let mut guide = at_home();
    assert_eq!(after(&mut guide, &[Up]), (LANDING, entry(0)), "nothing is above the first entry");
    assert_eq!(after(&mut guide, &[Down]), (LANDING, entry(1)));
    assert_eq!(after(&mut guide, &[Down, Down, Down, Down]), (LANDING, entry(5)));
    assert_eq!(after(&mut guide, &[Down]), (LANDING, utility(0)));
    assert_eq!(after(&mut guide, &[Down]), (LANDING, utility(0)), "nothing is under the Utilities");
    assert_eq!(after(&mut guide, &[Up]), (LANDING, entry(5)));
    assert_eq!(after(&mut guide, &[Up, Up]), (LANDING, entry(3)));
}

#[test]
fn left_and_right_move_along_the_utilities_and_do_nothing_on_an_entry() {
    let mut guide = at_home();
    assert_eq!(after(&mut guide, &[Right, Left]), (LANDING, entry(0)));
    press(&mut guide, &[Down; 6]);
    assert_eq!(after(&mut guide, &[Left]), (LANDING, utility(0)), "the row stops at its ends");
    assert_eq!(after(&mut guide, &[Right]), (LANDING, utility(1)));
    assert_eq!(after(&mut guide, &[Right]), (LANDING, utility(2)));
    assert_eq!(after(&mut guide, &[Right]), (LANDING, utility(3)), "Downloads, last");
    assert_eq!(after(&mut guide, &[Right]), (LANDING, utility(3)));
    assert_eq!(after(&mut guide, &[Left]), (LANDING, utility(2)));
}

#[test]
fn up_from_any_utility_is_the_last_entry_and_down_again_is_the_first_utility() {
    let mut guide = at_home();
    press(&mut guide, &[Down; 6]);
    assert_eq!(after(&mut guide, &[Right, Right, Up]), (LANDING, entry(5)));
    assert_eq!(after(&mut guide, &[Down]), (LANDING, utility(0)), "not the Utility it left");
}

#[test]
fn the_bumpers_change_tab_stop_at_the_ends_and_focus_the_new_tabs_first_entry() {
    let mut guide = at_home();
    assert_eq!(after(&mut guide, &[LeftBumper]), (LANDING, entry(0)), "nothing is left of the landing tab");
    assert_eq!(after(&mut guide, &[Down, Down, RightBumper]), (PROFILE, entry(0)));
    assert_eq!(kinds(&guide), [Kind::Profile, Kind::SwitchProfile, Kind::Settings, Kind::Power]);
    assert_eq!(after(&mut guide, &[RightBumper]), (PROFILE, entry(0)), "nothing is right of Profile & system");
    // Back: the landing tab's first entry, not the one focus was on before.
    assert_eq!(after(&mut guide, &[Down, Down, LeftBumper]), (LANDING, entry(0)));
    // From a Utility too.
    press(&mut guide, &[Down; 6]);
    assert_eq!(after(&mut guide, &[RightBumper]), (PROFILE, entry(0)));
}

#[test]
fn profile_and_system_has_no_utilities() {
    let mut guide = at_home();
    assert_eq!(after(&mut guide, &[RightBumper, Down, Down, Down]), (PROFILE, entry(3)));
    assert_eq!(after(&mut guide, &[Down]), (PROFILE, entry(3)), "Power is the last thing on the tab");
    assert_eq!(after(&mut guide, &[Right, Left]), (PROFILE, entry(3)));
    let dividers: Vec<bool> = guide.guide().entries().iter().map(|entry| entry.divider).collect();
    assert_eq!(dividers, [false, false, true, true], "Settings and Power each stand under a divider");
}

#[test]
fn the_guide_remembers_nothing_from_the_last_time_it_was_open() {
    let mut guide = at_home();
    press(&mut guide, &[RightBumper, Down, B]);
    assert!(!guide.guide_open());
    assert_eq!(after(&mut guide, &[Guide]), (LANDING, entry(0)));
    press(&mut guide, &[Down, Down, Down, Guide]);
    assert_eq!(after(&mut guide, &[Guide]), (LANDING, entry(0)));
}

#[test]
fn b_and_the_guide_button_close_it() {
    for button in [B, Guide] {
        let mut guide = at_home();
        press(&mut guide, &[Down, button]);
        assert!(!guide.guide_open(), "{button:?}");
        assert_eq!(asked(&mut guide), [Effect::GuideClosed]);
    }
}

#[test]
fn buttons_the_guide_has_no_use_for_do_nothing() {
    let mut guide = at_home();
    let before = guide.guide().focus();
    for button in [Button::X, Button::Y, Button::View, Button::LeftTrigger, Button::RightTrigger, Menu] {
        press(&mut guide, &[button]);
        assert!(guide.guide_open() && guide.guide().focus() == before, "{button:?}");
        assert!(guide.take_effects().is_empty(), "{button:?}");
    }
}

#[test]
fn a_on_home_closes_the_guide_onto_home() {
    let mut guide = at_home();
    press(&mut guide, &[A]);
    assert!(!guide.guide_open());
    assert_eq!(asked(&mut guide), [Effect::GuideClosed]);
}

#[test]
fn a_on_a_recent_game_closes_the_guide_and_launches_it_with_homes_focus_on_its_tile() {
    let mut guide = at_home();
    press(&mut guide, &[Down, Down, Down, Down, A]);
    assert!(!guide.guide_open());
    assert_eq!(asked(&mut guide), [Effect::GuideClosed, Effect::Launch(2)]);
    assert_eq!(guide.home_focus(), screens::home::Focus::Recent(2));
}

#[test]
fn the_profile_entry_is_there_and_does_nothing() {
    let mut guide = at_home();
    // (The Utilities' Flyouts are tests/flyouts.rs; what opens a page is tests/pages.rs; Switch profile, which asks
    // Who's playing today?, is tests/startup.rs.)
    press(&mut guide, &[Down, Down, Down, Down, Down, Down, Right]);
    assert_eq!(guide.guide().focus(), (LANDING, utility(1)));
    press(&mut guide, &[RightBumper, A]);
    assert_eq!(guide.guide().focus(), (PROFILE, entry(0)));
    assert!(guide.guide_open() && !guide.starting_up());
    assert!(guide.take_effects().iter().all(|effect| *effect == Effect::Sound(screens::Sound::GuideMove)));
}

/// Power, whose own tests are tests/power.rs.
#[test]
fn a_on_power_opens_its_dialog_and_asks_nothing_of_the_shell() {
    let mut guide = at_home();
    press(&mut guide, &[RightBumper, Down, Down, Down, A]);
    assert!(guide.guide().dialog().is_some());
    assert!(guide.guide_open() && asked(&mut guide).is_empty());
}

#[test]
fn over_a_game_the_running_game_is_first_of_the_games_and_the_rest_follow() {
    let guide = over_game(screens::fake_library(), 2);
    let games = [Kind::Running(2), Kind::Recent(0), Kind::Recent(1), Kind::Recent(3)];
    assert_eq!(kinds(&guide), [&[Kind::Home, Kind::MyGames][..], &games[..]].concat());
    assert!(guide.guide().entries()[2].divider);
    assert_eq!(guide.guide().focus(), (LANDING, entry(0)), "focus starts on Home over a game too");
    assert!(guide.guide_over_game());

    // A game never played before is first all the same.
    let guide = over_game(screens::fake_library(), 12);
    assert_eq!(kinds(&guide)[2..], [Kind::Running(12), Kind::Recent(0), Kind::Recent(1), Kind::Recent(2)]);
}

#[test]
fn a_on_the_running_game_goes_back_to_it() {
    let mut guide = over_game(screens::fake_library(), 2);
    press(&mut guide, &[Down, Down, A]);
    assert!(!guide.guide_open());
    // To the game's own window, whatever the Guide opened over.
    assert_eq!(asked(&mut guide), [Effect::GuideClosed, Effect::ShowGame]);
    assert_eq!(guide.playing(), Some(2));
}

#[test]
fn only_the_running_game_has_a_menu_and_quitting_from_it_leaves_the_guide_until_the_game_has_gone() {
    let mut guide = over_game(screens::fake_library(), 2);
    press(&mut guide, &[Down, Menu]);
    assert!(guide.guide().dialog().is_none(), "My games & apps has no menu");
    press(&mut guide, &[Down, Menu]);
    assert!(guide.guide().dialog().is_some());
    assert!(asked(&mut guide).is_empty(), "the menu is asked first");
    press(&mut guide, &[A]);
    assert_eq!(asked(&mut guide), [Effect::QuitGame]);
    assert!(guide.guide_open());
    press(&mut guide, &[Down, Menu]);
    assert!(guide.guide().dialog().is_none(), "nor has a recent game");
    // The game gone, the Guide closes half a second later (tests/guide_games.rs has the rest of it).
    guide.game_ended();
    assert!(guide.guide_open());
    guide.advance(screens::CLOSE_AFTER_QUIT);
    assert!(!guide.guide_open());
}

#[test]
fn over_a_game_switch_profile_is_greyed_out_and_does_nothing() {
    let mut guide = over_game(screens::fake_library(), 2);
    // Greyed out, and still taking focus.
    press(&mut guide, &[RightBumper, Down]);
    assert!(!guide.guide().entries()[1].enabled);
    assert_eq!(after(&mut guide, &[A, Down, Down]), (PROFILE, entry(3)));
    assert!(guide.guide_open() && guide.guide().dialog().is_none() && asked(&mut guide).is_empty());

    // At Home Switch profile is not greyed out.
    let mut guide = at_home();
    press(&mut guide, &[RightBumper]);
    assert!(guide.guide().entries().iter().all(|entry| entry.enabled));
}

#[test]
fn a_held_direction_keeps_moving_and_a_held_button_does_nothing_more() {
    let mut guide = at_home();
    guide.press(Press { button: Down, repeat: true });
    guide.press(Press { button: Down, repeat: true });
    assert_eq!(guide.guide().focus(), (LANDING, entry(2)));
    for button in [A, RightBumper, B, Guide] {
        guide.press(Press { button, repeat: true });
        assert!(guide.guide_open() && guide.guide().focus() == (LANDING, entry(2)), "{button:?}");
    }
    assert!(asked(&mut guide).is_empty());
}

#[test]
fn with_nothing_played_the_landing_tab_is_home_and_my_games_over_the_utilities() {
    let mut library = screens::fake_library();
    library.games.iter_mut().for_each(|game| game.last_played = 0);
    let mut guide = Dashboard::new(library);
    press(&mut guide, &[Guide]);
    assert_eq!(kinds(&guide), [Kind::Home, Kind::MyGames]);
    assert_eq!(after(&mut guide, &[Down, Down]), (LANDING, utility(0)));
    assert_eq!(after(&mut guide, &[Up]), (LANDING, entry(1)));
}

#[test]
fn a_new_snapshot_of_the_library_leaves_focus_at_its_place_in_the_list() {
    let mut guide = at_home();
    press(&mut guide, &[Down, Down, Down, Down, Down]);
    let mut next = screens::fake_library();
    next.games.swap(0, 1);
    guide.set_library(next);
    assert_eq!(guide.guide().focus(), (LANDING, entry(5)));
    assert_eq!(guide.guide().entries()[2].title, "Fernway Rally 5");
    // A shorter list: focus on its last entry.
    let mut next = screens::fake_library();
    next.games.truncate(2);
    guide.set_library(next);
    assert_eq!(guide.guide().focus(), (LANDING, entry(3)));
    // On a Utility it stays on the Utility.
    press(&mut guide, &[Down, Right]);
    guide.set_library(screens::fake_library());
    assert_eq!(guide.guide().focus(), (LANDING, utility(1)));
}

#[test]
fn a_game_starting_with_the_guide_open_appears_as_running() {
    let mut guide = at_home();
    press(&mut guide, &[Down, Down]);
    guide.launch_started(1);
    assert_eq!(kinds(&guide)[2..], [Kind::Running(1), Kind::Recent(0), Kind::Recent(2), Kind::Recent(3)]);
    assert_eq!(guide.guide().focus(), (LANDING, entry(2)));
}

#[test]
fn with_discord_connected_its_tab_stands_between_the_two() {
    let mut guide = Dashboard::new(screens::fake_library());
    let signed_in = model::discord::Discord { connected: true, user: Some("E2E Tester".into()), ..Default::default() };
    guide.set_discord(&signed_in);
    press(&mut guide, &[Guide]);
    assert_eq!(guide.guide().tabs(), [LANDING, Tab::Discord, PROFILE]);
    // Out of a Call with no Shortcuts: focus on Servers, under who is signed in and that they are in no voice
    // channel; A on either of those does nothing.
    assert_eq!(after(&mut guide, &[RightBumper]), (Tab::Discord, entry(2)));
    press(&mut guide, &[Up, A]);
    assert_eq!(guide.guide().focus(), (Tab::Discord, entry(1)));
    assert!(asked(&mut guide).is_empty() && guide.guide_open());
    assert_eq!(after(&mut guide, &[RightBumper]), (PROFILE, entry(0)));
    assert_eq!(after(&mut guide, &[LeftBumper, LeftBumper]), (LANDING, entry(0)));
    // Disconnect while its tab shows: back to the landing tab.
    press(&mut guide, &[RightBumper]);
    guide.set_discord(&model::discord::Discord::default());
    assert_eq!(guide.guide().tabs(), [LANDING, PROFILE]);
    assert_eq!(guide.guide().focus().0, LANDING);
}
