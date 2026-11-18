//! Play with Battle.net closed (#172). The client drops `--exec="launch <code>"` given to it as it starts: the TV PC's
//! client (2026-11-18) logged `Command line arguments: {[0]=--exec=launch LBRA}`, signed in, showed its main window
//! and started nothing. bnetlauncher says why in its code ("On start Battle.net client isn't fully functional, this
//! issuing commands to it will just do nothing"; since Shadowlands it "requires the UI to fully load before accepting
//! commands"), and every tool that starts a Battle.net game does it so: the client started on its own, waited for, and
//! only then given `--exec="launch <code>"` (Playnite's BattleNetLibrary, bnetlauncher, the GOG Galaxy plugin,
//! NonSteamLaunchers; none uses `battlenet://`).
//!
//! **Up** is the client's main window shown (a visible window of a `Battle.net.exe` titled `Battle.net`, `Blizzard
//! Battle.net` or `暴雪战网`: the GOG Galaxy plugin's test) with at least [`HELPERS`] `Battle.net.exe` processes
//! running (bnetlauncher waits for three of the client's own; Playnite for five of any `Battle.net`), or that window
//! shown for [`Times::window_alone`] whatever the count; then [`Times::settle`] more (bnetlauncher's 2 s, Playnite's
//! 5 s). Not up in [`Times::ready`] (bnetlauncher's 120 s): asked all the same. Asked, the game is waited for by a
//! process in its folder (Playnite's way; the game's `Base.exe`, its switcher, is one) for [`Times::game`]
//! (bnetlauncher's 15 s, a little longer), then asked once more, then given up. A window of the client's in front
//! that is not its main window (a sign-in, an update, a licence) is the user's: said once, and the Launch screen
//! lets it through as it lets any window that wants the user.
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::{Game, Opener, Start};

/// How many `Battle.net.exe` processes, the client and its helpers, make it up with its main window shown.
pub const HELPERS: usize = 3;

/// The titles of the client's main window: the GOG Galaxy plugin's (`FindWindowW`).
pub const MAIN_TITLES: [&str; 3] = ["Battle.net", "Blizzard Battle.net", "暴雪战网"];

/// The waits of a launch through a client that was not running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Times {
    /// How long the client is given to come up before it is asked all the same.
    pub ready: Duration,
    /// How long its main window shown counts as up without the helpers' count.
    pub window_alone: Duration,
    /// How long after it is up it is asked.
    pub settle: Duration,
    /// How long a process of the game's is waited for after each ask.
    pub game: Duration,
    /// How often the desktop is looked at.
    pub every: Duration,
}

impl Default for Times {
    fn default() -> Self {
        Times {
            ready: Duration::from_secs(120),
            window_alone: Duration::from_secs(10),
            settle: Duration::from_secs(3),
            game: Duration::from_secs(20),
            every: Duration::from_millis(250),
        }
    }
}

/// What one look saw of the client and of the game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// The `Battle.net.exe` processes running (for a fake, its own).
    pub processes: usize,
    /// Whether the client's main window is shown.
    pub main_window: bool,
    /// The title of a window of the client's in front that is not its main window: what it asks of the user.
    pub asking: Option<String>,
    /// A process of a program in the game's folder, as the log says it (`Base.exe, pid 4242`).
    pub game: Option<String>,
}

/// What the desktop shows: the client's processes and windows, and the game's processes.
pub trait Look: Send + Sync {
    /// One look. `root` is a fake Battle.net's folder, whose own `Battle.net.exe` alone counts; `game` the game's.
    fn look(&self, root: Option<&Path>, game: &Path) -> Seen;
}

/// The desktop's own [`Look`]: the process list and the windows (Windows; nothing anywhere else).
#[derive(Debug, Default)]
pub struct Desktop;

impl Look for Desktop {
    fn look(&self, root: Option<&Path>, game: &Path) -> Seen {
        let processes = crate::listening::processes();
        let client: Vec<u32> = processes
            .iter()
            .filter(|p| p.name.eq_ignore_ascii_case(crate::CLIENT_EXE))
            .filter(|p| root.is_none_or(|root| p.program.as_deref().is_some_and(|path| crate::under(root, path))))
            .map(|p| p.pid)
            .collect();
        let game = processes
            .iter()
            .find(|p| p.program.as_deref().is_some_and(|path| crate::under(game, path)))
            .map(|p| format!("{}, pid {}", p.name, p.pid));
        let (main_window, asking) = windows::of(&client);
        Seen { processes: client.len(), main_window, asking, game }
    }
}

/// What the launch does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Wait,
    /// Give the client the start.
    Ask,
    /// Over: the game's process is up, or it was given up; said in the log.
    Over,
}

/// A launch through a client that was not running, look by look: what to do after each, and what to say.
#[derive(Debug)]
pub struct Warming {
    times: Times,
    started: Instant,
    main_since: Option<Instant>,
    up_at: Option<Instant>,
    asked: Vec<Instant>,
    asking_said: Option<String>,
    asking: bool,
    over: bool,
    /// The log's lines from the last look.
    pub said: Vec<String>,
}

impl Warming {
    pub fn new(times: Times, now: Instant) -> Self {
        Warming {
            times,
            started: now,
            main_since: None,
            up_at: None,
            asked: Vec::new(),
            asking_said: None,
            asking: false,
            over: false,
            said: Vec::new(),
        }
    }

    /// Whether the client is still getting ready to start the game: not over, and asking nothing of the user.
    pub fn getting_ready(&self) -> bool {
        !self.over && !self.asking
    }

    fn ms(&self, now: Instant) -> u128 {
        (now - self.started).as_millis()
    }

    /// One look's worth: what to do now.
    pub fn look(&mut self, now: Instant, seen: &Seen) -> Next {
        self.said.clear();
        if self.over {
            return Next::Over;
        }
        if let Some(game) = &seen.game {
            let asks = match self.asked.len() {
                0 => "before it was asked".to_string(),
                1 => "after one ask".to_string(),
                n => format!("after {n} asks"),
            };
            self.said.push(format!(
                "the game's process is up ({game}), {} ms after Battle.net was started, {asks}",
                self.ms(now)
            ));
            self.over = true;
            return Next::Over;
        }
        self.asking = seen.asking.is_some();
        if seen.asking != self.asking_said {
            if let Some(title) = &seen.asking {
                self.said.push(format!(
                    "Battle.net shows {title:?}, not its main window: it asks something of the user, let through"
                ));
            }
            self.asking_said = seen.asking.clone();
        }
        if let Some(last) = self.asked.last() {
            if now - *last < self.times.game {
                return Next::Wait;
            }
            if self.asked.len() >= 2 {
                self.said.push(format!(
                    "no process of the game's {} s after asking twice: given up, {} ms after Battle.net was started",
                    self.times.game.as_secs(),
                    self.ms(now)
                ));
                self.over = true;
                return Next::Over;
            }
            self.said
                .push(format!("no process of the game's {} s after asking: asking again", self.times.game.as_secs()));
            return Next::Ask;
        }
        if let Some(up) = self.up_at {
            return if now - up >= self.times.settle { Next::Ask } else { Next::Wait };
        }
        let since = if seen.main_window {
            *self.main_since.get_or_insert(now)
        } else {
            self.main_since = None;
            now
        };
        let alone = now - since >= self.times.window_alone;
        if seen.main_window && (seen.processes >= HELPERS || alone) {
            self.up_at = Some(now);
            self.said.push(format!(
                "Battle.net is up, {} ms after it was started: its main window shown, {} Battle.net.exe processes{}; \
                 asking it in {} ms",
                self.ms(now),
                seen.processes,
                if seen.processes >= HELPERS { "" } else { " (its window up a while)" },
                self.times.settle.as_millis()
            ));
            return if self.times.settle.is_zero() { Next::Ask } else { Next::Wait };
        }
        if now - self.started >= self.times.ready {
            self.said.push(format!(
                "Battle.net not seen up {} s after it was started ({} Battle.net.exe processes, its main window {}): \
                 asking it all the same",
                self.times.ready.as_secs(),
                seen.processes,
                if seen.main_window { "shown" } else { "not shown" }
            ));
            return Next::Ask;
        }
        Next::Wait
    }

    /// The client was given the start.
    pub fn asked(&mut self, now: Instant) {
        self.asked.push(now);
    }
}

/// The launch under way through a client that was not running, by the game's product code: what
/// [`model::Source::getting_ready`] answers by.
#[derive(Debug, Clone, Default)]
pub struct Launching(Arc<Mutex<Option<(String, bool)>>>);

impl Launching {
    fn set(&self, code: &str, getting_ready: bool) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = Some((code.to_string(), getting_ready));
    }

    pub fn getting_ready(&self, code: &str) -> bool {
        let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        state.as_ref().is_some_and(|(at, ready)| at.eq_ignore_ascii_case(code) && *ready)
    }
}

/// The desktop's [`Look`], or a test's.
#[derive(Clone)]
pub struct Looker(pub Arc<dyn Look>);

impl Default for Looker {
    fn default() -> Self {
        Looker(Arc::new(Desktop))
    }
}

impl std::fmt::Debug for Looker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Looker")
    }
}

/// Asks the client for the game (`start`, `Battle.net.exe --exec="launch <code>"`): at once where it runs; where it
/// does not, it is started on its own and asked once it is up, on a thread of its own (module doc). `root` is a
/// fake's folder.
#[allow(clippy::too_many_arguments)]
pub fn launch(
    code: &str,
    game: &Game,
    start: &Start,
    root: Option<&Path>,
    looker: &Looker,
    launching: &Launching,
    times: Times,
    open: &mut dyn Opener,
) -> Result<(), String> {
    let log = |line: &str| crate::log(&format!("{code}: {line}"));
    let (Start::Program { path: client, .. }, Some(dir)) = (start, game.install_dir.as_ref()) else {
        return open.open(start);
    };
    let seen = looker.0.look(root, dir);
    if seen.processes > 0 {
        log(&format!("Battle.net is running ({} Battle.net.exe processes): asked at once", seen.processes));
        return open.open(start);
    }
    let Some(mut detached) = open.detached() else {
        log("Battle.net is not running, and nothing to ask it with later: asked at once, as it starts");
        return open.open(start);
    };
    let cold = Start::Program { path: client.clone(), args: Vec::new(), dir: None };
    log(&format!(
        "Battle.net is not running: started on its own ({cold}), and asked for the game once it is up: it drops a \
         launch given to it as it starts"
    ));
    open.open(&cold)?;
    launching.set(code, true);
    let (code, start, root, dir) = (code.to_string(), start.clone(), root.map(Path::to_path_buf), dir.clone());
    let (looker, launching) = (looker.clone(), launching.clone());
    std::thread::spawn(move || {
        follow(&code, &start, root.as_deref(), &dir, &looker, &launching, times, &mut *detached)
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn follow(
    code: &str,
    start: &Start,
    root: Option<&Path>,
    dir: &Path,
    looker: &Looker,
    launching: &Launching,
    times: Times,
    open: &mut dyn Opener,
) {
    let log = |line: &str| crate::log(&format!("{code}: {line}"));
    let mut warming = Warming::new(times, Instant::now());
    loop {
        let now = Instant::now();
        let next = warming.look(now, &looker.0.look(root, dir));
        warming.said.iter().for_each(|line| log(line));
        match next {
            Next::Wait => {}
            Next::Ask => {
                warming.asked(now);
                match open.open(start) {
                    Ok(()) => log(&format!("asked: {start}")),
                    Err(why) => log(&format!("{start} not taken ({why})")),
                }
            }
            Next::Over => break,
        }
        launching.set(code, warming.getting_ready());
        std::thread::sleep(times.every);
    }
    launching.set(code, false);
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search<'a> {
        client: &'a [u32],
        main: bool,
    }

    fn title(window: HWND) -> String {
        let mut text = [0u16; 256];
        // SAFETY: the title read into a buffer of its length.
        let len = unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) }.max(0) as usize;
        String::from_utf16_lossy(&text[..len])
    }

    fn pid(window: HWND) -> u32 {
        let mut pid = 0u32;
        // SAFETY: a window handle Windows looks up, its process id written to a u32.
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        pid
    }

    unsafe extern "system" fn each(window: HWND, param: LPARAM) -> i32 {
        let search = &mut *(param as *mut Search);
        if IsWindowVisible(window) != 0
            && search.client.contains(&pid(window))
            && super::MAIN_TITLES.contains(&title(window).trim())
        {
            search.main = true;
            return 0;
        }
        1
    }

    /// Whether one of `client`'s processes shows its main window; the title of a window of theirs in front that is
    /// not the main one.
    pub fn of(client: &[u32]) -> (bool, Option<String>) {
        if client.is_empty() {
            return (false, None);
        }
        let mut search = Search { client, main: false };
        // SAFETY: the callback reads `search` only while EnumWindows runs.
        unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
        // SAFETY: the foreground window, looked up.
        let front = unsafe { GetForegroundWindow() };
        let asking = (!front.is_null() && unsafe { IsWindowVisible(front) } != 0 && client.contains(&pid(front)))
            .then(|| title(front))
            .filter(|title| !super::MAIN_TITLES.contains(&title.trim()));
        (search.main, asking)
    }
}

#[cfg(not(windows))]
mod windows {
    pub fn of(_client: &[u32]) -> (bool, Option<String>) {
        (false, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times() -> Times {
        Times {
            ready: Duration::from_secs(120),
            window_alone: Duration::from_secs(10),
            settle: Duration::from_secs(3),
            game: Duration::from_secs(20),
            every: Duration::from_millis(250),
        }
    }

    fn seen(processes: usize, main_window: bool) -> Seen {
        Seen { processes, main_window, ..Seen::default() }
    }

    #[test]
    fn the_client_is_asked_once_up_and_settled_and_the_launch_is_over_when_the_game_runs() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut warming = Warming::new(times(), t0);
        assert_eq!(warming.look(at(1), &seen(1, false)), Next::Wait, "starting: one process, no window");
        assert_eq!(warming.look(at(3), &seen(2, true)), Next::Wait, "its window, its helpers not yet");
        assert!(warming.getting_ready());
        assert_eq!(warming.look(at(4), &seen(3, true)), Next::Wait, "up: settling");
        assert!(warming.said[0].starts_with("Battle.net is up, 4000 ms after it was started"), "{:?}", warming.said);
        assert_eq!(warming.look(at(6), &seen(3, true)), Next::Wait);
        assert_eq!(warming.look(at(7), &seen(3, true)), Next::Ask, "3 s after it was up");
        warming.asked(at(7));
        assert_eq!(warming.look(at(10), &seen(3, true)), Next::Wait, "the game waited for");
        assert!(warming.getting_ready(), "the Launch screen stays up until the game runs");
        let game = Seen { game: Some("Base.exe, pid 4242".into()), ..seen(3, true) };
        assert_eq!(warming.look(at(12), &game), Next::Over);
        assert_eq!(
            warming.said,
            ["the game's process is up (Base.exe, pid 4242), 12000 ms after Battle.net was started, after one ask"]
        );
        assert!(!warming.getting_ready());
    }

    #[test]
    fn asked_again_once_and_then_given_up_when_no_process_of_the_games_comes() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut warming = Warming::new(Times { settle: Duration::ZERO, ..times() }, t0);
        assert_eq!(warming.look(at(2), &seen(4, true)), Next::Ask, "up, with no settle");
        warming.asked(at(2));
        assert_eq!(warming.look(at(21), &seen(4, true)), Next::Wait);
        assert_eq!(warming.look(at(22), &seen(4, true)), Next::Ask);
        assert_eq!(warming.said, ["no process of the game's 20 s after asking: asking again"]);
        warming.asked(at(22));
        assert_eq!(warming.look(at(42), &seen(4, true)), Next::Over);
        assert!(warming.said[0].starts_with("no process of the game's 20 s after asking twice: given up"));
        assert!(!warming.getting_ready());
    }

    #[test]
    fn its_window_alone_for_a_while_is_up_and_nothing_up_in_two_minutes_is_asked_all_the_same() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut alone = Warming::new(times(), t0);
        assert_eq!(alone.look(at(1), &seen(1, true)), Next::Wait);
        assert_eq!(alone.look(at(10), &seen(1, true)), Next::Wait);
        assert_eq!(alone.look(at(11), &seen(1, true)), Next::Wait, "up: settling");
        assert!(alone.said[0].contains("(its window up a while)"), "{:?}", alone.said);

        let mut never = Warming::new(times(), t0);
        assert_eq!(never.look(at(119), &seen(2, false)), Next::Wait);
        assert_eq!(never.look(at(120), &seen(2, false)), Next::Ask);
        assert!(never.said[0].starts_with("Battle.net not seen up 120 s after it was started"));
    }

    #[test]
    fn a_window_of_the_clients_that_asks_the_user_is_let_through_and_said_once() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut warming = Warming::new(times(), t0);
        let login = Seen { asking: Some("Battle.net Login".into()), ..seen(3, false) };
        assert_eq!(warming.look(at(2), &login), Next::Wait);
        assert_eq!(
            warming.said,
            ["Battle.net shows \"Battle.net Login\", not its main window: it asks something of the user, let through"]
        );
        assert!(!warming.getting_ready(), "the Launch screen lets the sign-in through");
        assert_eq!(warming.look(at(3), &login), Next::Wait);
        assert!(warming.said.is_empty(), "said once");
        assert_eq!(warming.look(at(30), &seen(3, true)), Next::Wait, "signed in: up");
        assert!(warming.getting_ready());
        assert_eq!(warming.look(at(33), &seen(3, true)), Next::Ask);
    }

    #[test]
    fn a_game_already_running_ends_it_before_any_ask() {
        let mut warming = Warming::new(times(), Instant::now());
        let game = Seen { game: Some("game.exe, pid 7".into()), ..Seen::default() };
        assert_eq!(warming.look(Instant::now(), &game), Next::Over);
        assert!(warming.said[0].ends_with("before it was asked"));
    }
}
