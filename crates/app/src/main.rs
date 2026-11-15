//! The executable: the Dashboard in a window, or one frame of it drawn offscreen to a PNG.
mod offscreen;
mod window;

use std::path::PathBuf;

use model::Library;
pub use shell::since_start;

#[derive(Default)]
pub struct Args {
    library: Option<PathBuf>,
    steam: Option<PathBuf>,
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
}

impl Args {
    /// The presses that carry out `--focus`.
    pub fn focus_presses(&self) -> Vec<ui::Button> {
        focus_presses(self.focus)
    }
}

/// The presses that take focus from the profile, where Home starts, to this Recent tile: Down, then along.
pub fn focus_presses(tile: Option<usize>) -> Vec<ui::Button> {
    let along = |tile| std::iter::repeat_n(ui::Button::Right, tile);
    tile.map_or(Vec::new(), |tile| std::iter::once(ui::Button::Down).chain(along(tile)).collect())
}

const USAGE: &str = "usage: dashboard [--steam <steam root> | --library <file.json>] [--windowed]
       dashboard --screenshot <out.png> [--size WxH] [--focus N] [--press right,down,a,...] [--guide]
                 [--launch-screen] [--bench] [--clock <text>] [--steam <steam root> | --library <file.json>]";

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
        "guide" => Guide,
        _ => return Err(format!("bad --press: no button {name}")),
    })
}

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
            "--focus" => args.focus = Some(value()?.parse().map_err(|e| format!("bad --focus: {e}"))?),
            "--press" => args.press = value()?.split(',').map(button).collect::<Result<_, _>>()?,
            "--bench" => args.bench = true,
            "--windowed" => args.windowed = true,
            "--clock" => args.clock = Some(value()?),
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(args)
}

/// The one place the Library comes from: `--library`'s JSON, `--steam`'s install, the Steam install the registry
/// names, else the built-in fake one.
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
        None => screens::fake_library(),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("dashboard: {message}");
    std::process::exit(2);
}

fn main() {
    since_start();
    let args = parse_args().unwrap_or_else(|e| fail(&format!("{e}\n{USAGE}")));
    let library = load_library(&args);
    if args.screenshot.is_some() {
        offscreen::run(&args, library);
    } else if args.bench || args.size.is_some() {
        fail(&format!("--bench and --size go with --screenshot\n{USAGE}"));
    } else {
        window::run(&args, library);
    }
}
