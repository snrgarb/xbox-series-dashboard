//! Dates as the user's Windows writes them short: every screen that shows a date (the Game hub's Last played,
//! the achievements' Unlocked and Last unlocked, Play history's rows) writes it here, in the short date format the
//! Dashboard is given once at start (`set_short_format`, read from Windows by `shell::locale`). With none given, on
//! Linux and in the tests, US English's, month first: "11/18/2026".
use std::sync::RwLock;

use chrono::{Datelike, Local, NaiveDate, TimeZone};

/// The short date format when Windows has given none: US English's.
pub const FALLBACK: &str = "M/d/yyyy";

static SHORT_FORMAT: RwLock<Option<String>> = RwLock::new(None);

/// The user's short date format, as a Windows date picture ("d/MM/yyyy"), or none for [`FALLBACK`].
pub fn set_short_format(picture: Option<String>) {
    *SHORT_FORMAT.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = picture.filter(|p| !p.trim().is_empty());
}

/// `day` in the user's short date format.
pub fn short(day: NaiveDate) -> String {
    let picture = SHORT_FORMAT.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    format(day, picture.as_deref().unwrap_or(FALLBACK))
}

/// The day a moment in Unix seconds was on where the user is, in their short date format; nothing for a moment no
/// calendar has.
pub fn short_at(unix_seconds: u64) -> String {
    let at = i64::try_from(unix_seconds).ok().and_then(|at| Local.timestamp_opt(at, 0).single());
    at.map_or(String::new(), |at| short(at.date_naive()))
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// `day` written by a Windows date picture (Win32's "Day, Month, Year, and Era Format Pictures"): `d` `dd` the day
/// without and with a leading zero, `ddd` `dddd` the weekday's name short and long; `M` `MM` the month, `MMM` `MMMM`
/// its name; `y` `yy` the year in its century without and with a leading zero, `yyyy` (or more) all of it; `g` the
/// era, left out; text in single quotes as it is, `''` a quote; anything else as it is. Names in English, the
/// Dashboard's language.
pub fn format(day: NaiveDate, picture: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = picture.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c == '\'' && chars.get(at + 1) == Some(&'\'') {
            out.push('\'');
            at += 2;
            continue;
        }
        if c == '\'' {
            at += 1;
            while at < chars.len() {
                if chars[at] == '\'' {
                    if chars.get(at + 1) == Some(&'\'') {
                        out.push('\'');
                        at += 2;
                        continue;
                    }
                    break;
                }
                out.push(chars[at]);
                at += 1;
            }
            at += 1;
            continue;
        }
        let run = chars[at..].iter().take_while(|&&next| next == c).count();
        at += run;
        match (c, run) {
            ('d', 1) => out.push_str(&day.day().to_string()),
            ('d', 2) => out.push_str(&format!("{:02}", day.day())),
            ('d', 3) => out.push_str(&WEEKDAYS[day.weekday().num_days_from_monday() as usize][..3]),
            ('d', _) => out.push_str(WEEKDAYS[day.weekday().num_days_from_monday() as usize]),
            ('M', 1) => out.push_str(&day.month().to_string()),
            ('M', 2) => out.push_str(&format!("{:02}", day.month())),
            ('M', 3) => out.push_str(&MONTHS[day.month0() as usize][..3]),
            ('M', _) => out.push_str(MONTHS[day.month0() as usize]),
            ('y', 1) => out.push_str(&(day.year().rem_euclid(100)).to_string()),
            ('y', 2) => out.push_str(&format!("{:02}", day.year().rem_euclid(100))),
            ('y', _) => out.push_str(&day.year().to_string()),
            ('g', _) => {}
            _ => (0..run).for_each(|_| out.push(c)),
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_written_as_the_users_windows_writes_it_short() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        // Australia's, as on the TV PC; the United States'; Germany's; Sweden's; Japan's; the United Kingdom's.
        assert_eq!(format(day, "d/MM/yyyy"), "8/11/2026");
        assert_eq!(format(day, "M/d/yyyy"), "11/8/2026");
        assert_eq!(format(day, "dd.MM.yyyy"), "08.11.2026");
        assert_eq!(format(day, "yyyy-MM-dd"), "2026-11-08");
        assert_eq!(format(day, "yyyy/MM/dd"), "2026/11/08");
        assert_eq!(format(day, "dd/MM/yyyy"), "08/11/2026");
        // A short year; names; quoted text and a quote; an era, left out.
        assert_eq!(format(day, "d-MMM-yy"), "8-Nov-26");
        assert_eq!(format(day, "dddd, MMMM d"), "Sunday, November 8");
        assert_eq!(format(day, "ddd d 'de' MMM ''yy"), "Sun 8 de Nov '26");
        assert_eq!(format(day, "gg yyyy/M/d"), "2026/11/8");
        assert_eq!(format(NaiveDate::from_ymd_opt(2005, 1, 2).unwrap(), "y yy"), "5 05");
    }

    #[test]
    fn with_no_format_from_windows_dates_are_us_englishs() {
        assert_eq!(short(NaiveDate::from_ymd_opt(2026, 11, 18).unwrap()), "11/18/2026");
        assert_eq!(short_at(u64::MAX), "");
    }
}
