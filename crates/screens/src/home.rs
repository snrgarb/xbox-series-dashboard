//! Home: one page that scrolls. The top bar, the Recent row, the wide tiles and the Groups, and behind them the
//! focused game's art. docs/spec/home.md has the Bundle's measures and rules, which these are.
//!
//! This file is where focus is and where it goes; each part of the page draws itself in its own module.
pub(crate) mod background;
pub(crate) mod groups;
mod recent;
pub(crate) mod swirl;
mod tile;
mod top_bar;
mod wide;

use std::time::{Duration, Instant};

use model::Library;
use ui::{Button, Ease, Ui};

use self::background::Background;
pub use self::background::{hero_art, DELAY_SECONDS, FADE_SECONDS, LEAVE_SECONDS, SETTLE_SECONDS};
use self::groups::Group;
use self::recent::Row;
pub use self::tile::{
    art_for, cover_art, game_at as tile_at, source, source_badge_at, source_mark, square_art, status_band_at,
    title_band_at, Source,
};
pub use self::top_bar::{Status, TopButton};
use crate::hint_bar::{self, Hint};
use crate::pages::Open;
use crate::theme::ROW_SCALE as S;
use crate::{game_hub, my_games, settings, tile_menu};

/// How many games the Recent row holds, before its Play history tile.
pub const RECENT_GAMES: usize = recent::GAMES;

/// The page slides between its rests, and the rows fade as it enters and leaves the Groups, in this long.
const SLIDE_SECONDS: f32 = 0.3;
/// How far from the screen's top the Group with focus rests.
const GROUP_REST: f32 = 64.0;
/// The double tap's time: a second A on the same game's tile this soon after the first plays the game; with none,
/// the game's Game hub opens when the time is up (docs/spec/game-hub.md, 9.1).
pub const DOUBLE_TAP: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    TopBar(TopButton),
    /// A tile of the Recent row, from the left. The last is the Play history tile.
    Recent(usize),
    Wide(usize),
    /// A card of a Group, from the left. The last is See all.
    Group {
        group: usize,
        card: usize,
    },
}

/// What a press on Home asks the Dashboard to do.
#[derive(Debug, Clone, PartialEq)]
pub enum HomeAction {
    /// A on a game's tile: nothing yet. Its Game hub opens when the double tap's time is up ([`Home::tap`]).
    Chosen,
    /// The second A of a double tap: launch the game at this place in the library.
    Launch(usize),
    /// Open the Guide: the profile was chosen, or the Achievements tile.
    OpenGuide,
    /// Open this page over Home.
    Open(Open),
    /// The Discord tile was chosen: the Guide's Discord tab while there is one, else Settings on Discord.
    Discord,
}

/// An A on a game's tile that may yet be the first of a double tap.
#[derive(Debug, Clone)]
struct Chosen {
    at: Instant,
    tile: Focus,
    game: game_hub::Entry,
}

/// What has become of the last A on a game's tile.
#[derive(Debug, Clone, PartialEq)]
pub enum Tap {
    /// There is none to wait for.
    None,
    /// A second A may still come, for this long.
    Waiting(Duration),
    /// None came: the game's Game hub opens.
    Hub(game_hub::Entry),
}

/// Where the page rests: how far it has risen (the Recent row, the wide tiles and the Groups together), and how
/// far the top bar has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rest {
    pub page: f32,
    pub top_bar: f32,
}

/// Where the page rests with focus at `focus`, on a screen `screen_height` design units tall. On the top bar and
/// the Recent row, as laid out. On the wide tiles, risen until the first Group's header and the top third of its
/// cards show at the screen's foot; the top bar stays. On a Group, its header 64 from the screen's top; the top
/// bar has gone up with the page from the wide tiles' rest, above the screen.
pub fn rest(focus: Focus, groups: usize, screen_height: f32) -> Rest {
    let wide = match groups {
        0 => 0.0,
        _ => (groups::top(0) + (groups::CARDS_TOP + groups::CARD_HEIGHT / 3.0) * S - screen_height).max(0.0),
    };
    match focus {
        Focus::TopBar(_) | Focus::Recent(_) => Rest { page: 0.0, top_bar: 0.0 },
        Focus::Wide(_) => Rest { page: wide, top_bar: 0.0 },
        Focus::Group { group, .. } => {
            let page = groups::top(group) - GROUP_REST;
            Rest { page, top_bar: (page - wide).max(top_bar::BOTTOM) }
        }
    }
}

pub struct Home {
    focus: Focus,
    /// The Recent row's game tile that last had focus: where B on a Group goes back to.
    last_game: usize,
    /// The Recent tile Down last left, which Up from the wide tiles goes back to.
    left_recent: Option<usize>,
    /// The wide tile that last had focus, which Up from the first Group goes back to.
    last_wide: usize,
    /// The last A on a game's tile, while a second may still follow it.
    chosen: Option<Chosen>,
    /// Settings' Show Play history: whether the Recent row ends with the Play history tile.
    play_history: bool,
    background: Background,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            // Home starts on the profile, as the Bundle's does once Start-up has gone; Down is the Recent row.
            focus: Focus::TopBar(TopButton::Profile),
            last_game: 0,
            left_recent: None,
            last_wide: 0,
            chosen: None,
            play_history: true,
            background: Background::default(),
        }
    }
}

impl Home {
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// Settings' Background delay: on, as it starts, the background follows focus along the Recent row once it has
    /// rested on a tile for a second and a half; off, almost at once.
    pub fn set_background_delay(&mut self, on: bool) {
        self.background.delay = on;
    }

    /// Settings' Show Play history: on, as it starts, the Recent row is seven games and the Play history tile;
    /// off, eight games. The row is made again at once: focus that was on the tile is on the game in its place.
    pub fn set_play_history(&mut self, on: bool, library: &Library) {
        if std::mem::replace(&mut self.play_history, on) != on {
            let row = Row::of(library, on);
            self.keep_focus_in(&row, &groups::of(library));
            self.last_game = self.last_game_tile(&row);
            self.left_recent = self.left_recent.filter(|&tile| tile < row.tiles());
        }
    }

    /// How many games the Recent row holds at most.
    pub fn recent_games(&self) -> usize {
        recent::games(self.play_history)
    }

    /// Whether Home is what the player is looking at, or lies under the Guide: the swirl moves only in front.
    pub fn set_in_front(&mut self, in_front: bool) {
        self.background.in_front = in_front;
    }

    /// Puts focus on the Recent row's tile for the game at this place in the library, if it has one.
    pub fn focus_recent(&mut self, game: usize, library: &Library) {
        if game < Row::of(library, self.play_history).games.len() {
            self.go_recent(game, &Row::of(library, self.play_history));
        }
    }

    /// A page that was over Home has gone: Home is as it was left, at the same rest, and its rows take focus back
    /// where they had it (docs/spec/home.md, 7.4). With focus on a wide tile or a Group's card, See all too, it is
    /// there still. With it on the Recent row or the top bar, it is on the Recent row's game last focused: nothing
    /// gives the top bar focus back, nor the Play history tile. But Search and Settings give it back to the button
    /// that opened them, which `to_button` says this was (docs/spec/search.md, 4.2; docs/spec/settings.md, 8.2).
    pub fn uncovered(&mut self, to_button: bool, library: &Library) {
        let row = Row::of(library, self.play_history);
        self.keep_focus_in(&row, &groups::of(library));
        let before = self.focus;
        match self.focus {
            Focus::TopBar(_) if to_button => {}
            Focus::TopBar(_) | Focus::Recent(_) => self.go_recent(self.last_game_tile(&row), &row),
            Focus::Wide(_) | Focus::Group { .. } => {}
        }
        if self.focus != before {
            self.background.focus_moved();
        }
    }

    /// The hint bar's hints with focus where it is: More options and Search on a Group's game card, and nowhere
    /// else on Home, though Menu works on a Recent tile and Y everywhere (docs/spec/home.md, 10.4).
    pub fn hints(&self, library: &Library) -> &'static [Hint] {
        match self.focus {
            Focus::Group { group, card } if groups::of(library).get(group).is_some_and(|g| card < g.games.len()) => {
                hint_bar::MORE_OPTIONS_AND_SEARCH
            }
            _ => hint_bar::NONE,
        }
    }

    /// The game focus is on, by its place in the library.
    pub fn focused_game(&self, library: &Library) -> Option<usize> {
        match self.focus {
            Focus::Recent(i) => (i < Row::of(library, self.play_history).games.len()).then_some(i),
            Focus::Group { group, card } => place(library, groups::of(library).get(group)?.games.get(card)?),
            Focus::TopBar(_) | Focus::Wide(_) => None,
        }
    }

    /// What has become of the last A on a game's tile, at `now`. `has_pad` is whether Home still has the pad:
    /// under the Guide, a page or a game no hub opens, and the A is forgotten. Moving focus does not forget it: as
    /// in the Bundle, the hub of the tile that was chosen opens all the same.
    pub fn tap(&mut self, now: Instant, has_pad: bool) -> Tap {
        let Some(chosen) = self.chosen.clone() else { return Tap::None };
        let since = now.saturating_duration_since(chosen.at);
        if has_pad && since < DOUBLE_TAP {
            return Tap::Waiting(DOUBLE_TAP - since);
        }
        self.chosen = None;
        if has_pad {
            Tap::Hub(chosen.game)
        } else {
            Tap::None
        }
    }

    /// A press of the pad at `now`, which is only to tell a double tap by.
    pub fn press(&mut self, button: Button, library: &Library, now: Instant) -> Option<HomeAction> {
        let before = self.focus;
        let action = self.pressed(button, library, now);
        if self.focus != before {
            self.background.focus_moved();
        }
        action
    }

    fn pressed(&mut self, button: Button, library: &Library, now: Instant) -> Option<HomeAction> {
        let row = Row::of(library, self.play_history);
        let groups = groups::of(library);
        self.keep_focus_in(&row, &groups);
        let open = |page: Open| Some(HomeAction::Open(page));
        // Menu is the Tile menu's, which is then what the tile's A was not waiting for: no hub opens.
        if button == Button::Menu {
            self.chosen = None;
        }
        match (self.focus, button) {
            // Y is Search, wherever focus is.
            (_, Button::Y) => return open(Open::Search),

            // The top bar: along its buttons, stopping at the ends; down to the Recent tile under the button.
            (Focus::TopBar(at), Button::Left) => self.focus = Focus::TopBar(at.left().unwrap_or(at)),
            (Focus::TopBar(at), Button::Right) => self.focus = Focus::TopBar(at.right().unwrap_or(at)),
            (Focus::TopBar(at), Button::Down) => {
                let tiles = recent::rects_at_rest(row.tiles(), None);
                self.go_recent(nearest(at.rect().center().0, tiles.iter().map(|r| r.center().0)), &row);
            }
            (Focus::TopBar(TopButton::Profile), Button::A) => return Some(HomeAction::OpenGuide),
            (Focus::TopBar(TopButton::MyGames), Button::A) => return open(Open::MyGames(my_games::Entry::Games)),
            (Focus::TopBar(TopButton::Search), Button::A) => return open(Open::Search),
            (Focus::TopBar(TopButton::Settings), Button::A) => return open(Open::Settings(settings::Entry::Rail)),

            // The Recent row: along its tiles; up to the top bar's button over the tile; down to the wide tile
            // under it, remembering the tile for the way back.
            (Focus::Recent(i), Button::Left) if i > 0 => self.go_recent(i - 1, &row),
            (Focus::Recent(i), Button::Right) if i + 1 < row.tiles() => self.go_recent(i + 1, &row),
            (Focus::Recent(i), Button::Up) => self.focus = Focus::TopBar(TopButton::nearest(tile_middle(&row, i))),
            (Focus::Recent(i), Button::Down) => {
                self.left_recent = Some(i);
                self.go_wide(wide::nearest(tile_middle(&row, i)));
            }
            (Focus::Recent(i), Button::A) if i < row.games.len() => return self.choose(&row.games[i], Some(i), now),
            // The Play history tile, at once: it has no double tap to wait for.
            (Focus::Recent(_), Button::A) => return open(Open::MyGames(my_games::Entry::PlayHistory)),

            // Menu on a game's tile is its Tile menu: every game of the Recent row and of the Groups, a hidden
            // one's card too. Nothing else on Home has one (docs/spec/tile-menu.md, section 1).
            (Focus::Recent(i), Button::Menu) if i < row.games.len() => {
                return open(tile_menu::entry(&row.games[i], tile_menu::Opener::Home));
            }
            (Focus::Group { group, card }, Button::Menu) if card < groups[group].games.len() => {
                return open(tile_menu::entry(groups[group].games[card], tile_menu::Opener::Home));
            }

            // The wide tiles: along them; up to the Recent tile Down left; down into the first Group, at its start.
            (Focus::Wide(i), Button::Left) if i > 0 => self.go_wide(i - 1),
            (Focus::Wide(i), Button::Right) if i + 1 < wide::COUNT => self.go_wide(i + 1),
            (Focus::Wide(_), Button::Up) => {
                let tile = self.left_recent.filter(|&i| i < row.tiles()).unwrap_or(self.last_game_tile(&row));
                self.go_recent(tile, &row);
            }
            (Focus::Wide(_), Button::Down) if !groups.is_empty() => self.focus = Focus::Group { group: 0, card: 0 },
            (Focus::Wide(wide::DISCORD), Button::A) => return Some(HomeAction::Discord),
            (Focus::Wide(wide::SETTINGS), Button::A) => return open(Open::Settings(settings::Entry::Tile)),
            // As the profile: the achievements are the Guide's.
            (Focus::Wide(wide::ACHIEVEMENTS), Button::A) => return Some(HomeAction::OpenGuide),
            (Focus::Wide(wide::DOWNLOADS), Button::A) => return open(crate::manage_queue::open()),

            // The Groups: along a Group's cards; up and down to the same place in the next Group, or its last
            // card if it is shorter; up from the first to the wide tile last focused; B back to the Recent row.
            (Focus::Group { group, card }, Button::Left) if card > 0 => {
                self.focus = Focus::Group { group, card: card - 1 }
            }
            (Focus::Group { group, card }, Button::Right) if card + 1 < groups[group].cards() => {
                self.focus = Focus::Group { group, card: card + 1 }
            }
            (Focus::Group { group: 0, .. }, Button::Up) => self.go_wide(self.last_wide),
            (Focus::Group { group, card }, Button::Up) => self.go_group(group - 1, card, &groups),
            (Focus::Group { group, card }, Button::Down) if group + 1 < groups.len() => {
                self.go_group(group + 1, card, &groups)
            }
            (Focus::Group { .. }, Button::B) => self.go_recent(self.last_game_tile(&row), &row),
            (Focus::Group { group, card }, Button::A) => {
                let Some(game) = groups[group].games.get(card) else {
                    // See all: Full library on this Group's tab.
                    return open(Open::MyGames(my_games::Entry::FullLibrary { filter: groups[group].filter }));
                };
                // A hidden game's card (the Hidden Group, where the user shows it) has a hub, but no place to
                // launch from.
                return self.choose(game, place(library, game), now);
            }
            _ => {}
        }
        None
    }

    /// A on a game's tile. The first waits: the game's Game hub opens if no second follows within the double tap's
    /// time (`tap`). A second on the same tile within it plays the game, from its `place` in the library, and no
    /// hub opens.
    fn choose(&mut self, game: &model::Game, place: Option<usize>, now: Instant) -> Option<HomeAction> {
        let first = self.chosen.take();
        if first.is_some_and(|first| first.tile == self.focus && now.saturating_duration_since(first.at) < DOUBLE_TAP) {
            return place.map(HomeAction::Launch);
        }
        self.chosen = Some(Chosen { at: now, tile: self.focus, game: game_hub::Entry::of(game) });
        Some(HomeAction::Chosen)
    }

    fn go_recent(&mut self, tile: usize, row: &Row) {
        if tile < row.games.len() {
            self.last_game = tile;
        }
        self.focus = Focus::Recent(tile);
    }

    fn go_wide(&mut self, tile: usize) {
        self.last_wide = tile;
        self.focus = Focus::Wide(tile);
    }

    fn go_group(&mut self, group: usize, card: usize, groups: &[Group]) {
        self.focus = Focus::Group { group, card: card.min(groups[group].cards() - 1) };
    }

    /// The Recent row's game tile that last had focus, or with no games the Play history tile.
    fn last_game_tile(&self, row: &Row) -> usize {
        self.last_game.min(row.games.len().saturating_sub(1))
    }

    /// The library may have changed under Home: focus on a tile that is gone moves to the nearest that is left.
    fn keep_focus_in(&mut self, row: &Row, groups: &[Group]) {
        match self.focus {
            Focus::Recent(i) if i >= row.tiles() => self.go_recent(row.tiles() - 1, row),
            Focus::Group { .. } if groups.is_empty() => self.go_recent(self.last_game_tile(row), row),
            Focus::Group { group, card } => self.go_group(group.min(groups.len() - 1), card, groups),
            _ => {}
        }
    }

    pub fn draw(&mut self, ui: &mut Ui, library: &Library, status: &Status) {
        let row = Row::of(library, self.play_history);
        let groups = groups::of(library);
        self.keep_focus_in(&row, &groups);
        let (mut on_bar, mut on_recent, mut on_wide, mut on_group) = (None, None, None, None);
        match self.focus {
            Focus::TopBar(button) => on_bar = Some(button),
            Focus::Recent(i) => on_recent = Some(i),
            Focus::Wide(i) => on_wide = Some(i),
            Focus::Group { group, card } => on_group = Some((group, card)),
        }

        // Only a Recent game's art shows; with focus anywhere else, the swirl.
        self.background.draw(ui, on_recent.and_then(|i| row.games.get(i)));

        // The page and the top bar slide to where they rest with focus here. The Recent row and the wide tiles
        // fade out as the page rises into the Groups, where the wide tiles' foot would show over the header.
        let rest = rest(self.focus, groups.len(), ui.screen().h);
        let page = ui.slide("home page", rest.page, SLIDE_SECONDS, Ease::CubicOut);
        let bar = ui.slide("home top bar", rest.top_bar, SLIDE_SECONDS, Ease::CubicOut);
        let rows = ui.tween("home rows", if on_group.is_some() { 0.0 } else { 1.0 }, SLIDE_SECONDS, Ease::CubicOut);

        ui.with_offset(0.0, -bar, |ui| top_bar::draw(ui, status, on_bar));
        ui.with_offset(0.0, -page, |ui| {
            if rows > 0.0 {
                ui.with_alpha(rows, |ui| {
                    recent::draw(ui, &row, &library.queue, on_recent);
                    let score = library.achievements.totals.map(|totals| totals.score);
                    wide::draw(ui, on_wide, library.queue.len(), score);
                });
            }
            groups::draw(ui, &groups, &library.queue, on_group);
        });
    }
}

/// A game's place in the library's games, which a hidden game has none in.
fn place(library: &Library, game: &model::Game) -> Option<usize> {
    library.games.iter().position(|g| std::ptr::eq(g, game))
}

/// The middle, across, of the Recent row's tile `i` while it has focus.
fn tile_middle(row: &Row, i: usize) -> f32 {
    recent::rects_at_rest(row.tiles(), Some(i))[i].center().0
}

/// Which of `middles` is nearest `x`.
fn nearest(x: f32, middles: impl Iterator<Item = f32>) -> usize {
    let distances = middles.map(|middle| (middle - x).abs());
    distances.enumerate().min_by(|a, b| a.1.total_cmp(&b.1)).map_or(0, |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.05
    }

    /// docs/spec/home.md, section 7.2.
    #[test]
    fn the_page_rests_where_the_spec_says() {
        for focus in [Focus::TopBar(TopButton::Search), Focus::Recent(3)] {
            assert_eq!(rest(focus, 3, 1080.0), Rest { page: 0.0, top_bar: 0.0 });
        }
        let wide = rest(Focus::Wide(0), 3, 1080.0);
        assert!(close(wide.page, 175.05) && wide.top_bar == 0.0, "{wide:?}");
        let first = rest(Focus::Group { group: 0, card: 0 }, 3, 1080.0);
        assert!(close(first.page, 1024.90) && close(first.top_bar, 849.85), "{first:?}");
        let third = rest(Focus::Group { group: 2, card: 4 }, 3, 1080.0);
        assert!(close(third.page, 1024.90 + 2.0 * 430.54) && close(third.top_bar, 849.85 + 2.0 * 430.54), "{third:?}");
    }

    #[test]
    fn the_wide_tiles_rest_follows_the_screen() {
        // Tall enough that a third of the first Group's cards shows already: the page stays.
        assert_eq!(rest(Focus::Wide(0), 2, 1400.0).page, 0.0);
        // With no Groups there is nothing to bring up.
        assert_eq!(rest(Focus::Wide(0), 0, 1080.0).page, 0.0);
        // On a Group the top bar is above the screen however little the page rose to reach it.
        assert!(rest(Focus::Group { group: 0, card: 0 }, 1, 1080.0).top_bar >= top_bar::BOTTOM);
    }

    #[test]
    fn the_groups_are_laid_out_as_the_spec_says() {
        assert!(close(groups::top(0), 1088.90));
        assert!(close(groups::top(1), 1519.44));
        let card = groups::card_rect(0, 6);
        assert!(close(card.y - groups::top(0), 56.23) && close(card.w, 216.12) && close(card.h, 329.75));
        assert!(close(card.right(), crate::theme::ROW_RIGHT), "seven cards span the rows' width: {card:?}");
    }
}
