//! What Steam has under way, from its app manifests, as the Bundle reads them (its `SteamDownloads`): a manifest is
//! written as Steam takes an app on, its `StateFlags` (Steam's `EAppState`) say where the app is, and its
//! `BytesDownloaded` and `BytesToDownload` how far the download has got, as Steam last wrote them down (every few
//! seconds, and as it pauses or ends). An install Steam drops loses its manifest; one that finishes keeps it, with
//! `FullyInstalled` set.
use std::fs;
use std::path::{Path, PathBuf};

use model::{Install, InstallState};

use crate::vdf;

/// The app's files are all on disk; a fresh install's manifest lacks it until Steam has committed the last of them.
pub const FULLY_INSTALLED: u64 = 4;
/// Steam works on the app's download: preallocating, downloading, staging, committing.
pub const UPDATE_RUNNING: u64 = 256;
/// The user has the app's download paused.
pub const UPDATE_PAUSED: u64 = 512;
/// From the moment Steam queues the app's download until it is done.
pub const UPDATE_STARTED: u64 = 1024;
/// An uninstall is under way: a manifest on its way out is no install, whatever else it says.
pub const UNINSTALLING: u64 = 2048;
/// What a manifest Steam has just written for an app it has queued says: `UpdateRequired | UpdateStarted`.
pub const QUEUED: u64 = 2 | UPDATE_STARTED;

/// Every `appmanifest_<appid>.acf` in a `steamapps` folder, by appid.
pub fn manifests(steamapps: &Path) -> Vec<(u32, PathBuf)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(steamapps).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let id = name.strip_prefix("appmanifest_").and_then(|s| s.strip_suffix(".acf"));
        if let Some(Ok(id)) = id.map(str::parse::<u32>) {
            found.push((id, entry.path()));
        }
    }
    found.sort();
    found
}

/// Whether a manifest's `StateFlags` say the game is installed: its files all on disk and no uninstall under way.
/// An installed game being updated still is. A manifest with no flags (an old one, or a made-up one) is taken at its
/// word that the game is there.
pub fn installed(flags: Option<u64>) -> bool {
    flags.is_none_or(|flags| flags & FULLY_INSTALLED != 0 && flags & UNINSTALLING == 0)
}

/// Where an app is by its manifest's flags and byte counts, while it is being installed: Steam has it queued or
/// running (`UpdateStarted` or `UpdateRunning`), it is not installed yet, and it is not on its way out. A download
/// cancelled by an uninstall can leave its manifest at `UpdateRequired | UpdatePaused`, with neither bit, for an app
/// Steam has dropped. `running` is a second word that Steam works on it (the registry's `Updating`).
pub fn state(flags: u64, done: u64, total: u64, running: bool) -> Option<InstallState> {
    if flags & FULLY_INSTALLED != 0 || flags & UNINSTALLING != 0 || flags & (UPDATE_STARTED | UPDATE_RUNNING) == 0 {
        return None;
    }
    Some(if flags & UPDATE_RUNNING != 0 || running {
        InstallState::Downloading { done, total }
    } else if flags & UPDATE_PAUSED != 0 {
        InstallState::Paused { done, total }
    } else {
        InstallState::Queued
    })
}

/// The installs under way in these `steamapps` folders. Each manifest is read on its own: one Steam is rewriting as
/// it is read is passed over this time, and the rest still count. `updating` is the registry's word for an appid.
pub fn read(steamapps: &[PathBuf], updating: &dyn Fn(u32) -> bool) -> Vec<Install> {
    let mut installs: Vec<Install> = Vec::new();
    for dir in steamapps {
        for (appid, file) in manifests(dir) {
            let Some(manifest) = fs::read(&file).ok().and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok())
            else {
                continue;
            };
            let number = |key: &str| manifest.u64_at(&["AppState", key]);
            let Some(flags) = number("StateFlags") else { continue };
            let (done, total) = (number("BytesDownloaded").unwrap_or(0), number("BytesToDownload").unwrap_or(0));
            let source_id = appid.to_string();
            // The registry is asked only of an app that is under way.
            let under_way = state(flags, done, total, false).map(|_| state(flags, done, total, updating(appid)));
            if let Some(Some(state)) = under_way.filter(|_| !installs.iter().any(|i| i.source_id == source_id)) {
                installs.push(Install { source_id, state });
            }
        }
    }
    installs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{self, Progress};

    #[test]
    fn the_flags_say_where_an_install_is() {
        let at = |flags| state(flags, 3, 10, false);
        assert_eq!(at(QUEUED), Some(InstallState::Queued));
        assert_eq!(at(QUEUED | UPDATE_RUNNING), Some(InstallState::Downloading { done: 3, total: 10 }));
        assert_eq!(at(QUEUED | UPDATE_PAUSED), Some(InstallState::Paused { done: 3, total: 10 }));
        assert_eq!(state(QUEUED, 0, 0, true), Some(InstallState::Downloading { done: 0, total: 0 }));
        // Installed, being updated, on its way out, or dropped: none is an install under way.
        for flags in [
            FULLY_INSTALLED,
            FULLY_INSTALLED | UPDATE_RUNNING | UPDATE_STARTED,
            QUEUED | UNINSTALLING,
            2 | UPDATE_PAUSED,
            1,
        ] {
            assert_eq!(at(flags), None, "{flags}");
        }
        assert!(
            installed(Some(FULLY_INSTALLED)) && installed(Some(FULLY_INSTALLED | UPDATE_RUNNING)) && installed(None)
        );
        assert!(!installed(Some(QUEUED)) && !installed(Some(FULLY_INSTALLED | UNINSTALLING)) && !installed(Some(1)));
    }

    #[test]
    fn an_install_is_read_from_its_manifest_from_queued_to_done() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        fixture::write(&root, &fixture::Fixture::standard()).unwrap();
        let steamapps = [root.join("steamapps")];
        let quiet = 9_100_040; // Quiet Orbit, not installed.
        let read = |updating: &dyn Fn(u32) -> bool| read(&steamapps, updating);
        assert_eq!(read(&|_| false), []);
        let install = |state| vec![Install { source_id: quiet.to_string(), state }];

        fixture::write_progress(&root, quiet, Progress::Queued).unwrap();
        assert_eq!(read(&|_| false), install(InstallState::Queued));
        assert_eq!(read(&|appid| appid == quiet), install(InstallState::Downloading { done: 0, total: 0 }));
        fixture::write_progress(&root, quiet, Progress::Downloading { done: 250, total: 1000 }).unwrap();
        assert_eq!(read(&|_| false), install(InstallState::Downloading { done: 250, total: 1000 }));
        fixture::write_progress(&root, quiet, Progress::Paused { done: 400, total: 1000 }).unwrap();
        assert_eq!(read(&|_| false), install(InstallState::Paused { done: 400, total: 1000 }));
        // While it downloads the game is not installed; once done it is, with its folder, and nothing is under way.
        let game =
            |root: &Path| crate::load(root).unwrap().games.into_iter().find(|g| g.source_id == "9100040").unwrap();
        assert!(!game(&root).installed && game(&root).install_dir.is_none());
        fixture::write_progress(&root, quiet, Progress::Done).unwrap();
        assert_eq!(read(&|_| false), []);
        let done = game(&root);
        assert!(done.installed);
        assert_eq!(done.install_dir, Some(root.join("steamapps").join("common").join("Quiet Orbit")));
        // Dropped: the manifest goes, and the game is as it was.
        fixture::write_progress(&root, quiet, Progress::Dropped).unwrap();
        assert!(!game(&root).installed);
        assert_eq!(read(&|_| false), []);
    }
}
