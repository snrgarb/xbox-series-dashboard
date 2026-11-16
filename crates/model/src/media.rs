//! Windows' media sessions as the Guide's Now Playing card shows them (docs/spec/now-playing.md): every app that
//! tells Windows what it plays, the one Windows names current among them, and what each plays. The shell follows
//! them (`shell::media`); the Dashboard only ever sees this.
use std::path::PathBuf;
use std::time::Duration;

/// Settings' Now Playing source that follows Windows' current session always.
pub const ANY_SOURCE: &str = "Any";
/// The Now Playing source until one is chosen.
pub const DEFAULT_SOURCE: &str = "Spotify";

/// One app's media, as Windows' media flyout shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaSession {
    /// The app's raw id, as Windows has it: an AUMID (`SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`), an
    /// executable's name (`chrome.exe`) or path. What the controls ask by.
    pub app_id: String,
    pub title: String,
    /// None for none, as for many a video.
    pub artist: Option<String>,
    /// Playing, rather than paused or stopped.
    pub playing: bool,
    /// Whether the app takes each control now, as its playback controls say.
    pub can_play_pause: bool,
    pub can_previous: bool,
    pub can_next: bool,
    /// How far into the track it is, as of the reading that brought it.
    pub position: Duration,
    /// The track's length: zero for none, as for a live stream.
    pub length: Duration,
    /// The album art, decoded from a file the shell wrote; none while the app gives none.
    pub art: Option<PathBuf>,
}

impl MediaSession {
    /// The app as people know it: Spotify, Chrome, Edge, rather than its raw id.
    pub fn app(&self) -> String {
        app_name(&self.app_id)
    }

    /// What it plays, as far as its art goes: the app, the title and the artist. A new track is a new one of these.
    pub fn track(&self) -> (&str, &str, Option<&str>) {
        (&self.app_id, &self.title, self.artist.as_deref())
    }
}

/// Every app's media session, as last read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Media {
    pub sessions: Vec<MediaSession>,
    /// The one Windows names current, by its place in `sessions`.
    pub current: Option<usize>,
}

impl Media {
    /// Windows' current session.
    pub fn current(&self) -> Option<&MediaSession> {
        self.sessions.get(self.current?)
    }

    /// The session Now Playing shows with this source: the source app's while it has one, else Windows' current;
    /// Windows' current always with the source Any.
    pub fn shown(&self, source: &str) -> Option<&MediaSession> {
        let chosen = (!same_app(source, ANY_SOURCE))
            .then(|| self.sessions.iter().find(|session| same_app(&session.app(), source)))
            .flatten();
        chosen.or_else(|| self.current())
    }
}

/// What the Now Playing card's controls ask the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    Previous,
    PlayPause,
    Next,
}

/// Two app names are the same app, whatever their case.
pub fn same_app(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Apps by a word their raw ids carry, once all but letters and digits are taken out and the rest made lower case,
/// and the names people know them by: the first that matches wins (the Bundle's `MediaSession.KnownApps`).
const KNOWN_APPS: [(&str, &str); 21] = [
    ("spotify", "Spotify"),
    ("msedge", "Edge"),
    ("microsoftedge", "Edge"),
    ("chrome", "Chrome"),
    ("firefox", "Firefox"),
    // Firefox's AUMID: a hash of its install folder, the same for every default install.
    ("308046b0af4a39cb", "Firefox"),
    ("brave", "Brave"),
    ("opera", "Opera"),
    ("vivaldi", "Vivaldi"),
    ("zunemusic", "Media Player"),
    ("zunevideo", "Films & TV"),
    ("wmplayer", "Windows Media Player"),
    ("vlc", "VLC"),
    ("applemusic", "Apple Music"),
    ("itunes", "iTunes"),
    ("amazonmusic", "Amazon Music"),
    ("tidal", "TIDAL"),
    ("deezer", "Deezer"),
    ("musicbee", "MusicBee"),
    ("aimp", "AIMP"),
    ("discord", "Discord"),
];

/// An app's raw id as people know the app: a known app by its name, any other by its package's or executable's
/// name, its dots as spaces and its first letter a capital.
pub fn app_name(app_id: &str) -> String {
    if app_id.is_empty() {
        return String::new();
    }
    let squashed: String = app_id.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    if let Some((_, name)) = KNOWN_APPS.iter().find(|(word, _)| squashed.contains(word)) {
        return name.to_string();
    }
    // A packaged app's AUMID, Publisher.Name_hash!App: its package's name. Else an executable's name or path.
    let name = match app_id.split_once('!') {
        Some((package, _)) => match package.find('_') {
            Some(underscore) if underscore > 0 => &package[..underscore],
            _ => package,
        },
        None => {
            let file = app_id.rsplit(['\\', '/']).next().unwrap_or(app_id);
            match file.len().checked_sub(4).filter(|&at| file.is_char_boundary(at)) {
                Some(at) if file[at..].eq_ignore_ascii_case(".exe") => &file[..at],
                _ => file,
            }
        }
    };
    let name = name.replace('.', " ");
    let name = name.trim();
    let mut chars = name.chars();
    match chars.next() {
        None => app_id.to_string(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// A time in a track as the card shows it: `1:05`, and past an hour `1:02:07`.
pub fn time(at: Duration) -> String {
    let seconds = at.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(app_id: &str) -> MediaSession {
        MediaSession { app_id: app_id.into(), title: app_id.into(), ..MediaSession::default() }
    }

    #[test]
    fn an_app_is_known_by_the_name_people_know_it_by() {
        assert_eq!(app_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify");
        assert_eq!(app_name("Spotify.exe"), "Spotify");
        assert_eq!(app_name("chrome.exe"), "Chrome");
        assert_eq!(app_name("MSEdge"), "Edge");
        assert_eq!(app_name("308046B0AF4A39CB"), "Firefox");
        assert_eq!(app_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "Media Player");
        // Any other: its package's name, or its program's, dots as spaces, a capital first.
        assert_eq!(app_name("Acme.Tunes_1234abcd!App"), "Acme Tunes");
        assert_eq!(app_name(r"C:\Program Files\Foo\foobar2000.exe"), "Foobar2000");
        assert_eq!(app_name("standin.exe"), "Standin");
        assert_eq!(app_name(""), "");
    }

    #[test]
    fn the_source_apps_session_is_shown_while_it_has_one_else_windows_current() {
        let media = Media { sessions: vec![session("chrome.exe"), session("Spotify.exe")], current: Some(0) };
        assert_eq!(media.shown("Spotify").unwrap().app_id, "Spotify.exe");
        assert_eq!(media.shown("spotify").unwrap().app_id, "Spotify.exe", "whatever the case");
        assert_eq!(media.shown("Any").unwrap().app_id, "chrome.exe");
        assert_eq!(media.shown("VLC").unwrap().app_id, "chrome.exe", "an app with no session: Windows' current");
        let none = Media { sessions: vec![session("chrome.exe")], current: None };
        assert_eq!(none.shown("Spotify"), None);
        assert_eq!(none.shown("Chrome").unwrap().app_id, "chrome.exe");
    }

    #[test]
    fn a_time_is_minutes_and_seconds_and_hours_past_an_hour() {
        assert_eq!(time(Duration::from_secs(65)), "1:05");
        assert_eq!(time(Duration::from_secs(200)), "3:20");
        assert_eq!(time(Duration::from_secs(3727)), "1:02:07");
        assert_eq!(time(Duration::ZERO), "0:00");
    }
}
