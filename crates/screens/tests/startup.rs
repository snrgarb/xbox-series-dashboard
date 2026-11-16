//! Start-up (docs/spec/start-up.md): the Boot video, which a press skips; Who's playing today?, the Profiles in a
//! row, A signing the focused one in; Signing you in for three seconds; then Home and the greeting. Start-up has the
//! pad and the Guide button to itself while it shows. Switch profile on the Guide, and waking from sleep, ask again
//! without the video. The pictures are tests/whos_playing.rs.
use model::{Library, Profile, ProfileChange};
use screens::guide::{Focus, Tab};
use screens::settings::{Change, Setting};
use screens::toasts::ToastKind;
use screens::{fake_library, Dashboard, Effect, OverlayShows, Phase, Sound};
use ui::Button::{self, Down, Guide, Left, Right, RightBumper, A, B};
use ui::{PadEvent, Press};

fn press(dashboard: &mut Dashboard, button: Button) {
    dashboard.press(Press { button, repeat: false });
}

fn presses(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        press(dashboard, button);
    }
}

/// Where focus is on Home as the Dashboard starts.
fn home_line() -> String {
    Dashboard::new(fake_library()).focus_line()
}

/// The fake library with these Profiles, ids 1 up, the one at `last` signed in last.
fn library(names: &[&str], last: usize) -> Library {
    let mut library = fake_library();
    let profile =
        |(i, name): (usize, &&str)| Profile { id: i as i64 + 1, gamertag: name.to_string(), ..Profile::default() };
    library.profiles.list = names.iter().enumerate().map(profile).collect();
    library.profiles.signed_in = Some(last as i64 + 1);
    library
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

/// Started with the video Off on a library of these Profiles: Who's playing today? shows.
fn asking(names: &[&str], last: usize) -> Dashboard {
    let mut dashboard = Dashboard::new(library(names, last));
    dashboard.start_up(false);
    dashboard
}

/// The sounds among what the shell was asked.
fn sounds(effects: &[Effect]) -> Vec<Sound> {
    effects.iter().filter_map(|effect| if let Effect::Sound(sound) = effect { Some(*sound) } else { None }).collect()
}

fn signed_in(effects: &[Effect]) -> Vec<i64> {
    let id = |effect: &Effect| match effect {
        Effect::Profile(ProfileChange::SignIn(id)) => Some(*id),
        _ => None,
    };
    effects.iter().filter_map(id).collect()
}

/// The greeting or Toast showing now, if one is.
fn toast(dashboard: &Dashboard) -> Option<(String, ToastKind)> {
    dashboard.toasts().showing().map(|toast| (toast.text.clone(), toast.kind))
}

// --- The Boot video ---

#[test]
fn with_the_video_off_whos_playing_today_is_there_at_once() {
    let dashboard = asking(&["Alpha", "Beta"], 1);
    assert!(dashboard.starting_up() && !dashboard.boot_video_playing());
    assert_eq!(dashboard.startup_phase(), Some(Phase::WhosPlaying));
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Beta\"", "focus on the one signed in last");
    assert_eq!(dashboard.overlay(), OverlayShows::Nothing);
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
fn any_button_skips_it_once_it_has_shown_a_moment_and_who_s_playing_today_follows() {
    for button in [A, B, Down, Button::Menu, Button::View, Button::LeftBumper] {
        let mut dashboard = showing();
        dashboard.advance(0.3);
        // A held direction's repeats are not presses.
        dashboard.press(Press { button: Down, repeat: true });
        assert!(dashboard.boot_video_playing());
        press(&mut dashboard, button);
        assert!(!dashboard.boot_video_playing(), "{button:?} skips it");
        // The fake library has no Profiles: Who's playing today? shows the one it has, its user.
        assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Player One\"", "the press did nothing else");
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
    assert!(!dashboard.boot_video_playing());
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(!dashboard.guide_open() && dashboard.take_effects().is_empty());

    // A hold: it skips the video as it is held, and its release is spent.
    let mut dashboard = showing();
    dashboard.advance(1.0);
    dashboard.pad_event(PadEvent::GuideHeld);
    assert!(!dashboard.boot_video_playing());
    dashboard.pad_event(PadEvent::Release(Guide));
    assert!(!dashboard.guide_open() && dashboard.take_effects().is_empty());

    // And the button does nothing more on Who's playing today?: someone has to sign in.
    dashboard.pad_event(PadEvent::Release(Guide));
    dashboard.pad_event(PadEvent::GuideTap);
    dashboard.pad_event(PadEvent::GuideHeld);
    dashboard.pad_event(PadEvent::Release(Guide));
    assert!(!dashboard.guide_open() && dashboard.startup_phase() == Some(Phase::WhosPlaying));
}

#[test]
fn a_guide_button_too_soon_after_the_first_frame_neither_skips_nor_opens_the_guide() {
    let mut dashboard = showing();
    dashboard.pad_event(PadEvent::Release(Guide));
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(dashboard.boot_video_playing() && !dashboard.guide_open());
}

#[test]
fn the_video_ending_goes_on_and_said_twice_is_said_once() {
    let mut dashboard = showing();
    dashboard.boot_video_ended();
    assert_eq!(dashboard.startup_phase(), Some(Phase::WhosPlaying));
    dashboard.boot_video_ended();
    assert_eq!(dashboard.startup_phase(), Some(Phase::WhosPlaying));
    // A picture that comes late is nothing now.
    dashboard.boot_video_picture(render::WHITE, 4.0 / 3.0);
    assert_eq!(dashboard.startup_phase(), Some(Phase::WhosPlaying));
}

#[test]
fn a_video_that_never_showed_a_frame_ends_all_the_same() {
    let mut dashboard = starting();
    dashboard.advance(30.0);
    press(&mut dashboard, A);
    assert!(dashboard.boot_video_playing(), "no picture, no skip: the executable's deadline ends it");
    dashboard.boot_video_ended();
    assert!(!dashboard.boot_video_playing());
}

// --- Who's playing today? and Signing you in ---

#[test]
fn left_and_right_move_along_the_profiles_with_the_guides_move_sound_stopping_at_the_ends() {
    let mut dashboard = asking(&["Alpha", "Beta", "Gamma"], 1);
    press(&mut dashboard, Right);
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Gamma\"");
    assert_eq!(sounds(&dashboard.take_effects()), [Sound::GuideMove]);
    press(&mut dashboard, Right);
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Gamma\"");
    assert!(dashboard.take_effects().is_empty(), "the end stops, in silence");
    presses(&mut dashboard, &[Left, Left, Left]);
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Alpha\"");
    assert_eq!(sounds(&dashboard.take_effects()), [Sound::GuideMove, Sound::GuideMove], "two moves, one stop");
    // Nothing else does anything: Up, Down, B, Y, Menu, the bumpers.
    for button in [Button::Up, Down, B, Button::Y, Button::Menu, RightBumper, Button::LeftBumper] {
        press(&mut dashboard, button);
    }
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Alpha\"");
    assert!(dashboard.take_effects().is_empty() && !dashboard.guide_open());
}

#[test]
fn a_signs_the_focused_profile_in_at_once_and_signing_in_shows_three_seconds_then_home_and_the_greeting() {
    let mut dashboard = asking(&["Alpha", "Beta"], 0);
    dashboard.toasts_ready();
    assert!(toast(&dashboard).is_none(), "nobody is greeted during Start-up");
    presses(&mut dashboard, &[Right]);
    dashboard.take_effects();
    press(&mut dashboard, A);
    let effects = dashboard.take_effects();
    // The Bundle's Enter, then its Login; and Beta signed in, everywhere and remembered, at once.
    assert_eq!(sounds(&effects), [Sound::GuideSelect, Sound::Login]);
    assert_eq!(signed_in(&effects), [2]);
    assert_eq!(dashboard.profile().gamertag, "Beta");
    assert_eq!(dashboard.focus_line(), "Startup: SigningIn \"Beta\"");
    assert_eq!(dashboard.timer(), Some(3.0));
    // Nothing moves it.
    presses(&mut dashboard, &[Left, A, B]);
    assert_eq!(dashboard.focus_line(), "Startup: SigningIn \"Beta\"");
    dashboard.advance(2.9);
    assert!(dashboard.starting_up());
    dashboard.advance(0.1);
    assert!(!dashboard.starting_up());
    assert!(dashboard.take_timed(), "Home is to be drawn");
    assert_eq!(dashboard.focus_line(), home_line(), "Home as it was");
    assert_eq!(toast(&dashboard), Some(("Hi Beta!".into(), ToastKind::Greeting)));
    // And Home has the pad.
    press(&mut dashboard, Down);
    assert_ne!(dashboard.focus_line(), home_line());
}

#[test]
fn with_toasts_nowhere_to_show_yet_the_greeting_waits_for_them() {
    let mut dashboard = asking(&["Alpha"], 0);
    presses(&mut dashboard, &[A]);
    dashboard.advance(3.0);
    assert!(!dashboard.starting_up() && toast(&dashboard).is_none());
    // The executable makes the Toast strip once Home has come to rest after its first frame.
    dashboard.toasts_ready();
    assert_eq!(toast(&dashboard), Some(("Hi Alpha!".into(), ToastKind::Greeting)));
}

#[test]
fn toasts_that_come_during_startup_follow_the_greeting() {
    let mut dashboard = asking(&["Alpha"], 0);
    dashboard.toasts_ready();
    dashboard.notify("Download complete");
    assert!(toast(&dashboard).is_none(), "nothing shows over Start-up");
    assert_eq!(dashboard.notifications().len(), 1, "listed all the same");
    press(&mut dashboard, A);
    dashboard.advance(3.0);
    assert_eq!(toast(&dashboard), Some(("Hi Alpha!".into(), ToastKind::Greeting)));
    assert_eq!(dashboard.toasts().waiting(), 1);
}

#[test]
fn not_asking_the_profile_signed_in_last_signs_in_by_itself() {
    let mut dashboard = Dashboard::new(library(&["Alpha", "Beta"], 1));
    let mut options = dashboard.options();
    options.ask_whos_playing = false;
    dashboard.set_options(options);
    dashboard.start_up(false);
    assert_eq!(dashboard.focus_line(), "Startup: SigningIn \"Beta\"");
    let effects = dashboard.take_effects();
    assert_eq!((sounds(&effects), signed_in(&effects)), (vec![Sound::Login], vec![2]));
    dashboard.toasts_ready();
    dashboard.advance(3.0);
    assert_eq!(toast(&dashboard), Some(("Hi Beta!".into(), ToastKind::Greeting)));

    // After the video too.
    let mut dashboard = Dashboard::new(library(&["Alpha", "Beta"], 0));
    dashboard.set_options(options);
    dashboard.start_up(true);
    dashboard.boot_video_ended();
    assert_eq!(dashboard.focus_line(), "Startup: SigningIn \"Alpha\"");
}

#[test]
fn one_profile_is_asked_about_all_the_same_as_the_bundle_does() {
    let mut dashboard = asking(&["Alpha"], 0);
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Alpha\"");
    presses(&mut dashboard, &[Left, Right]);
    assert!(dashboard.take_effects().is_empty());
    press(&mut dashboard, A);
    assert_eq!(dashboard.focus_line(), "Startup: SigningIn \"Alpha\"");
}

#[test]
fn a_library_with_no_profiles_shows_its_user_and_signs_nobody_in() {
    let mut dashboard = Dashboard::new(fake_library());
    dashboard.start_up(false);
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Player One\"");
    press(&mut dashboard, A);
    assert!(signed_in(&dashboard.take_effects()).is_empty());
    dashboard.advance(3.0);
    assert!(!dashboard.starting_up());
}

#[test]
fn the_greeting_has_the_guide_button_open_the_guide_on_profile_and_system() {
    let mut dashboard = asking(&["Alpha", "Beta"], 0);
    dashboard.toasts_ready();
    presses(&mut dashboard, &[Right, A]);
    dashboard.advance(3.0);
    assert!(toast(&dashboard).is_some());
    dashboard.pad_event(PadEvent::Release(Guide));
    dashboard.pad_event(PadEvent::GuideTap);
    assert!(dashboard.guide_open());
    assert_eq!(dashboard.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(0)));
}

// --- Switch profile, and waking from sleep ---

/// Started straight to Home (`--no-startup`) on a library of these Profiles, greeted, the Guide open on Profile &
/// system with focus on Switch profile.
fn on_switch_profile(names: &[&str], last: usize) -> Dashboard {
    let mut dashboard = Dashboard::new(library(names, last));
    dashboard.toasts_ready();
    dashboard.advance(6.0);
    press(&mut dashboard, Guide);
    presses(&mut dashboard, &[RightBumper, Down]);
    assert_eq!(dashboard.guide().focus(), (Tab::ProfileAndSystem, Focus::Entry(1)));
    dashboard.take_effects();
    dashboard
}

#[test]
fn switch_profile_closes_the_guide_at_once_and_asks_without_the_video_whatever_settings_says() {
    for ask in [true, false] {
        let mut dashboard = on_switch_profile(&["Alpha", "Beta"], 0);
        let mut options = dashboard.options();
        options.ask_whos_playing = ask;
        dashboard.set_options(options);
        press(&mut dashboard, A);
        assert!(!dashboard.guide_open() && !dashboard.guide().showing(), "gone at once, with no slide");
        assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Alpha\"");
        let effects = dashboard.take_effects();
        assert_eq!(sounds(&effects), [Sound::GuideSelect]);
        assert!(effects.contains(&Effect::GuideClosed));
        // B does nothing, as on start: someone has to sign in.
        press(&mut dashboard, B);
        assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Alpha\"");
        presses(&mut dashboard, &[Right, A]);
        assert_eq!(signed_in(&dashboard.take_effects()), [2]);
        dashboard.advance(3.0);
        assert_eq!(toast(&dashboard), Some(("Hi Beta!".into(), ToastKind::Greeting)), "greeted again");
    }
}

#[test]
fn switch_profile_by_the_pad_hands_the_foreground_back_as_a_is_let_go() {
    let mut dashboard = on_switch_profile(&["Alpha", "Beta"], 0);
    dashboard.pad_event(PadEvent::Press(Press { button: A, repeat: false }));
    assert!(dashboard.starting_up());
    // The Overlay keeps the foreground until A is up, then hands it back to Home's window.
    assert_eq!(dashboard.overlay(), OverlayShows::Guide);
    dashboard.pad_event(PadEvent::Release(A));
    assert_eq!(dashboard.overlay(), OverlayShows::Nothing);
}

#[test]
fn over_a_game_switch_profile_does_nothing() {
    let mut dashboard = Dashboard::new(library(&["Alpha", "Beta"], 0));
    dashboard.launch_started(2);
    dashboard.game_booted();
    press(&mut dashboard, Guide);
    presses(&mut dashboard, &[RightBumper, Down, A]);
    assert!(dashboard.guide_open() && !dashboard.starting_up());
}

#[test]
fn waking_from_sleep_asks_without_the_video_if_settings_has_it_ask() {
    let mut dashboard = on_switch_profile(&["Alpha", "Beta"], 1);
    assert!(dashboard.woke_from_sleep());
    assert!(!dashboard.guide_open(), "the Guide goes first");
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Beta\"");
    // A second wake while it asks changes nothing.
    assert!(!dashboard.woke_from_sleep());

    // Turned off, waking asks nothing.
    let mut dashboard = Dashboard::new(library(&["Alpha", "Beta"], 1));
    let change = Change { setting: Setting::AskWhosPlaying, on: false };
    let mut options = dashboard.options();
    options.set(change.setting, change.on);
    dashboard.set_options(options);
    assert!(!dashboard.woke_from_sleep());
    assert!(!dashboard.starting_up());
}

#[test]
fn waking_with_a_game_running_brings_the_dashboard_in_front_of_it_first() {
    let mut dashboard = Dashboard::new(library(&["Alpha", "Beta"], 0));
    dashboard.launch_started(2);
    dashboard.game_booted();
    dashboard.take_effects();
    assert!(dashboard.woke_from_sleep());
    assert!(dashboard.take_effects().contains(&Effect::ShowHome));
    assert!(!dashboard.home_covered());
    assert_eq!(dashboard.focus_line(), "Startup: WhosPlaying \"Alpha\"");
}
