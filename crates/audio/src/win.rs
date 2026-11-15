//! The default sound device through WASAPI in shared mode, as the video spike proved it on the test VM
//! (`spikes/video/src/win/audio.rs`): 16-bit stereo PCM at the device's own rate, which
//! `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM` has the audio engine turn into whatever the device takes.
use std::cell::Cell;
use std::ffi::c_void;

use windows::core::{Error, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Variant::VT_LPWSTR;

use crate::engine::Output;
use crate::mix::to_i16;

/// How much sound the device's buffer holds, in 100 ns units. The engine keeps far less than this queued.
const BUFFER_100NS: i64 = 1_000_000;
/// The rate asked for when the device will not say its own.
const FALLBACK_RATE: u32 = 48_000;
/// `WAVE_FORMAT_PCM`.
const PCM: u16 = 1;

/// An error as its HRESULT and Windows' words for it.
fn hr(error: &Error) -> String {
    let message = error.message();
    let code = error.code().0 as u32;
    if message.is_empty() {
        format!("HRESULT 0x{code:08X}")
    } else {
        format!("HRESULT 0x{code:08X} {}", message.trim())
    }
}

/// Says which step failed.
trait Step<T> {
    fn step(self, what: &str) -> Result<T, String>;
}

impl<T> Step<T> for windows::core::Result<T> {
    fn step(self, what: &str) -> Result<T, String> {
        self.map_err(|e| format!("{what}: {}", hr(&e)))
    }
}

thread_local! {
    /// COM has been started on this thread. It is the mixer's, which lasts as long as the program.
    static COM: Cell<bool> = const { Cell::new(false) };
}

fn start_com() -> Result<(), String> {
    if !COM.get() {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok().step("CoInitializeEx")?;
        COM.set(true);
    }
    Ok(())
}

pub struct Wasapi {
    name: String,
    /// The endpoint's id, to tell whether it is still the default one.
    id: String,
    enumerator: IMMDeviceEnumerator,
    client: IAudioClient,
    render: IAudioRenderClient,
    /// The buffer's length in frames (a frame is one sample for each channel).
    buffer_frames: u32,
    rate: u32,
}

fn default_endpoint(enumerator: &IMMDeviceEnumerator) -> Result<IMMDevice, String> {
    unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }.step("no default audio endpoint")
}

/// A string Windows allocated, copied and freed.
unsafe fn take_string(string: PWSTR) -> Option<String> {
    if string.is_null() {
        return None;
    }
    let copy = string.to_string().ok();
    CoTaskMemFree(Some(string.0 as *const c_void));
    copy
}

fn endpoint_id(endpoint: &IMMDevice) -> Option<String> {
    unsafe { take_string(endpoint.GetId().ok()?) }
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

/// The rate the audio engine mixes this device at, so nothing is resampled twice.
fn mix_rate(client: &IAudioClient) -> Option<u32> {
    unsafe {
        let format = client.GetMixFormat().ok()?;
        if format.is_null() {
            return None;
        }
        // SAFETY: GetMixFormat gave a whole WAVEFORMATEX, which is read unaligned as it is a packed struct.
        let rate = std::ptr::read_unaligned(format).nSamplesPerSec;
        CoTaskMemFree(Some(format as *const c_void));
        (rate > 0).then_some(rate)
    }
}

impl Output for Wasapi {
    fn open() -> Result<Wasapi, String> {
        start_com()?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).step("no audio device enumerator")?;
            let endpoint = default_endpoint(&enumerator)?;
            let name = friendly_name(&endpoint).unwrap_or_else(|| "unnamed".into());
            let id = endpoint_id(&endpoint).unwrap_or_default();
            let client: IAudioClient = endpoint.Activate(CLSCTX_ALL, None).step("IMMDevice::Activate")?;
            let rate = mix_rate(&client).unwrap_or(FALLBACK_RATE);
            let format = WAVEFORMATEX {
                wFormatTag: PCM,
                nChannels: 2,
                nSamplesPerSec: rate,
                nAvgBytesPerSec: rate * 4,
                nBlockAlign: 4,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            // Shared mode takes only the mix format unless asked to convert.
            let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
            client
                .Initialize(AUDCLNT_SHAREMODE_SHARED, flags, BUFFER_100NS, 0, &format, None)
                .step("IAudioClient::Initialize")?;
            let buffer_frames = client.GetBufferSize().step("IAudioClient::GetBufferSize")?;
            let render: IAudioRenderClient = client.GetService().step("IAudioClient::GetService(render)")?;
            Ok(Wasapi { name, id, enumerator, client, render, buffer_frames, rate })
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn format(&self) -> String {
        let ms = self.buffer_frames as u64 * 1000 / self.rate as u64;
        format!("{} Hz 2 ch 16-bit, buffer {ms} ms", self.rate)
    }

    fn rate(&self) -> u32 {
        self.rate
    }

    fn capacity(&self) -> u32 {
        self.buffer_frames
    }

    fn queued(&mut self) -> Result<u32, String> {
        unsafe { self.client.GetCurrentPadding() }.step("IAudioClient::GetCurrentPadding")
    }

    fn write(&mut self, frames: &[[f32; 2]]) -> Result<(), String> {
        if frames.is_empty() {
            return Ok(());
        }
        unsafe {
            let buffer = self.render.GetBuffer(frames.len() as u32).step("IAudioRenderClient::GetBuffer")?;
            // SAFETY: GetBuffer gave room for `frames.len()` frames of two 16-bit samples each.
            let samples = buffer as *mut i16;
            for (i, sample) in frames.iter().flatten().enumerate() {
                samples.add(i).write_unaligned(to_i16(*sample));
            }
            self.render.ReleaseBuffer(frames.len() as u32, 0).step("IAudioRenderClient::ReleaseBuffer")
        }
    }

    fn start(&mut self) -> Result<(), String> {
        unsafe { self.client.Start() }.step("IAudioClient::Start")
    }

    fn stop(&mut self) {
        let _ = unsafe { self.client.Stop() };
    }

    fn is_default(&self) -> bool {
        default_endpoint(&self.enumerator)
            .ok()
            .and_then(|endpoint| endpoint_id(&endpoint))
            .is_some_and(|id| id == self.id)
    }
}

impl Drop for Wasapi {
    fn drop(&mut self) {
        self.stop();
    }
}
