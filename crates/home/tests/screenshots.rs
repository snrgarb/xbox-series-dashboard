//! Home drawn offscreen from the built-in fake library and compared with the committed reference images.
use std::path::{Path, PathBuf};
use std::process::Command;

fn shot(name: &str, args: &[&str]) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let out = dir.join(format!("{name}.png"));
    let status = Command::new(env!("CARGO_BIN_EXE_home"))
        .arg("--screenshot")
        .arg(&out)
        .args(["--clock", "9:41 AM"])
        .args(args)
        .status()
        .expect("run home");
    assert!(status.success(), "home --screenshot failed for {name}");
    shots::check(name, &out, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
}

#[test]
fn home() {
    shot("home", &[]);
}

#[test]
fn home_focus_moved() {
    shot("home-focus-3", &["--focus", "3"]);
}

#[test]
fn home_720p() {
    shot("home-720p", &["--size", "1280x720"]);
}

#[test]
fn guide() {
    shot("guide", &["--guide"]);
}

#[test]
fn launch_screen() {
    shot("launch-screen", &["--launch-screen"]);
}
