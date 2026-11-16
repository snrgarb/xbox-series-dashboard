//! Apps (docs/spec/my-games.md, section 8; docs/spec/apps.md): the Apps in a grid of square tiles, five to a row, in
//! the order they were added. A starts the App focus is on, at once and whatever is running: no Launch screen, no
//! Play session. Left from the first column goes to the nav; every other edge stops. No Tile menu, no hints.
use model::App;
use ui::{Button, Press, Rect, Ui};

use super::look::TEXT_DIM;
use super::nav;
use super::{Look, Outcome, Pane};
use crate::app_tile;
use crate::pages::PageAction;
use crate::Effect;

pub const COLUMNS: usize = 5;
/// The first tile's top left, a tile's side, and from one tile to the next: the list's -24 margin, the items' 16.5
/// and each tile's 7.5 put the tiles flush with the content's edges (x 520 to 1750).
const LEFT: f32 = 520.0;
const TOP: f32 = 190.0;
pub const SIDE: f32 = 234.0;
const PITCH: f32 = 249.0;
/// The list's viewport: from 24 over the first row (its -24 margin) to the screen's foot. What scrolls brings the
/// focused tile wholly into it, at once, as WPF brings an item into view.
const VIEW_TOP: f32 = TOP - 24.0;
/// With nothing added, the pane's one line.
pub const EMPTY: &str = "Apps you add in Settings show up here.";

/// Where tile `index` is before any scrolling.
pub fn rect(index: usize) -> Rect {
    let (row, column) = (index / COLUMNS, index % COLUMNS);
    Rect::new(LEFT + PITCH * column as f32, TOP + PITCH * row as f32, SIDE, SIDE)
}

#[derive(Default)]
pub struct Apps {
    focus: usize,
    /// The App focus is on, by which it is found again when the library changes.
    id: Option<i64>,
    scroll: f32,
}

impl Apps {
    /// Focus on its App wherever that is now, else on the tile in its place. False with no Apps.
    fn keep(&mut self, apps: &[App]) -> bool {
        if apps.is_empty() {
            (self.focus, self.id, self.scroll) = (0, None, 0.0);
            return false;
        }
        if let Some(at) = self.id.and_then(|id| apps.iter().position(|app| app.id == id)) {
            self.focus = at;
        }
        self.focus = self.focus.min(apps.len() - 1);
        self.id = Some(apps[self.focus].id);
        true
    }

    /// The least scroll that has the focused tile wholly between the viewport's top and the screen's foot, with the
    /// 24 over it that the first row has (its margin and the list's padding, as WPF brings an item into view): so a
    /// first-row tile always scrolls the list back to its top.
    fn follow(&mut self, foot: f32) {
        let tile = rect(self.focus);
        let (top, bottom) = (tile.y - (TOP - VIEW_TOP) - self.scroll, tile.bottom() - self.scroll);
        if top < VIEW_TOP {
            self.scroll -= VIEW_TOP - top;
        } else if bottom > foot {
            self.scroll += bottom - foot;
        }
        self.scroll = self.scroll.max(0.0);
    }
}

impl Pane for Apps {
    fn nav_count(&self, around: crate::pages::Around) -> Option<usize> {
        Some(around.library.apps.len())
    }

    fn enter(&mut self, around: crate::pages::Around) -> bool {
        self.keep(&around.library.apps)
    }

    fn holds_focus(&mut self, around: crate::pages::Around) -> bool {
        self.keep(&around.library.apps)
    }

    /// `Tile(1)`: its place. The state says which App that is.
    fn focus(&self) -> Option<String> {
        Some(format!("Tile({})", self.focus))
    }

    /// The App focus is on, and how many there are: `"Notepad", 2 apps`. A test on the VM looks for the name.
    fn state(&self, around: crate::pages::Around) -> String {
        let apps = &around.library.apps;
        // With none, nothing: the pane is its note.
        match apps.get(self.focus) {
            Some(app) => format!("{:?}, {} apps", app.name, apps.len()),
            None => String::new(),
        }
    }

    fn press(&mut self, press: Press, around: crate::pages::Around) -> Outcome {
        let apps = &around.library.apps;
        if !self.keep(apps) {
            return Outcome::Nav;
        }
        let (count, at) = (apps.len(), self.focus);
        let (row, column) = (at / COLUMNS, at % COLUMNS);
        match press.button {
            Button::Left if column == 0 => return Outcome::Nav,
            Button::Left => self.focus -= 1,
            Button::Right if column + 1 < COLUMNS && at + 1 < count => self.focus += 1,
            Button::Up if row > 0 => self.focus -= COLUMNS,
            // Onto a shorter last row: its last tile.
            Button::Down if row < (count - 1) / COLUMNS => self.focus = (at + COLUMNS).min(count - 1),
            Button::A if !press.repeat => {
                return Outcome::Action(PageAction::Effect(Effect::StartApp(apps[at].id)));
            }
            _ => {}
        }
        self.id = Some(apps[self.focus].id);
        Outcome::Stay
    }

    fn leave(&mut self) {
        (self.focus, self.id, self.scroll) = (0, None, 0.0);
    }

    fn draw(&mut self, ui: &mut Ui, around: crate::pages::Around, look: Look) {
        let apps: &[App] = &around.library.apps;
        if !self.keep(apps) {
            ui.with_alpha(look.fade, |ui| ui.label(548.0, TOP, EMPTY, &TEXT_DIM));
            return;
        }
        let screen = ui.screen();
        self.follow(screen.bottom());
        let left = nav::WIDTH + 2.0;
        let clip = Rect::new(left, VIEW_TOP, screen.right() - left, screen.bottom() - VIEW_TOP);
        let scroll = self.scroll;
        let focused = look.focused.then_some(self.focus);
        ui.with_alpha(look.fade, |ui| {
            ui.with_clip(clip, |ui| {
                let shown = |index: &usize| {
                    let tile = rect(*index);
                    tile.bottom() - scroll > VIEW_TOP - 30.0 && tile.y - scroll < screen.bottom()
                };
                for index in (0..apps.len()).filter(shown).filter(|&index| Some(index) != focused) {
                    let tile = rect(index);
                    app_tile::draw(
                        ui,
                        Rect { y: tile.y - scroll, ..tile },
                        ("app", apps[index].id),
                        &apps[index],
                        false,
                    );
                }
                // The focused tile last: its ring and glow lie over its neighbours.
                if let Some(index) = focused {
                    let tile = rect(index);
                    app_tile::draw(
                        ui,
                        Rect { y: tile.y - scroll, ..tile },
                        ("app", apps[index].id),
                        &apps[index],
                        true,
                    );
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/spec/my-games.md, section 8: tile 234, pitch 249, the first at (520, 190), the fifth ending at 1750.
    #[test]
    fn the_tiles_stand_where_the_spec_says() {
        assert_eq!(rect(0), Rect::new(520.0, 190.0, 234.0, 234.0));
        assert_eq!(rect(4).right(), 1750.0);
        assert_eq!(rect(6), Rect::new(769.0, 439.0, 234.0, 234.0));
    }

    #[test]
    fn the_viewport_follows_focus_by_the_least_scroll() {
        let mut apps = Apps { focus: 15, ..Apps::default() };
        apps.follow(1080.0);
        // The fourth row's foot (937 + 234) at the screen's foot.
        assert_eq!(apps.scroll, 91.0);
        apps.focus = 2;
        apps.follow(1080.0);
        assert_eq!(apps.scroll, 0.0);
    }
}
