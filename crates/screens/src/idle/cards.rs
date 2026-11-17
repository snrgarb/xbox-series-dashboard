//! What the idle panel shows, one card at a time (docs/spec/idle-screen.md, 4): the nearly-finished games, the last
//! unlocks, the last games played and tips.
use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use model::{Game, GameKey, Library};
use ui::Button;

/// How many cards the pool holds of each kind but tips, which are all in it.
const OF_A_KIND: usize = 5;

/// What a card tells of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One of the games with the most achievements unlocked without all: its locked one most players have.
    CloseAchievement,
    /// One of the last achievements unlocked.
    RecentUnlock,
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
    /// An achievement's Score, at the top left with the G, and whether it is Rare, with the diamond before it.
    pub score: Option<u32>,
    pub rare: bool,
    /// How far the game has got with its achievements, 0 to 1: the share at the top right and the bar under it.
    pub progress: Option<f32>,
    /// An achievement's icon, large over the art, once fetched.
    pub icon: Option<PathBuf>,
}

impl Card {
    /// A card with nothing of an achievement's.
    fn plain(kind: Kind, title: String, prompt: &'static str) -> Self {
        Self {
            kind,
            header: None,
            title,
            label: None,
            description: None,
            game: None,
            prompt,
            buttons: &[],
            score: None,
            rare: false,
            progress: None,
            icon: None,
        }
    }
}

/// One of the tips: its text, a button it names in braces as the Bundle writes it ("{Y}"), and the buttons it is
/// about.
struct Tip {
    text: &'static str,
    buttons: &'static [Button],
}

/// The Bundle's tips, as they apply to the port: those about the Guide only while it is on. Those about Discord and
/// the Boot video are left out until those come (M6, #126); Playnite's filter presets are Settings' Groups here.
/// The one about the Score is [`SCORE_TIP`].
const TIPS: [Tip; 8] = [
    Tip { text: "Press {Guide} anywhere, even in a game, to open the Guide.", buttons: &[Button::Guide] },
    Tip { text: "Press {Start} on a game tile to play it, favourite it or manage it.", buttons: &[Button::Menu] },
    Tip { text: "Press {Y} on Home to search your games and apps.", buttons: &[Button::Y] },
    Tip {
        text: "Customise Home from a tile's menu, or Settings › Games & apps › Groups.", buttons: &[Button::Menu]
    },
    Tip { text: "Choose the Groups Home shows in Settings › Games & apps › Groups.", buttons: &[] },
    Tip {
        text: "On the keyboard, {X} is backspace, {Y} is space, {Start} is Enter.",
        buttons: &[Button::X, Button::Y, Button::Menu],
    },
    Tip { text: "See what's downloading in My games & apps › Manage › Queue.", buttons: &[] },
    Tip { text: "See what's playing in the Guide's Audio, under Now Playing.", buttons: &[Button::Guide] },
];

/// The Bundle's tip about the Score, only while there is a Score to see: it adds up every game's achievements here,
/// not every platform's, Steam's being the only ones read.
const SCORE_TIP: Tip = Tip { text: "Your Score adds up achievements across every game.", buttons: &[] };

/// A tip's text with its buttons named in words, as the port writes them until it draws a button's glyph in a line of
/// text: "{Y}" is "Y", "{Start}" "Menu", "{Guide}" "the Guide button".
fn words(text: &str) -> String {
    text.replace("{Guide}", "the Guide button").replace("{Start}", "Menu").replace("{Y}", "Y").replace("{X}", "X")
}

/// Every card of the pool, unshuffled: the nearly-finished games and the last unlocks while there are achievements,
/// the last games played, then the tips, those about the Guide only with the Guide on (Settings > Guide). `today`
/// is the date, which "Played yesterday" is counted from.
pub fn pool(library: &Library, use_guide: bool, today: NaiveDate) -> Vec<Card> {
    let mut played: Vec<_> = library.games.iter().filter(|game| game.last_played > 0).collect();
    played.sort_by_key(|game| std::cmp::Reverse(game.last_played));
    let games = played.into_iter().take(OF_A_KIND).map(|game| Card {
        header: Some(format!("Played {}", when(local_date(game.last_played), today))),
        label: Some("Recently played".into()),
        description: playtime(game.playtime_minutes).map(|played| format!("{played} played")),
        game: Some(game.key()),
        ..Card::plain(Kind::RecentlyPlayed, game.name.clone(), "See game")
    });
    let achievements = library.achievements.totals.is_some();
    let score_tip = achievements.then_some(&SCORE_TIP);
    let tips = TIPS.iter().chain(score_tip).filter(|tip| use_guide || !tip.buttons.contains(&Button::Guide));
    let tips = tips.map(|tip| Card {
        header: Some("Tip".into()),
        buttons: tip.buttons,
        ..Card::plain(Kind::Tip, words(tip.text), "Got it")
    });
    let mut cards = Vec::new();
    if achievements {
        cards.extend(close_achievements(library));
        cards.extend(recent_unlocks(library, today));
    }
    cards.extend(games.chain(tips));
    cards
}

/// The games in the library, never hidden ones, by their id in the library database.
fn shown(library: &Library, id: i64) -> Option<&Game> {
    library.games.iter().find(|game| game.id == id && id != 0)
}

/// The nearly-finished games: the most achievements unlocked without all, those nearer finished first among as
/// many, then by name; each the game and its locked achievement most players have, "Ashfall Courier - Night Route"
/// (the game alone with none to name), that one's Score, Rarity and description, and the game's progress.
fn close_achievements(library: &Library) -> Vec<Card> {
    let mut close: Vec<_> = library
        .achievements
        .games
        .iter()
        .filter(|p| p.unlocked > 0 && p.unlocked < p.of)
        .filter_map(|p| Some((p, shown(library, p.game)?)))
        .collect();
    let share = |p: &model::achievement::GameProgress| f64::from(p.unlocked) / f64::from(p.of);
    close.sort_by(|(a, a_game), (b, b_game)| {
        b.unlocked
            .cmp(&a.unlocked)
            .then(share(b).total_cmp(&share(a)))
            .then_with(|| a_game.name.to_lowercase().cmp(&b_game.name.to_lowercase()))
    });
    let card = |(progress, game): (&model::achievement::GameProgress, &Game)| {
        let next = library.achievements.next_of(game.id);
        let title = next.map_or(game.name.clone(), |next| format!("{} - {}", game.name, next.name));
        Card {
            label: Some("Close achievement".into()),
            description: next.and_then(|next| next.description.clone()),
            game: Some(game.key()),
            score: next.map(|next| next.score()),
            rare: next.is_some_and(|next| next.is_rare()),
            progress: Some(share(progress) as f32),
            ..Card::plain(Kind::CloseAchievement, title, "See game")
        }
    };
    close.into_iter().take(OF_A_KIND).map(card).collect()
}

/// The last unlocks of games in the library, never hidden ones, newest first: each with when atop, "Unlocked 2 days
/// ago" ("Unlocked" alone for a time not known), its Score and Rare, its game and name, "Ashfall Courier - Ash
/// Runner", its description and its icon.
fn recent_unlocks(library: &Library, today: NaiveDate) -> Vec<Card> {
    let unlocks = library.achievements.recent.iter().filter_map(|unlock| Some((unlock, shown(library, unlock.game)?)));
    let card = |(unlock, game): (&model::achievement::Unlock, &Game)| {
        let a = &unlock.achievement;
        let header = match a.unlocked {
            Some(at) if at > 0 => format!("Unlocked {}", when(local_date(at), today)),
            _ => "Unlocked".to_string(),
        };
        Card {
            header: Some(header),
            label: Some("Achievement unlocked".into()),
            description: a.description.clone(),
            game: Some(game.key()),
            score: Some(a.score()),
            rare: a.is_rare(),
            icon: a.icon_file.clone(),
            ..Card::plain(Kind::RecentUnlock, format!("{} - {}", game.name, a.name), "See game")
        }
    };
    unlocks.take(OF_A_KIND).map(card).collect()
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
    fn with_achievements_the_nearly_finished_and_the_last_unlocks_come_first() {
        use model::achievement::{Achievement, GameProgress, Totals, Unlock};
        let mut library = crate::fake_library();
        let today = local_date(library.games[0].last_played).succ_opt().unwrap();
        // Without totals, no achievement cards.
        assert!(pool(&library, true, today).iter().all(|card| card.score.is_none()));

        for (i, game) in library.games.iter_mut().enumerate() {
            game.id = i as i64 + 1;
        }
        let a = |key: &str, rarity: f64, unlocked: Option<u64>| Achievement {
            key: key.into(),
            name: key.into(),
            description: Some(format!("Do {key}.")),
            rarity: Some(rarity),
            unlocked,
            ..Achievement::default()
        };
        let progress = |game, unlocked, of| GameProgress { game, unlocked, of, score: 0 };
        library.achievements.totals = Some(Totals { score: 95, unlocked: 6, of: 16, games: 4 });
        // Halo 2 of 6, Hades II 3 of 4, Starfield 2 of 4 (as many as Halo, nearer finished), Celeste all of 2.
        library.achievements.games = vec![progress(1, 2, 6), progress(5, 3, 4), progress(4, 2, 4), progress(12, 2, 2)];
        library.achievements.next = vec![
            Unlock { game: 5, achievement: a("Night Bloom", 7.5, None) },
            Unlock { game: 1, achievement: a("Legend", 40.0, None) },
        ];
        let at = library.games[0].last_played;
        library.achievements.recent = vec![
            Unlock { game: 5, achievement: a("Blessed", 50.0, Some(at)) },
            Unlock { game: 1, achievement: a("Spartan", 2.0, Some(0)) },
        ];
        let cards = pool(&library, true, today);
        let close: Vec<_> = cards.iter().filter(|card| card.kind == Kind::CloseAchievement).collect();
        let titles: Vec<&str> = close.iter().map(|card| card.title.as_str()).collect();
        assert_eq!(titles, ["Hades II - Night Bloom", "Starfield", "Halo Infinite - Legend"]);
        assert_eq!((close[0].score, close[0].rare, close[0].progress), (Some(30), true, Some(0.75)));
        assert_eq!(close[0].label.as_deref(), Some("Close achievement"));
        assert_eq!(close[0].description.as_deref(), Some("Do Night Bloom."));
        assert_eq!((close[1].score, close[1].description.as_deref()), (None, None));

        let unlocks: Vec<_> = cards.iter().filter(|card| card.kind == Kind::RecentUnlock).collect();
        assert_eq!(unlocks[0].title, "Hades II - Blessed");
        assert_eq!(unlocks[0].header.as_deref(), Some("Unlocked yesterday"));
        assert_eq!((unlocks[0].score, unlocks[0].rare), (Some(15), false));
        assert_eq!(unlocks[0].label.as_deref(), Some("Achievement unlocked"));
        assert_eq!(
            (unlocks[1].header.as_deref(), unlocks[1].score, unlocks[1].rare),
            (Some("Unlocked"), Some(50), true)
        );
        // They come before the games played, and the tips are all there, the Score's with them.
        assert_eq!(cards[0].kind, Kind::CloseAchievement);
        assert_eq!(cards.iter().filter(|card| card.kind == Kind::Tip).count(), TIPS.len() + 1);
        assert!(cards.iter().any(|card| card.title == SCORE_TIP.text));
    }

    #[test]
    fn a_playtime_is_hours_or_under_an_hour_minutes() {
        assert_eq!(playtime(0), None);
        assert_eq!(playtime(45).as_deref(), Some("45 min"));
        assert_eq!(playtime(60).as_deref(), Some("1 h"));
        assert_eq!(playtime(779).as_deref(), Some("12 h"));
    }
}
