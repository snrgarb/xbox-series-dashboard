//! Spike for #6: plays a video file full screen through the renderer, decoded by Media Foundation's Source Reader
//! into a texture overwritten every frame, its sound through WASAPI and the picture paced by the sound's clock.
//! `--make-clip` writes the clip to test with. Windows only; findings in `docs/research/video-media-foundation.md`.
#![cfg_attr(not(windows), allow(dead_code))]
mod fit;
mod pattern;
#[cfg(windows)]
mod win;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

const USAGE: &str = "usage: video-spike <file> [--seconds n] [--windowed] [--no-audio] [--dxva] [--basic-processing]
       video-spike --make-clip <out.mp4 | out.wmv> [--seconds 5] [--size 1280x720] [--no-audio]";

#[derive(Debug, PartialEq)]
pub struct Play {
    pub file: PathBuf,
    /// Stop after this long, if the file hasn't ended.
    pub seconds: Option<f64>,
    pub windowed: bool,
    pub no_audio: bool,
    /// Decode on the GPU through a Direct3D 11 device of Media Foundation's own, reading each frame back.
    pub dxva: bool,
    /// The Source Reader's older, software-only YUV to RGB conversion in place of the video processor.
    pub basic_processing: bool,
}

#[derive(Debug, PartialEq)]
pub struct MakeClip {
    pub out: PathBuf,
    pub seconds: u32,
    pub size: (u32, u32),
    pub no_audio: bool,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Play(Play),
    MakeClip(MakeClip),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let (mut file, mut clip, mut seconds, mut size) = (None, None, None::<f64>, None);
    let (mut windowed, mut no_audio, mut dxva, mut basic_processing) = (false, false, false, false);
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--make-clip" => clip = Some(PathBuf::from(value()?)),
            "--seconds" => {
                let value = value()?;
                seconds = Some(value.parse().ok().filter(|&s| s > 0.0).ok_or(format!("bad --seconds {value}"))?);
            }
            "--size" => {
                let value = value()?;
                let parsed = value.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                let even = |&(w, h): &(u32, u32)| w >= 64 && h >= 64 && w % 2 == 0 && h % 2 == 0;
                size = Some(parsed.filter(even).ok_or(format!("bad --size {value}: even sides of 64 or more"))?);
            }
            "--windowed" => windowed = true,
            "--no-audio" => no_audio = true,
            "--dxva" => dxva = true,
            "--basic-processing" => basic_processing = true,
            _ if arg.starts_with("--") => return Err(format!("unknown argument {arg}")),
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => return Err(format!("more than one file: {arg}")),
        }
    }
    match (clip, file) {
        (Some(out), None) => Ok(Command::MakeClip(MakeClip {
            out,
            seconds: seconds.map_or(5, |s| s.ceil() as u32),
            size: size.unwrap_or((1280, 720)),
            no_audio,
        })),
        (None, Some(file)) if size.is_none() => {
            Ok(Command::Play(Play { file, seconds, windowed, no_audio, dxva, basic_processing }))
        }
        (None, Some(_)) => Err("--size goes with --make-clip".into()),
        (Some(_), Some(_)) => Err("either a file to play or --make-clip".into()),
        (None, None) => Err("no file".into()),
    }
}

/// Milliseconds since the first call, which `main` makes.
pub fn since_start() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// A line on stderr with the time since the process started.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!("[{:9.1} ms] {}", $crate::since_start(), format_args!($($arg)*))
    };
}

fn main() {
    since_start();
    let command = parse_args(std::env::args().skip(1)).unwrap_or_else(|e| {
        eprintln!("{e}\n{USAGE}");
        std::process::exit(2);
    });
    std::process::exit(run(command));
}

#[cfg(windows)]
fn run(command: Command) -> i32 {
    let result = match &command {
        Command::Play(play) => win::player::run(play),
        Command::MakeClip(clip) => win::make_clip::run(clip),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            log!("error: {e}");
            1
        }
    }
}

#[cfg(not(windows))]
fn run(_: Command) -> i32 {
    eprintln!("Windows only");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn a_file_plays() {
        let play = Play {
            file: "boot.mp4".into(),
            seconds: None,
            windowed: false,
            no_audio: false,
            dxva: false,
            basic_processing: false,
        };
        assert_eq!(parse(&["boot.mp4"]), Ok(Command::Play(play)));
    }

    #[test]
    fn switches_come_before_or_after_the_file() {
        let Ok(Command::Play(play)) = parse(&["--windowed", "boot.mp4", "--seconds", "2.5", "--no-audio"]) else {
            panic!("not a play command");
        };
        assert!(play.windowed && play.no_audio && !play.dxva);
        assert_eq!(play.seconds, Some(2.5));
    }

    #[test]
    fn make_clip_defaults_to_five_seconds_of_720p() {
        let clip = MakeClip { out: "clip.mp4".into(), seconds: 5, size: (1280, 720), no_audio: false };
        assert_eq!(parse(&["--make-clip", "clip.mp4"]), Ok(Command::MakeClip(clip)));
        let clip = MakeClip { out: "clip.wmv".into(), seconds: 2, size: (3840, 2160), no_audio: true };
        let args = ["--make-clip", "clip.wmv", "--size", "3840x2160", "--seconds", "2", "--no-audio"];
        assert_eq!(parse(&args), Ok(Command::MakeClip(clip)));
    }

    #[test]
    fn what_makes_no_sense_is_refused() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["a.mp4", "b.mp4"]).is_err());
        assert!(parse(&["a.mp4", "--make-clip", "b.mp4"]).is_err());
        assert!(parse(&["a.mp4", "--size", "640x480"]).is_err());
        assert!(parse(&["--make-clip", "b.mp4", "--size", "641x480"]).is_err());
        assert!(parse(&["a.mp4", "--seconds", "0"]).is_err());
        assert!(parse(&["a.mp4", "--loud"]).is_err());
    }
}
