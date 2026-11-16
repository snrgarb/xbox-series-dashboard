# Start-up specification

Start-up is the console's start-up sequence, full screen before Home (the Bundle's glossary, `CONTEXT.md`: Start-up, Boot video, Start-up's first frame). In the Bundle it is the Boot video, then Who's playing today? (or, with that not asking, Signing you in), then Home with the "Hi X!" Toast. #126 built its first part, the **Boot video**; Who's playing today? and Signing you in are #127's, and go in at the hook in section 4.

Sources: the Bundle's `StartupSequence` and `BootVideo` (C/ExtensionCore.Startup.cs:147-215, 337-384), its Personalisation page (E/PersonalisationPane.xaml:281-286, E/PersonalisationPane.xaml.cs:29-40, 355-359). Code: `crates/screens/src/startup.rs` (the sequence, as the core keeps it), `crates/screens/src/settings/boot_video.rs` (its Settings page), `crates/app/src/startup.rs` (the video found, chosen and played), `crates/media` (Media Foundation playback, from the `video-spike`; docs/research/video-media-foundation.md).

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
- **End.** At the file's end, at a skip, when the file cannot be played (no Media Foundation, a file it cannot decode), or 20 s after it began (the Bundle's `BootVideoDeadline`), Start-up goes on (section 4).
- **No preview.** Settings never plays it.

### 1.3 What goes on behind it

The file is opened before the window, so it decodes while the window and the GPU are made. As soon as the window's first frame is out (black, or the video's first frame) the sources are read, the stores' apps listed and the Queue followed, exactly as they are after Home's first frame with no Start-up. Home is drawn once, unseen, under the video's first frame, so its art is asked for and decoded while the video plays. When the video ends, Home's first frame is whole.

Held back until Start-up is over: the Overlay and the Toast strip, so the greeting's Toast and `--toast`'s notifications (the Bundle holds Toasts back during Start-up), and the art lookup online.

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

- **`--no-startup`** starts at Home with no Start-up, whatever is chosen. Every e2e payload starts so but `boot-video` (`Start-Dashboard -Startup`); tests and `--screenshot` never have Start-up.
- **Anywhere but Windows** the video cannot play (`media::Video` fails at once): Start-up goes on at once. `--screenshot` draws Home as ever.

## 4. After the Boot video: the hook for #127

`Dashboard::after_boot_video` (`crates/screens/src/lib.rs`) is where Start-up goes on after the video, or at once with it Off or not found. #127 puts Who's playing today? and Signing you in there, as `startup::Phase`s of their own after `Phase::BootVideo`, ending Start-up as Signing you in has shown (the Bundle: 3 s, then a 250 ms fade to Home, then the "Hi X!" Toast).

Until then Start-up ends with the video: Home shows, and once Home has come to rest (or 2 s after its first frame) the executable makes the Overlay and greets the Profile signed in last, the "Hi X!" Toast, as it did before Start-up was built.

## 5. The log

Times are milliseconds since the process started. The `boot-video` e2e payload judges by these lines.

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
| `startup: Home's first frame at <N> ms` | Home's first frame has been presented: always, with Start-up or without |

The `media` crate says what it opened and how (`video: opened ...`, `video: sound started on ...`), as the spike did.
