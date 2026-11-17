//! Asking Steam to install or uninstall a game without any of its windows being seen (#132), by the Bundle's two
//! routes (its `SteamDownloads.AskSteamApp`), the first that can be taken:
//!
//! 1. **Its console, over its CEF debugger** (`source_steam::cef`), where the user has the debugger on and Steam
//!    is running: `app_install <appid>` typed into the client's console from its windowless page. Nothing of
//!    Steam's is raised and nothing takes the foreground. It is the user's to turn on (an empty
//!    `.cef-enable-remote-debugging` in Steam's folder, then Steam started again), never the Dashboard's: while it
//!    is on, any program on the PC can drive Steam.
//! 2. **`steam.exe`**, given `-silent +app_install <appid>` (or `-silent -console +app_uninstall <appid>`), which
//!    hands the command to the running client, or starts Steam with it. Seen on the real machine (2026-11-16), the
//!    running client brings its main window up as it takes a command handed over this way, from the tray as well
//!    as open, whatever `-silent` says: a second `steam.exe` is taken as the user starting Steam again, which shows
//!    its window (reasoned, from the log: the window came whichever way Steam was left; Valve does not say). So
//!    this route keeps it from being seen rather than from coming:
//!    - Steam's open windows are sent to the tray first (`WM_CLOSE`, as their own close button does), as the
//!      Bundle does.
//!    - A watch is put on Steam's windows before the command goes, by Windows' own events (`SetWinEventHook`, out
//!      of context: a window shown, given the foreground, or renamed), not by looking now and then, so a window is
//!      met within moments of its showing and not up to 200 ms after as before. Steam's main window, titled
//!      "Steam", is hidden there and then (`ShowWindowAsync`, `SW_HIDE`), and closed to the tray as its own close
//!      button does, so Steam knows it is there. Cloaking it first (`DWMWA_CLOAK`), which would take it off the
//!      screen at once, is tried too: Windows is not known to allow it on another program's window, and the log
//!      says what it answered. Never another of Steam's windows: one asking the user something (Steam Guard,
//!      sign-in) would lose its answer.
//!    - While Steam is running, for the first seconds the foreground is locked (`LockSetForegroundWindow`), so no
//!      other program, Steam among them, can take it from the Dashboard as it takes the command.
//!
//!    Whether Steam's window is then never seen depends on how soon Windows tells of it against how soon the
//!    screen is next drawn, which only the real machine can say. Every window of Steam's met in the watch is said
//!    in the log, with its class, title, when and what was done to it, so one install there tells.
//!
//! A Steam that is not running is started by `steam.exe`, `-silent` keeping its main window down; its sign-in
//! window, which it may need, is left alone, and the foreground is not locked. Steam's windows are those of
//! `steam.exe` and `steamwebhelper.exe` under the folder of the `steam.exe` asked, and of no other: the stand-in
//! Steam a test runs from its own folder never touches a real Steam's windows, nor its debugger.
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
    source_steam::under(folder, path) && (name == "steam.exe" || name == "steamwebhelper.exe")
}

/// The title of Steam's main window: the one it brings up as it takes a command, and the only one hidden.
pub const MAIN_WINDOW: &str = "Steam";

/// What the watch does to a window of Steam's it meets: Steam's main window, showing, is hidden; any other is left.
pub fn hides(title: &str, visible: bool) -> bool {
    visible && title == MAIN_WINDOW
}

#[cfg(not(windows))]
pub fn ask(_start: &Start) -> Result<(), String> {
    Err("only Windows has a Steam to ask".into())
}

#[cfg(windows)]
pub use win::ask;

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use model::Start;
    use source_steam::cef::{self, Evaluated, Failed};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, CREATE_NO_WINDOW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindowVisible, KillTimer, LockSetForegroundWindow, PostMessageW, SetTimer,
        ShowWindowAsync, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, GA_ROOT, LSFW_LOCK,
        LSFW_UNLOCK, MSG, SW_HIDE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_CLOSE, WM_TIMER,
    };

    use super::{hides, of_steam};

    const TRAY_WAIT: Duration = Duration::from_secs(2);
    const TRAY_EVERY: Duration = Duration::from_millis(50);
    /// How long Steam's windows are watched once a running Steam is asked: it takes the command within a second
    /// or two.
    const WATCH_RUNNING: Duration = Duration::from_secs(15);
    /// How long once `steam.exe` starts a Steam that was not running: it updates and signs in first.
    const WATCH_STARTING: Duration = Duration::from_secs(45);
    /// How long the foreground is locked as a running Steam takes the command. Short: while it is locked the
    /// Dashboard cannot hand the foreground to a game it starts either.
    const LOCK_FOR: Duration = Duration::from_secs(5);
    const OBJID_WINDOW: i32 = 0;
    const CHILDID_SELF: i32 = 0;

    fn log(text: &str) {
        eprintln!("store: {text} at {:.0} ms", crate::since_start());
    }

    /// Asks Steam as the module says, on a thread of its own. Errs only when there is no folder to run it in.
    pub fn ask(start: &Start) -> Result<(), String> {
        let Start::Program { path, args, .. } = start else { return Err("not a program".into()) };
        let folder = start.folder().ok_or("steam.exe has no folder")?;
        let (exe, args) = (path.clone(), args.clone());
        // The window in front as Steam is asked: Home's, which keeps the foreground or gets it back.
        let home = unsafe { GetForegroundWindow() } as isize;
        std::thread::spawn(move || run(&exe, &args, &folder, home));
        Ok(())
    }

    fn run(exe: &Path, args: &[String], folder: &Path, home: isize) {
        let running = steam_running(folder);
        let command = cef::console_command(args).unwrap_or_else(|| args.join(" "));
        if running.is_some() && cef::enabled(folder) {
            // A real Steam's ports, or the stand-in's own (never the other's).
            match cef::evaluate(&cef::ports(folder), cef::SHARED, &cef::console_script(&command)) {
                Ok(Evaluated::Value(value)) if value == "asked" => {
                    log(&format!("asked Steam through its console over its CEF debugger: {command}; no window raised"));
                    return;
                }
                // Sent, so Steam may well be at it: asking again through steam.exe would raise the window this
                // route is for, for a command that has likely taken.
                Err(Failed::Lost(why)) => {
                    log(&format!(
                        "Steam's console over CEF was lost after {command} was sent ({why}); not asked again"
                    ));
                    return;
                }
                Ok(other) => log(&format!(
                    "Steam's console over CEF answered {other:?} to {command}, so it goes through steam.exe"
                )),
                Err(Failed::NotSent(why)) => log(&format!(
                    "Steam's CEF debugger could not be reached ({why}), so {command} goes through steam.exe"
                )),
            }
        } else if let Some(pid) = running {
            log(&format!(
                "Steam is running (pid {pid}) with its CEF debugger off (no {} in {}), so {command} goes through \
                 steam.exe, its windows watched",
                cef::FILE,
                folder.display()
            ));
        } else {
            log(&format!("Steam is not running, so steam.exe starts it with {command}, its windows watched"));
        }

        if running.is_some() {
            let open = windows(folder);
            if !open.is_empty() {
                log(&format!("sending Steam's {} open window(s) to the tray before asking it", open.len()));
                for &(window, ref title) in &open {
                    log(&format!("  {} titled {title:?}", hex(window)));
                    unsafe { PostMessageW(window as HWND, WM_CLOSE, 0, 0) };
                }
                let until = Instant::now() + TRAY_WAIT;
                while !windows(folder).is_empty() && Instant::now() < until {
                    std::thread::sleep(TRAY_WAIT.min(TRAY_EVERY));
                }
            }
        }

        let watch_for = if running.is_some() { WATCH_RUNNING } else { WATCH_STARTING };
        let watch = Watch::start(folder.to_path_buf(), home, watch_for);
        let locked = running.is_some() && unsafe { LockSetForegroundWindow(LSFW_LOCK) } != 0;
        if running.is_some() {
            log(&format!(
                "the foreground {} as Steam takes the command",
                if locked { "locked" } else { "could not be locked" }
            ));
        }
        let started =
            std::process::Command::new(exe).args(args).current_dir(folder).creation_flags(CREATE_NO_WINDOW).spawn();
        match started {
            Ok(_) => log(&format!("ran {} {}", exe.display(), args.join(" "))),
            Err(e) => log(&format!("could not run {} {}: {e}", exe.display(), args.join(" "))),
        }
        if locked {
            std::thread::sleep(LOCK_FOR);
            unsafe { LockSetForegroundWindow(LSFW_UNLOCK) };
            log("the foreground unlocked");
        }
        if let Some(watch) = watch {
            let _ = watch.join();
        }
    }

    fn hex(window: isize) -> String {
        format!("{window:#x}")
    }

    fn title(window: HWND) -> String {
        let mut text = [0u16; 256];
        let len = unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) }.max(0) as usize;
        String::from_utf16_lossy(&text[..len])
    }

    fn class(window: HWND) -> String {
        let mut text = [0u16; 256];
        let len = unsafe { GetClassNameW(window, text.as_mut_ptr(), text.len() as i32) }.max(0) as usize;
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

    /// The Steam client at `folder`'s process, if it is running.
    fn steam_running(folder: &Path) -> Option<u32> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() || snapshot as isize == -1 {
                return None;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = None;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more && found.is_none() {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case("steam.exe")
                    && program(entry.th32ProcessID).is_some_and(|path| of_steam(folder, &path))
                {
                    found = Some(entry.th32ProcessID);
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            found
        }
    }

    struct Search<'a> {
        folder: &'a Path,
        found: Vec<(isize, String)>,
    }

    /// The showing top-level windows of the Steam at `folder`, with their titles.
    fn windows(folder: &Path) -> Vec<(isize, String)> {
        let mut search = Search { folder, found: Vec::new() };
        unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
        search.found
    }

    unsafe extern "system" fn each(window: HWND, param: LPARAM) -> i32 {
        let search = &mut *(param as *mut Search);
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        if pid != 0 && pid != std::process::id() && program(pid).is_some_and(|path| of_steam(search.folder, &path)) {
            search.found.push((window as isize, title(window)));
        }
        1
    }

    /// The watch on Steam's windows: what its thread keeps between Windows' events.
    struct Watch {
        folder: PathBuf,
        home: isize,
        since: Instant,
        /// Each process met, and whether it is one of this Steam's, by its program.
        processes: HashMap<u32, Option<String>>,
        /// Each window of Steam's met, by its title when last said.
        said: HashMap<isize, String>,
        shown: usize,
        hidden: Vec<isize>,
        cloaked: Vec<isize>,
    }

    thread_local! {
        static WATCHING: RefCell<Option<Watch>> = const { RefCell::new(None) };
    }

    impl Watch {
        /// Watches Steam's windows for `seconds` on a thread of its own, once its hooks are in: returned only then,
        /// so the command goes after. None if the hooks could not be put in, which is said.
        fn start(folder: PathBuf, home: isize, watch_for: Duration) -> Option<std::thread::JoinHandle<()>> {
            let (ready, hooked) = std::sync::mpsc::channel();
            let thread = std::thread::spawn(move || {
                let watch = Watch {
                    folder,
                    home,
                    since: Instant::now(),
                    processes: HashMap::new(),
                    said: HashMap::new(),
                    shown: 0,
                    hidden: Vec::new(),
                    cloaked: Vec::new(),
                };
                WATCHING.with(|watching| *watching.borrow_mut() = Some(watch));
                let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
                let hook = |event| unsafe {
                    SetWinEventHook(event, event, std::ptr::null_mut(), Some(event_seen), 0, 0, flags)
                };
                let hooks: Vec<HWINEVENTHOOK> = [EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_NAMECHANGE]
                    .into_iter()
                    .map(hook)
                    .collect();
                let all = hooks.iter().all(|hook| !hook.is_null());
                let _ = ready.send(all);
                if all {
                    // Out-of-context events come in while the thread waits for messages; the timer ends the watch.
                    let timer = unsafe { SetTimer(std::ptr::null_mut(), 0, watch_for.as_millis() as u32, None) };
                    let mut msg: MSG = unsafe { std::mem::zeroed() };
                    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                        if msg.message == WM_TIMER {
                            break;
                        }
                    }
                    unsafe { KillTimer(std::ptr::null_mut(), timer) };
                }
                for hook in hooks.into_iter().filter(|hook| !hook.is_null()) {
                    unsafe { UnhookWinEvent(hook) };
                }
                if let Some(watch) = WATCHING.with(|watching| watching.borrow_mut().take()) {
                    watch.end(watch_for, all);
                }
            });
            match hooked.recv_timeout(Duration::from_secs(1)) {
                Ok(true) => {
                    log(&format!("watching Steam's windows for {} s", watch_for.as_secs()));
                    Some(thread)
                }
                _ => {
                    log("could not watch Steam's windows: Windows took no hook");
                    None
                }
            }
        }

        /// Whether a process is this Steam's, and its program's name, remembered.
        fn steams(&mut self, pid: u32) -> Option<String> {
            let folder = &self.folder;
            self.processes
                .entry(pid)
                .or_insert_with(|| {
                    let path = program(pid)?;
                    of_steam(folder, &path).then(|| super::name(&path))
                })
                .clone()
        }

        fn seen(&mut self, event: u32, window: HWND) {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(window, &mut pid) };
            let Some(program) = self.steams(pid) else { return };
            let key = window as isize;
            let (title, visible) = (title(window), unsafe { IsWindowVisible(window) } != 0);
            let what = match event {
                EVENT_OBJECT_SHOW => "shown",
                EVENT_SYSTEM_FOREGROUND => "took the foreground",
                _ => "renamed",
            };
            // A rename is said only of a window showing whose title is new to the watch.
            if event == EVENT_OBJECT_NAMECHANGE && (!visible || self.said.get(&key) == Some(&title)) {
                return;
            }
            if event == EVENT_OBJECT_SHOW {
                self.shown += 1;
            }
            self.said.insert(key, title.clone());
            let done = if hides(&title, visible) {
                // Off the screen first, where Windows allows; then hidden, then closed as its own button closes it.
                let on = 1i32;
                let cloak = unsafe { DwmSetWindowAttribute(window, DWMWA_CLOAK as u32, (&on as *const i32).cast(), 4) };
                if cloak == 0 && !self.cloaked.contains(&key) {
                    self.cloaked.push(key);
                }
                unsafe {
                    ShowWindowAsync(window, SW_HIDE);
                    PostMessageW(window, WM_CLOSE, 0, 0);
                }
                if !self.hidden.contains(&key) {
                    self.hidden.push(key);
                }
                let mut done = format!(
                    "hidden at once (cloak {}), and sent to the tray",
                    if cloak == 0 { "taken".to_string() } else { format!("refused, {cloak:#010x}") }
                );
                if unsafe { GetForegroundWindow() } == window {
                    let back = unsafe { crate::take_foreground(self.home as HWND) };
                    done.push_str(if back {
                        "; the foreground back to Home"
                    } else {
                        "; Home could not get the foreground back"
                    });
                }
                done
            } else if title == super::MAIN_WINDOW {
                "left: not showing".into()
            } else {
                "left alone: not Steam's main window".into()
            };
            let at = self.since.elapsed().as_millis();
            log(&format!(
                "Steam window {} {what} {at} ms into the watch ({program}, class {:?}, title {title:?}, {}): {done}",
                hex(key),
                class(window),
                if visible { "showing" } else { "not showing" },
            ));
        }

        /// The watch is over: any window cloaked is uncloaked, so Steam's own window shows when the user opens it,
        /// and what was met is summed up.
        fn end(self, watched: Duration, hooked: bool) {
            let off = 0i32;
            for &window in &self.cloaked {
                unsafe { DwmSetWindowAttribute(window as HWND, DWMWA_CLOAK as u32, (&off as *const i32).cast(), 4) };
            }
            if !hooked {
                return;
            }
            let front = unsafe { GetForegroundWindow() } as isize;
            let still: Vec<String> = self
                .hidden
                .iter()
                .filter(|&&window| unsafe { IsWindowVisible(window as HWND) } != 0)
                .map(|&window| hex(window))
                .collect();
            log(&format!(
                "watched Steam's windows for {} s: {} shown, {} hidden{}; the foreground {}",
                watched.as_secs(),
                self.shown,
                self.hidden.len(),
                if still.is_empty() { String::new() } else { format!(", showing again: {}", still.join(", ")) },
                if front == self.home { "Home's".to_string() } else { format!("{}'s", hex(front)) },
            ));
        }
    }

    /// Windows tells of a window shown, given the foreground, or renamed: only top-level windows go to the watch.
    unsafe extern "system" fn event_seen(
        _hook: HWINEVENTHOOK,
        event: u32,
        window: HWND,
        object: i32,
        child: i32,
        _thread: u32,
        _time: u32,
    ) {
        if window.is_null() || object != OBJID_WINDOW || child != CHILDID_SELF || GetAncestor(window, GA_ROOT) != window
        {
            return;
        }
        WATCHING.with(|watching| {
            if let Ok(mut watching) = watching.try_borrow_mut() {
                if let Some(watch) = watching.as_mut() {
                    watch.seen(event, window);
                }
            }
        });
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

    #[test]
    fn only_steams_main_window_is_hidden_and_only_while_it_shows() {
        assert!(hides("Steam", true));
        assert!(!hides("Steam", false));
        // Sign-in, Steam Guard, an install dialog, the friends list: each asks or shows the user something.
        for title in [
            "Sign in to Steam",
            "Steam Guard - Computer Authorization Required",
            "Install - Lantern Gate 2",
            "Friends List",
            "",
        ] {
            assert!(!hides(title, true), "{title}");
        }
    }
}
