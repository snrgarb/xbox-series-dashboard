//! Launching a game through its source and following it: asked of its store (or its program run), its window up,
//! booted, ended. While the game starts the Launch screen is over everything, on the Overlay; this says when it should
//! go, and puts Home's window, which shows the game's backdrop meanwhile, directly behind the game's window, where it
//! stays, not activated, until the game's window goes: it takes the foreground then, and Home comes back in it once
//! the game has ended (docs/spec/launch-screen.md). The game's window is marked full screen for the shell, so the
//! taskbar goes under the backdrop while the game is active. Home's window is otherwise left as it is: it is never
//! made topmost, layered or clipped here. (An install's wizard keeps it topmost for a while, `crate::front`; that hold
//! is let go as the launch opens the game, `SystemAddresses::open`, and is never taken over a game that runs.)
use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use model::{Game, Source};

use crate::store::Addresses;
use crate::{Freezer, Overlay};

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
    pub fn start(
        game: &Game,
        _: Arc<dyn Source>,
        _: &Path,
        _: *mut c_void,
        _: bool,
        _: &mut dyn Addresses,
    ) -> Option<Self> {
        eprintln!("launch: {} can only be started on Windows", game.name);
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

    pub fn freeze(&self, _: &mut Freezer) {}

    pub fn quit_game(&mut self, _: &mut Freezer) {}

    pub fn quitting(&mut self, _: bool) {}

    pub fn force_quit(&mut self, _: &mut Freezer) -> bool {
        false
    }

    pub fn game_window(&self) -> *mut c_void {
        std::ptr::null_mut()
    }

    pub fn show_home(&mut self, _: bool) {}

    pub fn show_game(&mut self, _: bool) -> bool {
        false
    }

    pub fn game_in_front(&self) -> bool {
        false
    }
}

#[cfg(windows)]
pub use win::Launch;

/// Whether a window cloaked so (its `DWMWA_CLOAKED`) may be the game's. Not cloaked (0), or cloaked by the shell (2,
/// `DWM_CLOAKED_SHELL`): Windows' Xbox mode cloaks the app that is not in front, the game's windows while Home is,
/// and a virtual desktop not shown cloaks its windows so too; the game still has them. Not a window its own app
/// cloaked (1, `DWM_CLOAKED_APP`: a UWP frame kept for later, a window hidden by design), nor one cloaked with its
/// owner (4, `DWM_CLOAKED_INHERITED`). Other programs' windows are kept out by their path, not by this.
#[cfg_attr(not(windows), allow(dead_code))]
fn may_be_the_games(cloaked: u32) -> bool {
    matches!(cloaked, 0 | 2)
}

#[cfg(test)]
mod tests {
    use super::may_be_the_games;

    #[test]
    fn a_window_the_shell_cloaked_is_still_the_games_and_one_its_app_cloaked_is_not() {
        assert!(may_be_the_games(0), "shown");
        assert!(may_be_the_games(2), "cloaked by the shell: Xbox mode with Home in front");
        assert!(!may_be_the_games(1), "cloaked by its own app");
        assert!(!may_be_the_games(4), "cloaked with its owner");
        assert!(!may_be_the_games(1 | 2), "its app's cloak as well as the shell's");
    }
}

#[cfg(windows)]
mod win {
    use super::{c_void, Addresses, Arc, Duration, Freezer, Game, Overlay, Path, Source, Step};
    use crate::take_foreground;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };
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
    /// A launch whose game has shown nothing yet, after the Launch screen stepped aside for a window that wanted the
    /// user, is over once its store and the desktop have still shown nothing of it for this long. A game that has
    /// been seen is over as soon as its windows and processes have gone and its store no longer says it runs.
    const GONE: Duration = Duration::from_secs(2);
    /// The game's own window covers at least this much of the screen; a launcher's or a splash doesn't.
    const GAME_COVERS: f64 = 0.6;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum State {
        /// Asked of its store. The Launch screen is over whatever the store and the game put up; Home's window is
        /// under it all, as it was, showing the game's backdrop once the Launch screen covers it.
        Starting,
        /// The Launch screen is fading away, Home's window directly behind the game's: what shows through is the
        /// game, or the window that wants the user, with the backdrop round it.
        Revealing,
        /// The Launch screen has gone. The game runs, with the Guide over it or not.
        Running,
    }

    pub struct Launch {
        /// The game, and its source, which is asked whether it runs.
        playing: Game,
        source: Arc<dyn Source>,
        /// The game's folder, lower case with backslashes and a trailing one.
        dir: String,
        /// What a program's path begins with when it is the game's: the folder, and where it really is when the
        /// folder is a link (`freeze::prefixes`).
        prefixes: Vec<String>,
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
        /// The looks while the game starts, and how many of them came late: said when the launch gives up, where
        /// it tells a machine that did not keep up from a game that never showed a window.
        looks: (u32, u32),
        /// Whether the game's window has been said to be up.
        said_up: bool,
        /// Since when nothing of the game has shown.
        gone_since: Option<Instant>,
        /// Whether its store or the desktop has shown anything of the game yet.
        seen: bool,
        /// The game's largest window when last looked for.
        game: Option<HWND>,
        /// Whether the Guide is open over the game.
        guide: bool,
        /// Whether the game has been asked to quit: from then on it is not frozen, so it is free to close.
        quitting: bool,
        /// Whether a window of the game's was up at the last look: its coming and going are said.
        window_up: bool,
        /// The window Home's window was last put directly behind, once the Launch screen has gone.
        behind: HWND,
        /// Whether Home's window is in front of the running game: Home was chosen in the Guide, or with Alt+Tab.
        home_in_front: bool,
        /// The windows marked full screen for the shell, so that the taskbar goes under Home's window while one of
        /// them is active; unmarked when the launch ends.
        marked: Vec<HWND>,
        /// With no window of the game's up: the processes of its still running when last said.
        left: Vec<String>,
        /// With no window or process of the game's left: since when its store has still said it runs.
        store_since: Option<Instant>,
        /// The game's largest window when last said, and how it was cloaked.
        found: (HWND, u32),
    }

    fn log(message: &str) {
        eprintln!("launch: {message} at {:.0} ms", crate::since_start());
    }

    impl Launch {
        /// Asks the game's source for it: its store's address opened, or its program run. `dir` is the folder it is
        /// installed in, whose windows are the game's; `home` is Home's window, which has the foreground;
        /// `launch_screen` is whether there is an Overlay to show the Launch screen on; `addresses` opens what the
        /// source gives. None, having said why, when the game can't be started.
        // An HWND is a handle Windows looks up, never a pointer this code follows.
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub fn start(
            game: &Game,
            source: Arc<dyn Source>,
            dir: &Path,
            home: *mut c_void,
            launch_screen: bool,
            addresses: &mut dyn Addresses,
        ) -> Option<Self> {
            if home.is_null() {
                return None;
            }
            let Some(start) = source.start(game) else {
                eprintln!("launch: {} can't start {}", source.name(), game.name);
                return None;
            };
            let mut dir = dir.to_string_lossy().replace('/', "\\").to_lowercase();
            if !dir.ends_with('\\') {
                dir.push('\\');
            }
            // Home has the foreground, and with it the right to let the store and the game take it.
            if let Err(why) = addresses.open(&start) {
                eprintln!("launch: {} didn't take {start} ({why})", source.name());
                return None;
            }
            let screen = if launch_screen { "Launch screen up" } else { "no Overlay, so no Launch screen" };
            log(&format!("{} ({}) asked of {}, {screen}", game.name, game.source_id, source.name()));
            let now = Instant::now();
            Some(Self {
                playing: game.clone(),
                source,
                prefixes: crate::freeze::prefixes_here(&dir),
                dir,
                home,
                state: State::Starting,
                started: now,
                next_look: now,
                game_since: None,
                other: None,
                needs_user: needs_user(),
                looked: now,
                looks: (0, 0),
                said_up: false,
                gone_since: None,
                seen: false,
                game: None,
                guide: false,
                quitting: false,
                window_up: false,
                behind: std::ptr::null_mut(),
                home_in_front: false,
                marked: Vec::new(),
                left: Vec::new(),
                store_since: None,
                found: (std::ptr::null_mut(), 0),
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
            let look = look(&self.prefixes, self.home);
            let running = self.source.running(&self.playing);
            self.seen |= running || look.any;
            self.game = look.largest;
            // Which window of the game's is its largest, and how it is cloaked: said as either changes.
            let found = (look.largest.unwrap_or(std::ptr::null_mut()), look.cloaked);
            if found != self.found && look.largest.is_some() {
                let how = match look.cloaked {
                    0 => "not cloaked",
                    2 => "cloaked by the shell",
                    _ => "cloaked",
                };
                log(&format!("the game's window {}, {how} ({})", unsafe { crate::describe(found.0) }, found.1));
            }
            self.found = found;
            let starting = self.state == State::Starting;
            if look.any != self.window_up {
                self.window_up = look.any;
                log(match (look.any, starting) {
                    (true, _) => "game window seen",
                    (false, true) => "game gone, Launch screen showing",
                    (false, false) if self.home_in_front => "game gone, Home in front",
                    (false, false) => "game gone, backdrop showing",
                });
                if !look.any && !starting {
                    self.home_takes_front();
                }
            }

            match self.state {
                State::Starting => self.starting(now, since_last, &look, overlay),
                State::Revealing | State::Running => {
                    // After a window that wanted the user, or after giving up: the game's window, when it comes.
                    if look.game.is_some() && !self.said_up {
                        self.said_up = true;
                        log("the game's window is up");
                    }
                    self.keep_behind(look.largest);
                    if look.any {
                        (self.gone_since, self.store_since) = (None, None);
                        self.left.clear();
                        return Step::Same;
                    }
                    // Nothing of the game's has shown yet (the Launch screen stepped aside for a window that wanted
                    // the user): over once its store and the desktop have shown nothing of it for a while.
                    if !self.seen {
                        if now - *self.gone_since.get_or_insert(now) >= GONE {
                            return self.ended(overlay, "nothing of the game's showed");
                        }
                        return Step::Same;
                    }
                    self.gone(now, running, overlay)
                }
            }
        }

        /// No window of the game's is up: it has ended once none of its processes runs and its store no longer
        /// says it does, which is looked at every LOOK_EVERY with no wait of its own. A process of the game's
        /// still running is a game between windows or handing off to another of its programs; its store still
        /// saying it runs (Steam's `Running`) is a store that has not seen it go yet, or a launcher outside the
        /// game's folder starting it again. Home's window, the backdrop, holds meanwhile.
        fn gone(&mut self, now: Instant, running: bool, overlay: Option<&Overlay>) -> Step {
            let left = crate::freeze::running_under(&self.prefixes);
            if !left.is_empty() {
                if left != self.left {
                    log(&format!("no window of the game's, its processes still running: {}", left.join(", ")));
                    self.left = left;
                }
                self.store_since = None;
                return Step::Same;
            }
            // The store still saying it runs is waited for only while something else has the foreground: a launcher
            // outside the game's folder may be starting it again. With Home's window in front the user is back at
            // the Dashboard, and a store slow to see the game go (Steam took 2.2 s on the TV PC) holds nothing up.
            let home_front = unsafe { GetForegroundWindow() } == self.home;
            if running && !home_front {
                if self.store_since.is_none() {
                    log(&format!("the game's processes have gone, {} still says it runs", self.source.name()));
                    self.store_since = Some(now);
                }
                return Step::Same;
            }
            let why = match (running, self.store_since) {
                (true, _) => format!(
                    "the game's windows and processes have gone, Home's window has the foreground: ended without \
                     waiting for {} to say so",
                    self.source.name()
                ),
                (false, Some(since)) => format!(
                    "the game's windows and processes have gone, and {} said so {:.0} ms later",
                    self.source.name(),
                    (now - since).as_secs_f64() * 1000.0
                ),
                (false, None) => "the game's windows and processes have gone".to_string(),
            };
            self.ended(overlay, &why)
        }

        /// The game has ended: Home's window back in front, the windows marked full screen for the shell unmarked.
        fn ended(&mut self, overlay: Option<&Overlay>, why: &str) -> Step {
            log(why);
            for window in std::mem::take(&mut self.marked) {
                if unsafe { IsWindow(window) } != 0 {
                    if let Err(why) = mark_fullscreen(window, false) {
                        let window = unsafe { crate::describe(window) };
                        log(&format!("{window} could not be unmarked as full screen: {why}"));
                    }
                }
            }
            // Under the open Guide the foreground stays the Guide's, as in the Bundle: Home's window comes back
            // behind it, and gets the foreground when the Guide closes.
            if overlay.is_some_and(|overlay| overlay.mode() == Some(crate::Mode::Guide)) {
                log("the game has ended, Home back behind the Guide");
                let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE;
                unsafe { SetWindowPos(self.home, HWND_NOTOPMOST, 0, 0, 0, 0, flags) };
            } else {
                log("the game has ended, Home back");
                unsafe { restore(self.home) };
            }
            Step::Ended
        }

        /// The game's last window has gone: Home's window, the backdrop, takes the foreground at once, so that what
        /// Windows would hand it on to (whatever was active before, a console) does not come up over the backdrop.
        /// Not while Home was chosen over the game, which has it already, or under the open Guide, which keeps it.
        fn home_takes_front(&self) {
            if self.home_in_front || self.guide {
                return;
            }
            unsafe {
                let taken = GetForegroundWindow() == self.home || take_foreground(self.home);
                log(&format!(
                    "Home's window takes the foreground: {taken}, {} has it; Home's window {}",
                    crate::describe(GetForegroundWindow()),
                    window_state(self.home)
                ));
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
            self.looks.0 += 1;
            self.looks.1 += (since_last >= LATE) as u32;
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
                let (looks, late) = self.looks;
                log(&format!("{late} of {looks} looks at the desktop came late"));
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

        /// The Launch screen is to go. Home's window, covered by it until now and showing the game's backdrop, goes
        /// directly behind the window in front (the game's, or the one that wants the user), so what the fade shows
        /// is that window with the backdrop round it, and never Home or the desktop.
        fn reveal(&mut self) -> Step {
            let front = unsafe { GetForegroundWindow() };
            let front = if front.is_null() || ours(front) || unsafe { IsWindowVisible(front) } == 0 {
                self.game.unwrap_or(std::ptr::null_mut())
            } else {
                front
            };
            if front.is_null() {
                log("no window of the game's to put Home's window behind: it stays where it is");
            } else {
                self.put_behind(front);
            }
            self.state = State::Revealing;
            Step::LaunchScreenGoes
        }

        /// Home's window directly behind `front` in the z-order, shown and not activated: whatever does not cover
        /// round `front`, and whatever is left when it goes, is Home's window, which shows the game's backdrop
        /// meanwhile. Behind every topmost window instead, when `front` is one: a window put after a topmost one is
        /// made topmost itself, and Home's must never be.
        ///
        /// `front` is marked full screen for the shell too: while it is active the taskbar goes to the bottom of the
        /// z-order, under Home's window, however little of the screen `front` covers (docs/spec/launch-screen.md).
        fn put_behind(&mut self, front: HWND) {
            unsafe {
                let topmost = GetWindowLongPtrW(front, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
                let after = if topmost { HWND_TOP } else { front };
                let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW;
                SetWindowPos(self.home, after, 0, 0, 0, 0, flags);
                let which = if topmost { "the topmost windows, below " } else { "" };
                log(&format!(
                    "Home's window behind {which}{} as the game's backdrop; Home's window {}",
                    crate::describe(front),
                    window_state(self.home)
                ));
            }
            if !self.marked.contains(&front) {
                match mark_fullscreen(front, true) {
                    Ok(()) => log("the game's window marked full screen: the taskbar goes under the backdrop"),
                    Err(why) => log(&format!("the game's window could not be marked full screen: {why}")),
                }
                self.marked.push(front);
            }
            self.behind = front;
        }

        /// A window of the game's that came since Home's window was put behind the last: after a hand-off to another
        /// process, or a window the game made anew. Home's window goes behind it, and it is given the foreground if
        /// Home's window has it, as it has when Windows gave it on the last window's going.
        fn keep_behind(&mut self, largest: Option<HWND>) {
            let Some(window) = largest else { return };
            if self.home_in_front || window == self.behind {
                return;
            }
            self.put_behind(window);
            if !self.guide && unsafe { GetForegroundWindow() } == self.home {
                let given = unsafe { SetForegroundWindow(window) } != 0;
                log(&format!("the game's new window given the foreground: {given}"));
            }
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

        /// Holds the game still under the Guide: every process of a program in its folder, and the one that owns
        /// its window, until `freezer` thaws them. Not a game that is quitting. Does nothing while it is frozen
        /// already.
        pub fn freeze(&self, freezer: &mut Freezer) {
            if self.quitting || freezer.is_frozen() {
                return;
            }
            let mut owner = 0u32;
            if let Some(game) = self.game {
                unsafe { GetWindowThreadProcessId(game, &mut owner) };
            }
            freezer.freeze(&self.dir, Some(owner).filter(|&owner| owner != 0));
        }

        /// Asks the game to close, as its window's close button would. A frozen game can't answer: it is thawed
        /// first, and stays so until it has gone or the quit is given up (`quitting`).
        pub fn quit_game(&mut self, freezer: &mut Freezer) {
            freezer.thaw();
            self.quitting = true;
            match self.game {
                Some(game) if unsafe { PostMessageW(game, WM_CLOSE, 0, 0) } != 0 => log("asked the game to quit"),
                Some(_) => log("the game's window could not be asked to close"),
                None => log("the game has no window to ask to close"),
            }
        }

        /// Says whether the game is still being quit: once the user has declined to force it, it is a game like
        /// any other again, and held still under the Guide.
        pub fn quitting(&mut self, quitting: bool) {
            if self.quitting && !quitting {
                log("the quit is over, the game left running");
            }
            self.quitting = quitting;
        }

        /// Ends the program that owns the game's window, as the Bundle does (Win32WindowSystem.ForceKill): the
        /// game did not close when asked. False when there is no window, and so nothing to end.
        pub fn force_quit(&mut self, freezer: &mut Freezer) -> bool {
            freezer.thaw();
            let Some(game) = self.game.filter(|&game| unsafe { IsWindow(game) } != 0) else {
                log("force quit: the game has no window, so nothing to end");
                return false;
            };
            let mut owner = 0u32;
            unsafe {
                GetWindowThreadProcessId(game, &mut owner);
                let process = OpenProcess(PROCESS_TERMINATE, 0, owner);
                if process.is_null() {
                    // Gone already, or elevated beyond reach.
                    log(&format!("force quit: process {owner} could not be opened to end it"));
                    return true;
                }
                let ended = TerminateProcess(process, 1) != 0;
                CloseHandle(process);
                log(&format!("force quit: process {owner} ended: {ended}"));
            }
            true
        }

        /// The game's largest window when last looked for; null when it has none.
        pub fn game_window(&self) -> *mut c_void {
            self.game.filter(|&game| unsafe { IsWindow(game) } != 0).unwrap_or(std::ptr::null_mut())
        }

        /// Home's window in front of the running game, which runs on behind it: Home or a page was chosen in the
        /// Guide. With `foreground` it takes the foreground too; without, the Overlay is about to hand it over.
        pub fn show_home(&mut self, foreground: bool) {
            self.home_in_front = true;
            unsafe {
                if foreground {
                    restore(self.home);
                } else {
                    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE;
                    SetWindowPos(self.home, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
                }
            }
            log("Home in front of the game");
        }

        /// The game in front again and Home's window directly behind it as its backdrop, as while it was played.
        /// With `foreground` the game's window is given the foreground, which is Home's to give; without, the
        /// Overlay has just handed it over. False when the game has no window to go back to: nothing is done.
        pub fn show_game(&mut self, foreground: bool) -> bool {
            let game = self.game_window();
            if game.is_null() {
                log("the game has no window to go back to");
                return false;
            }
            self.home_in_front = false;
            unsafe {
                if foreground {
                    if IsIconic(game) != 0 {
                        ShowWindow(game, SW_RESTORE);
                    }
                    // In Xbox mode the game's window is cloaked by the shell while Home is in front. No documented
                    // call switches Xbox mode's app: activating the window is how an app is brought forward there,
                    // as it is for a window on another virtual desktop. Said, with the window's state after, so a
                    // run shows whether the shell uncloaked it (the next look says so again as it changes).
                    let given = SetForegroundWindow(game) != 0;
                    log(&format!("the game's window given the foreground: {given}; it is {}", window_state(game)));
                }
            }
            self.put_behind(game);
            log("back to the game, Home behind it");
            true
        }

        /// Whether the game is what the user is playing: the Launch screen has gone, Home was not chosen over it, and
        /// its window is up. Home's window getting the foreground then is the user choosing the Dashboard (Alt+Tab),
        /// and not Windows handing it on as the game's window went.
        pub fn game_in_front(&self) -> bool {
            let game = self.game_window();
            self.state == State::Running
                && !self.home_in_front
                && !game.is_null()
                && unsafe { IsWindowVisible(game) } != 0
        }
    }

    /// Whether `hwnd` is a window of the Dashboard's own: Home's, the Overlay, the Toast strip.
    fn ours(hwnd: HWND) -> bool {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid == std::process::id()
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
        if GetForegroundWindow() != home && !take_foreground(home) {
            log(&format!(
                "Windows did not give Home the foreground: {} has it",
                crate::describe(GetForegroundWindow())
            ));
        }
    }

    /// What Windows has a window as, for the log: shown, minimised, cloaked (hidden by Windows or the shell while
    /// still shown: in Xbox mode, the app not in front), and where.
    unsafe fn window_state(window: HWND) -> String {
        let mut cloaked = 0u32;
        DwmGetWindowAttribute(window, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut c_void, 4);
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(window, &mut rect);
        let cloaked = match cloaked {
            0 => String::new(),
            1 => ", cloaked by the Dashboard".into(),
            2 => ", cloaked by the shell".into(),
            4 => ", cloaked with its owner".into(),
            other => format!(", cloaked ({other})"),
        };
        format!(
            "{}{}{cloaked}, {}x{} at ({}, {})",
            if IsWindowVisible(window) != 0 { "shown" } else { "hidden" },
            if IsIconic(window) != 0 { ", minimised" } else { "" },
            rect.right - rect.left,
            rect.bottom - rect.top,
            rect.left,
            rect.top
        )
    }

    /// Tells the shell that `window` is a full-screen window, or leaves it to the shell's own judging again: while a
    /// window marked so is active, the taskbar goes to the bottom of the z-order (ITaskbarList2::MarkFullscreenWindow,
    /// whose documentation says so). The game's own window is marked, since it is the one active; Home's window behind
    /// it covers the display, the taskbar's place included.
    fn mark_fullscreen(window: HWND, fullscreen: bool) -> Result<(), String> {
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{ITaskbarList2, TaskbarList};
        unsafe {
            // The window's thread has COM already; either answer is fine.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let list: ITaskbarList2 =
                CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).map_err(|why| why.to_string())?;
            list.HrInit().map_err(|why| why.to_string())?;
            list.MarkFullscreenWindow(windows::Win32::Foundation::HWND(window), fullscreen)
                .map_err(|why| why.to_string())
        }
    }

    #[derive(Default)]
    struct Look {
        /// A window of the game's that covers the screen.
        game: Option<HWND>,
        /// Any visible window of a program in the game's folder.
        any: bool,
        /// The largest of them.
        largest: Option<HWND>,
        /// How the largest is cloaked (`DWMWA_CLOAKED`): 0, or 2 where the shell has set the game aside (Xbox
        /// mode, with Home in front).
        cloaked: u32,
    }

    struct Search<'a> {
        prefixes: &'a [String],
        screen: f64,
        best: f64,
        largest: f64,
        look: Look,
    }

    /// The visible windows of programs that live in the game's folder. `home` is the size of the screen.
    fn look(prefixes: &[String], home: HWND) -> Look {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(home, &mut rect) };
        let screen = ((rect.right - rect.left) as f64 * (rect.bottom - rect.top) as f64).max(1.0);
        let mut search = Search { prefixes, screen, best: 0.0, largest: 0.0, look: Look::default() };
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
        if !super::may_be_the_games(cloaked) || pid == 0 || pid == std::process::id() {
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
        let path = String::from_utf16_lossy(&path[..len as usize]).to_lowercase();
        if ok == 0 || !search.prefixes.iter().any(|prefix| path.starts_with(prefix.as_str())) {
            return 1;
        }
        search.look.any = true;
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let area = (rect.right - rect.left).max(0) as f64 * (rect.bottom - rect.top).max(0) as f64;
        if area > search.largest {
            search.largest = area;
            search.look.largest = Some(hwnd);
            search.look.cloaked = cloaked;
        }
        if area >= search.screen * GAME_COVERS && area > search.best {
            search.best = area;
            search.look.game = Some(hwnd);
        }
        1
    }
}
