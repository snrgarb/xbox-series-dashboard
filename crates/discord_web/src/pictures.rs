//! The avatars and icons of the Direct Messages (docs/spec/party-chats.md, section 9): fetched from Discord's CDN by
//! the host side, off the web view's thread and the Dashboard's, with a few threads at most, and kept **in memory only**
//! (never a file), by the path the page gave (`avatars/<user>/<hash>.png?size=128`, `channel-icons/<channel>/<hash>.png`).
//! Dropped with the web session ([`Pictures::forget`]): a fetch still running then is let go of its result.
//!
//! The CDN's address is Discord's, or a stand-in's in a test (the Dashboard's `--discord-cdn-url`, as the voice part's
//! avatars). With none given, nothing is fetched. No address is ever logged: ids are in them.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use model::discord_dms::Picture;

/// How many pictures are fetched at once.
const THREADS: usize = 3;
/// The most a picture may be.
const MOST_BYTES: u64 = 1024 * 1024;

type Wake = Arc<dyn Fn() + Send + Sync>;

enum State {
    Fetching,
    Got(Picture),
    /// Not tried again until the session is forgotten.
    Failed,
}

struct Job {
    epoch: u64,
    path: String,
    cdn: String,
}

struct Done {
    epoch: u64,
    path: String,
    result: Result<Vec<u8>, String>,
}

pub struct Pictures {
    cdn: Option<String>,
    map: HashMap<String, State>,
    epoch: Arc<AtomicU64>,
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    done_sender: Sender<Done>,
    wake: Wake,
    /// Failures since the last [`Pictures::pump`] said so, by reason: "HTTP 404", "no answer".
    failures: Vec<String>,
}

/// Whether `path` is one of the CDN's picture paths the page may name: nothing else is ever fetched.
pub fn valid(path: &str) -> bool {
    let Some((kind, rest)) = path.split_once('/') else { return false };
    if kind != "avatars" && kind != "channel-icons" {
        return false;
    }
    let mut parts = rest.split('/');
    let (Some(id), Some(file), None) = (parts.next(), parts.next(), parts.next()) else { return false };
    let Some((hash, query)) = file.split_once(".png?size=") else { return false };
    !id.is_empty()
        && id.bytes().all(|b| b.is_ascii_digit())
        && !hash.is_empty()
        && hash.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !query.is_empty()
        && query.bytes().all(|b| b.is_ascii_digit())
}

impl Pictures {
    pub fn new(cdn: Option<String>, wake: Wake) -> Self {
        let (done_sender, done) = mpsc::channel();
        Self {
            cdn: cdn.map(|c| c.trim_end_matches('/').to_string()),
            map: HashMap::new(),
            epoch: Arc::new(AtomicU64::new(0)),
            jobs: None,
            done,
            done_sender,
            wake,
            failures: Vec::new(),
        }
    }

    pub fn set_cdn(&mut self, cdn: Option<String>) {
        self.cdn = cdn.map(|c| c.trim_end_matches('/').to_string());
    }

    /// The picture, once it has come.
    pub fn get(&self, path: &str) -> Option<Picture> {
        match self.map.get(path) {
            Some(State::Got(picture)) => Some(picture.clone()),
            _ => None,
        }
    }

    /// Fetches `path` unless it is empty, not a CDN picture path, known already, or there is no CDN.
    pub fn want(&mut self, path: &str) {
        if path.is_empty() || self.map.contains_key(path) || !valid(path) {
            return;
        }
        let Some(cdn) = self.cdn.clone() else { return };
        if self.jobs.is_none() {
            let (sender, jobs) = mpsc::channel::<Job>();
            let jobs = Arc::new(Mutex::new(jobs));
            for n in 0..THREADS {
                let (jobs, done, wake, epoch) =
                    (jobs.clone(), self.done_sender.clone(), self.wake.clone(), self.epoch.clone());
                let spawned = std::thread::Builder::new().name(format!("discord pictures {n}")).spawn(move || {
                    let agent = agent();
                    loop {
                        let job = match jobs.lock().map(|rx| rx.recv()) {
                            Ok(Ok(job)) => job,
                            _ => return,
                        };
                        // Forgotten while it waited: not fetched.
                        if job.epoch != epoch.load(Ordering::SeqCst) {
                            continue;
                        }
                        let result = download(&agent, &format!("{}/{}", job.cdn, job.path));
                        let _ = done.send(Done { epoch: job.epoch, path: job.path, result });
                        wake();
                    }
                });
                if spawned.is_err() && n == 0 {
                    self.map.insert(path.to_string(), State::Failed);
                    return;
                }
            }
            self.jobs = Some(sender);
        }
        let job = Job { epoch: self.epoch.load(Ordering::SeqCst), path: path.to_string(), cdn };
        let sent = self.jobs.as_ref().is_some_and(|jobs| jobs.send(job).is_ok());
        self.map.insert(path.to_string(), if sent { State::Fetching } else { State::Failed });
    }

    /// Takes in what has arrived: whether any picture did.
    pub fn pump(&mut self) -> bool {
        let mut arrived = false;
        while let Ok(done) = self.done.try_recv() {
            if done.epoch != self.epoch.load(Ordering::SeqCst) {
                continue;
            }
            match done.result {
                Ok(bytes) => {
                    self.map.insert(done.path, State::Got(Picture(bytes.into())));
                    arrived = true;
                }
                Err(why) => {
                    self.map.insert(done.path, State::Failed);
                    self.failures.push(why);
                }
            }
        }
        arrived
    }

    /// The reasons pictures failed since last asked (for one log line; never an address).
    pub fn take_failures(&mut self) -> Vec<String> {
        std::mem::take(&mut self.failures)
    }

    /// Drops every picture and lets go of every fetch under way.
    pub fn forget(&mut self) {
        self.map.clear();
        self.failures.clear();
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    /// How many pictures are held.
    #[cfg(test)]
    pub fn held(&self) -> usize {
        self.map.values().filter(|s| matches!(s, State::Got(_))).count()
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

/// The picture's bytes, or why not as a reason without an address.
fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent.get(url).call().map_err(|_| "no answer".to_string())?;
    match response.status().as_u16() {
        200..=299 => {}
        status => return Err(format!("HTTP {status}")),
    }
    response
        .body_mut()
        .with_config()
        .limit(MOST_BYTES)
        .read_to_vec()
        .map_err(|_| "unreadable".to_string())
        .and_then(|bytes| if bytes.is_empty() { Err("an empty answer".to_string()) } else { Ok(bytes) })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    #[test]
    fn only_the_cdns_picture_paths_are_fetched() {
        assert!(valid("avatars/300000000000000001/ha1.png?size=128"));
        assert!(valid("channel-icons/100/a_b2.png?size=64"));
        for bad in [
            "",
            "avatars/1/h.png",
            "../x",
            "avatars/1/h.png?size=128&x=1/../..",
            "avatars/x/h.png?size=128",
            "icons/1/h.png?size=128",
            "avatars/1/h/y.png?size=128",
            "https://evil.test/avatars/1/h.png?size=1",
            "avatars/1/h.png?size=",
        ] {
            assert!(!valid(bad), "{bad}");
        }
    }

    /// A stand-in CDN: serves `bytes` for avatars/1/*, 404 for the rest; says every path it was asked.
    pub(crate) fn cdn(bytes: &'static [u8]) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let kept = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let kept = kept.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                    loop {
                        let mut header = String::new();
                        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                            break;
                        }
                    }
                    kept.lock().unwrap().push(path.clone());
                    let mut out = stream;
                    if path.starts_with("/avatars/1/") {
                        write!(out, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len())
                            .unwrap();
                        out.write_all(bytes).unwrap();
                    } else {
                        write!(out, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                            .unwrap();
                    }
                    let _ = out.flush();
                    let _ = reader.read(&mut [0u8; 1]);
                });
            }
        });
        (url, asked)
    }

    fn wait(pictures: &mut Pictures, mut done: impl FnMut(&Pictures) -> bool) {
        for _ in 0..400 {
            pictures.pump();
            if done(pictures) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("the pictures did not come");
    }

    #[test]
    fn pictures_are_fetched_into_memory_failures_say_a_reason_and_forgetting_drops_them() {
        let (url, asked) = cdn(b"made-up picture bytes");
        let mut pictures = Pictures::new(Some(url), Arc::new(|| {}));
        pictures.want("");
        pictures.want("https://evil.test/avatars/1/h.png?size=1");
        pictures.want("avatars/1/ha.png?size=128");
        pictures.want("avatars/1/ha.png?size=128");
        pictures.want("avatars/2/missing.png?size=128");
        wait(&mut pictures, |p| p.held() == 1 && asked.lock().unwrap().len() == 2);
        let picture = pictures.get("avatars/1/ha.png?size=128").unwrap();
        assert_eq!(&*picture.0, b"made-up picture bytes");
        assert!(pictures.get("avatars/2/missing.png?size=128").is_none());
        let mut paths = asked.lock().unwrap().clone();
        paths.sort();
        assert_eq!(
            paths,
            ["/avatars/1/ha.png?size=128", "/avatars/2/missing.png?size=128"],
            "once each, only the CDN's paths"
        );
        wait(&mut pictures, |p| !p.failures.is_empty());
        assert_eq!(pictures.take_failures(), vec!["HTTP 404".to_string()]);
        pictures.forget();
        assert_eq!(pictures.held(), 0);
        assert!(pictures.get("avatars/1/ha.png?size=128").is_none());
    }

    #[test]
    fn with_no_cdn_nothing_is_fetched() {
        let mut pictures = Pictures::new(None, Arc::new(|| {}));
        pictures.want("avatars/1/ha.png?size=128");
        assert!(pictures.jobs.is_none() && pictures.map.is_empty());
    }
}
