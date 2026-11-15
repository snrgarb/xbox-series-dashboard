//! The executable: the Dashboard in a window, or one frame of it drawn offscreen to a PNG.
mod offscreen;
mod sounds;
mod store;
mod window;

use std::path::PathBuf;

pub use shell::since_start;

#[derive(Default)]
pub struct Args {
    library: Option<PathBuf>,
    steam: Option<PathBuf>,
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
    /// The folder the sounds are in, in place of the data folder's and the Bundle's.
    media: Option<PathBuf>,
    /// Every sound played is a line in the log.
    log_sounds: bool,
    /// Say every few seconds how many frames were drawn: what an idle Dashboard is checked by.
    log_frames: bool,
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

const USAGE: &str = "usage: dashboard [--steam <steam root> | --library <file.json>] [--data-dir <dir>] [--windowed]
                 [--media <dir>] [--log-sounds] [--log-frames]
       dashboard --screenshot <out.png> [--size WxH] [--focus N] [--press right,down,a,...] [--guide]
                 [--launch-screen] [--bench] [--clock <text>] [--steam <steam root> | --library <file.json>]
                 [--data-dir <dir>]";

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
            "--log-sounds" => args.log_sounds = true,
            "--log-frames" => args.log_frames = true,
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
    let args = parse_args().unwrap_or_else(|e| fail(&format!("{e}\n{USAGE}")));
    let (library, store) = store::open(&args);
    if args.screenshot.is_some() {
        offscreen::run(&args, library);
    } else if args.bench || args.size.is_some() {
        fail(&format!("--bench and --size go with --screenshot\n{USAGE}"));
    } else {
        window::run(&args, library, store);
    }
}
