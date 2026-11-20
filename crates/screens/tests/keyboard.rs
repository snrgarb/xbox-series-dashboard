//! Text entry on the Console keyboard, through the Dashboard (docs/spec/search.md, sections 8 to 10): opened by the
//! stand-in page that Full library's name box, Settings' Groups will take the place of
//! (`screens::keyboard::stand_in`), typed on by walking the keys, by the pad's shortcuts and on a physical
//! keyboard, kept and dropped. Judged by the focus log's line, as a test on the VM judges. The board's own rules,
//! every edge of it among them, are unit tests beside it (`keyboard/board.rs`); the pictures are tests/shots.rs.
use screens::keyboard::{Typed, REPEAT};
use screens::pages::Open;
use screens::{Dashboard, Effect, Sound};
use ui::Button::{
    self, Down, Guide, Left, LeftBumper, LeftStick, LeftTrigger, Menu, Right, RightBumper, RightTrigger, Up, View, A,
    B, X, Y,
};
use ui::{PadEvent, Press, Repeat};

/// The stand-in page over Home, focus on its Name box.
fn stand_in() -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.open_page(Open::KeyboardStandIn);
    dashboard.take_effects();
    dashboard
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// The focus line after these presses.
fn after(dashboard: &mut Dashboard, buttons: &[Button]) -> String {
    press(dashboard, buttons);
    dashboard.focus_line()
}

fn sounds(dashboard: &mut Dashboard) -> Vec<Sound> {
    let sound = |effect| match effect {
        Effect::Sound(sound) => Some(sound),
        _ => None,
    };
    dashboard.take_effects().into_iter().filter_map(sound).collect()
}

/// "Ha" typed by walking, as the Bundle's picture of text entry has it: H is five along the second row, and A is
/// back at its start.
const HA: [Button; 13] = [Down, Right, Right, Right, Right, Right, A, Left, Left, Left, Left, Left, A];

#[test]
fn text_entry_opens_over_the_page_on_q_with_shift_on_and_the_page_keeps_its_place() {
    let mut dashboard = stand_in();
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"\" group \"Couch co-op\" secret 0, nothing");
    assert_eq!(after(&mut dashboard, &[A]), "TextEntry: Q; \"|\" upper letters");
    assert_eq!(dashboard.pages_up(), 2, "text entry is over the page, which stays under it");
    assert_eq!(dashboard.hints(), screens::hint_bar::NONE);
    // Over a text it starts with the caret at its end and Shift off.
    press(&mut dashboard, &[B, Down]);
    assert_eq!(after(&mut dashboard, &[A]), "TextEntry: Q; \"Couch co-op|\" lower letters");
}

#[test]
fn a_name_typed_by_walking_the_keys_narrows_the_page_as_it_goes_and_menu_keeps_it() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[A]);
    // The first character is a capital; Shift goes off with it.
    assert_eq!(after(&mut dashboard, &HA[..7]), "TextEntry: H; \"H|\" lower letters");
    assert_eq!(after(&mut dashboard, &HA[7..]), "TextEntry: A; \"Ha|\" lower letters");
    // The page under it heard each change at once: B from here would have to put the old name back.
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"Ha\" group \"Couch co-op\" secret 0, kept");
    assert_eq!(dashboard.pages_up(), 1);
    // Opened again it starts from the name kept, and A on the Enter key keeps as Menu does.
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), "TextEntry: Q; \"Ha|\" lower letters");
    press(&mut dashboard, &[Right, Right, Right, A]);
    press(&mut dashboard, &[Down, Down]);
    press(&mut dashboard, &[Right; 8]);
    assert_eq!(dashboard.focus_line(), "TextEntry: Enter; \"Har|\" lower letters");
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"Har\" group \"Couch co-op\" secret 0, kept");
}

#[test]
fn b_drops_the_text_and_the_page_puts_back_what_it_had() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[A]);
    press(&mut dashboard, &HA);
    press(&mut dashboard, &[Menu, A, X, Y, A]);
    assert_eq!(dashboard.focus_line(), "TextEntry: Q; \"H q|\" lower letters");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"Ha\" group \"Couch co-op\" secret 0, dropped");
}

#[test]
fn a_name_is_typed_and_mended_with_the_pads_shortcuts() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[A]);
    // Q, Y's space, w; LB twice and a character in at the caret; X before it; LS for a capital; LT for a symbol.
    assert_eq!(after(&mut dashboard, &[A, Y, Right, A]), "TextEntry: W; \"Q w|\" lower letters");
    assert_eq!(after(&mut dashboard, &[LeftBumper, LeftBumper, A]), "TextEntry: W; \"Qw| w\" lower letters");
    assert_eq!(after(&mut dashboard, &[X, RightBumper]), "TextEntry: W; \"Q |w\" lower letters");
    assert_eq!(after(&mut dashboard, &[LeftStick]), "TextEntry: W; \"Q |w\" upper letters");
    assert_eq!(after(&mut dashboard, &[A]), "TextEntry: W; \"Q W|w\" lower letters");
    assert_eq!(after(&mut dashboard, &[LeftTrigger]), "TextEntry: W; \"Q W|w\" lower symbols");
    assert_eq!(after(&mut dashboard, &[A]), "TextEntry: W; \"Q W@|w\" lower symbols");
    assert_eq!(after(&mut dashboard, &[LeftTrigger]), "TextEntry: W; \"Q W@|w\" lower letters");
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"Q W@w\" group \"Couch co-op\" secret 0, kept");
}

#[test]
fn every_button_is_text_entrys_and_none_reaches_the_page_or_leaves_the_keys() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[A]);
    // Up from the top row goes nowhere: the field takes no focus. Down from the foot neither.
    assert_eq!(after(&mut dashboard, &[Up]), "TextEntry: 1; \"|\" upper letters");
    assert_eq!(after(&mut dashboard, &[Up, Up]), "TextEntry: 1; \"|\" upper letters");
    assert_eq!(after(&mut dashboard, &[Down, Down, Down, Down, Down]), "TextEntry: Space; \"|\" upper letters");
    // What the keyboard has no use for is swallowed: no Search for View or RT, and the page hears nothing.
    let line = dashboard.focus_line();
    assert_eq!(after(&mut dashboard, &[View, RightTrigger]), line);
    assert_eq!(dashboard.pages_up(), 2);
    assert!(dashboard.take_effects().iter().all(|effect| matches!(effect, Effect::Sound(_))));
    // Y is the space bar here, not Search.
    assert_eq!(after(&mut dashboard, &[Y]), "TextEntry: Space; \" |\" lower letters");
    assert_eq!(dashboard.page(), Some("TextEntry"));
}

#[test]
fn a_group_name_changes_only_when_kept_stops_at_forty_characters_and_is_not_kept_blank() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[Down, A]);
    press(&mut dashboard, &[A; 40]);
    let forty = format!("Couch co-op{}", "q".repeat(29));
    assert_eq!(dashboard.focus_line(), format!("TextEntry: Q; \"{forty}|\" lower letters"));
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), format!("Keyboard: Group; name \"\" group \"{forty}\" secret 0, kept"));
    // Emptied and kept: the name stays.
    press(&mut dashboard, &[A]);
    press(&mut dashboard, &[X; 40]);
    assert_eq!(after(&mut dashboard, &[Y]), "TextEntry: Q; \" |\" lower letters");
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), format!("Keyboard: Group; name \"\" group \"{forty}\" secret 0, kept"));
}

#[test]
fn a_secret_starts_empty_is_never_said_and_kept_empty_leaves_the_saved_one() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[Down, Down, A]);
    assert_eq!(after(&mut dashboard, &[A, A, A]), "TextEntry: Q; \"***|\" lower letters");
    assert_eq!(after(&mut dashboard, &[LeftBumper]), "TextEntry: Q; \"**|*\" lower letters");
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Secret; name \"\" group \"Couch co-op\" secret 3, kept");
    // Opened again it is empty: the saved secret is not shown, not even as dots.
    assert_eq!(after(&mut dashboard, &[A]), "TextEntry: Q; \"|\" upper letters");
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Secret; name \"\" group \"Couch co-op\" secret 3, kept");
}

#[test]
fn a_physical_keyboard_types_into_text_entry_and_nowhere_else() {
    let mut dashboard = stand_in();
    // With no text entry up nothing takes text: Backspace is then B, which is the window's to press.
    assert!(!dashboard.typed(Typed::Text("x")));
    assert!(!dashboard.typed(Typed::Backspace));
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"\" group \"Couch co-op\" secret 0, nothing");
    press(&mut dashboard, &[A, Right]);
    // As typed, its own case whatever Shift says; Space is a space and never presses the focused key.
    assert!(dashboard.typed(Typed::Text("h")));
    assert!(dashboard.typed(Typed::Text("A")));
    assert!(dashboard.typed(Typed::Text(" ")));
    assert_eq!(dashboard.focus_line(), "TextEntry: W; \"hA |\" lower letters");
    assert!(dashboard.typed(Typed::Backspace));
    assert!(dashboard.typed(Typed::Text("\r")), "a control character is taken and types nothing");
    assert_eq!(dashboard.focus_line(), "TextEntry: W; \"hA|\" lower letters");
    // The page heard each change.
    press(&mut dashboard, &[Menu]);
    assert_eq!(dashboard.focus_line(), "Keyboard: Name; name \"hA\" group \"Couch co-op\" secret 0, kept");
    // Under the Guide the keyboard's text goes nowhere.
    press(&mut dashboard, &[A, Guide]);
    assert!(!dashboard.typed(Typed::Text("x")));
    press(&mut dashboard, &[Guide]);
    assert_eq!(dashboard.focus_line(), "TextEntry: Q; \"hA|\" lower letters");
}

#[test]
fn moving_between_keys_sounds_and_typing_an_edge_and_a_shortcut_do_not() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[A]);
    assert_eq!(sounds(&mut dashboard), [Sound::Activate], "A that opened text entry chose something");
    press(&mut dashboard, &[Right, Down]);
    assert_eq!(sounds(&mut dashboard), [Sound::Move, Sound::Move]);
    // A key pressed, a shortcut, Shift and the layer turning: focus is where it was, so nothing sounds.
    press(&mut dashboard, &[A, X, Y, LeftBumper, RightBumper, LeftStick, LeftTrigger]);
    assert_eq!(sounds(&mut dashboard), []);
    // An edge: Left from the caret's key goes nowhere.
    press(&mut dashboard, &[Left, Left]);
    assert_eq!(sounds(&mut dashboard), [Sound::Move, Sound::Move]);
    press(&mut dashboard, &[Left]);
    assert_eq!(sounds(&mut dashboard), []);
    // B and Menu close it in silence.
    press(&mut dashboard, &[B]);
    assert_eq!(sounds(&mut dashboard), []);
}

#[test]
fn a_held_direction_repeats_at_the_bundles_pace_on_the_keys_and_at_the_pages_under_them() {
    let mut dashboard = stand_in();
    assert_eq!(dashboard.repeat(), Repeat::default());
    press(&mut dashboard, &[A]);
    assert_eq!((dashboard.repeat(), REPEAT), (Repeat::millis(700, 80), Repeat::millis(700, 80)));
    // A repeat moves focus on; a repeat of anything else does nothing.
    dashboard.press(Press { button: Right, repeat: true });
    dashboard.press(Press { button: A, repeat: true });
    dashboard.press(Press { button: B, repeat: true });
    assert_eq!(dashboard.focus_line(), "TextEntry: W; \"|\" upper letters");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.repeat(), Repeat::default());
}

#[test]
fn the_guide_opens_over_text_entry_which_is_there_after_and_a_game_ending_takes_it_away() {
    let mut dashboard = stand_in();
    press(&mut dashboard, &[A]);
    press(&mut dashboard, &HA);
    press(&mut dashboard, &[Guide]);
    assert!(dashboard.guide_open());
    // The Guide has the pad: nothing is typed under it.
    press(&mut dashboard, &[Down, Up]);
    assert_eq!(dashboard.focus_line(), "TextEntry: A; \"Ha|\" lower letters");
    press(&mut dashboard, &[B]);
    assert!(!dashboard.guide_open());
    assert_eq!(after(&mut dashboard, &[A]), "TextEntry: A; \"Haa|\" lower letters");
    // Whatever takes the page away takes text entry with it.
    dashboard.game_ended();
    assert_eq!(dashboard.pages_up(), 0);
}

#[test]
fn a_pads_button_lets_go_of_its_key_whatever_the_press_did() {
    // Nothing to see here without a picture (tests/shots.rs has it): this is that a press which closes text entry
    // and a release with no page up are both taken in stride.
    let mut dashboard = stand_in();
    let down = |button| PadEvent::Press(Press { button, repeat: false });
    dashboard.pad_event(down(A));
    dashboard.pad_event(PadEvent::Release(A));
    dashboard.pad_event(down(X));
    dashboard.pad_event(down(Menu));
    assert_eq!(dashboard.page(), Some("Keyboard"));
    dashboard.pad_event(PadEvent::Release(Menu));
    dashboard.pad_event(PadEvent::Release(X));
    dashboard.pad_event(down(B));
    dashboard.pad_event(PadEvent::Release(B));
    assert_eq!(dashboard.pages_up(), 0);
}
