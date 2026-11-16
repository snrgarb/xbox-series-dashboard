//! Shell mode's logs: each line to stderr, and to a file in the data folder's `shell\logs\` once one is open. The
//! --shell process has no console at sign-in, so its file is the only record of a sign-in; a file is opened per
//! sign-in and the newest 30 are kept.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

/// How many of one kind of log are kept.
pub const KEEP: usize = 30;

pub struct Log {
    started: Instant,
    /// Lines said before the file opened, written to it as it does.
    early: Vec<String>,
    file: Option<File>,
    kind: &'static str,
}

impl Log {
    pub fn new(kind: &'static str) -> Self {
        Self { started: Instant::now(), early: Vec::new(), file: None, kind }
    }

    /// Says a line: as it is to stderr, and in the file after the time and the time since the start
    /// (`14:30:05.123 +1234 ms <text>`).
    pub fn line(&mut self, text: &str) {
        eprintln!("{text}");
        let line = format!(
            "{} +{} ms {text}",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            self.started.elapsed().as_millis()
        );
        match &mut self.file {
            Some(file) => {
                let _ = writeln!(file, "{line}");
            }
            None => self.early.push(line),
        }
    }

    /// Opens `<folder>\<kind>-<stamp>.log` for this run, and removes all but the newest [`KEEP`] of its kind.
    pub fn open(&mut self, folder: &Path) {
        let name = format!("{}-{}.log", self.kind, crate::setup::stamp());
        self.open_file(folder, &folder.join(name), false);
        prune(folder, self.kind, KEEP);
    }

    /// Appends to one file, whatever the run.
    pub fn append(&mut self, file: &Path) {
        if let Some(folder) = file.parent() {
            self.open_file(folder, file, true);
        }
    }

    fn open_file(&mut self, folder: &Path, path: &Path, append: bool) {
        let opened = std::fs::create_dir_all(folder)
            .and_then(|_| OpenOptions::new().create(true).append(append).write(true).truncate(!append).open(path));
        match opened {
            Ok(mut file) => {
                for line in self.early.drain(..) {
                    let _ = writeln!(file, "{line}");
                }
                self.file = Some(file);
            }
            Err(e) => {
                let text = format!("couldn't open the log {}: {e}", path.display());
                self.line(&text);
            }
        }
    }
}

/// Removes the oldest `<kind>-*.log` in `folder` beyond the newest `keep`, by name (the stamp sorts by time).
pub fn prune(folder: &Path, kind: &str, keep: usize) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let prefix = format!("{kind}-");
    let mut logs: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with(&prefix) && name.ends_with(".log")
        })
        .collect();
    logs.sort();
    let over = logs.len().saturating_sub(keep);
    for old in &logs[..over] {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_logs_of_a_kind_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("shell-2026111{i}-000000.log")), "").unwrap();
        }
        std::fs::write(dir.path().join("dashboard-20261110-000000.log"), "").unwrap();
        prune(dir.path(), "shell", 3);
        let mut left: Vec<String> =
            std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(
            left,
            [
                "dashboard-20261110-000000.log",
                "shell-20261112-000000.log",
                "shell-20261113-000000.log",
                "shell-20261114-000000.log"
            ]
        );
    }

    #[test]
    fn lines_said_before_the_file_opens_are_in_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = Log::new("shell");
        log.line("black up");
        log.open(dir.path());
        log.line("events made");
        let file = std::fs::read_dir(dir.path()).unwrap().flatten().next().unwrap().path();
        let text = std::fs::read_to_string(file).unwrap();
        assert!(text.contains(" ms black up\n") && text.contains(" ms events made\n"), "{text}");
    }
}
