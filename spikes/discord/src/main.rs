//! Spike for #196: discord.com in a WebView2 hosted from Rust, run step by step on the TV PC by the lead, each step
//! writing what it found to a log. Windows only; what each step answers is in `docs/research/discord-web-spike.md`,
//! and for the voice steps of #292 (`--voice-*`, `--call`) in `docs/research/discord-web-voice.md`.
//!
//! Never read, printed or kept: the password, the token, cookies, query strings and fragments of addresses. Names of
//! conversations go to a file of their own (`--names`), never the log; the message sent and its conversation come
//! from files the lead writes, and only their lengths are logged.
//!
//! A Windows-subsystem exe: run by a scheduled task in the signed-in session, it shows no console window (a hidden
//! step must show nothing). Everything it has to say is in the log.
#![cfg_attr(not(windows), allow(dead_code))]
#![windows_subsystem = "windows"]
mod args;
mod index;
mod keyboard;
mod log;
mod pad;
mod redact;
#[cfg(windows)]
mod win;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match args::parse(&args) {
        Ok(options) => options,
        Err(message) => {
            // No console to say it on: the log beside the exe says it too.
            if let Some(dir) = std::env::current_exe().ok().as_deref().and_then(std::path::Path::parent) {
                log::open(&dir.join("discord-spike.log"));
            }
            log::line(&format!("the command line is refused: {message}"));
            eprintln!("\n{}", args::USAGE);
            std::process::exit(64);
        }
    };
    log::open(&options.log);
    log::line(&format!(
        "discord-spike {} started: {} (pid {})",
        env!("CARGO_PKG_VERSION"),
        options.step.name(),
        std::process::id()
    ));
    let code = run(&options);
    log::line(&format!("done: exit {code}"));
    std::process::exit(code);
}

#[cfg(windows)]
fn run(options: &args::Options) -> i32 {
    win::run(options)
}

#[cfg(not(windows))]
fn run(_: &args::Options) -> i32 {
    log::line("this spike runs on Windows only");
    69
}
