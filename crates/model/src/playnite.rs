//! Import from Playnite, once (docs/import.md): what a Playnite library on this PC says of the games this library
//! has, as a plan the library database carries out, and how far the import has got, as Settings shows it.
//!
//! The plan is made by the `playnite` crate from Playnite's own files, and carried out by `library::import`; the
//! screens only ever see an [`Import`].
use std::path::PathBuf;

use crate::{Rule, Sort};

/// What one of Playnite's games says, under the store and the store's own id it is matched by: never its name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    /// The Dashboard's key for the store (`steam`), from Playnite's library plugin.
    pub source: String,
    /// The store's own id for it, as Playnite keeps it (`GameId`: Steam's appid).
    pub source_id: String,
    /// Playnite's name for it: for the log, never to match by.
    pub name: String,
    /// Playnite's play time, in whole minutes.
    pub playtime_minutes: u32,
    /// When Playnite last saw it played, in Unix seconds; 0 for never.
    pub last_played: u64,
    pub favourite: bool,
    pub hidden: bool,
}

/// What a Group from Playnite picks.
#[derive(Debug, Clone, PartialEq)]
pub enum Picks {
    /// A rule the library works out for itself, as it does for a Group made in Settings: the preset asked only
    /// what a rule can say (installed, favourite, hidden, which stores).
    Rule(Rule),
    /// These games, by store and store id, as Playnite's filter picked them on the day: the preset asked what only
    /// Playnite knows of a game (its categories, tags, genres, ...).
    Games(Vec<(String, String)>),
}

/// One of the Bundle's Groups: a Playnite filter preset flagged for Fullscreen's quick selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Playnite's id for the preset, by which a second import knows it brought it already.
    pub id: String,
    pub name: String,
    pub picks: Picks,
    pub sort: Sort,
    pub descending: bool,
}

/// What an import would bring, read from Playnite and not yet compared with this library.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// Playnite's library folder it was read from.
    pub from: PathBuf,
    /// How many games Playnite has.
    pub games_read: u32,
    /// Its games from a store the Dashboard reads, each with what Playnite says of it.
    pub games: Vec<Marks>,
    /// How many of its games are from a library plugin the Dashboard has no store for (added by hand, emulated,
    /// another store's plugin): they cannot be matched, and are left.
    pub other_stores: u32,
    /// The Bundle's Groups, in Playnite's order, that can come over.
    pub groups: Vec<Group>,
    /// The Bundle's Groups that cannot, by name, each with why ("it filters by release year").
    pub skipped: Vec<(String, String)>,
}

/// What an import brings, or brought, measured against this library: only what would change something.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preview {
    /// Playnite's library folder.
    pub from: String,
    /// How many games Playnite has.
    pub games_read: u32,
    /// Of those, how many this library has, by store and store id.
    pub matched: u32,
    /// Games whose play time goes up (Playnite has counted more than this library).
    pub playtime: u32,
    /// Games whose last played goes later.
    pub last_played: u32,
    /// Games newly marked a favourite.
    pub favourites: u32,
    /// Games newly hidden.
    pub hidden: u32,
    /// Groups that come over, by name, in order.
    pub groups: Vec<String>,
    /// Playnite's games from a store the Dashboard reads that this library does not have.
    pub not_here: u32,
    /// Playnite's games from a library plugin the Dashboard has no store for.
    pub other_stores: u32,
    /// The Bundle's Groups that cannot come over, each with why.
    pub skipped: Vec<(String, String)>,
}

impl Preview {
    /// Whether importing would change nothing.
    pub fn is_empty(&self) -> bool {
        self.playtime == 0
            && self.last_played == 0
            && self.favourites == 0
            && self.hidden == 0
            && self.groups.is_empty()
    }

    /// What it brings, in a line for the log and the page: "play time for 2 games, 1 favourite, Groups: A, B".
    pub fn describe(&self) -> String {
        let games = |n: u32| if n == 1 { "1 game".to_string() } else { format!("{n} games") };
        let mut parts = Vec::new();
        if self.playtime > 0 {
            parts.push(format!("play time for {}", games(self.playtime)));
        }
        if self.last_played > 0 {
            parts.push(format!("last played for {}", games(self.last_played)));
        }
        if self.favourites > 0 {
            parts.push(if self.favourites == 1 {
                "1 favourite".into()
            } else {
                format!("{} favourites", self.favourites)
            });
        }
        if self.hidden > 0 {
            parts.push(format!("{} hidden", games(self.hidden)));
        }
        if !self.groups.is_empty() {
            let word = if self.groups.len() == 1 { "Group" } else { "Groups" };
            parts.push(format!("{word}: {}", self.groups.join(", ")));
        }
        if parts.is_empty() {
            "nothing new".into()
        } else {
            parts.join(", ")
        }
    }
}

/// What Settings asks of whoever has Playnite's files and the library database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Find Playnite's library and say what an import would bring.
    Look,
    /// Bring it.
    Import,
}

/// How the import stands, as Settings > Import from Playnite shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Import {
    /// Not looked for yet, or being looked for.
    #[default]
    Looking,
    /// No Playnite library on this PC.
    NotFound,
    /// There is one, and it could not be read: why, in words for the page.
    Unreadable(String),
    /// What an import would bring.
    Ready(Preview),
    /// Brought: what it brought.
    Done(Preview),
}

impl Import {
    /// A word or two for the focus log: `looking`, `not found`, `unreadable`, `ready`, `nothing new`, `done`.
    pub fn state_line(&self) -> &'static str {
        match self {
            Import::Looking => "looking",
            Import::NotFound => "not found",
            Import::Unreadable(_) => "unreadable",
            Import::Ready(preview) if preview.is_empty() => "nothing new",
            Import::Ready(_) => "ready",
            Import::Done(_) => "done",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_says_what_it_brings() {
        let mut preview = Preview::default();
        assert!(preview.is_empty());
        assert_eq!(preview.describe(), "nothing new");
        preview.playtime = 1;
        preview.last_played = 2;
        preview.favourites = 1;
        preview.hidden = 1;
        preview.groups = vec!["Couch picks".into(), "Installed".into()];
        assert_eq!(
            preview.describe(),
            "play time for 1 game, last played for 2 games, 1 favourite, 1 game hidden, Groups: Couch picks, Installed"
        );
        assert!(!preview.is_empty());
        // Not matching anything is not something to bring.
        let left = Preview { not_here: 3, other_stores: 1, ..Preview::default() };
        assert!(left.is_empty());
        assert_eq!(Import::Ready(left).state_line(), "nothing new");
    }
}
