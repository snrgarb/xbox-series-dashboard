//! Windows itself: the Overlay, the window above every other that the Launch screen and the Guide are drawn on,
//! the Toast strip above it, launching a game through its source and following it until it ends, and holding it still
//! under the Guide. Anywhere but Windows there is no Overlay or strip, nothing can be launched and nothing is
//! frozen, and the same calls say so or do nothing.
//! Turning the machine off is behind a trait, so that only a run that means it can do it, and so is asking a store
//! to install or uninstall a game.
pub mod account;
pub mod audio;
pub mod clipboard;
pub mod freeze;
pub mod idle;
mod launch;
pub mod media;
pub mod overlay;
pub mod power;
pub mod steam;
pub mod store;
pub mod toast_strip;
pub mod wake;

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

/// Set when another program asks the Dashboard to close through the Overlay's or the Toast strip's window.
static CLOSE_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether another program has asked the Dashboard to close since this was last called: a `WM_CLOSE` posted to the
/// Overlay's or the Toast strip's window, as `taskkill` without `/f` posts one to every top-level window of the
/// process. Home's own window is asked as any window is (winit's `CloseRequested`). Alt+F4 is not such a request:
/// those windows never take it.
pub fn close_asked() -> bool {
    CLOSE_ASKED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Says that `window` was asked to close, and has the Dashboard close.
#[cfg_attr(not(windows), allow(dead_code))]
fn ask_to_close(window: &str) {
    eprintln!("window: asked to close (WM_CLOSE to {window}) at {:.0} ms", since_start());
    CLOSE_ASKED.store(true, std::sync::atomic::Ordering::Relaxed);
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

/// Opens what a source gave as Windows opens it: an address with the program registered for it, a program with
/// its command line in its folder. Home has the foreground, and with it the right to let what opens take it, which
/// it gives. Gives ShellExecute's answer, which is over 32 when it worked.
#[cfg(windows)]
fn shell_open(start: &model::Start) -> isize {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY, SW_SHOWNORMAL};
    let (verb, file) = (wide("open"), wide(&start.target()));
    let parameters = start.parameters().map(|parameters| wide(&parameters));
    let folder = start.folder().map(|folder| wide(&folder.to_string_lossy()));
    let or_null = |text: &Option<Vec<u16>>| text.as_ref().map_or(std::ptr::null(), |text| text.as_ptr());
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            or_null(&parameters),
            or_null(&folder),
            SW_SHOWNORMAL,
        );
        result as isize
    }
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
