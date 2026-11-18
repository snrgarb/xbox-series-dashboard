//! The installs the Dashboard has asked the Agent for, each followed on a thread of its own by `GET
//! /install/<uid>` once a second, for the Queue (`Source::installs` reads what they last said and asks nothing).
//! An install is over once the Agent says it is playable and `product.db` says installed (the source's own read then
//! finds it), or once the Agent gives an error or forgets it: the Queue then drops it as a store's dropped install.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::{Install, InstallState};

use crate::agent::{calls, Session};

/// How often an install's progress is asked.
pub const EVERY: Duration = Duration::from_secs(1);

/// How long `product.db` is waited on to say installed once the Agent says playable.
const SETTLE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
struct Job {
    uid: String,
    state: InstallState,
    /// Bits a second, as the Queue takes a store's speed.
    speed: Option<u64>,
    /// The user cancelled it: the follower stops.
    stopped: bool,
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
        jobs.insert(code.into(), Job { uid: uid.into(), state: InstallState::Queued, speed: None, stopped: false });
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
}

/// What the Queue reads of an answer: Queued until it has bytes, Paused where it says so, else Downloading.
pub fn state(progress: &calls::Progress) -> (InstallState, Option<u64>) {
    let bytes = match (progress.done, progress.total) {
        (Some(done), Some(total)) if total > 0 => Some((done.min(total), total)),
        // Only a share: a whole of a million, so the Queue's share is right.
        _ if progress.progress > 0.0 => Some(((progress.progress * 1e6) as u64, 1_000_000)),
        _ => None,
    };
    match bytes {
        Some((done, total)) if progress.paused => (InstallState::Paused { done, total }, None),
        Some((done, total)) => (InstallState::Downloading { done, total }, progress.rate.map(|bytes| bytes * 8)),
        None if progress.paused => (InstallState::Paused { done: 0, total: 0 }, None),
        None => (InstallState::Queued, None),
    }
}

/// Follows `code`'s install on a thread of its own until it is over; `installed` says whether `product.db` says it
/// is installed now.
pub fn follow(jobs: Jobs, mut session: Session, code: String, installed: impl Fn() -> bool + Send + 'static) {
    std::thread::spawn(move || {
        let uid = jobs.uid(&code).unwrap_or_else(|| code.clone());
        let (method, path) = calls::progress(&uid);
        let mut said: Option<InstallState> = None;
        let mut playable_at: Option<Instant> = None;
        let mut misses = 0;
        loop {
            std::thread::sleep(EVERY);
            if jobs.stopped(&code) {
                return;
            }
            let answer = match session.call_quiet(&format!("progress {code}"), method, &path) {
                Ok(answer) => answer,
                Err(why) => {
                    misses += 1;
                    if misses >= 30 {
                        crate::log(&format!(
                            "{code}: the Agent has not answered for 30 s ({why}): dropped from the Queue"
                        ));
                        jobs.end(&code);
                        return;
                    }
                    continue;
                }
            };
            misses = 0;
            if let Some(why) = calls::error(&answer) {
                crate::log(&format!("{code}: the Agent says {why}: dropped from the Queue"));
                jobs.end(&code);
                return;
            }
            let progress = calls::read_progress(&answer);
            let (state, speed) = state(&progress);
            // A line when the state's kind changes, and every tenth of the way.
            let tenth = |s: InstallState| (std::mem::discriminant(&s), s.percent().map(|p| p / 10));
            if said.is_none_or(|before| tenth(before) != tenth(state)) {
                crate::log(&format!("{code}: {}", say(&state, progress.rate)));
                said = Some(state);
            }
            jobs.set(&code, state, speed);
            if progress.playable {
                let since = *playable_at.get_or_insert_with(Instant::now);
                if installed() {
                    crate::log(&format!("{code}: done: product.db says installed"));
                    jobs.end(&code);
                    return;
                }
                if since.elapsed() > SETTLE {
                    crate::log(&format!("{code}: the Agent says playable and product.db does not say installed after {} s: left to the next read", SETTLE.as_secs()));
                    jobs.end(&code);
                    return;
                }
            }
        }
    });
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
        let share = calls::Progress { progress: 0.5, ..calls::Progress::default() };
        assert_eq!(state(&share).0, InstallState::Downloading { done: 500_000, total: 1_000_000 });

        let jobs = Jobs::default();
        assert!(jobs.begin("emb", "emberwatch") && !jobs.begin("emb", "emberwatch"));
        assert_eq!(jobs.installs(), [Install::new("emb", InstallState::Queued)]);
        jobs.set("emb", InstallState::Paused { done: 1, total: 2 }, None);
        assert!(jobs.paused("emb") && jobs.has("emb"));
        jobs.stop("emb");
        assert!(jobs.installs().is_empty() && jobs.stopped("emb"));
    }
}
