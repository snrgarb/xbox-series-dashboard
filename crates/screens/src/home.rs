//! Home: one page that scrolls. The top bar, the Recent row, the wide tiles and the Groups, and behind them the
//! focused game's art. docs/spec/home.md has the Bundle's measures and rules, which these are.
//!
//! This file is where focus is and where it goes; each part of the page draws itself in its own module.
mod background;
mod groups;
mod recent;
mod swirl;
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
pub use self::tile::cover_art;
pub use self::top_bar::{profile_picture, Status, TopButton};
use crate::theme::ROW_SCALE as S;

/// How many games the Recent row holds, before its Play history tile.
pub const RECENT_GAMES: usize = recent::GAMES;

/// The page slides between its rests, and the rows fade as it enters and leaves the Groups, in this long.
const SLIDE_SECONDS: f32 = 0.3;
/// How far from the screen's top the Group with focus rests.
const GROUP_REST: f32 = 64.0;
/// A second A on the same tile this soon after the first is the same choice, not another.
const DOUBLE_TAP: Duration = Duration::from_millis(300);

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HomeAction {
    /// Launch the game at this place in the library.
    Launch(usize),
    /// Open the Guide: the profile was chosen.
    OpenGuide,
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
    /// When A last chose a tile, and which.
    chosen: Option<(Instant, Focus)>,
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

    /// Whether Home is what the player is looking at, or lies under the Guide: the swirl moves only in front.
    pub fn set_in_front(&mut self, in_front: bool) {
        self.background.in_front = in_front;
    }

    /// Puts focus on the Recent row's tile for the game at this place in the library, if it has one.
    pub fn focus_recent(&mut self, game: usize, library: &Library) {
        if game < Row::of(library).games.len() {
            self.go_recent(game, &Row::of(library));
        }
    }

    /// The game focus is on, by its place in the library.
    pub fn focused_game(&self, library: &Library) -> Option<usize> {
        match self.focus {
            Focus::Recent(i) => (i < Row::of(library).games.len()).then_some(i),
            Focus::Group { group, card } => groups::of(library).get(group)?.games.get(card).copied(),
            Focus::TopBar(_) | Focus::Wide(_) => None,
        }
    }

    pub fn press(&mut self, button: Button, library: &Library) -> Option<HomeAction> {
        let before = self.focus;
        let action = self.pressed(button, library);
        if self.focus != before {
            self.background.focus_moved();
        }
        action
    }

    fn pressed(&mut self, button: Button, library: &Library) -> Option<HomeAction> {
        let row = Row::of(library);
        let groups = groups::of(library);
        self.keep_focus_in(&row, &groups);
        match (self.focus, button) {
            // The top bar: along its buttons, stopping at the ends; down to the Recent tile under the button.
            (Focus::TopBar(at), Button::Left) => self.focus = Focus::TopBar(at.left().unwrap_or(at)),
            (Focus::TopBar(at), Button::Right) => self.focus = Focus::TopBar(at.right().unwrap_or(at)),
            (Focus::TopBar(at), Button::Down) => {
                let tiles = recent::rects_at_rest(row.tiles(), None);
                self.go_recent(nearest(at.rect().center().0, tiles.iter().map(|r| r.center().0)), &row);
            }
            (Focus::TopBar(TopButton::Profile), Button::A) => return Some(HomeAction::OpenGuide),

            // The Recent row: along its tiles; up to the top bar's button over the tile; down to the wide tile
            // under it, remembering the tile for the way back.
            (Focus::Recent(i), Button::Left) if i > 0 => self.go_recent(i - 1, &row),
            (Focus::Recent(i), Button::Right) if i + 1 < row.tiles() => self.go_recent(i + 1, &row),
            (Focus::Recent(i), Button::Up) => self.focus = Focus::TopBar(TopButton::nearest(tile_middle(&row, i))),
            (Focus::Recent(i), Button::Down) => {
                self.left_recent = Some(i);
                self.go_wide(wide::nearest(tile_middle(&row, i)));
            }
            (Focus::Recent(i), Button::A) if i < row.games.len() => return self.choose(i),

            // The wide tiles: along them; up to the Recent tile Down left; down into the first Group, at its start.
            (Focus::Wide(i), Button::Left) if i > 0 => self.go_wide(i - 1),
            (Focus::Wide(i), Button::Right) if i + 1 < wide::COUNT => self.go_wide(i + 1),
            (Focus::Wide(_), Button::Up) => {
                let tile = self.left_recent.filter(|&i| i < row.tiles()).unwrap_or(self.last_game_tile(&row));
                self.go_recent(tile, &row);
            }
            (Focus::Wide(_), Button::Down) if !groups.is_empty() => self.focus = Focus::Group { group: 0, card: 0 },

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
                if let Some(&game) = groups[group].games.get(card) {
                    return self.choose(game);
                }
            }
            _ => {}
        }
        None
    }

    /// A on a game's tile launches it. A again on the same tile within the double tap's time asks for nothing:
    /// in the Bundle the pair is one choice (play, where one A opens the Game hub), and it must not launch twice.
    fn choose(&mut self, game: usize) -> Option<HomeAction> {
        let now = Instant::now();
        let again = self.chosen.is_some_and(|(at, tile)| tile == self.focus && now.duration_since(at) < DOUBLE_TAP);
        self.chosen = Some((now, self.focus));
        (!again).then_some(HomeAction::Launch(game))
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
        let row = Row::of(library);
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
                    recent::draw(ui, &row, on_recent);
                    wide::draw(ui, on_wide);
                });
            }
            groups::draw(ui, library, &groups, on_group);
        });
    }
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
