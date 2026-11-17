//! Where the loop's thread spends its time, for --log-frames (#99): every moment from the window's making on is
//! billed to one named part of a turn of the loop, so the time between two frames of a window adds up to the gap
//! between them, part by part. A late frame is then said with what its gap went on:
//!
//! `frame late: overlay at 41230 ms, gap 25.0 ms: most in Windows 14.9; home wait 8.1, overlay draw 1.5, ...`
//!
//! The loop's own time, between the handlers winit calls, is `asleep` up to the moment the loop asked to be woken
//! at, and `in Windows` past it: messages dispatched, the thread kept from running. Everything else is the
//! Dashboard's, by the part of the turn it was in.
use std::time::{Duration, Instant};

/// A part of a turn of the loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The loop asleep, as it was asked to be.
    Asleep,
    /// The loop's own time past the moment it was to wake: Windows dispatching messages, or not running the thread.
    Windows,
    /// Window events other than Home's frame: keys, focus, a resize and the surface configured for it, a wake.
    Events,
    /// The turn's start: updates, the time told, the Toasts' posts, the pad read.
    Updates,
    /// The Overlay and the Toast strip made: windows, composition and swap chains.
    MakeOverlay,
    /// The sources' messages: the library read, the Queue, art found, achievements, helpers, a launcher set up.
    Sources,
    /// A new snapshot of the library read from its database and given to the Dashboard.
    Library,
    /// A launch followed, and the keys the Overlay took.
    Launch,
    /// The presses given to the Dashboard.
    Input,
    /// What the Dashboard asked for carried out.
    Effects,
    /// The Audio Flyout, media sessions, Discord, the Guide's hand-back and the game held still.
    Follow,
    /// The Overlay shown, hidden, or given the foreground.
    OverlayShown,
    OverlayWait,
    OverlayDraw,
    OverlayPresent,
    /// The Toast strip followed: shown, hidden, kept above, drawn.
    Toasts,
    HomeWait,
    HomeDraw,
    HomePresent,
    /// What follows Home's present: the log's Start-up lines, the first frame's start of the sources.
    HomeAfter,
    /// The turn's end: the Boot video followed, the frames said, the next wake worked out.
    End,
}

const PARTS: usize = 21;

impl Part {
    const ALL: [Part; PARTS] = [
        Part::Asleep,
        Part::Windows,
        Part::Events,
        Part::Updates,
        Part::MakeOverlay,
        Part::Sources,
        Part::Library,
        Part::Launch,
        Part::Input,
        Part::Effects,
        Part::Follow,
        Part::OverlayShown,
        Part::OverlayWait,
        Part::OverlayDraw,
        Part::OverlayPresent,
        Part::Toasts,
        Part::HomeWait,
        Part::HomeDraw,
        Part::HomePresent,
        Part::HomeAfter,
        Part::End,
    ];

    /// As the log says it.
    pub fn name(self) -> &'static str {
        match self {
            Part::Asleep => "asleep",
            Part::Windows => "in Windows",
            Part::Events => "window events",
            Part::Updates => "updates",
            Part::MakeOverlay => "making the Overlay",
            Part::Sources => "sources",
            Part::Library => "library snapshot",
            Part::Launch => "launch",
            Part::Input => "input",
            Part::Effects => "effects",
            Part::Follow => "follow",
            Part::OverlayShown => "overlay shown or hidden",
            Part::OverlayWait => "overlay wait",
            Part::OverlayDraw => "overlay draw",
            Part::OverlayPresent => "overlay present",
            Part::Toasts => "toasts",
            Part::HomeWait => "home wait",
            Part::HomeDraw => "home draw",
            Part::HomePresent => "home present",
            Part::HomeAfter => "home after present",
            Part::End => "turn end",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The time billed to each part so far.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals([Duration; PARTS]);

impl Totals {
    /// What was billed after `before`, part by part.
    pub fn since(&self, before: &Totals) -> Spent {
        Spent(std::array::from_fn(|i| self.0[i].saturating_sub(before.0[i])))
    }
}

/// The time each part took over a stretch: between two frames of a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spent([Duration; PARTS]);

impl Spent {
    pub fn of(&self, part: Part) -> Duration {
        self.0[part.index()]
    }

    /// The parts that took a tenth of a millisecond or more, longest first: `most in Windows 14.9; home wait 8.1,
    /// overlay draw 1.5`.
    pub fn describe(&self) -> String {
        let ms = |time: Duration| time.as_secs_f64() * 1000.0;
        let mut parts: Vec<(Part, Duration)> = Part::ALL
            .iter()
            .map(|&part| (part, self.of(part)))
            .filter(|(_, time)| *time >= Duration::from_micros(100))
            .collect();
        parts.sort_by_key(|&(_, time)| std::cmp::Reverse(time));
        let mut said = String::new();
        for (n, (part, time)) in parts.iter().enumerate() {
            match n {
                0 => said.push_str(&format!("most {} {:.1}", part.name(), ms(*time))),
                1 => said.push_str(&format!("; {} {:.1}", part.name(), ms(*time))),
                _ => said.push_str(&format!(", {} {:.1}", part.name(), ms(*time))),
            }
        }
        if said.is_empty() {
            said.push_str("nothing over 0.1 ms");
        }
        said
    }
}

/// Bills the loop's thread's time to the part of the turn it is in.
pub struct Ledger {
    totals: Totals,
    part: Part,
    /// Since when the time not yet billed runs.
    mark: Instant,
    /// When the loop was last asked to wake.
    wake_at: Instant,
}

impl Ledger {
    pub fn new(now: Instant) -> Self {
        Self { totals: Totals::default(), part: Part::End, mark: now, wake_at: now }
    }

    fn bill(&mut self, part: Part, now: Instant) {
        self.totals.0[part.index()] += now.saturating_duration_since(self.mark);
        self.mark = now.max(self.mark);
    }

    /// The time since the last bill goes to the part under way, and `part` is under way from now. Returns the part
    /// that was, for a part within another to hand back to it.
    pub fn enter(&mut self, part: Part) -> Part {
        self.enter_at(part, Instant::now())
    }

    pub fn enter_at(&mut self, part: Part, now: Instant) -> Part {
        self.bill(self.part, now);
        std::mem::replace(&mut self.part, part)
    }

    /// A handler of the loop's begins, with `part`: the time since the last one ended was the loop's own, asleep
    /// up to when it was asked to wake and in Windows past that.
    pub fn back(&mut self, part: Part) {
        self.back_at(part, Instant::now());
    }

    pub fn back_at(&mut self, part: Part, now: Instant) {
        let asleep = self.wake_at.min(now).max(self.mark);
        self.bill(Part::Asleep, asleep);
        self.bill(Part::Windows, now);
        self.part = part;
    }

    /// A handler of the loop's ends: the time from now is the loop's own until the next begins.
    pub fn away(&mut self) {
        self.away_at(Instant::now());
    }

    pub fn away_at(&mut self, now: Instant) {
        self.bill(self.part, now);
        // Billed by the next `back`: a part named here would never be used.
        self.part = Part::Windows;
    }

    /// The loop has been asked to wake at `at`, or sooner on an event.
    pub fn wake_at(&mut self, at: Instant) {
        self.wake_at = at;
    }

    /// Everything billed up to `now`.
    pub fn totals_at(&mut self, now: Instant) -> Totals {
        self.bill(self.part, now);
        self.totals
    }
}

/// How many late frames are said one by one between two of the frames' summaries; the rest are counted.
const LATE_SAID: u32 = 20;

/// Says late frames one by one, with what their gaps went on.
#[derive(Default)]
pub struct LateFrames {
    said: u32,
    unsaid: u32,
}

impl LateFrames {
    /// A frame of `window` came `gap` after the one before, which was still moving: `spent` is what the loop's
    /// thread did meanwhile. `draws` are each window's last draw, by the part its drawing is billed to: what a draw
    /// that took a millisecond or more of the gap spent its time on is said with it.
    /// `frame late: home at 9120 ms, gap 59.0 ms: most home draw 20.4; ...; home's draw: 31 art uploaded in 18.2
    /// ms, declare 1.1 ms, render 0.9 ms, glyphs 0.1 ms`.
    pub fn say(&mut self, window: &str, gap: Duration, spent: &Spent, draws: [(&str, Part, &ui::FrameCost); 2]) {
        if self.said >= LATE_SAID {
            self.unsaid += 1;
            return;
        }
        self.said += 1;
        let mut line = format!(
            "frame late: {window} at {:.0} ms, gap {:.1} ms: {}",
            crate::since_start(),
            gap.as_secs_f64() * 1000.0,
            spent.describe()
        );
        for (name, part, cost) in draws {
            if spent.of(part) >= Duration::from_millis(1) {
                line.push_str(&format!("; {name}'s draw: {}", cost.describe()));
            }
        }
        eprintln!("{line}");
    }

    /// How many late frames went unsaid since the last call, for the frames' summary; they are said one by one
    /// again from here.
    pub fn take_unsaid(&mut self) -> u32 {
        self.said = 0;
        std::mem::take(&mut self.unsaid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn every_moment_between_two_frames_is_billed_to_one_part() {
        let t0 = Instant::now();
        let mut ledger = Ledger::new(t0);
        // A turn: updates, the Overlay's frame, then asked to wake at once.
        ledger.back_at(Part::Updates, t0);
        ledger.enter_at(Part::OverlayWait, t0 + ms(1));
        let first = ledger.totals_at(t0 + ms(2));
        ledger.enter_at(Part::OverlayDraw, t0 + ms(2));
        ledger.enter_at(Part::End, t0 + ms(4));
        ledger.wake_at(t0 + ms(4));
        ledger.away_at(t0 + ms(5));
        // Windows kept the loop 15 ms past the wake it asked for, then Home drew in a turn of its own.
        ledger.back_at(Part::HomeWait, t0 + ms(20));
        ledger.enter_at(Part::HomeDraw, t0 + ms(21));
        ledger.away_at(t0 + ms(23));
        ledger.back_at(Part::Updates, t0 + ms(23));
        ledger.enter_at(Part::OverlayWait, t0 + ms(24));
        let second = ledger.totals_at(t0 + ms(27));
        let spent = second.since(&first);
        let total: Duration = Part::ALL.iter().map(|&part| spent.of(part)).sum();
        assert_eq!(total, ms(25), "the gap, all of it");
        assert_eq!(spent.of(Part::Windows), ms(15));
        assert_eq!(spent.of(Part::Asleep), Duration::ZERO);
        assert_eq!(spent.of(Part::OverlayDraw), ms(2));
        assert_eq!(spent.of(Part::HomeDraw), ms(2));
        assert_eq!(spent.of(Part::OverlayWait), ms(3));
        assert!(spent.describe().starts_with("most in Windows 15.0; overlay wait 3.0, "), "{}", spent.describe());
    }

    #[test]
    fn the_loop_asleep_as_asked_is_asleep_and_past_it_in_windows() {
        let t0 = Instant::now();
        let mut ledger = Ledger::new(t0);
        ledger.back_at(Part::Updates, t0);
        ledger.wake_at(t0 + ms(10));
        ledger.away_at(t0 + ms(1));
        ledger.back_at(Part::Updates, t0 + ms(12));
        let totals = ledger.totals_at(t0 + ms(12));
        let spent = totals.since(&Totals::default());
        assert_eq!((spent.of(Part::Asleep), spent.of(Part::Windows)), (ms(9), ms(2)));
        // Woken early by an event: asleep all the while.
        ledger.wake_at(t0 + ms(100));
        ledger.away_at(t0 + ms(13));
        ledger.back_at(Part::Events, t0 + ms(20));
        let spent = ledger.totals_at(t0 + ms(20)).since(&totals);
        assert_eq!((spent.of(Part::Asleep), spent.of(Part::Windows)), (ms(7), Duration::ZERO));
    }

    #[test]
    fn a_part_within_another_hands_back_to_it() {
        let t0 = Instant::now();
        let mut ledger = Ledger::new(t0);
        ledger.back_at(Part::Effects, t0);
        let was = ledger.enter_at(Part::Library, t0 + ms(1));
        ledger.enter_at(was, t0 + ms(6));
        let spent = ledger.totals_at(t0 + ms(7)).since(&Totals::default());
        assert_eq!((spent.of(Part::Effects), spent.of(Part::Library)), (ms(2), ms(5)));
    }
}
