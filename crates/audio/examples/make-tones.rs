//! `cargo run -p audio --example make-tones <dir>`: writes a short tone into <dir> under each of the names the
//! Dashboard's sounds have in the Bundle, for a machine with none of the console's own. `standin make-tones <dir>`
//! does the same on a test machine. Point the Dashboard at them with `--media <dir>`.
use std::path::PathBuf;

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: make-tones <dir>");
        std::process::exit(2);
    };
    let names = model::Sound::all_files();
    if let Err(e) = audio::tones::write(&dir, &names) {
        eprintln!("make-tones: {}: {e}", dir.display());
        std::process::exit(1);
    }
    eprintln!("make-tones: {} tones in {}", names.len(), dir.display());
}
