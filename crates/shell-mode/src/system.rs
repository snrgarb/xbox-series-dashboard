//! The one way shell mode touches the PC: every registry value, scheduled task and file it reads or writes goes
//! through [`System`]. There are three:
//!
//! - [`ThisPc`]: Windows itself. It writes only the current user's hive (HKCU): a write to HKLM is refused here,
//!   whoever asks. Anywhere but Windows it reads nothing and writes nothing, and says so.
//! - [`Dry`]: reads through to another, and writes nothing (`--dry-shell`). Whoever uses it says `would` for each
//!   write it was asked (`setup`).
//! - [`Fake`]: a made-up PC in memory, for the unit tests: nothing in `cargo test` touches a real registry. A write
//!   can be made to fail, to test each way back.
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Which hive a value is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hive {
    /// HKEY_CURRENT_USER: the only one ever written.
    User,
    /// HKEY_LOCAL_MACHINE: only ever read.
    Machine,
}

/// A registry value: its hive, its key's path under the hive, and its name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Place {
    pub hive: Hive,
    pub key: String,
    pub name: String,
}

impl Place {
    pub fn user(key: &str, name: &str) -> Self {
        Self { hive: Hive::User, key: key.into(), name: name.into() }
    }

    pub fn machine(key: &str, name: &str) -> Self {
        Self { hive: Hive::Machine, key: key.into(), name: name.into() }
    }
}

/// `HKCU\Software\...\Winlogon\Shell`.
impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hive = match self.hive {
            Hive::User => "HKCU",
            Hive::Machine => "HKLM",
        };
        write!(f, r"{hive}\{}\{}", self.key, self.name)
    }
}

/// A registry value's data: text (REG_SZ), a number (REG_DWORD), or anything else as its type and bytes, which is
/// put back as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Number(u32),
    Raw { kind: u32, data: Vec<u8> },
}

impl Value {
    /// Whether two values are the same: text ignoring case, as Windows reads paths.
    pub fn same(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Text(a), Value::Text(b)) => a.eq_ignore_ascii_case(b),
            _ => self == other,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// Text in quotes as it is, quotes inside it and all; a number with its type.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(text) => write!(f, "\"{text}\""),
            Value::Number(n) => write!(f, "{n} (DWORD)"),
            Value::Raw { kind, data } => {
                write!(f, "type {kind}, ")?;
                data.iter().try_for_each(|byte| write!(f, "{byte:02X}"))
            }
        }
    }
}

/// A scheduled task in Task Scheduler's root folder, as shell mode writes one: a program run at this user's sign-in,
/// after a delay, at the user's own rights. What the record keeps of it, as Windows rewrites the rest of its XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub command: String,
    pub arguments: String,
    pub delay_seconds: u32,
}

impl Task {
    pub fn same(&self, other: &Task) -> bool {
        self.command.eq_ignore_ascii_case(&other.command)
            && self.arguments == other.arguments
            && self.delay_seconds == other.delay_seconds
    }
}

/// `at sign-in + 90 s: C:\Windows\System32\conhost.exe --headless ...`
impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at sign-in + {} s: {} {}", self.delay_seconds, self.command, self.arguments)
    }
}

/// What shell mode reads and writes on a PC. Every error is a sentence to say.
pub trait System {
    /// Writes nothing: each write is only said (`--dry-shell`).
    fn dry(&self) -> bool {
        false
    }

    /// A value, or none if it isn't there (nor its key).
    fn read(&self, place: &Place) -> Result<Option<Value>, String>;

    /// Sets a value of the user's hive, its key made if need be; none deletes it (nothing to delete is no error).
    fn write(&mut self, place: &Place, value: Option<&Value>) -> Result<(), String>;

    /// A task in the root folder, or none.
    fn task(&self, name: &str) -> Result<Option<Task>, String>;

    /// Registers a task for this user, in place of one of the same name; none deletes it.
    fn set_task(&mut self, name: &str, task: Option<&Task>) -> Result<(), String>;

    /// A text file's contents, or none if it isn't there.
    fn read_file(&self, path: &Path) -> Result<Option<String>, String>;

    /// Writes a text file whole (beside it first, then over it), its folder made if need be.
    fn write_file(&mut self, path: &Path, text: &str) -> Result<(), String>;

    /// Renames a file within its folder; one at `to` is replaced.
    fn rename_file(&mut self, from: &Path, to: &Path) -> Result<(), String>;

    /// Whether a file is there.
    fn exists(&self, path: &Path) -> bool;

    /// Runs `<exe> --shell-probe` and waits for it to say it starts: what proves an exe is fit to be named in
    /// `Shell`. Writes nothing.
    fn probe(&self, exe: &Path) -> Result<(), String>;
}

/// Reads through to `S`, writes nothing.
pub struct Dry<S>(pub S);

impl<S: System> System for Dry<S> {
    fn dry(&self) -> bool {
        true
    }

    fn read(&self, place: &Place) -> Result<Option<Value>, String> {
        self.0.read(place)
    }

    fn write(&mut self, _: &Place, _: Option<&Value>) -> Result<(), String> {
        Ok(())
    }

    fn task(&self, name: &str) -> Result<Option<Task>, String> {
        self.0.task(name)
    }

    fn set_task(&mut self, _: &str, _: Option<&Task>) -> Result<(), String> {
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Option<String>, String> {
        self.0.read_file(path)
    }

    fn write_file(&mut self, _: &Path, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn rename_file(&mut self, _: &Path, _: &Path) -> Result<(), String> {
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.0.exists(path)
    }

    fn probe(&self, exe: &Path) -> Result<(), String> {
        self.0.probe(exe)
    }
}

/// A made-up PC: its values, tasks and files in memory. `fail` names what a write fails on: a value or a task as
/// it is said (`HKCU\...\Shell`, `Task XboxSeriesDashboard.SafetyNet`), or a file's path. `fail_reads` the same, for
/// a read. `writes` says every write that was made, in order.
#[derive(Debug, Clone)]
pub struct Fake {
    pub values: BTreeMap<Place, Value>,
    pub tasks: BTreeMap<String, Task>,
    pub files: BTreeMap<PathBuf, String>,
    pub fail: Vec<String>,
    pub fail_reads: Vec<String>,
    /// What the probe answers.
    pub probe: Result<(), String>,
    pub writes: Vec<String>,
    /// A write of Shell lands as this in place of what was written, as a value that reads back otherwise.
    pub garble_shell: Option<String>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
            tasks: BTreeMap::new(),
            files: BTreeMap::new(),
            fail: Vec::new(),
            fail_reads: Vec::new(),
            probe: Ok(()),
            writes: Vec::new(),
            garble_shell: None,
        }
    }
}

impl Fake {
    fn fails(&self, what: &str) -> Result<(), String> {
        if self.fail.iter().any(|f| f.eq_ignore_ascii_case(what)) {
            return Err(format!("access is denied (a test's failure at {what})"));
        }
        Ok(())
    }

    fn read_fails(&self, what: &str) -> Result<(), String> {
        if self.fail_reads.iter().any(|f| f.eq_ignore_ascii_case(what)) {
            return Err(format!("access is denied (a test's failure reading {what})"));
        }
        Ok(())
    }
}

impl System for Fake {
    fn read(&self, place: &Place) -> Result<Option<Value>, String> {
        self.read_fails(&place.to_string())?;
        Ok(self.values.get(place).cloned())
    }

    fn write(&mut self, place: &Place, value: Option<&Value>) -> Result<(), String> {
        if place.hive != Hive::User {
            return Err(format!("{place}: HKLM is never written"));
        }
        self.fails(&place.to_string())?;
        match value {
            Some(value) => {
                let landed = match &self.garble_shell {
                    Some(garbled) if place.name == "Shell" => Value::Text(garbled.clone()),
                    _ => value.clone(),
                };
                self.writes.push(format!("set {place} = {value}"));
                self.values.insert(place.clone(), landed);
            }
            None => {
                self.writes.push(format!("delete {place}"));
                self.values.remove(place);
            }
        }
        Ok(())
    }

    fn task(&self, name: &str) -> Result<Option<Task>, String> {
        self.read_fails(&format!("Task {name}"))?;
        Ok(self.tasks.get(name).cloned())
    }

    fn set_task(&mut self, name: &str, task: Option<&Task>) -> Result<(), String> {
        self.fails(&format!("Task {name}"))?;
        match task {
            Some(task) => {
                self.writes.push(format!("register Task {name}"));
                self.tasks.insert(name.into(), task.clone());
            }
            None => {
                self.writes.push(format!("delete Task {name}"));
                self.tasks.remove(name);
            }
        }
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Option<String>, String> {
        self.read_fails(&path.display().to_string())?;
        Ok(self.files.get(path).cloned())
    }

    fn write_file(&mut self, path: &Path, text: &str) -> Result<(), String> {
        self.fails(&path.display().to_string())?;
        self.writes.push(format!("save {}", path.display()));
        self.files.insert(path.into(), text.into());
        Ok(())
    }

    fn rename_file(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        self.fails(&from.display().to_string())?;
        let text = self.files.remove(from).ok_or(format!("{} isn't there", from.display()))?;
        self.writes.push(format!("rename {} to {}", from.display(), to.display()));
        self.files.insert(to.into(), text);
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn probe(&self, _: &Path) -> Result<(), String> {
        self.probe.clone()
    }
}

/// Windows itself, for this user. `scratch` is where a task's XML is written for schtasks to read: the data
/// folder's `shell` folder, so nothing is written outside the data folder but the registry and the task.
pub struct ThisPc {
    pub user: String,
    pub scratch: PathBuf,
}

#[cfg(not(windows))]
impl System for ThisPc {
    fn read(&self, place: &Place) -> Result<Option<Value>, String> {
        Err(format!("{place}: only Windows has a registry"))
    }

    fn write(&mut self, place: &Place, _: Option<&Value>) -> Result<(), String> {
        Err(format!("{place}: only Windows has a registry"))
    }

    fn task(&self, name: &str) -> Result<Option<Task>, String> {
        Err(format!("Task {name}: only Windows has Task Scheduler"))
    }

    fn set_task(&mut self, name: &str, _: Option<&Task>) -> Result<(), String> {
        Err(format!("Task {name}: only Windows has Task Scheduler"))
    }

    fn read_file(&self, path: &Path) -> Result<Option<String>, String> {
        files::read(path)
    }

    fn write_file(&mut self, path: &Path, text: &str) -> Result<(), String> {
        files::write(path, text)
    }

    fn rename_file(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        std::fs::rename(from, to).map_err(|e| format!("{}: {e}", from.display()))
    }

    fn exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn probe(&self, exe: &Path) -> Result<(), String> {
        Err(format!("{}: shell mode is Windows' only", exe.display()))
    }
}

/// Files, as every [`System`] but the fake has them.
pub(crate) mod files {
    use std::path::Path;

    pub fn read(path: &Path) -> Result<Option<String>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn write(path: &Path, text: &str) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(fail)?;
        }
        let beside = path.with_extension("json.tmp");
        std::fs::write(&beside, text).map_err(fail)?;
        std::fs::rename(&beside, path).map_err(fail)
    }
}

#[cfg(windows)]
mod windows;

/// The user the safety net's task runs as: this process's token's SID (`S-1-5-21-...`), never a name from the
/// environment. Off Windows, and if the token can't be read, the reason, which the task's registration then fails
/// with.
pub fn task_user() -> Result<String, String> {
    #[cfg(windows)]
    {
        windows::token_user()
    }
    #[cfg(not(windows))]
    {
        Err("only Windows has a token's user".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The task's user is the token's SID, whatever the environment says (an SSH session's USERDOMAIN is
    /// WORKGROUP); off Windows there is none.
    #[test]
    fn the_task_user_is_the_tokens_sid_never_the_environments_name() {
        let user = task_user();
        if cfg!(windows) {
            let sid = user.expect("this process's token has a user");
            assert!(sid.starts_with("S-1-5-"), "{sid}");
            std::env::set_var("USERDOMAIN", "WORKGROUP");
            assert_eq!(task_user().unwrap(), sid);
        } else {
            assert!(user.is_err());
        }
    }

    #[test]
    fn a_place_and_a_value_are_said_as_regedit_has_them() {
        let place = Place::user(r"Software\Microsoft\Windows NT\CurrentVersion\Winlogon", "Shell");
        assert_eq!(place.to_string(), r"HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell");
        let value = Value::Text(r#""C:\Users\A B\dashboard.exe" --shell"#.into());
        assert_eq!(value.to_string(), r#"""C:\Users\A B\dashboard.exe" --shell""#);
        assert_eq!(Value::Number(0).to_string(), "0 (DWORD)");
        assert_eq!(Value::Raw { kind: 3, data: vec![1, 0xAB] }.to_string(), "type 3, 01AB");
    }

    #[test]
    fn the_fake_never_writes_the_machines_hive() {
        let mut fake = Fake::default();
        let place = Place::machine("Software", "Shell");
        assert!(fake.write(&place, Some(&Value::Text("x".into()))).is_err());
        assert!(fake.values.is_empty() && fake.writes.is_empty());
    }

    #[test]
    fn a_dry_run_reads_through_and_writes_nothing() {
        let place = Place::user("Software", "Shell");
        let mut fake = Fake::default();
        fake.values.insert(place.clone(), Value::Text("explorer.exe".into()));
        let mut dry = Dry(fake);
        assert_eq!(dry.read(&place).unwrap(), Some(Value::Text("explorer.exe".into())));
        dry.write(&place, None).unwrap();
        dry.set_task("T", None).unwrap();
        dry.write_file(Path::new("record.json"), "{}").unwrap();
        assert!(dry.0.writes.is_empty());
        assert!(dry.0.values.contains_key(&place));
    }
}
