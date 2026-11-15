//! Toasts: the banner low on the screen that tells of something new, above every window and taking nothing from
//! whatever is in front (docs/spec/guide.md, section 9). One shows at a time, for five seconds; those behind it
//! wait their turn, half a second after it goes. Opening the Guide drops them all.
//!
//! This is the queue and its timing, and the notifications list the Guide's Notifications Flyout shows. The banner
//! draws itself in `banner`. The queue is told how much time has passed (`advance`) and has no clock of its own, so
//! a test steps it as it likes.
mod banner;

use std::collections::VecDeque;

use ui::Ui;

/// How long a Toast shows, counted from when it starts to appear.
pub const SHOWN_SECONDS: f64 = 5.0;
/// How long after one goes the next one waiting shows.
pub const GAP_SECONDS: f64 = 0.5;
/// How long a Toast takes to go: it is still on screen meanwhile.
pub const GOING_SECONDS: f64 = banner::OUT_SECONDS as f64;

/// What a Toast tells of, which picks its tile and where a tap of the Guide button opens the Guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// A notification: the bell. A tap opens Notifications.
    Notification,
    /// "Hi <user>!" at start-up: the profile's picture, sliding in, and no prompt. A tap opens Profile & system.
    Greeting,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub kind: ToastKind,
    /// Its line.
    pub text: String,
    /// Whether its line takes turns with "Press (Guide) to view": not a greeting's, and none with the Guide off.
    pub prompt: bool,
}

/// A notification, as the Notifications Flyout lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Its own for as long as the Dashboard runs: 1 for the first, counting up.
    pub id: u64,
    pub text: String,
}

/// The Toast on screen.
struct Banner {
    toast: Toast,
    /// When it began to appear, on the queue's clock.
    since: f64,
    /// When it began to go, if it has.
    going_since: Option<f64>,
}

#[derive(Default)]
pub struct Toasts {
    /// Seconds told of so far.
    clock: f64,
    waiting: VecDeque<Toast>,
    banner: Option<Banner>,
    /// When the next one waiting shows: the gap after one that went with others behind it.
    next_at: Option<f64>,
    /// How many Toasts have shown.
    shown: u64,
    /// Every notification posted, oldest first.
    notifications: Vec<Notification>,
}

impl Toasts {
    /// Adds a notification to the list, and gives its id. Its Toast is `post`'s business.
    pub fn record(&mut self, text: &str) -> u64 {
        let id = self.notifications.last().map_or(1, |last| last.id + 1);
        self.notifications.push(Notification { id, text: text.to_string() });
        id
    }

    /// Every notification posted since the Dashboard started, oldest first.
    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    /// A Toast joins those waiting. True when it shows at once: nothing is showing and no gap is being waited out.
    pub fn post(&mut self, toast: Toast) -> bool {
        self.waiting.push_back(toast);
        let free = self.showing().is_none() && self.next_at.is_none();
        free && self.show_next()
    }

    /// The first one waiting takes the banner, from the start of its coming in, over one still going.
    fn show_next(&mut self) -> bool {
        let Some(toast) = self.waiting.pop_front() else { return false };
        self.banner = Some(Banner { toast, since: self.clock, going_since: None });
        self.shown += 1;
        true
    }

    /// This much time has passed. True when a Toast began to show.
    pub fn advance(&mut self, seconds: f32) -> bool {
        self.clock += seconds.max(0.0) as f64;
        if let Some(banner) = &mut self.banner {
            match banner.going_since {
                None if self.clock >= banner.since + SHOWN_SECONDS => {
                    let went = banner.since + SHOWN_SECONDS;
                    banner.going_since = Some(went);
                    // With none behind it nothing is waited for: one that comes later shows at once.
                    self.next_at = (!self.waiting.is_empty()).then_some(went + GAP_SECONDS);
                }
                _ => {}
            }
            if banner.going_since.is_some_and(|went| self.clock >= went + GOING_SECONDS) {
                self.banner = None;
            }
        }
        if self.next_at.is_some_and(|at| self.clock >= at) {
            self.next_at = None;
            return self.show_next();
        }
        false
    }

    /// The Guide opened: the Toast showing goes, and those waiting are forgotten.
    pub fn drop_all(&mut self) {
        self.waiting.clear();
        self.next_at = None;
        if let Some(banner) = &mut self.banner {
            banner.going_since.get_or_insert(self.clock);
        }
    }

    /// The Toast showing: not one that is going.
    pub fn showing(&self) -> Option<&Toast> {
        self.banner.as_ref().filter(|banner| banner.going_since.is_none()).map(|banner| &banner.toast)
    }

    /// The Toast on screen, one that is going too: there is something to draw.
    pub fn on_screen(&self) -> Option<&Toast> {
        self.banner.as_ref().map(|banner| &banner.toast)
    }

    /// How many wait behind the one showing.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// How many Toasts have shown since the Dashboard started: one more says another began.
    pub fn shown(&self) -> u64 {
        self.shown
    }

    /// How long until the queue next changes with nothing posted: a Toast goes, is gone, or the next shows.
    pub fn wake(&self) -> Option<f32> {
        let banner = self.banner.as_ref().map(|banner| match banner.going_since {
            None => banner.since + SHOWN_SECONDS,
            Some(went) => went + GOING_SECONDS,
        });
        let soonest = [banner, self.next_at].into_iter().flatten().reduce(f64::min)?;
        Some((soonest - self.clock).max(0.0) as f32)
    }

    /// The banner, where it is in its coming, its turns of text and its going.
    pub fn draw(&self, ui: &mut Ui) {
        if let Some(banner) = &self.banner {
            let age = (self.clock - banner.since) as f32;
            let going = banner.going_since.map(|went| (self.clock - went) as f32);
            banner::draw(ui, &banner.toast, age, going);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(text: &str) -> Toast {
        Toast { kind: ToastKind::Notification, text: text.into(), prompt: true }
    }

    fn showing(toasts: &Toasts) -> Option<&str> {
        toasts.showing().map(|toast| toast.text.as_str())
    }

    #[test]
    fn a_toast_shows_for_five_seconds_and_the_next_half_a_second_after_it_goes() {
        let mut toasts = Toasts::default();
        assert!(toasts.post(toast("one")));
        assert!(!toasts.post(toast("two")));
        assert_eq!((showing(&toasts), toasts.waiting()), (Some("one"), 1));
        assert_eq!(toasts.wake(), Some(5.0));

        assert!(!toasts.advance(4.9));
        assert_eq!(showing(&toasts), Some("one"));
        // Going: no longer showing, still on screen.
        assert!(!toasts.advance(0.2));
        assert_eq!(showing(&toasts), None);
        assert_eq!(toasts.on_screen().map(|toast| toast.text.as_str()), Some("one"));
        // Gone by 5.41 s, and nothing shows until 5.5 s.
        assert!(!toasts.advance(0.35));
        assert!(toasts.on_screen().is_none());
        assert!((toasts.wake().unwrap() - 0.05).abs() < 1e-3);
        assert!(toasts.advance(0.05));
        assert_eq!((showing(&toasts), toasts.waiting(), toasts.shown()), (Some("two"), 0, 2));

        // The last one goes and nothing is left to wake for.
        toasts.advance(5.0);
        toasts.advance(0.5);
        assert!(toasts.on_screen().is_none());
        assert_eq!(toasts.wake(), None);
    }

    #[test]
    fn one_posted_after_the_last_has_gone_shows_at_once_even_over_its_going() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.advance(5.1);
        assert_eq!(showing(&toasts), None);
        assert!(toasts.post(toast("two")));
        assert_eq!(showing(&toasts), Some("two"));
    }

    #[test]
    fn one_posted_in_the_gap_waits_it_out_behind_those_before_it() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        toasts.advance(5.2);
        assert!(!toasts.post(toast("three")));
        assert!(toasts.advance(0.35));
        assert_eq!((showing(&toasts), toasts.waiting()), (Some("two"), 1));
    }

    #[test]
    fn a_toast_that_was_late_to_show_still_has_its_five_seconds() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        // The loop stood still for a while: the second begins when it is seen to.
        assert!(toasts.advance(9.0));
        assert_eq!(showing(&toasts), Some("two"));
        assert!(!toasts.advance(4.9));
        assert_eq!(showing(&toasts), Some("two"));
    }

    #[test]
    fn dropping_sends_the_one_showing_away_and_forgets_the_rest() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        toasts.advance(1.0);
        toasts.drop_all();
        assert_eq!((showing(&toasts), toasts.waiting()), (None, 0));
        assert!(toasts.on_screen().is_some());
        assert!((toasts.wake().unwrap() - GOING_SECONDS as f32).abs() < 1e-3);
        assert!(!toasts.advance(0.5));
        assert!(toasts.on_screen().is_none());
        assert_eq!((toasts.wake(), toasts.shown()), (None, 1));
    }

    #[test]
    fn notifications_are_kept_in_the_order_they_came() {
        let mut toasts = Toasts::default();
        assert_eq!(toasts.record("first"), 1);
        assert_eq!(toasts.record("second"), 2);
        let texts: Vec<&str> = toasts.notifications().iter().map(|n| n.text.as_str()).collect();
        assert_eq!(texts, ["first", "second"]);
    }
}
