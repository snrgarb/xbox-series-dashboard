//! Account > Discord (#197 and #200; docs/spec/party-chats.md 8; docs/discord-web.md): the Dashboard's own
//! discord.com session, which the Guide's Direct Messages and its Discord tab's voice go through, and the one Discord
//! page there is (the RPC connection's was retired, #299). The port's own page, laid out as System's:
//! the switch from y 190, the risk said plainly under it, Sign in and Sign out, how the session stands, then the
//! Notifications the Toasts will read (Direct Messages, Show what they say, Mentions, Incoming Calls, and While
//! playing). The switch is off until the user turns it on, and nothing of the session starts before.
use model::discord_dms::DirectState;
use model::discord_web::{Web, WebAsk, WebState, DEVICES};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, Dropdown, BOXES_TOP, FIELD_HEIGHT, FIELD_WIDTH, LEFT, RIGHT, TOP};
use super::options::{Change, Setting, WhilePlaying};
use super::{Pane, Step};
use crate::pages::{Around, Open, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

pub const TITLE: &str = "Discord";
pub const SWITCH: &str = "Direct Messages from discord.com";
/// What turning it on does, as the user must know first (docs/spec/party-chats.md, 8.2).
pub const RISK: &str = "This signs in to discord.com as you, then reads and sends your messages for you. Discord's terms don't allow programs to do that with a user account, and Discord could close your account for it.";
const SIGN_IN: &str = "Sign in";
const SIGN_OUT: &str = "Sign out";
const NOTIFICATIONS: &str = "Notifications";
const WHILE_PLAYING: &str = "While playing";
const BUTTON_GAP: f32 = 20.0;
/// Between one row and the next.
const PITCH: f32 = FIELD_HEIGHT + 12.0;

/// The four checkboxes under Notifications.
const NOTIFY: [(&str, Setting); 4] = [
    ("Direct Messages", Setting::NotifyDirectMessages),
    ("Show what they say", Setting::NotifyShowWhatTheySay),
    ("Mentions", Setting::NotifyMentions),
    ("Incoming Calls", Setting::NotifyIncomingCalls),
];

/// Where focus is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Focus {
    #[default]
    Switch,
    SignIn,
    SignOut,
    /// One of the four Notifications, by its place in [`NOTIFY`].
    Notify(usize),
    WhilePlaying,
}

impl Focus {
    fn name(self) -> &'static str {
        match self {
            Focus::Switch => SWITCH,
            Focus::SignIn => SIGN_IN,
            Focus::SignOut => SIGN_OUT,
            Focus::Notify(at) => NOTIFY[at].0,
            Focus::WhilePlaying => WHILE_PLAYING,
        }
    }
}

#[derive(Default)]
pub struct DiscordWeb {
    focus: Focus,
    /// How the session stood when last drawn or pressed: what the focus log says after the focus.
    seen: Web,
    direct: Option<DirectState>,
    on: bool,
    while_playing: Dropdown,
    scroll: f32,
}

/// The status line: how the session and the DM list stand, in the words of the spec (8.2).
pub fn status_line(on: bool, web: Web, direct: DirectState) -> &'static str {
    if !on {
        return "Off";
    }
    match (direct, web.state) {
        (DirectState::PageChanged, _) => "Discord has changed its page",
        (DirectState::Unreachable, _) => "Can't reach Discord",
        (DirectState::NoRuntime, _) | (_, WebState::NoRuntime) => "Microsoft Edge WebView2 isn't on this PC",
        (DirectState::Loading, WebState::SignedIn) | (_, WebState::Checking) => "Starting...",
        (_, WebState::SignedOut) if web.kept => "Discord signed you out",
        (_, WebState::SignedOut) | (_, WebState::Off) => "Not signed in",
        (_, WebState::SigningIn) => "Signing in...",
        (_, WebState::SignedIn) => "Signed in",
        (_, WebState::Failed) => "Discord's page couldn't be opened.",
    }
}

/// The log's word for it: `signed in`, `signed out`, `starting`, `no runtime`, `unreachable`, `page changed`.
pub fn state_word(web: Web, direct: DirectState) -> &'static str {
    match (direct, web.state) {
        (DirectState::PageChanged, _) => "page changed",
        (DirectState::Unreachable, _) => "unreachable",
        (DirectState::NoRuntime, _) | (_, WebState::NoRuntime) => "no runtime",
        (DirectState::Loading, WebState::SignedIn) | (_, WebState::Checking) => "starting",
        (_, state) => state.word(),
    }
}

impl DiscordWeb {
    /// The page with focus on Sign in: where the Guide's sign-in line lands.
    pub fn at_sign_in() -> Self {
        Self { focus: Focus::SignIn, ..Self::default() }
    }

    fn take(&mut self, around: Around) {
        let options = around.options;
        self.on = options.discord_messages;
        self.seen = options.discord_web;
        self.direct = Some(options.direct_state);
    }

    /// The buttons are there to be focused only while the switch is on.
    fn buttons(&self) -> bool {
        self.on
    }

    fn up(&self, from: Focus) -> Focus {
        match from {
            Focus::Switch => Focus::Switch,
            Focus::SignIn | Focus::SignOut => Focus::Switch,
            Focus::Notify(0) if self.buttons() => Focus::SignIn,
            Focus::Notify(0) => Focus::Switch,
            Focus::Notify(at) => Focus::Notify(at - 1),
            Focus::WhilePlaying => Focus::Notify(NOTIFY.len() - 1),
        }
    }

    fn down(&self, from: Focus) -> Focus {
        match from {
            Focus::Switch if self.buttons() => Focus::SignIn,
            Focus::Switch | Focus::SignIn | Focus::SignOut => Focus::Notify(0),
            Focus::Notify(at) if at + 1 < NOTIFY.len() => Focus::Notify(at + 1),
            Focus::Notify(_) | Focus::WhilePlaying => Focus::WhilePlaying,
        }
    }
}

impl Pane for DiscordWeb {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `Discord: Direct Messages from discord.com`, `Discord: Sign in`, `Discord: While
    /// playing`.
    fn focus(&self) -> String {
        match self.while_playing.focused() {
            Some(choice) => format!("{TITLE}: {WHILE_PLAYING}: {choice}"),
            None => format!("{TITLE}: {}", self.focus.name()),
        }
    }

    /// `dms on, signed in`: the switch, and how the session and the list stand. No name (docs/spec/party-chats.md,
    /// 8.2).
    fn state(&self) -> String {
        let direct = self.direct.unwrap_or(DirectState::Off);
        format!("dms {}, {}", if self.on { "on" } else { "off" }, state_word(self.seen, direct))
    }

    fn tick(&mut self, around: Around) {
        self.take(around);
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        self.take(around);
        let options = around.options;
        let web = options.discord_web;
        if self.while_playing.is_open() {
            return match self.while_playing.press(press.button) {
                Some(at) => match WhilePlaying::ALL.get(at) {
                    Some(&choice) => Step::Do(PageAction::Effect(Effect::DiscordWhilePlaying(choice))),
                    None => Step::Stay,
                },
                None => Step::Stay,
            };
        }
        match press.button {
            Button::B => Step::Back,
            Button::Left if !matches!(self.focus, Focus::SignOut) => Step::Back,
            Button::Left => {
                self.focus = Focus::SignIn;
                Step::Stay
            }
            Button::Right if self.focus == Focus::SignIn => {
                self.focus = Focus::SignOut;
                Step::Stay
            }
            Button::Down => {
                self.focus = self.down(self.focus);
                Step::Stay
            }
            Button::Up => {
                self.focus = self.up(self.focus);
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
                Focus::SignOut if self.on && web.can_sign_out() => {
                    Step::Do(PageAction::Effect(Effect::DiscordWeb(WebAsk::SignOut)))
                }
                Focus::SignIn | Focus::SignOut => Step::Stay,
                Focus::Notify(at) => {
                    let (_, setting) = NOTIFY[at];
                    let change = Change { setting, on: !options.get(setting) };
                    Step::Do(PageAction::Effect(Effect::Set(change)))
                }
                Focus::WhilePlaying => {
                    let choices = WhilePlaying::ALL.iter().map(|choice| choice.name().to_string()).collect();
                    let set = WhilePlaying::ALL.iter().position(|&choice| choice == options.while_playing).unwrap_or(0);
                    self.while_playing.open(choices, set);
                    Step::Stay
                }
            },
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        self.take(around);
        let options = around.options;
        let web = options.discord_web;
        let direct = options.direct_state;
        if !self.on && matches!(self.focus, Focus::SignIn | Focus::SignOut) {
            self.focus = Focus::Switch;
        }
        // Where each part is, from y 190.
        let mut y = TOP;
        let switch = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        y += FIELD_HEIGHT + 14.0;
        let risk_height = ui.measure_wrapped(&BODY, RISK, RIGHT - LEFT, 0).height;
        let risk_top = y;
        y += risk_height + 24.0;
        let buttons_top = y;
        y += FIELD_HEIGHT + 14.0;
        let status_top = y;
        let kept = (web.kept || web.state == WebState::SignedIn) && self.on;
        y += ui.measure_wrapped(&BODY, DEVICES, RIGHT - LEFT, 0).height.max(35.0) * if kept { 2.0 } else { 1.0 } + 30.0;
        let notifications = y;
        y += 40.0 + 14.0;
        let rows: Vec<Rect> =
            (0..NOTIFY.len()).map(|at| Rect::new(LEFT, y + PITCH * at as f32, FIELD_WIDTH, FIELD_HEIGHT)).collect();
        y += PITCH * NOTIFY.len() as f32 + 18.0;
        let playing_label = y;
        let playing_box = Rect::new(LEFT, y + 40.0 + 6.0, FIELD_WIDTH, FIELD_HEIGHT);
        let rect = match self.focus {
            Focus::Switch => switch,
            Focus::SignIn | Focus::SignOut => Rect::new(LEFT, buttons_top, 300.0, FIELD_HEIGHT),
            Focus::Notify(at) => rows[at],
            Focus::WhilePlaying if self.while_playing.is_open() => {
                Rect::new(playing_box.x, playing_box.y, playing_box.w, playing_box.h + 260.0)
            }
            Focus::WhilePlaying => playing_box,
        };
        let value = options.while_playing.name();
        let Self { focus, on, scroll, while_playing, .. } = self;
        let (focus, on) = (*focus, *on);
        controls::scrolled(ui, scroll, BOXES_TOP.min(TOP), rect.y, rect.bottom(), |ui| {
            controls::checkbox_row(ui, switch, SWITCH, on, focused && focus == Focus::Switch);
            ui.paragraph(
                Rect::new(LEFT, risk_top, RIGHT - LEFT, risk_height),
                RISK,
                &BODY.color(TEXT_DIM),
                HAlign::Left,
                VAlign::Top,
                0,
            );
            let mut x = LEFT;
            for (which, name, enabled) in [
                (Focus::SignIn, SIGN_IN, on && web.state.can_sign_in()),
                (Focus::SignOut, SIGN_OUT, on && web.can_sign_out()),
            ] {
                let width = controls::pane_button_width(ui, name);
                let button = Rect::new(x, buttons_top, width, FIELD_HEIGHT);
                controls::pane_button(ui, button, name, enabled, focused && on && focus == which);
                x += width + BUTTON_GAP;
            }
            let line = status_line(on, web, direct);
            ui.paragraph(Rect::new(LEFT, status_top, RIGHT - LEFT, 35.0), line, &BODY, HAlign::Left, VAlign::Top, 1);
            if kept {
                let devices = Rect::new(LEFT, status_top + 35.0, RIGHT - LEFT, 70.0);
                ui.paragraph(devices, DEVICES, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 0);
            }
            controls::label(ui, LEFT, notifications, NOTIFICATIONS);
            for (at, &(name, setting)) in NOTIFY.iter().enumerate() {
                controls::checkbox_row(ui, rows[at], name, options.get(setting), focused && focus == Focus::Notify(at));
            }
            controls::label(ui, LEFT, playing_label, WHILE_PLAYING);
            while_playing.draw(ui, playing_box, value, focused && focus == Focus::WhilePlaying);
        });
        let shown = Rect::new(playing_box.x, playing_box.y - *scroll, playing_box.w, playing_box.h);
        while_playing.draw_list(ui, shown);
        let _ = Color::WHITE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_says_the_switch_and_how_the_session_and_the_list_stand_and_no_name() {
        let page = DiscordWeb {
            on: true,
            seen: Web { state: WebState::SignedIn, kept: true, field: None },
            direct: Some(DirectState::Ready),
            ..DiscordWeb::default()
        };
        assert_eq!(page.state(), "dms on, signed in");
        assert_eq!(DiscordWeb::default().state(), "dms off, off");
    }

    #[test]
    fn the_status_line_uses_the_specs_words() {
        let web = |state, kept| Web { state, kept, field: None };
        let line = |web, direct| status_line(true, web, direct);
        assert_eq!(line(web(WebState::SignedOut, false), DirectState::Off), "Not signed in");
        assert_eq!(line(web(WebState::SignedOut, true), DirectState::SignedOut), "Discord signed you out");
        assert_eq!(line(web(WebState::Checking, false), DirectState::Loading), "Starting...");
        assert_eq!(
            line(web(WebState::NoRuntime, false), DirectState::NoRuntime),
            "Microsoft Edge WebView2 isn't on this PC"
        );
        assert_eq!(line(web(WebState::SignedIn, true), DirectState::Unreachable), "Can't reach Discord");
        assert_eq!(line(web(WebState::SignedIn, true), DirectState::PageChanged), "Discord has changed its page");
        assert_eq!(state_word(web(WebState::SignedIn, true), DirectState::Ready), "signed in");
        assert_eq!(state_word(web(WebState::SignedIn, true), DirectState::PageChanged), "page changed");
    }
}
