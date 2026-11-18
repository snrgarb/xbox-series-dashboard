//! The steps, on Windows. Each runs on a thread of its own (the web view host's: COM single-threaded, its own window
//! and messages), with a watchdog that ends the process if a step hangs past `--minutes`.
mod dms;
mod host;
mod meter;
mod probe;
mod procs;
mod send;
mod sign_in;
mod sign_out;
mod system;
mod voice;
mod voice_list;
mod voice_probe;
mod webaudio;

use std::time::Duration;

use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};

use crate::args::{Options, Step};
use crate::say;

pub fn run(options: &Options) -> i32 {
    // Pixels are the screen's own at 4K, not scaled by Windows.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let minutes = options.minutes;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(minutes * 60 + 120));
        say!("watchdog: the step is still running {} min after it started: ended here", minutes + 2);
        std::process::exit(9);
    });
    if options.step == Step::SignOut {
        return sign_out::run(options);
    }
    let options = options.clone();
    let thread = std::thread::Builder::new().name("webview".into()).spawn(move || {
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
            say!("COM: {e}");
        }
        let loader = match host::Loader::load() {
            Ok(loader) => loader,
            Err(e) => {
                say!("loader: {e}");
                if options.step == Step::Probe {
                    probe::without_loader(&options);
                }
                return 2;
            }
        };
        match options.step {
            Step::Probe => probe::run(&options, &loader),
            Step::SignIn => sign_in::run(&options, &loader),
            Step::Dms => dms::run(&options, &loader),
            Step::Send => send::run(&options, &loader),
            Step::VoiceProbe => voice_probe::run(&options, &loader),
            Step::VoiceList => voice_list::run(&options, &loader),
            Step::VoiceJoin | Step::Call | Step::VoiceIncoming | Step::DumpShape => voice::run(&options, &loader),
            Step::SignOut => unreachable!(),
        }
    });
    match thread.map(|t| t.join()) {
        Ok(Ok(code)) => code,
        Ok(Err(_)) => {
            say!("the web view thread panicked");
            70
        }
        Err(e) => {
            say!("no thread for the web view: {e}");
            71
        }
    }
}
