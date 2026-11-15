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
