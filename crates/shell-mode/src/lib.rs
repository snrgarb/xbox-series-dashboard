//! Shell mode (docs/spec/shell-mode.md; docs/research/shell.md): the Dashboard in place of the Windows desktop at
//! sign-in, as the Bundle's Boot shell put Playnite there.
//!
//! One exe, run several ways. `dashboard.exe` is the Dashboard. The same file run with
//!
//! - `--shell` is what Winlogon's per-user `Shell` value names: the Boot shell's job (`boot`), a small process that
//!   never touches the GPU, starts the Dashboard behind a black window, starts explorer behind Start-up, keeps the
//!   Dashboard up and reads the pad while none runs;
//! - `--sentinel` is started by explorer's startup run, and writes `Shell` back if Windows dropped it;
//! - `--shell-on` and `--shell-off` turn it on and off from a terminal, as Settings > System does; with
//!   `--dry-shell` they say what they would write and write nothing;
//! - `--shell-probe` answers that this exe starts, before `Shell` may name it.
//!
//! Everything shell mode writes goes through [`system::System`]: the registry, the task, the record.
pub mod contract;
pub mod dashboard;
pub mod log;
pub mod setup;
pub mod system;
// Only Windows has Task Scheduler to read and write; its tests run everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
mod task_xml;
pub mod watch;

#[cfg(windows)]
mod boot;
#[cfg(windows)]
mod processes;

use std::path::{Path, PathBuf};

use setup::{Outcome, Paths};
use system::{Dry, System, ThisPc};

/// This PC, for this user: what writes, or with `dry` only reads. A task's XML is written in `paths`' shell folder.
pub fn this_pc(paths: &Paths, dry: bool) -> Box<dyn System> {
    let var = |name| std::env::var(name).unwrap_or_default();
    let user = match (var("USERDOMAIN"), var("USERNAME")) {
        (domain, name) if domain.is_empty() => name,
        (domain, name) => format!(r"{domain}\{name}"),
    };
    let pc = ThisPc { user, scratch: paths.folder() };
    if dry {
        Box::new(Dry(pc))
    } else {
        Box::new(pc)
    }
}

/// This PC's paths: the installed exe under %LOCALAPPDATA%, this one, and the data folder. `as_installed` takes this
/// exe for the installed one: only ever with a dry run, for a test on a PC where nothing is installed.
pub fn paths(data: &Path, as_installed: bool) -> Paths {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    let mut paths = Paths::this_pc(&local, data);
    if as_installed {
        paths.installed = paths.running.clone();
    }
    paths
}

/// Runs the job the arguments ask for, if they ask for one of shell mode's, before anything else of the
/// Dashboard's is set up; the code to exit with. None: the Dashboard itself is to run. `data` is the data folder
/// when none is given (`library::data_dir`).
pub fn run_job(args: &[String], data: Option<PathBuf>) -> Option<i32> {
    let first = args.first()?.as_str();
    match first {
        contract::SHELL => Some(shell(data)),
        contract::SENTINEL => Some(sentinel(data)),
        contract::PROBE => {
            println!("dashboard: {} ok", contract::PROBE);
            Some(contract::PROBE_OK as i32)
        }
        contract::ON | contract::OFF => Some(on_or_off(first == contract::ON, &args[1..], data)),
        _ => None,
    }
}

/// Where shell mode stands for Settings > System: read only, slow enough (schtasks) to be asked off the frame's
/// thread. Unknown anywhere but Windows.
pub fn settings_status(data: &Path, dry: bool) -> model::shell_mode::ShellMode {
    if !cfg!(windows) {
        return model::shell_mode::ShellMode::Unknown;
    }
    let paths = paths(data, false);
    setup::status(&*this_pc(&paths, dry), &paths)
}

/// Settings > System's change, said in the Dashboard's log and, done for real, in `setup.log` as `--shell-on` keeps
/// it; where shell mode stands after. A dry run's Done stands as done, so the page can be tried.
pub fn settings_change(on: bool, data: &Path, dry: bool) -> model::shell_mode::ShellMode {
    use model::shell_mode::ShellMode;
    if !cfg!(windows) {
        return ShellMode::Unknown;
    }
    let paths = paths(data, false);
    let mut system = this_pc(&paths, dry);
    let mut log = log::Log::new("setup");
    if !dry {
        log.append(&paths.logs().join("setup.log"));
    }
    let mut say = |line: String| log.line(&line);
    let outcome = if on {
        setup::turn_on(&mut *system, &paths, &mut say)
    } else {
        setup::turn_off(&mut *system, &paths, &mut say)
    };
    match outcome {
        Outcome::Done if dry => {
            if on {
                ShellMode::On
            } else {
                ShellMode::Off
            }
        }
        Outcome::Done | Outcome::Already => setup::status(&*system, &paths),
        Outcome::Refused(why) => why.first().map_or(ShellMode::Failed, |refusal| ShellMode::Refused(*refusal)),
        Outcome::Failed => ShellMode::Failed,
    }
}

/// `--shell-on` and `--shell-off`, with `--dry-shell`, `--as-installed` (a dry run's only) and `--data-dir <dir>`.
fn on_or_off(on: bool, rest: &[String], data: Option<PathBuf>) -> i32 {
    let (mut dry, mut as_installed, mut data) = (false, false, data);
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            contract::DRY => dry = true,
            contract::AS_INSTALLED => as_installed = true,
            "--data-dir" => match it.next() {
                Some(dir) => data = Some(dir.into()),
                None => return usage("--data-dir needs a folder"),
            },
            other => return usage(&format!("unknown argument {other}")),
        }
    }
    if as_installed && !dry {
        return usage("--as-installed goes only with --dry-shell");
    }
    let Some(data) = data else { return usage("no data folder") };
    let paths = paths(&data, as_installed);
    let mut system = this_pc(&paths, dry);
    // A real change is kept in the data folder's log as well; a dry run writes nothing anywhere.
    let mut log = log::Log::new("setup");
    if !dry {
        log.append(&paths.logs().join("setup.log"));
    }
    let mut say = |line: String| log.line(&line);
    let outcome: Outcome = if on {
        setup::turn_on(&mut *system, &paths, &mut say)
    } else {
        setup::turn_off(&mut *system, &paths, &mut say)
    };
    outcome.code()
}

fn usage(why: &str) -> i32 {
    eprintln!(
        "dashboard: {why}\nusage: dashboard --shell-on | --shell-off [--dry-shell [--as-installed]] [--data-dir <dir>]"
    );
    2
}

/// `--shell`: the Boot shell's job, for the rest of the session.
fn shell(data: Option<PathBuf>) -> i32 {
    #[cfg(windows)]
    {
        boot::run(data)
    }
    #[cfg(not(windows))]
    {
        let _ = data;
        eprintln!("dashboard: --shell is Windows' only");
        2
    }
}

/// `--sentinel`: from explorer's startup run.
fn sentinel(data: Option<PathBuf>) -> i32 {
    let Some(data) = data else { return contract::NOT_A_DASHBOARD as i32 };
    let paths = paths(&data, false);
    let mut log = log::Log::new("sentinel");
    log.append(&paths.logs().join("sentinel.log"));
    let mut system = this_pc(&paths, false);
    let shell_runs = dashboard::mutex_held(contract::SHELL_MUTEX);
    let mut say = |line: String| log.line(&line);
    if setup::sentinel(&mut *system, &paths, shell_runs, &mut say) == setup::Sentinel::StartDashboard {
        if dashboard::mutex_held(&dashboard::running_mutex(None)) {
            say("sentinel: a Dashboard runs already".into());
        } else {
            match dashboard::start(&paths.installed, &[contract::FROM_SHELL]) {
                Ok(pid) => say(format!("sentinel: started the Dashboard, pid {pid}")),
                Err(why) => say(format!("sentinel: couldn't start the Dashboard: {why}")),
            }
        }
    }
    // Never mistaken for a Dashboard's exit by a --shell process watching.
    contract::NOT_A_DASHBOARD as i32
}
