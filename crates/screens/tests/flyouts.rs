//! The Guide's Flyouts (docs/spec/guide.md, section 5): Notifications and Audio, opened by A on their Utilities.
//! What a press does, what it sounds like, and what is asked of the shell: the system's sound is the shell's, so
//! here it is whatever the test says it is, and a change of it is an `Effect`.
use model::{Audio, AudioDevice, Flow};
use screens::guide::{Flyout, FlyoutFocus, Focus, Tab};
use screens::{Dashboard, Effect, Sound};
use ui::Button::{self, Down, Guide, Left, LeftBumper, Right, RightBumper, Up, A, B};
use ui::Press;

const SELECT: Effect = Effect::Sound(Sound::GuideSelect);
const MOVE: Effect = Effect::Sound(Sound::GuideMove);

const ON_NOTIFICATIONS: (Tab, Focus) = (Tab::Landing, Focus::Utility(0));
const ON_AUDIO: (Tab, Focus) = (Tab::Landing, Focus::Utility(1));
/// Audio's entries, top to bottom.
const OUTPUT: usize = 0;
const INPUT: usize = 1;
const VOLUME: usize = 2;
const MUTE: usize = 3;
const NOTHING_PLAYING: usize = 4;

fn press(dashboard: &mut Dashboard, buttons: &[Button]) -> Vec<Effect> {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
    dashboard.take_effects()
}

/// A direction held down, repeating.
fn hold(dashboard: &mut Dashboard, button: Button, times: usize) -> Vec<Effect> {
    for _ in 0..times {
        dashboard.press(Press { button, repeat: true });
    }
    dashboard.take_effects()
}

/// Two outputs, the first the default, and a microphone; volume 67.
fn sound() -> Audio {
    Audio {
        outputs: vec![AudioDevice::new("out-1", "Speakers"), AudioDevice::new("out-2", "TV")],
        output: Some(0),
        inputs: vec![AudioDevice::new("in-1", "Microphone")],
        input: Some(0),
        volume: 67,
        muted: false,
    }
}

/// The Guide open at Home, focus on the first Utility: Down from the landing tab's last entry.
fn on_utilities() -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.set_audio(sound());
    press(&mut dashboard, &[Guide]);
    let entries = dashboard.guide().entries().len();
    press(&mut dashboard, &vec![Down; entries]);
    assert_eq!(dashboard.guide().focus(), ON_NOTIFICATIONS);
    dashboard
}

/// The Audio Flyout open, focus on Output.
fn in_audio() -> Dashboard {
    let mut dashboard = on_utilities();
    press(&mut dashboard, &[Right, A]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Item(OUTPUT))));
    dashboard
}

fn item(at: usize) -> Option<(Flyout, FlyoutFocus)> {
    Some((Flyout::Audio, FlyoutFocus::Item(at)))
}

#[test]
fn a_on_notifications_opens_its_flyout_with_nothing_to_focus_when_there_are_none() {
    let mut dashboard = on_utilities();
    assert_eq!(press(&mut dashboard, &[A]), [SELECT]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Nothing)));
    assert_eq!(dashboard.guide_line(), "Landing Utility(0), flyout Notifications Nothing");
    // Only B does anything.
    assert_eq!(press(&mut dashboard, &[Down, Up, A, Left, Right]), []);
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Nothing)));
}

#[test]
fn notifications_are_listed_newest_first_and_focus_walks_them() {
    let mut dashboard = on_utilities();
    assert_eq!((dashboard.notify("first"), dashboard.notify("second"), dashboard.notify("third")), (1, 2, 3));
    let texts: Vec<&str> = dashboard.notifications().iter().map(|n| n.text.as_str()).collect();
    assert_eq!(texts, ["first", "second", "third"], "kept oldest first; the Flyout lists them the other way");

    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Item(0))));
    assert_eq!(press(&mut dashboard, &[Down]), [MOVE]);
    assert_eq!(press(&mut dashboard, &[Down]), [MOVE]);
    assert_eq!(press(&mut dashboard, &[Down]), [], "the last: no further, and no sound");
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Item(2))));
    assert_eq!(press(&mut dashboard, &[A]), [], "A on a notification does nothing, in silence");
    assert_eq!(press(&mut dashboard, &[Up, Up, Up]), [MOVE, MOVE]);
}

#[test]
fn a_notification_posted_while_the_flyout_is_open_joins_it() {
    let mut dashboard = on_utilities();
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Nothing)));
    dashboard.notify("something happened");
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Item(0))));
}

#[test]
fn b_closes_the_flyout_onto_its_utility_and_then_the_guide_in_silence() {
    let mut dashboard = in_audio();
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(press(&mut dashboard, &[B]), []);
    assert_eq!(dashboard.flyout(), None);
    assert!(dashboard.guide().is_open());
    assert_eq!(dashboard.guide().focus(), ON_AUDIO);
    assert_eq!(dashboard.guide_line(), "Landing Utility(1)");
    // Opened again it starts from its first entry: it remembers nothing.
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.flyout(), item(OUTPUT));
    press(&mut dashboard, &[B]);
    assert_eq!(press(&mut dashboard, &[B]), [Effect::GuideClosed]);
    assert!(!dashboard.guide().is_open());
}

#[test]
fn the_guide_button_closes_the_guide_whatever_is_open_and_it_opens_with_no_flyout() {
    let mut dashboard = in_audio();
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Device(Flow::Output, 0))));
    assert_eq!(press(&mut dashboard, &[Guide]), [Effect::GuideClosed]);
    assert_eq!((dashboard.guide().is_open(), dashboard.flyout()), (false, None));
    assert!(!dashboard.audio_shown());
    press(&mut dashboard, &[Guide]);
    assert_eq!((dashboard.guide().focus(), dashboard.flyout()), ((Tab::Landing, Focus::Entry(0)), None));
}

#[test]
fn the_bumpers_do_nothing_while_a_flyout_is_open() {
    let mut dashboard = in_audio();
    assert_eq!(press(&mut dashboard, &[RightBumper, LeftBumper]), []);
    assert_eq!((dashboard.guide().focus(), dashboard.flyout()), (ON_AUDIO, item(OUTPUT)));
}

#[test]
fn audio_has_five_entries_and_focus_stops_at_its_ends() {
    let mut dashboard = in_audio();
    assert!(dashboard.audio_shown());
    assert_eq!(press(&mut dashboard, &[Up]), []);
    assert_eq!(press(&mut dashboard, &[Down, Down, Down, Down]), [MOVE, MOVE, MOVE, MOVE]);
    assert_eq!(dashboard.flyout(), item(NOTHING_PLAYING));
    assert_eq!(press(&mut dashboard, &[Down]), []);
    assert_eq!(press(&mut dashboard, &[A, Left, Right]), [], "the line about music does nothing");
    assert_eq!(dashboard.guide_line(), "Landing Utility(1), flyout Audio Item(4), volume 67");
}

#[test]
fn left_and_right_on_volume_move_the_systems_volume_by_two() {
    let mut dashboard = in_audio();
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(dashboard.flyout(), item(VOLUME));
    // The sound of a move, and the level shows at once.
    assert_eq!(press(&mut dashboard, &[Right]), [MOVE, Effect::Volume(69)]);
    assert_eq!(dashboard.audio().volume, 69);
    assert_eq!(press(&mut dashboard, &[Left, Left]), [MOVE, Effect::Volume(67), MOVE, Effect::Volume(65)]);
    assert_eq!(dashboard.guide_line(), "Landing Utility(1), flyout Audio Item(2), volume 65");
    assert_eq!(press(&mut dashboard, &[A]), [], "A does nothing on Volume");
    // On the other entries Left and Right do nothing.
    press(&mut dashboard, &[Up]);
    assert_eq!(press(&mut dashboard, &[Left, Right]), []);
}

#[test]
fn the_volume_stops_at_its_ends_in_silence_and_a_held_direction_repeats() {
    let mut dashboard = in_audio();
    dashboard.set_audio(Audio { volume: 97, ..sound() });
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(hold(&mut dashboard, Right, 3), [MOVE, Effect::Volume(99), MOVE, Effect::Volume(100)]);
    assert_eq!(press(&mut dashboard, &[Right]), [], "at 100: nothing changed, so nothing is heard");

    dashboard.set_audio(Audio { volume: 1, ..sound() });
    assert_eq!(hold(&mut dashboard, Left, 2), [MOVE, Effect::Volume(0)]);
}

#[test]
fn a_on_mute_turns_the_systems_mute_on_and_off() {
    let mut dashboard = in_audio();
    press(&mut dashboard, &[Down, Down, Down]);
    assert_eq!(dashboard.flyout(), item(MUTE));
    assert_eq!(press(&mut dashboard, &[A]), [SELECT, Effect::Mute(true)]);
    assert!(dashboard.audio().muted);
    assert_eq!(dashboard.guide_line(), "Landing Utility(1), flyout Audio Item(3), volume 67, muted");
    assert_eq!(press(&mut dashboard, &[A]), [SELECT, Effect::Mute(false)]);
    assert!(!dashboard.audio().muted);
}

#[test]
fn a_dropdown_opens_its_list_on_the_default_device() {
    let mut dashboard = in_audio();
    dashboard.set_audio(Audio { output: Some(1), ..sound() });
    assert_eq!(press(&mut dashboard, &[A]), [SELECT]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Device(Flow::Output, 1))));
    assert_eq!(dashboard.guide_line(), "Landing Utility(1), flyout Audio Device(Output, 1), volume 67");
    // Up and Down move within it and stop at its ends; Left and Right do nothing.
    assert_eq!(press(&mut dashboard, &[Down]), []);
    assert_eq!(press(&mut dashboard, &[Up]), [MOVE]);
    assert_eq!(press(&mut dashboard, &[Up, Left, Right]), []);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Device(Flow::Output, 0))));
    // B closes it with nothing changed, focus back on the dropdown.
    assert_eq!(press(&mut dashboard, &[B]), []);
    assert_eq!(dashboard.flyout(), item(OUTPUT));
    assert_eq!(dashboard.audio().output, Some(1));
}

#[test]
fn a_on_a_device_makes_it_the_default_and_closes_the_list() {
    let mut dashboard = in_audio();
    press(&mut dashboard, &[A, Down]);
    assert_eq!(press(&mut dashboard, &[A]), [SELECT, Effect::DefaultDevice(Flow::Output, 1)]);
    assert_eq!(dashboard.flyout(), item(OUTPUT));
    assert_eq!(dashboard.audio().selection(Flow::Output), "TV", "shown at once");

    press(&mut dashboard, &[Down]);
    assert_eq!(dashboard.flyout(), item(INPUT));
    press(&mut dashboard, &[A]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Device(Flow::Input, 0))));
    assert_eq!(press(&mut dashboard, &[A]), [SELECT, Effect::DefaultDevice(Flow::Input, 0)]);
}

#[test]
fn with_no_devices_the_dropdowns_say_none_and_nothing_can_be_changed() {
    let mut dashboard = in_audio();
    dashboard.set_audio(Audio::default());
    assert_eq!(dashboard.audio().selection(Flow::Output), "None");
    assert_eq!(press(&mut dashboard, &[A]), [], "no list to open, and no sound");
    assert_eq!(dashboard.flyout(), item(OUTPUT));
    press(&mut dashboard, &[Down, Down]);
    assert_eq!(press(&mut dashboard, &[Right, Left]), [], "no output to turn up");
    press(&mut dashboard, &[Down]);
    assert_eq!(press(&mut dashboard, &[A]), [], "nor to mute");
}

#[test]
fn an_open_flyout_follows_the_system() {
    let mut dashboard = in_audio();
    press(&mut dashboard, &[A, Down]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Device(Flow::Output, 1))));
    // The TV is unplugged: the list is shorter, and focus is on what is left.
    let one = Audio { outputs: vec![AudioDevice::new("out-1", "Speakers")], ..sound() };
    dashboard.set_audio(one);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Device(Flow::Output, 0))));
    // The last output goes: the list closes onto its dropdown.
    dashboard.set_audio(Audio { outputs: Vec::new(), output: None, ..sound() });
    assert_eq!(dashboard.flyout(), item(OUTPUT));
    // The volume changed elsewhere shows.
    dashboard.set_audio(Audio { volume: 30, ..sound() });
    assert_eq!(dashboard.guide_line(), "Landing Utility(1), flyout Audio Item(0), volume 30");
}

#[test]
fn the_audio_flyout_is_what_has_the_shell_read_the_systems_sound() {
    let mut dashboard = on_utilities();
    assert!(!dashboard.audio_shown());
    press(&mut dashboard, &[A]);
    assert!(!dashboard.audio_shown(), "Notifications is not Audio");
    press(&mut dashboard, &[B, Right, A]);
    assert!(dashboard.audio_shown());
    press(&mut dashboard, &[B]);
    assert!(!dashboard.audio_shown());
}

#[test]
fn open_notifications_opens_the_flyout_in_an_open_guide() {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.notify("one");
    dashboard.open_notifications();
    assert_eq!(dashboard.flyout(), None, "the Guide is closed");
    press(&mut dashboard, &[Guide]);
    dashboard.open_notifications();
    assert_eq!(dashboard.guide().focus(), ON_NOTIFICATIONS);
    assert_eq!(dashboard.flyout(), Some((Flyout::Notifications, FlyoutFocus::Item(0))));
}

#[test]
fn power_by_a_hold_and_a_prompt_take_the_flyouts_place() {
    use ui::PadEvent;
    let mut dashboard = in_audio();
    dashboard.pad_event(PadEvent::GuideHeld);
    assert_eq!(dashboard.flyout(), None);
    assert!(dashboard.guide().dialog().is_some());
}

#[test]
fn the_flyouts_open_over_a_running_game_too() {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.set_audio(sound());
    dashboard.launch_started(2);
    dashboard.game_booted();
    press(&mut dashboard, &[Guide]);
    let entries = dashboard.guide().entries().len();
    press(&mut dashboard, &vec![Down; entries]);
    assert_eq!(press(&mut dashboard, &[Right, A]), [MOVE, SELECT]);
    assert_eq!(dashboard.flyout(), item(OUTPUT));
    // The game ending leaves the Flyout open, as it leaves the Guide.
    dashboard.game_ended();
    assert_eq!(dashboard.flyout(), item(OUTPUT));
}
