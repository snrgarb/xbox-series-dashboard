//! When anyone last used the keyboard or the mouse, anywhere in Windows, whichever window is in front: what the Idle
//! screen counts from, with the pads' own (`input::Pad::last_used`) (docs/spec/idle-screen.md). Windows keeps the
//! time of the last keyboard and mouse input of the session (`GetLastInputInfo`); it knows nothing of XInput's pads,
//! which the pad's thread watches itself.
//!
//! Behind a trait, so the rest is the same everywhere: on Windows it is Windows' word, anywhere else a stand-in that
//! is told (`FakeInput`), where nothing but the Dashboard's own presses counts.
use std::time::{Duration, Instant};

/// When the keyboard or the mouse was last used.
pub trait LastInput {
    /// The moment of the last keyboard or mouse input anywhere in the session, as near as is known; None where
    /// nothing is known of any.
    fn last_input(&mut self) -> Option<Instant>;
}

/// Windows' own count: `GetLastInputInfo`, against the tick count it is in.
#[cfg(windows)]
#[derive(Default)]
pub struct SystemInput;

#[cfg(windows)]
impl LastInput for SystemInput {
    fn last_input(&mut self) -> Option<Instant> {
        use windows_sys::Win32::System::SystemInformation::GetTickCount;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        // The tick wraps every 49.7 days: how long ago, not which tick, is what is compared.
        let (read, now) = unsafe { (GetLastInputInfo(&mut info), GetTickCount()) };
        (read != 0).then(|| ago(now, info.dwTime))
    }
}

/// The moment `then` was, of Windows' tick count, when the count is `now`: how long ago, wrapping as the count does.
#[cfg_attr(not(windows), allow(dead_code))]
fn ago(now: u32, then: u32) -> Instant {
    let since = Duration::from_millis(u64::from(now.wrapping_sub(then)));
    let at = Instant::now();
    at.checked_sub(since).unwrap_or(at)
}

/// The stand-in: input whenever it is told (`input_at`), and none before.
#[derive(Debug, Default, Clone)]
pub struct FakeInput {
    pub at: Option<Instant>,
}

impl LastInput for FakeInput {
    fn last_input(&mut self) -> Option<Instant> {
        self.at
    }
}

/// Windows' count on Windows; the stand-in, told nothing, anywhere else.
pub fn system() -> Box<dyn LastInput> {
    #[cfg(windows)]
    return Box::new(SystemInput);
    #[cfg(not(windows))]
    Box::new(FakeInput::default())
}

/// The later of two moments of input, either of which may be unknown: the Idle screen counts from the last input of
/// any kind (docs/spec/idle-screen.md, 2).
pub fn later(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_time_counts_from_the_later_input() {
        let start = Instant::now();
        let (keys, pad) = (start + Duration::from_secs(3), start + Duration::from_secs(5));
        assert_eq!(later(Some(keys), Some(pad)), Some(pad));
        assert_eq!(later(Some(pad), Some(keys)), Some(pad));
        assert_eq!(later(None, Some(keys)), Some(keys));
        assert_eq!(later(Some(keys), None), Some(keys));
        assert_eq!(later(None, None), None);
    }

    #[test]
    fn windows_tick_is_read_as_how_long_ago_across_its_wrap() {
        let before = Instant::now();
        let at = ago(5_000, 3_000);
        let since = before.saturating_duration_since(at);
        assert!(since >= Duration::from_millis(1_990) && since <= Duration::from_millis(2_010), "{since:?}");
        // The count went round between the input and now.
        let at = ago(500, u32::MAX - 499);
        assert!(Instant::now().duration_since(at) >= Duration::from_millis(1_000));
        assert!(Instant::now().duration_since(at) < Duration::from_millis(1_100));
    }

    #[test]
    fn the_stand_in_says_what_it_is_told() {
        let mut fake = FakeInput::default();
        assert_eq!(fake.last_input(), None);
        let now = Instant::now();
        fake.at = Some(now);
        assert_eq!(fake.last_input(), Some(now));
    }
}
