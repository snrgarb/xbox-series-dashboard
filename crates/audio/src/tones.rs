//! Generated tones, for the tests and for a machine that has none of the console's sounds.
use std::path::Path;

use crate::wav::{self, Clip};

/// A sine of `hz` for `seconds`, at a fifth of full scale, with a few milliseconds of fade at each end so it
/// doesn't click.
pub fn tone(hz: f32, seconds: f32, rate: u32) -> Clip {
    let count = (seconds * rate as f32).round() as usize;
    let fade = (rate as usize / 200).max(1);
    let frames = (0..count).map(|i| {
        let edge = i.min(count - 1 - i).min(fade) as f32 / fade as f32;
        let sample = (i as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.2 * edge;
        [sample, sample]
    });
    Clip { rate, frames: frames.collect() }
}

/// Writes a tone of 0.15 s into `dir` under each of `names`, each a semitone above the one before, as 16-bit
/// stereo WAV at 44.1 kHz: what the console's own files are.
pub fn write(dir: &Path, names: &[&str]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (i, name) in names.iter().enumerate() {
        let hz = 440.0 * 2f32.powf(i as f32 / 12.0);
        std::fs::write(dir.join(name), wav::encode(&tone(hz, 0.15, 44_100)))?;
    }
    Ok(())
}
