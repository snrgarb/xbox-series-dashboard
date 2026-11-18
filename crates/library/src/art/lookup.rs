//! Looking up the art a store's files on the PC gave none for, off the UI thread: a game's missing pictures are taken
//! from our own cache if it has them, else asked of the game's own store online (GOG's catalogue, a client's named
//! addresses), then of the online sources in the user's order (`model::ArtOrder`: Steam's store, the Microsoft
//! Store's catalogue, the PlayStation Store, SteamGridDB with the user's key, by default), each kind of picture
//! falling through them apart until one has it; a few games at a time, and kept in the cache for every start after.
//! What was looked for and not found is remembered for a week, in each place apart, so a start does not ask again.
//! Each game asked about is said in one line of the log: by its source and its id there (never its name), and for
//! each kind asked, which source gave it, or that none had it.
//!
//! Every host is asked politely: one request at a time per worker, two workers, a gap between any two requests to
//! the same host, and when one says to wait (429 or 503, `Retry-After`) every worker waits that long before asking
//! it again, longer each time it says so again. A key SteamGridDB refuses (401) stops every lookup at once, and is
//! told once.
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::{ArtSource, Library, OnlineArt};

use super::cache::{ArtDir, Kind, Online};
use super::steamgriddb::{Error, Service, NAME};
use super::store_online::{Store, StoreArt};

/// A game of a store, the pictures it has, and which it lacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Want {
    pub source: String,
    pub source_id: String,
    pub name: String,
    /// The kinds it has no picture of.
    pub kinds: Vec<Kind>,
    /// The pictures it has: its store's files', or our cache's (`ArtDir`) that a snapshot filled in.
    pub has: Vec<PathBuf>,
    /// What its client said of its art online.
    pub online: OnlineArt,
}

/// Every game of the library that is a store's, with the pictures it lacks, in its order (the most recently played
/// first, so what Home shows comes first). A made-up game, of no source, is not looked up.
pub fn wanted(library: &Library) -> Vec<Want> {
    library
        .games
        .iter()
        .filter(|game| !game.source.is_empty() && !game.source_id.is_empty())
        .map(|game| {
            let pictures = [&game.cover, &game.hero, &game.header, &game.square];
            let kinds = Kind::ALL.into_iter().zip(pictures).filter(|(_, has)| has.is_none()).map(|(k, _)| k).collect();
            Want {
                source: game.source.clone(),
                source_id: game.source_id.clone(),
                name: game.name.clone(),
                kinds,
                has: pictures.into_iter().flatten().cloned().collect(),
                online: game.online.clone(),
            }
        })
        .collect()
}

/// Where the missing pictures are asked for: either may be off. With neither, nothing is asked.
#[derive(Debug)]
pub struct Services {
    /// The stores' art online: on unless Settings' Download art online is off.
    pub stores: Option<StoreArt>,
    /// SteamGridDB, with the user's key: only with one, and with art online on.
    pub steamgriddb: Option<Service>,
    /// The online sources that are on, in the order they are asked (Settings > Art's sources): a source not here is
    /// not asked, whatever else is set. The game's own store comes before all of them.
    pub order: Vec<ArtSource>,
}

impl Default for Services {
    fn default() -> Self {
        Self { stores: None, steamgriddb: None, order: ArtSource::ALL.to_vec() }
    }
}

impl Services {
    pub fn any(&self) -> bool {
        self.stores.is_some() || self.steamgriddb.is_some()
    }
}

/// How hard each host is asked.
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    /// Games looked up at once.
    pub workers: usize,
    /// The least time between two requests to one host, whichever worker makes them.
    pub gap: Duration,
    /// The wait when a host says to wait and not for how long; doubled each time it says so again in a row.
    pub wait: Duration,
    /// The longest wait.
    pub most_wait: Duration,
    /// How many times one request is made while the host says to wait, before the picture is passed over.
    pub tries: u32,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            workers: 2,
            gap: Duration::from_millis(250),
            wait: Duration::from_secs(30),
            most_wait: Duration::from_secs(600),
            tries: 5,
        }
    }
}

/// What a run of lookups says as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A game has a new picture in the cache: the library's next snapshot shows it.
    Found { source: String, source_id: String },
    /// Another game has been looked at, whatever came of it: `done` of the run's `of`.
    Looked { done: usize, of: usize },
    /// SteamGridDB refused the key: nothing more is asked of it until it changes.
    KeyRefused,
    /// The run is over.
    Done(Summary),
}

/// Where the pictures of the games a run looked at came from, in counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pictures {
    /// The stores' own files on the PC.
    pub store_files: usize,
    /// Our own cache, from earlier runs.
    pub cache: usize,
    /// The stores' own art online, in this run.
    pub store_online: usize,
    /// SteamGridDB, in this run.
    pub steamgriddb: usize,
}

/// What a run did, in counts: what the log says of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Games something was asked about online.
    pub looked_up: usize,
    /// Of those, the games that got a picture.
    pub found: usize,
    /// The games nothing was found for.
    pub none: usize,
    /// The games a request failed for, to be asked about again next time.
    pub failed: usize,
    pub millis: u128,
    pub pictures: Pictures,
    /// The run was stopped before it had looked at every game: told to, or the key was refused.
    pub stopped: bool,
}

impl std::fmt::Display for Summary {
    /// `12 looked up, 9 found, 3 none, in 2.1 s; pictures: 40 store files, 12 cache, 9 store online, 3 SteamGridDB`,
    /// with `, 1 failed` before the time when one did.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} looked up, {} found, {} none", self.looked_up, self.found, self.none)?;
        if self.failed > 0 {
            write!(f, ", {} failed", self.failed)?;
        }
        let p = &self.pictures;
        write!(
            f,
            ", in {:.1} s; pictures: {} store files, {} cache, {} store online, {} SteamGridDB",
            self.millis as f64 / 1000.0,
            p.store_files,
            p.cache,
            p.store_online,
            p.steamgriddb
        )
    }
}

/// Where a run says what it did: a line of the log.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

/// A run of lookups going on in the background. Dropping it, or `stop`, ends it after the requests under way.
pub struct Lookup {
    stop: Arc<AtomicBool>,
}

impl Lookup {
    /// Looks up `wants` on threads of its own, telling `tell` as each game's art comes in and when it is over.
    /// `now` is the time, in Unix seconds, that remembered misses are counted from.
    pub fn start(
        dir: ArtDir,
        services: Services,
        wants: Vec<Want>,
        now: u64,
        pace: Pace,
        log: Log,
        tell: impl Fn(Event) + Send + Sync + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let run = Run::new(dir, services, wants, now, pace, log, Box::new(tell), stop.clone());
        std::thread::spawn(move || run.run());
        Self { stop }
    }

    /// Looks up `wants` here and now, and gives what it did: for a picture drawn offscreen, and tests.
    pub fn run_now(dir: ArtDir, services: Services, wants: Vec<Want>, now: u64, pace: Pace, log: Log) -> Summary {
        let run = Run::new(dir, services, wants, now, pace, log, Box::new(|_| {}), Arc::new(AtomicBool::new(false)));
        run.run()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Lookup {
    fn drop(&mut self) {
        self.stop();
    }
}

/// When a host may next be asked, and how many times in a row it has said to wait.
struct Gate {
    next: Instant,
    busy: u32,
}

struct Run {
    dir: ArtDir,
    services: Services,
    queue: Mutex<VecDeque<Want>>,
    /// Each host's gate, by its name.
    gates: Mutex<HashMap<String, Gate>>,
    now: u64,
    pace: Pace,
    log: Log,
    tell: Box<dyn Fn(Event) + Send + Sync>,
    stop: Arc<AtomicBool>,
    refused: AtomicBool,
    /// The first failure's words: said once a run.
    failed_said: AtomicBool,
    summary: Mutex<Summary>,
    /// The games the run was given, and how many have been looked at.
    of: usize,
    done: AtomicUsize,
}

/// Why a request was not answered.
enum Halt {
    /// The run is stopping: the key was refused, or it was told to.
    Stop,
    Failed,
}

/// How one game went.
#[derive(Default)]
struct Outcome {
    /// Something was asked online.
    asked: bool,
    /// The kinds asked for online, in this run.
    tried: Vec<Kind>,
    /// The kinds found, and where.
    got: Vec<(Kind, &'static str)>,
    store_online: usize,
    steamgriddb: usize,
    failed: bool,
    stopped: bool,
}

impl Run {
    #[allow(clippy::too_many_arguments)]
    fn new(
        dir: ArtDir,
        services: Services,
        wants: Vec<Want>,
        now: u64,
        pace: Pace,
        log: Log,
        tell: Box<dyn Fn(Event) + Send + Sync>,
        stop: Arc<AtomicBool>,
    ) -> Self {
        let mut pictures = Pictures::default();
        for path in wants.iter().flat_map(|want| &want.has) {
            if path.starts_with(dir.root()) {
                pictures.cache += 1;
            } else {
                pictures.store_files += 1;
            }
        }
        Self {
            dir,
            services,
            of: wants.len(),
            done: AtomicUsize::new(0),
            queue: Mutex::new(wants.into()),
            gates: Mutex::new(HashMap::new()),
            now,
            pace,
            log,
            tell,
            stop,
            refused: AtomicBool::new(false),
            failed_said: AtomicBool::new(false),
            summary: Mutex::new(Summary { pictures, ..Summary::default() }),
        }
    }

    fn run(self) -> Summary {
        let started = Instant::now();
        if self.services.any() {
            std::thread::scope(|scope| {
                for _ in 0..self.pace.workers.max(1) {
                    scope.spawn(|| self.work());
                }
            });
        }
        let mut summary = self.summary.lock().unwrap().clone();
        summary.millis = started.elapsed().as_millis();
        // With no service to ask nothing was looked at, and the run is not over: it is for one that can ask.
        summary.stopped = self.stopped() || !self.services.any() || self.done.load(Ordering::Relaxed) < self.of;
        (self.log)(&format!("art: {summary}"));
        (self.tell)(Event::Done(summary.clone()));
        summary
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn work(&self) {
        while !self.stopped() {
            let Some(want) = self.queue.lock().unwrap().pop_front() else { return };
            let outcome = self.look_up(&want);
            let found = outcome.store_online + outcome.steamgriddb > 0;
            if found {
                (self.tell)(Event::Found { source: want.source.clone(), source_id: want.source_id.clone() });
            }
            // A game cut short by a stop is not looked at: the next run takes it again.
            if !outcome.stopped {
                let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
                (self.tell)(Event::Looked { done, of: self.of });
            }
            let mut summary = self.summary.lock().unwrap();
            summary.pictures.store_online += outcome.store_online;
            summary.pictures.steamgriddb += outcome.steamgriddb;
            if !outcome.asked || (outcome.stopped && !found) {
                continue;
            }
            summary.looked_up += 1;
            if found {
                summary.found += 1;
            } else if outcome.failed {
                summary.failed += 1;
            } else {
                summary.none += 1;
            }
        }
    }

    /// The kinds of `want` that our cache does not have now and that were not missed `there` lately.
    fn still_wanted(&self, want: &Want, kinds: &[Kind], there: Online) -> Vec<Kind> {
        let (source, id) = (want.source.as_str(), want.source_id.as_str());
        kinds
            .iter()
            .copied()
            .filter(|&kind| {
                self.dir.find(source, id, kind).is_none() && !self.dir.missed(source, id, kind, there, self.now)
            })
            .collect()
    }

    fn miss(&self, want: &Want, kind: Kind, there: Online) {
        if let Err(e) = self.dir.miss(&want.source, &want.source_id, kind, there, self.now) {
            self.failed(&format!("the art folder: {e:#}"));
        }
    }

    /// Downloads a picture and keeps it: whether it was kept. Bytes that are not a picture this program draws are
    /// as good as none.
    fn fetch(&self, want: &Want, kind: Kind, url: &str, there: Online, out: &mut Outcome) -> bool {
        let download = |url: &str| match there {
            Online::SteamGridDb => self.services.steamgriddb.as_ref().map(|service| service.download(url)),
            _ => self.services.stores.as_ref().map(|stores| stores.download(url)),
        };
        match self.ask(&host(url), || download(url).unwrap_or(Err(Error::Failed("no service".into())))) {
            Ok(bytes) => match self.dir.save(&want.source, &want.source_id, kind, &bytes) {
                Ok(_) => true,
                Err(_) => {
                    self.miss(want, kind, there);
                    false
                }
            },
            Err(Halt::Stop) => {
                out.stopped = true;
                false
            }
            Err(Halt::Failed) => {
                out.failed = true;
                false
            }
        }
    }

    fn look_up(&self, want: &Want) -> Outcome {
        let mut out = Outcome::default();
        if let Some(stores) = &self.services.stores {
            self.ask_store(stores, Store::Own, want, &mut out);
        }
        for &source in &self.services.order {
            if out.stopped {
                break;
            }
            match (Store::of(source), &self.services.stores, &self.services.steamgriddb) {
                (Some(store), Some(stores), _) => self.ask_store(stores, store, want, &mut out),
                (None, _, Some(service)) => self.ask_steamgriddb(service, want, &mut out),
                _ => {}
            }
        }
        if !out.stopped {
            self.say(want, &out);
        }
        out
    }

    /// The game's line: `art: steam 620: cover Steam, hero SteamGridDB, header Steam, square none`, each kind asked
    /// for in this run, with where it came from, `none` when no source had it, or `failed` when a request for it
    /// did. Nothing for a game nothing was asked about.
    fn say(&self, want: &Want, out: &Outcome) {
        let mut tried: Vec<Kind> = Vec::new();
        for kind in Kind::ALL.into_iter().filter(|kind| out.tried.contains(kind)) {
            tried.push(kind);
        }
        if tried.is_empty() {
            return;
        }
        let parts: Vec<String> = tried
            .into_iter()
            .map(|kind| {
                let from = out.got.iter().find(|(k, _)| *k == kind).map(|(_, from)| *from);
                let from = from.unwrap_or(if out.failed { "failed" } else { "none" });
                format!("{} {from}", kind.name())
            })
            .collect();
        (self.log)(&format!("art: {} {}: {}", want.source, want.source_id, parts.join(", ")));
    }

    /// A store's art online, for what it may have of what the game still lacks: by the game's own id there, else
    /// found by its name.
    fn ask_store(&self, stores: &StoreArt, store: Store, want: &Want, out: &mut Outcome) {
        let offered = stores.offers(store, want);
        let kinds: Vec<Kind> = want.kinds.iter().copied().filter(|kind| offered.contains(kind)).collect();
        let kinds = self.still_wanted(want, &kinds, store.online());
        if kinds.is_empty() {
            return;
        }
        out.asked = true;
        out.tried.extend(&kinds);
        let missed = |kinds: &[Kind]| kinds.iter().for_each(|&kind| self.miss(want, kind, store.online()));
        let mut id = stores.own_id(store, want);
        let mut item = None;
        if id.is_none() && store != Store::Own {
            let Some(search) = stores.search(store, want) else { return missed(&kinds) };
            match self.ask(search.name, || stores.get(&search)) {
                Ok(Some(answer)) => match stores.matched(store, want, &answer) {
                    Some(found) => {
                        id = Some(found.id);
                        item = found.item;
                    }
                    None => return missed(&kinds),
                },
                Ok(None) => return missed(&kinds),
                Err(Halt::Stop) => {
                    out.stopped = true;
                    return;
                }
                Err(Halt::Failed) => {
                    out.failed = true;
                    return;
                }
            }
        }
        let answer = match (item, stores.ask(store, want, id.as_deref(), &kinds)) {
            (Some(item), _) => Some(item),
            (None, None) => None,
            (None, Some(ask)) => match self.ask(ask.name, || stores.get(&ask)) {
                Ok(answer) => answer,
                Err(Halt::Stop) => {
                    out.stopped = true;
                    return;
                }
                Err(Halt::Failed) => {
                    out.failed = true;
                    return;
                }
            },
        };
        let pictures = stores.pictures(store, want, &kinds, answer.as_ref());
        for kind in kinds {
            match pictures.iter().find(|(k, _)| *k == kind) {
                Some((_, url)) => {
                    if self.fetch(want, kind, url, store.online(), out) {
                        out.store_online += 1;
                        out.got.push((kind, store.name()));
                    }
                }
                None => self.miss(want, kind, store.online()),
            }
            if out.stopped {
                return;
            }
        }
    }

    /// SteamGridDB, for what the game still lacks.
    fn ask_steamgriddb(&self, service: &Service, want: &Want, out: &mut Outcome) {
        let kinds = self.still_wanted(want, &want.kinds, Online::SteamGridDb);
        if kinds.is_empty() {
            return;
        }
        out.asked = true;
        out.tried.extend(&kinds);
        let game = match self.ask(NAME, || service.find_game(&want.source, &want.source_id, &want.name)) {
            Ok(Some(game)) => game,
            Ok(None) => {
                kinds.into_iter().for_each(|kind| self.miss(want, kind, Online::SteamGridDb));
                return;
            }
            Err(Halt::Stop) => {
                out.stopped = true;
                return;
            }
            Err(Halt::Failed) => {
                out.failed = true;
                return;
            }
        };
        for kind in kinds {
            let url = match self.ask(NAME, || service.picture(game, kind)) {
                Ok(Some(url)) => url,
                Ok(None) => {
                    self.miss(want, kind, Online::SteamGridDb);
                    continue;
                }
                Err(Halt::Stop) => {
                    out.stopped = true;
                    return;
                }
                Err(Halt::Failed) => {
                    out.failed = true;
                    continue;
                }
            };
            if self.fetch(want, kind, &url, Online::SteamGridDb, out) {
                out.steamgriddb += 1;
                out.got.push((kind, NAME));
            }
            if out.stopped {
                return;
            }
        }
    }

    /// Makes a request of `host` when its gate lets it, again while it says to wait.
    fn ask<T>(&self, host: &str, request: impl Fn() -> Result<T, Error>) -> Result<T, Halt> {
        for _ in 0..self.pace.tries.max(1) {
            if !self.pass_gate(host) {
                return Err(Halt::Stop);
            }
            match request() {
                Ok(answer) => {
                    if let Some(gate) = self.gates.lock().unwrap().get_mut(host) {
                        gate.busy = 0;
                    }
                    return Ok(answer);
                }
                Err(Error::Busy(after)) => self.busy(host, after),
                Err(Error::KeyRefused) => {
                    self.stop.store(true, Ordering::Relaxed);
                    if !self.refused.swap(true, Ordering::Relaxed) {
                        (self.log)("art: SteamGridDB refused the key: no more lookups until it is changed");
                        (self.tell)(Event::KeyRefused);
                    }
                    return Err(Halt::Stop);
                }
                Err(Error::Failed(why)) => {
                    self.failed(&why);
                    return Err(Halt::Failed);
                }
            }
        }
        self.failed(&format!("{host} kept asking to wait"));
        Err(Halt::Failed)
    }

    /// Waits for the host's gate to open, and closes it behind; false if the run stopped meanwhile.
    fn pass_gate(&self, host: &str) -> bool {
        loop {
            if self.stopped() {
                return false;
            }
            let wait = {
                let mut gates = self.gates.lock().unwrap();
                let now = Instant::now();
                let gate = gates.entry(host.to_string()).or_insert(Gate { next: now, busy: 0 });
                if gate.next <= now {
                    gate.next = now + self.pace.gap;
                    return true;
                }
                gate.next - now
            };
            std::thread::sleep(wait.min(Duration::from_millis(100)));
        }
    }

    /// The host said to wait: every worker waits what it said before asking it again, or the wait doubled each
    /// time in a row.
    fn busy(&self, host: &str, after: Option<Duration>) {
        let mut gates = self.gates.lock().unwrap();
        let gate = gates.entry(host.to_string()).or_insert(Gate { next: Instant::now(), busy: 0 });
        gate.busy += 1;
        let doubled = self.pace.wait.saturating_mul(1 << (gate.busy - 1).min(16));
        // What it said the first time; after that, never less than the doubled wait.
        let wait = match after {
            Some(after) if gate.busy == 1 => after,
            Some(after) => after.max(doubled),
            None => doubled,
        }
        .min(self.pace.most_wait);
        gate.next = gate.next.max(Instant::now() + wait);
        (self.log)(&format!("art: {host} asked to wait; waiting {:.0} s", wait.as_secs_f64()));
    }

    fn failed(&self, why: &str) {
        if !self.failed_said.swap(true, Ordering::Relaxed) {
            (self.log)(&format!("art: {why}"));
        }
    }
}

/// A picture's host, by its address: what its gate is kept by. Never more of the address than the host's name.
pub(crate) fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pictures_host_is_its_name_alone() {
        assert_eq!(
            host("https://shared.steamstatic.com/store_item_assets/steam/apps/620/x.jpg?t=1"),
            "shared.steamstatic.com"
        );
        assert_eq!(host("http://127.0.0.1:4000/pictures/1/cover.png"), "127.0.0.1:4000");
    }
}
