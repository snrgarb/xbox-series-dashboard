//! `standin media`: a media session of its own that Windows lists among every app's, as a music player's is, for
//! the Guide's Now Playing card to show and its controls to reach (docs/spec/now-playing.md). It is a hidden
//! top-level window's System Media Transport Controls (`ISystemMediaTransportControlsInterop::GetForWindow`), which
//! Windows names by this program, `standin.exe`: the Dashboard shows it as "Standin".
//!
//! It plays, or with --paused stands paused, a track with a title, an artist, a picture and a place in it. Each
//! button Windows passes on is said in the log (`media: pressed Play`, `Pause`, `Next` or `Previous`) and answered
//! as a player would: Play and Pause play and pause it, Next moves to the next track (its title gets " (2)" and so
//! on, from its start), Previous to the last. With --no-previous Previous is off, which the card shows dimmed.
use std::path::PathBuf;

/// What `standin media` was asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaArgs {
    pub title: String,
    pub artist: String,
    /// A picture's file; none is a square drawn here.
    pub picture: Option<PathBuf>,
    /// Where in the track it starts, and the track's length; a length of 0 is a track with none, as a stream is.
    pub position: u64,
    pub length: u64,
    pub paused: bool,
    pub no_previous: bool,
    /// How long it stays, in case a test leaves it behind.
    pub seconds: u32,
}

impl Default for MediaArgs {
    fn default() -> Self {
        Self {
            title: "Stand-in Song".into(),
            artist: "The Stand-ins".into(),
            picture: None,
            position: 65,
            length: 200,
            paused: false,
            no_previous: false,
            seconds: 600,
        }
    }
}

impl MediaArgs {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut out = Self::default();
        let mut args = args.iter();
        let number = |value: &String| value.parse::<u64>().map_err(|e| format!("bad number {value}: {e}"));
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or(format!("{arg} needs a value"));
            match arg.as_str() {
                "--title" => out.title = value()?.clone(),
                "--artist" => out.artist = value()?.clone(),
                "--picture" => out.picture = Some(value()?.into()),
                "--position" => out.position = number(value()?)?,
                "--length" => out.length = number(value()?)?,
                "--seconds" => out.seconds = number(value()?)?.min(u64::from(u32::MAX)) as u32,
                "--paused" => out.paused = true,
                "--no-previous" => out.no_previous = true,
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(out)
    }

    /// The title of the track `skipped` tracks on from the first: " (2)" after one Next, and so on.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn title_after(&self, skipped: i64) -> String {
        if skipped == 0 {
            self.title.clone()
        } else {
            format!("{} ({})", self.title, skipped + 1)
        }
    }
}

/// The picture the session shows when none is given: a square in two greens with a white frame, as PNG.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn drawn_picture() -> Vec<u8> {
    let image = image::RgbImage::from_fn(256, 256, |x, y| {
        if x < 8 || y < 8 || x >= 248 || y >= 248 {
            image::Rgb([255, 255, 255])
        } else if (x / 32 + y / 32) % 2 == 0 {
            image::Rgb([16, 124, 16])
        } else {
            image::Rgb([44, 162, 67])
        }
    });
    let mut bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).expect("a PNG in memory");
    bytes
}

#[cfg(windows)]
pub use self::windows::run;

#[cfg(not(windows))]
pub fn run(_: &MediaArgs) -> i32 {
    crate::note("media: Windows only");
    2
}

#[cfg(windows)]
mod windows {
    use std::ptr::{null, null_mut};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use windows::core::{factory, HSTRING};
    use windows::Foundation::{TimeSpan, TypedEventHandler};
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls as Controls,
        SystemMediaTransportControlsButton as ControlsButton, SystemMediaTransportControlsTimelineProperties,
    };
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference};
    use windows::Win32::System::WinRT::{ISystemMediaTransportControlsInterop, RoInitialize, RO_INIT_MULTITHREADED};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{drawn_picture, MediaArgs};
    use crate::note;

    const TIME_UP: usize = 1;

    /// Where the session is: which track, whether it plays, and its place, as of when.
    struct Playing {
        skipped: i64,
        playing: bool,
        position: Duration,
        since: Instant,
    }

    impl Playing {
        fn now(&self) -> Duration {
            if self.playing {
                self.position + self.since.elapsed()
            } else {
                self.position
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn span(duration: Duration) -> TimeSpan {
        TimeSpan { Duration: (duration.as_nanos() / 100) as i64 }
    }

    pub fn run(args: &MediaArgs) -> i32 {
        match session(args) {
            Ok(()) => 0,
            Err(e) => {
                note(&format!("media: no session: {e}"));
                1
            }
        }
    }

    fn session(args: &MediaArgs) -> windows::core::Result<()> {
        // SAFETY: once, on this thread, before any WinRT object; the window is ours and lives until the pump ends.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
        let window = unsafe { open_window() };
        if window.is_null() {
            note("media: could not open its window");
            return Err(windows::core::Error::from_thread());
        }
        let interop = factory::<Controls, ISystemMediaTransportControlsInterop>()?;
        let controls: Controls = unsafe { interop.GetForWindow(windows::Win32::Foundation::HWND(window))? };
        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsNextEnabled(true)?;
        controls.SetIsPreviousEnabled(!args.no_previous)?;

        let playing = Arc::new(Mutex::new(Playing {
            skipped: 0,
            playing: !args.paused,
            position: Duration::from_secs(args.position),
            since: Instant::now(),
        }));
        let picture = match &args.picture {
            Some(file) => std::fs::read(file).unwrap_or_else(|e| {
                note(&format!("media: couldn't read {}: {e}; a square drawn in its place", file.display()));
                drawn_picture()
            }),
            None => drawn_picture(),
        };
        let thumbnail = {
            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream)?;
            writer.WriteBytes(&picture)?;
            writer.StoreAsync()?.join()?;
            writer.FlushAsync()?.join()?;
            writer.DetachStream()?;
            stream.Seek(0)?;
            RandomAccessStreamReference::CreateFromStream(&stream)?
        };
        let show = {
            let (controls, args, thumbnail) = (controls.clone(), args.clone(), thumbnail.clone());
            move |playing: &Playing| -> windows::core::Result<()> {
                let display = controls.DisplayUpdater()?;
                display.SetType(MediaPlaybackType::Music)?;
                let music = display.MusicProperties()?;
                let title = args.title_after(playing.skipped);
                music.SetTitle(&HSTRING::from(&title))?;
                music.SetArtist(&HSTRING::from(&args.artist))?;
                display.SetThumbnail(&thumbnail)?;
                display.Update()?;
                let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
                let length = Duration::from_secs(args.length);
                timeline.SetStartTime(span(Duration::ZERO))?;
                timeline.SetEndTime(span(length))?;
                timeline.SetMinSeekTime(span(Duration::ZERO))?;
                timeline.SetMaxSeekTime(span(length))?;
                let at = if length.is_zero() { Duration::ZERO } else { playing.now().min(length) };
                timeline.SetPosition(span(at))?;
                controls.UpdateTimelineProperties(&timeline)?;
                let status = if playing.playing { MediaPlaybackStatus::Playing } else { MediaPlaybackStatus::Paused };
                controls.SetPlaybackStatus(status)?;
                note(&format!(
                    "media: showing {title:?} by {:?}, {}, {} s of {} s",
                    args.artist,
                    if playing.playing { "playing" } else { "paused" },
                    at.as_secs(),
                    args.length
                ));
                Ok(())
            }
        };
        show(&playing.lock().unwrap())?;

        let pressed = {
            let (playing, show) = (playing.clone(), show.clone());
            TypedEventHandler::new(
                move |_,
                      pressed: windows::core::Ref<
                    windows::Media::SystemMediaTransportControlsButtonPressedEventArgs,
                >| {
                    let Some(pressed) = pressed.as_ref() else { return Ok(()) };
                    let button = pressed.Button()?;
                    let mut playing = playing.lock().unwrap();
                    let now = playing.now();
                    let name = match button {
                        ControlsButton::Play => {
                            *playing = Playing { playing: true, position: now, since: Instant::now(), ..*playing };
                            "Play"
                        }
                        ControlsButton::Pause => {
                            *playing = Playing { playing: false, position: now, since: Instant::now(), ..*playing };
                            "Pause"
                        }
                        ControlsButton::Next => {
                            let skipped = playing.skipped + 1;
                            *playing = Playing { skipped, position: Duration::ZERO, since: Instant::now(), ..*playing };
                            "Next"
                        }
                        ControlsButton::Previous => {
                            let skipped = (playing.skipped - 1).max(0);
                            *playing = Playing { skipped, position: Duration::ZERO, since: Instant::now(), ..*playing };
                            "Previous"
                        }
                        other => {
                            note(&format!("media: pressed button {}, which it does nothing for", other.0));
                            return Ok(());
                        }
                    };
                    note(&format!("media: pressed {name}"));
                    if let Err(e) = show(&playing) {
                        note(&format!("media: couldn't show what {name} did: {e}"));
                    }
                    Ok(())
                },
            )
        };
        controls.ButtonPressed(&pressed)?;
        note(&format!("media: session up for {} s", args.seconds));
        unsafe {
            SetTimer(window, TIME_UP, args.seconds.saturating_mul(1000).min(USER_TIMER_MAXIMUM), None);
            pump();
        }
        note("media: session over");
        Ok(())
    }

    /// A top-level window that never shows: the session's, which Windows names by this program.
    unsafe fn open_window() -> HWND {
        let instance = GetModuleHandleW(null());
        let class = wide("StandInMedia");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let title = wide("Stand-in media");
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            100,
            100,
            null_mut(),
            null_mut(),
            instance,
            null(),
        )
    }

    unsafe fn pump() {
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_TIMER if wparam == TIME_UP => {
                DestroyWindow(hwnd);
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
            }
            WM_DESTROY => PostQuitMessage(0),
            _ => return DefWindowProcW(hwnd, message, wparam, lparam),
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<MediaArgs, String> {
        MediaArgs::parse(&list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn its_arguments_and_their_defaults() {
        assert_eq!(args(&[]).unwrap(), MediaArgs::default());
        let parsed =
            args(&["--title", "Song", "--length", "0", "--paused", "--no-previous", "--seconds", "90"]).unwrap();
        assert_eq!((parsed.title.as_str(), parsed.length, parsed.paused, parsed.no_previous), ("Song", 0, true, true));
        assert_eq!(parsed.seconds, 90);
        assert!(args(&["--length", "x"]).is_err());
        assert!(args(&["--loud"]).is_err());
    }

    #[test]
    fn next_numbers_the_title_and_the_picture_is_a_png() {
        let media = MediaArgs::default();
        assert_eq!(media.title_after(0), "Stand-in Song");
        assert_eq!(media.title_after(1), "Stand-in Song (2)");
        assert!(drawn_picture().starts_with(b"\x89PNG"));
    }
}
