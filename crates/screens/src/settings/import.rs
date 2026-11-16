//! Theme settings > Import from Playnite (docs/import.md): a Playnite library on this PC, read once into the
//! library: play time and last played, favourites, hidden games, and the Bundle's Groups where they can come over.
//! The Bundle has no such page; it is laid out as Art's is (section 16.8): a label, its description, then what was
//! found and the one button.
//!
//! The page asks the shell to look as it opens (`Effect::Playnite(Ask::Look)`) and shows what an import would bring
//! before anything is done; Import asks for it, and the shell answers each time with how it stands
//! (`Dashboard::set_playnite`). Nothing is ever written to Playnite's files, and an import run twice brings nothing
//! the second time.
use model::playnite::{Ask, Import as Stand, Preview};
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, FIELD_HEIGHT, LEFT, ROW_WIDTH, TOP};
use super::{Pane, Step};
use crate::pages::{Around, PageAction};
use crate::Effect;

/// The box's name, and the page's header.
pub const TITLE: &str = "Import from Playnite";
const LABEL: &str = "Playnite library";
const ABOUT: &str = "Brings over what Playnite knows of your games, once: play time, when each was last played, \
                     favourites, hidden games, and the Groups you picked for Fullscreen. A game is matched by its \
                     store and the store's id, never by name. Playnite's files are only read; close Playnite first.";
const BRINGS: &str = "What comes over";
const LEFT_OVER: &str = "What stays";
const IMPORT: &str = "Import";

pub struct Import {
    stand: Stand,
    asks: Vec<Effect>,
}

impl Default for Import {
    /// Opens looking: the shell is asked at once.
    fn default() -> Self {
        Self { stand: Stand::Looking, asks: vec![Effect::Playnite(Ask::Look)] }
    }
}

/// What the page says of where things stand, under the description.
fn found(stand: &Stand) -> String {
    match stand {
        Stand::Looking => "Looking for Playnite's library\u{2026}".into(),
        Stand::NotFound => "No Playnite library was found on this PC.".into(),
        Stand::Unreadable(why) => format!("Playnite's library could not be read. {why}"),
        Stand::Ready(preview) => format!("Found at {}: {}.", preview.from, games(preview)),
        Stand::Done(preview) => format!("Imported from {}.", preview.from),
    }
}

fn games(preview: &Preview) -> String {
    let of = if preview.games_read == 1 { "game" } else { "games" };
    format!("{} {of}, {} of them in your library", preview.games_read, preview.matched)
}

/// The lines under "What comes over", or under what came over once it is done.
fn brings(preview: &Preview) -> String {
    if preview.is_empty() {
        return "Nothing new: everything Playnite has is here already.".into();
    }
    let mut text = preview.describe();
    if let Some(first) = text.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    text + "."
}

/// What an import leaves, and why: games of no store here, and Groups that cannot come over.
fn stays(preview: &Preview) -> Option<String> {
    let mut lines = Vec::new();
    let games = |n: u32| if n == 1 { "1 game".to_string() } else { format!("{n} games") };
    if preview.not_here > 0 {
        lines.push(format!("{} that this library does not have.", games(preview.not_here)));
    }
    if preview.other_stores > 0 {
        lines.push(format!("{} from a store or emulator the Dashboard does not read.", games(preview.other_stores)));
    }
    for (name, why) in &preview.skipped {
        lines.push(format!("The Group \u{201C}{name}\u{201D}: {why}."));
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

impl Import {
    /// What there is to bring now: Import does something.
    fn can_import(&self) -> bool {
        matches!(&self.stand, Stand::Ready(preview) if !preview.is_empty())
    }
}

impl Pane for Import {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// One button: `Import from Playnite: Import`.
    fn focus(&self) -> String {
        format!("{TITLE}: {IMPORT}")
    }

    /// `looking`, `not found`, `unreadable`, `ready`, `nothing new` or `done`.
    fn state(&self) -> String {
        self.stand.state_line().into()
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        if press.repeat {
            return Step::Stay;
        }
        match press.button {
            Button::B | Button::Left => Step::Back,
            Button::A if self.can_import() => Step::Do(PageAction::Effect(Effect::Playnite(Ask::Import))),
            _ => Step::Stay,
        }
    }

    fn set_playnite(&mut self, import: &Stand) {
        self.stand = import.clone();
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, ABOUT) + 30.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, &found(&self.stand)) + 40.0;
        let preview = match &self.stand {
            Stand::Ready(preview) | Stand::Done(preview) => Some(preview),
            _ => None,
        };
        if let Some(preview) = preview {
            let heading = if matches!(self.stand, Stand::Done(_)) { "What came over" } else { BRINGS };
            y += controls::label(ui, LEFT, y, heading) + 4.0;
            let text = match &self.stand {
                Stand::Done(done) if done.is_empty() => "Nothing new: everything Playnite has was here already.".into(),
                _ => brings(preview),
            };
            y += controls::description(ui, LEFT, y, ROW_WIDTH, &text) + 30.0;
            if let Some(stays) = stays(preview) {
                y += controls::label(ui, LEFT, y, LEFT_OVER) + 4.0;
                for line in stays.lines() {
                    y += controls::description(ui, LEFT, y, ROW_WIDTH, line);
                }
                y += 30.0;
            }
        }
        let button = Rect::new(LEFT, y, controls::pane_button_width(ui, IMPORT), FIELD_HEIGHT);
        controls::pane_button(ui, button, IMPORT, self.can_import(), focused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_says_what_comes_over_and_what_stays() {
        let preview = Preview {
            from: "C:\\Users\\me\\AppData\\Roaming\\Playnite\\library".into(),
            games_read: 8,
            matched: 5,
            playtime: 1,
            favourites: 1,
            groups: vec!["Couch picks".into()],
            not_here: 2,
            other_stores: 1,
            skipped: vec![("Released in 2019".into(), "it filters by release year".into())],
            ..Preview::default()
        };
        assert_eq!(
            found(&Stand::Ready(preview.clone())),
            "Found at C:\\Users\\me\\AppData\\Roaming\\Playnite\\library: 8 games, 5 of them in your library."
        );
        assert_eq!(brings(&preview), "Play time for 1 game, 1 favourite, Group: Couch picks.");
        assert_eq!(
            stays(&preview).unwrap(),
            "2 games that this library does not have.\n1 game from a store or emulator the Dashboard does not \
             read.\nThe Group \u{201C}Released in 2019\u{201D}: it filters by release year."
        );
        assert_eq!(stays(&Preview::default()), None);
    }
}
