//! The PC as the probe sees it: Windows' version, the session and desktop the step runs in, whether Explorer is the
//! shell, and where the registry says the WebView2 runtime is.
use std::ffi::c_void;

use windows::core::HSTRING;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};
use windows::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
    DESKTOP_READOBJECTS, UOI_NAME,
};
use windows::Win32::UI::WindowsAndMessaging::GetShellWindow;

pub fn reg_string(root: HKEY, key: &str, value: &str) -> Option<String> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut size = 0u32;
    unsafe {
        RegGetValueW(root, &key, &value, RRF_RT_REG_SZ, None, None, Some(&mut size)).ok().ok()?;
        let mut buf = vec![0u16; size as usize / 2 + 1];
        RegGetValueW(root, &key, &value, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut c_void), Some(&mut size))
            .ok()
            .ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

pub fn reg_dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
    let (key, value) = (HSTRING::from(key), HSTRING::from(value));
    let mut out = 0u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            root,
            &key,
            &value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut out as *mut u32 as *mut c_void),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    Some(out)
}

/// How many processes of each of these names run now (any session).
pub fn processes(names: &[&str]) -> Vec<usize> {
    let mut counts = vec![0; names.len()];
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return counts };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
            for (i, name) in names.iter().enumerate() {
                if exe.eq_ignore_ascii_case(name) {
                    counts[i] += 1;
                }
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    counts
}

pub fn shell_window() -> bool {
    !unsafe { GetShellWindow() }.is_invalid()
}

/// This process's session, and the console's.
pub fn sessions() -> (u32, u32) {
    let mut mine = u32::MAX;
    unsafe {
        let _ = ProcessIdToSessionId(std::process::id(), &mut mine);
        (mine, WTSGetActiveConsoleSessionId())
    }
}

/// The name of the desktop that takes input now ("Default" when the user's desktop is up, nothing when this
/// process cannot see it: a task not run in the signed-in session).
pub fn input_desktop() -> String {
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(DESKTOP_READOBJECTS.0)) {
            Ok(desk) => {
                let mut buf = [0u16; 128];
                let mut needed = 0u32;
                let ok = GetUserObjectInformationW(
                    HANDLE(desk.0),
                    UOI_NAME,
                    Some(buf.as_mut_ptr() as *mut c_void),
                    (buf.len() * 2) as u32,
                    Some(&mut needed),
                )
                .is_ok();
                let _ = CloseDesktop(desk);
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                if ok {
                    String::from_utf16_lossy(&buf[..len])
                } else {
                    "(name not readable)".into()
                }
            }
            Err(e) => format!("(cannot open it: {e})"),
        }
    }
}
