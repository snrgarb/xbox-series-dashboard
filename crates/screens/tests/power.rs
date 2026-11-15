//! The Guide's dialogs: Power, from its entry and by a hold of the Guide button, and the running game's menu
//! (docs/spec/guide.md, sections 6 and 7.1). What a press does, what it sounds like, and what is asked of the
//! shell. Nothing here can turn anything off: what Power's options ask for is an effect, and the test reads it.
use screens::guide::{DialogFocus, Focus, Tab};
use screens::wide_dialog::Focus as PowerFocus;
use screens::{Dashboard, Effect, OverlayShows, PowerCommand, Sound};
use ui::Button::{self, Down, Guide, Left, LeftBumper, Menu, Right, RightBumper, Up, A, B};
use ui::{PadEvent, Press};

const SELECT: Effect = Effect::Sound(Sound::GuideSelect);
const MOVE: Effect = Effect::Sound(Sound::GuideMove);
const PROMPT_OPEN: Effect = Effect::Sound(Sound::PromptOpen);
const PROMPT_CLOSE: Effect = Effect::Sound(Sound::PromptClose);
const POWER_OFF: Effect = Effect::Sound(Sound::PowerOff);
const CLOSED: Effect = Effect::GuideClosed;

const ON_CLOSE: Option<DialogFocus> = Some(DialogFocus::Power(PowerFocus::Close));

fn on_tile(at: usize) -> Option<DialogFocus> {
    Some(DialogFocus::Power(PowerFocus::Tile(at)))
}

/// Presses each button and gives everything the presses asked for.
fn press(dashboard: &mut Dashboard, buttons: &[Button]) -> Vec<Effect> {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
    dashboard.take_effects()
}

/// The Guide button held to its 0.7 s.
fn hold(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.pad_event(PadEvent::GuideHeld);
    dashboard.take_effects()
}

fn home() -> Dashboard {
    Dashboard::new(screens::fake_library())
}

/// A game running, its Launch screen gone.
fn in_game() -> Dashboard {
    let mut dashboard = home();
    dashboard.launch_started(2);
    dashboard.game_booted();
    dashboard
}

/// The Guide on Profile & system with focus on Power, its last entry.
fn on_power_entry(mut dashboard: Dashboard) -> Dashboard {
    press(&mut dashboard, &[Guide, RightBumper, Down, Down, Down]);
    assert_eq!(dashboard.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(3)));
    dashboard
}

#[test]
fn a_on_the_power_entry_opens_power_over_the_guide_with_focus_on_close() {
    let mut guide = on_power_entry(home());
    assert_eq!(press(&mut guide, &[A]), [SELECT, PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
    assert!(guide.guide_open() && guide.guide().panel_shown());
}

#[test]
fn b_and_close_put_power_away_and_leave_the_guide_where_it_was() {
    let mut guide = on_power_entry(home());
    press(&mut guide, &[A]);
    assert_eq!(press(&mut guide, &[B]), [PROMPT_CLOSE]);
    assert_eq!(guide.guide().dialog(), None);
    assert_eq!(guide.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(3)));
    assert!(guide.guide_open());

    // A on Close is a choice like any other of Power's, and sounds as one.
    press(&mut guide, &[A]);
    assert_eq!(press(&mut guide, &[A]), [SELECT]);
    assert_eq!(guide.guide().dialog(), None);
    assert_eq!(guide.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(3)));
    assert!(guide.guide_open());
}

#[test]
fn powers_focus_moves_over_its_tiles_with_the_guides_move_sound_and_is_silent_at_an_edge() {
    let mut guide = on_power_entry(home());
    press(&mut guide, &[A]);
    for button in [Down, Left, Right] {
        assert_eq!(press(&mut guide, &[button]), [], "{button:?} on Close");
    }
    // Up to Exit to desktop, alone on the second row; up to Turn off PC; along to Sleep; down to the last tile.
    assert_eq!(press(&mut guide, &[Up]), [MOVE]);
    assert_eq!(guide.guide().dialog(), on_tile(3));
    assert_eq!(press(&mut guide, &[Right]), []);
    assert_eq!(press(&mut guide, &[Up]), [MOVE]);
    assert_eq!(guide.guide().dialog(), on_tile(0));
    assert_eq!(press(&mut guide, &[Up]), []);
    assert_eq!(press(&mut guide, &[Left]), []);
    assert_eq!(press(&mut guide, &[Right, Right]), [MOVE, MOVE]);
    assert_eq!(guide.guide().dialog(), on_tile(2));
    assert_eq!(press(&mut guide, &[Right]), []);
    assert_eq!(press(&mut guide, &[Down]), [MOVE]);
    assert_eq!(guide.guide().dialog(), on_tile(3));
    assert_eq!(press(&mut guide, &[Down]), [MOVE]);
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
    // A direction held keeps moving.
    guide.press(Press { button: Up, repeat: true });
    assert_eq!(guide.take_effects(), [MOVE]);
}

#[test]
fn a_dialog_takes_the_pad_from_the_guide() {
    let mut guide = on_power_entry(home());
    press(&mut guide, &[A]);
    let others = [LeftBumper, RightBumper, Menu, Button::X, Button::Y, Button::View];
    for button in others {
        assert_eq!(press(&mut guide, &[button]), [], "{button:?}");
    }
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
    assert_eq!(guide.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(3)), "the Guide under it has not moved");
    // A held A or B does nothing more.
    guide.press(Press { button: A, repeat: true });
    guide.press(Press { button: B, repeat: true });
    assert_eq!(guide.take_effects(), []);
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
}

#[test]
fn a_power_option_closes_the_guide_and_then_asks_the_shell() {
    let options = [
        (&[Up, Up][..], Effect::Power(PowerCommand::TurnOff)),
        (&[Up, Up, Right][..], Effect::Power(PowerCommand::Restart)),
        (&[Up, Up, Right, Right][..], Effect::Power(PowerCommand::Sleep)),
    ];
    for (way, effect) in options {
        let mut guide = on_power_entry(home());
        press(&mut guide, &[A]);
        press(&mut guide, way);
        assert_eq!(press(&mut guide, &[A]), [SELECT, CLOSED, effect]);
        assert!(!guide.guide_open());
        assert_eq!(guide.guide().dialog(), None);
    }
}

#[test]
fn exit_to_desktop_closes_the_guide_plays_the_sound_of_leaving_and_exits() {
    let mut guide = on_power_entry(home());
    press(&mut guide, &[A, Up]);
    assert_eq!(press(&mut guide, &[A]), [SELECT, CLOSED, POWER_OFF, Effect::Exit]);
    assert!(!guide.guide_open());
}

#[test]
fn power_is_the_same_over_a_game() {
    let mut guide = on_power_entry(in_game());
    assert_eq!(press(&mut guide, &[A]), [SELECT, PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
    assert_eq!(guide.overlay(), OverlayShows::Guide);
    assert_eq!(
        press(&mut guide, &[Up, Up, Right, Right, A]),
        [MOVE, MOVE, MOVE, MOVE, SELECT, CLOSED, Effect::Power(PowerCommand::Sleep)]
    );
    assert_eq!(guide.playing(), Some(2), "the game is still running");
}

#[test]
fn a_tap_of_the_guide_button_closes_the_guide_and_its_dialog_in_silence() {
    let mut guide = on_power_entry(home());
    press(&mut guide, &[A, Up]);
    assert_eq!(press(&mut guide, &[Guide]), [CLOSED]);
    assert!(!guide.guide_open());
    // The Guide opens next as it always does: no dialog, the landing tab.
    press(&mut guide, &[Guide]);
    assert_eq!(guide.guide().dialog(), None);
    assert_eq!(guide.guide().focus(), (Tab::Landing, Focus::Entry(0)));
}

#[test]
fn a_hold_opens_power_alone_and_closing_it_closes_everything() {
    let mut dashboard = home();
    assert_eq!(hold(&mut dashboard), [PROMPT_OPEN]);
    assert!(dashboard.guide_open());
    assert!(!dashboard.guide().panel_shown());
    assert_eq!(dashboard.guide().dialog(), ON_CLOSE);
    assert_eq!(press(&mut dashboard, &[B]), [PROMPT_CLOSE, CLOSED]);
    assert!(!dashboard.guide_open());

    // Close does the same.
    hold(&mut dashboard);
    assert_eq!(press(&mut dashboard, &[A]), [SELECT, CLOSED]);
    assert!(!dashboard.guide_open());

    // And an option is carried out as from the entry.
    hold(&mut dashboard);
    assert_eq!(
        press(&mut dashboard, &[Up, Up, Right, A]),
        [MOVE, MOVE, MOVE, SELECT, CLOSED, Effect::Power(PowerCommand::Restart)]
    );

    // The Guide opened after has its panel again.
    press(&mut dashboard, &[Guide]);
    assert!(dashboard.guide().panel_shown());
    assert_eq!(dashboard.guide().dialog(), None);
}

#[test]
fn a_hold_with_the_guide_open_takes_the_panel_away_and_whatever_dialog_was_up() {
    let mut guide = on_power_entry(home());
    assert_eq!(hold(&mut guide), [PROMPT_OPEN]);
    assert!(guide.guide_open() && !guide.guide().panel_shown());
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
    // There is no panel to go back to: B closes the Guide.
    assert_eq!(press(&mut guide, &[B]), [PROMPT_CLOSE, CLOSED]);

    // Power from the entry, focus moved, then a hold: Power anew, focus on Close, and no panel.
    let mut guide = on_power_entry(home());
    press(&mut guide, &[A, Up]);
    assert_eq!(hold(&mut guide), [PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
    assert_eq!(press(&mut guide, &[A]), [SELECT, CLOSED]);

    // The running game's menu gives way to it too.
    let mut guide = in_game();
    press(&mut guide, &[Guide, Down, Down, Menu]);
    assert!(matches!(guide.guide().dialog(), Some(DialogFocus::Menu { .. })));
    hold(&mut guide);
    assert_eq!(guide.guide().dialog(), ON_CLOSE);
}

#[test]
fn a_hold_over_a_game_opens_power_on_the_overlay() {
    let mut dashboard = in_game();
    assert_eq!(hold(&mut dashboard), [PROMPT_OPEN]);
    assert!(dashboard.guide_over_game());
    assert_eq!(dashboard.overlay(), OverlayShows::Guide);
    assert_eq!(press(&mut dashboard, &[B]), [PROMPT_CLOSE, CLOSED]);
    assert!(!dashboard.guide_over_game());
}

#[test]
fn a_hold_does_nothing_where_the_guide_button_does_nothing() {
    // The Guide setting off.
    let mut dashboard = home();
    dashboard.set_guide_enabled(false);
    assert_eq!(hold(&mut dashboard), []);
    assert!(!dashboard.guide_open());

    // While a game starts.
    let mut dashboard = home();
    dashboard.launch_started(2);
    assert_eq!(hold(&mut dashboard), []);
    assert!(!dashboard.guide_open());

    // Over a game with no Overlay to show it on.
    let mut dashboard = home();
    dashboard.set_overlay(false);
    dashboard.launch_started(2);
    dashboard.game_booted();
    assert_eq!(hold(&mut dashboard), []);
    assert!(!dashboard.guide_open());
}

#[test]
fn menu_on_the_running_game_opens_its_menu_whose_one_option_quits_it() {
    let mut guide = in_game();
    press(&mut guide, &[Guide, Down, Down]);
    assert_eq!(press(&mut guide, &[Menu]), [PROMPT_OPEN]);
    assert_eq!(guide.guide().dialog(), Some(DialogFocus::Menu { prompt: false, row: 0 }));
    // One row: nowhere to move.
    assert_eq!(press(&mut guide, &[Up, Down, Left, Right, LeftBumper, Menu]), []);

    // B puts it away; the Guide is where it was.
    assert_eq!(press(&mut guide, &[B]), [PROMPT_CLOSE]);
    assert_eq!(guide.guide().dialog(), None);
    assert_eq!(guide.guide().focus(), (Tab::Landing, Focus::Entry(2)));

    // Quit: the menu goes, the Guide stays until the game has.
    press(&mut guide, &[Menu]);
    assert_eq!(press(&mut guide, &[A]), [SELECT, Effect::QuitGame]);
    assert_eq!(guide.guide().dialog(), None);
    assert!(guide.guide_open());
}

#[test]
fn a_menu_whose_game_stops_by_itself_goes_with_it() {
    let mut guide = in_game();
    press(&mut guide, &[Guide, Down, Down, Menu]);
    guide.game_ended();
    assert_eq!(guide.take_effects(), []);
    assert_eq!(guide.guide().dialog(), None);
    assert!(!guide.guide_open());
}

#[test]
fn homes_profile_opens_the_guide_on_profile_and_system() {
    let mut dashboard = home();
    // Home starts with focus on the profile.
    assert_eq!(press(&mut dashboard, &[A]), [Effect::Sound(Sound::GuideOpen)]);
    assert_eq!(dashboard.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(0)));
    // The Guide button's is the landing tab, as ever.
    press(&mut dashboard, &[B, Guide]);
    assert_eq!(dashboard.guide().focus(), (Tab::Landing, Focus::Entry(0)));
}
