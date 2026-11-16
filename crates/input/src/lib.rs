//! The gamepad, read through XInput so it works whether or not the Dashboard has the foreground: the Guide button
//! has to open the Guide over a game. `XInputGetStateEx` (xinput1_4's ordinal 100) is the call that reports the Guide
//! button, as the theme's docs/research/guide-button-capture.md found. Ctrl+Alt+G stands in for the Guide button on a
//! keyboard, as a tap however long it is held.
//!
//! Everything timed about the pad is timed here, by `Presses`: a held direction's repeats, and the Guide button's
//! tap and hold (docs/spec/guide.md, 7.1). The thread that reads the pads is the one thing that looks at them every
//! few milliseconds whatever is being drawn, and it wakes the Dashboard when there is something to tell it. Which
//! pace a held direction repeats at is the Dashboard's to say (`Pad::set_repeat`).
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ui::{Battery, Button, PadEvent, PadStatus, Press, Repeat};

/// XInput's button bits, and two of our own for the triggers. The Guide button is not among them: it is followed
/// on its own.
const BITS: [(u32, Button); 15] = [
    (0x0001, Button::Up),
    (0x0002, Button::Down),
    (0x0004, Button::Left),
    (0x0008, Button::Right),
    (0x0010, Button::Menu),
    (0x0020, Button::View),
    (0x0040, Button::LeftStick),
    (0x0100, Button::LeftBumper),
    (0x0200, Button::RightBumper),
    (0x1000, Button::A),
    (0x2000, Button::B),
    (0x4000, Button::X),
    (0x8000, Button::Y),
    (LEFT_TRIGGER, Button::LeftTrigger),
    (RIGHT_TRIGGER, Button::RightTrigger),
];
const UP: u32 = 0x0001;
const DOWN: u32 = 0x0002;
const LEFT: u32 = 0x0004;
const RIGHT: u32 = 0x0008;
const GUIDE: u32 = 0x0400;
const LEFT_TRIGGER: u32 = 0x1_0000;
const RIGHT_TRIGGER: u32 = 0x2_0000;
/// Ours: the keyboard's stand-in for the Guide button, which is only ever a tap.
#[cfg_attr(not(windows), allow(dead_code))]
const GUIDE_KEY: u32 = 0x4_0000;
const DIRECTIONS: u32 = UP | DOWN | LEFT | RIGHT;
#[cfg_attr(not(windows), allow(dead_code))]
const STICK: i16 = 20000;
#[cfg_attr(not(windows), allow(dead_code))]
const TRIGGER: u8 = 64;

/// A tap of the Guide button is told this long after the button went down, so that a hold shows nothing first.
pub const TAP_AFTER: Duration = Duration::from_millis(300);
/// The Guide button down this long is a hold.
pub const HOLD: Duration = Duration::from_millis(700);

/// Where the Guide button is.
#[derive(Default)]
enum GuideButton {
    #[default]
    Up,
    /// Down since then. `pad`: on a pad, not only on the keyboard.
    Down { since: Instant, pad: bool },
    /// Still down, and the hold has been told.
    Held,
    /// Let go, a tap that is told then.
    Tapped { due: Instant },
}

/// Turns what is held, looked at again and again, into events: a press as a button goes down, more as a direction
/// stays held, a release as it is let go, and the Guide button's tap or hold. Time is whatever `update` is told it
/// is.
#[derive(Default)]
pub struct Presses {
    held: u32,
    repeat: Repeat,
    /// When the held directions last pressed, and whether that was a repeat.
    direction: Option<(Instant, bool)>,
    guide: GuideButton,
}

impl Presses {
    /// How a held direction repeats from here on, one already held too.
    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }

    pub fn update(&mut self, buttons: u32, now: Instant) -> Vec<PadEvent> {
        let pressed = buttons & !self.held;
        let released = self.held & !buttons;
        let bits = |mask: u32| BITS.iter().filter(move |(bit, _)| mask & bit != 0).map(|&(_, button)| button);
        let mut out: Vec<PadEvent> = bits(released).map(PadEvent::Release).collect();
        out.extend(bits(pressed).map(|button| PadEvent::Press(Press { button, repeat: false })));
        let directions = buttons & DIRECTIONS;
        if pressed & DIRECTIONS != 0 {
            self.direction = Some((now, false));
        } else if directions == 0 {
            self.direction = None;
        } else if let Some((last, repeated)) = self.direction {
            if now >= last + if repeated { self.repeat.every } else { self.repeat.after } {
                out.extend(bits(directions).map(|button| PadEvent::Press(Press { button, repeat: true })));
                self.direction = Some((now, true));
            }
        }
        self.held = buttons;
        self.guide(buttons, now, &mut out);
        out
    }

    /// The Guide button, as the Bundle judges it (docs/spec/guide.md, 7.1): by when it went down and when it was let
    /// go, so a hold is a hold however late it is looked at.
    fn guide(&mut self, buttons: u32, now: Instant, out: &mut Vec<PadEvent>) {
        let on_pad = buttons & GUIDE != 0;
        let down = buttons & (GUIDE | GUIDE_KEY) != 0;
        self.guide = match std::mem::take(&mut self.guide) {
            // Pressed again before its tap was told: that tap is dropped, as the Bundle drops it.
            GuideButton::Up | GuideButton::Tapped { .. } if down => {
                out.push(PadEvent::GuideDown);
                GuideButton::Down { since: now, pad: on_pad }
            }
            GuideButton::Up => GuideButton::Up,
            GuideButton::Tapped { due } if now >= due => {
                out.push(PadEvent::GuideTap);
                GuideButton::Up
            }
            tapped @ GuideButton::Tapped { .. } => tapped,
            GuideButton::Down { since, pad } => {
                let pad = pad || on_pad;
                let held = pad && now >= since + HOLD;
                if held {
                    out.push(PadEvent::GuideHeld);
                }
                if down {
                    if held {
                        GuideButton::Held
                    } else {
                        GuideButton::Down { since, pad }
                    }
                } else {
                    out.push(PadEvent::Release(Button::Guide));
                    if held {
                        GuideButton::Up
                    } else if now >= since + TAP_AFTER {
                        out.push(PadEvent::GuideTap);
                        GuideButton::Up
                    } else {
                        GuideButton::Tapped { due: since + TAP_AFTER }
                    }
                }
            }
            GuideButton::Held if down => GuideButton::Held,
            GuideButton::Held => {
                out.push(PadEvent::Release(Button::Guide));
                GuideButton::Up
            }
        };
    }
}

/// The pad's battery is asked after this often, and as a pad connects.
#[cfg_attr(not(windows), allow(dead_code))]
const BATTERY_EVERY: Duration = Duration::from_secs(5);

/// Follows which pad is connected and what powers it.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Default)]
struct Watch {
    status: PadStatus,
    /// The first pad connected, by its place among XInput's four.
    first: Option<u32>,
    battery_read: Option<Instant>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Watch {
    /// `first` is the first pad connected now; `battery` asks a pad what powers it, and is called only when that is
    /// due. True when the status is not what it was.
    fn update(&mut self, first: Option<u32>, now: Instant, battery: impl FnOnce(u32) -> Option<Battery>) -> bool {
        let was = self.status;
        if first != self.first {
            self.first = first;
            self.battery_read = None;
        }
        self.status.connected = first.is_some();
        match first {
            None => self.status.battery = None,
            Some(pad) => {
                if self.battery_read.is_none_or(|read| now >= read + BATTERY_EVERY) {
                    self.status.battery = battery(pad);
                    self.battery_read = Some(now);
                }
            }
        }
        self.status != was
    }
}

/// XInput's battery type and level as what the Status area shows. The four levels are all XInput says of the charge.
#[cfg_attr(not(windows), allow(dead_code))]
fn battery(kind: u8, level: u8) -> Option<Battery> {
    match (kind, level) {
        // Wired.
        (1, _) => Some(Battery::Wired),
        // Alkaline or NiMH: empty, low, medium, full.
        (2 | 3, 0) => Some(Battery::Charge(0.0)),
        (2 | 3, 1) => Some(Battery::Charge(0.3)),
        (2 | 3, 2) => Some(Battery::Charge(0.6)),
        (2 | 3, 3) => Some(Battery::Charge(1.0)),
        // Disconnected, or unknown.
        _ => None,
    }
}

/// One pad as XInput reads it: its buttons (the Guide button's bit among them), its triggers and its sticks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left: (i16, i16),
    pub right: (i16, i16),
}

/// XINPUT_GAMEPAD_TRIGGER_THRESHOLD: a trigger pressed past it is pressed.
const TRIGGER_THRESHOLD: u8 = 30;
/// XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE and the right's: a stick pushed less far than this, any way, is at rest.
const LEFT_DEADZONE: i32 = 7849;
const RIGHT_DEADZONE: i32 = 8689;
/// How far a trigger (of 255) or a stick's axis (of 32767 each way) moves before it counts as moved, not as noise.
const TRIGGER_STEP: i32 = 16;
const STICK_STEP: i32 = 2048;

impl Reading {
    /// Whether someone holds the pad off its rest: a button down, a trigger past its threshold, or a stick pushed past
    /// its deadzone, measured round it as XInput's guidance has, so part way on both axes counts.
    pub fn off_rest(&self) -> bool {
        let pushed = |(x, y): (i16, i16), zone: i32| {
            let (x, y) = (i64::from(x), i64::from(y));
            x * x + y * y > i64::from(zone) * i64::from(zone)
        };
        self.buttons != 0
            || self.left_trigger > TRIGGER_THRESHOLD
            || self.right_trigger > TRIGGER_THRESHOLD
            || pushed(self.left, LEFT_DEADZONE)
            || pushed(self.right, RIGHT_DEADZONE)
    }

    /// Whether someone moved the pad from `before` to this: a button pressed or let go, or a trigger or a stick moved
    /// more than a small step, inside its deadzone or not, so easing one counts and a pad lying still, its readings
    /// jittering, never does.
    pub fn moved_since(&self, before: &Reading) -> bool {
        let moved = |a: i32, b: i32, step: i32| (a - b).abs() > step;
        let axes = [
            (self.left.0, before.left.0),
            (self.left.1, before.left.1),
            (self.right.0, before.right.0),
            (self.right.1, before.right.1),
        ];
        self.buttons != before.buttons
            || moved(self.left_trigger.into(), before.left_trigger.into(), TRIGGER_STEP)
            || moved(self.right_trigger.into(), before.right_trigger.into(), TRIGGER_STEP)
            || axes.iter().any(|&(now, was)| moved(now.into(), was.into(), STICK_STEP))
    }

    /// Whether this reading says someone is using the pad, after `before` (none: the pad had just come): what the
    /// Idle screen counts as input (docs/spec/idle-screen.md).
    pub fn used(&self, before: Option<&Reading>) -> bool {
        self.off_rest() || before.is_some_and(|before| self.moved_since(before))
    }
}

/// What the thread that reads the pads and the one that asks share.
#[derive(Default)]
struct Shared {
    repeat: Mutex<Repeat>,
    status: Mutex<PadStatus>,
    /// When a pad was last used: held off its rest or moved (`Reading::used`).
    used: Mutex<Option<Instant>>,
}

/// The pads, read on a thread of their own every few milliseconds, so a short press is caught however long a frame
/// takes to draw, and the Guide button's 300 ms and 700 ms are told when they are up with no frame being drawn.
pub struct Pad {
    events: std::sync::mpsc::Receiver<PadEvent>,
    shared: Arc<Shared>,
}

/// How often the pads are read.
#[cfg_attr(not(windows), allow(dead_code))]
const READ_EVERY: Duration = Duration::from_millis(4);

#[cfg_attr(not(windows), allow(dead_code))]
struct Reader {
    #[cfg(windows)]
    get_state: Option<unsafe extern "system" fn(u32, *mut State) -> u32>,
    #[cfg(windows)]
    get_battery: Option<unsafe extern "system" fn(u32, u8, *mut BatteryInformation) -> u32>,
}

#[repr(C)]
#[derive(Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct State {
    packet: u32,
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    left_x: i16,
    left_y: i16,
    right_x: i16,
    right_y: i16,
    // XInputGetStateEx writes a little past the documented state.
    reserved: [u32; 2],
}

/// XINPUT_BATTERY_INFORMATION.
#[repr(C)]
#[derive(Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct BatteryInformation {
    kind: u8,
    level: u8,
}

impl Pad {
    /// Starts reading. `wake` is called, on the reading thread, after events have been queued and when a pad has
    /// come, gone, or says something else of its battery.
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let (sender, events) = std::sync::mpsc::channel();
        let shared = Arc::new(Shared::default());
        #[cfg(windows)]
        std::thread::Builder::new()
            .name("pad".into())
            .spawn({
                let shared = shared.clone();
                move || {
                    let reader = Reader::new();
                    let (mut held, mut watch) = (Presses::default(), Watch::default());
                    let mut last: [Option<Reading>; 4] = [None; 4];
                    loop {
                        let now = Instant::now();
                        let (buttons, first, readings) = reader.read();
                        held.set_repeat(*shared.repeat.lock().unwrap());
                        let events = held.update(buttons, now);
                        let mut news = !events.is_empty();
                        for event in events {
                            if sender.send(event).is_err() {
                                return;
                            }
                        }
                        // After the events: whoever sees a pad used also finds the presses it made waiting.
                        let used =
                            readings.iter().zip(&last).any(|(now, was)| now.is_some_and(|n| n.used(was.as_ref())));
                        last = readings;
                        if used {
                            *shared.used.lock().unwrap() = Some(now);
                        }
                        if watch.update(first, now, |pad| reader.battery(pad)) {
                            match first {
                                Some(pad) => {
                                    eprintln!("gamepad: pad {pad} connected, battery {:?}", watch.status.battery)
                                }
                                None => eprintln!("gamepad: none connected"),
                            }
                            *shared.status.lock().unwrap() = watch.status;
                            news = true;
                        }
                        if news {
                            wake();
                        }
                        std::thread::sleep(READ_EVERY);
                    }
                }
            })
            .expect("the pad thread starts");
        #[cfg(not(windows))]
        let _ = (sender, wake);
        Self { events, shared }
    }

    /// What has happened since the last call.
    pub fn poll(&mut self) -> Vec<PadEvent> {
        self.events.try_iter().collect()
    }

    /// Whether a pad is connected, and what powers the first.
    pub fn status(&self) -> PadStatus {
        *self.shared.status.lock().unwrap()
    }

    /// How a held direction repeats from here on: each screen has its own pace, which the Dashboard knows.
    pub fn set_repeat(&self, repeat: Repeat) {
        *self.shared.repeat.lock().unwrap() = repeat;
    }

    /// When someone last used a pad, whichever window is in front: a button, a trigger past its threshold or a stick
    /// past its deadzone held, or any of them moved (`Reading::used`). None since the start, and anywhere but
    /// Windows. Asked before `poll`, whatever presses that use made are in the `poll` after.
    pub fn last_used(&self) -> Option<Instant> {
        *self.shared.used.lock().unwrap()
    }
}

#[cfg(windows)]
impl Reader {
    fn new() -> Self {
        unsafe {
            use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
            let name: Vec<u16> = "xinput1_4.dll".encode_utf16().chain(Some(0)).collect();
            let library = LoadLibraryW(name.as_ptr());
            let find = |name: *const u8| if library.is_null() { None } else { GetProcAddress(library, name) };
            let (state, battery) = (find(100 as *const u8), find(c"XInputGetBatteryInformation".as_ptr().cast()));
            if state.is_none() {
                eprintln!("gamepad: no XInputGetStateEx");
            }
            if battery.is_none() {
                eprintln!("gamepad: no XInputGetBatteryInformation");
            }
            Self {
                get_state: state.map(|f| std::mem::transmute(f)),
                get_battery: battery.map(|f| std::mem::transmute(f)),
            }
        }
    }

    /// Every pad's buttons together, the left stick counting as the d-pad, the first pad connected, and each slot's
    /// reading.
    fn read(&self) -> (u32, Option<u32>, [Option<Reading>; 4]) {
        let (mut buttons, mut first, mut readings) = (0u32, None, [None; 4]);
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
            if let Some(get_state) = self.get_state {
                for user in 0..4 {
                    let mut state = State::default();
                    if get_state(user, &mut state) != 0 {
                        continue;
                    }
                    first = first.or(Some(user));
                    readings[user as usize] = Some(Reading {
                        buttons: state.buttons,
                        left_trigger: state.left_trigger,
                        right_trigger: state.right_trigger,
                        left: (state.left_x, state.left_y),
                        right: (state.right_x, state.right_y),
                    });
                    buttons |= state.buttons as u32;
                    buttons |= if state.left_x > STICK {
                        RIGHT
                    } else if state.left_x < -STICK {
                        LEFT
                    } else {
                        0
                    };
                    buttons |= if state.left_y > STICK {
                        UP
                    } else if state.left_y < -STICK {
                        DOWN
                    } else {
                        0
                    };
                    buttons |= if state.left_trigger > TRIGGER { LEFT_TRIGGER } else { 0 };
                    buttons |= if state.right_trigger > TRIGGER { RIGHT_TRIGGER } else { 0 };
                }
            }
            let down = |key: i32| GetAsyncKeyState(key) < 0;
            if down(VK_CONTROL as i32) && down(VK_MENU as i32) && down('G' as i32) {
                buttons |= GUIDE_KEY;
            }
        }
        (buttons, first, readings)
    }

    /// What powers this pad, as XInput says it.
    fn battery(&self, pad: u32) -> Option<Battery> {
        let mut information = BatteryInformation::default();
        // 0: the pad itself, not its headset.
        if unsafe { self.get_battery?(pad, 0, &mut information) } != 0 {
            return None;
        }
        battery(information.kind, information.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 0x1000;

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    fn press(button: Button) -> PadEvent {
        PadEvent::Press(Press { button, repeat: false })
    }

    fn repeat(button: Button) -> PadEvent {
        PadEvent::Press(Press { button, repeat: true })
    }

    const GUIDE_UP: PadEvent = PadEvent::Release(Button::Guide);

    #[test]
    fn a_button_presses_once_however_long_it_is_held_and_is_released_once() {
        let mut presses = Presses::default();
        let start = Instant::now();
        assert_eq!(presses.update(A, start), [press(Button::A)]);
        assert!(presses.update(A, start + Duration::from_secs(2)).is_empty());
        assert_eq!(presses.update(0, start + Duration::from_secs(3)), [PadEvent::Release(Button::A)]);
        assert!(presses.update(0, start + Duration::from_secs(4)).is_empty());
        assert_eq!(presses.update(A, start + Duration::from_secs(5)), [press(Button::A)]);
    }

    #[test]
    fn a_held_direction_repeats_after_a_pause() {
        let mut presses = Presses::default();
        let (start, pace) = (Instant::now(), Repeat::default());
        assert_eq!(presses.update(RIGHT, start), [press(Button::Right)]);
        assert!(presses.update(RIGHT, start + ms(300)).is_empty());
        assert_eq!(presses.update(RIGHT, start + pace.after), [repeat(Button::Right)]);
        assert!(presses.update(RIGHT, start + pace.after + ms(50)).is_empty());
        assert_eq!(presses.update(RIGHT, start + pace.after + pace.every), [repeat(Button::Right)]);
    }

    #[test]
    fn a_button_pressed_while_a_direction_is_held_does_not_restart_the_repeat() {
        let mut presses = Presses::default();
        let (start, pace) = (Instant::now(), Repeat::default());
        presses.update(RIGHT, start);
        assert_eq!(presses.update(RIGHT | A, start + ms(200)), [press(Button::A)]);
        assert_eq!(presses.update(RIGHT | A, start + pace.after), [repeat(Button::Right)]);
    }

    #[test]
    fn a_held_direction_repeats_at_the_pace_it_is_told() {
        // The Guide's: 700 ms, then every 80 ms.
        let mut presses = Presses::default();
        presses.set_repeat(Repeat::millis(700, 80));
        let start = Instant::now();
        assert_eq!(presses.update(DOWN, start), [press(Button::Down)]);
        assert!(presses.update(DOWN, start + ms(699)).is_empty());
        assert_eq!(presses.update(DOWN, start + ms(700)), [repeat(Button::Down)]);
        assert!(presses.update(DOWN, start + ms(779)).is_empty());
        assert_eq!(presses.update(DOWN, start + ms(780)), [repeat(Button::Down)]);
        assert_eq!(presses.update(DOWN, start + ms(860)), [repeat(Button::Down)]);

        // Home's again, the direction still held: the next repeat is Home's interval after the last.
        presses.set_repeat(Repeat::default());
        assert!(presses.update(DOWN, start + ms(940)).is_empty());
        assert_eq!(presses.update(DOWN, start + ms(980)), [repeat(Button::Down)]);
        assert_eq!(presses.update(0, start + ms(990)), [PadEvent::Release(Button::Down)]);
    }

    #[test]
    fn a_tap_of_the_guide_button_is_told_300_ms_after_the_press() {
        let mut presses = Presses::default();
        let start = Instant::now();
        // Only that it went down: it may yet be a hold.
        assert_eq!(presses.update(GUIDE, start), [PadEvent::GuideDown]);
        assert!(presses.update(GUIDE, start + ms(100)).is_empty());
        assert_eq!(presses.update(0, start + ms(120)), [GUIDE_UP]);
        assert!(presses.update(0, start + ms(299)).is_empty());
        assert_eq!(presses.update(0, start + ms(300)), [PadEvent::GuideTap]);
        assert!(presses.update(0, start + ms(2000)).is_empty());
    }

    #[test]
    fn a_tap_let_go_after_300_ms_is_told_as_it_is_let_go() {
        let mut presses = Presses::default();
        let start = Instant::now();
        presses.update(GUIDE, start);
        assert!(presses.update(GUIDE, start + ms(500)).is_empty());
        assert_eq!(presses.update(0, start + ms(699)), [GUIDE_UP, PadEvent::GuideTap]);
        assert!(presses.update(0, start + ms(1000)).is_empty());
    }

    #[test]
    fn the_guide_button_down_700_ms_is_a_hold_and_no_tap() {
        let mut presses = Presses::default();
        let start = Instant::now();
        presses.update(GUIDE, start);
        assert!(presses.update(GUIDE, start + ms(699)).is_empty());
        assert_eq!(presses.update(GUIDE, start + ms(700)), [PadEvent::GuideHeld]);
        // Once, however long it stays down, and letting go is only that.
        assert!(presses.update(GUIDE, start + ms(3000)).is_empty());
        assert_eq!(presses.update(0, start + ms(3100)), [GUIDE_UP]);
        assert!(presses.update(0, start + ms(5000)).is_empty());
    }

    #[test]
    fn a_hold_is_judged_by_the_clock_however_late_the_button_is_looked_at() {
        let mut presses = Presses::default();
        let start = Instant::now();
        presses.update(GUIDE, start);
        // Not looked at between the press and a release 800 ms later.
        assert_eq!(presses.update(0, start + ms(800)), [PadEvent::GuideHeld, GUIDE_UP]);
        assert!(presses.update(0, start + ms(2000)).is_empty());
    }

    #[test]
    fn the_guide_button_pressed_again_before_its_tap_is_told_drops_that_tap() {
        let mut presses = Presses::default();
        let start = Instant::now();
        presses.update(GUIDE, start);
        assert_eq!(presses.update(0, start + ms(100)), [GUIDE_UP]);
        assert_eq!(presses.update(GUIDE, start + ms(200)), [PadEvent::GuideDown]);
        assert!(presses.update(GUIDE, start + ms(400)).is_empty());
        assert_eq!(presses.update(0, start + ms(450)), [GUIDE_UP]);
        assert_eq!(presses.update(0, start + ms(500)), [PadEvent::GuideTap]);
    }

    #[test]
    fn other_buttons_go_on_as_they_do_while_the_guide_button_is_down() {
        let mut presses = Presses::default();
        let start = Instant::now();
        assert_eq!(presses.update(GUIDE, start), [PadEvent::GuideDown]);
        assert_eq!(presses.update(GUIDE | A, start + ms(100)), [press(Button::A)]);
        assert_eq!(presses.update(GUIDE, start + ms(700)), [PadEvent::Release(Button::A), PadEvent::GuideHeld]);
    }

    #[test]
    fn the_keyboards_guide_button_is_a_tap_however_long_it_is_held() {
        let mut presses = Presses::default();
        let start = Instant::now();
        assert_eq!(presses.update(GUIDE_KEY, start), [PadEvent::GuideDown]);
        assert!(presses.update(GUIDE_KEY, start + ms(2000)).is_empty());
        assert_eq!(presses.update(0, start + ms(2100)), [GUIDE_UP, PadEvent::GuideTap]);
    }

    #[test]
    fn a_pad_connecting_and_leaving_is_told_once_each() {
        let mut watch = Watch::default();
        let start = Instant::now();
        assert!(!watch.update(None, start, |_| unreachable!("no pad to ask")));
        assert!(watch.update(Some(1), start + ms(4), |pad| (pad == 1).then_some(Battery::Wired)));
        assert_eq!(watch.status, PadStatus { connected: true, battery: Some(Battery::Wired) });
        assert!(!watch.update(Some(1), start + ms(8), |_| unreachable!("asked only every few seconds")));
        assert!(watch.update(None, start + ms(12), |_| unreachable!("no pad to ask")));
        assert_eq!(watch.status, PadStatus::default());
        assert!(!watch.update(None, start + ms(16), |_| unreachable!("no pad to ask")));
    }

    #[test]
    fn the_battery_is_asked_after_as_a_pad_connects_and_every_few_seconds() {
        let mut watch = Watch::default();
        let start = Instant::now();
        assert!(watch.update(Some(0), start, |_| Some(Battery::Charge(1.0))));
        assert!(!watch.update(Some(0), start + BATTERY_EVERY - ms(1), |_| unreachable!("not yet")));
        // The same answer is no news; another is.
        assert!(!watch.update(Some(0), start + BATTERY_EVERY, |_| Some(Battery::Charge(1.0))));
        assert!(!watch.update(Some(0), start + BATTERY_EVERY + ms(4), |_| unreachable!("not yet")));
        assert!(watch.update(Some(0), start + BATTERY_EVERY * 2, |_| Some(Battery::Charge(0.6))));
        assert_eq!(watch.status.battery, Some(Battery::Charge(0.6)));

        // Another pad is the first now: asked at once.
        assert!(watch.update(Some(2), start + BATTERY_EVERY * 2 + ms(4), |pad| (pad == 2).then_some(Battery::Wired)));
        assert_eq!(watch.status, PadStatus { connected: true, battery: Some(Battery::Wired) });
        // A pad that does not say what powers it is still a pad.
        assert!(watch.update(Some(2), start + BATTERY_EVERY * 4, |_| None));
        assert_eq!(watch.status, PadStatus { connected: true, battery: None });
    }

    #[test]
    fn a_pad_held_off_its_rest_is_in_use_and_one_at_rest_is_not() {
        let rest = Reading::default();
        assert!(!rest.used(None) && !rest.used(Some(&rest)));
        // A button, the Guide button's bit among them.
        assert!(Reading { buttons: 0x0400, ..rest }.used(Some(&rest)));
        // A trigger past its threshold, not at it.
        assert!(!Reading { left_trigger: 30, ..rest }.off_rest());
        assert!(Reading { right_trigger: 31, ..rest }.off_rest());
        // A stick past its deadzone round it: part way on both axes counts, where neither alone would.
        assert!(!Reading { left: (7000, 0), ..rest }.off_rest());
        assert!(Reading { left: (6000, 6000), ..rest }.off_rest());
        assert!(!Reading { right: (8689, 0), ..rest }.off_rest());
        assert!(Reading { right: (0, -8690), ..rest }.off_rest());
    }

    #[test]
    fn a_pad_moved_inside_its_deadzone_is_in_use_and_one_jittering_is_not() {
        let lying = Reading { left: (1200, -900), right: (300, 400), left_trigger: 3, ..Reading::default() };
        let jitter = Reading { left: (1300, -1000), right: (250, 450), left_trigger: 10, ..lying };
        assert!(!jitter.used(Some(&lying)));
        // Eased a little more than a step, inside the deadzone still.
        assert!(Reading { left: (1200 + 2049, -900), ..lying }.used(Some(&lying)));
        assert!(Reading { left_trigger: 3 + 17, ..lying }.used(Some(&lying)));
        // A pad that has just come counts only if it is held.
        assert!(!lying.used(None));
    }

    #[test]
    fn xinputs_battery_is_read_as_wired_or_a_charge() {
        assert_eq!(battery(1, 0), Some(Battery::Wired));
        assert_eq!(battery(2, 3), Some(Battery::Charge(1.0)));
        assert_eq!(battery(3, 0), Some(Battery::Charge(0.0)));
        // Disconnected, and unknown.
        assert_eq!(battery(0, 0), None);
        assert_eq!(battery(0xFF, 2), None);
    }
}
