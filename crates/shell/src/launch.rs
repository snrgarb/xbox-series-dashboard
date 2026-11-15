//! Launching a game through Steam and following it: asked of Steam, its window up, booted, ended. While the game
//! starts the Launch screen is over everything, on the Overlay; this says when it should go, hides Home's window
//! behind the game, and brings Home back when the game ends. Home's window is otherwise left as it is: it is never
//! made topmost, layered or clipped.
use std::ffi::c_void;
use std::path::Path;
use std::time::Duration;

use crate::Overlay;

/// What a look at the launch found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing new.
    Same,
    /// The Launch screen should go: the game has booted, a window wants the user, or no window of the game's came.
    LaunchScreenGoes,
    /// The game ended, or never started: Home's window is back and in front, and the launch is over.
    Ended,
}

#[cfg(not(windows))]
pub struct Launch(());

#[cfg(not(windows))]
impl Launch {
    pub fn start(_: u32, name: &str, _: &Path, _: *mut c_void, _: bool) -> Option<Self> {
        eprintln!("launch: {name} can only be started on Windows");
        None
    }

    pub fn tick(&mut self, _: Option<&Overlay>) -> Step {
        Step::Ended
    }

    pub fn wait(&self) -> Duration {
        Duration::ZERO
    }

    pub fn launch_screen_gone(&mut self) {}

    pub fn guide(&mut self, _: bool) {}

    pub fn quit_game(&self) {}
}

#[cfg(windows)]
pub use win::Launch;

#[cfg(windows)]
mod win {
    use super::{c_void, Duration, Overlay, Path, Step};
    use crate::{take_foreground, wide};
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    /// How often the desktop is looked at.
    const LOOK_EVERY: Duration = Duration::from_millis(200);
    /// How long the game's window must have been up before the Launch screen leaves: time to draw its first frames.
    const SETTLE: Duration = Duration::from_millis(1500);
    /// A window that isn't the game's and holds the foreground this long needs the user: a EULA, a sign-in, an error.
    /// The Bundle's rule and its time (ExtensionCore.LaunchScreen.cs), which makes nothing of whose window it is: a
    /// launcher's or a splash that stays this long steps the Launch screen aside too.
    /// `DASHBOARD_NEEDS_USER_MS` is another time, for a test.
    const NEEDS_USER: Duration = Duration::from_secs(5);
    /// A look this long after the one before came late: the machine is not keeping up.
    const LATE: Duration = Duration::from_millis(500);
    const GIVE_UP: Duration = Duration::from_secs(120);
    /// The game is over once Steam and the desktop have shown nothing of it for this long.
    const GONE: Duration = Duration::from_secs(2);
    /// The game's own window covers at least this much of the screen; a launcher's or a splash doesn't.
    const GAME_COVERS: f64 = 0.6;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum State {
        /// Asked of Steam. The Launch screen is over whatever Steam and the game put up; Home's window is under
        /// it all, as it was.
        Starting,
        /// The Launch screen is fading away, Home's window hidden: what shows through is the game, or the window
        /// that wants the user.
        Revealing,
        /// The Launch screen has gone. The game runs, with the Guide over it or not.
        Running,
    }

    pub struct Launch {
        appid: u32,
        /// The game's folder, lower case with backslashes and a trailing one.
        dir: String,
        /// Home's window.
        home: HWND,
        state: State,
        started: Instant,
        next_look: Instant,
        /// Since when the game has had a window that covers the screen.
        game_since: Option<Instant>,
        /// A window that isn't the game's and has the foreground, and how long it has been seen to hold it.
        other: Option<(HWND, Duration)>,
        /// How long such a window holds the foreground before it counts as needing the user.
        needs_user: Duration,
        /// When the desktop was last looked at.
        looked: Instant,
        /// Whether the game's window has been said to be up.
        said_up: bool,
        /// Since when nothing of the game has shown.
        gone_since: Option<Instant>,
        /// Whether Steam or the desktop has shown anything of the game yet.
        seen: bool,
        /// The game's largest window when last looked for.
        game: Option<HWND>,
        /// Whether the Guide is open over the game.
        guide: bool,
    }

    fn log(message: &str) {
        eprintln!("launch: {message} at {:.0} ms", crate::since_start());
    }

    impl Launch {
        /// Asks Steam for the game. `home` is Home's window, which has the foreground; `launch_screen` is whether
        /// there is an Overlay to show the Launch screen on. None, having said why, when Steam can't be asked.
        // An HWND is a handle Windows looks up, never a pointer this code follows.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub fn start(appid: u32, name: &str, dir: &Path, home: *mut c_void, launch_screen: bool) -> Option<Self> {
            if home.is_null() {
                return None;
            }
            let mut dir = dir.to_string_lossy().replace('/', "\\").to_lowercase();
            if !dir.ends_with('\\') {
                dir.push('\\');
            }
            unsafe {
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
                    return None;
                }
            }
            let screen = if launch_screen { "Launch screen up" } else { "no Overlay, so no Launch screen" };
            log(&format!("{name} ({appid}) asked of Steam, {screen}"));
            let now = Instant::now();
            Some(Self {
                appid,
                dir,
                home,
                state: State::Starting,
                started: now,
                next_look: now,
                game_since: None,
                other: None,
                needs_user: needs_user(),
                looked: now,
                said_up: false,
                gone_since: None,
                seen: false,
                game: None,
                guide: false,
            })
        }

        /// How long until the desktop is next looked at: there is no call to `tick` sooner.
        pub fn wait(&self) -> Duration {
            self.next_look.saturating_duration_since(Instant::now())
        }

        /// Called every turn of the loop: looks at the desktop a few times a second and moves the launch on.
        /// `overlay` is kept above everything while the game starts.
        pub fn tick(&mut self, overlay: Option<&Overlay>) -> Step {
            let now = Instant::now();
            if now < self.next_look {
                return Step::Same;
            }
            self.next_look = now + LOOK_EVERY;
            let since_last = now - std::mem::replace(&mut self.looked, now);
            let look = look(&self.dir, self.home);
            let running = steam_says_running(self.appid);
            self.seen |= running || look.any;
            self.game = look.largest;

            match self.state {
                State::Starting => self.starting(now, since_last, &look, overlay),
                State::Revealing | State::Running => {
                    // After a window that wanted the user, or after giving up: the game's window, when it comes.
                    if look.game.is_some() && !self.said_up {
                        self.said_up = true;
                        log("the game's window is up");
                    }
                    if running || look.any {
                        self.gone_since = None;
                    } else if now - *self.gone_since.get_or_insert(now) >= GONE {
                        log("the game has ended, Home back");
                        unsafe { restore(self.home) };
                        return Step::Ended;
                    }
                    Step::Same
                }
            }
        }

        /// One look while the game starts: its window up and settled, a window that wants the user, or time up.
        fn starting(&mut self, now: Instant, since_last: Duration, look: &Look, overlay: Option<&Overlay>) -> Step {
            // Above a launcher or a splash that made itself topmost since the last look.
            if let Some(overlay) = overlay {
                overlay.raise();
            }
            let foreground = unsafe { GetForegroundWindow() };
            if let Some(game) = look.game {
                if self.game_since.is_none() {
                    self.said_up = true;
                    log("the game's window is up");
                }
                let since = *self.game_since.get_or_insert(now);
                if foreground != game {
                    unsafe { SetForegroundWindow(game) };
                }
                if now - since >= SETTLE {
                    log("the game has booted, Launch screen fading");
                    return self.reveal();
                }
                return Step::Same;
            }
            self.game_since = None;
            unsafe { AllowSetForegroundWindow(ASFW_ANY) };
            // A window that keeps the foreground without being the game wants the user. How long it has kept it is
            // counted in looks that came on time, a fifth of a second each, and not by the clock: on a machine that
            // is not keeping up a launcher that would be gone in a moment stays for many, and that is no question
            // put to the user. A look that came late, or a window that is not answering Windows, counts for nothing.
            if foreground.is_null() || foreground == self.home || unsafe { IsWindowVisible(foreground) } == 0 {
                self.other = None;
            } else if self.other.map(|o| o.0) != Some(foreground) {
                self.other = Some((foreground, Duration::ZERO));
            } else if let Some((_, held)) = &mut self.other {
                if since_last < LATE && unsafe { IsHungAppWindow(foreground) } == 0 {
                    *held += since_last.min(LOOK_EVERY);
                }
                if *held >= self.needs_user {
                    log("a window wants the user, Launch screen stepping aside");
                    return self.reveal();
                }
            }
            if now - self.started >= GIVE_UP {
                if self.seen {
                    log("no game window after two minutes, Launch screen stepping aside");
                    return self.reveal();
                }
                log("the game never started");
                unsafe { restore(self.home) };
                return Step::Ended;
            }
            Step::Same
        }

        /// The Launch screen is to go. Home's window, covered by it until now, hides first, so what the fade
        /// shows is the game or the window that wants the user and never Home.
        fn reveal(&mut self) -> Step {
            unsafe { ShowWindow(self.home, SW_HIDE) };
            self.state = State::Revealing;
            Step::LaunchScreenGoes
        }

        /// The Launch screen has faded away, or there was none to fade.
        pub fn launch_screen_gone(&mut self) {
            if self.state == State::Revealing {
                self.state = State::Running;
                log("Launch screen gone");
            }
        }

        /// Says whether the Guide is open over the game: on the Overlay, which takes the foreground for it and
        /// gives it back.
        pub fn guide(&mut self, open: bool) {
            if open != self.guide {
                self.guide = open;
                log(if open { "Guide over the game" } else { "Guide closed, back to the game" });
            }
        }

        /// Asks the game to close, as its window's close button would.
        pub fn quit_game(&self) {
            if let Some(game) = self.game {
                unsafe { PostMessageW(game, WM_CLOSE, 0, 0) };
                log("asked the game to quit");
            }
        }
    }

    /// How long a window holds the foreground before it needs the user: `DASHBOARD_NEEDS_USER_MS`, or 5 s.
    fn needs_user() -> Duration {
        let Some(given) = std::env::var_os("DASHBOARD_NEEDS_USER_MS") else { return NEEDS_USER };
        match given.to_string_lossy().trim().parse() {
            Ok(ms) => {
                log(&format!("a window needs the user after {ms} ms (DASHBOARD_NEEDS_USER_MS)"));
                Duration::from_millis(ms)
            }
            Err(_) => {
                log(&format!("DASHBOARD_NEEDS_USER_MS is not a number of milliseconds: {given:?}"));
                NEEDS_USER
            }
        }
    }

    /// Home's window shown again and in front.
    unsafe fn restore(home: HWND) {
        SetWindowPos(home, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
        if !take_foreground(home) {
            log("Windows did not give Home the foreground");
        }
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

    /// The visible windows of programs that live in the game's folder. `home` is the size of the screen.
    fn look(dir: &str, home: HWND) -> Look {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(home, &mut rect) };
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
