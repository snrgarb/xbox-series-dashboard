//! Which programs listen on which ports of this machine: how the Agent's port is found, as barncastle's
//! Battle.Net-Installer and HSCentric find it (`GetExtendedTcpTable` for the Agent's process; docs/research/
//! installs.md 7). Windows only; anywhere else nothing listens.
use std::path::PathBuf;

/// A port something listens on over IPv4, and the program listening: its full path, where Windows gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    pub program: Option<PathBuf>,
}

/// Every IPv4 TCP port listened on, with its process.
#[cfg(windows)]
pub fn all() -> Vec<Listener> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
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
    let program = |pid: u32| -> Option<PathBuf> {
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
    };
    rows.iter()
        .map(|row| Listener {
            // The port is in network order in the low 16 bits.
            port: u16::from_be(row.dwLocalPort as u16),
            pid: row.dwOwningPid,
            program: program(row.dwOwningPid),
        })
        .collect()
}

#[cfg(not(windows))]
pub fn all() -> Vec<Listener> {
    Vec::new()
}
