//! The user's own choice of a game's pictures, made in the art picker (docs/spec/art-picker.md, #213): one row per
//! game and kind, keyed by the game's store and its id there, so it follows the game whatever its launcher does to
//! it. A SteamGridDB picture chosen is kept in a folder of its own beside the art cache, `art-chosen/` (as
//! `art/`: `<source>/<source_id>/<kind>.<ext>`), which neither Refresh all art, a game's Refresh art nor Clear
//! downloaded art touches. No row is "Automatic", the Dashboard's own pick.
//!
//! A snapshot shows a choice over everything else: over the store's own files, which otherwise win.
use std::path::PathBuf;

use anyhow::{bail, Result};
use model::art_picker::{Chosen, Tab};
use model::Game;
use rusqlite::params;

use crate::art::{ArtDir, Kind};
use crate::Database;

pub const MIGRATION: &str = "CREATE TABLE art_choices (
    -- The game, by its store and the store's id for it, as games are.
    source TEXT NOT NULL,
    source_id TEXT NOT NULL,
    -- square, cover or background.
    kind TEXT NOT NULL,
    -- 'store' (the store's own picture), or 'steamgriddb:<id>' (that picture, in art-chosen/).
    chosen TEXT NOT NULL,
    PRIMARY KEY (source, source_id, kind)
) WITHOUT ROWID;";

/// The chosen pictures' folder, beside the art cache's.
pub const FOLDER: &str = "art-chosen";

/// A tab's kind as the art caches name it.
pub fn kind(tab: Tab) -> Kind {
    match tab {
        Tab::Square => Kind::Square,
        Tab::Cover => Kind::Cover,
        Tab::Background => Kind::Hero,
    }
}

fn text(chosen: &Chosen) -> String {
    match chosen {
        Chosen::Store => "store".into(),
        Chosen::Picture(id) => format!("steamgriddb:{id}"),
    }
}

fn parse(text: &str) -> Option<Chosen> {
    match text {
        "store" => Some(Chosen::Store),
        _ => text.strip_prefix("steamgriddb:")?.parse().ok().map(Chosen::Picture),
    }
}

impl Database {
    /// Where the chosen pictures are kept: beside the art cache, in a folder of its own. None without a cache.
    pub fn chosen_dir(&self) -> Option<ArtDir> {
        let art = self.art_dir()?;
        Some(ArtDir::new(art.root().parent()?.join(FOLDER)))
    }

    /// Keeps the user's choice of a game's picture of a kind: `None` is Automatic, which forgets the choice. A
    /// SteamGridDB picture comes with its bytes (`picture`), kept in the chosen pictures' folder; refused if they
    /// are not a PNG, JPEG or WebP, and nothing is changed then.
    pub fn choose_art(
        &self,
        source: &str,
        source_id: &str,
        tab: Tab,
        chosen: Option<&Chosen>,
        picture: Option<&[u8]>,
    ) -> Result<()> {
        let dir = self.chosen_dir();
        match (chosen, picture, &dir) {
            (Some(Chosen::Picture(_)), Some(bytes), Some(dir)) => {
                dir.save(source, source_id, kind(tab), bytes)?;
            }
            (Some(Chosen::Picture(_)), _, _) => bail!("a chosen picture needs its bytes and a folder to keep them in"),
            (_, _, Some(dir)) => dir.remove(source, source_id, kind(tab))?,
            (_, _, None) => {}
        }
        match chosen {
            Some(chosen) => self.conn.execute(
                "INSERT INTO art_choices (source, source_id, kind, chosen) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (source, source_id, kind) DO UPDATE SET chosen = ?4",
                params![source, source_id, tab.key(), text(chosen)],
            )?,
            None => self.conn.execute(
                "DELETE FROM art_choices WHERE source = ?1 AND source_id = ?2 AND kind = ?3",
                params![source, source_id, tab.key()],
            )?,
        };
        Ok(())
    }

    /// Every choice kept, as `(source, source_id, tab, chosen)`.
    pub fn art_choices(&self) -> Result<Vec<(String, String, Tab, Chosen)>> {
        let mut rows = self.conn.prepare("SELECT source, source_id, kind, chosen FROM art_choices")?;
        let rows = rows.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (source, source_id, kind, chosen) = row?;
            if let (Some(tab), Some(chosen)) = (Tab::from_key(&kind), parse(&chosen)) {
                out.push((source, source_id, tab, chosen));
            }
        }
        Ok(out)
    }

    /// Puts the user's choices on the snapshot's games: each game's `art.chosen`, and the picture shown in its
    /// place. A SteamGridDB picture whose file has gone, or the store's own where the store has none now, leaves the
    /// Dashboard's own pick.
    pub(crate) fn apply_choices(&self, games: &mut [Game]) -> Result<()> {
        let choices = self.art_choices()?;
        if choices.is_empty() {
            return Ok(());
        }
        let dir = self.chosen_dir();
        for game in games.iter_mut() {
            for (_, _, tab, chosen) in
                choices.iter().filter(|(source, id, _, _)| *source == game.source && *id == game.source_id)
            {
                let picture: Option<PathBuf> = match chosen {
                    Chosen::Store => game.art.store(*tab).cloned(),
                    Chosen::Picture(_) => {
                        dir.as_ref().and_then(|dir| dir.find(&game.source, &game.source_id, kind(*tab)))
                    }
                };
                game.art.chosen[tab.index()] = Some(chosen.clone());
                if let Some(picture) = picture {
                    let field = match tab {
                        Tab::Square => &mut game.square,
                        Tab::Cover => &mut game.cover,
                        Tab::Background => &mut game.hero,
                    };
                    *field = Some(picture);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use model::Library;

    use super::*;

    const NOW: u64 = 1_800_000_000;
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nchosen";
    const OTHER: &[u8] = b"\x89PNG\r\n\x1a\nfetched";

    fn game() -> Game {
        Game {
            source: "steam".into(),
            source_id: "70".into(),
            name: "Made-up Game".into(),
            cover: Some("C:/Steam/librarycache/70/library_600x900.jpg".into()),
            ..Game::default()
        }
    }

    #[test]
    fn a_choice_shows_over_the_stores_files_and_outlives_every_refresh_of_art() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open(&dir.path().join("library.db")).unwrap();
        let library = Library { games: vec![game()], ..Library::default() };
        db.sync("steam", &library, NOW).unwrap();
        let art = db.art_dir().unwrap().clone();
        art.save("steam", "70", Kind::Square, OTHER).unwrap();
        let shown = db.snapshot().unwrap().games.remove(0);
        assert_eq!(shown.art.store(Tab::Cover), library.games[0].cover.as_ref(), "the store's own, to choose");
        assert_eq!(shown.art.store(Tab::Square), None, "the cache's is not the store's");

        db.choose_art("steam", "70", Tab::Cover, Some(&Chosen::Picture(7001)), Some(PNG)).unwrap();
        db.choose_art("steam", "70", Tab::Square, Some(&Chosen::Picture(7002)), Some(PNG)).unwrap();
        let chosen = db.chosen_dir().unwrap();
        let check = |db: &Database| {
            let shown = db.snapshot().unwrap().games.remove(0);
            assert_eq!(shown.cover, chosen.find("steam", "70", Kind::Cover), "over the store's own file");
            assert_eq!(shown.square, chosen.find("steam", "70", Kind::Square));
            assert_eq!(shown.art.chosen(Tab::Cover), Some(&Chosen::Picture(7001)));
            assert_eq!(std::fs::read(shown.square.unwrap()).unwrap(), PNG);
        };
        check(&db);
        // Refresh all art, a game's Refresh art, Clear downloaded art, and a sync of the store: still the user's.
        db.forget_fetched().unwrap();
        check(&db);
        art.save("steam", "70", Kind::Square, OTHER).unwrap();
        db.forget_art_of("steam", "70").unwrap();
        art.clear().unwrap();
        db.sync("steam", &library, NOW + 60).unwrap();
        check(&db);

        // The store's own, then Automatic: the Dashboard's pick again, the chosen file gone.
        db.choose_art("steam", "70", Tab::Cover, Some(&Chosen::Store), None).unwrap();
        let shown = db.snapshot().unwrap().games.remove(0);
        assert_eq!(shown.cover, library.games[0].cover);
        assert_eq!(shown.art.chosen(Tab::Cover), Some(&Chosen::Store));
        assert!(chosen.find("steam", "70", Kind::Cover).is_none());
        db.choose_art("steam", "70", Tab::Square, None, None).unwrap();
        let shown = db.snapshot().unwrap().games.remove(0);
        assert_eq!((shown.square, shown.art.chosen(Tab::Square)), (None, None));
        assert_eq!(db.art_choices().unwrap().len(), 1);
        // A picture with no bytes, or bytes that are no picture, is refused and changes nothing.
        assert!(db.choose_art("steam", "70", Tab::Square, Some(&Chosen::Picture(1)), None).is_err());
        assert!(db.choose_art("steam", "70", Tab::Square, Some(&Chosen::Picture(1)), Some(b"no")).is_err());
        assert_eq!(db.art_choices().unwrap().len(), 1);
    }
}
