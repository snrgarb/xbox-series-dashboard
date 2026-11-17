//! Set up this PC (#178) against the stand-in launchers' installers: this crate's program, served by the stand-in
//! release pages as `standin-setup-<key>.exe`, downloaded and checked as the Dashboard downloads a real installer,
//! run with that app's own switches, and followed until the test's registry file (standing in for Windows' list of
//! installed programs) has the app; then its own registered uninstaller run until the list no longer has it. Nothing
//! of the machine's is read or written: the registry is the test's file, every folder the test's own.
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use helpers::clients::{self, Client, Outcome};
use model::Source;
use source_kit::registry::Registry;

/// The test's own folder, under Cargo's `target/tmp` rather than the system's temp folder: each test copies this
/// crate's program (tens of MB) into it, which filled a small `/tmp` and failed the tests now and then (#158).
fn scratch() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn registry(sandbox: &Path) -> Registry {
    Registry::load(&sandbox.join("registry.json")).unwrap_or(Registry::File(Default::default()))
}

fn runs(sandbox: &Path) -> Vec<String> {
    std::fs::read_to_string(sandbox.join("runs.txt")).unwrap_or_default().lines().map(str::to_string).collect()
}

#[cfg(unix)]
fn runnable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn runnable(_: &Path) {}

/// Starts `command` with the stand-in's folder, and follows it until the list says the app is `installed` or not.
fn run(client: &Client, command: &clients::Uninstall, sandbox: &Path, installed: bool) -> Outcome {
    let env = vec![
        ("STANDIN_SETUP".to_string(), OsString::from(sandbox)),
        ("STANDIN_SETUP_MS".to_string(), OsString::from("100")),
    ];
    let mut started = clients::start(command, &env).expect("the stand-in starts");
    assert!(!started.elevated, "nothing here asks Windows for an administrator");
    let mut done = || clients::entry(&registry(sandbox), client).is_some() == installed;
    clients::follow(&mut started, &mut done, Duration::from_secs(60), Duration::from_millis(50), &mut |_| {})
}

#[test]
fn battlenet_is_downloaded_checked_installed_with_its_switches_read_and_removed_by_its_own_uninstaller() {
    let dir = scratch();
    let (data, sandbox) = (dir.path().join("data"), dir.path().join("sandbox"));
    std::fs::create_dir_all(&sandbox).unwrap();
    let me = std::fs::read(env!("CARGO_BIN_EXE_standin")).unwrap();
    let service = helpers::fake::FakeHelpers::start(0, me).unwrap();
    let table = service.client_table(&sandbox);
    let client = table.iter().find(|client| client.key == "battlenet").unwrap().clone();
    assert_eq!(clients::entry(&registry(&sandbox), &client), None);

    // Downloaded and checked, then run with Battle.net's own switches, told to go to the test's folder.
    let installer = clients::download(&helpers::agent(), &data, &client, &mut |_| {}).unwrap();
    runnable(&installer);
    let command = clients::install_command(&client, &installer);
    assert_eq!(command.line, format!("--lang=enUS --installpath=\"{}\"", sandbox.join("battlenet").display()));
    assert_eq!(run(&client, &command, &sandbox, true), Outcome::Done);
    let ran = runs(&sandbox);
    assert_eq!(ran.len(), 1);
    assert!(ran[0].starts_with("install battlenet --lang=enUS --installpath="), "{ran:?}");

    // Windows' list has it, with its uninstaller; Battle.net's source reads its games from where it went.
    let entry = clients::entry(&registry(&sandbox), &client).expect("its Uninstall key");
    assert_eq!(entry.display, "Battle.net");
    let folder = entry.location.clone().unwrap();
    assert_eq!(folder, std::path::absolute(sandbox.join("battlenet")).unwrap());
    let found = source_battlenet::BattleNet::at(Some(folder.clone())).read().unwrap().expect("a Battle.net");
    assert!(found.library.games.iter().any(|game| game.name == "Emberwatch" && game.installed));

    // Removed by the uninstaller its key names, which hands over to a copy of itself and ends at once, as NSIS's do:
    // the key gone, its folder left empty.
    let uninstall = entry.uninstall.unwrap();
    assert!(uninstall.program.starts_with(&folder) && !uninstall.elevate);
    assert_eq!(run(&client, &uninstall, &sandbox, false), Outcome::Done);
    assert_eq!(runs(&sandbox).last().map(String::as_str), Some("uninstall battlenet"));
    assert_eq!(clients::entry(&registry(&sandbox), &client), None);
    let left: Vec<_> = std::fs::read_dir(&folder).unwrap().flatten().map(|item| item.file_name()).collect();
    assert!(left.is_empty(), "{left:?}");
    assert!(source_battlenet::BattleNet::at(Some(folder)).read().unwrap().is_none());
}

#[test]
fn steams_installer_runs_quiet_and_its_uninstaller_is_told_to_be_quiet_too() {
    let dir = scratch();
    let (data, sandbox) = (dir.path().join("data"), dir.path().join("sandbox"));
    std::fs::create_dir_all(&sandbox).unwrap();
    let service = helpers::fake::FakeHelpers::start(0, std::fs::read(env!("CARGO_BIN_EXE_standin")).unwrap()).unwrap();
    let client = service.client_table(&sandbox).into_iter().find(|client| client.key == "steam").unwrap();
    let installer = clients::download(&helpers::agent(), &data, &client, &mut |_| {}).unwrap();
    runnable(&installer);
    assert_eq!(run(&client, &clients::install_command(&client, &installer), &sandbox, true), Outcome::Done);
    let entry = clients::entry(&registry(&sandbox), &client).unwrap();
    // Steam's key is named as Steam's own installer names it.
    assert!(entry.key.ends_with(r"\Steam"), "{}", entry.key);
    let uninstall = entry.uninstall.unwrap();
    assert_eq!(uninstall.line, "/S");
    assert_eq!(run(&client, &uninstall, &sandbox, false), Outcome::Done);
    assert_eq!(runs(&sandbox), ["install steam /S", "uninstall steam /S"]);
}

#[test]
fn an_installer_that_installs_nothing_is_not_taken_for_done_and_without_its_folder_the_stand_in_writes_nothing() {
    let dir = scratch();
    let (data, sandbox) = (dir.path().join("data"), dir.path().join("sandbox"));
    std::fs::create_dir_all(&sandbox).unwrap();
    let service = helpers::fake::FakeHelpers::start(0, std::fs::read(env!("CARGO_BIN_EXE_standin")).unwrap()).unwrap();
    let client = service.client_table(&sandbox).into_iter().find(|client| client.key == "ubisoft").unwrap();
    let installer = clients::download(&helpers::agent(), &data, &client, &mut |_| {}).unwrap();
    runnable(&installer);
    let command = clients::install_command(&client, &installer);
    // No STANDIN_SETUP: it says so, writes nothing, and ends with 3.
    let mut started = clients::start(&command, &[]).unwrap();
    let code = loop {
        if let Some(code) = started.ended() {
            break code;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(code, Some(3));
    assert!(std::fs::read_dir(&sandbox).unwrap().next().is_none());
}
