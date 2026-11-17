//! A store's downloads through its helper program (docs/research/installs.md 10.1; docs/sources.md "Installs"): one
//! job a game, run by [`crate::helper::Job`] on a thread of its own, its lines read by the store's own parser into
//! bytes, a whole and a speed, which [`Downloads::installs`] gives the Queue as `Source::installs` does for every
//! store. Pause is the helper stopped (both helpers resume from what they kept when run again), Resume the same job
//! run again, Cancel the helper stopped and what it fetched deleted. A job that ends badly leaves the Queue as a
//! store's dropped install does, said in the log.
//!
//! Bytes across a pause: a helper run again says only what is left (legendary's "Download size", gogdl's progress
//! total, both counted after what its resume file skips), so the whole is the first run's, and a later run's bytes
//! count on from the whole less what that run has left.
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use model::{Install, InstallState};

use crate::helper::{Job, Said, Stopper};

/// What a line of a helper's output says of its download, as the store's parser reads it.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// The bytes this run has to download.
    Total(u64),
    /// The bytes this run has downloaded so far.
    Done(u64),
    /// Both on one line, as gogdl says them every second (`= Progress: <pct> <done>/<total>`): the whole is taken
    /// as `Total` the first time it is said, and only then.
    Progress { done: u64, total: u64 },
    /// How fast it downloads now, in bytes a second.
    Speed(u64),
    /// An error the helper says; kept, to say why if it then ends badly.
    Error(String),
    /// A progress line with nothing the Queue needs: not logged (they come every second).
    Quiet,
    /// Anything else: logged as it is.
    Other,
}

/// One job to run: the helper, what it is given, how its lines read, and what is left to do once it ends well.
pub struct Run {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, OsString)>,
    pub parse: fn(&str) -> Line,
    /// Run on the job's thread once the helper has ended with 0: the store's records checked, and the rest of the
    /// install done (GOG's set-up step). Why not, where it did not end well.
    pub finish: Box<dyn FnOnce() -> Result<(), String> + Send>,
}

/// What the Queue asked of a job under way.
enum Asked {
    Pause,
    Cancel(Box<dyn FnOnce() + Send>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Running,
    Paused,
    /// The helper is done; the rest of the install is being done.
    Finishing,
}

#[derive(Default)]
struct Download {
    phase: Phase,
    /// The whole download: the first run's total.
    whole: Option<u64>,
    /// What this run's bytes count on from.
    base: u64,
    /// Done when this run began: a paused run's bytes.
    before: u64,
    run_done: u64,
    /// Bytes a second, as the helper last said.
    speed: Option<u64>,
    stopper: Option<Stopper>,
    asked: Option<Asked>,
    error: Option<String>,
}

impl Download {
    fn done(&self) -> u64 {
        let done = self.base + self.run_done;
        self.whole.map_or(done, |whole| done.min(whole))
    }

    fn state(&self) -> InstallState {
        let (done, total) = (self.done(), self.whole.unwrap_or(0));
        match self.phase {
            Phase::Running => InstallState::Downloading { done, total },
            Phase::Paused => InstallState::Paused { done, total },
            Phase::Finishing => InstallState::Downloading { done: total, total },
        }
    }
}

/// A store's downloads under way, by the game's `source_id`. Cloned, it is the same downloads.
#[derive(Clone)]
pub struct Downloads {
    /// What its log lines start with: `epic: legendary`.
    log: &'static str,
    map: Arc<Mutex<HashMap<String, Download>>>,
}

impl std::fmt::Debug for Downloads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Downloads").field("log", &self.log).finish_non_exhaustive()
    }
}

impl Downloads {
    pub fn new(log: &'static str) -> Self {
        Self { log, map: Arc::default() }
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Download>> {
        self.map.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The downloads under way, for `Source::installs`.
    pub fn installs(&self) -> Vec<Install> {
        let mut installs: Vec<Install> = self
            .map()
            .iter()
            .map(|(id, download)| {
                let running = download.phase == Phase::Running;
                let speed = download.speed.filter(|_| running).map(|bytes| bytes.saturating_mul(8));
                Install { source_id: id.clone(), state: download.state(), speed }
            })
            .collect();
        installs.sort_by(|a, b| a.source_id.cmp(&b.source_id));
        installs
    }

    /// Whether the game has a download here, running or paused.
    pub fn has(&self, id: &str) -> bool {
        self.map().contains_key(id)
    }

    /// Whether the game's download is paused.
    pub fn paused(&self, id: &str) -> bool {
        self.map().get(id).is_some_and(|download| download.phase == Phase::Paused)
    }

    /// Starts the game's job, or runs a paused one again (its bytes carried on). Refused while one runs.
    pub fn start(&self, id: &str, run: Run) -> Result<(), String> {
        let log = self.log;
        let mut map = self.map();
        let download = map.entry(id.to_string()).or_default();
        if download.stopper.is_some() || download.phase == Phase::Finishing {
            return Err("it is already under way".into());
        }
        let wake = || {};
        let job = Job::start_with(&run.program, &run.args, &run.env, wake)
            .map_err(|e| format!("{} could not be started: {e}", run.program.display()))?;
        eprintln!("{log}: {id}: started (pid {}): {}", job.pid(), run.args.join(" "));
        download.before = download.done();
        download.base = download.before;
        download.run_done = 0;
        download.phase = Phase::Running;
        download.speed = None;
        download.error = None;
        download.stopper = Some(job.stopper());
        download.asked = None;
        drop(map);
        let (downloads, id, parse, finish) = (self.clone(), id.to_string(), run.parse, run.finish);
        std::thread::spawn(move || downloads.follow(&id, job, parse, finish));
        Ok(())
    }

    /// Reads the job's lines until it ends, then does as it ended, or as the Queue asked.
    fn follow(
        &self,
        id: &str,
        job: Job,
        parse: fn(&str) -> Line,
        finish: Box<dyn FnOnce() -> Result<(), String> + Send>,
    ) {
        let log = self.log;
        let mut said_total = None;
        let code = loop {
            let line = match job.next(Duration::from_secs(60)) {
                Some(Said::Out(line) | Said::Err(line)) => line,
                Some(Said::Ended(code)) => break code,
                None => continue,
            };
            let read = parse(&line);
            let mut map = self.map();
            let Some(download) = map.get_mut(id) else { continue };
            let total = match read {
                Line::Total(bytes) => Some(bytes),
                Line::Progress { total, .. } if said_total.is_none() => Some(total),
                _ => None,
            };
            if let Some(bytes) = total {
                let whole = *download.whole.get_or_insert(download.before + bytes);
                download.base = whole.saturating_sub(bytes);
                said_total = Some(bytes);
                eprintln!("{log}: {id}: {bytes} bytes to download, {whole} in all");
            }
            match read {
                Line::Total(_) => {}
                Line::Progress { done, .. } => download.run_done = done,
                Line::Done(bytes) => download.run_done = bytes,
                Line::Speed(bytes) => download.speed = Some(bytes),
                Line::Error(why) => {
                    eprintln!("{log}: {id}: says {line}");
                    download.error = Some(why);
                }
                Line::Quiet => {}
                Line::Other => eprintln!("{log}: {id}: says {line}"),
            }
        };
        let said = code.map_or("no code".to_string(), |code| code.to_string());
        let (asked, error) = {
            let mut map = self.map();
            let Some(download) = map.get_mut(id) else { return };
            download.stopper = None;
            download.speed = None;
            (download.asked.take(), download.error.take())
        };
        match asked {
            Some(Asked::Pause) => {
                let mut map = self.map();
                if let Some(download) = map.get_mut(id) {
                    download.phase = Phase::Paused;
                    download.before = download.done();
                    eprintln!("{log}: {id}: paused at {} of {} bytes", download.before, download.whole.unwrap_or(0));
                }
            }
            Some(Asked::Cancel(cleanup)) => {
                cleanup();
                self.map().remove(id);
                eprintln!("{log}: {id}: cancelled, and what it fetched deleted");
            }
            None if code == Some(0) => {
                if let Some(download) = self.map().get_mut(id) {
                    download.phase = Phase::Finishing;
                }
                let finished = finish();
                self.map().remove(id);
                match finished {
                    Ok(()) => eprintln!("{log}: {id}: done"),
                    Err(why) => eprintln!("{log}: {id}: ended with 0, but {why}: dropped"),
                }
            }
            None => {
                self.map().remove(id);
                let why = error.map_or_else(String::new, |error| format!(": {error}"));
                eprintln!("{log}: {id}: ended with {said}{why}: dropped");
            }
        }
    }

    /// Pauses the game's job: it is stopped, and stays in the Queue paused with its bytes.
    pub fn pause(&self, id: &str) -> Result<(), String> {
        let mut map = self.map();
        let download = map.get_mut(id).ok_or("it has no download here")?;
        match (&download.stopper, download.phase) {
            (_, Phase::Finishing) => Err("its download is done and its set-up is running".into()),
            (_, Phase::Paused) => Ok(()),
            (Some(stopper), Phase::Running) => {
                download.asked = Some(Asked::Pause);
                stopper.stop();
                Ok(())
            }
            (None, Phase::Running) => Err("its helper is not running".into()),
        }
    }

    /// Cancels the game's job: it is stopped, and `cleanup` deletes what it fetched once it has; a paused one's at
    /// once. It leaves the Queue.
    pub fn cancel(&self, id: &str, cleanup: Box<dyn FnOnce() + Send>) -> Result<(), String> {
        let mut map = self.map();
        let download = map.get_mut(id).ok_or("it has no download here")?;
        if download.phase == Phase::Finishing {
            return Err("its download is done and its set-up is running".into());
        }
        match download.stopper.clone() {
            Some(stopper) => {
                download.asked = Some(Asked::Cancel(cleanup));
                stopper.stop();
            }
            None => {
                map.remove(id);
                drop(map);
                cleanup();
                eprintln!("{}: {id}: cancelled while paused, and what it fetched deleted", self.log);
            }
        }
        Ok(())
    }
}

/// Runs a helper to its end on this thread, saying each line it says in the log as `<log>: <what>: says <line>`,
/// stopped after `most`: its exit code (none when stopped) and its lines.
pub fn run_to_end(
    log: &str,
    what: &str,
    program: &std::path::Path,
    args: &[String],
    env: &[(String, OsString)],
    most: Duration,
) -> Result<(Option<i32>, Vec<String>), String> {
    let job = Job::start_with(program, args, env, || {})
        .map_err(|e| format!("{} could not be started: {e}", program.display()))?;
    eprintln!("{log}: {what}: started (pid {}): {}", job.pid(), args.join(" "));
    let until = std::time::Instant::now() + most;
    let mut lines = Vec::new();
    loop {
        let left = until.saturating_duration_since(std::time::Instant::now());
        match job.next(left) {
            Some(Said::Out(line) | Said::Err(line)) => {
                eprintln!("{log}: {what}: says {line}");
                lines.push(line);
            }
            Some(Said::Ended(code)) => {
                eprintln!("{log}: {what}: ended with {}", code.map_or("no code".into(), |c| c.to_string()));
                return Ok((code, lines));
            }
            None => {
                eprintln!("{log}: {what}: still running after {} s: stopped", most.as_secs());
                job.stop();
                return Ok((None, lines));
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn parse(line: &str) -> Line {
        if let Some(rest) = line.strip_prefix("total ") {
            return Line::Total(rest.parse().unwrap());
        }
        if let Some(rest) = line.strip_prefix("done ") {
            return Line::Done(rest.parse().unwrap());
        }
        if let Some(rest) = line.strip_prefix("speed ") {
            return Line::Speed(rest.parse().unwrap());
        }
        if let Some(rest) = line.strip_prefix("ERROR: ") {
            return Line::Error(rest.into());
        }
        Line::Other
    }

    fn run(script: &str, finish: impl FnOnce() -> Result<(), String> + Send + 'static) -> Run {
        Run {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            env: Vec::new(),
            parse,
            finish: Box::new(finish),
        }
    }

    fn soon(what: impl Fn() -> bool) -> bool {
        for _ in 0..500 {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn a_download_counts_its_bytes_pauses_resumes_from_what_is_left_and_finishes() {
        let downloads = Downloads::new("test");
        let state = |downloads: &Downloads| downloads.installs().first().map(|install| install.state);
        downloads.start("g", run("echo total 100; echo done 40; echo speed 5; sleep 30", || Ok(()))).unwrap();
        assert!(soon(|| state(&downloads) == Some(InstallState::Downloading { done: 40, total: 100 })));
        assert_eq!(downloads.installs()[0].speed, Some(40));
        assert!(downloads.start("g", run("true", || Ok(()))).is_err(), "one job a game");
        downloads.pause("g").unwrap();
        assert!(soon(|| state(&downloads) == Some(InstallState::Paused { done: 40, total: 100 })));
        assert_eq!(downloads.installs()[0].speed, None);
        // Run again, it has 60 left: its own bytes count on from 40.
        let (tell, finished) = std::sync::mpsc::channel();
        let finish = move || {
            tell.send(()).unwrap();
            Ok(())
        };
        downloads.start("g", run("echo total 60; echo done 30; sleep 0.3; echo done 60", finish)).unwrap();
        assert!(soon(|| matches!(state(&downloads), Some(InstallState::Downloading { done: 70 | 100, total: 100 }))));
        finished.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(soon(|| downloads.installs().is_empty()));
    }

    #[test]
    fn cancel_deletes_what_was_fetched_and_a_bad_end_drops_it() {
        let downloads = Downloads::new("test");
        let (tell, cleaned) = std::sync::mpsc::channel();
        downloads.start("g", run("echo total 10; sleep 30", || Ok(()))).unwrap();
        downloads.cancel("g", Box::new(move || tell.send(()).unwrap())).unwrap();
        cleaned.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(soon(|| !downloads.has("g")));

        // Paused, then cancelled: deleted at once.
        downloads.start("h", run("echo total 10; sleep 30", || Ok(()))).unwrap();
        downloads.pause("h").unwrap();
        assert!(soon(|| downloads.paused("h")));
        let (tell, cleaned) = std::sync::mpsc::channel();
        downloads.cancel("h", Box::new(move || tell.send(()).unwrap())).unwrap();
        cleaned.try_recv().unwrap();
        assert!(!downloads.has("h"));

        downloads.start("i", run("echo 'ERROR: no space'; exit 1", || panic!("not finished"))).unwrap();
        assert!(soon(|| !downloads.has("i")));
        // Ended with 0, and the store's records say it is not there: dropped all the same.
        downloads.start("j", run("exit 0", || Err("it is not in installed.json".into()))).unwrap();
        assert!(soon(|| !downloads.has("j")));
    }
}
