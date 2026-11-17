//! What Manage Queue's rows and My games & apps' current download say of a game in the Queue, worded as the
//! Bundle words them (its `QueueEntry`, `StorageRing.Size` and `Speed`): its state, its speed, how much is down of
//! the whole, the share down, and how far its bar fills.
use model::{InstallState, Queued};

/// One game in the Queue, as its row reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Figures {
    /// "Installing...", "Queued", "Paused", or "Downloading Workshop content" (#188).
    pub state: &'static str,
    /// "106 Mbps", "323 Kbps": none while it waits, or until its bytes have first moved.
    pub speed: Option<String>,
    /// "67.80 MB of 3.55 GB": none while it waits, or while its client does not know the whole yet.
    pub amount: Option<String>,
    /// "12 of 160 items": a game installed whose Workshop items are coming in (#188), where its store says how
    /// many; none for anything else.
    pub items: Option<String>,
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
                    items: None,
                    percent: (total > 0).then(|| percent(fraction)),
                    fraction: fraction as f32,
                }
            }
            // Paused, here or in the client (#170): the console's word, with what is down of the whole; no speed.
            InstallState::Paused { done, total } => {
                let fraction = if total == 0 { 0.0 } else { (done as f64 / total as f64).clamp(0.0, 1.0) };
                Figures {
                    state: model::queue::PAUSED,
                    speed: None,
                    amount: (total > 0).then(|| format!("{} of {}", size(done, 2), size(total, 2))),
                    items: None,
                    percent: Some(percent(fraction)),
                    fraction: fraction as f32,
                }
            }
            // Waiting its turn: only its share, 0.0%, as the Bundle reads a download that is not active.
            InstallState::Queued => Figures {
                state: entry.state.status(),
                speed: None,
                amount: None,
                items: None,
                percent: Some(percent(0.0)),
                fraction: 0.0,
            },
            // The game installed, its Workshop items still coming in (#188): what they are, how many are in, their
            // bytes where known, and the share by the bytes, else by the items. Never a speed: Steam's is not known
            // to be theirs, and a speed of 0 said nothing true of them on the real machine.
            InstallState::Extras { items_done, items, done, total } => {
                let fraction = entry.state.fraction();
                Figures {
                    state: model::queue::WORKSHOP,
                    speed: None,
                    amount: (total > 0).then(|| format!("{} of {}", size(done, 2), size(total, 2))),
                    items: (items > 0).then(|| format!("{items_done} of {items} items")),
                    percent: fraction.map(percent),
                    fraction: fraction.unwrap_or(0.0) as f32,
                }
            }
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
            speed: Some(106_000_000),
            ..Queued::new(1, InstallState::Downloading { done: 71_094_272, total: 3_811_768_320 }, 0)
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
        // Paused half way: Paused, with what is down, and no speed.
        let paused = Queued { state: InstallState::Paused { done: 1 << 30, total: 2 << 30 }, ..downloading };
        let figures = Figures::of(&paused);
        assert_eq!((figures.state, figures.percent.as_deref(), figures.fraction), ("Paused", Some("50.0%"), 0.5));
        assert_eq!((figures.speed, figures.amount.as_deref()), (None, Some("1.00 GB of 2.00 GB")));
        // Waiting its turn: Queued, 0.0%, the bar empty.
        let queued = Queued { state: InstallState::Queued, ..downloading };
        let figures = Figures::of(&queued);
        assert_eq!((figures.state, figures.percent.as_deref(), figures.fraction), ("Queued", Some("0.0%"), 0.0));
        assert_eq!((figures.speed, figures.amount), (None, None));
        // Installed, its Workshop items coming in: what it is, the items in, the share by them, and never a speed.
        let extras =
            Queued { state: InstallState::Extras { items_done: 3, items: 12, done: 0, total: 0 }, ..downloading };
        let figures = Figures::of(&extras);
        assert_eq!(
            (figures.state, figures.percent.as_deref(), figures.speed),
            ("Downloading Workshop content", Some("25.0%"), None)
        );
        assert_eq!((figures.items.as_deref(), figures.amount, figures.fraction), (Some("3 of 12 items"), None, 0.25));
        // Their bytes known: the share by them.
        let sized = InstallState::Extras { items_done: 3, items: 12, done: GB / 2, total: 2 * GB };
        let figures = Figures::of(&Queued { state: sized, ..extras });
        assert_eq!(
            (figures.amount.as_deref(), figures.percent.as_deref()),
            (Some("512.00 MB of 2.00 GB"), Some("25.0%"))
        );
        // Nothing said of them: the words alone.
        let unknown = Queued { state: InstallState::Extras { items_done: 0, items: 0, done: 0, total: 0 }, ..extras };
        let figures = Figures::of(&unknown);
        assert_eq!((figures.items, figures.percent, figures.fraction), (None, None, 0.0));
    }
}
