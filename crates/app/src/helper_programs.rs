//! Settings > Helpers, carried out (docs/spec/helpers.md): each helper's state on this PC, and on the user's Yes its
//! fetch, check and first run on a thread of its own, or its removal. The table is the `helpers` crate's, or a
//! test's (`--helper-table`); the helpers are kept under the data folder. Every step says one `helpers:` line.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use model::helper::{HelperAsk, HelperState};

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
    told: (Sender<(String, HelperState)>, Receiver<(String, HelperState)>),
}

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
        let helpers = Self { table, data, states, told: mpsc::channel() };
        eprintln!("helpers: {}", helpers.line());
        helpers
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
        self.table.iter().map(|helper| helpers::shown(helper, self.states[&helper.key].clone())).collect()
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
