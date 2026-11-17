//! Choosing a game's pictures from SteamGridDB's candidates (docs/spec/art-picker.md, #213): the kinds of picture
//! the Dashboard draws, the user's choice of each, and the paging of a kind's candidates, with no I/O. The library
//! fetches the pages (`library::art::picker`), the screens show them (`screens::art_picker`).
//!
//! **Paging.** SteamGridDB pages its lists itself: `page` from 0, `limit` at most 50, and each reply says its
//! `page`, `limit` and the `total` of candidates (docs/spec/art-picker.md, section 2). The picker's own pages are
//! whole grids, of a size the kind's proportions set, which is not the service's. [`Pager`] keeps the candidates in
//! the service's order as they come, asks for the service's pages one at a time and in order, each once, only as far
//! as the grid page on screen needs and never past the last the total allows, and makes the grid's pages from what
//! it has. A candidate the service gives twice (its list moved between two requests) is shown once.
use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::GameKey;

/// A kind of picture the Dashboard draws, and so a tab of the picker. The landscape header has none: it is never
/// drawn at its own shape, only cropped in a cover's place when a game has no cover (docs/spec/art-picker.md, 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Square,
    Cover,
    /// The hero: Home's background and the Game hub's.
    Background,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Square, Tab::Cover, Tab::Background];

    /// Its place among the tabs, and in [`GameArt`]'s arrays.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The tab's label.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Square => "Square",
            Tab::Cover => "Cover",
            Tab::Background => "Background",
        }
    }

    /// The kind as the library database's `art` table and our caches name it: `square`, `cover`, `background`.
    pub fn key(self) -> &'static str {
        match self {
            Tab::Square => "square",
            Tab::Cover => "cover",
            Tab::Background => "background",
        }
    }

    pub fn from_key(key: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|tab| tab.key() == key)
    }

    /// Where the picture shows, in one line under the tabs.
    pub fn shows(self) -> &'static str {
        match self {
            Tab::Square => "Shows on square tiles: Home's Recent row, Search, the Guide and the Queue.",
            Tab::Cover => "Shows on portrait tiles: My games & apps and Full library.",
            Tab::Background => "Shows behind Home and the Game hub when the game is focused.",
        }
    }

    /// Width over height, as the picture is shown: what a candidate's cell in the grid is.
    pub fn aspect(self) -> f32 {
        match self {
            Tab::Square => 1.0,
            Tab::Cover => 600.0 / 900.0,
            Tab::Background => 1920.0 / 620.0,
        }
    }

    /// The grid's rows and columns on one page (docs/spec/art-picker.md, 3.3): as many rows as keep a cell large
    /// enough to judge, as many columns as fit across.
    pub fn grid(self) -> (usize, usize) {
        match self {
            Tab::Square => (2, 5),
            Tab::Cover => (2, 7),
            Tab::Background => (4, 3),
        }
    }

    /// Cells on one page of the grid.
    pub fn per_page(self) -> usize {
        let (rows, columns) = self.grid();
        rows * columns
    }
}

/// What the user chose for one kind of a game's picture. No choice is "Automatic": the Dashboard's own pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Chosen {
    /// The store's own picture, the one its client keeps on the PC.
    Store,
    /// A SteamGridDB picture, by its id there.
    Picture(u64),
}

/// A game's pictures as the picker sees them: what the user chose of each kind, and the store's own picture of each
/// where its client keeps one. Indexed by [`Tab::index`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameArt {
    #[serde(default)]
    pub chosen: [Option<Chosen>; 3],
    #[serde(default)]
    pub store: [Option<PathBuf>; 3],
}

impl GameArt {
    pub fn chosen(&self, tab: Tab) -> Option<&Chosen> {
        self.chosen[tab.index()].as_ref()
    }

    pub fn store(&self, tab: Tab) -> Option<&PathBuf> {
        self.store[tab.index()].as_ref()
    }
}

/// What A chose on a tab: the user's choice to keep (`None`: Automatic, the Dashboard's own pick back), and for a
/// SteamGridDB picture where its full picture is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub game: GameKey,
    pub tab: Tab,
    pub chosen: Option<Chosen>,
    /// A SteamGridDB picture's own address, to download: the full picture, not the thumbnail the grid showed.
    pub url: Option<String>,
}

/// One of SteamGridDB's candidates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Its id at SteamGridDB.
    pub id: u64,
    /// The full picture's address.
    pub url: String,
    /// A small picture of it, for the grid.
    pub thumb: String,
    pub width: u32,
    pub height: u32,
}

/// One page of the service's list, as it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The page it says this is.
    pub page: u32,
    /// How many candidates it says the list has, all pages.
    pub total: usize,
    /// Its page size; 0 when it did not say.
    pub limit: usize,
    pub items: Vec<Candidate>,
}

/// Why a page did not come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// No SteamGridDB key is kept, or the service has refused it.
    NoKey,
    /// Settings > Art's Download art online is off.
    Offline,
    /// The service refused the key just now.
    KeyRefused,
    /// SteamGridDB does not know the game, by its store's id or by its name.
    NoGame,
    /// The service asked to wait, again and again.
    Busy,
    /// Anything else, in words that name no address.
    Failed(String),
}

impl Failure {
    /// What the picker says of it, plainly.
    pub fn says(&self) -> String {
        match self {
            Failure::NoKey | Failure::KeyRefused => {
                "Enter your SteamGridDB API key in Settings > Art to choose from its pictures.".into()
            }
            Failure::Offline => {
                "Turn on Download art online in Settings > Art to choose from SteamGridDB's pictures.".into()
            }
            Failure::NoGame => "SteamGridDB doesn't know this game.".into(),
            Failure::Busy => "SteamGridDB is busy. Try again in a minute.".into(),
            Failure::Failed(why) => format!("These pictures couldn't be fetched: {why}."),
        }
    }
}

/// What the picker asks of whoever has the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// A page of the service's list of the game's candidates of this kind.
    Page { game: GameKey, name: String, tab: Tab, page: u32 },
    /// A candidate's thumbnail, downloaded to a file the grid can draw.
    Thumb { id: u64, url: String },
}

/// What it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Page {
        game: GameKey,
        tab: Tab,
        page: u32,
        result: Result<Reply, Failure>,
    },
    /// None: it could not be had, and the cell shows the candidate's size instead.
    Thumb {
        id: u64,
        path: Option<PathBuf>,
    },
}

/// The page size asked for: the most the service gives (docs/spec/art-picker.md, 2).
pub const LIMIT: usize = 50;

/// A cell of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell<'a> {
    /// One of the entries before the candidates (Automatic, the store's own), by its place.
    Fixed(usize),
    Candidate(&'a Candidate),
    /// A candidate whose page has not come yet.
    Coming,
}

/// One tab's candidates, as far as they have come, and its grid's pages made of them.
#[derive(Debug, Clone)]
pub struct Pager {
    /// The entries before the candidates, on the first page.
    fixed: usize,
    per_page: usize,
    /// The candidates, in the service's order, each once.
    got: Vec<Candidate>,
    seen: HashSet<u64>,
    /// Candidates the service gave again, which are not shown again.
    repeats: usize,
    /// The next service page to ask for: every one before it has come.
    next: u32,
    /// Its page size, as its first reply said.
    limit: Option<usize>,
    /// The total its last reply said.
    total: Option<usize>,
    /// There is no service page after those that have come.
    end: bool,
    /// A page asked for and not yet answered.
    asked: Option<u32>,
    /// The last page asked for failed, and why: nothing more is asked until [`Pager::retry`].
    failed: Option<Failure>,
}

impl Pager {
    /// A tab's pager: `fixed` entries before the candidates, `per_page` cells to a page of the grid.
    pub fn new(fixed: usize, per_page: usize) -> Self {
        Self {
            fixed,
            per_page: per_page.max(1),
            got: Vec::new(),
            seen: HashSet::new(),
            repeats: 0,
            next: 0,
            limit: None,
            total: None,
            end: false,
            asked: None,
            failed: None,
        }
    }

    /// One that asks for nothing: there is no key, or no game to look for.
    pub fn without_service(fixed: usize, per_page: usize, why: Failure) -> Self {
        Self { end: true, failed: Some(why), ..Self::new(fixed, per_page) }
    }

    pub fn per_page(&self) -> usize {
        self.per_page
    }

    /// The candidates there are, as far as is known: all that came once the last page has, else what the service's
    /// total says less what it gave twice.
    pub fn total(&self) -> Option<usize> {
        if self.end {
            return Some(self.got.len());
        }
        self.total.map(|total| total.saturating_sub(self.repeats).max(self.got.len()))
    }

    /// The grid's pages: one at least, for the fixed entries.
    pub fn pages(&self) -> usize {
        let cells = self.fixed + self.total().unwrap_or(0);
        cells.div_ceil(self.per_page).max(1)
    }

    /// Whether the number of pages is known yet: the first reply has come, or nothing will be asked.
    pub fn counted(&self) -> bool {
        self.end || self.total.is_some()
    }

    /// The cells of the grid's page `page` (from 0).
    pub fn cells(&self, page: usize) -> Vec<Cell<'_>> {
        let cells = self.fixed + self.total().unwrap_or(0);
        let start = page * self.per_page;
        let end = (start + self.per_page).min(cells.max(self.fixed));
        (start..end)
            .map(|at| match at.checked_sub(self.fixed) {
                None => Cell::Fixed(at),
                Some(i) => self.got.get(i).map_or(Cell::Coming, Cell::Candidate),
            })
            .collect()
    }

    /// The service page to ask for now so that the grid's page `page` is whole: the next in order, if that page
    /// still lacks candidates, none is asked for already, the last did not fail, and there is one.
    pub fn want(&self, page: usize) -> Option<u32> {
        if self.end || self.asked.is_some() || self.failed.is_some() {
            return None;
        }
        let needed = ((page + 1) * self.per_page).saturating_sub(self.fixed);
        if self.got.len() >= needed && self.total.is_some() {
            return None;
        }
        Some(self.next)
    }

    /// Notes that the page `want` gave is being fetched.
    pub fn asking(&mut self, page: u32) {
        self.asked = Some(page);
    }

    pub fn waiting(&self) -> bool {
        self.asked.is_some()
    }

    pub fn failure(&self) -> Option<&Failure> {
        self.failed.as_ref()
    }

    /// A page has come. One that is not the page asked for (a late answer to an earlier picker) is passed over.
    pub fn took(&mut self, reply: Reply) {
        if self.asked != Some(reply.page) || reply.page != self.next {
            return;
        }
        self.asked = None;
        let limit = *self.limit.get_or_insert(if reply.limit > 0 { reply.limit } else { LIMIT });
        let came = reply.items.len().min(limit);
        for item in reply.items.into_iter().take(limit) {
            if self.seen.insert(item.id) {
                self.got.push(item);
            } else {
                self.repeats += 1;
            }
        }
        self.next += 1;
        self.total = Some(reply.total);
        // A short page is the last, whatever the total said; and none is asked past what the total allows.
        if came < limit || self.next as usize * limit >= reply.total {
            self.end = true;
        }
    }

    /// The page asked for did not come.
    pub fn fail(&mut self, page: u32, why: Failure) {
        if self.asked == Some(page) {
            self.asked = None;
            self.failed = Some(why);
        }
    }

    /// Retry: the page that failed may be asked for again. False when there is nothing to retry, or it can't be
    /// (no key).
    pub fn retry(&mut self) -> bool {
        match self.failed {
            Some(Failure::NoKey | Failure::KeyRefused | Failure::Offline | Failure::NoGame) | None => false,
            Some(_) => {
                self.failed = None;
                true
            }
        }
    }

    /// Where a candidate is among them all, from 0: what its place in the grid is made from.
    pub fn place(&self, id: u64) -> Option<usize> {
        self.got.iter().position(|candidate| candidate.id == id)
    }
}

/// The service pages a grid page's candidates are on, before any repeat: what [`Pager`] asks for, in order, to show
/// it. Empty (`1..=0`) when the page holds fixed entries alone.
pub fn service_pages(grid_page: usize, per_page: usize, fixed: usize, limit: usize) -> RangeInclusive<u32> {
    let first = (grid_page * per_page).saturating_sub(fixed);
    let last = ((grid_page + 1) * per_page).saturating_sub(fixed);
    if last == 0 {
        return 1..=0;
    }
    (first / limit) as u32..=((last - 1) / limit) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: u64) -> Candidate {
        Candidate { id, url: format!("full/{id}"), thumb: format!("thumb/{id}"), width: 600, height: 900 }
    }

    /// A list of `total` candidates served `limit` at a time, as SteamGridDB serves its pages.
    fn serve(list: &[u64], page: u32, limit: usize) -> Reply {
        let start = (page as usize * limit).min(list.len());
        let end = (start + limit).min(list.len());
        Reply { page, total: list.len(), limit, items: list[start..end].iter().map(|&id| candidate(id)).collect() }
    }

    /// Walks the grid from its first page to its last, fetching what each needs; gives the service pages asked for,
    /// in order, and the candidates each grid page shows.
    fn walk(list: &[u64], limit: usize, fixed: usize, per_page: usize) -> (Vec<u32>, Vec<Vec<Option<u64>>>) {
        let mut pager = Pager::new(fixed, per_page);
        let mut asked = Vec::new();
        let mut shown = Vec::new();
        let mut page = 0;
        loop {
            while let Some(want) = pager.want(page) {
                pager.asking(want);
                asked.push(want);
                pager.took(serve(list, want, limit));
            }
            let cells = pager.cells(page);
            assert!(cells.iter().all(|cell| *cell != Cell::Coming), "page {page} is whole");
            shown.push(
                cells
                    .iter()
                    .map(|cell| match cell {
                        Cell::Candidate(c) => Some(c.id),
                        _ => None,
                    })
                    .collect(),
            );
            if page + 1 >= pager.pages() {
                break;
            }
            page += 1;
        }
        (asked, shown)
    }

    fn ids(n: u64) -> Vec<u64> {
        (1..=n).collect()
    }

    /// Every candidate once, in order, across the grid's pages.
    fn flat(shown: &[Vec<Option<u64>>]) -> Vec<u64> {
        shown.iter().flatten().flatten().copied().collect()
    }

    #[test]
    fn none_one_a_page_one_more_and_several_pages_each_page_asked_once_and_in_order() {
        // Two fixed entries, ten cells a page, the service's 50 a page.
        for (total, pages, asked) in
            [(0, 1, vec![0]), (1, 1, vec![0]), (50, 6, vec![0]), (51, 6, vec![0, 1]), (170, 18, vec![0, 1, 2, 3])]
        {
            let list = ids(total);
            let (asked_for, shown) = walk(&list, 50, 2, 10);
            assert_eq!(asked_for, asked, "{total}: each page once, in order, none past the last");
            assert_eq!(shown.len(), pages, "{total}");
            assert_eq!(flat(&shown), list, "{total}: nothing skipped, nothing twice");
            // Every page but the last is full.
            for page in &shown[..shown.len() - 1] {
                assert_eq!(page.len(), 10);
            }
        }
    }

    #[test]
    fn a_service_page_that_does_not_divide_the_grid_page_is_asked_once_across_the_edge() {
        // 14 cells a page (Cover), 2 fixed, the service's 5 a page: grid page 1 needs service pages 2 to 5.
        let list = ids(37);
        let (asked, shown) = walk(&list, 5, 2, 14);
        assert_eq!(asked, (0..8).collect::<Vec<u32>>());
        assert_eq!(flat(&shown), list);
        assert_eq!(service_pages(0, 14, 2, 5), 0..=2);
        assert_eq!(service_pages(1, 14, 2, 5), 2..=5);
        assert_eq!(service_pages(0, 2, 2, 5), 1..=0, "fixed entries alone");
    }

    #[test]
    fn only_what_the_page_on_screen_needs_is_asked() {
        let list = ids(500);
        let mut pager = Pager::new(2, 10);
        assert_eq!(pager.want(0), Some(0));
        pager.asking(0);
        assert_eq!(pager.want(0), None, "one at a time");
        pager.took(serve(&list, 0, 50));
        assert_eq!(pager.pages(), 51);
        // Pages 0 to 4 are on the first service page (2 + 48): no request. Page 5 needs the second.
        for page in 0..5 {
            assert_eq!(pager.want(page), None, "{page}");
        }
        assert_eq!(pager.want(5), Some(1));
    }

    #[test]
    fn a_total_that_changes_between_requests_shows_nothing_twice_and_ends_where_the_list_does() {
        let mut pager = Pager::new(1, 4);
        pager.asking(0);
        pager.took(Reply { page: 0, total: 6, limit: 3, items: [1, 2, 3].map(candidate).to_vec() });
        assert_eq!(pager.pages(), 2);
        // A picture was added at the top meanwhile: the second page repeats 3, and the total is 7.
        assert_eq!(pager.want(1), Some(1));
        pager.asking(1);
        pager.took(Reply { page: 1, total: 7, limit: 3, items: [3, 4, 5].map(candidate).to_vec() });
        assert_eq!(pager.want(1), Some(2));
        pager.asking(2);
        // And then two were deleted: the third page is short, so it is the last.
        pager.took(Reply { page: 2, total: 5, limit: 3, items: vec![candidate(6)] });
        assert_eq!(pager.want(1), None);
        assert_eq!(pager.total(), Some(6));
        let shown: Vec<u64> = (0..pager.pages())
            .flat_map(|page| pager.cells(page))
            .filter_map(|cell| match cell {
                Cell::Candidate(c) => Some(c.id),
                _ => None,
            })
            .collect();
        assert_eq!(shown, [1, 2, 3, 4, 5, 6]);
        assert_eq!(pager.pages(), 2);
    }

    #[test]
    fn a_page_answered_late_or_out_of_turn_is_passed_over() {
        let mut pager = Pager::new(0, 4);
        pager.took(serve(&ids(10), 0, 4));
        assert_eq!(pager.total(), None, "not asked for");
        pager.asking(0);
        pager.took(serve(&ids(10), 1, 4));
        assert!(pager.waiting(), "still waits for page 0");
        pager.took(serve(&ids(10), 0, 4));
        assert_eq!(pager.total(), Some(10));
    }

    #[test]
    fn a_failed_page_is_said_and_asked_again_only_on_retry() {
        let mut pager = Pager::new(2, 10);
        pager.asking(0);
        pager.fail(0, Failure::Failed("SteamGridDB could not be reached".into()));
        assert_eq!(pager.want(0), None);
        assert_eq!(pager.cells(0), [Cell::Fixed(0), Cell::Fixed(1)], "the fixed entries all the same");
        assert!(pager.retry());
        assert_eq!(pager.want(0), Some(0), "the same page again");
        // No key: nothing to retry.
        let mut without = Pager::without_service(1, 10, Failure::NoKey);
        assert!(!without.retry() && without.want(0).is_none());
        assert_eq!((without.pages(), without.cells(0)), (1, vec![Cell::Fixed(0)]));
    }

    #[test]
    fn a_page_still_coming_shows_its_cells_as_coming() {
        let list = ids(120);
        let mut pager = Pager::new(2, 10);
        pager.asking(0);
        pager.took(serve(&list, 0, 50));
        // Grid page 5 is candidates 48..58: 48 and 49 have come, the rest are on the next service page.
        let cells = pager.cells(5);
        assert_eq!(cells.len(), 10);
        assert!(matches!(cells[1], Cell::Candidate(c) if c.id == 50));
        assert_eq!(cells[2], Cell::Coming);
    }
}
