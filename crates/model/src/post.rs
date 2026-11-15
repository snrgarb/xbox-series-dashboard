//! How another program on the same desktop posts a notification to a running Dashboard: a `WM_COPYDATA` message
//! to the Toast strip's window, found by its class, carrying the text as UTF-8. The Dashboard lists it and shows
//! its Toast, if it was started with `--allow-toast-posts`: otherwise it takes nothing. `standin toast <text>`
//! does it for the tests; nothing else posts any (docs/spec/guide.md, 9.7).

/// The class of the window to send to. There is one such window per Dashboard, hidden unless a Toast shows.
pub const WINDOW_CLASS: &str = "XboxSeriesDashboardToasts";
/// `COPYDATASTRUCT::dwData` of a post: "TOST". Anything else is ignored.
pub const COPYDATA_ID: usize = 0x5453_4F54;
/// The longest text taken, in characters. A longer one is cut: a Toast's line is one line.
pub const MAX_CHARS: usize = 200;

/// A post's text as the Dashboard takes it: on one line, trimmed, and no longer than `MAX_CHARS`. None when
/// nothing is left.
pub fn clean(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let line: String = line.trim().chars().take(MAX_CHARS).collect();
    (!line.is_empty()).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_post_is_one_trimmed_line_of_a_bounded_length() {
        assert_eq!(clean(b"  Download complete \r\n").as_deref(), Some("Download complete"));
        assert_eq!(clean(b"two\nlines").as_deref(), Some("two lines"));
        assert_eq!(clean(b" \0\t "), None);
        assert_eq!(clean(b""), None);
        assert_eq!(clean("é".repeat(300).as_bytes()).map(|text| text.chars().count()), Some(MAX_CHARS));
        // Bytes that are not UTF-8 do not stop the rest being read.
        assert_eq!(clean(b"caf\xff").as_deref(), Some("caf\u{FFFD}"));
    }
}
