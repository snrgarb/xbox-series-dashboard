//! Running a helper program (legendary, gogdl; docs/spec/helpers.md 4): a job started with its arguments, no
//! window of its own on Windows, its output read line by line on threads of its own, never the UI thread's, and
//! handed over through a channel; and a way to stop it. What a line means is the store's (#175, #176): here a line
//! is a line.
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// How long, once a job has ended, what it wrote is waited for before its end is said.
const DRAIN: std::time::Duration = std::time::Duration::from_secs(1);

/// What a job says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A line of its standard output, without its line ending.
    Out(String),
    /// A line of its standard error: legendary and gogdl write their progress there.
    Err(String),
    /// It ended, with its exit code where it has one (none when it was stopped, or killed by a signal). Everything
    /// it wrote comes before; a line from a process it started that outlives it may come after.
    Ended(Option<i32>),
}

/// A helper job running.
pub struct Job {
    child: Arc<Mutex<Child>>,
    said: Receiver<Said>,
    pid: u32,
}

impl Job {
    /// Starts `program` with `args`, in its own folder. `wake` is called after every line and at the end, so a
    /// sleeping UI thread can look.
    pub fn start(program: &Path, args: &[String], wake: impl Fn() + Send + Sync + 'static) -> std::io::Result<Job> {
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(folder) = program.parent().filter(|f| !f.as_os_str().is_empty()) {
            command.current_dir(folder);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        }
        let mut child = command.spawn()?;
        let pid = child.id();
        let (tell, said) = mpsc::channel();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let out = child.stdout.take().map(|out| read_lines(out, tell.clone(), wake.clone(), Said::Out));
        let err = child.stderr.take().map(|err| read_lines(err, tell.clone(), wake.clone(), Said::Err));
        let child = Arc::new(Mutex::new(child));
        let waited = child.clone();
        // The end is said once the process has ended and what it wrote is read: its outputs to their ends, or for a
        // moment more where a process it started holds them open after it, which would otherwise keep its end
        // unsaid for as long as that one runs.
        std::thread::spawn(move || {
            let code = loop {
                match waited.lock().unwrap().try_wait() {
                    Ok(Some(status)) => break status.code(),
                    Ok(None) => {}
                    Err(_) => break None,
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            };
            let until = std::time::Instant::now() + DRAIN;
            let readers: Vec<_> = [out, err].into_iter().flatten().collect();
            while readers.iter().any(|reader| !reader.is_finished()) && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let _ = tell.send(Said::Ended(code));
            wake();
        });
        Ok(Job { child, said, pid })
    }

    /// Its process id, for the log.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// What it has said since this was last asked, never waiting.
    pub fn said(&self) -> Vec<Said> {
        self.said.try_iter().collect()
    }

    /// Waits for the next thing it says, up to `timeout`; None when it said nothing, or has nothing more to say.
    pub fn next(&self, timeout: std::time::Duration) -> Option<Said> {
        self.said.recv_timeout(timeout).ok()
    }

    /// Stops it: the process is killed (a helper keeps what it has downloaded and resumes from it, as the stores'
    /// own pause does). Its end is said as any end is. Stopping one that has ended does nothing.
    pub fn stop(&self) {
        let mut child = self.child.lock().unwrap();
        if matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
        }
    }
}

impl Drop for Job {
    /// A job no one holds is not left running.
    fn drop(&mut self) {
        self.stop();
    }
}

/// Reads `from` a line at a time on a thread of its own, saying each as `kind` says it. A line that is not UTF-8
/// is said as nearly as it can be. Carriage returns end a line too: a progress bar rewrites its line with them.
fn read_lines(
    from: impl Read + Send + 'static,
    tell: Sender<Said>,
    wake: Arc<dyn Fn() + Send + Sync>,
    kind: fn(String) -> Said,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(from);
        let mut line = Vec::new();
        loop {
            line.clear();
            // Up to the next '\n', and split on any '\r' inside it.
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            for part in line.split(|b| *b == b'\n' || *b == b'\r').filter(|part| !part.is_empty()) {
                if tell.send(kind(String::from_utf8_lossy(part).into_owned())).is_err() {
                    return;
                }
                wake();
            }
        }
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    fn until_end(job: &Job) -> Vec<Said> {
        let mut said = Vec::new();
        while let Some(s) = job.next(Duration::from_secs(10)) {
            let end = matches!(s, Said::Ended(_));
            said.push(s);
            if end {
                break;
            }
        }
        said
    }

    #[test]
    fn a_jobs_lines_come_one_by_one_from_both_outputs_then_its_end() {
        let script = "printf 'one\\ntwo\\r50%%\\r' ; printf 'progress\\n' >&2 ; exit 3";
        let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = woken.clone();
        let wake = move || {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        };
        let job = Job::start(Path::new("/bin/sh"), &["-c".into(), script.into()], wake).unwrap();
        let said = until_end(&job);
        let out: Vec<_> = said.iter().filter(|s| matches!(s, Said::Out(_))).cloned().collect();
        assert_eq!(out, [Said::Out("one".into()), Said::Out("two".into()), Said::Out("50%".into())]);
        assert!(said.contains(&Said::Err("progress".into())));
        assert_eq!(said.last(), Some(&Said::Ended(Some(3))));
        assert!(woken.load(std::sync::atomic::Ordering::Relaxed) >= 5);
    }

    #[test]
    fn a_job_stopped_ends_with_no_code() {
        let job = Job::start(Path::new("/bin/sh"), &["-c".into(), "echo started; sleep 30".into()], || {}).unwrap();
        assert_eq!(job.next(Duration::from_secs(10)), Some(Said::Out("started".into())));
        job.stop();
        assert_eq!(until_end(&job), [Said::Ended(None)]);
        job.stop();
    }
}
