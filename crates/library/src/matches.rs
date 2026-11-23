//! The user's matches of a game to the online sources' own entries for it, made on the Match game page
//! (docs/spec/match-game.md): one row per game and source, keyed by the game's store and its id there as the art
//! choices are (`choices`), so a match follows the game whatever its launcher does to it. No row is Automatic: the
//! game found by its name, as before.
//!
//! A snapshot puts each game's matches on it (`Game::matches`); the art lookup and the art picker read them there.
use anyhow::Result;
use model::art_match::Match;
use model::{ArtSource, Game};
use rusqlite::params;

use crate::Database;

pub const MIGRATION: &str = "CREATE TABLE art_matches (
    -- The game, by its store and the store's id for it, as games are.
    source TEXT NOT NULL,
    source_id TEXT NOT NULL,
    -- The online source: steam, xbox, playstation or steamgriddb (model::ArtSource::key).
    online TEXT NOT NULL,
    -- The game's id there, and its name there.
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (source, source_id, online)
) WITHOUT ROWID;";

impl Database {
    /// Keeps the user's match of a game on an online source; `None` is Automatic, which forgets it.
    pub fn match_game(&self, source: &str, source_id: &str, online: ArtSource, matched: Option<&Match>) -> Result<()> {
        match matched {
            Some(m) => self.conn.execute(
                "INSERT INTO art_matches (source, source_id, online, id, name) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (source, source_id, online) DO UPDATE SET id = excluded.id, name = excluded.name",
                params![source, source_id, online.key(), m.id, m.name],
            )?,
            None => self.conn.execute(
                "DELETE FROM art_matches WHERE source = ?1 AND source_id = ?2 AND online = ?3",
                params![source, source_id, online.key()],
            )?,
        };
        Ok(())
    }

    /// Every match kept, as `(source, source_id, match)`.
    pub fn art_matches(&self) -> Result<Vec<(String, String, Match)>> {
        let mut rows = self.conn.prepare("SELECT source, source_id, online, id, name FROM art_matches")?;
        let rows = rows.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (source, source_id, online, id, name) = row?;
            if let Some(online) = ArtSource::from_key(&online) {
                out.push((source, source_id, Match { source: online, id, name }));
            }
        }
        Ok(out)
    }

    /// Puts the user's matches on the snapshot's games, in the sources' own order.
    pub(crate) fn apply_matches(&self, games: &mut [Game]) -> Result<()> {
        let mut matches = self.art_matches()?;
        if matches.is_empty() {
            return Ok(());
        }
        matches.sort_by_key(|(_, _, m)| ArtSource::ALL.iter().position(|s| *s == m.source));
        for game in games.iter_mut() {
            game.matches = matches
                .iter()
                .filter(|(source, id, _)| *source == game.source && *id == game.source_id)
                .map(|(_, _, m)| m.clone())
                .collect();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use model::Library;

    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn game(id: &str) -> Game {
        Game {
            source: "battlenet".into(),
            source_id: id.into(),
            name: "Made-up Game: Forever".into(),
            ..Game::default()
        }
    }

    #[test]
    fn a_match_is_on_the_game_in_every_snapshot_until_it_is_set_back_to_automatic() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open(&dir.path().join("library.db")).unwrap();
        let library = Library { games: vec![game("wow"), game("other")], ..Library::default() };
        db.sync("battlenet", &library, NOW).unwrap();
        assert!(db.snapshot().unwrap().games.iter().all(|g| g.matches.is_empty()));

        let grid = Match { source: ArtSource::SteamGridDb, id: "5000".into(), name: "Made-up Game".into() };
        let steam = Match { source: ArtSource::Steam, id: "70".into(), name: "Made-up Game Classic".into() };
        db.match_game("battlenet", "wow", ArtSource::SteamGridDb, Some(&grid)).unwrap();
        db.match_game("battlenet", "wow", ArtSource::Steam, Some(&steam)).unwrap();
        let shown = |db: &Database| {
            let snapshot = db.snapshot().unwrap();
            let of = |id: &str| snapshot.games.iter().find(|g| g.source_id == id).unwrap().matches.clone();
            (of("wow"), of("other"))
        };
        assert_eq!(shown(&db), (vec![steam.clone(), grid.clone()], vec![]), "the sources' order, the one game's");

        // Matched again: the new one in its place. Kept across a sync, and across a reopen.
        let again = Match { id: "5001".into(), ..grid.clone() };
        db.match_game("battlenet", "wow", ArtSource::SteamGridDb, Some(&again)).unwrap();
        db.sync("battlenet", &library, NOW + 60).unwrap();
        drop(db);
        let db = Database::open(&dir.path().join("library.db")).unwrap();
        assert_eq!(shown(&db).0, vec![steam.clone(), again]);
        assert_eq!(db.snapshot().unwrap().games[0].matched(ArtSource::Steam), Some(&steam));

        // Automatic: forgotten.
        db.match_game("battlenet", "wow", ArtSource::SteamGridDb, None).unwrap();
        assert_eq!(shown(&db).0, vec![steam]);
    }
}
