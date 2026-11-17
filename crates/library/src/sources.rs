//! Many sources into one library (docs/sources.md): each source is read in turn, and what each gave is merged into
//! the database under the source's key, so its games are rows `(source, id in that source)` and a game owned in two
//! stores is two games. A source that could not be read is said in the log and passed over, and one whose client is
//! not installed is passed over in silence: either way its games stay as the database last had them, with their
//! Play history and the user's marks, and the other sources load as ever.
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use model::{Library, Source};

use crate::{Database, Synced};

/// What reading the sources gave.
#[derive(Debug, Default)]
pub struct Read {
    /// Each source that was read, by its key, with what it read, in the sources' order.
    pub found: Vec<(&'static str, Library)>,
    /// Each source that could not be read, by its key, with why.
    pub failed: Vec<(&'static str, anyhow::Error)>,
    /// Each source whose client is not on this PC, by its key.
    pub absent: Vec<&'static str>,
}

impl Read {
    /// How many games the sources that were read have between them.
    pub fn games(&self) -> usize {
        self.found.iter().map(|(_, library)| library.games.len()).sum()
    }
}

/// Reads every source in turn, and says in the log what each gave: `steam: 14 games from C:\Steam in 5 ms`, or
/// for one that could not be read `steam: <why>`. One whose client is not installed says nothing.
pub fn read(sources: &[Arc<dyn Source>]) -> Read {
    let mut read = Read::default();
    for source in sources {
        let started = Instant::now();
        match source.read() {
            Ok(Some(found)) => {
                let ms = started.elapsed().as_millis();
                eprintln!("{}: {} games from {} in {ms} ms", source.key(), found.library.games.len(), found.from);
                read.found.push((source.key(), found.library));
            }
            Ok(None) => read.absent.push(source.key()),
            Err(e) => {
                eprintln!("{}: {e:#}", source.key());
                read.failed.push((source.key(), e));
            }
        }
    }
    read
}

impl Database {
    /// Merges what each source was read as into the database, each under its own key (`sync`), at the time `now`.
    /// A source that is not in `read` (not installed, or not read) is left as it was. What was found of each is kept
    /// for Settings > Launchers (`launchers`): read now, not installed, or not readable.
    pub fn sync_all(&mut self, read: &Read, now: u64) -> Result<Synced> {
        self.note_read(read, now)?;
        let mut all = Synced::default();
        for (source, found) in &read.found {
            let synced = self.sync(source, found, now)?;
            all.added += synced.added;
            all.updated += synced.updated;
            all.gone += synced.gone;
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use anyhow::bail;
    use model::{Ask, Found, Game, Start};

    use super::*;

    const NOW: u64 = 1_800_000_000;

    /// What a fake source gives when it is next read.
    #[derive(Clone)]
    enum Gives {
        Absent,
        Fails,
        Games(Vec<Game>),
    }

    struct Fake {
        key: &'static str,
        gives: Mutex<Gives>,
    }

    impl Fake {
        fn new(key: &'static str, gives: Gives) -> Arc<Self> {
            Arc::new(Self { key, gives: Mutex::new(gives) })
        }

        fn give(&self, gives: Gives) {
            *self.gives.lock().unwrap() = gives;
        }
    }

    impl Source for Fake {
        fn key(&self) -> &'static str {
            self.key
        }

        fn name(&self) -> &'static str {
            "Fake"
        }

        fn read(&self) -> Result<Option<Found>> {
            match self.gives.lock().unwrap().clone() {
                Gives::Absent => Ok(None),
                Gives::Fails => bail!("its files are not what was expected"),
                Gives::Games(games) => {
                    let games = games.into_iter().map(|game| Game { source: self.key.into(), ..game }).collect();
                    let library = Library { user: Some(format!("{} player", self.key)), games, ..Library::default() };
                    Ok(Some(Found { library, from: format!("C:\\{}", self.key) }))
                }
            }
        }

        fn start(&self, game: &Game) -> Option<Start> {
            Some(Start::Address(format!("{}://run/{}", self.key, game.source_id)))
        }

        fn ask(&self, _: &Game, _: Ask) -> Option<Start> {
            None
        }
    }

    fn game(id: &str, name: &str, last_played: u64) -> Game {
        Game {
            source_id: id.into(),
            name: name.into(),
            last_played,
            installed: true,
            install_dir: Some(PathBuf::from("games").join(name)),
            ..Game::default()
        }
    }

    fn refresh(db: &mut Database, sources: &[Arc<dyn Source>]) -> Synced {
        db.sync_all(&read(sources), NOW).unwrap()
    }

    fn of(library: &Library) -> Vec<(String, String, String)> {
        let mut games: Vec<_> =
            library.games.iter().map(|g| (g.source.clone(), g.source_id.clone(), g.name.clone())).collect();
        games.sort();
        games
    }

    fn row(source: &str, id: &str, name: &str) -> (String, String, String) {
        (source.into(), id.into(), name.into())
    }

    #[test]
    fn two_sources_are_merged_and_a_game_in_both_is_two_games() {
        let alpha = Fake::new("alpha", Gives::Games(vec![game("1", "Moss", 0), game("2", "Orchard", 0)]));
        let beta = Fake::new("beta", Gives::Games(vec![game("1", "Moss", 0), game("XK-9", "Tern", 0)]));
        let mut db = Database::in_memory().unwrap();
        let got = read(&[alpha.clone() as Arc<dyn Source>, beta.clone()]);
        assert_eq!((got.found.len(), got.failed.len(), got.games()), (2, 0, 4));
        assert_eq!(db.sync_all(&got, NOW).unwrap(), Synced { added: 4, updated: 0, gone: 0 });

        let snapshot = db.snapshot().unwrap();
        let both = [
            row("alpha", "1", "Moss"),
            row("alpha", "2", "Orchard"),
            row("beta", "1", "Moss"),
            row("beta", "XK-9", "Tern"),
        ];
        assert_eq!(of(&snapshot), both);
        let ids: std::collections::HashSet<i64> = snapshot.games.iter().map(|g| g.id).collect();
        assert_eq!(ids.len(), 4, "each a row of its own");
        // The same again changes nothing.
        assert_eq!(refresh(&mut db, &[alpha as Arc<dyn Source>, beta]), Synced::default());
    }

    #[test]
    fn a_source_that_fails_or_is_absent_is_passed_over_and_keeps_its_games() {
        let alpha = Fake::new("alpha", Gives::Games(vec![game("1", "Moss", 100)]));
        let beta = Fake::new("beta", Gives::Games(vec![game("7", "Tern", 0)]));
        let sources: [Arc<dyn Source>; 2] = [alpha.clone(), beta.clone()];
        let mut db = Database::in_memory().unwrap();
        refresh(&mut db, &sources);
        let before = db.snapshot().unwrap();

        // Alpha's files can't be read: beta still loads, and alpha's game is as it was, not gone.
        alpha.give(Gives::Fails);
        beta.give(Gives::Games(vec![game("7", "Tern", 0), game("8", "Wren", 0)]));
        let got = read(&sources);
        assert_eq!(got.failed.iter().map(|(key, _)| *key).collect::<Vec<_>>(), ["alpha"]);
        assert_eq!(format!("{:#}", got.failed[0].1), "its files are not what was expected");
        assert_eq!(db.sync_all(&got, NOW).unwrap(), Synced { added: 1, updated: 0, gone: 0 });
        let after = db.snapshot().unwrap();
        assert_eq!(of(&after), [row("alpha", "1", "Moss"), row("beta", "7", "Tern"), row("beta", "8", "Wren")]);
        assert_eq!(after.games.iter().find(|g| g.name == "Moss"), before.games.iter().find(|g| g.name == "Moss"));

        // Its client uninstalled: nothing is said, and nothing of its games changes.
        alpha.give(Gives::Absent);
        let got = read(&sources);
        assert!(got.failed.is_empty() && got.found.len() == 1);
        assert_eq!(db.sync_all(&got, NOW).unwrap(), Synced::default());
        assert_eq!(db.snapshot().unwrap(), after);
    }

    #[test]
    fn a_game_that_leaves_its_source_keeps_its_history_and_marks() {
        let games = vec![game("1", "Moss", 100), game("2", "Orchard", 0), game("3", "Quill", 0)];
        let alpha = Fake::new("alpha", Gives::Games(games.clone()));
        let sources: [Arc<dyn Source>; 1] = [alpha.clone()];
        let mut db = Database::in_memory().unwrap();
        refresh(&mut db, &sources);
        let id = |db: &Database, name: &str| {
            let snapshot = db.snapshot().unwrap();
            snapshot.games.iter().chain(&snapshot.hidden).find(|g| g.name == name).unwrap().id
        };
        let (moss, orchard, quill) = (id(&db, "Moss"), id(&db, "Orchard"), id(&db, "Quill"));
        let session = db.session_started(moss, NOW).unwrap();
        db.session_ended(session, NOW + 3600).unwrap();
        db.set_favourite(moss, true).unwrap();
        db.set_hidden(orchard, true).unwrap();
        db.set_favourite(quill, true).unwrap();

        // Moss is uninstalled; Orchard and Quill are no longer the user's at all.
        let uninstalled = Game { installed: false, install_dir: None, ..games[0].clone() };
        alpha.give(Gives::Games(vec![uninstalled]));
        assert_eq!(refresh(&mut db, &sources), Synced { added: 0, updated: 1, gone: 2 });
        let kept = db.game(moss).unwrap().unwrap();
        assert!(!kept.installed && kept.favourite);
        assert_eq!((kept.last_played, kept.playtime_minutes, kept.play_count), (NOW + 3600, 60, 1));
        assert_eq!(db.sessions(moss).unwrap().len(), 1);
        let snapshot = db.snapshot().unwrap();
        assert_eq!(of(&snapshot), [row("alpha", "1", "Moss")]);
        assert!(snapshot.hidden.is_empty());
        assert_eq!(snapshot.history.iter().map(|row| row.game).collect::<Vec<_>>(), [moss]);

        // Back in the source, each is the game it was, with what the user did with it.
        alpha.give(Gives::Games(games));
        assert_eq!(refresh(&mut db, &sources), Synced { added: 0, updated: 3, gone: 0 });
        assert_eq!((id(&db, "Moss"), id(&db, "Orchard"), id(&db, "Quill")), (moss, orchard, quill));
        let (orchard, quill) = (db.game(orchard).unwrap().unwrap(), db.game(quill).unwrap().unwrap());
        assert!(orchard.hidden && quill.favourite && db.game(moss).unwrap().unwrap().installed);
    }

    #[test]
    fn the_user_is_the_one_the_source_with_the_most_games_names() {
        let alpha = Fake::new("alpha", Gives::Games(vec![game("1", "Moss", 0)]));
        let beta = Fake::new("beta", Gives::Games(vec![game("1", "Moss", 0), game("2", "Tern", 0)]));
        let mut db = Database::in_memory().unwrap();
        refresh(&mut db, &[alpha.clone() as Arc<dyn Source>, beta.clone()]);
        assert_eq!(db.snapshot().unwrap().user.as_deref(), Some("beta player"));
        beta.give(Gives::Games(Vec::new()));
        refresh(&mut db, &[alpha as Arc<dyn Source>, beta]);
        assert_eq!(db.snapshot().unwrap().user.as_deref(), Some("alpha player"));
    }
}
