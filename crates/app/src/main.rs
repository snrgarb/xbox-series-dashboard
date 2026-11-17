//! The executable: the Dashboard in a window, or one frame of it drawn offscreen to a PNG; or one of shell mode's
//! jobs (`shell_mode::run_job`: --shell, --sentinel, --shell-on, --shell-off, --shell-probe).
//!
//! On Windows a GUI program: no console window opens for it. Run from a terminal it prints there
//! (`shell_mode::dashboard::attach_parent_console`), but cmd and PowerShell don't wait for a GUI program: `start
//! /wait dashboard.exe ...`, or `Start-Process -Wait -NoNewWindow`, to wait for it and have its exit code.
#![cfg_attr(windows, windows_subsystem = "windows")]
mod as_shell;
mod games;
mod media_folder;
mod offscreen;
mod sounds;
mod sources;
mod startup;
mod store;
mod store_apps;
mod window;

use std::path::PathBuf;

pub use shell::since_start;

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
                 [--xbox <dir>]
                 [--media <dir>] [--no-startup] [--log-sounds] [--log-frames] [--log-focus] [--dry-power]
                 [--fake-media [--fake-media-art <picture>]]
                 [--log-queue-sources]
                 [--toast <text>]... [--notify <text>]... [--allow-toast-posts] [--open-addresses-with <program>]
                 [--art-service <url>] [--store-art-service <url>] [--idle-after <dim s>,<show s>]
                 [--discord-pipe <name> [--discord-token-url <url>] [--discord-cdn-url <url>]]
                 [--from-shell] [--dry-shell] [--log-file <file>]
       dashboard --shell-on | --shell-off [--dry-shell [--as-installed]] [--data-dir <dir>]
       dashboard --shell | --sentinel | --shell-probe: shell mode's own, which Windows starts
       dashboard --screenshot <out.png> [--size WxH] [--focus N] [--press right,down,a,lb,rb,...] [--guide]
                 [--launch-screen] [--bench] [--clock <text>] [--steam <steam root> | --library <file.json>]
                 [--data-dir <dir>] [--toast <text>] [--notify <text>]...
                 [--fake-media [--fake-media-art <picture>]]
       either with [--page keyboard]: start on the page that opens text entry, until Full library and Settings do";

/// A page by its name, for `--page`.
fn page(name: &str) -> Result<screens::pages::Open, String> {
    match name {
        "keyboard" => Ok(screens::pages::Open::KeyboardStandIn),
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
    let given: Vec<String> = std::env::args().skip(1).collect();
    // A GUI program, so no console window opens for it however it is started; run from a terminal it prints there.
    // The --shell process and the sentinel never take a terminal's console, which would tie them to it.
    if !matches!(given.first().map(String::as_str), Some(shell_mode::contract::SHELL | shell_mode::contract::SENTINEL))
    {
        shell_mode::dashboard::attach_parent_console();
    }
    // Shell mode's jobs come before anything of the Dashboard's: the --shell process never touches the GPU.
    if let Some(code) = shell_mode::run_job(&given, library::data_dir()) {
        std::process::exit(code);
    }
    let args = parse_args(given).unwrap_or_else(|e| fail(&format!("{e}\n{USAGE}")));
    // A log for every start: --log-file's; under shell mode its own place (as_shell); otherwise, where its lines
    // would go nowhere, the data folder's logs\.
    let data = args.data_dir.clone().or_else(library::data_dir);
    if let Some(path) = &args.log_file {
        if !shell_mode::dashboard::log_to_path(path) {
            eprintln!("dashboard: couldn't open --log-file {}", path.display());
        }
    } else if !args.from_shell && !shell_mode::dashboard::errors_go_somewhere() {
        if let Some(data) = &data {
            shell_mode::dashboard::log_to_file(&data.join("logs"));
        }
    }
    // One Dashboard at a time, and under shell mode its log in the data folder from its first line.
    let as_shell = args.screenshot.is_none().then(|| {
        let own_log = args.from_shell && args.log_file.is_none();
        as_shell::AsShell::start(args.from_shell, own_log, args.dry_shell, args.data_dir.clone(), data.clone())
    });
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
        offscreen::run(&args, library, store.options(), |game| store.achievements_of(game));
    } else if args.bench || args.size.is_some() {
        fail(&format!("--bench and --size go with --screenshot\n{USAGE}"));
    } else {
        if args.log_queue_sources {
            log_queue_sources(&args);
        }
        window::run(&args, library, store, as_shell.expect("made without --screenshot"));
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
