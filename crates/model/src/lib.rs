//! The domain: what a game is, the library of them, a saved filter, a Play session. No I/O.
pub mod achievement;
pub mod audio;
pub mod by_hand;
pub mod discord;
pub mod launcher;
pub mod media;
pub mod post;
pub mod profile;
pub mod queue;
pub mod shell_mode;
pub mod sound;
pub mod source;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use achievement::{AchievementSource, Achievements};
pub use audio::{Audio, AudioDevice, Flow};
pub use by_hand::{Added, App, AppKind, ByHand, StoreApp};
pub use launcher::{Launcher, Launchers};
pub use media::{Media, MediaCommand, MediaSession};
pub use profile::{Picture, Profile, ProfileChange, Profiles};
pub use queue::{Install, InstallState, Installed, Queue, Queued};
pub use sound::Sound;
pub use source::{Ask, Found, Source, Start};

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
    /// Which store the game is from, by its source's key (`"steam"`), as the library database has it. Empty for a
    /// made-up game.
    #[serde(default)]
    pub source: String,
    /// The store's own id for the game (Steam's appid, `"440"`): with `source`, what tells it from every other
    /// game, in the library database and across its snapshots.
    #[serde(default)]
    pub source_id: String,
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
    /// Square art (the Microsoft Store's box art, 1:1), if on disk: what a square tile shows, else its cover cropped.
    #[serde(default)]
    pub square: Option<PathBuf>,
    /// Where the store's own pictures of the game are online, as far as its client's files on this PC say: what the
    /// art lookup asks for before SteamGridDB (docs/sources.md, "Art"). Never a picture itself.
    #[serde(default, skip_serializing_if = "OnlineArt::is_empty")]
    pub online: OnlineArt,
    /// The user's own mark. A source may say a game it has just read is one (Steam's Favourites): the database
    /// takes that when it first sees the game and never again.
    #[serde(default)]
    pub favourite: bool,
    /// The user has hidden the game from every list. As `favourite`: a source's word (Steam's hidden games) counts
    /// when the game is first seen. In a snapshot only the games of `Library::hidden` are.
    #[serde(default)]
    pub hidden: bool,
    /// When the game came out, in Unix seconds; 0 when its store does not say. This and the five that follow are
    /// what the Game hub tells of a game beyond its name (docs/spec/game-hub.md, 14.1): only what the store's own
    /// files on this machine hold, so any of them may be missing.
    #[serde(default)]
    pub released: u64,
    /// Who made it, as the store names them.
    #[serde(default)]
    pub developers: Vec<String>,
    /// Who published it.
    #[serde(default)]
    pub publishers: Vec<String>,
    /// What the installed game takes on disk, in bytes; 0 when it is not installed or the store does not say.
    #[serde(default)]
    pub size_on_disk: u64,
    /// The critics' score, 0 to 100 (Steam's is Metacritic's).
    #[serde(default)]
    pub critic_score: Option<u8>,
    /// The share of the store's players who recommend it, 0 to 100.
    #[serde(default)]
    pub community_score: Option<u8>,
    /// How many Play sessions the library database has of it: the times it was played from the Dashboard. Only a
    /// snapshot counts them; a source reads 0.
    #[serde(default)]
    pub play_count: u32,
}

/// What a store's client on this PC says of the game's own art online: the id its public catalogue knows the game by
/// where that is not `Game::source_id` (an Xbox game's Store product id), and the addresses of pictures it has
/// named (the Epic launcher's catalogue cache names its games' key images). Each is optional; most games have none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnlineArt {
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub hero: Option<String>,
    #[serde(default)]
    pub header: Option<String>,
    #[serde(default)]
    pub square: Option<String>,
}

impl OnlineArt {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Each part by the name the library database keeps it under (`online:<name>`), with its value.
    pub fn parts(&self) -> [(&'static str, &Option<String>); 5] {
        [
            ("product", &self.product),
            ("cover", &self.cover),
            ("hero", &self.hero),
            ("header", &self.header),
            ("square", &self.square),
        ]
    }

    /// The part of this name, to be set.
    pub fn part_mut(&mut self, name: &str) -> Option<&mut Option<String>> {
        match name {
            "product" => Some(&mut self.product),
            "cover" => Some(&mut self.cover),
            "hero" => Some(&mut self.hero),
            "header" => Some(&mut self.header),
            "square" => Some(&mut self.square),
            _ => None,
        }
    }
}

/// What tells one game from another across snapshots of the library: its row in the library database, and its
/// store and the store's id for it, which tell apart the games of a library that is in no database (all of whose
/// rows are 0). A game owned in two stores is two games.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct GameKey {
    pub id: i64,
    pub source: String,
    pub source_id: String,
}

impl Game {
    pub fn key(&self) -> GameKey {
        GameKey { id: self.id, source: self.source.clone(), source_id: self.source_id.clone() }
    }

    /// Whether this is the game `key` names.
    pub fn is(&self, key: &GameKey) -> bool {
        self.id == key.id && self.source == key.source && self.source_id == key.source_id
    }
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

/// How long ago a game was last played, by the calendar where the user is: the sections of Play history, in their
/// order (docs/spec/my-games.md, 6.1). The first that holds is the game's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ago {
    Today,
    Yesterday,
    /// Since the week's first day.
    ThisWeek,
    LastWeek,
    /// Since the 1st.
    ThisMonth,
    LastMonth,
    ThisYear,
    Older,
}

impl Ago {
    /// The section's title.
    pub fn title(self) -> &'static str {
        match self {
            Ago::Today => "Today",
            Ago::Yesterday => "Yesterday",
            Ago::ThisWeek => "This week",
            Ago::LastWeek => "Last week",
            Ago::ThisMonth => "This month",
            Ago::LastMonth => "Last month",
            Ago::ThisYear => "This year",
            Ago::Older => "Older",
        }
    }
}

/// A day of the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Day {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
}

/// A row of Play history: a game that has been played, and when it last was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRow {
    /// The game's id in the library database, found with `Library::game`.
    pub game: i64,
    /// Unix seconds: the latest of what its source says and the Play sessions seen here.
    pub at: u64,
    /// The day that was, where the user is.
    pub day: Day,
    /// The section it is in, as of when the snapshot was taken.
    pub ago: Ago,
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

/// A change to the saved filters, as Settings' Groups page asks for it (docs/spec/filters.md, section 6). Whoever
/// owns the library database carries it out (`library::Database::change_filter`) and shows the snapshot after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterChange {
    /// Whether the filter is a Group on Home and a tab on Full library.
    Shown { filter: i64, shown: bool },
    /// Its place among all the filters: -1 one up, +1 one down.
    Move { filter: i64, by: i32 },
    /// A custom filter of this name: every game, by name, shown, last.
    Add { name: String },
    /// A custom filter's name, what it picks and how it sorts, all at once.
    Set { filter: i64, name: String, rule: Rule, sort: Sort, descending: bool },
    /// A custom filter goes.
    Delete { filter: i64 },
}

/// One of a store's own groupings of games, as a source read it: a Steam collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// The store's own id for it (Steam's `uc-...`), the same however it is renamed.
    pub source_id: String,
    pub name: String,
    /// The store's ids of its games, as `Game::source_id` has them. Some may be of no game the library has: a
    /// tool, DLC, a game no longer owned.
    pub games: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    /// The signed-in user's name at a store, if one says it: in a snapshot, that of the source with the most games.
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
    /// Play history: every game that has been played and is not hidden, installed or not, the last played first.
    /// Only a snapshot of the library database has it.
    #[serde(default)]
    pub history: Vec<HistoryRow>,
    /// The store's collections as a source has just read them, for the database to take in. `None` when they
    /// could not be read, which leaves the database's as they are; and always in a snapshot.
    #[serde(default)]
    pub collections: Option<Vec<Collection>>,
    /// The Queue: every game being installed, and those it held that have finished. Only a snapshot of the library
    /// database has it; a source reads none.
    #[serde(default)]
    pub queue: Queue,
    /// The Apps (`by_hand`): not games, so in none of the lists above. In the order they were added. Only a
    /// snapshot of the library database has any.
    #[serde(default)]
    pub apps: Vec<App>,
    /// The Microsoft Store apps installed on this machine, for the user to tick as Apps: not the database's, but
    /// listed by the executable as it starts. None until they have been listed, and where they can't be (anywhere
    /// but Windows).
    #[serde(default)]
    pub store_apps: Option<Vec<StoreApp>>,
    /// Who plays here (`profile`), and who is signed in. Only a snapshot of the library database has any.
    #[serde(default)]
    pub profiles: Profiles,
    /// The Profile's achievement totals, the last unlocks and how far each game has got (`achievement`). Only a
    /// snapshot of the library database has any.
    #[serde(default)]
    pub achievements: Achievements,
    /// The launchers games come in from, each switched on or off, and how an update of them is going (Settings >
    /// Launchers). Only a snapshot of the library database has them; the games of one switched off are in none of
    /// the lists above.
    #[serde(default)]
    pub launchers: Launchers,
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
