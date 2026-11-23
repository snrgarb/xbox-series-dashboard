//! The installs the Dashboard has asked the Agent for, each followed on a thread of its own once a second, for the
//! Queue (`Source::installs` reads what they last said and asks nothing).
//!
//! **Where the progress is read** (#172, after the TV PC's real install, 2026-11-18): every path the Agent named
//! (`calls::progress_paths`: `/update/<uid>` first, then `/install/<uid>`, which is 404 `{}` from the finalized post
//! on) and the game's own (`calls::game_path`, `/game/<uid>`). The first progress path that says anything is read;
//! the first answer of each path is logged whole, and any change of its status, so that one real run shows where the
//! Agent says what.
//!
//! **When an install is over** ([`Judge`]), and only then: `product.db` says installed, playable and its update
//! complete (done: `product::Product::done`; **playable is not done**: Blizzard games stream-install, so one that is
//! playable but not update complete is followed while its download moves, [`taken_as_done`] only once nothing has moved
//! for [`STALLED_PLAYABLE`]; installed alone is taken as done only once it has stayed so, quiet, for
//! [`SETTLE`]), after which the Agent is given time to finish ([`settle`]); the Agent names an
//! error (failed, its code shown); the user cancels; or every path, the game's too, has said nothing for [`SILENT`]
//! with the Agent's process gone (or for [`FORGOTTEN`] with it there). **A 404 on one path is never by itself the
//! end**: on the TV PC that was taken for a stop, and the Agent the Dashboard started, and its download, killed.
//! The follower holds the session, and with it the Agent the Dashboard started (`agent::Lease`), until then.
//!
//! An install that fails or falls silent is told to the user with why (`Opener::stopped`, `Stop::Failed`): the
//! Queue's stopped row and its Toast, and a log line, never only "stopped".
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::{Game, Install, InstallState, Opener, Stop};
use serde_json::Value;

use crate::agent::{self, calls, Place, Session};
use crate::http::{masked, Answer};
use crate::product::Product;

/// How often an install's progress is asked.
pub const EVERY: Duration = Duration::from_secs(1);

/// How long `product.db` is waited on to say installed once the Agent says the download is done.
const SETTLE: Duration = Duration::from_secs(120);

/// How long every path may say nothing, the Agent's process gone, before the install is taken to have stopped.
pub const SILENT: Duration = Duration::from_secs(60);

/// How long every path may say nothing with the Agent still there before the install is taken to be forgotten.
pub const FORGOTTEN: Duration = Duration::from_secs(15 * 60);

/// How often, in rounds, `product.db` is looked at while nothing says the download is done, and whether Battle.net
/// has started beside the Agent the Dashboard started.
const LOOK_EVERY: u32 = 10;

#[derive(Debug, Clone)]
struct Job {
    uid: String,
    state: InstallState,
    /// Bits a second, as the Queue takes a store's speed.
    speed: Option<u64>,
    /// The user cancelled it: the follower stops.
    stopped: bool,
    /// The path its progress is read on, once one says anything: where pause and cancel are asked.
    live: Option<String>,
}

/// What is under way, by product code.
#[derive(Debug, Clone, Default)]
pub struct Jobs(Arc<Mutex<HashMap<String, Job>>>);

impl Jobs {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The Queue's word on each.
    pub fn installs(&self) -> Vec<Install> {
        let mut installs: Vec<Install> = self
            .lock()
            .iter()
            .map(|(code, job)| Install { source_id: code.clone(), state: job.state, speed: job.speed })
            .collect();
        installs.sort_by(|a, b| a.source_id.cmp(&b.source_id));
        installs
    }

    /// Takes on `code`'s install, Queued until the Agent says more. False where it is under way already.
    pub fn begin(&self, code: &str, uid: &str) -> bool {
        let mut jobs = self.lock();
        if jobs.contains_key(code) {
            return false;
        }
        let job = Job { uid: uid.into(), state: InstallState::Queued, speed: None, stopped: false, live: None };
        jobs.insert(code.into(), job);
        true
    }

    pub fn has(&self, code: &str) -> bool {
        self.lock().contains_key(code)
    }

    pub fn uid(&self, code: &str) -> Option<String> {
        self.lock().get(code).map(|job| job.uid.clone())
    }

    pub fn paused(&self, code: &str) -> bool {
        self.lock().get(code).is_some_and(|job| job.state.paused())
    }

    /// The path its progress is read on, where one has said anything yet.
    pub fn live(&self, code: &str) -> Option<String> {
        self.lock().get(code).and_then(|job| job.live.clone())
    }

    /// Forgets it: the Queue drops it, or reads it installed.
    pub fn end(&self, code: &str) {
        self.lock().remove(code);
    }

    /// Stops following it, and forgets it.
    pub fn stop(&self, code: &str) {
        if let Some(job) = self.lock().get_mut(code) {
            job.stopped = true;
        }
        self.end(code);
    }

    fn stopped(&self, code: &str) -> bool {
        self.lock().get(code).is_none_or(|job| job.stopped)
    }

    fn set(&self, code: &str, state: InstallState, speed: Option<u64>) {
        if let Some(job) = self.lock().get_mut(code) {
            (job.state, job.speed) = (state, speed);
        }
    }

    fn set_live(&self, code: &str, live: &str) {
        if let Some(job) = self.lock().get_mut(code) {
            job.live = Some(live.to_string());
        }
    }
}

/// What the Queue reads of an answer: Queued until it has bytes, Paused where it says so, else Downloading.
pub fn state(progress: &calls::Progress) -> (InstallState, Option<u64>) {
    let bytes = match (progress.done, progress.total) {
        (Some(done), Some(total)) if total > 0 => Some((done.min(total), total)),
        // Only a share: a whole of a million, so the Queue's share is right.
        _ => progress.fraction.filter(|f| *f > 0.0).map(|f| ((f * 1e6) as u64, 1_000_000)),
    };
    match bytes {
        Some((done, total)) if progress.paused => (InstallState::Paused { done, total }, None),
        Some((done, total)) => (InstallState::Downloading { done, total }, progress.rate.map(|bytes| bytes * 8)),
        None if progress.paused => (InstallState::Paused { done: 0, total: 0 }, None),
        None => (InstallState::Queued, None),
    }
}

/// What the Dashboard knows of an install the Agent took, to follow it by.
#[derive(Debug, Clone)]
pub struct Watch {
    pub code: String,
    /// Where its progress is read, in order (`calls::progress_paths`).
    pub paths: Vec<String>,
    /// The game's own (`calls::game_path`).
    pub game_path: String,
    /// The form's `required_space`, bytes: the whole, for an answer with only a share.
    pub required: Option<u64>,
    /// Its folder, where the Agent made it (it was not there before the install): taken away on Cancel, once the
    /// Agent the Dashboard started is stopped.
    pub made_dir: Option<PathBuf>,
}

/// How a round of answers came out.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Under way: what it says, merged from its progress path and the game's, and the answer read, as the log says it.
    Going { progress: calls::Progress, said: String },
    /// The Agent named an error: its code, the path, and the answer as the log says it.
    Failed { error: i64, path: String, said: String },
    /// Nothing has said anything of it for this long, the Agent's process there or not.
    Silent { for_: Duration, agent_there: bool },
}

/// What is kept of one path between rounds.
#[derive(Debug, Clone, Default)]
struct Path {
    /// Its last status, as the log says it (`200`, `404`, `no answer`): none before its first answer.
    last: Option<String>,
    /// Whether it has answered 2xx.
    answered: bool,
}

/// Judges each round of answers for one install: what is logged, which path is read, and whether it is over. A
/// 404, or no answer, on a path is never by itself the end.
#[derive(Debug)]
pub struct Judge {
    code: String,
    required: Option<u64>,
    game_path: String,
    paths: HashMap<String, Path>,
    /// The path read last, once one has said anything.
    live: Option<String>,
    /// When any path last said anything.
    heard: Instant,
    /// The lines to log, drained by the follower.
    pub lines: Vec<String>,
}

fn status_of(result: &Result<Answer, String>) -> String {
    match result {
        Ok(answer) => answer.status.to_string(),
        Err(_) => "no answer".to_string(),
    }
}

fn shown(result: &Result<Answer, String>) -> String {
    match result {
        Ok(answer) => format!("{} {}", answer.status, masked(&answer.body)),
        Err(why) => format!("no answer ({why})"),
    }
}

impl Judge {
    pub fn new(watch: &Watch, now: Instant) -> Self {
        Self {
            code: watch.code.clone(),
            required: watch.required,
            game_path: watch.game_path.clone(),
            paths: HashMap::new(),
            live: None,
            heard: now,
            lines: Vec::new(),
        }
    }

    /// The path read last.
    pub fn live(&self) -> Option<&str> {
        self.live.as_deref()
    }

    /// One round: each progress path's answer in order, the game's, and whether the Agent is still there (asked
    /// only once everything has been silent a while).
    pub fn round(
        &mut self,
        now: Instant,
        progress: &[(String, Result<Answer, String>)],
        game: &Result<Answer, String>,
        agent_there: &mut dyn FnMut() -> bool,
    ) -> Verdict {
        let code = self.code.clone();
        let game_path = self.game_path.clone();
        let mut reading: Option<(String, calls::Progress, Value)> = None;
        let mut from_game: Option<(calls::Progress, Value)> = None;
        let all = progress.iter().map(|(path, result)| (path.clone(), result)).chain([(game_path.clone(), game)]);
        for (path, result) in all {
            let record = self.paths.entry(path.clone()).or_default();
            let status = status_of(result);
            match &record.last {
                None => self.lines.push(format!("{code}: {path}: first answer: {}", shown(result))),
                Some(last) if *last != status => {
                    self.lines.push(format!("{code}: {path} now answers {}", shown(result)))
                }
                _ => {}
            }
            record.last = Some(status);
            let Ok(answer) = result else { continue };
            let read = calls::read_progress(answer, self.required);
            // An error the Agent names: on a 2xx answer, or on a path that has answered 2xx before. A path that has
            // never answered may name one only for not being where this install is.
            if let Some(error) = read.error.filter(|_| answer.ok() || record.answered) {
                return Verdict::Failed { error, path, said: masked(&answer.body).to_string() };
            }
            if !answer.ok() {
                continue;
            }
            record.answered = true;
            if !read.says_anything() {
                continue;
            }
            if path == game_path {
                from_game = Some((read, answer.body.clone()));
            } else if reading.is_none() {
                reading = Some((path, read, answer.body.clone()));
            }
        }
        if reading.is_none() && from_game.is_none() {
            let for_ = now.saturating_duration_since(self.heard);
            if for_ >= SILENT {
                let there = agent_there();
                if !there || for_ >= FORGOTTEN {
                    return Verdict::Silent { for_, agent_there: there };
                }
            }
            return Verdict::Going { progress: calls::Progress::default(), said: "nothing said yet".into() };
        }
        self.heard = now;
        let (path, mut progress, body) = match (&reading, &from_game) {
            (Some((path, read, body)), _) => (path.clone(), *read, body.clone()),
            (None, Some((read, body))) => (game_path.clone(), *read, body.clone()),
            (None, None) => unreachable!(),
        };
        if let Some((game, _)) = &from_game {
            progress.installed = game.installed.or(progress.installed);
            progress.playable |= game.playable;
            progress.complete |= game.complete;
        }
        if self.live.as_deref() != Some(path.as_str()) {
            self.lines.push(format!("{code}: progress read on {path}"));
            self.live = Some(path.clone());
        }
        Verdict::Going { progress, said: format!("{path} said {}", masked(&body)) }
    }
}

/// What the user is told of an install that stopped by itself: the Queue's row and the Toast's line.
pub fn told(verdict: &Verdict) -> Option<String> {
    match verdict {
        Verdict::Failed { error, .. } => {
            Some(format!("Battle.net's Agent stopped it with {}.", calls::error_said(*error)))
        }
        Verdict::Silent { agent_there: false, .. } => {
            Some("Battle.net's Agent stopped answering. Install it again, or finish it in Battle.net.".into())
        }
        Verdict::Silent { agent_there: true, .. } => {
            Some("Battle.net's Agent stopped reporting it. Install it again, or finish it in Battle.net.".into())
        }
        Verdict::Going { .. } => None,
    }
}

/// Follows an install the Agent took on a thread of its own until it is over (see the module), holding `session`,
/// and with it the Agent the Dashboard started, until then. `installed` says whether `product.db` says it is
/// installed now; `open` tells the user why where it stops by itself.
pub fn follow(
    jobs: Jobs,
    mut session: Session,
    place: Place,
    watch: Watch,
    game: Game,
    mut open: Box<dyn Opener + Send>,
    product: impl Fn() -> Option<Product> + Send + 'static,
) {
    std::thread::spawn(move || {
        let code = watch.code.clone();
        let mut judge = Judge::new(&watch, Instant::now());
        let mut said: Option<InstallState> = None;
        let mut done_at: Option<Instant> = None;
        // Since when `product.db` has said installed, and whether that has been said.
        let mut installed_at: Option<Instant> = None;
        let mut installed_said = false;
        let mut client_said = false;
        let mut round: u32 = 0;
        // What the progress last said, and since when it has said anything else (bytes, share or rate moving).
        let mut last_seen: Option<Seen> = None;
        let mut moved_at = Instant::now();
        loop {
            std::thread::sleep(EVERY);
            round = round.wrapping_add(1);
            if jobs.stopped(&code) {
                cancelled(session, &place, &watch);
                return;
            }
            let answers: Vec<(String, Result<Answer, String>)> =
                watch.paths.iter().map(|path| (path.clone(), session.get_silent(path))).collect();
            let game_answer = session.get_silent(&watch.game_path);
            let verdict = {
                let session = &session;
                let mut there = || session.agent_alive(&place.dir);
                judge.round(Instant::now(), &answers, &game_answer, &mut there)
            };
            for line in judge.lines.drain(..) {
                crate::log(&session.http.mask(&line));
            }
            if let Some(live) = judge.live() {
                if jobs.live(&code).as_deref() != Some(live) {
                    jobs.set_live(&code, live);
                }
            }
            let progress = match &verdict {
                Verdict::Going { progress, said: answer } => {
                    let (state, speed) = state(progress);
                    // A line when the state's kind changes, and every tenth of the way, with the answer read.
                    let tenth = |s: InstallState| (std::mem::discriminant(&s), s.percent().map(|p| p / 10));
                    // A round in which nothing said anything leaves the Queue's word as it was.
                    if progress.says_anything() {
                        let now_seen = Seen::of(progress);
                        if last_seen != Some(now_seen) || progress.rate.unwrap_or(0) > 0 {
                            moved_at = Instant::now();
                        }
                        last_seen = Some(now_seen);
                        if said.is_none_or(|before| tenth(before) != tenth(state)) {
                            let line = format!("{code}: {}; {answer}", say(&state, progress.rate));
                            crate::log(&session.http.mask(&line));
                            said = Some(state);
                        }
                        jobs.set(&code, state, speed);
                    }
                    *progress
                }
                stopped => {
                    let why = match stopped {
                        Verdict::Failed { error, path, said } => {
                            format!("the Agent names {} on {path} ({said})", calls::error_said(*error))
                        }
                        Verdict::Silent { for_, agent_there: false } => format!(
                            "its Agent stopped answering: no path has said anything of it for {} s, and the Agent's \
                             process is gone",
                            for_.as_secs()
                        ),
                        _ => format!(
                            "the Agent still runs and no path has said anything of it for {} min",
                            FORGOTTEN.as_secs() / 60
                        ),
                    };
                    let told = told(stopped).unwrap_or_default();
                    crate::log(&session.http.mask(&format!("{code}: stopped: {why}: told \"{told}\"")));
                    // Told before it is forgotten, so the Queue has why before it sees the install gone.
                    open.stopped(&game, &Stop::Failed(told));
                    jobs.end(&code);
                    return;
                }
            };
            // `product.db` looked at each round once the Agent says the download is done, else every tenth round.
            let done = progress.says_done();
            if done || round.is_multiple_of(LOOK_EVERY) {
                if let Some(product) = product().filter(|p| p.installed) {
                    let since = *installed_at.get_or_insert_with(Instant::now);
                    if product.done() || taken_as_done(&product, since.elapsed(), moved_at.elapsed()) {
                        let how = if product.done() {
                            String::new()
                        } else {
                            format!(
                                " (taken as done: still so, and nothing moving, {} s on)",
                                moved_at.elapsed().as_secs()
                            )
                        };
                        crate::log(&format!("{code}: done: product.db says {}{how}", product.said()));
                        settle(&mut session, &watch, moved_at);
                        jobs.end(&code);
                        return;
                    }
                    if !installed_said {
                        installed_said = true;
                        crate::log(&format!(
                            "{code}: product.db says {}: not done until it is playable and its update complete",
                            product.said()
                        ));
                    }
                }
            }
            if done {
                let since = *done_at.get_or_insert_with(Instant::now);
                if since.elapsed() > SETTLE && installed_at.is_none() {
                    let told = "Battle.net's Agent finished the download, and Battle.net doesn't list the game as \
                                installed. Look for it in Battle.net.";
                    crate::log(&format!(
                        "{code}: stopped: the Agent says the download is done and product.db does not say installed \
                         after {} s: told \"{told}\"",
                        SETTLE.as_secs()
                    ));
                    open.stopped(&game, &Stop::Failed(told.into()));
                    jobs.end(&code);
                    return;
                }
            }
            // Battle.net started meanwhile: the Agent the Dashboard started is left running until the install is over.
            if session.ours() && round.is_multiple_of(LOOK_EVERY) {
                let others = place.others_running(agent::ours(&place.dir));
                if !others.is_empty() && !client_said {
                    crate::log(&format!(
                        "{code}: {} started while the Agent the Dashboard started downloads it: that Agent is left \
                         running until the install is over (Battle.net may take the install over: barncastle's \
                         warning); product.db says when it is done",
                        others.join(", ")
                    ));
                }
                client_said = !others.is_empty();
            }
        }
    });
}

/// What a progress says that changes as the download moves.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seen {
    done: Option<u64>,
    fraction: Option<f64>,
}

impl Seen {
    fn of(progress: &calls::Progress) -> Self {
        Self { done: progress.done, fraction: progress.fraction }
    }
}

/// How long a game `product.db` lists as installed and playable but not update complete may go with nothing moving
/// before it is taken as done all the same. Blizzard's games are playable early (stream installs: World of Warcraft at
/// about 10% of 132 GB), so "playable" is not "done": the update-complete flag is, and this is only the way out for
/// a game whose Agent never sets it.
pub const STALLED_PLAYABLE: Duration = Duration::from_secs(15 * 60);

/// Whether an install `product.db` lists as installed, not (all of) done, is taken as done: it has stayed so for
/// [`SETTLE`] and nothing has moved for as long (a game not yet playable), or, where it is playable, for
/// [`STALLED_PLAYABLE`]: a download that keeps moving is never taken as done.
fn taken_as_done(product: &Product, installed_for: Duration, quiet_for: Duration) -> bool {
    let patience = if product.playable { STALLED_PLAYABLE } else { SETTLE };
    installed_for >= patience && quiet_for >= patience
}

/// How long, once `product.db` says the install is done, the Agent is waited on to be done with it too.
pub const IDLE_WAIT: Duration = Duration::from_secs(60);

/// An install is done: before the session is let go (and with it the Agent the Dashboard started, `DELETE /agent`),
/// the Agent is waited on, [`IDLE_WAIT`] at most, to say the game is installed and playable on its own path
/// (`/game/<uid>`) and to have nothing more under way on the install's (each `404 {}`, says nothing, or is idle:
/// `calls::Progress::idle`); then the Agent is given [`agent::GRACE_AFTER_INSTALL`] to go by itself.
/// While the Agent's paths keep moving it is waited on however long that takes (a stream install goes on after
/// `product.db` says playable): [`IDLE_WAIT`] counts from when they last changed.
fn settle(session: &mut Session, watch: &Watch, moved_at: Instant) {
    let code = &watch.code;
    let mut begun = moved_at.min(Instant::now());
    let mut last: Vec<Seen> = Vec::new();
    loop {
        let read =
            |answer: Result<Answer, String>| answer.ok().filter(Answer::ok).map(|a| calls::read_progress(&a, None));
        let game = read(session.get_silent(&watch.game_path));
        let ready = game.is_some_and(|game| game.ready());
        let paths: Vec<Option<calls::Progress>> =
            watch.paths.iter().map(|path| read(session.get_silent(path))).collect();
        let idle = paths.iter().all(|p| p.is_none_or(|progress| !progress.says_anything() || progress.idle()));
        let seen: Vec<Seen> = paths.iter().flatten().chain(game.iter()).map(Seen::of).collect();
        if seen != last || paths.iter().flatten().any(|p| p.rate.unwrap_or(0) > 0) {
            begun = Instant::now();
            last = seen;
        }
        if ready && idle {
            crate::log(&format!(
                "{code}: the Agent is done with it: {} says installed and playable, nothing under way, {} ms on",
                watch.game_path,
                begun.elapsed().as_millis()
            ));
            break;
        }
        if begun.elapsed() >= IDLE_WAIT {
            crate::log(&format!(
                "{code}: the Agent not seen done with it {} s on ({}: {}; under way: {}): let go all the same",
                IDLE_WAIT.as_secs(),
                watch.game_path,
                if ready { "installed and playable" } else { "not installed and playable" },
                if idle { "nothing" } else { "something" }
            ));
            break;
        }
        std::thread::sleep(EVERY);
    }
    session.install_over();
}

/// The user cancelled: the follower lets go of the session, which stops the Agent the Dashboard started (and with it
/// the download), then takes away the folder the Agent made for it.
fn cancelled(session: Session, place: &Place, watch: &Watch) {
    let code = &watch.code;
    let ours = session.ours() && agent::alive(&place.dir) == Some(true);
    crate::log(&format!(
        "{code}: cancelled: no longer followed{}",
        if ours { "; the Agent the Dashboard started is stopped, which ends its download" } else { "" }
    ));
    let was_ours = session.ours();
    drop(session);
    let Some(dir) = watch.made_dir.as_ref().filter(|dir| dir.exists()) else { return };
    if !was_ours {
        crate::log(&format!(
            "{code}: {} is left: the Agent is not the Dashboard's, and may still use it",
            dir.display()
        ));
        return;
    }
    match std::fs::remove_dir_all(dir) {
        Ok(()) => crate::log(&format!("{code}: {} taken away", dir.display())),
        Err(e) => crate::log(&format!("{code}: {} could not be taken away: {e}", dir.display())),
    }
}

fn say(state: &InstallState, rate: Option<u64>) -> String {
    match state {
        InstallState::Queued => "queued in the Agent".into(),
        InstallState::Paused { done, total } => format!("paused at {done} of {total} bytes"),
        InstallState::Downloading { done, total } => match rate {
            Some(rate) => format!("downloading, {done} of {total} bytes, {rate} bytes a second"),
            None => format!("downloading, {done} of {total} bytes"),
        },
        InstallState::Extras { .. } => "extras".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The TV PC's World of Warcraft (#338): `product.db` said installed and playable, not update complete, from
    /// about 10% of 132 GB on; taken as done after two minutes, and the Agent and its download stopped.
    #[test]
    fn a_game_playable_before_its_update_is_complete_is_not_done_while_it_moves() {
        let streaming = Product {
            uid: "wow".into(),
            code: "wow".into(),
            installed: true,
            playable: true,
            update_complete: false,
            ..Product::default()
        };
        assert!(!streaming.done());
        let min = |m: u64| Duration::from_secs(m * 60);
        // Moving (quiet for no time at all): never taken as done, however long it has been listed as installed.
        assert!(!taken_as_done(&streaming, min(600), Duration::ZERO));
        assert!(!taken_as_done(&streaming, SETTLE + Duration::from_secs(1), Duration::ZERO));
        // Quiet two minutes, as the TV PC's was while it still downloaded: not yet.
        assert!(!taken_as_done(&streaming, min(30), SETTLE));
        // Nothing moved for a long while: the way out for an Agent that never says it complete.
        assert!(taken_as_done(&streaming, min(30), STALLED_PLAYABLE));
        // Update complete: done, whatever else.
        assert!(Product { update_complete: true, ..streaming.clone() }.done());
        // Not yet playable: the shorter patience, as before.
        let early = Product { playable: false, ..streaming };
        assert!(taken_as_done(&early, SETTLE, SETTLE));
        assert!(!taken_as_done(&early, SETTLE, Duration::ZERO));
    }

    #[test]
    fn the_downloads_moving_is_seen_in_its_bytes_and_its_share() {
        let at = |done: u64| calls::Progress { done: Some(done), fraction: Some(0.1), ..Default::default() };
        assert_eq!(Seen::of(&at(5)), Seen::of(&at(5)));
        assert_ne!(Seen::of(&at(5)), Seen::of(&at(6)));
    }

    #[test]
    fn the_agents_word_is_the_queues() {
        let progress = |done, total, paused| calls::Progress {
            done: Some(done),
            total: Some(total),
            rate: Some(100),
            paused,
            ..calls::Progress::default()
        };
        assert_eq!(state(&progress(10, 100, false)), (InstallState::Downloading { done: 10, total: 100 }, Some(800)));
        assert_eq!(state(&progress(10, 100, true)), (InstallState::Paused { done: 10, total: 100 }, None));
        assert_eq!(state(&calls::Progress::default()), (InstallState::Queued, None));
        let share = calls::Progress { fraction: Some(0.5), ..calls::Progress::default() };
        assert_eq!(state(&share).0, InstallState::Downloading { done: 500_000, total: 1_000_000 });

        let jobs = Jobs::default();
        assert!(jobs.begin("emb", "emberwatch") && !jobs.begin("emb", "emberwatch"));
        assert_eq!(jobs.installs(), [Install::new("emb", InstallState::Queued)]);
        jobs.set("emb", InstallState::Paused { done: 1, total: 2 }, None);
        jobs.set_live("emb", "/update/emberwatch");
        assert!(jobs.paused("emb") && jobs.has("emb"));
        assert_eq!(jobs.live("emb").as_deref(), Some("/update/emberwatch"));
        jobs.stop("emb");
        assert!(jobs.installs().is_empty() && jobs.stopped("emb"));
    }

    fn answer(status: u16, body: Value) -> Result<Answer, String> {
        Ok(Answer { status, body })
    }

    fn watch() -> Watch {
        Watch {
            code: "lbra".into(),
            paths: vec!["/update/libra".into(), "/install/libra".into()],
            game_path: "/game/libra".into(),
            required: Some(57_000_000_000),
            made_dir: None,
        }
    }

    /// The TV PC's real install: `/install/libra` 404 `{}` once finalized. With the progress on `/update/libra`, it
    /// is read there; with nothing anywhere, it goes on until the Agent's process is gone a minute on.
    #[test]
    fn a_404_on_one_path_is_never_by_itself_the_end() {
        let begun = Instant::now();
        let mut judge = Judge::new(&watch(), begun);
        let gone = answer(404, json!({}));
        let update = answer(200, json!({"progress": 0.1, "state": 1003, "download_rate": 1000.0}));
        let game = answer(200, json!({"installed": false, "playable": false, "update_progress": 0.1}));
        let mut never = || -> bool { panic!("the Agent is asked after only once all is silent") };
        let paths = [("/update/libra".to_string(), update), ("/install/libra".to_string(), gone.clone())];
        let verdict = judge.round(begun, &paths, &game, &mut never);
        let Verdict::Going { progress, said } = verdict else { panic!("{verdict:?}") };
        assert_eq!((progress.done, progress.total), (Some(5_700_000_000), Some(57_000_000_000)));
        assert_eq!(progress.installed, Some(false));
        assert!(said.starts_with("/update/libra said {"), "{said}");
        assert_eq!(judge.live(), Some("/update/libra"));
        let lines = std::mem::take(&mut judge.lines);
        assert!(lines.contains(&"lbra: /install/libra: first answer: 404 {}".to_string()), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("lbra: /update/libra: first answer: 200 {")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("lbra: /game/libra: first answer: 200 {")), "{lines:?}");
        assert!(lines.contains(&"lbra: progress read on /update/libra".to_string()), "{lines:?}");

        // Everything 404 from then on: going on for the minute, the Agent there or not.
        let silent = [("/update/libra".to_string(), gone.clone()), ("/install/libra".to_string(), gone.clone())];
        let mut there = || true;
        let at = |s: u64| begun + Duration::from_secs(s);
        assert!(matches!(judge.round(at(30), &silent, &gone, &mut there), Verdict::Going { .. }));
        let lines = std::mem::take(&mut judge.lines);
        assert!(lines.contains(&"lbra: /update/libra now answers 404 {}".to_string()), "{lines:?}");
        // A minute on with the Agent there: still going, until it has been a quarter of an hour.
        assert!(matches!(judge.round(at(61), &silent, &gone, &mut there), Verdict::Going { .. }));
        let mut gone_now = || false;
        assert_eq!(
            judge.round(at(62), &silent, &gone, &mut gone_now),
            Verdict::Silent { for_: Duration::from_secs(62), agent_there: false }
        );
        let forgotten = judge.round(at(15 * 60 + 1), &silent, &gone, &mut there);
        assert!(matches!(forgotten, Verdict::Silent { agent_there: true, .. }), "{forgotten:?}");
        assert_eq!(
            told(&Verdict::Silent { for_: SILENT, agent_there: false }).unwrap(),
            "Battle.net's Agent stopped answering. Install it again, or finish it in Battle.net."
        );
    }

    #[test]
    fn an_error_the_agent_names_ends_it_and_is_told() {
        let begun = Instant::now();
        let mut judge = Judge::new(&watch(), begun);
        let mut there = || true;
        // A 404 with an error, on a path that never answered: where it is not, not an end.
        let elsewhere = [("/update/libra".to_string(), answer(404, json!({"error": 2300})))];
        let verdict = judge.round(begun, &elsewhere, &answer(200, json!({"installed": false})), &mut there);
        assert!(matches!(verdict, Verdict::Going { .. }), "{verdict:?}");
        // On a 2xx answer: failed, with its code.
        let failed = [("/update/libra".to_string(), answer(200, json!({"error": 2421, "state": 1003})))];
        let verdict = judge.round(begun, &failed, &answer(404, json!({})), &mut there);
        assert_eq!(
            verdict,
            Verdict::Failed {
                error: 2421,
                path: "/update/libra".into(),
                said: r#"{"error":2421,"state":1003}"#.into()
            }
        );
        assert_eq!(
            told(&verdict).unwrap(),
            "Battle.net's Agent stopped it with error 2421: the PC does not meet its needs, or there is not enough \
             space."
        );
    }

    #[test]
    fn the_game_alone_keeps_it_going_and_says_when_it_is_done() {
        let begun = Instant::now();
        let mut judge = Judge::new(&watch(), begun);
        let mut there = || true;
        let silent = [("/update/libra".to_string(), answer(404, json!({})))];
        let game = answer(
            200,
            json!({"installed": true, "playable": true, "download_complete": true,
            "update_progress": 1.0}),
        );
        let Verdict::Going { progress, .. } = judge.round(begun, &silent, &game, &mut there) else { panic!() };
        assert!(progress.says_done());
        assert_eq!(judge.live(), Some("/game/libra"));
    }
}
