//! Toasts: the banner low on the screen that tells of something new, above every window and taking nothing from
//! whatever is in front (docs/spec/guide.md, section 9). One shows at a time, for five seconds; those behind it
//! wait their turn, half a second after it goes. Opening the Guide drops them all.
//!
//! This is the queue and its timing, and the notifications list the Guide's Notifications Flyout shows. The banner
//! draws itself in `banner`. The queue is told how much time has passed (`advance`) and has no clock of its own, so
//! a test steps it as it likes.
//!
//! It is also told when its picture has been put on the screen (`presented`), and goes by that: a Toast's five
//! seconds begin when its first frame is out, its going when the first frame of that is, and the next one waits
//! until a frame without the last is. What the queue says and what is on the screen are then the same thing however
//! slowly frames come.
//!
//! An Achievement unlocked Toast is the console's pill instead (`pill`, docs/spec/achievement-toast.md), shown for
//! longer and going for longer, and made from the unlocks the Dashboard is given (`achievement`).
pub mod achievement;
mod banner;
pub mod pill;

use std::collections::VecDeque;

use ui::Ui;

/// How long a Toast shows, counted from when its first frame is on the screen.
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
    /// "Queued": a game joined the Queue (docs/spec/manage-queue.md). The game's cover, else the download arrow. A
    /// tap opens Manage Queue, and those of its kind waiting are let go.
    Queued,
    /// "Ready to play": a game the Queue followed is installed. Its cover, else the play triangle; its prompt says
    /// "to start", and a tap starts it.
    Installed,
    /// "<Name> was uninstalled". Its cover, else the bin. A tap opens the Guide.
    Uninstalled,
    /// An install stopped, or a game left the Queue without installing (#230): why, in a few words ("Not enough
    /// space"). Its cover, else the download arrow. A tap opens Manage Queue, where its row says the rest.
    Stopped,
    /// "Achievement unlocked", or "<n> achievements unlocked": the green pill with the achievement's icon, or the
    /// trophy. A tap opens Profile & system.
    Achievement,
    /// "Rare Achievement unlocked - <n>%": one Rare achievement told of alone, the blue pill with the spinning
    /// diamond and a sound of its own. A tap opens Profile & system.
    RareAchievement,
    /// A Direct Message (docs/spec/party-chats.md, 7.1): the sender's avatar round, else Discord's mark. A tap opens
    /// the Guide on that conversation, focus on the reply box.
    DiscordDm,
    /// Someone mentioned the user in a server's channel (7.2): Discord's mark. A tap opens the Guide on the Discord tab.
    DiscordMention,
    /// A DM call is ringing (7.3): the caller's avatar, else Discord's mark. Shows up to 30 s. A tap opens the
    /// Guide on the Discord tab, focus on Answer.
    DiscordCall,
}

/// How long a ring's Toast shows: the Bundle's `RingToastDuration`.
pub const RING_SECONDS: f64 = 30.0;

impl ToastKind {
    /// Whether its line is what someone wrote or who they are: the log says the kind and no more.
    pub fn is_private(self) -> bool {
        matches!(self, ToastKind::DiscordDm | ToastKind::DiscordMention | ToastKind::DiscordCall)
    }

    /// What the prompt says after the Guide button's mark.
    pub fn prompt(self) -> &'static str {
        match self {
            ToastKind::Installed => "to start",
            _ => "to view",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub kind: ToastKind,
    /// Its line.
    pub text: String,
    /// Whether its line takes turns with "Press (Guide) to view": not a greeting's, and none with the Guide off.
    pub prompt: bool,
    /// The greeting's tile: the picture of the Profile it greets.
    pub picture: Option<model::Picture>,
    /// The game it tells of, by its library id: a Queue Toast's.
    pub game: Option<i64>,
    /// The game whose cover its tile shows, where it has one: its art is picked for the screen as it is drawn.
    pub art: Option<Box<model::Game>>,
    /// What an Achievement unlocked Toast tells of, beyond its line.
    pub told: Option<Box<achievement::Told>>,
    /// A Discord Toast's DM, by its channel: what a burst, a ring's end and the open conversation go by.
    pub channel: Option<model::discord_dms::Id>,
    /// What a DM Toast's line says before its ": ..." (the sender, "in {group}"), and how many messages it tells of.
    pub who: Option<(String, u32)>,
    /// A DM's or a caller's avatar, from memory.
    pub avatar: Option<model::discord_dms::Picture>,
}

impl Toast {
    /// A Toast with only a line, of a kind with no game.
    pub fn new(kind: ToastKind, text: impl Into<String>, prompt: bool) -> Self {
        Self {
            kind,
            text: text.into(),
            prompt,
            picture: None,
            game: None,
            art: None,
            told: None,
            channel: None,
            who: None,
            avatar: None,
        }
    }

    /// Whether it is the pill, an achievement's, rather than the banner.
    pub fn is_pill(&self) -> bool {
        matches!(self.kind, ToastKind::Achievement | ToastKind::RareAchievement)
    }

    /// How long it shows, from its first frame, before it goes: five seconds, an achievement's longer.
    pub fn shown_seconds(&self) -> f64 {
        match self.kind {
            ToastKind::Achievement | ToastKind::RareAchievement => {
                let description = self.told.as_ref().and_then(|told| told.description.as_deref());
                f64::from(pill::shown_for(self.kind == ToastKind::RareAchievement, description))
            }
            ToastKind::DiscordCall => RING_SECONDS,
            _ => SHOWN_SECONDS,
        }
    }

    /// How long it takes to go, on the screen meanwhile.
    pub fn going_seconds(&self) -> f64 {
        if self.is_pill() {
            f64::from(pill::OUT_SECONDS)
        } else {
            GOING_SECONDS
        }
    }

    /// How long after it begins to go the next one waiting shows: the gap, after an achievement's pill has gone.
    fn next_after(&self) -> f64 {
        if self.is_pill() {
            self.going_seconds() + GAP_SECONDS
        } else {
            GAP_SECONDS
        }
    }

    /// What the log adds after its line and kind: an achievement's name, Score and rarity, else nothing.
    pub fn told_line(&self) -> String {
        self.told.as_ref().map_or_else(String::new, |told| told.line())
    }
}

/// A notification, as the Notifications Flyout lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Its own for as long as the Dashboard runs: 1 for the first, counting up.
    pub id: u64,
    pub text: String,
    /// The game an Achievement unlocked notification is of, by its row in the library database: A on it opens
    /// that game's Achievements page. None for any other.
    pub game: Option<i64>,
}

/// The Toast on screen.
struct Banner {
    toast: Toast,
    /// When its first frame was put on the screen, on the queue's clock. None until one has been: its time has not
    /// begun.
    since: Option<f64>,
    /// It is time for it to go, or it was sent away.
    going: bool,
    /// When the first frame of its going was put on the screen. None until one has been: it is seen to go however
    /// late that is.
    going_since: Option<f64>,
}

/// What the screen has of the Toasts, by the frames presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Nothing,
    /// A Toast, come or coming in.
    Showing,
    /// A Toast on its way out, or one that has gone and is still in the last frame presented.
    Going,
}

#[derive(Default)]
pub struct Toasts {
    /// Seconds told of so far.
    clock: f64,
    waiting: VecDeque<Toast>,
    banner: Option<Banner>,
    /// When the next one waiting shows: the gap after one that went with others behind it.
    next_at: Option<f64>,
    /// The last Toast has gone and no frame without it has been put on the screen yet: it is still there.
    uncleared: bool,
    /// How many Toasts have shown.
    shown: u64,
    /// Every notification posted, oldest first.
    notifications: Vec<Notification>,
}

impl Toasts {
    /// Adds a notification to the list, and gives its id. Its Toast is `post`'s business.
    pub fn record(&mut self, text: &str) -> u64 {
        self.record_of(text, None)
    }

    /// The same, for a notification of a game's achievements: A on it in the Flyout opens their page.
    pub fn record_of(&mut self, text: &str, game: Option<i64>) -> u64 {
        let id = self.notifications.last().map_or(1, |last| last.id + 1);
        self.notifications.push(Notification { id, text: text.to_string(), game });
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

    /// A Toast comes before every one waiting (a ring's); it shows at once when nothing is showing or being waited out.
    pub fn post_first(&mut self, toast: Toast) -> bool {
        self.waiting.push_front(toast);
        let free = self.showing().is_none() && self.next_at.is_none();
        free && self.show_next()
    }

    /// A Toast waiting, of this kind, for this channel.
    pub fn waiting_for(&self, kind: ToastKind, channel: &model::discord_dms::Id) -> Option<&Toast> {
        self.waiting.iter().find(|toast| toast.kind == kind && toast.channel.as_ref() == Some(channel))
    }

    /// Puts `toast` in the place of the one waiting of its kind for its channel. False when there is none.
    pub fn replace_waiting(&mut self, toast: Toast) -> bool {
        let same = |waiting: &Toast| waiting.kind == toast.kind && waiting.channel == toast.channel;
        match self.waiting.iter_mut().find(|waiting| same(waiting)) {
            Some(waiting) => {
                *waiting = toast;
                true
            }
            None => false,
        }
    }

    /// Whether a Toast of this kind for this channel is showing or waiting.
    pub fn has(&self, kind: ToastKind, channel: &model::discord_dms::Id) -> bool {
        let same = |toast: &Toast| toast.kind == kind && toast.channel.as_ref() == Some(channel);
        self.showing().is_some_and(same) || self.waiting_for(kind, channel).is_some()
    }

    /// A Toast of this kind for this channel is taken away: gone from the queue, or sent away if showing.
    pub fn withdraw(&mut self, kind: ToastKind, channel: &model::discord_dms::Id) {
        self.withdraw_where(|toast| toast.kind == kind && toast.channel.as_ref() == Some(channel));
    }

    /// Takes away those waiting that `same` picks, and the one showing if it picks that.
    pub fn withdraw_where(&mut self, same: impl Fn(&Toast) -> bool) {
        self.waiting.retain(|toast| !same(toast));
        if self.waiting.is_empty() {
            self.next_at = None;
        }
        if self.showing().is_some_and(same) {
            self.send_away();
        }
    }

    /// The first one waiting takes the banner, from the start of its coming in, over one still going.
    fn show_next(&mut self) -> bool {
        let Some(toast) = self.waiting.pop_front() else { return false };
        self.banner = Some(Banner { toast, since: None, going: false, going_since: None });
        // Its first frame is drawn over whatever was left of the last.
        self.uncleared = false;
        self.shown += 1;
        true
    }

    /// This much time has passed, and nothing else: the clock is put right before `presented`, which may be well
    /// into a turn of the loop that began with `advance`.
    pub fn tick(&mut self, seconds: f32) {
        self.clock += seconds.max(0.0) as f64;
    }

    /// The Toasts' picture as it is now has been put on the screen, or taken off it: whoever shows it says so after
    /// each frame, and after hiding it. A Toast's time begins with its first frame, its going with the first frame
    /// of that, which is what the next one's wait is counted from, and one that has gone is gone.
    pub fn presented(&mut self) {
        match &mut self.banner {
            Some(banner) => {
                banner.since.get_or_insert(self.clock);
                if banner.going && banner.going_since.is_none() {
                    banner.going_since = Some(self.clock);
                    // With none behind it nothing is waited for: one that comes later shows at once.
                    let after = banner.toast.next_after();
                    self.next_at = (!self.waiting.is_empty()).then_some(self.clock + after);
                }
            }
            None => self.uncleared = false,
        }
    }

    /// This much time has passed. True when a Toast began to show. One step does one thing to a Toast: it is
    /// seen to be going before it is gone, and gone from the screen before the next shows, however long the step.
    pub fn advance(&mut self, seconds: f32) -> bool {
        self.clock += seconds.max(0.0) as f64;
        if let Some(banner) = &mut self.banner {
            match (banner.since, banner.going_since) {
                (Some(since), None) if !banner.going && self.clock >= since + banner.toast.shown_seconds() => {
                    banner.going = true
                }
                (_, Some(went)) if self.clock >= went + banner.toast.going_seconds() => {
                    self.banner = None;
                    self.uncleared = true;
                }
                _ => {}
            }
            return false;
        }
        if !self.uncleared && self.next_at.is_some_and(|at| self.clock >= at) {
            self.next_at = None;
            return self.show_next();
        }
        false
    }

    /// Those of `kind` waiting are let go: a tap on one has seen to them all (the Bundle's Queued Toasts, once
    /// Manage Queue is open).
    pub fn drop_waiting(&mut self, kind: ToastKind) {
        self.waiting.retain(|toast| toast.kind != kind);
        if self.waiting.is_empty() {
            self.next_at = None;
        }
    }

    /// The Toast showing goes, as if its time were up: one tapped to do what it says.
    pub fn send_away(&mut self) {
        if let Some(banner) = &mut self.banner {
            match banner.since {
                None => {
                    self.banner = None;
                    self.next_at = (!self.waiting.is_empty()).then_some(self.clock);
                }
                Some(_) => banner.going = true,
            }
        }
    }

    /// The Guide opened: the Toast showing goes, and those waiting are forgotten.
    pub fn drop_all(&mut self) {
        self.waiting.clear();
        self.next_at = None;
        match &mut self.banner {
            // Not yet on the screen: there is nothing to see go.
            Some(banner) if banner.since.is_none() => self.banner = None,
            Some(banner) => banner.going = true,
            None => {}
        }
    }

    /// The Toast showing: not one that is going.
    pub fn showing(&self) -> Option<&Toast> {
        self.banner.as_ref().filter(|banner| !banner.going).map(|banner| &banner.toast)
    }

    /// The Toast on screen, one that is going too: there is something to draw.
    pub fn on_screen(&self) -> Option<&Toast> {
        self.banner.as_ref().map(|banner| &banner.toast)
    }

    /// How many wait behind the one showing.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// The kinds of those waiting, first first.
    pub fn waiting_kinds(&self) -> Vec<ToastKind> {
        self.waiting.iter().map(|toast| toast.kind).collect()
    }

    /// How many Toasts have shown since the Dashboard started: one more says another began.
    pub fn shown(&self) -> u64 {
        self.shown
    }

    /// Whether the queue waits to hear its picture is on the screen: a Toast's first frame, the first of its
    /// going, or the first without one that has gone. Whoever shows Toasts has a frame to draw, or a window to hide.
    pub fn awaits_frame(&self) -> bool {
        let unseen = |banner: &Banner| banner.since.is_none() || (banner.going && banner.going_since.is_none());
        self.uncleared || self.banner.as_ref().is_some_and(unseen)
    }

    /// What the screen has of the Toasts, by the frames presented: what the log says, and when.
    pub fn seen(&self) -> Seen {
        match &self.banner {
            None if self.uncleared => Seen::Going,
            None => Seen::Nothing,
            Some(banner) if banner.since.is_none() => Seen::Nothing,
            Some(banner) if banner.going_since.is_none() => Seen::Showing,
            Some(_) => Seen::Going,
        }
    }

    /// How many Toasts have had a frame on the screen.
    pub fn seen_count(&self) -> u64 {
        self.shown - u64::from(self.banner.as_ref().is_some_and(|banner| banner.since.is_none()))
    }

    /// Whether a Toast is on the screen or coming: one showing, going, not yet cleared, or waiting.
    pub fn busy(&self) -> bool {
        self.banner.is_some() || self.uncleared || !self.waiting.is_empty()
    }

    /// How long until the queue next changes with nothing posted and nothing presented: a Toast goes, is gone, or
    /// the next shows. None while it waits for a frame, too.
    pub fn wake(&self) -> Option<f32> {
        let at = match &self.banner {
            Some(banner) => match banner.going_since {
                None if banner.going => return None,
                None => banner.since? + banner.toast.shown_seconds(),
                Some(went) => went + banner.toast.going_seconds(),
            },
            None if self.uncleared => return None,
            None => self.next_at?,
        };
        Some((at - self.clock).max(0.0) as f32)
    }

    /// The banner, where it is in its coming, its turns of text and its going.
    pub fn draw(&self, ui: &mut Ui) {
        if let Some(banner) = &self.banner {
            let age = banner.since.map_or(0.0, |since| (self.clock - since) as f32);
            let going = banner.going.then(|| banner.going_since.map_or(0.0, |went| (self.clock - went) as f32));
            if banner.toast.is_pill() {
                pill::draw(ui, &banner.toast, age, going);
            } else {
                banner::draw(ui, &banner.toast, age, going);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(text: &str) -> Toast {
        Toast::new(ToastKind::Notification, text, true)
    }

    fn showing(toasts: &Toasts) -> Option<&str> {
        toasts.showing().map(|toast| toast.text.as_str())
    }

    /// A turn of a loop that puts each frame on the screen at once: what there is now, the time passing, what
    /// there is then.
    fn pass(toasts: &mut Toasts, seconds: f32) -> bool {
        toasts.presented();
        let began = toasts.advance(seconds);
        toasts.presented();
        began
    }

    #[test]
    fn a_toast_shows_for_five_seconds_and_the_next_half_a_second_after_it_goes() {
        let mut toasts = Toasts::default();
        assert!(toasts.post(toast("one")));
        assert!(!toasts.post(toast("two")));
        assert_eq!((showing(&toasts), toasts.waiting()), (Some("one"), 1));
        toasts.presented();
        assert_eq!(toasts.wake(), Some(5.0));

        assert!(!pass(&mut toasts, 4.9));
        assert_eq!(showing(&toasts), Some("one"));
        // Going, from 5.1 s: no longer showing, still on screen.
        assert!(!pass(&mut toasts, 0.2));
        assert_eq!(showing(&toasts), None);
        assert_eq!(toasts.on_screen().map(|toast| toast.text.as_str()), Some("one"));
        // Gone 0.41 s on, and nothing shows until half a second after it went.
        assert!(!pass(&mut toasts, 0.45));
        assert!(toasts.on_screen().is_none());
        assert!((toasts.wake().unwrap() - 0.05).abs() < 1e-3);
        assert!(pass(&mut toasts, 0.06));
        assert_eq!((showing(&toasts), toasts.waiting(), toasts.shown()), (Some("two"), 0, 2));

        // The last one goes and nothing is left to wake for.
        pass(&mut toasts, 5.0);
        pass(&mut toasts, 0.5);
        assert!(toasts.on_screen().is_none());
        assert_eq!(toasts.wake(), None);
    }

    #[test]
    fn a_toasts_time_begins_when_its_first_frame_is_on_the_screen() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        // However long that takes: it is showing, waits for its frame, and wakes nobody.
        assert!(!toasts.advance(30.0));
        assert_eq!((showing(&toasts), toasts.awaits_frame(), toasts.wake()), (Some("one"), true, None));
        assert_eq!((toasts.seen(), toasts.seen_count()), (Seen::Nothing, 0));
        // The frame is out well into the turn: the clock is put right first.
        toasts.tick(2.0);
        toasts.presented();
        assert_eq!((toasts.seen(), toasts.seen_count()), (Seen::Showing, 1));
        assert_eq!((toasts.awaits_frame(), toasts.wake()), (false, Some(5.0)));
        assert!(!toasts.advance(4.9));
        assert_eq!(showing(&toasts), Some("one"));
    }

    #[test]
    fn the_next_does_not_show_until_the_last_is_off_the_screen() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        toasts.presented();
        // Its going waits for a frame too.
        toasts.advance(5.0);
        assert!(!toasts.advance(3.0));
        assert!(toasts.on_screen().is_some() && toasts.awaits_frame());
        toasts.presented();
        toasts.advance(0.45);
        // Gone, and no frame has said so: the card is still there, and the second waits past its half second.
        assert!(toasts.on_screen().is_none() && toasts.awaits_frame());
        assert!(!toasts.advance(3.0));
        assert_eq!((showing(&toasts), toasts.wake()), (None, None));
        toasts.presented();
        assert!(toasts.advance(0.0));
        assert_eq!(showing(&toasts), Some("two"));
    }

    #[test]
    fn a_loop_that_stood_still_misses_nothing_out() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        // Nine seconds in one step: the first is seen to go, then is gone, then the second shows, a step each.
        assert!(!pass(&mut toasts, 9.0));
        assert_eq!(toasts.on_screen().map(|toast| toast.text.as_str()), Some("one"));
        assert_eq!(showing(&toasts), None);
        assert!(!pass(&mut toasts, 9.0));
        assert!(toasts.on_screen().is_none());
        assert!(pass(&mut toasts, 9.0));
        // And has its own five seconds, from its own first frame.
        assert!(!pass(&mut toasts, 4.9));
        assert_eq!(showing(&toasts), Some("two"));
    }

    #[test]
    fn one_posted_after_the_last_has_gone_shows_at_once_even_over_its_going() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        pass(&mut toasts, 5.1);
        assert_eq!(showing(&toasts), None);
        assert!(toasts.post(toast("two")));
        assert_eq!(showing(&toasts), Some("two"));
    }

    #[test]
    fn one_posted_in_the_gap_waits_it_out_behind_those_before_it() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        pass(&mut toasts, 5.0);
        pass(&mut toasts, 0.45);
        assert!(!toasts.post(toast("three")));
        assert!(pass(&mut toasts, 0.1));
        assert_eq!((showing(&toasts), toasts.waiting()), (Some("two"), 1));
    }

    #[test]
    fn dropping_sends_the_one_showing_away_and_forgets_the_rest() {
        let mut toasts = Toasts::default();
        toasts.post(toast("one"));
        toasts.post(toast("two"));
        pass(&mut toasts, 1.0);
        toasts.drop_all();
        assert_eq!((showing(&toasts), toasts.waiting()), (None, 0));
        assert!(toasts.on_screen().is_some());
        // It goes from the first frame of its going.
        assert_eq!(toasts.wake(), None);
        toasts.presented();
        assert!((toasts.wake().unwrap() - GOING_SECONDS as f32).abs() < 1e-3);
        assert!(!pass(&mut toasts, 0.5));
        assert!(toasts.on_screen().is_none());
        assert_eq!((toasts.wake(), toasts.shown()), (None, 1));

        // One that had not reached the screen has nothing to show of its going.
        toasts.post(toast("three"));
        toasts.drop_all();
        assert!(toasts.on_screen().is_none() && !toasts.awaits_frame());
    }

    #[test]
    fn a_tapped_toast_goes_and_those_of_its_kind_waiting_are_let_go() {
        let mut toasts = Toasts::default();
        toasts.post(Toast::new(ToastKind::Queued, "Queued", true));
        toasts.post(Toast::new(ToastKind::Queued, "Queued", true));
        toasts.post(toast("other"));
        pass(&mut toasts, 1.0);
        toasts.drop_waiting(ToastKind::Queued);
        toasts.send_away();
        assert_eq!((showing(&toasts), toasts.waiting()), (None, 1));
        pass(&mut toasts, 0.5);
        assert!(pass(&mut toasts, 0.5));
        assert_eq!(showing(&toasts), Some("other"));
        assert_eq!(ToastKind::Installed.prompt(), "to start");
        assert_eq!(ToastKind::Queued.prompt(), "to view");
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
