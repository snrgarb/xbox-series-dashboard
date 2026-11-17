//! Updates, the executable's side (docs/spec/updates.md): Settings > About's Check for updates and Install, and the
//! daily check, carried out off the frame's thread (`update` does the asking, the checking and the renaming); the
//! handover to the new version once it is in place; and, as the Dashboard starts, the old versions left by an
//! update removed.
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use model::update::{Updates, Version};
use shell_mode::dashboard::{Handover, HandoverEnd};

/// How long the new version has to show its first frame before it is taken for one that does not start.
const HANDOVER_LIMIT: Duration = Duration::from_secs(30);

/// How often the daily check looks at whether it is due.
const LOOK_EVERY: Duration = Duration::from_secs(60);

/// What a thread says.
pub enum Event {
    /// Where updates stand now.
    Changed(Updates),
    /// The new version has started: this one exits.
    HandedOver,
}

pub struct Updater {
    source: String,
    current: Version,
    /// This program, which an update replaces.
    exe: Option<PathBuf>,
    /// The arguments the new version is started with: this one's, but for --pretend-version and --handover.
    args: Vec<String>,
    state: Updates,
    send: mpsc::Sender<Event>,
    answers: mpsc::Receiver<Event>,
    last_check: Option<u64>,
    next_look: Instant,
    /// The version a Toast has told of being ready.
    told: Option<Version>,
}

impl Updater {
    /// `source`: --update-source, else the one address (`update::SOURCE`). `given`: this start's arguments.
    /// `last_check`: the settings row's (`LAST_UPDATE_CHECK`).
    pub fn new(source: Option<String>, current: Version, given: &[String], last_check: Option<String>) -> Self {
        let (send, answers) = mpsc::channel();
        Self {
            source: source.unwrap_or_else(|| update::SOURCE.to_string()),
            current,
            exe: std::env::current_exe().ok(),
            args: args_for_new(given),
            state: Updates::Unchecked,
            send,
            answers,
            last_check: last_check.and_then(|text| text.trim().parse().ok()),
            // A minute after the start, so a check never holds up the first frame or the stores' reading.
            next_look: Instant::now() + LOOK_EVERY,
            told: None,
        }
    }

    /// Asks the update address, and downloads and checks a newer version, on a thread of its own; `wake` once
    /// there is news. The time of the check, to be kept (`LAST_UPDATE_CHECK`); none while one is under way.
    pub fn check(&mut self, why: &str, wake: impl Fn() + Send + 'static) -> Option<u64> {
        if self.state.busy() {
            return None;
        }
        eprintln!("update: checking {} ({why}); this is {}", self.source, self.current);
        self.state = Updates::Checking;
        let (source, current, exe, send) = (self.source.clone(), self.current, self.exe.clone(), self.send.clone());
        std::thread::spawn(move || {
            let tell = |updates: Updates| {
                eprintln!("update: {}", updates.state());
                let _ = send.send(Event::Changed(updates));
                wake();
            };
            tell(look(&source, current, exe, &tell));
        });
        let now = now();
        self.last_check = Some(now);
        Some(now)
    }

    /// Puts the version ready in place and starts it, on a thread of its own: this one exits once it has shown
    /// its first frame (`Event::HandedOver`); one that does not start is put back.
    pub fn install(&mut self, wake: impl Fn() + Send + 'static) {
        let (Updates::Ready(version), Some(exe)) = (self.state, self.exe.clone()) else { return };
        self.state = Updates::Installing(version);
        eprintln!("update: installing {version} over {}", self.current);
        let (current, send) = (self.current, self.send.clone());
        let mut args = self.args.clone();
        args.extend([shell_mode::contract::HANDOVER.to_string(), std::process::id().to_string()]);
        std::thread::spawn(move || {
            let event = hand_over(&exe, current, version, &args);
            if let Event::Changed(updates) = &event {
                eprintln!("update: {}", updates.state());
            }
            let _ = send.send(event);
            wake();
        });
    }

    /// What the threads have said since the last look.
    pub fn poll(&mut self) -> Vec<Event> {
        let events: Vec<Event> = self.answers.try_iter().collect();
        for event in &events {
            if let Event::Changed(updates) = event {
                self.state = *updates;
            }
        }
        events
    }

    pub fn state(&self) -> Updates {
        self.state
    }

    /// Whether the daily check is due: it is on, none is under way, and a day has passed since the last. Looked at
    /// once a minute.
    pub fn due(&mut self, on: bool) -> bool {
        if !on || self.state.busy() || Instant::now() < self.next_look {
            return false;
        }
        self.next_look = Instant::now() + LOOK_EVERY;
        update::due(self.last_check, now())
    }

    /// The version ready that no Toast has told of yet; told of from now.
    pub fn untold(&mut self) -> Option<Version> {
        let Updates::Ready(version) = self.state else { return None };
        (self.told != Some(version)).then(|| {
            self.told = Some(version);
            version
        })
    }

    /// Whether a version is ready that no Toast has told of.
    pub fn owes_toast(&self) -> bool {
        matches!(self.state, Updates::Ready(version) if self.told != Some(version))
    }
}

/// Asks, and downloads and stages a newer version: where updates stand then. `tell` says each step on the way.
fn look(source: &str, current: Version, exe: Option<PathBuf>, tell: &dyn Fn(Updates)) -> Updates {
    let agent = update::agent();
    let release = match update::check(&agent, source, current) {
        Ok(update::Checked::NotPublic) => {
            eprintln!("update: the update source answered 404: the releases aren't public");
            return Updates::NotPublic;
        }
        Ok(update::Checked::UpToDate(newest)) => return Updates::UpToDate(newest),
        Ok(update::Checked::Newer(release)) => release,
        Err(e) => {
            eprintln!("update: couldn't check: {e}");
            return Updates::Failed;
        }
    };
    let version = release.version;
    tell(Updates::Downloading(version));
    let Some(exe) = exe else {
        eprintln!("update: found {version}, but this program's own path couldn't be had");
        return Updates::Failed;
    };
    match update::download(&agent, &release) {
        Ok(bytes) => match update::stage(&exe, &bytes) {
            Ok(staged) => {
                let hash = update::sha256(&bytes);
                eprintln!(
                    "update: {version}'s zip matches its manifest; dashboard.exe (SHA-256 {hash}) staged as {}",
                    staged.display()
                );
                Updates::Ready(version)
            }
            Err(e) => {
                eprintln!("update: {version} couldn't be staged beside {}: {e}", exe.display());
                Updates::Failed
            }
        },
        Err(update::Error::BadHash { want, got }) => {
            eprintln!(
                "update: refused {version}: its zip's SHA-256 is {got}, not the manifest's {want}; nothing was changed"
            );
            Updates::BadDownload(version)
        }
        Err(e) => {
            eprintln!("update: couldn't download {version}: {e}");
            Updates::Failed
        }
    }
}

/// Puts `version` in place of `exe` and starts it with `args`; the old one is kept until it has shown its first
/// frame, and put back if it does not.
fn hand_over(exe: &std::path::Path, current: Version, version: Version, args: &[String]) -> Event {
    let old = match update::put_in_place(exe, current) {
        Ok(old) => old,
        Err(e) => {
            eprintln!("update: {version} couldn't be put in place of {}: {e}; nothing was changed", exe.display());
            return Event::Changed(Updates::InstallFailed(version));
        }
    };
    eprintln!("update: {} is {version} now; {current} is kept as {}", exe.display(), old.display());
    let put_back = |why: &str| {
        eprintln!("update: {version} {why}: putting {current} back");
        match update::put_back(exe, &old, version) {
            Ok(()) => {
                eprintln!("update: {current} is back; {version} is kept as {}", update::refused(exe, version).display())
            }
            Err(e) => eprintln!("update: ERROR: {current} couldn't be put back: {e}"),
        }
        Event::Changed(Updates::RolledBack(version))
    };
    let handover = match Handover::start(exe, args) {
        Ok(handover) => handover,
        Err(e) => return put_back(&format!("couldn't be started ({e})")),
    };
    eprintln!(
        "update: started {version} (pid {}) at {:.0} ms with {}; waiting up to {} s for its first frame",
        handover.pid(),
        crate::since_start(),
        args.join(" "),
        HANDOVER_LIMIT.as_secs()
    );
    let waited = std::time::Instant::now();
    match handover.wait(HANDOVER_LIMIT) {
        HandoverEnd::Started => {
            eprintln!(
                "update: {version} (pid {}) has shown its first frame: this one exits ({} ms after its start)",
                handover.pid(),
                waited.elapsed().as_millis()
            );
            Event::HandedOver
        }
        HandoverEnd::Exited(code) => put_back(&format!("exited with {code:#x} before its first frame")),
        HandoverEnd::TimedOut => {
            handover.end();
            put_back("showed no first frame in time, and was ended")
        }
    }
}

/// This start's arguments as the new version's: all but `--pretend-version` and `--handover` and their values.
pub fn args_for_new(given: &[String]) -> Vec<String> {
    let mut args = Vec::new();
    let mut it = given.iter();
    while let Some(arg) = it.next() {
        if arg == "--pretend-version" || arg == shell_mode::contract::HANDOVER {
            it.next();
        } else {
            args.push(arg.clone());
        }
    }
    args
}

/// As a Dashboard starts that is not taking over from another: the versions an update kept beside this program
/// are removed where they can be (one still running, the --shell process's until sign-out, is left for later).
pub fn remove_old() {
    let Some(folder) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)) else { return };
    for removed in update::remove_old(&folder) {
        eprintln!("update: removed {}, kept by an update", removed.display());
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_new_version_is_started_as_this_one_was_but_for_what_only_this_one_had() {
        let given: Vec<String> =
            ["--data-dir", r"C:\t data", "--pretend-version", "0.0.1", "--handover", "12", "--from-shell"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        assert_eq!(args_for_new(&given), [r"--data-dir", r"C:\t data", "--from-shell"]);
    }
}
