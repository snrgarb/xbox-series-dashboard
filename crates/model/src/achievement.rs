//! Achievements (docs/achievements.md): a game's, as its store's own files on this PC say, with how rare each is;
//! what each is worth (its **Score**) and whether it is **Rare**, by the Bundle's rules (`CONTEXT.md`); and the
//! totals a snapshot of the library carries.
//!
//! A store's achievements are read through [`AchievementSource`], as its games are through `Source`: Steam's from
//! `appcache/stats` (`source_steam::achievements`). How rare each is comes from the store's own public figures, which
//! the library asks for and keeps (`library::achievements`), never the source.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// An achievement fewer than this percentage of players have is **Rare**, as the console's rule; one whose rarity
/// is not known never is.
pub const RARE_BELOW: f64 = 10.0;

/// A Score by rarity, for an achievement with no points of its own: at most each percentage here, the Score in the
/// same place of [`SCORE_AT_RARITY`]; anything commoner, or of unknown rarity, [`COMMON_SCORE`].
pub const RARITY_AT_MOST: [f64; 5] = [1.0, 5.0, 10.0, 25.0, 50.0];
pub const SCORE_AT_RARITY: [u32; 5] = [100, 50, 30, 20, 15];
pub const COMMON_SCORE: u32 = 10;

/// How many of the last unlocks a snapshot carries: the Guide's Profile & system lists three.
pub const RECENT: usize = 3;

/// One achievement of a game.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Achievement {
    /// What tells it from the game's other achievements: Steam's API name. The same from one read to the next.
    pub key: String,
    /// Its name, as the game shows it.
    pub name: String,
    /// What it asks of the player; none where the store has none.
    #[serde(default)]
    pub description: Option<String>,
    /// The store hides it until it is unlocked: its name and description are a secret until then.
    #[serde(default)]
    pub hidden: bool,
    /// Where its unlocked icon is online, as the store names it.
    #[serde(default)]
    pub icon: Option<String>,
    /// Where its locked (grey) icon is online.
    #[serde(default)]
    pub icon_locked: Option<String>,
    /// The icon in our own cache, once fetched: what the screens draw. Filled by the library, never by a source.
    #[serde(default)]
    pub icon_file: Option<PathBuf>,
    /// The locked icon in our own cache, once fetched.
    #[serde(default)]
    pub icon_locked_file: Option<PathBuf>,
    /// The percentage of the store's players who have unlocked it, 0 to 100; none while that is not known.
    #[serde(default)]
    pub rarity: Option<f64>,
    /// Its points on its own store, where the store has them (Xbox's Gamerscore); none for Steam, which has none.
    #[serde(default)]
    pub points: Option<u32>,
    /// When it was unlocked, in Unix seconds; none while it is locked. `Some(0)`: unlocked, at a time not known.
    #[serde(default)]
    pub unlocked: Option<u64>,
}

impl Achievement {
    pub fn is_unlocked(&self) -> bool {
        self.unlocked.is_some()
    }

    /// Its **Score**: its store's own points where it has them, 0 included, else by its rarity ([`score_of`]).
    pub fn score(&self) -> u32 {
        self.points.unwrap_or_else(|| score_of(self.rarity))
    }

    /// Whether it is **Rare**: fewer than [`RARE_BELOW`] percent of players have it. Unknown rarity never is.
    pub fn is_rare(&self) -> bool {
        self.rarity.is_some_and(|rarity| rarity < RARE_BELOW)
    }
}

/// The Score of an achievement with no points of its own, by its rarity: 1% or less 100, 5% or less 50, 10% or less
/// 30, 25% or less 20, 50% or less 15, and anything commoner, or unknown, 10.
pub fn score_of(rarity: Option<f64>) -> u32 {
    let Some(rarity) = rarity else { return COMMON_SCORE };
    RARITY_AT_MOST.iter().zip(SCORE_AT_RARITY).find(|(&most, _)| rarity <= most).map_or(COMMON_SCORE, |(_, s)| s)
}

/// One unlock of one game's: what the Achievement unlocked Toast tells of, and Profile & system lists.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Unlock {
    /// The game's row in the library database.
    pub game: i64,
    pub achievement: Achievement,
}

/// How far one game has got.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameProgress {
    /// The game's row in the library database.
    pub game: i64,
    pub unlocked: u32,
    /// Of how many.
    pub of: u32,
    /// The Score of those unlocked.
    pub score: u64,
}

/// The **Profile**'s totals, over every game this PC has achievements for. As the Bundle's, they are this PC's and
/// not a Profile's own: every Profile shows the same.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    /// The Score of every unlock.
    pub score: u64,
    /// How many achievements are unlocked.
    pub unlocked: u32,
    /// Of how many, in every game that has any.
    pub of: u32,
    /// How many games have achievements.
    pub games: u32,
}

/// What a snapshot of the library carries of achievements. Only a snapshot of the library database has any; one
/// game's whole list is `library::Database::achievements`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Achievements {
    /// None while no game has any achievements: the Bundle shows no totals then.
    #[serde(default)]
    pub totals: Option<Totals>,
    /// The last [`RECENT`] unlocks, newest first (one of unknown time counts as the oldest).
    #[serde(default)]
    pub recent: Vec<Unlock>,
    /// Every game that has achievements, by its id.
    #[serde(default)]
    pub games: Vec<GameProgress>,
}

impl Achievements {
    /// How far this game has got, if it has achievements.
    pub fn of(&self, game: i64) -> Option<&GameProgress> {
        self.games.iter().find(|progress| progress.game == game)
    }
}

/// What the Dashboard asks of a store's achievements: the one interface every store's are behind, as `Source` is
/// for its games. Only the store's own files on this PC, never the network: how rare each is, and the icons, are
/// the library's to fetch (`library::achievements`). Steam is the only one (`source_steam::achievements`); a store
/// that needs a sign-in (Xbox, Epic, GOG) can join later behind it.
pub trait AchievementSource: Send + Sync {
    /// The key of the `Source` whose games these are (`"steam"`): a game is matched by it and its `source_id`.
    fn key(&self) -> &'static str;

    /// The games this PC has the store's achievements for, by their `source_id`; None when the client is not
    /// installed.
    fn games(&self) -> Option<Vec<String>>;

    /// One game's achievements as the files say now, in the store's order, with no rarity and no icon file; Ok(None)
    /// when the PC has none for it.
    fn read(&self, source_id: &str) -> anyhow::Result<Option<Vec<Achievement>>>;

    /// The files a change to which means the game's achievements should be read again: what is watched while the
    /// game runs.
    fn files(&self, source_id: &str) -> Vec<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_score_follows_the_bundles_rarity_table() {
        let cases = [
            (Some(0.0), 100),
            (Some(1.0), 100),
            (Some(1.01), 50),
            (Some(5.0), 50),
            (Some(9.95), 30),
            (Some(10.0), 30),
            (Some(25.0), 20),
            (Some(50.0), 15),
            (Some(50.1), 10),
            (Some(100.0), 10),
            (None, 10),
        ];
        for (rarity, score) in cases {
            assert_eq!(score_of(rarity), score, "{rarity:?}");
        }
    }

    #[test]
    fn points_of_its_own_win_even_when_none() {
        let a = Achievement { rarity: Some(0.5), points: Some(0), ..Achievement::default() };
        assert_eq!(a.score(), 0);
        let a = Achievement { rarity: Some(0.5), points: Some(25), ..Achievement::default() };
        assert_eq!(a.score(), 25);
    }

    #[test]
    fn rare_is_under_ten_percent_and_never_unknown() {
        let rare = |rarity| Achievement { rarity, ..Achievement::default() }.is_rare();
        assert!(rare(Some(9.99)));
        assert!(rare(Some(0.0)));
        assert!(!rare(Some(10.0)));
        assert!(!rare(None));
    }
}
