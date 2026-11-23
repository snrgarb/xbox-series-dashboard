//! Start-up's Boot video, as the executable plays it (docs/spec/start-up.md): which videos the media folder has,
//! which one plays, and the playing of it through the `media` crate into a texture the Dashboard draws. The
//! Dashboard (`screens::startup`) owns the screen, the pad and the Guide button meanwhile, and says when a press
//! skipped it.
//!
//! The file is opened before the window, so that its decoding starts while the window and the GPU are made.
//! Every step is said in the log with its time since the process started (`startup: ...`), which the e2e payload
//! `boot-video` reads.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use media::{Status, Video};
use render::gpu::Gpu;
use screens::{BootVideo, Dashboard, Found, Options};
use ui::{Host, TexId};

use crate::media_folder::{self, Kind, Place};
use crate::sounds;
use crate::store::Store;
use crate::{since_start, Args};

/// The Bundle's `BootVideoDeadline`: a video still playing this long after it began is stopped, and Start-up goes
/// on as if it had ended.
pub const DEADLINE: Duration = Duration::from_secs(20);

/// The boot videos in the first of the media folder's places that has any (`media_folder`): what Settings lists,
/// and where each is.
pub fn find(places: &[Place]) -> (Found, Vec<(BootVideo, PathBuf)>) {
    for place in places {
        let files: Vec<(BootVideo, PathBuf)> =
            BootVideo::ALL.into_iter().filter_map(|video| Some((video, place.find(&video.file()?)?))).collect();
        if !files.is_empty() {
            let found = files.iter().fold(Found::NONE, |found, (video, _)| found.with(*video));
            let names: Vec<&str> = files.iter().map(|(video, _)| video.key()).collect();
            eprintln!("startup: boot videos {} from {}", names.join(", "), place.name());
            return (found, files);
        }
    }
    eprintln!("startup: no boot videos (looked in {})", media_folder::looked(places));
    (Found::NONE, Vec::new())
}

/// What Start-up begins with, given whether it is wanted, the choice, and the files found: the file of the video to
/// play, if one is to. Says which in the log.
pub fn choose(no_startup: bool, chosen: BootVideo, files: &[(BootVideo, PathBuf)]) -> Option<PathBuf> {
    if no_startup {
        eprintln!("startup: off (--no-startup)");
        return None;
    }
    if chosen == BootVideo::Off {
        eprintln!("startup: boot video Off");
        return None;
    }
    match files.iter().find(|(video, _)| *video == chosen) {
        Some((_, path)) => {
            eprintln!("startup: boot video {} from {}", chosen.key(), path.display());
            Some(path.clone())
        }
        None => {
            eprintln!("startup: boot video {} not found, going straight on", chosen.key());
            None
        }
    }
}

/// The Boot video being played.
pub struct Playing {
    video: Video,
    began: Instant,
    /// The texture its frames go into, made once its size is known, with that size.
    texture: Option<(TexId, (u32, u32))>,
    /// A frame went into the texture for the frame being drawn: once that is presented, the clock runs from it.
    uploaded: bool,
    /// The first frame has been presented, and said.
    shown: bool,
}

/// How the Boot video ended, for the log and for the Dashboard.
#[derive(Debug, PartialEq)]
pub enum End {
    Ended,
    Skipped,
    Failed(String),
    TooLong,
}

impl Playing {
    /// Finds the media folder's boot videos, tells the Dashboard which there are, and opens the one to play if one
    /// is: Start-up begins, with it or without. Before the window, so the decoding starts meanwhile.
    pub fn start(args: &Args, store: &Store, options: &mut Options) -> Option<Playing> {
        let data_dir = args.data_dir.clone().or_else(library::data_dir);
        let playnite = media_folder::playnite_folders();
        let (found, files) =
            find(&media_folder::places(Kind::Videos, args.media.as_deref(), data_dir.as_deref(), &playnite));
        options.boot_videos = found;
        let file = choose(args.no_startup, options.boot_video, &files)?;
        Some(Playing::open(&file, store))
    }

    fn open(file: &Path, store: &Store) -> Playing {
        // The Dashboard's own sound: its volume, and none while it is muted.
        let volume = sounds::volume(store.setting(sounds::VOLUME).as_deref());
        let muted = sounds::muted(store.setting(sounds::MUTED).as_deref());
        let options = media::Options { audio: !muted, volume, ..media::Options::default() };
        Playing {
            video: Video::open(file, options),
            began: Instant::now(),
            texture: None,
            uploaded: false,
            shown: false,
        }
    }

    /// Before a frame is drawn: the frame that has come due goes into the texture, which is made the first time
    /// there is a picture, and the Dashboard told of it. `refresh` is a refresh's length, in seconds.
    pub fn before_frame(&mut self, host: &mut Host, gpu: &Gpu, dashboard: &mut Dashboard, refresh: f64) {
        self.uploaded = false;
        let Some(picture) = self.video.picture() else { return };
        let Some(frame) = self.video.due(refresh / 2.0) else { return };
        let (width, height) = picture.size;
        let tex = match self.texture {
            Some((tex, size)) if size == picture.size => tex,
            _ => {
                let tex = host.renderer.add_video_texture(&gpu.device, width, height);
                self.texture = Some((tex, picture.size));
                dashboard.boot_video_picture(tex, picture.aspect());
                tex
            }
        };
        let stride = if height > 0 { (frame.pixels.len() / height as usize) as u32 } else { width * 4 };
        host.renderer.update_video_texture(&gpu.queue, tex, &frame.pixels, stride);
        self.video.recycle(frame.pixels);
        self.uploaded = true;
    }

    /// The GPU was rebuilt: the texture is gone with the old device, and is made again for the next picture.
    pub fn gpu_lost(&mut self) {
        self.texture = None;
    }

    /// After the frame is presented: the clock and the sound start from the first picture shown, which is said.
    pub fn after_present(&mut self) {
        if self.uploaded {
            self.video.presented();
            if !self.shown {
                self.shown = true;
                eprintln!("startup: first video frame at {:.0} ms", since_start());
            }
        }
    }

    /// How it has ended, if it has: by itself, by a press the Dashboard took (`playing` is false), by failing, or by
    /// running past the deadline. Said in the log.
    pub fn ended(&mut self, playing: bool) -> Option<End> {
        let end = if !playing {
            End::Skipped
        } else {
            match self.video.status() {
                Status::Ended => End::Ended,
                Status::Failed(why) => End::Failed(why),
                _ if self.began.elapsed() >= DEADLINE => End::TooLong,
                Status::Opening | Status::Playing => return None,
            }
        };
        let at = since_start();
        let (shown, dropped) = (self.video.shown, self.video.dropped);
        match &end {
            End::Ended => eprintln!("startup: video ended at {at:.0} ms ({shown} frames shown, {dropped} dropped)"),
            End::Skipped => eprintln!("startup: video skipped at {at:.0} ms"),
            End::Failed(why) => eprintln!("startup: video failed at {at:.0} ms: {why}"),
            End::TooLong => eprintln!("startup: video stopped after {} s at {at:.0} ms", DEADLINE.as_secs()),
        }
        Some(end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(dir: &Path, names: &[&str]) {
        std::fs::create_dir_all(dir).unwrap();
        for name in names {
            std::fs::write(dir.join(name), b"not really a video").unwrap();
        }
    }

    #[test]
    fn the_videos_are_the_first_places_by_the_bundles_names_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let (data, playnite) = (dir.path().join("data"), dir.path().join("Playnite"));
        let bundle = playnite.join("Extensions").join(media_folder::BUNDLE_EXTENSION).join("Videos");
        files(&bundle, &["XboxSeriesX.mp4", "Xbox360.mp4", "Intro.mp4"]);
        let places =
            |data: &Path| media_folder::places(Kind::Videos, None, Some(data), std::slice::from_ref(&playnite));
        let (found, list) = find(&places(&data));
        assert_eq!(found.videos().collect::<Vec<_>>(), [BootVideo::Xbox360, BootVideo::XboxSeriesX]);
        assert_eq!(list[1], (BootVideo::XboxSeriesX, bundle.join("XboxSeriesX.mp4")));

        // One in the data folder: only that folder counts.
        files(&data.join("media").join("videos"), &["XboxOne.mp4"]);
        assert_eq!(find(&places(&data)).0.videos().collect::<Vec<_>>(), [BootVideo::XboxOne]);

        // --media: its videos folder, or itself; nowhere else.
        let media = dir.path().join("media");
        files(&media, &["XboxSeriesS.mp4"]);
        let only = |media: &Path| find(&media_folder::places(Kind::Videos, Some(media), Some(&data), &[])).0;
        assert_eq!(only(&media).videos().collect::<Vec<_>>(), [BootVideo::XboxSeriesS]);
        files(&media.join("videos"), &["Xbox.mp4"]);
        assert_eq!(only(&media).videos().collect::<Vec<_>>(), [BootVideo::Xbox]);
        assert!(only(&dir.path().join("empty")).is_empty());
    }

    #[test]
    fn the_video_chosen_plays_if_it_is_there_and_startup_is_wanted() {
        let files = vec![(BootVideo::XboxOne, PathBuf::from("XboxOne.mp4"))];
        assert_eq!(choose(false, BootVideo::XboxOne, &files), Some(PathBuf::from("XboxOne.mp4")));
        assert_eq!(choose(true, BootVideo::XboxOne, &files), None, "--no-startup");
        assert_eq!(choose(false, BootVideo::Off, &files), None);
        assert_eq!(choose(false, BootVideo::XboxSeriesX, &files), None, "chosen, and not found");
    }

    #[cfg(not(windows))]
    #[test]
    fn anywhere_but_windows_the_video_fails_at_once_and_startup_goes_on() {
        let mut playing = Playing {
            video: Video::open(Path::new("XboxSeriesX.mp4"), media::Options::default()),
            began: Instant::now(),
            texture: None,
            uploaded: false,
            shown: false,
        };
        assert!(matches!(playing.ended(true), Some(End::Failed(_))));
        assert_eq!(playing.ended(false), Some(End::Skipped));
    }
}
