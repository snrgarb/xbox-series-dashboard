//! What Search finds (docs/spec/search.md, 3.7): with nothing typed the games most recently added, else the games
//! whose name has the text in it, those starting with it first, then those with a word starting with it, then the
//! rest, each by its sorting name. Hidden games are never found.
//!
//! The Bundle compares as .NET's current culture does, ignoring case and accents. Here a fixed fold stands for it
//! (section 13): lower case, and the accents taken off the letters that are a letter and an accent in Unicode's
//! canonical decomposition (é, ñ, ü; not ø or ł, which are letters of their own). Culture-specific tailoring, such as
//! Turkish's dotless i, is not followed.
use model::{Game, Library};

/// How many games Recently added shows.
pub const RECENTLY_ADDED: usize = 12;

/// A row of results, as the page lays them out top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Nothing typed: the games most recently added.
    RecentlyAdded,
    /// The games found.
    Games,
}

impl Kind {
    /// The row's heading.
    pub fn heading(self) -> &'static str {
        match self {
            Kind::RecentlyAdded => "Recently added",
            Kind::Games => "Games",
        }
    }

    /// Its name in the focus log: one word.
    pub fn name(self) -> &'static str {
        match self {
            Kind::RecentlyAdded => "RecentlyAdded",
            Kind::Games => "Games",
        }
    }
}

/// The rows for `text` in this library, each with its games' places in `library.games`. Recently added shows
/// whenever nothing is typed, even with no game to show; the Games row only when something is found. (Apps join in
/// M4, under Games.)
pub fn rows(library: &Library, text: &str) -> Vec<(Kind, Vec<usize>)> {
    let text = text.trim_matches(' ');
    let shown = library.games.iter().enumerate().filter(|(_, game)| !game.hidden);
    if text.is_empty() {
        return vec![(Kind::RecentlyAdded, recently_added(shown))];
    }
    let found = found(shown, text);
    if found.is_empty() {
        Vec::new()
    } else {
        vec![(Kind::Games, found)]
    }
}

/// Those with a time they were added, the latest first, then those with none in the library's order: the first
/// twelve. Games added at the same moment (a whole library read at once) go by their sorting name.
fn recently_added<'a>(games: impl Iterator<Item = (usize, &'a Game)>) -> Vec<usize> {
    let mut games: Vec<(usize, &Game)> = games.collect();
    games.sort_by(|(a_at, a), (b_at, b)| {
        let undated = (a.added == 0).cmp(&(b.added == 0));
        let order = if a.added == 0 { a_at.cmp(b_at) } else { sort_name(&a.name).cmp(&sort_name(&b.name)) };
        undated.then(b.added.cmp(&a.added)).then(order)
    });
    games.into_iter().take(RECENTLY_ADDED).map(|(at, _)| at).collect()
}

/// The games whose name has `text` in it, by rank and then by sorting name.
fn found<'a>(games: impl Iterator<Item = (usize, &'a Game)>, text: &str) -> Vec<usize> {
    let text = fold(text);
    let mut found: Vec<(u8, String, usize)> =
        games.filter_map(|(at, game)| Some((rank(&fold(&game.name), &text)?, sort_name(&game.name), at))).collect();
    found.sort();
    found.into_iter().map(|(_, _, at)| at).collect()
}

/// 0 for a name that starts with the text, 1 for one with a word that starts with it, 2 for one with it anywhere
/// else; None for one without it. Both are folded.
fn rank(name: &[char], text: &[char]) -> Option<u8> {
    let at = |i: usize| name[i..].starts_with(text);
    if at(0) {
        return Some(0);
    }
    let anywhere = (0..name.len()).any(at);
    if !anywhere {
        return None;
    }
    // A word starts at a letter or digit that follows something that is neither.
    let word = (1..name.len()).any(|i| !name[i - 1].is_alphanumeric() && name[i].is_alphanumeric() && at(i));
    Some(if word { 1 } else { 2 })
}

/// What a name sorts by, as the library database has it: folded, without a leading article.
fn sort_name(name: &str) -> String {
    let folded: String = fold(name.trim()).into_iter().collect();
    let rest = ["the ", "a ", "an "].iter().find_map(|article| folded.strip_prefix(article));
    rest.map_or(folded.clone(), |rest| rest.trim_start().to_string())
}

/// Lower case, with the accents taken off.
pub fn fold(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).filter_map(unaccented).collect()
}

/// The letter under an accent, for the Latin letters that decompose into one; nothing for an accent on its own (a
/// combining mark); the character itself for everything else.
fn unaccented(c: char) -> Option<char> {
    if ('\u{300}'..='\u{36F}').contains(&c) {
        return None;
    }
    const BASES: [(&str, char); 19] = [
        ("àáâãäåāăąǎ", 'a'),
        ("çćĉċč", 'c'),
        ("ď", 'd'),
        ("èéêëēĕėęě", 'e'),
        ("ĝğġģǧ", 'g'),
        ("ĥ", 'h'),
        ("ìíîïĩīĭįǐ", 'i'),
        ("ĵ", 'j'),
        ("ķǩ", 'k'),
        ("ĺļľ", 'l'),
        ("ñńņň", 'n'),
        ("òóôõöōŏőǒ", 'o'),
        ("ŕŗř", 'r'),
        ("śŝşšș", 's'),
        ("ţťț", 't'),
        ("ùúûüũūŭůűųǔ", 'u'),
        ("ŵ", 'w'),
        ("ýÿŷ", 'y'),
        ("źżž", 'z'),
    ];
    Some(BASES.iter().find(|(accented, _)| accented.contains(c)).map_or(c, |&(_, base)| base))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_of(names: &[&str]) -> Library {
        let games = names.iter().enumerate().map(|(i, name)| Game {
            appid: i as u32 + 1,
            name: name.to_string(),
            ..Game::default()
        });
        Library { games: games.collect(), ..Library::default() }
    }

    fn names(library: &Library, text: &str) -> Vec<String> {
        rows(library, text)
            .into_iter()
            .flat_map(|(_, games)| games.into_iter().map(|at| library.games[at].name.clone()))
            .collect()
    }

    #[test]
    fn a_name_is_found_by_any_part_of_it_whatever_its_case_and_accents() {
        let library = library_of(&["Pokémon Violet", "Halo Infinite", "Forza Horizon 5", "Ærø Ølstrup"]);
        assert_eq!(names(&library, "pokemon"), ["Pokémon Violet"]);
        assert_eq!(names(&library, "POKÉ"), ["Pokémon Violet"]);
        assert_eq!(names(&library, "HALO"), ["Halo Infinite"]);
        assert_eq!(names(&library, "izon"), ["Forza Horizon 5"]);
        // Ø is a letter of its own, not an O with something on it.
        assert!(names(&library, "olstrup").is_empty());
        assert_eq!(names(&library, "ølstrup"), ["Ærø Ølstrup"]);
        assert!(names(&library, "halo wars").is_empty());
    }

    /// The Bundle's own example (CT/SearchTests.cs:237-254).
    #[test]
    fn the_name_starting_with_it_comes_first_then_a_word_starting_with_it_then_the_rest() {
        let library =
            library_of(&["Unhalo", "Super Halo", "Halo Wars", "Ahalo", "The Halo Collection", "Halo Infinite"]);
        assert_eq!(
            names(&library, "halo"),
            ["Halo Infinite", "Halo Wars", "The Halo Collection", "Super Halo", "Ahalo", "Unhalo"]
        );
        // A word starts after anything that is not a letter or a digit.
        let library = library_of(&["X-Halo", "Xhalo", "2Halo"]);
        assert_eq!(names(&library, "halo"), ["X-Halo", "2Halo", "Xhalo"]);
    }

    #[test]
    fn spaces_round_the_text_are_not_matched_and_only_spaces_is_nothing_typed() {
        let library = library_of(&["Halo Infinite", "Celeste"]);
        assert_eq!(names(&library, "  halo "), ["Halo Infinite"]);
        assert_eq!(rows(&library, "   ")[0].0, Kind::RecentlyAdded);
        // Within the text a space counts.
        assert_eq!(names(&library, "o i"), ["Halo Infinite"]);
        assert!(rows(&library, "zz").is_empty(), "nothing found is no row at all");
    }

    #[test]
    fn hidden_games_are_never_found_nor_recently_added() {
        let mut library = library_of(&["Halo Infinite", "Halo Wars"]);
        library.games[1].hidden = true;
        assert_eq!(names(&library, "halo"), ["Halo Infinite"]);
        assert_eq!(names(&library, ""), ["Halo Infinite"]);
    }

    #[test]
    fn recently_added_is_the_twelve_latest_then_those_with_no_date_in_the_librarys_order() {
        let names_: Vec<String> = (0..16).map(|i| format!("Game {i:02}")).collect();
        let mut library = library_of(&names_.iter().map(String::as_str).collect::<Vec<_>>());
        for (i, game) in library.games.iter_mut().enumerate() {
            // The last four have no date; of the rest the higher the number the later.
            game.added = if i < 12 { 1000 + i as u64 } else { 0 };
        }
        library.games[3].added = 0;
        let shown = names(&library, "");
        assert_eq!(shown.len(), 12);
        assert_eq!(shown[..3], ["Game 11", "Game 10", "Game 09"]);
        // Game 03, with no date, comes after every dated game, then the others with none in the library's order.
        assert_eq!(shown[10..], ["Game 00", "Game 03"]);
        // Added together, they go by sorting name.
        let mut library = library_of(&["The Witness", "Braid", "Antichamber"]);
        library.games.iter_mut().for_each(|game| game.added = 5);
        assert_eq!(names(&library, ""), ["Antichamber", "Braid", "The Witness"]);
        // An empty library still has the row, with nothing in it.
        assert_eq!(rows(&Library::default(), ""), [(Kind::RecentlyAdded, Vec::new())]);
    }
}
