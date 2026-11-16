//! The Idle screen (docs/spec/idle-screen.md): with no input for a while the screen dims, over everything but
//! Toasts, and later shows things over the dim, the console's idle panel. The first input after only wakes it.
//!
//! [`Idle`] is the stage machine, told the time (the Dashboard's clock) and what input there was: the presses the
//! Dashboard gets, and the moment of the last input anywhere in Windows that the shell's clock saw (`input_at`),
//! keyboard, mouse and pad alike. It reads no clock of its own and is tested with none. [`Times`] are Settings'
//! two times; [`cards`] is what the panel shows; [`draw`] draws the dim and the panel.
pub mod cards;
mod draw;

use std::time::{Duration, Instant};

use ui::{Button, Press};

pub use self::cards::Card;
pub use self::draw::draw;

/// How long a wake seen only by the clock (a mouse, a stick, a key in another window) waits for a press of the pad
/// or a key to come to the Dashboard, which is then the wake press and swallowed; with none, it wakes all the same.
/// The Bundle's `WakeGrace`.
pub const GRACE: Duration = Duration::from_millis(500);

/// How long each card of the panel shows before the next (the Bundle's `IdleCardShownFor`).
pub const CARD_FOR: Duration = Duration::from_secs(20);

/// The settings table's keys for the two times: seconds, or `never`.
pub const DIM_KEY: &str = "idle.dim_after";
pub const SHOW_KEY: &str = "idle.show_things_after";

const fn minutes(n: u64) -> Option<Duration> {
    Some(Duration::from_secs(n * 60))
}

/// The steps both of Settings > Idle's dropdowns offer, in order: 1, 2, 5, 10, 15 and 30 min, 1, 2, 3 and 4 h, then
/// None for Never.
pub const STEPS: [Option<Duration>; 11] = [
    minutes(1),
    minutes(2),
    minutes(5),
    minutes(10),
    minutes(15),
    minutes(30),
    minutes(60),
    minutes(120),
    minutes(180),
    minutes(240),
    None,
];

/// Whether `a` is shorter than `b`, Never (None) longer than any time.
fn shorter(a: Option<Duration>, b: Option<Duration>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a < b,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// A step as its dropdown shows it: "1 min", "30 min", "1 h", "4 h", "Never". A time under a minute, which only
/// `--idle-after` gives, in seconds: "3 s".
pub fn describe(step: Option<Duration>) -> String {
    match step.map(|step| step.as_secs()) {
        None => "Never".into(),
        Some(secs) if secs < 60 => format!("{secs} s"),
        Some(secs) if secs < 3600 => format!("{} min", secs / 60),
        Some(secs) => format!("{} h", secs / 3600),
    }
}

/// Settings > Idle's two times: how long with no input before the screen dims, and before it shows things. Never is
/// None. Show things after is never shorter than Dim after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Times {
    pub dim_after: Option<Duration>,
    pub show_after: Option<Duration>,
}

impl Default for Times {
    /// 2 min and 5 min, as the Bundle's.
    fn default() -> Self {
        Self { dim_after: minutes(2), show_after: minutes(5) }
    }
}

impl Times {
    /// Dim after set to `dim`: Show things after raised with it when it would be shorter. None for a time that is
    /// not a step, or the one set already: nothing changes.
    pub fn with_dim(self, dim: Option<Duration>) -> Option<Times> {
        if !STEPS.contains(&dim) || dim == self.dim_after {
            return None;
        }
        let show_after = if shorter(self.show_after, dim) { dim } else { self.show_after };
        Some(Times { dim_after: dim, show_after })
    }

    /// Show things after set to `show`. None for a time shorter than Dim after, not a step, or the one set already.
    pub fn with_show(self, show: Option<Duration>) -> Option<Times> {
        if !STEPS.contains(&show) || shorter(show, self.dim_after) || show == self.show_after {
            return None;
        }
        Some(Times { show_after: show, ..self })
    }

    /// The steps Show things after's dropdown offers: those not shorter than Dim after, Never always among them.
    pub fn show_steps(self) -> Vec<Option<Duration>> {
        STEPS.into_iter().filter(|&step| !shorter(step, self.dim_after)).collect()
    }

    /// The times as a settings table has them: `stored` gives a key's value, or none for one never set. A value that
    /// is not a step is its default, and a Show things after shorter than Dim after is raised to it.
    pub fn read(stored: impl Fn(&str) -> Option<String>) -> Times {
        let defaults = Times::default();
        let step = |key: &str, default: Option<Duration>| match stored(key) {
            Some(text) => parse_step(&text).filter(|step| STEPS.contains(step)).unwrap_or(default),
            None => default,
        };
        let dim_after = step(DIM_KEY, defaults.dim_after);
        let show_after = step(SHOW_KEY, defaults.show_after);
        let show_after = if shorter(show_after, dim_after) { dim_after } else { show_after };
        Times { dim_after, show_after }
    }

    /// The two settings rows to save: a key and its value each, seconds or `never`.
    pub fn saved(self) -> [(&'static str, String); 2] {
        [(DIM_KEY, write_step(self.dim_after)), (SHOW_KEY, write_step(self.show_after))]
    }

    /// `--idle-after <dim s>,<show s>`: the two in seconds, either `never`. A test's own, at any length; Show things
    /// after shorter than Dim after is raised to it.
    pub fn parse_seconds(text: &str) -> Result<Times, String> {
        let bad = || format!("bad --idle-after {text}: <dim s>,<show s>, either of them never");
        let (dim, show) = text.split_once(',').ok_or_else(bad)?;
        let (dim_after, show_after) = (parse_step(dim).ok_or_else(bad)?, parse_step(show).ok_or_else(bad)?);
        let show_after = if shorter(show_after, dim_after) { dim_after } else { show_after };
        Ok(Times { dim_after, show_after })
    }

    /// What the log says of them: `dim after 2 min, show things after 5 min`.
    pub fn describe(self) -> String {
        format!("dim after {}, show things after {}", describe(self.dim_after), describe(self.show_after))
    }
}

/// A step as saved, `120` or `never`: None for text that is neither, Some(None) for Never.
fn parse_step(text: &str) -> Option<Option<Duration>> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("never") {
        return Some(None);
    }
    text.parse::<f64>()
        .ok()
        .filter(|secs| secs.is_finite() && *secs >= 0.0)
        .map(|secs| Some(Duration::from_secs_f64(secs)))
}

fn write_step(step: Option<Duration>) -> String {
    step.map_or("never".into(), |step| step.as_secs().to_string())
}

/// Where the Idle screen is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Awake,
    /// The dim, after Dim after with no input.
    Dimmed,
    /// The panel over the dim, after Show things after with no input.
    ShowingThings,
}

/// What woke the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woken {
    /// A press of a pad's button or a key that stands for one: swallowed.
    Button(Button),
    /// The Guide button going down: its tap or hold is swallowed with it.
    Guide,
    /// A key that types, in a page that takes text: swallowed.
    Typing,
    /// Input that only the shell's clock saw: the mouse, a stick or a trigger inside what makes a press, a key in
    /// another window. Nothing of it was the Dashboard's to swallow.
    Input,
}

impl std::fmt::Display for Woken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Woken::Button(button) => write!(f, "{button:?}"),
            Woken::Guide => f.write_str("Guide"),
            Woken::Typing => f.write_str("typing"),
            Woken::Input => f.write_str("input"),
        }
    }
}

/// What happened, for the log: `idle: dimmed`, `idle: showing things`, `idle: woken by A`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum News {
    Dimmed,
    ShowingThings,
    Woken(Woken),
}

impl std::fmt::Display for News {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            News::Dimmed => f.write_str("dimmed"),
            News::ShowingThings => f.write_str("showing things"),
            News::Woken(by) => write!(f, "woken by {by}"),
        }
    }
}

/// What came of a press.
#[derive(Debug, Clone, PartialEq)]
pub enum Pressed {
    /// Nothing of the Idle screen's: the press goes on as it would. It counted as input.
    Pass,
    /// Part of the press that woke the screen (its repeat, its release): it does nothing more.
    Swallowed,
    /// It woke the screen, and does nothing more; the card that showed as it did, for Y's "See game".
    Woke(Option<Box<Card>>),
}

/// Where the Guide button that woke the screen is in its press, whose tap or hold is swallowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuideWake {
    None,
    Down,
    Held,
}

/// The cards of one Showing things, in their turn's order, and which shows.
#[derive(Debug, Clone, Default)]
struct Turn {
    cards: Vec<Card>,
    at: usize,
    /// When the card showing began to.
    since: Option<Instant>,
    /// The number of the first card of this Showing things.
    first: u64,
}

pub struct Idle {
    times: Times,
    /// Counting from here: the later of the last input and the last time the count started again.
    from: Instant,
    stage: Stage,
    /// Start-up or the Launch screen is up: nothing is counted, and the count starts again as it goes.
    waiting: bool,
    /// Until when a wake seen by the clock alone waits for a press to be the wake press.
    grace: Option<Instant>,
    /// The pad's button or key whose press woke the screen: its repeats and its release are swallowed too.
    held: Option<Button>,
    guide: GuideWake,
    news: Vec<News>,
    turn: Turn,
    /// How many cards have shown since the start: each card's own number, which the panel's crossfade goes by.
    shown: u64,
    /// The card showing as the screen last woke, and its number: drawn while the panel fades with the dim.
    last: Option<(u64, Card)>,
    /// The card before the one showing, fading out under it.
    pub(crate) previous: Option<Card>,
    /// What the cards are shuffled with: xorshift's state.
    seed: u64,
    /// How much of the dim showed when last drawn: it fades out after the screen wakes, and the Overlay stays up
    /// until it has.
    pub(crate) showing: f32,
    /// Each Showing things' own number, so the panel flies in afresh each time.
    pub(crate) session: u64,
}

impl Idle {
    pub fn new(now: Instant) -> Self {
        Self {
            times: Times::default(),
            from: now,
            stage: Stage::Awake,
            waiting: false,
            grace: None,
            held: None,
            guide: GuideWake::None,
            news: Vec::new(),
            turn: Turn::default(),
            shown: 0,
            last: None,
            previous: None,
            seed: 0x9E37_79B9_7F4A_7C15,
            showing: 0.0,
            session: 0,
        }
    }

    pub fn times(&self) -> Times {
        self.times
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// The cards are shuffled from here: the window's, some of the time it starts; a test's is the same every run.
    pub fn seed(&mut self, seed: u64) {
        self.seed = seed.max(1);
    }

    /// New times, in force at once: the count starts again from `now`.
    pub fn set_times(&mut self, times: Times, now: Instant) {
        self.times = times;
        self.restart(now);
    }

    /// Whether the screen is anything but awake, or still fading: the Overlay is wanted for it.
    pub fn shown(&self) -> bool {
        self.stage != Stage::Awake || self.showing > 0.0
    }

    /// What has happened since this was last asked.
    pub fn take_news(&mut self) -> Vec<News> {
        std::mem::take(&mut self.news)
    }

    /// The card showing and its number, while Showing things; the last one, once the screen has woken, while the
    /// dim fades.
    pub fn card(&self) -> Option<(u64, &Card)> {
        if self.stage == Stage::ShowingThings {
            let card = self.turn.cards.get(self.turn.at)?;
            return Some((self.shown, card));
        }
        self.last.as_ref().map(|(number, card)| (*number, card))
    }

    /// Whether the card showing is the first of this Showing things: it flies in with the panel, whole, rather
    /// than fading in.
    pub(crate) fn first_card(&self) -> bool {
        self.shown == self.turn.first
    }

    /// The count starts again from `now`, the screen awake.
    fn restart(&mut self, now: Instant) {
        self.from = now;
        self.grace = None;
        if self.stage != Stage::Awake {
            self.leave();
        }
    }

    /// The screen is awake: the card showing is kept to fade out with the dim.
    fn leave(&mut self) {
        if self.stage == Stage::ShowingThings {
            self.last = self.turn.cards.get(self.turn.at).cloned().map(|card| (self.shown, card));
        }
        self.stage = Stage::Awake;
        self.turn = Turn::default();
    }

    fn wake(&mut self, by: Woken, now: Instant) -> Option<Card> {
        let card = (self.stage == Stage::ShowingThings).then(|| self.turn.cards.get(self.turn.at).cloned()).flatten();
        self.news.push(News::Woken(by));
        self.restart(now);
        card
    }

    /// Whether Start-up or the Launch screen is up. While it is nothing counts; as it begins the screen wakes, and
    /// as it ends the count starts again from zero (the Bundle's: a game launched counts from its Launch screen
    /// going).
    pub fn wait(&mut self, waiting: bool, now: Instant) {
        if waiting != self.waiting {
            self.waiting = waiting;
            self.restart(now);
        }
    }

    /// The shell's clock saw input at `at`, anywhere in Windows. Awake, the count goes from there. Dimmed, it wakes
    /// the screen once `GRACE` has passed with no press to be the wake press (`press`, `guide_down`, `typed`), which
    /// the input may have been and not yet have come as.
    pub fn input_at(&mut self, at: Instant, now: Instant) {
        let at = at.min(now);
        if at <= self.from {
            return;
        }
        if self.stage == Stage::Awake {
            self.from = at;
        } else if self.grace.is_none() {
            self.grace = Some(now + GRACE);
        }
    }

    /// The time is `now`: the stage moves on if its time has come, the panel's card turns, and a wake that waited
    /// out its grace is done. `pool` is the cards to show, asked for as Showing things begins. True when what is
    /// drawn changed.
    pub fn tick(&mut self, now: Instant, pool: impl FnOnce() -> Vec<Card>) -> bool {
        if self.waiting {
            return false;
        }
        if let Some(due) = self.grace {
            if now >= due {
                self.wake(Woken::Input, now);
                return true;
            }
            return false;
        }
        let idle = now.saturating_duration_since(self.from);
        let past = |after: Option<Duration>| after.is_some_and(|after| idle >= after);
        let target = if past(self.times.show_after) {
            Stage::ShowingThings
        } else if past(self.times.dim_after) {
            Stage::Dimmed
        } else {
            Stage::Awake
        };
        let mut changed = false;
        if target > self.stage {
            if self.stage == Stage::Awake {
                self.news.push(News::Dimmed);
                self.last = None;
            }
            if target == Stage::ShowingThings {
                self.news.push(News::ShowingThings);
                self.session += 1;
                self.previous = None;
                let mut cards = pool();
                self.shuffle(&mut cards, None);
                self.shown += 1;
                self.turn = Turn { cards, at: 0, since: Some(now), first: self.shown };
            }
            self.stage = target;
            changed = true;
        }
        if self.stage == Stage::ShowingThings && self.turn.cards.len() > 1 {
            if let Some(since) = self.turn.since.filter(|&since| now >= since + CARD_FOR) {
                self.previous = self.turn.cards.get(self.turn.at).cloned();
                self.turn.at += 1;
                if self.turn.at == self.turn.cards.len() {
                    let last = self.turn.cards.last().cloned();
                    let mut cards = std::mem::take(&mut self.turn.cards);
                    self.shuffle(&mut cards, last.as_ref());
                    self.turn.cards = cards;
                    self.turn.at = 0;
                }
                // The next card's time from when this one was due, so a late frame takes nothing from it; after a
                // long sleep, from now.
                let due = since + CARD_FOR;
                self.turn.since = Some(if now >= due + CARD_FOR { now } else { due });
                self.shown += 1;
                changed = true;
            }
        }
        changed
    }

    /// How long from `now` until `tick` has something to do: None when nothing is coming.
    pub fn due(&self, now: Instant) -> Option<Duration> {
        if self.waiting {
            return None;
        }
        if let Some(due) = self.grace {
            return Some(due.saturating_duration_since(now));
        }
        let at = |after: Option<Duration>| after.map(|after| (self.from + after).saturating_duration_since(now));
        match self.stage {
            Stage::Awake => at(self.times.dim_after).into_iter().chain(at(self.times.show_after)).min(),
            Stage::Dimmed => at(self.times.show_after),
            Stage::ShowingThings if self.turn.cards.len() > 1 => {
                self.turn.since.map(|since| (since + CARD_FOR).saturating_duration_since(now))
            }
            Stage::ShowingThings => None,
        }
    }

    /// A press of a pad's button, or a key that stands for one, at `now`. While the screen is dimmed it is the wake
    /// press: it wakes it and does nothing more, and nor do its repeats or its release.
    pub fn press(&mut self, press: Press, now: Instant) -> Pressed {
        if self.stage != Stage::Awake {
            self.held = Some(press.button);
            return Pressed::Woke(self.wake(Woken::Button(press.button), now).map(Box::new));
        }
        if self.held == Some(press.button) {
            if press.repeat {
                return Pressed::Swallowed;
            }
            // Pressed afresh: it was let go, whether or not anyone said so (a key has no release).
            self.held = None;
        }
        self.from = self.from.max(now);
        Pressed::Pass
    }

    /// A pad's button let go: true when it was the wake press's, which does nothing more.
    pub fn release(&mut self, button: Button) -> bool {
        if self.held == Some(button) {
            self.held = None;
            return true;
        }
        false
    }

    /// The Guide button went down: while the screen is dimmed, the wake press, whose tap or hold is swallowed as it
    /// comes. True when it woke the screen.
    pub fn guide_down(&mut self, now: Instant) -> bool {
        if self.stage != Stage::Awake {
            self.guide = GuideWake::Down;
            self.wake(Woken::Guide, now);
            return true;
        }
        self.guide = GuideWake::None;
        self.from = self.from.max(now);
        false
    }

    /// The Guide button that woke the screen has been held: swallowed, its release with it.
    pub fn guide_held(&mut self) -> bool {
        if self.guide == GuideWake::None {
            return false;
        }
        self.guide = GuideWake::Held;
        true
    }

    /// The Guide button let go: swallowed if it woke the screen. After a hold that is the end of its press; after a
    /// tap, its tap is still to come.
    pub fn guide_release(&mut self) -> bool {
        match self.guide {
            GuideWake::None => false,
            GuideWake::Down => true,
            GuideWake::Held => {
                self.guide = GuideWake::None;
                true
            }
        }
    }

    /// The Guide button's tap: swallowed if the press woke the screen.
    pub fn guide_tap(&mut self) -> bool {
        let woke = self.guide != GuideWake::None;
        self.guide = GuideWake::None;
        woke
    }

    /// A key that types, at `now`: true when it woke the screen, and does nothing more.
    pub fn typed(&mut self, now: Instant) -> bool {
        if self.stage != Stage::Awake {
            self.wake(Woken::Typing, now);
            return true;
        }
        self.from = self.from.max(now);
        false
    }

    /// Fisher-Yates with xorshift; `last`, the card shown last, never first after, so no card shows twice running.
    fn shuffle(&mut self, cards: &mut [Card], last: Option<&Card>) {
        for i in (1..cards.len()).rev() {
            let j = (self.random() % (i as u64 + 1)) as usize;
            cards.swap(i, j);
        }
        if cards.len() > 1 && last.is_some_and(|last| cards[0] == *last) {
            let j = 1 + (self.random() % (cards.len() as u64 - 1)) as usize;
            cards.swap(0, j);
        }
    }

    fn random(&mut self) -> u64 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed = x;
        x
    }
}

#[cfg(test)]
mod tests;
