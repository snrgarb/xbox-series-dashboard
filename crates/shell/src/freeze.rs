//! Holding a running game still while the Guide is open over it, as the Bundle does (its GameFreezer.cs;
//! docs/spec/guide.md, 7.2): the game's processes are suspended, so a game that reads the pad in the background
//! can't take the Guide's input, and resumed as the Guide closes.
//!
//! The processes are those of programs under the game's install folder and the one that owns its window, never the
//! Dashboard's own nor one of Windows'. One Windows won't let us open or suspend (a game running elevated, or one
//! an anti-cheat guards) is left running and said in the log: that is the Bundle's limit, accepted. A game
//! installed at a drive's root isn't frozen, since everything on the drive would be.
//!
//! A game left frozen would stay so after the Dashboard is gone. So what is frozen is written to a record first,
//! `frozen-game.txt` in the data folder: a line a process, its id, when it started (which tells it from a later
//! process given the same id) and its name. The record is deleted once all are resumed, and whatever it still lists
//! when the Dashboard next starts is resumed then.
//!
//! What is Windows' is behind `Processes`, so the rest is tested anywhere.
use std::path::{Path, PathBuf};

/// The record's file, in the data folder.
pub const RECORD: &str = "frozen-game.txt";

/// A running process: its id, its program's name, and the program's path where Windows gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Process {
    pub id: u32,
    pub name: String,
    pub path: Option<String>,
}

/// What came of suspending or resuming a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum Outcome {
    Done,
    /// It has ended, or its id is another process's now, which is not touched.
    Gone,
    /// Windows refused, with this NTSTATUS.
    Refused(i32),
}

/// Windows' processes, as much as freezing needs of them.
pub(crate) trait Processes {
    /// This process's id.
    fn own_id(&self) -> u32;
    /// Windows' own folder.
    fn windows_dir(&self) -> String;
    /// A folder's path past junctions, links and `subst` drives; None when it can't be had.
    fn final_path(&self, dir: &str) -> Option<String>;
    /// Every running process.
    fn list(&self) -> Vec<Process>;
    /// When a process started, having opened it to suspend it; the Windows error when it can't be opened.
    fn start_time(&self, id: u32) -> Result<u64, u32>;
    /// Suspends the process with this id that started then.
    fn suspend(&self, id: u32, started: u64) -> Outcome;
    /// Resumes it.
    fn resume(&self, id: u32, started: u64) -> Outcome;
}

/// A process frozen, as the record has it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Frozen {
    id: u32,
    started: u64,
    name: String,
}

impl Frozen {
    fn line(&self) -> String {
        format!("{} {} {}", self.id, self.started, self.name)
    }

    /// A line of the record; None when it isn't one.
    fn parse(line: &str) -> Option<Self> {
        let mut parts = line.trim().splitn(3, ' ');
        let (id, started) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
        Some(Self { id, started, name: parts.next().unwrap_or("?").to_string() })
    }
}

impl std::fmt::Display for Frozen {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

fn names(processes: &[Frozen]) -> String {
    processes.iter().map(Frozen::to_string).collect::<Vec<_>>().join(", ")
}

fn log(message: &str) {
    eprintln!("freeze: {message} at {:.0} ms", crate::since_start());
}

/// A path as paths are compared here: backslashes, lower case, without the `\\?\` Windows puts before a long one.
fn normal(path: &str) -> String {
    let path = path.replace('/', "\\").to_lowercase();
    match path.strip_prefix("\\\\?\\") {
        Some(rest) => match rest.strip_prefix("unc\\") {
            Some(share) => format!("\\\\{share}"),
            None => rest.to_string(),
        },
        None => path,
    }
}

/// A folder as what the paths of the programs in it start with.
fn prefix(dir: &str) -> String {
    format!("{}\\", normal(dir).trim_end_matches('\\'))
}

/// The prefixes a program in the folder may have: the folder as it is given and as it is past junctions and `subst`
/// drives (`real`), since a program's path may be either. An Xbox game is registered under `WindowsApps`, a link,
/// while Windows says its program runs from `<drive>\XboxGames\<game>\Content` (seen 2026-11-16). Never a root
/// that a junction comes to. Lower case, backslashes, a trailing one.
pub(crate) fn prefixes(dir: &str, real: Option<String>) -> Vec<String> {
    let mut prefixes = vec![prefix(dir)];
    if let Some(real) = real.filter(|real| !is_root(real)).map(|real| prefix(&real)) {
        if !prefixes.contains(&real) {
            prefixes.push(real);
        }
    }
    prefixes
}

/// The same for a folder on this machine.
#[cfg(windows)]
pub(crate) fn prefixes_here(dir: &str) -> Vec<String> {
    prefixes(dir, std::fs::canonicalize(dir).ok().map(|real| real.to_string_lossy().into_owned()))
}

/// Whether a folder is a drive's root or a share's, under which every program on it would be.
/// The game's processes still running on this machine: those whose program is under one of `prefixes`, without the
/// Dashboard's own, as "name (id)". Asked when the game has no window left, to tell a game that has gone from one
/// between windows or handing off to another of its programs.
#[cfg(windows)]
pub(crate) fn running_under(prefixes: &[String]) -> Vec<String> {
    under(&*system(), prefixes)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn under(system: &dyn Processes, prefixes: &[String]) -> Vec<String> {
    let own = system.own_id();
    system
        .list()
        .into_iter()
        .filter(|process| process.id != own && process.id != 0)
        .filter(|process| {
            let path = process.path.as_deref().map(normal);
            path.is_some_and(|path| prefixes.iter().any(|prefix| path.starts_with(prefix)))
        })
        .map(|process| format!("{} ({})", process.name, process.id))
        .collect()
}

fn is_root(dir: &str) -> bool {
    let dir = normal(dir);
    let dir = dir.trim_end_matches('\\');
    match dir.strip_prefix("\\\\") {
        // \\server\share
        Some(share) => share.matches('\\').count() <= 1,
        None => dir.is_empty() || (dir.len() == 2 && dir.ends_with(':')),
    }
}

/// Holds a game still, and lets it go.
pub struct Freezer {
    system: Box<dyn Processes>,
    /// The record. None where there is no data folder: nothing is then frozen, since nothing could say so.
    file: Option<PathBuf>,
    /// The processes frozen. None when no freeze is in effect; an empty list is one that froze none.
    frozen: Option<Vec<Frozen>>,
}

impl Freezer {
    /// A freezer that keeps its record in `data_dir`. Anywhere but Windows it finds no process to freeze.
    pub fn new(data_dir: Option<&Path>) -> Self {
        Self::with(system(), data_dir)
    }

    pub(crate) fn with(system: Box<dyn Processes>, data_dir: Option<&Path>) -> Self {
        Self { system, file: data_dir.map(|dir| dir.join(RECORD)), frozen: None }
    }

    /// Whether a freeze is in effect: `freeze` was called and `thaw` not since.
    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }

    /// Suspends the processes of programs under `dir`, the game's install folder, and the process `window_owner`,
    /// the one that owns the game's window, until `thaw`. Does nothing while a freeze is in effect. A process
    /// Windows won't let us suspend, or one that ends meanwhile, is left out and said.
    pub fn freeze(&mut self, dir: &str, window_owner: Option<u32>) {
        if self.frozen.is_some() {
            return;
        }
        self.frozen = Some(Vec::new());
        if is_root(dir) {
            log(&format!(
                "not freezing the game: its folder, {dir}, is a drive's root, and everything on it would freeze"
            ));
            return;
        }
        // Each one's start time is read first, so the record tells it from a later process given the same id.
        let mut found = Vec::new();
        for process in self.candidates(dir, window_owner) {
            match self.system.start_time(process.id) {
                Ok(started) => found.push(Frozen { id: process.id, started, name: process.name }),
                Err(error) => log(&format!(
                    "Windows won't let {} ({}) be frozen (error {error}): a game that runs elevated, or one an \
                     anti-cheat guards, keeps running while the Guide is open (docs/spec/guide.md, 7.2)",
                    process.name, process.id
                )),
            }
        }
        if found.is_empty() {
            log(&format!("found no process of the game to freeze, under {dir} or owning its window"));
            return;
        }
        // Written first, so a crash between here and the thaw still leaves a record of what to resume.
        if !self.save(&found) {
            log("not freezing the game: without its record, a crash would leave it frozen");
            return;
        }
        let mut frozen = Vec::new();
        for process in found.iter() {
            match self.system.suspend(process.id, process.started) {
                Outcome::Done => frozen.push(process.clone()),
                Outcome::Gone => log(&format!("{process} ended before it could be frozen")),
                Outcome::Refused(status) => log(&format!(
                    "Windows refused to freeze {process} (NTSTATUS {status:#010x}): it keeps running while the Guide \
                     is open"
                )),
            }
        }
        if frozen.len() != found.len() {
            self.save(&frozen);
        }
        if frozen.is_empty() {
            log("froze none of the game's processes");
            self.delete();
        } else {
            log(&format!("froze {}", names(&frozen)));
        }
        self.frozen = Some(frozen);
    }

    /// Resumes every process `freeze` suspended and deletes the record. Does nothing when no freeze is in effect.
    pub fn thaw(&mut self) {
        if let Some(frozen) = self.frozen.take() {
            if !frozen.is_empty() {
                self.resume(&frozen);
                self.delete();
            }
        }
    }

    /// Resumes whatever the record of a Dashboard that stopped with a game frozen still lists, and deletes it.
    /// Called once, as the Dashboard starts.
    pub fn thaw_leftovers(&mut self) {
        let Some(file) = &self.file else { return };
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                log(&format!("could not read {}, the record of a game left frozen: {e}", file.display()));
                return;
            }
        };
        let leftovers: Vec<Frozen> = text.lines().filter_map(Frozen::parse).collect();
        log(&format!("the Dashboard stopped last time with a game frozen: thawing {}", names(&leftovers)));
        self.resume(&leftovers);
        self.delete();
    }

    /// The processes to freeze: those of programs under the folder and the window's owner, without this process,
    /// Windows' own and any whose program can't be told.
    fn candidates(&self, dir: &str, window_owner: Option<u32>) -> Vec<Process> {
        let prefixes = prefixes(dir, self.system.final_path(dir));
        let windows = prefix(&self.system.windows_dir());
        let own = self.system.own_id();
        let mut found = Vec::new();
        for process in self.system.list() {
            if process.id == own || process.id == 0 {
                continue;
            }
            let path = process.path.as_deref().map(normal);
            let under = path.as_ref().is_some_and(|path| prefixes.iter().any(|prefix| path.starts_with(prefix)));
            if !under && Some(process.id) != window_owner {
                continue;
            }
            // Explorer, or a host that owns a store game's window: freezing one would freeze Windows.
            if path.is_none_or(|path| path.starts_with(&windows)) {
                log(&format!(
                    "not freezing {} ({}), a process of Windows' or one that can't be read",
                    process.name, process.id
                ));
                continue;
            }
            found.push(process);
        }
        found
    }

    fn resume(&self, processes: &[Frozen]) {
        let mut thawed = Vec::new();
        for process in processes {
            match self.system.resume(process.id, process.started) {
                Outcome::Done => thawed.push(process.clone()),
                Outcome::Gone => log(&format!("{process} has ended, or can't be opened, so isn't thawed")),
                Outcome::Refused(status) => {
                    log(&format!("Windows refused to thaw {process} (NTSTATUS {status:#010x})"))
                }
            }
        }
        if !thawed.is_empty() {
            log(&format!("thawed {}", names(&thawed)));
        }
    }

    fn save(&self, processes: &[Frozen]) -> bool {
        let Some(file) = &self.file else {
            log("there is no data folder to keep the record of a frozen game in");
            return false;
        };
        let lines: String = processes.iter().map(|process| process.line() + "\n").collect();
        let written = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(file, lines));
        if let Err(e) = &written {
            log(&format!("could not write {}, the record of the game's frozen processes: {e}", file.display()));
        }
        written.is_ok()
    }

    fn delete(&self) {
        let Some(file) = &self.file else { return };
        match std::fs::remove_file(file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log(&format!(
                "could not delete {}: the next start will try to thaw what it lists again: {e}",
                file.display()
            )),
        }
    }
}

/// The Dashboard leaving, however it leaves in an orderly way, lets the game go.
impl Drop for Freezer {
    fn drop(&mut self) {
        self.thaw();
    }
}

#[cfg(not(windows))]
fn system() -> Box<dyn Processes> {
    /// Anywhere but Windows: no processes, so nothing to freeze.
    struct Nowhere;

    impl Processes for Nowhere {
        fn own_id(&self) -> u32 {
            std::process::id()
        }

        fn windows_dir(&self) -> String {
            String::new()
        }

        fn final_path(&self, _: &str) -> Option<String> {
            None
        }

        fn list(&self) -> Vec<Process> {
            Vec::new()
        }

        fn start_time(&self, _: u32) -> Result<u64, u32> {
            Err(0)
        }

        fn suspend(&self, _: u32, _: u64) -> Outcome {
            Outcome::Gone
        }

        fn resume(&self, _: u32, _: u64) -> Outcome {
            Outcome::Gone
        }
    }
    Box::new(Nowhere)
}

#[cfg(windows)]
fn system() -> Box<dyn Processes> {
    Box::new(win::Windows)
}

#[cfg(windows)]
mod win {
    use super::{Outcome, Process, Processes};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SUSPEND_RESUME,
    };

    // Every thread of a process suspended, or resumed, at once: one more, or one less, on each one's count. Not in
    // the Windows SDK's headers, but what Process Explorer's Suspend is, and what the Bundle calls.
    #[link(name = "ntdll")]
    extern "system" {
        fn NtSuspendProcess(process: HANDLE) -> i32;
        fn NtResumeProcess(process: HANDLE) -> i32;
    }

    pub struct Windows;

    /// A process's program's path.
    unsafe fn path_of(id: u32) -> Option<String> {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
        if process.is_null() {
            return None;
        }
        let mut path = [0u16; 1024];
        let mut len = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
        CloseHandle(process);
        (ok != 0).then(|| String::from_utf16_lossy(&path[..len as usize]))
    }

    /// When the process behind a handle started, as a FILETIME's count; 0 when Windows won't say.
    unsafe fn started(process: HANDLE) -> u64 {
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        if GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
            return 0;
        }
        (created.dwHighDateTime as u64) << 32 | created.dwLowDateTime as u64
    }

    /// Opens the process to suspend or resume it, and does `action` if it is still the one that started then.
    unsafe fn apply(id: u32, since: u64, action: unsafe extern "system" fn(HANDLE) -> i32) -> Outcome {
        let process = OpenProcess(PROCESS_SUSPEND_RESUME | PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
        if process.is_null() {
            return Outcome::Gone;
        }
        let outcome = if started(process) != since {
            Outcome::Gone
        } else {
            match action(process) {
                0 => Outcome::Done,
                status => Outcome::Refused(status),
            }
        };
        CloseHandle(process);
        outcome
    }

    impl Processes for Windows {
        fn own_id(&self) -> u32 {
            std::process::id()
        }

        fn windows_dir(&self) -> String {
            std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())
        }

        fn final_path(&self, dir: &str) -> Option<String> {
            Some(std::fs::canonicalize(dir).ok()?.to_string_lossy().into_owned())
        }

        fn list(&self) -> Vec<Process> {
            let mut found = Vec::new();
            unsafe {
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
                if snapshot == INVALID_HANDLE_VALUE {
                    super::log(&format!("could not list the running processes (error {})", GetLastError()));
                    return found;
                }
                let mut entry: PROCESSENTRY32W = std::mem::zeroed();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                let mut more = Process32FirstW(snapshot, &mut entry);
                while more != 0 {
                    let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                    let name = name.strip_suffix(".exe").unwrap_or(&name).to_string();
                    found.push(Process { id: entry.th32ProcessID, name, path: path_of(entry.th32ProcessID) });
                    more = Process32NextW(snapshot, &mut entry);
                }
                CloseHandle(snapshot);
            }
            found
        }

        fn start_time(&self, id: u32) -> Result<u64, u32> {
            unsafe {
                let process = OpenProcess(PROCESS_SUSPEND_RESUME | PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
                if process.is_null() {
                    return Err(GetLastError());
                }
                let since = started(process);
                CloseHandle(process);
                Ok(since)
            }
        }

        fn suspend(&self, id: u32, started: u64) -> Outcome {
            unsafe { apply(id, started, NtSuspendProcess) }
        }

        fn resume(&self, id: u32, started: u64) -> Outcome {
            unsafe { apply(id, started, NtResumeProcess) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    const GAME_DIR: &str = "C:\\Steam\\steamapps\\common\\Brass Meridian";

    /// A machine's processes: each with when it started and how many times it is suspended.
    #[derive(Default)]
    struct Machine {
        processes: Vec<Process>,
        started: BTreeMap<u32, u64>,
        suspended: BTreeMap<u32, u32>,
        /// Processes Windows won't open to suspend: elevated ones.
        guarded: Vec<u32>,
        /// Processes Windows opens and then won't suspend.
        refusing: Vec<u32>,
        /// Whether the record was there each time a process was suspended.
        recorded: Vec<bool>,
        record: PathBuf,
        final_path: Option<String>,
    }

    #[derive(Clone, Default)]
    struct Fake(Rc<RefCell<Machine>>);

    impl Fake {
        fn run(&self, id: u32, path: &str) {
            let name = path.rsplit('\\').next().unwrap().trim_end_matches(".exe").to_string();
            let mut machine = self.0.borrow_mut();
            machine.processes.push(Process { id, name, path: Some(path.to_string()) });
            machine.started.insert(id, 1000 + id as u64);
        }

        fn suspended(&self) -> Vec<u32> {
            self.0.borrow().suspended.iter().filter(|(_, &count)| count > 0).map(|(&id, _)| id).collect()
        }

        fn count(&self, id: u32, by: i32) -> Outcome {
            let mut machine = self.0.borrow_mut();
            let recorded = machine.record.exists();
            machine.recorded.push(recorded);
            let count = machine.suspended.entry(id).or_default();
            *count = count.saturating_add_signed(by);
            Outcome::Done
        }
    }

    impl Processes for Fake {
        fn own_id(&self) -> u32 {
            1
        }

        fn windows_dir(&self) -> String {
            "C:\\WINDOWS".into()
        }

        fn final_path(&self, _: &str) -> Option<String> {
            self.0.borrow().final_path.clone()
        }

        fn list(&self) -> Vec<Process> {
            self.0.borrow().processes.clone()
        }

        fn start_time(&self, id: u32) -> Result<u64, u32> {
            let machine = self.0.borrow();
            if machine.guarded.contains(&id) {
                return Err(5);
            }
            machine.started.get(&id).copied().ok_or(87)
        }

        fn suspend(&self, id: u32, started: u64) -> Outcome {
            if self.0.borrow().started.get(&id) != Some(&started) {
                return Outcome::Gone;
            }
            if self.0.borrow().refusing.contains(&id) {
                // STATUS_ACCESS_DENIED.
                return Outcome::Refused(0xC000_0022_u32 as i32);
            }
            self.count(id, 1)
        }

        fn resume(&self, id: u32, started: u64) -> Outcome {
            if self.0.borrow().started.get(&id) != Some(&started) {
                return Outcome::Gone;
            }
            self.count(id, -1)
        }
    }

    /// A machine running the Dashboard (1), Explorer (2), the game (10), its helper in a folder of its own (11),
    /// another game (20) and a process that can't be read (30).
    fn machine(data: &Path) -> (Fake, Freezer) {
        let fake = Fake::default();
        fake.0.borrow_mut().record = data.join(RECORD);
        fake.run(1, "C:\\Program Files\\Dashboard\\dashboard.exe");
        fake.run(2, "C:\\Windows\\explorer.exe");
        fake.run(10, "c:\\steam\\steamapps\\common\\Brass Meridian\\game.exe");
        fake.run(11, "C:\\Steam\\steamapps\\common\\Brass Meridian\\bin\\helper.exe");
        fake.run(20, "C:\\Steam\\steamapps\\common\\Brass Meridian 2\\game.exe");
        fake.0.borrow_mut().processes.push(Process { id: 30, name: "secret".into(), path: None });
        let freezer = Freezer::with(Box::new(fake.clone()), Some(data));
        (fake, freezer)
    }

    #[test]
    fn the_games_processes_still_running_are_those_under_its_folder() {
        let data = tempfile::tempdir().unwrap();
        let (fake, _) = machine(data.path());
        let prefixes = prefixes(GAME_DIR, None);
        assert_eq!(under(&fake, &prefixes), ["game (10)", "helper (11)"]);
        fake.0.borrow_mut().processes.retain(|process| process.id != 10 && process.id != 11);
        assert!(under(&fake, &prefixes).is_empty(), "the game has gone");
    }

    #[test]
    fn the_games_processes_freeze_and_thaw_and_the_record_is_there_between() {
        let data = tempfile::tempdir().unwrap();
        let (fake, mut freezer) = machine(data.path());
        let record = data.path().join(RECORD);
        assert!(!freezer.is_frozen());

        freezer.freeze(GAME_DIR, Some(10));
        assert!(freezer.is_frozen());
        assert_eq!(fake.suspended(), [10, 11], "the game and its helper, and no other game");
        assert_eq!(std::fs::read_to_string(&record).unwrap(), "10 1010 game\n11 1011 helper\n");
        assert_eq!(fake.0.borrow().recorded, [true, true], "the record is written before anything is suspended");

        // Once per freeze: a second call suspends nothing again.
        freezer.freeze(GAME_DIR, Some(10));
        assert_eq!(fake.0.borrow().suspended[&10], 1);

        freezer.thaw();
        assert!(!freezer.is_frozen());
        assert_eq!(fake.suspended(), [0u32; 0]);
        assert!(!record.exists(), "the record goes once all are resumed");
        freezer.thaw();
        assert_eq!(fake.0.borrow().recorded.len(), 4, "a second thaw resumes nothing");
    }

    #[test]
    fn the_windows_owner_freezes_too_unless_it_is_windows_or_the_dashboard() {
        let data = tempfile::tempdir().unwrap();
        // A game whose window belongs to a program outside its folder, as a launcher's does.
        let (fake, mut freezer) = machine(data.path());
        fake.run(40, "D:\\Launcher\\launcher.exe");
        freezer.freeze(GAME_DIR, Some(40));
        assert_eq!(fake.suspended(), [10, 11, 40]);
        freezer.thaw();

        // A store game's window, owned by a host of Windows': never frozen. Nor a process that can't be read.
        for owner in [2, 30, 1] {
            freezer.freeze(GAME_DIR, Some(owner));
            assert_eq!(fake.suspended(), [10, 11], "window owned by {owner}");
            freezer.thaw();
        }
    }

    #[test]
    fn a_folder_reached_through_a_junction_is_the_games_too() {
        let data = tempfile::tempdir().unwrap();
        let (fake, mut freezer) = machine(data.path());
        fake.0.borrow_mut().final_path = Some("\\\\?\\E:\\Games\\Brass Meridian".into());
        fake.run(50, "E:\\Games\\Brass Meridian\\game.exe");
        freezer.freeze(GAME_DIR, None);
        assert_eq!(fake.suspended(), [10, 11, 50]);
        freezer.thaw();

        // A junction that comes to a drive's root is followed nowhere.
        fake.0.borrow_mut().final_path = Some("\\\\?\\E:\\".into());
        freezer.freeze(GAME_DIR, None);
        assert_eq!(fake.suspended(), [10, 11]);
    }

    #[test]
    fn a_game_at_a_drives_root_is_not_frozen() {
        let data = tempfile::tempdir().unwrap();
        let (fake, mut freezer) = machine(data.path());
        for root in ["C:\\", "c:", "C:/", "\\\\server\\share\\", ""] {
            freezer.freeze(root, Some(10));
            assert!(freezer.is_frozen(), "a freeze that froze none is still one");
            assert_eq!(fake.suspended(), [0u32; 0], "{root}");
            freezer.thaw();
        }
        assert!(!is_root("C:\\Games"));
        assert!(!is_root("\\\\server\\share\\Games"));
    }

    #[test]
    fn a_process_windows_guards_is_left_running_and_out_of_the_record() {
        let data = tempfile::tempdir().unwrap();
        let (fake, mut freezer) = machine(data.path());
        fake.0.borrow_mut().guarded = vec![10];
        freezer.freeze(GAME_DIR, Some(10));
        assert_eq!(fake.suspended(), [11]);
        assert_eq!(std::fs::read_to_string(data.path().join(RECORD)).unwrap(), "11 1011 helper\n");
        freezer.thaw();

        // An elevated game with nothing else: nothing frozen, and no record left.
        fake.0.borrow_mut().guarded = vec![10, 11];
        freezer.freeze(GAME_DIR, Some(10));
        assert_eq!(fake.suspended(), [0u32; 0]);
        assert!(freezer.is_frozen());
        assert!(!data.path().join(RECORD).exists());
        freezer.thaw();

        // One Windows opens and then refuses to suspend, and one that ends between being found and being frozen:
        // the record is left with what was frozen.
        fake.0.borrow_mut().guarded = vec![];
        fake.0.borrow_mut().refusing = vec![10];
        freezer.freeze(GAME_DIR, Some(10));
        assert_eq!(fake.suspended(), [11]);
        assert_eq!(std::fs::read_to_string(data.path().join(RECORD)).unwrap(), "11 1011 helper\n");
        freezer.thaw();
        assert_eq!(fake.suspended(), [0u32; 0]);
    }

    #[test]
    fn what_a_dashboard_that_stopped_left_frozen_thaws_at_the_next_start() {
        let data = tempfile::tempdir().unwrap();
        let (fake, mut freezer) = machine(data.path());
        freezer.freeze(GAME_DIR, Some(10));
        // Killed: no thaw, no drop.
        std::mem::forget(freezer);
        assert_eq!(fake.suspended(), [10, 11]);

        // Meanwhile the helper ended and its id went to another program.
        fake.0.borrow_mut().started.insert(11, 9999);
        let mut next = Freezer::with(Box::new(fake.clone()), Some(data.path()));
        next.thaw_leftovers();
        assert_eq!(fake.suspended(), [11], "the game thaws; the process that has the helper's id is not touched");
        assert!(!data.path().join(RECORD).exists());
        assert!(!next.is_frozen());

        // Nothing left: nothing to do.
        let resumed = fake.0.borrow().recorded.len();
        next.thaw_leftovers();
        assert_eq!(fake.0.borrow().recorded.len(), resumed);
    }

    #[test]
    fn leaving_in_order_thaws_the_game() {
        let data = tempfile::tempdir().unwrap();
        let (fake, mut freezer) = machine(data.path());
        freezer.freeze(GAME_DIR, Some(10));
        drop(freezer);
        assert_eq!(fake.suspended(), [0u32; 0]);
        assert!(!data.path().join(RECORD).exists());
    }

    #[test]
    fn without_a_record_nothing_is_frozen() {
        let fake = Fake::default();
        fake.run(10, "C:\\Steam\\steamapps\\common\\Brass Meridian\\game.exe");
        let mut freezer = Freezer::with(Box::new(fake.clone()), None);
        freezer.freeze(GAME_DIR, Some(10));
        assert_eq!(fake.suspended(), [0u32; 0]);

        // Nor where the record can't be written: its folder is a file.
        let data = tempfile::tempdir().unwrap();
        let file = data.path().join("a-file");
        std::fs::write(&file, b"").unwrap();
        let mut freezer = Freezer::with(Box::new(fake.clone()), Some(&file));
        freezer.freeze(GAME_DIR, Some(10));
        assert_eq!(fake.suspended(), [0u32; 0]);
    }

    #[test]
    fn the_record_is_read_as_the_bundle_writes_it() {
        let frozen = Frozen::parse("4242 133760000000000000 Brass Meridian").unwrap();
        assert_eq!(frozen, Frozen { id: 4242, started: 133_760_000_000_000_000, name: "Brass Meridian".into() });
        assert_eq!(Frozen::parse(&frozen.line()), Some(frozen));
        assert_eq!(Frozen::parse("7 9").unwrap().name, "?");
        assert_eq!(Frozen::parse("game 9 7"), None);
        assert_eq!(Frozen::parse(""), None);
    }
}
