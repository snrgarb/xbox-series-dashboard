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
