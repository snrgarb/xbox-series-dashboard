//! The Game hub's opening, timed (#214): from A on a tile to the hub's first frame and to its last moving frame,
//! and what the time between went on. One line a hub, always, since it costs a few clock readings a frame:
//!
//! `hub: opened Ashfall Courier in 42 ms: first frame 42 ms, last moving frame 218 ms after A; ...`
//!
//! The parts, in the order the line gives them: the frame that answered A; when the Dashboard put the hub up after A
//! (in the turn that took the press: there has been no double tap to wait for since 2026-11-17, docs/spec/game-hub.md
//! 9.1); the page's data (the Dashboard's work
//! in the turn that opened it); what the first frame spent its time on; each picture the hub shows as it opens (a
//! cache hit, or how long it queued, decoded and uploaded, and when it came); the transition's own length (none: the
//! hub shows in one frame, 9.2); and the longest frame while the hub was opening, with what ran in it.
use std::time::{Duration, Instant};

use ui::{ArtCache, ArtKey, ArtTiming, Button, FrameCost, Press};

/// A hub still moving this long after it was put up is said as it is.
const GIVE_UP: Duration = Duration::from_secs(5);
/// An A longer ago than this is not what opened the hub; the hub's line then counts from when it was put up.
const A_TOO_OLD: Duration = Duration::from_secs(3);

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// A frame of the hub's opening: when it began, how long it waited for the swap chain, how long it took in all,
/// what it spent that on.
#[derive(Clone, Copy)]
struct Frame {
    began: Instant,
    wait: Duration,
    took: Duration,
    cost: FrameCost,
    /// Its number among the hub's frames, from 1.
    at: u32,
}

struct Opening {
    game: String,
    /// The pictures the hub shows as it opens, and whether each was ready to draw in its first frame.
    art: Vec<(&'static str, ArtKey, bool)>,
    /// The A it counts from, or when it was put up for want of one.
    from: Instant,
    /// Whether `from` is an A.
    pressed: bool,
    put_up: Instant,
    /// The Dashboard's work in the turn that put it up.
    built: Duration,
    /// The first frame presented after the A.
    answered: Option<Instant>,
    first: Option<Frame>,
    last: Option<Instant>,
    longest: Option<Frame>,
    frames: u32,
}

/// A hub on top: its number, its game's name, and the pictures it shows as it opens.
pub type Hub = (u64, String, Vec<(&'static str, ArtKey)>);

/// Follows hubs as they open, and says how each went once it is at rest.
#[derive(Default)]
pub struct HubOpen {
    /// The last A, and the first frame presented after it.
    a: Option<Instant>,
    answered: Option<Instant>,
    /// The Dashboard's work this turn of the loop.
    spent: Duration,
    /// The hub last seen on top, by its number.
    seen: Option<u64>,
    open: Option<Opening>,
    /// The renderer is the processor (WARP on the test VM): its frames are slow whatever the screen.
    processor: bool,
}

impl HubOpen {
    /// Whether the renderer is the processor, which the line says: a bound on its times means nothing on one.
    pub fn set_processor(&mut self, processor: bool) {
        self.processor = processor;
    }

    /// A new turn of the loop.
    pub fn turn(&mut self) {
        self.spent = Duration::ZERO;
    }

    /// The Dashboard did some of this turn's work, which may be what opened a hub.
    pub fn spent(&mut self, took: Duration) {
        self.spent += took;
    }

    /// A press was given to the Dashboard at `now`.
    pub fn pressed(&mut self, press: Press, now: Instant) {
        if press.button == Button::A && !press.repeat {
            (self.a, self.answered) = (Some(now), None);
        }
    }

    /// The hub on top after this turn's work, if one is (`Dashboard::game_hub_up`), with its pictures
    /// (`screens::game_hub::art_keys`) and whether each is ready: a new one starts being timed. A hub that goes
    /// before it came to rest is said as it was.
    pub fn look(&mut self, hub: Option<Hub>, art: &ArtCache, now: Instant) {
        let serial = hub.as_ref().map(|(serial, ..)| *serial);
        if serial == self.seen {
            return;
        }
        self.seen = serial;
        if let Some(open) = self.open.take() {
            eprintln!("{}", Self::line(&open, art, "it went before it came to rest", self.processor));
        }
        let Some((_, game, keys)) = hub else { return };
        let a = self.a.filter(|a| now.saturating_duration_since(*a) < A_TOO_OLD);
        self.open = Some(Opening {
            game,
            art: keys.into_iter().map(|(name, key)| (name, key, false)).collect(),
            from: a.unwrap_or(now),
            pressed: a.is_some(),
            put_up: now,
            built: self.spent,
            answered: self.answered.filter(|_| a.is_some()),
            first: None,
            last: None,
            longest: None,
            frames: 0,
        });
    }

    /// A frame of Home's window was presented: begun at `began`, given a target at `acquired`, done at `done`;
    /// `moving` when it left something moving or art on its way. True when it ended a hub's opening, whose line has
    /// been said.
    pub fn frame(
        &mut self,
        began: Instant,
        acquired: Instant,
        done: Instant,
        cost: FrameCost,
        moving: bool,
        art: &ArtCache,
    ) -> bool {
        if self.a.is_some() && self.answered.is_none() {
            self.answered = Some(done);
        }
        let Some(open) = &mut self.open else { return false };
        if open.answered.is_none() && open.pressed {
            open.answered = Some(done);
        }
        open.frames += 1;
        let frame = Frame { began, wait: acquired - began, took: done - began, cost, at: open.frames };
        if open.first.is_none() {
            open.first = Some(frame);
            for (_, key, ready) in &mut open.art {
                *ready = art.is_ready(key);
            }
        }
        if open.longest.is_none_or(|longest| frame.took > longest.took) {
            open.longest = Some(frame);
        }
        open.last = Some(done);
        let gave_up = done.saturating_duration_since(open.put_up) >= GIVE_UP;
        if moving && !gave_up {
            return false;
        }
        let open = self.open.take().expect("timed above");
        let how = if moving { "still moving 5 s after it was put up" } else { "at rest" };
        eprintln!("{}", Self::line(&open, art, how, self.processor));
        true
    }

    /// What became of one picture by the hub's last frame.
    fn picture(open: &Opening, name: &str, key: &ArtKey, at_first: bool, art: &ArtCache) -> String {
        let after = |at: Instant| ms(at.saturating_duration_since(open.from));
        let Some(ArtTiming { asked, queued, decoded, uploaded, ready }) = art.timing(key) else {
            return format!("{name}: not asked for");
        };
        let how = format!(
            "queued {:.0} ms, decoded {:.0} ms from disk, uploaded {:.1} ms",
            ms(queued),
            ms(decoded),
            ms(uploaded)
        );
        let failed = ready.is_some() && !art.is_ready(key);
        match ready {
            _ if failed => format!("{name}: could not be read ({how})"),
            Some(_) if at_first && asked < open.from => format!("{name}: cache hit"),
            Some(ready) if at_first => format!(
                "{name}: ready in the first frame, asked {:.0} ms after A ({how}, ready at {:.0} ms)",
                after(asked),
                after(ready)
            ),
            Some(ready) => format!(
                "{name}: came at {:.0} ms, after the first frame ({how}, asked at {:.0} ms)",
                after(ready),
                after(asked)
            ),
            None => format!("{name}: still on its way at the last frame (asked at {:.0} ms)", after(asked)),
        }
    }

    fn line(open: &Opening, art: &ArtCache, how: &str, processor: bool) -> String {
        let how = format!("{how}, drawn on the {}", if processor { "processor" } else { "GPU" });
        let after = |at: Instant| ms(at.saturating_duration_since(open.from));
        let first = open.first.map(|frame| frame.began + frame.took);
        let mut parts = Vec::new();
        let head = match (first, open.last) {
            (Some(first), Some(last)) => format!(
                "hub: opened {} in {:.0} ms: first frame {:.0} ms, last moving frame {:.0} ms after {} ({how})",
                open.game,
                after(first),
                after(first),
                after(last),
                if open.pressed { "A" } else { "it was put up (no A)" },
            ),
            _ => format!("hub: opened {}, and no frame of it was drawn ({how})", open.game),
        };
        if let Some(answered) = open.answered {
            parts.push(format!("A answered by a frame at {:.0} ms", after(answered)));
        }
        if open.pressed {
            parts.push(format!(
                "put up {:.0} ms after A (no double tap's wait: A opens the hub at once, the user's decision of \
                 2026-11-17)",
                after(open.put_up)
            ));
        }
        parts.push(format!(
            "page's data {:.1} ms (the Dashboard's turn that opened it: the library snapshot in memory, no database \
             query; achievements and details from that snapshot, none read from disk or asked of a store)",
            ms(open.built)
        ));
        if let Some(frame) = open.first {
            parts.push(format!(
                "first frame began {:.1} ms after it was put up, took {:.1} ms (swap chain {:.1} ms, {})",
                ms(frame.began.saturating_duration_since(open.put_up)),
                ms(frame.took),
                ms(frame.wait),
                frame.cost.describe()
            ));
        }
        for (name, key, at_first) in &open.art {
            parts.push(Self::picture(open, name, key, *at_first, art));
        }
        if open.art.is_empty() {
            parts.push("no pictures (the game has no art)".into());
        }
        parts.push("logo: none drawn (the hub has none); nothing fetched (the hub fetches no art)".into());
        parts.push("transition: none (the hub shows in one frame, docs/spec/game-hub.md 9.2)".into());
        if let Some(frame) = open.longest {
            parts.push(format!(
                "{} frames, the longest {:.1} ms (frame {}: swap chain {:.1} ms, {})",
                open.frames,
                ms(frame.took),
                frame.at,
                ms(frame.wait),
                frame.cost.describe()
            ));
        }
        format!("{head}; {}", parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(button: Button) -> Press {
        Press { button, repeat: false }
    }

    #[test]
    fn a_hub_is_timed_from_its_a_to_its_first_frame_and_said_once_at_rest() {
        let art = ArtCache::new(|| {});
        let mut hub = HubOpen::default();
        let a = Instant::now();
        let at = |ms: u64| a + Duration::from_millis(ms);
        hub.pressed(press(Button::A), a);
        // The turn that took the press puts the hub up.
        hub.turn();
        hub.spent(Duration::from_micros(300));
        hub.look(Some((7, "Ashfall Courier".into(), Vec::new())), &art, at(1));
        let open = hub.open.as_ref().unwrap();
        assert!(open.pressed && open.answered.is_none() && open.built == Duration::from_micros(300));
        // The hub's first frame answers A, still moving; then one at rest ends it.
        assert!(!hub.frame(at(2), at(3), at(20), FrameCost::default(), true, &art));
        assert_eq!(hub.open.as_ref().unwrap().answered, Some(at(20)));
        assert!(hub.frame(at(30), at(31), at(40), FrameCost::default(), false, &art));
        assert!(hub.open.is_none());
        // The same hub seen again is not a new one.
        hub.look(Some((7, "Ashfall Courier".into(), Vec::new())), &art, at(400));
        assert!(hub.open.is_none());
    }

    #[test]
    fn the_line_says_first_and_last_frames_from_a_and_when_the_hub_was_put_up() {
        let art = ArtCache::new(|| {});
        let mut hub = HubOpen::default();
        let a = Instant::now();
        let at = |ms: u64| a + Duration::from_millis(ms);
        hub.pressed(press(Button::A), a);
        hub.look(Some((1, "Brass Meridian".into(), Vec::new())), &art, at(1));
        let open = hub.open.as_mut().unwrap();
        open.frames = 2;
        let frame = Frame {
            began: at(2),
            wait: Duration::from_millis(1),
            took: Duration::from_millis(18),
            cost: FrameCost::default(),
            at: 1,
        };
        (open.first, open.longest, open.last) = (Some(frame), Some(frame), Some(at(200)));
        let line = HubOpen::line(hub.open.as_ref().unwrap(), &art, "at rest", false);
        assert!(line.starts_with("hub: opened Brass Meridian in 20 ms: first frame 20 ms, last moving frame 200 ms after A (at rest, drawn on the GPU)"), "{line}");
        assert!(line.contains("put up 1 ms after A (no double tap's wait"), "{line}");
        assert!(!line.contains("double tap wait"), "{line}");
        assert!(line.contains("first frame began 1.0 ms after it was put up, took 18.0 ms"), "{line}");
        assert!(line.contains("2 frames, the longest 18.0 ms (frame 1"), "{line}");
    }

    #[test]
    fn a_hub_opened_with_no_a_counts_from_when_it_was_put_up() {
        let art = ArtCache::new(|| {});
        let mut hub = HubOpen::default();
        let now = Instant::now();
        hub.pressed(press(Button::A), now);
        hub.look(Some((3, "Glasshouse".into(), Vec::new())), &art, now + Duration::from_secs(10));
        let open = hub.open.as_ref().unwrap();
        assert!(!open.pressed && open.answered.is_none());
    }
}
