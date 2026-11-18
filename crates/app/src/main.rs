//! The executable: the Dashboard in a window, or one frame of it drawn offscreen to a PNG; or one of shell mode's
//! jobs (`shell_mode::run_job`: --shell, --sentinel, --shell-on, --shell-off, --shell-probe).
//!
//! On Windows a GUI program: no console window opens for it. Run from a terminal it prints there
//! (`shell_mode::dashboard::attach_parent_console`), but cmd and PowerShell don't wait for a GUI program: `start
//! /wait dashboard.exe ...`, or `Start-Process -Wait -NoNewWindow`, to wait for it and have its exit code.
#![cfg_attr(windows, windows_subsystem = "windows")]
// Every log line reaches stderr in one write, so two Dashboards appending to one log never tear each other's lines
// (crates/log-line). Defined before the modules, so they all take it rather than std's.
#[allow(unused_macros)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        log_line::eprintln!($($arg)*)
    };
}

mod as_shell;
mod backdrop;
mod game_pass;
mod games;
mod helper_programs;
mod hub_open;
mod media_folder;
mod offscreen;
mod set_up;
mod sounds;
mod sources;
mod startup;
mod storage;
mod store;
mod store_apps;
mod turns;
mod updates;
mod window;

use std::path::PathBuf;

pub use shell::since_start;

/// Which build this is: the workspace's version and the commit it was built from (build.rs), for Settings > About
/// and `--version`.
pub const BUILD: screens::settings::about::Build =
    screens::settings::about::Build { version: env!("CARGO_PKG_VERSION"), commit: env!("DASHBOARD_COMMIT") };

/// `--pretend-version`'s version, which a test's Dashboard takes for its own, so that the update it serves is newer.
static PRETEND: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// Which build this Dashboard takes itself for: [`BUILD`], or with `--pretend-version` that version and its commit.
pub fn build() -> screens::settings::about::Build {
    match PRETEND.get() {
        Some(version) => screens::settings::about::Build { version, ..BUILD },
        None => BUILD,
    }
}

#[derive(Default)]
pub struct Args {
    library: Option<PathBuf>,
    steam: Option<PathBuf>,
    /// A folder standing for the Epic Games Launcher's (`source_epic::Places::under`).
    epic: Option<PathBuf>,
    /// A folder standing for GOG's (`source_gog::Places::under`).
    gog: Option<PathBuf>,
    /// Fake stores to read in place of this machine's (the end-to-end tests'): each a folder with its own
    /// `registry.json` (docs/sources.md).
    ea: Option<PathBuf>,
    ubisoft: Option<PathBuf>,
    battlenet: Option<PathBuf>,
    /// A folder standing for a drive of Xbox games (`source_xbox::fixture`), read in place of this machine's packages:
    /// an empty one has none.
    xbox: Option<PathBuf>,
    /// A local PC Game Pass catalogue (`source_xbox::fixture::write_game_pass`), read in place of Microsoft's: a
    /// test's, which asks no service (`game_pass::ENV` names one too).
    gamepass: Option<PathBuf>,
    /// The folder the library database is in, in place of the user's own.
    data_dir: Option<PathBuf>,
    launch_screen: bool,
    guide: bool,
    screenshot: Option<PathBuf>,
    size: Option<(u32, u32)>,
    /// The Recent tile to put focus on, from the left. Home starts with focus on the profile.
    focus: Option<usize>,
    /// Buttons to press before the picture is taken, in order.
    press: Vec<ui::Button>,
    bench: bool,
    windowed: bool,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
    /// The folder the sounds and boot videos are in, in place of the data folder's and the Bundle's
    /// (`media_folder`).
    media: Option<PathBuf>,
    /// Start at Home, with no Start-up: no Boot video whatever is chosen. What the tests start with, but the one
    /// that asks for Start-up.
    no_startup: bool,
    /// Every sound played is a line in the log.
    log_sounds: bool,
    /// Say every few seconds how many frames were drawn: what an idle Dashboard is checked by.
    log_frames: bool,
    /// Say where focus goes: Home's, the Guide's, a dialog's.
    log_focus: bool,
    /// Say once a second, while Steam has a download under way, every count of how far it has got that might be
    /// read (`source_steam::probe`): what a real install is checked by.
    log_queue_sources: bool,
    /// Turning off, restarting and sleeping are said in the log and not done.
    dry_power: bool,
    /// A media session made up in place of Windows' (`shell::media::FakeMedia::stand_in`): the Now Playing card
    /// drawn on a machine with none, as a picture of it offscreen is.
    fake_media: bool,
    /// The fake session's album art.
    fake_media_art: Option<PathBuf>,
    /// The first pad's battery made up in place of Windows' word (`input::battery::Fake`): a percentage, a level,
    /// `wired` or `none`, for a payload whose pad is a virtual one.
    fake_pad_battery: Option<input::battery::Fake>,
    /// The program every store's address is given to, as `<program> open-address <address>`, in place of the one
    /// Windows has registered for it: a test machine whose registry is left alone (`standin.exe`).
    open_addresses_with: Option<PathBuf>,
    /// A page to start on, in place of Home: one that nothing on Home opens yet.
    page: Option<screens::pages::Open>,
    /// Notifications to post as the Dashboard starts, each telling of itself in a Toast.
    toasts: Vec<String>,
    /// Other programs may post notifications to the running Dashboard (docs/spec/guide.md, 9.7): for tests.
    allow_toast_posts: bool,
    /// SteamGridDB's address, in place of its own: a test's stand-in (`standin serve-art`). Never the real
    /// service in a test.
    art_service: Option<String>,
    /// The stores' own art online at one address, in place of each store's own services: a test's stand-in
    /// (`standin serve-art`'s `/stores`, `library::art::Hosts::all_at`). Never a store's real service in a test.
    store_art_service: Option<String>,
    /// Notifications that are in the list as the Dashboard starts, in order, with no Toast: what a test has in the
    /// Notifications Flyout.
    notify: Vec<String>,
    /// The Idle screen's two times in seconds, in place of Settings' and not saved: a test's, a few seconds long.
    idle_after: Option<screens::idle::Times>,
    /// Where Discord is (docs/discord.md): `--discord-pipe` names a test's pipe (`standin discord`) in place of
    /// Discord's own, `--discord-token-url` its token endpoint and `--discord-cdn-url` where avatars come from. With
    /// a pipe and no endpoint or CDN there is none: a test never reaches the real Discord.
    discord: discord::Place,
    /// Started by shell mode's --shell process (docs/spec/shell-mode.md, 6): the log goes to the data folder, the
    /// two events are set, and Start-up stays over everything.
    from_shell: bool,
    /// Settings > System says what it would write to turn shell mode on or off, and writes nothing: what every test
    /// starts the Dashboard with.
    dry_shell: bool,
    /// Where the log goes (appended), in place of stderr. Without it, a start whose stderr goes nowhere (by
    /// Explorer, a shortcut or another program: a GUI program has no console) logs to
    /// `<data>\logs\dashboard-<stamp>.log`.
    log_file: Option<PathBuf>,
    /// The update address, in place of `update::SOURCE`: a test's stand-in (`standin serve-updates`). Never the
    /// real one in a test.
    update_source: Option<String>,
    /// The helpers' table, in place of the `helpers` crate's: a test's, whose addresses are a stand-in's
    /// (`standin serve-helpers`). Never the real one in a test.
    helper_table: Option<PathBuf>,
    /// A test's folder for the helpers' records and installs (`legendary`, `games`), in place of the user's own.
    helper_home: Option<PathBuf>,
    /// The launchers' apps' table (Set up this PC), in place of the `helpers` crate's: a test's, whose addresses are
    /// a stand-in's (`standin serve-helpers --clients`). Never the real one in a test.
    client_table: Option<PathBuf>,
    /// A file standing in for Windows' list of installed programs (Set up this PC), in place of the registry: a
    /// test's, which the stand-in installers write and nothing of the machine's.
    setup_registry: Option<PathBuf>,
    /// The version this Dashboard takes itself for, in place of its own: a test's, older than the one it serves.
    pretend_version: Option<model::update::Version>,
    /// An update's new Dashboard, started by the one with this pid (docs/spec/updates.md): it takes the
    /// single-instance guard alongside it, has no Start-up, and tells it of its first frame.
    handover: Option<u32>,
}

impl Args {
    /// The presses that carry out `--focus`.
    pub fn focus_presses(&self) -> Vec<ui::Button> {
        focus_presses(self.focus)
    }

    /// With --fake-media, the media session made up in place of Windows'.
    pub fn fake_media(&self) -> Option<shell::media::FakeMedia> {
        self.fake_media.then(|| shell::media::FakeMedia::stand_in(self.fake_media_art.clone()))
    }
}

/// The presses that take focus from the profile, where Home starts, to this Recent tile: Down, then along.
pub fn focus_presses(tile: Option<usize>) -> Vec<ui::Button> {
    let along = |tile| std::iter::repeat_n(ui::Button::Right, tile);
    tile.map_or(Vec::new(), |tile| std::iter::once(ui::Button::Down).chain(along(tile)).collect())
}

const USAGE: &str = "usage: dashboard [--steam <steam root> | --library <file.json>] [--data-dir <dir>] [--windowed]
                 [--epic <dir>] [--gog <dir>] [--ea <dir>] [--ubisoft <dir>] [--battlenet <dir>]
                 [--xbox <dir>] [--gamepass <dir>]
                 [--media <dir>] [--no-startup] [--log-sounds] [--log-frames] [--log-focus] [--dry-power]
                 [--fake-media [--fake-media-art <picture>]]
                 [--fake-pad-battery <percent> | empty | low | medium | full | wired | none]
                 [--log-queue-sources]
                 [--toast <text>]... [--notify <text>]... [--allow-toast-posts] [--open-addresses-with <program>]
                 [--art-service <url>] [--store-art-service <url>] [--idle-after <dim s>,<show s>]
                 [--discord-pipe <name> [--discord-token-url <url>] [--discord-cdn-url <url>]]
                 [--from-shell] [--dry-shell] [--log-file <file>]
                 [--update-source <url>] [--pretend-version <x.y.z>] [--helper-table <file>]
                 [--helper-home <dir>] [--client-table <file>] [--setup-registry <file>]
       dashboard --shell-on | --shell-off [--dry-shell [--as-installed]] [--data-dir <dir>]
       dashboard --shell | --sentinel | --shell-probe: shell mode's own, which Windows starts
       dashboard --version: prints `dashboard <version> (<commit>)`
       dashboard --screenshot <out.png> [--size WxH] [--focus N] [--press right,down,a,lb,rb,...] [--guide]
                 [--launch-screen] [--bench] [--clock <text>] [--steam <steam root> | --library <file.json>]
                 [--data-dir <dir>] [--toast <text>] [--notify <text>]...
                 [--fake-media [--fake-media-art <picture>]]
       either with [--page keyboard | settings]: start on the page that opens text entry, until Full library and
                 Settings do; or on Settings, focus on its rail";

/// A page by its name, for `--page`.
fn page(name: &str) -> Result<screens::pages::Open, String> {
    match name {
        "keyboard" => Ok(screens::pages::Open::KeyboardStandIn),
        "settings" => Ok(screens::pages::Open::Settings(screens::settings::Entry::Rail)),
        _ => Err(format!("bad --page: no page {name}")),
    }
}

/// A button by its name on the pad, for `--press`.
fn button(name: &str) -> Result<ui::Button, String> {
    use ui::Button::*;
    Ok(match name.to_ascii_lowercase().as_str() {
        "up" => Up,
        "down" => Down,
        "left" => Left,
        "right" => Right,
        "a" => A,
        "b" => B,
        "x" => X,
        "y" => Y,
        "menu" => Menu,
        "view" => View,
        "lb" => LeftBumper,
        "rb" => RightBumper,
        "lt" => LeftTrigger,
        "rt" => RightTrigger,
        "ls" => LeftStick,
        "guide" => Guide,
        _ => return Err(format!("bad --press: no button {name}")),
    })
}

fn parse_args(given: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = given.into_iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--library" => args.library = Some(value()?.into()),
            "--launch-screen" => args.launch_screen = true,
            "--guide" => args.guide = true,
            "--steam" => args.steam = Some(value()?.into()),
            "--epic" => args.epic = Some(value()?.into()),
            "--gog" => args.gog = Some(value()?.into()),
            "--ea" => args.ea = Some(value()?.into()),
            "--ubisoft" => args.ubisoft = Some(value()?.into()),
            "--battlenet" => args.battlenet = Some(value()?.into()),
            "--xbox" => args.xbox = Some(value()?.into()),
            "--gamepass" => args.gamepass = Some(value()?.into()),
            "--data-dir" => args.data_dir = Some(value()?.into()),
            "--screenshot" => args.screenshot = Some(value()?.into()),
            "--size" => {
                let value = value()?;
                let size = value.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                args.size = Some(size.filter(|&(w, h)| w > 0 && h > 0).ok_or(format!("bad --size {value}"))?);
            }
            "--focus" => args.focus = Some(value()?.parse().map_err(|e| format!("bad --focus: {e}"))?),
            "--press" => args.press = value()?.split(',').map(button).collect::<Result<_, _>>()?,
            "--bench" => args.bench = true,
            "--windowed" => args.windowed = true,
            "--clock" => args.clock = Some(value()?),
            "--media" => args.media = Some(value()?.into()),
            "--no-startup" => args.no_startup = true,
            "--log-sounds" => args.log_sounds = true,
            "--log-frames" => args.log_frames = true,
            "--log-focus" => args.log_focus = true,
            "--log-queue-sources" => args.log_queue_sources = true,
            "--dry-power" => args.dry_power = true,
            "--fake-media" => args.fake_media = true,
            "--fake-pad-battery" => args.fake_pad_battery = Some(input::battery::Fake::parse(&value()?)?),
            "--fake-media-art" => args.fake_media_art = Some(value()?.into()),
            "--open-addresses-with" => args.open_addresses_with = Some(value()?.into()),
            "--page" => args.page = Some(page(&value()?)?),
            "--toast" => args.toasts.push(value()?),
            "--allow-toast-posts" => args.allow_toast_posts = true,
            "--notify" => args.notify.push(value()?),
            "--art-service" => args.art_service = Some(value()?),
            "--store-art-service" => args.store_art_service = Some(value()?),
            "--idle-after" => args.idle_after = Some(screens::idle::Times::parse_seconds(&value()?)?),
            "--discord-pipe" => args.discord.pipe = Some(value()?),
            "--discord-token-url" => args.discord.token_url = Some(value()?),
            "--discord-cdn-url" => args.discord.cdn_url = Some(value()?),
            "--from-shell" => args.from_shell = true,
            "--dry-shell" => args.dry_shell = true,
            "--log-file" => args.log_file = Some(value()?.into()),
            "--update-source" => args.update_source = Some(value()?),
            "--helper-table" => args.helper_table = Some(value()?.into()),
            "--helper-home" => args.helper_home = Some(value()?.into()),
            "--client-table" => args.client_table = Some(value()?.into()),
            "--setup-registry" => args.setup_registry = Some(value()?.into()),
            "--pretend-version" => {
                let value = value()?;
                args.pretend_version =
                    Some(model::update::Version::parse(&value).ok_or(format!("bad --pretend-version {value}"))?);
            }
            shell_mode::contract::HANDOVER => {
                args.handover = Some(value()?.parse().map_err(|e| format!("bad {arg}: {e}"))?);
                // The new version comes up in place of the old one, which has had its Start-up.
                args.no_startup = true;
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(args)
}

fn fail(message: &str) -> ! {
    eprintln!("dashboard: {message}");
    std::process::exit(2);
}

fn main() {
    since_start();
    // A panic's message in one write too, like every other line (crates/log-line).
    log_line::panics_in_one_write();
    let given: Vec<String> = std::env::args().skip(1).collect();
    // A GUI program, so no console window opens for it however it is started; run from a terminal it prints there.
    // The --shell process and the sentinel never take a terminal's console, which would tie them to it.
    if !matches!(given.first().map(String::as_str), Some(shell_mode::contract::SHELL | shell_mode::contract::SENTINEL))
    {
        shell_mode::dashboard::attach_parent_console();
    }
    // What a script, the installer or a test asks to know which build this is: nothing else is set up.
    if given == ["--version"] {
        println!("dashboard {}", BUILD.says_short());
        std::process::exit(0);
    }
    // Shell mode's jobs come before anything of the Dashboard's: the --shell process never touches the GPU.
    if let Some(code) = shell_mode::run_job(&given, library::data_dir()) {
        std::process::exit(code);
    }
    let args = parse_args(given.clone()).unwrap_or_else(|e| fail(&format!("{e}\n{USAGE}")));
    if let Some(version) = args.pretend_version {
        let _ = PRETEND.set(Box::leak(version.to_string().into_boxed_str()));
    }
    // A test's legendary folder, for every run of legendary, the game started through it too (#175).
    if let Some(dir) = &args.helper_home {
        std::env::set_var("LEGENDARY_CONFIG_PATH", dir.join("legendary"));
    }
    // A log for every start: --log-file's; under shell mode its own place (as_shell); otherwise, where its lines
    // would go nowhere, the data folder's logs\.
    let data = args.data_dir.clone().or_else(library::data_dir);
    if let Some(path) = &args.log_file {
        if !shell_mode::dashboard::log_to_path(path) {
            // Not lost: the data folder's logs\ instead, where the lines say why.
            if let Some(data) = &data {
                shell_mode::dashboard::log_to_file(&data.join("logs"));
            }
            eprintln!(
                "dashboard: couldn't open --log-file {}; logging to the data folder's logs instead",
                path.display()
            );
        }
    } else if !args.from_shell && !shell_mode::dashboard::errors_go_somewhere() {
        if let Some(data) = &data {
            shell_mode::dashboard::log_to_file(&data.join("logs"));
        }
    }
    // One Dashboard at a time, and under shell mode its log in the data folder from its first line.
    let as_shell = args.screenshot.is_none().then(|| {
        let own_log = args.from_shell && args.log_file.is_none();
        as_shell::AsShell::start(
            args.from_shell,
            own_log,
            args.dry_shell,
            args.data_dir.clone(),
            data.clone(),
            args.handover,
        )
    });
    eprintln!("dashboard {}", BUILD.says_short());
    if let Some(version) = PRETEND.get() {
        eprintln!("dashboard: taking itself for {version} (--pretend-version)");
    }
    // The versions an update kept beside this program, once it has started on its own: not while it takes over.
    if args.screenshot.is_none() && args.handover.is_none() {
        updates::remove_old();
    }
    // Every date in the user's short date format, as their Windows writes it (#237).
    let short_date = shell::locale::short_date_format();
    eprintln!("dashboard: dates as {}", short_date.as_deref().unwrap_or(screens::date::FALLBACK));
    screens::date::set_short_format(short_date);
    let (library, mut store) = store::open(&args);
    if args.screenshot.is_some() {
        // Art for the games a store's files gave none for, before the picture is taken, where there is a data
        // folder for it and art online is not turned off.
        let mut library = store.art_now(library);
        // An install under way in a store's client shows as it would once the Queue's thread had looked.
        if let Some(queue) = store.queue_now() {
            library.queue = queue;
        }
        // The stores' achievements as their files say, with no rarity: the thread that would fetch it is not run.
        if let Some(achievements) = store.achievements_now() {
            library.achievements = achievements;
        }
        // An art picker a press opens is given what it asks for, fetched here and now, as the window fetches it.
        let (sender, answers) = std::sync::mpsc::channel();
        let sender = std::sync::Mutex::new(sender);
        let picker = store.art_picker(move |answer| {
            let _ = sender.lock().unwrap().send(answer);
        });
        let fetch = |asks: Vec<model::art_picker::Ask>| -> Vec<model::art_picker::Answer> {
            let Some(picker) = &picker else { return Vec::new() };
            let count = asks.len();
            asks.into_iter().for_each(|ask| picker.ask(ask));
            (0..count).map_while(|_| answers.recv_timeout(std::time::Duration::from_secs(30)).ok()).collect()
        };
        offscreen::run(&args, library, store.options(), |game| store.achievements_of(game), fetch);
    } else if args.bench || args.size.is_some() {
        fail(&format!("--bench and --size go with --screenshot\n{USAGE}"));
    } else {
        if args.log_queue_sources {
            log_queue_sources(&args);
        }
        window::run(&args, &given, library, store, as_shell.expect("made without --screenshot"));
    }
}

/// `--log-queue-sources`: a thread of its own looks at the Steam the Dashboard reads once a second, and says each
/// count of every download under way (`queue-sources: <appid> at <s> s: ...`), nothing while there is none.
fn log_queue_sources(args: &Args) {
    let Some(root) = args.steam.clone().or_else(source_steam::find_steam_root) else {
        eprintln!("queue-sources: no Steam to look at");
        return;
    };
    eprintln!("queue-sources: looking at {} once a second while a download is under way", root.display());
    let mut probe = source_steam::probe::Probe::new(root, args.steam.is_none());
    std::thread::spawn(move || loop {
        for line in probe.look() {
            eprintln!("{line}");
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn open_addresses_with_names_the_program_and_needs_one() {
        let args = parse(&["--open-addresses-with", r"C:\Users\Test User\e2e\standin.exe", "--dry-power"]).unwrap();
        assert_eq!(args.open_addresses_with, Some(PathBuf::from(r"C:\Users\Test User\e2e\standin.exe")));
        assert!(args.dry_power);
        assert_eq!(parse(&[]).unwrap().open_addresses_with, None);
        assert!(parse(&["--open-addresses-with"]).is_err());
    }

    #[test]
    fn log_file_names_where_the_log_goes_and_needs_one() {
        let args = parse(&["--log-file", r"C:\Users\Test User\dashboard.log"]).unwrap();
        assert_eq!(args.log_file, Some(PathBuf::from(r"C:\Users\Test User\dashboard.log")));
        assert_eq!(parse(&[]).unwrap().log_file, None);
        assert!(parse(&["--log-file"]).is_err());
    }

    #[test]
    fn idle_after_is_two_times_in_seconds() {
        let args = parse(&["--idle-after", "3,6"]).unwrap();
        let secs = |n| Some(std::time::Duration::from_secs(n));
        assert_eq!(args.idle_after, Some(screens::idle::Times { dim_after: secs(3), show_after: secs(6) }));
        assert!(parse(&["--idle-after", "3"]).is_err());
        assert_eq!(parse(&[]).unwrap().idle_after, None);
    }

    #[test]
    fn discord_pipe_and_token_url_point_the_dashboard_at_a_tests_discord() {
        let args = parse(&[
            "--discord-pipe",
            "e2e-discord",
            "--discord-token-url",
            "http://127.0.0.1:1/t",
            "--discord-cdn-url",
            "http://127.0.0.1:1",
        ])
        .unwrap();
        let place = discord::Place {
            pipe: Some("e2e-discord".into()),
            token_url: Some("http://127.0.0.1:1/t".into()),
            cdn_url: Some("http://127.0.0.1:1".into()),
        };
        assert_eq!(args.discord, place);
        assert_eq!(parse(&[]).unwrap().discord, discord::Place::default());
        assert!(parse(&["--discord-pipe"]).is_err());
    }
}
