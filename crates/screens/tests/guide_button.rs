//! The Guide button's tap and hold, and a held direction's pace (docs/spec/guide.md, 3.1 and 7.1): a pad held as the
//! test says, at the times it says, read by `input` as the executable's pad thread reads it and told to the
//! Dashboard. No window, and no clock but the test's.
use std::time::{Duration, Instant};

use input::Presses;
use screens::{Dashboard, Effect, OverlayShows, Sound, GUIDE_REPEAT};
use ui::{Button, Press, Repeat};

/// XInput's bits for the buttons held here.
const DOWN: u32 = 0x0002;
const B: u32 = 0x2000;
const GUIDE: u32 = 0x0400;

const GUIDE_OPEN: Effect = Effect::Sound(Sound::GuideOpen);
const GUIDE_MOVE: Effect = Effect::Sound(Sound::GuideMove);
const PROMPT_OPEN: Effect = Effect::Sound(Sound::PromptOpen);

struct Pad {
    dashboard: Dashboard,
    presses: Presses,
    start: Instant,
}

impl Pad {
    fn new() -> Self {
        Self { dashboard: Dashboard::new(screens::fake_library()), presses: Presses::default(), start: Instant::now() }
    }

    /// The pad is looked at `ms` after the start, with `buttons` held. Gives what the Dashboard asked for.
    fn at(&mut self, ms: u64, buttons: u32) -> Vec<Effect> {
        self.presses.set_repeat(self.dashboard.repeat());
        for event in self.presses.update(buttons, self.start + Duration::from_millis(ms)) {
            self.dashboard.pad_event(event);
        }
        self.dashboard.take_effects()
    }

    /// The Guide button tapped for 120 ms from `ms`, and the Guide open 300 ms after the press.
    fn open_guide(&mut self, ms: u64) {
        self.at(ms, GUIDE);
        self.at(ms + 120, 0);
        assert_eq!(self.at(ms + 300, 0), [GUIDE_OPEN]);
        assert!(self.dashboard.guide_open());
    }
}

#[test]
fn a_tap_opens_the_guide_300_ms_after_the_press() {
    let mut pad = Pad::new();
    assert_eq!(pad.at(0, GUIDE), []);
    assert_eq!(pad.at(120, 0), []);
    assert_eq!(pad.at(299, 0), []);
    assert!(!pad.dashboard.guide_open());
    assert_eq!(pad.at(300, 0), [GUIDE_OPEN]);
    assert!(pad.dashboard.guide_open());
    // And that is all it does.
    assert_eq!(pad.at(2000, 0), []);
    assert!(pad.dashboard.guide_open());
}

#[test]
fn a_tap_let_go_after_300_ms_opens_the_guide_as_it_is_let_go() {
    let mut pad = Pad::new();
    pad.at(0, GUIDE);
    assert_eq!(pad.at(500, GUIDE), []);
    assert!(!pad.dashboard.guide_open());
    assert_eq!(pad.at(600, 0), [GUIDE_OPEN]);
    assert!(pad.dashboard.guide_open());
}

#[test]
fn a_tap_closes_an_open_guide_as_the_button_is_let_go() {
    let mut pad = Pad::new();
    pad.open_guide(0);
    assert_eq!(pad.at(1000, GUIDE), []);
    assert!(pad.dashboard.guide_open());
    assert_eq!(pad.at(1120, 0), [Effect::GuideClosed]);
    assert!(!pad.dashboard.guide_open());
    // 300 ms after that press, the tap that closed it does not open it again.
    assert_eq!(pad.at(1300, 0), []);
    assert!(!pad.dashboard.guide_open());
    // The next tap does.
    pad.open_guide(2000);
}

#[test]
fn a_hold_opens_power_with_no_panel_and_its_release_does_nothing() {
    let mut pad = Pad::new();
    assert_eq!(pad.at(0, GUIDE), []);
    assert_eq!(pad.at(300, GUIDE), []);
    assert_eq!(pad.at(699, GUIDE), []);
    assert!(!pad.dashboard.guide_open(), "nothing shows until the hold is a hold");
    assert_eq!(pad.at(700, GUIDE), [PROMPT_OPEN]);
    assert!(pad.dashboard.guide().dialog().is_some() && !pad.dashboard.guide().panel_shown());
    assert_eq!(pad.at(1500, GUIDE), []);
    assert_eq!(pad.at(1600, 0), []);
    assert_eq!(pad.at(3000, 0), []);
    assert!(pad.dashboard.guide().dialog().is_some(), "letting go leaves Power up");
    // A tap after it is a tap: with Power up it closes everything, and the next opens the Guide.
    pad.at(4000, GUIDE);
    assert_eq!(pad.at(4100, 0), [Effect::GuideClosed]);
    assert!(!pad.dashboard.guide_open());
    pad.open_guide(5000);
    assert!(pad.dashboard.guide().dialog().is_none() && pad.dashboard.guide().panel_shown());
}

#[test]
fn a_hold_with_the_guide_open_opens_power_and_its_release_does_not_close_the_guide() {
    let mut pad = Pad::new();
    pad.open_guide(0);
    pad.at(1000, GUIDE);
    assert_eq!(pad.at(1700, GUIDE), [PROMPT_OPEN]);
    assert_eq!(pad.at(1900, 0), []);
    assert!(pad.dashboard.guide_open());
    assert!(pad.dashboard.guide().dialog().is_some() && !pad.dashboard.guide().panel_shown());
    // A tap after it closes the Guide.
    pad.at(3000, GUIDE);
    assert_eq!(pad.at(3100, 0), [Effect::GuideClosed]);
}

#[test]
fn with_the_guide_setting_off_the_button_does_nothing() {
    let mut pad = Pad::new();
    pad.dashboard.set_guide_enabled(false);
    // A tap.
    pad.at(0, GUIDE);
    assert_eq!(pad.at(120, 0), []);
    assert_eq!(pad.at(300, 0), []);
    // A hold.
    pad.at(1000, GUIDE);
    assert_eq!(pad.at(1700, GUIDE), []);
    assert_eq!(pad.at(1800, 0), []);
    // The keyboard's Guide button, and the profile on Home's top bar, where focus starts.
    pad.dashboard.press(Press { button: Button::Guide, repeat: false });
    pad.dashboard.press(Press { button: Button::A, repeat: false });
    assert_eq!(pad.dashboard.take_effects(), []);
    assert!(!pad.dashboard.guide_open());

    // On again, it is the Guide button again.
    pad.dashboard.set_guide_enabled(true);
    pad.open_guide(3000);
}

#[test]
fn a_tap_over_a_game_opens_the_guide_over_it_and_another_closes_it() {
    let mut pad = Pad::new();
    pad.dashboard.launch_started(0);
    pad.dashboard.game_booted();
    pad.open_guide(0);
    assert!(pad.dashboard.guide_over_game());
    pad.at(1000, GUIDE);
    assert_eq!(pad.at(1120, 0), [Effect::GuideClosed]);
    assert!(!pad.dashboard.guide_over_game());
}

#[test]
fn a_held_direction_repeats_at_the_guides_pace_in_the_guide_and_at_homes_at_home() {
    let mut pad = Pad::new();
    assert_eq!(pad.dashboard.repeat(), Repeat::default());
    pad.open_guide(0);
    assert_eq!(pad.dashboard.repeat(), GUIDE_REPEAT);

    // In the Guide: 700 ms, then every 80 ms.
    assert_eq!(pad.at(1000, DOWN), [GUIDE_MOVE]);
    assert_eq!(pad.at(1699, DOWN), []);
    assert_eq!(pad.at(1700, DOWN), [GUIDE_MOVE]);
    assert_eq!(pad.at(1779, DOWN), []);
    assert_eq!(pad.at(1780, DOWN), [GUIDE_MOVE]);
    assert_eq!(pad.at(1860, DOWN), [GUIDE_MOVE]);
    assert_eq!(pad.at(1900, 0), []);

    // At Home, focus on the profile: Down to the Recent row, then to the wide tiles 400 ms later.
    pad.at(3000, GUIDE);
    pad.at(3100, 0);
    assert!(!pad.dashboard.guide_open());
    let moved = Effect::Sound(Sound::Move);
    assert_eq!(pad.at(4000, DOWN), [moved]);
    assert_eq!(pad.at(4399, DOWN), []);
    assert_eq!(pad.at(4400, DOWN), [moved]);
    assert_eq!(pad.at(4519, DOWN), []);
    assert_eq!(pad.at(4520, DOWN), [moved]);
}

/// docs/spec/guide.md, 3.5: closed with B, the Guide keeps the foreground until B is let go, so the window that
/// gets it back never sees the press; closed with a tap of the Guide button, it hands back at once.
#[test]
fn the_guide_closed_with_b_keeps_the_foreground_until_b_is_let_go() {
    let mut pad = Pad::new();
    pad.dashboard.launch_started(0);
    pad.dashboard.game_booted();
    pad.open_guide(0);
    assert_eq!(pad.dashboard.overlay(), OverlayShows::Guide);

    assert_eq!(pad.at(1000, B), [Effect::GuideClosed]);
    assert!(!pad.dashboard.guide_open());
    assert!(!pad.dashboard.guide_over_game());
    assert!(pad.dashboard.awaiting_release());
    assert_eq!(pad.dashboard.overlay(), OverlayShows::Guide, "the foreground is not handed back under a held B");
    pad.at(1100, B);
    assert_eq!(pad.dashboard.overlay(), OverlayShows::Guide);
    pad.at(1120, 0);
    assert!(!pad.dashboard.awaiting_release());
    assert_ne!(pad.dashboard.overlay(), OverlayShows::Guide);

    // A B that is never seen let go: whoever has the clock says when a second is up.
    pad.open_guide(2000);
    pad.at(3000, B);
    assert!(pad.dashboard.awaiting_release());
    pad.dashboard.release_wait_over();
    assert_ne!(pad.dashboard.overlay(), OverlayShows::Guide);
    pad.at(3500, 0);

    // The Guide button's tap closes it as the button is let go: there is nothing to wait for.
    pad.open_guide(4000);
    pad.at(5000, GUIDE);
    pad.at(5120, 0);
    assert!(!pad.dashboard.guide_open());
    assert!(!pad.dashboard.awaiting_release());
    assert_ne!(pad.dashboard.overlay(), OverlayShows::Guide);

    // Nor for a key: the keyboard tells no releases.
    pad.open_guide(6000);
    pad.dashboard.press(Press { button: Button::B, repeat: false });
    assert!(!pad.dashboard.guide_open());
    assert!(!pad.dashboard.awaiting_release());
}

/// Opened again before the foreground went back, the Guide keeps the Overlay as it is: the window it first opened
/// over is still the one that gets the foreground back.
#[test]
fn the_guide_opened_again_under_a_held_b_keeps_the_foreground() {
    let mut pad = Pad::new();
    pad.open_guide(0);
    assert_eq!(pad.dashboard.overlay(), OverlayShows::Guide, "at Home the Guide is on the Overlay too");
    pad.at(1000, B);
    assert!(pad.dashboard.awaiting_release());
    // The keyboard's Guide button, B still down.
    pad.dashboard.press(Press { button: Button::Guide, repeat: false });
    assert!(pad.dashboard.guide_open());
    assert!(!pad.dashboard.awaiting_release());
    assert_eq!(pad.dashboard.overlay(), OverlayShows::Guide);
    // B let go now closes nothing and hands nothing back.
    pad.at(1500, 0);
    assert!(pad.dashboard.guide_open());
    assert_eq!(pad.dashboard.overlay(), OverlayShows::Guide);
}

/// With no Overlay the Guide at Home is in Home's window, which has the foreground all along: nothing is waited for.
#[test]
fn with_no_overlay_the_guide_closed_with_b_waits_for_nothing() {
    let mut pad = Pad::new();
    pad.dashboard.set_overlay(false);
    pad.open_guide(0);
    pad.at(1000, B);
    assert!(!pad.dashboard.guide_open());
    assert!(!pad.dashboard.awaiting_release());
}
