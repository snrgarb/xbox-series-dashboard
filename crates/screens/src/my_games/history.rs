//! Play history: every played game, the last played first, a row each, under the days they were played
//! (docs/spec/my-games.md, section 6). The rows and their sections are the library database's (`Library::history`).
use std::time::{Duration, Instant};

use model::{Ago, Day, Game, HistoryRow};
use ui::{Button, Crop, HAlign, Press, Rect, Ui, VAlign};

use super::look::{self, LINE, ROW, SECTION, TEXT_DIM, TEXT_SEMIBOLD};
use super::nav;
use super::{open_game_hub, open_tile_menu, Look, Outcome, Pane};
use crate::hint_bar::{self, Hint};
use crate::home::cover_art;
use crate::pages::{Around, PageAction, Tick};
use crate::{theme, Effect};

/// The list's top: the content row's, under the header.
const TOP: f32 = 190.0;
/// A section's header: its text 32 in from the content's edge, and the rows 54 under its top.
const HEADER_LEFT: f32 = 552.0;
const HEADER: f32 = 54.0;
/// A row's item, 176 tall and 7 from the next, 5 in from the content's left edge (WPF's group indent, which the
/// Bundle's pictures show); its back is 4 inside it.
const ITEM_LEFT: f32 = 525.0;
const ITEM_RIGHT: f32 = 1750.0;
const ITEM: f32 = 176.0;
const PITCH: f32 = 183.0;
const BACK_INSET: f32 = 4.0;
/// After a section's last row.
const SECTION_GAP: f32 = 24.0;
/// Under the last row of all.
const END_PADDING: f32 = 80.0;
/// The text beside the cover: 22 right of it, to 24 inside the back's right edge; its two lines 18 apart.
const TEXT_LEFT: f32 = 22.0;
const TEXT_RIGHT: f32 = 24.0;
const LINES_APART: f32 = 18.0;
/// A second A on the same row this soon after the first plays the game.
const DOUBLE_TAP: Duration = Duration::from_millis(300);

/// The date last played, as the Bundle's pictures have it. The Bundle writes it in the user's regional short
/// format; nothing here reads Windows' yet, so it is US English's, month first. **The hook for the user's own.**
fn short_date(day: Day) -> String {
    format!("{}/{}/{}", day.month, day.day, day.year)
}

/// What stands after the date: the game's platform. Every source there is gives Windows games, which the Bundle's
/// library calls "PC (Windows)"; a game of no source has none, and its bullet stands alone, as the Bundle's does.
fn platform(game: &Game) -> Option<&'static str> {
    (!game.source.is_empty()).then_some("PC (Windows)")
}

/// Where each row's item is, down from the list's top, and the header over it if it is its section's first; and
/// how tall they all are together.
fn layout(rows: &[HistoryRow]) -> (Vec<(f32, Option<Ago>)>, f32) {
    let mut y = 0.0;
    let mut section = None;
    let places = rows.iter().map(|row| {
        let first = section != Some(row.ago);
        if first {
            y += if section.is_some() { SECTION_GAP } else { 0.0 } + HEADER;
            section = Some(row.ago);
        }
        let top = y;
        y += PITCH;
        (top, first.then_some(row.ago))
    });
    let places: Vec<_> = places.collect();
    (places, y + if rows.is_empty() { 0.0 } else { SECTION_GAP })
}

/// How far the list is scrolled to show row `focus`, from `scroll`: the least movement that brings the row and its
/// ring into a view `view` tall, as WPF's lists do, with its section's header if it is the section's first row and
/// the end's padding if it is the last row of all.
fn scroll_to(scroll: f32, focus: usize, places: &[(f32, Option<Ago>)], total: f32, view: f32) -> f32 {
    let Some(&(top, header)) = places.get(focus) else { return 0.0 };
    let from = if header.is_some() { top - HEADER } else { top - BACK_INSET };
    let to = if focus + 1 == places.len() { total + END_PADDING } else { top + ITEM + BACK_INSET };
    let scroll = if from < scroll { from } else { scroll.max(to - view) };
    scroll.clamp(0.0, (total + END_PADDING - view).max(0.0))
}

#[derive(Default)]
pub struct History {
    focus: usize,
    /// The game whose row focus is on, or last was: where it goes back to.
    game: Option<i64>,
    scroll: f32,
    /// When A last chose a row, and whose.
    chosen: Option<(Instant, i64)>,
}

impl History {
    /// The library may have changed: focus stays on its game's row, or goes to the first. False with no rows.
    fn keep(&mut self, rows: &[HistoryRow]) -> bool {
        if rows.is_empty() {
            (self.focus, self.game) = (0, None);
            return false;
        }
        if let Some(was) = self.game.filter(|&was| rows.get(self.focus).map(|row| row.game) != Some(was)) {
            self.focus = rows.iter().position(|row| row.game == was).unwrap_or(0);
        }
        self.focus = self.focus.min(rows.len() - 1);
        self.game = Some(rows[self.focus].game);
        true
    }
}

impl Pane for History {
    fn nav_count(&self, around: Around) -> Option<usize> {
        Some(around.library.history.len())
    }

    fn enter(&mut self, around: Around) -> bool {
        self.keep(&around.library.history)
    }

    fn holds_focus(&mut self, around: Around) -> bool {
        self.keep(&around.library.history)
    }

    fn focus(&self) -> Option<String> {
        Some(format!("Row({})", self.focus))
    }

    /// The row A chose, once the double tap's time has passed with no second A: its game's hub opens, wherever
    /// focus has gone on the list since, as on Home.
    fn tick(&mut self, around: Around) -> Tick {
        let Some((at, chosen)) = self.chosen else { return Tick::Nothing };
        let since = around.now.saturating_duration_since(at);
        if since < DOUBLE_TAP {
            return Tick::Wait(DOUBLE_TAP - since);
        }
        self.chosen = None;
        // Long over, with the pane not asked meanwhile: something else was shown or had the pad, and the choice
        // went with it.
        if since > DOUBLE_TAP * 3 {
            return Tick::Nothing;
        }
        // A game that has left the library since has no hub to show.
        let game = around.library.games.iter().find(|game| game.id == chosen);
        game.and_then(open_game_hub).map_or(Tick::Nothing, Tick::Act)
    }

    fn press(&mut self, press: Press, around: Around) -> Outcome {
        let rows = &around.library.history;
        if !self.keep(rows) {
            return Outcome::Nav;
        }
        let row = rows[self.focus];
        let place = around.library.games.iter().position(|game| game.id == row.game);
        let once = !press.repeat;
        // Anything but A and a move along the list drops the row A chose: its hub does not open over what the
        // press brought.
        if press.button != Button::A && !matches!(press.button, Button::Up | Button::Down) {
            self.chosen = None;
        }
        match press.button {
            // Across the sections, stopping at the ends.
            Button::Up => self.focus = self.focus.saturating_sub(1),
            Button::Down => self.focus = (self.focus + 1).min(rows.len() - 1),
            Button::Left => return Outcome::Nav,
            Button::A if once => {
                let Some(place) = place else { return Outcome::Stay };
                let now = around.now;
                let again = self
                    .chosen
                    .is_some_and(|(at, game)| game == row.game && now.saturating_duration_since(at) < DOUBLE_TAP);
                if again {
                    // The second A of a double tap plays the game.
                    self.chosen = None;
                    return Outcome::Action(PageAction::Effect(Effect::Launch(place)));
                }
                // The first opens its Game hub when the double tap's time has passed with no second A (`tick`).
                self.chosen = Some((now, row.game));
            }
            Button::Menu if once => {
                let menu = place.and_then(|place| open_tile_menu(&around.library.games[place]));
                return menu.map_or(Outcome::Stay, Outcome::Action);
            }
            _ => {}
        }
        self.game = Some(rows[self.focus].game);
        Outcome::Stay
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::MORE_OPTIONS
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, look: Look) {
        let rows = &around.library.history;
        if !self.keep(rows) {
            ui.with_alpha(look.fade, |ui| ui.label(548.0, TOP, "Games you play show up here.", &TEXT_DIM));
            return;
        }
        let screen = ui.screen();
        let view = screen.h - TOP;
        let (places, total) = layout(rows);
        // There is no slide: the list is where the focused row needs it at once, as the Bundle's is.
        self.scroll = scroll_to(self.scroll, self.focus, &places, total, view);
        let scroll = self.scroll;
        let left = nav::WIDTH + 2.0;
        let item = |top: f32| Rect::new(ITEM_LEFT, TOP + top - scroll, ITEM_RIGHT - ITEM_LEFT, ITEM);
        ui.with_alpha(look.fade, |ui| {
            ui.with_clip(Rect::new(left, TOP, screen.right() - left, view), |ui| {
                for (row, &(top, header)) in rows.iter().zip(&places) {
                    let item = item(top);
                    if item.bottom() < TOP || item.y - HEADER > screen.h {
                        continue;
                    }
                    if let Some(section) = header {
                        ui.label(HEADER_LEFT, item.y - HEADER, section.title(), &SECTION);
                    }
                    let back = item.grow(-BACK_INSET);
                    ui.fill(back, ROW, 0.0);
                    // The cover: a square at the back's left, the dark tile until it is in, and for want of one.
                    let cover = Rect::new(back.x, back.y, back.h, back.h);
                    ui.fill(cover, theme::TILE, 0.0);
                    let Some(game) = around.library.games.iter().find(|game| game.id == row.game) else { continue };
                    if let Some(key) = cover_art(game, ui.px()) {
                        ui.art(cover, &key, Crop::Top, 1.0, 0.0);
                    }
                    let lines = 2.0 * LINE + LINES_APART;
                    let text = cover.right() + TEXT_LEFT;
                    let line = |y: f32| Rect::new(text, y, back.right() - TEXT_RIGHT - text, LINE);
                    let first = back.y + (back.h - lines) / 2.0;
                    ui.paragraph(line(first), &game.name, &TEXT_SEMIBOLD, HAlign::Left, VAlign::Top, 1);
                    let when = format!("{}  \u{2022}  {}", short_date(row.day), platform(game).unwrap_or(""));
                    let second = line(first + LINE + LINES_APART);
                    ui.paragraph(second, when.trim_end(), &TEXT_DIM, HAlign::Left, VAlign::Top, 1);
                }
                // The ring stands 4 outside the focused row's item, over the row under it.
                if look.focused {
                    look::ring(ui, item(places[self.focus].0).grow(4.0));
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(sections: &[(Ago, usize)]) -> Vec<HistoryRow> {
        let day = Day { year: 2026, month: 11, day: 15 };
        let rows =
            sections.iter().flat_map(|&(ago, count)| (0..count).map(move |_| HistoryRow { game: 0, at: 1, day, ago }));
        rows.enumerate().map(|(i, row)| HistoryRow { game: i as i64 + 1, ..row }).collect()
    }

    /// docs/spec/my-games.md, 6.2: the first row's item is 54 under the list's top, a section of n rows takes
    /// 54 + 183 n + 24, and with a row each the sections are 261 apart.
    #[test]
    fn rows_and_sections_stand_where_the_spec_says() {
        let (places, total) =
            layout(&rows(&[(Ago::Today, 1), (Ago::Yesterday, 1), (Ago::LastWeek, 2), (Ago::Older, 1)]));
        let tops: Vec<f32> = places.iter().map(|place| place.0).collect();
        assert_eq!(tops, [54.0, 315.0, 576.0, 759.0, 1020.0]);
        // On screen: the backs at 248, 509, 770 and 953, as the Bundle's picture has them.
        assert_eq!(TOP + tops[2] + BACK_INSET, 770.0);
        let headers: Vec<bool> = places.iter().map(|place| place.1.is_some()).collect();
        assert_eq!(headers, [true, true, true, false, true]);
        assert_eq!(total, 4.0 * (54.0 + 24.0) + 5.0 * 183.0);
        assert_eq!(layout(&[]).1, 0.0);
    }

    #[test]
    fn the_list_moves_the_least_that_shows_the_focused_row() {
        let (places, total) =
            layout(&rows(&[(Ago::Today, 1), (Ago::Yesterday, 1), (Ago::LastWeek, 2), (Ago::Older, 3)]));
        let view = 890.0;
        let to = |scroll: f32, focus: usize| scroll_to(scroll, focus, &places, total, view);
        // The first three rows are in view from the start.
        assert_eq!((to(0.0, 0), to(0.0, 1), to(0.0, 2)), (0.0, 0.0, 0.0));
        // The fourth comes up until its ring's foot is the view's.
        let fourth = 759.0 + 176.0 + 4.0 - view;
        assert_eq!(to(0.0, 3), fourth);
        // Back up, nothing moves while the row is in view; then the row's ring is at the top, or its section's
        // header if it is the section's first row.
        assert_eq!(to(fourth, 2), fourth);
        assert_eq!(to(780.0, 3), 755.0);
        assert_eq!(to(700.0, 2), 576.0 - 54.0);
        assert_eq!(to(300.0, 0), 0.0);
        // The last row of all brings the end's padding with it.
        assert_eq!(to(0.0, 6), total + 80.0 - view);
        // A list shorter than the view never moves.
        let (few, short) = layout(&rows(&[(Ago::Today, 2)]));
        assert_eq!(scroll_to(40.0, 1, &few, short, view), 0.0);
    }

    #[test]
    fn the_date_is_month_day_year_and_the_platform_follows_a_bullet() {
        assert_eq!(short_date(Day { year: 2026, month: 11, day: 5 }), "11/5/2026");
        let steam = Game { source: "steam".into(), ..Game::default() };
        assert_eq!((platform(&steam), platform(&Game::default())), (Some("PC (Windows)"), None));
    }
}
