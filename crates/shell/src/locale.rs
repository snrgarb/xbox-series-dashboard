//! What the user's Windows region says of how to write things: their short date format, which every date the
//! Dashboard shows follows (`screens::date`, #237).

/// The user's short date format, as a Windows date picture ("d/MM/yyyy" for Australia, "M/d/yyyy" for the United
/// States): Windows' `LOCALE_SSHORTDATE` for the user's own locale, with what they set in Region over it. None
/// anywhere but Windows, and on any failure.
pub fn short_date_format() -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SSHORTDATE};
        let mut buffer = [0u16; 128];
        // SAFETY: a null locale name is the user's default locale; the buffer is as long as said.
        let written = unsafe { GetLocaleInfoEx(std::ptr::null(), LOCALE_SSHORTDATE, buffer.as_mut_ptr(), 128) };
        // What was written counts its closing NUL.
        let length = usize::try_from(written).ok()?.checked_sub(1)?;
        Some(String::from_utf16_lossy(&buffer[..length])).filter(|picture| !picture.trim().is_empty())
    }
    #[cfg(not(windows))]
    None
}
