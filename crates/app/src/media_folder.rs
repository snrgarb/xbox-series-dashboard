//! The media folder: where the console's sounds and boot videos are looked for as the Dashboard starts. They are
//! Microsoft's, never in this repository (#10); the user's machine has them from the Bundle. Each kind is looked
//! for in the first of these places that has any of it:
//!
//! 1. `--media <dir>`: `<dir>\sounds` or `<dir>\videos` if there is one, else `<dir>` itself. With `--media`,
//!    nowhere else.
//! 2. `<data dir>\media\sounds` or `<data dir>\media\videos`, the data folder being `--data-dir`'s or
//!    `%LOCALAPPDATA%\XboxSeriesDashboard`.
//! 3. The Bundle, where it is installed in Playnite: its extension's `Sounds` and its theme's `audio` for sounds,
//!    its extension's `Videos` for boot videos, under `%APPDATA%\Playnite` (an installed Playnite) or
//!    `%LOCALAPPDATA%\Playnite` (a portable one left where the installer would put it).
use std::path::{Path, PathBuf};

/// The Bundle's extension and theme, as Playnite names their folders.
pub const BUNDLE_EXTENSION: &str = "XboxSeries_eb759ff1-325b-4c34-9a64-d9704f72612b";
pub const BUNDLE_THEME: &str = "XboxSeries_Dashboard";

/// What is looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sounds,
    Videos,
}

impl Kind {
    /// The folder of this kind in a media folder.
    fn folder(self) -> &'static str {
        match self {
            Kind::Sounds => "sounds",
            Kind::Videos => "videos",
        }
    }

    /// Where the Bundle keeps this kind, under Playnite's folder.
    fn bundle(self, playnite: &Path) -> Vec<PathBuf> {
        let extension = playnite.join("Extensions").join(BUNDLE_EXTENSION);
        match self {
            Kind::Sounds => vec![
                extension.join("Sounds"),
                playnite.join("Themes").join("Fullscreen").join(BUNDLE_THEME).join("audio"),
            ],
            Kind::Videos => vec![extension.join("Videos")],
        }
    }
}

/// A place files may be: one folder, or the Bundle's.
pub struct Place {
    pub folders: Vec<PathBuf>,
}

impl Place {
    /// The file of this name in the place's first folder that has it.
    pub fn find(&self, file: &str) -> Option<PathBuf> {
        self.folders.iter().map(|folder| folder.join(file)).find(|path| path.is_file())
    }

    pub fn name(&self) -> String {
        let names: Vec<String> = self.folders.iter().map(|folder| folder.display().to_string()).collect();
        names.join(" and ")
    }
}

/// The places to look for this kind, in order.
pub fn places(kind: Kind, media: Option<&Path>, data_dir: Option<&Path>, playnite: &[PathBuf]) -> Vec<Place> {
    if let Some(media) = media {
        let inner = media.join(kind.folder());
        return vec![Place { folders: vec![if inner.is_dir() { inner } else { media.to_path_buf() }] }];
    }
    let mut places = Vec::new();
    places.extend(data_dir.map(|dir| Place { folders: vec![dir.join("media").join(kind.folder())] }));
    places.extend(playnite.iter().map(|playnite| Place { folders: kind.bundle(playnite) }));
    places
}

/// Where Playnite keeps its extensions and themes, if it is on this machine.
pub fn playnite_folders() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if !cfg!(windows) {
        return Vec::new();
    }
    ["APPDATA", "LOCALAPPDATA"].iter().filter_map(|name| Some(var(name)?.join("Playnite"))).collect()
}

/// What `places` says, for a log line: where was looked.
pub fn looked(places: &[Place]) -> String {
    let looked: Vec<String> = places.iter().map(Place::name).collect();
    if looked.is_empty() {
        "no folder".into()
    } else {
        looked.join(", ")
    }
}
