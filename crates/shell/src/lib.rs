//! Windows itself: the Overlay, the window above every other that the Launch screen and the Guide are drawn on,
//! the Toast strip above it, launching a game through Steam and following it until it ends, and holding it still
//! under the Guide. Anywhere but Windows there is no Overlay or strip, nothing can be launched and nothing is
//! frozen, and the same calls say so or do nothing.
//! Turning the machine off is behind a trait, so that only a run that means it can do it, and so is asking a store
//! to install or uninstall a game.
pub mod audio;
pub mod freeze;
mod launch;
pub mod overlay;
pub mod power;
pub mod store;
pub mod toast_strip;

use std::sync::OnceLock;
use std::time::Instant;

pub use audio::{AudioControl, FakeAudio, SystemAudio};
pub use freeze::Freezer;
pub use launch::{Launch, Step};
pub use overlay::{Key, Mode, Overlay};
pub use toast_strip::ToastStrip;

static START: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since the process started: since this was first called, which `main` does first of all.
pub fn since_start() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// wgpu's own warnings and errors, which say why a swap chain could not be made.
struct WgpuLog;

impl log::Log for WgpuLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("wgpu: {} {}: {} at {:.0} ms", record.level(), record.target(), record.args(), since_start());
        }
    }

    fn flush(&self) {}
}

/// Has wgpu's warnings and errors logged with everything else. Call once, before the first window.
pub fn log_wgpu() {
    static LOGGER: WgpuLog = WgpuLog;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// A window, for the log: its handle, process, class and title.
#[cfg(windows)]
unsafe fn describe(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetWindowTextW, GetWindowThreadProcessId};
    if hwnd.is_null() {
        return "none".into();
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let mut class = [0u16; 128];
    let class_len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
    let mut title = [0u16; 128];
    let title_len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
    format!(
        "{hwnd:?} (pid {pid}, class \"{}\", title \"{}\")",
        String::from_utf16_lossy(&class[..class_len]),
        String::from_utf16_lossy(&title[..title_len])
    )
}

/// Takes the foreground from another program's window; whether Windows allowed it. Windows lets a program do that
/// only after input it received, so tap Alt first (xbox-series-theme docs/research/topmost-overlay.md §4, and
/// docs/research/overlay-window.md: without the tap it is refused).
///
/// Never tapped while a window of ours has the foreground, as Home's has when the Guide opens over it: the
/// foreground is then ours to move, and the tap would land in our own window (the Bundle's rule, NativeMethods.cs
/// BringToForeground).
#[cfg(windows)]
unsafe fn take_foreground(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };
    let mut front = 0;
    GetWindowThreadProcessId(GetForegroundWindow(), &mut front);
    if front == std::process::id() {
        return SetForegroundWindow(hwnd) != 0;
    }
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    let taps = [key(0), key(KEYEVENTF_KEYUP)];
    let sent = SendInput(2, taps.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    if sent != 2 {
        eprintln!("shell: SendInput sent {sent} of 2 events at {:.0} ms", since_start());
    }
    SetForegroundWindow(hwnd) != 0
}
