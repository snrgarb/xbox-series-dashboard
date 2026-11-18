//! Discord's sign-in page (#197; docs/discord-web.md, docs/spec/settings.md 6.10): Discord's own discord.com/login,
//! in the web view the executable keeps (`crates/discord_web`), over the whole screen but a band at the foot where
//! this page says what the pad does. The page itself draws only that band and what shows while the web view is put
//! away; the executable shows the web view while this page has the pad and nothing is over it
//! (`Dashboard::discord_sign_in_shown`).
//!
//! **The QR code first**: the page opens on Discord's QR code, zoomed so it can be scanned from a sofa, with no field
//! focused and no keyboard up (the session lets go of the field Discord focuses by itself, `crates/discord_web`); the
//! band's first line says to scan it with the Discord mobile app. Typing is the second way:
//! the pad drives Discord's page by keys (`WebAsk::Key`): Up and Down step through its fields and buttons as Shift+Tab
//! and Tab do, A on a text field opens the Dashboard's own Console keyboard (a password field's typed as dots), and on
//! anything else presses Enter. What the keyboard keeps goes into the field (`WebAsk::Text`) and nowhere else: never
//! logged, never kept. The QR code is Discord's own and left to Discord (it refreshes itself): never read.
//!
//! The page closes by itself once the session is signed in (`Dashboard::discord_signed_in`); B closes it before.
use model::discord_web::{WebAsk, WebField, WebKey};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use crate::keyboard::{self, Entry};
use crate::pages::{Around, Open, Page, PageAction};
use crate::theme::{BODY, HEADING, TEXT_DIM};
use crate::Effect;

/// The page's name in the focus log.
pub const NAME: &str = "DiscordSignIn";

/// The band at the foot that is the Dashboard's: Discord's page is shown above it.
pub const BAND: f32 = 110.0;
const BACK: Color = Color::rgb(0x0E0E10);
const PAGE: Color = Color::rgb(0x000000);

/// The band's first line: the QR code first, in Discord's words ("Scan this with the Discord mobile app to log in
/// instantly."), then the typed way.
pub const HINT: &str =
    "Scan the QR code with the Discord mobile app to log in instantly. To type instead, move to a field and press A.";
/// The band's second line: what the pad does.
pub const LEGEND: &str = "Up/Down  Move     A  Select or type     X  Tick     View  Esc     B  Back";
const TITLE: &str = "Sign in to Discord";
const WAITING: &str = "Discord's page shows here. If it doesn't, press B and look at Settings > Account > Discord \
messages.";

const TEXT_ENTRY: Entry = Entry::new("Discord");
const PASSWORD_ENTRY: Entry = Entry::new("Discord password").masked();

#[derive(Default)]
pub struct DiscordSignIn {
    /// The last key sent, for the focus log: `DiscordSignIn: Tab`.
    last: Option<&'static str>,
    /// Text entry is up for the field focus was on.
    typing: Option<WebField>,
    asks: Vec<Effect>,
}

impl DiscordSignIn {
    pub fn open() -> Self {
        Self::default()
    }

    fn key(&mut self, key: WebKey, word: &'static str) -> Option<PageAction> {
        self.last = Some(word);
        Some(PageAction::Effect(Effect::DiscordWeb(WebAsk::Key(key))))
    }
}

impl Page for DiscordSignIn {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `Discord's page`, then the last key the pad sent it (`Tab`); the focus line puts the screen's name in front.
    fn focus(&self) -> String {
        self.last.unwrap_or("Discord's page").to_string()
    }

    /// The kind of field focus is on in Discord's page, as the session last said: `field text`, `field password`.
    fn state(&self) -> String {
        match self.typing {
            Some(WebField::Password) => "typing password".into(),
            Some(WebField::Text) => "typing text".into(),
            None => String::new(),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let field = around.options.discord_web.field;
        match press.button {
            Button::Up => self.key(WebKey::ShiftTab, "Shift+Tab"),
            Button::Down => self.key(WebKey::Tab, "Tab"),
            Button::Left => self.key(WebKey::Left, "Left"),
            Button::Right => self.key(WebKey::Right, "Right"),
            _ if press.repeat => None,
            Button::B => Some(PageAction::Close),
            Button::A | Button::Y if field.is_some() => {
                self.typing = field;
                self.last = Some("keyboard");
                let entry = if field == Some(WebField::Password) { PASSWORD_ENTRY } else { TEXT_ENTRY };
                Some(PageAction::Open(Open::TextEntry(entry)))
            }
            Button::A | Button::Menu => self.key(WebKey::Enter, "Enter"),
            Button::X => self.key(WebKey::Space, "Space"),
            Button::View => self.key(WebKey::Escape, "Esc"),
            _ => None,
        }
    }

    /// The field's own text is never read back: text entry starts empty, and what it keeps takes the field's place.
    fn entry_text(&self) -> String {
        String::new()
    }

    fn entry(&mut self, event: keyboard::Event, _around: Around) {
        match event {
            keyboard::Event::Kept(text) => {
                self.typing = None;
                self.last = Some("typed");
                self.asks.push(Effect::DiscordWeb(WebAsk::Text(text)));
            }
            keyboard::Event::Dropped(_) => self.typing = None,
            keyboard::Event::Typed(_) => {}
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        let screen = ui.screen();
        ui.fill(screen, PAGE, 0.0);
        // What shows while Discord's page is away (text entry over it) or not yet up.
        ui.label(140.0, 120.0, TITLE, &HEADING);
        let style = BODY.color(TEXT_DIM);
        ui.paragraph(Rect::new(140.0, 180.0, 1100.0, 400.0), WAITING, &style, HAlign::Left, VAlign::Top, 0);
        let band = Rect::new(screen.x, screen.bottom() - BAND, screen.w, BAND);
        ui.fill(band, BACK, 0.0);
        ui.label_middle(140.0, band.y + 36.0, HINT, &BODY);
        ui.label_middle(140.0, band.y + 76.0, LEGEND, &BODY.color(TEXT_DIM));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_kept_goes_to_the_session_and_nowhere_else() {
        let library = model::Library::default();
        let around = Around {
            library: &library,
            playing: None,
            options: crate::settings::Options::default(),
            now: std::time::Instant::now(),
            now_playing: crate::now_playing::none(),
            discord: crate::no_discord(),
        };
        let mut page = DiscordSignIn::open();
        page.typing = Some(WebField::Password);
        page.entry(keyboard::Event::Kept("hunter2".into()), around);
        assert_eq!(page.asks(), vec![Effect::DiscordWeb(WebAsk::Text("hunter2".into()))]);
        assert!(page.asks().is_empty(), "asked once");
        assert!(!page.focus().contains("hunter2") && !page.state().contains("hunter2"));
    }

    #[test]
    fn the_focus_does_not_say_the_screens_name_again() {
        let mut page = DiscordSignIn::open();
        assert_eq!(page.focus(), "Discord's page");
        page.last = Some("Tab");
        assert_eq!(page.focus(), "Tab", "the focus line reads `DiscordSignIn: Tab`");
    }
}
