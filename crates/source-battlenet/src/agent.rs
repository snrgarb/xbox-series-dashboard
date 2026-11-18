//! Battle.net's Agent (`Agent.exe`), the client's own downloader, through its local HTTP API: the route by which
//! the Dashboard installs, uninstalls and follows a Battle.net game with no window of Blizzard's (docs/research/
//! installs.md 7; docs/sources.md "Battle.net through its Agent"). Nothing of Blizzard's documents it; what is relied
//! on, and how each thing is known, is in [`calls`], the one place that says what is asked and how an answer reads,
//! so that what the TV PC shows to be otherwise is changed there alone.
//!
//! Every request and answer is one line of the log (`battlenet: agent: <what>: <METHOD> <path> <body> -> <status>
//! <answer>`), the session's token written `<token>`.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::http::{Answer, Http};
use crate::listening;

pub mod calls;

/// The file in the Agent's folder that holds the port it listens on, as one number (DroidDeck's `agent_port`,
/// bnetswitch's `read_agent_port`): the stand-in Agent writes its own there.
pub const PORT_FILE: &str = "Agent.dat";

/// The port the wowdev wiki gives as the Agent's default; reported to be another, random one in practice.
pub const USUAL_PORT: u16 = 1120;

/// The Agent's folder (`%ProgramData%\Battle.net\Agent`, or a fake's `Agent`), and whether it is a fake's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub dir: PathBuf,
    pub fake: bool,
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

    /// The ports to try, in order: the one `Agent.dat` names; then, for the machine's own and never a fake's, every
    /// port an `Agent.exe` under this folder listens on (barncastle's and HSCentric's way), then the wiki's 1120.
    pub fn ports(&self) -> Vec<u16> {
        let mut ports: Vec<u16> = self.written_port().into_iter().collect();
        if self.fake {
            return ports;
        }
        let listening = listening::all()
            .into_iter()
            .filter(|listener| listener.program.as_deref().is_some_and(|program| self.owns(program)))
            .map(|listener| listener.port);
        for port in listening.chain([USUAL_PORT]) {
            if !ports.contains(&port) {
                ports.push(port);
            }
        }
        ports
    }

    /// Whether a running program is this folder's Agent.
    fn owns(&self, program: &Path) -> bool {
        let named = program.file_name().is_some_and(|name| name.eq_ignore_ascii_case("Agent.exe"));
        named && crate::under(&self.dir, program)
    }
}

/// A session with the Agent: its port and its token.
pub struct Session {
    pub http: Http,
    /// `GET /agent`'s `region`: which patch server installs are fetched from.
    pub region: String,
    said_once: bool,
}

/// How long the Agent is waited for once started.
const START_WAIT: Duration = Duration::from_secs(20);

/// A session with the Agent: on the first port that gives a token, else after starting the Agent itself
/// (`Agent.exe` with [`calls::START_ARGS`], no window) and waiting for it. Why not, said as the log will say it.
pub fn connect(place: &Place) -> Result<Session, String> {
    let mut last = String::from("nothing listened");
    for port in place.ports() {
        match token(port) {
            Ok(session) => return Ok(session),
            Err(why) => last = why,
        }
    }
    let program = place.program().ok_or_else(|| format!("no Agent.<build>\\Agent.exe in {}", place.dir.display()))?;
    crate::log(&format!(
        "agent: not answering ({last}): starting {} {}",
        program.display(),
        calls::START_ARGS.join(" ")
    ));
    start(&program)?;
    let begun = Instant::now();
    while begun.elapsed() < START_WAIT {
        std::thread::sleep(Duration::from_millis(500));
        for port in place.ports() {
            match token(port) {
                Ok(session) => {
                    crate::log(&format!("agent: up on port {port} after {} ms", begun.elapsed().as_millis()));
                    return Ok(session);
                }
                Err(why) => last = why,
            }
        }
    }
    Err(format!("started, and gave no token within {} s: {last}", START_WAIT.as_secs()))
}

/// `GET /agent` on `port`, and its token.
fn token(port: u16) -> Result<Session, String> {
    let mut http = Http::new(port);
    let answer = http.call("session", "GET", calls::SESSION, None)?;
    let token = calls::token(&answer).ok_or_else(|| format!("port {port} gave no token ({})", answer.status))?;
    http.token = Some(token);
    let region = answer.text("region").unwrap_or("us").to_string();
    Ok(Session { http, region, said_once: false })
}

fn start(program: &Path) -> Result<(), String> {
    let mut command = std::process::Command::new(program);
    command.args(calls::START_ARGS).current_dir(program.parent().unwrap_or(Path::new(".")));
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().map(drop).map_err(|e| format!("{} could not be started: {e}", program.display()))
}

impl Session {
    /// One call, again once with a new token where the Agent says the old one is no good (401, 403).
    pub fn call(&mut self, what: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Answer, String> {
        let answer = self.http.call(what, method, path, body)?;
        if !calls::refused(&answer) {
            return Ok(answer);
        }
        self.http = token(self.http.port())?.http;
        self.http.call(what, method, path, body)
    }

    /// A `GET` said in the log only the first time, and where it goes wrong: the progress asked once a second.
    pub fn call_quiet(&mut self, what: &str, method: &str, path: &str) -> Result<Answer, String> {
        let say = !self.said_once;
        self.said_once = true;
        let answer = self.http.call_said(what, method, path, None, say)?;
        if !calls::refused(&answer) {
            return Ok(answer);
        }
        self.http = token(self.http.port())?.http;
        self.http.call(what, method, path, None)
    }
}
