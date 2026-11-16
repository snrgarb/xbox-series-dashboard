//! The sound, through WASAPI in shared mode on the default device (the one the `audio` crate's sounds play on), and
//! the clock the picture is paced by: the position of the sound the device has played. With no sound (none in the
//! file, no device, none asked for, or the device lost part-way) the clock runs on the system's monotonic clock
//! instead.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioClient, IAudioClock, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Variant::VT_LPWSTR;

use super::{Com, Step};
use crate::Clock;

/// How much sound the device's buffer holds, in 100 ns units: enough to ride out a late wake-up of the feeding
/// thread.
const BUFFER_100NS: i64 = 2_000_000;
/// The feeding thread's nap between top-ups, well under the buffer's length.
const NAP: Duration = Duration::from_millis(5);
/// Sound queued before the device is started, so it doesn't start on an empty buffer.
const PREROLL_SECONDS: f64 = 0.1;
/// A device position that hasn't moved for this long has stopped (it otherwise moves every 10 ms or so).
const STALL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy)]
enum Time {
    /// Media time standing still: before the first frame, or while the device has nothing to play.
    Stopped(f64),
    /// Media time `media` was reached at `at` and has moved with the system's clock since.
    Running { media: f64, at: Instant },
}

#[derive(Default)]
struct Pcm {
    bytes: VecDeque<u8>,
    /// The media time of the first sample queued.
    start: Option<f64>,
    ended: bool,
}

/// What the decoder, the thread feeding the sound device and whoever shows the picture share: the clock, the
/// decoded sound waiting to be played, and the count of frames decoded.
pub struct Playback {
    time: Mutex<Time>,
    pcm: Mutex<Pcm>,
    /// A device is playing the sound and driving the clock.
    audio: AtomicBool,
    /// The first frame is on screen: the sound may start.
    go: AtomicBool,
    closed: AtomicBool,
    pub decoded: AtomicU64,
}

impl Playback {
    pub fn new() -> Arc<Playback> {
        Arc::new(Playback {
            time: Mutex::new(Time::Stopped(0.0)),
            pcm: Mutex::new(Pcm::default()),
            audio: AtomicBool::new(false),
            go: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            decoded: AtomicU64::new(0),
        })
    }

    /// Playing is over: whoever shows it has let it go.
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Queues decoded sound whose first sample is at media time `time`. Dropped when no device is playing.
    pub fn push(&self, time: f64, bytes: &[u8]) {
        if self.audio.load(Ordering::SeqCst) {
            let mut pcm = self.pcm.lock().unwrap();
            pcm.start.get_or_insert(time);
            pcm.bytes.extend(bytes);
        }
    }

    /// No more sound will be queued.
    pub fn end_audio(&self) {
        self.pcm.lock().unwrap().ended = true;
    }

    fn run_free(&self, media: f64) {
        *self.time.lock().unwrap() = Time::Running { media, at: Instant::now() };
    }

    /// The device is gone or has played everything: from here the clock runs by itself, if it has been started.
    fn release_clock(&self) {
        self.audio.store(false, Ordering::SeqCst);
        if self.go.load(Ordering::SeqCst) {
            self.run_free(self.now());
        }
        self.pcm.lock().unwrap().bytes = VecDeque::new();
    }
}

impl Clock for Playback {
    fn now(&self) -> f64 {
        match *self.time.lock().unwrap() {
            Time::Stopped(media) => media,
            Time::Running { media, at } => media + at.elapsed().as_secs_f64(),
        }
    }

    fn start(&self, media: f64) {
        self.go.store(true, Ordering::SeqCst);
        if !self.audio.load(Ordering::SeqCst) {
            self.run_free(media);
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn decoded(&self) -> u64 {
        self.decoded.load(Ordering::Relaxed)
    }
}

/// Opens the default sound device for `format` (a `WAVEFORMATEX` and whatever follows it, as bytes) at `volume` on
/// a thread of its own, which then plays what [`Playback::push`] queues once the clock has been started. Returns
/// the device's name, or why there is no sound.
pub fn open(playback: &Arc<Playback>, format: Vec<u8>, volume: f32) -> Result<String, String> {
    let (opened, result) = mpsc::channel();
    let playback = playback.clone();
    let thread = std::thread::Builder::new().name("video sound".into()).spawn(move || {
        let com = match Com::start() {
            Ok(com) => com,
            Err(e) => return drop(opened.send(Err(e))),
        };
        let device = match Device::open(&format, volume) {
            Ok(device) => device,
            Err(e) => return drop(opened.send(Err(e))),
        };
        playback.audio.store(true, Ordering::SeqCst);
        let _ = opened.send(Ok(device.name.clone()));
        if let Err(e) = device.play(&playback) {
            eprintln!("video: the sound stopped ({e}); the picture carries on by the system's clock");
        }
        playback.release_clock();
        drop(device);
        drop(com);
    });
    thread.map_err(|e| format!("no sound thread: {e}"))?;
    result.recv().unwrap_or(Err("the sound thread died".into()))
}

struct Device {
    name: String,
    client: IAudioClient,
    render: IAudioRenderClient,
    clock: IAudioClock,
    /// The buffer's length in frames (a frame is one sample for every channel).
    buffer_frames: u32,
    frame_bytes: usize,
    rate: u32,
    description: String,
}

impl Device {
    fn open(format: &[u8], volume: f32) -> Result<Device, String> {
        if format.len() < size_of::<WAVEFORMATEX>() {
            return Err("the decoder gave no wave format".into());
        }
        // SAFETY: `format` holds a whole WAVEFORMATEX, checked above; it is read unaligned, and WASAPI reads the
        // extension after it only as far as its own `cbSize`, which Media Foundation sized the allocation by.
        let wave = unsafe { std::ptr::read_unaligned(format.as_ptr() as *const WAVEFORMATEX) };
        let (channels, rate, bits, frame_bytes) =
            (wave.nChannels, wave.nSamplesPerSec, wave.wBitsPerSample, wave.nBlockAlign as usize);
        if frame_bytes == 0 || rate == 0 {
            return Err("the decoder gave an empty wave format".into());
        }
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).step("no audio device enumerator")?;
            let endpoint = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).step("no default audio endpoint")?;
            let name = friendly_name(&endpoint).unwrap_or_else(|| "unnamed".into());
            let client: IAudioClient = endpoint.Activate(CLSCTX_ALL, None).step("IMMDevice::Activate")?;
            // Shared mode takes only the mix format's rate and channels unless asked to convert.
            let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    flags,
                    BUFFER_100NS,
                    0,
                    format.as_ptr() as *const WAVEFORMATEX,
                    None,
                )
                .step("IAudioClient::Initialize")?;
            let buffer_frames = client.GetBufferSize().step("IAudioClient::GetBufferSize")?;
            let render: IAudioRenderClient = client.GetService().step("IAudioClient::GetService(render)")?;
            let clock: IAudioClock = client.GetService().step("IAudioClient::GetService(clock)")?;
            // The stream's own volume, under the system's: the Dashboard's sound volume.
            if volume < 1.0 {
                let level: ISimpleAudioVolume = client.GetService().step("IAudioClient::GetService(volume)")?;
                level.SetMasterVolume(volume.clamp(0.0, 1.0), std::ptr::null()).step("SetMasterVolume")?;
            }
            let description = format!("{rate} Hz {channels} ch {bits}-bit");
            Ok(Device { name, client, render, clock, buffer_frames, frame_bytes, rate, description })
        }
    }

    /// Feeds the device from the queue and keeps the clock at what it has played, until everything queued has been
    /// played, playback is closed, or the device fails.
    fn play(&self, playback: &Playback) -> Result<(), String> {
        let frequency = unsafe { self.clock.GetFrequency() }.step("IAudioClock::GetFrequency")?.max(1);
        let preroll_bytes = (PREROLL_SECONDS * self.rate as f64) as usize * self.frame_bytes;
        let mut started = false;
        let mut start = 0.0;
        let mut last_position = 0;
        let mut last_moved = Instant::now();
        let mut scratch = Vec::new();
        loop {
            if playback.closed() {
                break;
            }
            if !started {
                let pcm = playback.pcm.lock().unwrap();
                let ready = pcm.bytes.len() >= preroll_bytes || pcm.ended;
                if !(playback.go.load(Ordering::SeqCst) && ready) {
                    drop(pcm);
                    std::thread::sleep(NAP);
                    continue;
                }
                if pcm.bytes.is_empty() {
                    return Ok(());
                }
                start = pcm.start.unwrap_or(0.0);
            }

            // Top the buffer up.
            let padding = unsafe { self.client.GetCurrentPadding() }.step("IAudioClient::GetCurrentPadding")?;
            let (ended, queued_frames) = {
                let mut pcm = playback.pcm.lock().unwrap();
                let frames =
                    (pcm.bytes.len() / self.frame_bytes).min(self.buffer_frames.saturating_sub(padding) as usize);
                scratch.clear();
                scratch.extend(pcm.bytes.drain(..frames * self.frame_bytes));
                (pcm.ended && pcm.bytes.len() < self.frame_bytes, frames)
            };
            if queued_frames > 0 {
                unsafe {
                    let buffer = self.render.GetBuffer(queued_frames as u32).step("IAudioRenderClient::GetBuffer")?;
                    // SAFETY: GetBuffer gave room for `queued_frames` frames, which is `scratch.len()` bytes.
                    std::ptr::copy_nonoverlapping(scratch.as_ptr(), buffer, scratch.len());
                    self.render.ReleaseBuffer(queued_frames as u32, 0).step("IAudioRenderClient::ReleaseBuffer")?;
                }
            }
            if !started {
                unsafe { self.client.Start() }.step("IAudioClient::Start")?;
                started = true;
                last_moved = Instant::now();
                eprintln!(
                    "video: sound started on \"{}\", {}, buffer {} ms",
                    self.name,
                    self.description,
                    self.buffer_frames as u64 * 1000 / self.rate as u64
                );
            }

            // The clock: what the device has played, as of when it said so.
            let (mut position, mut counter) = (0, 0);
            unsafe { self.clock.GetPosition(&mut position, Some(&mut counter)) }.step("IAudioClock::GetPosition")?;
            let media = start + position as f64 / frequency as f64;
            if position != last_position {
                last_position = position;
                last_moved = Instant::now();
                let age = Duration::from_nanos((counter_now().saturating_sub(counter) * 100).min(500_000_000));
                let at = last_moved.checked_sub(age).unwrap_or(last_moved);
                *playback.time.lock().unwrap() = Time::Running { media, at };
            } else if last_moved.elapsed() > STALL {
                *playback.time.lock().unwrap() = Time::Stopped(media);
            }
            // Everything has gone to the device: the caller lets the clock run on by itself.
            if ended && padding == 0 && queued_frames == 0 {
                break;
            }
            std::thread::sleep(NAP);
        }
        let _ = unsafe { self.client.Stop() };
        Ok(())
    }
}

/// The performance counter in 100 ns units, as `IAudioClock::GetPosition` reports it.
fn counter_now() -> u64 {
    let (mut counter, mut frequency) = (0, 0);
    unsafe {
        let _ = QueryPerformanceCounter(&mut counter);
        let _ = QueryPerformanceFrequency(&mut frequency);
    }
    (counter as u128 * 10_000_000 / frequency.max(1) as u128) as u64
}

fn friendly_name(endpoint: &IMMDevice) -> Option<String> {
    unsafe {
        let store = endpoint.OpenPropertyStore(STGM_READ).ok()?;
        let mut value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let inner = &value.Anonymous.Anonymous;
        let name = if inner.vt == VT_LPWSTR && !inner.Anonymous.pwszVal.is_null() {
            inner.Anonymous.pwszVal.to_string().ok()
        } else {
            None
        };
        let _ = PropVariantClear(&mut value);
        name
    }
}
