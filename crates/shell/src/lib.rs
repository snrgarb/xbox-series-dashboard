//! Launching a game through Steam, with Home's window as its Launch screen: above every window and never taking
//! focus, hiding Steam's dialogs and the game's splash until the game's own window has been up a moment. Then the
//! window fades away and hides while the game runs, and Home comes back when it ends.
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since the process started: since this was first called, which `main` does first of all.
pub fn since_start() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// What Home's window is showing during a launch.
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Showing {
    /// The Launch screen, or its fade: keep drawing.
    LaunchScreen,
    /// Hidden behind the running game.
    Nothing,
    /// The Guide over the running game: keep drawing.
    Guide,
    /// The game ended or never started: Home again.
    Home,
}

#[cfg(not(windows))]
pub struct Launch;

#[cfg(not(windows))]
impl Launch {
    pub fn start(_: u32, name: &str, _: &std::path::Path, _: *mut std::ffi::c_void) -> Option<Self> {
        eprintln!("launch: {name} can only be started on Windows");
        None
    }

    pub fn tick(&mut self) -> Showing {
        Showing::Home
    }

    pub fn open_guide(&mut self) {}

    pub fn close_guide(&mut self) {}

    pub fn clip_to_guide(&self, _: u32) {}

    pub fn quit_game(&self) {}
}

#[cfg(windows)]
pub use win::Launch;

#[cfg(windows)]
mod win {
    use super::Showing;
    use std::ffi::c_void;
    use std::path::Path;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::Graphics::Gdi::{CreateRectRgn, SetWindowRgn};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    /// How long the game's window must have been up before the Launch screen leaves: time to draw its first frames.
    const SETTLE: Duration = Duration::from_millis(1500);
    /// A window that isn't the game's and holds the foreground this long needs the user: a EULA, a sign-in, an error.
    const NEEDS_USER: Duration = Duration::from_secs(5);
    const GIVE_UP: Duration = Duration::from_secs(120);
    const FADE: Duration = Duration::from_millis(400);
    /// The game is over once Steam and the desktop have shown nothing of it for this long.
    const GONE: Duration = Duration::from_secs(2);
    /// The game's own window covers at least this much of the screen; a launcher's or a splash doesn't.
    const GAME_COVERS: f64 = 0.6;

    enum Phase {
        Covering,
        Fading(Instant),
        Hidden,
        /// The Guide showing over the running game.
        Guide,
    }

    pub struct Launch {
        appid: u32,
        /// The game's folder, lower case with backslashes and a trailing one.
        dir: String,
        hwnd: HWND,
        phase: Phase,
        started: Instant,
        next_look: Instant,
        game_since: Option<Instant>,
        other: Option<(HWND, Instant)>,
        gone_since: Option<Instant>,
        seen: bool,
        /// The game's largest window when last looked for.
        game: Option<HWND>,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn log(message: &str) {
        eprintln!("launch: {message} at {:.0} ms", crate::since_start());
    }

    impl Launch {
        // An HWND is a handle Windows looks up, never a pointer this code follows.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub fn start(appid: u32, name: &str, dir: &Path, hwnd: HWND) -> Option<Self> {
            if hwnd.is_null() {
                return None;
            }
            let mut dir = dir.to_string_lossy().replace('/', "\\").to_lowercase();
            if !dir.ends_with('\\') {
                dir.push('\\');
            }
            unsafe {
                // Above every window, clicked through, never activated, and layered so it can fade away.
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let overlay = (WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize;
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | overlay);
                SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                // Home has the foreground, and with it the right to let Steam and the game take it.
                AllowSetForegroundWindow(ASFW_ANY);
                let url = wide(&format!("steam://rungameid/{appid}"));
                let result = ShellExecuteW(
                    std::ptr::null_mut(),
                    wide("open").as_ptr(),
                    url.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    SW_SHOWNORMAL,
                );
                if result as isize <= 32 {
                    eprintln!("launch: Steam didn't take steam://rungameid/{appid} ({})", result as isize);
                    restore(hwnd);
                    return None;
                }
            }
            log(&format!("{name} ({appid}) asked of Steam, Launch screen up"));
            let now = Instant::now();
            Some(Self {
                appid,
                dir,
                hwnd,
                phase: Phase::Covering,
                started: now,
                next_look: now,
                game_since: None,
                other: None,
                gone_since: None,
                seen: false,
                game: None,
            })
        }

        /// Called every turn of the loop: looks at the desktop a few times a second and moves the launch on.
        pub fn tick(&mut self) -> Showing {
            let now = Instant::now();
            if let Phase::Fading(from) = self.phase {
                let t = (now - from).as_secs_f32() / FADE.as_secs_f32();
                unsafe {
                    if t < 1.0 {
                        SetLayeredWindowAttributes(self.hwnd, 0, (255.0 * (1.0 - t)) as u8, LWA_ALPHA);
                    } else {
                        ShowWindow(self.hwnd, SW_HIDE);
                        self.phase = Phase::Hidden;
                        log("Launch screen gone");
                    }
                }
                return Showing::LaunchScreen;
            }
            if now < self.next_look {
                return self.showing();
            }
            self.next_look = now + Duration::from_millis(200);
            let look = look(&self.dir, self.hwnd);
            let running = steam_says_running(self.appid);
            self.seen |= running || look.any;
            self.game = look.largest;

            match self.phase {
                Phase::Covering => unsafe {
                    SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    let foreground = GetForegroundWindow();
                    if let Some(game) = look.game {
                        if self.game_since.is_none() {
                            log("the game's window is up");
                        }
                        let since = *self.game_since.get_or_insert(now);
                        if foreground != game {
                            SetForegroundWindow(game);
                        }
                        if now - since >= SETTLE {
                            log("the game has booted, Launch screen fading");
                            self.phase = Phase::Fading(now);
                        }
                    } else {
                        self.game_since = None;
                        AllowSetForegroundWindow(ASFW_ANY);
                        // A window that keeps the foreground without being the game wants the user.
                        if foreground.is_null() || foreground == self.hwnd || IsWindowVisible(foreground) == 0 {
                            self.other = None;
                        } else if self.other.map(|o| o.0) != Some(foreground) {
                            self.other = Some((foreground, now));
                        } else if self.other.is_some_and(|o| now - o.1 >= NEEDS_USER) {
                            log("a window wants the user, Launch screen stepping aside");
                            self.phase = Phase::Fading(now);
                        }
                        if now - self.started >= GIVE_UP {
                            if self.seen {
                                log("no game window after two minutes, Launch screen stepping aside");
                                self.phase = Phase::Fading(now);
                            } else {
                                log("the game never started");
                                restore(self.hwnd);
                                return Showing::Home;
                            }
                        }
                    }
                },
                Phase::Hidden | Phase::Guide => {
                    if running || look.any {
                        self.gone_since = None;
                    } else if now - *self.gone_since.get_or_insert(now) >= GONE {
                        log("the game has ended, Home back");
                        unsafe { restore(self.hwnd) };
                        return Showing::Home;
                    }
                }
                Phase::Fading(_) => {}
            }
            self.showing()
        }

        fn showing(&self) -> Showing {
            match self.phase {
                Phase::Hidden => Showing::Nothing,
                Phase::Guide => Showing::Guide,
                _ => Showing::LaunchScreen,
            }
        }

        /// Shows Home's window over the running game, for the Guide: in front and with the keyboard, as the Launch
        /// screen never is. `clip_to_guide` keeps all of it but the Guide's panel from covering the game.
        pub fn open_guide(&mut self) {
            if !matches!(self.phase, Phase::Hidden) {
                return;
            }
            unsafe {
                let ex = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex & !((WS_EX_NOACTIVATE | WS_EX_TRANSPARENT) as isize));
                SetLayeredWindowAttributes(self.hwnd, 0, 255, LWA_ALPHA);
                self.clip_to_guide(1);
                SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                take_foreground(self.hwnd);
            }
            self.phase = Phase::Guide;
            log("Guide over the game");
        }

        /// Cuts the window down to its left `width` pixels: the Guide's panel, the game showing beside it.
        pub fn clip_to_guide(&self, width: u32) {
            unsafe {
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(self.hwnd, &mut rect);
                // The window owns the region once it is set.
                SetWindowRgn(self.hwnd, CreateRectRgn(0, 0, width.max(1) as i32, rect.bottom), 1);
            }
        }

        /// Hides the window again and gives the game the foreground back.
        pub fn close_guide(&mut self) {
            if !matches!(self.phase, Phase::Guide) {
                return;
            }
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
                if let Some(game) = self.game {
                    SetForegroundWindow(game);
                }
            }
            self.phase = Phase::Hidden;
            log("Guide closed, back to the game");
        }

        /// Asks the game to close, as its window's close button would.
        pub fn quit_game(&self) {
            if let Some(game) = self.game {
                unsafe { PostMessageW(game, WM_CLOSE, 0, 0) };
                log("asked the game to quit");
            }
        }
    }

    /// Takes the foreground from another program's window. Windows lets a program do that only after input it
    /// received, so tap Alt first (docs/research/topmost-overlay.md).
    unsafe fn take_foreground(hwnd: HWND) {
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let taps = [key(0), key(KEYEVENTF_KEYUP)];
        SendInput(2, taps.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        SetForegroundWindow(hwnd);
    }

    /// Home's window as it was: opaque, an ordinary window, shown and in front.
    unsafe fn restore(hwnd: HWND) {
        SetWindowRgn(hwnd, std::ptr::null_mut(), 1);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex & !((WS_EX_NOACTIVATE | WS_EX_TRANSPARENT) as isize));
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
        take_foreground(hwnd);
    }

    /// Steam's own word on whether the game runs: HKCU\Software\Valve\Steam\Apps\<appid>\Running.
    fn steam_says_running(appid: u32) -> bool {
        let key = wide(&format!("Software\\Valve\\Steam\\Apps\\{appid}"));
        let mut value = 0u32;
        let mut size = 4u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                wide("Running").as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                &mut value as *mut u32 as *mut c_void,
                &mut size,
            )
        };
        status == 0 && value != 0
    }

    #[derive(Default)]
    struct Look {
        /// A window of the game's that covers the screen.
        game: Option<HWND>,
        /// Any visible window of a program in the game's folder.
        any: bool,
        /// The largest of them.
        largest: Option<HWND>,
    }

    struct Search<'a> {
        dir: &'a str,
        screen: f64,
        best: f64,
        largest: f64,
        look: Look,
    }

    /// The visible windows of programs that live in the game's folder.
    fn look(dir: &str, own: HWND) -> Look {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(own, &mut rect) };
        let screen = ((rect.right - rect.left) as f64 * (rect.bottom - rect.top) as f64).max(1.0);
        let mut search = Search { dir, screen, best: 0.0, largest: 0.0, look: Look::default() };
        unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
        search.look
    }

    unsafe extern "system" fn each(hwnd: HWND, param: LPARAM) -> i32 {
        let search = &mut *(param as *mut Search);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut c_void, 4);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if cloaked != 0 || pid == 0 || pid == std::process::id() {
            return 1;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return 1;
        }
        let mut path = [0u16; 1024];
        let mut len = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 || !String::from_utf16_lossy(&path[..len as usize]).to_lowercase().starts_with(search.dir) {
            return 1;
        }
        search.look.any = true;
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let area = (rect.right - rect.left).max(0) as f64 * (rect.bottom - rect.top).max(0) as f64;
        if area > search.largest {
            search.largest = area;
            search.look.largest = Some(hwnd);
        }
        if area >= search.screen * GAME_COVERS && area > search.best {
            search.best = area;
            search.look.game = Some(hwnd);
        }
        1
    }
}
