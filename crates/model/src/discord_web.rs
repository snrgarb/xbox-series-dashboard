//! Discord's web session (#197; docs/discord-web.md): the Dashboard's own discord.com, signed in by the user in
//! Discord's own page and kept in the data folder, one per Profile. Off unless Settings > Account > Discord
//! turns it on. Voice stays on the RPC connection (`discord`); this is what text will come through (#198).
//!
//! What the screens see of it: how it stands, and the kind of field focus is on while its sign-in page is shown.
//! Never a name, an address, a token or anything typed.

/// The one plain line Settings keeps beside the switch (docs/spec/settings.md, the kept lines): the user must know
/// it before turning it on. From Discord's Terms of Service and its self-bot article, as
/// docs/research/discord-bridge.md (section 4) quotes them.
pub const TERMS: &str = "Discord's terms don't allow other programs to use your account, and Discord says it can \
close an account for it. With this on, the Dashboard signs in to discord.com as you and reads the page for you.";

/// Under the status once signed in: removing the session here is not Discord's own log out.
pub const DEVICES: &str = "Signing out here removes the session from this PC. Discord lists it under Devices until \
you log it out there.";

/// How the session stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebState {
    /// Settings' switch is off: nothing runs.
    #[default]
    Off,
    /// The switch is on, but this PC has no WebView2 runtime (or the Dashboard lacks its loader).
    NoRuntime,
    /// Looking whether the session is signed in.
    Checking,
    SignedOut,
    /// Discord's sign-in page is up.
    SigningIn,
    SignedIn,
    /// The web view could not be started or ended unexpectedly; the log says why.
    Failed,
}

impl WebState {
    /// Settings' status line.
    pub fn line(self) -> &'static str {
        match self {
            WebState::Off => "Off",
            WebState::NoRuntime => "This PC doesn't have Microsoft Edge WebView2, which Discord's page needs.",
            WebState::Checking => "Checking...",
            WebState::SignedOut => "Not signed in",
            WebState::SigningIn => "Signing in...",
            WebState::SignedIn => "Signed in",
            WebState::Failed => "Discord's page couldn't be opened.",
        }
    }

    /// The log's word for it.
    pub fn word(self) -> &'static str {
        match self {
            WebState::Off => "off",
            WebState::NoRuntime => "no runtime",
            WebState::Checking => "checking",
            WebState::SignedOut => "signed out",
            WebState::SigningIn => "signing in",
            WebState::SignedIn => "signed in",
            WebState::Failed => "failed",
        }
    }

    pub fn can_sign_in(self) -> bool {
        matches!(self, WebState::SignedOut | WebState::Failed)
    }
}

/// The kind of text field focus is on in the sign-in page: what the Console keyboard is opened for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebField {
    Text,
    /// Typed as dots.
    Password,
}

/// The session as the screens see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Web {
    pub state: WebState,
    /// The sign-in page's focused text field, while it is up.
    pub field: Option<WebField>,
    /// A session folder is on the PC for this Profile: Sign out has something to remove.
    pub kept: bool,
}

impl Web {
    pub fn can_sign_out(&self) -> bool {
        self.kept || matches!(self.state, WebState::SignedIn)
    }
}

/// A key the sign-in page sends to Discord's page, from the pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebKey {
    Tab,
    ShiftTab,
    Enter,
    Escape,
    Space,
    Backspace,
    Left,
    Right,
    Up,
    Down,
}

/// What Settings and the sign-in page ask of the session.
#[derive(Clone, PartialEq, Eq)]
pub enum WebAsk {
    /// Discord's sign-in page, over the sign-in page's area.
    ShowSignIn,
    /// Puts it away (text entry is up over it, or the page closed).
    HideSignIn,
    Key(WebKey),
    /// What the Console keyboard kept, in place of the focused field's text. Never logged.
    Text(String),
    /// The session's folder removed: nothing of it left on the PC.
    SignOut,
}

/// Never says the text: it may be a password.
impl std::fmt::Debug for WebAsk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebAsk::ShowSignIn => write!(f, "ShowSignIn"),
            WebAsk::HideSignIn => write!(f, "HideSignIn"),
            WebAsk::Key(key) => write!(f, "Key({key:?})"),
            WebAsk::Text(text) => write!(f, "Text({} characters)", text.chars().count()),
            WebAsk::SignOut => write!(f, "SignOut"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_never_in_a_debug_line() {
        let line = format!("{:?}", WebAsk::Text("hunter2".into()));
        assert_eq!(line, "Text(7 characters)");
    }

    #[test]
    fn sign_out_needs_something_to_remove() {
        assert!(!Web::default().can_sign_out());
        assert!(Web { kept: true, ..Web::default() }.can_sign_out());
        assert!(WebState::SignedOut.can_sign_in() && !WebState::Off.can_sign_in());
    }
}
