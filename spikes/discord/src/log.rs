//! The log: one line at a time, appended to the file `--log` names and written to stderr, each with the UTC time.
//! Callers never hand it a name, a message, a password, a token or a whole address (`redact` makes the addresses).
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn open(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file),
        Err(e) => eprintln!("cannot open the log {}: {e}", path.display()),
    }
}

pub fn line(message: &str) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let text = format!("{} {message}\n", stamp(now.as_secs(), now.subsec_millis()));
    eprint!("{text}");
    if let Some(file) = FILE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = file.write_all(text.as_bytes());
        let _ = file.flush();
    }
}

/// `2026-11-18T12:34:56.789Z`.
fn stamp(secs: u64, millis: u32) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z", rest / 3600, rest / 60 % 60, rest % 60)
}

#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::log::line(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn stamps_are_utc_dates() {
        assert_eq!(super::stamp(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(super::stamp(1_795_000_000, 7), "2026-11-18T11:06:40.007Z");
        assert_eq!(super::stamp(951_782_400, 999), "2000-02-29T00:00:00.999Z");
    }
}
