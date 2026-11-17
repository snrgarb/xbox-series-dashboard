//! Settings > Art's "Refresh all art and details" (docs/spec/settings.md, 16.8; #190): everything the Dashboard
//! fetched from outside for the games is forgotten, and fetched again in the background by the art lookup and the
//! achievements' thread, as they fetch it at a start.
//!
//! What is fetched from outside, and so forgotten: the art cache (every picture downloaded, and the misses
//! remembered), the achievements' icons, and when each game's rarity was last asked (so every game's is asked again;
//! the figures kept stand until the new ones come). Never what is the user's or the stores' files': the games, play
//! time and Play sessions, favourites, hidden games, Groups and pins, Profiles, settings, the SteamGridDB key, a
//! store's own pictures on the PC.
//!
//! The settings row [`SETTING`] says a refresh was asked for and has not finished: it is written before anything is
//! forgotten and removed once a lookup has looked at every game, so a refresh cut short (the Dashboard stopped,
//! art online turned off) goes on at the next start where it stopped: what came in stays, what did not is looked up.
use anyhow::Result;

use crate::achievements::IconDir;
use crate::Database;

/// The settings row that says a refresh is under way: `true` from the press until a lookup has been through every
/// game.
pub const SETTING: &str = "art.refresh";

/// What a refresh forgot, in counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Forgot {
    /// Pictures of the art cache.
    pub pictures: usize,
    /// Games whose achievements' rarity will be asked for again.
    pub rarity: usize,
}

impl std::fmt::Display for Forgot {
    /// `forgot 120 pictures and when 30 games' rarity was asked`
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "forgot {} pictures, the achievements' icons and {} games' rarity", self.pictures, self.rarity)
    }
}

impl Database {
    /// Starts a refresh: marks it under way, then forgets everything fetched from outside (above).
    pub fn forget_fetched(&self) -> Result<Forgot> {
        self.set_setting(SETTING, "true")?;
        let pictures = match self.art_dir() {
            Some(art) => art.clear()?,
            None => 0,
        };
        if let Some(root) = self.achievement_icons().as_ref().map(IconDir::root) {
            match std::fs::remove_dir_all(root) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        let rarity = self.conn.execute("DELETE FROM achievement_rarity", [])?;
        Ok(Forgot { pictures, rarity })
    }

    /// Whether a refresh was asked for and a lookup has not yet been through every game.
    pub fn refreshing(&self) -> Result<bool> {
        Ok(self.setting(SETTING)?.is_some())
    }

    /// A lookup has been through every game: the refresh is over.
    pub fn refreshed(&self) -> Result<()> {
        self.delete_setting(SETTING)
    }

    /// Forgets one game's downloaded art, its pictures and its misses, for the Tile menu's Refresh art: gives how
    /// many pictures went.
    pub fn forget_art_of(&self, source: &str, source_id: &str) -> Result<usize> {
        match self.art_dir() {
            Some(art) => art.forget(source, source_id),
            None => Ok(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use model::{Game, Library};

    use super::*;
    use crate::art::{ArtDir, Kind, Online};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nmade up";

    fn made_up(id: &str, name: &str) -> Game {
        Game { source: "steam".into(), source_id: id.into(), name: name.into(), ..Game::default() }
    }

    #[test]
    fn a_refresh_forgets_what_was_fetched_and_nothing_of_the_users() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open(&dir.path().join("library.db")).unwrap();
        let games = vec![
            Game { last_played: 20, playtime_minutes: 90, favourite: true, ..made_up("1", "First Made-up Game") },
            made_up("2", "Second Made-up Game"),
        ];
        db.sync("steam", &Library { games, ..Library::default() }, 100).unwrap();
        db.set_setting(crate::art::KEY_SETTING, "a-made-up-key").unwrap();
        let art = ArtDir::under(dir.path());
        art.save("steam", "1", Kind::Square, PNG).unwrap();
        art.save("steam", "2", Kind::Cover, PNG).unwrap();
        art.miss("steam", "2", Kind::Square, Online::SteamGridDb, 1).unwrap();
        let icons = db.achievement_icons().unwrap();
        std::fs::create_dir_all(icons.root().join("steam/1")).unwrap();
        std::fs::write(icons.root().join("steam/1/icon.jpg"), PNG).unwrap();
        let first = db.game_id("steam", "1").unwrap().unwrap();
        db.set_rarity(first, None, 1).unwrap();
        assert!(!db.refreshing().unwrap());

        let forgot = db.forget_fetched().unwrap();
        assert_eq!(forgot, Forgot { pictures: 2, rarity: 1 });
        assert!(db.refreshing().unwrap());
        assert!(art.index().is_empty() && !art.missed("steam", "2", Kind::Square, Online::SteamGridDb, 1));
        assert!(!icons.root().exists());
        assert_eq!(db.rarity_asked(first).unwrap(), None);
        // The user's own stay: the games, their play time and marks, the key.
        let snapshot = db.snapshot().unwrap();
        let game = snapshot.games.iter().find(|game| game.source_id == "1").unwrap();
        assert_eq!((game.playtime_minutes, game.favourite, game.last_played), (90, true, 20));
        assert_eq!(snapshot.games.len(), 2);
        assert_eq!(db.setting(crate::art::KEY_SETTING).unwrap().as_deref(), Some("a-made-up-key"));
        assert!(game.square.is_none());

        db.refreshed().unwrap();
        assert!(!db.refreshing().unwrap());
    }

    #[test]
    fn one_games_art_is_forgotten_alone() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("library.db")).unwrap();
        let art = ArtDir::under(dir.path());
        art.save("steam", "1", Kind::Square, PNG).unwrap();
        art.save("steam", "2", Kind::Square, PNG).unwrap();
        assert_eq!(db.forget_art_of("steam", "1").unwrap(), 1);
        assert!(art.find("steam", "1", Kind::Square).is_none() && art.find("steam", "2", Kind::Square).is_some());
        assert!(!db.refreshing().unwrap());
    }
}
