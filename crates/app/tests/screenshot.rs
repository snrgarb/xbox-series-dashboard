//! `dashboard --screenshot` writes a PNG of the size asked for. What the screens look like is the screens crate's
//! tests' business.
use std::path::PathBuf;
use std::process::Command;

#[test]
fn screenshot_mode_writes_the_frame() {
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("frame.png");
    let _ = std::fs::remove_file(&out);
    let status = Command::new(env!("CARGO_BIN_EXE_dashboard"))
        .arg("--screenshot")
        .arg(&out)
        .args(["--size", "640x360", "--focus", "2", "--guide", "--clock", "9:41 AM"])
        .status()
        .expect("run dashboard");
    assert!(status.success());
    let image = image::open(&out).expect("the screenshot").into_rgba8();
    assert_eq!(image.dimensions(), (640, 360));
    // The Guide's panel is on the left, the dimmed Home on the right: not one flat colour.
    assert_ne!(image.get_pixel(20, 300), image.get_pixel(500, 300));
}

/// What `dashboard --screenshot` logs, drawing the Steam install at `steam` with its database in `data`.
fn log_of_a_run(steam: &std::path::Path, data: &std::path::Path) -> String {
    let out = data.join("frame.png");
    let output = Command::new(env!("CARGO_BIN_EXE_dashboard"))
        .arg("--screenshot")
        .arg(&out)
        .args(["--size", "640x360", "--clock", "9:41 AM"])
        // No test asks a store's real services: this address answers nothing.
        .args(["--store-art-service", "http://127.0.0.1:9"])
        .arg("--steam")
        .arg(steam)
        .arg("--data-dir")
        .arg(data)
        .output()
        .expect("run dashboard");
    let log = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{log}");
    assert!(out.exists(), "{log}");
    log
}

#[test]
fn the_library_database_is_filled_from_steam_and_read_on_the_next_run() {
    use source_steam::fixture::{self, Fixture};
    let dir = tempfile::tempdir().unwrap();
    let (steam, data) = (dir.path().join("steam"), dir.path().join("data"));
    let mut spec = Fixture::standard();
    spec.games.iter_mut().for_each(|g| g.art = fixture::Art::None);
    fixture::write(&steam, &spec).unwrap();
    let has = |log: &str, start: &str| log.lines().any(|line| line.starts_with(start));

    // A first run: no database yet, so Steam is read and the database made.
    let first = log_of_a_run(&steam, &data);
    assert!(has(&first, "steam: 14 games "), "{first}");
    assert!(has(&first, "library: 14 added, 0 updated, 0 gone, "), "{first}");
    assert!(!first.contains("from the database"), "{first}");
    assert!(data.join("library.db").is_file());

    // The next: the games are in the database, and Steam has nothing new.
    let second = log_of_a_run(&steam, &data);
    assert!(has(&second, "library: 14 games from the database in "), "{second}");
    assert!(has(&second, "steam: 14 games "), "{second}");
    assert!(has(&second, "library: 0 added, 0 updated, 0 gone, "), "{second}");
}

/// Art for games their store gives none for (docs/sources.md, "Art"): a fake Steam with no art, a key in the data
/// folder's settings, and a stand-in SteamGridDB on 127.0.0.1. The picture has the stand-in's covers; a second run
/// has them from our own cache and asks the stand-in nothing; and the key is in neither run's log.
#[test]
fn art_a_store_gave_none_for_comes_from_the_service_once_and_then_from_the_cache() {
    use library::art::fake::{FakeService, Painter, Script, StoreScript};
    use source_steam::fixture::{self, Fixture};
    const KEY: &str = "a-test-key-never-logged";
    let dir = tempfile::tempdir().unwrap();
    let (steam, data) = (dir.path().join("steam"), dir.path().join("data"));
    let mut spec = Fixture::standard();
    spec.games.iter_mut().for_each(|g| g.art = fixture::Art::None);
    fixture::write(&steam, &spec).unwrap();
    std::fs::create_dir_all(&data).unwrap();
    let database = library::Database::open(&data.join(library::FILE)).unwrap();
    database.set_setting(library::art::KEY_SETTING, KEY).unwrap();
    drop(database);
    let orange: Painter = std::sync::Arc::new(|_, _| {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(60, 90, image::Rgb([0xF0, 0x80, 0x10]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        png.into_inner()
    });
    // Steam's own art online, a stand-in that knows none of these games: SteamGridDB has them all.
    let stores = StoreScript::new(std::sync::Arc::new(|_, _, _| Vec::new()));
    let script = Script { any_name: true, stores: Some(stores), ..Script::new(KEY, orange) };
    let service = FakeService::start(0, script).unwrap();

    let run = || {
        let out = data.join("frame.png");
        let output = Command::new(env!("CARGO_BIN_EXE_dashboard"))
            .arg("--screenshot")
            .arg(&out)
            .args(["--size", "640x360", "--clock", "9:41 AM", "--art-service", &service.url()])
            .args(["--store-art-service", &service.stores_url()])
            .arg("--steam")
            .arg(&steam)
            .arg("--data-dir")
            .arg(&data)
            .output()
            .expect("run dashboard");
        let log = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(output.status.success(), "{log}");
        assert!(!log.contains(KEY), "the key is in the log: {log}");
        let image = image::open(&out).expect("the screenshot").into_rgba8();
        let orange =
            image.pixels().filter(|p| p.0[0] > 0xD0 && (0x60..0xA0).contains(&p.0[1]) && p.0[2] < 0x40).count();
        (log, orange)
    };
    let (first, orange) = run();
    assert!(first.lines().any(|l| l.starts_with("art: 14 looked up, 14 found, 0 none, in ")), "{first}");
    // Steam has no square online, and knew none of these: every picture is SteamGridDB's, four a game.
    let counts = "; pictures: 0 store files, 0 cache, 0 store online, 56 SteamGridDB";
    assert!(first.lines().any(|l| l.starts_with("art: ") && l.ends_with(counts)), "{first}");
    assert!(orange > 2000, "the covers are the stand-in's: {orange} orange pixels\n{first}");
    let asked = service.requests().len();

    let (second, orange) = run();
    assert_eq!(service.requests().len(), asked, "a second run asks nothing: {second}");
    assert!(orange > 2000, "and has the covers from the cache: {orange} orange pixels\n{second}");
}
