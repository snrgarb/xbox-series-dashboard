//! Matching a game to an online source's own entry for it (docs/spec/match-game.md), as Playnite's metadata
//! download lets its user pick the game a search found: the user's word on which of a source's games this one is,
//! where its name alone finds none or the wrong one (Battle.net's "World of Warcraft: Forever" is "World of
//! Warcraft" everywhere else). With no I/O: the library keeps a match (`library::matches`) and searches the sources
//! (`library::art::search`), the screens show the results (`screens::match_game`).
//!
//! A match is per source: the game may be matched on SteamGridDB and left to its name on Steam's store. The art
//! lookup and the art picker ask a matched source for its picture by the match's id, never by the game's name.
use crate::ArtSource;

/// The user's word on which of a source's games a game is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub source: ArtSource,
    /// The game's id at the source: Steam's appid, the Microsoft Store's product id, the PlayStation Store's
    /// content id, SteamGridDB's game id.
    pub id: String,
    /// Its name there, as the search said it: what the page shows, and what the PlayStation Store is searched by
    /// again (its pictures come with its search's results).
    pub name: String,
}

/// One game a source's search found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub id: String,
    pub name: String,
    /// The year it came out, where the source says.
    pub year: Option<u16>,
}

impl Hit {
    /// The match it is on `source`.
    pub fn on(&self, source: ArtSource) -> Match {
        Match { source, id: self.id.clone(), name: self.name.clone() }
    }
}

/// The most results a search shows: the first sources give no more than this, and a page of them is enough to
/// find the game or to say the words were wrong.
pub const MOST_HITS: usize = 25;
