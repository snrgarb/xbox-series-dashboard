//! Shell mode on and off against a made-up PC ([`Fake`]): nothing here touches a real registry or Task Scheduler.
use super::*;
use crate::system::Fake;

const LOCAL: &str = r"C:\Users\Test User\AppData\Local";

fn paths() -> Paths {
    let installed = installed_exe(Path::new(LOCAL));
    Paths {
        running: installed.clone(),
        installed,
        data: PathBuf::from(format!(r"{LOCAL}\XboxSeriesDashboard")),
        windows: PathBuf::from(r"C:\Windows"),
    }
}

/// A PC as Windows leaves it: HKLM's Shell is explorer.exe, nothing of the user's, the Dashboard installed.
fn pc() -> Fake {
    let mut fake = Fake::default();
    fake.values.insert(Place::machine(WINLOGON, "Shell"), Value::Text("explorer.exe".into()));
    fake.files.insert(paths().installed, "MZ".into());
    fake
}

fn on(fake: &mut Fake) -> (Outcome, Vec<String>) {
    let mut said = Vec::new();
    let outcome = turn_on(fake, &paths(), &mut |line| said.push(line));
    (outcome, said)
}

fn off(fake: &mut Fake) -> (Outcome, Vec<String>) {
    let mut said = Vec::new();
    let outcome = turn_off(fake, &paths(), &mut |line| said.push(line));
    (outcome, said)
}

/// The values and tasks as they are, without the files.
fn state(fake: &Fake) -> (BTreeMapOf, BTreeMapOfTasks) {
    (fake.values.clone(), fake.tasks.clone())
}

type BTreeMapOf = std::collections::BTreeMap<Place, Value>;
type BTreeMapOfTasks = std::collections::BTreeMap<String, Task>;

fn shell_value() -> Value {
    Value::Text(format!("\"{LOCAL}\\Programs\\XboxSeriesDashboard\\dashboard.exe\" --shell"))
}

#[test]
fn on_writes_the_safety_net_first_and_shell_last_and_off_puts_back_exactly_what_was_there() {
    let mut fake = pc();
    // The user had SCOOBE on, and no Shell of their own.
    fake.values.insert(scoobe_place(), Value::Number(1));
    let before = state(&fake);

    let (outcome, said) = on(&mut fake);
    assert_eq!(outcome, Outcome::Done, "{said:#?}");
    let writes: Vec<&String> = fake.writes.iter().filter(|w| !w.starts_with("save")).collect();
    assert_eq!(
        writes,
        [
            "register Task XboxSeriesDashboard.SafetyNet",
            r"set HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement\ScoobeSystemSettingEnabled = 0 (DWORD)",
            &format!(
                r#"set HKCU\Software\Microsoft\Windows\CurrentVersion\Run\XboxSeriesDashboardSentinel = ""{LOCAL}\Programs\XboxSeriesDashboard\dashboard.exe" --sentinel""#
            ),
            &format!(r"set HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell = {}", shell_value()),
        ]
    );
    // The record is saved before each value is first written.
    let first_save = fake.writes.iter().position(|w| w.starts_with("save")).unwrap();
    assert!(first_save < fake.writes.iter().position(|w| w.starts_with("register")).unwrap());
    assert_eq!(fake.values.get(&shell_place()), Some(&shell_value()));
    let task = &fake.tasks[SAFETY_NET];
    assert_eq!(task.command, r"C:\Windows\System32\conhost.exe");
    assert!(task.arguments.starts_with(r"--headless C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"));
    assert!(task.arguments.contains(r"Start-Process 'C:\Windows\explorer.exe'"));
    assert_eq!(task.delay_seconds, 90);
    assert_eq!(status(&fake, &paths()), ShellMode::On);
    assert!(
        said.iter()
            .any(|l| l
                .starts_with("shell: set HKCU\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\\Shell = "))
    );

    fake.writes.clear();
    let (outcome, said) = off(&mut fake);
    assert_eq!(outcome, Outcome::Done, "{said:#?}");
    assert_eq!(state(&fake), before, "{said:#?}");
    // The sentinel goes before Shell, then the rest.
    let order: Vec<&String> = fake.writes.iter().filter(|w| !w.starts_with("rename")).collect();
    assert_eq!(order[0], r"delete HKCU\Software\Microsoft\Windows\CurrentVersion\Run\XboxSeriesDashboardSentinel");
    assert_eq!(order[1], r"delete HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell");
    assert!(!fake.files.contains_key(&paths().record()), "the record is put aside");
    assert_eq!(status(&fake, &paths()), ShellMode::Off);
}

#[test]
fn a_shell_of_explorer_the_user_had_is_put_back_as_it_was() {
    let mut fake = pc();
    fake.values.insert(shell_place(), Value::Text("explorer.exe".into()));
    let before = state(&fake);
    assert_eq!(on(&mut fake).0, Outcome::Done);
    assert_eq!(fake.values.get(&shell_place()), Some(&shell_value()));
    assert_eq!(off(&mut fake).0, Outcome::Done);
    assert_eq!(state(&fake), before);
}

#[test]
fn turned_on_twice_it_is_on_already_and_the_first_before_is_kept() {
    let mut fake = pc();
    let before = state(&fake);
    assert_eq!(on(&mut fake).0, Outcome::Done);
    assert_eq!(on(&mut fake).0, Outcome::Already);
    assert_eq!(off(&mut fake).0, Outcome::Done);
    assert_eq!(state(&fake), before);
}

/// What a refusal's case does to the PC (and the paths) before shell mode is asked.
type Arrange = Box<dyn Fn(&mut Fake, &mut Paths)>;

#[test]
fn each_reason_to_refuse_is_said_and_nothing_is_written() {
    let cases: Vec<(&str, Arrange, Refusal)> = vec![
        (
            "the Bundle's sentinel",
            Box::new(|f, _| {
                f.values.insert(Place::user(RUN, BUNDLE_SENTINEL), Value::Text("x --sentinel".into()));
            }),
            Refusal::BundleBootShell,
        ),
        (
            "the Bundle's safety net",
            Box::new(|f, _| {
                f.tasks.insert(
                    BUNDLE_SAFETY_NET.into(),
                    Task { command: "c".into(), arguments: "a".into(), delay_seconds: 90 },
                );
            }),
            Refusal::BundleBootShell,
        ),
        (
            "Shell naming the Bundle's Boot shell",
            Box::new(|f, _| {
                let bundle = r#""C:\Users\T\AppData\Local\XboxSeries.BootShell\XboxSeries.BootShell.exe""#;
                f.values.insert(shell_place(), Value::Text(bundle.into()));
            }),
            Refusal::BundleBootShell,
        ),
        (
            "Shell naming another program",
            Box::new(|f, _| {
                f.values.insert(shell_place(), Value::Text(r"C:\Tools\kiosk.exe".into()));
            }),
            Refusal::OtherShell,
        ),
        (
            "HKLM's Shell not explorer",
            Box::new(|f, _| {
                f.values.insert(Place::machine(WINLOGON, "Shell"), Value::Text("cmd.exe".into()));
            }),
            Refusal::MachineShell,
        ),
        (
            "HKLM's Shell not there",
            Box::new(|f, _| {
                f.values.remove(&Place::machine(WINLOGON, "Shell"));
            }),
            Refusal::MachineShell,
        ),
        (
            "OverrideShellProgram set",
            Box::new(|f, _| {
                f.values.insert(Place::machine(POLICY, "OverrideShellProgram"), Value::Number(1));
            }),
            Refusal::ShellOverridden,
        ),
        (
            "a portable copy",
            Box::new(|_, p| p.running = PathBuf::from(r"D:\Downloads\dashboard\dashboard.exe")),
            Refusal::NotInstalled,
        ),
    ];
    for (what, set_up, refusal) in cases {
        let (mut fake, mut paths) = (pc(), paths());
        set_up(&mut fake, &mut paths);
        let before = state(&fake);
        let mut said = Vec::new();
        let outcome = turn_on(&mut fake, &paths, &mut |line| said.push(line));
        assert_eq!(outcome, Outcome::Refused(vec![refusal]), "{what}: {said:#?}");
        assert!(fake.writes.is_empty(), "{what}: {:?}", fake.writes);
        assert_eq!(state(&fake), before, "{what}");
        assert!(said.iter().any(|l| l.starts_with(&format!("shell: refused: {}: ", refusal.key()))), "{what}");
        assert_eq!(status(&fake, &paths), ShellMode::Refused(refusal), "{what}");
        assert_eq!(outcome.code(), 3);
    }
}

#[test]
fn a_shell_of_explorer_or_one_naming_this_dashboard_is_no_reason_to_refuse() {
    for shell in ["explorer.exe", r"C:\Windows\explorer.exe", "\"C:\\Windows\\explorer.exe\""] {
        let mut fake = pc();
        fake.values.insert(shell_place(), Value::Text(shell.into()));
        assert_eq!(refusals(&fake, &paths()).unwrap(), [], "{shell}");
    }
}

#[test]
fn an_exe_that_is_not_there_or_does_not_answer_is_never_named_in_shell() {
    let mut fake = pc();
    fake.files.clear();
    assert_eq!(on(&mut fake).0, Outcome::Failed);
    assert!(fake.writes.is_empty());

    let mut fake = pc();
    fake.probe = Err("dashboard.exe --shell-probe ended with exit code 0xc0000135".into());
    let (outcome, said) = on(&mut fake);
    assert_eq!(outcome, Outcome::Failed);
    assert!(fake.writes.is_empty(), "{:?}", fake.writes);
    assert!(said.last().unwrap().contains("nothing was written"), "{said:#?}");
}

#[test]
fn a_write_that_fails_part_way_through_on_puts_back_everything_and_never_leaves_shell() {
    let fails = [
        format!("Task {SAFETY_NET}"),
        scoobe_place().to_string(),
        sentinel_place().to_string(),
        shell_place().to_string(),
        paths().record().display().to_string(),
    ];
    for fail in fails {
        let mut fake = pc();
        fake.values.insert(scoobe_place(), Value::Number(1));
        let before = state(&fake);
        fake.fail = vec![fail.clone()];
        let (outcome, said) = on(&mut fake);
        assert_eq!(outcome, Outcome::Failed, "{fail}");
        assert_eq!(state(&fake), before, "{fail}: {said:#?}");
        assert!(said.iter().any(|l| l.contains("everything it had written is put back")), "{fail}: {said:#?}");
        assert!(!fake.files.contains_key(&paths().record()), "{fail}: the record is put aside");
    }
}

#[test]
fn a_shell_that_reads_back_otherwise_is_taken_away_again() {
    let mut fake = pc();
    let before = state(&fake);
    fake.garble_shell = Some(r#""C:\Users\Test" User\dashboard.exe --shell"#.into());
    let (outcome, said) = on(&mut fake);
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(state(&fake), before, "{said:#?}");
}

#[test]
fn off_stops_whole_if_the_sentinel_cant_go_and_puts_it_back_if_shell_cant() {
    // The sentinel can't be deleted: nothing is changed, and it is still on.
    let mut fake = pc();
    on(&mut fake);
    let on_state = state(&fake);
    fake.fail = vec![sentinel_place().to_string()];
    let (outcome, said) = off(&mut fake);
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(state(&fake), on_state, "{said:#?}");
    assert_eq!(status(&fake, &paths()), ShellMode::On);

    // Shell can't be put back: the sentinel goes back too, so it is still on, whole.
    let mut fake = pc();
    on(&mut fake);
    let on_state = state(&fake);
    fake.fail = vec![shell_place().to_string()];
    let (outcome, said) = off(&mut fake);
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(state(&fake), on_state, "{said:#?}");
    assert!(said.iter().any(|l| l.contains("reg delete")), "the way by hand: {said:#?}");
}

#[test]
fn off_that_cant_delete_the_task_keeps_the_record_and_a_second_off_finishes() {
    let mut fake = pc();
    let before = state(&fake);
    on(&mut fake);
    fake.fail = vec![format!("Task {SAFETY_NET}")];
    let (outcome, said) = off(&mut fake);
    assert_eq!(outcome, Outcome::Failed);
    assert!(!fake.values.contains_key(&shell_place()), "Shell is off: {said:#?}");
    assert!(!fake.values.contains_key(&sentinel_place()));
    assert!(fake.files.contains_key(&paths().record()));
    fake.fail.clear();
    assert_eq!(off(&mut fake).0, Outcome::Done);
    assert_eq!(state(&fake), before);
}

#[test]
fn off_leaves_alone_what_changed_since() {
    let mut fake = pc();
    on(&mut fake);
    fake.values.insert(scoobe_place(), Value::Number(1));
    let (outcome, said) = off(&mut fake);
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(fake.values.get(&scoobe_place()), Some(&Value::Number(1)));
    assert!(said.iter().any(|l| l.contains("changed since shell mode was turned on, left alone")), "{said:#?}");
}

#[test]
fn off_with_no_record_takes_away_only_what_names_this_dashboard() {
    let mut fake = pc();
    on(&mut fake);
    fake.files.remove(&paths().record());
    let (outcome, said) = off(&mut fake);
    assert_eq!(outcome, Outcome::Done, "{said:#?}");
    assert!(!fake.values.contains_key(&shell_place()) && !fake.values.contains_key(&sentinel_place()));
    assert!(!fake.tasks.contains_key(SAFETY_NET));

    let mut fake = pc();
    fake.values.insert(shell_place(), Value::Text(r"C:\Tools\kiosk.exe".into()));
    assert_eq!(off(&mut fake).0, Outcome::Already);
    assert!(fake.writes.is_empty());
}

#[test]
fn turned_on_again_after_a_lost_record_off_still_deletes_shell() {
    let mut fake = pc();
    on(&mut fake);
    fake.files.remove(&paths().record());
    assert_eq!(on(&mut fake).0, Outcome::Done);
    assert_eq!(off(&mut fake).0, Outcome::Done);
    // What named this Dashboard was shell mode's, and is gone; SCOOBE's 0 may have been the user's, and stays.
    assert!(fake.tasks.is_empty());
    assert_eq!(fake.values.get(&shell_place()), None);
    assert_eq!(fake.values.get(&sentinel_place()), None);
    assert_eq!(fake.values.get(&scoobe_place()), Some(&Value::Number(0)));
}

#[test]
fn a_dry_run_says_what_it_would_write_and_writes_nothing() {
    let mut dry = crate::system::Dry(pc());
    let before = state(&dry.0);
    let mut said = Vec::new();
    assert_eq!(turn_on(&mut dry, &paths(), &mut |line| said.push(line)), Outcome::Done);
    assert!(dry.0.writes.is_empty());
    assert_eq!(state(&dry.0), before);
    assert_eq!(said[0], "shell: a dry run (--dry-shell): nothing is written");
    let would: Vec<&String> =
        said.iter().filter(|l| l.starts_with("shell: would set") || l.starts_with("shell: would register")).collect();
    assert_eq!(would.len(), 4, "{said:#?}");
    assert!(would[0].starts_with("shell: would register Task XboxSeriesDashboard.SafetyNet: at sign-in + 90 s: "));
    assert_eq!(
        would[3],
        &format!(
            r"shell: would set HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell = {} (now none)",
            shell_value()
        )
    );
    assert!(said.last().unwrap().starts_with("shell: would be on from the next sign-in"));

    let mut said = Vec::new();
    assert_eq!(turn_off(&mut dry, &paths(), &mut |line| said.push(line)), Outcome::Already);
    assert!(dry.0.writes.is_empty());
}

#[test]
fn the_sentinel_writes_shell_back_only_while_shell_mode_is_on_and_no_shell_process_runs() {
    let mut said = Vec::new();
    let mut say = |line| said.push(line);
    // Off: a sentinel left behind does nothing.
    let mut fake = pc();
    assert_eq!(sentinel(&mut fake, &paths(), false, &mut say), Sentinel::Nothing);
    assert!(fake.writes.is_empty());

    let mut fake = pc();
    on(&mut fake);
    fake.writes.clear();
    assert_eq!(sentinel(&mut fake, &paths(), true, &mut say), Sentinel::Nothing);
    assert_eq!(sentinel(&mut fake, &paths(), false, &mut say), Sentinel::StartDashboard);
    assert!(fake.writes.is_empty(), "Shell holds: nothing written");

    // A Windows upgrade dropped Shell.
    fake.values.remove(&shell_place());
    assert_eq!(sentinel(&mut fake, &paths(), false, &mut say), Sentinel::StartDashboard);
    assert_eq!(fake.values.get(&shell_place()), Some(&shell_value()));
}

#[test]
fn a_registry_that_cant_be_read_is_never_guessed_at() {
    let mut fake = pc();
    fake.fail_reads = vec![Place::machine(WINLOGON, "Shell").to_string()];
    assert_eq!(on(&mut fake).0, Outcome::Failed);
    assert!(fake.writes.is_empty());
    assert_eq!(status(&fake, &paths()), ShellMode::Failed);
}
