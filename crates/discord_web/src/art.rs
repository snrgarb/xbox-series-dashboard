//! The pictures of the Call, the Shortcuts and the Servers Flyout (docs/spec/discord-tab.md; docs/discord-web.md, "Voice"):
//! the people's avatars and the servers' icons, fetched from Discord's CDN off the frame and kept **on disk** in the
//! art cache the RPC connection's avatars use, under the same names, so the Discord tab draws them as it always has
//! and a picture the RPC crate kept is used at once: `<art>/avatars/<user>/<hash>.png`, `<art>/embed/<n>.png` (a
//! default avatar), `<art>/icons/<guild>/<hash>.png`. A picture's hash changes with the picture, so a kept one is never
//! stale. (The Direct Messages' pictures are another thing: memory only, `pictures.rs`.)
//!
//! A copy of the small part of `crates/discord/src/avatars.rs` and `voice.rs` that names, fetches and keeps a picture:
//! that crate is to be deleted, so nothing is shared. A test points the session at a stand-in CDN, never Discord's; with
//! none given nothing is fetched. Nothing is logged by address or name: ids are in them.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// The most a picture may be.
const MOST_BYTES: u64 = 1024 * 1024;

/// A picture on Discord's CDN, as the cache names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pic {
    /// A user's own avatar.
    Own { user: String, hash: String },
    /// The default avatar Discord gives a user without one: 0 to 5.
    Default(u64),
    /// A server's icon.
    Icon { guild: String, hash: String },
}

fn id_ok(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn hash_ok(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl Pic {
    /// The avatar of user `id` whose avatar hash is `hash`: their own, else the default one Discord gives them, picked by
    /// their id. None for an id that is not digits or a hash that is not safe.
    pub fn avatar(id: &str, hash: &str) -> Option<Pic> {
        if !id_ok(id) {
            return None;
        }
        if hash.is_empty() {
            return Some(Pic::Default((id.parse::<u64>().unwrap_or(0) >> 22) % 6));
        }
        hash_ok(hash).then(|| Pic::Own { user: id.to_string(), hash: hash.to_string() })
    }

    /// A server's icon; None for a server without one.
    pub fn icon(guild: &str, hash: &str) -> Option<Pic> {
        (id_ok(guild) && hash_ok(hash)).then(|| Pic::Icon { guild: guild.to_string(), hash: hash.to_string() })
    }

    /// Its address on the CDN at `cdn`: an avatar 64 across, as the RPC connection asks.
    pub fn url(&self, cdn: &str) -> String {
        let cdn = cdn.trim_end_matches('/');
        match self {
            Pic::Own { user, hash } => format!("{cdn}/avatars/{user}/{hash}.png?size=64"),
            Pic::Default(n) => format!("{cdn}/embed/avatars/{n}.png"),
            Pic::Icon { guild, hash } => format!("{cdn}/icons/{guild}/{hash}.png"),
        }
    }

    /// Where it is kept in the cache. Every part is digits, letters or `_`, so none can climb out of the folder.
    pub fn file(&self) -> PathBuf {
        match self {
            Pic::Own { user, hash } => PathBuf::from("avatars").join(user).join(format!("{hash}.png")),
            Pic::Default(n) => PathBuf::from("embed").join(format!("{n}.png")),
            Pic::Icon { guild, hash } => PathBuf::from("icons").join(guild).join(format!("{hash}.png")),
        }
    }

    /// As a Shortcut keeps it: the CDN path (`avatars/<user>/<hash>.png`, `embed/avatars/<n>.png`,
    /// `icons/<guild>/<hash>.png`), the shape the RPC connection's Shortcuts keep their address in.
    pub fn path(&self) -> String {
        match self {
            Pic::Own { user, hash } => format!("avatars/{user}/{hash}.png"),
            Pic::Default(n) => format!("embed/avatars/{n}.png"),
            Pic::Icon { guild, hash } => format!("icons/{guild}/{hash}.png"),
        }
    }

    /// The picture a kept path (or a full CDN address) names, any size or query; None for anything else.
    pub fn from_path(path: &str) -> Option<Pic> {
        let path = path.split_once("://").map_or(path, |(_, rest)| rest.split_once('/').map_or("", |(_, p)| p));
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let stem = |file: &str| file.rsplit_once('.').map_or(file, |(stem, _)| stem).to_string();
        let parts: Vec<&str> = path.split('/').collect();
        match parts.as_slice() {
            ["avatars", user, file] if id_ok(user) && hash_ok(&stem(file)) => {
                Some(Pic::Own { user: user.to_string(), hash: stem(file) })
            }
            ["embed", "avatars", file] => stem(file).parse().ok().filter(|n| *n < 6).map(Pic::Default),
            ["icons", guild, file] if id_ok(guild) && hash_ok(&stem(file)) => {
                Some(Pic::Icon { guild: guild.to_string(), hash: stem(file) })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Kept(PathBuf),
    Fetching,
    /// Not tried again until the Dashboard starts again.
    Failed,
}

struct Job {
    pic: Pic,
    url: String,
    file: PathBuf,
}

type Wake = Arc<dyn Fn() + Send + Sync>;
type Done = (Pic, Result<PathBuf, String>);

pub struct Art {
    /// The cache's folder; None with no data folder, when nothing is kept or fetched.
    dir: Option<PathBuf>,
    /// The CDN's address; None for none, when only what is kept already is shown.
    cdn: Option<String>,
    known: HashMap<Pic, State>,
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    done_sender: Sender<Done>,
    wake: Wake,
    failures: Vec<String>,
}

impl Art {
    pub fn new(dir: Option<PathBuf>, cdn: Option<String>, wake: Wake) -> Self {
        let (done_sender, done) = mpsc::channel();
        Self {
            dir,
            cdn: cdn.map(|c| c.trim_end_matches('/').to_string()),
            known: HashMap::new(),
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

    /// The picture's file, if it is kept; if not, it is fetched, and this is None until it has arrived ([`Art::pump`]).
    pub fn get(&mut self, pic: &Pic) -> Option<PathBuf> {
        match self.known.get(pic) {
            Some(State::Kept(path)) => return Some(path.clone()),
            Some(State::Fetching | State::Failed) => return None,
            None => {}
        }
        let file = self.dir.as_ref()?.join(pic.file());
        if file.is_file() {
            self.known.insert(pic.clone(), State::Kept(file.clone()));
            return Some(file);
        }
        let cdn = self.cdn.clone()?;
        let job = Job { pic: pic.clone(), url: pic.url(&cdn), file };
        let state = if self.fetch(job) { State::Fetching } else { State::Failed };
        self.known.insert(pic.clone(), state);
        None
    }

    fn fetch(&mut self, job: Job) -> bool {
        if self.jobs.is_none() {
            let (sender, jobs) = mpsc::channel::<Job>();
            let (done, wake) = (self.done_sender.clone(), self.wake.clone());
            let spawned = std::thread::Builder::new().name("discord art".into()).spawn(move || {
                let agent = agent();
                for job in jobs {
                    let fetched = download(&agent, &job.url).and_then(|bytes| keep(&job.file, &bytes));
                    let _ = done.send((job.pic, fetched));
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

    /// Takes in what has arrived: whether any picture did.
    pub fn pump(&mut self) -> bool {
        let mut arrived = false;
        while let Ok((pic, fetched)) = self.done.try_recv() {
            match fetched {
                Ok(path) => {
                    self.known.insert(pic, State::Kept(path));
                    arrived = true;
                }
                Err(why) => {
                    self.known.insert(pic, State::Failed);
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
    let mut response = agent.get(url).call().map_err(|_| "no answer".to_string())?;
    match response.status().as_u16() {
        200..=299 => {}
        status => return Err(format!("HTTP {status}")),
    }
    let bytes =
        response.body_mut().with_config().limit(MOST_BYTES).read_to_vec().map_err(|_| "unreadable".to_string())?;
    if bytes.is_empty() {
        return Err("an empty answer".into());
    }
    Ok(bytes)
}

/// Writes the picture whole, to `.new` and renamed, so a half-written one is never taken for kept.
fn keep(file: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let folder = file.parent().ok_or("no folder")?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let new = file.with_extension("png.new");
    std::fs::write(&new, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&new, file).map_err(|e| e.to_string())?;
    Ok(file.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_the_rpc_crates_and_none_can_leave_the_folder() {
        let own = Pic::avatar("123", "abc_1").unwrap();
        assert_eq!(own.file(), PathBuf::from("avatars/123/abc_1.png"));
        assert_eq!(own.url("https://cdn.test/"), "https://cdn.test/avatars/123/abc_1.png?size=64");
        assert_eq!(Pic::avatar("123", "").unwrap(), Pic::Default((123u64 >> 22) % 6));
        assert_eq!(Pic::Default(3).file(), PathBuf::from("embed/3.png"));
        assert_eq!(Pic::Default(3).url("https://c"), "https://c/embed/avatars/3.png");
        assert_eq!(Pic::icon("9", "h").unwrap().file(), PathBuf::from("icons/9/h.png"));
        assert_eq!(Pic::icon("9", "h").unwrap().url("https://c"), "https://c/icons/9/h.png");
        assert_eq!(Pic::icon("9", ""), None);
        assert_eq!(Pic::avatar("../x", "h"), None);
        assert_eq!(Pic::avatar("1", "../x"), None);
        assert_eq!(Pic::icon("1", "a/b"), None);
    }

    #[test]
    fn a_kept_path_round_trips() {
        for pic in [Pic::avatar("5", "h").unwrap(), Pic::Default(2), Pic::icon("6", "g").unwrap()] {
            assert_eq!(Pic::from_path(&pic.path()), Some(pic.clone()));
            assert_eq!(Pic::from_path(&format!("https://cdn.discordapp.com/{}?size=64", pic.path())), Some(pic));
        }
        assert_eq!(Pic::from_path("avatars/x/y.png"), None);
        assert_eq!(Pic::from_path("embed/avatars/9.png"), None);
        assert_eq!(Pic::from_path("nonsense"), None);
    }

    #[test]
    fn a_kept_file_is_used_without_the_network() {
        let dir = tempfile::tempdir().unwrap();
        let mut art = Art::new(Some(dir.path().to_path_buf()), None, Arc::new(|| {}));
        let pic = Pic::avatar("7", "hh").unwrap();
        assert_eq!(art.get(&pic), None, "no CDN, nothing kept");
        std::fs::create_dir_all(dir.path().join("avatars/7")).unwrap();
        std::fs::write(dir.path().join("avatars/7/hh.png"), b"made up").unwrap();
        let mut art = Art::new(Some(dir.path().to_path_buf()), None, Arc::new(|| {}));
        assert_eq!(art.get(&pic), Some(dir.path().join("avatars/7/hh.png")));
    }
}
