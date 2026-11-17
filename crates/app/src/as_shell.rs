//! The Dashboard's side of shell mode (docs/spec/shell-mode.md, 6): one Dashboard at a time; under `--from-shell`
//! its log in the data folder, the two events for the --shell process and the test's crash on purpose; and
//! Settings > System's asks, carried out off the frame's thread.
use std::path::PathBuf;
use std::sync::mpsc;

use model::shell_mode::ShellMode;
use shell_mode::dashboard::{self, Instance, Signals};

pub struct AsShell {
    from_shell: bool,
    dry: bool,
    data: Option<PathBuf>,
    signals: Option<Signals>,
    /// The test's crash on purpose, right after the first frame.
    crash: Option<u32>,
    _guard: Option<dashboard::Guard>,
    asked: Option<mpsc::Receiver<ShellMode>>,
    /// An update's new Dashboard: the pid of the one it takes over from, told of the first frame.
    handover: Option<u32>,
}

impl AsShell {
    /// As the Dashboard starts, before anything else: a second Dashboard brings the first forward and ends here.
    /// `own_log`: under --from-shell without --log-file, the log goes to `<data>\shell\logs\`.
    /// `handover`: an update's new Dashboard (`--handover <pid>`), which takes the guard alongside the old one's.
    pub fn start(
        from_shell: bool,
        own_log: bool,
        dry: bool,
        data_dir: Option<PathBuf>,
        data: Option<PathBuf>,
        handover: Option<u32>,
    ) -> Self {
        let shell_folder = data.as_ref().map(|data| data.join("shell"));
        if own_log {
            if let Some(folder) = &shell_folder {
                dashboard::log_to_file(&folder.join("logs"));
            }
        }
        let guard = match handover {
            Some(old) => {
                eprintln!(
                    "update: taking over from pid {old}, an update's earlier version, at {:.0} ms",
                    crate::since_start()
                );
                Instance::First(dashboard::taking_over(data_dir.as_deref()))
            }
            None => dashboard::single_instance(data_dir.as_deref()),
        };
        let guard = match guard {
            Instance::First(guard) => guard,
            Instance::Second => {
                let brought = dashboard::bring_first_forward();
                eprintln!("dashboard: one runs already; brought it forward: {brought}");
                std::process::exit(shell_mode::contract::NOT_A_DASHBOARD as i32);
            }
        };
        // With --dry-shell (a test's --from-shell, never the shell's own start) the events are left alone, whoever
        // made them, and the crash marker isn't read: only the window's path is tried.
        let real = from_shell && !dry;
        let signals = if real { Signals::open() } else { None };
        let crash = if real { shell_folder.as_deref().and_then(dashboard::crash_on_purpose) } else { None };
        if from_shell {
            eprintln!(
                "shell: started by the shell; {}",
                if dry {
                    "--dry-shell: the events are left alone, and only the window's path is tried"
                } else if signals.is_some() {
                    "telling it of the first frame and the Boot video's end"
                } else {
                    "no --shell process to tell"
                }
            );
        }
        if let Some(left) = crash {
            eprintln!("shell: the test asks for a crash after the first frame ({left} more after this one)");
        }
        if dry {
            eprintln!("shell: --dry-shell: Settings > System says what it would write and writes nothing");
        }
        Self { from_shell, dry, data, signals, crash, _guard: Some(guard), asked: None, handover }
    }

    /// The first frame is on the screen: the --shell process may take its black down.
    pub fn first_frame(&mut self) {
        // An update's new Dashboard: the old one exits now.
        if let Some(old) = self.handover.take() {
            let told = dashboard::handed_over(old);
            eprintln!("update: first frame at {:.0} ms; told pid {old}: {told}", crate::since_start());
        }
        if let Some(signals) = &mut self.signals {
            if signals.first_frame() {
                eprintln!("shell: FirstFrame set at {:.0} ms", crate::since_start());
            }
        }
        if self.crash.take().is_some() {
            eprintln!("shell: crashing on purpose, as the test asked");
            std::process::exit(0x5844_0DEA_u32 as i32);
        }
    }

    /// The Boot video is over, or there was none: the --shell process starts explorer behind the Dashboard.
    pub fn boot_video_ended(&mut self) {
        if let Some(signals) = &mut self.signals {
            if signals.boot_video_ended() {
                eprintln!("shell: BootVideoEnded set at {:.0} ms", crate::since_start());
            }
        }
    }

    /// Whether Start-up stays over everything (topmost): only under --from-shell, where explorer starts behind it.
    pub fn topmost_start_up(&self) -> bool {
        self.from_shell
    }

    /// Asks where shell mode stands, or for a change, on a thread of its own; `wake` once it is answered.
    pub fn ask(&mut self, change: Option<bool>, wake: impl Fn() + Send + 'static) {
        let Some(data) = self.data.clone() else { return };
        let dry = self.dry;
        let (send, answer) = mpsc::channel();
        self.asked = Some(answer);
        std::thread::spawn(move || {
            let shell = match change {
                Some(on) => shell_mode::settings_change(on, &data, dry),
                None => shell_mode::settings_status(&data, dry),
            };
            let _ = send.send(shell);
            wake();
        });
    }

    /// Where shell mode stands, once a thread has answered.
    pub fn answer(&mut self) -> Option<ShellMode> {
        let shell = self.asked.as_ref()?.try_recv().ok()?;
        self.asked = None;
        Some(shell)
    }
}
