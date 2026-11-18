//! Settings > Helpers, carried out (docs/spec/helpers.md): each helper's state on this PC, and on the user's Yes its
//! fetch, check and first run on a thread of its own, or its removal. The table is the `helpers` crate's, or a
//! test's (`--helper-table`); the helpers are kept under the data folder. Every step says one `helpers:` line.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use model::helper::{Account, HelperAsk, HelperState};
use model::Source;

/// How long a helper's first run may take to say its version.
const FIRST_RUN: Duration = Duration::from_secs(30);

/// The table: a test's (`--helper-table`), else the `helpers` crate's own.
pub fn table(args: &crate::Args) -> Vec<helpers::Helper> {
    match &args.helper_table {
        Some(path) => helpers::read_table(path).unwrap_or_else(|e| {
            eprintln!("helpers: --helper-table {e}: no helpers");
            Vec::new()
        }),
        None => helpers::builtin(),
    }
}

pub struct Helpers {
    table: Vec<helpers::Helper>,
    /// The data folder; none, and nothing is fetched or kept.
    data: Option<PathBuf>,
    states: HashMap<String, HelperState>,
    told: (Sender<Told>, Receiver<Told>),
    /// How each helper's sign-in stands, read from its own records when Settings is shown it.
    sign_ins: HashMap<String, Box<dyn Fn() -> Account>>,
    /// The stores whose lists of owned games a helper brought up to date, to be read again.
    refreshed: (Sender<Vec<String>>, Receiver<Vec<String>>),
}

/// A helper's key and how it stands now, as the thread that fetched it says.
type Told = (String, HelperState);

impl Helpers {
    /// The table, and how each of its helpers stands in `data` now: added, if it is kept at the table's version.
    pub fn new(table: Vec<helpers::Helper>, data: Option<PathBuf>) -> Self {
        let states = table
            .iter()
            .map(|helper| {
                let kept = data.as_deref().and_then(|data| helpers::kept(data, helper));
                let state = match kept {
                    Some(kept) if kept.version == helper.version => HelperState::Added { version: kept.version },
                    Some(kept) => {
                        eprintln!(
                            "helpers: {} {} is kept, not {}: add it again",
                            helper.key, kept.version, helper.version
                        );
                        HelperState::NotAdded
                    }
                    None => HelperState::NotAdded,
                };
                (helper.key.clone(), state)
            })
            .collect();
        let (told, sign_ins, refreshed) = (mpsc::channel(), HashMap::new(), mpsc::channel());
        let helpers = Self { table, data, states, told, sign_ins, refreshed };
        eprintln!("helpers: {}", helpers.line());
        helpers
    }

    /// The Dashboard's: the table (`table`), and each helper's sign-in read where its store's route says.
    pub fn for_args(args: &crate::Args, data: Option<PathBuf>) -> Self {
        let mut helpers = Self::new(table(args), data);
        if let Some(home) = crate::sources::legendary_home(args) {
            helpers.sign_ins.insert("legendary".into(), Box::new(move || legendary_account(&home)));
        }
        if let Some(home) = crate::sources::gogdl_home(args) {
            helpers.sign_ins.insert("gogdl".into(), Box::new(move || gogdl_account(&home)));
        }
        // How each added helper's sign-in stands at start: `helpers: legendary: signed in as Stand-in Player`.
        for shown in helpers.shown() {
            match shown.account {
                Account::Unknown => {}
                Account::SignedIn(name) if name.is_empty() => eprintln!("helpers: {}: signed in", shown.key),
                Account::SignedIn(name) => eprintln!("helpers: {}: signed in as {name}", shown.key),
                Account::SignedOut(_) => eprintln!("helpers: {}: not signed in", shown.key),
            }
        }
        helpers
    }

    /// Brings each store's list of owned games up to date through its helper (`Installer::refresh`) and its own
    /// owned-list file (`Source::refresh_owned`, #276), on a thread of its own; `wake` is called when it is over,
    /// and `refreshed` then names those to be read again.
    pub fn refresh(&self, sources: &[Arc<dyn Source>], wake: impl Fn() + Send + 'static) {
        let (tell, sources) = (self.refreshed.0.clone(), sources.to_vec());
        std::thread::spawn(move || {
            let keys: Vec<String> = sources
                .iter()
                .filter(|source| {
                    let helper = source.installer().is_some_and(|installer| installer.refresh());
                    let owned = source.refresh_owned();
                    helper || owned
                })
                .map(|source| source.key().to_string())
                .collect();
            if !keys.is_empty() {
                eprintln!("helpers: owned games listed again for {}", keys.join(", "));
                let _ = tell.send(keys);
                wake();
            }
        });
    }

    /// The stores a refresh brought up to date since last asked.
    pub fn refreshed(&mut self) -> Vec<String> {
        self.refreshed.1.try_iter().flatten().collect()
    }

    /// `legendary 0.21.1 not added, gogdl 1.4.0 added`.
    fn line(&self) -> String {
        let said: Vec<String> = self
            .table
            .iter()
            .map(|helper| {
                let state = match &self.states[&helper.key] {
                    HelperState::NotAdded => "not added".to_string(),
                    HelperState::Fetching => "fetching".into(),
                    HelperState::Added { .. } => "added".into(),
                    HelperState::Failed(_) => "failed".into(),
                };
                format!("{} {} {state}", helper.key, helper.version)
            })
            .collect();
        said.join(", ")
    }

    /// Every helper as Settings shows it.
    pub fn shown(&self) -> Vec<model::helper::Helper> {
        let shown = |helper: &helpers::Helper| {
            let state = self.states[&helper.key].clone();
            let added = matches!(state, HelperState::Added { .. });
            let account = self.sign_ins.get(&helper.key).filter(|_| added).map(|read| read()).unwrap_or_default();
            model::helper::Helper { account, ..helpers::shown(helper, state) }
        };
        self.table.iter().map(shown).collect()
    }

    /// Does what Settings asked. A fetch goes on a thread of its own; `wake` is called when it is over.
    pub fn ask(&mut self, ask: HelperAsk, wake: impl Fn() + Send + 'static) {
        let key = match &ask {
            HelperAsk::Add(key) | HelperAsk::Remove(key) => key.clone(),
        };
        let Some(helper) = self.table.iter().find(|helper| helper.key == key).cloned() else {
            eprintln!("helpers: {key} is not a helper");
            return;
        };
        let Some(data) = self.data.clone() else {
            eprintln!("helpers: {key}: there is no data folder to keep it in");
            return;
        };
        match ask {
            HelperAsk::Remove(_) => {
                let state = match helpers::remove(&data, &helper) {
                    Ok(()) => {
                        eprintln!("helpers: {key}: removed {}", helpers::folder(&data, &helper).display());
                        HelperState::NotAdded
                    }
                    Err(e) => {
                        eprintln!("helpers: {key}: could not be removed: {e}");
                        HelperState::Failed("It couldn't be removed. The Dashboard's log says why.".into())
                    }
                };
                self.states.insert(key, state);
            }
            HelperAsk::Add(_) => {
                if matches!(self.states.get(&key), Some(HelperState::Fetching)) {
                    return;
                }
                self.states.insert(key.clone(), HelperState::Fetching);
                let tell = self.told.0.clone();
                std::thread::spawn(move || {
                    let state = add(&data, &helper);
                    let _ = tell.send((helper.key.clone(), state));
                    wake();
                });
            }
        }
    }

    /// Takes what the threads have said: whether anything changed.
    pub fn take(&mut self) -> bool {
        let mut changed = false;
        while let Ok((key, state)) = self.told.1.try_recv() {
            self.states.insert(key, state);
            changed = true;
        }
        changed
    }
}

/// A wake for the window's loop, from another thread.
pub fn wake(proxy: winit::event_loop::EventLoopProxy<()>) -> impl Fn() + Send + 'static {
    move || {
        let _ = proxy.send_event(());
    }
}

/// What signing legendary in takes, said where it is not: legendary's own `auth`, which shows Epic's sign-in page in
/// a window of its own. The Dashboard shows no sign-in page of its own yet (docs/spec/helpers.md 5).
const LEGENDARY_SIGN_IN: &str = "that takes Epic's sign-in page, which the Dashboard doesn't show yet. Run \
                                 \"legendary auth\" from the helper's folder once, with a keyboard. Until then the \
                                 Epic Games Launcher installs.";

fn legendary_account(home: &std::path::Path) -> Account {
    match source_epic::legendary::signed_in(home) {
        source_epic::legendary::SignIn::As(name) => Account::SignedIn(name),
        source_epic::legendary::SignIn::No => Account::SignedOut(LEGENDARY_SIGN_IN.into()),
    }
}

/// What signing gogdl in takes, said where it is not: GOG's sign-in page in a browser, and the code from the address
/// it ends on given to gogdl's own `auth --code`. The Dashboard shows no sign-in page of its own yet.
const GOGDL_SIGN_IN: &str = "that takes GOG's sign-in page, which the Dashboard doesn't show yet. Sign in at \
                             login.gog.com in a browser, then give gogdl the code from the address it ends on \
                             (\"gogdl auth --code\"). Until then GOG Galaxy installs.";

fn gogdl_account(home: &std::path::Path) -> Account {
    match source_gog::gogdl::signed_in(&home.join("auth.json")) {
        // gogdl keeps a user id, not a name.
        true => Account::SignedIn(String::new()),
        false => Account::SignedOut(GOGDL_SIGN_IN.into()),
    }
}

/// Fetches the helper, checks it, and runs it once (`--version`), saying each line it says: added if it ran and
/// ended well. One that did not is removed again.
fn add(data: &std::path::Path, helper: &helpers::Helper) -> HelperState {
    let key = &helper.key;
    eprintln!("helpers: {key}: fetching {} ({} licence, from {})", helper.version, helper.licence, helper.source);
    let program = match helpers::fetch(&helpers::agent(), data, helper) {
        Ok(program) => program,
        Err(e) => {
            eprintln!("helpers: {key}: refused: {e}");
            return HelperState::Failed(e.for_the_user());
        }
    };
    eprintln!("helpers: {key}: checked and kept at {}", program.display());
    let ran = run_once(key, &program);
    if ran {
        return HelperState::Added { version: helper.version.clone() };
    }
    if let Err(e) = helpers::remove(data, helper) {
        eprintln!("helpers: {key}: could not be removed again: {e}");
    }
    HelperState::Failed("It was downloaded but didn't run, so it wasn't kept.".into())
}

/// Runs the program with `--version` through the shell's runner, saying every line; whether it ended with 0.
fn run_once(key: &str, program: &std::path::Path) -> bool {
    let job = match shell::helper::Job::start(program, &["--version".into()], || {}) {
        Ok(job) => job,
        Err(e) => {
            eprintln!("helpers: {key}: did not start: {e}");
            return false;
        }
    };
    eprintln!("helpers: {key}: running --version (pid {})", job.pid());
    let until = Instant::now() + FIRST_RUN;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match job.next(left) {
            Some(shell::helper::Said::Out(line) | shell::helper::Said::Err(line)) => {
                eprintln!("helpers: {key}: says {line}")
            }
            Some(shell::helper::Said::Ended(code)) => {
                eprintln!("helpers: {key}: ended with {}", code.map_or("no code".into(), |c| c.to_string()));
                return code == Some(0);
            }
            None => {
                eprintln!("helpers: {key}: said nothing for {} s: stopped", FIRST_RUN.as_secs());
                job.stop();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_checksum_is_refused_and_nothing_is_kept_then_a_kept_helper_is_added_and_removed() {
        let service = helpers::fake::FakeHelpers::start(0, b"not a program".to_vec()).unwrap();
        let data = tempfile::tempdir().unwrap();
        let mut helpers = Helpers::new(service.table(), Some(data.path().to_path_buf()));
        assert!(helpers.shown().iter().all(|h| h.state == HelperState::NotAdded));

        let (tell, heard) = mpsc::channel();
        helpers.ask(HelperAsk::Add("gogdl".into()), move || tell.send(()).unwrap());
        assert_eq!(helpers.shown()[1].state, HelperState::Fetching);
        heard.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(helpers.take());
        assert!(matches!(&helpers.shown()[1].state, HelperState::Failed(why) if why.contains("wasn't the file")));
        assert!(!data.path().join("helpers").join("gogdl").join("gogdl.exe").exists());

        // Kept by hand at the table's version: added; removed, not.
        let legendary = &service.table()[0];
        std::fs::create_dir_all(helpers::folder(data.path(), legendary)).unwrap();
        std::fs::write(helpers::program(data.path(), legendary), b"x").unwrap();
        let kept = r#"{"version":"0.21.1","sha256":"00"}"#;
        std::fs::write(helpers::folder(data.path(), legendary).join("helper.json"), kept).unwrap();
        let mut helpers = Helpers::new(service.table(), Some(data.path().to_path_buf()));
        assert_eq!(helpers.shown()[0].state, HelperState::Added { version: "0.21.1".into() });
        helpers.ask(HelperAsk::Remove("legendary".into()), || {});
        assert_eq!(helpers.shown()[0].state, HelperState::NotAdded);
        assert!(!helpers::folder(data.path(), legendary).exists());
    }
}
