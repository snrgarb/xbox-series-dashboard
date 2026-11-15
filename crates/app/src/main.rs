//! The executable: Home in a window, or one frame of it drawn offscreen to a PNG.
mod offscreen;
mod window;

use std::path::PathBuf;

use model::{Game, Library};

pub use shell::since_start;

#[derive(Default)]
pub struct Args {
    library: Option<PathBuf>,
    steam: Option<PathBuf>,
    launch_screen: bool,
    guide: bool,
    screenshot: Option<PathBuf>,
    size: Option<(u32, u32)>,
    focus: usize,
    bench: bool,
    windowed: bool,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
}

const USAGE: &str = "usage: home [--steam <steam root> | --library <file.json>] [--windowed]
       home --screenshot <out.png> [--size WxH] [--focus N] [--bench] [--clock <text>] [--library <file.json>]";

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--library" => args.library = Some(value()?.into()),
            "--launch-screen" => args.launch_screen = true,
            "--guide" => args.guide = true,
            "--steam" => args.steam = Some(value()?.into()),
            "--screenshot" => args.screenshot = Some(value()?.into()),
            "--size" => {
                let value = value()?;
                let size = value.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                args.size = Some(size.filter(|&(w, h)| w > 0 && h > 0).ok_or(format!("bad --size {value}"))?);
            }
            "--focus" => args.focus = value()?.parse().map_err(|e| format!("bad --focus: {e}"))?,
            "--bench" => args.bench = true,
            "--windowed" => args.windowed = true,
            "--clock" => args.clock = Some(value()?),
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(args)
}

/// The one place the Library comes from: `--library`'s JSON, `--steam`'s install, the
/// Steam install the registry names, else the built-in fake one.
fn load_library(args: &Args) -> Library {
    if let Some(path) = &args.library {
        let json = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        return serde_json::from_str(&json).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
    }
    match args.steam.clone().or_else(source_steam::find_steam_root) {
        Some(root) => {
            let t = std::time::Instant::now();
            let library = source_steam::load(&root).unwrap_or_else(|e| fail(&format!("{}: {e}", root.display())));
            eprintln!("steam: {} games from {} in {} ms", library.games.len(), root.display(), t.elapsed().as_millis());
            library
        }
        None => fake_library(),
    }
}

/// Ten games with no art, so Home always has something to show.
fn fake_library() -> Library {
    let names = [
        "Halo Infinite",
        "Forza Horizon 5",
        "Sea of Thieves",
        "Starfield",
        "Hades II",
        "Baldur's Gate 3",
        "Hollow Knight: Silksong",
        "Microsoft Flight Simulator",
        "Cyberpunk 2077",
        "Stardew Valley",
    ];
    let games = names.iter().enumerate().map(|(i, name)| Game {
        appid: 1000 + i as u32,
        name: name.to_string(),
        last_played: 1_790_000_000 - i as u64 * 86_400,
        playtime_minutes: 600 - i as u32 * 50,
        installed: true,
        install_dir: None,
        cover: None,
        hero: None,
        header: None,
    });
    Library { user: Some("Player One".into()), games: games.collect() }
}

fn fail(message: &str) -> ! {
    eprintln!("home: {message}");
    std::process::exit(2);
}

fn main() {
    since_start();
    let args = parse_args().unwrap_or_else(|e| fail(&format!("{e}\n{USAGE}")));
    if let Some(clock) = &args.clock {
        screens::set_clock(clock.clone());
    }
    let library = load_library(&args);
    if args.screenshot.is_some() {
        offscreen::run(&args, &library);
    } else if args.bench || args.size.is_some() {
        fail(&format!("--bench and --size go with --screenshot\n{USAGE}"));
    } else {
        window::run(&args, library);
    }
}
