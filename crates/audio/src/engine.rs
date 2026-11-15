//! What the mixer's thread does, apart from the device itself: opens the device for the first sound, keeps a
//! little of the mix ahead of what it is playing, stops it after a second of silence, and opens it again when it
//! has gone or is no longer the default one. The device is an [`Output`], so all of this runs under a test with a
//! made-up one.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::mix::{resample, Frames, Mixer};
use crate::wav::Clip;

/// How much sound is kept queued in the device while something plays: what a late wake-up of the thread can eat
/// before the sound breaks up, and the most a sound started over another waits. A sound into silence waits for
/// none of it.
const AHEAD: Duration = Duration::from_millis(30);
/// The device is stopped once nothing has played for this long.
pub const IDLE: Duration = Duration::from_secs(1);

/// A sound device, open and taking 16-bit stereo at a rate of its own choosing.
pub trait Output: Sized {
    /// Opens the default device, stopped. `Err` says which step failed, or that there is none.
    fn open() -> Result<Self, String>;
    /// The device's name, for the log.
    fn name(&self) -> String;
    /// Its format and buffer, for the log.
    fn format(&self) -> String;
    /// Frames a second.
    fn rate(&self) -> u32;
    /// How many frames its buffer holds.
    fn capacity(&self) -> u32;
    /// How many frames it has been given and not yet played.
    fn queued(&mut self) -> Result<u32, String>;
    /// Queues frames: no more than `capacity` less `queued`.
    fn write(&mut self, frames: &[[f32; 2]]) -> Result<(), String>;
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self);
    /// Whether it is still the device sound goes to by default.
    fn is_default(&self) -> bool;
}

pub struct Engine<O: Output> {
    device: Option<O>,
    /// The device is started: it is playing what it is given, or silence.
    running: bool,
    mixer: Mixer,
    /// Each clip played so far at the device's rate, by the clip's id.
    resampled: HashMap<usize, Frames>,
    /// The rate `resampled` is at.
    rate: u32,
    /// Since when nothing has been playing and the device has had nothing queued.
    quiet_since: Option<Instant>,
    /// The device could not be opened or started, and that has been said.
    failing: bool,
    /// The device has been opened and not yet started.
    fresh: bool,
    log_plays: bool,
    scratch: Vec<[f32; 2]>,
}

impl<O: Output> Engine<O> {
    pub fn new(volume: f32, log_plays: bool) -> Self {
        Engine {
            device: None,
            running: false,
            mixer: Mixer::new(volume),
            resampled: HashMap::new(),
            rate: 0,
            quiet_since: None,
            failing: false,
            fresh: false,
            log_plays,
            scratch: Vec::new(),
        }
    }

    /// Whether the device is started, and so wants [`tick`](Self::tick) every few milliseconds.
    pub fn running(&self) -> bool {
        self.running
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.mixer.volume = volume;
    }

    /// Starts `clip` now, over whatever is playing. With no device, or one that fails, nothing plays; the next
    /// sound tries again.
    pub fn play(&mut self, id: usize, name: &str, clip: &Arc<Clip>, now: Instant) {
        // Twice: a device that turns out to be gone when it is written to is opened again at once.
        for _ in 0..2 {
            if !self.ready() {
                return;
            }
            let rate = self.rate;
            let frames = self.resampled.entry(id).or_insert_with(|| resample(clip, rate).into()).clone();
            self.mixer.play(frames);
            if self.feed(now) && self.start() {
                if self.log_plays {
                    eprintln!("audio: played {name}");
                }
                return;
            }
        }
    }

    /// Tops the device up with what is playing; stops it when nothing has for a while.
    pub fn tick(&mut self, now: Instant) {
        if self.running {
            self.feed(now);
        }
    }

    /// A device to play on: the one already open, if it is running or still the default; else the default, opened.
    fn ready(&mut self) -> bool {
        if !self.running && self.device.as_ref().is_some_and(|device| !device.is_default()) {
            eprintln!("audio: the default device has changed");
            self.device = None;
        }
        if self.device.is_none() {
            match O::open() {
                Ok(device) => {
                    if device.rate() != self.rate {
                        self.rate = device.rate();
                        self.resampled.clear();
                    }
                    self.device = Some(device);
                    self.fresh = true;
                }
                Err(e) => {
                    self.failed(&format!("no sound ({e})"));
                    return false;
                }
            }
        }
        true
    }

    /// Starts the device if it is stopped. It has been fed first, so it does not start on an empty buffer.
    fn start(&mut self) -> bool {
        let Some(device) = &mut self.device else { return false };
        if self.running {
            return true;
        }
        match device.start() {
            Ok(()) => {
                // Said once for each time the device is opened; after a silence only when sounds are logged.
                if std::mem::take(&mut self.fresh) || self.log_plays {
                    eprintln!("audio: device \"{}\" started, {}", device.name(), device.format());
                }
                self.running = true;
                self.failing = false;
                true
            }
            Err(e) => {
                self.failed(&e);
                false
            }
        }
    }

    /// Queues what is playing, up to [`AHEAD`]. False when the device failed, and has been let go.
    fn feed(&mut self, now: Instant) -> bool {
        let Some(device) = &mut self.device else { return false };
        let step = (|| {
            let queued = device.queued()?;
            if self.mixer.playing() > 0 {
                let ahead = (device.rate() as u64 * AHEAD.as_millis() as u64 / 1000) as u32;
                let room = ahead.min(device.capacity()).saturating_sub(queued) as usize;
                if room > 0 {
                    self.scratch.resize(room, [0.0; 2]);
                    let filled = self.mixer.mix(&mut self.scratch);
                    device.write(&self.scratch[..filled])?;
                }
                self.quiet_since = None;
            } else if queued == 0 && self.running {
                let since = *self.quiet_since.get_or_insert(now);
                if now.duration_since(since) >= IDLE {
                    device.stop();
                    self.running = false;
                    if self.log_plays {
                        eprintln!("audio: device stopped, nothing playing");
                    }
                }
            }
            Ok::<(), String>(())
        })();
        match step {
            Ok(()) => true,
            Err(e) => {
                self.failed(&e);
                false
            }
        }
    }

    /// Lets the device go, to be opened again for the next sound. Said once until a device works again.
    fn failed(&mut self, why: &str) {
        if !self.failing {
            eprintln!("audio: {why}; the next sound tries the default device again");
        }
        self.failing = true;
        self.device = None;
        self.running = false;
        self.quiet_since = None;
        self.mixer.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// The machine the made-up device is on.
    #[derive(Default)]
    struct World {
        /// No device at all.
        none: bool,
        /// Which device is the default.
        default: u32,
        rate: u32,
        opened: u32,
        started: u32,
        stopped: u32,
        /// The next write fails, as a device unplugged does.
        gone: bool,
        queued: u32,
        written: Vec<[f32; 2]>,
    }

    thread_local! {
        static WORLD: RefCell<World> = RefCell::new(World { rate: 1000, ..World::default() });
    }

    fn world<T>(f: impl FnOnce(&mut World) -> T) -> T {
        WORLD.with(|world| f(&mut world.borrow_mut()))
    }

    /// An engine on a machine with one device, at 1000 Hz, that nothing has happened to yet.
    fn engine(volume: f32) -> Engine<Fake> {
        world(|w| *w = World { rate: 1000, ..World::default() });
        Engine::new(volume, false)
    }

    struct Fake {
        id: u32,
        rate: u32,
    }

    impl Output for Fake {
        fn open() -> Result<Fake, String> {
            world(|w| {
                if w.none {
                    return Err("no default audio endpoint".into());
                }
                w.opened += 1;
                w.queued = 0;
                Ok(Fake { id: w.default, rate: w.rate })
            })
        }
        fn name(&self) -> String {
            format!("fake {}", self.id)
        }
        fn format(&self) -> String {
            format!("{} Hz", self.rate)
        }
        fn rate(&self) -> u32 {
            self.rate
        }
        fn capacity(&self) -> u32 {
            100
        }
        fn queued(&mut self) -> Result<u32, String> {
            Ok(world(|w| w.queued))
        }
        fn write(&mut self, frames: &[[f32; 2]]) -> Result<(), String> {
            world(|w| {
                if std::mem::take(&mut w.gone) {
                    return Err("the device is gone".into());
                }
                assert!(w.queued as usize + frames.len() <= 100, "more written than the buffer holds");
                w.queued += frames.len() as u32;
                w.written.extend(frames);
                Ok(())
            })
        }
        fn start(&mut self) -> Result<(), String> {
            world(|w| w.started += 1);
            Ok(())
        }
        fn stop(&mut self) {
            world(|w| w.stopped += 1);
        }
        fn is_default(&self) -> bool {
            world(|w| w.default == self.id)
        }
    }

    /// A clip of `count` frames at the made-up device's rate, every sample `level`.
    fn clip(level: f32, count: usize) -> Arc<Clip> {
        Arc::new(Clip { rate: 1000, frames: vec![[level; 2]; count] })
    }

    /// The device plays `frames` of what it holds.
    fn played(frames: u32) {
        world(|w| w.queued = w.queued.saturating_sub(frames));
    }

    #[test]
    fn the_first_sound_opens_the_device_and_silence_stops_it() {
        let mut engine = engine(1.0);
        let start = Instant::now();
        assert!(!engine.running());
        assert_eq!(world(|w| w.opened), 0, "nothing is opened until something plays");

        // 30 ms at 1000 Hz is 30 frames ahead: the clip's 40 go in two parts.
        engine.play(0, "a", &clip(0.5, 40), start);
        assert!(engine.running());
        assert_eq!(world(|w| (w.opened, w.started, w.written.len())), (1, 1, 30));
        played(10);
        engine.tick(start + Duration::from_millis(10));
        assert_eq!(world(|w| w.written.len()), 40);
        // It was fed before it was started, and what it was fed is the clip.
        assert!(world(|w| w.written.iter().all(|f| *f == [0.5, 0.5])));

        // A second sound while the first's end is still in the device: queued behind it, and no silence after.
        played(10);
        engine.play(1, "b", &clip(0.25, 5), start + Duration::from_millis(20));
        assert_eq!(world(|w| w.written.len()), 45);
        assert!(world(|w| w.written[40..].iter().all(|f| *f == [0.25, 0.25])));

        // Everything played: a second of nothing and it is stopped, but not before.
        played(100);
        let quiet = start + Duration::from_millis(100);
        engine.tick(quiet);
        engine.tick(quiet + IDLE - Duration::from_millis(1));
        assert!(engine.running());
        engine.tick(quiet + IDLE);
        assert!(!engine.running());
        assert_eq!(world(|w| w.stopped), 1);

        // The next sound starts the same device again: it is not opened a second time.
        engine.play(0, "a", &clip(0.5, 40), quiet + IDLE * 2);
        assert!(engine.running());
        assert_eq!(world(|w| (w.opened, w.started)), (1, 2));
    }

    #[test]
    fn with_no_device_nothing_plays_and_the_next_sound_looks_again() {
        let mut engine = engine(1.0);
        world(|w| w.none = true);
        let now = Instant::now();
        engine.play(0, "a", &clip(0.5, 10), now);
        engine.play(0, "a", &clip(0.5, 10), now);
        assert!(!engine.running());
        assert_eq!(world(|w| w.written.len()), 0);
        // A device is plugged in.
        world(|w| w.none = false);
        engine.play(0, "a", &clip(0.5, 10), now);
        assert!(engine.running());
        assert_eq!(world(|w| w.written.len()), 10);
    }

    #[test]
    fn a_device_that_goes_is_opened_again() {
        let mut engine = engine(1.0);
        let now = Instant::now();
        engine.play(0, "a", &clip(0.5, 100), now);
        // Unplugged while playing: the sound is lost, nothing panics.
        played(30);
        world(|w| w.gone = true);
        engine.tick(now + Duration::from_millis(30));
        assert!(!engine.running());
        engine.tick(now + Duration::from_millis(40));
        // The next sound opens the default device afresh and plays from its own beginning.
        let before = world(|w| w.written.len());
        engine.play(1, "b", &clip(0.25, 10), now + Duration::from_millis(50));
        assert!(engine.running());
        assert_eq!(world(|w| w.opened), 2);
        assert!(world(|w| w.written[before..] == [[0.25, 0.25]; 10]));

        // Gone at the very write that starts a sound: opened again at once, and the sound plays.
        played(100);
        world(|w| w.gone = true);
        let before = world(|w| w.written.len());
        engine.play(1, "b", &clip(0.25, 10), now + Duration::from_millis(60));
        assert!(engine.running());
        assert_eq!(world(|w| w.opened), 3);
        assert!(world(|w| w.written[before..] == [[0.25, 0.25]; 10]));
    }

    #[test]
    fn a_new_default_device_is_followed_at_the_next_sound_after_a_silence() {
        let mut engine = engine(1.0);
        let now = Instant::now();
        engine.play(0, "a", &clip(0.5, 10), now);
        played(100);
        engine.tick(now);
        engine.tick(now + IDLE);
        assert!(!engine.running());

        // Headphones become the default, at another rate: the clip is brought to it.
        world(|w| (w.default, w.rate) = (7, 2000));
        let before = world(|w| w.written.len());
        engine.play(0, "a", &clip(0.5, 10), now + IDLE * 2);
        assert!(engine.running());
        assert_eq!(world(|w| w.opened), 2);
        assert_eq!(world(|w| w.written.len()) - before, 20, "10 frames at 1000 Hz are 20 at 2000");
    }

    #[test]
    fn the_volume_is_the_mixers() {
        let mut engine = engine(0.5);
        engine.play(0, "a", &clip(0.5, 2), Instant::now());
        engine.set_volume(0.0);
        played(2);
        engine.play(0, "a", &clip(0.5, 2), Instant::now());
        assert_eq!(world(|w| w.written.clone()), [[0.25, 0.25], [0.25, 0.25], [0.0, 0.0], [0.0, 0.0]]);
    }
}
