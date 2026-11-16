//! Play history: every game that has been played, the last played first, each with the day it was and the section
//! of My games & apps' Play history it falls in (docs/spec/my-games.md, section 6.1).
//!
//! When a game was last played is the latest of what its source says (`games.last_played`, which a Play session
//! also moves on) and its Play sessions themselves, so a session whose game row was since written by an older
//! source still counts. Hidden games and games their source no longer lists have no row.
//!
//! The day and the section are told by the calendar where the user is, as of the moment the snapshot is taken:
//! a snapshot kept past midnight says "Today" of yesterday's games until the next one, as its filters that count
//! days do.
use anyhow::Result;
use chrono::{Datelike, Days, Local, Months, NaiveDate, TimeZone, Weekday};
use model::{Ago, Day, HistoryRow};
use rusqlite::Connection;

/// The first day of the week. The Bundle takes the user's regional setting; nothing here reads Windows' yet, so
/// it is the one the Bundle's pictures were taken with (US English). **The hook for the user's own.**
pub const WEEK_STARTS: Weekday = Weekday::Sun;

/// The section a game last played on `day` is in when today is `today`: the first of the rules that holds.
pub fn ago(day: NaiveDate, today: NaiveDate, week_starts: Weekday) -> Ago {
    let back = |days: u64| today.checked_sub_days(Days::new(days)).unwrap_or(NaiveDate::MIN);
    let week = back(u64::from(today.weekday().days_since(week_starts)));
    let month = today.with_day(1).unwrap_or(today);
    let last_week = week.checked_sub_days(Days::new(7)).unwrap_or(NaiveDate::MIN);
    let last_month = month.checked_sub_months(Months::new(1)).unwrap_or(NaiveDate::MIN);
    if day >= today {
        Ago::Today
    } else if day == back(1) {
        Ago::Yesterday
    } else if day >= week {
        Ago::ThisWeek
    } else if day >= last_week {
        Ago::LastWeek
    } else if day >= month {
        Ago::ThisMonth
    } else if day >= last_month {
        Ago::LastMonth
    } else if day.year() == today.year() {
        Ago::ThisYear
    } else {
        Ago::Older
    }
}

/// The day of a moment (Unix seconds) where the user is.
fn local_day(at: u64) -> NaiveDate {
    let moment = i64::try_from(at).ok().and_then(|at| Local.timestamp_opt(at, 0).earliest());
    moment.map_or(NaiveDate::MIN, |moment| moment.date_naive())
}

/// Play history as of `now`: the last played first, then by name.
pub(crate) fn rows(conn: &Connection, now: u64) -> Result<Vec<HistoryRow>> {
    let today = local_day(now);
    let mut rows = conn.prepare(
        "SELECT g.id, MAX(g.last_played, COALESCE(MAX(s.started), 0), COALESCE(MAX(s.ended), 0)) AS at
         FROM games g LEFT JOIN play_sessions s ON s.game = g.id
         WHERE g.hidden = 0 AND g.gone = 0
         GROUP BY g.id HAVING at > 0
         ORDER BY at DESC, g.sort_name, g.name, g.id",
    )?;
    let rows = rows.query_map([], |row| {
        let (game, at): (i64, i64) = (row.get(0)?, row.get(1)?);
        let at = at.max(0) as u64;
        let day = local_day(at);
        Ok(HistoryRow {
            game,
            at,
            day: Day { year: day.year(), month: day.month() as u8, day: day.day() as u8 },
            ago: ago(day, today, WEEK_STARTS),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::STEAM;
    use crate::Database;
    use model::{Game, Library};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    /// docs/spec/my-games.md, 6.1, on the day the Bundle's pictures were taken: Sunday 15 November 2026.
    #[test]
    fn a_day_falls_in_the_first_section_whose_rule_holds() {
        let today = date(2026, 11, 15);
        let on = |month, day| ago(date(2026, month, day), today, Weekday::Sun);
        assert_eq!(on(11, 15), Ago::Today);
        assert_eq!(on(11, 16), Ago::Today, "a clock that went back");
        assert_eq!(on(11, 14), Ago::Yesterday);
        // The week began today, so the days before yesterday are last week's, back to Sunday the 8th.
        assert_eq!((on(11, 13), on(11, 8)), (Ago::LastWeek, Ago::LastWeek));
        assert_eq!((on(11, 7), on(11, 1)), (Ago::ThisMonth, Ago::ThisMonth));
        assert_eq!((on(10, 31), on(10, 1)), (Ago::LastMonth, Ago::LastMonth));
        assert_eq!((on(9, 30), on(1, 1)), (Ago::ThisYear, Ago::ThisYear));
        assert_eq!(ago(date(2025, 12, 31), today, Weekday::Sun), Ago::Older);
    }

    #[test]
    fn the_week_begins_on_the_day_it_is_told() {
        // Wednesday 18 November 2026.
        let today = date(2026, 11, 18);
        let on = |day, starts| ago(date(2026, 11, day), today, starts);
        assert_eq!((on(16, Weekday::Sun), on(15, Weekday::Sun)), (Ago::ThisWeek, Ago::ThisWeek));
        assert_eq!(on(14, Weekday::Sun), Ago::LastWeek);
        assert_eq!((on(16, Weekday::Mon), on(15, Weekday::Mon)), (Ago::ThisWeek, Ago::LastWeek));
    }

    #[test]
    fn last_month_reaches_back_over_a_new_year_and_this_year_does_not() {
        let today = date(2027, 1, 20);
        let on = |year, month, day| ago(date(year, month, day), today, Weekday::Sun);
        assert_eq!(on(2026, 12, 1), Ago::LastMonth);
        assert_eq!(on(2026, 11, 30), Ago::Older);
        assert_eq!(on(2027, 1, 2), Ago::ThisMonth);
    }

    fn game(appid: u32, name: &str, last_played: u64) -> Game {
        let installed = appid.is_multiple_of(2);
        Game { source_id: appid.to_string(), name: name.into(), last_played, installed, ..Game::default() }
    }

    #[test]
    fn play_history_is_every_played_game_newest_first_but_the_hidden() {
        const NOW: u64 = 1_800_000_000;
        const DAY: u64 = 86_400;
        let mut db = Database::in_memory().unwrap();
        let games = vec![
            game(1, "Moss", NOW - 400 * DAY),
            game(2, "Anchor", NOW - 3 * DAY),
            game(3, "Never", 0),
            game(4, "Hidden", NOW - DAY),
            game(5, "Bramble", NOW - 3 * DAY),
        ];
        db.sync(STEAM, &Library { games, ..Library::default() }, NOW).unwrap();
        let id = |name: &str| db.snapshot_at(NOW).unwrap().games.iter().find(|g| g.name == name).unwrap().id;
        let (moss, anchor, never, hidden, bramble) =
            (id("Moss"), id("Anchor"), id("Never"), id("Hidden"), id("Bramble"));
        db.set_hidden(hidden, true).unwrap();

        let history = db.snapshot_at(NOW).unwrap().history;
        // Installed or not; the same moment by name.
        assert_eq!(history.iter().map(|row| row.game).collect::<Vec<_>>(), [anchor, bramble, moss]);
        assert_eq!(history[2].ago, Ago::Older);
        assert_eq!(history[0].at, NOW - 3 * DAY);

        // A Play session puts its game first, under Today, whatever its source last said.
        let session = db.session_started(never, NOW).unwrap();
        let history = db.snapshot_at(NOW + 60).unwrap().history;
        assert_eq!((history[0].game, history[0].ago, history[0].at), (never, Ago::Today, NOW));
        db.session_ended(session, NOW + 3600).unwrap();
        let history = db.snapshot_at(NOW + 3600).unwrap().history;
        assert_eq!((history[0].game, history[0].at, history.len()), (never, NOW + 3600, 4));
        let day = local_day(NOW + 3600);
        assert_eq!(history[0].day, Day { year: day.year(), month: day.month() as u8, day: day.day() as u8 });

        // Shown again, the hidden game is back in its place.
        db.set_hidden(hidden, false).unwrap();
        let history = db.snapshot_at(NOW + 3600).unwrap().history;
        assert_eq!(history.iter().map(|row| row.game).collect::<Vec<_>>(), [never, hidden, anchor, bramble, moss]);
    }
}
