//! Theme settings > Discord (docs/spec/settings.md, 6.9 and 9.5; docs/discord.md): connecting the user's own
//! Discord application. The intro and the four steps, the developer portal's QR code with its address under it, the
//! Client ID and Client Secret rows with Type and Paste, Connect and Disconnect, and the status line.
//!
//! The page keeps nothing of the connection: what it shows is `around.discord`, and what a button does goes to the
//! shell as `Effect::Discord`. The Secret is typed from empty and shown as dots; the page shows only whether one is
//! saved ("Saved"), never what it is.
use model::discord::{Ask, Field, INTRO, PORTAL_URL, STEPS};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, FIELD_HEIGHT, LEFT, RIGHT, TOP};
use super::{Pane, Step};
use crate::keyboard::{self, Event};
use crate::pages::{Around, Open, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

const CLIENT_ID: &str = "Client ID";
const CLIENT_SECRET: &str = "Client Secret";
const NOT_SET: &str = "Not set";
const SAVED: &str = "Saved";
const CONNECT: &str = "Connect";
const DISCONNECT: &str = "Disconnect";
/// The Client ID on the Console keyboard, from what is saved; Discord's are 17 to 20 digits.
const ID_ENTRY: keyboard::Entry = keyboard::Entry::new(CLIENT_ID).limit(32);
/// The Secret from empty, shown as dots; Discord's are 32 characters.
const SECRET_ENTRY: keyboard::Entry = keyboard::Entry::new(CLIENT_SECRET).masked().limit(64);

/// The intro, then the steps and QR 30 under it (the code at y 255, as in the Bundle's picture); a step's number in a
/// 40-wide column, 16 between steps.
const STEPS_GAP: f32 = 30.0;
const NUMBER_WIDTH: f32 = 40.0;
const STEP_GAP: f32 = 16.0;
/// The QR code's column at the right edge (x 1530..1750), the code 200 square in it, the address under it.
const QR_COLUMN: f32 = 220.0;
const QR_SIZE: f32 = 200.0;
const QR_X: f32 = 1540.0;
/// The address's lines under the code, centred, as the Bundle's picture breaks it.
const ADDRESS_GAP: f32 = 10.0;
/// The rows: a 420-wide name column, then Type and Paste.
const NAME_WIDTH: f32 = 420.0;
const TYPE_X: f32 = 940.0;
const PASTE_X: f32 = 1140.0;
const BUTTON_WIDTH: f32 = 180.0;
/// Measured off the Bundle's picture: the Client ID row 34 under the steps' block (its buttons at y 645), the
/// Secret's 19 under it (734), Connect 35 under that (833), the status 24 under.
const ID_GAP: f32 = 34.0;
const SECRET_GAP: f32 = 19.0;
const CONNECT_GAP: f32 = 35.0;
const STATUS_GAP: f32 = 24.0;
/// Connect and Disconnect, 20 apart.
const BUTTONS_GAP: f32 = 20.0;
/// The QR code's stand-in: the Bundle's picture is not the port's to copy (docs/discord.md).
const QR_FILL: Color = Color::rgb(0xFFFFFF);
const QR_MARK: Color = Color::argb(0x80000000);

/// Three rows of two: [Type, Paste], [Type, Paste], [Connect, Disconnect] (section 9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Focus {
    row: usize,
    column: usize,
}

impl Focus {
    fn name(self) -> &'static str {
        match (self.row, self.column) {
            (0, 0) => "Client ID Type",
            (0, _) => "Client ID Paste",
            (1, 0) => "Client Secret Type",
            (1, _) => "Client Secret Paste",
            (_, 0) => CONNECT,
            _ => DISCONNECT,
        }
    }
}

#[derive(Default)]
pub struct Discord {
    focus: Focus,
    /// The field the Console keyboard is up for.
    typing: Option<Field>,
    /// The Client ID saved as Type was pressed: text entry starts from it.
    saved_id: String,
    /// What text entry kept, to be asked of the shell.
    asks: Vec<Effect>,
}

impl Discord {
    fn ask(ask: Ask) -> Step {
        Step::Do(PageAction::Effect(Effect::Discord(ask)))
    }
}

/// The address broken after each slash, as the Bundle's picture shows it under the code.
fn address_lines() -> Vec<&'static str> {
    let mut lines = Vec::new();
    let mut rest = PORTAL_URL;
    while let Some(at) = rest.find('/') {
        // "https://" stays whole.
        let end = if rest[at..].starts_with("//") { at + 2 } else { at + 1 };
        lines.push(&rest[..end]);
        rest = &rest[end..];
    }
    if !rest.is_empty() {
        lines.push(rest);
    }
    lines
}

/// The developer portal's QR code. Its stand-in for now: a white square with the code's three corner marks' places
/// outlined, so that nobody takes it for a code to scan. Where the real one is drawn (docs/discord.md).
fn portal_qr(ui: &mut Ui, rect: Rect) {
    ui.fill(rect, QR_FILL, 0.0);
    let mark = 56.0;
    let inset = 14.0;
    for (x, y) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
        let at = Rect::new(
            rect.x + inset + x * (rect.w - 2.0 * inset - mark),
            rect.y + inset + y * (rect.h - 2.0 * inset - mark),
            mark,
            mark,
        );
        ui.fill(at, QR_MARK, 0.0);
        ui.fill(Rect::new(at.x + 8.0, at.y + 8.0, at.w - 16.0, at.h - 16.0), QR_FILL, 0.0);
    }
}

impl Pane for Discord {
    fn header(&self) -> String {
        "Discord".into()
    }

    /// `Discord: Client ID Type`, `... Client ID Paste`, `... Client Secret Type`, `... Client Secret Paste`,
    /// `Discord: Connect`, `Discord: Disconnect`.
    fn focus(&self) -> String {
        format!("Discord: {}", self.focus.name())
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        let Focus { row, column } = self.focus;
        match press.button {
            Button::Up if row > 0 => self.focus.row -= 1,
            Button::Down if row < 2 => self.focus.row += 1,
            Button::Right if column == 0 => self.focus.column = 1,
            Button::Left if column == 1 => self.focus.column = 0,
            // Left from the first column is B (section 9.5).
            Button::Left | Button::B => return Step::Back,
            Button::A => {
                let discord = around.discord;
                return match (row, column) {
                    (0, 0) => {
                        self.typing = Some(Field::ClientId);
                        self.saved_id = discord.client_id.clone().unwrap_or_default();
                        Step::Do(PageAction::Open(Open::TextEntry(ID_ENTRY)))
                    }
                    (1, 0) => {
                        self.typing = Some(Field::ClientSecret);
                        Step::Do(PageAction::Open(Open::TextEntry(SECRET_ENTRY)))
                    }
                    (0, _) => Self::ask(Ask::Paste(Field::ClientId)),
                    (1, _) => Self::ask(Ask::Paste(Field::ClientSecret)),
                    (_, 0) if discord.can_connect() => Self::ask(Ask::Connect),
                    (_, 1) if discord.can_disconnect() => Self::ask(Ask::Disconnect),
                    _ => Step::Stay,
                };
            }
            _ => {}
        }
        Step::Stay
    }

    /// The Client ID is typed from what is saved; the Secret from empty, never shown.
    fn entry_text(&self) -> String {
        match self.typing {
            Some(Field::ClientId) => self.saved_id.clone(),
            _ => String::new(),
        }
    }

    fn entry(&mut self, event: Event, _around: Around) {
        match event {
            Event::Kept(text) => match self.typing.take() {
                Some(Field::ClientId) => self.asks.push(Effect::Discord(Ask::ClientId(text))),
                // An empty Secret kept leaves the saved one as it is.
                Some(Field::ClientSecret) if !text.trim().is_empty() => {
                    self.asks.push(Effect::Discord(Ask::ClientSecret(text)));
                }
                _ => {}
            },
            Event::Dropped(_) => self.typing = None,
            Event::Typed(_) => {}
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let discord = around.discord;
        let dim = BODY.color(TEXT_DIM);
        let text = BODY.color(Color::WHITE);
        // The intro, and 36 under it the steps on the left and the code on the right.
        let mut y = TOP;
        y += controls::description(ui, LEFT, y, RIGHT - LEFT, INTRO) + STEPS_GAP;
        let block = y;
        let steps_width = QR_X - 10.0 - LEFT - NUMBER_WIDTH;
        for (n, step) in STEPS.iter().enumerate() {
            ui.label(LEFT, y, &format!("{}.", n + 1), &text);
            let rect = Rect::new(LEFT + NUMBER_WIDTH, y, steps_width, 2000.0);
            y += ui.paragraph(rect, step, &text, HAlign::Left, VAlign::Top, 0).height + STEP_GAP;
        }
        let steps_bottom = y - STEP_GAP;
        let column_x = RIGHT - QR_COLUMN;
        portal_qr(ui, Rect::new(QR_X, block, QR_SIZE, QR_SIZE));
        let mut address_y = block + QR_SIZE + ADDRESS_GAP;
        for line in address_lines() {
            let rect = Rect::new(column_x, address_y, QR_COLUMN, 2000.0);
            address_y += ui.paragraph(rect, line, &dim, HAlign::Center, VAlign::Top, 0).height;
        }
        y = steps_bottom.max(address_y) + ID_GAP;
        // The two rows: the name and its value in the left column, Type and Paste beside them.
        let mut rows = [Rect::default(); 2];
        let values =
            [discord.client_id.as_deref().unwrap_or(NOT_SET), if discord.has_secret { SAVED } else { NOT_SET }];
        for (i, (name, value)) in [CLIENT_ID, CLIENT_SECRET].into_iter().zip(values).enumerate() {
            if i == 1 {
                y += SECRET_GAP;
            }
            let name_height = ui.label(LEFT, y, name, &text).height;
            let value_rect = Rect::new(LEFT, y + name_height, NAME_WIDTH, 2000.0);
            let value_height = ui.paragraph(value_rect, value, &text, HAlign::Left, VAlign::Top, 1).height;
            let height = name_height + value_height;
            rows[i] = Rect::new(LEFT, y + (height - FIELD_HEIGHT) / 2.0, 0.0, FIELD_HEIGHT);
            y += height;
        }
        y += CONNECT_GAP;
        let connect = Rect::new(LEFT, y, controls::pane_button_width(ui, CONNECT), FIELD_HEIGHT);
        let disconnect_x = connect.x + connect.w + BUTTONS_GAP;
        let disconnect = Rect::new(disconnect_x, y, controls::pane_button_width(ui, DISCONNECT), FIELD_HEIGHT);
        y += FIELD_HEIGHT + STATUS_GAP;
        let status = Rect::new(LEFT, y, RIGHT - LEFT, 2000.0);
        ui.paragraph(status, &discord.status, &text, HAlign::Left, VAlign::Top, 0);
        // The buttons, the focused one last, its ring over its neighbours.
        let at = |row: Rect, x: f32| Rect::new(x, row.y, BUTTON_WIDTH, FIELD_HEIGHT);
        let buttons = [
            (Focus { row: 0, column: 0 }, at(rows[0], TYPE_X), "Type", true),
            (Focus { row: 0, column: 1 }, at(rows[0], PASTE_X), "Paste", true),
            (Focus { row: 1, column: 0 }, at(rows[1], TYPE_X), "Type", true),
            (Focus { row: 1, column: 1 }, at(rows[1], PASTE_X), "Paste", true),
            (Focus { row: 2, column: 0 }, connect, CONNECT, discord.can_connect()),
            (Focus { row: 2, column: 1 }, disconnect, DISCONNECT, discord.can_disconnect()),
        ];
        let has = |focus: Focus| focused && self.focus == focus;
        for (focus, rect, label, enabled) in buttons.iter().filter(|b| !has(b.0)) {
            controls::pane_button(ui, *rect, label, *enabled, has(*focus));
        }
        for (_, rect, label, enabled) in buttons.iter().filter(|b| has(b.0)) {
            controls::pane_button(ui, *rect, label, *enabled, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_breaks_after_each_slash_as_the_bundle_shows_it() {
        assert_eq!(address_lines(), ["https://", "discord.com/", "developers/", "applications"]);
    }
}
