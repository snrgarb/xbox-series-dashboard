//! Decoding, on a thread of its own: one Source Reader gives the picture as RGB32 frames in system memory and the
//! sound as PCM. Frames go to the window over a short queue, so decoding runs a few frames ahead of the screen and
//! no further; the sound is kept about a second ahead of the picture.
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Arc;
use std::time::Instant;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Multithread, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
    D3D11_SDK_VERSION,
};
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFAttributes, IMFDXGIDeviceManager, IMFMediaType, IMFSample, IMFSourceReader, MFAudioFormat_PCM,
    MFCreateAttributes, MFCreateDXGIDeviceManager, MFCreateMediaType, MFCreateSourceReaderFromURL,
    MFCreateWaveFormatExFromMFMediaType, MFGetStrideForBitmapInfoHeader, MFMediaType_Audio, MFMediaType_Video,
    MFVideoFormat_RGB32, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
    MF_MT_MINIMUM_DISPLAY_APERTURE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MF_PD_DURATION,
    MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED, MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR,
    MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_D3D_MANAGER, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READER_MEDIASOURCE,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use windows::Win32::System::Variant::VT_UI8;

use super::audio::{self, Playback};
use super::{hr, pair, subtype_name, wide, MediaFoundation, Step};
use crate::log;

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
/// How far ahead of the picture the sound is decoded, in seconds.
const AUDIO_LEAD: f64 = 1.0;
/// Frames decoded and waiting for the window: [`spawn`]'s queue length.
pub const QUEUE: usize = 4;

pub struct Options {
    pub file: PathBuf,
    pub audio: bool,
    pub dxva: bool,
    pub basic_processing: bool,
}

/// What the file turned out to hold.
pub struct Opened {
    /// The picture's size in its own pixels.
    pub size: (u32, u32),
    /// Each pixel's shape, width to height.
    pub pixel_aspect: (u32, u32),
    pub playback: Arc<Playback>,
}

pub struct Frame {
    /// When it is to be shown, in seconds of media time.
    pub time: f64,
    /// Rows of B, G, R, X bytes from the top, four times the width apart.
    pub pixels: Vec<u8>,
    /// What the decoder and the colour conversion took for this frame.
    pub decode_ms: f64,
    /// What copying it out of Media Foundation's buffer took.
    pub copy_ms: f64,
}

pub enum Event {
    Opened(Opened),
    Frame(Frame),
    End,
    Failed(String),
}

/// Starts decoding `options.file`. The first event is `Opened` or `Failed`; then frames in order, until `End` or
/// `Failed`. `recycled` takes back the pixel buffers of frames done with, to be filled again.
pub fn spawn(options: Options, recycled: Receiver<Vec<u8>>) -> Receiver<Event> {
    let (events, receiver) = std::sync::mpsc::sync_channel(QUEUE);
    let thread = std::thread::Builder::new().name("decode".into());
    let failed = events.clone();
    let spawned = thread.spawn(move || {
        if let Err(e) = decode(&options, &events, &recycled) {
            let _ = events.send(Event::Failed(e));
        }
    });
    if let Err(e) = spawned {
        let _ = failed.send(Event::Failed(format!("no decode thread: {e}")));
    }
    receiver
}

/// How a frame lies in the buffers the reader hands out.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Layout {
    /// The size of the buffer's picture, which a codec may have rounded up to whole blocks.
    coded: (u32, u32),
    /// The part of it to show: x, y, width, height.
    shown: (u32, u32, u32, u32),
    /// Bytes from one row to the next when the buffer doesn't say; negative when the bottom row comes first.
    stride: i32,
    pixel_aspect: (u32, u32),
}

impl Layout {
    fn of(media_type: &IMFMediaType) -> Result<Layout, String> {
        let coded = pair(media_type, &MF_MT_FRAME_SIZE).ok_or("the decoded video has no frame size")?;
        // The aperture is an MFVideoArea: two MFOffsets (a fraction then a whole number, 16 bits each) and a SIZE.
        let mut area = [0u8; 16];
        let mut shown = (0, 0, coded.0, coded.1);
        if unsafe { media_type.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut area, None) }.is_ok() {
            let short = |at: usize| i16::from_le_bytes([area[at], area[at + 1]]).max(0) as u32;
            let long = |at: usize| i32::from_le_bytes(area[at..at + 4].try_into().unwrap()).max(0) as u32;
            let (x, y, w, h) = (short(2), short(6), long(8), long(12));
            if w > 0 && h > 0 && x + w <= coded.0 && y + h <= coded.1 {
                shown = (x, y, w, h);
            }
        }
        let stride = unsafe { media_type.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
            .map(|stride| stride as i32)
            .or_else(|_| unsafe { MFGetStrideForBitmapInfoHeader(MFVideoFormat_RGB32.data1, coded.0) })
            .unwrap_or(4 * coded.0 as i32);
        let pixel_aspect =
            pair(media_type, &MF_MT_PIXEL_ASPECT_RATIO).filter(|&(x, y)| x > 0 && y > 0).unwrap_or((1, 1));
        Ok(Layout { coded, shown, stride, pixel_aspect })
    }
}

fn decode(options: &Options, events: &SyncSender<Event>, recycled: &Receiver<Vec<u8>>) -> Result<(), String> {
    let _media_foundation = MediaFoundation::start()?;
    let reader = open_reader(options)?;
    let playback = Playback::new();

    // The picture: the first video stream, decoded and converted to RGB32.
    let (native_subtype, fps) = unsafe {
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false).step("deselecting the streams")?;
        reader.SetStreamSelection(VIDEO, true).step("no video stream in the file")?;
        let native = reader.GetNativeMediaType(VIDEO, 0).step("the video stream's format")?;
        let fps = pair(&native, &MF_MT_FRAME_RATE).filter(|&(_, d)| d > 0).map_or(0.0, |(n, d)| n as f64 / d as f64);
        let wanted = MFCreateMediaType().step("MFCreateMediaType")?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).step("SetGUID")?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).step("SetGUID")?;
        reader.SetCurrentMediaType(VIDEO, None, &wanted).step("no decoder to RGB32 for the video stream")?;
        (native.GetGUID(&MF_MT_SUBTYPE).map(|s| subtype_name(&s)).unwrap_or_else(|_| "?".into()), fps)
    };
    let current = unsafe { reader.GetCurrentMediaType(VIDEO) }.step("the decoded video's format")?;
    let mut layout = Layout::of(&current)?;

    // The sound: the first audio stream as PCM, if there is one and a device to play it.
    let sound = if options.audio { open_audio(&reader, &playback) } else { Err("--no-audio".into()) };
    let mut audio_left = sound.is_ok();
    if !audio_left {
        let _ = unsafe { reader.SetStreamSelection(AUDIO, false) };
    }

    let (_, _, w, h) = layout.shown;
    log!(
        "video: opened {w}x{h} {fps:.2} fps {}, video {native_subtype} -> RGB32{}, audio {}",
        duration(&reader).map_or("unknown length".into(), |seconds| format!("{seconds:.2} s")),
        if options.dxva { " (--dxva)" } else { "" },
        sound.as_ref().map_or("none", |(format, _)| format.as_str()),
    );
    match &sound {
        Ok((_, device)) => log!("audio: device \"{device}\" opened"),
        Err(why) => log!("audio: none ({why})"),
    }
    let opened = Opened { size: (w, h), pixel_aspect: layout.pixel_aspect, playback: playback.clone() };
    if events.send(Event::Opened(opened)).is_err() {
        return Ok(());
    }

    let (mut video_time, mut audio_time) = (0.0, 0.0);
    loop {
        if audio_left && audio_time < video_time + AUDIO_LEAD {
            let (flags, time, sample) = read(&reader, AUDIO).step("reading sound")?;
            if let Some(sample) = sample {
                audio_time = time + unsafe { sample.GetSampleDuration() }.unwrap_or(0) as f64 / 1e7;
                queue_sound(&sample, time, &playback).step("the decoded sound's buffer")?;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                playback.end_audio();
                audio_left = false;
            }
            continue;
        }

        let before = Instant::now();
        let (flags, time, sample) = read(&reader, VIDEO).step("reading video")?;
        let decode_ms = before.elapsed().as_secs_f64() * 1000.0;
        if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
            let changed = Layout::of(&unsafe { reader.GetCurrentMediaType(VIDEO) }.step("the video's new format")?)?;
            if changed.shown.2 != layout.shown.2 || changed.shown.3 != layout.shown.3 {
                return Err(format!("the video changed size part-way, to {}x{}", changed.shown.2, changed.shown.3));
            }
            layout = changed;
        }
        if let Some(sample) = sample {
            let mut pixels = recycled.try_recv().unwrap_or_default();
            let before = Instant::now();
            copy_frame(&sample, &layout, &mut pixels)?;
            let copy_ms = before.elapsed().as_secs_f64() * 1000.0;
            video_time = time;
            playback.decoded.fetch_add(1, Ordering::Relaxed);
            if events.send(Event::Frame(Frame { time, pixels, decode_ms, copy_ms })).is_err() {
                return Ok(());
            }
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            // The sound ends with the picture.
            playback.end_audio();
            let _ = events.send(Event::End);
            return Ok(());
        }
    }
}

fn open_reader(options: &Options) -> Result<IMFSourceReader, String> {
    unsafe {
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 2).step("MFCreateAttributes")?;
        let attributes = attributes.ok_or("MFCreateAttributes gave nothing")?;
        // The video processor converts the decoder's NV12 to RGB32 (and deinterlaces, should a file need it).
        let processing = if options.basic_processing {
            &MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING
        } else {
            &MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING
        };
        attributes.SetUINT32(processing, 1).step("SetUINT32")?;
        if options.dxva {
            if options.basic_processing {
                log!("video: --dxva ignored, --basic-processing is software only");
            } else {
                match device_manager() {
                    Ok(manager) => attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, &manager).step("SetUnknown")?,
                    Err(e) => log!("video: --dxva unavailable ({e}), decoding in software"),
                }
            }
        }
        let url = wide(&options.file);
        MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), &attributes)
            .map_err(|e| format!("can't open {}: {}", options.file.display(), hr(&e)))
    }
}

/// A Direct3D 11 device for Media Foundation to decode on, behind the manager the Source Reader takes.
fn device_manager() -> Result<IMFDXGIDeviceManager, String> {
    unsafe {
        let mut device = None;
        let flags = D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            flags,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .step("D3D11CreateDevice")?;
        let device = device.ok_or("D3D11CreateDevice gave nothing")?;
        // The decoder uses the device from its own threads.
        let _ = device.cast::<ID3D11Multithread>().step("ID3D11Multithread")?.SetMultithreadProtected(true);
        let (mut token, mut manager) = (0, None);
        MFCreateDXGIDeviceManager(&mut token, &mut manager).step("MFCreateDXGIDeviceManager")?;
        let manager = manager.ok_or("MFCreateDXGIDeviceManager gave nothing")?;
        manager.ResetDevice(&device, token).step("IMFDXGIDeviceManager::ResetDevice")?;
        Ok(manager)
    }
}

/// Sets the first audio stream to decode to PCM and opens a device for it. Returns the format's description and the
/// device's name, or why there is no sound.
fn open_audio(reader: &IMFSourceReader, playback: &Arc<Playback>) -> Result<(String, String), String> {
    unsafe {
        reader.SetStreamSelection(AUDIO, true).map_err(|_| "the file has no audio stream")?;
        let native = reader.GetNativeMediaType(AUDIO, 0).step("the audio stream's format")?;
        let native = native.GetGUID(&MF_MT_SUBTYPE).map(|s| subtype_name(&s)).unwrap_or_else(|_| "?".into());
        let wanted = MFCreateMediaType().step("MFCreateMediaType")?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).step("SetGUID")?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).step("SetGUID")?;
        reader.SetCurrentMediaType(AUDIO, None, &wanted).step("no decoder to PCM for the audio stream")?;
        let current = reader.GetCurrentMediaType(AUDIO).step("the decoded sound's format")?;

        let (mut wave, mut size) = (std::ptr::null_mut(), 0u32);
        MFCreateWaveFormatExFromMFMediaType(&current, &mut wave, Some(&mut size), 0).step("the sound's wave format")?;
        if wave.is_null() {
            return Err("the sound has no wave format".into());
        }
        // SAFETY: Media Foundation allocated `size` bytes at `wave`, freed just below.
        let format = std::slice::from_raw_parts(wave as *const u8, size as usize).to_vec();
        let (channels, rate, bits) = ((*wave).nChannels, (*wave).nSamplesPerSec, (*wave).wBitsPerSample);
        CoTaskMemFree(Some(wave as *const _));
        let device = audio::open(playback, format)?;
        Ok((format!("{native} -> PCM {rate} Hz {channels} ch {bits}-bit"), device))
    }
}

/// The file's length in seconds.
fn duration(reader: &IMFSourceReader) -> Option<f64> {
    unsafe {
        let mut value = reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION).ok()?;
        let inner = &value.Anonymous.Anonymous;
        let seconds = (inner.vt == VT_UI8).then(|| inner.Anonymous.uhVal as f64 / 1e7);
        let _ = PropVariantClear(&mut value);
        seconds
    }
}

/// The next sample of a stream: the reader's flags, the sample's time in seconds, and the sample, which a gap in the
/// stream or its end leaves out.
fn read(reader: &IMFSourceReader, stream: u32) -> windows::core::Result<(u32, f64, Option<IMFSample>)> {
    let (mut flags, mut time, mut sample) = (0, 0, None);
    unsafe { reader.ReadSample(stream, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))? };
    if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
        return Err(windows::core::Error::from(windows::Win32::Foundation::E_FAIL));
    }
    Ok((flags, time as f64 / 1e7, sample))
}

fn queue_sound(sample: &IMFSample, time: f64, playback: &Playback) -> windows::core::Result<()> {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let (mut data, mut len) = (std::ptr::null_mut(), 0);
        buffer.Lock(&mut data, None, Some(&mut len))?;
        // SAFETY: the buffer is locked, with `len` valid bytes at `data`, until Unlock.
        playback.push(time, std::slice::from_raw_parts(data, len as usize));
        buffer.Unlock()
    }
}

/// Copies the shown part of a decoded frame into `pixels`, top row first with no padding.
fn copy_frame(sample: &IMFSample, layout: &Layout, pixels: &mut Vec<u8>) -> Result<(), String> {
    let (x, y, w, h) = layout.shown;
    let row = 4 * w as usize;
    pixels.resize(row * h as usize, 0);
    // SAFETY: `first` is the buffer's top row and `stride` the distance to the next, for `coded.1` rows of at least
    // `4 * coded.0` bytes; the shown part lies inside the coded picture (Layout::of checks it).
    let copy = |first: *const u8, stride: isize, pixels: &mut [u8]| unsafe {
        for (line, out) in pixels.chunks_exact_mut(row).enumerate() {
            let from = first.offset((y as usize + line) as isize * stride).add(4 * x as usize);
            std::ptr::copy_nonoverlapping(from, out.as_mut_ptr(), row);
        }
    };
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer().step("the decoded frame's buffer")?;
        if let Ok(buffer) = buffer.cast::<IMF2DBuffer>() {
            // The buffer's own account of its rows: right whatever the media type says, and a read-back when the
            // frame is a texture.
            let (mut first, mut stride) = (std::ptr::null_mut(), 0);
            buffer.Lock2D(&mut first, &mut stride).step("IMF2DBuffer::Lock2D")?;
            if !first.is_null() && stride.unsigned_abs() as usize >= 4 * layout.coded.0 as usize {
                copy(first, stride as isize, pixels);
            }
            buffer.Unlock2D().step("IMF2DBuffer::Unlock2D")?;
            if first.is_null() || (stride.unsigned_abs() as usize) < 4 * layout.coded.0 as usize {
                return Err(format!("a decoded frame with rows {stride} bytes apart"));
            }
        } else {
            let (mut data, mut len) = (std::ptr::null_mut(), 0);
            buffer.Lock(&mut data, None, Some(&mut len)).step("IMFMediaBuffer::Lock")?;
            let stride = layout.stride.unsigned_abs() as usize;
            let fits = stride >= 4 * layout.coded.0 as usize && len as usize >= stride * layout.coded.1 as usize;
            if fits && layout.stride > 0 {
                copy(data, stride as isize, pixels);
            } else if fits {
                // The bottom row is first in memory.
                copy(data.add(stride * (layout.coded.1 as usize - 1)), -(stride as isize), pixels);
            }
            buffer.Unlock().step("IMFMediaBuffer::Unlock")?;
            if !fits {
                return Err(format!("a decoded frame of {len} bytes for {}x{}", layout.coded.0, layout.coded.1));
            }
        }
    }
    Ok(())
}
