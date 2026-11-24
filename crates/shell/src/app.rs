//! An App started from My games & apps, followed for a moment after its start (docs/spec/apps.md): its window is put
//! in front once it shows. Windows gives a newly started program the foreground only when the program that started
//! it has had the user's input, and the Dashboard's comes from the pad through XInput, which Windows does not count:
//! on the TV PC an App could come up behind Home's window (2026-11-23, the Xbox app and Settings). The App's window
//! is found by what it is: a Store app's by its package family (its frame is ApplicationFrameHost's, the app's own
//! window a child of it), a program's by the program's path.
use std::time::Duration;

/// How long after its start an App's window is looked for.
pub const FOLLOW_FOR: Duration = Duration::from_secs(20);
/// How often it is looked for.
pub const LOOK_EVERY: Duration = Duration::from_millis(150);
/// How long its window stays in front before the App is let go: a splash that hands over to the App's own window
/// is followed to that one.
pub const SETTLED: Duration = Duration::from_millis(1500);

/// What an App's window is known by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Whose {
    /// A packaged app's windows: its package family, the Application User Model ID up to its `!`.
    Package(String),
    /// A program's windows: the program's full path, as the process's image is named, in lower case.
    Program(String),
}

impl Whose {
    /// What the App's windows are known by, from its kind and target (`model::App`).
    pub fn of(app: &model::App) -> Self {
        match app.kind {
            model::AppKind::Store => Self::Package(app.target.split('!').next().unwrap_or(&app.target).to_lowercase()),
            model::AppKind::Program => {
                Self::Program(app.target.trim().trim_matches('"').replace('/', "\\").to_lowercase())
            }
        }
    }

    /// Whether a process of this package family (lower case, empty for none) and image path is the App's.
    pub fn is(&self, family: &str, image: &str) -> bool {
        match self {
            Self::Package(want) => !family.is_empty() && family.eq_ignore_ascii_case(want),
            Self::Program(want) => image.eq_ignore_ascii_case(want),
        }
    }
}

/// An App just started, followed until its window has been in front for [`SETTLED`] or [`FOLLOW_FOR`] has passed.
pub struct Started {
    name: String,
    #[cfg_attr(not(windows), allow(dead_code))]
    whose: Whose,
    since: std::time::Instant,
    #[cfg_attr(not(windows), allow(dead_code))]
    in_front_since: Option<std::time::Instant>,
    /// How many times its window has been put in front: the log says the first and every tenth.
    #[cfg_attr(not(windows), allow(dead_code))]
    tries: u32,
}

impl Started {
    pub fn new(app: &model::App) -> Self {
        Self {
            name: app.name.clone(),
            whose: Whose::of(app),
            since: std::time::Instant::now(),
            in_front_since: None,
            tries: 0,
        }
    }

    /// One look: its window put in front if it has shown and is not. False once it is no longer followed.
    pub fn tick(&mut self) -> bool {
        if self.since.elapsed() >= FOLLOW_FOR {
            eprintln!(
                "apps: {} showed no window in front in {} s: no longer followed",
                self.name,
                FOLLOW_FOR.as_secs()
            );
            return false;
        }
        self.look()
    }

    #[cfg(not(windows))]
    fn look(&mut self) -> bool {
        false
    }

    #[cfg(windows)]
    fn look(&mut self) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let Some(window) = win::window_of(&self.whose) else {
            self.in_front_since = None;
            return true;
        };
        let front = unsafe { GetForegroundWindow() };
        if front == window {
            let since = *self.in_front_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= SETTLED {
                eprintln!("apps: {} in front: {}", self.name, unsafe { crate::describe(window) });
                return false;
            }
            return true;
        }
        self.in_front_since = None;
        let taken = unsafe { win::bring_in_front(window) };
        self.tries += 1;
        if self.tries % 10 != 1 {
            return true;
        }
        eprintln!(
            "apps: {}'s window up at {} ms, not in front ({} is): put in front: {taken} (try {})",
            self.name,
            self.since.elapsed().as_millis(),
            unsafe { crate::describe(front) },
            self.tries
        );
        true
    }
}

/// The App's window highest on the desktop, if one is up; null if none, and anywhere but Windows.
pub fn window(app: &model::App) -> *mut std::ffi::c_void {
    #[cfg(windows)]
    {
        win::window_of(&Whose::of(app)).unwrap_or(std::ptr::null_mut())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        std::ptr::null_mut()
    }
}

/// The Apps started from the Dashboard whose window is up, by their ids (#345): the Guide lists them as running.
pub fn running(apps: &[model::App]) -> Vec<i64> {
    apps.iter().filter(|app| app.last_opened > 0 && !window(app).is_null()).map(|app| app.id).collect()
}

/// Asks the App's windows to close, as their close button does (Quit in its menu in the Guide). How many were asked.
pub fn quit(app: &model::App) -> usize {
    #[cfg(windows)]
    {
        let windows = win::windows_of(&Whose::of(app));
        for &window in &windows {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                    window,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    0,
                    0,
                )
            };
        }
        eprintln!("apps: {} asked to close: {} windows", app.name, windows.len());
        windows.len()
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        0
    }
}

/// Puts the App's window in front, as A on it in the Guide or on its tile asks. Whether it is in front after.
pub fn show(app: &model::App, window: *mut std::ffi::c_void) -> bool {
    #[cfg(windows)]
    unsafe {
        let taken = win::bring_in_front(window as _);
        eprintln!("apps: {} running: put in front: {taken}", app.name);
        taken
    }
    #[cfg(not(windows))]
    {
        let _ = (app, window);
        false
    }
}

/// Home's window in front of whatever is (an App), as Home chosen in the Guide with no game running asks; nothing if
/// it is in front already. Whether it is in front after.
pub fn home_in_front(home: *mut std::ffi::c_void) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let front = GetForegroundWindow();
        if std::ptr::eq(front, home) {
            return true;
        }
        let taken = win::bring_in_front(home as _);
        eprintln!("apps: Home's window in front of {}: {taken}", crate::describe(front));
        taken
    }
    #[cfg(not(windows))]
    {
        let _ = home;
        true
    }
}

#[cfg(windows)]
mod win {
    use super::Whose;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::Storage::Packaging::Appx::GetPackageFamilyName;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    /// The process's package family, lower case ("" for a process of no package), and its image's path, lower case.
    unsafe fn process_of(window: HWND) -> (String, String) {
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return (String::new(), String::new());
        }
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        let family = if GetPackageFamilyName(process, &mut len, name.as_mut_ptr()) == 0 {
            String::from_utf16_lossy(&name[..len.saturating_sub(1) as usize]).to_lowercase()
        } else {
            String::new()
        };
        let mut path = [0u16; 1024];
        let mut len = path.len() as u32;
        let image = if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len) != 0 {
            String::from_utf16_lossy(&path[..len as usize]).to_lowercase()
        } else {
            String::new()
        };
        CloseHandle(process);
        (family, image)
    }

    struct Search<'a> {
        whose: &'a Whose,
        found: HWND,
        /// Every one found, for `windows_of`; the search stops at the first otherwise.
        all: Option<Vec<HWND>>,
        /// For a child: whether it is the App's.
        child: bool,
    }

    unsafe extern "system" fn each_child(child: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let (family, image) = process_of(child);
        if search.whose.is(&family, &image) {
            search.child = true;
            return 0;
        }
        1
    }

    unsafe extern "system" fn each_window(window: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        if IsWindowVisible(window) == 0 || !GetWindow(window, GW_OWNER).is_null() {
            return 1;
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(window, DWMWA_CLOAKED as u32, (&mut cloaked as *mut u32).cast(), 4);
        if !matches!(cloaked, 0 | 2) {
            return 1;
        }
        let (family, image) = process_of(window);
        // A Store app's frame is ApplicationFrameHost's; the app's own window is a child of it.
        let mine = search.whose.is(&family, &image) || {
            search.child = false;
            EnumChildWindows(window, Some(each_child), lparam);
            search.child
        };
        if mine {
            search.found = window;
            if let Some(all) = &mut search.all {
                all.push(window);
                return 1;
            }
            return 0;
        }
        1
    }

    /// The App's window highest on the desktop, if one has shown.
    pub fn window_of(whose: &Whose) -> Option<HWND> {
        let mut search = Search { whose, found: std::ptr::null_mut(), all: None, child: false };
        unsafe { EnumWindows(Some(each_window), &mut search as *mut Search as LPARAM) };
        (!search.found.is_null()).then_some(search.found)
    }

    /// Every window of the App's that is up.
    pub fn windows_of(whose: &Whose) -> Vec<HWND> {
        let mut search = Search { whose, found: std::ptr::null_mut(), all: Some(Vec::new()), child: false };
        unsafe { EnumWindows(Some(each_window), &mut search as *mut Search as LPARAM) };
        search.all.unwrap_or_default()
    }

    /// Puts the window in front: restored if it was minimised, then given the foreground. Whether it has it.
    pub unsafe fn bring_in_front(window: HWND) -> bool {
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_RESTORE);
        }
        crate::take_foreground(window);
        GetForegroundWindow() == window
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{App, AppKind};

    fn app(kind: AppKind, target: &str) -> App {
        App { id: 1, name: "An app".into(), kind, target: target.into(), icon: None, last_opened: 0 }
    }

    #[test]
    fn a_store_apps_windows_are_its_package_familys_whatever_the_program() {
        let whose = Whose::of(&app(AppKind::Store, "Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App"));
        assert_eq!(whose, Whose::Package("microsoft.gamingapp_8wekyb3d8bbwe".into()));
        assert!(whose.is("microsoft.gamingapp_8wekyb3d8bbwe", r"c:\program files\windowsapps\x\xboxpcapp.exe"));
        assert!(!whose.is("", r"c:\windows\system32\applicationframehost.exe"), "the frame's host is no package");
        assert!(!whose.is("microsoft.windowsstore_8wekyb3d8bbwe", ""));
    }

    #[test]
    fn a_programs_windows_are_those_of_its_path_however_it_was_typed() {
        let whose = Whose::of(&app(AppKind::Program, r#""C:/Tools/Thing.exe""#));
        assert_eq!(whose, Whose::Program(r"c:\tools\thing.exe".into()));
        assert!(whose.is("", r"c:\tools\thing.exe"));
        assert!(!whose.is("", r"c:\tools\other.exe"));
    }
}
