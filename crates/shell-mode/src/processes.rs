//! The Dashboard's processes in this session, as the --shell process watches them: every process of `dashboard.exe`
//! at the watched path, never by its handle alone, as an update's handover starts the next one itself. Each is
//! opened as it is first seen, so its exit code can be read once it has exited.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
};

pub struct Processes {
    exe: PathBuf,
    own: u32,
    session: u32,
    tracked: BTreeMap<u32, HANDLE>,
}

fn session_of(pid: u32) -> Option<u32> {
    let mut session = 0;
    (unsafe { ProcessIdToSessionId(pid, &mut session) } != 0).then_some(session)
}

fn image(handle: HANDLE) -> Option<PathBuf> {
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size) };
    (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize])))
}

fn same(a: &Path, b: &Path) -> bool {
    a.display().to_string().eq_ignore_ascii_case(&b.display().to_string())
}

impl Processes {
    /// Watches the processes of `exe` in this session, but this one.
    pub fn new(exe: PathBuf) -> Self {
        let own = std::process::id();
        Self { exe, own, session: session_of(own).unwrap_or(u32::MAX), tracked: BTreeMap::new() }
    }

    /// One the --shell process started, by its handle: watched from its start, however short its life.
    pub fn adopt(&mut self, pid: u32, handle: HANDLE) {
        if let Some(old) = self.tracked.insert(pid, handle) {
            unsafe { CloseHandle(old) };
        }
    }

    /// The new ones since the last look, and the ones that have exited, with their codes.
    pub fn poll(&mut self) -> (Vec<u32>, Vec<(u32, u32)>) {
        let mut appeared = Vec::new();
        let name = self.exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if !snapshot.is_null() && snapshot as isize != -1 {
                let mut entry: PROCESSENTRY32W = std::mem::zeroed();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                let mut more = Process32FirstW(snapshot, &mut entry) != 0;
                while more {
                    let pid = entry.th32ProcessID;
                    let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let file = String::from_utf16_lossy(&entry.szExeFile[..end]);
                    if pid != self.own
                        && !self.tracked.contains_key(&pid)
                        && file.eq_ignore_ascii_case(&name)
                        && session_of(pid) == Some(self.session)
                    {
                        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, 0, pid);
                        if !handle.is_null() {
                            if image(handle).is_some_and(|path| same(&path, &self.exe)) {
                                self.tracked.insert(pid, handle);
                                appeared.push(pid);
                            } else {
                                CloseHandle(handle);
                            }
                        }
                    }
                    more = Process32NextW(snapshot, &mut entry) != 0;
                }
                CloseHandle(snapshot);
            }
        }
        let mut exited = Vec::new();
        self.tracked.retain(|&pid, &mut handle| unsafe {
            if WaitForSingleObject(handle, 0) != WAIT_OBJECT_0 {
                return true;
            }
            let mut code = 0;
            GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            exited.push((pid, code));
            false
        });
        (appeared, exited)
    }

    /// WM_CLOSE to every top-level window of these processes, as taskkill without /f asks.
    pub fn close(&self, pids: &[u32]) -> usize {
        struct Asked<'a> {
            pids: &'a [u32],
            count: usize,
        }
        unsafe extern "system" fn each(window: *mut core::ffi::c_void, asked: isize) -> i32 {
            let asked = &mut *(asked as *mut Asked);
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if asked.pids.contains(&pid) {
                PostMessageW(window, WM_CLOSE, 0, 0);
                asked.count += 1;
            }
            1
        }
        let mut asked = Asked { pids, count: 0 };
        unsafe { EnumWindows(Some(each), &mut asked as *mut Asked as isize) };
        asked.count
    }

    /// Which of these processes have the Dashboard's window, by its title: none of this exe's jobs makes one.
    pub fn windowed(&self, pids: &[u32]) -> Vec<u32> {
        struct Looking<'a> {
            pids: &'a [u32],
            title: Vec<u16>,
            found: Vec<u32>,
        }
        unsafe extern "system" fn each(window: *mut core::ffi::c_void, looking: isize) -> i32 {
            let looking = &mut *(looking as *mut Looking);
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if looking.pids.contains(&pid) && !looking.found.contains(&pid) {
                let mut text = [0u16; 64];
                let length = GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
                if text[..length] == looking.title[..] {
                    looking.found.push(pid);
                }
            }
            1
        }
        if pids.is_empty() {
            return Vec::new();
        }
        let title = crate::dashboard::WINDOW_TITLE.encode_utf16().collect();
        let mut looking = Looking { pids, title, found: Vec::new() };
        unsafe { EnumWindows(Some(each), &mut looking as *mut Looking as isize) };
        looking.found
    }

    /// Ends these processes.
    pub fn end(&self, pids: &[u32]) {
        for &pid in pids {
            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
                if !handle.is_null() {
                    TerminateProcess(handle, 1);
                    CloseHandle(handle);
                }
            }
        }
    }
}

impl Drop for Processes {
    fn drop(&mut self) {
        for (_, handle) in std::mem::take(&mut self.tracked) {
            unsafe { CloseHandle(handle) };
        }
    }
}
