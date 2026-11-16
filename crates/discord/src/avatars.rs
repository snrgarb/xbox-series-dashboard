//! The avatars of the people in a Call, a Shortcut's and a server's voice channels', and the servers' icons
//! (docs/discord.md, "Avatars"): fetched from Discord's CDN on a thread of their own, off the frame, and kept in the
//! data folder as art is (`art/discord/avatars/<user>/<hash>.png`, `art/discord/embed/<n>.png`,
//! `art/discord/icons/<guild>/<hash>.png`), so each is fetched once. An avatar's hash changes with the picture, so a kept one is
//! never stale. Until one is there the Discord tab draws a plain disc in its place.
//!
//! A test points it at a stand-in CDN (`standin discord` serves plain pictures), never Discord's: with a test's pipe
//! and no CDN given there is none, and nothing is fetched.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::ipc::Wake;
use crate::voice::Avatar;

/// The most an avatar may be: a 64-across PNG is a few kilobytes.
const MOST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Kept(PathBuf),
    Fetching,
    /// Couldn't be fetched: not tried again until the Dashboard starts again.
    Failed,
}

/// A fetch for the thread: the avatar, whose it is (for the log), its address and where it goes.
struct Job {
    avatar: Avatar,
    name: String,
    url: String,
    file: PathBuf,
}

pub struct Avatars {
    /// The cache's folder; None with no data folder, when nothing is kept or fetched.
    dir: Option<PathBuf>,
    /// The CDN's address; None for none, when only what is kept already is shown.
    cdn: Option<String>,
    known: HashMap<Avatar, State>,
    jobs: Option<Sender<Job>>,
    done: Receiver<(Avatar, String, Result<PathBuf, String>)>,
    done_sender: Sender<(Avatar, String, Result<PathBuf, String>)>,
    wake: Wake,
    log: Vec<String>,
}

impl Avatars {
    /// Avatars kept under `dir` and fetched from `cdn`, waking the loop with `wake` as each arrives.
    pub fn new(dir: Option<PathBuf>, cdn: Option<String>, wake: Wake) -> Self {
        let (done_sender, done) = mpsc::channel();
        Self { dir, cdn, known: HashMap::new(), jobs: None, done, done_sender, wake, log: Vec::new() }
    }

    /// The avatar's file, if it is kept; if not, it is fetched, and this is None until it has arrived
    /// ([`Avatars::pump`]). `name` is whose it is, for the log.
    pub fn get(&mut self, avatar: &Avatar, name: &str) -> Option<PathBuf> {
        match self.known.get(avatar) {
            Some(State::Kept(path)) => return Some(path.clone()),
            Some(State::Fetching | State::Failed) => return None,
            None => {}
        }
        let file = self.dir.as_ref()?.join(avatar.file());
        if file.is_file() {
            self.known.insert(avatar.clone(), State::Kept(file.clone()));
            return Some(file);
        }
        let cdn = self.cdn.clone()?;
        let job = Job { avatar: avatar.clone(), name: name.to_string(), url: avatar.url(&cdn), file };
        if self.fetch(job) {
            self.known.insert(avatar.clone(), State::Fetching);
        } else {
            self.known.insert(avatar.clone(), State::Failed);
        }
        None
    }

    /// Hands the job to the thread, starting it the first time.
    fn fetch(&mut self, job: Job) -> bool {
        if self.jobs.is_none() {
            let (sender, jobs) = mpsc::channel::<Job>();
            let (done, wake) = (self.done_sender.clone(), self.wake.clone());
            let spawned = std::thread::Builder::new().name("discord avatars".into()).spawn(move || {
                let agent = agent();
                for job in jobs {
                    let fetched = download(&agent, &job.url).and_then(|bytes| keep(&job.file, &bytes));
                    let _ = done.send((job.avatar, job.name, fetched));
                    wake();
                }
            });
            if spawned.is_err() {
                return false;
            }
            self.jobs = Some(sender);
        }
        self.jobs.as_ref().is_some_and(|jobs| jobs.send(job).is_ok())
    }

    /// Takes in the avatars that have arrived, or couldn't be fetched: whether any did arrive.
    pub fn pump(&mut self) -> bool {
        let mut arrived = false;
        while let Ok((avatar, name, fetched)) = self.done.try_recv() {
            let what = if matches!(avatar, Avatar::Icon { .. }) { "icon" } else { "avatar" };
            match fetched {
                Ok(path) => {
                    self.log.push(format!("discord: {what} of {name} fetched"));
                    self.known.insert(avatar, State::Kept(path));
                    arrived = true;
                }
                Err(why) => {
                    self.log.push(format!("discord: {what} of {name} couldn't be fetched: {why}"));
                    self.known.insert(avatar, State::Failed);
                }
            }
        }
        arrived
    }

    /// The log's lines since this was last asked.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("xbox-series-dashboard/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent.get(url).call().map_err(|e| format!("no answer ({e})"))?;
    match response.status().as_u16() {
        200..=299 => {}
        status => return Err(format!("HTTP {status}")),
    }
    let bytes = response.body_mut().with_config().limit(MOST_BYTES).read_to_vec().map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("an empty answer".into());
    }
    Ok(bytes)
}

/// Writes the avatar whole, to `.new` and renamed, so a half-written one is never taken for kept.
fn keep(file: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let folder = file.parent().ok_or("no folder")?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let new = file.with_extension("png.new");
    std::fs::write(&new, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&new, file).map_err(|e| e.to_string())?;
    Ok(file.to_path_buf())
}
