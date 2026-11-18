//! How the same game is told across launchers: by its name, normalised (docs/spec/install-from.md, section 2; #249).
//! The library keeps one entry per launcher, so a game owned on Steam and on Epic is two games; these say which games
//! are one product, for Install's "Install from" question and for anything else that needs to know (art looked up by
//! name, #246).
//!
//! **The rule, exact and nothing looser:** two names are the same game when their normalised forms are equal and not
//! empty. Normalising lowercases the name; drops the trademark signs (™ ® © ℠) and apostrophes (' ’ ‘ ʼ ` ´) where they
//! stand; turns every other character that is not a letter or a digit (punctuation, symbols, dashes, any whitespace)
//! into a break between words; and joins the words with one space. Nothing else: edition words stay ("X" and "X
//! Deluxe Edition" are different products), accents stay ("Pokémon" is not "Pokemon"), "&" is not "and", and a
//! numeral is not a Roman one. A wrong match would offer the wrong download, so a doubtful pair is left apart.
use crate::{by_hand, Game};

/// Signs a store puts after a name, or not: dropped where they stand, joining what is either side of them.
const DROPPED: [char; 10] = ['™', '®', '©', '℠', '\'', '’', '‘', 'ʼ', '`', '´'];

/// A game's name as the matching compares it: `"Tom Clancy’s Ghost Recon®: Wildlands"` is
/// `"tom clancys ghost recon wildlands"`. Empty for a name with no letter or digit in it.
pub fn normalised_name(name: &str) -> String {
    let mut words = String::with_capacity(name.len());
    let mut gap = false;
    for c in name.chars().filter(|c| !DROPPED.contains(c)) {
        if c.is_alphanumeric() {
            if gap && !words.is_empty() {
                words.push(' ');
            }
            gap = false;
            words.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    words
}

/// Whether two names are the same game's, by [`normalised_name`]: equal, and not empty.
pub fn same_name(a: &str, b: &str) -> bool {
    let a = normalised_name(a);
    !a.is_empty() && a == normalised_name(b)
}

/// Whether `other` is the same game as `game` from another launcher: another store's (never one added by hand, nor
/// one of no store), of the same name.
pub fn same_game_elsewhere(game: &Game, other: &Game) -> bool {
    let store = |game: &Game| !game.source.is_empty() && game.source != by_hand::MANUAL;
    store(game) && store(other) && game.source != other.source && same_name(&game.name, &other.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_punctuation_trademarks_and_spaces_do_not_tell_two_names_apart() {
        assert_eq!(normalised_name("Tom Clancy’s Ghost Recon®: Wildlands"), "tom clancys ghost recon wildlands");
        assert!(same_name("DOOM Eternal", "Doom Eternal"));
        assert!(same_name("Hitman: Absolution", "Hitman Absolution"));
        assert!(same_name("Half-Life 2", "Half Life 2"));
        assert!(same_name("Assassin's Creed® Origins", "Assassin’s Creed Origins"));
        assert!(same_name("Control™", "Control"));
        assert!(same_name("  Quiet   Orbit ", "Quiet\u{a0}Orbit"));
        assert!(same_name("Barlin's Gate 3", "BARLINS GATE™ 3"));
        assert!(same_name("Mr. Prepper", "Mr Prepper"));
    }

    #[test]
    fn editions_accents_words_and_numerals_keep_two_names_apart() {
        assert!(!same_name("Quiet Orbit", "Quiet Orbit Deluxe Edition"));
        assert!(!same_name("Quiet Orbit", "Quiet Orbit: Game of the Year Edition"));
        assert!(!same_name("Quiet Orbit", "Quiet Orbit 2"));
        assert!(!same_name("Pokémon Quest", "Pokemon Quest"));
        assert!(!same_name("Thimble & Thorn", "Thimble and Thorn"));
        assert!(!same_name("Fathom II", "Fathom 2"));
        assert!(!same_name("Halflife", "Half-Life"));
    }

    #[test]
    fn a_name_with_no_letter_or_digit_is_the_same_as_nothing() {
        assert_eq!(normalised_name("?!™"), "");
        assert!(!same_name("?!", "?!"));
        assert!(!same_name("", ""));
    }

    #[test]
    fn the_same_game_elsewhere_is_another_stores_of_the_same_name() {
        let game = |source: &str, name: &str| Game { source: source.into(), name: name.into(), ..Game::default() };
        let steam = game("steam", "Quiet Orbit");
        assert!(same_game_elsewhere(&steam, &game("epic", "QUIET ORBIT™")));
        assert!(!same_game_elsewhere(&steam, &game("steam", "Quiet Orbit")), "the same store's is not another's");
        assert!(!same_game_elsewhere(&steam, &game("epic", "Quiet Orbit Deluxe Edition")));
        assert!(!same_game_elsewhere(&steam, &game(by_hand::MANUAL, "Quiet Orbit")), "one added by hand is no store's");
        assert!(!same_game_elsewhere(&steam, &game("", "Quiet Orbit")));
    }
}
