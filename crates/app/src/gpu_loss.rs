//! What the window decides when the GPU device is lost (a driver reset, a GPU removed), with no GPU in it: when a
//! run of surface errors is taken for a lost device, and how long to wait before each try at rebuilding
//! (docs/spec/shell-mode.md, 6).
use std::time::Duration;

/// Waits before each try at rebuilding the GPU, in order. After the last has failed the Dashboard ends (exit 2, which
/// the watchdog counts as a crash and backs off from).
pub const RETRY_WAITS: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(5),
    Duration::from_secs(5),
];

/// How long to wait before try number `attempt` (from 0), or None when there are no more tries.
pub fn wait_before(attempt: usize) -> Option<Duration> {
    RETRY_WAITS.get(attempt).copied()
}

/// Consecutive frames the surface may answer "validation error" before it is taken for a lost device that wgpu has
/// not (yet) said is lost.
pub const VALIDATION_LIMIT: u32 = 3;

/// Counts surface validation errors in a row. A frame that was had resets it.
#[derive(Debug, Default)]
pub struct ValidationRun(u32);

impl ValidationRun {
    /// A frame's texture was had.
    pub fn frame_had(&mut self) {
        self.0 = 0;
    }

    /// The surface gave a validation error: whether that is the `VALIDATION_LIMIT`th in a row, which is taken for a
    /// lost device. The count starts again after it.
    pub fn error(&mut self) -> bool {
        self.0 += 1;
        if self.0 >= VALIDATION_LIMIT {
            self.0 = 0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waits_are_one_two_then_five_seconds_and_then_there_are_no_more() {
        let secs: Vec<u64> = (0..).map_while(wait_before).map(|d| d.as_secs()).collect();
        assert_eq!(secs, [1, 2, 5, 5, 5]);
        assert_eq!(wait_before(RETRY_WAITS.len()), None);
    }

    #[test]
    fn three_validation_errors_in_a_row_are_a_lost_device() {
        let mut run = ValidationRun::default();
        assert!(!run.error());
        assert!(!run.error());
        assert!(run.error());
        // Counted from nothing again.
        assert!(!run.error());
    }

    #[test]
    fn a_frame_that_was_had_ends_the_run() {
        let mut run = ValidationRun::default();
        assert!(!run.error());
        assert!(!run.error());
        run.frame_had();
        assert!(!run.error());
        assert!(!run.error());
        assert!(run.error());
    }
}
