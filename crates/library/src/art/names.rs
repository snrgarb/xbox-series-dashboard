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

/// What may end a name on its own and only says which build of the game a store sells: Steam's "Grand Theft Auto V
/// Enhanced" and "... Legacy" are the consoles' "Grand Theft Auto V™"; "GOTY" is the Game of the Year Edition.
/// Not "Remastered" or "Remake": those are often a game of their own ("... Modern Warfare" and "... Modern Warfare
/// Remastered" are two games).
const BUILDS: [&str; 3] = ["goty", "enhanced", "legacy"];

/// What may end a name and only says which platform the store sells it for (plain, as [`plain_name`] writes it), as
/// the stores were seen to write them (2026-11-18): "PS4 & PS5", "(PlayStation®5)", "Xbox One & Xbox Series X|S",
/// "(PC)".
const PLATFORMS: [&str; 20] = [
    "ps4 and ps5",
    "ps5 and ps4",
    "ps4 ps5",
    "ps4",
    "ps5",
    "playstation 4 and playstation 5",
    "playstation 5 and playstation 4",
    "playstation 4",
    "playstation 5",
    "playstation",
    "xbox one and xbox series x s",
    "xbox one xbox series x s",
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
        if let Some(build) = BUILDS.iter().find(|word| core.ends_with(&format!(" {word}"))) {
            core = core[..core.len() - build.len() - 1].to_string();
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

/// The name as it is first sent to a store's search: its marks (™ ® ©) gone and its spaces single. Nothing but the
/// name.
pub fn search_name(name: &str) -> String {
    let name: String = name.chars().map(|c| if matches!(c, '™' | '®' | '©') { ' ' } else { c }).collect();
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The name's words alone: a mark (™ ® ©) a space between two letters or digits ("Portal™2") and else nothing, a
/// comma between digits nothing ("40,000"), every other mark but an apostrophe a space, the spaces single.
pub fn search_words(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len());
    for (i, &c) in chars.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|i| chars.get(i)).copied();
        let after = chars.get(i + 1).copied();
        let between = |test: fn(&char) -> bool| before.as_ref().is_some_and(test) && after.as_ref().is_some_and(test);
        if c.is_alphanumeric() || c == '\'' {
            out.push(c);
        } else if matches!(c, '™' | '®' | '©') {
            if between(|c| c.is_alphanumeric()) {
                out.push(' ');
            }
        } else if c == ',' && between(char::is_ascii_digit) {
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The searches asked after [`search_name`] when it found no game of the name, in turn until one does: the name's
/// words alone ([`search_words`]), then those without the last word; each only where it differs from those before.
/// The stores' searches differ (seen 2026-11-18): the PlayStation Store's finds nothing for "Warhammer 40,000: Space
/// Marine 2" and the game for "Warhammer 40000 Space Marine 2", the Microsoft Store's the other way about; and the
/// Microsoft Store's finds nothing for "Battlefield 6", but "Battlefield™ 6" among its answers for "Battlefield".
/// What is taken from their answers is still only the same game by the rule above.
pub fn more_searches(name: &str) -> Vec<String> {
    let words = search_words(name);
    let split: Vec<&str> = words.split(' ').collect();
    let shorter = (split.len() >= 2).then(|| split[..split.len() - 1].join(" "));
    let first = search_name(name);
    let mut out: Vec<String> = Vec::new();
    for term in std::iter::once(words.clone()).chain(shorter) {
        if !term.is_empty() && term != first && !out.contains(&term) {
            out.push(term);
        }
    }
    out
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
    fn a_search_is_the_name_then_its_words_alone_then_without_the_last() {
        assert_eq!(search_words("Made-up Duty®: Black Ops II"), "Made up Duty Black Ops II");
        assert_eq!(search_words("Hammer 40,000: Space Crew 2"), "Hammer 40000 Space Crew 2");
        assert_eq!(search_words("Garry's Made-up Mod"), "Garry's Made up Mod");
        assert_eq!(search_words("Oda™ & the Orchard"), "Oda the Orchard");
        assert_eq!(search_words("Fields, Rivers"), "Fields Rivers", "a comma not between digits is a space");
        assert_eq!(
            more_searches("Hammer 40,000: Space Crew 2"),
            ["Hammer 40000 Space Crew 2", "Hammer 40000 Space Crew"]
        );
        assert_eq!(more_searches("Made-up Field™ 6"), ["Made up Field 6", "Made up Field"]);
        assert_eq!(more_searches("Lantern 2"), ["Lantern"], "the same words are not asked twice");
        assert!(more_searches("Hades").is_empty());
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
        // As the stores write them: Steam's builds, the PlayStation Store's platforms.
        assert_eq!(core_name("Made-up Auto V Enhanced"), "made up auto v");
        assert_eq!(core_name("Made-up Auto V Legacy"), "made up auto v");
        assert_eq!(core_name("Made-up Auto V™"), "made up auto v");
        assert_eq!(core_name("Made-up Online (PlayStation®5)"), "made up online");
        assert_eq!(core_name("Made-up Field™ 2042 Xbox One & Xbox Series X|S"), "made up field 2042");
        assert_eq!(core_name("Made-up Hunt — Remastered"), "made up hunt remastered", "a remaster may be its own game");
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
