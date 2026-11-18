//! Profiles (docs/spec/profiles.md): Settings > Account > Profiles, a Profile's page and its Picture page,
//! each change carried out on a library database as the executable carries it out, and every place the Profile
//! signed in shows (Home's top bar, the Guide, the greeting) following the next snapshot at once. The pictures are
//! compared with the committed references (crates/shots): `SHOTS=bless cargo test -p screens --test profiles`
//! accepts what is drawn now.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::{Picture, ProfileChange};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::guide::Tab;
use screens::keyboard::Typed;
use screens::pages::Open;
use screens::settings::Entry;
use screens::{Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Left, Menu, Right, Up, A, B};
use ui::{Host, Press};

/// When the tests' library came into its database.
const ADDED: u64 = 1_780_000_000;

/// Two pictures to wear, drawn once: the Windows account picture and one in the Gamerpics folder, each in bands of
/// its own colours so a crop shows.
fn pictures() -> &'static [PathBuf] {
    static PICTURES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    PICTURES.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("profiles-art");
        std::fs::create_dir_all(&dir).unwrap();
        let banded = |name: &str, a: [u8; 3], b: [u8; 3]| {
            let image = image::RgbImage::from_fn(192, 192, |_, y| image::Rgb(if (y / 24) % 2 == 0 { a } else { b }));
            let path = dir.join(name);
            shots::save_shared(&image, &path);
            path
        };
        vec![
            banded("user-192.png", [0xE0, 0x80, 0x20], [0x60, 0x30, 0x10]),
            banded("Moth.png", [0x30, 0x90, 0xD0], [0x10, 0x30, 0x60]),
        ]
    })
}

/// The Dashboard over a library database with its first Profile made, carrying out what it asks of the Profiles
/// as the executable does: the change, then the snapshot after, with the pictures to wear.
struct Shell {
    database: library::Database,
    dashboard: Dashboard,
    /// What the last presses asked of the shell.
    effects: Vec<Effect>,
    /// The picture files the snapshots offer.
    offered: Vec<PathBuf>,
}

impl Shell {
    fn new() -> Self {
        Self::offering(pictures().to_vec())
    }

    fn offering(offered: Vec<PathBuf>) -> Self {
        let mut database = library::Database::in_memory().unwrap();
        database.sync("steam", &screens::fake_library(), ADDED).unwrap();
        database.first_profile("Tester", offered.first().map(PathBuf::as_path)).unwrap();
        let mut shell = Self { database, dashboard: Dashboard::new(Default::default()), effects: Vec::new(), offered };
        let snapshot = shell.snapshot();
        shell.dashboard = Dashboard::new(snapshot);
        shell
    }

    fn snapshot(&self) -> model::Library {
        let mut library = self.database.snapshot().unwrap();
        library.profiles.pictures = self.offered.clone();
        library
    }

    /// Settings > Account > Profiles, focus on its first box.
    fn profiles() -> Self {
        let mut shell = Self::new();
        shell.dashboard.open_page(Open::Settings(Entry::Rail));
        shell.press(&[Down, Right, A]);
        assert_eq!(shell.focus(), "Settings: Profiles: Tester");
        shell
    }

    fn press(&mut self, buttons: &[Button]) {
        self.effects.clear();
        for &button in buttons {
            self.dashboard.press(Press { button, repeat: false });
            self.carry_out();
        }
    }

    /// Types `text` on text entry, which is up, over what it started from, and keeps it.
    fn type_over(&mut self, text: &str) {
        assert!(self.focus().starts_with("TextEntry: "), "{}", self.focus());
        for _ in 0..40 {
            self.dashboard.typed(Typed::Backspace);
        }
        self.dashboard.typed(Typed::Text(text));
        self.press(&[Menu]);
    }

    fn carry_out(&mut self) {
        for effect in self.dashboard.take_effects() {
            if let Effect::Profile(change) = &effect {
                if self.database.change_profile(change).is_ok() {
                    let snapshot = self.snapshot();
                    self.dashboard.set_library(snapshot);
                }
            }
            self.effects.push(effect);
        }
    }

    fn focus(&self) -> String {
        self.dashboard.focus_line()
    }

    fn changes(&self) -> Vec<ProfileChange> {
        let change = |effect: &Effect| match effect {
            Effect::Profile(change) => Some(change.clone()),
            _ => None,
        };
        self.effects.iter().filter_map(change).collect()
    }

    fn id(&self, gamertag: &str) -> i64 {
        self.dashboard.library().profiles.list.iter().find(|p| p.gamertag == gamertag).expect(gamertag).id
    }

    /// Add profile, then the gamertag typed and kept: focus on the new Profile's box.
    fn add(&mut self, gamertag: &str) {
        // Add profile is last: down and along to it.
        for _ in 0..20 {
            if self.focus() == "Settings: Profiles: Add profile" {
                break;
            }
            self.press(&[Down, Right]);
        }
        assert_eq!(self.focus(), "Settings: Profiles: Add profile");
        self.press(&[A]);
        self.type_over(gamertag);
        assert_eq!(self.changes(), [ProfileChange::Add { gamertag: gamertag.into() }]);
        assert_eq!(self.focus(), format!("Settings: Profiles: {gamertag}"));
    }
}

#[test]
fn profiles_is_accounts_first_box_and_lists_every_profile_then_add_profile() {
    let mut shell = Shell::new();
    shell.dashboard.open_page(Open::Settings(Entry::Rail));
    shell.press(&[Down]);
    assert_eq!(shell.focus(), "Settings: Rail: Account");
    // Profiles is Account's first box, Who's playing today? beside it, Discord under Profiles and Discord messages
    // under Who's playing today? (#197).
    shell.press(&[Right]);
    assert_eq!(shell.focus(), "Settings: Account: Profiles");
    shell.press(&[Right]);
    assert_eq!(shell.focus(), "Settings: Account: Who's playing today?");
    shell.press(&[Down]);
    assert_eq!(shell.focus(), "Settings: Account: Direct Messages");
    shell.press(&[Left]);
    assert_eq!(shell.focus(), "Settings: Account: Discord");
    shell.press(&[Up, A]);
    assert_eq!(shell.focus(), "Settings: Profiles: Tester");
    // Left at the first column does nothing on the page; Right is Add profile; the row's end stops.
    shell.press(&[Left]);
    assert_eq!(shell.focus(), "Settings: Profiles: Tester");
    shell.press(&[Right, Right]);
    assert_eq!(shell.focus(), "Settings: Profiles: Add profile");
    // B is Account, on Profiles.
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: Account: Profiles");
}

#[test]
fn add_profile_adds_the_gamertag_kept_and_focus_goes_to_it() {
    let mut shell = Shell::profiles();
    shell.add("Alpha");
    assert_eq!(shell.dashboard.library().profiles.list.len(), 2);
    // An empty gamertag, or one dropped, adds nothing and leaves focus on Add profile.
    shell.press(&[Down]);
    assert_eq!(shell.focus(), "Settings: Profiles: Add profile");
    shell.press(&[A]);
    shell.type_over("   ");
    assert!(shell.changes().is_empty());
    shell.press(&[A]);
    shell.dashboard.typed(Typed::Text("Beta"));
    shell.press(&[B]);
    assert!(shell.changes().is_empty());
    assert_eq!(shell.focus(), "Settings: Profiles: Add profile");
    assert_eq!(shell.dashboard.library().profiles.list.len(), 2);
}

#[test]
fn a_profiles_gamertag_and_real_name_are_typed_from_what_they_are() {
    let mut shell = Shell::profiles();
    shell.add("Alpha");
    let alpha = shell.id("Alpha");
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: Profile: Gamertag; \"Alpha\", real name \"\", shape 0");
    // Text entry starts from the gamertag.
    shell.press(&[A]);
    assert!(shell.focus().contains("\"Alpha|\""), "{}", shell.focus());
    shell.type_over("Alpha Two");
    assert_eq!(shell.changes(), [ProfileChange::Gamertag { profile: alpha, gamertag: "Alpha Two".into() }]);
    assert_eq!(shell.focus(), "Settings: Profile: Gamertag; \"Alpha Two\", real name \"\", shape 0");
    // An empty one changes nothing.
    shell.press(&[A]);
    shell.type_over("");
    assert!(shell.changes().is_empty());
    // The real name, which may be emptied.
    shell.press(&[Right, A]);
    shell.type_over(" Ann Smith ");
    assert_eq!(shell.changes(), [ProfileChange::RealName { profile: alpha, real_name: "Ann Smith".into() }]);
    assert_eq!(shell.focus(), "Settings: Profile: Real name; \"Alpha Two\", real name \"Ann Smith\", shape 0");
    // B is Profiles, on its box.
    shell.press(&[B]);
    assert_eq!(shell.focus(), "Settings: Profiles: Alpha Two");
}

#[test]
fn the_only_profile_has_no_remove_and_down_lands_on_the_last_box() {
    let mut shell = Shell::profiles();
    shell.press(&[A]);
    assert!(shell.focus().starts_with("Settings: Profile: Gamertag; \"Tester\""), "{}", shell.focus());
    shell.press(&[Right, Down]);
    assert!(shell.focus().starts_with("Settings: Profile: Picture;"), "{}", shell.focus());
    shell.press(&[Right]);
    assert!(shell.focus().starts_with("Settings: Profile: Picture;"), "no Remove profile: {}", shell.focus());
}

#[test]
fn remove_profile_asks_first_and_the_one_signed_in_gone_the_first_signs_in() {
    let mut shell = Shell::profiles();
    shell.add("Alpha");
    shell.press(&[Left, A, Right, Down]);
    assert!(shell.focus().starts_with("Settings: Profile: Remove profile;"), "{}", shell.focus());
    // Cancel, or B, removes nothing.
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: Profile: Prompt(Remove) of Tester");
    shell.press(&[Down, A]);
    assert!(shell.changes().is_empty());
    assert!(shell.focus().starts_with("Settings: Profile: Remove profile;"), "{}", shell.focus());
    assert_eq!(shell.dashboard.profile().gamertag, "Tester");
    // Remove: back on Profiles, focus on the Profile now in its place, and Alpha signed in.
    let tester = shell.id("Tester");
    shell.press(&[A, A]);
    assert_eq!(shell.changes(), [ProfileChange::Remove(tester)]);
    assert_eq!(shell.focus(), "Settings: Profiles: Alpha");
    assert_eq!(shell.dashboard.profile().gamertag, "Alpha");
    assert_eq!(shell.dashboard.profile_line(), "\"Alpha\", real name \"\", shape 0");
}

#[test]
fn picture_offers_the_files_then_the_plain_ones_then_a_path_and_a_wears_one() {
    let mut shell = Shell::profiles();
    let tester = shell.id("Tester");
    shell.press(&[A, Down]);
    assert!(shell.focus().starts_with("Settings: Profile: Picture; \"Tester\""), "{}", shell.focus());
    shell.press(&[A]);
    // The account picture is worn: focus is on it, ticked.
    assert_eq!(shell.focus(), "Settings: Picture: user-192.png; worn");
    shell.press(&[Right, Right]);
    assert_eq!(shell.focus(), "Settings: Picture: Shape 0");
    shell.press(&[Down]);
    assert_eq!(shell.focus(), "Settings: Picture: Shape 4");
    shell.press(&[A]);
    assert_eq!(shell.changes(), [ProfileChange::Picture { profile: tester, picture: Picture::Shape(4) }]);
    assert_eq!(shell.focus(), "Settings: Profile: Picture; \"Tester\", real name \"\", shape 4");
    // Opened again, focus is on the one worn now; From a file is last.
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: Picture: Shape 4; worn");
    shell.press(&[Down, Down]);
    assert_eq!(shell.focus(), "Settings: Picture: From a file");
    // A path typed is worn, and the page steps back.
    shell.press(&[A]);
    let moth = pictures()[1].clone();
    shell.type_over(&format!("\"{}\"", moth.display()));
    assert_eq!(shell.changes(), [ProfileChange::Picture { profile: tester, picture: Picture::File(moth) }]);
    assert_eq!(shell.focus(), "Settings: Profile: Picture; \"Tester\", real name \"\", Moth.png");
    shell.press(&[A]);
    assert_eq!(shell.focus(), "Settings: Picture: Moth.png; worn");
    // B steps back a page at a time: the Profile, Profiles, Account.
    shell.press(&[B]);
    assert!(shell.focus().starts_with("Settings: Profile: Picture;"));
    shell.press(&[B, B]);
    assert_eq!(shell.focus(), "Settings: Account: Profiles");
}

#[test]
fn home_the_guide_and_the_greeting_show_the_profile_signed_in() {
    let mut shell = Shell::profiles();
    let tester = shell.id("Tester");
    shell.press(&[A, Right, A]);
    shell.type_over("Ann Smith");
    shell.press(&[Left, A]);
    shell.type_over("Annie");
    assert_eq!(shell.dashboard.profile().gamertag, "Annie");
    assert_eq!(shell.dashboard.profile().real_name, "Ann Smith");
    // The greeting greets the real name's first word, with the picture.
    shell.dashboard.greet();
    let toast = shell.dashboard.toasts().showing().unwrap().clone();
    assert_eq!(toast.text, "Hi Ann!");
    assert_eq!(toast.picture, Some(shell.dashboard.profile().picture));
    // The Guide's profile entry and Home's top bar draw `Dashboard::profile`.
    let mut dashboard = Dashboard::new(shell.snapshot());
    assert_eq!(dashboard.profile().id, tester);
    dashboard.sign_in(987);
    assert!(dashboard.take_effects().is_empty(), "a Profile the library has not got changes nothing");
}

#[test]
fn the_hook_signs_a_profile_in_and_the_database_remembers_it() {
    let mut shell = Shell::profiles();
    shell.add("Alpha");
    let alpha = shell.id("Alpha");
    shell.effects.clear();
    shell.dashboard.sign_in(alpha);
    assert_eq!(shell.dashboard.profile().gamertag, "Alpha");
    shell.carry_out();
    assert_eq!(shell.changes(), [ProfileChange::SignIn(alpha)]);
    assert_eq!(shell.database.profiles().unwrap().signed_in, Some(alpha));
}

#[test]
fn with_no_database_the_stores_user_shows_under_the_plain_picture() {
    let dashboard = Dashboard::new(screens::fake_library());
    assert_eq!(dashboard.profile_line(), "\"Player One\", real name \"\", shape 0");
}

// --- What needs a picture ---

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
    shell: Shell,
    /// Held until the fields above, the device among them, are gone.
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn over(mut shell: Shell) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        shell.dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, shell, _drawing: drawing }
    }

    /// The Profiles page with a second Profile, Alpha, wearing a plain picture: focus on Tester's box.
    fn profiles() -> Self {
        let mut shell = Shell::profiles();
        shell.add("Alpha");
        let alpha = shell.id("Alpha");
        shell.database.change_profile(&ProfileChange::Picture { profile: alpha, picture: Picture::Shape(2) }).unwrap();
        let snapshot = shell.snapshot();
        shell.dashboard.set_library(snapshot);
        shell.press(&[Left]);
        assert_eq!(shell.focus(), "Settings: Profiles: Tester");
        Self::over(shell)
    }

    fn press(&mut self, buttons: &[Button]) {
        self.shell.press(buttons);
    }

    /// The frame at rest, `seconds` into the ui's clock, compared with its reference.
    fn check_at(&mut self, name: &str, seconds: f64) {
        self.host.store.set_time(seconds);
        let Self { gpu, host, target, shell, .. } = self;
        let pixels = target.still(gpu, host, |ui| shell.dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(HD.0, HD.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }

    fn check(&mut self, name: &str) {
        self.check_at(name, 1.0);
    }
}

/// Account, focus on Profiles, its first box.
#[test]
fn settings_account_profiles() {
    let mut stage = Stage::profiles();
    stage.press(&[B]);
    assert_eq!(stage.shell.focus(), "Settings: Account: Profiles");
    stage.check("settings-account-profiles");
}

/// Profiles: a box a Profile, its picture and gamertag, then Add profile; focus on the first, then on Add profile.
#[test]
fn settings_profiles() {
    let mut stage = Stage::profiles();
    stage.check("settings-profiles");
    stage.press(&[Down]);
    assert_eq!(stage.shell.focus(), "Settings: Profiles: Add profile");
    stage.check("settings-profiles-add-profile");
}

/// A Profile's page: its picture, Gamertag, Real name, Picture and Remove profile; text entry over it on Gamertag;
/// Remove's question.
#[test]
fn settings_profile() {
    let mut stage = Stage::profiles();
    stage.press(&[Right, A]);
    stage.check("settings-profile");
    stage.press(&[A]);
    stage.check("settings-profile-gamertag-entry");
    stage.press(&[B, Right, Down, A]);
    assert_eq!(stage.shell.focus(), "Settings: Profile: Prompt(Remove) of Alpha");
    stage.check("settings-profile-remove-prompt");
}

/// The only Profile's page: no Remove profile, focus on Picture.
#[test]
fn settings_profile_only() {
    let mut stage = Stage::over(Shell::profiles());
    stage.press(&[A, Down]);
    stage.check("settings-profile-only");
}

/// Picture: the account picture worn and focused, the Gamerpics folder's, the plain ones, From a file; then focus
/// on a plain one.
#[test]
fn settings_profile_picture() {
    let mut stage = Stage::profiles();
    stage.press(&[A, Down, A]);
    assert_eq!(stage.shell.focus(), "Settings: Picture: user-192.png; worn");
    stage.check("settings-profile-picture");
    stage.press(&[Down, Down, Down]);
    assert_eq!(stage.shell.focus(), "Settings: Picture: From a file");
    stage.check("settings-profile-picture-from-a-file");
}

/// Picture with no picture files: the hint over the plain ones, which start lower.
#[test]
fn settings_profile_picture_hint() {
    let mut shell = Shell::offering(Vec::new());
    shell.dashboard.open_page(Open::Settings(Entry::Rail));
    shell.press(&[Down, Right, A, A, Down, A]);
    assert_eq!(shell.focus(), "Settings: Picture: Shape 0; worn");
    Stage::over(shell).check("settings-profile-picture-hint");
}

/// Home's top bar and the Guide's Profile & system with a Profile wearing a picture file, with a real name: the
/// gamertag first, the real name in its place 7.5 s on.
#[test]
fn home_and_the_guide_show_the_profile() {
    let mut shell = Shell::new();
    let tester = shell.id("Tester");
    let changes = [
        ProfileChange::Gamertag { profile: tester, gamertag: "Moth Runner".into() },
        ProfileChange::RealName { profile: tester, real_name: "Ann Smith".into() },
    ];
    for change in changes {
        shell.database.change_profile(&change).unwrap();
    }
    let snapshot = shell.snapshot();
    shell.dashboard.set_library(snapshot);
    let mut stage = Stage::over(shell);
    stage.check("home-profile");
    stage.check_at("home-profile-real-name", 10.0);
    stage.shell.dashboard.open_guide_on(Tab::ProfileAndSystem);
    stage.check("guide-profile-of-a-profile");
}
