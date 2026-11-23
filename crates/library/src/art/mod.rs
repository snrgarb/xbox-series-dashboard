//! Art for games a store gives none for (docs/sources.md, "Art"; decisions #9 and #110). One lookup for every
//! source's games and every kind of picture (cover, hero, header, square), in this order:
//!
//! 1. What the source gave: the pictures its client keeps on disk (`Game::cover`, `hero`, `header`, `square`, as
//!    read).
//! 2. Our own cache in the data folder ([`ArtDir`]): a snapshot of the library fills in from it what the source did
//!    not give (`Database::snapshot`).
//! 3. The game's own store online ([`Store::Own`]: GOG's catalogue, a client's named addresses), no key.
//! 4. The online sources in the user's order (`model::ArtOrder`, Settings > Art), by default Playnite's: Steam's store,
//!    Xbox (the Microsoft Store's catalogue), the PlayStation Store ([`StoreArt`], no key), then SteamGridDB
//!    ([`Service`], only with the user's own key). Each kind of picture falls through them apart, until one has it;
//!    a game that is not a store's own is found there by its name ([`names`]: the same game or nothing).
//!
//! 3 and 4 only with Settings' Download art online on, off the UI thread ([`Lookup`]), into that cache.
pub mod cache;
pub mod fake;
pub mod lookup;
pub mod names;
pub mod picker;
pub mod steamgriddb;
pub mod store_online;

pub use cache::{ArtDir, Kind, Online};
pub use lookup::{wanted, Event, Log, Lookup, Pace, Pictures, Services, Summary, Want};
pub use picker::{download_full, Picker};
pub use steamgriddb::{Service, SERVICE};
pub use store_online::{Hosts, Store, StoreArt};

/// The settings row that holds the user's SteamGridDB key. Its value is never logged.
pub const KEY_SETTING: &str = "art.steamgriddb_key";
/// The settings row that says the service refused the key in it: lookups wait for another key.
pub const REFUSED_SETTING: &str = "art.steamgriddb_refused";
/// The settings row of Settings' Download art online: `"false"` turns all art online off (the stores' own and
/// SteamGridDB); on without one.
pub const ONLINE_SETTING: &str = "art.online";

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
        let services = Services { steamgriddb: Some(Service::new(&service.url(), key)), ..Services::default() };
        let summary = Lookup::run_now(dir.clone(), services, wanted(library), NOW, pace(), log);
        let log = lines.lock().unwrap().clone();
        Ran { summary, log }
    }

    fn api(requests: &[Request]) -> Vec<String> {
        requests.iter().filter(|r| !r.path.starts_with("/pictures/")).map(|r| r.path.clone()).collect()
    }

    #[test]
    fn the_stores_art_wins_and_ours_fills_only_what_it_lacks() {
        let mut steam = game("steam", "620", "Lantern Gate 2");
        steam.cover = Some("C:/Steam/librarycache/620/cover.jpg".into());
        let library = Library { games: vec![steam], ..Library::default() };
        assert_eq!(wanted(&library)[0].kinds, [Kind::Hero, Kind::Header, Kind::Square]);

        let fake =
            FakeService::start(0, script(vec![known(5, "Lantern Gate 2", &[("steam", "620")], &Kind::ALL)])).unwrap();
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
        // Tern Island has a cover and nothing else: the others are remembered as missed there.
        assert!(art.find("ea", "Origin.OFR.1", Kind::Cover).is_some());
        for kind in [Kind::Hero, Kind::Header, Kind::Square] {
            assert!(art.missed("ea", "Origin.OFR.1", kind, Online::SteamGridDb, NOW), "{kind:?}");
            assert!(!art.missed("ea", "Origin.OFR.1", kind, Online::Store, NOW), "{kind:?}");
        }
    }

    #[test]
    fn only_a_game_of_the_same_name_is_taken() {
        let fake = FakeService::start(
            0,
            script(vec![known(1, "Hadrel II", &[], &Kind::ALL), known(2, "Hadrel's Star", &[], &Kind::ALL)]),
        )
        .unwrap();
        let library = Library { games: vec![game("ubisoft", "4", "Hadrel")], ..Library::default() };
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
        let services = Services { steamgriddb: Some(Service::new(&fake.url(), KEY)), ..Services::default() };
        Lookup::run_now(art.clone(), services, wanted(&library), later, pace(), log);
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
            Services { steamgriddb: Some(Service::new(&fake.url(), "wrong-key")), ..Services::default() },
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
        assert!(art.index().is_empty() && !art.missed("steam", "0", Kind::Cover, Online::SteamGridDb, NOW));
    }

    #[test]
    fn a_refresh_empties_the_cache_and_fills_it_again_square_and_all_saying_how_far_it_has_got() {
        let mut first = game("steam", "11", "First Made-up Game");
        first.cover = Some("C:/Steam/librarycache/11/cover.jpg".into());
        let library = Library { games: vec![first, game("steam", "12", "Second Made-up Game")], ..Library::default() };
        let fake = FakeService::start(
            0,
            script(vec![
                known(1, "First Made-up Game", &[("steam", "11")], &Kind::ALL),
                known(2, "Second Made-up Game", &[("steam", "12")], &[Kind::Cover]),
            ]),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut db = crate::Database::open(&dir.path().join("library.db")).unwrap();
        db.sync("steam", &library, NOW).unwrap();
        let art = db.art_dir().unwrap().clone();
        // What an earlier run left: no square for the first game, remembered as missed, so that for a week it is
        // not asked for again, and a stale cover of the second's.
        art.miss("steam", "11", Kind::Square, Online::SteamGridDb, NOW).unwrap();
        art.save("steam", "12", Kind::Cover, b"\xFF\xD8\xFFan old picture").unwrap();
        assert!(db.snapshot().unwrap().games.iter().all(|game| game.square.is_none()));

        let forgot = db.forget_fetched().unwrap();
        assert_eq!(forgot.pictures, 1);
        assert!(art.index().is_empty(), "the cache is emptied");

        let events = Arc::new(Mutex::new(Vec::new()));
        let kept = events.clone();
        let (done, finished) = std::sync::mpsc::channel();
        let snapshot = db.snapshot().unwrap();
        let _lookup = Lookup::start(
            art.clone(),
            Services { steamgriddb: Some(Service::new(&fake.url(), KEY)), ..Services::default() },
            wanted(&snapshot),
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
        let events = events.lock().unwrap().clone();
        let mut looked: Vec<(usize, usize)> = events
            .iter()
            .filter_map(|event| match event {
                Event::Looked { done, of } => Some((*done, *of)),
                _ => None,
            })
            .collect();
        looked.sort();
        assert_eq!(looked, [(1, 2), (2, 2)]);
        let Some(Event::Done(summary)) = events.last() else { panic!("{events:?}") };
        assert!(!summary.stopped);
        assert_eq!(summary.pictures.steamgriddb, 4);

        // Filled again: the square tile's square among it, the stale cover a new one.
        let shown = db.snapshot().unwrap();
        let first = shown.games.iter().find(|game| game.source_id == "11").unwrap();
        assert_eq!(first.square, art.find("steam", "11", Kind::Square));
        assert!(first.square.is_some());
        assert_eq!(std::fs::read(art.find("steam", "12", Kind::Cover).unwrap()).unwrap(), PNG);
    }

    #[test]
    fn a_run_with_nothing_to_ask_is_not_over_for_a_refresh() {
        let library = Library { games: vec![game("steam", "1", "Made-up Game")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let summary = Lookup::run_now(
            ArtDir::under(dir.path()),
            Services::default(),
            wanted(&library),
            NOW,
            pace(),
            Arc::new(|_| {}),
        );
        assert!(summary.stopped);
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

    // ------------------------------------------------------------------------- the stores' own art online (#110)

    /// The stores' stand-ins' pictures: a PNG naming the store, the id and the kind, so a test can tell whose a
    /// picture in the cache is.
    fn store_png(store: &str, id: &str, kind: Kind) -> Vec<u8> {
        [PNG, format!("store {store} {id} {}", kind.name()).as_bytes()].concat()
    }

    fn with_stores(games: Vec<FakeGame>, stores: Vec<(&str, &str, &[Kind])>) -> FakeService {
        let mut stores_script = fake::StoreScript::new(Arc::new(store_png));
        stores_script.games = stores.into_iter().map(|(s, i, k)| (s.to_string(), i.to_string(), k.to_vec())).collect();
        FakeService::start(0, Script { stores: Some(stores_script), ..script(games) }).unwrap()
    }

    fn run_with(dir: &ArtDir, fake: &FakeService, key: Option<&str>, library: &Library) -> Ran {
        run_in(dir, fake, key, library, &model::ArtSource::ALL)
    }

    fn run_in(
        dir: &ArtDir,
        fake: &FakeService,
        key: Option<&str>,
        library: &Library,
        order: &[model::ArtSource],
    ) -> Ran {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let kept = lines.clone();
        let log: Log = Arc::new(move |line| kept.lock().unwrap().push(line.to_string()));
        let services = Services {
            stores: Some(StoreArt::new(Hosts::all_at(&fake.stores_url()))),
            steamgriddb: key.map(|key| Service::new(&fake.url(), key)),
            order: order.to_vec(),
        };
        let summary = Lookup::run_now(dir.clone(), services, wanted(library), NOW, pace(), log);
        let log = lines.lock().unwrap().clone();
        Ran { summary, log }
    }

    fn whose(art: &ArtDir, source: &str, id: &str, kind: Kind) -> String {
        let Some(path) = art.find(source, id, kind) else { return "none".into() };
        let bytes = std::fs::read(path).unwrap();
        if bytes.ends_with(b"enough for the cache") {
            "steamgriddb".into()
        } else {
            String::from_utf8_lossy(&bytes[PNG.len()..]).into_owned()
        }
    }

    fn xbox(id: &str, name: &str, product: &str) -> Game {
        let mut game = game("xbox", id, name);
        game.online.product = Some(product.into());
        game
    }

    #[test]
    fn the_stores_own_art_online_comes_before_steamgriddb_and_steamgriddb_is_the_last() {
        let mut halyard = xbox("Family.Halyard_1a2b3c4d5e6f7", "Halyard", "9ZZZMADEUP11");
        // Its own square logo, from its package: the store's files win.
        halyard.square = Some("C:/XboxGames/Halyard/Content/Logo480.png".into());
        let lantern = game("steam", "620", "Lantern Gate 2");
        let wayfarer = game("gog", "1207664663", "The Wayfarer 3");
        let library = Library { games: vec![halyard, lantern, wayfarer], ..Library::default() };
        let fake = with_stores(
            vec![
                known(1, "Halyard", &[], &Kind::ALL),
                known(2, "Lantern Gate 2", &[("steam", "620")], &Kind::ALL),
                known(3, "The Wayfarer 3", &[("gog", "1207664663")], &Kind::ALL),
            ],
            vec![
                ("xbox", "9ZZZMADEUP11", &Kind::ALL),
                // Steam has no square online; its hero is missing here, so SteamGridDB's is taken.
                ("steam", "620", &[Kind::Cover, Kind::Header]),
                ("gog", "1207664663", &[Kind::Cover, Kind::Hero, Kind::Header]),
            ],
        );
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());

        // With no key: the stores alone, and never SteamGridDB.
        let ran = run_with(&art, &fake, None, &library);
        assert!(fake.requests().iter().all(|r| r.path.starts_with("/stores/")), "{:?}", fake.requests());
        assert!(fake.requests().iter().all(|r| r.authorization.is_none()));
        let xbox_id = "Family.Halyard_1a2b3c4d5e6f7";
        assert_eq!(whose(&art, "xbox", xbox_id, Kind::Cover), "store xbox 9ZZZMADEUP11 cover");
        assert_eq!(whose(&art, "xbox", xbox_id, Kind::Square), "none", "the package's own square wins");
        assert_eq!(whose(&art, "steam", "620", Kind::Cover), "store steam 620 cover");
        assert_eq!(whose(&art, "steam", "620", Kind::Hero), "none");
        assert_eq!(whose(&art, "gog", "1207664663", Kind::Header), "store gog 1207664663 header");
        // Halyard: cover, hero, header; Lantern Gate 2: cover, header; The Wayfarer 3: cover, hero, header.
        assert_eq!(ran.summary.pictures, Pictures { store_files: 1, cache: 0, store_online: 8, steamgriddb: 0 });
        assert_eq!((ran.summary.looked_up, ran.summary.found), (3, 3));
        let line = ran.log.last().unwrap();
        assert!(line.starts_with("art: 3 looked up, 3 found, 0 none, in "), "{line}");
        assert!(line.ends_with("; pictures: 1 store files, 0 cache, 8 store online, 0 SteamGridDB"), "{line}");
        // What a store had not is remembered as its miss, not SteamGridDB's.
        assert!(art.missed("steam", "620", Kind::Hero, Online::Steam, NOW));
        assert!(art.missed("steam", "620", Kind::Hero, Online::Microsoft, NOW), "and Xbox had no game of its name");
        assert!(!art.missed("steam", "620", Kind::Hero, Online::SteamGridDb, NOW));

        // A key, the next start: SteamGridDB for what the stores had not, and the stores not asked again.
        let before = fake.requests().len();
        let ran = run_with(&art, &fake, Some(KEY), &library);
        let asked: Vec<Request> = fake.requests()[before..].to_vec();
        assert!(asked.iter().all(|r| !r.path.starts_with("/stores/")), "{asked:?}");
        assert_eq!(whose(&art, "steam", "620", Kind::Hero), "steamgriddb");
        assert_eq!(whose(&art, "steam", "620", Kind::Square), "steamgriddb", "Steam's square: SteamGridDB's");
        assert_eq!(whose(&art, "gog", "1207664663", Kind::Square), "steamgriddb");
        assert_eq!(whose(&art, "xbox", xbox_id, Kind::Square), "none", "never asked: the package has one");
        assert!(api(&asked).iter().any(|path| path.contains("dimensions=1024x1024,512x512")), "{asked:?}");
        // Steam's hero and square and GOG's square; Halyard is not asked about: the store gave it everything else.
        assert_eq!(ran.summary.pictures.steamgriddb, 3);
        assert_eq!(ran.summary.pictures.store_online, 0);
        let line = ran.log.last().unwrap();
        assert!(line.ends_with("; pictures: 1 store files, 0 cache, 0 store online, 3 SteamGridDB"), "{line}");
        for line in &ran.log {
            assert!(!line.contains(KEY) && !line.contains("Lantern") && !line.contains("Wayfarer"), "{line}");
        }
    }

    #[test]
    fn an_xbox_game_gets_the_stores_box_art_as_its_square() {
        let library =
            Library { games: vec![xbox("Family.Fernway_1", "Fernway", "9ZZZMADEUP12")], ..Library::default() };
        let fake = with_stores(Vec::new(), vec![("xbox", "9ZZZMADEUP12", &Kind::ALL)]);
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run_with(&art, &fake, None, &library);
        assert_eq!(ran.summary.pictures.store_online, 4);
        assert_eq!(whose(&art, "xbox", "Family.Fernway_1", Kind::Square), "store xbox 9ZZZMADEUP12 square");
        let catalogue: Vec<String> = api(&fake.requests()).into_iter().filter(|p| p.contains("/microsoft/")).collect();
        assert_eq!(catalogue, ["/stores/microsoft/v7.0/products?bigIds=9ZZZMADEUP12&market=US&languages=en-us"]);
        // Its pictures at a width a screen needs.
        assert!(fake.requests().iter().any(|r| r.path.ends_with("/BoxArt.png?w=600")), "{:?}", fake.requests());
        // And the snapshot shows it.
        let mut db = crate::Database::in_memory().unwrap();
        db.set_art_dir(Some(art.clone()));
        db.sync("xbox", &library, NOW).unwrap();
        let shown = db.snapshot().unwrap().games.remove(0);
        assert_eq!(shown.square, art.find("xbox", "Family.Fernway_1", Kind::Square));
        assert_eq!(shown.online.product.as_deref(), Some("9ZZZMADEUP12"), "kept in the database");
    }

    #[test]
    fn the_addresses_a_client_named_are_taken_with_no_catalogue_asked() {
        let fake = with_stores(Vec::new(), vec![("gog", "77", &[Kind::Cover])]);
        let mut moss = game("epic", "MossApp", "Moss");
        moss.online.cover = Some(format!("{}/gog-images/77/cover.png", fake.stores_url()));
        let library = Library { games: vec![moss], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run_with(&art, &fake, None, &library);
        assert_eq!(ran.summary.pictures.store_online, 1);
        let asked: Vec<String> = fake.requests().into_iter().map(|r| r.path).collect();
        // The picture, and for the rest the other stores' searches by name; no catalogue for the cover.
        assert_eq!(asked.iter().filter(|path| path.contains("/gog-images/")).count(), 1, "{asked:?}");
        assert!(asked.iter().all(|path| !path.contains("/gog/v2/")), "{asked:?}");
        assert!(art.find("epic", "MossApp", Kind::Cover).is_some());
        // Nothing else of an Epic game is online at its own store: no misses there, for another source to find.
        assert!(!art.missed("epic", "MossApp", Kind::Hero, Online::Store, NOW));
        assert!(art.missed("epic", "MossApp", Kind::Hero, Online::Steam, NOW), "no Steam game of its name");
    }

    #[test]
    fn a_store_asking_to_wait_is_waited_for_alone() {
        let mut stores_script = fake::StoreScript::new(Arc::new(store_png));
        stores_script.games = vec![("steam".into(), "9".into(), vec![Kind::Cover])];
        stores_script.busy = 1;
        let fake = FakeService::start(0, Script { stores: Some(stores_script), ..script(Vec::new()) }).unwrap();
        let library = Library { games: vec![game("steam", "9", "Quill")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let started = Instant::now();
        let ran = run_with(&art, &fake, None, &library);
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert_eq!(ran.summary.pictures.store_online, 1);
        assert!(ran.log.iter().any(|line| line == "art: Steam asked to wait; waiting 1 s"), "{:?}", ran.log);
    }

    #[test]
    fn with_art_online_off_nothing_is_asked_and_the_log_still_counts() {
        let fake = with_stores(Vec::new(), vec![("steam", "620", &Kind::ALL)]);
        let mut lantern = game("steam", "620", "Lantern Gate 2");
        lantern.cover = Some("C:/Steam/librarycache/620/library_600x900.jpg".into());
        let library = Library { games: vec![lantern], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let lines = Arc::new(Mutex::new(Vec::new()));
        let kept = lines.clone();
        let log: Log = Arc::new(move |line| kept.lock().unwrap().push(line.to_string()));
        let summary = Lookup::run_now(art.clone(), Services::default(), wanted(&library), NOW, pace(), log);
        assert!(fake.requests().is_empty());
        assert_eq!(summary.pictures.store_files, 1);
        let lines = lines.lock().unwrap().clone();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with("; pictures: 1 store files, 0 cache, 0 store online, 0 SteamGridDB"), "{lines:?}");
    }

    // ------------------------------------------------------------- any source's game, in the user's order (#246)

    /// A stand-in whose stores know games by name: `named` as `(store, name, id, kinds)`.
    fn with_named(games: Vec<FakeGame>, named: Vec<(&str, &str, &str, &[Kind])>) -> FakeService {
        let mut stores_script = fake::StoreScript::new(Arc::new(store_png));
        for (store, name, id, kinds) in named {
            stores_script.games.push((store.into(), id.into(), kinds.to_vec()));
            stores_script.named.push((store.into(), name.into(), id.into()));
        }
        FakeService::start(0, Script { stores: Some(stores_script), ..script(games) }).unwrap()
    }

    #[test]
    fn a_steam_game_with_no_square_at_steam_or_steamgriddb_gets_xboxs_and_each_kind_falls_through_apart() {
        let library = Library {
            games: vec![game("steam", "1874", "Made-up Reforger™"), game("epic", "Quarry", "Made-up Quarry")],
            ..Library::default()
        };
        let fake = with_named(
            // SteamGridDB has covers alone.
            vec![
                known(1, "Made-up Reforger", &[("steam", "1874")], &[Kind::Cover]),
                known(2, "Made-up Quarry", &[], &[Kind::Cover]),
            ],
            vec![
                // Steam has its own game's cover, hero and header; and the Epic game's, by its name.
                ("steam", "Made-up Reforger", "1874", &[Kind::Cover, Kind::Hero, Kind::Header]),
                ("steam", "Made-up Quarry", "77", &[Kind::Cover]),
                // A near name at Xbox is passed over; the same name, mark and all, is taken.
                ("xbox", "Made-up Reforger - Rally", "9ZZZMADEUP31", &Kind::ALL),
                ("xbox", "MADE-UP REFORGER", "9ZZZMADEUP32", &Kind::ALL),
                // PlayStation has the Epic game, Xbox not.
                ("playstation", "Made-up Quarry PS4 & PS5", "EP1-QUARRY", &[Kind::Square, Kind::Hero]),
            ],
        );
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run_with(&art, &fake, Some(KEY), &library);
        assert_eq!(whose(&art, "steam", "1874", Kind::Cover), "store steam 1874 cover");
        assert_eq!(whose(&art, "steam", "1874", Kind::Square), "store xbox 9ZZZMADEUP32 square");
        assert_eq!(whose(&art, "epic", "Quarry", Kind::Cover), "store steam 77 cover");
        assert_eq!(whose(&art, "epic", "Quarry", Kind::Hero), "store playstation EP1-QUARRY hero");
        assert_eq!(whose(&art, "epic", "Quarry", Kind::Square), "store playstation EP1-QUARRY square");
        assert_eq!(whose(&art, "epic", "Quarry", Kind::Header), "none", "no source had it");
        let asked: Vec<String> = fake.requests().into_iter().map(|r| r.path).collect();
        assert!(!asked.iter().any(|path| path.contains("9ZZZMADEUP31")), "the near name is never asked: {asked:?}");
        // Steam's own game is asked by its appid, never searched for by name at Steam.
        assert!(!asked.iter().any(|path| path.contains("storesearch") && path.contains("Reforger")), "{asked:?}");
        // SteamGridDB is asked only for what no store had: the Epic game's header.
        let grids: Vec<&String> =
            asked.iter().filter(|path| path.starts_with("/grids") || path.starts_with("/heroes")).collect();
        assert!(grids.iter().all(|path| path.contains("920x430")), "{grids:?}");
        // A line a game, by its id: which source gave each kind.
        let line = "art: steam 1874: cover Steam, hero Steam, header Steam, square Xbox";
        assert!(ran.log.contains(&line.to_string()), "{:?}", ran.log);
        assert!(
            ran.log
                .contains(&"art: epic Quarry: cover Steam, hero PlayStation, header none, square PlayStation".into()),
            "{:?}",
            ran.log
        );
        for line in &ran.log {
            assert!(!line.contains("Made-up"), "no game's name: {line}");
        }
    }

    #[test]
    fn a_name_the_stores_search_misses_whole_is_found_by_its_start_and_a_miss_is_said_without_the_name() {
        let library = Library {
            games: vec![game("steam", "6", "Made-up Field™ 6 Enhanced"), game("steam", "7", "Made-up Nowhere")],
            ..Library::default()
        };
        let fake = with_named(
            vec![],
            vec![
                // Its search finds nothing for "Made up Field 6 Enhanced"; for "Made up Field 6", the game and a
                // near name.
                ("xbox", "Made-up Field 6 Season Pass", "9ZZZMADEUP60", &Kind::ALL),
                ("xbox", "MADE-UP FIELD™ 6", "9ZZZMADEUP61", &Kind::ALL),
                ("xbox", "Made-up Nowhere 2", "9ZZZMADEUP70", &Kind::ALL),
            ],
        );
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run_with(&art, &fake, None, &library);
        assert_eq!(whose(&art, "steam", "6", Kind::Square), "store xbox 9ZZZMADEUP61 square");
        assert_eq!(whose(&art, "steam", "7", Kind::Square), "none", "a near name is never taken");
        let asked: Vec<String> = fake.requests().into_iter().map(|r| r.path).collect();
        assert!(!asked.iter().any(|path| path.contains("9ZZZMADEUP60") || path.contains("9ZZZMADEUP70")), "{asked:?}");
        // Asked twice ("Made up Nowhere", then "Made up"), the near names counted, none taken: said by the id.
        let said = |line: &&String| {
            line.starts_with("art: steam 7: Xbox: ") && line.ends_with(" candidates, none the same name")
        };
        assert!(ran.log.iter().any(|line| said(&line) && !line.contains(": 0 candidates")), "{:?}", ran.log);
        for line in &ran.log {
            assert!(!line.contains("Made-up") && !line.contains("Nowhere"), "no game's name: {line}");
        }
    }

    #[test]
    fn the_users_order_is_followed_and_a_source_turned_off_is_never_asked() {
        use model::ArtSource::*;
        let library = Library { games: vec![game("steam", "5", "Made-up Lantern")], ..Library::default() };
        let make = || {
            with_named(
                vec![known(5, "Made-up Lantern", &[("steam", "5")], &Kind::ALL)],
                vec![
                    ("steam", "Made-up Lantern", "5", &[Kind::Cover, Kind::Hero, Kind::Header]),
                    ("xbox", "Made-up Lantern", "9ZZZMADEUP05", &Kind::ALL),
                ],
            )
        };
        // SteamGridDB first: every kind is its.
        let fake = make();
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run_in(&art, &fake, Some(KEY), &library, &[SteamGridDb, Steam, Xbox, PlayStation]);
        assert_eq!(ran.summary.pictures.steamgriddb, 4);
        assert!(fake.requests().iter().all(|r| !r.path.starts_with("/stores/")), "{:?}", fake.requests());

        // Steam off, Xbox before PlayStation and SteamGridDB: Xbox's, and Steam never asked.
        let fake = make();
        let art = ArtDir::under(&dir.path().join("second"));
        let ran = run_in(&art, &fake, Some(KEY), &library, &[Xbox, PlayStation, SteamGridDb]);
        assert_eq!(ran.summary.pictures.store_online, 4);
        assert_eq!(whose(&art, "steam", "5", Kind::Cover), "store xbox 9ZZZMADEUP05 cover");
        assert!(fake.requests().iter().all(|r| !r.path.contains("/steam-")), "{:?}", fake.requests());
        assert!(fake.requests().iter().all(|r| r.authorization.is_none()), "SteamGridDB not needed");
    }

    #[test]
    fn steamgriddb_is_asked_for_its_plainest_style_first_then_any() {
        let fake = FakeService::start(
            0,
            Script {
                style: "white_logo".into(),
                ..script(vec![known(6, "Made-up Wren", &[("gog", "6")], &[Kind::Square])])
            },
        )
        .unwrap();
        let library = Library { games: vec![game("gog", "6", "Made-up Wren")], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run(&art, &fake, KEY, &library);
        assert_eq!(ran.summary.pictures.steamgriddb, 1);
        let squares: Vec<String> =
            api(&fake.requests()).into_iter().filter(|path| path.contains("1024x1024")).collect();
        assert_eq!(squares.len(), 2, "{squares:?}");
        assert!(squares[0].ends_with("&styles=alternate") && !squares[1].contains("styles="), "{squares:?}");
    }

    /// The user's match of a game on a source (docs/spec/match-game.md).
    fn matched(source: model::ArtSource, id: &str, name: &str) -> model::art_match::Match {
        model::art_match::Match { source, id: id.into(), name: name.into() }
    }

    #[test]
    fn a_game_matched_on_steamgriddb_is_asked_for_by_the_match_never_by_its_name() {
        // Battle.net's name for it is no name SteamGridDB knows; the user matched it to the game it is there.
        let fake = FakeService::start(0, script(vec![known(9, "Made-up Realm", &[], &Kind::ALL)])).unwrap();
        let mut forever = game("battlenet", "realm", "Made-up Realm: Forever");
        forever.matches = vec![matched(model::ArtSource::SteamGridDb, "9", "Made-up Realm")];
        let unmatched = game("battlenet", "other", "Made-up Realm: Forever");
        let library = Library { games: vec![forever, unmatched], ..Library::default() };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let ran = run(&art, &fake, KEY, &library);
        for kind in Kind::ALL {
            assert!(art.find("battlenet", "realm", kind).is_some(), "{kind:?}");
            assert!(art.find("battlenet", "other", kind).is_none(), "{kind:?}: its name finds none");
        }
        assert_eq!((ran.summary.found, ran.summary.none), (1, 1));
        let asked = api(&fake.requests());
        assert!(asked.iter().any(|path| path.starts_with("/grids/game/9?")), "{asked:?}");
        // The unmatched one is searched for by its name, once; the matched one never.
        assert_eq!(asked.iter().filter(|path| path.starts_with("/search/autocomplete/")).count(), 1, "{asked:?}");
    }

    #[test]
    fn a_game_matched_on_a_store_is_asked_for_by_the_stores_id_for_it() {
        use model::ArtSource::*;
        let library = || {
            let mut forever = game("battlenet", "realm", "Made-up Realm: Forever");
            forever.matches = vec![
                matched(Steam, "77", "Made-up Realm Classic"),
                matched(PlayStation, "UP0000-MADEUP77", "Made-up Realm"),
            ];
            Library { games: vec![forever], ..Library::default() }
        };
        let fake = || {
            with_named(
                vec![],
                vec![
                    ("steam", "Made-up Realm Classic", "77", &[Kind::Cover, Kind::Hero, Kind::Header]),
                    // Two of the PlayStation Store's games its search finds for the name: the one matched is taken.
                    ("playstation", "Made-up Realm", "UP0000-MADEUP76", &[Kind::Square, Kind::Hero]),
                    ("playstation", "Made-up Realm", "UP0000-MADEUP77", &[Kind::Square, Kind::Hero]),
                ],
            )
        };
        let dir = tempfile::tempdir().unwrap();
        let art = ArtDir::under(dir.path());
        let service = fake();
        run_in(&art, &service, None, &library(), &[Steam, PlayStation]);
        assert_eq!(whose(&art, "battlenet", "realm", Kind::Cover), "store steam 77 cover");
        assert_eq!(whose(&art, "battlenet", "realm", Kind::Square), "store playstation UP0000-MADEUP77 square");
        let asked: Vec<String> = service.requests().into_iter().map(|r| r.path).collect();
        assert!(!asked.iter().any(|path| path.contains("steam-store")), "Steam's search not asked: {asked:?}");

        // A PlayStation match its search no longer finds: none, and said by the game's id.
        let service = with_named(vec![], vec![("playstation", "Made-up Realm", "UP0000-MADEUP76", &[Kind::Square])]);
        let art = ArtDir::under(&dir.path().join("second"));
        let ran = run_in(&art, &service, None, &library(), &[PlayStation]);
        assert_eq!(whose(&art, "battlenet", "realm", Kind::Square), "none");
        let said = "art: battlenet realm: PlayStation: the game matched is not in its search";
        assert!(ran.log.iter().any(|line| line == said), "{:?}", ran.log);
    }
}
