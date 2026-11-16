# Start-up specification

Start-up is the console's start-up sequence, full screen before Home (the Bundle's glossary, `CONTEXT.md`: Start-up, Boot video, Start-up's first frame). As in the Bundle it is the **Boot video** (#126), then **Who's playing today?** (or, with that not asking, straight to **Signing you in**), then Home with the "Hi X!" Toast (#127). The Guide's **Switch profile** and **waking from sleep** ask Who's playing today? again, without the video.

Sources: the Bundle's `StartupSequence` and `BootVideo` (C/ExtensionCore.Startup.cs:147-215, 337-384; C/StartupSequence.cs), its start-up window (StartupWindow.xaml and .xaml.cs), its `LoadingSpinner` (Spinner.cs), its Personalisation page (E/PersonalisationPane.xaml:281-294, E/PersonalisationPane.xaml.cs:29-40, 194-200, 355-359). Pictures: the Bundle's `port-reference-20261115-200355/whos-playing.jpg` (its artifacts) and the console's, the Bundle's `docs/reference/startup/1-whos-playing.jpg` to `4-home.jpg`. Code: `crates/screens/src/startup.rs` (the sequence, as the core keeps it, and its pictures), `crates/screens/src/settings/boot_video.rs` and `whos_playing.rs` (its Settings pages), `crates/shell/src/wake.rs` (waking from sleep), `crates/app/src/startup.rs` (the video found, chosen and played), `crates/media` (Media Foundation playback, from the `video-spike`; docs/research/video-media-foundation.md).

## 1. The Boot video

One console generation's boot animation, played full screen with its sound as the Dashboard starts, or none.

| Choice | Name on Settings | File |
|---|---|---|
| Off | Off | none |
| Xbox | Xbox (2001) | `Xbox.mp4` |
| Xbox360 | Xbox 360 (2005) | `Xbox360.mp4` |
| Xbox360Slim | Xbox 360 Slim (2010) | `Xbox360Slim.mp4` |
| Xbox360SpecialEdition | Xbox 360 Special Edition (2012) | `Xbox360SpecialEdition.mp4` |
| XboxOne | Xbox One (2013) | `XboxOne.mp4` |
| XboxOneX | Xbox One X (2017) | `XboxOneX.mp4` |
| XboxSeriesS | Xbox Series S (2020) | `XboxSeriesS.mp4` |
| XboxSeriesX | **Xbox Series X (2020)**, the default | `XboxSeriesX.mp4` |

The choice and the file are the Bundle's enum names; the order is the Bundle's, Off first.

### 1.1 Where the videos are

They are Microsoft's and never in this repository (#10). They are looked for as the Dashboard starts, by the names above, in the first of these places that has any of them (the same places as the sounds', `crates/app/src/media_folder.rs`):

1. `--media <dir>`: `<dir>\videos` if there is one, else `<dir>` itself. With `--media`, nowhere else.
2. `<data dir>\media\videos`, the data folder being `--data-dir`'s or `%LOCALAPPDATA%\XboxSeriesDashboard`.
3. The Bundle's extension, where it is installed in Playnite: `Extensions\XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b\Videos` under `%APPDATA%\Playnite` or `%LOCALAPPDATA%\Playnite`. A PC that has the Bundle has every video there.

What plays is the choice if its file was found; otherwise nothing, and Start-up goes straight on.

### 1.2 Playing

- **Picture.** Letterboxed to its own shape on black (the Bundle's `Stretch="Uniform"`), over everything: nothing of Home, its top bar or a Toast shows. Before the first frame is decoded the screen is black.
- **Sound.** The file's, on the default device, at the Dashboard's sound volume (`sound.volume`), and none while sounds are muted (`sound.muted`). The Bundle plays it at full volume; the port keeps the user's level for the Dashboard's sounds.
- **Clock.** The picture follows the sound's clock, as the spike did (docs/research/video-media-foundation.md); with no sound, the system's. The clock and the sound start as the first frame is presented: **Start-up's first frame** is the video's first frame.
- **Skip.** Any button of the pad skips it, and so do the keyboard's keys that stand for buttons (arrows, Enter, Escape, Backspace). A direction's repeats do not. A press counts once the picture has been on screen 0.3 s: one pressed for something else as the window opens is not taken as a skip. (The Bundle ignores buttons that were held as the video began until they are let go; the port has no held state at that point, and uses the short wait instead.) A skip makes no sound.
- **The Guide button** is Start-up's: its release after a tap, or its hold, skips the video like any button, and the rest of that press (its tap, its release) does nothing once Home shows. The Guide never opens over Start-up.
- **Escape** skips it, and does not close the Dashboard while Start-up shows.
- **End.** At the file's end, at a skip, when the file cannot be played (no Media Foundation, a file it cannot decode), or 20 s after it began (the Bundle's `BootVideoDeadline`), Start-up goes on (section 4): Who's playing today?, at once, with no fade between.
- **No preview.** Settings never plays it.

### 1.3 What goes on behind it

The file is opened before the window, so it decodes while the window and the GPU are made. As soon as the window's first frame is out (black, the video's first frame, or Who's playing today?) the sources are read, the stores' apps listed and the Queue followed, exactly as they are after Home's first frame with no Start-up. Home is drawn once, unseen, under the video's first frame (with no video, under Who's playing today?'s first), so its art is asked for and decoded meanwhile. When Start-up ends, Home's first frame is whole.

Held back until Start-up is over: the Overlay and the Toast strip, and the art lookup online. Toasts wait until the greeting has shown (section 6), as the Bundle holds them back.

## 2. Settings > Personalisation > Boot video

Personalisation's first box, (0, 0), U+E714, as the Bundle's (settings.md 6.2). Its page (settings.md 6.3):

- y 210: "Plays as the Dashboard starts. Any button skips it." With no video found: "No boot videos were found on this PC, so none plays as the Dashboard starts. Put them in the media folder's videos folder to choose one here."
- Then a Row (settings.md 4.3) for **Off**, and one for each video **found**, in the Bundle's order, 74 apart from y 270.58.
- The Row of what plays is ticked: the choice if its video was found, else Off. A choice whose video has gone is kept, and Off is ticked while it is gone.
- The page opens with focus on the ticked Row. Up and Down move, stopping at the ends; Left and Right do nothing; B is Personalisation, on Boot video.
- A on a Row makes it the choice, at once and saved: the tick moves, focus stays, no sound beyond the move's, no preview. It plays from the next start.

The Bundle lists all nine whatever is on the PC, its videos being in its package. Here they are the user's to bring, so only those found are listed.

The choice is the settings table's `startup.boot_video` row (settings.md 16.4), its value one of the names in section 1's first column; with no row it is Xbox Series X. The log says `settings: startup.boot_video is <name>` as the Dashboard starts with one set, and `settings: startup.boot_video = <name>` as one is saved. `--log-focus` says `focus: Settings: Boot video: <Row's name>`, with `; ticked` after it on the ticked Row.

## 3. Not playing it

- **`--no-startup`** starts at Home with no Start-up, whatever is chosen, the Profile signed in last signed in and greeted once Home has come to rest, as before Start-up was built. Every e2e payload starts so but `boot-video` and `whos-playing` (`Start-Dashboard -Startup`); tests and `--screenshot` never have Start-up.
- **Anywhere but Windows** the video cannot play (`media::Video` fails at once): Start-up goes on at once. `--screenshot` draws Home as ever.

## 4. After the Boot video

`Dashboard::after_boot_video` (`crates/screens/src/lib.rs`) is where Start-up goes on after the video, or at once with it Off or not found: Who's playing today? if Settings has it ask (section 9), else Signing you in for the Profile signed in last. The order is the Bundle's `StartupSequence`: `BootVideo`, `WhosPlaying`, `SigningIn` (`startup::Phase`). Not asking, it goes from the video straight to Signing you in, which still shows its three seconds.

With **one Profile** Who's playing today? asks all the same, as the Bundle does. A library with no Profiles (no database: tests, `--screenshot`) shows the one the Dashboard shows, its store's user, and A signs nobody in.

## 5. Who's playing today?

StartupWindow.xaml. Over everything in Home's window (the Overlay and the Toast strip are not made until Start-up is over), on the 1920x1080 canvas centred across and at the top, as every screen (docs/ui.md):

- **Behind**: black, then Home's swirl, fading in over 0.3 s as Who's playing today? first shows (the Bundle's swirl fade). Nothing of Home shows.
- **Title**: "Who's playing today?", Bahnschrift (`Font::Display`) 60, white, centred across, top at y 150. It stays on Signing you in.
- **The row**: each Profile in a cell 420 wide, the focused one's centred across (x 960); the row slides 0.22 s CubicOut as focus moves (in place under Reduce motion). With many Profiles cells run off the screen's edges. Top at y 340.
- **A Profile**: its picture (profiles.md 3), round, 380 across focused and 300 (0.79) otherwise, centred on the cell at y 530, growing or shrinking over 0.15 s. The focused one wears the focus ring: 7 thick, #2CA243, 12 outside the picture (404 across), with a soft glow, its opacity following the growth. Under the picture, at y 766: the gamertag, Segoe UI (`Font::Ui`) 30, white, centred, 400 wide, one line ending in an ellipsis.
- **Keys**: Left and Right move along, with the Guide's move sound (the Bundle's `Navigate`), stopping at the ends in silence; a held direction does not repeat. **A** signs the focused Profile in with the Guide's select sound (`Enter`), then the sign-in sound (`Login`: `Sound::Login`, `Login.wav` in the media folder, none if it is not there). **B, the Guide button and every other button do nothing**: someone has to sign in, as in the Bundle.

`--log-focus` says `focus: Startup: WhosPlaying "<gamertag>"` for the focused Profile.

## 6. Signing you in, and the end of Start-up

The Profile is signed in **at once** as Signing you in shows (`Dashboard::sign_in`: everything shows it, and the database remembers it: profiles.md 6), as the Bundle's core does.

- **Picture**: the Profile's alone, 380 round, centred across at y 540, under the same title and swirl.
- **The Spinner**, round the picture: 404 across, 7 thick, flat ends, turning without a rest (the Bundle's `LoadingSpinner` with `Unbroken`: spinner.rs's Spinner with no wait for its head, 1.25 times as slow, a green tail ahead of white; `spinner::unbroken_ring`).
- **Under it**: the gamertag, Segoe UI SemiBold 30 at y 746, and "Signing you in", Segoe UI 26, 8 under it, both white and centred.
- **For 3 s** (the Bundle's `SigningInDuration`), by the Dashboard's clock, which starts as the window has been made. Nothing the pad does changes it.
- **Then Start-up is over**: Home shows at once, focus where it was, Start-up's swirl alone fading out over it in 0.25 s (the Bundle hides the window's content at once and fades only the swirl). The Overlay and the Toast strip are made, and the **greeting**, the "Hi X!" Toast for the Profile just signed in, shows; then every Toast that came while Start-up showed, in order. The Guide button during the greeting opens the Guide on Profile & system, as ever (guide.md 9).

`--log-focus` says `focus: Startup: SigningIn "<gamertag>"`.

## 7. Switch profile

The Guide's Switch profile (guide.md 4.3), with A: the Guide goes **at once**, with no slide (the Bundle's `DismissAtOnce`), with any dialog or Flyout and the Toast on screen, and Who's playing today? asks with no Boot video, **whatever Settings says of asking**, focus on the Profile signed in. Then Signing you in, Home and the greeting, as at the start. A dialog page on Home goes too; the pages under it stay. B does nothing, as at the start. Over a running game Switch profile is greyed out and does nothing.

## 8. Waking from sleep

When Windows wakes from sleep, Who's playing today? asks again, without the Boot video, **if Settings has it ask**; with Sign in the last profile nothing happens. Over a running game the Dashboard first comes in front of it, as Home from the Guide does. A wake while Start-up already shows changes nothing. An Idle screen that was up goes at once. Waking the **Idle screen** (idle-screen.md) is not waking from sleep, and never asks. The Idle screen's count waits while any of Start-up shows, and starts from zero as Home shows after it.

The wake is heard from `PowerRegisterSuspendResumeNotification`, a callback that needs no window, on `PBT_APMRESUMEAUTOMATIC` (the same event as `WM_POWERBROADCAST`'s, which only a top-level window asking for it gets), behind `shell::wake::Wakes`. For a test, a Dashboard started with `--allow-toast-posts` takes a pretended wake from `standin wake` (a `WM_COPYDATA` to the Toast strip's window, `model::post::WAKE_ID`), which goes the same way. Sleeping the machine itself is checked by hand on the TV PC.

## 9. Settings > Personalisation > Who's playing today?

Personalisation's box at (0, 1), U+E748, the Bundle's place (settings.md 6.2, 6.4). Its page, as Boot video's:

- y 210: "Choose who's playing as the Dashboard starts and wakes from sleep."
- Two Rows: **Ask every time** (the default) and **Sign in the last profile**, the one in force ticked. The page opens with focus on it; Up and Down move; A chooses, at once and saved; B is Personalisation, on Who's playing today?.

The choice is the settings table's `startup.ask_whos_playing` row (settings.md 16.4), `true` or `false`; with no row it asks. The log says `settings: startup.ask_whos_playing = false` as Sign in the last profile is chosen, and `settings: startup.ask_whos_playing is false` at the next start. `--log-focus` says `focus: Settings: Who's playing today?: <Row's name>`, with `; ticked` after it on the ticked Row.

## 10. Differences from the Bundle

- **Sounds**: the Bundle's own Navigate, Enter and Login. The port plays the Guide's move and select sounds for the first two and `Login.wav` for the third, from the media folder like every sound.
- **The swirl** is Home's swirl drawn behind, where the Bundle's start-up window has its own copy of the animation.
- **The Spinner's ends** are flat, as the Bundle's `LoadingSpinner`'s; the Guide's Spinner elsewhere keeps its round ends.
- **Waking over a game**: the Bundle activates Playnite's window; the port brings Home in front of the game as Home from the Guide does, the game left running.
- **Signing you in's three seconds** are counted by the Dashboard's clock from the press, which a slow renderer's frames do not stretch; the Bundle's timer is the same, by the wall clock.

## 11. The log

Times are milliseconds since the process started. The `boot-video` and `whos-playing` e2e payloads judge by these lines; each Start-up line is said once the frame it is about has been presented.

| Line | When |
|---|---|
| `startup: boot videos <Name>, <Name> from <folder>` / `startup: no boot videos (looked in <folders>)` | As the Dashboard starts, before the window |
| `startup: off (--no-startup)` | Start-up is not wanted |
| `startup: boot video Off` | Off is chosen |
| `startup: boot video <Name> not found, going straight on` | The chosen video's file is not there |
| `startup: boot video <Name> from <file>` | It is to play |
| `startup: first video frame at <N> ms` | Its first frame has been presented |
| `startup: video ended at <N> ms (<n> frames shown, <n> dropped)` | It played to its end |
| `startup: video skipped at <N> ms` | A press skipped it |
| `startup: video failed at <N> ms: <why>` | It could not be played |
| `startup: video stopped after 20 s at <N> ms` | It ran past the deadline |
| `startup: who's playing shown at <N> ms, <n> profiles, focus on "<gamertag>"` | Who's playing today?'s first frame has been presented, each time it asks |
| `startup: signing in "<gamertag>" at <N> ms` | Signing you in's first frame has been presented |
| `startup: signed in at <N> ms` | Start-up is over, Home's frame presented |
| `startup: woke from sleep at <N> ms: Who's playing today? asks` / `...: nothing asked` | Windows woke from sleep (or `standin wake`) |
| `wake: the machine woke from sleep at <N> ms` / `wake: woken by a post, as from sleep at <N> ms` | What heard the wake (`shell::wake`) |
| `startup: Home's first frame at <N> ms` | Home's first frame has been presented: always, with Start-up or without |

The `media` crate says what it opened and how (`video: opened ...`, `video: sound started on ...`), as the spike did.
