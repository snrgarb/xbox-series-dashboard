//! Looking up the art a store gave none for, off the UI thread: a game's missing pictures are taken from our own
//! cache if it has them, else asked of SteamGridDB, a few games at a time, and kept in the cache for every start
//! after. What was looked for and not found is remembered for a week, so a start does not ask again.
//!
//! The service is asked politely: one request at a time per worker, two workers, a gap between any two requests,
//! and when it says to wait (429, `Retry-After`) every worker waits that long, longer each time it says so again.
//! A key it refuses (401) stops every lookup at once, and is told once.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::Library;

use super::cache::{ArtDir, Kind};
use super::steamgriddb::{Error, Service};

/// A game whose store gave it less than every picture, and which of them it lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Want {
    pub source: String,
    pub source_id: String,
    pub name: String,
    pub kinds: Vec<Kind>,
}

/// The games of the library that lack a picture, in its order (the most recently played first, so what Home shows
/// comes first). A made-up game, of no source, is not looked up.
pub fn wanted(library: &Library) -> Vec<Want> {
    library
        .games
        .iter()
        .filter(|game| !game.source.is_empty() && !game.source_id.is_empty())
        .filter_map(|game| {
            let has = [game.cover.is_some(), game.hero.is_some(), game.header.is_some()];
            let kinds: Vec<Kind> = Kind::ALL.into_iter().zip(has).filter(|(_, has)| !has).map(|(k, _)| k).collect();
            (!kinds.is_empty()).then(|| Want {
                source: game.source.clone(),
                source_id: game.source_id.clone(),
                name: game.name.clone(),
                kinds,
            })
        })
        .collect()
}

/// How hard the service is asked.
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    /// Games looked up at once.
    pub workers: usize,
    /// The least time between two requests, whichever worker makes them.
    pub gap: Duration,
    /// The wait when the service says to wait and not for how long; doubled each time it says so again in a row.
    pub wait: Duration,
    /// The longest wait.
    pub most_wait: Duration,
    /// How many times one request is made while the service says to wait, before the game is passed over.
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
    /// The service refused the key: nothing more is asked until it changes.
    KeyRefused,
    /// The run is over.
    Done(Summary),
}

/// What a run did, in counts: what the log says of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Games the service was asked about.
    pub looked_up: usize,
    /// Of those, the games that got a picture.
    pub found: usize,
    /// The games it had nothing for.
    pub none: usize,
    /// The games a request failed for, to be asked about again next time.
    pub failed: usize,
    pub millis: u128,
}

impl std::fmt::Display for Summary {
    /// `12 looked up, 9 found, 3 none, in 2.1 s`, with `, 1 failed` before the time when one did.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} looked up, {} found, {} none", self.looked_up, self.found, self.none)?;
        if self.failed > 0 {
            write!(f, ", {} failed", self.failed)?;
        }
        write!(f, ", in {:.1} s", self.millis as f64 / 1000.0)
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
        service: Service,
        wants: Vec<Want>,
        now: u64,
        pace: Pace,
        log: Log,
        tell: impl Fn(Event) + Send + Sync + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let run = Run::new(dir, service, wants, now, pace, log, Box::new(tell), stop.clone());
        std::thread::spawn(move || run.run());
        Self { stop }
    }

    /// Looks up `wants` here and now, and gives what it did: for a picture drawn offscreen, and tests.
    pub fn run_now(dir: ArtDir, service: Service, wants: Vec<Want>, now: u64, pace: Pace, log: Log) -> Summary {
        let run = Run::new(dir, service, wants, now, pace, log, Box::new(|_| {}), Arc::new(AtomicBool::new(false)));
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

/// When the next request may be made, and how many times in a row the service has said to wait.
struct Gate {
    next: Instant,
    busy: u32,
}

struct Run {
    dir: ArtDir,
    service: Service,
    queue: Mutex<VecDeque<Want>>,
    gate: Mutex<Gate>,
    now: u64,
    pace: Pace,
    log: Log,
    tell: Box<dyn Fn(Event) + Send + Sync>,
    stop: Arc<AtomicBool>,
    refused: AtomicBool,
    /// The first failure's words: said once a run.
    failed_said: AtomicBool,
    summary: Mutex<Summary>,
}

/// Why a request was not answered.
enum Halt {
    /// The run is stopping: the key was refused, or it was told to.
    Stop,
    Failed,
}

/// How one game went.
enum Outcome {
    /// Our cache had everything, or it was all missed lately: nothing was asked.
    Cached,
    Found,
    None,
    Failed,
    Stopped,
}

impl Run {
    #[allow(clippy::too_many_arguments)]
    fn new(
        dir: ArtDir,
        service: Service,
        wants: Vec<Want>,
        now: u64,
        pace: Pace,
        log: Log,
        tell: Box<dyn Fn(Event) + Send + Sync>,
        stop: Arc<AtomicBool>,
    ) -> Self {
        Self {
            dir,
            service,
            queue: Mutex::new(wants.into()),
            gate: Mutex::new(Gate { next: Instant::now(), busy: 0 }),
            now,
            pace,
            log,
            tell,
            stop,
            refused: AtomicBool::new(false),
            failed_said: AtomicBool::new(false),
            summary: Mutex::new(Summary::default()),
        }
    }

    fn run(self) -> Summary {
        let started = Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..self.pace.workers.max(1) {
                scope.spawn(|| self.work());
            }
        });
        let mut summary = self.summary.lock().unwrap().clone();
        summary.millis = started.elapsed().as_millis();
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
            let mut summary = self.summary.lock().unwrap();
            match outcome {
                Outcome::Cached | Outcome::Stopped => {}
                Outcome::Found => {
                    summary.looked_up += 1;
                    summary.found += 1;
                }
                Outcome::None => {
                    summary.looked_up += 1;
                    summary.none += 1;
                }
                Outcome::Failed => {
                    summary.looked_up += 1;
                    summary.failed += 1;
                }
            }
        }
    }

    fn look_up(&self, want: &Want) -> Outcome {
        let (source, id) = (want.source.as_str(), want.source_id.as_str());
        let kinds: Vec<Kind> = want
            .kinds
            .iter()
            .copied()
            .filter(|&kind| self.dir.find(source, id, kind).is_none() && !self.dir.missed(source, id, kind, self.now))
            .collect();
        if kinds.is_empty() {
            return Outcome::Cached;
        }
        let miss = |kind| {
            if let Err(e) = self.dir.miss(source, id, kind, self.now) {
                self.failed(&format!("the art folder: {e:#}"));
            }
        };
        let game = match self.ask(|| self.service.find_game(source, id, &want.name)) {
            Ok(Some(game)) => game,
            Ok(None) => {
                kinds.into_iter().for_each(miss);
                return Outcome::None;
            }
            Err(Halt::Stop) => return Outcome::Stopped,
            Err(Halt::Failed) => return Outcome::Failed,
        };
        let (mut found, mut failed) = (false, false);
        for kind in kinds {
            let url = match self.ask(|| self.service.picture(game, kind)) {
                Ok(Some(url)) => url,
                Ok(None) => {
                    miss(kind);
                    continue;
                }
                Err(Halt::Stop) => return Outcome::Stopped,
                Err(Halt::Failed) => {
                    failed = true;
                    continue;
                }
            };
            match self.ask(|| self.service.download(&url)) {
                Ok(bytes) => match self.dir.save(source, id, kind, &bytes) {
                    Ok(_) => found = true,
                    // Not a picture this program draws: as good as none.
                    Err(_) => miss(kind),
                },
                Err(Halt::Stop) => return Outcome::Stopped,
                Err(Halt::Failed) => failed = true,
            }
        }
        if found {
            (self.tell)(Event::Found { source: source.into(), source_id: id.into() });
            Outcome::Found
        } else if failed {
            Outcome::Failed
        } else {
            Outcome::None
        }
    }

    /// Makes a request when the gate lets it, again while the service says to wait.
    fn ask<T>(&self, request: impl Fn() -> Result<T, Error>) -> Result<T, Halt> {
        for _ in 0..self.pace.tries.max(1) {
            if !self.pass_gate() {
                return Err(Halt::Stop);
            }
            match request() {
                Ok(answer) => {
                    self.gate.lock().unwrap().busy = 0;
                    return Ok(answer);
                }
                Err(Error::Busy(after)) => self.busy(after),
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
        self.failed("SteamGridDB kept asking to wait");
        Err(Halt::Failed)
    }

    /// Waits for the gate to open, and closes it behind; false if the run stopped meanwhile.
    fn pass_gate(&self) -> bool {
        loop {
            if self.stopped() {
                return false;
            }
            let wait = {
                let mut gate = self.gate.lock().unwrap();
                let now = Instant::now();
                if gate.next <= now {
                    gate.next = now + self.pace.gap;
                    return true;
                }
                gate.next - now
            };
            std::thread::sleep(wait.min(Duration::from_millis(100)));
        }
    }

    /// The service said to wait: every worker waits what it said, or the wait doubled each time in a row.
    fn busy(&self, after: Option<Duration>) {
        let mut gate = self.gate.lock().unwrap();
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
        (self.log)(&format!("art: SteamGridDB asked to wait; waiting {:.0} s", wait.as_secs_f64()));
    }

    fn failed(&self, why: &str) {
        if !self.failed_said.swap(true, Ordering::Relaxed) {
            (self.log)(&format!("art: {why}"));
        }
    }
}
