//! What the idle panel shows, one card at a time (docs/spec/idle-screen.md, 4): the last games played and tips. The
//! Bundle's nearly-finished games and last unlocks come from PlayniteAchievements, and the port has no achievements
//! yet; they join the pool when it has.
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use model::{GameKey, Library};
use ui::Button;

/// How many cards the pool holds of each kind but tips, which are all in it.
const OF_A_KIND: usize = 5;

/// What a card tells of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One of the last games played.
    RecentlyPlayed,
    /// A tip, text only, with the pad drawn in its art's place.
    Tip,
}

/// One card of the idle panel: a light top line, a bold title, a bold label and a description, a game's art in the
/// lower half (a tip's pad in its place), and a Y prompt. What is None isn't shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub kind: Kind,
    /// The light top line: "Played yesterday", "Tip".
    pub header: Option<String>,
    /// The bold title: a game's name, or a tip's text.
    pub title: String,
    /// The bold line under the title: "Recently played".
    pub label: Option<String>,
    /// The line under the label: the game's playtime, "12 h played".
    pub description: Option<String>,
    /// The game whose art shows and whose Game hub Y opens.
    pub game: Option<GameKey>,
    /// The Y prompt's words: "See game", "Got it".
    pub prompt: &'static str,
    /// The pad's buttons a tip is about, filled on its pad.
    pub buttons: &'static [Button],
}

/// One of the tips: its text, a button it names in braces as the Bundle writes it ("{Y}"), and the buttons it is
/// about.
struct Tip {
    text: &'static str,
    buttons: &'static [Button],
}

/// The Bundle's tips, as they apply to the port: those about the Guide only while it is on. Those about Discord, the
/// Score and the Boot video are left out until those come (M6, achievements, #126); Playnite's filter presets are
/// Settings' Groups here.
const TIPS: [Tip; 8] = [
    Tip { text: "Press {Guide} anywhere, even in a game, to open the Guide.", buttons: &[Button::Guide] },
    Tip { text: "Press {Start} on a game tile to play it, favourite it or manage it.", buttons: &[Button::Menu] },
    Tip { text: "Press {Y} on Home to search your games and apps.", buttons: &[Button::Y] },
    Tip {
        text: "Customise Home from a tile's menu, or Settings › Theme settings › Personalisation.",
        buttons: &[Button::Menu],
    },
    Tip {
        text: "Choose the Groups Home shows in Settings › Theme settings › Personalisation › Groups.", buttons: &[]
    },
    Tip {
        text: "On the keyboard, {X} is backspace, {Y} is space, {Start} is Enter.",
        buttons: &[Button::X, Button::Y, Button::Menu],
    },
    Tip { text: "See what's downloading in My games & apps › Manage › Queue.", buttons: &[] },
    Tip { text: "See what's playing in the Guide's Audio, under Now Playing.", buttons: &[Button::Guide] },
];

/// A tip's text with its buttons named in words, as the port writes them until it draws a button's glyph in a line of
/// text: "{Y}" is "Y", "{Start}" "Menu", "{Guide}" "the Guide button".
fn words(text: &str) -> String {
    text.replace("{Guide}", "the Guide button").replace("{Start}", "Menu").replace("{Y}", "Y").replace("{X}", "X")
}

/// Every card of the pool, unshuffled: the last games played, then the tips, those about the Guide only with the
/// Guide on (Settings > Guide). `today` is the date, which "Played yesterday" is counted from.
pub fn pool(library: &Library, use_guide: bool, today: NaiveDate) -> Vec<Card> {
    let mut played: Vec<_> = library.games.iter().filter(|game| game.last_played > 0).collect();
    played.sort_by_key(|game| std::cmp::Reverse(game.last_played));
    let games = played.into_iter().take(OF_A_KIND).map(|game| Card {
        kind: Kind::RecentlyPlayed,
        header: Some(format!("Played {}", when(local_date(game.last_played), today))),
        title: game.name.clone(),
        label: Some("Recently played".into()),
        description: playtime(game.playtime_minutes).map(|played| format!("{played} played")),
        game: Some(game.key()),
        prompt: "See game",
        buttons: &[],
    });
    let tips = TIPS.iter().filter(|tip| use_guide || !tip.buttons.contains(&Button::Guide)).map(|tip| Card {
        kind: Kind::Tip,
        header: Some("Tip".into()),
        title: words(tip.text),
        label: None,
        description: None,
        game: None,
        prompt: "Got it",
        buttons: tip.buttons,
    });
    games.chain(tips).collect()
}

/// A playtime as the card words it, "12 h" or under an hour "45 min"; None under a minute.
fn playtime(minutes: u32) -> Option<String> {
    match minutes {
        0 => None,
        1..=59 => Some(format!("{minutes} min")),
        _ => Some(format!("{} h", minutes / 60)),
    }
}

/// The local date a moment in Unix seconds was on.
pub fn local_date(seconds: u64) -> NaiveDate {
    let at = i64::try_from(seconds).unwrap_or(i64::MAX);
    Local.timestamp_opt(at, 0).single().map_or(NaiveDate::MIN, |at| at.date_naive())
}

/// Today, by the machine's clock.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// When something happened, from `today` by calendar day, as the cards word it: "today", "yesterday", "3 days ago",
/// "a week ago", "2 weeks ago", "a month ago", "5 months ago", "a year ago", "3 years ago". A day ahead of today is
/// today.
pub fn when(day: NaiveDate, today: NaiveDate) -> String {
    let days = (today - day).num_days();
    match days {
        ..=0 => "today".into(),
        1 => "yesterday".into(),
        2..=6 => format!("{days} days ago"),
        7..=13 => "a week ago".into(),
        14..=29 => format!("{} weeks ago", days / 7),
        _ => {
            // Whole calendar months, one more only once the day of the month comes round again.
            let months = (today.year() - day.year()) * 12 + today.month() as i32
                - day.month() as i32
                - i32::from(today.day() < day.day());
            match months.max(1) {
                1 => "a month ago".into(),
                months @ 2..=11 => format!("{months} months ago"),
                months if months < 24 => "a year ago".into(),
                months => format!("{} years ago", months / 12),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn when_is_worded_by_calendar_day_as_the_bundle_words_it() {
        let today = date(2026, 11, 16);
        let cases = [
            (date(2026, 11, 17), "today"),
            (date(2026, 11, 16), "today"),
            (date(2026, 11, 15), "yesterday"),
            (date(2026, 11, 13), "3 days ago"),
            (date(2026, 11, 9), "a week ago"),
            (date(2026, 11, 2), "2 weeks ago"),
            (date(2026, 10, 18), "4 weeks ago"),
            (date(2026, 10, 16), "a month ago"),
            // The 17th has not come round again this month: still one month.
            (date(2026, 9, 17), "a month ago"),
            (date(2026, 6, 1), "5 months ago"),
            (date(2025, 11, 16), "a year ago"),
            (date(2023, 1, 1), "3 years ago"),
        ];
        for (day, words) in cases {
            assert_eq!(when(day, today), words, "{day}");
        }
    }

    #[test]
    fn the_pool_is_the_last_five_games_played_then_the_tips() {
        let library = crate::fake_library();
        let today = local_date(library.games[0].last_played).succ_opt().unwrap();
        let cards = pool(&library, true, today);
        let games: Vec<_> = cards.iter().filter(|card| card.kind == Kind::RecentlyPlayed).collect();
        assert_eq!(games.len(), 5);
        assert_eq!(games[0].title, "Halo Infinite");
        assert_eq!(games[0].header.as_deref(), Some("Played yesterday"));
        assert_eq!(games[0].description.as_deref(), Some("10 h played"));
        assert_eq!(games[1].header.as_deref(), Some("Played 2 days ago"));
        assert_eq!(cards.iter().filter(|card| card.kind == Kind::Tip).count(), TIPS.len());
        assert!(cards.iter().any(|card| card.title == "Press Y on Home to search your games and apps."));
        // With the Guide off, nothing about it.
        let cards = pool(&library, false, today);
        assert!(cards.iter().all(|card| !card.buttons.contains(&Button::Guide)));
        assert!(cards.iter().all(|card| !card.title.contains("Guide")));
    }

    #[test]
    fn a_playtime_is_hours_or_under_an_hour_minutes() {
        assert_eq!(playtime(0), None);
        assert_eq!(playtime(45).as_deref(), Some("45 min"));
        assert_eq!(playtime(60).as_deref(), Some("1 h"));
        assert_eq!(playtime(779).as_deref(), Some("12 h"));
    }
}
