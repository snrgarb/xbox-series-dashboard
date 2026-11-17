//! The Dashboard's side of shell mode (docs/spec/shell-mode.md, 6): the two events it sets for the --shell process,
//! its single-instance guard, its log under shell mode, and the test's crash on purpose. Anywhere but Windows each
//! does nothing.
use std::path::Path;

use crate::contract;

/// Home's window's title, which a second Dashboard finds the first's window by.
pub const WINDOW_TITLE: &str = "Xbox Series Dashboard";

/// The test's marker in the data folder's `shell` folder: a number of starts `--from-shell` still to crash right
/// after their first frame (the VM payload's P5). Nothing else writes it.
pub const CRASH_MARKER: &str = "test-crash-after-first-frame";

/// The single-instance guard's name: the plain one for the user's own data folder, one of its own for a test's
/// (`--data-dir`), so a test's Dashboard never gives way to the user's.
pub fn running_mutex(data: Option<&Path>) -> String {
    match data {
        None => contract::RUNNING_MUTEX.to_string(),
        Some(data) => {
            // FNV-1a of the folder as Windows compares it.
            let text = data.display().to_string().to_lowercase();
            let hash =
                text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            format!("{}.{hash:016x}", contract::RUNNING_MUTEX)
        }
    }
}

/// The next start's crash on purpose, if the test's marker asks for one: how many are left after this one. The
/// marker is counted down, and removed at none.
pub fn crash_on_purpose(shell_folder: &Path) -> Option<u32> {
    let marker = shell_folder.join(CRASH_MARKER);
    let count: u32 = std::fs::read_to_string(&marker).ok()?.trim().parse().ok()?;
    if count == 0 {
        let _ = std::fs::remove_file(&marker);
        return None;
    }
    let left = count - 1;
    let _ = if left == 0 { std::fs::remove_file(&marker) } else { std::fs::write(&marker, left.to_string()) };
    Some(left)
}

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub use elsewhere::*;

#[cfg(not(windows))]
mod elsewhere {
    use std::path::Path;

    pub fn mutex_held(_: &str) -> bool {
        false
    }

    pub fn start(exe: &Path, _: &[&str]) -> Result<u32, String> {
        Err(format!("{}: only Windows starts it", exe.display()))
    }

    pub struct Guard;

    pub enum Instance {
        First(Guard),
        Second,
    }

    pub fn single_instance(_: Option<&Path>) -> Instance {
        Instance::First(Guard)
    }

    pub fn bring_first_forward() -> bool {
        false
    }

    pub fn log_to_file(_: &Path) {}

    pub fn attach_parent_console() -> bool {
        false
    }

    pub fn describe_window(_: isize) -> String {
        String::new()
    }

    pub struct Signals;

    impl Signals {
        pub fn open() -> Option<Self> {
            None
        }

        pub fn first_frame(&mut self) -> bool {
            false
        }

        pub fn boot_video_ended(&mut self) -> bool {
            false
        }
    }
}

#[cfg(windows)]
mod win {
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE, FILE_SHARE_DELETE, FILE_SHARE_READ, OPEN_ALWAYS,
    };
    use windows_sys::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};
    use windows_sys::Win32::System::Threading::{
        CreateMutexW, CreateProcessW, OpenEventW, OpenMutexW, SetEvent, EVENT_MODIFY_STATE, PROCESS_INFORMATION,
        STARTUPINFOW, SYNCHRONIZATION_SYNCHRONIZE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    use crate::contract;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// Whether a named mutex is held in this session: the --shell process's, or a running Dashboard's.
    pub fn mutex_held(name: &str) -> bool {
        let name = wide(name);
        unsafe {
            let handle = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
            if handle.is_null() {
                return false;
            }
            CloseHandle(handle);
            true
        }
    }

    /// Starts `exe` with `args`, in its own folder, passing no handles and in no job, so it outlives whoever
    /// started it (a GUI program: no console window); its pid. The process handle is closed: whoever watches finds
    /// it by its exe.
    pub fn start(exe: &Path, args: &[&str]) -> Result<u32, String> {
        let (pid, handle) = start_held(exe, args)?;
        unsafe { CloseHandle(handle) };
        Ok(pid)
    }

    /// As [`start`], keeping the process's handle, which the caller closes.
    pub fn start_held(exe: &Path, args: &[&str]) -> Result<(u32, HANDLE), String> {
        let mut line = format!("\"{}\"", exe.display());
        for arg in args {
            line.push(' ');
            line.push_str(arg);
        }
        let mut line = wide(&line);
        let application = wide(&exe.display().to_string());
        let folder = exe.parent().map(|folder| wide(&folder.display().to_string()));
        unsafe {
            let startup = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..std::mem::zeroed() };
            let mut process: PROCESS_INFORMATION = std::mem::zeroed();
            let ok = CreateProcessW(
                application.as_ptr(),
                line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null(),
                folder.as_ref().map_or(std::ptr::null(), |f| f.as_ptr()),
                &startup,
                &mut process,
            );
            if ok == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            CloseHandle(process.hThread);
            Ok((process.dwProcessId, process.hProcess))
        }
    }

    /// The single-instance guard, held until the process ends.
    pub struct Guard(#[allow(dead_code)] HANDLE);

    pub enum Instance {
        First(Guard),
        /// Another Dashboard with this data folder runs in this session.
        Second,
    }

    pub fn single_instance(data: Option<&Path>) -> Instance {
        let name = wide(&super::running_mutex(data));
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if !handle.is_null() && GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return Instance::Second;
            }
            Instance::First(Guard(handle))
        }
    }

    /// Brings the running Dashboard's window to the front: a second start has the foreground to give.
    pub fn bring_first_forward() -> bool {
        let title = wide(super::WINDOW_TITLE);
        unsafe {
            let window = FindWindowW(std::ptr::null(), title.as_ptr());
            if window.is_null() {
                return false;
            }
            if IsIconic(window) != 0 {
                ShowWindow(window, SW_RESTORE);
            }
            SetForegroundWindow(window) != 0
        }
    }

    /// Everything the Dashboard says goes to `<folder>\dashboard-<stamp>.log` from here on: started by the --shell
    /// process it has no console, and its lines would go nowhere.
    pub fn log_to_file(folder: &Path) {
        let _ = std::fs::create_dir_all(folder);
        let path = folder.join(format!("dashboard-{}.log", crate::setup::stamp()));
        crate::log::prune(folder, "dashboard", crate::log::KEEP - 1);
        let name = wide(&path.display().to_string());
        unsafe {
            let file = CreateFileW(
                name.as_ptr(),
                FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            if !file.is_null() && file as isize != -1 {
                SetStdHandle(STD_ERROR_HANDLE, file);
            }
        }
    }

    /// What a window is, for the log when its swap chain can't be made: its styles, its client size, whether it is
    /// shown, minimised, owned or a child, and the desktop this thread is on (the things DXGI's flip model refuses or
    /// a sign-in can change).
    pub fn describe_window(hwnd: isize) -> String {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::System::StationsAndDesktops::{GetThreadDesktop, GetUserObjectInformationW, UOI_NAME};
        use windows_sys::Win32::System::Threading::GetCurrentThreadId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetClientRect, GetParent, GetWindow, GetWindowLongPtrW, IsIconic, IsWindowVisible, GWL_EXSTYLE, GWL_STYLE,
            GW_OWNER,
        };
        let window = hwnd as *mut core::ffi::c_void;
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
            let ex = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(window, &mut rect);
            let mut name = [0u16; 128];
            let mut needed = 0;
            let desktop = GetThreadDesktop(GetCurrentThreadId());
            let named = GetUserObjectInformationW(
                desktop,
                UOI_NAME,
                name.as_mut_ptr().cast(),
                (name.len() * 2) as u32,
                &mut needed,
            ) != 0;
            let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            format!(
                "window {hwnd:#x}: style {style:#010x}, extended {ex:#010x}{}, client {}x{}, visible {}, minimised {}, \
                 owner {:?}, parent {:?}, desktop {}",
                if ex & 0x0008_0000 != 0 { " (layered)" } else { "" },
                rect.right - rect.left,
                rect.bottom - rect.top,
                IsWindowVisible(window) != 0,
                IsIconic(window) != 0,
                GetWindow(window, GW_OWNER),
                GetParent(window),
                if named { String::from_utf16_lossy(&name[..end]) } else { "unknown".into() }
            )
        }
    }

    /// `dashboard.exe` is a GUI program, so Windows gives it no console: started from Explorer, a shortcut, the Run
    /// key or another program, no console window shows. Run from a terminal, its output and errors, where they
    /// aren't already going somewhere (a file or a pipe, as the tests' are, which stay as they are), go to that
    /// terminal's console, which it attaches to. Whether it attached.
    pub fn attach_parent_console() -> bool {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileType, FILE_GENERIC_READ, FILE_SHARE_WRITE, FILE_TYPE_UNKNOWN, OPEN_EXISTING,
        };
        use windows_sys::Win32::System::Console::{
            AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_HANDLE, STD_OUTPUT_HANDLE,
        };
        unsafe {
            let missing = |which: STD_HANDLE| {
                let handle = GetStdHandle(which);
                handle.is_null() || handle == INVALID_HANDLE_VALUE || GetFileType(handle) == FILE_TYPE_UNKNOWN
            };
            let (output, errors) = (missing(STD_OUTPUT_HANDLE), missing(STD_ERROR_HANDLE));
            if !(output || errors) || AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                return false;
            }
            let name = wide("CONOUT$");
            let console = CreateFileW(
                name.as_ptr(),
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );
            if console.is_null() || console == INVALID_HANDLE_VALUE {
                return false;
            }
            if output {
                SetStdHandle(STD_OUTPUT_HANDLE, console);
            }
            if errors {
                SetStdHandle(STD_ERROR_HANDLE, console);
            }
            true
        }
    }

    /// The two events, opened to be set. None with no --shell process to tell.
    pub struct Signals {
        first_frame: HANDLE,
        ended: HANDLE,
        first_sent: bool,
        ended_sent: bool,
    }

    impl Signals {
        pub fn open() -> Option<Self> {
            let open = |name: &str| {
                let name = wide(name);
                unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) }
            };
            let (first_frame, ended) = (open(contract::FIRST_FRAME), open(contract::BOOT_VIDEO_ENDED));
            if first_frame.is_null() || ended.is_null() {
                return None;
            }
            Some(Self { first_frame, ended, first_sent: false, ended_sent: false })
        }

        /// Sets FirstFrame, once; whether it was set now.
        pub fn first_frame(&mut self) -> bool {
            let first = !std::mem::replace(&mut self.first_sent, true);
            first && unsafe { SetEvent(self.first_frame) } != 0
        }

        /// Sets BootVideoEnded, once; whether it was set now.
        pub fn boot_video_ended(&mut self) -> bool {
            let first = !std::mem::replace(&mut self.ended_sent, true);
            first && unsafe { SetEvent(self.ended) } != 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tests_data_folder_has_a_guard_of_its_own() {
        assert_eq!(running_mutex(None), r"Local\XboxSeriesDashboard.Running");
        let a = running_mutex(Some(Path::new(r"C:\e2e\payload\data")));
        assert!(a.starts_with(r"Local\XboxSeriesDashboard.Running."));
        assert_eq!(a, running_mutex(Some(Path::new(r"C:\E2E\Payload\Data"))), "as Windows compares paths");
        assert_ne!(a, running_mutex(Some(Path::new(r"C:\e2e\other"))));
    }

    #[test]
    fn the_crash_marker_counts_down_and_goes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(crash_on_purpose(dir.path()), None);
        std::fs::write(dir.path().join(CRASH_MARKER), "2").unwrap();
        assert_eq!(crash_on_purpose(dir.path()), Some(1));
        assert_eq!(crash_on_purpose(dir.path()), Some(0));
        assert_eq!(crash_on_purpose(dir.path()), None);
        assert!(!dir.path().join(CRASH_MARKER).exists());
    }
}
