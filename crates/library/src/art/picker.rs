//! The art picker's network (docs/spec/art-picker.md, #213): SteamGridDB's pages of a game's candidates, their
//! thumbnails, and the full picture of the one chosen, each fetched off the UI thread and told back as it comes.
//!
//! Two threads: one asks the API, a page at a time in the order asked; the other downloads thumbnails into a folder
//! of their own, which is emptied as the picker starts. Every request passes its host's gate first, as the art
//! lookup's do ([`Pace`]): a gap between any two requests to one host, and when a host says to wait (429 or 503,
//! `Retry-After`) nothing is asked of it until then, the wait doubling each time it says so again, up to
//! `Pace::tries` times before the page is said to have failed. Only the game's store id or name goes to the
//! service, and the key goes to its API alone.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::art_match::Hit;
use model::art_picker::{Answer, Ask, Failure};
use model::{ArtSource, GameKey};

use super::cache::extension;
use super::lookup::{host, Log, Pace};
use super::steamgriddb::{agent, download, Error, Service};
use super::store_online::{Store, StoreArt};

/// The thumbnails' folder, under the data folder: emptied each time a picker starts.
pub const THUMBS: &str = "art-picker";

/// When each host may next be asked, and how many times in a row it has said to wait.
struct Gates {
    pace: Pace,
    hosts: Mutex<HashMap<String, (Instant, u32)>>,
    stop: Arc<AtomicBool>,
    log: Log,
}

impl Gates {
    /// Makes `request` of `host` when its gate lets it, again while it says to wait.
    fn ask<T>(&self, host: &str, request: impl Fn() -> Result<T, Error>) -> Result<T, Error> {
        for _ in 0..self.pace.tries.max(1) {
            self.pass(host)?;
            match request() {
                Err(Error::Busy(after)) => self.busy(host, after),
                Ok(answer) => {
                    if let Some(gate) = self.hosts.lock().unwrap().get_mut(host) {
                        gate.1 = 0;
                    }
                    return Ok(answer);
                }
                Err(e) => return Err(e),
            }
        }
        Err(Error::Busy(None))
    }

    /// Waits for the host's gate to open, and closes it behind.
    fn pass(&self, host: &str) -> Result<(), Error> {
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return Err(Error::Failed("the picker closed".into()));
            }
            let wait = {
                let mut hosts = self.hosts.lock().unwrap();
                let now = Instant::now();
                let gate = hosts.entry(host.to_string()).or_insert((now, 0));
                if gate.0 <= now {
                    gate.0 = now + self.pace.gap;
                    return Ok(());
                }
                gate.0 - now
            };
            std::thread::sleep(wait.min(Duration::from_millis(100)));
        }
    }

    fn busy(&self, host: &str, after: Option<Duration>) {
        let wait = {
            let mut hosts = self.hosts.lock().unwrap();
            let gate = hosts.entry(host.to_string()).or_insert((Instant::now(), 0));
            gate.1 += 1;
            let doubled = self.pace.wait.saturating_mul(1 << (gate.1 - 1).min(16));
            let wait = match after {
                Some(after) if gate.1 == 1 => after,
                Some(after) => after.max(doubled),
                None => doubled,
            }
            .min(self.pace.most_wait);
            gate.0 = gate.0.max(Instant::now() + wait);
            wait
        };
        (self.log)(&format!("art: picker: {host} asked to wait; waiting {:.0} s", wait.as_secs_f64()));
    }
}

/// The picker's network, for as long as a picker is up. Dropping it stops what is under way after the requests in
/// flight; nothing more is told.
pub struct Picker {
    pages: Sender<Ask>,
    thumbs: Sender<(u64, String)>,
    gates: Arc<Gates>,
}

impl Picker {
    /// Starts the two threads. `service` is SteamGridDB with the user's key, or None without one: every page is
    /// then `Failure::NoKey` at once. `stores` is the stores' art online, None with art online off: their searches
    /// are then `Failure::Offline`. Thumbnails go into `thumbs`, emptied first. `tell` hears every answer.
    pub fn start(
        service: Option<Service>,
        stores: Option<StoreArt>,
        thumbs: &Path,
        pace: Pace,
        log: Log,
        tell: impl Fn(Answer) + Send + Sync + 'static,
    ) -> Self {
        let _ = std::fs::remove_dir_all(thumbs);
        let stop = Arc::new(AtomicBool::new(false));
        let gates = Arc::new(Gates { pace, hosts: Mutex::new(HashMap::new()), stop: stop.clone(), log: log.clone() });
        let tell = Arc::new(tell);
        let (pages, asked) = channel::<Ask>();
        {
            let (gates, tell, log) = (gates.clone(), tell.clone(), log.clone());
            std::thread::spawn(move || {
                let mut games: HashMap<GameKey, Option<u64>> = HashMap::new();
                let mut refused = false;
                while let Ok(ask) = asked.recv() {
                    if gates.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let answer = match ask {
                        Ask::Page { game, name, matched, tab, page } => {
                            let result = match (&service, refused) {
                                (None, _) | (_, true) => Err(Failure::NoKey),
                                (Some(service), false) => {
                                    let found = matched.map(|id| (game.clone(), Some(id)));
                                    games.extend(found);
                                    page_of(service, &gates, &mut games, &game, &name, tab, page)
                                }
                            };
                            match &result {
                                Ok(reply) => log(&format!(
                                    "art: picker: {} page {page}: {} of {} candidates",
                                    tab.key(),
                                    reply.items.len(),
                                    reply.total
                                )),
                                Err(Failure::KeyRefused) => {
                                    refused = true;
                                    log("art: picker: SteamGridDB refused the key");
                                }
                                Err(why) => log(&format!("art: picker: {} page {page} failed: {why:?}", tab.key())),
                            }
                            Answer::Page { game, tab, page, result }
                        }
                        Ask::Search { game, source, words } => {
                            let service = service.as_ref().filter(|_| !refused);
                            let result = search(service, stores.as_ref(), &gates, source, &words);
                            match &result {
                                Ok(hits) => log(&format!("art: match: {} found {}", source.name(), hits.len())),
                                Err(Failure::KeyRefused) => {
                                    refused = true;
                                    log("art: match: SteamGridDB refused the key");
                                }
                                Err(why) => log(&format!("art: match: {} failed: {why:?}", source.name())),
                            }
                            Answer::Results { game, source, words, result }
                        }
                        Ask::Thumb { .. } => continue,
                    };
                    if gates.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    tell(answer);
                }
            });
        }
        let (thumbs_to, wanted) = channel::<(u64, String)>();
        {
            let (gates, dir) = (gates.clone(), thumbs.to_path_buf());
            std::thread::spawn(move || {
                let agent = agent();
                while let Ok((id, url)) = wanted.recv() {
                    if gates.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let bytes = gates.ask(&host(&url), || download(&agent, &url));
                    let path = bytes.ok().and_then(|bytes| keep(&dir, &id.to_string(), &bytes));
                    if gates.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    tell(Answer::Thumb { id, path });
                }
            });
        }
        Self { pages, thumbs: thumbs_to, gates }
    }

    /// Asks for a page, or a thumbnail. A thumbnail asked for twice is fetched once (the file is kept).
    pub fn ask(&self, ask: Ask) {
        match ask {
            Ask::Thumb { id, url } => {
                let _ = self.thumbs.send((id, url));
            }
            page => {
                let _ = self.pages.send(page);
            }
        }
    }
}

/// The full picture of the one chosen, at `url`, downloaded on a thread of its own: whatever picker it was chosen in
/// has closed by then, and the download is the user's, so it is not stopped with it. Its host is asked as politely
/// (`pace`). `done` hears its bytes (a PNG, JPEG or WebP), or why not.
pub fn download_full(url: String, pace: Pace, log: Log, done: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    std::thread::spawn(move || {
        let gates = Gates { pace, hosts: Mutex::new(HashMap::new()), stop: Arc::new(AtomicBool::new(false)), log };
        let agent = agent();
        let got = match gates.ask(&host(&url), || download(&agent, &url)) {
            Ok(bytes) if extension(&bytes).is_some() => Ok(bytes),
            Ok(_) => Err("the picture is not a PNG, JPEG or WebP".to_string()),
            Err(Error::Failed(why)) => Err(why),
            Err(Error::Busy(_)) => Err("its server kept asking to wait".to_string()),
            Err(Error::KeyRefused) => Err("its server refused it".to_string()),
        };
        done(got);
    });
}

impl Drop for Picker {
    fn drop(&mut self) {
        self.gates.stop.store(true, Ordering::Relaxed);
    }
}

/// Why a request for the picker came to nothing, as the page says it.
fn failure(error: Error) -> Failure {
    match error {
        Error::KeyRefused => Failure::KeyRefused,
        Error::Busy(_) => Failure::Busy,
        Error::Failed(why) => Failure::Failed(why),
    }
}

/// One page of the game's candidates of a kind: the game found first, once for the picker's life.
fn page_of(
    service: &Service,
    gates: &Gates,
    games: &mut HashMap<GameKey, Option<u64>>,
    game: &GameKey,
    name: &str,
    tab: model::art_picker::Tab,
    page: u32,
) -> Result<model::art_picker::Reply, Failure> {
    let api = host(service.base());
    let id = match games.get(game) {
        Some(id) => *id,
        None => {
            let id = gates.ask(&api, || service.find_game(&game.source, &game.source_id, name)).map_err(failure)?;
            games.insert(game.clone(), id);
            id
        }
    };
    let id = id.ok_or(Failure::NoGame)?;
    gates.ask(&api, || service.candidates(id, tab, page)).map_err(failure)
}

/// An online source's search for the Match game page (docs/spec/match-game.md): the games it finds for `words`.
/// Only the words are sent.
fn search(
    service: Option<&Service>,
    stores: Option<&StoreArt>,
    gates: &Gates,
    source: ArtSource,
    words: &str,
) -> Result<Vec<Hit>, Failure> {
    match (Store::of(source), stores, service) {
        (Some(store), Some(stores), _) => {
            gates.ask(source.key(), || stores.find(store, words).unwrap_or(Ok(Vec::new()))).map_err(failure)
        }
        (Some(_), None, _) => Err(Failure::Offline),
        (None, _, Some(service)) => gates.ask(&host(service.base()), || service.search(words)).map_err(failure),
        (None, _, None) => Err(Failure::NoKey),
    }
}

/// Keeps a picture's bytes as `<dir>/<name>.<ext>` if they are a PNG, JPEG or WebP; gives where.
fn keep(dir: &Path, name: &str, bytes: &[u8]) -> Option<PathBuf> {
    let ext = extension(bytes)?;
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!("{name}.{ext}"));
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use model::art_picker::{Cell, Pager, Tab};

    use super::super::fake::{candidate_id, FakeGame, FakeService, Script};
    use super::super::Kind;
    use super::*;

    const KEY: &str = "picker-test-key";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nmade up";

    fn pace() -> Pace {
        Pace {
            workers: 1,
            gap: Duration::ZERO,
            wait: Duration::from_millis(50),
            most_wait: Duration::from_secs(5),
            tries: 5,
        }
    }

    fn fake(candidates: usize, busy: u32) -> FakeService {
        let game = FakeGame {
            id: 4,
            name: "Made-up Game".into(),
            platforms: vec![("steam".into(), "40".into())],
            kinds: Kind::ALL.to_vec(),
        };
        let script = Script { games: vec![game], candidates, busy, ..Script::new(KEY, Arc::new(|_, _| PNG.to_vec())) };
        FakeService::start(0, script).unwrap()
    }

    fn key() -> GameKey {
        GameKey { id: 1, source: "steam".into(), source_id: "40".into() }
    }

    /// Walks a tab's grid from its first page to its last against the stand-in, as the picker does: gives the
    /// candidates each grid page showed and the requests the stand-in was sent.
    fn walk(candidates: usize, tab: Tab) -> (Vec<Vec<u64>>, Vec<String>) {
        let fake = fake(candidates, 0);
        let dir = tempfile::tempdir().unwrap();
        let (sender, answers) = mpsc::channel();
        let sender = Mutex::new(sender);
        let picker = Picker::start(
            Some(Service::new(&fake.url(), KEY)),
            None,
            &dir.path().join(THUMBS),
            pace(),
            Arc::new(|_| {}),
            move |answer| {
                let _ = sender.lock().unwrap().send(answer);
            },
        );
        let mut pager = Pager::new(2, tab.per_page());
        let mut shown = Vec::new();
        let mut page = 0;
        loop {
            while let Some(want) = pager.want(page) {
                pager.asking(want);
                picker.ask(Ask::Page { game: key(), name: "Made-up Game".into(), matched: None, tab, page: want });
                match answers.recv_timeout(Duration::from_secs(10)).expect("an answer") {
                    Answer::Page { result: Ok(reply), .. } => pager.took(reply),
                    other => panic!("{other:?}"),
                }
            }
            shown.push(
                pager
                    .cells(page)
                    .into_iter()
                    .filter_map(|cell| match cell {
                        Cell::Candidate(c) => Some(c.id),
                        Cell::Coming => panic!("page {page} is whole"),
                        Cell::Fixed(_) => None,
                    })
                    .collect(),
            );
            page += 1;
            if page >= pager.pages() {
                break;
            }
        }
        let api =
            fake.requests().into_iter().map(|r| r.path).filter(|p| p.starts_with("/grids") || p.starts_with("/heroes"));
        (shown, api.collect())
    }

    #[test]
    fn against_the_stand_in_every_candidate_once_each_service_page_once_in_order_none_past_the_last() {
        let kind = Kind::Cover;
        // 0, 1, exactly one of the service's pages (50), one more than a page, and several pages.
        for candidates in [0, 1, 50, 51, 140] {
            let (shown, asked) = walk(candidates, Tab::Cover);
            let all: Vec<u64> = shown.iter().flatten().copied().collect();
            let expected: Vec<u64> = (0..candidates).map(|n| candidate_id(4, kind, n)).collect();
            assert_eq!(all, expected, "{candidates}: nothing skipped, nothing twice");
            // Whole pages of the grid: 14 cells, the first page's first two Automatic and the store's.
            assert_eq!(shown.len(), (2 + candidates).div_ceil(14).max(1), "{candidates}");
            let pages: Vec<String> = asked.iter().map(|path| path.rsplit('&').next().unwrap().to_string()).collect();
            let wanted: Vec<String> = (0..candidates.div_ceil(50).max(1)).map(|page| format!("page={page}")).collect();
            assert_eq!(pages, wanted, "{candidates}: {asked:?}");
            assert!(asked.iter().all(|path| path.contains("limit=50") && path.contains("dimensions=600x900")));
        }
    }

    #[test]
    fn each_tab_asks_for_its_own_kind() {
        let (_, square) = walk(3, Tab::Square);
        assert!(square[0].starts_with("/grids/game/4?dimensions=1024x1024,512x512"), "{square:?}");
        let (shown, background) = walk(3, Tab::Background);
        assert!(background[0].starts_with("/heroes/game/4?"), "{background:?}");
        assert_eq!(shown[0], (0..3).map(|n| candidate_id(4, Kind::Hero, n)).collect::<Vec<_>>());
    }

    #[test]
    fn without_a_key_nothing_is_asked_and_a_wait_is_waited_out() {
        let (sender, answers) = mpsc::channel();
        let sender = Mutex::new(sender);
        let dir = tempfile::tempdir().unwrap();
        let picker = Picker::start(None, None, &dir.path().join(THUMBS), pace(), Arc::new(|_| {}), move |answer| {
            let _ = sender.lock().unwrap().send(answer);
        });
        picker.ask(Ask::Page { game: key(), name: "Made-up Game".into(), matched: None, tab: Tab::Square, page: 0 });
        let answer = answers.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(answer, Answer::Page { result: Err(Failure::NoKey), .. }), "{answer:?}");

        // Asked to wait twice (Retry-After: 1): the page comes all the same, a second and more later.
        let fake = fake(5, 2);
        let (sender, answers) = mpsc::channel();
        let sender = Mutex::new(sender);
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let kept = lines.clone();
        let picker = Picker::start(
            Some(Service::new(&fake.url(), KEY)),
            None,
            &dir.path().join(THUMBS),
            pace(),
            Arc::new(move |line| kept.lock().unwrap().push(line.to_string())),
            move |answer| {
                let _ = sender.lock().unwrap().send(answer);
            },
        );
        let started = Instant::now();
        picker.ask(Ask::Page { game: key(), name: "Made-up Game".into(), matched: None, tab: Tab::Square, page: 0 });
        let answer = answers.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(started.elapsed() >= Duration::from_secs(1));
        let Answer::Page { result: Ok(reply), .. } = answer else { panic!("{answer:?}") };
        assert_eq!((reply.total, reply.items.len(), reply.limit), (5, 5, 50));
        assert!(lines.lock().unwrap().iter().any(|line| line.contains("asked to wait")));

        // A thumbnail: downloaded with no key, into the picker's folder.
        picker.ask(Ask::Thumb { id: reply.items[1].id, url: reply.items[1].thumb.clone() });
        let Answer::Thumb { path: Some(path), .. } = answers.recv_timeout(Duration::from_secs(10)).unwrap() else {
            panic!("a thumbnail")
        };
        assert!(path.starts_with(dir.path().join(THUMBS)));
        let thumbs: Vec<_> = fake.requests().into_iter().filter(|r| r.path.starts_with("/thumbs/")).collect();
        assert_eq!(thumbs.len(), 1);
        assert_eq!(thumbs[0].authorization, None);
        // And the full picture of the one chosen, from its own address.
        let (done, got) = mpsc::channel();
        drop(picker);
        download_full(reply.items[1].url.clone(), pace(), Arc::new(|_| {}), move |bytes| {
            let _ = done.send(bytes);
        });
        assert_eq!(got.recv_timeout(Duration::from_secs(10)).unwrap().unwrap(), PNG);
    }

    #[test]
    fn a_search_for_the_match_game_page_asks_the_source_named_with_the_words_alone() {
        use super::super::fake::StoreScript;
        use super::super::store_online::Hosts;
        use model::ArtSource;

        let mut stores = StoreScript::new(Arc::new(|_, _, _| PNG.to_vec()));
        stores.named = vec![
            ("steam".into(), "Made-up Realm".into(), "70".into()),
            ("steam".into(), "Made-up Realm Classic".into(), "71".into()),
        ];
        let games = vec![
            FakeGame { id: 9, name: "Made-up Realm".into(), platforms: vec![], kinds: Kind::ALL.to_vec() },
            FakeGame { id: 8, name: "Another Made-up Game".into(), platforms: vec![], kinds: Kind::ALL.to_vec() },
        ];
        let script = Script { games, stores: Some(stores), ..Script::new(KEY, Arc::new(|_, _| PNG.to_vec())) };
        let fake = FakeService::start(0, script).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let start = |service: Option<Service>, stores: Option<StoreArt>| {
            let (sender, answers) = mpsc::channel();
            let sender = Mutex::new(sender);
            let picker = Picker::start(service, stores, &dir.path().join(THUMBS), pace(), Arc::new(|_| {}), move |a| {
                let _ = sender.lock().unwrap().send(a);
            });
            (picker, answers)
        };
        let search = |source| Ask::Search { game: key(), source, words: "Made-up Realm".into() };
        let hits = |answer: Answer| match answer {
            Answer::Results { result: Ok(hits), words, .. } if words == "Made-up Realm" => {
                hits.into_iter().map(|hit| (hit.id, hit.name)).collect::<Vec<_>>()
            }
            other => panic!("{other:?}"),
        };

        let (picker, answers) =
            start(Some(Service::new(&fake.url(), KEY)), Some(StoreArt::new(Hosts::all_at(&fake.stores_url()))));
        picker.ask(search(ArtSource::SteamGridDb));
        let found = hits(answers.recv_timeout(Duration::from_secs(10)).unwrap());
        assert_eq!(found, [("9".to_string(), "Made-up Realm".to_string())]);
        picker.ask(search(ArtSource::Steam));
        let found = hits(answers.recv_timeout(Duration::from_secs(10)).unwrap());
        assert_eq!(found, [("70".into(), "Made-up Realm".into()), ("71".into(), "Made-up Realm Classic".into())]);
        // The key went to SteamGridDB's API alone; Steam's search was sent the words and nothing else.
        for request in fake.requests() {
            let to_api = request.path.starts_with("/search/");
            assert_eq!(request.authorization.is_some(), to_api, "{request:?}");
        }
        drop(picker);

        // With art online off the stores are not asked; with no key, SteamGridDB is not.
        let (_picker, answers) = start(None, None);
        _picker.ask(search(ArtSource::Steam));
        let answer = answers.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(answer, Answer::Results { result: Err(Failure::Offline), .. }), "{answer:?}");
        _picker.ask(search(ArtSource::SteamGridDb));
        let answer = answers.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(answer, Answer::Results { result: Err(Failure::NoKey), .. }), "{answer:?}");
    }
}
