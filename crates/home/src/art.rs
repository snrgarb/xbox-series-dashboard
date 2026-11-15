//! Decodes game art off the UI thread: tiles cropped square with mips, heroes cropped to the screen's shape.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use image::imageops::FilterType;
use image::RgbaImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Tile(usize),
    Hero(usize),
}

pub struct Job {
    pub slot: Slot,
    pub path: PathBuf,
    /// Tile: the side of the largest square it is drawn at. Hero: the output size.
    pub target: (u32, u32),
}

pub struct Loaded {
    pub slot: Slot,
    /// Mip chain, largest first; empty when the file couldn't be decoded.
    pub mips: Vec<RgbaImage>,
}

/// Starts the workers on `jobs`, in order; each result goes to `tx`, then `wake` is called.
pub fn spawn(jobs: Vec<Job>, tx: Sender<Loaded>, wake: impl Fn() + Send + Sync + 'static) {
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4).min(jobs.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(jobs)));
    let wake = Arc::new(wake);
    for _ in 0..workers {
        let (queue, tx, wake) = (queue.clone(), tx.clone(), wake.clone());
        std::thread::spawn(move || loop {
            let Some(job) = queue.lock().unwrap().pop_front() else { break };
            let mips = decode(&job).unwrap_or_else(|e| {
                eprintln!("art: {}: {e}", job.path.display());
                Vec::new()
            });
            if tx.send(Loaded { slot: job.slot, mips }).is_err() {
                break;
            }
            wake();
        });
    }
}

fn decode(job: &Job) -> image::ImageResult<Vec<RgbaImage>> {
    // Steam stores some PNGs under a .jpg name, so go by content, not extension.
    let img = image::ImageReader::open(&job.path)?.with_guessed_format()?.decode()?.into_rgba8();
    let (w, h) = img.dimensions();
    let (tw, th) = job.target;
    // The largest centred crop of the target's shape; a portrait cover's is taken a little above the middle, where
    // the title usually is.
    let (cw, ch) = if w as u64 * th as u64 > h as u64 * tw as u64 {
        ((h as u64 * tw as u64 / th as u64) as u32, h)
    } else {
        (w, (w as u64 * th as u64 / tw as u64) as u32)
    };
    let (cw, ch) = (cw.max(1), ch.max(1));
    let x = (w - cw) / 2;
    let y = if matches!(job.slot, Slot::Tile(_)) { (h - ch) * 3 / 10 } else { (h - ch) / 2 };
    let mut level = image::imageops::crop_imm(&img, x, y, cw, ch).to_image();
    if cw > tw {
        level = image::imageops::resize(&level, tw, th, FilterType::CatmullRom);
    }
    let mut mips = vec![level];
    if matches!(job.slot, Slot::Tile(_)) {
        // Tiles are drawn at two sizes and in between, so they get a mip chain; a hero is drawn at one.
        while mips.last().unwrap().width() > 32 {
            let last = mips.last().unwrap();
            let next = image::imageops::resize(last, last.width() / 2, last.height() / 2, FilterType::Triangle);
            mips.push(next);
        }
    }
    Ok(mips)
}
