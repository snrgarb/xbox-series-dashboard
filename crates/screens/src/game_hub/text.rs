//! The hub's words for a game's numbers, as Playnite's converters write them for the Bundle (docs/spec/game-hub.md,
//! 4.2 and 5.2).
use chrono::{Datelike, Local, TimeZone, Utc};

/// How long a game was played: under an hour "N minutes", else "Hh Mm". The library counts whole minutes, so
/// Playnite's "N seconds" is never said.
pub fn playtime(minutes: u32) -> String {
    match minutes {
        0..60 => format!("{minutes} minutes"),
        _ => format!("{}h {}m", minutes / 60, minutes % 60),
    }
}

/// The day a game was last played, where the player is: month, day and year, as the Bundle's picture has it
/// ("11/15/2026").
pub fn date(unix_seconds: u64) -> String {
    let Some(at) = i64::try_from(unix_seconds).ok().and_then(|at| Local.timestamp_opt(at, 0).single()) else {
        return String::new();
    };
    at.format("%-m/%-d/%Y").to_string()
}

/// The year a game came out.
pub fn year(unix_seconds: u64) -> String {
    let at = i64::try_from(unix_seconds).ok().and_then(|at| Utc.timestamp_opt(at, 0).single());
    at.map_or(String::new(), |at| at.year().to_string())
}

/// What an installed game takes on disk, in the largest of B, KB, MB, GB and TB that fits (1024 of one to the
/// next), with two decimals, or three when the third is not 0: "45.20 GB", "512.00 MB", "48.894 GB". As Playnite,
/// the number is cut to a whole count of the unit below before it is divided.
pub fn size(bytes: u64) -> String {
    let (cut, unit) = match bytes {
        0..0x400 => return format!("{bytes} B"),
        0x400..0x10_0000 => (bytes, "KB"),
        0x10_0000..0x4000_0000 => (bytes >> 10, "MB"),
        0x4000_0000..0x100_0000_0000 => (bytes >> 20, "GB"),
        _ => (bytes >> 30, "TB"),
    };
    let number = format!("{:.3}", cut as f64 / 1024.0);
    format!("{} {unit}", number.strip_suffix('0').unwrap_or(&number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_time_is_minutes_under_an_hour_and_hours_and_minutes_from_there() {
        assert_eq!(playtime(1), "1 minutes");
        assert_eq!(playtime(59), "59 minutes");
        assert_eq!(playtime(60), "1h 0m");
        assert_eq!(playtime(600), "10h 0m");
        assert_eq!(playtime(1250), "20h 50m");
    }

    #[test]
    fn a_size_has_two_decimals_or_three() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1536), "1.50 KB");
        assert_eq!(size(512 << 20), "512.00 MB");
        assert_eq!(size(45 * (1 << 30) + 205 * (1 << 20)), "45.20 GB");
        // What the Bundle's picture of its hub reads: 50067 MB and a little.
        assert_eq!(size(50_067 * (1 << 20) + 777), "48.894 GB");
        assert_eq!(size(1_264 << 30), "1.234 TB");
    }

    #[test]
    fn a_year_is_the_release_dates() {
        assert_eq!(year(1_552_348_800), "2019");
        assert_eq!(year(1_698_364_800), "2023");
    }

    #[test]
    fn a_date_is_month_day_and_year_without_leading_zeros() {
        // Noon in Greenwich is the same day in every time zone a test is likely to run in.
        assert_eq!(date(1_789_992_000), "9/21/2026");
        assert_eq!(date(1_552_392_000), "3/12/2019");
    }
}
