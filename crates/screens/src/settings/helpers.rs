//! Games & apps > Helpers (docs/spec/helpers.md, 2): the helper programs some stores install through, legendary for
//! Epic Games and gogdl for GOG. Each is a group, as About's are: its name and store, the version the Dashboard
//! fetches with its licence and source, how it stands on this PC, and one button, Add or Remove. Add asks first,
//! the licence and the source in the question, and only Yes fetches; Remove asks too.
//!
//! The page shows the helpers as the last snapshot has them (`Library::helpers`) and asks with `Effect::Helper`.
use std::sync::atomic::{AtomicU32, Ordering};

use model::helper::{Account, Helper, HelperAsk, HelperState};
use ui::{Button, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, FIELD_HEIGHT, LEFT, ROW_WIDTH, TOP};
use super::{Pane, Step};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

pub const TITLE: &str = "Helpers";
pub const ADD: &str = "Add";
pub const REMOVE: &str = "Remove";
const NONE: &str = "There are no helpers to add.";

/// The answers to the questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Yes,
    No,
}

/// What the question in the middle asks, of the helper focus is on.
struct Question {
    key: String,
    remove: bool,
    panel: MenuPanel<Reply>,
}

static FLIGHTS: AtomicU32 = AtomicU32::new(0);

fn flight() -> u32 {
    FLIGHTS.fetch_add(1, Ordering::Relaxed)
}

/// The source without its scheme, as it is said: `github.com/legendary-gl/legendary`.
fn source(helper: &Helper) -> &str {
    helper.source.trim_start_matches("https://").trim_start_matches("http://")
}

/// The line under a helper's name: its version, its licence and its source.
pub fn line(helper: &Helper) -> String {
    format!("Version {}. {} licence. Source: {}", helper.version, helper.licence, source(helper))
}

/// How it stands, in words.
pub fn status(helper: &Helper) -> String {
    match &helper.state {
        HelperState::NotAdded => format!("Not added. {} games can't be installed from here without it.", helper.store),
        HelperState::Fetching => "Downloading and checking it...".into(),
        HelperState::Added { version } => {
            let added = format!("Added: version {version}, in the Dashboard's data folder.");
            match &helper.account {
                Account::Unknown => added,
                Account::SignedIn(name) if name.is_empty() => format!("{added} Signed in to {}.", helper.store),
                Account::SignedIn(name) => format!("{added} Signed in to {} as {name}.", helper.store),
                Account::SignedOut(how) => format!("{added} Not signed in to {}: {how}", helper.store),
            }
        }
        HelperState::Failed(why) => why.clone(),
    }
}

/// The question Add asks: the licence and the source in front of the user before anything is fetched.
fn add_question(helper: &Helper) -> MenuPanel<Reply> {
    let message = format!(
        "{} is free software under the {} licence, from {}. The Dashboard downloads version {} from there, checks \
         it, and keeps it in its data folder. It isn't part of the Dashboard, and you can remove it here.",
        helper.key,
        helper.licence,
        source(helper),
        helper.version
    );
    let rows = vec![Row::new(icon::INSTALL, "Yes", Reply::Yes), Row::new(icon::CANCEL, "No", Reply::No)];
    MenuPanel::prompt(&format!("Add {}?", helper.key), Some(&message), rows)
}

fn remove_question(helper: &Helper) -> MenuPanel<Reply> {
    let message = format!("{} games can't be installed from here until it's added again.", helper.store);
    let rows = vec![Row::new(icon::DELETE, REMOVE, Reply::Yes), Row::new(icon::CANCEL, "Cancel", Reply::No)];
    MenuPanel::prompt(&format!("Remove {}?", helper.key), Some(&message), rows)
}

pub struct Helpers {
    helpers: Vec<Helper>,
    at: usize,
    question: Option<Question>,
    flying: u32,
    leaving: Option<(MenuPanel<Reply>, u32)>,
}

impl Helpers {
    pub fn open(around: Around) -> Self {
        Self { helpers: around.library.helpers.clone(), at: 0, question: None, flying: 0, leaving: None }
    }

    fn close_question(&mut self) {
        if let Some(question) = self.question.take() {
            self.leaving = Some((question.panel, self.flying));
        }
    }

    /// The button a helper has as it stands: none while it is being fetched.
    fn button(helper: &Helper) -> Option<&'static str> {
        match helper.state {
            HelperState::NotAdded | HelperState::Failed(_) => Some(ADD),
            HelperState::Added { .. } => Some(REMOVE),
            HelperState::Fetching => None,
        }
    }
}

impl Pane for Helpers {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `Helpers: legendary; Add`, or `Helpers: Nothing`.
    fn focus(&self) -> String {
        match self.helpers.get(self.at) {
            Some(helper) => format!("{TITLE}: {}; {}", helper.key, Self::button(helper).unwrap_or("busy")),
            None => format!("{TITLE}: Nothing"),
        }
    }

    /// How the focused helper stands, and the question while it is up.
    fn state(&self) -> String {
        let Some(helper) = self.helpers.get(self.at) else { return String::new() };
        let state = match &helper.state {
            HelperState::NotAdded => "not added".to_string(),
            HelperState::Fetching => "fetching".into(),
            HelperState::Added { version } => format!("added {version}"),
            HelperState::Failed(_) => "failed".into(),
        };
        match &self.question {
            Some(question) => format!("{state}; asking {}", question.panel.title()),
            None => state,
        }
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        if let Some(question) = self.question.as_mut() {
            let (key, remove) = (question.key.clone(), question.remove);
            match question.panel.press(press) {
                Answer::Chose(Reply::Yes) => {
                    self.close_question();
                    let ask = if remove { HelperAsk::Remove(key) } else { HelperAsk::Add(key) };
                    return Step::Do(PageAction::Effect(Effect::Helper(ask)));
                }
                Answer::Chose(Reply::No) | Answer::Declined => self.close_question(),
                Answer::Moved | Answer::Nothing => {}
            }
            return Step::Stay;
        }
        match press.button {
            Button::B | Button::Left if !press.repeat => Step::Back,
            Button::Up => {
                self.at = self.at.saturating_sub(1);
                Step::Stay
            }
            Button::Down => {
                self.at = (self.at + 1).min(self.helpers.len().saturating_sub(1));
                Step::Stay
            }
            Button::A if !press.repeat => {
                let Some(helper) = self.helpers.get(self.at) else { return Step::Stay };
                let panel = match Self::button(helper) {
                    Some(ADD) => add_question(helper),
                    Some(_) => remove_question(helper),
                    None => return Step::Stay,
                };
                let remove = matches!(helper.state, HelperState::Added { .. });
                self.question = Some(Question { key: helper.key.clone(), remove, panel });
                self.flying = flight();
                self.leaving = None;
                Step::Stay
            }
            _ => Step::Stay,
        }
    }

    fn dialog(&self) -> Option<Dialog> {
        self.question.as_ref().map(|_| Dialog::Prompt)
    }

    fn dismiss_dialog(&mut self) {
        self.question = None;
        self.leaving = None;
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        let key = self.helpers.get(self.at).map(|helper| helper.key.clone());
        self.helpers = around.library.helpers.clone();
        self.at = key
            .and_then(|key| self.helpers.iter().position(|helper| helper.key == key))
            .unwrap_or(self.at)
            .min(self.helpers.len().saturating_sub(1));
        None
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, focused: bool) {
        let mut y = TOP;
        if self.helpers.is_empty() {
            controls::description(ui, LEFT, y, ROW_WIDTH, NONE);
        }
        let style = BODY.color(TEXT_DIM);
        for (i, helper) in self.helpers.iter().enumerate() {
            y += controls::label(ui, LEFT, y, &format!("{} for {}", helper.key, helper.store)) + 4.0;
            y += controls::description(ui, LEFT, y, ROW_WIDTH, &line(helper)) + 4.0;
            let status = status(helper);
            y += ui
                .paragraph(Rect::new(LEFT, y, ROW_WIDTH, 2000.0), &status, &style, HAlign::Left, VAlign::Top, 0)
                .height
                + 18.0;
            match Self::button(helper) {
                Some(label) => {
                    let rect = Rect::new(LEFT, y, controls::pane_button_width(ui, label), FIELD_HEIGHT);
                    controls::pane_button(ui, rect, label, true, focused && i == self.at && self.question.is_none());
                }
                None => crate::spinner::spinner(ui, LEFT + 16.0, y + FIELD_HEIGHT / 2.0, 32.0),
            }
            y += FIELD_HEIGHT + 40.0;
        }
        // The question over the dimmed page; or on its way out, the dim gone at once.
        if let Some(question) = &self.question {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("helpers question", self.flying), false);
            question.panel.draw_flown(ui, fly, shown);
        } else if let Some((panel, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("helpers question", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.leaving = None;
            } else {
                panel.draw_flown(ui, fly, shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helper(state: HelperState) -> Helper {
        Helper {
            key: "legendary".into(),
            store: "Epic Games".into(),
            version: "0.21.1".into(),
            licence: "GPL-3.0".into(),
            source: "https://github.com/legendary-gl/legendary".into(),
            state,
            account: Account::Unknown,
        }
    }

    #[test]
    fn an_added_helper_says_its_sign_in_as_its_own_records_keep_it() {
        let added = || helper(HelperState::Added { version: "0.21.1".into() });
        let base = "Added: version 0.21.1, in the Dashboard's data folder.";
        assert_eq!(status(&added()), base);
        let signed_in = Helper { account: Account::SignedIn("Stand-in Player".into()), ..added() };
        assert_eq!(status(&signed_in), format!("{base} Signed in to Epic Games as Stand-in Player."));
        let signed_out = Helper { account: Account::SignedOut("run legendary auth.".into()), ..added() };
        assert_eq!(status(&signed_out), format!("{base} Not signed in to Epic Games: run legendary auth."));
        // Not added: its sign-in is not said.
        assert_eq!(
            status(&Helper { state: HelperState::NotAdded, ..signed_in }),
            status(&helper(HelperState::NotAdded))
        );
    }

    #[test]
    fn the_question_names_the_licence_the_source_and_the_version_before_anything_is_fetched() {
        let question = add_question(&helper(HelperState::NotAdded));
        assert_eq!(question.title(), "Add legendary?");
        assert_eq!(
            line(&helper(HelperState::NotAdded)),
            "Version 0.21.1. GPL-3.0 licence. Source: github.com/legendary-gl/legendary"
        );
        assert!(question.is_prompt() && question.rows()[0].label == "Yes");
    }

    #[test]
    fn each_state_has_its_button() {
        assert_eq!(Helpers::button(&helper(HelperState::NotAdded)), Some(ADD));
        assert_eq!(Helpers::button(&helper(HelperState::Failed("x".into()))), Some(ADD));
        assert_eq!(Helpers::button(&helper(HelperState::Added { version: "0.21.1".into() })), Some(REMOVE));
        assert_eq!(Helpers::button(&helper(HelperState::Fetching)), None);
    }
}
