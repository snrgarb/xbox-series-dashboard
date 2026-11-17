//! What the two halves of shell mode agree on: the --shell process of one build and a Dashboard of another must
//! still understand each other, as an update leaves the --shell process running the build it started with
//! (docs/research/shell.md, 3). A change that breaks this goes with a new number in the event names.

/// The contract's number, in the events' names.
pub const VERSION: u32 = 1;

/// Set by the Dashboard started `--from-shell` when Start-up's first frame is presented: the black comes down.
pub const FIRST_FRAME: &str = r"Local\XboxSeriesDashboard.v1.FirstFrame";
/// Set at the Boot video's end, skip, failure or 20 s stop, or with the first frame when there is none: explorer
/// starts.
pub const BOOT_VIDEO_ENDED: &str = r"Local\XboxSeriesDashboard.v1.BootVideoEnded";
/// Held by the --shell process for the session: what the sentinel looks for.
pub const SHELL_MUTEX: &str = r"Local\XboxSeriesDashboard.Shell";
/// Held by a running Dashboard: the single-instance guard (with a data folder of its own, a name of its own).
pub const RUNNING_MUTEX: &str = r"Local\XboxSeriesDashboard.Running";

/// The arguments.
pub const SHELL: &str = "--shell";
pub const SENTINEL: &str = "--sentinel";
pub const PROBE: &str = "--shell-probe";
pub const ON: &str = "--shell-on";
pub const OFF: &str = "--shell-off";
pub const DRY: &str = "--dry-shell";
pub const AS_INSTALLED: &str = "--as-installed";
/// The Dashboard was started by the --shell process (or the sentinel): it sets the events, and Start-up's window is
/// topmost.
pub const FROM_SHELL: &str = "--from-shell";

/// The Dashboard's logging switches, which `--shell` passes on to the Dashboard it starts `--from-shell`
/// (`dashboard --shell --log-focus`), so that the live shell's Dashboard can log as any other.
pub const PASSED_ON: [&str; 4] = ["--log-focus", "--log-sounds", "--log-frames", "--log-queue-sources"];

/// What the Dashboard started `--from-shell` is given, after `--shell`'s own `args`: [`FROM_SHELL`], then each of
/// [`PASSED_ON`] that `args` has, once, in the order given. Anything else in `args` is not passed on.
pub fn from_shell_args(args: &[String]) -> Vec<String> {
    let mut out = vec![FROM_SHELL.to_string()];
    for arg in args {
        if PASSED_ON.contains(&arg.as_str()) && !out.contains(arg) {
            out.push(arg.clone());
        }
    }
    out
}

/// An update's new Dashboard, started by the one it replaces with that one's pid (docs/spec/updates.md): it takes
/// the single-instance guard alongside it, and sets [`handover_event`] once its first frame is presented.
pub const HANDOVER: &str = "--handover";

/// Made by the Dashboard handing over (its pid in the name) and set by the new one at its first frame: the old one
/// then exits 0, which the watchdog takes for a handover, another Dashboard being left.
pub fn handover_event(old_pid: u32) -> String {
    format!(r"Local\XboxSeriesDashboard.v{VERSION}.Handover.{old_pid}")
}

/// The exit code of every job of this exe that isn't a Dashboard (`--sentinel`, `--shell-on`'s probe, a second
/// Dashboard that gave way to the first, a second --shell): the --shell process watches every process of this exe
/// and counts these for nothing.
pub const NOT_A_DASHBOARD: u32 = 0x5844_0001;
/// `--shell-probe`'s exit code: this exe starts.
pub const PROBE_OK: u32 = 0x5844_0002;

#[cfg(test)]
mod passed_on {
    use super::*;

    #[test]
    fn the_logging_switches_are_passed_on_from_shell_and_nothing_else() {
        let given: Vec<String> =
            ["--log-focus", "--data-dir", r"C:\d", "--log-queue-sources", "--log-focus", "--dry-shell"]
                .map(String::from)
                .to_vec();
        assert_eq!(from_shell_args(&given), ["--from-shell", "--log-focus", "--log-queue-sources"]);
        assert_eq!(from_shell_args(&[]), ["--from-shell"]);
    }
}
