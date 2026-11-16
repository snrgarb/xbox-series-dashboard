//! Asking Steam without its windows, as the Bundle's `SteamDownloads.AskThroughSteamExe` does when Steam's CEF
//! debugger is off (its first route, the client's console over that debugger, is not taken here: it needs the user
//! to have the debugger on). `steam.exe` is given `-silent +app_install <appid>` (or `-silent -console
//! +app_uninstall <appid>`), which hands it to the running client and exits. The running client brings its main
//! window forward as it takes a command if that window is open, but stays in the tray if it is there, so:
//!
//! 1. While Steam is running, its open windows are sent to the tray first (`WM_CLOSE`, as their own close button
//!    does), and the command goes once they have gone, or after two seconds regardless.
//! 2. For fifteen seconds after, should Steam's main window (titled "Steam") come to the front even so, it is sent to
//!    the tray again and the window that was in front as Steam was asked gets the foreground back. Never another of
//!    Steam's windows: one asking the user something (Steam Guard, sign-in) would lose its answer.
//!
//! A Steam that is not running is started by the command, `-silent` keeping its main window down; its sign-in
//! window, which it may need, is left alone. Steam's windows are those of `steam.exe` and `steamwebhelper.exe` under
//! the folder of the `steam.exe` asked, and of no other: the stand-in Steam a test runs from its own folder never
//! touches a real Steam's windows.
//!
//! It all happens on a thread of its own, so the Dashboard's window never waits on it.
use std::path::{Path, PathBuf};

use model::Start;

/// The program `start` runs, when it is a `steam.exe`: what is asked of it goes through [`ask`].
pub fn is_steam(start: &Start) -> Option<PathBuf> {
    let Start::Program { path, .. } = start else { return None };
    (name(path) == "steam.exe").then(|| path.clone())
}

/// A program's file name, in lower case, by either kind of slash.
fn name(path: &Path) -> String {
    let path = path.to_string_lossy().to_lowercase();
    path.rsplit(['\\', '/']).next().unwrap_or_default().to_string()
}

/// Whether a program at `path` is one of the Steam's at `folder`: its `steam.exe` or its `steamwebhelper.exe`,
/// anywhere under it.
pub fn of_steam(folder: &Path, path: &Path) -> bool {
    let name = name(path);
    let (folder, path) = (folder.to_string_lossy().to_lowercase(), path.to_string_lossy().to_lowercase());
    let folder = folder.trim_end_matches(['\\', '/']);
    let under = path.strip_prefix(folder).is_some_and(|rest| rest.starts_with(['\\', '/']));
    under && (name == "steam.exe" || name == "steamwebhelper.exe")
}

#[cfg(not(windows))]
pub fn ask(_start: &Start) -> Result<(), String> {
    Err("only Windows has a Steam to ask".into())
}

#[cfg(windows)]
pub use win::ask;

#[cfg(windows)]
mod win {
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use model::Start;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, CREATE_NO_WINDOW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
        WM_CLOSE,
    };

    use super::of_steam;

    const TRAY_WAIT: Duration = Duration::from_secs(2);
    const TRAY_EVERY: Duration = Duration::from_millis(50);
    const KEEP_IN_FRONT: Duration = Duration::from_secs(15);
    const KEEP_EVERY: Duration = Duration::from_millis(200);
    /// The title of the client's main window, the one it brings forward as it takes a command.
    const MAIN_WINDOW: &str = "Steam";

    /// Asks Steam as the module says, on a thread of its own. Errs only when there is no folder to run it in.
    pub fn ask(start: &Start) -> Result<(), String> {
        let Start::Program { path, args, .. } = start else { return Err("not a program".into()) };
        let folder = start.folder().ok_or("steam.exe has no folder")?;
        let (exe, args) = (path.clone(), args.clone());
        // The window in front as Steam is asked: Home's, which gets the foreground back.
        let home = unsafe { GetForegroundWindow() } as isize;
        std::thread::spawn(move || run(&exe, &args, &folder, home));
        Ok(())
    }

    fn run(exe: &Path, args: &[String], folder: &Path, home: isize) {
        let running = steam_running(folder);
        if running {
            let open = windows(folder, false);
            if !open.is_empty() {
                eprintln!("store: sending Steam's {} open window(s) to the tray before asking it", open.len());
                for &window in &open {
                    unsafe { PostMessageW(window as HWND, WM_CLOSE, 0, 0) };
                }
                let until = Instant::now() + TRAY_WAIT;
                while !windows(folder, false).is_empty() && Instant::now() < until {
                    std::thread::sleep(TRAY_EVERY);
                }
            }
        }
        let started =
            std::process::Command::new(exe).args(args).current_dir(folder).creation_flags(CREATE_NO_WINDOW).spawn();
        if let Err(e) = started {
            eprintln!("store: could not run {} {}: {e}", exe.display(), args.join(" "));
            return;
        }
        if !running {
            return;
        }
        let until = Instant::now() + KEEP_IN_FRONT;
        while Instant::now() < until {
            std::thread::sleep(KEEP_EVERY);
            let front = unsafe { GetForegroundWindow() };
            if front.is_null() || front as isize == home || !windows(folder, true).contains(&(front as isize)) {
                continue;
            }
            if title(front) != MAIN_WINDOW {
                continue;
            }
            eprintln!("store: Steam came to the front as it took the command; sending it back to the tray");
            unsafe {
                PostMessageW(front, WM_CLOSE, 0, 0);
                crate::take_foreground(home as HWND);
            }
        }
    }

    fn title(window: HWND) -> String {
        let mut text = [0u16; 128];
        let len = unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) }.max(0) as usize;
        String::from_utf16_lossy(&text[..len])
    }

    /// The path of a process's program, if it can be asked.
    fn program(pid: u32) -> Option<PathBuf> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut path = [0u16; 1024];
            let mut len = path.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
            CloseHandle(process);
            (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&path[..len as usize])))
        }
    }

    /// Whether the Steam at `folder` is running.
    fn steam_running(folder: &Path) -> bool {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() || snapshot as isize == -1 {
                return false;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = false;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more && !found {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case("steam.exe") {
                    found = program(entry.th32ProcessID).is_some_and(|path| of_steam(folder, &path));
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            found
        }
    }

    struct Search<'a> {
        folder: &'a Path,
        /// Hidden windows too, not only those showing.
        hidden: bool,
        found: Vec<isize>,
    }

    /// The top-level windows of the Steam at `folder`: those showing, or with `hidden` all of them.
    fn windows(folder: &Path, hidden: bool) -> Vec<isize> {
        let mut search = Search { folder, hidden, found: Vec::new() };
        unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
        search.found
    }

    unsafe extern "system" fn each(window: HWND, param: LPARAM) -> i32 {
        let search = &mut *(param as *mut Search);
        if !search.hidden && IsWindowVisible(window) == 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        if pid != 0 && pid != std::process::id() && program(pid).is_some_and(|path| of_steam(search.folder, &path)) {
            search.found.push(window as isize);
        }
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steams_windows_are_those_of_its_own_programs_under_its_folder() {
        let folder = Path::new(r"C:\Program Files (x86)\Steam");
        assert!(of_steam(folder, Path::new(r"C:\Program Files (x86)\Steam\steam.exe")));
        assert!(of_steam(folder, Path::new(r"c:\program files (x86)\steam\bin\cef\cef.win7x64\steamwebhelper.exe")));
        assert!(!of_steam(folder, Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\Moss\moss.exe")));
        assert!(!of_steam(folder, Path::new(r"C:\Program Files (x86)\Steam2\steam.exe")));
        assert!(!of_steam(folder, Path::new(r"C:\Users\Tester\e2e\fakes\steam\steam.exe")));
        let program = |path: &str| Start::Program { path: path.into(), args: vec!["-silent".into()], dir: None };
        assert_eq!(is_steam(&program(r"C:\Steam\Steam.exe")), Some(PathBuf::from(r"C:\Steam\Steam.exe")));
        assert_eq!(is_steam(&program(r"C:\Battle.net\Battle.net.exe")), None);
        assert_eq!(is_steam(&Start::Address("steam://install/1".into())), None);
    }
}
