# Now Playing

The Guide's Audio Flyout ends, under its divider, with Now Playing: the media one app plays, as Windows' own media flyout shows it, with Previous, Play/Pause and Next under it; or "No music app is running" when there is none. Settings > Guide's Now Playing source says which app it follows. Built in #129.

Sources: the Bundle's C/ExtensionCore.NowPlaying.cs (sessions, settling, art, card 173-295, controls 699-723), C/ExtensionCore.NowPlayingSource.cs (the source and its choices), C/FriendlyAppName.cs (app names), C/MediaTimeConverter.cs, and the Audio Flyout's XAML (E/GuideFlyouts.xaml, the `NowPlaying` entry and its `Media` grid). The Bundle has **no picture of the card**: its one picture of the Flyout (54-guide-audio-flyout.jpg) has "No music app is running". The card is built from the XAML's measures alone, and waits for a picture taken with music playing to be checked against.

Code: `model::media` (the sessions, names, times), `screens::now_playing` (which session shows, where in its track it is, the source), `screens::guide::flyout` (the card and its controls), `screens::settings::guide` (the source's dropdown), `shell::media` (Windows' sessions, followed on a thread; their album art), `app` (the wiring), `standin media` (a session of its own, for tests).

## 1. Where the sessions come from

Windows' Global System Media Transport Controls (`Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager`): every app that tells Windows what it plays (Spotify, a browser's tab, a video player) has a session in its list, and Windows names one of them current.

- **Read on a thread of their own** (`shell::media::SystemMedia`), from the Dashboard's start until it ends: the manager asked for once, then every session read at the start and each time something changes. The thread is told of a change by Windows' events (sessions come or go, the current one changes, and on each session what it plays, whether it plays, and where it is in its track), and reads again once the changes have **settled for 200 ms** (the Bundle's `MediaSettle`): an app tells of a new track in several changes within 100 ms, the title often before its art.
- **A reading** is every session's app, title, artist, whether it plays, which of Previous, Play/Pause and Next it has on, its place in its track and the track's length, and its album art (section 4), with which one is current (`model::Media`). A reading the same as the last is not passed on; a new one is passed to the window's loop, which wakes, gives it to the Dashboard (`Dashboard::set_media`), and draws.
- **Its place** is as of the reading: the place the app last gave, moved on by the time since it gave it while it plays (`LastUpdatedTime`), held within the track. The Dashboard moves it on from there by its own clock while the track plays (section 3.3).
- **A session whose app goes as it is read** is left out of that reading; its going brings another.
- **Elsewhere** (`NoMedia`) there are none, and Now Playing says "No music app is running". On Linux the screenshots and tests use a session made up (`FakeMedia`, `dashboard --fake-media [--fake-media-art <picture>]`, `Dashboard::set_media`).

Nothing is said in the log of Windows' sessions but what changes: `media: following <app id>`, `media: no longer following <app id>`, and `media: 2 sessions, current "Spotify": Spotify (Spotify.exe) "Song" playing; Chrome (chrome.exe) "Video" paused` as each reading that differs comes.

## 2. Which session shows

The **Now Playing source** (Settings > Guide, settings.md 6.8; `guide.now_playing_source` in the settings table) decides it, as the Bundle's `NowPlayingSource`:

- **Spotify** (the default) or another app: that app's session while it has one, playing or paused; else Windows' current one, whatever app it is.
- **Any**: Windows' current session, always.
- No session to show (none at all, or none current and none of the source's): "No music app is running".

Apps are told apart by name, whatever the case, so "Spotify" is Spotify's `Spotify.exe`. **The choices** are Any, Spotify, then every other app that has had a session since the start, in the order seen, kept after its session has gone; a source saved in an earlier run whose app has had none yet is among them too, first after Spotify. Choosing one is in force at once and saved; choosing the one set already, or one not among them, changes nothing.

**An app's name** is the one people know (the Bundle's `FriendlyAppName`): a table of the usual players and browsers, found anywhere in the app's id whatever its case (Spotify, Edge, Chrome, Firefox, Brave, Opera, Vivaldi, Media Player, Films & TV, Windows Media Player, VLC, Apple Music, iTunes, Amazon Music, TIDAL, Deezer, MusicBee, AIMP, Discord: `model::media::KNOWN_APPS`); else a packaged app's package name, before the `_` of its publisher's hash (`Contoso.Player_8wekyb3d8bbwe!App` is "Contoso Player"), or a program's file name without `.exe`; dots as spaces, the first letter a capital (`standin.exe` is "Standin").

## 3. The card

### 3.1 Layout

The entry is **218 high** with the card (150 for the card, 52 for the controls, 16 under them) where "No music app is running" is 75, with the Flyout's entry margins (9 either side, 3 under). Within it, the `Media` grid, 18 in from the left and 16 from the right:

| Part | Value |
|---|---|
| Album art | 120 square, radius 4, centred down the card's 150; centre-cropped, from a 240 thumbnail |
| No art | the same square in `#4A4A4A`, a note (U+E8D6) at 44 in `#B3FFFFFF` |
| Text column | 22 right of the art to the box's right less 16; its lines together, centred down the card's 150 |
| Title | the body style, white, wrapped; two lines at most (68 high), cut with an ellipsis |
| Artist | under it, `#B3FFFFFF`, one line, cut with an ellipsis; nothing when the app gives none |
| Bar | 10 under the artist, 4 high, the column's width: its track `#40FFFFFF`, how far in the track is in the accent (`#2CA243`), radius 2 |
| App line | 8 under the bar: the play mark (U+E768) while it plays or the pause mark (U+E769) while paused, 16 in `#B3FFFFFF` in a column 20 wide; the app's name 8 past it, `#B3FFFFFF`, cut short before the time; the time "1:05 / 3:20" at the right, `#B3FFFFFF`, 12 clear of the name |
| No length | no bar and no time: the app's name has the line |
| Controls | under the card: Previous (U+E892), Play (U+E768) or Pause (U+E769) as it is paused or plays, Next (U+E893); each 72 by 52 with 10 either side, the three centred; fill `#14FFFFFF`, radius 4, glyph 22 white |
| Focused control | fill `#26FFFFFF` and the Guide's focus ring (section 1 of guide.md) |
| A control the app has off | its fill at half, its glyph at 0.35 |

Times are `m:ss`, and `h:mm:ss` from an hour (the Bundle's `MediaTimeConverter`).

### 3.2 Album art

The shell reads a session's picture once a track, writes it into `<data folder>/now-playing/` (emptied as the Dashboard starts), and the session carries the file's path; the card draws it through the art cache as any picture is drawn, whatever its kind. **A track change swaps the picture once:** the last track's art stays up until the new one's has been decoded; a new track whose app has given no picture for it keeps the last track's for a second (the Bundle's `ArtPatience`), then shows the grey square. A file no longer shown is deleted; an app whose session has gone lets go of its own.

### 3.3 Time

While the track plays, the card's place moves on from the reading's by the Dashboard's clock, and is held at the track's end. The card is drawn again as the time turns each second (`Dashboard::timer`), while the Audio Flyout is open and a track with a length plays, and at no other time for the time's sake. Paused, or with no length, nothing on the card moves until the next reading.

## 4. The controls

- **Focus.** Down from the card (the Flyout's last entry) goes to Play/Pause; Left and Right step along the row, held at its ends; Up goes back to the card; Down on the row stays. B closes the Flyout, as on any entry. Each move is the Guide's move sound.
- **A** on a control the app has on asks the app (the Guide's select sound); on one it has off, nothing. A on the card, or on "No music app is running", does nothing (C/ExtensionCore.cs:2045).
- **Nothing shows changed until the app answers**: the control asks the app shown, by its id (`Effect::Media(command, app id)`), and the card shows what the next reading of its session says. The shell says in the log `media: asked <app id> to play or pause` (or `skip`, `go back`), `media: <app id> refused to ...` when the app says no, and `media: <app id> has no media session to ...` when it has gone.
- **The card goes** (its session gone, or the source's app stopping while nothing is current): focus on one of its controls is on "No music app is running" in its place. A card for another session keeps focus on the same control.

## 5. The log

With `--log-focus`:

- The `guide:` line has `, flyout Audio Item(4), volume 67` on the card or on "No music app is running", and `, flyout Audio Control(PlayPause), volume 67` (`Previous`, `Next`) on a control.
- `now playing: <what>` as the Audio Flyout opens and whenever what the card shows changes while it is open: `nothing`, or `"Stand-in Song" by "The Stand-ins" in Standin, playing, 3:20, art yes, previous off, play/pause on, next on` (the artist left out when there is none; `paused`; `no length`; `art none`). Not the time, which would be a line a second.
- Settings: `Settings: Guide: Now Playing source`, `Settings: Guide: Now Playing source, list on "Spotify"`, and `settings: guide.now_playing_source = Chrome` as a choice is saved.

## 6. Testing it

- Unit tests: the source and its choices, the place moving on and held, the log line (`screens::now_playing`); names and times (`model::media`); the art shelf, its patience and its files (`shell::media`); the controls' focus and what they ask (`crates/screens/tests/flyouts.rs`); the dropdown (`tests/settings.rs`).
- Pictures (`tests/shots.rs`): `guide-audio-nothing-playing`, `guide-audio-now-playing` (playing, art, focus on the card), `guide-audio-now-playing-paused` (no art, a long title, focus on Play), `guide-audio-now-playing-no-length` (focus on Next); `settings-guide`, `settings-guide-now-playing-source-list`.
- On Windows: `standin media` is a session of its own, which Windows names `standin.exe`, for the e2e payload `now-playing` (e2e/README.md). The payload sets the source to Standin first, so that nothing it presses ever reaches the user's own media on the TV PC.

## 7. Differences from the Bundle

- **No picture of the card to check against**: the layout is the XAML's, as section 3.1; the type is set in the Dashboard's body style, not measured.
- **The source's dropdown keeps a saved app as a choice** before its app has a session this run, where the Bundle's list would not hold the value its box shows.
- **The controls answer only through the session** (as the Bundle); the Dashboard shows no "pressed" state of its own while it waits.
