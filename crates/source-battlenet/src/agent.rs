//! Battle.net's Agent (`Agent.exe`), the client's own downloader, through its local HTTP API: the route by which
//! the Dashboard installs, uninstalls and follows a Battle.net game with no window of Blizzard's (docs/research/
//! installs.md 7; docs/sources.md "Battle.net through its Agent"). Nothing of Blizzard's documents it; what is relied
//! on, and how each thing is known, is in [`calls`], the one place that says what is asked and how an answer reads,
//! so that what the TV PC shows to be otherwise is changed there alone.
//!
//! Every request and answer is one line of the log (`battlenet: agent: <what>: <METHOD> <path> <body> -> <status>
//! <answer>`), the session's token, and the account's and Windows' names for the user, written masked (`http`).
//!
//! **Which Agent is asked** (#172, after the TV PC's first try): the one answering on the port `Agent.dat` names,
//! else on a port an `Agent.exe` of this folder listens on, else on the wiki's 1120, where one gives a token. One that
//! answers and gives none (the client's own Agent, started by Battle.net with its `--session`, answers 401 to anyone
//! else) is asked once more with its process id in a `pid` header, as redact7's bnetinstaller asks; and if it still
//! gives none, **no Agent is started beside it**: two Agents would both write `product.db`, and DroidDeck, the one
//! tool that thought of it, waits for Battle.net to close before starting its own. The client's way is taken instead,
//! said with why. Nor is one started while `Battle.net.exe`, or an `Agent.exe` of this folder that answers on no
//! port, runs. Only where nothing of Battle.net's runs does the Dashboard start the Agent itself (`--internalclienttools`,
//! no window: barncastle's and HSCentric's way), find it by its own process's port, and **stop it** once nothing
//! of the Dashboard's uses it: `DELETE /agent` (barncastle's end), then killed if it has not gone 5 s on (30 s once
//! an install it did is over: [`GRACE_AFTER_INSTALL`]). An install it took holds it until the install is over (`installing`): never while a download is under way, which stopping it
//! would end. On Windows it is in a job object that kills it when the Dashboard's process ends, however it ends (and
//! with it any download it had: docs/sources.md, "When the Dashboard ends mid-download"). An Agent the Dashboard did
//! not start is never stopped.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::http::{Answer, Http};
use crate::listening;

pub mod calls;

/// The file in the Agent's folder that holds the port it listens on, as one number (DroidDeck's `agent_port`,
/// bnetswitch's `read_agent_port`; on the TV PC, the client's Agent's port): the stand-in Agent writes its own there.
pub const PORT_FILE: &str = "Agent.dat";

/// The port the wowdev wiki gives as the Agent's default; reported to be another, random one in practice.
pub const USUAL_PORT: u16 = 1120;

/// The Agent's folder (`%ProgramData%\Battle.net\Agent`, or a fake's `Agent`), and whether it is a fake's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub dir: PathBuf,
    pub fake: bool,
}

/// A port to try, and why it is tried, as the log says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    pub port: u16,
    pub whence: &'static str,
    /// The process listening on it, where the table of listeners says.
    pub pid: Option<u32>,
}

impl Place {
    /// The newest `Agent.<build>\Agent.exe` in the folder, by its build number (`Agent.exe` beside them is only a
    /// stub that starts it: barncastle's `AgentApp`, DroidDeck).
    pub fn program(&self) -> Option<PathBuf> {
        let mut builds: Vec<(u64, PathBuf)> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let build = name.strip_prefix("Agent.")?.parse::<u64>().ok()?;
                let exe = entry.path().join("Agent.exe");
                exe.is_file().then_some((build, exe))
            })
            .collect();
        builds.sort();
        builds.pop().map(|(_, exe)| exe)
    }

    /// Whether the Agent is on this machine at all.
    pub fn there(&self) -> bool {
        self.program().is_some()
    }

    /// The port `Agent.dat` names, where it does.
    pub fn written_port(&self) -> Option<u16> {
        std::fs::read_to_string(self.dir.join(PORT_FILE)).ok()?.trim().parse().ok()
    }

    /// The ports to try, in order: the one `Agent.dat` names; then every port an `Agent.exe` of this folder listens
    /// on (barncastle's and HSCentric's way); then, for the machine's own and never a fake's, the wiki's 1120.
    pub fn ports(&self) -> Vec<Port> {
        let listening: Vec<listening::Listener> = listening::all()
            .into_iter()
            .filter(|listener| listener.program.as_deref().is_some_and(|program| self.owns(program)))
            .collect();
        let pid_on = |port: u16| listening.iter().find(|l| l.port == port).map(|l| l.pid);
        let mut ports: Vec<Port> =
            self.written_port().map(|port| Port { port, whence: "Agent.dat", pid: pid_on(port) }).into_iter().collect();
        let mut add = |port: Port| {
            if !ports.iter().any(|p| p.port == port.port) {
                ports.push(port);
            }
        };
        for listener in &listening {
            add(Port { port: listener.port, whence: "an Agent.exe listens", pid: Some(listener.pid) });
        }
        if !self.fake {
            add(Port { port: USUAL_PORT, whence: "the wiki's", pid: pid_on(USUAL_PORT) });
        }
        ports
    }

    /// Whether a running program is this folder's Agent.
    fn owns(&self, program: &Path) -> bool {
        let named = program.file_name().is_some_and(|name| name.eq_ignore_ascii_case("Agent.exe"));
        named && crate::under(&self.dir, program)
    }

    /// What of Battle.net's runs that the Dashboard did not start, by which no Agent is started: `Battle.net.exe`
    /// (for a fake, only its own) and this folder's `Agent.exe`.
    pub fn others_running(&self, ours: Option<u32>) -> Vec<String> {
        let root = self.dir.parent().map(Path::to_path_buf).unwrap_or_default();
        listening::processes()
            .into_iter()
            .filter(|p| Some(p.pid) != ours)
            .filter(|p| {
                let client = p.name.eq_ignore_ascii_case(crate::CLIENT_EXE)
                    && (!self.fake || p.program.as_deref().is_some_and(|path| crate::under(&root, path)));
                let agent = p.program.as_deref().is_some_and(|path| self.owns(path));
                client || agent
            })
            .map(|p| format!("{} (pid {})", p.name, p.pid))
            .collect()
    }
}

/// An Agent the Dashboard started: its process, its port, and how many sessions use it.
struct Owned {
    child: Child,
    port: u16,
    users: usize,
    /// How long it is given to go by itself after `DELETE /agent` before it is killed ([`GRACE`], or
    /// [`GRACE_AFTER_INSTALL`] once an install it did is over).
    grace: Duration,
    /// The job object that kills it with the Dashboard (Windows), closed once it is stopped.
    #[cfg(windows)]
    job: Option<windows_sys::Win32::Foundation::HANDLE>,
}

// SAFETY: the job's handle is only closed, once, under the lock.
unsafe impl Send for Owned {}

/// The Agents the Dashboard started, by their folder.
static OWNED: Mutex<Option<HashMap<PathBuf, Owned>>> = Mutex::new(None);
/// Held while a session is made, so that two installs at once never start two Agents.
static CONNECTING: Mutex<()> = Mutex::new(());

fn owned() -> std::sync::MutexGuard<'static, Option<HashMap<PathBuf, Owned>>> {
    OWNED.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A session's hold on an Agent the Dashboard started: the last one let go stops it.
#[derive(Debug)]
pub struct Lease {
    dir: PathBuf,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let stop = {
            let mut owned = owned();
            let agents = owned.get_or_insert_with(HashMap::new);
            match agents.get_mut(&self.dir) {
                Some(agent) if agent.users > 1 => {
                    agent.users -= 1;
                    None
                }
                Some(_) => agents.remove(&self.dir),
                None => None,
            }
        };
        if let Some(agent) = stop {
            stop_owned(agent);
        }
    }
}

/// Whether the Dashboard has an Agent of its own running for `dir`, and its process id.
pub fn ours(dir: &Path) -> Option<u32> {
    owned().as_ref()?.get(dir).map(|agent| agent.child.id())
}

/// Whether the Agent the Dashboard started for `dir` still runs: None where it started none.
pub fn alive(dir: &Path) -> Option<bool> {
    let mut owned = owned();
    let agent = owned.as_mut()?.get_mut(dir)?;
    Some(matches!(agent.child.try_wait(), Ok(None)))
}

/// How long an Agent the Dashboard started is given to go by itself after `DELETE /agent` before it is killed:
/// where nothing it did is to be kept (a form abandoned, a download cancelled, an uninstall seen done).
pub const GRACE: Duration = Duration::from_secs(5);

/// The same once an install it did is over (#172). On the TV PC the Agent was still running 5 s after `DELETE
/// /agent` both times, and was killed; after the first real install the client then showed the game as not
/// installed. Whether the kill cut off a last write is not known (the client's uid, not this, is what is proven), but
/// what the Agent does after the download (product.db, the Uninstall key, shortcuts, `aggregate.json`) is its own,
/// and half a minute lets it finish while still never leaving it running. The log says how long it took.
pub const GRACE_AFTER_INSTALL: Duration = Duration::from_secs(30);

/// Stops an Agent the Dashboard started: `DELETE /agent`, then killed where it has not gone within its grace.
fn stop_owned(mut agent: Owned) {
    let pid = agent.child.id();
    let (method, path) = calls::stop();
    let asked = token_on(agent.port, None)
        .map_err(|why| why.to_string())
        .and_then(|session| session.http.call("stop", method, path, None));
    let begun = Instant::now();
    let mut gone = None;
    while begun.elapsed() < agent.grace {
        if let Ok(Some(status)) = agent.child.try_wait() {
            gone = Some(status);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let how = match (gone, asked) {
        (Some(_), Ok(_)) => format!("it exited {} ms after DELETE /agent", begun.elapsed().as_millis()),
        (Some(_), Err(why)) => format!("it had exited (DELETE /agent: {why})"),
        (None, _) => match agent.child.kill().and_then(|()| agent.child.wait()) {
            Ok(_) => format!("killed: still running {} s after DELETE /agent", agent.grace.as_secs()),
            Err(e) => format!("could not be killed: {e}"),
        },
    };
    crate::log(&format!("agent: the Agent the Dashboard started (pid {pid}, port {}) stopped: {how}", agent.port));
    #[cfg(windows)]
    if let Some(job) = agent.job.take() {
        // SAFETY: the job's own handle, closed once; the Agent in it is gone.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(job) };
    }
}

/// A session with the Agent: its port and its token, and a hold on the Agent where the Dashboard started it.
pub struct Session {
    pub http: Http,
    /// `GET /agent`'s `region`: which patch server installs are fetched from.
    pub region: String,
    lease: Option<Lease>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Session(port {}, ours {})", self.http.port(), self.lease.is_some())
    }
}

/// How long the Agent is waited for once started.
const START_WAIT: Duration = Duration::from_secs(20);

/// Why a port gave no token.
enum NoToken {
    /// It answered, with this status, and no token: an Agent that takes no outside caller.
    Answered(u16),
    /// Nothing answered.
    Silent(String),
}

impl std::fmt::Display for NoToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoToken::Answered(status) => write!(f, "{status}, no token"),
            NoToken::Silent(why) => write!(f, "no answer ({why})"),
        }
    }
}

/// A session with the Agent, by the rule above. Why not, said as the log will say it.
pub fn connect(place: &Place) -> Result<Session, String> {
    let _one_at_a_time = CONNECTING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // The Agent the Dashboard started, while it runs.
    if let Some(session) = ours_again(place) {
        return Ok(session);
    }
    let mut tried = Vec::new();
    let mut answering: Option<Port> = None;
    for port in place.ports() {
        match token_on(port.port, None) {
            Ok(session) => return Ok(session),
            Err(why) => {
                tried.push(format!("port {} ({}): {why}", port.port, port.whence));
                if matches!(why, NoToken::Answered(_)) && answering.is_none() {
                    answering = Some(port);
                }
            }
        }
    }
    if let Some(port) = answering {
        // Asked again with its process id, as redact7's bnetinstaller asks the Agent it finds running.
        if let Some(pid) = port.pid {
            match token_on(port.port, Some(pid)) {
                Ok(session) => return Ok(session),
                Err(why) => tried.push(format!("port {} with pid {pid}: {why}", port.port)),
            }
        }
        let why = format!(
            "an Agent the Dashboard did not start answers on port {} and gives it no token ({}): Battle.net's own; no \
             second Agent is started beside it",
            port.port,
            tried.join("; ")
        );
        crate::log(&format!("agent: {why}"));
        return Err(why);
    }
    let running = place.others_running(None);
    if !running.is_empty() {
        let why = format!(
            "{} running and no Agent answering ({}): no Agent is started beside Battle.net's",
            running.join(", "),
            if tried.is_empty() { "no port to try".to_string() } else { tried.join("; ") }
        );
        crate::log(&format!("agent: {why}"));
        return Err(why);
    }
    let program = place.program().ok_or_else(|| format!("no Agent.<build>\\Agent.exe in {}", place.dir.display()))?;
    let last = if tried.is_empty() { "no port to try".to_string() } else { tried.join("; ") };
    crate::log(&format!(
        "agent: not answering ({last}): starting {} {}",
        program.display(),
        calls::START_ARGS.join(" ")
    ));
    let child = start(&program)?;
    let pid = child.id();
    let begun = Instant::now();
    let mut child = Some(child);
    let mut last = String::from("nothing listened");
    while begun.elapsed() < START_WAIT {
        std::thread::sleep(Duration::from_millis(500));
        if let Some(Ok(Some(status))) = child.as_mut().map(Child::try_wait) {
            return Err(format!("the Agent it started (pid {pid}) exited before it answered ({status})"));
        }
        // Its own port: the one its process listens on (barncastle, HSCentric), else what Agent.dat says now.
        let own = listening::all().into_iter().filter(|l| l.pid == pid).map(|l| l.port);
        let ports: Vec<u16> = own.chain(place.written_port()).collect();
        for port in ports {
            match token_on(port, None) {
                Ok(mut session) => {
                    crate::log(&format!(
                        "agent: up on port {port} after {} ms (pid {pid})",
                        begun.elapsed().as_millis()
                    ));
                    if let Some(child) = child.take() {
                        session.lease = Some(keep(place, child, port));
                    }
                    return Ok(session);
                }
                Err(why) => last = format!("port {port}: {why}"),
            }
        }
    }
    if let Some(child) = child.take() {
        // Started and never answered: not left running.
        stop_owned(Owned {
            child,
            port: 0,
            users: 0,
            grace: GRACE,
            #[cfg(windows)]
            job: None,
        });
    }
    Err(format!("started, and gave no token within {} s: {last}", START_WAIT.as_secs()))
}

/// The Agent the Dashboard started for this folder, while it still runs, and a session with it.
fn ours_again(place: &Place) -> Option<Session> {
    let port = {
        let mut owned = owned();
        let agent = owned.as_mut()?.get_mut(&place.dir)?;
        if !matches!(agent.child.try_wait(), Ok(None)) {
            return None;
        }
        agent.port
    };
    let mut session = token_on(port, None).ok()?;
    let mut owned = owned();
    let agent = owned.as_mut()?.get_mut(&place.dir)?;
    agent.users += 1;
    session.lease = Some(Lease { dir: place.dir.clone() });
    Some(session)
}

/// Keeps an Agent the Dashboard started, with its first session's hold on it.
fn keep(place: &Place, child: Child, port: u16) -> Lease {
    #[cfg(windows)]
    let job = tie_to_us(&child);
    let agent = Owned {
        child,
        port,
        users: 1,
        grace: GRACE,
        #[cfg(windows)]
        job,
    };
    owned().get_or_insert_with(HashMap::new).insert(place.dir.clone(), agent);
    Lease { dir: place.dir.clone() }
}

/// `GET /agent` on `port`, and its token; with `pid`, that header sent too.
fn token_on(port: u16, pid: Option<u32>) -> Result<Session, NoToken> {
    let mut http = Http::new(port);
    if let Some(pid) = pid {
        http.extra = Some((calls::PID_HEADER, pid.to_string()));
    }
    let what = if pid.is_some() { "session with its pid" } else { "session" };
    let answer = http.call(what, "GET", calls::SESSION, None).map_err(NoToken::Silent)?;
    let token = calls::token(&answer).ok_or(NoToken::Answered(answer.status))?;
    http.token = Some(token);
    let region = answer.text("region").unwrap_or("us").to_string();
    Ok(Session { http, region, lease: None })
}

fn start(program: &Path) -> Result<Child, String> {
    let mut command = std::process::Command::new(program);
    command.args(calls::START_ARGS).current_dir(program.parent().unwrap_or(Path::new(".")));
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().map_err(|e| format!("{} could not be started: {e}", program.display()))
}

/// Puts the Agent in a job object that kills it when its last handle closes: when the Dashboard's process ends,
/// however it ends. Where that cannot be done it is said, and the Agent is still stopped as usual.
#[cfg(windows)]
fn tie_to_us(child: &Child) -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    // SAFETY: plain calls on a job made here and the child's own process handle; the limits are a zeroed struct of
    // the size given, with one flag set.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            crate::log("agent: no job object for the Agent: it is stopped only when the Dashboard is done with it");
            return None;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if set == 0 || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0 {
            CloseHandle(job);
            crate::log(
                "agent: the Agent could not be tied to the Dashboard's process: it is stopped only when the \
                        Dashboard is done with it",
            );
            return None;
        }
        Some(job)
    }
}

impl Session {
    /// Whether the Dashboard started the Agent this session is with.
    pub fn ours(&self) -> bool {
        self.lease.is_some()
    }

    /// An install the Agent the Dashboard started did is over: when it is stopped, it is given
    /// [`GRACE_AFTER_INSTALL`] to go by itself.
    pub fn install_over(&self) {
        let Some(lease) = &self.lease else { return };
        if let Some(agent) = owned().as_mut().and_then(|agents| agents.get_mut(&lease.dir)) {
            agent.grace = agent.grace.max(GRACE_AFTER_INSTALL);
        }
    }

    /// One call, again once with a new token where the Agent says the old one is no good (401, 403).
    pub fn call(&mut self, what: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Answer, String> {
        let answer = self.http.call(what, method, path, body)?;
        if !calls::refused(&answer) {
            return Ok(answer);
        }
        self.renew()?;
        self.http.call(what, method, path, body)
    }

    /// A `GET` said nowhere, again once with a new token where the Agent says the old one is no good: the progress
    /// asked once a second, whose answers the follower says itself (`installing`).
    pub fn get_silent(&mut self, path: &str) -> Result<Answer, String> {
        let answer = self.http.fetch("GET", path)?;
        if !calls::refused(&answer) {
            return Ok(answer);
        }
        self.renew()?;
        self.http.fetch("GET", path)
    }

    /// Whether the Agent is still there: for one the Dashboard started, its process; for another, whether anything
    /// answers `GET /agent` on its port.
    pub fn agent_alive(&self, dir: &Path) -> bool {
        match alive(dir) {
            Some(alive) if self.ours() => alive,
            _ => self.http.fetch("GET", calls::SESSION).is_ok(),
        }
    }

    fn renew(&mut self) -> Result<(), String> {
        let extra = self.http.extra.clone();
        let pid = extra.and_then(|(_, pid)| pid.parse().ok());
        let fresh = token_on(self.http.port(), pid).map_err(|why| format!("a new token: {why}"))?;
        self.http = fresh.http;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{self, Fixture};
    use crate::standin;
    use std::sync::Arc;

    #[test]
    fn an_agent_that_answers_and_gives_no_token_is_battlenets_own_and_none_is_started_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        // The client's own Agent, as the TV PC's: 401 with no body to anyone else.
        let _client = standin::serve_as(&root, standin::Mode::Clients, Duration::from_millis(600), |_| {}).unwrap();
        let place = Place { dir: root.join("Agent"), fake: true };
        let why = connect(&place).unwrap_err();
        assert!(why.contains("did not start answers on port") && why.contains("Agent.dat): 401, no token"), "{why}");
        assert!(why.contains("no second Agent is started"), "{why}");
        assert_eq!(ours(&place.dir), None);
    }

    #[test]
    fn an_agent_the_dashboard_started_is_stopped_by_the_last_session_and_one_it_did_not_never() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("battlenet");
        fixture::write(&root, &Fixture::standard()).unwrap();
        let said = Arc::new(Mutex::new(Vec::<String>::new()));
        let kept = said.clone();
        let served =
            standin::serve(&root, Duration::from_millis(600), move |line| kept.lock().unwrap().push(line.to_string()))
                .unwrap();
        let place = Place { dir: root.join("Agent"), fake: true };
        // Not started here: no hold on it, and nothing stops it.
        let theirs = connect(&place).unwrap();
        assert!(!theirs.ours());
        drop(theirs);
        assert!(!said.lock().unwrap().iter().any(|l| l.contains("DELETE /agent")));

        // As though the Dashboard had started it: a process of its own stands for the Agent's, the stand-in's port
        // for its port. Two sessions; the second let go first.
        let child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sleep" })
            .args(if cfg!(windows) { vec!["/c", "ping", "-n", "30", "127.0.0.1"] } else { vec!["30"] })
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut first = token_on(served.port, None).ok().unwrap();
        first.lease = Some(keep(&place, child, served.port));
        let second = connect(&place).unwrap();
        assert!(second.ours() && first.ours());
        assert_eq!(ours(&place.dir), Some(pid));
        drop(second);
        assert_eq!(ours(&place.dir), Some(pid), "still held by the first");
        drop(first);
        assert_eq!(ours(&place.dir), None);
        let lines = said.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("DELETE /agent -> 200")), "{lines:?}");
    }
}
