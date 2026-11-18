//! Times of Direct Messages: the model gives Unix milliseconds, the screens write them as the Guide's clock does
//! (`chrono` on the machine's local time, "9:40 PM") and the Game hub's dates do (the user's short date format).
//! "Now" is injectable, so a screenshot or a test is of one moment whatever the machine's clock and zone say.
use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// What the time is and where: `now` in Unix milliseconds and the zone's offset in seconds east of UTC, each from
/// the machine when None.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct When {
    pub now: Option<i64>,
    pub offset: Option<i32>,
}

impl When {
    /// A fixed moment in a fixed zone: a test's.
    pub fn at(now: i64, offset: i32) -> Self {
        Self { now: Some(now), offset: Some(offset) }
    }

    /// Now, in Unix milliseconds.
    pub fn now_ms(&self) -> i64 {
        self.now.unwrap_or_else(|| Utc::now().timestamp_millis())
    }

    fn local(&self, ms: i64) -> NaiveDateTime {
        let utc = DateTime::from_timestamp_millis(ms).unwrap_or_default();
        match self.offset.and_then(FixedOffset::east_opt) {
            Some(zone) => utc.with_timezone(&zone).naive_local(),
            None => Local.from_utc_datetime(&utc.naive_utc()).naive_local(),
        }
    }

    /// The day a moment was on.
    pub fn day_of(&self, ms: i64) -> NaiveDate {
        self.local(ms).date()
    }

    /// The time of day, as the Guide's clock writes it: "9:40 PM".
    pub fn time(&self, ms: i64) -> String {
        self.local(ms).format("%-I:%M %p").to_string()
    }

    /// How many days ago a moment's day was: 0 for today.
    fn days_ago(&self, ms: i64) -> i64 {
        (self.day_of(self.now_ms()) - self.day_of(ms)).num_days()
    }

    /// A day, written: "Today", "Yesterday", a weekday within 7 days, else the short date.
    pub fn day(&self, ms: i64) -> String {
        match self.days_ago(ms) {
            0 => "Today".into(),
            1 => "Yesterday".into(),
            2..=6 => WEEKDAYS[self.day_of(ms).weekday().num_days_from_monday() as usize].into(),
            _ => crate::date::short(self.day_of(ms)),
        }
    }

    /// A DM's last message in the list: its time today, else its day.
    pub fn stamp(&self, ms: i64) -> String {
        if self.days_ago(ms) == 0 {
            self.time(ms)
        } else {
            self.day(ms)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: i64 = 1_795_000_000_000;

    #[test]
    fn a_day_is_written_by_how_long_ago_it_was() {
        let when = When::at(NOON, 0);
        let day = 86_400_000;
        assert_eq!(when.day(NOON - 3_600_000), "Today");
        assert_eq!(when.day(NOON - day), "Yesterday");
        assert!(WEEKDAYS.contains(&when.day(NOON - 3 * day).as_str()));
        assert_eq!(when.day(NOON - 10 * day), crate::date::short(when.day_of(NOON - 10 * day)));
        assert_eq!(when.stamp(NOON - 1_000), when.time(NOON - 1_000));
        assert_eq!(when.stamp(NOON - day), "Yesterday");
    }

    #[test]
    fn the_zone_is_the_one_given() {
        let utc = When::at(NOON, 0);
        let east = When::at(NOON, 10 * 3600);
        assert_ne!(utc.time(NOON), east.time(NOON));
    }
}
