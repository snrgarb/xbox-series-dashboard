//! Game art, decoded off the UI thread and kept as textures while screens ask for it. A screen asks every frame for
//! what it shows; what nothing has asked for in a while is freed once the cache is over its budget.
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use image::imageops::FilterType;
pub use image::RgbaImage;
use render::gpu::Gpu;
use render::{Renderer, TexId};

/// How an image is prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Scaled down to at most this many pixels wide, with mips: a tile's cover, drawn at several sizes.
    Thumb(u32),
    /// Scaled down to at most this many pixels wide, no mips: a background.
    Full(u32),
    /// A background made of a cover: 120 wide, blurred. Drawn stretched far past the screen.
    Blurred,
    /// A picture for behind a panel, as the Game hub's band has one: cut about its middle to 128 by 72 (a fifteenth
    /// of the screen), and blurred there by this radius as WPF's `BlurEffect` blurs. Drawn stretched over the
    /// screen, which smooths it further.
    Soft(u32),
    /// A picture this function draws, not one read from a file. The key's path is its name: it is what tells one
    /// painted picture from another. It is drawn on a decoding thread, so it may take its time.
    Painted(Painter),
}

/// Draws a picture: straight alpha, no mips.
#[derive(Clone, Copy)]
pub struct Painter(pub fn() -> RgbaImage);

// The name beside it in the key is what a painted picture is known by, never the function: a function's address
// is not one thing across a build.
impl PartialEq for Painter {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Painter {}
impl std::hash::Hash for Painter {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}
impl std::fmt::Debug for Painter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Painter")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtKey {
    pub path: PathBuf,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Art {
    /// Being decoded.
    Loading,
    Ready {
        tex: TexId,
        width: u32,
        height: u32,
    },
    /// No such file, or not an image.
    Failed,
}

struct Entry {
    art: Art,
    bytes: usize,
    used: u64,
    timing: ArtTiming,
}

/// How a piece of art came to be ready, for a log that says what a screen waited on (the Game hub's, #214).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArtTiming {
    /// When it was first asked for.
    pub asked: Instant,
    /// How long it waited for a decoding thread.
    pub queued: Duration,
    /// How long it took to read, decode and scale on that thread.
    pub decoded: Duration,
    /// How long its upload to the GPU took, on the thread that draws.
    pub uploaded: Duration,
    /// When it was ready to draw: None while it is still on its way.
    pub ready: Option<Instant>,
}

struct Loaded {
    key: ArtKey,
    /// Mip chain, largest first; empty when the file couldn't be decoded.
    mips: Vec<RgbaImage>,
    /// When a thread took it up, and how long it took there.
    started: Instant,
    took: Duration,
}

#[derive(Default)]
struct Queue {
    jobs: Mutex<(VecDeque<ArtKey>, bool)>,
    wake: Condvar,
}

pub struct ArtCache {
    entries: HashMap<ArtKey, Entry>,
    queue: Arc<Queue>,
    results: Receiver<Loaded>,
    pending: usize,
    frame: u64,
    bytes: usize,
    budget: usize,
    /// Pieces uploaded since [`ArtCache::take_uploads`] was last asked, and the time that took.
    uploads: (u32, Duration),
}

impl ArtCache {
    /// Starts the decoding threads. `wake` is called from one each time a piece of art is ready, to ask for a frame.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let queue = Arc::new(Queue::default());
        let (tx, results) = channel();
        let wake = Arc::new(wake);
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4);
        for _ in 0..workers {
            let (queue, tx, wake) = (queue.clone(), tx.clone(), wake.clone());
            std::thread::spawn(move || work(&queue, &tx, &*wake));
        }
        Self {
            entries: HashMap::new(),
            queue,
            results,
            pending: 0,
            frame: 0,
            bytes: 0,
            budget: 384 << 20,
            uploads: Default::default(),
        }
    }

    /// The art under `key`, asking for it to be decoded the first time. The newest asked for is decoded first, as
    /// that is what the screen shows now.
    pub fn get(&mut self, key: &ArtKey) -> Art {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.used = self.frame;
            return entry.art;
        }
        let timing = ArtTiming {
            asked: Instant::now(),
            queued: Duration::ZERO,
            decoded: Duration::ZERO,
            uploaded: Duration::ZERO,
            ready: None,
        };
        self.entries.insert(key.clone(), Entry { art: Art::Loading, bytes: 0, used: self.frame, timing });
        self.pending += 1;
        self.queue.jobs.lock().unwrap().0.push_back(key.clone());
        self.queue.wake.notify_one();
        Art::Loading
    }

    /// How the art under `key` came to be ready, or how long it has been on its way: None if it was never asked
    /// for, or has been freed since. Asking does not ask for it.
    pub fn timing(&self, key: &ArtKey) -> Option<ArtTiming> {
        self.entries.get(key).map(|entry| entry.timing)
    }

    /// Whether the art under `key` is ready to draw, without asking for it.
    pub fn is_ready(&self, key: &ArtKey) -> bool {
        self.entries.get(key).is_some_and(|entry| matches!(entry.art, Art::Ready { .. }))
    }

    /// How many pieces were uploaded since the last call, and the time that took.
    pub fn take_uploads(&mut self) -> (u32, Duration) {
        std::mem::take(&mut self.uploads)
    }

    /// How many pieces are still being decoded.
    pub fn pending(&self) -> usize {
        self.pending
    }

    /// Uploads what has been decoded since the last call and, over budget, frees what has gone unused longest.
    /// Called once a frame, before drawing.
    pub fn pump(&mut self, gpu: &Gpu, renderer: &mut Renderer) {
        self.frame += 1;
        while let Ok(loaded) = self.results.try_recv() {
            self.take(gpu, renderer, loaded);
        }
        if self.bytes > self.budget {
            let mut idle: Vec<(u64, ArtKey)> = self
                .entries
                .iter()
                .filter(|(_, e)| e.used + 120 < self.frame && matches!(e.art, Art::Ready { .. }))
                .map(|(k, e)| (e.used, k.clone()))
                .collect();
            idle.sort_by_key(|(used, _)| *used);
            for (_, key) in idle {
                if self.bytes <= self.budget * 3 / 4 {
                    break;
                }
                if let Some(Entry { art: Art::Ready { tex, .. }, bytes, .. }) = self.entries.remove(&key) {
                    renderer.remove_texture(tex);
                    self.bytes -= bytes;
                }
            }
        }
    }

    /// Waits for one more piece to be decoded and uploads it; false when nothing is pending.
    pub fn wait(&mut self, gpu: &Gpu, renderer: &mut Renderer) -> bool {
        if self.pending == 0 {
            return false;
        }
        match self.results.recv() {
            Ok(loaded) => {
                self.take(gpu, renderer, loaded);
                true
            }
            Err(_) => false,
        }
    }

    fn take(&mut self, gpu: &Gpu, renderer: &mut Renderer, loaded: Loaded) {
        self.pending = self.pending.saturating_sub(1);
        let Some(entry) = self.entries.get_mut(&loaded.key) else { return };
        entry.timing.queued = loaded.started.saturating_duration_since(entry.timing.asked);
        entry.timing.decoded = loaded.took;
        let Some(first) = loaded.mips.first() else {
            entry.art = Art::Failed;
            entry.timing.ready = Some(Instant::now());
            return;
        };
        let (width, height) = first.dimensions();
        entry.bytes = loaded.mips.iter().map(|m| m.as_raw().len()).sum();
        self.bytes += entry.bytes;
        let began = Instant::now();
        let tex = renderer.add_texture(&gpu.device, &gpu.queue, &loaded.mips);
        entry.art = Art::Ready { tex, width, height };
        let now = Instant::now();
        entry.timing.uploaded = now - began;
        entry.timing.ready = Some(now);
        self.uploads.0 += 1;
        self.uploads.1 += now - began;
    }
}

impl Drop for ArtCache {
    fn drop(&mut self) {
        self.queue.jobs.lock().unwrap().1 = true;
        self.queue.wake.notify_all();
    }
}

fn work(queue: &Queue, tx: &Sender<Loaded>, wake: &(dyn Fn() + Send + Sync)) {
    loop {
        let key = {
            let mut jobs = queue.jobs.lock().unwrap();
            loop {
                if jobs.1 {
                    return;
                }
                // Newest first: what was asked for last is what is on screen.
                if let Some(key) = jobs.0.pop_back() {
                    break key;
                }
                jobs = queue.wake.wait(jobs).unwrap();
            }
        };
        let started = Instant::now();
        let mips = decode(&key).unwrap_or_else(|e| {
            eprintln!("art: {}: {e}", key.path.display());
            Vec::new()
        });
        if tx.send(Loaded { key, mips, started, took: started.elapsed() }).is_err() {
            return;
        }
        wake();
    }
}

fn decode(key: &ArtKey) -> image::ImageResult<Vec<RgbaImage>> {
    if let Kind::Painted(Painter(paint)) = key.kind {
        return Ok(vec![paint()]);
    }
    // Steam stores some PNGs under a .jpg name, so go by content, not extension.
    let image = image::ImageReader::open(&key.path)?.with_guessed_format()?.decode()?.into_rgba8();
    let fit = |image: RgbaImage, max: u32, filter| {
        if image.width() <= max {
            return image;
        }
        let height = (image.height() as u64 * max as u64 / image.width() as u64).max(1) as u32;
        image::imageops::resize(&image, max, height, filter)
    };
    Ok(match key.kind {
        Kind::Thumb(max) => {
            let mut mips = vec![fit(image, max, FilterType::CatmullRom)];
            while mips.last().unwrap().width() > 32 && mips.last().unwrap().height() > 32 {
                let last = mips.last().unwrap();
                mips.push(image::imageops::resize(last, last.width() / 2, last.height() / 2, FilterType::Triangle));
            }
            mips
        }
        Kind::Full(max) => vec![fit(image, max, FilterType::CatmullRom)],
        Kind::Blurred => {
            // As the theme: decoded 120 wide and blurred by an eighth of that.
            let small = fit(image, 120, FilterType::Triangle);
            vec![image::imageops::blur(&small, 7.5)]
        }
        Kind::Soft(radius) => {
            let small = image::DynamicImage::ImageRgba8(image).resize_to_fill(128, 72, FilterType::Triangle);
            // WPF's blur reaches its radius at three deviations.
            vec![image::imageops::blur(&small.into_rgba8(), (radius as f32 / 3.0).max(0.01))]
        }
        Kind::Painted(_) => unreachable!("painted above"),
    })
}
