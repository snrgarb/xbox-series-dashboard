//! On the fixture's fake Steam and a fake of what is asked online: no test touches the network.
use std::sync::Mutex;

use model::achievement::AchievementSource;
use source_steam::achievements::SteamAchievements;
use source_steam::fixture::{self, Fixture, ACHIEVEMENTS, ACHIEVEMENTS_GAME, NEWEST_PLAY};

use super::*;

const NOW: u64 = 1_800_000_000;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nan icon, as far as the cache can tell";

/// What the fake was asked, and how it answers.
#[derive(Default)]
struct Asked {
    rarity: Vec<String>,
    icons: Vec<String>,
    offline: bool,
    /// Answers 429.
    busy: bool,
    /// Icons, by the end of their address, the server has not got (404), or answers with something else.
    missing: Vec<String>,
    not_pictures: Vec<String>,
}

struct FakeFetch(Arc<Mutex<Asked>>);

impl Fetch for FakeFetch {
    fn rarity(&self, _source: &str, source_id: &str) -> Result<Option<HashMap<String, f64>>, Error> {
        let mut asked = self.0.lock().unwrap();
        asked.rarity.push(source_id.to_string());
        if asked.offline {
            return Err(Error::Failed("Steam could not be found: is the PC online?".into()));
        }
        if source_id != ACHIEVEMENTS_GAME.to_string() {
            return Ok(None);
        }
        // Steam's own names, in another case: matched all the same. One is left out: not known.
        let known = ACHIEVEMENTS.iter().filter(|a| a.key != "ACH_LAST_PARCEL");
        Ok(Some(known.map(|a| (a.key.to_lowercase(), a.rarity)).collect()))
    }

    fn icon(&self, url: &str) -> Result<Option<Vec<u8>>, Error> {
        let mut asked = self.0.lock().unwrap();
        asked.icons.push(url.to_string());
        if asked.offline {
            return Err(Error::Failed("a picture's server could not be reached".into()));
        }
        if asked.busy {
            return Err(Error::Busy(None));
        }
        if asked.missing.iter().any(|m| url.ends_with(m.as_str())) {
            return Ok(None);
        }
        if asked.not_pictures.iter().any(|m| url.ends_with(m.as_str())) {
            return Ok(Some(b"<html>not here</html>".to_vec()));
        }
        Ok(Some(PNG.to_vec()))
    }
}

struct Setup {
    _dir: tempfile::TempDir,
    root: PathBuf,
    db: Database,
    asked: Arc<Mutex<Asked>>,
    lines: Arc<Mutex<Vec<String>>>,
    follower: Follower,
    game: i64,
}

/// The fixture's fake Steam, its games synced into a database in a data folder, and a follower of it.
fn setup(icons: bool) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("steam");
    fixture::write(&root, &Fixture::standard()).unwrap();
    let mut db = Database::open(&dir.path().join("data").join(crate::FILE)).unwrap();
    db.sync("steam", &source_steam::load(&root).unwrap(), NOW).unwrap();
    let game = db.game_id("steam", &ACHIEVEMENTS_GAME.to_string()).unwrap().unwrap();
    let asked = Arc::new(Mutex::new(Asked::default()));
    let lines = Arc::new(Mutex::new(Vec::new()));
    let kept = lines.clone();
    let log: Log = Arc::new(move |line| kept.lock().unwrap().push(line.to_string()));
    let sources: Vec<Arc<dyn AchievementSource>> = vec![Arc::new(SteamAchievements::at(Some(root.clone())))];
    let follower = Follower::new(sources, Some(Box::new(FakeFetch(asked.clone()))), icons, log);
    Setup { _dir: dir, root, db, asked, lines, follower, game }
}

impl Setup {
    fn said(&self) -> Vec<String> {
        std::mem::take(&mut *self.lines.lock().unwrap())
    }

    /// Does every piece of work online there is.
    fn work_all(&mut self, now: u64) -> bool {
        let mut changed = false;
        while self.follower.has_work() {
            changed |= self.follower.work(&mut self.db, now).unwrap().changed;
        }
        changed
    }
}

#[test]
fn the_read_keeps_each_game_says_it_and_tells_of_nothing() {
    let mut s = setup(false);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    let said = s.said();
    assert_eq!(said[0], format!("achievements: {ACHIEVEMENTS_GAME} 2 of 6, score 20"));
    assert!(said[1].starts_with("achievements: 1 games, 2 of 6 unlocked, score 20, read in "), "{said:?}");

    let all = s.db.achievements(s.game).unwrap();
    let keys: Vec<&str> = all.iter().map(|a| a.key.as_str()).collect();
    assert_eq!(keys, ACHIEVEMENTS.iter().map(|a| a.key).collect::<Vec<_>>());
    assert!(all.iter().all(|a| a.rarity.is_none() && a.icon_file.is_none()));

    let summary = s.db.snapshot().unwrap().achievements;
    assert_eq!(summary.totals, Some(Totals { score: 20, unlocked: 2, of: 6, games: 1 }));
    assert_eq!(summary.of(s.game), Some(&GameProgress { game: s.game, unlocked: 2, of: 6, score: 20 }));
    // Newest first.
    let recent: Vec<_> = summary.recent.iter().map(|u| (u.game, u.achievement.key.as_str())).collect();
    assert_eq!(recent, [(s.game, "ACH_ASH_RUNNER"), (s.game, "ACH_FIRST_DELIVERY")]);
}

#[test]
fn no_game_with_achievements_is_no_totals() {
    let db = Database::in_memory().unwrap();
    assert_eq!(db.snapshot().unwrap().achievements, Achievements::default());
}

#[test]
fn rarity_is_asked_once_a_week_kept_with_its_date_and_scores_by_it() {
    let mut s = setup(false);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.said();
    assert!(s.work_all(NOW));
    assert_eq!(s.asked.lock().unwrap().rarity, [ACHIEVEMENTS_GAME.to_string()]);
    let said = s.said();
    assert_eq!(said[0], format!("achievements: rarity of {ACHIEVEMENTS_GAME}: 5 of 6 known"));
    // 82.5% is 10 and 31% is 15.
    assert_eq!(said[1], format!("achievements: {ACHIEVEMENTS_GAME} 2 of 6, score 25"));
    let all = s.db.achievements(s.game).unwrap();
    let rare: Vec<_> = all.iter().filter(|a| a.is_rare()).map(|a| a.key.as_str()).collect();
    assert_eq!(rare, ["ACH_NO_DETOURS", "ACH_PERFECT_LEDGER"]);
    assert_eq!(all.last().unwrap().rarity, None);
    assert_eq!(s.db.rarity_asked(s.game).unwrap(), Some(NOW));
    assert_eq!(s.db.snapshot().unwrap().achievements.totals.unwrap().score, 25);

    // A day on, the Dashboard started again: nothing asked, and the rarity read again stands.
    s.follower.read_all(&mut s.db, NOW + 86_400).unwrap();
    assert!(!s.follower.has_work());
    assert_eq!(s.db.achievements(s.game).unwrap()[0].rarity, Some(82.5));
    // A week on: asked again.
    s.follower.read_all(&mut s.db, NOW + RARITY_FOR).unwrap();
    assert!(s.follower.has_work());
}

#[test]
fn offline_the_rarity_kept_stands_and_nothing_more_is_asked() {
    let mut s = setup(false);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.work_all(NOW);
    s.asked.lock().unwrap().offline = true;
    s.said();
    s.follower.read_all(&mut s.db, NOW + RARITY_FOR).unwrap();
    assert!(!s.work_all(NOW + RARITY_FOR));
    let said = s.said();
    assert!(
        said.iter().any(|l| l.starts_with("achievements: rarity not fetched (Steam could not be found")),
        "{said:?}"
    );
    assert_eq!(s.db.achievements(s.game).unwrap()[0].rarity, Some(82.5));
    // Still due: asked at the next start.
    assert_eq!(s.db.rarity_asked(s.game).unwrap(), Some(NOW));
}

#[test]
fn a_new_unlock_while_the_game_runs_is_told_of_once_with_its_icon() {
    let mut s = setup(true);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.work_all(NOW);
    s.said();
    // The last unlocks' icons came in with the read.
    assert!(s.db.snapshot().unwrap().achievements.recent.iter().all(|u| u.achievement.icon_file.is_some()));

    s.follower.ask(&mut s.db, Ask::Playing { game: s.game, since: NOW }, NOW).unwrap();
    assert_eq!(s.said(), [format!("achievements: watching {ACHIEVEMENTS_GAME} (Ashfall Courier)")]);
    // The first look reads the files as they are: nothing new.
    assert!(s.follower.look(&mut s.db).unwrap().is_empty());
    assert!(s.follower.look(&mut s.db).unwrap().is_empty());
    s.said();

    fixture::unlock_achievement(&s.root, "ACH_NO_DETOURS", NOW + 30).unwrap();
    let told = s.follower.look(&mut s.db).unwrap();
    assert_eq!(told.len(), 1);
    let a = &told[0].achievement;
    assert_eq!(
        (told[0].game, a.key.as_str(), a.unlocked, a.score(), a.is_rare()),
        (s.game, "ACH_NO_DETOURS", Some(NOW + 30), 30, true)
    );
    assert!(a.icon_file.as_ref().is_some_and(|f| f.is_file()), "{a:?}");
    assert_eq!(
        s.said(),
        [
            format!("achievements: {ACHIEVEMENTS_GAME} unlocked ACH_NO_DETOURS \"No Detours\": score 30, 7.4% of players, Rare"),
            format!("achievements: {ACHIEVEMENTS_GAME} 3 of 6, score 55"),
        ]
    );
    // Told of once.
    assert!(s.follower.look(&mut s.db).unwrap().is_empty());
    let summary = s.db.snapshot().unwrap().achievements;
    assert_eq!(summary.recent[0].achievement.key, "ACH_NO_DETOURS");
    assert_eq!(summary.totals.unwrap().score, 55);
}

#[test]
fn an_older_sessions_unlock_is_kept_and_not_told_of() {
    let mut s = setup(false);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.follower.ask(&mut s.db, Ask::Playing { game: s.game, since: NOW }, NOW).unwrap();
    s.follower.look(&mut s.db).unwrap();
    // Steam syncs an unlock from another PC, from yesterday.
    fixture::unlock_achievement(&s.root, "ACH_NIGHT_ROUTE", NOW - 86_400).unwrap();
    assert!(s.follower.look(&mut s.db).unwrap().is_empty());
    assert_eq!(s.db.snapshot().unwrap().achievements.totals.unwrap().unlocked, 3);
    // But one from the minute before the game was seen running is this session's.
    fixture::unlock_achievement(&s.root, "ACH_LAST_PARCEL", NOW - 20).unwrap();
    assert_eq!(s.follower.look(&mut s.db).unwrap().len(), 1);
}

#[test]
fn icons_on_request_and_none_with_art_online_off() {
    let mut s = setup(true);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.work_all(NOW);
    s.follower.ask(&mut s.db, Ask::Icons(s.game), NOW).unwrap();
    let mut came = Vec::new();
    while s.follower.has_work() {
        came.extend(s.follower.work(&mut s.db, NOW).unwrap().icons);
    }
    assert!(came.iter().all(|&g| g == s.game) && !came.is_empty());
    let all = s.db.achievements(s.game).unwrap();
    assert!(all.iter().all(|a| a.icon_file.is_some() && a.icon_locked_file.is_some()), "{all:?}");
    // Every icon fetched once.
    let asked = s.asked.lock().unwrap().icons.clone();
    assert_eq!(asked.len(), 12, "{asked:?}");

    let mut s = setup(false);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.follower.ask(&mut s.db, Ask::Icons(s.game), NOW).unwrap();
    s.work_all(NOW);
    assert!(s.asked.lock().unwrap().icons.is_empty());
}

/// The last part of an address.
fn file_of(url: &str) -> String {
    url.rsplit('/').next().unwrap().to_string()
}

/// Every icon of the game's that is fetched on request, until there is no more to do.
fn fetch_icons(s: &mut Setup) {
    s.follower.ask(&mut s.db, Ask::Icons(s.game), NOW).unwrap();
    s.work_all(NOW);
}

#[test]
fn an_icon_the_server_has_not_got_is_said_once_and_the_rest_come_in() {
    let mut s = setup(true);
    s.follower.read_all(&mut s.db, NOW).unwrap();
    s.work_all(NOW);
    let all = s.db.achievements(s.game).unwrap();
    // Night Route's icon is a 404; No Detours' grey one is a page that is no picture.
    let missing = all[2].icon.clone().unwrap();
    let not_picture = all[3].icon_locked.clone().unwrap();
    s.asked.lock().unwrap().missing.push(file_of(&missing));
    s.asked.lock().unwrap().not_pictures.push(file_of(&not_picture));
    s.said();

    fetch_icons(&mut s);
    let said = s.said();
    assert_eq!(
        said,
        [
            format!("achievements: an icon of {ACHIEVEMENTS_GAME} not found: {}", file_of(&missing)),
            format!("achievements: an icon of {ACHIEVEMENTS_GAME} is not a picture: {}", file_of(&not_picture)),
        ]
    );
    let all = s.db.achievements(s.game).unwrap();
    let cached =
        all.iter().map(|a| a.icon_file.is_some() as usize + a.icon_locked_file.is_some() as usize).sum::<usize>();
    assert_eq!(cached, 10);
    assert!(all[2].icon_file.is_none() && all[3].icon_locked_file.is_none());

    // Asked for again, and unlocked while the game runs: neither is asked of the server again this run.
    fetch_icons(&mut s);
    s.follower.ask(&mut s.db, Ask::Playing { game: s.game, since: NOW }, NOW).unwrap();
    s.follower.look(&mut s.db).unwrap();
    fixture::unlock_achievement(&s.root, "ACH_NIGHT_ROUTE", NOW + 5).unwrap();
    let told = s.follower.look(&mut s.db).unwrap();
    assert_eq!((told.len(), told[0].achievement.icon_file.as_ref()), (1, None));
    let asked = s.asked.lock().unwrap().icons.clone();
    assert_eq!(asked.iter().filter(|u| **u == missing).count(), 1, "{asked:?}");
    assert_eq!(asked.iter().filter(|u| **u == not_picture).count(), 1, "{asked:?}");
    assert!(s.said().iter().all(|l| !l.contains("not found") && !l.contains("not a picture")));
}

#[test]
fn a_busy_or_unreachable_server_stops_icons_for_the_run() {
    for busy in [true, false] {
        let mut s = setup(true);
        s.follower.read_all(&mut s.db, NOW).unwrap();
        {
            let mut asked = s.asked.lock().unwrap();
            (asked.busy, asked.offline) = (busy, !busy);
        }
        s.work_all(NOW);
        let why = if busy { "the service asked to wait" } else { "a picture's server could not be reached" };
        assert!(s.said().contains(&format!("achievements: icons not fetched: {why}")));
        // One asked, and none after it: not on request, nor for an unlock.
        fetch_icons(&mut s);
        s.follower.ask(&mut s.db, Ask::Playing { game: s.game, since: NOW }, NOW).unwrap();
        s.follower.look(&mut s.db).unwrap();
        fixture::unlock_achievement(&s.root, "ACH_NIGHT_ROUTE", NOW + 5).unwrap();
        assert_eq!(s.follower.look(&mut s.db).unwrap().len(), 1);
        assert_eq!(s.asked.lock().unwrap().icons.len(), 1);
    }
}

#[test]
fn every_shape_of_icon_a_schema_holds_is_asked_for_at_its_address_and_none_for_none() {
    use source_steam::achievements::{icon_address, ICONS};
    let mut s = setup(true);
    let hash = "4c936bef605fe82e00145c7b37287a13cd690af3";
    let appid = ACHIEVEMENTS_GAME;
    let shapes = [
        format!("{hash}.jpg"),
        hash.to_uppercase(),
        "tf_burn_bw".to_string(),
        "https://cdn.example.com/icons/full.jpg".to_string(),
        String::new(),
        "  ".to_string(),
    ];
    let list: Vec<Achievement> = shapes
        .iter()
        .enumerate()
        .map(|(i, given)| Achievement {
            key: format!("K{i}"),
            name: format!("K{i}"),
            icon: icon_address(appid, given),
            icon_locked: icon_address(appid, ""),
            ..Achievement::default()
        })
        .collect();
    s.db.put_achievements(s.game, &list).unwrap();
    fetch_icons(&mut s);
    let asked = s.asked.lock().unwrap().icons.clone();
    // The hash twice over (with and without its extension, in either case) is one icon, asked once.
    assert_eq!(
        asked,
        [
            format!("{ICONS}/{appid}/{hash}.jpg"),
            format!("{ICONS}/{appid}/tf_burn_bw.jpg"),
            "https://cdn.example.com/icons/full.jpg".to_string(),
        ]
    );
    let all = s.db.achievements(s.game).unwrap();
    assert!(all[..4].iter().all(|a| a.icon_file.is_some()), "{all:?}");
    assert!(all[4..].iter().all(|a| a.icon.is_none() && a.icon_file.is_none() && a.icon_locked.is_none()));
}

#[test]
fn an_achievement_the_store_drops_goes_and_a_reset_one_locks_again() {
    let mut db = Database::in_memory().unwrap();
    db.sync(
        "steam",
        &Library {
            games: vec![model::Game {
                source: "steam".into(),
                source_id: "7".into(),
                name: "Seven".into(),
                ..model::Game::default()
            }],
            ..Library::default()
        },
        NOW,
    )
    .unwrap();
    let game = db.game_id("steam", "7").unwrap().unwrap();
    let a = |key: &str, unlocked| Achievement { key: key.into(), name: key.into(), unlocked, ..Achievement::default() };
    let newly = db.put_achievements(game, &[a("A", Some(5)), a("B", None)]).unwrap();
    assert_eq!(newly.len(), 1);
    db.set_rarity(game, Some(&HashMap::from([("A".to_string(), 3.0)])), NOW).unwrap();
    let newly = db.put_achievements(game, &[a("A", None), a("C", Some(0))]).unwrap();
    assert_eq!(newly.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(), ["C"]);
    let all = db.achievements(game).unwrap();
    assert_eq!(
        all.iter().map(|a| (a.key.as_str(), a.unlocked, a.rarity)).collect::<Vec<_>>(),
        [("A", None, Some(3.0)), ("C", Some(0), None)]
    );
    // Unlocked at a time not known: counted, the oldest of the recent.
    assert_eq!(db.snapshot().unwrap().achievements.totals, Some(Totals { score: 10, unlocked: 1, of: 2, games: 1 }));
}

#[test]
fn the_thread_sends_the_read_then_an_unlock_while_the_game_runs() {
    let s = setup(false);
    let path = s._dir.path().join("data").join(crate::FILE);
    let sources: Vec<Arc<dyn AchievementSource>> = vec![Arc::new(SteamAchievements::at(Some(s.root.clone())))];
    let log: Log = Arc::new(|_| {});
    let handle = spawn(path, sources, None, false, log, || {});
    let wait_for = |what: &dyn Fn(&Update) -> bool| {
        let until = Instant::now() + Duration::from_secs(20);
        while Instant::now() < until {
            if let Some(update) = handle.take().filter(|u| what(u)) {
                return update;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("nothing came");
    };
    let first = wait_for(&|u| u.library.is_some());
    assert_eq!(first.library.unwrap().achievements.totals.unwrap().unlocked, 2);
    assert!(first.unlocked.is_empty());

    handle.ask(Ask::Playing { game: s.game, since: crate::now() });
    // Its first look reads the files as they are.
    std::thread::sleep(WATCH * 2);
    fixture::unlock_achievement(&s.root, "ACH_PERFECT_LEDGER", crate::now()).unwrap();
    let told = wait_for(&|u| !u.unlocked.is_empty());
    assert_eq!(told.unlocked[0].achievement.key, "ACH_PERFECT_LEDGER");
    assert_eq!(told.library.unwrap().achievements.totals.unwrap().unlocked, 3);
}

#[test]
fn steams_percentages_are_read_as_strings_or_numbers() {
    let json = serde_json::json!({"achievementpercentages": {"achievements": [
        {"name": "ACH.ONE", "percent": "74.0"},
        {"name": "ACH.TWO", "percent": 0.5},
        {"name": "ACH.BAD", "percent": "lots"},
        {"name": "ACH.BIG", "percent": 140}
    ]}});
    let read = percentages(&json).unwrap();
    assert_eq!(read.len(), 3);
    assert_eq!((read["ACH.ONE"], read["ACH.TWO"], read["ACH.BIG"]), (74.0, 0.5, 100.0));
    assert_eq!(percentages(&serde_json::json!({})), None);
}

#[test]
fn a_stand_ins_hosts_take_every_icon() {
    let hosts = Hosts::all_at("http://127.0.0.1:9/stores/");
    assert_eq!(hosts.steam_api, "http://127.0.0.1:9/stores/steam-api");
    assert_eq!(
        hosts.icon("https://cdn.steamstatic.com/steamcommunity/public/images/apps/7/a.jpg"),
        "http://127.0.0.1:9/stores/icons/cdn.steamstatic.com/steamcommunity/public/images/apps/7/a.jpg"
    );
    assert_eq!(Hosts::default().icon("https://x/y.jpg"), "https://x/y.jpg");
    let _ = NEWEST_PLAY;
}
