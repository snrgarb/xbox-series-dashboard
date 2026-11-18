//! The PC Game Pass catalogue of a region (#277; docs/sources.md "Xbox", docs/research/game-pass.md): the games
//! Microsoft's public catalogue lists for PC Game Pass in a market, without a sign-in. **Whether the user has a
//! subscription is not known to the Dashboard**: a game of the catalogue is one that can be installed through the
//! Store with one, nothing more.
//!
//! Which region is the user's choice in Settings > Games & apps > Launchers ([`REGION_KEY`]), Windows' own region
//! until one is chosen; whether the catalogue's games are listed at all is [`SHOWN_KEY`], on until turned off.
use serde::{Deserialize, Serialize};

use crate::OnlineArt;

/// The settings row that holds the region chosen, a market's code (`GB`); no row is Windows' own region.
pub const REGION_KEY: &str = "gamepass.region";

/// The settings row that says whether the catalogue's games are listed (`Setting::ShowGamePass` in the screens).
pub const SHOWN_KEY: &str = "gamepass.shown";

/// A market, by its two-letter ISO 3166 code in capitals (`GB`, `US`): what the catalogue is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Region([u8; 2]);

impl Region {
    /// The region of a code, written in either case (`gb`, `GB`); none for anything that is not two letters.
    pub fn parse(code: &str) -> Option<Self> {
        let code = code.trim().as_bytes();
        match code {
            [a, b] if a.is_ascii_alphabetic() && b.is_ascii_alphabetic() => {
                Some(Self([a.to_ascii_uppercase(), b.to_ascii_uppercase()]))
            }
            _ => None,
        }
    }

    /// `GB`.
    pub fn code(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("US")
    }

    /// Its name, as Settings lists it (`United Kingdom`); its code where it is no market of [`MARKETS`].
    pub fn name(&self) -> &str {
        MARKETS.iter().find(|(code, _)| *code == self.code()).map_or(self.code(), |(_, name)| name)
    }

    /// Whether PC Game Pass is sold there ([`MARKETS`]).
    pub fn sold(&self) -> bool {
        MARKETS.iter().any(|(code, _)| *code == self.code())
    }

    /// Where nothing else says: the United States.
    pub const US: Region = Region(*b"US");
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

/// The markets where PC Game Pass is sold, by code and name, in name order: the "PC Game Pass" column of Xbox's own
/// table of the countries and regions where each of its services is available (`xbox.com/en-US/regions`, read
/// 2026-11-18: docs/research/game-pass.md), with its names. Russia (new sales suspended) and China (not available)
/// are not among them.
pub const MARKETS: &[(&str, &str)] = &[
    ("AL", "Albania"),
    ("DZ", "Algeria"),
    ("AR", "Argentina"),
    ("AU", "Australia"),
    ("AT", "Austria"),
    ("BH", "Bahrain"),
    ("BE", "Belgium"),
    ("BO", "Bolivia"),
    ("BA", "Bosnia & Herzegovina"),
    ("BR", "Brazil"),
    ("BG", "Bulgaria"),
    ("CA", "Canada"),
    ("CL", "Chile"),
    ("CO", "Colombia"),
    ("CR", "Costa Rica"),
    ("HR", "Croatia"),
    ("CY", "Cyprus"),
    ("CZ", "Czechia"),
    ("DK", "Denmark"),
    ("EC", "Ecuador"),
    ("EG", "Egypt"),
    ("SV", "El Salvador"),
    ("EE", "Estonia"),
    ("FI", "Finland"),
    ("FR", "France"),
    ("GE", "Georgia"),
    ("DE", "Germany"),
    ("GR", "Greece"),
    ("GT", "Guatemala"),
    ("HN", "Honduras"),
    ("HK", "Hong Kong SAR"),
    ("HU", "Hungary"),
    ("IS", "Iceland"),
    ("IN", "India"),
    ("ID", "Indonesia"),
    ("IE", "Ireland"),
    ("IL", "Israel"),
    ("IT", "Italy"),
    ("JP", "Japan"),
    ("KR", "Korea"),
    ("KW", "Kuwait"),
    ("LV", "Latvia"),
    ("LY", "Libya"),
    ("LI", "Liechtenstein"),
    ("LT", "Lithuania"),
    ("LU", "Luxembourg"),
    ("MY", "Malaysia"),
    ("MT", "Malta"),
    ("MX", "Mexico"),
    ("MD", "Moldova"),
    ("ME", "Montenegro"),
    ("MA", "Morocco"),
    ("NL", "Netherlands"),
    ("NZ", "New Zealand"),
    ("NI", "Nicaragua"),
    ("MK", "North Macedonia"),
    ("NO", "Norway"),
    ("OM", "Oman"),
    ("PA", "Panama"),
    ("PY", "Paraguay"),
    ("PE", "Peru"),
    ("PH", "Philippines"),
    ("PL", "Poland"),
    ("PT", "Portugal"),
    ("QA", "Qatar"),
    ("RO", "Romania"),
    ("SA", "Saudi Arabia"),
    ("RS", "Serbia"),
    ("SG", "Singapore"),
    ("SK", "Slovakia"),
    ("SI", "Slovenia"),
    ("ZA", "South Africa"),
    ("ES", "Spain"),
    ("SE", "Sweden"),
    ("CH", "Switzerland"),
    ("TW", "Taiwan"),
    ("TH", "Thailand"),
    ("TN", "Tunisia"),
    ("TR", "Turkey"),
    ("UA", "Ukraine"),
    ("AE", "United Arab Emirates"),
    ("GB", "United Kingdom"),
    ("US", "United States"),
    ("UY", "Uruguay"),
    ("VN", "Vietnam"),
];

/// Every market of [`MARKETS`], in their order.
pub fn markets() -> impl Iterator<Item = Region> {
    MARKETS.iter().filter_map(|(code, _)| Region::parse(code))
}

/// The region and language the catalogue is asked in: the market, and a language tag (`en-GB`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locale {
    pub market: String,
    pub language: String,
}

/// One game of the catalogue, as the Microsoft Store's display catalogue tells of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    /// The Store's product id (its "big id", `9NBLGGH4R2R6`): the catalogue lists games by it, and the Store's page
    /// is opened by it.
    pub id: String,
    pub name: String,
    /// The package family name of its PC package, where the catalogue gives one: what an installed Xbox game is
    /// known by (`Game::source_id`), so the installed game and the catalogue's are one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// The products a bundle is made of (`BundledSkus`), by their ids: an installed game's Store id may be one of
    /// them rather than the bundle's own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundled: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub developers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishers: Vec<String>,
    /// When it came out, Unix seconds; 0 where the catalogue does not say.
    #[serde(default)]
    pub released: u64,
    /// Its pictures on the Store's CDN, at the sizes the screens take; `product` its id.
    #[serde(default)]
    pub art: OnlineArt,
}

/// The catalogue of one region, as fetched and kept in the data folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalogue {
    /// The market it is of (`GB`), and the language its names are in (`en-GB`).
    pub market: String,
    pub language: String,
    /// When it was fetched, Unix seconds; 0 for none yet.
    pub fetched: u64,
    /// When it was last asked for and not had, Unix seconds; 0 for never: no new try for an hour after.
    #[serde(default)]
    pub tried: u64,
    /// The list's own validator (`ETag`), for a conditional request the next day, where it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    pub products: Vec<Product>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_is_two_letters_in_either_case() {
        assert_eq!(Region::parse(" gb").map(|r| r.to_string()), Some("GB".into()));
        assert_eq!(Region::parse("GBR"), None);
        assert_eq!(Region::parse("1A"), None);
        assert_eq!(Region::parse("gb").unwrap().name(), "United Kingdom");
        assert_eq!(Region::parse("AQ").unwrap().name(), "AQ");
        assert!(!Region::parse("AQ").unwrap().sold());
        assert!(Region::US.sold());
    }

    #[test]
    fn the_markets_are_in_name_order_each_once() {
        let names: Vec<&str> = MARKETS.iter().map(|(_, name)| *name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        let mut codes: Vec<&str> = MARKETS.iter().map(|(code, _)| *code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), MARKETS.len());
        assert_eq!(markets().count(), MARKETS.len());
    }
}
