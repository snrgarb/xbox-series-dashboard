//! The executable: the Dashboard in a window, or one frame of it drawn offscreen to a PNG.
mod offscreen;
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
    focus: usize,
    bench: bool,
    windowed: bool,
    /// What the clock reads, in place of the time: a screenshot test's is the same every run.
    clock: Option<String>,
}

const USAGE: &str = "usage: dashboard [--steam <steam root> | --library <file.json>] [--data-dir <dir>] [--windowed]
       dashboard --screenshot <out.png> [--size WxH] [--focus N] [--guide] [--launch-screen] [--bench]
                 [--clock <text>] [--steam <steam root> | --library <file.json>] [--data-dir <dir>]";

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
            "--focus" => args.focus = value()?.parse().map_err(|e| format!("bad --focus: {e}"))?,
            "--bench" => args.bench = true,
            "--windowed" => args.windowed = true,
            "--clock" => args.clock = Some(value()?),
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
