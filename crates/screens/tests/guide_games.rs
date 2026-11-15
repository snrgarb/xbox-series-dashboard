//! Games from the Guide (docs/spec/guide.md, 3.3, 6.3, 6.4, 7.3 and 7.4), as the Bundle's ExtensionCore.cs has them:
//! the running game's entry and its menu, Quit with its ten seconds and "Force quit?", a game chosen while another
//! runs, Home chosen over a game, and what a game ending does to an open Guide. What a press does, what it sounds
//! like and what is asked of the shell; time is told with `advance`, so nothing here waits.
use screens::guide::{DialogFocus, Focus, Kind, Tab};
use screens::{Dashboard, Effect, OverlayShows, Sound, CLOSE_AFTER_QUIT, QUIT_TIMEOUT};
use ui::Button::{self, Down, Guide, Menu, Up, A, B};
use ui::{PadEvent, Press};

const SELECT: Effect = Effect::Sound(Sound::GuideSelect);
const MOVE: Effect = Effect::Sound(Sound::GuideMove);
const PROMPT_OPEN: Effect = Effect::Sound(Sound::PromptOpen);
const PROMPT_CLOSE: Effect = Effect::Sound(Sound::PromptClose);
const CLOSED: Effect = Effect::GuideClosed;

/// The game that runs in these tests, and the Guide's entries over it: Home, My games & apps, the game, then the
/// games played before it, Halo Infinite (0), Forza Horizon 5 (1) and Starfield (3).
const RUNNING: usize = 2;
const ON_RUNNING: (Tab, Focus) = (Tab::Landing, Focus::Entry(2));
const ON_HALO: (Tab, Focus) = (Tab::Landing, Focus::Entry(3));

fn prompt(row: usize) -> Option<DialogFocus> {
    Some(DialogFocus::Menu { prompt: true, row })
}

/// Presses each button and gives everything the presses asked for.
fn press(dashboard: &mut Dashboard, buttons: &[Button]) -> Vec<Effect> {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
    dashboard.take_effects()
}

/// Lets `seconds` pass and gives what came of it.
fn after(dashboard: &mut Dashboard, seconds: f32) -> Vec<Effect> {
    dashboard.advance(seconds);
    dashboard.take_effects()
}

/// A game running, its Launch screen gone.
fn in_game() -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.launch_started(RUNNING);
    dashboard.game_booted();
    dashboard
}

/// The Guide open over the running game, focus on its entry.
fn on_running_game() -> Dashboard {
    let mut dashboard = in_game();
    press(&mut dashboard, &[Guide, Down, Down]);
    assert_eq!(dashboard.guide().focus(), ON_RUNNING);
    assert_eq!(dashboard.guide().entries()[2].kind, Kind::Running(RUNNING));
    dashboard
}

/// The same, focus on the first game under it: Halo Infinite, played before.
fn on_a_recent_game() -> Dashboard {
    let mut dashboard = on_running_game();
    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.guide().entries()[3].kind, Kind::Recent(0));
    dashboard
}

/// Quit chosen in the running game's menu.
fn quitting() -> Dashboard {
    let mut dashboard = on_running_game();
    assert_eq!(press(&mut dashboard, &[Menu]), [PROMPT_OPEN]);
    assert_eq!(dashboard.guide().dialog_title(), Some("Sea of Thieves"));
    assert_eq!(press(&mut dashboard, &[A]), [SELECT, Effect::QuitGame]);
    dashboard
}

// ---- The running game's entry.

#[test]
fn a_on_the_running_game_closes_the_guide_and_shows_the_game() {
    let mut guide = on_running_game();
    assert_eq!(press(&mut guide, &[A]), [SELECT, CLOSED, Effect::ShowGame]);
    assert!(!guide.guide_open() && guide.playing() == Some(RUNNING));
}

// ---- Quit: graceful, ten seconds, Force quit.

#[test]
fn quit_asks_the_game_to_close_with_no_question_and_the_guide_stays_open() {
    let mut guide = quitting();
    assert!(guide.guide_open() && guide.guide().dialog().is_none() && guide.quitting());
    assert_eq!(guide.guide().focus(), ON_RUNNING, "its entry stays until it has stopped");
    assert_eq!(guide.guide().entries()[2].kind, Kind::Running(RUNNING));
    assert_eq!(guide.guide_line(), "Landing Entry(2), quitting");
    // Quit a second time asks nothing more of the shell.
    assert_eq!(press(&mut guide, &[Menu, A]), [PROMPT_OPEN, SELECT]);
    assert!(guide.quitting());
}

#[test]
fn the_guide_closes_half_a_second_after_a_game_quit_from_it_stops() {
    let mut guide = quitting();
    guide.game_ended();
    assert_eq!(guide.take_effects(), []);
    assert!(guide.guide_open() && !guide.quitting());
    assert_eq!(guide.guide().entries().iter().filter(|entry| matches!(entry.kind, Kind::Running(_))).count(), 0);
    assert_eq!(guide.timer(), Some(CLOSE_AFTER_QUIT));
    assert_eq!(after(&mut guide, 0.4), []);
    assert!(guide.guide_open());
    // In silence, and at once: no button closed it, so there is none to wait for.
    assert_eq!(after(&mut guide, 0.1), [CLOSED]);
    assert!(!guide.guide_open() && !guide.awaiting_release());
    assert_eq!(guide.overlay(), OverlayShows::Passive);
    assert_eq!(guide.timer(), None);
    assert_eq!(guide.home_focus(), screens::home::Focus::Recent(RUNNING), "onto Home, on the game's tile");
}

#[test]
fn a_guide_closed_before_the_half_second_is_up_is_not_closed_again() {
    let mut guide = quitting();
    guide.game_ended();
    assert_eq!(press(&mut guide, &[B]), [CLOSED]);
    assert_eq!(guide.timer(), None);
    // Opened again within the half second: it stays.
    press(&mut guide, &[Guide]);
    assert_eq!(after(&mut guide, 1.0), []);
    assert!(guide.guide_open());
}

#[test]
fn a_game_that_has_not_closed_after_ten_seconds_is_asked_about() {
    let mut guide = quitting();
    assert_eq!(guide.timer(), Some(QUIT_TIMEOUT));
    assert_eq!(after(&mut guide, 9.9), []);
    assert_eq!(guide.guide().dialog(), None);
    assert_eq!(after(&mut guide, 0.2), [PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog(), prompt(0));
    assert_eq!(guide.guide().dialog_title(), Some("Force quit Sea of Thieves?"));
    assert_eq!(
        guide.guide_line(),
        "Landing Entry(2), dialog Menu { prompt: true, row: 0 } \"Force quit Sea of Thieves?\", quitting"
    );
    // Nothing is counted while the question stands.
    assert_eq!(guide.timer(), None);
    assert_eq!(after(&mut guide, 60.0), []);
    assert_eq!(press(&mut guide, &[Down]), [MOVE]);
    assert_eq!(guide.guide().dialog(), prompt(1));
    assert_eq!(press(&mut guide, &[Down]), [], "Cancel is the last");
}

#[test]
fn force_quit_ends_the_game_and_asks_again_if_it_is_still_there_ten_seconds_on() {
    let mut guide = quitting();
    after(&mut guide, QUIT_TIMEOUT);
    assert_eq!(press(&mut guide, &[A]), [PROMPT_CLOSE, Effect::ForceQuitGame]);
    assert!(guide.guide_open() && guide.guide().dialog().is_none() && guide.quitting());
    assert_eq!(after(&mut guide, 9.0), []);
    assert_eq!(after(&mut guide, 1.0), [PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog(), prompt(0));
    // It goes: the prompt with it, in silence, and the Guide half a second later.
    guide.game_ended();
    assert_eq!(guide.take_effects(), []);
    assert_eq!(guide.guide().dialog(), None);
    assert_eq!(after(&mut guide, CLOSE_AFTER_QUIT), [CLOSED]);
}

#[test]
fn cancel_and_b_leave_the_game_running_and_the_quit_is_over() {
    for (buttons, what) in [(&[Down, A][..], "Cancel"), (&[B][..], "B")] {
        let mut guide = quitting();
        after(&mut guide, QUIT_TIMEOUT);
        assert_eq!(press(&mut guide, buttons).last(), Some(&PROMPT_CLOSE), "{what}");
        assert!(guide.guide_open() && guide.guide().dialog().is_none(), "{what}");
        assert!(!guide.quitting() && guide.playing() == Some(RUNNING), "{what}");
        assert_eq!(after(&mut guide, 60.0), [], "{what}: it is not asked about again");
        // It can be quit anew.
        assert_eq!(press(&mut guide, &[Menu, A]), [PROMPT_OPEN, SELECT, Effect::QuitGame], "{what}");
    }
}

#[test]
fn the_guide_closing_under_force_quit_declines_it() {
    // By a tap of the Guide button.
    let mut guide = quitting();
    after(&mut guide, QUIT_TIMEOUT);
    assert_eq!(press(&mut guide, &[Guide]), [CLOSED]);
    assert!(!guide.quitting());
    // By a hold of it: Power takes the prompt's place.
    let mut guide = quitting();
    after(&mut guide, QUIT_TIMEOUT);
    guide.pad_event(PadEvent::GuideHeld);
    assert!(matches!(guide.guide().dialog(), Some(DialogFocus::Power(_))));
    assert!(!guide.quitting());
}

#[test]
fn the_shell_finding_nothing_to_end_gives_the_quit_up() {
    let mut guide = quitting();
    after(&mut guide, QUIT_TIMEOUT);
    press(&mut guide, &[A]);
    guide.quit_given_up();
    assert!(!guide.quitting());
    assert_eq!(after(&mut guide, 60.0), []);
}

#[test]
fn force_quit_comes_up_over_a_guide_that_was_closed_meanwhile() {
    let mut guide = quitting();
    assert_eq!(press(&mut guide, &[B]), [CLOSED]);
    assert_eq!(guide.guide_line(), "closed, quitting");
    // The Guide opens for it, panel and all, with the prompt's sound and not the Guide's.
    assert_eq!(after(&mut guide, QUIT_TIMEOUT), [PROMPT_OPEN]);
    assert!(guide.guide_open() && guide.guide().panel_shown());
    assert_eq!(guide.guide().focus(), (Tab::Landing, Focus::Entry(0)));
    assert_eq!(guide.guide().dialog(), prompt(0));
    assert_eq!(guide.overlay(), OverlayShows::Guide);
    // Declined, the Guide is left open.
    assert_eq!(press(&mut guide, &[B]), [PROMPT_CLOSE]);
    assert!(guide.guide_open() && !guide.quitting());
}

#[test]
fn with_the_guide_setting_off_force_quit_shows_on_its_own_and_goes_when_answered() {
    let mut guide = quitting();
    press(&mut guide, &[B]);
    guide.set_guide_enabled(false);
    assert_eq!(after(&mut guide, QUIT_TIMEOUT), [PROMPT_OPEN]);
    assert!(guide.guide_open() && !guide.guide().panel_shown());
    assert_eq!(press(&mut guide, &[Down, A]), [MOVE, PROMPT_CLOSE, CLOSED]);
    assert!(!guide.guide_open() && !guide.quitting());

    // And goes with the game, should that stop by itself after all.
    let mut guide = quitting();
    press(&mut guide, &[B]);
    guide.set_guide_enabled(false);
    after(&mut guide, QUIT_TIMEOUT);
    guide.game_ended();
    assert_eq!(guide.take_effects(), [CLOSED]);
    assert!(!guide.guide_open());
}

#[test]
fn a_game_being_quit_is_not_held_and_the_core_says_so() {
    let mut guide = on_running_game();
    assert!(guide.guide_over_game() && !guide.quitting());
    press(&mut guide, &[Menu, A]);
    assert!(guide.quitting(), "the shell does not freeze a game that is quitting");
    guide.game_ended();
    assert!(!guide.quitting());
}

// ---- A recent game while another runs.

#[test]
fn a_recent_game_while_another_runs_asks_quit_x_and_start_y() {
    let mut guide = on_a_recent_game();
    assert_eq!(press(&mut guide, &[A]), [SELECT, PROMPT_OPEN]);
    assert!(guide.guide_open());
    assert_eq!(guide.guide().dialog(), prompt(0));
    assert_eq!(guide.guide().dialog_title(), Some("Quit Sea of Thieves and start Halo Infinite?"));
    // Cancel, and B, put it away and change nothing.
    assert_eq!(press(&mut guide, &[Down, A]), [MOVE, PROMPT_CLOSE]);
    assert_eq!((guide.guide().dialog(), guide.guide().focus()), (None, ON_HALO));
    assert_eq!(press(&mut guide, &[A, B]), [SELECT, PROMPT_OPEN, PROMPT_CLOSE]);
    assert!(guide.guide_open() && !guide.quitting() && guide.playing() == Some(RUNNING));
    assert_eq!(press(&mut guide, &[Up]), [MOVE]);
}

#[test]
fn quit_and_start_quits_the_running_game_and_starts_the_other_when_it_has_stopped() {
    let mut guide = on_a_recent_game();
    press(&mut guide, &[A]);
    assert_eq!(press(&mut guide, &[A]), [PROMPT_CLOSE, Effect::QuitGame]);
    assert!(guide.guide_open() && guide.guide().dialog().is_none());
    assert_eq!(guide.guide_line(), "Landing Entry(3), quitting, then Halo Infinite");
    // The game stops: the Guide closes at once and the other is asked of the shell.
    guide.game_ended();
    assert_eq!(guide.take_effects(), [CLOSED, Effect::Launch(0)]);
    assert!(!guide.guide_open() && !guide.awaiting_release());
    assert_eq!(guide.timer(), None);
    assert_eq!(guide.home_focus(), screens::home::Focus::Recent(0), "Home's focus is on the game that starts");
    guide.launch_started(0);
    assert!(guide.launch_screen_showing() && !guide.quitting());
}

#[test]
fn the_game_to_start_is_the_same_game_in_a_newer_snapshot_of_the_library() {
    let mut guide = on_a_recent_game();
    press(&mut guide, &[A, A]);
    guide.take_effects();
    // Halo Infinite is the library's third now.
    let mut next = screens::fake_library();
    let halo = next.games.remove(0);
    next.games.insert(2, halo);
    guide.set_library(next);
    guide.game_ended();
    assert_eq!(guide.take_effects(), [CLOSED, Effect::Launch(2)]);
}

#[test]
fn a_game_that_will_not_stop_for_quit_and_start_is_asked_about_and_declining_starts_nothing() {
    let mut guide = on_a_recent_game();
    press(&mut guide, &[A, A]);
    guide.take_effects();
    assert_eq!(after(&mut guide, QUIT_TIMEOUT), [PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog_title(), Some("Force quit Sea of Thieves?"));
    assert_eq!(press(&mut guide, &[Down, A]), [MOVE, PROMPT_CLOSE]);
    assert!(!guide.quitting());
    // It stops later, by itself: nothing starts, and the Guide stays.
    guide.game_ended();
    assert_eq!(guide.take_effects(), []);
    assert!(guide.guide_open());

    // Forced, the other game starts when it has gone.
    let mut guide = on_a_recent_game();
    press(&mut guide, &[A, A]);
    after(&mut guide, QUIT_TIMEOUT);
    assert_eq!(press(&mut guide, &[A]), [PROMPT_CLOSE, Effect::ForceQuitGame]);
    guide.game_ended();
    assert_eq!(guide.take_effects(), [CLOSED, Effect::Launch(0)]);
}

#[test]
fn while_a_game_is_quitting_a_on_another_sounds_and_starts_nothing() {
    let mut guide = quitting();
    assert_eq!(press(&mut guide, &[Down, A]), [MOVE, SELECT]);
    assert!(guide.guide_open() && guide.guide().dialog().is_none());
}

#[test]
fn with_nothing_running_a_recent_game_starts_as_from_home() {
    let mut guide = Dashboard::new(screens::fake_library());
    assert_eq!(press(&mut guide, &[Guide, Down, Down, Down, A])[4..], [SELECT, CLOSED, Effect::Launch(1)]);
    assert!(!guide.guide_open());
}

#[test]
fn the_prompt_goes_in_silence_when_the_game_it_asks_about_stops_by_itself() {
    let mut guide = on_a_recent_game();
    press(&mut guide, &[A]);
    guide.game_ended();
    assert_eq!(guide.take_effects(), []);
    assert_eq!(guide.guide().dialog(), None);
    assert!(guide.guide_open());
    assert_eq!(guide.guide_line(), "Landing Entry(3)");
}

// ---- Home, and the pages, from the Guide over a game.

#[test]
fn home_from_the_guide_over_a_game_brings_the_dashboard_in_front_of_it() {
    let mut guide = in_game();
    assert!(guide.home_covered());
    assert_eq!(press(&mut guide, &[Guide, A])[1..], [SELECT, CLOSED, Effect::ShowHome]);
    assert!(!guide.guide_open());
    assert!(guide.in_front_of_game() && !guide.home_covered(), "Home draws again");
    assert_eq!(guide.playing(), Some(RUNNING), "the game runs on behind it");
    // Home takes the pad again.
    assert_eq!(press(&mut guide, &[Down]), [Effect::Sound(Sound::Move)]);
    assert_eq!(guide.home_focus(), screens::home::Focus::Recent(0));
}

#[test]
fn in_front_of_the_game_the_guide_lists_it_running_and_a_on_it_goes_back() {
    let mut guide = in_game();
    press(&mut guide, &[Guide, A]);
    press(&mut guide, &[Guide, Down, Down]);
    assert_eq!(guide.guide().entries()[2].kind, Kind::Running(RUNNING));
    assert!(guide.guide_over_game(), "the game is held under the Guide wherever the Guide opened");
    assert_eq!(press(&mut guide, &[A]), [SELECT, CLOSED, Effect::ShowGame]);
    assert!(!guide.in_front_of_game() && guide.home_covered());
    // Behind the game again, Home takes no input.
    assert_eq!(press(&mut guide, &[Down, A]), []);
}

#[test]
fn in_front_of_the_game_its_own_tile_goes_back_to_it_and_another_game_is_asked_about() {
    let mut home = in_game();
    press(&mut home, &[Guide, A, Down]);
    // Halo Infinite's tile: the question, over the Guide, which opens for it without the Guide's own sound.
    assert_eq!(press(&mut home, &[A]), [SELECT, PROMPT_OPEN]);
    assert!(home.guide_open() && home.guide().panel_shown());
    assert_eq!(home.guide().dialog_title(), Some("Quit Sea of Thieves and start Halo Infinite?"));
    assert_eq!(press(&mut home, &[A]), [PROMPT_CLOSE, Effect::QuitGame]);
    home.game_ended();
    assert_eq!(home.take_effects(), [CLOSED, Effect::Launch(0)]);

    // The running game's own tile: back to it.
    let mut home = in_game();
    press(&mut home, &[Guide, A, Down, ui::Button::Right, ui::Button::Right]);
    assert_eq!(home.focused_game(), Some(RUNNING));
    assert_eq!(press(&mut home, &[A]).last(), Some(&Effect::ShowGame));
    assert!(!home.in_front_of_game() && !home.guide_open());
}

#[test]
fn a_game_ending_behind_the_dashboard_leaves_it_at_home() {
    let mut home = in_game();
    press(&mut home, &[Guide, A]);
    home.game_ended();
    assert_eq!(home.take_effects(), []);
    assert!(!home.in_front_of_game() && !home.home_covered() && home.playing().is_none());
}

// ---- A game ending with the Guide open.

#[test]
fn a_game_ending_by_itself_leaves_the_guide_open_and_its_list_without_the_game() {
    let mut guide = on_a_recent_game();
    guide.game_ended();
    assert_eq!(guide.take_effects(), [], "nothing is asked and nothing sounds");
    assert!(guide.guide_open());
    assert_eq!(guide.overlay(), OverlayShows::Guide, "the foreground stays the Guide's");
    // Focus stays at the same place in the list, whatever is there now.
    assert_eq!(guide.guide().focus(), ON_HALO);
    let kinds: Vec<Kind> = guide.guide().entries().iter().map(|entry| entry.kind).collect();
    assert_eq!(kinds[2..], [Kind::Recent(0), Kind::Recent(1), Kind::Recent(2), Kind::Recent(3)]);
    assert_eq!(guide.timer(), None, "it is not closed for the user");
    assert_eq!(after(&mut guide, 5.0), []);
    assert!(guide.guide_open());
    // Closing it then lands on Home: nothing is running to go back to.
    assert_eq!(press(&mut guide, &[B]), [CLOSED]);
    assert!(!guide.home_covered() && guide.playing().is_none());
    assert_eq!(guide.home_focus(), screens::home::Focus::Recent(RUNNING));
}

#[test]
fn a_game_ending_with_focus_on_its_entry_leaves_focus_at_that_place() {
    let mut guide = on_running_game();
    guide.game_ended();
    assert_eq!(guide.guide().focus(), ON_RUNNING);
    assert_eq!(guide.guide().entries()[2].kind, Kind::Recent(0));
    // What is there now is a game like any other: A starts it.
    assert_eq!(press(&mut guide, &[A]), [SELECT, CLOSED, Effect::Launch(0)]);
}
