//! Updates (docs/spec/updates.md): the newest release asked of one address, its portable zip downloaded and checked
//! against the release's `SHA256SUMS.txt`, and `dashboard.exe` put in place by renaming, the running one renamed
//! aside and kept (docs/research/shell.md, 4.6). Starting the new one and judging whether it started is the
//! executable's (`--handover`); this crate does the rest, and none of it needs Windows.
//!
//! Nothing is sent but a plain GET to the update address and to the files its answer names: no key, no cookie, no
//! version or machine of the user's, and a User-Agent that names neither (GitHub's API refuses a request with none).
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use model::update::Version;
use sha2::{Digest, Sha256};

pub mod fake;
mod zip;

/// The update address: GitHub Releases' API for the repository's newest release. The one place it is written; a
/// test gives its stand-in's in its place (`dashboard --update-source`, `standin serve-updates`). While the
/// repository is private it answers 404 to a request with no token, which is said plainly (`Checked::NotPublic`).
pub const SOURCE: &str = "https://api.github.com/repos/snrgarb/xbox-series-dashboard/releases/latest";

/// The User-Agent every request carries: GitHub's API refuses one with none. It names no user, machine or version.
pub const USER_AGENT: &str = "dashboard-updates";

/// The release's manifest, as `tools/package.sh` makes it: `<sha256>  <file>` a line.
pub const SUMS: &str = "SHA256SUMS.txt";

/// The program, in the zip and in its folder.
pub const EXE: &str = "dashboard.exe";

/// The most a download may be: the zip is some 15 MB.
const MOST_BYTES: u64 = 200 << 20;

/// The portable zip of a version, as `tools/package.sh` names it.
pub fn zip_name(version: Version) -> String {
    format!("XboxSeriesDashboard-{version}-portable.zip")
}

/// A release, as the update address describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Where its portable zip and its manifest are.
    pub zip: String,
    pub sums: String,
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// The newest release is this version, no newer than the running one.
    UpToDate(Version),
    /// A newer one.
    Newer(Release),
    /// 404: the repository is private, or has no release.
    NotPublic,
}

/// Why something failed, in words that name no address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The download's SHA-256 is not the manifest's.
    BadHash {
        want: String,
        got: String,
    },
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadHash { want, got } => write!(f, "its SHA-256 is {got}, not the manifest's {want}"),
            Error::Failed(why) => f.write_str(why),
        }
    }
}

/// The agent every request is made with: a time limit, the plain User-Agent, every status given back to be read,
/// and redirects followed (a release's files are served from elsewhere).
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// Asks `source` for the newest release, and whether it is newer than `current`.
pub fn check(agent: &ureq::Agent, source: &str, current: Version) -> Result<Checked, Error> {
    let mut response = agent
        .get(source)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| transport(e, "the update source"))?;
    match response.status().as_u16() {
        200..=299 => {}
        404 => return Ok(Checked::NotPublic),
        status => return Err(Error::Failed(format!("the update source answered {status}"))),
    }
    let body = response.body_mut().with_config().limit(4 << 20).read_to_string();
    let release = parse_release(&body.map_err(|e| transport(e, "the update source"))?)?;
    Ok(if release.version > current { Checked::Newer(release) } else { Checked::UpToDate(release.version) })
}

/// A release from GitHub's JSON for one: its `tag_name` (`v` and the version), and among its `assets` the portable
/// zip of that version and the manifest, by name.
pub fn parse_release(json: &str) -> Result<Release, Error> {
    let bad = |why: &str| Error::Failed(format!("the update source's answer {why}"));
    let value: serde_json::Value = serde_json::from_str(json).map_err(|_| bad("is not JSON"))?;
    let tag = value["tag_name"].as_str().ok_or_else(|| bad("has no tag_name"))?;
    let version = Version::parse(tag).ok_or_else(|| bad(&format!("names no version: {tag}")))?;
    let assets = value["assets"].as_array().ok_or_else(|| bad("has no assets"))?;
    let url = |name: &str| {
        assets
            .iter()
            .find(|asset| asset["name"].as_str() == Some(name))
            .and_then(|asset| asset["browser_download_url"].as_str())
            .map(str::to_string)
            .ok_or_else(|| bad(&format!("has no {name}")))
    };
    Ok(Release { version, zip: url(&zip_name(version))?, sums: url(SUMS)? })
}

/// Downloads the release's manifest and portable zip, checks the zip's SHA-256 against the manifest's line for it,
/// and takes `dashboard.exe` out of it: its bytes.
pub fn download(agent: &ureq::Agent, release: &Release) -> Result<Vec<u8>, Error> {
    let sums = get(agent, &release.sums, 1 << 20)?;
    let zip = get(agent, &release.zip, MOST_BYTES)?;
    unpack(&zip, &String::from_utf8_lossy(&sums), &zip_name(release.version))
}

/// `dashboard.exe` out of `zip`, once its SHA-256 is the one `sums` gives `name`.
pub fn unpack(zip: &[u8], sums: &str, name: &str) -> Result<Vec<u8>, Error> {
    let want = sums
        .lines()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .find(|(_, file)| file.trim().trim_start_matches('*') == name)
        .map(|(hash, _)| hash.trim().to_ascii_lowercase())
        .ok_or_else(|| Error::Failed(format!("the release's {SUMS} has no line for {name}")))?;
    let got = sha256(zip);
    if got != want {
        return Err(Error::BadHash { want, got });
    }
    zip::read_one(zip, EXE, MOST_BYTES as usize).map_err(|why| Error::Failed(format!("{name}: {why}")))
}

/// SHA-256 as `sha256sum` writes it.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn get(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Vec<u8>, Error> {
    let mut response = agent.get(url).call().map_err(|e| transport(e, "the release's files"))?;
    match response.status().as_u16() {
        200..=299 => {}
        status => return Err(Error::Failed(format!("the release's files answered {status}"))),
    }
    response.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| transport(e, "the release's files"))
}

/// What went wrong on the way to `who`, in words that name no address: ureq's own may carry the request's.
fn transport(error: ureq::Error, who: &str) -> Error {
    Error::Failed(match error {
        ureq::Error::Timeout(_) => format!("{who} took too long to answer"),
        ureq::Error::HostNotFound => format!("{who} could not be found: is the PC online?"),
        ureq::Error::ConnectionFailed | ureq::Error::Io(_) => format!("{who} could not be reached"),
        ureq::Error::BodyExceedsLimit(_) => format!("{who} sent more than is ever expected"),
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => format!("{who}'s address is not one a request can be made to"),
        _ => format!("{who}'s answer could not be read"),
    })
}

/// Where the new program waits beside the running one: `dashboard.exe.new`.
pub fn staged(exe: &Path) -> PathBuf {
    exe.with_file_name(format!("{EXE}.new"))
}

/// Where the running one is kept once the new one is in place: `dashboard.<its version>.old`.
pub fn kept(exe: &Path, version: Version) -> PathBuf {
    exe.with_file_name(format!("dashboard.{version}.old"))
}

/// Where a new one that did not start is put: `dashboard.<its version>.bad`.
pub fn refused(exe: &Path, version: Version) -> PathBuf {
    exe.with_file_name(format!("dashboard.{version}.bad"))
}

/// Writes the new program beside `exe` as `dashboard.exe.new`.
pub fn stage(exe: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let staged = staged(exe);
    std::fs::write(&staged, bytes)?;
    Ok(staged)
}

/// Puts the staged program in `exe`'s place: `exe` renamed to `dashboard.<current>.old` (a running program may be
/// renamed, never written over), then `dashboard.exe.new` to `exe`. Each is a rename in one folder, so either
/// happens whole; if the second fails the first is undone. Where the old one now is.
pub fn put_in_place(exe: &Path, current: Version) -> std::io::Result<PathBuf> {
    let (staged, old) = (staged(exe), kept(exe, current));
    if !staged.is_file() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no dashboard.exe.new to put in place"));
    }
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(&staged, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(e);
    }
    Ok(old)
}

/// Undoes [`put_in_place`] for a new program that did not start: it goes to `dashboard.<new>.bad`, and the old one
/// back to `exe`.
pub fn put_back(exe: &Path, old: &Path, new: Version) -> std::io::Result<()> {
    std::fs::rename(exe, refused(exe, new))?;
    std::fs::rename(old, exe)
}

/// Removes every `dashboard.*.old` and a left `dashboard.exe.new` in `folder` that can be removed: one still
/// running (the --shell process's, until sign-out) can't, and is left for a later start. Those removed.
pub fn remove_old(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let ours = (name.starts_with("dashboard.") && name.ends_with(".old")) || name == format!("{EXE}.new");
        if ours && entry.path().is_file() && std::fs::remove_file(entry.path()).is_ok() {
            removed.push(entry.path());
        }
    }
    removed.sort();
    removed
}

/// A day: the automatic check's period.
pub const DAY: u64 = 24 * 60 * 60;

/// Whether the automatic check is due at `now`, the last having been at `last` (seconds since 1970): never checked,
/// a day or more ago, or a clock that went back.
pub fn due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|last| now >= last.saturating_add(DAY) || now < last)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1: Version = Version::new(0, 1, 0);
    const V2: Version = Version::new(0, 2, 0);

    #[test]
    fn a_release_is_read_from_githubs_json() {
        let json = r#"{"tag_name": "v0.2.0", "assets": [
            {"name": "XboxSeriesDashboard-0.2.0-setup.exe", "browser_download_url": "https://x/setup"},
            {"name": "XboxSeriesDashboard-0.2.0-portable.zip", "browser_download_url": "https://x/zip"},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://x/sums"}]}"#;
        let release = parse_release(json).unwrap();
        assert_eq!(release, Release { version: V2, zip: "https://x/zip".into(), sums: "https://x/sums".into() });
        assert!(parse_release(r#"{"tag_name": "v0.2.0", "assets": []}"#).is_err());
        assert!(parse_release(r#"{"tag_name": "nightly", "assets": []}"#).is_err());
        assert!(parse_release("<html>").is_err());
    }

    #[test]
    fn the_zip_is_unpacked_only_when_its_hash_is_the_manifests() {
        let zip = zip::write_one("XboxSeriesDashboard-0.2.0/dashboard.exe", b"MZ new");
        let name = zip_name(V2);
        let sums = format!("{}  XboxSeriesDashboard-0.2.0-setup.exe\n{}  {name}\n", sha256(b"x"), sha256(&zip));
        assert_eq!(unpack(&zip, &sums, &name).unwrap(), b"MZ new");
        let wrong = format!("{}  {name}\n", sha256(b"something else"));
        assert!(matches!(unpack(&zip, &wrong, &name), Err(Error::BadHash { .. })));
        assert!(matches!(unpack(&zip, "", &name), Err(Error::Failed(_))));
    }

    #[test]
    fn the_new_one_goes_in_place_the_old_is_kept_and_a_bad_one_is_put_back() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(EXE);
        std::fs::write(&exe, "old").unwrap();
        assert!(put_in_place(&exe, V1).is_err(), "nothing staged");
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "old");
        stage(&exe, b"new").unwrap();
        let old = put_in_place(&exe, V1).unwrap();
        assert_eq!(old, dir.path().join("dashboard.0.1.0.old"));
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "old");
        assert!(!staged(&exe).exists());
        put_back(&exe, &old, V2).unwrap();
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(dir.path().join("dashboard.0.2.0.bad")).unwrap(), "new");
        // Put in place again, then the next start removes what is kept, and nothing else.
        stage(&exe, b"new").unwrap();
        put_in_place(&exe, V1).unwrap();
        std::fs::write(dir.path().join("README.txt"), "").unwrap();
        assert_eq!(remove_old(dir.path()), vec![dir.path().join("dashboard.0.1.0.old")]);
        let mut left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        left.sort();
        assert_eq!(left, ["README.txt", "dashboard.0.2.0.bad", "dashboard.exe"]);
    }

    #[test]
    fn the_automatic_check_is_due_a_day_after_the_last() {
        assert!(due(None, 1000));
        assert!(!due(Some(1000), 1000 + DAY - 1));
        assert!(due(Some(1000), 1000 + DAY));
        assert!(due(Some(1000), 999), "a clock that went back");
    }

    #[test]
    fn the_stand_in_serves_a_release_that_is_found_downloaded_and_checked() {
        let exe = b"MZ the new program".to_vec();
        let service = fake::FakeReleases::start(0, fake::Script::new(V2, exe.clone())).unwrap();
        let agent = agent();
        let Checked::Newer(release) = check(&agent, &service.url(), V1).unwrap() else { panic!("not newer") };
        assert_eq!(release.version, V2);
        assert_eq!(download(&agent, &release).unwrap(), exe);
        assert_eq!(check(&agent, &service.url(), V2).unwrap(), Checked::UpToDate(V2));
        // A request carries nothing but the address and the plain User-Agent.
        let requests = service.requests();
        assert!(requests.iter().all(|r| r.user_agent.as_deref() == Some(USER_AGENT) && r.authorization.is_none()));

        let bad = fake::FakeReleases::start(0, fake::Script { bad_hash: true, ..fake::Script::new(V2, exe) }).unwrap();
        let Checked::Newer(release) = check(&agent, &bad.url(), V1).unwrap() else { panic!("not newer") };
        assert!(matches!(download(&agent, &release), Err(Error::BadHash { .. })));

        let private = fake::FakeReleases::start(0, fake::Script { not_public: true, ..fake::Script::new(V2, vec![]) });
        assert_eq!(check(&agent, &private.unwrap().url(), V1).unwrap(), Checked::NotPublic);
    }
}
