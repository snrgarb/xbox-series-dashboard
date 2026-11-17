//! The helper programs some stores install through (docs/spec/helpers.md; docs/research/installs.md 10.1, 13 item
//! 6): legendary for Epic Games, gogdl for GOG. Both are GPL-3.0 and are **never bundled or linked into the
//! Dashboard**: on the user's Yes in Settings > Helpers, with the licence and the source in front of them, the
//! release named in [`builtin`] (its version, its address and its SHA-256 fixed here, in the Dashboard's own source)
//! is fetched from the project's own GitHub releases into the data folder, checked against that SHA-256 before it is
//! kept, and can be removed again. Every update of a helper is a reviewed change of this table.
//!
//! Nothing is sent but a plain GET of the file's address, with a User-Agent that names no user, machine or version.
//! Running a helper is `shell::helper`'s; the stores' own use of them is #175 (Epic) and #176 (GOG).
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod fake;

/// The User-Agent every request carries. It names no user, machine or version.
pub const USER_AGENT: &str = "dashboard-helpers";

/// The folder under the data folder the helpers are kept in, one folder each.
pub const FOLDER: &str = "helpers";

/// What is kept beside a helper once it is checked: its version and SHA-256 (`Kept`).
const KEPT: &str = "helper.json";

/// The most a download may be: both helpers are under 20 MB.
const MOST_BYTES: u64 = 64 << 20;

/// A helper, as the table names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Helper {
    /// Its key: the folder it is kept in, and how Settings and the stores name it.
    pub key: String,
    /// The store it is for, as Settings writes it.
    pub store: String,
    /// The release fetched.
    pub version: String,
    /// The project's licence, as its repository names it.
    pub licence: String,
    /// The project's source, shown with the licence.
    pub source: String,
    /// The release's page, where the file and its SHA-256 are shown.
    pub release: String,
    /// The Windows x64 program's address on that page.
    pub url: String,
    /// Its SHA-256, lower-case hex. None: not known yet, and it is not fetched until it is.
    pub sha256: Option<String>,
    /// Where the SHA-256 comes from, for whoever reviews the table.
    pub sha256_from: String,
    /// The program's name in its folder: `legendary.exe`.
    pub exe: String,
}

/// GitHub shows a SHA-256 for each file it serves on a release's page (its `digest`); neither project publishes a
/// checksum file of its own (looked at on 2026-11-17: no `SHA256SUMS` among the assets, none in the notes).
const GITHUB_DIGEST: &str = "GitHub's SHA-256 of the asset as uploaded, shown beside it on the release page and in \
                             the releases API's `digest`; the project publishes no checksum of its own";

/// The helpers the Dashboard may fetch: one fixed release each. Looked up on the projects' own GitHub release pages
/// on 2026-11-17.
pub fn builtin() -> Vec<Helper> {
    vec![
        Helper {
            key: "legendary".into(),
            store: "Epic Games".into(),
            version: "0.21.1".into(),
            licence: "GPL-3.0".into(),
            source: "https://github.com/legendary-gl/legendary".into(),
            release: "https://github.com/legendary-gl/legendary/releases/tag/0.21.1".into(),
            url: "https://github.com/legendary-gl/legendary/releases/download/0.21.1/legendary_windows_x64.exe".into(),
            sha256: Some("6be77857dc0a6dd33ea7442ce8b5291944ec6da53757f81ca95b57ce794baf5b".into()),
            sha256_from: GITHUB_DIGEST.into(),
            exe: "legendary.exe".into(),
        },
        Helper {
            key: "gogdl".into(),
            store: "GOG".into(),
            version: "1.4.0".into(),
            licence: "GPL-3.0".into(),
            source: "https://github.com/Heroic-Games-Launcher/heroic-gogdl".into(),
            release: "https://github.com/Heroic-Games-Launcher/heroic-gogdl/releases/tag/v1.4.0".into(),
            url: "https://github.com/Heroic-Games-Launcher/heroic-gogdl/releases/download/v1.4.0/gogdl_windows_x86_64.exe"
                .into(),
            sha256: Some("291b653e6dbf7a394a3342ed0ebcea2cbc83e5f66d030777d65ccdd60d18ab02".into()),
            sha256_from: GITHUB_DIGEST.into(),
            exe: "gogdl.exe".into(),
        },
    ]
}

/// A table of a test's own in the builtin one's place (`dashboard --helper-table <file>`): the same fields, as JSON.
pub fn read_table(path: &Path) -> Result<Vec<Helper>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Why a helper could not be added, in words that name no address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The download's SHA-256 is not the table's: nothing was kept.
    BadHash {
        want: String,
        got: String,
    },
    /// The table has no SHA-256 for it yet: nothing is fetched.
    NoHash,
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadHash { want, got } => write!(f, "its SHA-256 is {got}, not the {want} the Dashboard expects"),
            Error::NoHash => f.write_str("the Dashboard has no SHA-256 for it yet"),
            Error::Failed(why) => f.write_str(why),
        }
    }
}

impl Error {
    /// What Settings says of it.
    pub fn for_the_user(&self) -> String {
        match self {
            Error::BadHash { .. } => "The download wasn't the file expected, so it wasn't kept.".into(),
            Error::NoHash => "It can't be checked yet, so it isn't fetched.".into(),
            Error::Failed(why) => format!("It couldn't be fetched: {why}."),
        }
    }
}

/// What is kept beside a helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub version: String,
    pub sha256: String,
}

/// The agent every download is made with: a time limit, the plain User-Agent, every status given back to be read,
/// and redirects followed (GitHub serves a release's files from elsewhere).
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(300)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The helper's folder under the data folder.
pub fn folder(data: &Path, helper: &Helper) -> PathBuf {
    data.join(FOLDER).join(&helper.key)
}

/// The helper's program, where it is kept.
pub fn program(data: &Path, helper: &Helper) -> PathBuf {
    folder(data, helper).join(&helper.exe)
}

/// The helper as it is kept, if it is: its program there and what was kept of it read back.
pub fn kept(data: &Path, helper: &Helper) -> Option<Kept> {
    if !program(data, helper).is_file() {
        return None;
    }
    let text = std::fs::read_to_string(folder(data, helper).join(KEPT)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Fetches the helper's program from its address, checks it against the table's SHA-256, and only then keeps it in
/// its folder, with what was kept of it. A download that is not the file expected is never written where a program
/// could be run from. The program's path.
pub fn fetch(agent: &ureq::Agent, data: &Path, helper: &Helper) -> Result<PathBuf, Error> {
    let want = helper.sha256.as_deref().filter(|hash| hash.len() == 64).ok_or(Error::NoHash)?.to_ascii_lowercase();
    let bytes = get(agent, &helper.url)?;
    let got = sha256(&bytes);
    if got != want {
        return Err(Error::BadHash { want, got });
    }
    let failed = |e: std::io::Error| Error::Failed(format!("it couldn't be written to the data folder ({e})"));
    let folder = folder(data, helper);
    std::fs::create_dir_all(&folder).map_err(failed)?;
    let program = folder.join(&helper.exe);
    let part = folder.join(format!("{}.part", helper.exe));
    std::fs::write(&part, &bytes).map_err(failed)?;
    std::fs::rename(&part, &program).map_err(failed)?;
    let kept = serde_json::to_string(&Kept { version: helper.version.clone(), sha256: got }).expect("plain fields");
    std::fs::write(folder.join(KEPT), kept).map_err(failed)?;
    Ok(program)
}

/// Removes the helper's folder and everything in it. Nothing there is nothing to do.
pub fn remove(data: &Path, helper: &Helper) -> std::io::Result<()> {
    match std::fs::remove_dir_all(folder(data, helper)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The helper as Settings shows it, standing as `state`.
pub fn shown(helper: &Helper, state: model::helper::HelperState) -> model::helper::Helper {
    model::helper::Helper {
        key: helper.key.clone(),
        store: helper.store.clone(),
        version: helper.version.clone(),
        licence: helper.licence.clone(),
        source: helper.source.clone(),
        state,
    }
}

fn get(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, Error> {
    let mut response = agent.get(url).call().map_err(transport)?;
    match response.status().as_u16() {
        200..=299 => {}
        status => return Err(Error::Failed(format!("its release answered {status}"))),
    }
    response.body_mut().with_config().limit(MOST_BYTES).read_to_vec().map_err(transport)
}

/// What went wrong, in words that name no address: ureq's own may carry the request's.
fn transport(error: ureq::Error) -> Error {
    Error::Failed(match error {
        ureq::Error::Timeout(_) => "its release took too long to answer".into(),
        ureq::Error::HostNotFound => "its release could not be found: is the PC online?".into(),
        ureq::Error::ConnectionFailed | ureq::Error::Io(_) => "its release could not be reached".into(),
        ureq::Error::BodyExceedsLimit(_) => "it is far bigger than it should be".into(),
        _ => "its release's answer could not be read".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeHelpers;

    fn table(service: &FakeHelpers, good: &[u8]) -> Vec<Helper> {
        let mut helpers = builtin();
        helpers[0].url = service.url("bad");
        helpers[1].url = service.url("good");
        for helper in &mut helpers {
            helper.sha256 = Some(sha256(good));
        }
        helpers
    }

    #[test]
    fn the_table_fixes_a_version_an_address_on_the_projects_own_releases_and_a_sha256_each() {
        let helpers = builtin();
        assert_eq!(helpers.iter().map(|h| h.key.as_str()).collect::<Vec<_>>(), ["legendary", "gogdl"]);
        for helper in helpers {
            let hash = helper.sha256.as_deref().unwrap();
            assert!(hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(helper.url.starts_with(&format!("{}/releases/download/", helper.source)), "{}", helper.url);
            assert!(helper.release.starts_with(&format!("{}/releases/tag/", helper.source)));
            assert!(helper.url.contains(&helper.version) && helper.licence == "GPL-3.0");
        }
    }

    #[test]
    fn a_helper_is_kept_only_once_its_sha256_is_the_tables_and_removed_whole() {
        let good = b"MZ stand-in helper".to_vec();
        let service = FakeHelpers::start(0, good.clone()).unwrap();
        let helpers = table(&service, &good);
        let data = tempfile::tempdir().unwrap();
        let agent = agent();

        let refused = fetch(&agent, data.path(), &helpers[0]);
        assert!(matches!(refused, Err(Error::BadHash { .. })), "{refused:?}");
        assert!(!folder(data.path(), &helpers[0]).join("legendary.exe").exists());
        assert!(!folder(data.path(), &helpers[0]).join("legendary.exe.part").exists());
        assert_eq!(kept(data.path(), &helpers[0]), None);

        let program = fetch(&agent, data.path(), &helpers[1]).unwrap();
        assert_eq!(std::fs::read(&program).unwrap(), good);
        assert_eq!(kept(data.path(), &helpers[1]).unwrap().version, "1.4.0");

        remove(data.path(), &helpers[1]).unwrap();
        remove(data.path(), &helpers[1]).unwrap();
        assert!(!folder(data.path(), &helpers[1]).exists() && kept(data.path(), &helpers[1]).is_none());

        // Nothing went but the plain request.
        let requests = service.requests();
        assert_eq!(requests.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["/helpers/bad", "/helpers/good"]);
        assert!(requests.iter().all(|r| r.user_agent.as_deref() == Some(USER_AGENT) && r.authorization.is_none()));
    }

    #[test]
    fn with_no_sha256_nothing_is_fetched() {
        let service = FakeHelpers::start(0, b"x".to_vec()).unwrap();
        let helper = Helper { sha256: None, url: service.url("good"), ..builtin().remove(0) };
        let data = tempfile::tempdir().unwrap();
        assert_eq!(fetch(&agent(), data.path(), &helper), Err(Error::NoHash));
        assert!(service.requests().is_empty());
    }

    #[test]
    fn a_table_of_a_tests_own_is_read_from_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("table.json");
        std::fs::write(&path, serde_json::to_string(&builtin()).unwrap()).unwrap();
        assert_eq!(read_table(&path).unwrap(), builtin());
        assert!(read_table(&dir.path().join("none.json")).is_err());
    }
}
