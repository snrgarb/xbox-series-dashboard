//! What Manage Queue's rows and My games & apps' current download say of a game in the Queue, worded as the
//! Bundle words them (its `QueueEntry`, `StorageRing.Size` and `Speed`): its state, its speed, how much is down of
//! the whole, the share down, and how far its bar fills.
use model::{InstallState, Queued};

/// One game in the Queue, as its row reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    /// "Installing..." or "Queued".
    pub state: &'static str,
    /// "106 Mbps", "323 Kbps": none while it waits, or until its bytes have first moved.
    pub speed: Option<String>,
    /// "67.80 MB of 3.55 GB": none while it waits, or while its client does not know the whole yet.
    pub amount: Option<String>,
    /// "1.9%"; "0.0%" while it waits, whatever of it is down already, as on the console. None while it installs
    /// with no whole known (its client gives no count yet): the row says "Installing..." alone, as the Bundle's
    /// does for a store that gives no bytes, rather than a share that stands at 0.0% (#132).
    pub percent: Option<String>,
    /// How far the bar fills, 0 to 1.
    pub fraction: f32,
}

impl Figures {
    pub fn of(entry: &Queued) -> Self {
        match entry.state {
            InstallState::Downloading { done, total } => {
                let fraction = if total == 0 { 0.0 } else { (done as f64 / total as f64).clamp(0.0, 1.0) };
                Figures {
                    state: entry.state.status(),
                    speed: entry.speed.map(speed),
                    amount: (total > 0).then(|| format!("{} of {}", size(done, 2), size(total, 2))),
                    percent: (total > 0).then(|| percent(fraction)),
                    fraction: fraction as f32,
                }
            }
            // Waiting its turn, or paused in the client: the Bundle reads a download that is not active as Queued.
            InstallState::Queued | InstallState::Paused { .. } => Figures {
                state: entry.state.status(),
                speed: None,
                amount: None,
                percent: Some(percent(0.0)),
                fraction: 0.0,
            },
        }
    }
}

/// A size as the console shows it, to `decimals` places: in GB, in MB below one, in TB from 1,024 GB up, a GB being
/// 2^30 bytes. The unit goes by the size as rounded, so it never reads "1024.00 GB".
pub fn size(bytes: u64, decimals: usize) -> String {
    const GB: f64 = (1u64 << 30) as f64;
    let gb = bytes as f64 / GB;
    let rounded = (gb * 10f64.powi(decimals as i32)).round() / 10f64.powi(decimals as i32);
    if rounded >= 1024.0 {
        format!("{:.*} TB", decimals, gb / 1024.0)
    } else if rounded >= 1.0 {
        format!("{:.*} GB", decimals, gb)
    } else {
        format!("{:.*} MB", decimals, gb * 1024.0)
    }
}

/// A speed as the console shows it, in decimal units, whole: "323 Kbps" below 1 Mbps, else "106 Mbps".
pub fn speed(bits_per_second: u64) -> String {
    let bits = bits_per_second as f64;
    if bits < 1_000_000.0 {
        format!("{:.0} Kbps", bits / 1_000.0)
    } else {
        format!("{:.0} Mbps", bits / 1_000_000.0)
    }
}

/// A share as the console shows it, to a tenth: "1.9%".
pub fn percent(fraction: f64) -> String {
    format!("{:.1}%", fraction * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;

    #[test]
    fn sizes_speeds_and_shares_read_as_the_bundles() {
        assert_eq!(size(71_094_272, 2), "67.80 MB");
        assert_eq!(size(3_811_768_320, 2), "3.55 GB");
        assert_eq!(size(1000 * MB + MB / 2, 2), "1000.50 MB");
        // Rounded up to a whole GB, it is read in GB.
        assert_eq!(size(GB - 1, 2), "1.00 GB");
        assert_eq!(size(2048 * GB, 2), "2.00 TB");
        assert_eq!(size(0, 2), "0.00 MB");
        assert_eq!(speed(323_400), "323 Kbps");
        assert_eq!(speed(106_200_000), "106 Mbps");
        assert_eq!(speed(0), "0 Kbps");
        assert_eq!(percent(0.019), "1.9%");
        assert_eq!(percent(0.0), "0.0%");
    }

    #[test]
    fn a_download_says_its_speed_amount_and_share_and_one_waiting_only_its_share() {
        let downloading = Queued {
            game: 1,
            state: InstallState::Downloading { done: 71_094_272, total: 3_811_768_320 },
            started: 0,
            speed: Some(106_000_000),
        };
        let figures = Figures::of(&downloading);
        assert_eq!(figures.state, "Installing...");
        assert_eq!(figures.speed.as_deref(), Some("106 Mbps"));
        assert_eq!(figures.amount.as_deref(), Some("67.80 MB of 3.55 GB"));
        assert_eq!(figures.percent.as_deref(), Some("1.9%"));
        assert!((figures.fraction - 0.01865).abs() < 0.0001);
        // Its client not knowing the whole yet, nor its bytes moved: Installing... alone, the bar empty.
        let starting = Queued { state: InstallState::Downloading { done: 0, total: 0 }, speed: None, ..downloading };
        let figures = Figures::of(&starting);
        assert_eq!((figures.amount, figures.speed, figures.percent), (None, None, None));
        assert_eq!((figures.state, figures.fraction), ("Installing...", 0.0));
        // Paused half way: Queued, 0.0%, the bar empty.
        let paused = Queued { state: InstallState::Paused { done: 50, total: 100 }, ..downloading };
        let figures = Figures::of(&paused);
        assert_eq!((figures.state, figures.percent.as_deref(), figures.fraction), ("Queued", Some("0.0%"), 0.0));
        assert_eq!((figures.speed, figures.amount), (None, None));
    }
}
