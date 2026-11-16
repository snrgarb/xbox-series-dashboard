//! The Windows clipboard's text, for Settings > Discord's Paste (docs/spec/settings.md, 6.9): `Some("")` for a
//! clipboard with no text (Paste then forgets the field, as the Bundle's does), None while another program holds it
//! open (Paste then pastes nothing). Anywhere but Windows there is no clipboard: None.

/// The clipboard's text as it is now.
pub fn text() -> Option<String> {
    #[cfg(windows)]
    return windows::text();
    #[cfg(not(windows))]
    None
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    /// CF_UNICODETEXT: UTF-16, ended by a nul.
    const UNICODE_TEXT: u32 = 13;

    pub fn text() -> Option<String> {
        // SAFETY: the clipboard is opened for this thread and closed before returning; the memory it hands over is
        // read only while locked, up to its nul.
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                eprintln!("clipboard: another program has it open: nothing pasted");
                return None;
            }
            let mut text = String::new();
            if IsClipboardFormatAvailable(UNICODE_TEXT) != 0 {
                let handle = GetClipboardData(UNICODE_TEXT);
                if !handle.is_null() {
                    let at = GlobalLock(handle) as *const u16;
                    if !at.is_null() {
                        let mut length = 0;
                        while *at.add(length) != 0 {
                            length += 1;
                        }
                        text = String::from_utf16_lossy(std::slice::from_raw_parts(at, length));
                        GlobalUnlock(handle);
                    }
                }
            }
            CloseClipboard();
            Some(text)
        }
    }
}
