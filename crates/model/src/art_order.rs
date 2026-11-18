//! The order the online art sources are asked in, and which are on (Settings > Games & apps > Art, "Art sources";
//! docs/spec/settings.md 16.8, docs/sources.md "Art"). The art lookup asks them in this order for each kind of
//! picture a game lacks, taking the first that has it; the Art picker lists SteamGridDB's pictures only while
//! SteamGridDB is on. Kept in the settings table as one row, [`KEY`]: the sources' keys in order, comma-separated,
//! one that is off written with a `-` before it (`steam,xbox,-playstation,steamgriddb`).

/// The settings row that holds the order.
pub const KEY: &str = "art.order";

/// An online source of art.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtSource {
    /// Steam's store: its public search and its CDN.
    Steam,
    /// The Microsoft Store's public catalogue.
    Xbox,
    /// PlayStation's Store.
    PlayStation,
    /// SteamGridDB, with the user's own key.
    SteamGridDb,
}

impl ArtSource {
    /// The default order, Playnite's: Steam, Xbox, PlayStation, SteamGridDB.
    pub const ALL: [ArtSource; 4] = [ArtSource::Steam, ArtSource::Xbox, ArtSource::PlayStation, ArtSource::SteamGridDb];

    /// Its key in the settings row. Never changed: a database has it.
    pub fn key(self) -> &'static str {
        match self {
            ArtSource::Steam => "steam",
            ArtSource::Xbox => "xbox",
            ArtSource::PlayStation => "playstation",
            ArtSource::SteamGridDb => "steamgriddb",
        }
    }

    /// Its name as Settings shows it.
    pub fn name(self) -> &'static str {
        match self {
            ArtSource::Steam => "Steam",
            ArtSource::Xbox => "Xbox",
            ArtSource::PlayStation => "PlayStation",
            ArtSource::SteamGridDb => "SteamGridDB",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|source| source.key() == key)
    }
}

/// Every source, in the order asked, each on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtOrder(pub [(ArtSource, bool); 4]);

impl Default for ArtOrder {
    fn default() -> Self {
        Self(ArtSource::ALL.map(|source| (source, true)))
    }
}

impl ArtOrder {
    /// The order a settings row holds: its sources in its order, each once, the ones it does not name after them in
    /// the default order and on; a word it does not know is passed over. None or an empty row is the default.
    pub fn read(text: Option<&str>) -> Self {
        let mut listed: Vec<(ArtSource, bool)> = Vec::new();
        for word in text.unwrap_or_default().split(',').map(str::trim) {
            let (key, on) = match word.strip_prefix('-') {
                Some(key) => (key.trim(), false),
                None => (word, true),
            };
            if let Some(source) = ArtSource::from_key(&key.to_ascii_lowercase()) {
                if !listed.iter().any(|(s, _)| *s == source) {
                    listed.push((source, on));
                }
            }
        }
        for source in ArtSource::ALL {
            if !listed.iter().any(|(s, _)| *s == source) {
                listed.push((source, true));
            }
        }
        let mut order = Self::default();
        for (slot, entry) in order.0.iter_mut().zip(listed) {
            *slot = entry;
        }
        order
    }

    /// The row as the settings table keeps it.
    pub fn text(&self) -> String {
        let words: Vec<String> =
            self.0.iter().map(|(source, on)| format!("{}{}", if *on { "" } else { "-" }, source.key())).collect();
        words.join(",")
    }

    /// The sources that are on, in order: what the lookup asks.
    pub fn on(&self) -> Vec<ArtSource> {
        self.0.iter().filter(|(_, on)| *on).map(|(source, _)| *source).collect()
    }

    pub fn is_on(&self, source: ArtSource) -> bool {
        self.0.iter().any(|(s, on)| *s == source && *on)
    }

    /// A source's number in the order as Settings > Art shows it: its place among the sources that are on, from 1;
    /// none while it is off.
    pub fn number(&self, source: ArtSource) -> Option<usize> {
        self.on().iter().position(|s| *s == source).map(|at| at + 1)
    }

    /// Where a source is, from 0.
    pub fn place(&self, source: ArtSource) -> usize {
        self.0.iter().position(|(s, _)| *s == source).unwrap_or(0)
    }

    /// The order with `source` moved `by` places (up is -1), no further than either end.
    pub fn moved(&self, source: ArtSource, by: i32) -> Self {
        let mut order = *self;
        let at = self.place(source);
        let to = (at as i64 + by as i64).clamp(0, order.0.len() as i64 - 1) as usize;
        let entry = order.0[at];
        if to < at {
            order.0.copy_within(to..at, to + 1);
        } else {
            order.0.copy_within(at + 1..=to, at);
        }
        order.0[to] = entry;
        order
    }

    /// The order with `source` turned on or off.
    pub fn switched(&self, source: ArtSource) -> Self {
        let mut order = *self;
        for (s, on) in order.0.iter_mut() {
            if *s == source {
                *on = !*on;
            }
        }
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ArtSource::*;

    #[test]
    fn the_default_is_playnites_order_all_on() {
        let order = ArtOrder::default();
        assert_eq!(order.on(), [Steam, Xbox, PlayStation, SteamGridDb]);
        assert_eq!(order.text(), "steam,xbox,playstation,steamgriddb");
        assert_eq!(ArtOrder::read(None), order);
        assert_eq!(ArtOrder::read(Some("")), order);
    }

    #[test]
    fn a_row_is_read_back_as_it_was_written_and_a_garbled_one_is_mended() {
        let order = ArtOrder::default().moved(SteamGridDb, -3).switched(PlayStation);
        assert_eq!(order.text(), "steamgriddb,steam,xbox,-playstation");
        assert_eq!(ArtOrder::read(Some(&order.text())), order);
        assert_eq!(order.on(), [SteamGridDb, Steam, Xbox]);
        // Unknown words passed over, a source twice taken once, the missing ones after and on.
        let read = ArtOrder::read(Some(" XBOX , epic, -xbox, -steamgriddb"));
        assert_eq!(read.text(), "xbox,-steamgriddb,steam,playstation");
    }

    #[test]
    fn a_source_moves_one_place_at_a_time_and_no_further_than_the_ends() {
        let order = ArtOrder::default();
        assert_eq!(order.moved(Xbox, -1).text(), "xbox,steam,playstation,steamgriddb");
        assert_eq!(order.moved(Xbox, 1).text(), "steam,playstation,xbox,steamgriddb");
        assert_eq!(order.moved(Steam, -1), order);
        assert_eq!(order.moved(SteamGridDb, 1), order);
        assert_eq!(order.switched(Steam).switched(Steam), order);
        assert!(!order.switched(Steam).is_on(Steam));
    }

    #[test]
    fn a_source_is_numbered_among_those_that_are_on_and_one_that_is_off_has_no_number() {
        let order = ArtOrder::default();
        assert_eq!(ArtSource::ALL.map(|source| order.number(source)), [Some(1), Some(2), Some(3), Some(4)]);
        let off = order.switched(Xbox);
        assert_eq!([Steam, Xbox, PlayStation, SteamGridDb].map(|s| off.number(s)), [Some(1), None, Some(2), Some(3)]);
        let moved = off.moved(SteamGridDb, -3);
        assert_eq!([SteamGridDb, Steam, Xbox, PlayStation].map(|s| moved.number(s)), [Some(1), Some(2), None, Some(3)]);
    }
}
