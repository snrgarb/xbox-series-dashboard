//! Media Foundation's functions, found at run time in `mfplat.dll` and `mfreadwrite.dll` rather than imported: an
//! executable that imports them does not start at all on a Windows without them (an N edition without the Media
//! Feature Pack), and the Dashboard must start there, with no video (docs/research/video-media-foundation.md,
//! "Windows N"). Only these free functions are needed: everything else of Media Foundation is reached through the
//! COM objects they make. Each wrapper does what the `windows` crate's function of the same name does.
use std::ffi::c_void;
use std::sync::OnceLock;

use windows::core::{Interface, HRESULT, PCSTR, PCWSTR};
use windows::Win32::Media::Audio::WAVEFORMATEX;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFDXGIDeviceManager, IMFMediaType, IMFSourceReader, MFSTARTUP_FULL, MF_VERSION,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};

use super::{hr, Step};

type Startup = unsafe extern "system" fn(u32, u32) -> HRESULT;
type Shutdown = unsafe extern "system" fn() -> HRESULT;
type CreateAttributes = unsafe extern "system" fn(*mut *mut c_void, u32) -> HRESULT;
type CreateMediaType = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;
type CreateWaveFormat = unsafe extern "system" fn(*mut c_void, *mut *mut WAVEFORMATEX, *mut u32, u32) -> HRESULT;
type GetStride = unsafe extern "system" fn(u32, u32, *mut i32) -> HRESULT;
type CreateDeviceManager = unsafe extern "system" fn(*mut u32, *mut *mut c_void) -> HRESULT;
type CreateSourceReader = unsafe extern "system" fn(PCWSTR, *mut c_void, *mut *mut c_void) -> HRESULT;

pub struct Mf {
    startup: Startup,
    shutdown: Shutdown,
    create_attributes: CreateAttributes,
    create_media_type: CreateMediaType,
    create_wave_format: CreateWaveFormat,
    get_stride: GetStride,
    create_device_manager: CreateDeviceManager,
    create_source_reader: CreateSourceReader,
}

/// What `GetProcAddress` gives.
type Proc = unsafe extern "system" fn() -> isize;

/// Loads a DLL from System32 and finds a function in it, by its name with a trailing NUL.
unsafe fn find(dll: &str, name: &str) -> Result<Proc, String> {
    let wide: Vec<u16> = dll.encode_utf16().chain(Some(0)).collect();
    let module = LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
        .map_err(|e| format!("no Media Foundation on this Windows ({dll}: {})", hr(&e)))?;
    GetProcAddress(module, PCSTR(name.as_ptr()))
        .ok_or_else(|| format!("no Media Foundation on this Windows ({dll} has no {})", name.trim_end_matches('\0')))
}

impl Mf {
    /// The functions, loaded the first time they are asked for; or why they could not be.
    pub fn get() -> Result<&'static Mf, String> {
        static MF: OnceLock<Result<Mf, String>> = OnceLock::new();
        MF.get_or_init(|| unsafe { Mf::load() }).as_ref().map_err(Clone::clone)
    }

    unsafe fn load() -> Result<Mf, String> {
        // SAFETY: each is the function of that name, whose signature is the one given (as the `windows` crate's
        // bindings declare it); a DLL loaded is never unloaded, so the pointers stay good.
        let plat = |name| find("mfplat.dll", name);
        Ok(Mf {
            startup: std::mem::transmute::<Proc, Startup>(plat("MFStartup\0")?),
            shutdown: std::mem::transmute::<Proc, Shutdown>(plat("MFShutdown\0")?),
            create_attributes: std::mem::transmute::<Proc, CreateAttributes>(plat("MFCreateAttributes\0")?),
            create_media_type: std::mem::transmute::<Proc, CreateMediaType>(plat("MFCreateMediaType\0")?),
            create_wave_format: std::mem::transmute::<Proc, CreateWaveFormat>(plat(
                "MFCreateWaveFormatExFromMFMediaType\0",
            )?),
            get_stride: std::mem::transmute::<Proc, GetStride>(plat("MFGetStrideForBitmapInfoHeader\0")?),
            create_device_manager: std::mem::transmute::<Proc, CreateDeviceManager>(plat(
                "MFCreateDXGIDeviceManager\0",
            )?),
            create_source_reader: std::mem::transmute::<Proc, CreateSourceReader>(find(
                "mfreadwrite.dll",
                "MFCreateSourceReaderFromURL\0",
            )?),
        })
    }

    pub fn startup(&self) -> Result<(), String> {
        unsafe { (self.startup)(MF_VERSION, MFSTARTUP_FULL) }.ok().step("MFStartup")
    }

    pub fn shutdown(&self) {
        let _ = unsafe { (self.shutdown)() };
    }

    pub fn create_attributes(&self, size: u32) -> Result<IMFAttributes, String> {
        let mut out = std::ptr::null_mut();
        unsafe { (self.create_attributes)(&mut out, size) }.ok().step("MFCreateAttributes")?;
        // SAFETY: on success it is an IMFAttributes the caller owns a reference to.
        unsafe { owned(out) }.ok_or("MFCreateAttributes gave nothing".into())
    }

    pub fn create_media_type(&self) -> Result<IMFMediaType, String> {
        let mut out = std::ptr::null_mut();
        unsafe { (self.create_media_type)(&mut out) }.ok().step("MFCreateMediaType")?;
        // SAFETY: as above, an IMFMediaType.
        unsafe { owned(out) }.ok_or("MFCreateMediaType gave nothing".into())
    }

    /// The WAVEFORMATEX of an audio media type, and its size in bytes; to be freed with `CoTaskMemFree`.
    pub fn wave_format(&self, media_type: &IMFMediaType) -> Result<(*mut WAVEFORMATEX, u32), String> {
        let (mut wave, mut size) = (std::ptr::null_mut(), 0u32);
        unsafe { (self.create_wave_format)(media_type.as_raw(), &mut wave, &mut size, 0) }
            .ok()
            .step("the sound's wave format")?;
        Ok((wave, size))
    }

    pub fn stride(&self, format: u32, width: u32) -> Result<i32, String> {
        let mut stride = 0;
        unsafe { (self.get_stride)(format, width, &mut stride) }.ok().step("MFGetStrideForBitmapInfoHeader")?;
        Ok(stride)
    }

    pub fn create_device_manager(&self) -> Result<(u32, IMFDXGIDeviceManager), String> {
        let (mut token, mut out) = (0, std::ptr::null_mut());
        unsafe { (self.create_device_manager)(&mut token, &mut out) }.ok().step("MFCreateDXGIDeviceManager")?;
        // SAFETY: as above, an IMFDXGIDeviceManager.
        let manager = unsafe { owned(out) }.ok_or("MFCreateDXGIDeviceManager gave nothing")?;
        Ok((token, manager))
    }

    pub fn create_source_reader(
        &self,
        url: PCWSTR,
        attributes: &IMFAttributes,
    ) -> windows::core::Result<IMFSourceReader> {
        let mut out = std::ptr::null_mut();
        unsafe { (self.create_source_reader)(url, attributes.as_raw(), &mut out) }.ok()?;
        // SAFETY: as above, an IMFSourceReader.
        unsafe { owned(out) }.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_POINTER))
    }
}

/// An interface pointer a function handed over with its reference: none for null.
unsafe fn owned<T: Interface>(raw: *mut c_void) -> Option<T> {
    (!raw.is_null()).then(|| T::from_raw(raw))
}
