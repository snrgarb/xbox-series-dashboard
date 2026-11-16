//! Epic Online Services' install helper's records, which a current launcher writes in place of the old manifests
//! (seen on a real machine on 2026-11-16, launcher build EpicInstaller-20.3.3; docs/research/stores.md):
//! `%ProgramData%\Epic\EpicOnlineServicesShared\InstallHelper\InstalledItems\<installationId>.egi`, one JSON object per
//! installation, `{"v4": {"installationId", "state", "dir", "sandboxId", "itemId", "artifactId", "manifestData":
//! {"version", "launchExe", "buildSize"}, "pendingManifestData": {…}, …}}`. The folder also holds a `.lock` and
//! `Revision.json`, which are not read.
//!
//! The launcher's own components (`overlay.egi`, `service.egi`, `support.egi`) are in the sibling
//! `EpicOnlineServices\InstallHelper\InstalledItems`, which is never read: only the `Shared` folder holds games.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The one version of the record that is known.
const VERSION: &str = "v4";

/// What `state` says of an installation that is done.
const INSTALLED: &str = "Installed";

/// One installation, as its `.egi` says it. Besides what the library lists, it says what an install in progress
/// will take (`pending_*`), for the Queue (#78).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub installation_id: String,
    /// `Installed` once it is done; `Incomplete` was seen too.
    pub state: String,
    /// `artifactId`: the `AppName`, the game's `source_id`.
    pub app_name: String,
    /// `sandboxId`: the catalogue namespace.
    pub namespace: String,
    pub item_id: String,
    pub dir: PathBuf,
    /// `manifestData.launchExe`, relative to `dir`.
    pub exe: String,
    /// `manifestData.version` and `buildSize`: the build that is there.
    pub version: String,
    pub size: u64,
    /// `pendingManifestData.version` and `buildSize`: the build being installed, empty and 0 when none is.
    pub pending_version: String,
    pub pending_size: u64,
}

impl Record {
    /// Done, and its folder is there.
    pub fn installed(&self) -> bool {
        self.state == INSTALLED && self.dir.is_dir()
    }
}

/// `EpicOnlineServicesShared\InstallHelper\InstalledItems` under `%ProgramData%\Epic`.
pub fn folder(data: &Path) -> PathBuf {
    data.join("EpicOnlineServicesShared").join("InstallHelper").join("InstalledItems")
}

/// Every record in `folder`, by file name. None when there is no such folder. A file that cannot be read, or is not
/// a record of a version known here, is said in the log and passed over: never an error for the whole store.
pub fn records(folder: &Path) -> Option<Vec<Record>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("epic: {}: {e}", folder.display());
            return Some(Vec::new());
        }
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("egi")))
        .collect();
    files.sort();
    let read = |file: &Path| -> Result<Record, String> {
        let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
        parse(&text)
    };
    let records = files.iter().filter_map(|file| match read(file) {
        Ok(record) => Some(record),
        Err(why) => {
            eprintln!("epic: {}: {why}, passed over", file.display());
            None
        }
    });
    Some(records.collect())
}

/// A record, or why it is not one known here. Keys it does not know are let be.
pub fn parse(text: &str) -> Result<Record, String> {
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "not JSON".to_string())?;
    let Some(object) = value.as_object() else { return Err("not a record".into()) };
    let Some(record) = object.get(VERSION) else {
        let versions: Vec<&str> = object.keys().map(String::as_str).collect();
        return Err(format!("a record of version {} (only {VERSION} is known)", versions.join(", ")));
    };
    let text = |pointer: &str| record.pointer(pointer).and_then(Value::as_str).unwrap_or_default().to_string();
    let size = |pointer: &str| record.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
    let app_name = text("/artifactId");
    if app_name.is_empty() {
        return Err("no artifactId".into());
    }
    Ok(Record {
        installation_id: text("/installationId"),
        state: text("/state"),
        app_name,
        namespace: text("/sandboxId"),
        item_id: text("/itemId"),
        dir: PathBuf::from(text("/dir")),
        exe: text("/manifestData/launchExe"),
        version: text("/manifestData/version"),
        size: size("/manifestData/buildSize"),
        pending_version: text("/pendingManifestData/version"),
        pending_size: size("/pendingManifestData/buildSize"),
    })
}

/// A record as the install helper writes it, for the fixture and the tests: made-up ids, no tokens.
pub(crate) fn write_record(record: &Record) -> String {
    let id = &record.installation_id;
    let meta = record.dir.join(".egstore");
    serde_json::json!({ VERSION: {
        "installationId": id,
        "state": record.state,
        "revision": format!("{:0>32}", 3),
        "dir": record.dir,
        "metaDir": meta,
        "manifestPath": meta.join(format!("{id}.manifest")),
        "pendingManifestPath": meta.join("Pending").join(format!("{id}.manifest")),
        "platform": "Windows",
        "sandboxId": record.namespace,
        "itemId": record.item_id,
        "artifactId": record.app_name,
        "tags": [],
        "pendingTags": [],
        "stagedVersion": "",
        "manifestData": {
            "version": record.version, "launchCommand": "", "launchExe": record.exe, "prereqSHA1Hash": "",
            "buildSize": record.size,
        },
        "pendingManifestData": {
            "version": record.pending_version, "launchCommand": "", "launchExe": "", "prereqSHA1Hash": "",
            "buildSize": record.pending_size,
        },
        "manifestUris": [],
    }})
    .to_string()
}

/// A made-up record of `app`, for the tests.
#[cfg(test)]
pub(crate) fn record(app: &str, state: &str, dir: &Path) -> Record {
    Record {
        installation_id: format!("{:0>32}", app.to_uppercase()),
        state: state.into(),
        app_name: app.into(),
        namespace: format!("ns-{app}"),
        item_id: format!("item-{app}"),
        dir: dir.into(),
        exe: "Game.exe".into(),
        version: "1.0.3".into(),
        size: 6_200_000_000,
        pending_version: String::new(),
        pending_size: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_reads_back_and_is_installed_when_done_and_its_folder_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let installed = record("kite", "Installed", dir.path());
        assert_eq!(parse(&write_record(&installed)), Ok(installed.clone()));
        assert!(installed.installed());
        let pending = Record { pending_version: "1.0.4".into(), pending_size: 7_000_000_000, ..installed.clone() };
        assert_eq!(parse(&write_record(&pending)), Ok(pending));
        assert!(!record("kite", "Incomplete", dir.path()).installed());
        assert!(!record("kite", "Installed", &dir.path().join("gone")).installed());
        // As the launcher writes it, on one line, with a byte-order mark let be.
        assert_eq!(parse(&format!("\u{feff}{}", write_record(&installed))), Ok(installed));
    }

    #[test]
    fn an_unknown_version_or_a_broken_file_is_passed_over_and_the_rest_read() {
        assert_eq!(parse("{\"v5\": {\"artifactId\": \"x\"}}"), Err("a record of version v5 (only v4 is known)".into()));
        assert_eq!(parse("not json"), Err("not JSON".into()));
        assert_eq!(parse("[]"), Err("not a record".into()));
        assert_eq!(parse("{\"v4\": {\"state\": \"Installed\"}}"), Err("no artifactId".into()));

        let dir = tempfile::tempdir().unwrap();
        let items = folder(dir.path());
        assert_eq!(records(&items), None);
        fs::create_dir_all(&items).unwrap();
        fs::write(items.join("A.egi"), write_record(&record("a", "Installed", dir.path()))).unwrap();
        fs::write(items.join("B.egi"), "{\"v5\": {}}").unwrap();
        fs::write(items.join("C.egi"), "").unwrap();
        fs::write(items.join(".lock"), "").unwrap();
        fs::write(items.join("Revision.json"), "{\"timestamp\":1,\"number\":3}").unwrap();
        let read = records(&items).unwrap();
        assert_eq!(read.iter().map(|r| r.app_name.as_str()).collect::<Vec<_>>(), ["a"]);
    }
}
