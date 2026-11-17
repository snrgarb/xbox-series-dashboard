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
