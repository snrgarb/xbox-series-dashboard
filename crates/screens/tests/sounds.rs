//! Which sound each press asks for: the Bundle's, where the Bundle plays them (docs/spec/home.md, section 9).
//! Nothing is played here: a sound is an effect the shell is asked for, as a launch is.
use screens::{Dashboard, Effect, Sound};
use ui::{Button, Press};

/// Home with focus brought down from the profile, where it starts, to the Recent row's first tile.
fn dashboard() -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    assert_eq!(press(&mut dashboard, Button::Down), [MOVE]);
    dashboard
}

/// Presses `button` and gives everything the press asked for.
fn press(dashboard: &mut Dashboard, button: Button) -> Vec<Effect> {
    dashboard.press(Press { button, repeat: false });
    dashboard.take_effects()
}

/// The same for a button held down.
fn hold(dashboard: &mut Dashboard, button: Button) -> Vec<Effect> {
    dashboard.press(Press { button, repeat: true });
    dashboard.take_effects()
}

const MOVE: Effect = Effect::Sound(Sound::Move);
const ACTIVATE: Effect = Effect::Sound(Sound::Activate);
const GUIDE_OPEN: Effect = Effect::Sound(Sound::GuideOpen);
const GUIDE_MOVE: Effect = Effect::Sound(Sound::GuideMove);
const GUIDE_SELECT: Effect = Effect::Sound(Sound::GuideSelect);

#[test]
fn every_sound_is_one_of_the_bundles_files() {
    let files = |sound: Sound| sound.files().to_vec();
    assert_eq!(files(Sound::Move), ["navigation.wav"]);
    assert_eq!(files(Sound::Activate), ["activation.wav"]);
    assert_eq!(files(Sound::GuideOpen), ["OpenMenu.wav"]);
    assert_eq!(files(Sound::GuideMove), ["Navigate1.wav", "Navigate2.wav", "Navigate3.wav", "Navigate4.wav"]);
    assert_eq!(files(Sound::GuideSelect), ["Enter.wav"]);
    let mut all = Sound::all_files();
    assert_eq!(all.len(), 8);
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 8, "no file is two sounds");
}

#[test]
fn focus_landing_somewhere_new_on_home_is_the_move_sound() {
    let mut home = dashboard();
    assert_eq!(press(&mut home, Button::Right), [MOVE]);
    assert_eq!(press(&mut home, Button::Left), [MOVE]);
    // Held, each tile focus reaches sounds as the first did.
    assert_eq!(hold(&mut home, Button::Right), [MOVE]);
}

#[test]
fn a_move_with_nowhere_to_go_is_silent() {
    let mut home = dashboard();
    let start = home.home_focus();
    assert!(press(&mut home, Button::Left).is_empty());
    assert_eq!(home.home_focus(), start, "Left of the first tile there is nothing");
}

/// Every direction, many times over, from wherever the last left focus: the sound is there exactly when focus is
/// somewhere new, and it is the only thing asked for.
#[test]
fn on_home_the_move_sound_is_heard_exactly_when_focus_moved() {
    let mut home = dashboard();
    let walk = [Button::Right, Button::Down, Button::Left, Button::Left, Button::Up, Button::Up, Button::Right];
    let (mut moved, mut stayed) = (0, 0);
    for step in 0..140 {
        // Runs of one direction, so every edge of the page is run into.
        let button = walk[(step / 10) % walk.len()];
        let before = home.home_focus();
        let asked = if step % 3 == 0 { hold(&mut home, button) } else { press(&mut home, button) };
        if home.home_focus() == before {
            assert!(asked.is_empty(), "{button:?} left focus on {before:?} and asked for {asked:?}");
            stayed += 1;
        } else {
            assert_eq!(asked, [MOVE], "{button:?} from {before:?}");
            moved += 1;
        }
    }
    assert!(moved > 10 && stayed > 10, "the walk both moves and runs into edges: {moved} moves, {stayed} not");
}

#[test]
fn a_on_a_tile_is_the_activation_sound_then_the_launch() {
    let mut home = dashboard();
    press(&mut home, Button::Right);
    assert_eq!(press(&mut home, Button::A), [ACTIVATE, Effect::Launch(1)]);
    // A still held starts nothing more and says nothing more.
    assert!(hold(&mut home, Button::A).is_empty());
}

#[test]
fn b_on_home_and_buttons_that_do_nothing_are_silent() {
    let mut home = dashboard();
    for button in [Button::B, Button::X, Button::View] {
        assert!(press(&mut home, button).is_empty(), "{button:?}");
    }
}

#[test]
fn the_guide_opens_with_its_sound_and_closes_without_one() {
    let mut home = dashboard();
    assert_eq!(press(&mut home, Button::Guide), [GUIDE_OPEN]);
    assert_eq!(press(&mut home, Button::Guide), [Effect::GuideClosed]);
    assert_eq!(press(&mut home, Button::Guide), [GUIDE_OPEN]);
    assert_eq!(press(&mut home, Button::B), [Effect::GuideClosed]);
    // Opened by something other than a press (a screenshot of it): nothing is heard.
    home.open_guide();
    assert!(home.take_effects().is_empty());
}

#[test]
fn moving_in_the_guide_is_its_own_sound_and_only_when_focus_moved() {
    let mut home = dashboard();
    press(&mut home, Button::Guide);
    assert!(press(&mut home, Button::Up).is_empty(), "nothing is above the first entry");
    assert_eq!(press(&mut home, Button::Down), [GUIDE_MOVE]);
    assert_eq!(hold(&mut home, Button::Down), [GUIDE_MOVE]);
    assert_eq!(press(&mut home, Button::Up), [GUIDE_MOVE]);
    // Left and Right do nothing in the list.
    assert!(press(&mut home, Button::Left).is_empty());
    assert!(press(&mut home, Button::Right).is_empty());
    // From the second entry down the other four, onto the first Utility, and against the panel's foot.
    for _ in 0..5 {
        assert_eq!(press(&mut home, Button::Down), [GUIDE_MOVE]);
    }
    assert!(press(&mut home, Button::Down).is_empty(), "nothing is below the Utilities");
    // Along the Utilities and against the row's ends.
    assert!(press(&mut home, Button::Left).is_empty());
    assert_eq!(press(&mut home, Button::Right), [GUIDE_MOVE]);
    assert_eq!(hold(&mut home, Button::Right), [GUIDE_MOVE]);
    assert!(press(&mut home, Button::Right).is_empty());
    assert_eq!(press(&mut home, Button::Up), [GUIDE_MOVE]);
    // Home's focus is where it was: the Guide's moves were not Home's.
    assert!(!press(&mut home, Button::B).contains(&MOVE));
}

#[test]
fn changing_tab_is_the_move_sound_and_a_bumper_at_the_last_tab_is_silent() {
    let mut home = dashboard();
    press(&mut home, Button::Guide);
    assert!(press(&mut home, Button::LeftBumper).is_empty());
    assert_eq!(press(&mut home, Button::RightBumper), [GUIDE_MOVE]);
    assert!(press(&mut home, Button::RightBumper).is_empty());
    assert!(hold(&mut home, Button::LeftBumper).is_empty(), "a bumper held changes tab once");
    assert_eq!(press(&mut home, Button::LeftBumper), [GUIDE_MOVE]);
}

#[test]
fn a_in_the_guide_is_the_select_sound_then_what_the_entry_does() {
    let mut home = dashboard();
    press(&mut home, Button::Guide);
    assert_eq!(press(&mut home, Button::A), [GUIDE_SELECT, Effect::GuideClosed], "Home");
    press(&mut home, Button::Guide);
    for _ in 0..3 {
        press(&mut home, Button::Down);
    }
    assert_eq!(press(&mut home, Button::A), [GUIDE_SELECT, Effect::GuideClosed, Effect::Launch(1)]);
    // A still held chooses nothing more.
    assert!(hold(&mut home, Button::A).is_empty());
}

#[test]
fn a_on_something_that_does_nothing_is_silent() {
    let mut home = dashboard();
    press(&mut home, Button::Guide);
    // A Utility, whose Flyout is yet to come.
    for _ in 0..6 {
        press(&mut home, Button::Down);
    }
    assert!(press(&mut home, Button::A).is_empty());
    // The Profile entry, which never does anything.
    press(&mut home, Button::RightBumper);
    assert!(press(&mut home, Button::A).is_empty());
    // And buttons the Guide has no use for.
    for button in [Button::X, Button::Y, Button::View, Button::Menu] {
        assert!(press(&mut home, button).is_empty(), "{button:?}");
    }
}

#[test]
fn while_a_game_starts_and_runs_only_the_guide_is_heard() {
    let mut home = dashboard();
    assert_eq!(press(&mut home, Button::A), [ACTIVATE, Effect::Launch(0)]);
    home.launch_started(0);
    for button in [Button::Right, Button::A, Button::Guide] {
        assert!(press(&mut home, button).is_empty(), "{button:?} behind the Launch screen");
    }
    home.game_booted();
    for button in [Button::Right, Button::A, Button::B] {
        assert!(press(&mut home, button).is_empty(), "{button:?} behind the game");
    }
    // Over the game: the Guide, down to the game, and A goes back to it.
    assert_eq!(press(&mut home, Button::Guide), [GUIDE_OPEN]);
    assert_eq!(press(&mut home, Button::Down), [GUIDE_MOVE]);
    assert_eq!(press(&mut home, Button::Down), [GUIDE_MOVE]);
    assert_eq!(press(&mut home, Button::A), [GUIDE_SELECT, Effect::GuideClosed]);
    // Again, and Menu on the game quits it: the menu's one option chosen.
    assert_eq!(press(&mut home, Button::Guide), [GUIDE_OPEN]);
    press(&mut home, Button::Down);
    press(&mut home, Button::Down);
    assert_eq!(press(&mut home, Button::Menu), [GUIDE_SELECT, Effect::QuitGame]);
    // The game ending puts focus back on its tile in silence.
    home.game_ended();
    assert!(home.take_effects().is_empty());
}
