//! Windows: named pipes opened for overlapped I/O, so that one thread can wait on a read while another writes (on a
//! handle opened for synchronous I/O the two would wait on each other), as .NET's `PipeOptions.Asynchronous` does
//! for the Bundle; and DPAPI.
use std::ffi::c_void;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use crate::ipc::Stream;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `\\.\pipe\<name>`.
pub fn pipe_path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

/// One end of a named pipe, opened for overlapped I/O. Reads and writes may come from different threads at once.
pub struct Pipe {
    handle: HANDLE,
    /// Set as it is shut: a read begun after that is cancelled by its own thread.
    shut: AtomicBool,
}

// The handle is the kernel's, used through calls that are safe from any thread.
unsafe impl Send for Pipe {}
unsafe impl Sync for Pipe {}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

/// An event for one overlapped call, closed with it.
struct Event(HANDLE);

impl Event {
    fn new() -> io::Result<Event> {
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Event(event))
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl Pipe {
    /// The client's end of the pipe `name`: an error at once where there is no such pipe, or every instance of it
    /// is busy.
    pub fn connect(name: &str) -> io::Result<Pipe> {
        let path = wide(&pipe_path(name));
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Pipe { handle, shut: AtomicBool::new(false) })
    }

    /// A new instance of the server's end of the pipe `name`, for one client: [`Pipe::accept`] waits for it.
    pub fn listen(name: &str) -> io::Result<Pipe> {
        let path = wide(&pipe_path(name));
        let handle = unsafe {
            CreateNamedPipeW(
                path.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                std::ptr::null(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Pipe { handle, shut: AtomicBool::new(false) })
    }

    /// Waits for a client, looking at `stop` every tenth of a second: false when it is set first.
    pub fn accept(&self, stop: &AtomicBool) -> io::Result<bool> {
        let event = Event::new()?;
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = event.0;
        if unsafe { ConnectNamedPipe(self.handle, &mut overlapped) } != 0 {
            return Ok(true);
        }
        match unsafe { GetLastError() } {
            ERROR_PIPE_CONNECTED => return Ok(true),
            ERROR_IO_PENDING => {}
            error => return Err(io::Error::from_raw_os_error(error as i32)),
        }
        loop {
            if unsafe { WaitForSingleObject(event.0, 100) } == WAIT_OBJECT_0 {
                let mut got = 0;
                if unsafe { GetOverlappedResult(self.handle, &overlapped, &mut got, 1) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                return Ok(true);
            }
            if stop.load(Ordering::SeqCst) {
                unsafe { CancelIoEx(self.handle, &overlapped) };
                let mut got = 0;
                unsafe { GetOverlappedResult(self.handle, &overlapped, &mut got, 1) };
                return Ok(false);
            }
        }
    }

    /// One overlapped call, waited for: what it moved, 0 for the pipe's end.
    fn io(&self, start: impl FnOnce(*mut OVERLAPPED) -> i32) -> io::Result<usize> {
        let event = Event::new()?;
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = event.0;
        if start(&mut overlapped) == 0 {
            match unsafe { GetLastError() } {
                ERROR_IO_PENDING => {}
                ERROR_BROKEN_PIPE | ERROR_NO_DATA => return Ok(0),
                error => return Err(io::Error::from_raw_os_error(error as i32)),
            }
        }
        // Shut while this was being begun: the closer's cancel may have come before it, so it cancels itself.
        if self.shut.load(Ordering::SeqCst) {
            unsafe { CancelIoEx(self.handle, &overlapped) };
        }
        let mut moved = 0u32;
        if unsafe { GetOverlappedResult(self.handle, &overlapped, &mut moved, 1) } == 0 {
            return match unsafe { GetLastError() } {
                // Cancelled by `shut`: the pipe's end, as on a Unix socket shut down.
                _ if self.shut.load(Ordering::SeqCst) => Ok(0),
                ERROR_BROKEN_PIPE | ERROR_NO_DATA => Ok(0),
                error => Err(io::Error::from_raw_os_error(error as i32)),
            };
        }
        Ok(moved as usize)
    }
}

impl Stream for Pipe {
    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        if self.shut.load(Ordering::SeqCst) {
            return Ok(0);
        }
        let len = buf.len().min(u32::MAX as usize) as u32;
        let ptr = buf.as_mut_ptr();
        self.io(|overlapped| unsafe { ReadFile(self.handle, ptr, len, std::ptr::null_mut(), overlapped) })
    }

    fn write_all(&self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            if self.shut.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let len = buf.len().min(u32::MAX as usize) as u32;
            let ptr = buf.as_ptr();
            let wrote =
                self.io(|overlapped| unsafe { WriteFile(self.handle, ptr, len, std::ptr::null_mut(), overlapped) })?;
            if wrote == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            buf = &buf[wrote..];
        }
        Ok(())
    }

    fn shut(&self) {
        self.shut.store(true, Ordering::SeqCst);
        unsafe { CancelIoEx(self.handle, std::ptr::null()) };
    }
}

/// DPAPI for the current Windows user: `CryptProtectData`, with no prompt.
pub struct Dpapi;

fn blob_of(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
}

/// The bytes DPAPI gave, copied out and its memory freed.
fn taken(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe { LocalFree(blob.pbData as *mut c_void) };
    bytes
}

impl crate::secrets::Protect for Dpapi {
    fn protect(&self, plain: &[u8]) -> io::Result<Vec<u8>> {
        let input = blob_of(plain);
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let done = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(taken(output))
    }

    fn unprotect(&self, protected: &[u8]) -> io::Result<Vec<u8>> {
        let input = blob_of(protected);
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let done = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if done == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(taken(output))
    }

    fn name(&self) -> &'static str {
        "encrypted for this Windows user with DPAPI"
    }
}
