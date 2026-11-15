//! Spike for issue #2: a full-screen, topmost, per-pixel-transparent Overlay window drawn by wgpu on DX12 through
//! DirectComposition, shown over whatever is running. Throwaway: what it found is in docs/research/overlay-window.md.
//!
//! Everything goes to stderr, each line stamped with the milliseconds since the process started.
#![cfg_attr(not(windows), allow(dead_code))]

mod args;
mod scene;
#[cfg(windows)]
mod win;

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since the process started: since this was first called, which `main` does first of all.
pub fn since_start() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// One line on stderr: the time, then `overlay: ` and the message.
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {
        eprintln!("{:>9.1} ms  overlay: {}", $crate::since_start(), format_args!($($arg)*))
    };
}

fn main() {
    since_start();
    #[cfg(not(windows))]
    {
        eprintln!("Windows only");
        std::process::exit(2);
    }
    #[cfg(windows)]
    {
        let args = match args::Args::parse(std::env::args().skip(1)) {
            Ok(args) => args,
            Err(message) => {
                eprintln!("{message}\n\n{}", args::USAGE);
                std::process::exit(2);
            }
        };
        std::process::exit(win::run(&args));
    }
}
