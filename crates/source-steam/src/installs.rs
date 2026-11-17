//! What Steam has under way, from its app manifests, as the Bundle reads them (its `SteamDownloads`): a manifest is
//! written as Steam takes an app on, its `StateFlags` (Steam's `EAppState`) say where the app is, and its
//! `BytesDownloaded` and `BytesToDownload` how far the download has got, as Steam last wrote them down. Seen on a
//! real Steam (2026-11-16) that is seldom: as the download starts, now and then on the way (0%, 11%, 100% for a
//! 510 MB game) and as it pauses or ends, so how far a running download has got is worked out live elsewhere
//! (`crate::live`). An install Steam drops loses its manifest; one that finishes keeps it, with `FullyInstalled`
//! set.
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

/// What an app's manifest says of its download, as Steam last wrote it: its flags, the bytes downloaded of those to
/// download, and the bytes staged (written out whole, as Steam downloads them compressed) of those to stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub flags: u64,
    pub downloaded: u64,
    pub to_download: u64,
    pub staged: u64,
    pub to_stage: u64,
}

/// What the manifest at `file` says, if it can be read and has its flags. One Steam is rewriting as it is read
/// may not be: it is passed over this time.
pub fn counts(file: &Path) -> Option<Counts> {
    let manifest = fs::read(file).ok().and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok())?;
    let number = |key: &str| manifest.u64_at(&["AppState", key]);
    Some(Counts {
        flags: number("StateFlags")?,
        downloaded: number("BytesDownloaded").unwrap_or(0),
        to_download: number("BytesToDownload").unwrap_or(0),
        staged: number("BytesStaged").unwrap_or(0),
        to_stage: number("BytesToStage").unwrap_or(0),
    })
}

/// The installs under way in these `steamapps` folders, each with its manifest's counts. Each manifest is read on
/// its own: one Steam is rewriting as it is read is passed over this time, and the rest still count. `updating` is
/// the registry's word for an appid.
pub fn under_way(steamapps: &[PathBuf], updating: &dyn Fn(u32) -> bool) -> Vec<(u32, InstallState, Counts)> {
    let mut found: Vec<(u32, InstallState, Counts)> = Vec::new();
    for dir in steamapps {
        for (appid, file) in manifests(dir) {
            let Some(counts) = counts(&file) else { continue };
            let (flags, done, total) = (counts.flags, counts.downloaded, counts.to_download);
            // The registry is asked only of an app that is under way.
            let state = state(flags, done, total, false).and_then(|_| state(flags, done, total, updating(appid)));
            if let Some(state) = state.filter(|_| !found.iter().any(|&(id, ..)| id == appid)) {
                found.push((appid, state, counts));
            }
        }
    }
    found
}

/// Whether Steam's Workshop record for the app, `workshop/appworkshop_<appid>.acf` in any of these `steamapps`
/// folders, says it still has items to fetch (`NeedsDownload`): the subscribed items Steam downloads once the game
/// itself is installed (#188).
pub fn workshop_pending(steamapps: &[PathBuf], appid: u32) -> bool {
    steamapps.iter().any(|dir| {
        let file = dir.join("workshop").join(format!("appworkshop_{appid}.acf"));
        let record = fs::read(file).ok().and_then(|b| vdf::parse_text(&String::from_utf8_lossy(&b)).ok());
        record.and_then(|record| record.u64_at(&["AppWorkshop", "NeedsDownload"])).is_some_and(|needs| needs != 0)
    })
}

/// The installs under way in these `steamapps` folders, by their manifests alone ([`under_way`]).
pub fn read(steamapps: &[PathBuf], updating: &dyn Fn(u32) -> bool) -> Vec<Install> {
    under_way(steamapps, updating).into_iter().map(|(appid, state, _)| Install::new(appid.to_string(), state)).collect()
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
        let install = |state| vec![Install::new(quiet.to_string(), state)];

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

    #[test]
    fn the_workshop_record_says_whether_items_are_still_to_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let (first, second) = (dir.path().join("a"), dir.path().join("b"));
        std::fs::create_dir_all(second.join("workshop")).unwrap();
        let record =
            |needs: u32| format!("\"AppWorkshop\"\n{{\n\t\"appid\"\t\"4000\"\n\t\"NeedsDownload\"\t\"{needs}\"\n}}\n");
        let file = second.join("workshop").join("appworkshop_4000.acf");
        let dirs = [first, second];
        assert!(!workshop_pending(&dirs, 4000), "no record");
        std::fs::write(&file, record(1)).unwrap();
        assert!(workshop_pending(&dirs, 4000) && !workshop_pending(&dirs, 4001));
        std::fs::write(&file, record(0)).unwrap();
        assert!(!workshop_pending(&dirs, 4000));
    }

    /// The stand-in Steam's Pause, Resume and Cancel (#170): from the bytes its manifest has.
    #[test]
    fn the_stand_in_pauses_resumes_and_cancels_at_the_bytes_it_has() {
        use model::Steer;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("steam");
        fixture::write(&root, &fixture::Fixture::standard()).unwrap();
        let quiet = 9_100_040;
        assert!(fixture::steered(&root, quiet, Steer::Pause).is_err(), "nothing under way");
        fixture::write_progress(&root, quiet, Progress::Downloading { done: 30, total: 100 }).unwrap();
        let paused = fixture::steered(&root, quiet, Steer::Pause).unwrap();
        assert_eq!(paused, Progress::Paused { done: 30, total: 100 });
        fixture::write_progress(&root, quiet, paused).unwrap();
        let resumed = fixture::steered(&root, quiet, Steer::Resume).unwrap();
        assert_eq!(resumed, Progress::Downloading { done: 30, total: 100 });
        assert_eq!(fixture::steered(&root, quiet, Steer::Cancel).unwrap(), Progress::Dropped);
        // An installed game has nothing to steer.
        fixture::write_progress(&root, quiet, Progress::Done).unwrap();
        assert!(fixture::steered(&root, quiet, Steer::Cancel).is_err());
    }
}
