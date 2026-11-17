//! Every log line in one write. The Dashboard logs with `eprintln!`, and its stderr may be a file another Dashboard
//! appends to as well: an update's new one shares the old one's `--log-file` (docs/spec/updates.md). std's
//! `eprintln!` writes a line in pieces (each literal and each argument, then the newline), so another process's line
//! can land between them. This one formats the whole line first and writes it, newline and all, at once; on a file
//! opened for appending (FILE_APPEND_DATA, O_APPEND) the system puts each write at the end, whole.
//!
//! A crate that logs shadows std's macro at its root, before its modules, so they all take this one:
//!
//! ```ignore
//! #[allow(unused_macros)]
//! macro_rules! eprintln { ($($arg:tt)*) => { log_line::eprintln!($($arg)*) } }
//! ```

use std::fmt;
use std::io::{self, Write};

/// `eprintln!`, the line written to stderr in one write.
#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::write_line(::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::write_line(::std::format_args!($($arg)*))
    };
}

/// The line and its newline to stderr in one write. A stderr that is gone takes nothing, as std's does.
pub fn write_line(args: fmt::Arguments) {
    let _ = line_to(&mut io::stderr().lock(), args);
}

/// The line and its newline to `out` in one write.
pub fn line_to(out: &mut impl Write, args: fmt::Arguments) -> io::Result<()> {
    let mut line = fmt::format(args);
    line.push('\n');
    out.write_all(line.as_bytes())
}

/// A panic's message (and its backtrace when one is asked for) in one write, as the lines above; std's own hook
/// writes it in pieces.
pub fn panics_in_one_write() {
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let backtrace = std::backtrace::Backtrace::capture();
        let tail = match backtrace.status() {
            std::backtrace::BacktraceStatus::Captured => format!("\nstack backtrace:\n{backtrace}"),
            _ => "\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace".into(),
        };
        write_line(format_args!("thread '{name}' {info}{tail}"));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;

    /// Two writers, each with its own handle opened for appending (as two processes have), write lines of many
    /// lengths at once: every line in the file is one of theirs, whole, and none is missing.
    #[test]
    fn two_writers_appending_at_once_never_tear_a_line() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("shared.log");
        const LINES: usize = 3000;
        let writers: Vec<_> = ["first", "second"]
            .into_iter()
            .map(|who| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let mut file = OpenOptions::new().create(true).append(true).open(&path).unwrap();
                    for n in 0..LINES {
                        let filler = "x".repeat(n % 300);
                        line_to(&mut file, format_args!("{who} {n} {filler} end")).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.ends_with('\n'));
        let mut seen = [0usize; 2];
        for line in text.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            let torn = || -> ! { panic!("a torn line: {line:?}") };
            let [who, n, filler, "end"] = parts[..] else { torn() };
            let n: usize = n.parse().unwrap_or_else(|_| torn());
            if filler.len() != n % 300 || filler.chars().any(|c| c != 'x') {
                torn();
            }
            match who {
                "first" => seen[0] += 1,
                "second" => seen[1] += 1,
                _ => torn(),
            }
        }
        assert_eq!(seen, [LINES, LINES]);
    }

    #[test]
    fn a_line_is_its_text_and_a_newline() {
        let mut out = Vec::new();
        let name = "pid";
        line_to(&mut out, format_args!("update: {name} {}", 7)).unwrap();
        line_to(&mut out, format_args!("")).unwrap();
        assert_eq!(out, b"update: pid 7\n\n");
    }
}
