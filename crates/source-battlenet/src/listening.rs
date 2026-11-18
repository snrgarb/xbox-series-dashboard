//! Which programs run, and which listen on which ports of this machine: how the Agent's port is found, as
//! barncastle's Battle.Net-Installer and HSCentric find it (`GetExtendedTcpTable` for the Agent's process; docs/
//! research/installs.md 7), and whether Battle.net's own client or Agent runs, by which the Dashboard starts no Agent
//! of its own (`agent`). Windows only; anywhere else nothing runs or listens.
use std::path::PathBuf;

/// A port something listens on over IPv4, and the program listening: its full path, where Windows gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    pub program: Option<PathBuf>,
}

/// A running process: its id, its program's file name, and its full path where Windows gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub program: Option<PathBuf>,
}

/// The full path of process `pid`'s program, where it may be asked.
#[cfg(windows)]
fn program_of(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: plain calls; the handle is closed before returning, and the name is read into a buffer of its size.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut name = vec![0u16; 1024];
        let mut length = name.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, name.as_mut_ptr(), &mut length);
        CloseHandle(process);
        (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&name[..length as usize])))
    }
}

/// Every IPv4 TCP port listened on, with its process.
#[cfg(windows)]
pub fn all() -> Vec<Listener> {
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    const AF_INET: u32 = 2;

    let mut size: u32 = 0;
    // SAFETY: a null table with a size of 0 asks only for the size needed.
    let first =
        unsafe { GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    if first != ERROR_INSUFFICIENT_BUFFER && first != NO_ERROR {
        return Vec::new();
    }
    // Room for a few more rows than asked, in case the table grows between the two calls; u32s keep it aligned.
    let mut buffer = vec![0u32; (size as usize + 16 * std::mem::size_of::<MIB_TCPROW_OWNER_PID>()) / 4 + 1];
    let mut size = (buffer.len() * 4) as u32;
    // SAFETY: the buffer is `size` bytes, writable and aligned for the table.
    let got = unsafe {
        GetExtendedTcpTable(buffer.as_mut_ptr().cast(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_LISTENER, 0)
    };
    if got != NO_ERROR {
        return Vec::new();
    }
    let table = buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
    // SAFETY: Windows wrote a table of `dwNumEntries` rows into the buffer, the rows following the count.
    let rows = unsafe {
        let count = (*table).dwNumEntries as usize;
        std::slice::from_raw_parts((*table).table.as_ptr(), count)
    };
    rows.iter()
        .map(|row| Listener {
            // The port is in network order in the low 16 bits.
            port: u16::from_be(row.dwLocalPort as u16),
            pid: row.dwOwningPid,
            program: program_of(row.dwOwningPid),
        })
        .collect()
}

#[cfg(not(windows))]
pub fn all() -> Vec<Listener> {
    Vec::new()
}

/// Every process running, by a snapshot of the process list.
#[cfg(windows)]
pub fn processes() -> Vec<Process> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let mut found = Vec::new();
    // SAFETY: a snapshot walked with an entry of the size it says, then closed.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return found;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            let pid = entry.th32ProcessID;
            if pid != 0 {
                found.push(Process { pid, name, program: program_of(pid) });
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    found
}

#[cfg(not(windows))]
pub fn processes() -> Vec<Process> {
    Vec::new()
}

/// Windows' language for the user's interface, as a locale name (`en-US`), where it says.
#[cfg(windows)]
pub fn ui_language() -> Option<String> {
    use windows_sys::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut name = [0u16; 85];
    // SAFETY: the language id is widened to an LCID (its sort bits 0), and the name read into a buffer of its size.
    let length = unsafe { LCIDToLocaleName(u32::from(GetUserDefaultUILanguage()), name.as_mut_ptr(), 85, 0) };
    (length > 1).then(|| String::from_utf16_lossy(&name[..length as usize - 1]))
}

#[cfg(not(windows))]
pub fn ui_language() -> Option<String> {
    None
}
