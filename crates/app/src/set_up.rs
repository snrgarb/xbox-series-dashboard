//! Settings > Set up this PC, carried out (docs/spec/set-up.md, #178): each launcher's app as Windows' list of
//! installed programs has it, and on the user's Yes its installer downloaded, checked and run, or its own uninstaller
//! run, on a thread of its own, one at a time. The table is the `helpers` crate's (`clients`), or a test's
//! (`--client-table`); Windows' list is the registry's, or a test's file standing in for it (`--setup-registry`).
//! Every step says one `set-up:` line.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use helpers::clients::{self, Client, Outcome};
use model::setup::{ClientState, SetUpAsk};
use source_kit::registry::Registry;

/// Windows' list of installed programs, as it is now: the registry, or a test's file read afresh each time.
type List = Arc<dyn Fn() -> Registry + Send + Sync>;

/// How often a running step looks at Windows' list.
const TICK: Duration = Duration::from_millis(500);

/// The table: a test's (`--client-table`), else the `helpers` crate's own.
pub fn table(args: &crate::Args) -> Vec<Client> {
    match &args.client_table {
        Some(path) => clients::read_table(path).unwrap_or_else(|e| {
            eprintln!("set-up: --client-table {e}: no launchers' apps");
            Vec::new()
        }),
        None => clients::builtin(),
    }
}

/// Windows' list: a test's file (`--setup-registry`), read afresh each time and empty while there is none; else
/// this machine's registry.
pub fn list(args: &crate::Args) -> List {
    match args.setup_registry.clone() {
        Some(path) => Arc::new(move || Registry::load(&path).unwrap_or(Registry::File(Default::default()))),
        None => Arc::new(|| Registry::Windows),
    }
}

/// What the thread doing a step says.
enum Told {
    State(String, ClientState),
    /// The step is over: its launcher is to be read again.
    Over(String),
}

pub struct SetUp {
    table: Vec<Client>,
    /// The data folder, where an installer is kept while it runs; none, and nothing is downloaded.
    data: Option<PathBuf>,
    list: List,
    states: HashMap<String, ClientState>,
    told: (Sender<Told>, Receiver<Told>),
    /// What tells whether a program in a folder runs: `shell::running::under`, or a test's.
    running: fn(&Path) -> Option<String>,
}

impl SetUp {
    /// The table, and how each app stands in Windows' list now.
    pub fn new(table: Vec<Client>, data: Option<PathBuf>, list: List) -> Self {
        let mut set_up =
            Self { table, data, list, states: HashMap::new(), told: mpsc::channel(), running: shell::running::under };
        set_up.look();
        eprintln!("set-up: {}", set_up.line());
        set_up
    }

    pub fn for_args(args: &crate::Args, data: Option<PathBuf>) -> Self {
        Self::new(table(args), data, list(args))
    }

    /// Looks at Windows' list again for every app with nothing under way.
    pub fn look(&mut self) {
        let registry = (self.list)();
        for client in &self.table {
            let state = self.states.get(&client.key);
            if state.is_some_and(ClientState::busy) {
                continue;
            }
            let found = clients::entry(&registry, client);
            let now = match found.as_ref().map(|entry| entry.uninstall.is_some()) {
                Some(true) => ClientState::Installed,
                Some(false) => ClientState::InstalledFixed,
                None => ClientState::NotInstalled,
            };
            // A failure is said until the next step; whether it is installed is brought up to date under it.
            let now = match state {
                Some(ClientState::Failed { why, .. }) => {
                    ClientState::Failed { why: why.clone(), installed: now.installed() }
                }
                _ => now,
            };
            self.states.insert(client.key.clone(), now);
        }
    }

    /// `steam installed, epic not installed, ...`.
    fn line(&self) -> String {
        let said: Vec<String> = self
            .table
            .iter()
            .map(|client| {
                let state = match &self.states[&client.key] {
                    ClientState::NotInstalled => "not installed",
                    ClientState::Installed => "installed",
                    ClientState::InstalledFixed => "installed, with no uninstaller",
                    ClientState::Downloading { .. } => "downloading",
                    ClientState::Installing => "installing",
                    ClientState::Removing => "removing",
                    ClientState::Failed { .. } => "failed",
                };
                format!("{} {state}", client.key)
            })
            .collect();
        said.join(", ")
    }

    /// Every app as Settings shows it.
    pub fn shown(&self) -> Vec<model::setup::Client> {
        let state = |client: &Client| self.states.get(&client.key).cloned().unwrap_or_default();
        self.table.iter().map(|client| clients::shown(client, state(client))).collect()
    }

    /// Whether a step is under way, and whose.
    fn busy(&self) -> Option<&Client> {
        self.table.iter().find(|client| self.states.get(&client.key).is_some_and(ClientState::busy))
    }

    fn refuse(&mut self, key: &str, why: &str, installed: bool) {
        eprintln!("set-up: {key}: refused: {why}");
        self.states.insert(key.to_string(), ClientState::Failed { why: why.to_string(), installed });
    }

    /// Does what Settings asked, on a thread of its own; `wake` is called whenever how it stands changes. `playing`
    /// is the name of a game of the launcher's that is playing or being launched, if one is.
    pub fn ask(&mut self, ask: SetUpAsk, playing: Option<&str>, wake: impl Fn() + Send + Sync + 'static) {
        let key = match &ask {
            SetUpAsk::Install(key) | SetUpAsk::Remove(key) => key.clone(),
        };
        let Some(client) = self.table.iter().find(|client| client.key == key).cloned() else {
            eprintln!("set-up: {key} is not a launcher's app");
            return;
        };
        self.look();
        let installed = self.states.get(&key).is_some_and(ClientState::installed);
        if let Some(other) = self.busy() {
            let why = format!("Wait until {} is done.", other.name);
            return self.refuse(&key, &why, installed);
        }
        let wake = Arc::new(wake);
        match ask {
            SetUpAsk::Install(_) => {
                if installed {
                    eprintln!("set-up: {key}: installed already");
                    return;
                }
                let Some(data) = self.data.clone() else {
                    return self.refuse(&key, "There's no data folder to download it to.", false);
                };
                self.states.insert(key.clone(), ClientState::Downloading { done: 0 });
                let (tell, list) = (self.told.0.clone(), self.list.clone());
                std::thread::spawn(move || {
                    let say = |state: ClientState| {
                        let _ = tell.send(Told::State(client.key.clone(), state));
                        wake();
                    };
                    install(&data, &client, &list, &say);
                    let _ = tell.send(Told::Over(client.key.clone()));
                    wake();
                });
            }
            SetUpAsk::Remove(_) => {
                if let Some(game) = playing {
                    let why = format!("Quit {game} before removing {}.", client.name);
                    return self.refuse(&key, &why, installed);
                }
                let registry = (self.list)();
                let Some(entry) = clients::entry(&registry, &client) else {
                    return self.refuse(&key, "Windows doesn't list it as installed.", false);
                };
                let Some(uninstall) = entry.uninstall.clone() else {
                    return self.refuse(&key, "It has no uninstaller: remove it in Windows' Settings.", true);
                };
                if let Some(program) = entry.location.as_deref().and_then(self.running) {
                    let why = format!("Close {} first: {program} is running.", client.name);
                    return self.refuse(&key, &why, true);
                }
                self.states.insert(key.clone(), ClientState::Removing);
                let (tell, list) = (self.told.0.clone(), self.list.clone());
                std::thread::spawn(move || {
                    let state = remove(&client, &uninstall, &list);
                    let _ = tell.send(Told::State(client.key.clone(), state));
                    let _ = tell.send(Told::Over(client.key.clone()));
                    wake();
                });
            }
        }
    }

    /// Takes what the threads have said: whether anything changed, and the launchers whose step is over, to be read
    /// again.
    pub fn take(&mut self) -> (bool, Vec<String>) {
        let (mut changed, mut over) = (false, Vec::new());
        while let Ok(told) = self.told.1.try_recv() {
            changed = true;
            match told {
                Told::State(key, state) => {
                    self.states.insert(key, state);
                }
                Told::Over(key) => over.push(key),
            }
        }
        if !over.is_empty() {
            eprintln!("set-up: {}", self.line());
        }
        (changed, over)
    }
}

/// Downloads, checks and runs the app's installer, saying each step, until Windows' list has the app.
fn install(data: &Path, client: &Client, list: &List, say: &dyn Fn(ClientState)) {
    let key = &client.key;
    let failed = |why: String| say(ClientState::Failed { why, installed: false });
    eprintln!(
        "set-up: {key}: downloading {} {} from {} ({} bytes)",
        client.name, client.version, client.url, client.size
    );
    let mut last = 0;
    let installer = match clients::download(&helpers::agent(), data, client, &mut |done| {
        // A step a 2 % at most, so the page is not told every megabyte of a large one.
        if done == client.size || done.saturating_sub(last) * 50 >= client.size.max(1) {
            last = done;
            say(ClientState::Downloading { done });
        }
    }) {
        Ok(installer) => installer,
        Err(e) => {
            eprintln!("set-up: {key}: refused: {e}");
            return failed(e.for_the_user());
        }
    };
    eprintln!("set-up: {key}: checked and kept at {}", installer.display());
    say(ClientState::Installing);
    let command = clients::install_command(client, &installer);
    let outcome = run(key, &command, list, client, true, clients::INSTALL_MOST);
    // The installer is not kept: one still running (as some do, with the app they started) is left to it.
    match std::fs::remove_dir_all(clients::folder(data, client)) {
        Ok(()) => eprintln!("set-up: {key}: installer deleted"),
        Err(e) => eprintln!("set-up: {key}: installer not deleted yet: {e}"),
    }
    match outcome {
        Outcome::Done => {
            let entry = clients::entry(&list(), client);
            let removable = entry.as_ref().is_some_and(|entry| entry.uninstall.is_some());
            eprintln!("set-up: {key}: installed: {}", entry.map(|entry| entry.key).unwrap_or_default());
            say(if removable { ClientState::Installed } else { ClientState::InstalledFixed });
        }
        Outcome::NotDone(code) => {
            let code = code.map_or("no code".to_string(), |code| code.to_string());
            eprintln!("set-up: {key}: not installed: its installer ended with {code}");
            failed(format!("It wasn't installed: its installer ended with {code}. The Dashboard's log says more."))
        }
        Outcome::TooLong => {
            eprintln!("set-up: {key}: not installed after {} minutes", clients::INSTALL_MOST.as_secs() / 60);
            failed("Its installer is taking too long: it's left to finish on its own.".into())
        }
    }
}

/// Runs the app's own uninstaller until Windows' list no longer has it. How it stands then.
fn remove(client: &Client, uninstall: &clients::Uninstall, list: &List) -> ClientState {
    let key = &client.key;
    match run(key, uninstall, list, client, false, clients::REMOVE_MOST) {
        Outcome::Done => {
            eprintln!("set-up: {key}: removed");
            ClientState::NotInstalled
        }
        Outcome::NotDone(code) => {
            let code = code.map_or("no code".to_string(), |code| code.to_string());
            eprintln!("set-up: {key}: still installed: its uninstaller ended with {code}");
            let why = format!("It's still installed: its uninstaller ended with {code}.");
            ClientState::Failed { why, installed: true }
        }
        Outcome::TooLong => {
            eprintln!("set-up: {key}: still installed after {} minutes", clients::REMOVE_MOST.as_secs() / 60);
            ClientState::Failed { why: "Its uninstaller is taking too long.".into(), installed: true }
        }
    }
}

/// Starts the step's program and follows it until Windows' list says the app is `installed` (or not).
fn run(
    key: &str,
    command: &clients::Uninstall,
    list: &List,
    client: &Client,
    installed: bool,
    most: Duration,
) -> Outcome {
    let name = command.program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let how = if command.elevate { ", through Windows' permission prompt" } else { "" };
    let line = if command.line.is_empty() { String::new() } else { format!(" {}", command.line) };
    eprintln!("set-up: {key}: running {name}{line}{how}");
    let mut started = match clients::start(command, &[]) {
        Ok(started) => started,
        Err(e) => {
            eprintln!("set-up: {key}: {name} did not start: {e}");
            return Outcome::NotDone(None);
        }
    };
    if started.elevated && !command.elevate {
        eprintln!("set-up: {key}: {name} wants an administrator: Windows asks on screen");
    }
    eprintln!("set-up: {key}: {name} started (pid {})", started.pid());
    let mut done = || clients::entry(&list(), client).is_some() == installed;
    clients::follow(&mut started, &mut done, most, TICK, &mut |said| eprintln!("set-up: {key}: {name} {said}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_list() -> List {
        Arc::new(|| Registry::File(Default::default()))
    }

    #[test]
    fn a_wrong_checksum_is_refused_and_nothing_is_run() {
        let service = helpers::fake::FakeHelpers::start(0, b"not an installer".to_vec()).unwrap();
        let data = tempfile::tempdir().unwrap();
        let mut set_up = SetUp::new(service.client_table(data.path()), Some(data.path().into()), no_list());
        assert!(set_up.shown().iter().all(|client| client.state == ClientState::NotInstalled));
        let (tell, heard) = mpsc::channel();
        set_up.ask(SetUpAsk::Install("gog".into()), None, move || {
            let _ = tell.send(());
        });
        assert!(matches!(set_up.shown()[2].state, ClientState::Downloading { .. }));
        let until = std::time::Instant::now() + Duration::from_secs(20);
        let over = loop {
            let _ = heard.recv_timeout(Duration::from_secs(1));
            let (_, over) = set_up.take();
            if !over.is_empty() || std::time::Instant::now() > until {
                break over;
            }
        };
        assert_eq!(over, ["gog"]);
        let ClientState::Failed { why, installed } = &set_up.shown()[2].state else { panic!("{:?}", set_up.shown()) };
        assert!(why.contains("wasn't the file") && !installed, "{why}");
        assert!(!clients::folder(data.path(), &set_up.table[2]).exists());
    }

    #[test]
    fn removing_is_refused_while_a_game_plays_while_another_step_works_and_when_windows_lists_nothing() {
        let data = tempfile::tempdir().unwrap();
        let mut set_up = SetUp::new(clients::builtin(), Some(data.path().into()), no_list());
        set_up.ask(SetUpAsk::Remove("steam".into()), Some("Halyard"), || {});
        let failed = |why: &str| ClientState::Failed { why: why.into(), installed: false };
        assert_eq!(set_up.shown()[0].state, failed("Quit Halyard before removing Steam."));
        set_up.ask(SetUpAsk::Remove("steam".into()), None, || {});
        assert_eq!(set_up.shown()[0].state, failed("Windows doesn't list it as installed."));
        set_up.states.insert("epic".into(), ClientState::Installing);
        set_up.ask(SetUpAsk::Install("gog".into()), None, || {});
        assert_eq!(set_up.shown()[2].state, failed("Wait until Epic Games Launcher is done."));
    }
}
