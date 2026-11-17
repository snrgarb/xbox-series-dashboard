//! Turning shell mode on and off (`--shell-on`, `--shell-off`, Settings > System), whether it may be turned on, and
//! the sentinel's check (docs/spec/shell-mode.md, 3 and 4; docs/research/shell.md, 6 and 8).
//!
//! On writes four things, in this order, each value's before-state saved in the record first:
//!
//! 1. the logon task `XboxSeriesDashboard.SafetyNet`, which starts explorer 90 s after sign-in if none runs;
//! 2. `HKCU\...\UserProfileEngagement\ScoobeSystemSettingEnabled` = 0;
//! 3. `HKCU\...\Run\XboxSeriesDashboardSentinel` = `"<exe>" --sentinel`;
//! 4. last, `HKCU\...\Winlogon\Shell` = `"<exe>" --shell`, and only once `<exe> --shell-probe` has answered.
//!
//! Off puts each back as the record had it before (none deleted), the sentinel first and `Shell` next, then the rest.
//! A value that no longer holds what on wrote is left alone, and said. Every write goes through [`System`].
use std::path::{Path, PathBuf};

use model::shell_mode::{Refusal, ShellMode};
use serde_json::{json, Map, Value as Json};

use crate::contract;
use crate::system::{Place, System, Task, Value};

pub const WINLOGON: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Winlogon";
pub const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const ENGAGEMENT: &str = r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement";
/// Where the WindowsLogon policy's OverrideShellProgram lands (Policy CSP).
pub const POLICY: &str = r"SOFTWARE\Microsoft\PolicyManager\current\device\WindowsLogon";
pub const SENTINEL: &str = "XboxSeriesDashboardSentinel";
pub const SAFETY_NET: &str = "XboxSeriesDashboard.SafetyNet";
/// The Bundle's Boot shell's own (xbox-series-theme bootshell/Setup.cs).
pub const BUNDLE_SENTINEL: &str = "XboxSeriesBootShellSentinel";
pub const BUNDLE_SAFETY_NET: &str = "XboxSeries.BootShell.SafetyNet";
pub const BUNDLE_EXE: &str = "XboxSeries.BootShell.exe";
/// How long after sign-in the safety net looks for explorer.
pub const SAFETY_NET_DELAY: u32 = 90;

pub fn shell_place() -> Place {
    Place::user(WINLOGON, "Shell")
}

pub fn sentinel_place() -> Place {
    Place::user(RUN, SENTINEL)
}

pub fn scoobe_place() -> Place {
    Place::user(ENGAGEMENT, "ScoobeSystemSettingEnabled")
}

/// Where shell mode reads and writes, and for whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The installed exe, which `Shell` and the sentinel name: `%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe`.
    pub installed: PathBuf,
    /// This process's own exe.
    pub running: PathBuf,
    /// The data folder: the record is `<data>\shell\record.json`, the logs `<data>\shell\logs\`.
    pub data: PathBuf,
    /// Windows' folder, `C:\Windows`: conhost, PowerShell and explorer are found under it.
    pub windows: PathBuf,
}

impl Paths {
    /// This PC's: the installed exe under `local_app_data`, this process's, the data folder and Windows' folder.
    pub fn this_pc(local_app_data: &Path, data: &Path) -> Self {
        let windows = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        Self {
            installed: installed_exe(local_app_data),
            running: std::env::current_exe().unwrap_or_default(),
            data: data.into(),
            windows,
        }
    }

    pub fn folder(&self) -> PathBuf {
        under(&self.data, "shell")
    }

    pub fn record(&self) -> PathBuf {
        under(&self.folder(), "record.json")
    }

    pub fn logs(&self) -> PathBuf {
        under(&self.folder(), "logs")
    }

    /// `Shell`'s value: quoted, as the Bundle learned, since a profile path with a space splits otherwise.
    pub fn shell_command(&self) -> String {
        command(&self.installed, contract::SHELL)
    }

    pub fn sentinel_command(&self) -> String {
        command(&self.installed, contract::SENTINEL)
    }

    pub fn explorer(&self) -> PathBuf {
        under(&self.windows, "explorer.exe")
    }

    /// The safety net: windowless PowerShell under conhost, never this exe, so it covers a missing or broken one.
    pub fn safety_net(&self) -> Task {
        let system32 = under(&self.windows, "System32");
        let quote = |path: &Path| path.display().to_string().replace('\'', "''");
        let log = quote(&under(&self.logs(), "safety-net.log"));
        let script = format!(
            "$s = (Get-Process -Id $PID).SessionId; if (-not (Get-Process explorer -ErrorAction SilentlyContinue | \
             Where-Object SessionId -eq $s)) {{ Start-Process '{explorer}'; Add-Content -LiteralPath '{log}' -Value \
             ((Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff') + '  no explorer {SAFETY_NET_DELAY} s after sign-in: \
             started it') }}",
            explorer = quote(&self.explorer()),
        );
        Task {
            command: under(&system32, "conhost.exe").display().to_string(),
            arguments: format!(
                "--headless {} -NoProfile -NonInteractive -Command \"{script}\"",
                under(&system32, r"WindowsPowerShell\v1.0\powershell.exe").display()
            ),
            delay_seconds: SAFETY_NET_DELAY,
        }
    }
}

/// `"<exe>" <argument>`.
fn command(exe: &Path, argument: &str) -> String {
    format!("\"{}\" {argument}", exe.display())
}

/// `rest` under `base`, joined as Windows joins a path, whatever the system the code runs on: every path shell mode
/// writes is a Windows one, and the unit tests on Linux check them as Windows has them.
fn under(base: &Path, rest: &str) -> PathBuf {
    PathBuf::from(format!("{}\\{rest}", base.display().to_string().trim_end_matches(['\\', '/'])))
}

/// The per-user install's exe (docs/research/shell.md, 8): kept apart from the data folder, so an uninstall can keep
/// the data.
pub fn installed_exe(local_app_data: &Path) -> PathBuf {
    under(local_app_data, r"Programs\XboxSeriesDashboard\dashboard.exe")
}

/// Whether two paths name one file, as Windows compares them.
fn same_path(a: &Path, b: &Path) -> bool {
    let normal = |p: &Path| p.display().to_string().replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase();
    normal(a) == normal(b)
}

/// Whether a `Shell` value is plain explorer: `explorer.exe`, or a path to it, quoted or not, with nothing after.
fn is_explorer(shell: &str) -> bool {
    let shell = shell.trim().trim_matches('"').to_ascii_lowercase();
    shell == "explorer.exe" || shell == "explorer" || shell.ends_with("\\explorer.exe")
}

/// One thing shell mode writes, as the record names it.
#[derive(Debug, Clone)]
enum Want {
    Value(Place, Value),
    Task(&'static str, Task),
}

#[derive(Debug, Clone)]
struct Step {
    name: String,
    want: Want,
}

/// What a step holds now: a value or a task, either as the record keeps it.
#[derive(Debug, Clone, PartialEq)]
enum Held {
    Value(Value),
    Task(Task),
}

impl Held {
    fn same(&self, other: &Held) -> bool {
        match (self, other) {
            (Held::Value(a), Held::Value(b)) => a.same(b),
            (Held::Task(a), Held::Task(b)) => a.same(b),
            _ => false,
        }
    }
}

fn show(held: &Option<Held>) -> String {
    match held {
        None => "none".into(),
        Some(Held::Value(value)) => value.to_string(),
        Some(Held::Task(task)) => task.to_string(),
    }
}

fn same(a: &Option<Held>, b: &Option<Held>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same(b),
        _ => false,
    }
}

impl Step {
    fn read(&self, system: &dyn System) -> Result<Option<Held>, String> {
        Ok(match &self.want {
            Want::Value(place, _) => system.read(place)?.map(Held::Value),
            Want::Task(name, _) => system.task(name)?.map(Held::Task),
        })
    }

    fn wanted(&self) -> Held {
        match &self.want {
            Want::Value(_, value) => Held::Value(value.clone()),
            Want::Task(_, task) => Held::Task(task.clone()),
        }
    }

    fn holds(&self, current: &Option<Held>) -> bool {
        current.as_ref().is_some_and(|current| current.same(&self.wanted()))
    }

    fn write(&self, system: &mut dyn System, held: &Option<Held>) -> Result<(), String> {
        match (&self.want, held) {
            (Want::Value(place, _), None) => system.write(place, None),
            (Want::Value(place, _), Some(Held::Value(value))) => system.write(place, Some(value)),
            (Want::Task(name, _), None) => system.set_task(name, None),
            (Want::Task(name, _), Some(Held::Task(task))) => system.set_task(name, Some(task)),
            (_, Some(other)) => Err(format!("{}: the record has {} for it", self.name, show(&Some(other.clone())))),
        }
    }

    /// The line for a write, made or (in a dry run) only said.
    fn said(&self, dry: bool, from: &Option<Held>, to: &Option<Held>) -> String {
        let would = if dry { "would " } else { "" };
        match (&self.want, to) {
            (Want::Value(..), Some(_)) => {
                format!("shell: {would}set {} = {} (now {})", self.name, show(to), show(from))
            }
            (Want::Value(..), None) => format!("shell: {would}delete {} (now {})", self.name, show(from)),
            (Want::Task(..), Some(_)) => format!("shell: {would}register {}: {}", self.name, show(to)),
            (Want::Task(..), None) => format!("shell: {would}delete {} (now {})", self.name, show(from)),
        }
    }

    /// How to put it back by hand, from a terminal.
    fn by_hand(&self, before: &Option<Held>) -> String {
        match (&self.want, before) {
            (Want::Value(place, _), None) => format!("reg delete \"HKCU\\{}\" /v {} /f", place.key, place.name),
            (Want::Value(place, _), Some(Held::Value(Value::Number(n)))) => {
                format!("reg add \"HKCU\\{}\" /v {} /t REG_DWORD /d {n} /f", place.key, place.name)
            }
            (Want::Value(place, _), Some(_)) => {
                format!("set HKCU\\{}\\{} back to {} in regedit", place.key, place.name, show(before))
            }
            (Want::Task(name, _), None) => format!("schtasks /Delete /TN {name} /F"),
            (Want::Task(name, _), Some(_)) => format!("register {name} again as {}", show(before)),
        }
    }
}

/// The four steps, in on's order.
fn steps(paths: &Paths) -> [Step; 4] {
    let value = |place: Place, value: Value| Step { name: place.to_string(), want: Want::Value(place, value) };
    [
        Step { name: format!("Task {SAFETY_NET}"), want: Want::Task(SAFETY_NET, paths.safety_net()) },
        value(scoobe_place(), Value::Number(0)),
        value(sentinel_place(), Value::Text(paths.sentinel_command())),
        value(shell_place(), Value::Text(paths.shell_command())),
    ]
}

/// The order off undoes them in: the sentinel first, since one left by an off that stopped before `Shell` would
/// write `Shell` back at the next sign-in; then `Shell`; then the rest, last written first.
const UNDO: [usize; 4] = [2, 3, 1, 0];

/// What a change came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Done: on (or off) from the next sign-in.
    Done,
    /// It was so already: nothing written.
    Already,
    /// Not turned on, for these reasons: nothing written.
    Refused(Vec<Refusal>),
    /// It couldn't be done. What it had written is put back (on), or `Shell` is off and something else is left
    /// (off); the lines say which.
    Failed,
}

impl Outcome {
    /// The exit code `--shell-on` and `--shell-off` end with: 0 done or already, 3 refused, 4 failed.
    pub fn code(&self) -> i32 {
        match self {
            Outcome::Done | Outcome::Already => 0,
            Outcome::Refused(_) => 3,
            Outcome::Failed => 4,
        }
    }
}

/// The record: the exe on named, and each step's value as it was before shell mode first wrote it.
#[derive(Debug, Clone, Default)]
struct Record {
    installed: PathBuf,
    before: Map<String, Json>,
}

fn held_json(held: &Option<Held>) -> Json {
    match held {
        None => Json::Null,
        Some(Held::Value(Value::Text(text))) => json!({ "text": text }),
        Some(Held::Value(Value::Number(n))) => json!({ "number": n }),
        Some(Held::Value(Value::Raw { kind, data })) => {
            json!({ "raw": { "kind": kind, "hex": data.iter().map(|b| format!("{b:02X}")).collect::<String>() } })
        }
        Some(Held::Task(task)) => json!({ "task": {
            "command": task.command, "arguments": task.arguments, "delay_seconds": task.delay_seconds } }),
    }
}

fn json_held(json: &Json) -> Result<Option<Held>, String> {
    let bad = || format!("the record's {json} is no value");
    if json.is_null() {
        return Ok(None);
    }
    if let Some(text) = json.get("text").and_then(Json::as_str) {
        return Ok(Some(Held::Value(Value::Text(text.into()))));
    }
    if let Some(n) = json.get("number").and_then(Json::as_u64) {
        return Ok(Some(Held::Value(Value::Number(u32::try_from(n).map_err(|_| bad())?))));
    }
    if let Some(raw) = json.get("raw") {
        let kind = raw.get("kind").and_then(Json::as_u64).ok_or_else(bad)? as u32;
        let hex = raw.get("hex").and_then(Json::as_str).ok_or_else(bad)?;
        let data = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| bad()))
            .collect::<Result<_, _>>()?;
        return Ok(Some(Held::Value(Value::Raw { kind, data })));
    }
    if let Some(task) = json.get("task") {
        let text = |key| task.get(key).and_then(Json::as_str).map(String::from).ok_or_else(bad);
        return Ok(Some(Held::Task(Task {
            command: text("command")?,
            arguments: text("arguments")?,
            delay_seconds: task.get("delay_seconds").and_then(Json::as_u64).ok_or_else(bad)? as u32,
        })));
    }
    Err(bad())
}

impl Record {
    fn load(system: &dyn System, paths: &Paths) -> Result<Option<Record>, String> {
        let file = paths.record();
        let Some(text) = system.read_file(&file)? else { return Ok(None) };
        let json: Json = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        let installed = json.get("installed").and_then(Json::as_str).ok_or(format!("{}: no exe", file.display()))?;
        let before = json.get("before").and_then(Json::as_object).cloned().unwrap_or_default();
        Ok(Some(Record { installed: installed.into(), before }))
    }

    fn save(&self, system: &mut dyn System, paths: &Paths) -> Result<(), String> {
        let json = json!({ "version": 1, "installed": self.installed.display().to_string(), "before": self.before });
        let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        system.write_file(&paths.record(), &text)
    }

    fn before(&self, name: &str) -> Result<Option<Option<Held>>, String> {
        self.before.get(name).map(json_held).transpose()
    }
}

/// Why shell mode can't be turned on now, if anything (docs/research/shell.md, 4.5 and 6: F8, F9, F11).
pub fn refusals(system: &dyn System, paths: &Paths) -> Result<Vec<Refusal>, String> {
    let mut found = Vec::new();
    let mut add = |refusal| {
        if !found.contains(&refusal) {
            found.push(refusal);
        }
    };
    let shell = system.read(&shell_place())?;
    let shell = shell.as_ref().map(|value| value.text().unwrap_or("").to_string());
    let bundle_shell =
        shell.as_deref().is_some_and(|shell| shell.to_ascii_lowercase().contains(&BUNDLE_EXE.to_ascii_lowercase()));
    if bundle_shell
        || system.read(&Place::user(RUN, BUNDLE_SENTINEL))?.is_some()
        || system.task(BUNDLE_SAFETY_NET)?.is_some()
    {
        add(Refusal::BundleBootShell);
    }
    if let Some(shell) = shell.as_deref() {
        let ours = shell.eq_ignore_ascii_case(&paths.shell_command())
            || shell.to_ascii_lowercase().contains(&paths.installed.display().to_string().to_ascii_lowercase());
        if !ours && !bundle_shell && !is_explorer(shell) {
            add(Refusal::OtherShell);
        }
    }
    match system.read(&Place::machine(WINLOGON, "Shell"))? {
        Some(Value::Text(machine)) if is_explorer(&machine) => {}
        _ => add(Refusal::MachineShell),
    }
    match system.read(&Place::machine(POLICY, "OverrideShellProgram"))? {
        None | Some(Value::Number(0)) => {}
        Some(_) => add(Refusal::ShellOverridden),
    }
    if !same_path(&paths.running, &paths.installed) {
        add(Refusal::NotInstalled);
    }
    Ok(found)
}

/// Where shell mode stands: on while there is a record, else off or refused. Reads only.
pub fn status(system: &dyn System, paths: &Paths) -> ShellMode {
    match Record::load(system, paths) {
        Ok(Some(_)) => ShellMode::On,
        Ok(None) => match refusals(system, paths) {
            Ok(found) => found.first().map_or(ShellMode::Off, |refusal| ShellMode::Refused(*refusal)),
            Err(_) => ShellMode::Failed,
        },
        Err(_) => ShellMode::Failed,
    }
}

/// Turns shell mode on from the next sign-in, saying each step through `say`.
pub fn turn_on(system: &mut dyn System, paths: &Paths, say: &mut dyn FnMut(String)) -> Outcome {
    let dry = system.dry();
    if dry {
        say("shell: a dry run (--dry-shell): nothing is written".into());
    }
    say(format!("shell: turning on for {}", paths.installed.display()));
    match refusals(system, paths) {
        Ok(found) if !found.is_empty() => {
            for refusal in &found {
                say(format!("shell: refused: {}: {}", refusal.key(), refusal.says()));
            }
            say("shell: nothing was written".into());
            return Outcome::Refused(found);
        }
        Ok(_) => {}
        Err(why) => {
            say(format!("shell: failed: couldn't look: {why}; nothing was written"));
            return Outcome::Failed;
        }
    }
    // Never a Shell naming a missing or broken exe: it is there, and it starts.
    if !system.exists(&paths.installed) {
        say(format!("shell: failed: there is no {}; nothing was written", paths.installed.display()));
        return Outcome::Failed;
    }
    if let Err(why) = system.probe(&paths.installed) {
        say(format!("shell: failed: {why}; nothing was written"));
        return Outcome::Failed;
    }
    say(format!("shell: {} {} answered", paths.installed.display(), contract::PROBE));

    let (mut record, existed) = match Record::load(system, paths) {
        Ok(Some(record)) => (record, true),
        Ok(None) => (Record { installed: paths.installed.clone(), before: Map::new() }, false),
        Err(why) => {
            say(format!("shell: failed: {why}; nothing was written"));
            return Outcome::Failed;
        }
    };
    let steps = steps(paths);
    let mut written: Vec<usize> = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        let result = (|| -> Result<(), String> {
            let current = step.read(system)?;
            if !record.before.contains_key(&step.name) {
                // What already names this Dashboard with no record to say so (a record lost) was shell mode's, not
                // the PC's own: before it there was none. ScoobeSystemSettingEnabled at 0 may be the user's own.
                let before = if i != 1 && step.holds(&current) { None } else { current.clone() };
                record.before.insert(step.name.clone(), held_json(&before));
                record.save(system, paths)?;
                let would = if dry { "would save" } else { "saved" };
                say(format!(
                    "shell: {would} the record {} with {} as {}",
                    paths.record().display(),
                    step.name,
                    show(&before)
                ));
            }
            if step.holds(&current) {
                say(format!("shell: {} holds {}", step.name, show(&current)));
                return Ok(());
            }
            let wanted = Some(step.wanted());
            step.write(system, &wanted)?;
            written.push(i);
            say(step.said(dry, &current, &wanted));
            // Shell is read back: a value that landed otherwise is put back with the rest.
            if !dry && i == 3 && !same(&step.read(system)?, &wanted) {
                return Err(format!("{} reads back as {}", step.name, show(&step.read(system)?)));
            }
            Ok(())
        })();
        if let Err(why) = result {
            say(format!("shell: failed: {why}"));
            put_back(system, paths, &steps, &written, &record, existed, say);
            return Outcome::Failed;
        }
    }
    if written.is_empty() && existed {
        say("shell: on already".into());
        return Outcome::Already;
    }
    let would = if dry { "would be " } else { "" };
    say(format!("shell: {would}on from the next sign-in: Windows starts {}", paths.shell_command()));
    Outcome::Done
}

/// After a failed on: what it wrote, put back as the record had it, the sentinel before Shell.
fn put_back(
    system: &mut dyn System,
    paths: &Paths,
    steps: &[Step; 4],
    written: &[usize],
    record: &Record,
    existed: bool,
    say: &mut dyn FnMut(String),
) {
    let dry = system.dry();
    let mut all = true;
    for i in UNDO.into_iter().filter(|i| written.contains(i)) {
        let step = &steps[i];
        let before = record.before(&step.name).ok().flatten().unwrap_or(None);
        match step.write(system, &before) {
            Ok(()) => say(format!(
                "shell: put back: {}",
                step.said(dry, &Some(step.wanted()), &before).trim_start_matches("shell: ")
            )),
            Err(why) => {
                all = false;
                say(format!("shell: couldn't put back {}: {why}; by hand: {}", step.name, step.by_hand(&before)));
            }
        }
    }
    if all && !existed && !dry {
        let aside = under(&paths.folder(), &format!("record.failed-{}.json", stamp()));
        let _ = system.rename_file(&paths.record(), &aside);
    }
    say(if all {
        "shell: everything it had written is put back; the next sign-in is as before".into()
    } else {
        format!("shell: not everything is put back: the record {} is kept for --shell-off", paths.record().display())
    });
}

/// Turns shell mode off from the next sign-in, saying each step through `say`.
pub fn turn_off(system: &mut dyn System, paths: &Paths, say: &mut dyn FnMut(String)) -> Outcome {
    let dry = system.dry();
    if dry {
        say("shell: a dry run (--dry-shell): nothing is written".into());
    }
    let record = match Record::load(system, paths) {
        Ok(record) => record,
        Err(why) => {
            say(format!("shell: failed: {why}; nothing was written"));
            return Outcome::Failed;
        }
    };
    let Some(record) = record else { return off_without_record(system, paths, say) };
    say(format!("shell: turning off what was turned on for {}", record.installed.display()));
    let paths = Paths { installed: record.installed.clone(), ..paths.clone() };
    let steps = steps(&paths);
    let mut sentinel_taken: Option<Option<Held>> = None;
    let mut all = true;
    for i in UNDO {
        let step = &steps[i];
        let before = match record.before(&step.name) {
            Ok(Some(before)) => before,
            Ok(None) => continue,
            Err(why) => {
                say(format!("shell: {}: {why}; left alone", step.name));
                all = false;
                continue;
            }
        };
        let result = (|| -> Result<(), String> {
            let current = step.read(system)?;
            if same(&current, &before) {
                say(format!("shell: {} as before, {}", step.name, show(&current)));
            } else if !step.holds(&current) {
                say(format!(
                    "shell: {} changed since shell mode was turned on, left alone: {} (before {})",
                    step.name,
                    show(&current),
                    show(&before)
                ));
            } else {
                step.write(system, &before)?;
                say(step.said(dry, &current, &before));
                if i == 2 {
                    sentinel_taken = Some(current);
                }
            }
            Ok(())
        })();
        if let Err(why) = result {
            say(format!("shell: failed: {why}"));
            match i {
                // Nothing is changed yet: shell mode stays on, whole.
                2 => {
                    say("shell: still on: nothing was changed".into());
                    return Outcome::Failed;
                }
                // Shell couldn't be put back: the sentinel goes back too, so it is on, whole, as before.
                3 => {
                    if let Some(taken) = &sentinel_taken {
                        let wanted = Some(steps[2].wanted());
                        match steps[2].write(system, &wanted) {
                            Ok(()) => say(format!(
                                "shell: put back: {}",
                                steps[2].said(dry, taken, &wanted).trim_start_matches("shell: ")
                            )),
                            Err(why) => say(format!("shell: couldn't put the sentinel back: {why}")),
                        }
                    }
                    say(format!("shell: still on; to turn it off by hand: {}", step.by_hand(&before)));
                    return Outcome::Failed;
                }
                _ => all = false,
            }
        }
    }
    if !all {
        say(format!(
            "shell: off from the next sign-in, but not everything is put back: the record {} is kept, so --shell-off \
             can try again",
            paths.record().display()
        ));
        return Outcome::Failed;
    }
    let aside = under(&paths.folder(), &format!("record.{}.json", stamp()));
    let would = if dry { "would rename" } else { "renamed" };
    if let Err(why) = system.rename_file(&paths.record(), &aside) {
        say(format!("shell: couldn't rename the record: {why}"));
    } else {
        say(format!("shell: {would} the record to {}", aside.display()));
    }
    let would = if dry { "would be " } else { "" };
    say(format!("shell: {would}off from the next sign-in: Windows starts its desktop"));
    Outcome::Done
}

/// Off with no record (lost, or never written): only what names this exe is taken away, the sentinel first.
fn off_without_record(system: &mut dyn System, paths: &Paths, say: &mut dyn FnMut(String)) -> Outcome {
    let dry = system.dry();
    let steps = steps(paths);
    let names_us = |value: &Option<Held>| match value {
        Some(Held::Value(Value::Text(text))) => {
            text.to_ascii_lowercase().contains(&paths.installed.display().to_string().to_ascii_lowercase())
        }
        Some(Held::Task(task)) => task.same(&paths.safety_net()),
        _ => false,
    };
    let mut changed = false;
    for i in [2, 3, 0] {
        let step = &steps[i];
        let current = match step.read(system) {
            Ok(current) => current,
            Err(why) => {
                say(format!("shell: failed: {why}"));
                return Outcome::Failed;
            }
        };
        if !names_us(&current) {
            continue;
        }
        if let Err(why) = step.write(system, &None) {
            say(format!("shell: failed: {why}; by hand: {}", step.by_hand(&None)));
            return Outcome::Failed;
        }
        say(step.said(dry, &current, &None));
        changed = true;
    }
    if !changed {
        say("shell: off already: no record, and nothing names this Dashboard".into());
        return Outcome::Already;
    }
    let would = if dry { "would be " } else { "" };
    say(format!("shell: {would}off from the next sign-in (there was no record: what named this Dashboard is deleted)"));
    Outcome::Done
}

/// What the sentinel does, once it has looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sentinel {
    /// The --shell process runs, or shell mode is off: nothing.
    Nothing,
    /// Explorer was the shell from the start, though shell mode is on: `Shell` is written back (if it had gone) and
    /// the Dashboard is to start.
    StartDashboard,
}

/// The sentinel's check (`--sentinel`, from explorer's startup run): with no --shell process in the session but a
/// record, explorer was the shell from the start, as after a Windows upgrade dropped `Shell`. It writes `Shell` back
/// and has the Dashboard started. With no record it does nothing: a sentinel left behind can't turn shell mode on.
pub fn sentinel(system: &mut dyn System, paths: &Paths, shell_runs: bool, say: &mut dyn FnMut(String)) -> Sentinel {
    if shell_runs {
        say("sentinel: the --shell process runs; nothing to do".into());
        return Sentinel::Nothing;
    }
    let record = match Record::load(system, paths) {
        Ok(Some(record)) => record,
        Ok(None) => {
            say("sentinel: shell mode is off (no record); nothing to do".into());
            return Sentinel::Nothing;
        }
        Err(why) => {
            say(format!("sentinel: {why}; nothing done"));
            return Sentinel::Nothing;
        }
    };
    say("sentinel: no --shell process in this session, so explorer was the shell from the start".into());
    let paths = Paths { installed: record.installed, ..paths.clone() };
    let wanted = Value::Text(paths.shell_command());
    match system.read(&shell_place()) {
        Ok(Some(shell)) if shell.same(&wanted) => say(format!("sentinel: Shell holds {shell}")),
        Ok(current) => {
            let now = current.map_or("none".to_string(), |value| value.to_string());
            match system.write(&shell_place(), Some(&wanted)) {
                Ok(()) => say(format!("sentinel: Shell: {now} -> {wanted}")),
                Err(why) => say(format!("sentinel: couldn't write Shell back: {why}")),
            }
        }
        Err(why) => say(format!("sentinel: couldn't read Shell: {why}")),
    }
    Sentinel::StartDashboard
}

/// The time now, for a file's name: `20261117-143005`.
pub fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

#[cfg(test)]
mod tests;
