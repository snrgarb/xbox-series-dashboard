//! Account > Discord messages (#197; docs/discord-web.md, docs/spec/settings.md 6.10): the Dashboard's own
//! discord.com session, which the Guide's Party will read DMs and notifications through (#198-#202). The port's own
//! page, laid out as System's: a label, the switch 14 under, the terms said plainly under it, how the session stands,
//! then Sign in and Sign out. The switch is off until the user turns it on, and nothing of the session starts before.
//!
//! Voice is not here: it stays on Settings > Discord's connection (`discord`), which this page never touches.
use model::discord_web::{Web, WebAsk, WebState, DEVICES, TERMS};
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, FIELD_HEIGHT, FIELD_WIDTH, LEFT, ROW_WIDTH, TOP};
use super::options::{Change, Setting};
use super::{Pane, Step};
use crate::pages::{Around, Open, PageAction};
use crate::Effect;

pub const TITLE: &str = "Discord messages";
pub const SWITCH: &str = "Use Discord messages";
const SIGN_IN: &str = "Sign in";
const SIGN_OUT: &str = "Sign out";
const BUTTON_GAP: f32 = 12.0;

/// Where focus is: the switch, or one of the two buttons under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Focus {
    #[default]
    Switch,
    SignIn,
    SignOut,
}

impl Focus {
    fn name(self) -> &'static str {
        match self {
            Focus::Switch => SWITCH,
            Focus::SignIn => SIGN_IN,
            Focus::SignOut => SIGN_OUT,
        }
    }
}

#[derive(Default)]
pub struct DiscordWeb {
    focus: Focus,
    /// How the session stood when last drawn or pressed: what the focus log says after the focus.
    seen: Web,
    on: bool,
}

impl Pane for DiscordWeb {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `Discord messages: Use Discord messages`, `Discord messages: Sign in`.
    fn focus(&self) -> String {
        format!("{TITLE}: {}", self.focus.name())
    }

    /// `on, signed in, kept`: the switch, how the session stands, and whether a session is kept on the PC.
    fn state(&self) -> String {
        let mut state = format!("{}, {}", if self.on { "on" } else { "off" }, self.seen.state.word());
        if self.seen.kept {
            state.push_str(", kept");
        }
        state
    }

    fn tick(&mut self, around: Around) {
        self.on = around.options.discord_messages;
        self.seen = around.options.discord_web;
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        let options = around.options;
        self.on = options.discord_messages;
        self.seen = options.discord_web;
        let web = options.discord_web;
        match press.button {
            Button::B => Step::Back,
            Button::Left if self.focus != Focus::SignOut => Step::Back,
            Button::Left => {
                self.focus = Focus::SignIn;
                Step::Stay
            }
            Button::Right if self.focus == Focus::SignIn => {
                self.focus = Focus::SignOut;
                Step::Stay
            }
            Button::Down if self.focus == Focus::Switch && self.on => {
                self.focus = Focus::SignIn;
                Step::Stay
            }
            Button::Up if self.focus != Focus::Switch => {
                self.focus = Focus::Switch;
                Step::Stay
            }
            _ if press.repeat => Step::Stay,
            Button::A => match self.focus {
                Focus::Switch => {
                    self.on = !self.on;
                    let change = Change { setting: Setting::DiscordMessages, on: self.on };
                    Step::Do(PageAction::Effect(Effect::Set(change)))
                }
                Focus::SignIn if self.on && web.state.can_sign_in() => Step::Do(PageAction::Open(Open::DiscordSignIn)),
                Focus::SignOut if web.can_sign_out() => {
                    Step::Do(PageAction::Effect(Effect::DiscordWeb(WebAsk::SignOut)))
                }
                _ => Step::Stay,
            },
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let options = around.options;
        self.on = options.discord_messages;
        self.seen = options.discord_web;
        if !self.on && self.focus != Focus::Switch {
            self.focus = Focus::Switch;
        }
        let web = options.discord_web;
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, TITLE) + 14.0;
        let row = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        controls::checkbox_row(ui, row, SWITCH, self.on, focused && self.focus == Focus::Switch);
        y += FIELD_HEIGHT + 12.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, TERMS) + 40.0;
        y += controls::label(ui, LEFT, y, "Status") + 4.0;
        let state = if self.on { web.state } else { WebState::Off };
        y += controls::description(ui, LEFT, y, ROW_WIDTH, state.line()) + 14.0;
        if !self.on {
            return;
        }
        let mut x = LEFT;
        for (focus, enabled) in [(Focus::SignIn, web.state.can_sign_in()), (Focus::SignOut, web.can_sign_out())] {
            let width = controls::pane_button_width(ui, focus.name());
            let button = Rect::new(x, y, width, FIELD_HEIGHT);
            controls::pane_button(ui, button, focus.name(), enabled, focused && self.focus == focus);
            x += width + BUTTON_GAP;
        }
        y += FIELD_HEIGHT + 14.0;
        if web.kept || web.state == WebState::SignedIn {
            controls::description(ui, LEFT, y, ROW_WIDTH, DEVICES);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_says_the_switch_the_session_and_what_is_kept() {
        let page = DiscordWeb {
            on: true,
            seen: Web { state: WebState::SignedIn, kept: true, field: None },
            ..DiscordWeb::default()
        };
        assert_eq!(page.state(), "on, signed in, kept");
        assert_eq!(DiscordWeb::default().state(), "off, off");
    }
}
