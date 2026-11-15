//! A page that opens text entry, **standing in** for the pages that will: Full library's name box (#53), Settings'
//! Groups (#57) and Discord's Secret (M6). It is how text entry is reached before they exist, by the tests and by
//! `dashboard --page keyboard`, and it shows each of them how: it goes when the last of them is built.
//!
//! Three boxes, A on one opening text entry for it:
//!
//! - **Name**, as Full library's: "Filter by name" over the name as it is; the games beside it narrow on every
//!   key; B puts the name back as it was.
//! - **Group**, as a Group's name: "Group name", 40 characters at most; nothing changes until it is kept, and a
//!   blank name is not kept.
//! - **Secret**, as Discord's: "Client Secret" from empty, shown as dots; kept empty, the saved one stays.
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::text_entry::{Entry, Event};
use crate::pages::{Around, Open, Page, PageAction};
use crate::theme::{focus_ring, BODY, HEADING, TEXT_DIM};

const NAME: Entry = Entry::new("Filter by name");
const GROUP: Entry = Entry::new("Group name").limit(40);
const SECRET: Entry = Entry::new("Client Secret").masked();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Group,
    Secret,
}

const FIELDS: [Field; 3] = [Field::Name, Field::Group, Field::Secret];

const FILL: Color = Color::rgb(0x1F1F1F);
const BOX: Color = Color::rgb(0x2E3136);
const TILE: Color = Color::rgb(0x3A3D44);

pub struct StandIn {
    focus: Field,
    name: String,
    group: String,
    secret: String,
    /// What text entry said last: `typed`, `kept`, `dropped`, or `nothing` yet.
    heard: &'static str,
}

impl StandIn {
    pub fn open() -> Self {
        Self {
            focus: Field::Name,
            name: String::new(),
            group: "Couch co-op".into(),
            secret: String::new(),
            heard: "nothing",
        }
    }
}

impl Page for StandIn {
    fn name(&self) -> &'static str {
        "Keyboard"
    }

    fn focus(&self) -> String {
        format!("{:?}", self.focus)
    }

    /// What the boxes hold and what text entry said last: `name "Ha" group "Couch co-op" secret 0, kept`. The
    /// secret is only ever its length.
    fn state(&self) -> String {
        let secret = self.secret.chars().count();
        format!("name {:?} group {:?} secret {secret}, {}", self.name, self.group, self.heard)
    }

    fn press(&mut self, press: Press, _around: Around) -> Option<PageAction> {
        let at = FIELDS.iter().position(|&field| field == self.focus).unwrap_or(0);
        match press.button {
            Button::Up => self.focus = FIELDS[at.saturating_sub(1)],
            Button::Down => self.focus = FIELDS[(at + 1).min(FIELDS.len() - 1)],
            Button::A if !press.repeat => {
                let entry = match self.focus {
                    Field::Name => NAME,
                    Field::Group => GROUP,
                    Field::Secret => SECRET,
                };
                return Some(PageAction::Open(Open::TextEntry(entry)));
            }
            Button::B if !press.repeat => return Some(PageAction::Close),
            _ => {}
        }
        None
    }

    /// What text entry starts from: the name and the Group's name as they are; a secret is never shown, so empty.
    fn entry_text(&self) -> String {
        match self.focus {
            Field::Name => self.name.clone(),
            Field::Group => self.group.clone(),
            Field::Secret => String::new(),
        }
    }

    fn entry(&mut self, event: Event, _around: Around) {
        self.heard = match &event {
            Event::Typed(_) => "typed",
            Event::Kept(_) => "kept",
            Event::Dropped(_) => "dropped",
        };
        match (self.focus, event) {
            // The name follows the typing, and B puts back what it was.
            (Field::Name, Event::Typed(text) | Event::Kept(text) | Event::Dropped(text)) => self.name = text,
            // A Group's name and a secret change only when kept, and never to nothing.
            (Field::Group, Event::Kept(text)) if !text.trim().is_empty() => self.group = text,
            (Field::Secret, Event::Kept(text)) if !text.trim().is_empty() => self.secret = text,
            _ => {}
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        ui.fill(ui.screen(), FILL, 0.0);
        ui.label(75.0, 78.0, "Console keyboard", &HEADING.size(30.0));
        let secret = "\u{2022}".repeat(self.secret.chars().count());
        let boxes = [("Name", self.name.as_str()), ("Group", self.group.as_str()), ("Secret", secret.as_str())];
        for (row, (field, (label, holds))) in FIELDS.iter().zip(boxes).enumerate() {
            let at = Rect::new(75.0, 180.0 + 96.0 * row as f32, 600.0, 64.0);
            ui.fill(at, BOX, 4.0);
            ui.label_middle(at.x + 20.0, at.center().1, label, &BODY.color(TEXT_DIM));
            let value = Rect::new(at.x + 140.0, at.y, at.w - 160.0, at.h);
            ui.with_clip(value, |ui| ui.label_middle(value.x, value.center().1, holds, &BODY));
            if *field == self.focus {
                focus_ring(ui, at, 1.0);
            }
        }
        // The games whose names have the name in them, as Full library's grid narrows.
        let wanted = self.name.to_lowercase();
        let found = around.library.games.iter().filter(|game| game.name.to_lowercase().contains(&wanted));
        for (place, game) in found.take(20).enumerate() {
            let (column, row) = ((place % 5) as f32, (place / 5) as f32);
            let tile = Rect::new(760.0 + 216.0 * column, 180.0 + 136.0 * row, 200.0, 120.0);
            ui.fill(tile, TILE, 4.0);
            ui.paragraph(tile.grow(-12.0), &game.name, &BODY, HAlign::Left, VAlign::Bottom, 2);
        }
    }
}
