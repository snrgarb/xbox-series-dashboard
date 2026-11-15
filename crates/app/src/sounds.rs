//! The Dashboard's sounds: where their files are found, and the playing of them.
//!
//! The files are the console's, which belong to Microsoft and are not in this repository (#10). They are looked
//! for when the window starts, by the names the Bundle gives them (`model::Sound::files`), in the first of these
//! places that has any of them:
//!
//! 1. `--media <dir>`: `<dir>\sounds` if there is one, else `<dir>` itself. With `--media`, nowhere else.
//! 2. `<data dir>\media\sounds`, the data folder being `--data-dir`'s or `%LOCALAPPDATA%\XboxSeriesDashboard`.
//! 3. The Bundle, where it is installed in Playnite: its extension's `Sounds` and its theme's `audio`, under
//!    `%APPDATA%\Playnite` (an installed Playnite) or `%LOCALAPPDATA%\Playnite` (a portable one left where the
//!    installer would put it).
//!
//! A sound with no file there is silent, and with no place at all so is the Dashboard.
use std::path::{Path, PathBuf};

use audio::Audio;
use model::Sound;

use crate::store::Store;
use crate::Args;

/// The Bundle's extension and theme, as Playnite names their folders.
const BUNDLE_EXTENSION: &str = "XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b";
const BUNDLE_THEME: &str = "XboxSeries_Dashboard";

/// The volume, 0 to 1, in the library database's settings.
const VOLUME: &str = "sound.volume";
/// Whether sounds are off, `true` or `false`, in the same place.
const MUTED: &str = "sound.muted";

pub struct Sounds {
    audio: Audio,
    /// Whose turn it is among each sound's files, by the sound's place in `Sound::ALL`.
    turns: [usize; Sound::ALL.len()],
}

/// A place sounds may be: one folder, or the Bundle's two.
struct Place {
    folders: Vec<PathBuf>,
}

impl Place {
    fn find(&self, file: &str) -> Option<PathBuf> {
        self.folders.iter().map(|folder| folder.join(file)).find(|path| path.is_file())
    }

    fn name(&self) -> String {
        let names: Vec<String> = self.folders.iter().map(|folder| folder.display().to_string()).collect();
        names.join(" and ")
    }
}

/// The places to look, in order.
fn places(media: Option<&Path>, data_dir: Option<&Path>, playnite: &[PathBuf]) -> Vec<Place> {
    if let Some(media) = media {
        let sounds = media.join("sounds");
        return vec![Place { folders: vec![if sounds.is_dir() { sounds } else { media.to_path_buf() }] }];
    }
    let mut places = Vec::new();
    places.extend(data_dir.map(|dir| Place { folders: vec![dir.join("media").join("sounds")] }));
    places.extend(playnite.iter().map(|playnite| Place {
        folders: vec![
            playnite.join("Extensions").join(BUNDLE_EXTENSION).join("Sounds"),
            playnite.join("Themes").join("Fullscreen").join(BUNDLE_THEME).join("audio"),
        ],
    }));
    places
}

/// Where Playnite keeps its extensions and themes, if it is on this machine.
fn playnite_folders() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if !cfg!(windows) {
        return Vec::new();
    }
    ["APPDATA", "LOCALAPPDATA"].iter().filter_map(|name| Some(var(name)?.join("Playnite"))).collect()
}

/// Loads every file of every sound from the first of `places` that has any. Says where they came from, or that
/// there are none.
fn load(audio: &mut Audio, places: &[Place]) {
    for place in places {
        let found: Vec<(&str, PathBuf)> =
            Sound::all_files().into_iter().filter_map(|file| Some((file, place.find(file)?))).collect();
        if found.is_empty() {
            continue;
        }
        let mut loaded = 0;
        for (file, path) in &found {
            match audio.load(file, path) {
                Ok(()) => loaded += 1,
                Err(e) => eprintln!("audio: {}: {e}", path.display()),
            }
        }
        eprintln!("audio: {loaded} sounds from {}", place.name());
        return;
    }
    let looked: Vec<String> = places.iter().map(Place::name).collect();
    eprintln!(
        "audio: no sounds (looked in {})",
        if looked.is_empty() { "no folder".into() } else { looked.join(", ") }
    );
}

/// A volume as the settings table holds it: a number from 0 to 1.
fn volume(setting: Option<&str>) -> f32 {
    let Some(text) = setting else { return 1.0 };
    match text.trim().parse::<f32>() {
        Ok(volume) if volume.is_finite() => volume.clamp(0.0, 1.0),
        _ => {
            eprintln!("audio: {VOLUME} is {text:?}, not a number from 0 to 1: full volume");
            1.0
        }
    }
}

fn muted(setting: Option<&str>) -> bool {
    matches!(setting.map(|text| text.trim().to_ascii_lowercase()).as_deref(), Some("true" | "1" | "on" | "yes"))
}

impl Sounds {
    /// Finds the sounds and reads them into memory, at the volume the settings say. No device is opened until
    /// the first one plays.
    pub fn open(args: &Args, store: &Store) -> Sounds {
        let mut audio = Audio::new();
        let data_dir = args.data_dir.clone().or_else(library::data_dir);
        load(&mut audio, &places(args.media.as_deref(), data_dir.as_deref(), &playnite_folders()));
        let (level, off) = (volume(store.setting(VOLUME).as_deref()), muted(store.setting(MUTED).as_deref()));
        audio.set_volume(level);
        audio.set_muted(off);
        if off || level < 1.0 {
            eprintln!("audio: volume {level}{}", if off { ", muted" } else { "" });
        }
        audio.log_plays(args.log_sounds);
        Sounds { audio, turns: [0; Sound::ALL.len()] }
    }

    /// Plays the sound's file, or of several the one whose turn it is. Returns at once, with how many seconds of
    /// sound it started: 0 when there is no file, no device or no volume.
    pub fn play(&mut self, sound: Sound) -> f32 {
        let files = sound.files();
        let turn = &mut self.turns[Sound::ALL.iter().position(|s| *s == sound).unwrap_or(0)];
        let seconds = self.audio.play(files[*turn % files.len()]);
        *turn = (*turn + 1) % files.len();
        seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of tones under some of the sounds' names.
    fn tones(dir: &Path, names: &[&str]) {
        audio::tones::write(dir, names).unwrap();
    }

    fn loaded(places: &[Place]) -> Vec<&'static str> {
        let mut audio = Audio::new();
        load(&mut audio, places);
        Sound::all_files().into_iter().filter(|file| audio.has(file)).collect()
    }

    #[test]
    fn the_data_folder_comes_before_the_bundle_and_the_first_place_with_any_sound_is_the_only_one() {
        let dir = tempfile::tempdir().unwrap();
        let (data, playnite) = (dir.path().join("data"), dir.path().join("Playnite"));
        // The Bundle's two folders, as Playnite lays them out.
        let extension = playnite.join("Extensions").join(BUNDLE_EXTENSION).join("Sounds");
        let theme = playnite.join("Themes").join("Fullscreen").join(BUNDLE_THEME).join("audio");
        let navigate = ["Navigate1.wav", "Navigate2.wav", "Navigate3.wav", "Navigate4.wav"];
        tones(&extension, &navigate);
        let rest = ["Enter.wav", "OpenMenu.wav", "PromptOpen.wav", "PromptClose.wav", "PowerOff.wav", "Notification.wav"];
        tones(&extension, &rest);
        tones(&theme, &["navigation.wav", "activation.wav"]);

        // Nothing in the data folder: the Bundle's, every one.
        let from = |data: &Path| places(None, Some(data), std::slice::from_ref(&playnite));
        assert_eq!(loaded(&from(&data)), Sound::all_files());

        // One sound in the data folder: that folder is used, and what it lacks is silent.
        tones(&data.join("media").join("sounds"), &["navigation.wav"]);
        assert_eq!(loaded(&from(&data)), ["navigation.wav"]);
    }

    #[test]
    fn media_overrides_everything_and_may_be_the_folder_or_hold_a_sounds_folder() {
        let dir = tempfile::tempdir().unwrap();
        let (data, media) = (dir.path().join("data"), dir.path().join("media"));
        tones(&data.join("media").join("sounds"), &Sound::all_files());
        tones(&media, &["Enter.wav"]);
        assert_eq!(loaded(&places(Some(&media), Some(&data), &[])), ["Enter.wav"]);
        tones(&media.join("sounds"), &["OpenMenu.wav"]);
        assert_eq!(loaded(&places(Some(&media), Some(&data), &[])), ["OpenMenu.wav"]);
        // A --media folder with nothing in it is silence, not a reason to look elsewhere.
        let empty = dir.path().join("empty");
        assert!(loaded(&places(Some(&empty), Some(&data), &[])).is_empty());
    }

    #[test]
    fn a_file_that_is_not_a_sound_is_left_out_and_the_rest_play() {
        let dir = tempfile::tempdir().unwrap();
        tones(dir.path(), &["navigation.wav", "activation.wav"]);
        std::fs::write(dir.path().join("activation.wav"), b"not a sound").unwrap();
        assert_eq!(loaded(&places(Some(dir.path()), None, &[])), ["navigation.wav"]);
        assert!(loaded(&[]).is_empty());
    }

    #[test]
    fn the_settings_are_read_as_written() {
        assert_eq!(volume(None), 1.0);
        assert_eq!(volume(Some("0.25")), 0.25);
        assert_eq!(volume(Some(" 0 ")), 0.0);
        assert_eq!(volume(Some("7")), 1.0);
        assert_eq!(volume(Some("-1")), 0.0);
        assert_eq!(volume(Some("loud")), 1.0);
        assert_eq!(volume(Some("NaN")), 1.0);
        assert!(!muted(None) && !muted(Some("false")) && !muted(Some("0")) && !muted(Some("")));
        assert!(muted(Some("true")) && muted(Some("TRUE")) && muted(Some("1")));
    }

    #[test]
    fn a_sound_of_several_files_takes_them_in_turn() {
        let mut sounds = Sounds { audio: Audio::new(), turns: [0; Sound::ALL.len()] };
        let at = Sound::ALL.iter().position(|s| *s == Sound::GuideMove).unwrap();
        for turn in [1, 2, 3, 0, 1] {
            sounds.play(Sound::GuideMove);
            sounds.play(Sound::Move);
            assert_eq!(sounds.turns[at], turn);
        }
    }
}
