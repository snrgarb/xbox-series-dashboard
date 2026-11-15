//! The gamepad, read through XInput so it works whether or not Home has the foreground: the Guide button has to open
//! the Guide over a game. `XInputGetStateEx` (xinput1_4's ordinal 100) is the call that reports the Guide button, as
//! docs/research/guide-button-capture.md found. Ctrl+Alt+G stands in for the Guide button on a keyboard.
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Moves {
    /// Left and right.
    pub by: i32,
    /// Up and down.
    pub down: i32,
    /// A.
    pub select: bool,
    /// B.
    pub back: bool,
    /// The Guide button.
    pub guide: bool,
}

const UP: u16 = 0x0001;
const DOWN: u16 = 0x0002;
const LEFT: u16 = 0x0004;
const RIGHT: u16 = 0x0008;
const GUIDE: u16 = 0x0400;
const A: u16 = 0x1000;
const B: u16 = 0x2000;
#[cfg_attr(not(windows), allow(dead_code))]
const STICK: i16 = 20000;

pub struct Pad {
    #[cfg(windows)]
    get_state: Option<unsafe extern "system" fn(u32, *mut State) -> u32>,
    held: u16,
    repeat_at: Instant,
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

impl Default for Pad {
    fn default() -> Self {
        Self::new()
    }
}

impl Pad {
    pub fn new() -> Self {
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
            held: 0,
            repeat_at: Instant::now(),
        }
    }

    /// Every pad's buttons together, the left stick counting as the d-pad.
    fn buttons(&self) -> u16 {
        #[allow(unused_mut)]
        let mut buttons = 0u16;
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
            if let Some(get_state) = self.get_state {
                for user in 0..4 {
                    let mut state = State::default();
                    if get_state(user, &mut state) != 0 {
                        continue;
                    }
                    buttons |= state.buttons;
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
                }
            }
            let down = |key: i32| GetAsyncKeyState(key) < 0;
            if down(VK_CONTROL as i32) && down(VK_MENU as i32) && down('G' as i32) {
                buttons |= GUIDE;
            }
        }
        buttons
    }

    pub fn poll(&mut self) -> Moves {
        let buttons = self.buttons();
        let now = Instant::now();
        let mut pressed = buttons & !self.held;
        // A held direction repeats.
        let directions = buttons & (UP | DOWN | LEFT | RIGHT);
        if pressed & directions != 0 {
            self.repeat_at = now + Duration::from_millis(400);
        } else if directions != 0 && now >= self.repeat_at {
            pressed |= directions;
            self.repeat_at = now + Duration::from_millis(150);
        }
        self.held = buttons;
        let on = |button: u16| pressed & button != 0;
        Moves {
            by: on(RIGHT) as i32 - on(LEFT) as i32,
            down: on(DOWN) as i32 - on(UP) as i32,
            select: on(A),
            back: on(B),
            guide: on(GUIDE),
        }
    }
}
