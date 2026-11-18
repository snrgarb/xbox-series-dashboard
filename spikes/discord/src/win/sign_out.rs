//! `--sign-out`: the session folder removed, and a look that nothing of it is left. This signs the PC out; Discord's
//! own record of the session (Settings > Devices) stays until it is logged out there.
use std::time::Duration;

use super::probe::walk;
use super::system;
use crate::args::Options;
use crate::say;

pub fn run(options: &Options) -> i32 {
    let dir = &options.data;
    if !dir.exists() {
        say!("sign-out: {} does not exist: nothing to remove", dir.display());
        return 0;
    }
    let (files, bytes) = walk(dir);
    say!("sign-out: {} holds {files} files, {:.1} MB", dir.display(), bytes as f64 / 1_048_576.0);
    let running = system::processes(&["discord-spike.exe"])[0];
    if running > 1 {
        say!(
            "sign-out: another discord-spike is running ({}), whose web view holds the folder: it may not all go",
            running - 1
        );
    }
    let mut last = None;
    for attempt in 1..=5 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => {
                last = None;
                break;
            }
            Err(e) => {
                say!("sign-out: attempt {attempt}: {e}");
                last = Some(e);
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
    let (left, _) = walk(dir);
    if !dir.exists() {
        say!("sign-out: nothing is left: {} is gone", dir.display());
        say!("sign-out: Discord still lists this session under User Settings > Devices until it is logged out there");
        0
    } else {
        say!(
            "sign-out: {left} files are left in {} ({})",
            dir.display(),
            last.map(|e| e.to_string()).unwrap_or_default()
        );
        1
    }
}
