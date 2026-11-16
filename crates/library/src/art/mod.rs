//! Art for games a store gives none for (docs/sources.md, "Art"; decision #9). One lookup for every source's games,
//! in this order:
//!
//! 1. What the source gave: the pictures its client keeps on disk (`Game::cover`, `hero`, `header`, as read).
//! 2. Our own cache in the data folder ([`ArtDir`]): a snapshot of the library fills in from it what the source did
//!    not give (`Database::snapshot`).
//! 3. SteamGridDB ([`Service`]), only with the user's own key, off the UI thread ([`Lookup`]), into that cache.
pub mod cache;
pub mod fake;
pub mod lookup;
pub mod steamgriddb;

pub use cache::{ArtDir, Kind};
pub use lookup::{wanted, Event, Log, Lookup, Pace, Summary, Want};
pub use steamgriddb::{Service, SERVICE};

/// The settings row that holds the user's SteamGridDB key. Its value is never logged.
pub const KEY_SETTING: &str = "art.steamgriddb_key";
/// The settings row that says the service refused the key in it: lookups wait for another key.
pub const REFUSED_SETTING: &str = "art.steamgriddb_refused";

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use model::{Game, Library};

    use super::fake::{FakeGame, FakeService, Request, Script};
    use super::*;

    const KEY: &str = "0123456789abcdef-test-key";
    const NOW: u64 = 1_800_000_000;
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really, but enough for the cache";

    fn game(source: &str, id: &str, name: &str) -> Game {
        Game { source: source.into(), source_id: id.into(), name: name.into(), ..Game::default() }
    }

    fn known(id: u64, name: &str, platforms: &[(&str, &str)], kinds: &[Kind]) -> FakeGame {
        FakeGame {
            id,
            name: name.into(),
            platforms: platforms.iter().map(|(p, i)| (p.to_string(), i.to_string())).collect(),
            kinds: kinds.to_vec(),
        }
    }

    fn script(games: Vec<FakeGame>) -> Script {
        Script { games, ..Script::new(KEY, Arc::new(|_, _| PNG.to_vec())) }
    }

    /// The pace of a test: no gap, and a wait the service does not name of a twentieth of a second.
    fn pace() -> Pace {
        Pace {
            workers: 2,
            gap: Duration::ZERO,
            wait: Duration::from_millis(50),
            most_wait: Duration::from_secs(5),
            tries: 5,
        }
    }

    struct Ran {
        summary: Summary,
        log: Vec<String>,
    }

    fn run(dir: &ArtDir, service: &FakeService, key: &str, library: &Library) -> Ran {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let kept = lines.clone();
        let log: Log = Arc::new(move |line| kept.lock().unwrap().push(line.to_string()));
        let service = Service::new(&service.url(), key);
        let summary = Lookup::run_now(dir.clone(), service, wanted(library), NOW, pace(), log);
        let log = lines.lock().unwrap().clone();
        Ran { summary, log }
    }

    fn api(requests: &[Request]) -> Vec<String> {
        requests.iter().filter(|r| !r.path.starts_with("/pictures/")).map(|r| r.path.clone()).collect()
    }

    #[test]
    fn the_stores_art_wins_and_ours_fills_only_what_it_lacks() {
        let mut steam = game("steam", "620", "Portal 2");
        steam.cover = Some("C:/Steam/librarycache/620/cover.jpg".into());
        let library = Library { games: vec![steam], ..Library::default() };
        assert_eq!(wanted(&library)[0].kinds, [Kind::Hero, Kind::Header]);

        let fake = FakeService::start(0, script(vec![known(5, "Portal 2", &[("steam", "620")], &Kind::ALL)])).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run(&art, &fake, KEY, &library);
        assert_eq!((ran.summary.looked_up, ran.summary.found), (1, 1));
        // Never the cover: the store has one.
        assert!(api(&fake.requests()).iter().all(|path| !path.contains("600x900")), "{:?}", fake.requests());
        assert!(art.find("steam", "620", Kind::Cover).is_none());
        assert!(art.find("steam", "620", Kind::Hero).is_some() && art.find("steam", "620", Kind::Header).is_some());

        // And the snapshot keeps the store's cover, with ours for the rest.
        let mut db = crate::Database::in_memory().unwrap();
        db.set_art_dir(Some(art.clone()));
        db.sync("steam", &library, NOW).unwrap();
        // A picture of ours that the store also has is not shown in place of the store's.
        art.save("steam", "620", Kind::Cover, PNG).unwrap();
        let shown = db.snapshot().unwrap().games.remove(0);
        assert_eq!(shown.cover, library.games[0].cover);
        assert_eq!(shown.hero, art.find("steam", "620", Kind::Hero));
        assert_eq!(shown.header, art.find("steam", "620", Kind::Header));
    }

    #[test]
    fn a_game_by_its_store_id_where_the_service_knows_the_store_else_by_its_name() {
        let fake = FakeService::start(
            0,
            script(vec![
                known(7, "Moss Garden", &[("egs", "MossApp")], &Kind::ALL),
                known(8, "Tern Island", &[], &[Kind::Cover]),
            ]),
        )
        .unwrap();
        let library = Library {
            games: vec![game("epic", "MossApp", "Moss"), game("ea", "Origin.OFR.1", "Tern Island")],
            ..Library::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run(&art, &fake, KEY, &library);
        assert_eq!((ran.summary.looked_up, ran.summary.found, ran.summary.none), (2, 2, 0));
        let asked = api(&fake.requests());
        assert!(asked.contains(&"/games/egs/MossApp".to_string()));
        assert!(asked.contains(&"/search/autocomplete/Tern%20Island".to_string()));
        assert!(!asked.iter().any(|path| path.contains("autocomplete/Moss")), "found by its id: {asked:?}");
        // Tern Island has a cover and nothing else: those two are remembered as missed.
        assert!(art.find("ea", "Origin.OFR.1", Kind::Cover).is_some());
        assert!(
            art.missed("ea", "Origin.OFR.1", Kind::Hero, NOW) && art.missed("ea", "Origin.OFR.1", Kind::Header, NOW)
        );
    }

    #[test]
    fn only_a_game_of_the_same_name_is_taken() {
        let fake = FakeService::start(
            0,
            script(vec![known(1, "Hades II", &[], &Kind::ALL), known(2, "Hades' Star", &[], &Kind::ALL)]),
        )
        .unwrap();
        let library = Library { games: vec![game("ubisoft", "4", "Hades")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run(&art, &fake, KEY, &library);
        assert_eq!((ran.summary.found, ran.summary.none), (0, 1));
        assert!(art.index().is_empty(), "no other game's art");
        assert!(api(&fake.requests()).iter().all(|path| !path.starts_with("/grids")));
    }

    #[test]
    fn what_the_cache_has_and_what_was_missed_lately_are_not_asked_again() {
        let fake = FakeService::start(0, script(vec![known(3, "Wren", &[("gog", "11")], &[Kind::Cover])])).unwrap();
        let library = Library { games: vec![game("gog", "11", "Wren")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let first = run(&art, &fake, KEY, &library);
        assert_eq!((first.summary.looked_up, first.summary.found), (1, 1));
        let asked = fake.requests().len();
        assert!(asked > 0);

        // The cover is ours now, and the hero and header were missed: a second start asks nothing.
        let second = run(&art, &fake, KEY, &library);
        assert_eq!(fake.requests().len(), asked, "no request");
        assert_eq!(second.summary, Summary { millis: second.summary.millis, ..Summary::default() });
        assert_eq!(second.log.len(), 1);
        assert!(second.log[0].starts_with("art: 0 looked up, 0 found, 0 none, in "), "{:?}", second.log);

        // A week on, the misses are asked about again.
        let lines = Arc::new(Mutex::new(Vec::new()));
        let kept = lines.clone();
        let log: Log = Arc::new(move |line| kept.lock().unwrap().push(line.to_string()));
        let later = NOW + 8 * 86_400;
        Lookup::run_now(art.clone(), Service::new(&fake.url(), KEY), wanted(&library), later, pace(), log);
        assert!(fake.requests().len() > asked);
    }

    #[test]
    fn a_refused_key_stops_every_lookup_and_is_told_once() {
        let games: Vec<FakeGame> =
            (0..6).map(|i| known(i, &format!("Game {i}"), &[("steam", &i.to_string())], &Kind::ALL)).collect();
        let fake = FakeService::start(0, script(games)).unwrap();
        let library = Library {
            games: (0..6).map(|i| game("steam", &i.to_string(), &format!("Game {i}"))).collect(),
            ..Library::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let events = Arc::new(Mutex::new(Vec::new()));
        let kept = events.clone();
        let (done, finished) = std::sync::mpsc::channel();
        let lookup = Lookup::start(
            art.clone(),
            Service::new(&fake.url(), "wrong-key"),
            wanted(&library),
            NOW,
            pace(),
            Arc::new(|_| {}),
            move |event| {
                let over = matches!(event, Event::Done(_));
                kept.lock().unwrap().push(event);
                if over {
                    let _ = done.send(());
                }
            },
        );
        finished.recv_timeout(Duration::from_secs(10)).expect("the run ends");
        drop(lookup);
        let events = events.lock().unwrap().clone();
        assert_eq!(events.iter().filter(|e| **e == Event::KeyRefused).count(), 1);
        // One request per worker at most: none after the refusal.
        assert!(fake.requests().len() <= pace().workers, "{:?}", fake.requests());
        // Nothing is remembered as missed: another key looks again.
        assert!(art.index().is_empty() && !art.missed("steam", "0", Kind::Cover, NOW));
    }

    #[test]
    fn when_asked_to_wait_it_waits_and_carries_on() {
        let mut script = script(vec![known(9, "Quill", &[("steam", "9")], &[Kind::Cover])]);
        script.busy = 2;
        let fake = FakeService::start(0, script).unwrap();
        let library = Library { games: vec![game("steam", "9", "Quill")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let started = Instant::now();
        let ran = run(&art, &fake, KEY, &library);
        // Retry-After: 1, and the second time no less than the doubled wait: 1 s each.
        assert!(started.elapsed() >= Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(ran.summary.found, 1);
        let asked = api(&fake.requests());
        assert_eq!(asked.iter().filter(|path| *path == "/games/steam/9").count(), 3, "{asked:?}");
        assert!(ran.log.iter().any(|line| line == "art: SteamGridDB asked to wait; waiting 1 s"), "{:?}", ran.log);
    }

    #[test]
    fn the_key_goes_only_to_the_api_and_never_into_the_log() {
        let fake = FakeService::start(0, script(vec![known(4, "Orchard", &[("gog", "4")], &Kind::ALL)])).unwrap();
        let library =
            Library { games: vec![game("gog", "4", "Orchard"), game("gog", "5", "Nowhere")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run(&art, &fake, KEY, &library);
        let requests = fake.requests();
        for request in &requests {
            if request.path.starts_with("/pictures/") {
                assert_eq!(request.authorization, None, "no key with a picture");
            } else {
                assert_eq!(request.authorization.as_deref(), Some(format!("Bearer {KEY}").as_str()));
                assert!(!request.path.contains(KEY));
            }
        }
        assert!(requests.iter().any(|r| r.path.starts_with("/pictures/")));
        assert!(!ran.log.is_empty());
        for line in &ran.log {
            assert!(!line.contains(KEY) && !line.contains("Orchard") && !line.contains("Nowhere"), "{line}");
        }
        // A refused key is said without itself.
        let refused = run(&ArtDir::under(&dir.path().join("other")), &fake, "my-secret-wrong-key", &library);
        assert!(refused.log.iter().any(|line| line.contains("refused")));
        assert!(refused.log.iter().all(|line| !line.contains("my-secret-wrong-key")));
    }
}
