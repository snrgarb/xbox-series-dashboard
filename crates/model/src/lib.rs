//! The domain: what a game is, the library of them, a saved filter, a Play session. No I/O.
pub mod sound;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use sound::Sound;

/// What Power can do to the machine (`docs/spec/guide.md`, section 6.2). The Dashboard's core only ever asks for
/// one; carrying it out is the shell's, behind `shell::Power`. Leaving to the desktop is not one of them: that is
/// the Dashboard's own to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerCommand {
    TurnOff,
    Restart,
    Sleep,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Game {
    /// The game's row in the library database, the same for as long as the database lives. 0 for a game that is in
    /// no database: one a source has just read, or a made-up one.
    #[serde(default)]
    pub id: i64,
    pub appid: u32,
    /// Which store the game is from (`"steam"`), as the library database has it. Empty for a made-up game.
    #[serde(default)]
    pub source: String,
    pub name: String,
    /// Unix seconds; 0 = never played.
    pub last_played: u64,
    pub playtime_minutes: u32,
    /// Unix seconds: when the game first came into the library database, which is not when it was bought. 0 for a
    /// game that is in no database.
    #[serde(default)]
    pub added: u64,
    pub installed: bool,
    /// Where the game is installed, if it is.
    #[serde(default)]
    pub install_dir: Option<PathBuf>,
    /// Portrait cover (Steam's 600x900 library capsule), if on disk.
    pub cover: Option<PathBuf>,
    /// Wide background art (Steam's library hero), if on disk.
    pub hero: Option<PathBuf>,
    /// Landscape header (460x215), if on disk.
    pub header: Option<PathBuf>,
    /// The user's own mark. A source may say a game it has just read is one (Steam's Favourites): the database
    /// takes that when it first sees the game and never again.
    #[serde(default)]
    pub favourite: bool,
    /// The user has hidden the game from every list. As `favourite`: a source's word (Steam's hidden games) counts
    /// when the game is first seen. In a snapshot only the games of `Library::hidden` are.
    #[serde(default)]
    pub hidden: bool,
}

/// One run of a game, from its window coming up to its ending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaySession {
    /// The game's id in the library database.
    pub game: i64,
    /// Unix seconds.
    pub started: u64,
    /// Unix seconds; `None` while the game runs, and for a session whose end the Dashboard never saw.
    pub ended: Option<u64>,
}

/// Who made a filter, which says what of it can change (docs/spec/filters.md, section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterKind {
    /// Made by a migration: its name and rule are fixed. It can only be shown or not, and moved.
    Builtin,
    /// One of a store's own collections (Steam's): it follows the store's name and games, and goes when the store's
    /// collection goes.
    Collection,
    /// The user's own: renamed, changed and deleted in Settings.
    Custom,
}

/// What a filter's games are ordered by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    /// "A-Z": the name without its case or a leading article.
    #[default]
    Name,
    LastPlayed,
    /// "Recently added".
    Added,
    Playtime,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::Name, Sort::LastPlayed, Sort::Added, Sort::Playtime];

    /// The direction a sort has unless something says otherwise: names upward, the rest newest or most first.
    pub fn default_descending(self) -> bool {
        self != Sort::Name
    }
}

/// `Rule::played`: whether a game has ever been played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Played {
    Played,
    Never,
}

/// Which games a filter picks: every clause that is set must hold, and one left unset holds for every game. A
/// hidden game is picked by no rule but one with `hidden`, and a game its source no longer lists by none at all.
///
/// Kept in the library database as JSON, with only the set clauses in it. A key this program does not know does
/// not parse, and the filter then picks nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favourite: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub played: Option<Played>,
    /// Last played this many days ago at most. A game never played was not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub played_within_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_within_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_playtime_hours: Option<u32>,
    /// From any of these sources, as `Game::source` names them. Empty: from any source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// The hidden games, and only those: the built-in Hidden filter's rule, which Settings does not offer.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// A saved filter with the games it picks: a Group on Home and a tab on Full library while it is shown and has a
/// game (docs/spec/filters.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// Its row in the library database.
    pub id: i64,
    pub kind: FilterKind,
    /// A built-in's own name for itself (`"favourites"`, `"installed"`, `"most-played"`, `"recently-added"`,
    /// `"hidden"`), by which code knows it. No other kind has one.
    #[serde(default)]
    pub key: Option<String>,
    pub name: String,
    pub shown: bool,
    /// What picks its games, for Settings to show and change (a custom filter's only). A collection filter's is
    /// empty: its collection is its rule. Nothing but the library database works a rule out.
    #[serde(default)]
    pub rule: Rule,
    pub sort: Sort,
    pub descending: bool,
    /// Every game the rule picks, in the filter's sort: the games' ids, found with `Library::game`.
    pub games: Vec<i64>,
}

/// One of a store's own groupings of games, as a source read it: a Steam collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// The store's own id for it (Steam's `uc-...`), the same however it is renamed.
    pub source_id: String,
    pub name: String,
    /// The store's ids of its games. Some may be of no game the library has: a tool, DLC, a game no longer owned.
    pub appids: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    /// Steam persona name of the signed-in user, if found.
    pub user: Option<String>,
    /// Every owned game (no tools, DLC, or soundtracks) that is not hidden, most recently played first.
    pub games: Vec<Game>,
    /// The games the user has hidden, by name: in no list but the Hidden filter's. Only a snapshot of the library
    /// database has them apart; what a source reads has every game in `games`.
    #[serde(default)]
    pub hidden: Vec<Game>,
    /// Every saved filter, shown or not, in the user's order, each with its games. Only a snapshot of the library
    /// database has any: a source reads none.
    #[serde(default)]
    pub filters: Vec<Filter>,
    /// The store's collections as a source has just read them, for the database to take in. `None` when they
    /// could not be read, which leaves the database's as they are; and always in a snapshot.
    #[serde(default)]
    pub collections: Option<Vec<Collection>>,
}

impl Library {
    /// The Recent row: the `n` most recently played games.
    pub fn recent(&self, n: usize) -> &[Game] {
        let played = self.games.iter().take_while(|g| g.last_played > 0).count();
        &self.games[..played.min(n)]
    }

    /// The game with this id, hidden or not: how a filter's games are found.
    pub fn game(&self, id: i64) -> Option<&Game> {
        self.games.iter().chain(&self.hidden).find(|g| g.id == id)
    }

    /// The filters that are a Group on Home and a tab on Full library, in order: those shown that have a game.
    pub fn groups(&self) -> impl Iterator<Item = &Filter> {
        self.filters.iter().filter(|f| f.shown && !f.games.is_empty())
    }
}
