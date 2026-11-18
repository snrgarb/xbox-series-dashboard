//! How a game is matched by name at a store or service that is not its own (docs/sources.md, "Art", "Matching by
//! name"). A wrong picture is worse than none, so the rule is narrow: two names are the same game only when they are
//! the same once written plainly ([`plain_name`]) and stripped of what only says which edition or platform it is
//! ([`core_name`]). Never the first hit of a search regardless, never a name that merely starts or contains the
//! other: "Hadrel" is not "Hadrel II", "Hadrel 2" or "Hadrel: Battle Out of Hollow".

/// A name as names are compared: lower case, its marks (™ ® ©) a space, `&` as "and", and every run of anything but
/// letters and digits one space. "VESPER Eternal™" and "Vesper: Eternal" are one name; "Lantern™2" is "lantern 2";
/// "Vesper" and "Vesper 3" are not one.
pub fn plain_name(name: &str) -> String {
    let name = name.replace('&', " and ");
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// What may stand before "edition" at a name's end and be dropped with it: the edition says which box, not which
/// game. The longer of two that end alike first ("digital deluxe" before "deluxe").
const EDITIONS: [&str; 22] = [
    "standard",
    "digital deluxe",
    "deluxe",
    "digital",
    "gold",
    "complete",
    "definitive",
    "ultimate",
    "premium",
    "game of the year",
    "goty",
    "enhanced",
    "special",
    "collector s",
    "collectors",
    "legendary",
    "launch",
    "anniversary",
    "pc",
    "windows",
    "steam",
    "xbox",
];

/// What may end a name and only says which platform the store sells it for (plain, as [`plain_name`] writes it).
const PLATFORMS: [&str; 14] = [
    "ps4 and ps5",
    "ps5 and ps4",
    "ps4 ps5",
    "ps4",
    "ps5",
    "xbox one and xbox series x s",
    "xbox series x s",
    "xbox one",
    "for windows 10",
    "for windows",
    "windows 10",
    "windows",
    "for pc",
    "pc",
];

/// A plain name without the edition or platform words at its end: "MADE-UP RING PS4 & PS5" and "Made-up Ring Deluxe
/// Edition" are both "made up ring"; "Made-up Ring Shadow of the Tree Edition" stays as it is ("tree" is not an
/// edition word), and so does a name that is nothing but such words.
pub fn core_name(name: &str) -> String {
    let mut core = plain_name(name);
    loop {
        let before = core.clone();
        if let Some(rest) = core.strip_suffix(" goty") {
            core = rest.to_string();
        }
        if let Some(rest) = core.strip_suffix(" edition") {
            if let Some(edition) = EDITIONS.iter().find(|word| rest.ends_with(&format!(" {word}"))) {
                core = rest[..rest.len() - edition.len() - 1].to_string();
            }
        }
        if let Some(platform) = PLATFORMS.iter().find(|word| core.ends_with(&format!(" {word}"))) {
            core = core[..core.len() - platform.len() - 1].to_string();
        }
        if core == before {
            return core;
        }
    }
}

/// Whether two names are one game by the rule above.
pub fn same_game(wanted: &str, found: &str) -> bool {
    let wanted = core_name(wanted);
    !wanted.is_empty() && wanted == core_name(found)
}

/// The one of a search's results that is the game named `wanted`: of those that are [`same_game`], one whose plain
/// name is the very same before one that differs only by an edition or platform, and of those the lowest `rank` (a
/// store's own preference: a full game before a bundle; a verified entry before another), the first of equals in the
/// order the store gave. None when no result is the game.
pub fn pick<'a, T>(wanted: &str, results: &'a [(String, T, u8)]) -> Option<&'a T> {
    let exact = plain_name(wanted);
    results
        .iter()
        .filter(|(name, _, _)| same_game(wanted, name))
        .min_by_key(|(name, _, rank)| (plain_name(name) != exact, *rank))
        .map(|(_, item, _)| item)
}

/// The name as it is sent to a store's search: its marks gone and its spaces single. Nothing but the name.
pub fn search_name(name: &str) -> String {
    let name: String = name.chars().map(|c| if matches!(c, '™' | '®' | '©') { ' ' } else { c }).collect();
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_written_plainly() {
        assert_eq!(plain_name("VESPER Eternal™"), "vesper eternal");
        assert_eq!(plain_name("Vesper: Eternal"), "vesper eternal");
        assert_eq!(plain_name("Oda & the Blind Orchard"), "oda and the blind orchard");
        assert_eq!(plain_name("Lantern™2"), "lantern 2");
        assert_eq!(plain_name("Café®  Racer"), "café racer");
        assert_eq!(search_name("Lantern™2 ®"), "Lantern 2");
    }

    #[test]
    fn edition_and_platform_words_at_the_end_are_set_aside_and_nothing_else() {
        assert_eq!(core_name("MADE-UP RING PS4 & PS5"), "made up ring");
        assert_eq!(core_name("Made-up Game: Deluxe Edition"), "made up game");
        assert_eq!(core_name("Made-up Game Game of the Year Edition"), "made up game");
        assert_eq!(core_name("Made-up Game GOTY"), "made up game");
        assert_eq!(core_name("Made-up Game - Digital Deluxe Edition PS5"), "made up game");
        assert_eq!(core_name("Made-up Game for Windows 10"), "made up game");
        assert_eq!(core_name("Made-up Game (Xbox Series X|S)"), "made up game");
        // Not an edition word: another game, or a part of the name.
        assert_eq!(core_name("Made-up Game Shadow of the Tree Edition"), "made up game shadow of the tree edition");
        assert_eq!(core_name("Made-up Game Remastered"), "made up game remastered");
        assert_eq!(core_name("Made-up Game 2"), "made up game 2");
        // A name that is nothing but such words keeps them.
        assert_eq!(core_name("Deluxe Edition"), "deluxe edition");
        assert_eq!(core_name("PC"), "pc");
    }

    #[test]
    fn only_the_same_game_is_matched() {
        assert!(same_game("Made-up Reforger", "MADE-UP REFORGER™"));
        assert!(same_game("Made-up Game", "Made-up Game PS4 & PS5"));
        assert!(same_game("Made-up Game Deluxe Edition", "Made-up Game"));
        for other in ["Made-up Game II", "Made-up Game 2", "Made-up Game: Rally", "Made-up", "Made-up Game Soundtrack"]
        {
            assert!(!same_game("Made-up Game", other), "{other}");
        }
        assert!(!same_game("", ""));
        assert!(!same_game("™", "™"));
    }

    #[test]
    fn the_very_name_before_an_edition_and_the_stores_own_preference_between_equals() {
        let results = vec![
            ("Hadrel II".to_string(), 1, 0),
            ("Hadrel Deluxe Edition".to_string(), 2, 0),
            ("HADREL".to_string(), 3, 1),
            ("Hadrel".to_string(), 4, 0),
            ("Hadrel's Star".to_string(), 5, 0),
        ];
        assert_eq!(pick("Hadrel", &results), Some(&4));
        assert_eq!(pick("Hadrel", &results[..3]), Some(&3), "the very name, though ranked after");
        assert_eq!(pick("Hadrel", &results[..2]), Some(&2), "an edition of it, with nothing closer");
        assert_eq!(pick("Hadrel 2", &results), None);
        assert_eq!(pick("Hadrel: Battle Out of Hollow", &results), None);
        assert_eq!(pick("", &results), None);
    }
}
