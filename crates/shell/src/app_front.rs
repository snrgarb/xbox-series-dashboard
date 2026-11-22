//! An App started from its tile comes up in front of the Dashboard and maximised (docs/spec/apps.md, section 1), as
//! an app on the console fills the screen. Before this an App was opened and left there: a Store app such as
//! Settings or the Microsoft Store came up behind Home's window, at whatever size it had last.
//!
//! What is open before the App starts is noted ([`shown_now`]). Once it has been asked for, a thread of its own
//! looks every [`LOOK_EVERY`] for its window, for up to [`GIVE_UP`]: one of the process its activation gave (a Store
//! app's, whose window is the frame `ApplicationFrameHost` draws round it), else the first window shown that was
//! not shown before. That window is maximised where it can be (resizable, with a maximise box, not already
//! maximised or covering its screen: a program that makes itself full screen is left as it is), and given the
//! foreground. The log says which: `apps: <window> maximised, given the foreground: true`.
#![cfg_attr(not(windows), allow(dead_code))]
use std::time::Duration;

/// How often the desktop is looked at for the App's window.
pub const LOOK_EVERY: Duration = Duration::from_millis(100);
/// How long the App's window is waited for: an App already running that shows nothing new is let be after this.
pub const GIVE_UP: Duration = Duration::from_secs(15);

/// `WS_THICKFRAME` and `WS_MAXIMIZEBOX`: a window the user could maximise themselves.
const RESIZABLE: u32 = 0x0004_0000;
const MAXIMIZE_BOX: u32 = 0x0001_0000;

/// A top-level window at a look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    pub window: isize,
    /// The process whose window it is; for a Store app's frame, the app's own (its `CoreWindow`'s).
    pub pid: u32,
    /// Visible, not cloaked, owned by no other window, not a tool window, and of some size: a window the user sees
    /// as one of its own.
    pub shown: bool,
}

/// The App's window among `now`, the windows at this look in z-order: a shown one of `pid`'s, the process its
/// activation gave, else the first shown one that was not among `before`. Never one of `ours`, the Dashboard's.
pub fn the_apps(now: &[Seen], before: &[isize], pid: Option<u32>, ours: u32) -> Option<isize> {
    let candidates = || now.iter().filter(|seen| seen.shown && seen.pid != ours);
    candidates()
        .find(|seen| pid.is_some_and(|pid| seen.pid == pid))
        .or_else(|| candidates().find(|seen| !before.contains(&seen.window)))
        .map(|seen| seen.window)
}

/// Whether a window of this style is maximised: one the user could maximise, not maximised already and not
/// covering its screen already.
pub fn to_maximise(style: u32, maximised: bool, covers_screen: bool) -> bool {
    style & RESIZABLE != 0 && style & MAXIMIZE_BOX != 0 && !maximised && !covers_screen
}

/// The windows shown now, to tell the App's from: called before the App is asked for.
#[cfg(windows)]
pub fn shown_now() -> Vec<isize> {
    win::look().into_iter().filter(|seen| seen.shown).map(|seen| seen.window).collect()
}

/// Follows the App `name`, just asked for, on a thread of its own: its window, once up, maximised and given the
/// foreground. `before` is [`shown_now`] from before it was asked for, `pid` the process its activation gave.
#[cfg(windows)]
pub fn follow(name: String, before: Vec<isize>, pid: Option<u32>) {
    let followed = name.clone();
    let spawned = std::thread::Builder::new().name("app-front".into()).spawn(move || {
        let started = std::time::Instant::now();
        loop {
            let now = win::look();
            if let Some(window) = the_apps(&now, &before, pid, std::process::id()) {
                win::bring_up(&name, window, started.elapsed());
                return;
            }
            if started.elapsed() >= GIVE_UP {
                eprintln!(
                    "apps: no window of {name}'s came in {} s: left as it is at {:.0} ms",
                    GIVE_UP.as_secs(),
                    crate::since_start()
                );
                return;
            }
            std::thread::sleep(LOOK_EVERY);
        }
    });
    if let Err(why) = spawned {
        eprintln!("apps: {followed}'s window not followed: {why}");
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, ShowWindowAsync, GWL_EXSTYLE, GWL_STYLE,
        GW_OWNER, SW_MAXIMIZE, SW_RESTORE, WS_EX_TOOLWINDOW,
    };

    use super::Seen;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    unsafe fn class(window: HWND) -> String {
        let mut name = [0u16; 128];
        let len = GetClassNameW(window, name.as_mut_ptr(), name.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&name[..len])
    }

    unsafe fn pid_of(window: HWND) -> u32 {
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        pid
    }

    /// Every top-level window, in z-order.
    pub fn look() -> Vec<Seen> {
        let mut seen = Vec::new();
        unsafe { EnumWindows(Some(each), &mut seen as *mut Vec<Seen> as LPARAM) };
        seen
    }

    unsafe extern "system" fn each(window: HWND, param: LPARAM) -> i32 {
        let seen = &mut *(param as *mut Vec<Seen>);
        let class = class(window);
        // A Store app's own window, before its frame takes it in: never the one to move.
        if class == "Windows.UI.Core.CoreWindow" {
            return 1;
        }
        let mut pid = pid_of(window);
        if class == "ApplicationFrameWindow" {
            let core = wide("Windows.UI.Core.CoreWindow");
            let inner = FindWindowExW(window, std::ptr::null_mut(), core.as_ptr(), std::ptr::null());
            if !inner.is_null() {
                pid = pid_of(inner);
            }
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(window, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut c_void, 4);
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(window, &mut rect);
        let tool = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0;
        let shown = IsWindowVisible(window) != 0
            && cloaked == 0
            && GetWindow(window, GW_OWNER).is_null()
            && !tool
            && rect.right > rect.left
            && rect.bottom > rect.top;
        seen.push(Seen { window: window as isize, pid, shown });
        1
    }

    /// Whether `window` covers the whole of the screen it is on.
    unsafe fn covers_screen(window: HWND) -> bool {
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(window, &mut rect);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
            return false;
        }
        let screen = info.rcMonitor;
        rect.left <= screen.left && rect.top <= screen.top && rect.right >= screen.right && rect.bottom >= screen.bottom
    }

    /// The App's window maximised where it can be, and given the foreground; the log says what was done.
    pub fn bring_up(name: &str, window: isize, after: Duration) {
        let window = window as HWND;
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
            let maximise = super::to_maximise(style, IsZoomed(window) != 0, covers_screen(window));
            let how = if maximise {
                ShowWindowAsync(window, SW_MAXIMIZE);
                "maximised"
            } else if IsIconic(window) != 0 {
                ShowWindowAsync(window, SW_RESTORE);
                "restored, not maximised (it can't be, or is already)"
            } else {
                "not maximised (it can't be, or is already)"
            };
            let given = crate::take_foreground(window);
            eprintln!(
                "apps: {name}'s window {} up after {:.0} ms: {how}, given the foreground: {given} at {:.0} ms",
                crate::describe(window),
                after.as_secs_f64() * 1000.0,
                crate::since_start()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: u32 = 10;

    fn seen(window: isize, pid: u32, shown: bool) -> Seen {
        Seen { window, pid, shown }
    }

    #[test]
    fn the_apps_window_is_its_process_s_even_where_it_was_there_before() {
        // Settings running already, brought up again: its frame was there, and is the one.
        let now = [seen(1, OURS, true), seen(2, 30, true), seen(3, 20, true)];
        assert_eq!(the_apps(&now, &[1, 2, 3], Some(20), OURS), Some(3));
    }

    #[test]
    fn with_no_process_known_the_app_s_window_is_the_first_new_one_shown() {
        let now = [seen(1, OURS, true), seen(4, 40, false), seen(5, 50, true), seen(6, 60, true), seen(2, 30, true)];
        assert_eq!(the_apps(&now, &[1, 2], None, OURS), Some(5));
    }

    #[test]
    fn a_window_not_shown_or_the_dashboard_s_is_never_the_app_s() {
        let now = [seen(1, OURS, true), seen(7, 20, false)];
        assert_eq!(the_apps(&now, &[], Some(20), OURS), None);
        assert_eq!(the_apps(&now, &[], None, OURS), None);
    }

    #[test]
    fn a_process_with_nothing_shown_yet_falls_back_to_a_new_window() {
        // A Store app's frame still ApplicationFrameHost's, its CoreWindow not yet inside it.
        let now = [seen(8, 99, true), seen(2, 30, true)];
        assert_eq!(the_apps(&now, &[2], Some(20), OURS), Some(8));
    }

    #[test]
    fn only_a_window_the_user_could_maximise_is_maximised() {
        let overlapped = 0x00CF_0000;
        assert!(to_maximise(overlapped, false, false));
        assert!(!to_maximise(overlapped, true, false), "maximised already");
        assert!(!to_maximise(overlapped, false, true), "full screen already");
        let popup = 0x8000_0000;
        assert!(!to_maximise(popup, false, false), "a borderless window of a fixed size");
        assert!(!to_maximise(overlapped & !MAXIMIZE_BOX, false, false), "no maximise box");
    }
}
