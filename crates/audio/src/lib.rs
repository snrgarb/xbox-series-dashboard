//! Short sounds, played at once and over each other.
//!
//! A sound is a WAV file read once into memory ([`Audio::load`]) and played by its name ([`Audio::play`]). Playing
//! hands the clip to a mixer thread, which sums whatever is playing and feeds the default sound device through
//! WASAPI in shared mode. The thread and the device are started by the first sound; the device is stopped after a
//! second of silence, and the thread then sleeps until the next sound, so nothing runs while nothing plays. A
//! device that goes, or stops being the default, is replaced by the default one at the next sound. With no device
//! there is no sound, said once in the log; nothing here panics over a device.
//!
//! Anywhere but Windows nothing is played. Reading WAV files ([`wav`]), bringing them to the device's rate and
//! mixing ([`mix`]) and what the thread does around the device ([`engine`]) are plain Rust and tested everywhere.
//!
//! No sound file is part of this repository: [`tones`] makes the ones the tests use.
#[cfg_attr(not(windows), allow(dead_code))]
mod engine;
pub mod mix;
pub mod tones;
pub mod wav;
#[cfg(windows)]
mod win;

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::wav::Clip;

#[cfg_attr(not(windows), allow(dead_code))]
enum Command {
    Play { id: usize, name: Arc<str>, clip: Arc<Clip> },
    Volume(f32),
}

pub struct Audio {
    clips: HashMap<String, (usize, Arc<str>, Arc<Clip>)>,
    /// How many clips have been given an id.
    loaded: usize,
    volume: f32,
    muted: bool,
    log_plays: bool,
    /// The mixer thread, once a sound has been played. `Some(None)`: there is no thread to be had.
    mixer: Option<Option<Sender<Command>>>,
}

impl Default for Audio {
    fn default() -> Self {
        Audio { clips: HashMap::new(), loaded: 0, volume: 1.0, muted: false, log_plays: false, mixer: None }
    }
}

impl Audio {
    /// Nothing loaded, at full volume. Nothing is opened or started until a sound is played.
    pub fn new() -> Audio {
        Audio::default()
    }

    /// Reads the WAV file at `path` and keeps it as `name`, in place of any clip of that name.
    pub fn load(&mut self, name: &str, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        self.add(name, wav::decode(&bytes)?);
        Ok(())
    }

    /// Keeps `clip` as `name`, in place of any clip of that name.
    pub fn add(&mut self, name: &str, clip: Clip) {
        self.clips.insert(name.into(), (self.loaded, name.into(), Arc::new(clip)));
        self.loaded += 1;
    }

    pub fn has(&self, name: &str) -> bool {
        self.clips.contains_key(name)
    }

    /// With this on, every sound played is a line in the log: `audio: played <name>`.
    pub fn log_plays(&mut self, on: bool) {
        self.log_plays = on;
    }

    /// The volume every sound plays at, 0 to 1. At 0 nothing is played at all.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { 1.0 };
        self.send_volume();
    }

    /// Muted, nothing is played, and what is playing falls silent.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.send_volume();
    }

    /// What sounds are mixed at: nothing when muted.
    fn level(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    fn send_volume(&mut self) {
        if let Some(Some(mixer)) = &self.mixer {
            let _ = mixer.send(Command::Volume(self.level()));
        }
    }

    /// Starts the clip called `name`, over whatever else is playing; the same clip can play over itself. Returns
    /// at once: the mixer thread does the rest. A name with no clip is silent, as is everything when muted or with
    /// no device. Says how many seconds of sound it started, for whoever must wait for its end: 0 when silent.
    pub fn play(&mut self, name: &str) -> f32 {
        if self.level() <= 0.0 {
            return 0.0;
        }
        let Some((id, name, clip)) = self.clips.get(name) else { return 0.0 };
        let (level, log_plays) = (self.level(), self.log_plays);
        let mixer = self.mixer.get_or_insert_with(|| start_mixer(level, log_plays));
        let Some(mixer) = mixer else { return 0.0 };
        match mixer.send(Command::Play { id: *id, name: name.clone(), clip: clip.clone() }) {
            Ok(()) => clip.seconds(),
            Err(_) => 0.0,
        }
    }
}

/// The mixer thread, which ends when the `Audio` it belongs to is dropped.
#[cfg(windows)]
fn start_mixer(volume: f32, log_plays: bool) -> Option<Sender<Command>> {
    use std::sync::mpsc::TryRecvError;
    use std::time::{Duration, Instant};

    /// The thread's nap between top-ups of the device while it is running.
    const NAP: Duration = Duration::from_millis(5);

    let (sender, commands) = std::sync::mpsc::channel();
    let thread = std::thread::Builder::new().name("audio".into()).spawn(move || {
        let mut engine = engine::Engine::<win::Wasapi>::new(volume, log_plays);
        loop {
            // With the device stopped there is nothing to do until the next sound.
            let command = if engine.running() {
                match commands.try_recv() {
                    Ok(command) => Some(command),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return,
                }
            } else {
                match commands.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                }
            };
            match command {
                Some(Command::Play { id, name, clip }) => engine.play(id, &name, &clip, Instant::now()),
                Some(Command::Volume(volume)) => engine.set_volume(volume),
                None => {
                    engine.tick(Instant::now());
                    std::thread::sleep(NAP);
                }
            }
        }
    });
    match thread {
        Ok(_) => Some(sender),
        Err(e) => {
            eprintln!("audio: no sound (no mixer thread: {e})");
            None
        }
    }
}

#[cfg(not(windows))]
fn start_mixer(_: f32, log_plays: bool) -> Option<Sender<Command>> {
    if log_plays {
        eprintln!("audio: no sound on this system");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_loaded_by_name_and_what_is_not_a_sound_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        tones::write(dir.path(), &["one.wav", "two.wav"]).unwrap();
        std::fs::write(dir.path().join("broken.wav"), b"not a sound").unwrap();

        let mut audio = Audio::new();
        audio.load("one", &dir.path().join("one.wav")).unwrap();
        assert!(audio.has("one") && !audio.has("two"));
        assert!(audio.load("broken", &dir.path().join("broken.wav")).is_err());
        assert!(audio.load("missing", &dir.path().join("missing.wav")).is_err());
        assert!(!audio.has("broken") && !audio.has("missing"));

        // The tones are 0.15 s each, a semitone apart.
        let (one, two) = (audio.clips["one"].2.clone(), std::fs::read(dir.path().join("two.wav")).unwrap());
        assert_eq!((one.rate, one.frames.len()), (44_100, 6_615));
        assert_ne!(one.frames, wav::decode(&two).unwrap().frames);
    }

    /// Playing never waits and never panics, whatever there is to play on: here, where a test has no device or
    /// (anywhere but Windows) no way to one.
    #[cfg(not(windows))]
    #[test]
    fn playing_is_safe_with_nothing_to_play_on() {
        let mut audio = Audio::new();
        audio.add("tone", tones::tone(440.0, 0.01, 44_100));
        audio.play("tone");
        audio.play("no such sound");
        audio.set_muted(true);
        audio.play("tone");
        audio.set_volume(f32::NAN);
        assert_eq!(audio.level(), 0.0);
        audio.set_muted(false);
        assert_eq!(audio.level(), 1.0);
    }
}
