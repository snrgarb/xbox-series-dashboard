//! Start-up (docs/spec/start-up.md): the Boot video before Home, which a press skips, with the pad and the Guide
//! button its own while it shows; and nothing of it with the video Off.
use screens::{fake_library, Dashboard, OverlayShows};
use ui::Button::{self, Down, Guide, Right, A, B};
use ui::{PadEvent, Press};

fn press(dashboard: &mut Dashboard, button: Button) {
    dashboard.press(Press { button, repeat: false });
}

/// Where focus is on Home as the Dashboard starts.
fn home_line() -> String {
    Dashboard::new(fake_library()).focus_line()
}

/// The Dashboard as the executable starts it with a video to play.
fn starting() -> Dashboard {
    let mut dashboard = Dashboard::new(fake_library());
    dashboard.start_up(true);
    dashboard
}

/// The video's first frame is on screen.
fn showing() -> Dashboard {
    let mut dashboard = starting();
    dashboard.boot_video_picture(render::WHITE, 16.0 / 9.0);
    dashboard
}

#[test]
fn with_the_video_off_home_is_there_at_once() {
    let mut dashboard = Dashboard::new(fake_library());
    let home = dashboard.focus_line();
    dashboard.start_up(false);
    assert!(!dashboard.starting_up() && !dashboard.boot_video_playing());
    assert_eq!(dashboard.focus_line(), home);
    press(&mut dashboard, Down);
    assert_ne!(dashboard.focus_line(), home, "Home has the pad");
}

#[test]
fn the_video_shows_over_everything_and_nothing_behind_it_hears_the_pad() {
    let mut dashboard = starting();
    assert!(dashboard.starting_up() && dashboard.boot_video_playing());
    assert_eq!(dashboard.focus_line(), "Startup: BootVideo");
    assert_eq!(dashboard.overlay(), OverlayShows::Nothing);
    // Before its first frame, and a moment after, a press is nothing: one meant for something else as the window
    // opened does not skip it.
    for button in [A, B, Down, Right] {
        press(&mut dashboard, button);
    }
    dashboard.boot_video_picture(render::WHITE, 16.0 / 9.0);
    dashboard.advance(0.2);
    press(&mut dashboard, A);
    assert!(dashboard.boot_video_playing());
    assert_eq!(dashboard.focus_line(), "Startup: BootVideo");
    assert!(dashboard.take_effects().is_empty(), "no sound, nothing for the shell");
}

#[test]
fn any_button_skips_it_once_it_has_shown_a_moment_and_home_is_as_it_was() {
    for button in [A, B, Down, Button::Menu, Button::View, Button::LeftBumper] {
        let mut dashboard = showing();
        dashboard.advance(0.3);
        // A held direction's repeats are not presses.
        dashboard.press(Press { button: Down, repeat: true });
        assert!(dashboard.boot_video_playing());
        press(&mut dashboard, button);
        assert!(!dashboard.starting_up(), "{button:?} skips it");
        assert_eq!(dashboard.focus_line(), home_line(), "the press did nothing else");
        assert!(dashboard.take_effects().is_empty(), "skipped in silence");
    }
}

#[test]
fn the_guide_button_skips_it_and_the_rest_of_that_press_does_not_open_the_guide() {
    // A tap: the release skips it, and the tap that follows is spent.
    let mut dashboard = showing();
    dashboard.advance(1.0);
    // The pad tells of the Guide button by its release and its tap, or its hold: never as a press.
    dashboard.pad_event(PadEvent::Release(Guide));
    assert!(!dashboard.starting_up());
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(!dashboard.guide_open() && dashboard.take_effects().is_empty());

    // A hold: it skips the video as it is held, and its release is spent.
    let mut dashboard = showing();
    dashboard.advance(1.0);
    dashboard.pad_event(PadEvent::GuideHeld);
    assert!(!dashboard.starting_up());
    dashboard.pad_event(PadEvent::Release(Guide));
    assert!(!dashboard.guide_open() && dashboard.take_effects().is_empty());

    // And the next press of the button is the Guide's as ever.
    dashboard.pad_event(PadEvent::Release(Guide));
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(dashboard.guide_open());
}

#[test]
fn a_guide_button_too_soon_after_the_first_frame_neither_skips_nor_opens_the_guide() {
    let mut dashboard = showing();
    dashboard.pad_event(PadEvent::Release(Guide));
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(dashboard.boot_video_playing() && !dashboard.guide_open());
}

#[test]
fn the_video_ending_goes_on_to_home_and_said_twice_is_said_once() {
    let mut dashboard = showing();
    dashboard.boot_video_ended();
    assert!(!dashboard.starting_up() && !dashboard.boot_video_playing());
    dashboard.boot_video_ended();
    assert!(!dashboard.starting_up());
    // A picture that comes late is nothing now.
    dashboard.boot_video_picture(render::WHITE, 4.0 / 3.0);
    assert!(!dashboard.starting_up());
    press(&mut dashboard, Down);
    assert_ne!(dashboard.focus_line(), home_line(), "Home has the pad");
}

#[test]
fn a_video_that_never_showed_a_frame_ends_all_the_same() {
    let mut dashboard = starting();
    dashboard.advance(30.0);
    press(&mut dashboard, A);
    assert!(dashboard.boot_video_playing(), "no picture, no skip: the executable's deadline ends it");
    dashboard.boot_video_ended();
    assert!(!dashboard.starting_up());
}
