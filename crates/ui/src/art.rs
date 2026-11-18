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

/// How long a frame spends uploading decoded art, at most: once it has spent this, what is left waits for the next
/// frame. A frame always uploads one piece, so art keeps coming however big a piece is (#99: a frame that took in
/// every cover that had come in at once was 15 to 20 ms long, two refreshes missed at 120 Hz).
pub const UPLOAD_BUDGET: Duration = Duration::from_millis(2);
/// And how many bytes, at most, past the first piece: a piece is not begun if it would take the frame past this.
/// The clock says what was spent only once it has been, and a 4K background alone is 20 MB to copy.
pub const UPLOAD_BYTES: usize = 8 << 20;

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

/// Pictures that are only ever in memory (a person's avatar, which is never written to disk): the encoded bytes are
/// registered, and the key made from them names them by a number that is never reused. The decoding thread finds the
/// bytes by that name while something still holds them; the registry itself holds them weakly, so it keeps nothing
/// alive.
pub mod memory {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, Weak};

    use super::{ArtKey, Kind};

    const PREFIX: &str = "memory/";
    type Registry = (u64, Vec<(u64, Weak<[u8]>)>);
    /// The last id given, and (id, bytes) for each picture still held.
    static REGISTRY: Mutex<Registry> = Mutex::new((0, Vec::new()));

    /// The key of this encoded picture, decoded to at most `max` wide: the same key for the same bytes while they
    /// are held.
    pub fn key(bytes: &Arc<[u8]>, max: u32) -> ArtKey {
        let mut registry = REGISTRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.1.retain(|(_, weak)| weak.strong_count() > 0);
        let known = registry.1.iter().find(|(_, weak)| std::ptr::eq(weak.as_ptr(), Arc::as_ptr(bytes)));
        let id = match known {
            Some(&(id, _)) => id,
            None => {
                registry.0 += 1;
                let id = registry.0;
                registry.1.push((id, Arc::downgrade(bytes)));
                id
            }
        };
        ArtKey { path: PathBuf::from(format!("{PREFIX}{id}")), kind: Kind::Thumb(max) }
    }

    pub(super) fn is_memory(path: &Path) -> bool {
        path.to_str().is_some_and(|path| path.starts_with(PREFIX))
    }

    pub(super) fn lookup(path: &Path) -> Option<Arc<[u8]>> {
        let id: u64 = path.to_str()?.strip_prefix(PREFIX)?.parse().ok()?;
        let registry = REGISTRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.1.iter().find(|(found, _)| *found == id)?.1.upgrade()
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
    /// Decoded and not yet uploaded: what a frame's budget left for the next.
    decoded: VecDeque<Loaded>,
    /// Asked for and not yet uploaded: being decoded, or decoded and waiting its turn to be uploaded.
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
            decoded: VecDeque::new(),
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

    /// How many pieces are still on their way: being decoded, or decoded and waiting to be uploaded.
    pub fn pending(&self) -> usize {
        self.pending
    }

    /// How many pieces have been decoded and wait for a frame to upload them.
    pub fn waiting(&self) -> usize {
        self.decoded.len()
    }

    /// Uploads what has been decoded since the last call, as much as a frame's budget allows (`UPLOAD_BUDGET`,
    /// `UPLOAD_BYTES`), and, over the cache's budget, frees what has gone unused longest. Called once a frame,
    /// before drawing.
    pub fn pump(&mut self, gpu: &Gpu, renderer: &mut Renderer) {
        self.frame += 1;
        self.decoded.extend(self.results.try_iter());
        let began = Instant::now();
        let mut bytes = 0;
        while let Some(next) = self.decoded.front() {
            let size: usize = next.mips.iter().map(|mip| mip.as_raw().len()).sum();
            // The first piece always goes: a frame gives art its turn however big the piece is.
            let first = bytes == 0;
            if !first && (began.elapsed() >= UPLOAD_BUDGET || bytes + size > UPLOAD_BYTES) {
                break;
            }
            bytes += size.max(1);
            let loaded = self.decoded.pop_front().expect("the front");
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

    /// Uploads one more piece, waiting for it to be decoded if none is waiting; false when nothing is pending.
    pub fn wait(&mut self, gpu: &Gpu, renderer: &mut Renderer) -> bool {
        if self.pending == 0 {
            return false;
        }
        if let Some(loaded) = self.decoded.pop_front() {
            self.take(gpu, renderer, loaded);
            return true;
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
    let image = match memory::lookup(&key.path) {
        Some(bytes) => image::ImageReader::new(std::io::Cursor::new(&bytes[..])).with_guessed_format()?.decode()?,
        None if memory::is_memory(&key.path) => return Ok(Vec::new()),
        None => image::ImageReader::open(&key.path)?.with_guessed_format()?.decode()?,
    }
    .into_rgba8();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 16 MB: over the bytes a frame uploads past its first piece.
    fn big() -> RgbaImage {
        RgbaImage::new(2048, 2048)
    }

    #[test]
    fn a_frame_uploads_what_its_budget_allows_and_leaves_the_rest_for_the_next() {
        let gpu = render::gpu::init(render::gpu::instance(None), None);
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let mut cache = ArtCache::new(|| {});
        let keys: Vec<ArtKey> =
            (0..3).map(|n| ArtKey { path: format!("big {n}").into(), kind: Kind::Painted(Painter(big)) }).collect();
        for key in &keys {
            assert_eq!(cache.get(key), Art::Loading);
        }
        // All three decoded before a frame takes them in.
        for _ in &keys {
            let loaded = cache.results.recv().unwrap();
            cache.decoded.push_back(loaded);
        }
        cache.pump(&gpu, &mut renderer);
        assert_eq!((cache.take_uploads().0, cache.waiting(), cache.pending()), (1, 2, 2));
        cache.pump(&gpu, &mut renderer);
        assert_eq!((cache.take_uploads().0, cache.waiting(), cache.pending()), (1, 1, 1));
        // A picture taken offscreen waits for all of it, one piece at a time.
        assert!(cache.wait(&gpu, &mut renderer));
        assert!(!cache.wait(&gpu, &mut renderer));
        assert!(keys.iter().all(|key| cache.is_ready(key)));
    }
}
