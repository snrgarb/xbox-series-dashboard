//! Manage: the page under the nav's line, which holds one box, the Queue's (docs/spec/my-games.md, 9.1). The Queue
//! and Manage Queue are M4's: until then the box is there, takes focus, and A on it does nothing.
use ui::{Button, Color, Press, Rect, Ui};

use super::look::{self, BOX, BOX_FOCUSED, TEXT};
use super::{Look, Outcome, Pane};
use crate::pages::{Around, PageAction};
use crate::theme::glyph;

/// The Queue box, 4 under the content row's top.
const QUEUE: Rect = Rect::new(520.0, 194.0, 437.0, 131.0);
/// Its glyph is centred in its first 100 across, and its label starts after them.
const ICON: f32 = 100.0;
const ICON_SIZE: f32 = 34.0;

/// **M4's hook.** A on the Queue box opens Manage Queue, the page over the whole of My games & apps that lists
/// what is installing (docs/spec/my-games.md, 9.2); B there comes back to the box. Nothing until there is a Queue.
fn open_manage_queue() -> Option<PageAction> {
    None
}

#[derive(Default)]
pub struct Manage;

impl Pane for Manage {
    /// The box is always there to take focus.
    fn enter(&mut self, _around: Around) -> bool {
        true
    }

    fn holds_focus(&mut self, _around: Around) -> bool {
        true
    }

    fn focus(&self) -> Option<String> {
        Some("Queue".into())
    }

    fn press(&mut self, press: Press, _around: Around) -> Outcome {
        match press.button {
            Button::Left => Outcome::Nav,
            Button::A if !press.repeat => open_manage_queue().map_or(Outcome::Stay, Outcome::Action),
            // Right, Up and Down are nothing: the box is alone.
            _ => Outcome::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, look: Look) {
        ui.with_alpha(look.fade, |ui| {
            ui.fill(QUEUE, if look.focused { BOX_FOCUSED } else { BOX }, 0.0);
            let middle = QUEUE.center().1;
            ui.icon(QUEUE.x + ICON / 2.0, middle, glyph::DOWNLOAD, ICON_SIZE, Color::WHITE);
            ui.label_middle(QUEUE.x + ICON, middle, "Queue", &TEXT);
            // The Queue's count, right-aligned 30 inside the box, comes with the Queue.
            if look.focused {
                // The ring's stroke is 4 to 8 outside the box.
                look::ring(ui, QUEUE.grow(8.0));
            }
        });
    }
}
