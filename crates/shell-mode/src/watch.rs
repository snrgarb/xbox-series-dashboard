//! What the --shell process decides, with no Windows in it: when to start the Dashboard and explorer, when the
//! black is up, what an exit was, and what the pad asks (docs/spec/shell-mode.md, 5; the Bundle's Watchdog.cs,
//! SignInSchedule.cs, ShellWatch.cs, PadWatch.cs and AudioSettle.cs). Time is the time since the --shell process
//! started, given to each call; each says what it decided as lines for the log, which the caller takes with
//! [`Watchdog::take_said`] and the like.
use std::collections::BTreeMap;
use std::time::Duration;

use crate::contract;

/// The black stays up this long for a start at most, first frame or not.
pub const BLACK_LIMIT: Duration = Duration::from_secs(60);
/// Three crashes within five minutes fall back to the desktop.
pub const CRASH_SPAN: Duration = Duration::from_secs(300);
pub const CRASH_LIMIT: usize = 3;
/// The fallback's line shows this long before the desktop starts.
pub const LINE_FOR: Duration = Duration::from_secs(5);
/// After View+Menu held, the Dashboard has this long to close before it is ended.
pub const CLOSE_FOR: Duration = Duration::from_secs(10);
/// The fallback's line, on the black.
pub const FALLBACK_LINE: &str = "The Dashboard closed 3 times in 5 minutes. Press the Guide button to try again.";

fn seconds(time: Duration) -> String {
    format!("{:.1} s", time.as_secs_f64())
}

/// What the --shell process is to do after the watchdog has heard something.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Orders {
    /// Reset the two events and start the Dashboard behind the black, then say `launched` or `launch_failed`.
    pub launch: bool,
    /// Make sure there is a desktop: explorer, if no shell window is up.
    pub start_desktop: bool,
    /// Ask these Dashboards to close: WM_CLOSE to each of their windows.
    pub close: Vec<u32>,
    /// End these: they didn't close in time.
    pub end: Vec<u32>,
}

impl Orders {
    fn and(mut self, other: Orders) -> Orders {
        self.launch |= other.launch;
        self.start_desktop |= other.start_desktop;
        self.close.extend(other.close);
        self.end.extend(other.end);
        self
    }
}

#[derive(Debug, Clone)]
struct Tracked {
    /// Started by the --shell process, behind the black: its first frame is waited for.
    launched: bool,
    past_first_frame: bool,
    closed_by_hold: bool,
    /// Has had a window: one it didn't start is a Dashboard only once it has, as this exe's jobs never make one.
    windowed: bool,
}

/// Keeps the Dashboard up for the whole sign-in. It watches every Dashboard process in the session by its exe's
/// path, not only the one it started: an update's handover starts the next one itself (docs/research/shell.md, 4.6).
/// When the last one exits, its code and what is left say what it was:
///
/// - this exe's other jobs (`--sentinel`, `--shell-on` and `--shell-off` from a terminal or Settings, the probe, a
///   second Dashboard that gave way) end with [`contract::NOT_A_DASHBOARD`] or, not started by the --shell process,
///   without ever having had a window; they count for nothing;
/// - 0 with another Dashboard left: a handover, and the other one is watched;
/// - 0 with nothing left: Exit to desktop. It stays down, and there is a desktop;
/// - anything else, or any exit before the first frame of a start this process made: a crash, relaunched at once
///   behind the black. The third within five minutes falls back: the line on the black for five seconds, then the
///   desktop, and nothing relaunched on its own until the Guide button.
#[derive(Debug, Default)]
pub struct Watchdog {
    alive: BTreeMap<u32, Tracked>,
    crashes: Vec<Duration>,
    /// The start the black is up for, and when it began.
    start: Option<(u32, Duration)>,
    black: bool,
    fallen_back: bool,
    line_until: Option<Duration>,
    close_until: Option<Duration>,
    /// A Dashboard exited 0 leaving another: if what is left turns out to be none, it was Exit to desktop.
    quit_pending: bool,
    said: Vec<String>,
}

impl Watchdog {
    pub fn take_said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    fn say(&mut self, line: String) {
        self.said.push(format!("watchdog: {line}"));
    }

    /// Whether the black is to be up: for a start before its first frame, or for the fallback's line.
    pub fn black(&self) -> bool {
        self.black || self.line_until.is_some()
    }

    /// The line on the black, if any.
    pub fn line(&self) -> Option<&'static str> {
        self.line_until.map(|_| FALLBACK_LINE)
    }

    /// Whether any Dashboard is up: one it started, or one with a window.
    pub fn running(&self) -> bool {
        self.alive.values().any(|tracked| tracked.windowed)
    }

    /// The processes it didn't start that haven't had a window yet: the caller looks for theirs.
    pub fn windowless(&self) -> Vec<u32> {
        self.alive.iter().filter(|(_, tracked)| !tracked.windowed).map(|(pid, _)| *pid).collect()
    }

    /// `pid` has a window: a Dashboard.
    pub fn has_window(&mut self, pid: u32) {
        if let Some(tracked) = self.alive.get_mut(&pid) {
            if !std::mem::replace(&mut tracked.windowed, true) {
                self.say(format!("pid {pid} has a window: a Dashboard"));
            }
        }
    }

    /// The --shell process started a Dashboard, `pid`: a start, behind the black.
    pub fn launched(&mut self, pid: u32, now: Duration) {
        self.alive
            .insert(pid, Tracked { launched: true, past_first_frame: false, closed_by_hold: false, windowed: true });
        self.start = Some((pid, now));
        self.black = true;
        self.say(format!(
            "Dashboard start, pid {pid}: black until its first frame, {} s at most",
            BLACK_LIMIT.as_secs()
        ));
    }

    /// The Dashboard couldn't be started: a crash, as it is down all the same.
    pub fn launch_failed(&mut self, now: Duration) -> Orders {
        self.say("the Dashboard didn't start: a crash".into());
        self.count_crash(now);
        if self.fallen_back {
            self.lift_black("the --shell process has fallen back");
            return Orders::default();
        }
        self.relaunch()
    }

    /// The Dashboard set FirstFrame.
    pub fn first_frame(&mut self, now: Duration) {
        let Some((pid, began)) = self.start else { return };
        let Some(tracked) = self.alive.get_mut(&pid) else { return };
        if tracked.past_first_frame {
            return;
        }
        tracked.past_first_frame = true;
        self.say(format!("first frame for pid {pid}, {} after its start", seconds(now.saturating_sub(began))));
        self.lift_black("the first frame");
    }

    /// A Dashboard the --shell process didn't start has appeared: the Start menu, an update's handover, or one
    /// of this exe's other jobs. No black is put up for it.
    pub fn appeared(&mut self, pid: u32) {
        self.alive
            .insert(pid, Tracked { launched: false, past_first_frame: true, closed_by_hold: false, windowed: false });
        self.say(format!("pid {pid} appeared"));
    }

    /// A Dashboard has exited with `code`.
    pub fn exited(&mut self, pid: u32, code: u32, now: Duration) -> Orders {
        let Some(process) = self.alive.remove(&pid) else { return Orders::default() };
        let left: Vec<u32> = self.alive.iter().filter(|(_, t)| t.windowed).map(|(pid, _)| *pid).collect();
        let names = |left: &[u32]| {
            if left.is_empty() {
                "nothing".to_string()
            } else {
                left.iter().map(|pid| format!("pid {pid}")).collect::<Vec<_>>().join(", ")
            }
        };
        if code == contract::NOT_A_DASHBOARD || !process.windowed {
            self.say(format!(
                "pid {pid} exited with code {code:#x} as another of this exe's jobs, not a Dashboard; left: {}",
                names(&left)
            ));
            if left.is_empty() && std::mem::take(&mut self.quit_pending) {
                self.say("no Dashboard left after the last one's exit 0: Exit to desktop; it stays down".into());
                return Orders { start_desktop: true, ..Orders::default() };
            }
            return Orders::default();
        }
        let before_first_frame = process.launched && !process.past_first_frame;
        let crash = !process.closed_by_hold && (code != 0 || (before_first_frame && left.is_empty()));
        let judgement = if process.closed_by_hold {
            "closed by the View+Menu hold, so not a crash"
        } else if code != 0 {
            "a crash"
        } else if before_first_frame && left.is_empty() {
            "before its first frame, so a crash"
        } else if left.is_empty() {
            "Exit to desktop"
        } else {
            "a handover"
        };
        self.say(format!("pid {pid} exited with code {code:#x}: {judgement}; left: {}", names(&left)));
        if crash {
            self.count_crash(now);
        }
        if self.start.is_some_and(|(start, _)| start == pid) {
            self.start = None;
        }
        if !left.is_empty() {
            self.quit_pending = !crash && !process.closed_by_hold;
            self.lift_black(&format!("its Dashboard exited, leaving {}", names(&left)));
            return Orders::default();
        }
        self.quit_pending = false;
        if crash && !self.fallen_back {
            return self.relaunch();
        }
        self.lift_black("the Dashboard is down");
        if crash {
            self.say("the Dashboard stays down, as the --shell process has fallen back".into());
            return Orders::default();
        }
        self.say("the Dashboard stays down; the Guide button brings it back".into());
        // Exit to desktop before explorer was up has a desktop all the same (F14).
        Orders { start_desktop: !process.closed_by_hold, ..Orders::default() }
    }

    /// The Guide button went down. While a Dashboard runs the button is its own; while none does it starts one,
    /// and the crash count starts again.
    pub fn guide_pressed(&mut self) -> Orders {
        if self.running() {
            return Orders::default();
        }
        let why = if self.fallen_back { "fallen back" } else { "none runs" };
        self.say(format!("Guide button with no Dashboard ({why}): starting it; the crash count starts again"));
        self.crashes.clear();
        self.fallen_back = false;
        if self.line_until.take().is_some() {
            self.say("fallback line down: the Guide button tries again".into());
        }
        self.relaunch()
    }

    /// View+Menu held for 5 s: every Dashboard is asked to close, and ended after 10 s; the desktop starts. Not a
    /// crash: it stays down.
    pub fn held(&mut self, now: Duration) -> Orders {
        let closing: Vec<u32> = self.alive.keys().copied().collect();
        for tracked in self.alive.values_mut() {
            tracked.closed_by_hold = true;
        }
        self.say(format!(
            "View+Menu hold: closing {} Dashboard(s), ending them after {} s; starting the desktop",
            closing.len(),
            CLOSE_FOR.as_secs()
        ));
        if !closing.is_empty() {
            self.close_until = Some(now + CLOSE_FOR);
        }
        self.lift_black("the View+Menu hold");
        if self.line_until.take().is_some() {
            self.say("fallback line down: the View+Menu hold".into());
        }
        Orders { start_desktop: true, close: closing, ..Orders::default() }
    }

    /// Every poll: the black's limit, the end of the fallback's line, and the hold's time to close.
    pub fn tick(&mut self, now: Duration) -> Orders {
        let mut orders = Orders::default();
        if let Some((pid, began)) = self.start {
            if self.black && now.saturating_sub(began) >= BLACK_LIMIT {
                // Its Start-up may never signal, so from here its exits are judged as after a first frame.
                if let Some(tracked) = self.alive.get_mut(&pid) {
                    tracked.past_first_frame = true;
                }
                self.lift_black(&format!("no first frame by the {} s limit", BLACK_LIMIT.as_secs()));
            }
        }
        if self.line_until.is_some_and(|until| now >= until) {
            self.line_until = None;
            self.say("fallback line down; starting the desktop".into());
            orders = orders.and(Orders { start_desktop: true, ..Orders::default() });
        }
        if self.close_until.is_some_and(|until| now >= until) {
            self.close_until = None;
            let end: Vec<u32> = self.alive.iter().filter(|(_, t)| t.closed_by_hold).map(|(pid, _)| *pid).collect();
            if !end.is_empty() {
                self.say(format!("still up {} s after the View+Menu hold: ending them", CLOSE_FOR.as_secs()));
                orders = orders.and(Orders { end, ..Orders::default() });
            }
        }
        orders
    }

    fn relaunch(&mut self) -> Orders {
        self.say("starting the Dashboard at once, behind the black".into());
        self.black = true;
        Orders { launch: true, ..Orders::default() }
    }

    fn count_crash(&mut self, now: Duration) {
        self.crashes.push(now);
        self.crashes.retain(|at| now.saturating_sub(*at) <= CRASH_SPAN);
        let count = self.crashes.len();
        self.say(format!(
            "{count} crash{} in the last {} min",
            if count == 1 { "" } else { "es" },
            CRASH_SPAN.as_secs() / 60
        ));
        if self.fallen_back || count < CRASH_LIMIT {
            return;
        }
        self.fallen_back = true;
        self.line_until = Some(now + LINE_FOR);
        self.say(format!(
            "fallback: the line on the black for {} s, then the desktop, and nothing more started on its own",
            LINE_FOR.as_secs()
        ));
    }

    fn lift_black(&mut self, why: &str) {
        if self.black {
            self.black = false;
            self.say(format!("black down: {why}"));
        }
    }
}

/// When explorer starts at a sign-in: as the Boot video ends; without that, 20 s after the first frame; without
/// either, at the 60 s limit. Once.
#[derive(Debug, Default)]
pub struct SignInSchedule {
    first_frame_at: Option<Duration>,
    video_ended: bool,
    done: bool,
}

pub const EXPLORER_LIMIT: Duration = Duration::from_secs(60);
pub const AFTER_FIRST_FRAME: Duration = Duration::from_secs(20);

impl SignInSchedule {
    pub fn first_frame(&mut self, now: Duration) {
        self.first_frame_at.get_or_insert(now);
    }

    pub fn boot_video_ended(&mut self) {
        self.video_ended = true;
    }

    /// Why explorer is due now, once.
    pub fn start_explorer(&mut self, now: Duration) -> Option<String> {
        if self.done {
            return None;
        }
        let at = self.first_frame_at.map_or(EXPLORER_LIMIT, |first| first + AFTER_FIRST_FRAME);
        let why = if self.video_ended {
            "the Boot video ended".to_string()
        } else if now < at {
            return None;
        } else if self.first_frame_at.is_some() {
            format!("no Boot video end {} s after the first frame", AFTER_FIRST_FRAME.as_secs())
        } else {
            format!("no first frame by the {} s limit", EXPLORER_LIMIT.as_secs())
        };
        self.done = true;
        Some(why)
    }
}

/// Explorer once it is the shell: Windows usually brings a dead explorer back itself, so it is started only when
/// there has been no shell window for 10 s, and not again for 10 s more.
#[derive(Debug, Default)]
pub struct ShellWatch {
    was_shell: bool,
    gone_since: Option<Duration>,
}

pub const SHELL_GRACE: Duration = Duration::from_secs(10);

impl ShellWatch {
    pub fn start_explorer(&mut self, shell_up: bool, now: Duration) -> bool {
        if shell_up {
            self.was_shell = true;
            self.gone_since = None;
            return false;
        }
        if !self.was_shell {
            return false;
        }
        let since = *self.gone_since.get_or_insert(now);
        if now.saturating_sub(since) < SHELL_GRACE {
            return false;
        }
        self.gone_since = Some(now);
        true
    }
}

/// What a pad's buttons did, as the --shell process reads them.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PadPresses {
    /// The Guide button went down.
    pub guide: bool,
    /// View and Menu have been held together for 5 s (once, until let go).
    pub hold: bool,
}

pub const MENU: u16 = 0x0010;
pub const VIEW: u16 = 0x0020;
pub const GUIDE: u16 = 0x0400;
pub const HOLD_FOR: Duration = Duration::from_secs(5);

/// Each pad's buttons, read every 30 ms: a Guide press as it goes down, and View+Menu held for 5 s.
#[derive(Debug, Default)]
pub struct PadWatch {
    was: [u16; 4],
    held_since: [Option<Duration>; 4],
    fired: [bool; 4],
    said: Vec<String>,
}

impl PadWatch {
    pub fn take_said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// Pad `pad`'s buttons, none while it isn't connected.
    pub fn read(&mut self, pad: usize, buttons: Option<u16>, now: Duration) -> PadPresses {
        let buttons = buttons.unwrap_or(0);
        let then = std::mem::replace(&mut self.was[pad], buttons);
        let mut presses = PadPresses::default();
        if buttons & GUIDE != 0 && then & GUIDE == 0 {
            self.said.push(format!("pad {pad}: Guide button down"));
            presses.guide = true;
        }
        if buttons & (VIEW | MENU) != VIEW | MENU {
            if let (Some(since), false) = (self.held_since[pad], self.fired[pad]) {
                self.said
                    .push(format!("pad {pad}: View+Menu let go after {}: nothing", seconds(now.saturating_sub(since))));
            }
            self.held_since[pad] = None;
            self.fired[pad] = false;
            return presses;
        }
        let Some(since) = self.held_since[pad] else {
            self.held_since[pad] = Some(now);
            self.said.push(format!("pad {pad}: View+Menu down"));
            return presses;
        };
        if !self.fired[pad] && now.saturating_sub(since) >= HOLD_FOR {
            self.fired[pad] = true;
            self.said.push(format!("pad {pad}: View+Menu held {} s", HOLD_FOR.as_secs()));
            presses.hold = true;
        }
        presses
    }
}

/// When the Dashboard starts at a sign-in: once Windows' default audio device has stayed the same for 1.5 s, or at
/// 6 s, settled or not. The Boot video's sound opens once as it starts, and nobody has seen whether it survives the
/// TV's HDMI device being chosen under it (docs/research/shell.md, 1 row 3).
#[derive(Debug, Default)]
pub struct AudioSettle {
    seen: bool,
    device: Option<String>,
    since: Duration,
    done: bool,
    said: Vec<String>,
}

pub const AUDIO_SETTLE: Duration = Duration::from_millis(1500);
pub const AUDIO_LIMIT: Duration = Duration::from_secs(6);

impl AudioSettle {
    pub fn take_said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// The default device's id now (none with none); why the Dashboard is due, once.
    pub fn start(&mut self, device: Option<String>, now: Duration) -> Option<String> {
        if self.done {
            return None;
        }
        if !self.seen || device != self.device {
            self.said.push(match &device {
                None => "audio: no default audio device".into(),
                Some(id) => format!("audio: default audio device {}{id}", if self.seen { "now " } else { "" }),
            });
            self.seen = true;
            self.device = device;
            self.since = now;
        }
        let why = if self.device.is_some() && now.saturating_sub(self.since) >= AUDIO_SETTLE {
            format!("audio settled after {} ms", now.as_millis())
        } else if now >= AUDIO_LIMIT {
            let what = if self.device.is_none() { "no default audio device" } else { "audio not settled" };
            format!("{what} after {} s; starting anyway", AUDIO_LIMIT.as_secs())
        } else {
            return None;
        };
        self.done = true;
        Some(why)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn a_start_is_behind_the_black_until_its_first_frame_or_60_s() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(1));
        assert!(dog.black());
        dog.first_frame(s(2));
        assert!(!dog.black());

        let mut dog = Watchdog::default();
        dog.launched(10, s(1));
        assert_eq!(dog.tick(s(30)), Orders::default());
        assert!(dog.black());
        dog.tick(s(61));
        assert!(!dog.black());
        // From then on its exit 0 is judged as after a first frame: Exit to desktop, not a crash.
        let orders = dog.exited(10, 0, s(70));
        assert!(!orders.launch && orders.start_desktop);
    }

    #[test]
    fn exit_to_desktop_stays_down_and_has_a_desktop() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(1));
        dog.first_frame(s(2));
        let orders = dog.exited(10, 0, s(100));
        assert_eq!(orders, Orders { start_desktop: true, ..Orders::default() });
        assert!(!dog.running() && !dog.black());
        // The Guide button brings it back.
        assert!(dog.guide_pressed().launch);
    }

    #[test]
    fn a_crash_relaunches_at_once_and_the_third_in_five_minutes_falls_back() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(0));
        dog.first_frame(s(1));
        assert!(dog.exited(10, 0xC000_0005, s(10)).launch);
        assert!(dog.black(), "the relaunch is behind the black");
        dog.launched(11, s(10));
        // An exit 0 before its first frame is a crash too.
        assert!(dog.exited(11, 0, s(12)).launch);
        dog.launched(12, s(12));
        dog.first_frame(s(13));
        let orders = dog.exited(12, 101, s(20));
        assert!(!orders.launch, "fallen back");
        assert_eq!(dog.line(), Some(FALLBACK_LINE));
        assert!(dog.black());
        assert_eq!(dog.tick(s(24)), Orders::default());
        assert_eq!(dog.tick(s(25)), Orders { start_desktop: true, ..Orders::default() });
        assert!(!dog.black() && dog.line().is_none());
        // The Guide button tries again, the count from nothing.
        assert!(dog.guide_pressed().launch);
        dog.launched(13, s(30));
        assert!(dog.exited(13, 1, s(31)).launch);
    }

    #[test]
    fn crashes_further_apart_than_five_minutes_never_fall_back() {
        let mut dog = Watchdog::default();
        for (i, at) in [0, 200, 400, 600].into_iter().enumerate() {
            dog.launched(i as u32, s(at));
            dog.first_frame(s(at + 1));
            assert!(dog.exited(i as u32, 1, s(at + 100)).launch, "crash {i}");
        }
    }

    #[test]
    fn a_launch_that_fails_counts_as_a_crash() {
        let mut dog = Watchdog::default();
        assert!(dog.launch_failed(s(0)).launch);
        assert!(dog.launch_failed(s(1)).launch);
        assert!(!dog.launch_failed(s(2)).launch);
        assert_eq!(dog.line(), Some(FALLBACK_LINE));
    }

    #[test]
    fn a_handover_watches_the_new_dashboard_and_its_exit_0_is_the_quit() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(0));
        dog.first_frame(s(1));
        dog.appeared(20);
        assert_eq!(dog.windowless(), [20]);
        dog.has_window(20);
        assert!(dog.windowless().is_empty());
        assert_eq!(dog.exited(10, 0, s(5)), Orders::default());
        assert!(dog.running());
        assert_eq!(dog.exited(20, 0, s(50)), Orders { start_desktop: true, ..Orders::default() });
    }

    #[test]
    fn this_exes_other_jobs_count_for_nothing() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(0));
        dog.first_frame(s(1));
        // The sentinel, or --shell-off from a terminal, is up as the Dashboard exits 0: windowless, it is no
        // Dashboard left, and the exit is Exit to desktop.
        dog.appeared(30);
        let orders = dog.exited(10, 0, s(5));
        assert_eq!(orders, Orders { start_desktop: true, ..Orders::default() }, "it was Exit to desktop");
        assert_eq!(dog.exited(30, contract::NOT_A_DASHBOARD, s(6)), Orders::default());
        // --shell-off from a terminal with no Dashboard up, failing (4), or the probe's code: not a crash.
        dog.appeared(32);
        assert!(!dog.running(), "the Guide button still starts the Dashboard");
        assert_eq!(dog.exited(32, 4, s(7)), Orders::default());
        dog.appeared(33);
        assert_eq!(dog.exited(33, contract::PROBE_OK, s(8)), Orders::default());
        assert!(dog.crashes.is_empty());
        // One that comes and goes while the Dashboard runs changes nothing.
        dog.launched(11, s(10));
        dog.first_frame(s(11));
        dog.appeared(31);
        assert_eq!(dog.exited(31, contract::NOT_A_DASHBOARD, s(12)), Orders::default());
        assert!(dog.running());
    }

    #[test]
    fn the_guide_button_belongs_to_a_running_dashboard() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(0));
        assert_eq!(dog.guide_pressed(), Orders::default());
    }

    #[test]
    fn view_and_menu_held_close_the_dashboard_end_it_after_10_s_and_it_stays_down() {
        let mut dog = Watchdog::default();
        dog.launched(10, s(0));
        dog.first_frame(s(1));
        let orders = dog.held(s(100));
        assert_eq!(orders, Orders { start_desktop: true, close: vec![10], ..Orders::default() });
        assert_eq!(dog.tick(s(105)), Orders::default());
        assert_eq!(dog.tick(s(110)), Orders { end: vec![10], ..Orders::default() });
        // Ended, its code is 1: still not a crash.
        assert_eq!(dog.exited(10, 1, s(111)), Orders::default());
        assert!(!dog.running());
    }

    #[test]
    fn explorer_starts_once_at_the_videos_end_or_20_s_after_the_first_frame_or_at_60_s() {
        let mut schedule = SignInSchedule::default();
        schedule.first_frame(s(1));
        assert_eq!(schedule.start_explorer(s(5)), None);
        schedule.boot_video_ended();
        assert_eq!(schedule.start_explorer(s(6)).as_deref(), Some("the Boot video ended"));
        assert_eq!(schedule.start_explorer(s(7)), None);

        let mut schedule = SignInSchedule::default();
        schedule.first_frame(s(1));
        assert_eq!(schedule.start_explorer(s(20)), None);
        assert!(schedule.start_explorer(s(21)).unwrap().contains("after the first frame"));

        let mut schedule = SignInSchedule::default();
        assert_eq!(schedule.start_explorer(s(59)), None);
        assert!(schedule.start_explorer(s(60)).unwrap().contains("60 s limit"));
    }

    #[test]
    fn explorer_is_started_again_only_after_10_s_with_no_shell() {
        let mut watch = ShellWatch::default();
        assert!(!watch.start_explorer(false, s(0)), "never a shell yet: the schedule's");
        assert!(!watch.start_explorer(true, s(1)));
        assert!(!watch.start_explorer(false, s(2)));
        assert!(!watch.start_explorer(false, s(11)));
        assert!(watch.start_explorer(false, s(12)));
        assert!(!watch.start_explorer(false, s(13)));
        assert!(watch.start_explorer(false, s(22)));
    }

    #[test]
    fn the_pad_says_the_guide_as_it_goes_down_and_view_and_menu_once_after_5_s() {
        let mut pad = PadWatch::default();
        assert!(pad.read(0, Some(GUIDE), s(0)).guide);
        assert!(!pad.read(0, Some(GUIDE), s(1)).guide);
        assert!(!pad.read(1, Some(VIEW | MENU), s(10)).hold);
        assert!(!pad.read(1, Some(VIEW | MENU), s(14)).hold);
        assert!(pad.read(1, Some(VIEW | MENU), s(15)).hold);
        assert!(!pad.read(1, Some(VIEW | MENU), s(30)).hold, "once until let go");
        assert!(!pad.read(1, None, s(31)).hold);
        assert!(!pad.read(1, Some(VIEW), s(32)).hold);
    }

    #[test]
    fn the_dashboard_starts_once_the_audio_device_has_settled_or_at_6_s() {
        let mut audio = AudioSettle::default();
        let ms = Duration::from_millis;
        assert_eq!(audio.start(None, ms(0)), None);
        assert_eq!(audio.start(Some("hdmi".into()), ms(500)), None);
        assert_eq!(audio.start(Some("hdmi".into()), ms(1900)), None);
        assert!(audio.start(Some("hdmi".into()), ms(2000)).unwrap().starts_with("audio settled"));

        let mut audio = AudioSettle::default();
        for (i, at) in (0..=60).map(|i| (i, ms(i * 100))) {
            let device = Some(format!("device {}", i % 2));
            if let Some(why) = audio.start(device, at) {
                assert_eq!(at, ms(6000));
                assert!(why.contains("not settled"));
                return;
            }
        }
        panic!("never started");
    }
}
