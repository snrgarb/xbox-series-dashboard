//! Windows' volumes, read for Settings > Storage and Manage's storage ring (#179): every fixed drive, and the
//! drive a folder is on, each with its label, free and total space. Behind `model::Drives`, so a test makes its
//! own; anywhere but Windows nothing is read.
use std::path::Path;

use model::storage::{Drive, Drives};

/// Windows' own drives.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDrives;

impl Drives for SystemDrives {
    #[cfg(windows)]
    fn all(&self) -> Vec<Drive> {
        windows_drives::all()
    }

    #[cfg(not(windows))]
    fn all(&self) -> Vec<Drive> {
        Vec::new()
    }

    #[cfg(windows)]
    fn of(&self, path: &Path) -> Option<Drive> {
        windows_drives::of(path)
    }

    #[cfg(not(windows))]
    fn of(&self, _path: &Path) -> Option<Drive> {
        None
    }
}

#[cfg(windows)]
mod windows_drives {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use model::storage::Drive;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW, GetVolumePathNameW,
    };

    /// `GetDriveTypeW`'s DRIVE_FIXED: a disk in the machine, not a network share, a CD or a card reader.
    const DRIVE_FIXED: u32 = 3;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn text(wide: &[u16]) -> String {
        let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
        String::from_utf16_lossy(&wide[..end])
    }

    pub fn all() -> Vec<Drive> {
        let mut buffer = vec![0u16; 512];
        // SAFETY: the buffer is as long as said; Windows writes root strings, each ended by a 0, then a 0.
        let written = unsafe { GetLogicalDriveStringsW(buffer.len() as u32, buffer.as_mut_ptr()) } as usize;
        if written == 0 || written > buffer.len() {
            return Vec::new();
        }
        buffer[..written]
            .split(|&c| c == 0)
            .filter(|root| !root.is_empty())
            .map(String::from_utf16_lossy)
            .filter(|root| {
                // SAFETY: a root string ended by a 0.
                let kind = unsafe { GetDriveTypeW(wide(root).as_ptr()) };
                kind == DRIVE_FIXED
            })
            .filter_map(|root| drive(&root))
            .collect()
    }

    pub fn of(path: &Path) -> Option<Drive> {
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut root = vec![0u16; 1024];
        // SAFETY: both strings are ended by a 0 and the out buffer is as long as said. The path need not exist.
        let ok = unsafe { GetVolumePathNameW(path.as_ptr(), root.as_mut_ptr(), root.len() as u32) };
        (ok != 0).then(|| text(&root)).and_then(|root| drive(&root))
    }

    /// The drive at this root (`C:\`): its label and space. None where Windows can't say (no disk in it).
    fn drive(root: &str) -> Option<Drive> {
        let wide_root = wide(root);
        let (mut free, mut total) = (0u64, 0u64);
        // SAFETY: a root string ended by a 0 and two outs of ours; the drive's whole free space (quotas ignored)
        // is not asked for. Free is what this user may use, as Explorer shows it.
        let ok = unsafe { GetDiskFreeSpaceExW(wide_root.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };
        if ok == 0 {
            return None;
        }
        let mut label = vec![0u16; 261];
        // SAFETY: the label buffer is as long as said; everything else asked for is left out (null, 0).
        let named = unsafe {
            GetVolumeInformationW(
                wide_root.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        let label = if named != 0 { text(&label) } else { String::new() };
        Some(Drive { root: root.to_string(), label, free, total })
    }
}
