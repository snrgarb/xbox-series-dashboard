//! The stores' installers that drive a helper program (#175, #176), pointed at this crate's program acting as the
//! helper: copied as `legendary(.exe)` or `gogdl(.exe)`, it prints that helper's own lines and writes its own files
//! in the test's folder. The installer runs it as it would run the real one, in a process of its own, and the Queue's view
//! of it (`Source::installs`) and the library read back (`Source::read`) are what is judged.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use model::{Game, Install, InstallState, Opener, Source, Start, Steer};

/// Opens nothing: a helper's route needs no opener.
struct Nothing;

impl Opener for Nothing {
    fn open(&mut self, start: &Start) -> Result<(), String> {
        Err(format!("nothing is opened here: {start}"))
    }
}

/// This crate's program, copied into `dir` under `name`, as the Dashboard keeps a helper.
fn helper(dir: &Path, name: &str) -> PathBuf {
    let program = dir.join("helpers").join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(program.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_standin"), &program).unwrap();
    program
}

fn soon(what: impl Fn() -> bool) -> bool {
    for _ in 0..1500 {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn state(installs: Option<Vec<Install>>) -> Option<InstallState> {
    installs.and_then(|installs| installs.first().map(|install| install.state))
}

#[test]
fn epic_through_the_stand_in_legendary_installs_pauses_resumes_cancels_uninstalls_and_starts() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let program = helper(dir, "legendary");
    let home = dir.join("legendary");
    let env = vec![
        ("LEGENDARY_CONFIG_PATH".to_string(), OsString::from(&home)),
        ("STANDIN_HELPER_STEP_MS".to_string(), OsString::from("150")),
    ];
    let games = dir.join("games");
    let route = source_kit::HelperRoute::fixed(program.clone(), home.clone(), env.clone(), games.clone());
    // The old launcher's Manifests, which legendary's egl-sync writes the game into.
    let epic_root = dir.join("epic");
    let manifests = source_epic::Places::under(&epic_root).manifests();
    std::fs::create_dir_all(&manifests).unwrap();
    let epic = source_epic::Epic::at(Some(epic_root.clone())).with_legendary(route);
    let installer = epic.installer().unwrap();
    let app = source_epic::legendary::standin::APP_NAME;
    let game = Game { source: "epic".into(), source_id: app.into(), name: "Zinnia Gate".into(), ..Game::default() };

    // Not signed in: nothing of legendary's is asked, and the launcher is.
    assert_eq!(installer.install(&game, None, &mut Nothing), Err(model::Refusal::Unsupported));
    assert!(!installer.refresh());

    // Signed in with legendary's own `auth`, as the user would; then the library listed.
    let args = ["auth".to_string(), "--code".into(), "made-up".into()];
    let (code, _) =
        source_kit::downloads::run_to_end("test", "auth", &program, &args, &env, Duration::from_secs(30)).unwrap();
    assert_eq!(code, Some(0));
    assert!(installer.refresh());
    let library = epic.read().unwrap().unwrap().library;
    assert!(library.games.iter().any(|g| g.source_id == app && !g.installed), "{:?}", library.games);

    // Install, then pause partway, resume, and cancel: its folder and resume file gone.
    let folder = installer.folders().into_iter().next().unwrap();
    installer.install(&game, Some(&folder), &mut Nothing).unwrap();
    let downloading = || matches!(state(epic.installs()), Some(InstallState::Downloading { done, total }) if done > 0 && done < total);
    assert!(soon(downloading), "{:?}", epic.installs());
    let speed = epic.installs().unwrap()[0].speed;
    assert!(speed.is_some_and(|bits| bits > 0), "legendary's own speed: {speed:?}");
    installer.steer(&game, Steer::Pause, &mut Nothing).unwrap();
    assert!(soon(|| matches!(state(epic.installs()), Some(InstallState::Paused { .. }))));
    let Some(InstallState::Paused { done: paused, total }) = state(epic.installs()) else { unreachable!() };
    assert!(paused > 0 && total == 8 * 6 * 1_048_576, "{paused} of {total}");
    assert!(home.join("tmp").join(format!("{app}.resume")).is_file());
    installer.steer(&game, Steer::Resume, &mut Nothing).unwrap();
    let on = || matches!(state(epic.installs()), Some(InstallState::Downloading { done, .. }) if done > paused);
    assert!(soon(on), "resumed from {paused}: {:?}", epic.installs());
    installer.steer(&game, Steer::Cancel, &mut Nothing).unwrap();
    assert!(soon(|| epic.installs() == Some(Vec::new())));
    assert!(!games.join("Zinnia Gate").exists() && !home.join("tmp").join(format!("{app}.resume")).exists());

    // Install to the end: installed, in the old launcher's records too, and started through legendary.
    installer.install(&game, Some(&folder), &mut Nothing).unwrap();
    assert!(soon(|| epic.installs() == Some(Vec::new())), "{:?}", epic.installs());
    let library = epic.read().unwrap().unwrap().library;
    let installed = library.games.iter().find(|g| g.source_id == app).unwrap();
    assert!(installed.installed && installed.install_dir.as_deref() == Some(games.join("Zinnia Gate").as_path()));
    let item = manifests.join(format!("{}.item", source_epic::legendary::standin::EGL_GUID));
    assert!(item.is_file(), "egl-sync wrote the launcher's record");
    assert!(matches!(epic.start(installed), Some(Start::Program { args, .. }) if args == ["launch", app]));

    // Uninstall: legendary deletes it and its record, and the Dashboard the old launcher's.
    installer.uninstall(installed, &mut Nothing).unwrap();
    assert!(soon(|| !games.join("Zinnia Gate").exists()));
    assert!(soon(|| !item.exists()), "the old launcher's record is left");
    assert!(soon(|| epic.read().unwrap().unwrap().library.games.iter().any(|g| g.source_id == app && !g.installed)));
}

#[test]
fn gog_through_the_stand_in_gogdl_downloads_pauses_resumes_cancels_sets_up_and_uninstalls() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path();
    let program = helper(dir, "gogdl");
    let home = dir.join("gogdl");
    let env = vec![("STANDIN_HELPER_STEP_MS".to_string(), OsString::from("150"))];
    let games = dir.join("games");
    let route = source_kit::HelperRoute::fixed(program.clone(), home.clone(), env.clone(), games.clone());
    // A fake GOG with Galaxy's database: Pewter Isles owned, not installed.
    let root = dir.join("gog");
    source_gog::fixture::write(&root, &source_gog::fixture::Fixture::standard()).unwrap();
    let gog = source_gog::Gog::at(Some(root)).with_gogdl(route);
    let installer = gog.installer().unwrap();
    let library = gog.read().unwrap().unwrap().library;
    let game = library.games.iter().find(|g| g.name == "Pewter Isles" && !g.installed).unwrap().clone();
    let id = game.source_id.clone();

    // Not signed in: Galaxy is asked, not gogdl.
    assert_eq!(installer.install(&game, None, &mut Nothing), Err(model::Refusal::Unsupported));
    assert_eq!(gog.installs(), None);

    // Signed in with gogdl's own `auth --code`, as the user would.
    let auth = home.join("auth.json").display().to_string();
    let args = ["--auth-config-path".to_string(), auth, "auth".into(), "--code".into(), "made-up".into()];
    let with_config = [env.clone(), vec![("GOGDL_CONFIG_PATH".to_string(), OsString::from(&home))]].concat();
    let ran = source_kit::downloads::run_to_end("test", "auth", &program, &args, &with_config, Duration::from_secs(30));
    assert_eq!(ran.unwrap().0, Some(0));
    let can = installer.can();
    assert!(can.pause && can.cancel && can.quiet_uninstall && !can.choose_folder);

    // Download, pause, resume, cancel: what it made gone, what was there before kept.
    std::fs::create_dir_all(games.join("Someone else's")).unwrap();
    let folder = installer.folders().into_iter().next().unwrap();
    installer.install(&game, Some(&folder), &mut Nothing).unwrap();
    let downloading =
        || matches!(state(gog.installs()), Some(InstallState::Downloading { done, total }) if done > 0 && done < total);
    assert!(soon(downloading), "{:?}", gog.installs());
    installer.steer(&game, Steer::Pause, &mut Nothing).unwrap();
    assert!(soon(|| matches!(state(gog.installs()), Some(InstallState::Paused { .. }))));
    let Some(InstallState::Paused { done: paused, total }) = state(gog.installs()) else { unreachable!() };
    assert!(paused > 0 && total == 8 * 6 * 1_048_576, "{paused} of {total}");
    assert!(games.join("Pewter Isles").join(".gogdl-resume").is_file());
    installer.steer(&game, Steer::Resume, &mut Nothing).unwrap();
    let on = || matches!(state(gog.installs()), Some(InstallState::Downloading { done, .. }) if done > paused);
    assert!(soon(on), "resumed from {paused}: {:?}", gog.installs());
    installer.steer(&game, Steer::Cancel, &mut Nothing).unwrap();
    assert!(soon(|| gog.installs() == Some(Vec::new())));
    assert!(!games.join("Pewter Isles").exists() && games.join("Someone else's").is_dir());

    // To the end: set up by the script interpreter, read installed, started as its own program.
    installer.install(&game, Some(&folder), &mut Nothing).unwrap();
    assert!(soon(|| gog.installs() == Some(Vec::new())), "{:?}", gog.installs());
    let game_dir = games.join("Pewter Isles");
    let set_up = std::fs::read_to_string(game_dir.join(source_gog::gogdl::standin::SET_UP_FILE)).unwrap();
    for arg in ["/VERYSILENT", "/galaxyclient", "/nodesktopshortcut", &format!("/ProductId={id}")] {
        assert!(set_up.lines().any(|line| line == arg), "{arg} in {set_up}");
    }
    assert!(set_up.contains(&format!("/buildId={}", source_gog::gogdl::standin::BUILD_ID)), "{set_up}");
    let library = gog.read().unwrap().unwrap().library;
    let installed = library.games.iter().find(|g| g.source_id == id).unwrap();
    assert!(installed.installed && installed.install_dir.as_deref() == Some(game_dir.as_path()));
    assert!(matches!(gog.start(installed), Some(Start::Program { path, .. }) if path == game_dir.join("game.exe")));

    // Uninstall: its uninstaller, then the rest, its manifest and its record.
    installer.uninstall(installed, &mut Nothing).unwrap();
    assert!(soon(|| !game_dir.exists()));
    assert!(soon(|| gog.read().unwrap().unwrap().library.games.iter().any(|g| g.source_id == id && !g.installed)));
    assert!(!home.join("heroic_gogdl").join("manifests").join(&id).exists());
}
