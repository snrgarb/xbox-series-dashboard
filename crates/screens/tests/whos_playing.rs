//! Who's playing today? and Signing you in, as pictures (docs/spec/start-up.md, sections 5 and 6): one Profile, two,
//! and many running off the screen's edges; Signing you in with the Spinner part way round. Compared with the
//! committed references (crates/shots): `SHOTS=bless cargo test -p screens --test whos_playing` accepts what is
//! drawn now. Look at each beside the Bundle's picture of Who's playing today?
//! (`port-reference-20261115-200355/whos-playing.jpg` in the Bundle's artifacts) before blessing it.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{Library, Picture, Profile};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::Dashboard;
use ui::host::Offscreen;
use ui::Button::{self, Right, A};
use ui::{Host, Press};

/// A picture file to wear, drawn once in bands of its own colours so a crop shows.
fn moth() -> PathBuf {
    static PICTURE: OnceLock<PathBuf> = OnceLock::new();
    PICTURE
        .get_or_init(|| {
            let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("whos-playing-art");
            std::fs::create_dir_all(&dir).unwrap();
            let (a, b) = ([0x30, 0x90, 0xD0], [0x10, 0x30, 0x60]);
            let image = image::RgbImage::from_fn(400, 400, |_, y| image::Rgb(if (y / 50) % 2 == 0 { a } else { b }));
            let path = dir.join("Moth.png");
            shots::save_shared(&image, &path);
            path
        })
        .clone()
}

/// The fake library with these Profiles, ids 1 up, wearing the plain pictures in turn and the second the picture
/// file; the one at `last` signed in last.
fn library(names: &[&str], last: usize) -> Library {
    let mut library = screens::fake_library();
    let profile = |(i, name): (usize, &&str)| Profile {
        id: i as i64 + 1,
        gamertag: name.to_string(),
        picture: if i == 1 { Picture::File(moth()) } else { Picture::Shape(i as u8 % model::profile::SHAPES) },
        ..Profile::default()
    };
    library.profiles.list = names.iter().enumerate().map(profile).collect();
    library.profiles.signed_in = Some(last as i64 + 1);
    library
}

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

/// The test fonts, before anything loads a font.
fn fonts() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
    });
}

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once on as many threads.
static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    size: (u32, u32),
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    /// Started with the video Off on these Profiles: Who's playing today?.
    fn asking(names: &[&str], last: usize) -> Self {
        Self::at(names, last, HD)
    }

    fn at(names: &[&str], last: usize, size: (u32, u32)) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, size);
        let mut dashboard = Dashboard::new(library(names, last));
        dashboard.set_clock(Some("9:41 AM".into()));
        dashboard.start_up(false);
        Self { gpu, host, target, dashboard, size, _drawing: drawing }
    }

    fn press(&mut self, buttons: &[Button]) {
        for &button in buttons {
            self.dashboard.press(Press { button, repeat: false });
        }
    }

    /// The frame at rest, `seconds` into the ui's clock, compared with its reference.
    fn check_at(&mut self, name: &str, seconds: f64) {
        self.host.store.set_time(seconds);
        let Self { gpu, host, target, dashboard, size, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let size = *size;
        let image = image::RgbaImage::from_raw(size.0, size.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }

    fn check(&mut self, name: &str) {
        self.check_at(name, 1.0);
    }
}

/// One Profile, focused in its ring at the centre: the Bundle's picture (the Windows user's, wearing Windows' own
/// look for an account with no picture).
#[test]
#[ignore = "screenshot"]
fn whos_playing_one_profile() {
    let mut stage = Stage::asking(&["win11"], 0);
    assert_eq!(stage.dashboard.focus_line(), "Startup: WhosPlaying \"win11\"");
    stage.check("whos-playing-one");
}

/// Two Profiles, focus on the first, signed in last; then on the second, wearing a picture file, the row slid left.
#[test]
#[ignore = "screenshot"]
fn whos_playing_two_profiles() {
    let mut stage = Stage::asking(&["win11", "Moth Runner"], 0);
    stage.check("whos-playing-two");
    stage.press(&[Right]);
    assert_eq!(stage.dashboard.focus_line(), "Startup: WhosPlaying \"Moth Runner\"");
    stage.check("whos-playing-two-second");
}

/// Six Profiles, focus on the third: the row runs off both edges of the screen. A long gamertag ends in an
/// ellipsis.
#[test]
#[ignore = "screenshot"]
fn whos_playing_many_profiles() {
    let names =
        ["Alpha", "Moth Runner", "Gamma", "A gamertag that is far too long to fit its place", "Epsilon", "Zeta"];
    let mut stage = Stage::asking(&names, 2);
    stage.check("whos-playing-many");
    stage.press(&[Right]);
    stage.check("whos-playing-many-long-name");
}

/// Signing you in: the picture alone, the Spinner part way round it, the gamertag and "Signing you in" under it.
#[test]
#[ignore = "screenshot"]
fn signing_you_in() {
    let mut stage = Stage::asking(&["win11", "Moth Runner"], 1);
    stage.press(&[A]);
    assert_eq!(stage.dashboard.focus_line(), "Startup: SigningIn \"Moth Runner\"");
    stage.check_at("signing-in", 0.45);
}

/// Who's playing today? on an ultrawide screen: the canvas centred across, the swirl filling it.
#[test]
#[ignore = "screenshot"]
fn whos_playing_ultrawide() {
    let mut stage = Stage::at(&["win11", "Moth Runner", "Gamma"], 1, (2560, 1080));
    stage.check("whos-playing-ultrawide");
}
