//! Set up this PC (docs/spec/set-up.md, #178): the launchers' own apps (Steam, the Epic Games Launcher, GOG Galaxy,
//! the EA app, Ubisoft Connect, Battle.net), installed and removed from Settings. What the page shows of each and
//! asks of the shell; the table of installers, the download and the runs are the `helpers` crate's (`clients`), the
//! threads the executable's. No I/O here.
use serde::{Deserialize, Serialize};

/// A launcher's app as Settings shows it: what it is, where its installer comes from, and how it stands on this PC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Client {
    /// The launcher's key, as its source has it: `steam`.
    pub key: String,
    /// The app's name: `Steam`, `Epic Games Launcher`.
    pub name: String,
    /// Who makes it: `Valve`.
    pub vendor: String,
    /// The version the Dashboard installs, fixed in its own source.
    pub version: String,
    /// Where its installer is downloaded from, as the question says it: the vendor's own address.
    pub address: String,
    /// The installer's size in bytes.
    pub size: u64,
    /// Its installer shows a window of its own whatever it is told (the EA app's, Battle.net's).
    pub window: bool,
    /// Its uninstaller shows a window of its own (Battle.net's, the EA app's), or none is known to be quiet.
    pub uninstall_window: bool,
    /// What removing it does to its games, in a sentence for the user.
    pub removing: String,
    pub state: ClientState,
}

/// How a launcher's app stands on this PC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientState {
    /// Not registered with Windows as installed.
    #[default]
    NotInstalled,
    /// Registered with Windows as installed, with an uninstaller: removable from here.
    Installed,
    /// Registered as installed, with no uninstaller the Dashboard can run: removed in Windows' Settings.
    InstalledFixed,
    /// Its installer being downloaded and checked: how many bytes so far.
    Downloading { done: u64 },
    /// Its installer running.
    Installing,
    /// Its uninstaller running.
    Removing,
    /// The last thing asked of it did not happen: why, in words for the user. How it stands otherwise is `installed`.
    Failed { why: String, installed: bool },
}

impl ClientState {
    /// Whether it is installed, as far as the Dashboard knows.
    pub fn installed(&self) -> bool {
        match self {
            ClientState::Installed | ClientState::InstalledFixed | ClientState::Removing => true,
            ClientState::Failed { installed, .. } => *installed,
            ClientState::NotInstalled | ClientState::Downloading { .. } | ClientState::Installing => false,
        }
    }

    /// Whether something is under way.
    pub fn busy(&self) -> bool {
        matches!(self, ClientState::Downloading { .. } | ClientState::Installing | ClientState::Removing)
    }
}

/// What Settings asks of the shell about a launcher's app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetUpAsk {
    /// Download its installer, check it and run it: the user said Yes, the vendor, the address and the size in
    /// front of them.
    Install(String),
    /// Run its own registered uninstaller: the user said Yes, what happens to its games in front of them.
    Remove(String),
}

/// A size as the question says it: `2.3 MB`, `264 MB`, `1.2 GB`.
pub fn size(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    let mb = bytes as f64 / MB;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0)
    } else if mb >= 10.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{:.1} MB", mb.max(0.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_said_in_megabytes_or_gigabytes() {
        assert_eq!(size(2_380_800), "2.4 MB");
        assert_eq!(size(264_237_056), "264 MB");
        assert_eq!(size(1_250_000_000), "1.3 GB");
        assert_eq!(size(10), "0.1 MB");
    }

    #[test]
    fn a_failure_keeps_whether_it_is_installed() {
        assert!(ClientState::Failed { why: "x".into(), installed: true }.installed());
        assert!(!ClientState::Failed { why: "x".into(), installed: false }.installed());
        assert!(ClientState::Removing.installed() && ClientState::Removing.busy());
        assert!(!ClientState::Installing.installed() && ClientState::Installing.busy());
    }
}
