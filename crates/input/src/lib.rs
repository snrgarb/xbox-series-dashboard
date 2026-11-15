//! The gamepad, read through XInput so it works whether or not the Dashboard has the foreground: the Guide button
//! has to open the Guide over a game. `XInputGetStateEx` (xinput1_4's ordinal 100) is the call that reports the Guide
//! button, as the theme's docs/research/guide-button-capture.md found. Ctrl+Alt+G stands in for the Guide button on a
//! keyboard.
use std::time::{Duration, Instant};

use ui::{Button, Press};

/// XInput's button bits, and two of our own for the triggers.
const BITS: [(u32, Button); 15] = [
    (0x0001, Button::Up),
    (0x0002, Button::Down),
    (0x0004, Button::Left),
    (0x0008, Button::Right),
    (0x0010, Button::Menu),
    (0x0020, Button::View),
    (0x0100, Button::LeftBumper),
    (0x0200, Button::RightBumper),
    (0x0400, Button::Guide),
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
#[cfg_attr(not(windows), allow(dead_code))]
const GUIDE: u32 = 0x0400;
const LEFT_TRIGGER: u32 = 0x1_0000;
const RIGHT_TRIGGER: u32 = 0x2_0000;
const DIRECTIONS: u32 = UP | DOWN | LEFT | RIGHT;
#[cfg_attr(not(windows), allow(dead_code))]
const STICK: i16 = 20000;
#[cfg_attr(not(windows), allow(dead_code))]
const TRIGGER: u8 = 64;

/// A held direction repeats after this long, then this often.
const REPEAT_AFTER: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(120);

/// Turns what is held, looked at again and again, into presses: one as a button goes down, and more as a
/// direction stays held.
#[derive(Default)]
pub struct Presses {
    held: u32,
    repeat_at: Option<Instant>,
}

impl Presses {
    pub fn update(&mut self, buttons: u32, now: Instant) -> Vec<Press> {
        let pressed = buttons & !self.held;
        let mut out: Vec<Press> = BITS
            .iter()
            .filter(|(bit, _)| pressed & bit != 0)
            .map(|&(_, button)| Press { button, repeat: false })
            .collect();
        let directions = buttons & DIRECTIONS;
        if pressed & DIRECTIONS != 0 {
            self.repeat_at = Some(now + REPEAT_AFTER);
        } else if directions == 0 {
            self.repeat_at = None;
        } else if self.repeat_at.is_some_and(|at| now >= at) {
            out.extend(
                BITS.iter().filter(|(bit, _)| directions & bit != 0).map(|&(_, button)| Press { button, repeat: true }),
            );
            self.repeat_at = Some(now + REPEAT_EVERY);
        }
        self.held = buttons;
        out
    }
}

/// The pads, read on a thread of their own every few milliseconds, so a short press is caught however long a frame
/// takes to draw.
pub struct Pad {
    presses: std::sync::mpsc::Receiver<Press>,
}

/// How often the pads are read.
#[cfg_attr(not(windows), allow(dead_code))]
const READ_EVERY: Duration = Duration::from_millis(4);

#[cfg_attr(not(windows), allow(dead_code))]
struct Reader {
    #[cfg(windows)]
    get_state: Option<unsafe extern "system" fn(u32, *mut State) -> u32>,
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

impl Pad {
    /// Starts reading. `wake` is called, on the reading thread, after presses have been queued.
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let (sender, presses) = std::sync::mpsc::channel();
        #[cfg(windows)]
        std::thread::Builder::new()
            .name("pad".into())
            .spawn(move || {
                let reader = Reader::new();
                let mut held = Presses::default();
                loop {
                    let pressed = held.update(reader.buttons(), Instant::now());
                    let any = !pressed.is_empty();
                    for press in pressed {
                        if sender.send(press).is_err() {
                            return;
                        }
                    }
                    if any {
                        wake();
                    }
                    std::thread::sleep(READ_EVERY);
                }
            })
            .expect("the pad thread starts");
        #[cfg(not(windows))]
        let _ = (sender, wake);
        Self { presses }
    }

    /// What has been pressed since the last call.
    pub fn poll(&mut self) -> Vec<Press> {
        self.presses.try_iter().collect()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Reader {
    fn new() -> Self {
        Self {
            #[cfg(windows)]
            get_state: unsafe {
                use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
                let name: Vec<u16> = "xinput1_4.dll".encode_utf16().chain(Some(0)).collect();
                let library = LoadLibraryW(name.as_ptr());
                let found = if library.is_null() { None } else { GetProcAddress(library, 100 as *const u8) };
                if found.is_none() {
                    eprintln!("gamepad: no XInputGetStateEx");
                }
                found.map(|f| std::mem::transmute(f))
            },
        }
    }

    /// Every pad's buttons together, the left stick counting as the d-pad.
    fn buttons(&self) -> u32 {
        #[allow(unused_mut)]
        let mut buttons = 0u32;
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
            if let Some(get_state) = self.get_state {
                for user in 0..4 {
                    let mut state = State::default();
                    if get_state(user, &mut state) != 0 {
                        continue;
                    }
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
                buttons |= GUIDE;
            }
        }
        buttons
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_presses_once_however_long_it_is_held() {
        let mut presses = Presses::default();
        let start = Instant::now();
        assert_eq!(presses.update(0x1000, start), [Press { button: Button::A, repeat: false }]);
        assert!(presses.update(0x1000, start + Duration::from_secs(2)).is_empty());
        assert!(presses.update(0, start + Duration::from_secs(3)).is_empty());
        assert_eq!(presses.update(0x1000, start + Duration::from_secs(4)).len(), 1);
    }

    #[test]
    fn a_held_direction_repeats_after_a_pause() {
        let mut presses = Presses::default();
        let start = Instant::now();
        assert_eq!(presses.update(RIGHT, start), [Press { button: Button::Right, repeat: false }]);
        assert!(presses.update(RIGHT, start + Duration::from_millis(300)).is_empty());
        assert_eq!(presses.update(RIGHT, start + REPEAT_AFTER), [Press { button: Button::Right, repeat: true }]);
        assert!(presses.update(RIGHT, start + REPEAT_AFTER + Duration::from_millis(50)).is_empty());
        assert_eq!(presses.update(RIGHT, start + REPEAT_AFTER + REPEAT_EVERY).len(), 1);
    }

    #[test]
    fn a_button_pressed_while_a_direction_is_held_does_not_restart_the_repeat() {
        let mut presses = Presses::default();
        let start = Instant::now();
        presses.update(RIGHT, start);
        assert_eq!(
            presses.update(RIGHT | 0x1000, start + Duration::from_millis(200)),
            [Press { button: Button::A, repeat: false }]
        );
        assert_eq!(presses.update(RIGHT | 0x1000, start + REPEAT_AFTER).len(), 1);
    }
}
