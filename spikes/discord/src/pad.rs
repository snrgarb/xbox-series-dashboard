//! The pad, as the sign-in step reads it: XInput's buttons turned into presses (a held direction repeats, as the
//! Dashboard's do), the left stick into pointer movement and the right stick into scrolling. Pure; the reading is in
//! `win::sign_in`.
use std::time::{Duration, Instant};

pub const UP: u16 = 0x0001;
pub const DOWN: u16 = 0x0002;
pub const LEFT: u16 = 0x0004;
pub const RIGHT: u16 = 0x0008;
pub const MENU: u16 = 0x0010;
pub const VIEW: u16 = 0x0020;
pub const LB: u16 = 0x0100;
pub const RB: u16 = 0x0200;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const Y: u16 = 0x8000;
const DIRECTIONS: u16 = UP | DOWN | LEFT | RIGHT;

/// The Dashboard's own pace for a held direction (ui::Repeat's default).
const REPEAT_AFTER: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(110);

#[derive(Default)]
pub struct Presses {
    held: u16,
    /// When the held directions last pressed, and whether that was a repeat.
    direction: Option<(Instant, bool)>,
}

impl Presses {
    /// The buttons that pressed since the last look, a held direction again at the repeat's pace.
    pub fn update(&mut self, buttons: u16, now: Instant) -> u16 {
        let mut pressed = buttons & !self.held;
        let directions = buttons & DIRECTIONS;
        if pressed & DIRECTIONS != 0 {
            self.direction = Some((now, false));
        } else if directions == 0 {
            self.direction = None;
        } else if let Some((last, repeated)) = self.direction {
            if now >= last + if repeated { REPEAT_EVERY } else { REPEAT_AFTER } {
                pressed |= directions;
                self.direction = Some((now, true));
            }
        }
        self.held = buttons;
        pressed
    }
}

/// XInput's recommended dead zone for the left stick.
const DEAD: i32 = 7849;

/// Pixels to move the pointer for one look (every 16 ms) at this stick position: nothing in the dead zone, then
/// growing with the square of the push, up to `fastest`. XInput's y grows upwards; the screen's downwards.
pub fn pointer(x: i16, y: i16, fastest: f32) -> (i32, i32) {
    let along = |v: i16| {
        let v = i32::from(v);
        if v.abs() <= DEAD {
            return 0.0;
        }
        let t = (v.abs() - DEAD) as f32 / (32767 - DEAD) as f32;
        v.signum() as f32 * t * t * fastest
    };
    (along(x).round() as i32, -along(y).round() as i32)
}

/// Wheel units for one look at the right stick's y: up scrolls up.
pub fn scroll(y: i16) -> i32 {
    let (_, dy) = pointer(0, y, 60.0);
    -dy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_once_then_a_held_direction_repeats() {
        let t = Instant::now();
        let mut p = Presses::default();
        assert_eq!(p.update(A, t), A);
        assert_eq!(p.update(A, t + Duration::from_millis(500)), 0, "A held does not repeat");
        assert_eq!(p.update(DOWN, t), DOWN);
        assert_eq!(p.update(DOWN, t + Duration::from_millis(100)), 0);
        assert_eq!(p.update(DOWN, t + Duration::from_millis(400)), DOWN);
        assert_eq!(p.update(DOWN, t + Duration::from_millis(450)), 0);
        assert_eq!(p.update(DOWN, t + Duration::from_millis(510)), DOWN);
        assert_eq!(p.update(0, t + Duration::from_millis(600)), 0);
    }

    #[test]
    fn the_stick_moves_nothing_at_rest_and_most_pushed_full() {
        assert_eq!(pointer(3000, -3000, 30.0), (0, 0));
        assert_eq!(pointer(32767, 0, 30.0), (30, 0));
        assert_eq!(pointer(0, 32767, 30.0), (0, -30), "up on the stick is up on the screen");
        assert!(pointer(16000, 0, 30.0).0 < 10, "half a push is slow");
        assert!(scroll(32767) > 0 && scroll(-32767) < 0);
    }
}
