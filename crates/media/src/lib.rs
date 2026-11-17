//! Video, played into the renderer: what the Boot video is (docs/spec/start-up.md), found in
//! docs/research/video-media-foundation.md and first built as the `video-spike` program.
//!
//! [`Video::open`] starts a thread that decodes the file with Media Foundation's Source Reader: each frame as RGB32
//! rows in memory, its sound as PCM to the default sound device through WASAPI. Whoever shows it asks, as each
//! frame of theirs is drawn, which frame is due ([`Video::due`]), writes it into a texture of its own
//! (`render::Renderer::update_video_texture`) and says when the first is on the screen ([`Video::presented`]):
//! the clock starts from it. The clock is the sound's, what the device has played; with no sound it is the
//! system's. A frame that comes due while an earlier one waits replaces it, so a slow screen drops frames and
//! never slows the film.
//!
//! Media Foundation is looked for when a file is opened, never linked to: where it is missing (an N edition of
//! Windows without the Media Feature Pack) a video fails to open, and nothing else changes. Anywhere but Windows
//! every video fails to open. What is decided here about which frame shows when is plain Rust, tested everywhere
//! with a clock of the test's own ([`Clock`]).
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(windows)]
mod win;

/// What a video file holds, once it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picture {
    /// The picture's size in its own pixels: the texture a frame is written into.
    pub size: (u32, u32),
    /// Each pixel's shape, width to height.
    pub pixel_aspect: (u32, u32),
}

impl Picture {
    /// The picture's shape, width over height, its pixels' shape counted in.
    pub fn aspect(&self) -> f32 {
        let wide = self.size.0 as f32 * self.pixel_aspect.0.max(1) as f32;
        let high = self.size.1 as f32 * self.pixel_aspect.1.max(1) as f32;
        if high > 0.0 {
            wide / high
        } else {
            16.0 / 9.0
        }
    }
}

/// One decoded frame.
pub struct Frame {
    /// When it is to be shown, in seconds of the file's own time.
    pub time: f64,
    /// Rows of B, G, R, X bytes from the top, four times the width apart: `Renderer::update_video_texture`'s.
    pub pixels: Vec<u8>,
    /// What the decoder and the conversion to RGB32 took for it, in ms.
    pub decode_ms: f64,
    /// What copying it out of Media Foundation's buffer took, in ms.
    pub copy_ms: f64,
}

/// What the decoding thread says, in order: `Opened` or `Failed`, then frames until `End` or `Failed`.
pub enum Event {
    Opened(Picture),
    Frame(Frame),
    End,
    Failed(String),
}

/// The time the picture is paced by, in seconds of the file's own time, and what the decoding thread counts.
pub trait Clock: Send + Sync {
    /// The time now.
    fn now(&self) -> f64;
    /// The first frame, of time `media`, is on the screen: the clock, and the sound, start from it.
    fn start(&self, media: f64);
    /// Playing is over: the sound stops, and the decoder with it.
    fn close(&self);
    /// How many frames have been decoded.
    fn decoded(&self) -> u64;
}

/// A clock that runs on the system's own from the moment it is started: what a video with no sound is paced by,
/// and a test's.
#[derive(Default)]
pub struct FreeClock {
    started: Mutex<Option<(f64, Instant)>>,
}

impl Clock for FreeClock {
    fn now(&self) -> f64 {
        self.started.lock().unwrap().map_or(0.0, |(media, at)| media + at.elapsed().as_secs_f64())
    }

    fn start(&self, media: f64) {
        *self.started.lock().unwrap() = Some((media, Instant::now()));
    }

    fn close(&self) {}

    fn decoded(&self) -> u64 {
        0
    }
}

/// How a video is played.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Its sound is played. Without, or with no sound device, the picture is paced by the system's clock.
    pub audio: bool,
    /// The sound's volume, 0 to 1.
    pub volume: f32,
    /// Decode on the GPU through a Direct3D 11 device of Media Foundation's own, reading each frame back.
    pub dxva: bool,
    /// The Source Reader's older, software-only YUV to RGB conversion in place of the video processor.
    pub basic_processing: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { audio: true, volume: 1.0, dxva: false, basic_processing: false }
    }
}

/// Where a video is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The file is being opened: nothing is known of it yet.
    Opening,
    /// Open, with frames still to show.
    Playing,
    /// Every frame has been handed out.
    Ended,
    /// It could not be opened or decoded, and says why. Frames already handed out stand.
    Failed(String),
}

/// A video being played. Dropped, it stops: its sound, and the decoding thread at its next frame.
pub struct Video {
    events: Receiver<Event>,
    /// Takes back the pixels of frames done with, for the decoder to fill again.
    recycle: Sender<Vec<u8>>,
    clock: Arc<dyn Clock>,
    picture: Option<Picture>,
    /// The frame decoded and not yet due.
    next: Option<Frame>,
    /// The decoder has nothing more to give.
    ended: bool,
    failed: Option<String>,
    /// The first frame's time, once it has been handed out.
    first: Option<f64>,
    /// Whether the first frame is on screen, and so the clock running.
    started: bool,
    /// Frames handed out, and frames that came due behind a later one and were never shown.
    pub shown: u64,
    pub dropped: u64,
}

impl Video {
    /// Starts playing `file`: it is opened and decoded on a thread of its own, and this returns at once. Anywhere
    /// but Windows it fails at once.
    pub fn open(file: &Path, options: Options) -> Video {
        let (recycle, recycled) = std::sync::mpsc::channel();
        #[cfg(windows)]
        {
            let (events, clock) = win::decode::spawn(file, options, recycled);
            Video::from(events, recycle, clock)
        }
        #[cfg(not(windows))]
        {
            let _ = (file, options, recycled);
            let (events, receiver) = std::sync::mpsc::channel();
            let _ = events.send(Event::Failed("video plays only on Windows".into()));
            Video::from(receiver, recycle, Arc::new(FreeClock::default()))
        }
    }

    /// A video whose events come from `events`, paced by `clock`: what `open` makes, and a test's.
    pub fn from(events: Receiver<Event>, recycle: Sender<Vec<u8>>, clock: Arc<dyn Clock>) -> Video {
        Video {
            events,
            recycle,
            clock,
            picture: None,
            next: None,
            ended: false,
            failed: None,
            first: None,
            started: false,
            shown: 0,
            dropped: 0,
        }
    }

    /// Takes in one event, if one is waiting (`block`: waits for it). False when there was none.
    fn take(&mut self, block: bool) -> bool {
        let event =
            if block { self.events.recv().map_err(|_| TryRecvError::Disconnected) } else { self.events.try_recv() };
        match event {
            Ok(Event::Opened(picture)) => self.picture = Some(picture),
            Ok(Event::Frame(frame)) => self.next = Some(frame),
            Ok(Event::End) => self.ended = true,
            Ok(Event::Failed(why)) => {
                self.failed.get_or_insert(why);
                self.ended = true;
            }
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                if !self.ended && self.picture.is_none() {
                    self.failed.get_or_insert("the decoder stopped before opening the file".into());
                }
                self.ended = true;
                return false;
            }
        }
        true
    }

    /// The picture's size and shape, once the file is open.
    pub fn picture(&mut self) -> Option<Picture> {
        while self.picture.is_none() && !self.ended && self.next.is_none() && self.take(false) {}
        self.picture
    }

    /// Waits for the file to open: its picture, or why it could not be.
    pub fn wait_open(&mut self) -> Result<Picture, String> {
        while self.picture.is_none() && !self.ended {
            self.take(true);
        }
        match (self.picture, &self.failed) {
            (Some(picture), _) => Ok(picture),
            (None, Some(why)) => Err(why.clone()),
            (None, None) => Err("the file ended before it opened".into()),
        }
    }

    /// The frame to show now, if one has come due: the latest whose time the clock has reached, `early` seconds
    /// before it (half a refresh, so it lands on the refresh nearest its time). Earlier ones never shown count as
    /// dropped. The first frame is due as soon as it is decoded, and the clock starts from it
    /// ([`Self::presented`]). Give its pixels back with [`Self::recycle`] once they are in a texture.
    pub fn due(&mut self, early: f64) -> Option<Frame> {
        self.picture();
        let now = self.clock.now();
        let mut due: Option<Frame> = None;
        loop {
            if self.next.is_none() && !self.ended {
                self.take(false);
            }
            let first = self.first.is_none() && due.is_none();
            match self.next.take() {
                Some(frame) if first || (self.started && frame.time <= now + early) => {
                    if let Some(skipped) = due.replace(frame) {
                        self.dropped += 1;
                        self.recycle(skipped.pixels);
                    }
                }
                waiting => {
                    self.next = waiting;
                    break;
                }
            }
        }
        if let Some(frame) = &due {
            self.first.get_or_insert(frame.time);
            self.shown += 1;
        }
        due
    }

    /// A frame's pixels, done with: the decoder fills them again.
    pub fn recycle(&self, pixels: Vec<u8>) {
        let _ = self.recycle.send(pixels);
    }

    /// The first frame [`Self::due`] gave is on the screen: the clock and the sound start from it. Said once; again
    /// it does nothing.
    pub fn presented(&mut self) {
        if let (false, Some(first)) = (self.started, self.first) {
            self.started = true;
            self.clock.start(first);
        }
    }

    pub fn status(&mut self) -> Status {
        self.picture();
        if self.next.is_none() && !self.ended {
            self.take(false);
        }
        match (&self.failed, self.picture) {
            (Some(why), _) => Status::Failed(why.clone()),
            _ if self.ended && self.next.is_none() => Status::Ended,
            (None, None) => Status::Opening,
            (None, Some(_)) => Status::Playing,
        }
    }

    /// The clock: the file's time now.
    pub fn clock(&self) -> f64 {
        self.clock.now()
    }

    /// How many frames the decoder has decoded.
    pub fn decoded(&self) -> u64 {
        self.clock.decoded()
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        self.clock.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Sender};

    /// A clock the test sets.
    #[derive(Default)]
    struct TestClock {
        now: Mutex<f64>,
        started: Mutex<Option<f64>>,
    }

    impl Clock for TestClock {
        fn now(&self) -> f64 {
            *self.now.lock().unwrap()
        }
        fn start(&self, media: f64) {
            *self.started.lock().unwrap() = Some(media);
            *self.now.lock().unwrap() = media;
        }
        fn close(&self) {}
        fn decoded(&self) -> u64 {
            0
        }
    }

    fn frame(time: f64) -> Event {
        Event::Frame(Frame { time, pixels: vec![time as u8; 4], decode_ms: 0.0, copy_ms: 0.0 })
    }

    fn video() -> (Video, Sender<Event>, Arc<TestClock>) {
        let (events, receiver) = channel();
        let clock = Arc::new(TestClock::default());
        let (recycle, _) = channel();
        (Video::from(receiver, recycle, clock.clone()), events, clock)
    }

    const PICTURE: Picture = Picture { size: (4, 2), pixel_aspect: (1, 1) };

    #[test]
    fn the_first_frame_is_due_at_once_and_the_clock_starts_from_it_once_it_is_shown() {
        let (mut video, events, clock) = video();
        assert_eq!(video.status(), Status::Opening);
        events.send(Event::Opened(PICTURE)).unwrap();
        assert_eq!(video.picture(), Some(PICTURE));
        assert_eq!(video.status(), Status::Playing);
        assert!(video.due(0.0).is_none(), "nothing decoded yet");
        events.send(frame(0.5)).unwrap();
        events.send(frame(0.6)).unwrap();
        assert_eq!(video.due(0.0).map(|f| f.time), Some(0.5));
        // Not shown yet: the clock stands, and nothing more comes due.
        assert!(video.due(0.0).is_none());
        video.presented();
        assert_eq!(*clock.started.lock().unwrap(), Some(0.5));
        video.presented();
        assert!(video.due(0.0).is_none(), "0.6 is not due at 0.5");
        assert_eq!(video.due(0.15).map(|f| f.time), Some(0.6), "due early by half a refresh");
    }

    #[test]
    fn of_frames_that_came_due_together_the_latest_shows_and_the_rest_are_dropped() {
        let (mut video, events, clock) = video();
        events.send(Event::Opened(PICTURE)).unwrap();
        for time in [0.0, 0.1, 0.2, 0.3, 0.4] {
            events.send(frame(time)).unwrap();
        }
        assert_eq!(video.due(0.0).map(|f| f.time), Some(0.0));
        video.presented();
        *clock.now.lock().unwrap() = 0.35;
        assert_eq!(video.due(0.0).map(|f| f.time), Some(0.3));
        assert_eq!((video.shown, video.dropped), (2, 2));
        events.send(Event::End).unwrap();
        assert_eq!(video.status(), Status::Playing, "0.4 is still to show");
        *clock.now.lock().unwrap() = 0.5;
        assert_eq!(video.due(0.0).map(|f| f.time), Some(0.4));
        assert_eq!(video.status(), Status::Ended);
    }

    #[test]
    fn a_file_that_cannot_be_opened_says_why() {
        let (mut video, events, _) = video();
        events.send(Event::Failed("no such file".into())).unwrap();
        assert_eq!(video.status(), Status::Failed("no such file".into()));
        assert_eq!(video.wait_open(), Err("no such file".into()));
        // A decoder that went away with nothing said has failed too.
        let (mut video, events, _) = self::video();
        drop(events);
        assert!(matches!(video.status(), Status::Failed(_)));
    }

    #[test]
    fn a_failure_part_way_keeps_what_was_shown() {
        let (mut video, events, _) = video();
        events.send(Event::Opened(PICTURE)).unwrap();
        events.send(frame(0.0)).unwrap();
        events.send(Event::Failed("bad sample".into())).unwrap();
        assert!(video.due(0.0).is_some());
        assert_eq!(video.status(), Status::Failed("bad sample".into()));
        assert_eq!(video.wait_open(), Ok(PICTURE));
    }

    #[cfg(not(windows))]
    #[test]
    fn anywhere_but_windows_nothing_plays() {
        let mut video = Video::open(Path::new("boot.mp4"), Options::default());
        assert!(matches!(video.status(), Status::Failed(_)));
        assert!(video.due(0.0).is_none());
    }

    #[test]
    fn a_pictures_shape_counts_its_pixels_shape() {
        assert_eq!(Picture { size: (1920, 1080), pixel_aspect: (1, 1) }.aspect(), 16.0 / 9.0);
        assert!((Picture { size: (720, 480), pixel_aspect: (32, 27) }.aspect() - 16.0 / 9.0).abs() < 1e-4);
    }
}
