//! Updates as Settings > About shows them (docs/spec/updates.md): which version was found, and how getting it goes.
//! Checking, downloading and installing are the `update` crate's and the executable's; the core only shows where it
//! stands and asks.

use std::fmt;

/// A version of the Dashboard: `major.minor.patch`, as the workspace's (docs/release.md) and a release's tag (`v`
/// and it) say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// `1.2.3` or `v1.2.3`, nothing before or after; none for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut parts = text.split('.');
        let mut part = || -> Option<u32> {
            let part = parts.next()?;
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse().ok())?
        };
        let version = Self::new(part()?, part()?, part()?);
        parts.next().is_none().then_some(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Where updates stand, as the executable last said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Updates {
    /// Not checked since the Dashboard started.
    #[default]
    Unchecked,
    /// Asking the update source.
    Checking,
    /// The source's newest is this version, which is not newer than this one.
    UpToDate(Version),
    /// The source answered 404: the repository's releases are not public (or there is none yet). Said plainly,
    /// never as an error.
    NotPublic,
    /// The source could not be asked, or its answer read: the log says why.
    Failed,
    /// A newer version was found and is being downloaded and checked.
    Downloading(Version),
    /// Downloaded, its SHA-256 the release's own: Install puts it in place.
    Ready(Version),
    /// The download's SHA-256 is not the one the release's manifest gives: it was thrown away.
    BadDownload(Version),
    /// Install was pressed: the new version is being put in place and started.
    Installing(Version),
    /// The new version was put in place but did not start: this one was put back.
    RolledBack(Version),
    /// The new version could not be put in place (a file in the way): nothing was changed.
    InstallFailed(Version),
}

impl Updates {
    /// Whether a check or an install is under way: Check for updates does nothing meanwhile.
    pub fn busy(self) -> bool {
        matches!(self, Updates::Checking | Updates::Downloading(_) | Updates::Installing(_))
    }

    /// As the log says it: `up-to-date 0.1.0`, `ready 0.2.0`, `not-public`.
    pub fn state(self) -> String {
        match self {
            Updates::Unchecked => "unchecked".into(),
            Updates::Checking => "checking".into(),
            Updates::UpToDate(v) => format!("up-to-date {v}"),
            Updates::NotPublic => "not-public".into(),
            Updates::Failed => "failed".into(),
            Updates::Downloading(v) => format!("downloading {v}"),
            Updates::Ready(v) => format!("ready {v}"),
            Updates::BadDownload(v) => format!("bad-download {v}"),
            Updates::Installing(v) => format!("installing {v}"),
            Updates::RolledBack(v) => format!("rolled-back {v}"),
            Updates::InstallFailed(v) => format!("install-failed {v}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_three_numbers_with_or_without_a_v() {
        assert_eq!(Version::parse("v0.2.0"), Some(Version::new(0, 2, 0)));
        assert_eq!(Version::parse("10.20.30"), Some(Version::new(10, 20, 30)));
        for bad in ["", "v", "1.2", "1.2.3.4", "1.2.x", "1.2.3-beta", "v1..3", "+1.2.3"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert!(Version::new(0, 10, 0) > Version::new(0, 9, 9));
        assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
    }
}
