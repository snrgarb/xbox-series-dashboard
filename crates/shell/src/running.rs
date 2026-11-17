//! Whether a program in a folder is running: how Set up this PC (#178) knows a launcher's app is open before it
//! runs the app's uninstaller. A process is looked at by its program's path, never by its name alone.
use std::path::Path;

/// The file name of a running program whose path is under `folder`, if one is: `steam.exe`. None anywhere but
/// Windows, and for a folder that is empty or the root of a drive.
pub fn under(folder: &Path) -> Option<String> {
    let folder = folder.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    // A drive's root, or nothing: every program would be "under" it.
    if folder.len() < 4 {
        return None;
    }
    imp::programs().into_iter().find_map(|path| {
        let lower = path.to_string_lossy().to_lowercase();
        let inside = lower.strip_prefix(&folder).is_some_and(|rest| rest.starts_with(['\\', '/']));
        inside.then(|| path.file_name().map(|name| name.to_string_lossy().into_owned())).flatten()
    })
}

#[cfg(windows)]
mod imp {
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// Every running program's path that can be asked: one Windows won't open (another user's, a protected one) is
    /// passed over.
    pub fn programs() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        // SAFETY: the snapshot is closed once; every buffer is as long as the length given with it.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() || snapshot as isize == -1 {
                return paths;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if !process.is_null() {
                    let mut path = [0u16; 1024];
                    let mut len = path.len() as u32;
                    if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len) != 0 {
                        paths.push(PathBuf::from(String::from_utf16_lossy(&path[..len as usize])));
                    }
                    CloseHandle(process);
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        paths
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn programs() -> Vec<std::path::PathBuf> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drives_root_or_nothing_is_never_a_folder_to_look_under() {
        assert_eq!(under(Path::new("")), None);
        assert_eq!(under(Path::new("C:\\")), None);
    }
}
