//! The Game hub's opening (#214): its first frame waits on nothing that is not loaded yet, and the pictures it shows
//! are asked for at the tile's A, while the double tap's time is waited out. The fonts are tools/test-fonts.sh's.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use model::achievement::GameProgress;
use model::Library;
use render::gpu::{self, Gpu};
use render::offscreen::{read_back, wait, FORMAT};
use screens::Dashboard;
use ui::host::Offscreen;
use ui::{Button, Host, Press};

const HD: (u32, u32) = (1920, 1080);

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
static DRAWING: Mutex<()> = Mutex::new(());

fn fonts() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
        let target = from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        let fonts = target.join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
    });
}

/// The fake library, every game given ids, achievements, and a cover and background art at these paths.
fn library(cover: &Path, hero: &Path) -> Library {
    let mut library = screens::fake_library();
    for (i, game) in library.games.iter_mut().enumerate() {
        game.id = i as i64 + 1;
        (game.cover, game.hero) = (Some(cover.to_path_buf()), Some(hero.to_path_buf()));
        library.achievements.games.push(GameProgress { game: game.id, unlocked: 2, of: 6, score: 25 });
    }
    library
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// One frame of both windows, drawn as the window draws it: art that is not decoded is not waited for.
fn frame(gpu: &Gpu, host: &mut Host, target: &Offscreen, dashboard: &mut Dashboard) -> image::RgbaImage {
    let index = host.draw(gpu, &target.view, target.size, 0.0, wgpu::Color::BLACK, |ui| dashboard.draw_all(ui));
    wait(gpu, Some(index));
    image::RgbaImage::from_raw(target.size.0, target.size.1, read_back(gpu, &target.texture, target.size)).unwrap()
}

/// A picture that never comes: a named pipe no one writes to, which a decoding thread waits on for ever.
#[cfg(unix)]
fn never(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_file(&path);
    let made = std::process::Command::new("mkfifo").arg(&path).status().expect("mkfifo");
    assert!(made.success(), "mkfifo {}", path.display());
    path
}

#[cfg(unix)]
#[test]
fn the_hubs_first_frame_waits_on_neither_its_pictures_nor_its_achievements() {
    use std::time::{Duration, Instant};

    fonts();
    let _drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (gpu, size) = (gpu::init(gpu::instance(None), None), HD);
    let mut host = Host::new(&gpu, FORMAT, || {});
    let target = Offscreen::new(&gpu, size);
    let mut dashboard = Dashboard::new(library(&never("never-cover"), &never("never-hero")));

    // A on the Recent row's first tile, a frame of Home meanwhile, and the double tap's time up: the hub is up.
    press(&mut dashboard, &[Button::Down, Button::A]);
    frame(&gpu, &mut host, &target, &mut dashboard);
    dashboard.advance(screens::home::DOUBLE_TAP.as_secs_f32());
    let (_, game) = dashboard.game_hub_up().expect("the hub is up");
    let keys = screens::game_hub::art_keys(game, 1.0);
    assert_eq!(keys.len(), 2, "the hero and the cover: {keys:?}");
    // Its achievements are the library snapshot's: nothing is read for it, as the Achievements page has its list read.
    assert_eq!(dashboard.achievements_wanted(), None);

    // Its first frame, with neither picture there and neither ever to come.
    let began = Instant::now();
    let image = frame(&gpu, &mut host, &target, &mut dashboard);
    assert!(began.elapsed() < Duration::from_secs(20), "the frame took {:?}", began.elapsed());
    assert!(keys.iter().all(|(_, key)| !host.art.is_ready(key)), "a picture came from a pipe no one writes to");
    // The hub's shell is in it: Play focused, green, in its place; the title, white, over the band.
    let play = image.get_pixel(270, 842).0;
    let green: [u8; 3] = [0x2C, 0xA2, 0x43];
    assert!((0..3).all(|c| (play[c] as i32 - green[c] as i32).abs() <= 3), "Play's slab is {play:?}");
    let title = (80..700).flat_map(|x| (130..184).map(move |y| (x, y)));
    let white = title.filter(|&(x, y)| image.get_pixel(x, y).0[..3].iter().all(|&c| c > 200)).count();
    assert!(white > 50, "the title is not drawn: {white} white pixels where it is");
    // In the band under the actions, black where the art will fade in: nothing of it yet.
    assert_eq!(image.get_pixel(960, 950).0[..3], [0, 0, 0], "the band before the art");
}

#[test]
fn a_tiles_a_asks_for_the_hubs_pictures_before_the_hub_opens() {
    fonts();
    let _drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("hub-open");
    std::fs::create_dir_all(&dir).unwrap();
    let (cover, hero) = (dir.join("cover.png"), dir.join("hero.png"));
    image::RgbImage::from_pixel(300, 450, image::Rgb([200, 40, 40])).save(&cover).unwrap();
    image::RgbImage::from_pixel(960, 310, image::Rgb([40, 40, 200])).save(&hero).unwrap();
    let (gpu, size) = (gpu::init(gpu::instance(None), None), HD);
    let mut host = Host::new(&gpu, FORMAT, || {});
    let target = Offscreen::new(&gpu, size);
    let mut dashboard = Dashboard::new(library(&cover, &hero));

    press(&mut dashboard, &[Button::Down]);
    let game = dashboard.library().games[dashboard.focused_game().unwrap()].clone();
    let keys = screens::game_hub::art_keys(&game, 1.0);
    frame(&gpu, &mut host, &target, &mut dashboard);
    assert!(
        keys.iter().all(|(_, key)| host.art.timing(key).is_none()),
        "Home alone asks for none of the hub's pictures"
    );

    // A, and the frame that answers it: the hub's pictures are on their way while the double tap's time runs.
    press(&mut dashboard, &[Button::A]);
    frame(&gpu, &mut host, &target, &mut dashboard);
    assert!(dashboard.game_hub_up().is_none(), "the hub waits out the double tap");
    for (name, key) in &keys {
        assert!(host.art.timing(key).is_some(), "the {name} was not asked for at A");
    }
}
