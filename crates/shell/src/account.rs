//! The Windows user the Dashboard runs as, as the first Profile is made from them (docs/spec/profiles.md): their
//! name, and their account picture where Windows keeps one, read with nothing more than the registry.
use std::path::PathBuf;

/// The user's name: Windows' `USERNAME` (elsewhere `USER`), or "Player" with none.
pub fn name() -> String {
    let var = if cfg!(windows) { "USERNAME" } else { "USER" };
    std::env::var(var)
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Player".into())
}

/// The user's account picture at 192 pixels, as Windows records it for the user under
/// `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\AccountPicture\Users\<their SID>`, `Image192`, if they set one
/// and the file is there. None anywhere but Windows, for a user who never set one (Windows' default picture is not
/// taken: the first plain picture is drawn in its likeness), and on any failure.
pub fn picture() -> Option<PathBuf> {
    windows::picture().filter(|path| path.is_file())
}

#[cfg(windows)]
mod windows {
    use std::path::PathBuf;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// The process's user's SID, as text (`S-1-5-21-...`).
    fn sid() -> Option<String> {
        let mut token: HANDLE = null_mut();
        // SAFETY: the process's own pseudo-handle; the token is closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return None;
        }
        let mut buffer = vec![0u64; 64];
        let mut length = 0u32;
        // SAFETY: the buffer is as long as said, and aligned for TOKEN_USER.
        let ok = unsafe {
            GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), (buffer.len() * 8) as u32, &mut length)
        };
        // SAFETY: the token was opened above.
        unsafe { CloseHandle(token) };
        if ok == 0 {
            return None;
        }
        // SAFETY: GetTokenInformation filled the buffer with a TOKEN_USER.
        let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        let mut text: *mut u16 = null_mut();
        // SAFETY: the SID is the token's, in the buffer; the text is freed below.
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 || text.is_null() {
            return None;
        }
        // SAFETY: a NUL-terminated string Windows made.
        let sid = unsafe {
            let length = (0..).take_while(|&i| *text.add(i) != 0).count();
            String::from_utf16_lossy(std::slice::from_raw_parts(text, length))
        };
        // SAFETY: made by ConvertSidToStringSidW, to be freed with LocalFree.
        unsafe { LocalFree(text.cast()) };
        Some(sid)
    }

    pub fn picture() -> Option<PathBuf> {
        let key = wide(&format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\AccountPicture\Users\{}", sid()?));
        let name = wide("Image192");
        let mut value = vec![0u16; 1024];
        let mut size = (value.len() * 2) as u32;
        // SAFETY: the buffer is as long as said in bytes; the key and value names are NUL-terminated.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                value.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != 0 {
            return None;
        }
        let length = value.iter().position(|&c| c == 0).unwrap_or(value.len());
        let path = String::from_utf16_lossy(&value[..length]);
        (!path.trim().is_empty()).then(|| PathBuf::from(path.trim()))
    }
}

#[cfg(not(windows))]
mod windows {
    pub fn picture() -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn there_is_always_a_name() {
        assert!(!super::name().is_empty());
        if cfg!(not(windows)) {
            assert_eq!(super::picture(), None);
        }
    }
}
