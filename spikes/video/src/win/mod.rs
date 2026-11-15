//! Everything that talks to Windows: Media Foundation to decode and encode, WASAPI for sound, the window.
pub mod audio;
pub mod decode;
pub mod make_clip;
pub mod player;

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{Error, GUID};
use windows::Win32::Media::MediaFoundation::{IMFAttributes, MFShutdown, MFStartup, MFSTARTUP_FULL, MF_VERSION};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// An error as its HRESULT and Windows' words for it.
pub fn hr(error: &Error) -> String {
    let message = error.message();
    let code = error.code().0 as u32;
    if message.is_empty() {
        format!("HRESULT 0x{code:08X}")
    } else {
        format!("HRESULT 0x{code:08X} {}", message.trim())
    }
}

/// Says which step failed.
pub trait Step<T> {
    fn step(self, what: &str) -> Result<T, String>;
}

impl<T> Step<T> for windows::core::Result<T> {
    fn step(self, what: &str) -> Result<T, String> {
        self.map_err(|e| format!("{what}: {}", hr(&e)))
    }
}

/// COM on this thread, in the multithreaded apartment, until dropped. Declare it before any COM object, so it is
/// dropped after them.
pub struct Com;

impl Com {
    pub fn start() -> Result<Com, String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok().step("CoInitializeEx")?;
        Ok(Com)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// COM and Media Foundation on this thread until dropped.
pub struct MediaFoundation {
    _com: Com,
}

impl MediaFoundation {
    pub fn start() -> Result<MediaFoundation, String> {
        let com = Com::start()?;
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.step("MFStartup")?;
        Ok(MediaFoundation { _com: com })
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        let _ = unsafe { MFShutdown() };
    }
}

/// A path as a null-terminated wide string.
pub fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Two 32-bit numbers in one attribute, as Media Foundation keeps sizes and ratios.
pub fn set_pair(attributes: &IMFAttributes, key: &GUID, high: u32, low: u32) -> windows::core::Result<()> {
    unsafe { attributes.SetUINT64(key, (high as u64) << 32 | low as u64) }
}

pub fn pair(attributes: &IMFAttributes, key: &GUID) -> Option<(u32, u32)> {
    unsafe { attributes.GetUINT64(key) }.ok().map(|v| ((v >> 32) as u32, v as u32))
}

/// A subtype's name: its FOURCC where it has one.
pub fn subtype_name(subtype: &GUID) -> String {
    let fourcc = subtype.data1.to_le_bytes();
    match subtype.data1 {
        1 => "PCM".into(),
        3 => "float PCM".into(),
        0x1610 => "AAC".into(),
        0x0161 => "WMA".into(),
        21 => "ARGB32".into(),
        22 => "RGB32".into(),
        _ if fourcc.iter().all(|b| b.is_ascii_graphic()) => String::from_utf8_lossy(&fourcc).into_owned(),
        other => format!("{other:#x}"),
    }
}
