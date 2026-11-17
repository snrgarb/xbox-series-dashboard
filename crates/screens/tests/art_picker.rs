//! The art picker (docs/spec/art-picker.md): opened from the Tile menu's More options, what it asks the shell for
//! and what A and B do; then each tab drawn offscreen and set against the committed references
//! (`SHOTS=bless cargo test -p screens --test art_picker` accepts what is drawn now).
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::art_picker::{Answer, Ask, Candidate, Choice, Chosen, Failure, Reply, Tab};
use model::Library;
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::{Dashboard, Effect, Options};
use ui::host::Offscreen;
use ui::Button::{self, Down, LeftBumper, Menu, Right, RightBumper, RightTrigger, A, B};
use ui::{Host, Press};

/// The fake library as the library database gives it, the Recent row's games with a cover and a background of their
/// store's own: the Cover and Background tabs offer them.
fn library() -> Library {
    let dir = pictures();
    let mut library = screens::fake_library();
    for (i, game) in library.games.iter_mut().enumerate().take(7) {
        game.cover = Some(dir.join(format!("store-{i}-cover.png")));
        game.hero = Some(dir.join(format!("store-{i}-hero.png")));
    }
    let mut database = library::Database::in_memory().unwrap();
    database.sync("steam", &library, 1_780_000_000).unwrap();
    database.snapshot().unwrap()
}

/// Made-up pictures: the stores' own in bands of grey, and every candidate of each kind, at its kind's shape, in a
/// colour of its own with bands across, so a crop or a stretch shows.
fn pictures() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("art-picker");
        std::fs::create_dir_all(&dir).unwrap();
        let banded = |w: u32, h: u32, rgb: [u8; 3]| {
            image::RgbImage::from_fn(w, h, |_, y| {
                let dim = if (y * 8 / h) % 2 == 0 { 1 } else { 2 };
                image::Rgb(rgb.map(|c| c / dim))
            })
        };
        for game in 0..7u32 {
            let grey = (60 + game * 20) as u8;
            shots::save_shared(&banded(300, 450, [grey; 3]), &dir.join(format!("store-{game}-cover.png")));
            shots::save_shared(&banded(960, 310, [grey; 3]), &dir.join(format!("store-{game}-hero.png")));
        }
        for tab in Tab::ALL {
            for n in 0..40u32 {
                let hue = |shift: u32| (((n * 37 + shift * 85 + tab as u32 * 29) % 200) + 40) as u8;
                let (w, h) = size(tab);
                shots::save_shared(&banded(w, h, [hue(0), hue(1), hue(2)]), &thumb(tab, n as u64));
            }
        }
        dir
    })
}

fn size(tab: Tab) -> (u32, u32) {
    match tab {
        Tab::Square => (256, 256),
        Tab::Cover => (200, 300),
        Tab::Background => (480, 155),
    }
}

fn thumb(tab: Tab, n: u64) -> PathBuf {
    pictures_dir().join(format!("{}-{n}.png", tab.key()))
}

fn pictures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("art-picker")
}

/// A candidate's id: the tab's hundreds and its place.
fn id(tab: Tab, n: u64) -> u64 {
    (tab as u64 + 1) * 1000 + n
}

/// SteamGridDB's list of each kind, `total` long, served 50 a page.
#[derive(Clone, Copy)]
struct Service {
    total: [u64; 3],
}

impl Service {
    /// Answers everything the picker asks for, until it asks nothing more; gives the pages asked.
    fn serve(self, dashboard: &mut Dashboard) -> Vec<(Tab, u32)> {
        let mut pages = Vec::new();
        loop {
            let asks = dashboard.art_asks();
            if asks.is_empty() {
                return pages;
            }
            for ask in asks {
                dashboard.art_answer(match ask {
                    Ask::Page { game, tab, page, .. } => {
                        pages.push((tab, page));
                        let total = self.total[tab.index()];
                        let start = page as u64 * 50;
                        let items = (start..(start + 50).min(total))
                            .map(|n| Candidate {
                                id: id(tab, n),
                                url: format!("http://127.0.0.1:1/pictures/{n}.png"),
                                thumb: format!("{tab:?}/{n}"),
                                width: size(tab).0,
                                height: size(tab).1,
                            })
                            .collect();
                        let reply = Reply { page, total: total as usize, limit: 50, items };
                        Answer::Page { game, tab, page, result: Ok(reply) }
                    }
                    Ask::Thumb { id: picture, url } => {
                        let (tab, n) = url.split_once('/').unwrap();
                        let tab = Tab::ALL.into_iter().find(|t| format!("{t:?}") == tab).unwrap();
                        Answer::Thumb { id: picture, path: Some(thumb(tab, n.parse().unwrap())) }
                    }
                });
            }
        }
    }
}

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// Home, with a SteamGridDB key, the Recent row's fifth game's picker opened from its Tile menu.
fn picker(key: bool) -> Dashboard {
    let mut dashboard = Dashboard::new(library());
    dashboard.set_options(Options { art_key: key, art_online: true, ..dashboard.options() });
    press(&mut dashboard, &[Down, Right, Right, Right, Right, Menu]);
    for _ in 0..8 {
        if dashboard.focus_line().contains("(More options)") {
            break;
        }
        press(&mut dashboard, &[Down]);
    }
    press(&mut dashboard, &[A, Down, Down]);
    assert_eq!(dashboard.focus_line(), "TileMenu: MoreOptions(Change art) of Hades II");
    press(&mut dashboard, &[A]);
    dashboard.take_effects();
    dashboard
}

// ---------------------------------------------------------------------------------------------------- behaviour

#[test]
fn change_art_opens_the_picker_in_the_menus_place_and_b_goes_back_to_home() {
    let mut dashboard = picker(true);
    assert_eq!(dashboard.page(), Some("ArtPicker"));
    assert_eq!(dashboard.pages_up(), 1, "in the menu's place");
    assert!(dashboard.art_picker_up());
    assert_eq!(dashboard.focus_line(), "ArtPicker: Square: Page 1: Automatic; in use: Automatic");
    let asked = Service { total: [23, 0, 3] }.serve(&mut dashboard);
    assert_eq!(asked, [(Tab::Square, 0)], "only the tab shown");
    assert_eq!(dashboard.focus_line(), "ArtPicker: Square: Page 1 of 3: Automatic; in use: Automatic");
    press(&mut dashboard, &[B]);
    assert_eq!(dashboard.page(), None);
    assert_eq!(dashboard.focus_line(), "Recent(4)");
    assert!(!dashboard.take_effects().iter().any(|effect| matches!(effect, Effect::ChooseArt(_))), "left as it was");
}

#[test]
fn a_on_a_candidate_of_a_later_page_chooses_it() {
    let mut dashboard = picker(true);
    let service = Service { total: [23, 30, 3] };
    service.serve(&mut dashboard);
    press(&mut dashboard, &[RightBumper]);
    assert_eq!(service.serve(&mut dashboard), [(Tab::Cover, 0)]);
    press(&mut dashboard, &[RightTrigger, RightTrigger, Right]);
    assert_eq!(dashboard.focus_line(), "ArtPicker: Cover: Page 3 of 3: Picture 2027; in use: Automatic");
    press(&mut dashboard, &[A]);
    let effects = dashboard.take_effects();
    let game = dashboard.library().games[4].key();
    let chosen = Choice {
        game,
        tab: Tab::Cover,
        chosen: Some(Chosen::Picture(2027)),
        url: Some("http://127.0.0.1:1/pictures/27.png".into()),
    };
    assert!(effects.contains(&Effect::ChooseArt(chosen)), "{effects:?}");
    assert_eq!(dashboard.page(), None);
}

#[test]
fn without_a_key_it_asks_nothing_and_says_where_to_set_one() {
    let mut dashboard = picker(false);
    assert!(dashboard.art_asks().is_empty());
    assert_eq!(dashboard.focus_line(), "ArtPicker: Square: Page 1 of 1: Automatic; in use: Automatic; no key");
}

// ---------------------------------------------------------------------------------------------------- pictures

/// One test at a time draws: lavapipe now and then crashes with many devices drawing at once.
static DRAWING: Mutex<()> = Mutex::new(());

fn target_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
}

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn picker(key: bool) -> Self {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, (1920, 1080));
        let mut dashboard = picker(key);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }
}

/// Square: ten cells a page at 284 x 284, Automatic first and in use; focus on a candidate.
#[test]
fn shot_the_square_tab() {
    let mut stage = Stage::picker(true);
    Service { total: [23, 30, 7] }.serve(&mut stage.dashboard);
    press(&mut stage.dashboard, &[Right, Right]);
    stage.check("art-picker-square");
}

/// Cover: fourteen cells a page at 2:3, the store's own second; its second page, focus on the second row.
#[test]
fn shot_the_cover_tab_on_its_second_page() {
    let mut stage = Stage::picker(true);
    let service = Service { total: [23, 30, 7] };
    service.serve(&mut stage.dashboard);
    press(&mut stage.dashboard, &[RightBumper]);
    service.serve(&mut stage.dashboard);
    press(&mut stage.dashboard, &[RightTrigger, Down]);
    service.serve(&mut stage.dashboard);
    stage.check("art-picker-cover");
}

/// Background: twelve cells a page at the hero's 1920:620; seven candidates, all on the first page.
#[test]
fn shot_the_background_tab() {
    let mut stage = Stage::picker(true);
    let service = Service { total: [23, 30, 7] };
    service.serve(&mut stage.dashboard);
    press(&mut stage.dashboard, &[RightBumper, RightBumper]);
    service.serve(&mut stage.dashboard);
    stage.check("art-picker-background");
}

/// A page that failed: said plainly, with Retry focused.
#[test]
fn shot_a_failed_page_with_retry() {
    let mut stage = Stage::picker(true);
    let asks = stage.dashboard.art_asks();
    let Some(Ask::Page { game, tab, page, .. }) = asks.into_iter().next() else { panic!("a page") };
    let why = Failure::Failed("SteamGridDB could not be reached".into());
    stage.dashboard.art_answer(Answer::Page { game, tab, page, result: Err(why) });
    press(&mut stage.dashboard, &[Down]);
    assert!(stage.dashboard.focus_line().contains(": Retry;"), "{}", stage.dashboard.focus_line());
    stage.check("art-picker-failed");
}

/// No key: the fixed entries alone, and where to set one.
#[test]
fn shot_without_a_key() {
    let mut stage = Stage::picker(false);
    press(&mut stage.dashboard, &[RightBumper, LeftBumper]);
    stage.check("art-picker-no-key");
}
