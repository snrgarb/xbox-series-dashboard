//! Now Playing (docs/spec/now-playing.md): which media session the Audio Flyout's card shows, and Settings' Now
//! Playing source that decides it. The shell reads Windows' sessions on a thread of its own and hands the Dashboard
//! each reading (`Dashboard::set_media`); this keeps the last, when it came, and the apps seen since the start.
use std::time::{Duration, Instant};

use model::media::{same_app, ANY_SOURCE, DEFAULT_SOURCE};
use model::{Media, MediaSession};

/// The settings row that keeps the source (`Effect::NowPlayingSource` saves it).
pub const SOURCE_KEY: &str = "guide.now_playing_source";

#[derive(Debug, Clone)]
pub struct NowPlaying {
    media: Media,
    /// When the reading came, by the Dashboard's clock: a playing track's place moves on from there.
    read_at: Option<Instant>,
    /// The app the card follows while it has a session: Any, Spotify or an app seen.
    source: String,
    /// The apps other than Spotify that have had a session since the start, in the order seen; a saved source not
    /// seen yet first, so that it is a choice before its app has started.
    apps: Vec<String>,
}

/// No media and the source as it is at first: for a page made with no Dashboard around it (a unit test's).
pub fn none() -> &'static NowPlaying {
    static NONE: std::sync::OnceLock<NowPlaying> = std::sync::OnceLock::new();
    NONE.get_or_init(NowPlaying::default)
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self::new(None)
    }
}

/// What the card shows: the session, and how far into its track it is now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Card<'a> {
    pub session: &'a MediaSession,
    pub position: Duration,
}

impl NowPlaying {
    /// With the source the settings table kept, if any: none or a blank one is Spotify, and Any and Spotify are
    /// known whatever their case.
    pub fn new(saved: Option<&str>) -> Self {
        let mut now_playing =
            Self { media: Media::default(), read_at: None, source: DEFAULT_SOURCE.into(), apps: Vec::new() };
        match saved.map(str::trim).filter(|saved| !saved.is_empty()) {
            None => {}
            Some(saved) if same_app(saved, ANY_SOURCE) => now_playing.source = ANY_SOURCE.into(),
            Some(saved) if same_app(saved, DEFAULT_SOURCE) => {}
            Some(saved) => {
                now_playing.note(saved);
                now_playing.source = saved.into();
            }
        }
        now_playing
    }

    /// A new reading of Windows' sessions, at `now`. Every app with a session is a choice from now on.
    pub fn set_media(&mut self, media: Media, now: Instant) {
        let apps: Vec<String> = media.sessions.iter().map(MediaSession::app).collect();
        for app in apps {
            self.note(&app);
        }
        self.media = media;
        self.read_at = Some(now);
    }

    pub fn media(&self) -> &Media {
        &self.media
    }

    /// Notes an app as a choice; nothing for one already, or with no name.
    fn note(&mut self, app: &str) {
        let known = [ANY_SOURCE, DEFAULT_SOURCE]
            .into_iter()
            .chain(self.apps.iter().map(String::as_str))
            .any(|known| same_app(known, app));
        if !app.is_empty() && !known {
            self.apps.push(app.into());
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Settings' dropdown: Any, Spotify, then the apps seen, in the order seen.
    pub fn choices(&self) -> Vec<String> {
        [ANY_SOURCE.to_string(), DEFAULT_SOURCE.to_string()].into_iter().chain(self.apps.iter().cloned()).collect()
    }

    /// The source to this choice: the choice as the list has it when it was not the source already, else none.
    pub fn choose(&mut self, choice: &str) -> Option<String> {
        let choice = self.choices().into_iter().find(|known| same_app(known, choice))?;
        if choice == self.source {
            return None;
        }
        self.source = choice.clone();
        Some(choice)
    }

    /// The session the card shows, and where in its track it is at `now`: while it plays, its place when read
    /// moved on by the time since, held within the track's length.
    pub fn card(&self, now: Instant) -> Option<Card<'_>> {
        let session = self.media.shown(&self.source)?;
        let mut position = session.position;
        if session.playing {
            position += self.read_at.map_or(Duration::ZERO, |read| now.saturating_duration_since(read));
        }
        if session.length > Duration::ZERO {
            position = position.min(session.length);
        }
        Some(Card { session, position })
    }

    /// How long from `now` until the card's time next turns a second: while it plays a track with a length that
    /// it has not reached the end of. The card is drawn again then, and at no other time for the time's sake.
    pub fn next_second(&self, now: Instant) -> Option<Duration> {
        let card = self.card(now)?;
        let session = card.session;
        if !session.playing || session.length.is_zero() || card.position >= session.length {
            return None;
        }
        let into = card.position.subsec_nanos();
        Some(Duration::from_nanos(u64::from(1_000_000_000 - into)))
    }

    /// What `--log-focus` says of the card (`now playing: ...`), which a test judges by: `nothing`, or the track,
    /// the app, whether it plays, the length and which controls the app has on. Not the time: it would be a line a
    /// second.
    pub fn line(&self) -> String {
        let Some(session) = self.media.shown(&self.source) else { return "nothing".into() };
        let artist = session.artist.as_deref().map(|artist| format!(" by {artist:?}")).unwrap_or_default();
        let state = if session.playing { "playing" } else { "paused" };
        let length =
            if session.length.is_zero() { "no length".to_string() } else { model::media::time(session.length) };
        let on = |on: bool| if on { "on" } else { "off" };
        format!(
            "{:?}{artist} in {}, {state}, {length}, art {}, previous {}, play/pause {}, next {}",
            session.title,
            session.app(),
            if session.art.is_some() { "yes" } else { "none" },
            on(session.can_previous),
            on(session.can_play_pause),
            on(session.can_next)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(app_id: &str, title: &str) -> MediaSession {
        MediaSession {
            app_id: app_id.into(),
            title: title.into(),
            artist: Some("Someone".into()),
            playing: true,
            can_play_pause: true,
            can_previous: true,
            can_next: true,
            position: Duration::from_secs(30),
            length: Duration::from_secs(200),
            art: None,
        }
    }

    fn media(sessions: Vec<MediaSession>, current: Option<usize>) -> Media {
        Media { sessions, current }
    }

    #[test]
    fn spotify_is_the_source_until_one_is_chosen_and_a_saved_one_is_a_choice_at_once() {
        assert_eq!(NowPlaying::new(None).source(), "Spotify");
        assert_eq!(NowPlaying::new(Some("  ")).source(), "Spotify");
        assert_eq!(NowPlaying::new(Some("any")).source(), "Any");
        assert_eq!(NowPlaying::new(Some("SPOTIFY")).source(), "Spotify");
        let saved = NowPlaying::new(Some("VLC"));
        assert_eq!(saved.source(), "VLC");
        assert_eq!(saved.choices(), ["Any", "Spotify", "VLC"]);
        assert_eq!(NowPlaying::new(None).choices(), ["Any", "Spotify"], "nothing else seen yet");
    }

    #[test]
    fn every_app_that_has_had_a_session_is_a_choice_in_the_order_seen_even_once_it_has_gone() {
        let mut now_playing = NowPlaying::default();
        let now = Instant::now();
        now_playing.set_media(media(vec![session("chrome.exe", "a"), session("Spotify.exe", "b")], Some(0)), now);
        now_playing.set_media(media(vec![session("vlc.exe", "c"), session("chrome.exe", "a")], Some(1)), now);
        now_playing.set_media(Media::default(), now);
        assert_eq!(now_playing.choices(), ["Any", "Spotify", "Chrome", "VLC"]);
    }

    #[test]
    fn choosing_takes_the_lists_spelling_and_a_choice_that_is_not_there_or_is_already_set_changes_nothing() {
        let mut now_playing = NowPlaying::default();
        now_playing.set_media(media(vec![session("chrome.exe", "a")], Some(0)), Instant::now());
        assert_eq!(now_playing.choose("chrome"), Some("Chrome".into()));
        assert_eq!(now_playing.source(), "Chrome");
        assert_eq!(now_playing.choose("Chrome"), None);
        assert_eq!(now_playing.choose("Winamp"), None);
        assert_eq!(now_playing.choose("any"), Some("Any".into()));
    }

    #[test]
    fn the_card_follows_the_source_while_it_has_a_session_else_windows_current() {
        let mut now_playing = NowPlaying::default();
        let now = Instant::now();
        now_playing
            .set_media(media(vec![session("chrome.exe", "video"), session("Spotify.exe", "song")], Some(0)), now);
        assert_eq!(now_playing.card(now).unwrap().session.title, "song");
        now_playing.choose("Any");
        assert_eq!(now_playing.card(now).unwrap().session.title, "video");
        now_playing.choose("Spotify");
        now_playing.set_media(media(vec![session("chrome.exe", "video")], Some(0)), now);
        assert_eq!(now_playing.card(now).unwrap().session.title, "video", "Spotify has none: Windows' current");
        now_playing.set_media(media(vec![session("chrome.exe", "video")], None), now);
        assert_eq!(now_playing.card(now), None, "nothing is current and the source has none");
        assert_eq!(now_playing.line(), "nothing");
    }

    #[test]
    fn a_playing_tracks_place_moves_on_from_its_reading_and_stops_at_its_end_and_a_paused_ones_stands() {
        let mut now_playing = NowPlaying::default();
        let read = Instant::now();
        now_playing.set_media(media(vec![session("Spotify.exe", "song")], Some(0)), read);
        let at = |now_playing: &NowPlaying, seconds: f32| {
            now_playing.card(read + Duration::from_secs_f32(seconds)).unwrap().position
        };
        assert_eq!(at(&now_playing, 0.0), Duration::from_secs(30));
        assert_eq!(at(&now_playing, 2.5), Duration::from_secs_f32(32.5));
        assert_eq!(at(&now_playing, 500.0), Duration::from_secs(200), "held at the track's end");
        assert_eq!(now_playing.next_second(read + Duration::from_millis(2500)), Some(Duration::from_millis(500)));
        assert_eq!(now_playing.next_second(read + Duration::from_secs(400)), None, "at the end: nothing to move");

        let paused = MediaSession { playing: false, ..session("Spotify.exe", "song") };
        now_playing.set_media(media(vec![paused], Some(0)), read);
        assert_eq!(at(&now_playing, 10.0), Duration::from_secs(30));
        assert_eq!(now_playing.next_second(read), None);

        let live = MediaSession { length: Duration::ZERO, ..session("Spotify.exe", "stream") };
        now_playing.set_media(media(vec![live], Some(0)), read);
        assert_eq!(now_playing.next_second(read), None, "no length: no time and no bar to move");
    }

    #[test]
    fn the_log_line_says_the_track_the_app_its_state_and_its_controls() {
        let mut now_playing = NowPlaying::default();
        let paused = MediaSession { playing: false, can_previous: false, ..session("standin.exe", "Stand-in Song") };
        now_playing.set_media(media(vec![paused], Some(0)), Instant::now());
        assert_eq!(
            now_playing.line(),
            r#""Stand-in Song" by "Someone" in Standin, paused, 3:20, art none, previous off, play/pause on, next on"#
        );
    }
}
