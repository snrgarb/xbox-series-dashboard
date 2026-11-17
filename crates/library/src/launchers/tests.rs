use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::bail;
use model::achievement::Achievement;
use model::{Ask, Collection, Found as Read, Game, Start};

use super::*;
use crate::sources::read;

const NOW: u64 = 1_800_000_000;

/// What a fake launcher gives when it is next read.
#[derive(Clone)]
enum Gives {
    Absent,
    Fails,
    Games(Vec<Game>, Vec<Collection>),
}

struct Fake {
    key: &'static str,
    gives: Mutex<Gives>,
    reads: Mutex<usize>,
}

impl Fake {
    fn new(key: &'static str, gives: Gives) -> Arc<Self> {
        Arc::new(Self { key, gives: Mutex::new(gives), reads: Mutex::new(0) })
    }

    fn give(&self, gives: Gives) {
        *self.gives.lock().unwrap() = gives;
    }

    fn reads(&self) -> usize {
        *self.reads.lock().unwrap()
    }
}

impl Source for Fake {
    fn key(&self) -> &'static str {
        self.key
    }

    fn name(&self) -> &'static str {
        "Fake"
    }

    fn read(&self) -> Result<Option<Read>> {
        *self.reads.lock().unwrap() += 1;
        match self.gives.lock().unwrap().clone() {
            Gives::Absent => Ok(None),
            Gives::Fails => bail!("its files are not what was expected"),
            Gives::Games(games, collections) => {
                let games = games.into_iter().map(|game| Game { source: self.key.into(), ..game }).collect();
                let library = Library {
                    user: Some(format!("{} player", self.key)),
                    games,
                    collections: Some(collections),
                    ..Library::default()
                };
                Ok(Some(Read { library, from: format!("C:\\{}", self.key) }))
            }
        }
    }

    fn start(&self, game: &Game) -> Option<Start> {
        Some(Start::Address(format!("{}://run/{}", self.key, game.source_id)))
    }

    fn ask(&self, _: &Game, _: Ask) -> Option<Start> {
        None
    }

    /// It keeps word of its installs, so that one of its games joins the Queue: none under way.
    fn installs(&self) -> Option<Vec<model::Install>> {
        Some(Vec::new())
    }
}

fn game(id: &str, name: &str, last_played: u64, playtime_minutes: u32) -> Game {
    Game {
        source_id: id.into(),
        name: name.into(),
        last_played,
        playtime_minutes,
        installed: true,
        install_dir: Some(PathBuf::from("games").join(name)),
        ..Game::default()
    }
}

fn names(games: &[Game]) -> Vec<&str> {
    let mut names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
    names.sort();
    names
}

/// Steam with three games, one in a collection; Epic with two, one played. Both read into a database.
fn two_launchers() -> (Database, Arc<Fake>, Arc<Fake>) {
    let couch = Collection { source_id: "uc-couch".into(), name: "Couch co-op".into(), games: vec!["2".into()] };
    let steam = Fake::new(
        "steam",
        Gives::Games(
            vec![game("1", "Ashfall", NOW - 100, 30), game("2", "Brass", 0, 0), game("3", "Copper", 0, 0)],
            vec![couch],
        ),
    );
    let wren = Game { installed: false, install_dir: None, ..game("B", "Wren", 0, 0) };
    let epic = Fake::new("epic", Gives::Games(vec![game("A", "Tern", NOW - 50, 600), wren], vec![]));
    let mut db = Database::in_memory().unwrap();
    let sources: [Arc<dyn Source>; 2] = [steam.clone(), epic.clone()];
    db.sync_all(&read(&sources), NOW).unwrap();
    (db, steam, epic)
}

fn id(db: &Database, source: &str, source_id: &str) -> i64 {
    db.conn
        .query_row("SELECT id FROM games WHERE source = ?1 AND source_id = ?2", [source, source_id], |row| row.get(0))
        .unwrap()
}

#[test]
fn a_launcher_switched_off_is_in_nothing_the_snapshot_shows_and_comes_back_as_it_was() {
    let (mut db, _, epic) = two_launchers();
    let (tern, wren) = (id(&db, "epic", "A"), id(&db, "epic", "B"));
    // What the user did with Epic's games: a favourite, a hidden game, a Play session, an achievement, a Group of
    // their own that picks every game, and one of Epic's installing.
    db.set_favourite(tern, true).unwrap();
    db.set_hidden(wren, true).unwrap();
    let session = db.session_started(tern, NOW).unwrap();
    db.session_ended(session, NOW + 3600).unwrap();
    let unlocked = Achievement {
        key: "WIN".into(),
        name: "Won".into(),
        unlocked: Some(NOW),
        rarity: Some(3.0),
        ..Achievement::default()
    };
    db.put_achievements(tern, &[unlocked]).unwrap();
    db.change_filter(&model::FilterChange::Add { name: "Everything".into() }).unwrap();
    let mut queue = crate::queue::Follower::new(vec![epic as Arc<dyn Source>]);
    assert!(queue.asked_install(&mut db, wren, NOW).unwrap(), "joins the Queue");
    assert!(queue.busy(&db).unwrap());
    let before = db.snapshot_at(NOW + 3600).unwrap();
    assert_eq!(names(&before.games), ["Ashfall", "Brass", "Copper", "Tern"]);
    assert_eq!(before.queue.entries.len(), 1);
    assert!(before.achievements.totals.is_some());
    assert_eq!(before.user.as_deref(), Some("steam player"), "the most games; of two with as many, the first by name");

    assert_eq!(db.set_launcher("epic", false).unwrap(), 2, "its games, the hidden one too");
    assert_eq!(db.launchers_off().unwrap(), HashSet::from(["epic".to_string()]));
    let off = db.snapshot_at(NOW + 3600).unwrap();
    assert_eq!(names(&off.games), ["Ashfall", "Brass", "Copper"]);
    assert!(off.hidden.is_empty(), "nor among the hidden");
    assert!(off.history.iter().all(|row| row.game != tern), "nor in Play history");
    assert!(db.recent(10).unwrap().iter().all(|g| g.source == "steam"), "nor in the Recent row");
    assert!(off.queue.entries.is_empty(), "nor in the Queue");
    // The Queue's follower neither asks the launcher nor drops its game: the row waits for it to be on again.
    assert!(!queue.busy(&db).unwrap());
    let polled = queue.poll(&mut db, NOW + 3600).unwrap();
    assert!(polled.changes.is_empty() && !polled.library_changed);
    assert!(off.achievements.totals.is_none() && off.achievements.recent.is_empty(), "nor in the totals");
    for filter in &off.filters {
        assert!(filter.games.iter().all(|&g| g != tern && g != wren), "nor in {}", filter.name);
    }
    let everything = off.filters.iter().find(|f| f.name == "Everything").unwrap();
    assert_eq!(everything.games.len(), 3);
    assert_eq!(db.launchers().unwrap().get("epic").map(|l| (l.on, l.games)), Some((false, 2)));

    // Steam off too: its collection's Group is empty, and so no Group on Home; the user is nobody's.
    db.set_launcher("steam", false).unwrap();
    let none = db.snapshot_at(NOW + 3600).unwrap();
    assert!(none.games.is_empty() && none.groups().next().is_none());
    assert_eq!(none.user, None);
    db.set_launcher("steam", true).unwrap();

    // Back on: every game as it was, with what the user did with it.
    db.set_launcher("epic", true).unwrap();
    let on = db.snapshot_at(NOW + 3600).unwrap();
    assert_eq!(on, before);
    let tern = on.game(tern).unwrap();
    assert!(tern.favourite && tern.playtime_minutes == 600 + 60 && tern.play_count == 1);
    assert!(on.game(wren).unwrap().hidden);
}

#[test]
fn a_launcher_switched_off_is_not_read_and_the_others_are() {
    let (mut db, steam, epic) = two_launchers();
    let sources: Vec<Arc<dyn Source>> = vec![steam.clone(), epic.clone()];
    db.set_launcher("epic", false).unwrap();
    let on = switched_on(&sources, &db.launchers_off().unwrap());
    assert_eq!(on.iter().map(|s| s.key()).collect::<Vec<_>>(), ["steam"]);
    // Epic lists a game more meanwhile: nothing of it comes in while it is off, and its games are not gone.
    epic.give(Gives::Games(vec![game("A", "Tern", NOW, 900), game("B", "Wren", 0, 0), game("C", "Yew", 0, 0)], vec![]));
    let reads = epic.reads();
    db.sync_all(&read(&on), NOW + 10).unwrap();
    assert_eq!(epic.reads(), reads);
    assert_eq!(db.launchers().unwrap().get("epic").unwrap().games, 2);
    // On again and read: the new game, Wren installed meanwhile, and Tern's play time.
    db.set_launcher("epic", true).unwrap();
    let before = db.snapshot().unwrap();
    db.sync_all(&read(&switched_on(&sources, &db.launchers_off().unwrap())), NOW + 20).unwrap();
    let changes = Changes::between(&before, &db.snapshot().unwrap());
    assert_eq!(changes, Changes { new: 1, installed: 1, played: 1, ..Changes::default() });
    assert_eq!(changes.to_string(), "1 new game, 1 installed, play time for 1");
}

#[test]
fn what_each_read_found_is_kept_for_settings() {
    let steam = Fake::new("steam", Gives::Games(vec![game("1", "Ashfall", 0, 0)], vec![]));
    let gog = Fake::new("gog", Gives::Absent);
    let ea = Fake::new("ea", Gives::Fails);
    let sources: [Arc<dyn Source>; 3] = [steam.clone(), gog.clone(), ea.clone()];
    let mut db = Database::in_memory().unwrap();
    let launchers = db.launchers().unwrap();
    assert_eq!(launchers.list.iter().map(|l| l.key.as_str()).collect::<Vec<_>>(), launcher::ALL);
    assert!(launchers.list.iter().all(|l| l.on && l.found == Found::NotYet && l.read.is_none() && l.games == 0));

    let got = read(&sources);
    assert_eq!(got.absent, ["gog"]);
    db.sync_all(&got, NOW).unwrap();
    let launchers = db.launchers().unwrap();
    let of = |key: &str| launchers.get(key).cloned().unwrap();
    assert_eq!(
        of("steam"),
        Launcher { key: "steam".into(), on: true, found: Found::Installed, read: Some(NOW), games: 1 }
    );
    assert_eq!((of("gog").found, of("gog").read), (Found::NotInstalled, None));
    assert!(!of("gog").switchable(), "not on this PC, with no games: no switch");
    assert_eq!((of("ea").found, of("ea").read), (Found::Failed, None));
    assert_eq!(of("epic").found, Found::NotYet, "not among the sources read");

    // Steam's client uninstalled later: its game is kept, it says so, and it can still be switched off.
    steam.give(Gives::Absent);
    db.sync_all(&read(&sources), NOW + 60).unwrap();
    let steam = db.launchers().unwrap().get("steam").cloned().unwrap();
    assert_eq!((steam.found, steam.read, steam.games), (Found::NotInstalled, Some(NOW), 1));
    assert!(steam.switchable());
}

#[test]
fn an_update_says_what_it_changed_in_a_few_words() {
    let steam = Fake::new("steam", Gives::Games(vec![game("1", "Ashfall", 10, 5), game("2", "Brass", 0, 0)], vec![]));
    let sources: [Arc<dyn Source>; 1] = [steam.clone()];
    let mut db = Database::in_memory().unwrap();
    db.sync_all(&read(&sources), NOW).unwrap();
    let before = db.snapshot().unwrap();
    assert_eq!(Changes::between(&before, &before).to_string(), "nothing new");

    let uninstalled = Game { installed: false, install_dir: None, ..game("1", "Ashfall", 20, 5) };
    let couch = Collection { source_id: "uc-1".into(), name: "Couch".into(), games: vec!["3".into()] };
    steam.give(Gives::Games(vec![uninstalled, game("3", "Copper", 0, 0), game("4", "Dune", 0, 0)], vec![couch]));
    db.sync_all(&read(&sources), NOW + 60).unwrap();
    let changes = Changes::between(&before, &db.snapshot().unwrap());
    assert_eq!(changes, Changes { new: 2, gone: 1, uninstalled: 1, played: 1, groups: true, installed: 0 });
    assert_eq!(changes.to_string(), "2 new games, 1 game gone, 1 uninstalled, play time for 1, Groups changed");
}

#[test]
fn an_off_value_written_by_hand_is_read_as_off() {
    let (db, _, _) = two_launchers();
    db.set_setting("launcher.epic", " Off ").unwrap();
    assert_eq!(names(&db.snapshot().unwrap().games), ["Ashfall", "Brass", "Copper"]);
    db.set_setting("launcher.epic", "yes").unwrap();
    assert_eq!(db.snapshot().unwrap().games.len(), 5);
    // Not a launcher's row: what was read of it.
    db.set_setting("launcher.epic.found", "false").unwrap();
    assert!(db.launchers_off().unwrap().is_empty());
}
