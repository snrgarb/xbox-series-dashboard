//! Windows' media sessions, which the Guide's Now Playing card shows (docs/spec/now-playing.md): every app's that
//! tells Windows what it plays, through the Global System Media Transport Controls (`Windows.Media.Control`), and the
//! one Windows names current. They are followed on a thread of their own from the start until the Dashboard ends, and
//! each change, once the changes have settled, is a new reading for the loop to take (`MediaSessions::take`).
//!
//! The controls ask the app through the same sessions; what the app does shows only in a later reading.
//! `SystemMedia` is Windows'; `FakeMedia` is a session made up for a picture or a test; anywhere but Windows there
//! is none.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use model::{Media, MediaCommand, MediaSession};

/// How long the sessions have to go without changing before a reading is taken: an app tells of a new track in
/// several changes within 100 ms, the title often before the art (the Bundle's `MediaSettle`).
pub const SETTLE: Duration = Duration::from_millis(200);
/// How long the last track's art stays up for a new track that has none so far, in case the app tells of it in a
/// later change (the Bundle's `ArtPatience`).
pub const ART_PATIENCE: Duration = Duration::from_secs(1);

/// Windows' media sessions, as the Dashboard is given them.
pub trait MediaSessions {
    /// The newest reading since this was last asked, if there has been one.
    fn take(&mut self) -> Option<Media>;
    /// Asks the app with this id (`MediaSession::app_id`) to do this. Never fails: an app that refuses, or has
    /// gone, is said in the log, and what it does shows only in a later reading.
    fn send(&mut self, command: MediaCommand, app_id: &str);
}

/// No media sessions, ever: anywhere but Windows, and where Windows has none to follow.
pub struct NoMedia;

impl MediaSessions for NoMedia {
    fn take(&mut self) -> Option<Media> {
        None
    }

    fn send(&mut self, command: MediaCommand, app_id: &str) {
        eprintln!("media: no media sessions to ask {app_id} to {}", name(command));
    }
}

/// A media session made up, for a picture of the card or a test: given once, and each control asked of it said in
/// the log and kept.
pub struct FakeMedia {
    media: Option<Media>,
    pub asked: Vec<(MediaCommand, String)>,
}

impl FakeMedia {
    pub fn new(media: Media) -> Self {
        Self { media: Some(media), asked: Vec::new() }
    }

    /// One session, playing a track 3:20 long 1:05 in, with a picture if `art` names one: `--fake-media`.
    pub fn stand_in(art: Option<PathBuf>) -> Self {
        let session = MediaSession {
            app_id: "standin.exe".into(),
            title: "Stand-in Song".into(),
            artist: Some("The Stand-ins".into()),
            playing: true,
            can_play_pause: true,
            can_previous: false,
            can_next: true,
            position: Duration::from_secs(65),
            length: Duration::from_secs(200),
            art,
        };
        Self::new(Media { sessions: vec![session], current: Some(0) })
    }
}

impl MediaSessions for FakeMedia {
    fn take(&mut self) -> Option<Media> {
        self.media.take()
    }

    fn send(&mut self, command: MediaCommand, app_id: &str) {
        eprintln!("media: asked {app_id} to {}", name(command));
        self.asked.push((command, app_id.to_string()));
    }
}

/// A command as the log says it.
pub fn name(command: MediaCommand) -> &'static str {
    match command {
        MediaCommand::Previous => "go back",
        MediaCommand::PlayPause => "play or pause",
        MediaCommand::Next => "skip",
    }
}

/// What the log says of a reading: `media: 2 sessions, current "Spotify": ...`.
pub fn describe(media: &Media) -> String {
    let current =
        media.current().map(|session| format!("current {:?}", session.app())).unwrap_or("none current".into());
    let sessions: Vec<String> = media
        .sessions
        .iter()
        .map(|session| {
            let state = if session.playing { "playing" } else { "paused" };
            format!("{} ({}) {:?} {state}", session.app(), session.app_id, session.title)
        })
        .collect();
    format!("{} sessions, {current}: {}", media.sessions.len(), sessions.join("; "))
}

/// The track a session plays, as far as its art goes.
type Track = (String, String, Option<String>);

/// Each app's album art, as files in a folder of the Dashboard's: a session's picture is read from the app once a
/// track, written there, and decoded by the art cache as any picture is. A new track keeps the last one's art for
/// [`ART_PATIENCE`] while the app has given none for it, so a track change swaps the picture once.
pub struct ArtShelf {
    dir: Option<PathBuf>,
    apps: HashMap<String, Shelf>,
    written: u64,
}

struct Shelf {
    track: Track,
    /// This track's art; none until it has been read.
    art: Option<PathBuf>,
    /// The last track's, while it may stand in for this one's, and since when.
    old: Option<(PathBuf, Instant)>,
}

/// What a session shows of its art now.
#[derive(Debug, PartialEq)]
pub enum Look {
    /// This, and if it is the last track's standing in, a look again this soon.
    Show(Option<PathBuf>, Option<Duration>),
    /// The app has a picture for this track that has not been read: read it and say so with `fetched`.
    Fetch,
}

impl ArtShelf {
    /// Art is written into `dir`, emptied of any left from before; with none there is no art.
    pub fn new(dir: Option<PathBuf>) -> Self {
        if let Some(dir) = &dir {
            let _ = std::fs::remove_dir_all(dir);
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("media: no folder for album art at {}: {e}", dir.display());
            }
        }
        Self { dir, apps: HashMap::new(), written: 0 }
    }

    /// What `session` shows of its art at `now`, the app saying whether it has a picture for it.
    pub fn look(&mut self, session: &MediaSession, has_picture: bool, now: Instant) -> Look {
        let track = (session.app_id.clone(), session.title.clone(), session.artist.clone());
        let shelf =
            self.apps.entry(session.app_id.clone()).or_insert(Shelf { track: track.clone(), art: None, old: None });
        if shelf.track != track {
            if let Some(art) = shelf.art.take() {
                if let Some((older, _)) = shelf.old.replace((art, now)) {
                    forget(&older);
                }
            }
            shelf.track = track;
        }
        if shelf.art.is_some() {
            return Look::Show(shelf.art.clone(), None);
        }
        if has_picture && self.dir.is_some() {
            return Look::Fetch;
        }
        match &shelf.old {
            Some((old, since)) if now.saturating_duration_since(*since) < ART_PATIENCE => {
                Look::Show(Some(old.clone()), Some(ART_PATIENCE - now.saturating_duration_since(*since)))
            }
            _ => {
                if let Some((old, _)) = shelf.old.take() {
                    forget(&old);
                }
                Look::Show(None, None)
            }
        }
    }

    /// The picture read for `session`'s track: written to the folder and shown from now on. None, or one that could
    /// not be written, is as no picture.
    pub fn fetched(&mut self, session: &MediaSession, bytes: Option<&[u8]>, now: Instant) -> Option<PathBuf> {
        let written = bytes.and_then(|bytes| self.write(bytes));
        let shelf = self.apps.get_mut(&session.app_id)?;
        match written {
            Some(path) => {
                if let Some((old, _)) = shelf.old.take() {
                    forget(&old);
                }
                shelf.art = Some(path.clone());
                Some(path)
            }
            None => match &shelf.old {
                Some((old, since)) if now.saturating_duration_since(*since) < ART_PATIENCE => Some(old.clone()),
                _ => None,
            },
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        self.written += 1;
        let path = dir.join(format!("art-{}.img", self.written));
        match std::fs::write(&path, bytes) {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("media: couldn't keep the album art at {}: {e}", path.display());
                None
            }
        }
    }

    /// The apps that have no session now let go of their art.
    pub fn keep_only(&mut self, apps: &[&str]) {
        self.apps.retain(|app, shelf| {
            let kept = apps.contains(&app.as_str());
            if !kept {
                for path in shelf.art.iter().chain(shelf.old.as_ref().map(|(old, _)| old)) {
                    forget(path);
                }
            }
            kept
        });
    }
}

fn forget(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(windows)]
pub use self::windows::SystemMedia;

/// Windows' media sessions, followed from now on, writing album art into `art_dir`; `wake` is called from the
/// thread that follows them each time there is a new reading. Anywhere but Windows there are none.
pub fn system(art_dir: Option<PathBuf>, wake: impl Fn() + Send + Sync + 'static) -> Box<dyn MediaSessions> {
    #[cfg(windows)]
    {
        Box::new(SystemMedia::start(art_dir, wake))
    }
    #[cfg(not(windows))]
    {
        let _ = (art_dir, wake);
        Box::new(NoMedia)
    }
}

#[cfg(windows)]
mod windows {
    use std::path::PathBuf;
    use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use model::{Media, MediaCommand, MediaSession};
    use windows::Foundation::TypedEventHandler;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::DataReader;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    use super::{describe, name, ArtShelf, Look, MediaSessions, SETTLE};

    /// What the thread is told.
    enum Message {
        /// A session came or went, or what one plays changed: read them again once they settle.
        Changed(&'static str),
        Ask(MediaCommand, String),
    }

    pub struct SystemMedia {
        to_thread: Sender<Message>,
        latest: Arc<Mutex<Option<Media>>>,
    }

    impl SystemMedia {
        pub fn start(art_dir: Option<PathBuf>, wake: impl Fn() + Send + Sync + 'static) -> Self {
            let (to_thread, messages) = channel();
            let latest = Arc::new(Mutex::new(None));
            let (tell, publish) = (to_thread.clone(), latest.clone());
            let spawned = std::thread::Builder::new().name("media sessions".into()).spawn(move || {
                follow(messages, tell, art_dir, |media| {
                    *publish.lock().unwrap() = Some(media);
                    wake();
                })
            });
            if let Err(e) = spawned {
                eprintln!("media: couldn't start following Windows' media sessions: {e}");
            }
            Self { to_thread, latest }
        }
    }

    impl MediaSessions for SystemMedia {
        fn take(&mut self) -> Option<Media> {
            self.latest.lock().unwrap().take()
        }

        fn send(&mut self, command: MediaCommand, app_id: &str) {
            let _ = self.to_thread.send(Message::Ask(command, app_id.to_string()));
        }
    }

    /// The handlers the thread has on Windows' objects: dropped with them as the thread ends.
    struct Followed {
        sessions: Vec<(String, Session, [i64; 3])>,
    }

    /// The thread: asks Windows for its session manager, then reads every session at the start and each time they
    /// have settled after a change, and carries out what the controls ask, until the Dashboard ends.
    fn follow(messages: Receiver<Message>, tell: Sender<Message>, art_dir: Option<PathBuf>, publish: impl Fn(Media)) {
        // SAFETY: once, for this thread, before any WinRT object is made on it.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let manager = match Manager::RequestAsync().and_then(|asked| asked.join()) {
            Ok(manager) => manager,
            Err(e) => {
                eprintln!(
                    "media: Windows has no media sessions to follow ({e}): Now Playing says no music app is running"
                );
                return;
            }
        };
        let handler = |why: &'static str| {
            let tell = Mutex::new(tell.clone());
            move || {
                let _ = tell.lock().map(|tell| tell.send(Message::Changed(why)));
            }
        };
        let (sessions_changed, current_changed) =
            (handler("sessions came or went"), handler("the current one changed"));
        let subscribed = manager
            .SessionsChanged(&TypedEventHandler::new(move |_, _| {
                sessions_changed();
                Ok(())
            }))
            .and_then(|_| {
                manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                    current_changed();
                    Ok(())
                }))
            });
        if let Err(e) = subscribed {
            eprintln!("media: couldn't follow Windows' media sessions as they come and go: {e}");
        }
        let mut followed = Followed { sessions: Vec::new() };
        let mut shelf = ArtShelf::new(art_dir);
        // Read at once, then each time the changes have settled, or the last track's art has had its time.
        let mut due = Some(Instant::now());
        let mut last: Option<Media> = None;
        loop {
            let wait = due.map_or(Duration::from_secs(3600), |due| due.saturating_duration_since(Instant::now()));
            match messages.recv_timeout(wait) {
                Ok(Message::Changed(why)) => {
                    // A change, then quiet for SETTLE: only the last of a burst is read.
                    let _ = why;
                    due = Some(Instant::now() + SETTLE);
                }
                Ok(Message::Ask(command, app_id)) => ask(&manager, command, &app_id),
                Err(RecvTimeoutError::Timeout) => {
                    let (media, again) = read(&manager, &mut followed, &mut shelf, &tell);
                    due = again.map(|again| Instant::now() + again);
                    if last.as_ref() != Some(&media) {
                        eprintln!("media: {}", describe(&media));
                        last = Some(media.clone());
                        publish(media);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Every session as it is now, and how soon to read them again for a track whose art is the last one's for
    /// now. A session whose app goes as it is read is left out, its going bringing another reading.
    fn read(
        manager: &Manager,
        followed: &mut Followed,
        shelf: &mut ArtShelf,
        tell: &Sender<Message>,
    ) -> (Media, Option<Duration>) {
        let listed = match manager.GetSessions() {
            Ok(list) => (0..list.Size().unwrap_or(0)).filter_map(|at| list.GetAt(at).ok()).collect::<Vec<_>>(),
            Err(e) => {
                eprintln!("media: Windows' media sessions couldn't be listed: {e}");
                Vec::new()
            }
        };
        let current = manager.GetCurrentSession().ok().and_then(|session| session.SourceAppUserModelId().ok());
        follow_sessions(followed, &listed, tell);
        let now = Instant::now();
        let mut media = Media::default();
        let mut again: Option<Duration> = None;
        for session in &listed {
            let Some((mut read, picture)) = read_session(session) else { continue };
            read.art = match shelf.look(&read, picture.is_some(), now) {
                Look::Show(art, wait) => {
                    again = match (again, wait) {
                        (Some(again), Some(wait)) => Some(again.min(wait)),
                        (again, wait) => again.or(wait),
                    };
                    art
                }
                Look::Fetch => {
                    let bytes = picture.and_then(|picture| picture());
                    shelf.fetched(&read, bytes.as_deref(), now)
                }
            };
            if current.as_ref().is_some_and(|current| *current == read.app_id) {
                media.current = Some(media.sessions.len());
            }
            media.sessions.push(read);
        }
        let apps: Vec<&str> = media.sessions.iter().map(|session| session.app_id.as_str()).collect();
        shelf.keep_only(&apps);
        (media, again)
    }

    /// The handlers moved onto the sessions listed: those gone let go of, those new subscribed to.
    fn follow_sessions(followed: &mut Followed, listed: &[Session], tell: &Sender<Message>) {
        let ids: Vec<String> = listed
            .iter()
            .map(|session| session.SourceAppUserModelId().map(|id| id.to_string()).unwrap_or_default())
            .collect();
        followed.sessions.retain(|(id, session, tokens)| {
            let kept = ids.contains(id);
            if !kept {
                // Its app may have gone already, and Windows dropped the handlers with it.
                let _ = session.RemoveMediaPropertiesChanged(tokens[0]);
                let _ = session.RemovePlaybackInfoChanged(tokens[1]);
                let _ = session.RemoveTimelinePropertiesChanged(tokens[2]);
                eprintln!("media: no longer following {id}");
            }
            kept
        });
        for (session, id) in listed.iter().zip(ids) {
            if followed.sessions.iter().any(|(known, _, _)| *known == id) {
                continue;
            }
            let changed = |why: &'static str| {
                let tell = Mutex::new(tell.clone());
                move || {
                    let _ = tell.lock().map(|tell| tell.send(Message::Changed(why)));
                }
            };
            let (media, playback, timeline) =
                (changed("what it plays changed"), changed("it played or paused"), changed("it moved in its track"));
            let tokens = [
                session.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    media();
                    Ok(())
                })),
                session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    playback();
                    Ok(())
                })),
                session.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    timeline();
                    Ok(())
                })),
            ];
            let tokens = tokens.map(|token| token.unwrap_or(0));
            eprintln!("media: following {id}");
            followed.sessions.push((id, session.clone(), tokens));
        }
    }

    /// A function that reads a session's picture, when it has one.
    type Picture = Box<dyn FnOnce() -> Option<Vec<u8>>>;

    /// What `session` plays now, and its picture to read if the art is wanted; none when its app went as it was
    /// read.
    fn read_session(session: &Session) -> Option<(MediaSession, Option<Picture>)> {
        let app_id = session.SourceAppUserModelId().ok()?.to_string();
        let properties = session.TryGetMediaPropertiesAsync().and_then(|asked| asked.join());
        let properties = match properties {
            Ok(properties) => properties,
            Err(e) => {
                eprintln!("media: the session of {app_id} went as it was read ({e}); leaving it out");
                return None;
            }
        };
        let playback = session.GetPlaybackInfo().ok();
        let controls = playback.as_ref().and_then(|playback| playback.Controls().ok());
        let enabled = |is: fn(
            &windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackControls,
        ) -> windows::core::Result<bool>| {
            controls.as_ref().and_then(|controls| is(controls).ok()).unwrap_or(false)
        };
        let playing = playback.as_ref().and_then(|playback| playback.PlaybackStatus().ok()) == Some(Status::Playing);
        let timeline = session.GetTimelineProperties().ok();
        let ticks = |span: windows::core::Result<windows::Foundation::TimeSpan>| span.map_or(0, |span| span.Duration);
        let (start, end, position, updated) = match &timeline {
            Some(timeline) => (
                ticks(timeline.StartTime()),
                ticks(timeline.EndTime()),
                ticks(timeline.Position()),
                timeline.LastUpdatedTime().map_or(0, |time| time.UniversalTime),
            ),
            None => (0, 0, 0, 0),
        };
        // Where it is now: where it was when the app last said, moved on by the time since while it plays.
        let mut at = (position - start).max(0);
        if playing && updated > 0 {
            at += (file_time_now() - updated).max(0);
        }
        let length = (end - start).max(0);
        if length > 0 {
            at = at.min(length);
        }
        let artist = properties.Artist().map(|artist| artist.to_string()).unwrap_or_default();
        let read = MediaSession {
            app_id,
            title: properties.Title().map(|title| title.to_string()).unwrap_or_default(),
            artist: (!artist.is_empty()).then_some(artist),
            playing,
            can_play_pause: enabled(|controls| controls.IsPlayPauseToggleEnabled()),
            can_previous: enabled(|controls| controls.IsPreviousEnabled()),
            can_next: enabled(|controls| controls.IsNextEnabled()),
            position: from_ticks(at),
            length: from_ticks(length),
            art: None,
        };
        let picture: Option<Picture> = properties.Thumbnail().ok().map(|thumbnail| {
            let app = read.app_id.clone();
            Box::new(move || match picture_bytes(&thumbnail) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    eprintln!("media: couldn't read the album art of {app}: {e}");
                    None
                }
            }) as Picture
        });
        Some((read, picture))
    }

    /// The picture's bytes, whatever kind of picture it is: the art cache decodes it by its content.
    fn picture_bytes(
        thumbnail: &windows::Storage::Streams::IRandomAccessStreamReference,
    ) -> windows::core::Result<Vec<u8>> {
        let stream = thumbnail.OpenReadAsync()?.join()?;
        let size = stream.Size()?.min(16 << 20) as u32;
        let reader = DataReader::CreateDataReader(&stream)?;
        let loaded = reader.LoadAsync(size)?.join()?;
        let mut bytes = vec![0; loaded as usize];
        reader.ReadBytes(&mut bytes)?;
        Ok(bytes)
    }

    /// Asks the app's session, among Windows' sessions now, to do `command`.
    fn ask(manager: &Manager, command: MediaCommand, app_id: &str) {
        let session = manager.GetSessions().ok().and_then(|list| {
            (0..list.Size().unwrap_or(0))
                .filter_map(|at| list.GetAt(at).ok())
                .find(|session| session.SourceAppUserModelId().is_ok_and(|id| id == app_id))
        });
        let Some(session) = session else {
            eprintln!("media: {app_id} has no media session to {}", name(command));
            return;
        };
        let asked = match command {
            MediaCommand::Previous => session.TrySkipPreviousAsync(),
            MediaCommand::PlayPause => session.TryTogglePlayPauseAsync(),
            MediaCommand::Next => session.TrySkipNextAsync(),
        };
        match asked.and_then(|asked| asked.join()) {
            Ok(true) => eprintln!("media: asked {app_id} to {}", name(command)),
            Ok(false) => eprintln!("media: {app_id} refused to {}", name(command)),
            Err(e) => eprintln!("media: {app_id} failed to {} ({e})", name(command)),
        }
    }

    /// WinRT's ticks of 100 ns.
    fn from_ticks(ticks: i64) -> Duration {
        Duration::from_nanos(ticks.max(0) as u64 * 100)
    }

    /// Now, as a WinRT `DateTime`: 100 ns ticks since 1601.
    fn file_time_now() -> i64 {
        const FROM_1601: i64 = 116_444_736_000_000_000;
        let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        FROM_1601 + (since.as_nanos() / 100) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(app: &str, title: &str) -> MediaSession {
        MediaSession { app_id: app.into(), title: title.into(), ..MediaSession::default() }
    }

    fn shelf() -> (tempfile::TempDir, ArtShelf) {
        let dir = tempfile::tempdir().unwrap();
        let shelf = ArtShelf::new(Some(dir.path().join("now-playing")));
        (dir, shelf)
    }

    #[test]
    fn a_tracks_picture_is_read_once_and_kept() {
        let (_dir, mut shelf) = shelf();
        let now = Instant::now();
        let song = session("spotify.exe", "one");
        assert_eq!(shelf.look(&song, true, now), Look::Fetch);
        let art = shelf.fetched(&song, Some(b"picture"), now).unwrap();
        assert_eq!(std::fs::read(&art).unwrap(), b"picture");
        assert_eq!(shelf.look(&song, true, now), Look::Show(Some(art), None), "not read again");
    }

    #[test]
    fn a_new_track_with_no_picture_yet_keeps_the_last_ones_for_a_second_then_shows_none() {
        let (_dir, mut shelf) = shelf();
        let now = Instant::now();
        let one = session("spotify.exe", "one");
        shelf.look(&one, true, now);
        let art = shelf.fetched(&one, Some(b"one"), now).unwrap();
        let two = session("spotify.exe", "two");
        let later = now + Duration::from_millis(400);
        // Its second starts as the new track is seen.
        assert_eq!(shelf.look(&two, false, later), Look::Show(Some(art.clone()), Some(ART_PATIENCE)));
        let still = later + Duration::from_millis(400);
        assert_eq!(shelf.look(&two, false, still), Look::Show(Some(art.clone()), Some(Duration::from_millis(600))));
        // Its picture comes in a later change: read, and the last one's let go.
        assert_eq!(shelf.look(&two, true, later), Look::Fetch);
        let new = shelf.fetched(&two, Some(b"two"), later).unwrap();
        assert!(!art.exists(), "the last track's file is gone");
        assert_eq!(shelf.look(&two, true, later), Look::Show(Some(new), None));

        // A track that never has one: the last one's goes once its second is up.
        let three = session("spotify.exe", "three");
        assert!(matches!(shelf.look(&three, false, later), Look::Show(Some(_), Some(_))));
        assert_eq!(shelf.look(&three, false, later + ART_PATIENCE), Look::Show(None, None));
    }

    #[test]
    fn with_no_folder_there_is_no_art_and_an_app_that_has_gone_lets_go_of_its_own() {
        let mut none = ArtShelf::new(None);
        assert_eq!(none.look(&session("a", "one"), true, Instant::now()), Look::Show(None, None));

        let (_dir, mut shelf) = shelf();
        let now = Instant::now();
        let song = session("spotify.exe", "one");
        shelf.look(&song, true, now);
        let art = shelf.fetched(&song, Some(b"one"), now).unwrap();
        shelf.keep_only(&["chrome.exe"]);
        assert!(!art.exists());
    }

    #[test]
    fn a_fake_session_is_given_once_and_keeps_what_it_is_asked() {
        let mut fake = FakeMedia::stand_in(None);
        assert_eq!(fake.take().unwrap().current().unwrap().title, "Stand-in Song");
        assert_eq!(fake.take(), None);
        fake.send(MediaCommand::Next, "standin.exe");
        assert_eq!(fake.asked, [(MediaCommand::Next, "standin.exe".to_string())]);
    }
}
