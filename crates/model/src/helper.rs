//! The helper programs some stores install through (legendary for Epic Games, gogdl for GOG; docs/spec/helpers.md,
//! docs/research/installs.md 10.1): what Settings > Helpers shows of each and asks of the shell. Fetching, checking
//! and removing them is the `helpers` crate's; running one is `shell::helper`'s. No I/O here.
use serde::{Deserialize, Serialize};

/// A helper as Settings shows it: what it is, where it comes from, and how it stands on this PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Helper {
    /// Its key: `legendary`, `gogdl`.
    pub key: String,
    /// The store it is for: `Epic Games`.
    pub store: String,
    /// The version the Dashboard fetches, fixed in its own source.
    pub version: String,
    /// Its licence, as its project names it: `GPL-3.0`.
    pub licence: String,
    /// Its source: the project's own page.
    pub source: String,
    pub state: HelperState,
}

/// How a helper stands on this PC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperState {
    /// Not in the data folder.
    #[default]
    NotAdded,
    /// Being fetched and checked, on the user's Yes.
    Fetching,
    /// In the data folder, this version, and it ran.
    Added { version: String },
    /// The last try to add it failed: why, in words for the user.
    Failed(String),
}

/// What Settings asks of the shell about a helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperAsk {
    /// Fetch it, check it and run it once: the user said Yes, its licence and source in front of them.
    Add(String),
    /// Delete it from the data folder.
    Remove(String),
}
