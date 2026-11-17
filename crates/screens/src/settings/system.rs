//! Theme settings > System (docs/spec/shell-mode.md, 2): "Start with Windows as the shell", which puts the
//! Dashboard in place of the Windows desktop at sign-in, with what to know before turning it on. The Bundle has no
//! such page (its Boot shell is set up from a terminal): its words are the port's own, laid out as the Bundle's
//! Discord page is (docs/spec/settings.md, 6.9): a label, its description, the control, then the next group 40 under.
//!
//! The page asks before turning it on, never before turning it off. It shows shell mode as the executable last found
//! it (`Options::shell`) and asks for a change with `Effect::ShellMode`; the executable answers with how it is then.
use std::sync::atomic::{AtomicU32, Ordering};

use model::shell_mode::ShellMode;
use ui::{Button, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, FIELD_HEIGHT, FIELD_WIDTH, LEFT, ROW_WIDTH, TOP};
use super::{Pane, Step};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

pub const TITLE: &str = "System";
const LABEL: &str = "Start with Windows";
const ABOUT: &str = "Starts the Dashboard in place of the Windows desktop when you sign in, as a console starts. The \
                     desktop still starts behind it once the Boot video ends. A change takes effect the next time you \
                     sign in.";
pub const SHELL: &str = "Start with Windows as the shell";
const BACK_LABEL: &str = "Getting to the desktop";
const BACK_ABOUT: &str = "Choose Exit to desktop in the Guide's Power, or hold View and Menu for 5 seconds. Press the \
                          Guide button to come back to the Dashboard.";
const STUCK_LABEL: &str = "If the Dashboard won't start";
const STUCK_ABOUT: &str = "The desktop starts by itself within two minutes. Or press Ctrl+Shift+Esc, choose Run new \
                           task and enter explorer.exe. Then turn this off here, or run dashboard.exe --shell-off.";
const PROMPT: &str = "Start with Windows as the shell?";
const PROMPT_ABOUT: &str =
    "From your next sign-in, the Dashboard starts in place of the Windows desktop. To get to the \
                            desktop, choose Exit to desktop or hold View and Menu for 5 seconds.";

/// What the line under the checkbox says of shell mode as it is.
pub fn status(shell: ShellMode) -> &'static str {
    match shell {
        ShellMode::Unknown => "Shell mode is only on Windows.",
        ShellMode::Off => "Off. Windows starts its desktop when you sign in.",
        ShellMode::On => "On. The Dashboard starts in place of the desktop when you sign in.",
        ShellMode::Refused(refusal) => refusal.says(),
        ShellMode::Failed => "Couldn't change it, and nothing was left half done. The Dashboard's log says why.",
    }
}

/// Shell mode as the log says it (`shell: Settings > System shows refused not-installed`): `on`, `off`,
/// `refused <why>`, `failed`, `unknown`.
pub fn state(shell: ShellMode) -> String {
    match shell {
        ShellMode::Unknown => "unknown".into(),
        ShellMode::Off => "off".into(),
        ShellMode::On => "on".into(),
        ShellMode::Refused(refusal) => format!("refused {}", refusal.key()),
        ShellMode::Failed => "failed".into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    TurnOn,
    Cancel,
}

/// Each prompt flies up afresh: a number for every one.
static FLIGHTS: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
pub struct System {
    prompt: Option<MenuPanel<Confirm>>,
    flying: u32,
    leaving: Option<(MenuPanel<Confirm>, u32)>,
}

impl System {
    fn prompt_press(&mut self, press: Press) -> Step {
        let Some(prompt) = self.prompt.as_mut() else { return Step::Stay };
        let chose = match prompt.press(press) {
            Answer::Chose(confirm) => confirm,
            Answer::Declined => Confirm::Cancel,
            Answer::Moved | Answer::Nothing => return Step::Stay,
        };
        if let Some(prompt) = self.prompt.take() {
            self.leaving = Some((prompt, self.flying));
        }
        match chose {
            Confirm::TurnOn => Step::Do(PageAction::Effect(Effect::ShellMode(true))),
            Confirm::Cancel => Step::Stay,
        }
    }
}

impl Pane for System {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `System: Start with Windows as the shell`; with the question up, `System: Prompt(Turn on) of Start with
    /// Windows as the shell`.
    fn focus(&self) -> String {
        match &self.prompt {
            Some(prompt) => {
                let row = prompt.rows().get(prompt.focus()).map_or("", |row| row.label.as_str());
                format!("{TITLE}: Prompt({row}) of {SHELL}")
            }
            None => format!("{TITLE}: {SHELL}"),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if self.prompt.is_some() {
            return self.prompt_press(press);
        }
        if press.repeat {
            return Step::Stay;
        }
        match (press.button, around.options.shell) {
            (Button::B | Button::Left, _) => Step::Back,
            // Off is never asked: it is the way back.
            (Button::A, ShellMode::On) => Step::Do(PageAction::Effect(Effect::ShellMode(false))),
            (Button::A, ShellMode::Off | ShellMode::Failed) => {
                let answers = vec![
                    Row::new(icon::PLAY, "Turn on", Confirm::TurnOn),
                    Row::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                ];
                self.prompt = Some(MenuPanel::prompt(PROMPT, Some(PROMPT_ABOUT), answers));
                self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
                self.leaving = None;
                Step::Stay
            }
            // Refused, or nowhere to turn it on: the line under it says why.
            _ => Step::Stay,
        }
    }

    fn dialog(&self) -> Option<Dialog> {
        self.prompt.as_ref().map(|_| Dialog::Prompt)
    }

    fn dismiss_dialog(&mut self) {
        self.prompt = None;
        self.leaving = None;
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let shell = around.options.shell;
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, ABOUT) + 18.0;
        let row = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        controls::checkbox_row(ui, row, SHELL, shell == ShellMode::On, focused && self.prompt.is_none());
        y += FIELD_HEIGHT + 12.0;
        let style = BODY.color(TEXT_DIM);
        y += ui
            .paragraph(Rect::new(LEFT, y, ROW_WIDTH, 2000.0), status(shell), &style, HAlign::Left, VAlign::Top, 0)
            .height
            + 40.0;
        y += controls::label(ui, LEFT, y, BACK_LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, BACK_ABOUT) + 40.0;
        y += controls::label(ui, LEFT, y, STUCK_LABEL) + 4.0;
        controls::description(ui, LEFT, y, ROW_WIDTH, STUCK_ABOUT);
        // The question over the dimmed page; or on its way out, the dim gone at once.
        if let Some(prompt) = &self.prompt {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("system prompt", self.flying), false);
            prompt.draw_flown(ui, fly, shown);
        } else if let Some((prompt, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("system prompt", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.leaving = None;
            } else {
                prompt.draw_flown(ui, fly, shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use model::shell_mode::Refusal;

    use super::*;

    #[test]
    fn every_state_has_its_line() {
        assert!(status(ShellMode::Off).starts_with("Off."));
        assert!(status(ShellMode::On).starts_with("On."));
        assert_eq!(status(ShellMode::Refused(Refusal::NotInstalled)), Refusal::NotInstalled.says());
        assert_eq!(state(ShellMode::Refused(Refusal::BundleBootShell)), "refused bundle-boot-shell");
    }
}
