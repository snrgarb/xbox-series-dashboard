//! The launchers the Dashboard brings games in from, as Settings > Launchers shows them (docs/spec/launchers.md):
//! each switched on or off, whether its client was found on this PC when it was last looked for, when it was last
//! read and how many games the library has of it; and how the update that reads them all again is going.
//!
//! The library database keeps all of it but the update (`library::launchers`) and gives it in every snapshot
//! (`Library::launchers`); the update is the executable's, which puts it on each snapshot it hands the screens.
use serde::{Deserialize, Serialize};

/// Every launcher there is a source for, by its key (`Game::source`), in the order Settings lists them.
pub const ALL: [&str; 7] = ["steam", "epic", "gog", "ea", "ubisoft", "battlenet", "xbox"];

/// A launcher's name, as Settings > Launchers and the Game hub write it, by its key; none for a key of no launcher.
pub fn name(key: &str) -> Option<&'static str> {
    Some(match key {
        "steam" => "Steam",
        "epic" => "Epic Games",
        "gog" => "GOG",
        "ea" => "EA app",
        "ubisoft" => "Ubisoft Connect",
        "battlenet" => "Battle.net",
        "xbox" => "Xbox",
        _ => return None,
    })
}

/// What was found the last time a launcher was looked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Found {
    /// Never looked for: a library that has not been read since launchers were kept count of, or one switched off
    /// since before then.
    #[default]
    NotYet,
    /// Its client or its files are on this PC, and were read.
    Installed,
    /// Neither its client nor its files are on this PC.
    NotInstalled,
    /// Its files are there and could not be read: the log says why. Its games are as they were.
    Failed,
}

/// One launcher, as the library has it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launcher {
    /// The source's key: `steam`.
    pub key: String,
    /// Its games come in, and are shown. Off, they are kept and shown nowhere, and it is not read.
    pub on: bool,
    pub found: Found,
    /// When it was last read with something found, in Unix seconds.
    pub read: Option<u64>,
    /// How many games the library has of it now: what its last read gave, hidden ones too.
    pub games: usize,
}

impl Launcher {
    /// Whether the user may switch it: one not on this PC may not, unless it has games kept from when it was, or is
    /// off already (to be switched on again).
    pub fn switchable(&self) -> bool {
        self.found != Found::NotInstalled || self.games > 0 || !self.on
    }
}

/// How the update that reads every launcher again is going ("Update now").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    /// None since the Dashboard started.
    #[default]
    Idle,
    /// Under way.
    Working,
    /// Done: what it changed, in a few words ("1 new game, play time for 1"; "nothing new").
    Done(String),
    /// It could not be done: the library database could not be opened or written. The log says why.
    Failed,
}

/// What Settings > Launchers shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launchers {
    /// One a key of [`ALL`], in that order. Empty in a Library that came from no database.
    pub list: Vec<Launcher>,
    pub update: Update,
}

impl Launchers {
    pub fn get(&self, key: &str) -> Option<&Launcher> {
        self.list.iter().find(|launcher| launcher.key == key)
    }

    /// Whether the launcher's games are shown: every source's that is not switched off, those of no launcher too.
    pub fn is_on(&self, key: &str) -> bool {
        self.get(key).is_none_or(|launcher| launcher.on)
    }
}
