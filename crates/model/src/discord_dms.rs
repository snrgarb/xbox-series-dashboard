//! Discord's Direct Messages as the screens see them (docs/spec/party-chats.md; M9): the DM list with its unread
//! counts, the conversation that is open, a ring, and what the screens ask of the web session (`discord_web`), which
//! reads them from Discord's own in-page stores (docs/research/discord-web-voice.md, 4b) and keeps them in memory
//! only (the spec's section 9).
//!
//! Nothing here is ever written to disk or to the log. Every name and every piece of a message is a [`Private`],
//! whose `Debug` says its length only, so a `{:?}` or a panic cannot print one; ids are [`Id`], whose `Debug` says
//! nothing of them. The log gets places and counts ([`Direct::counts`]).
use std::sync::Arc;

/// A name, or text someone wrote. `Debug` is its length: `(24 chars)`.
#[derive(Clone, PartialEq, Eq, Default, Hash)]
pub struct Private(pub String);

impl Private {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Private {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Private {
    fn from(text: &str) -> Self {
        Private(text.to_string())
    }
}

impl From<String> for Private {
    fn from(text: String) -> Self {
        Private(text)
    }
}

impl std::fmt::Debug for Private {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} chars)", self.0.chars().count())
    }
}

/// One of Discord's ids (a channel's, a message's, a user's). Never in the log: `Debug` is `Id`.
#[derive(Clone, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct Id(pub String);

impl Id {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Id {
    fn from(id: &str) -> Self {
        Id(id.to_string())
    }
}

impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Id")
    }
}

/// An avatar or a Group DM's icon, as fetched: the encoded picture (PNG, WebP or JPEG; decode by content), held in
/// memory only and dropped with the web session. Equal when it is the same fetch.
#[derive(Clone)]
pub struct Picture(pub Arc<[u8]>);

impl PartialEq for Picture {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Picture {}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture({} bytes)", self.0.len())
    }
}

/// Why the Direct Messages section shows a line in the list's place (the spec's 3.5), or that it can show the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirectState {
    /// Settings' switch is off: the tab has no Direct Messages section.
    #[default]
    Off,
    /// The web session is coming up, or the list is not yet read.
    Loading,
    SignedOut,
    NoRuntime,
    /// No network, or Discord is down: tried again by itself.
    Unreachable,
    /// One of Discord's stores or methods the Dashboard reads is no longer there (the health probe): nothing can be
    /// read until the Dashboard is updated.
    PageChanged,
    /// The list is read (it may be empty).
    Ready,
}

impl DirectState {
    /// The log's word (the spec's 10: `DMsState(SignedOut)`); None while the list shows.
    pub fn word(self, empty: bool) -> Option<&'static str> {
        match self {
            DirectState::Off => Some("Off"),
            DirectState::Loading => Some("Loading"),
            DirectState::SignedOut => Some("SignedOut"),
            DirectState::NoRuntime => Some("NoRuntime"),
            DirectState::Unreachable => Some("Unreachable"),
            DirectState::PageChanged => Some("PageChanged"),
            DirectState::Ready if empty => Some("Empty"),
            DirectState::Ready => None,
        }
    }
}

/// Discord's Direct Messages, as the web session last read them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Direct {
    pub state: DirectState,
    /// Every DM and Group DM, in Discord's own order (most recent first).
    pub dms: Vec<Dm>,
    /// The conversation the screens asked to open ([`DirectAsk::Open`]), once its messages have come.
    pub open: Option<Conversation>,
    /// DM calls ringing the user now.
    pub rings: Vec<Ring>,
}

impl Direct {
    pub fn dm(&self, id: &Id) -> Option<&Dm> {
        self.dms.iter().find(|dm| &dm.id == id)
    }

    /// Unread messages over all DMs, and in how many DMs.
    pub fn unread(&self) -> (u32, usize) {
        let unread = self.dms.iter().filter(|dm| dm.unread > 0);
        (unread.clone().map(|dm| dm.unread).sum(), unread.count())
    }

    /// The log's figures: `dms 25, unread 5 in 2`.
    pub fn counts(&self) -> String {
        let (unread, in_dms) = self.unread();
        format!("dms {}, unread {unread} in {in_dms}", self.dms.len())
    }
}

/// One DM or Group DM in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dm {
    /// The channel's id: what opening, sending and calling name it by.
    pub id: Id,
    /// The other person's display name; a Group DM's name, else its members' names joined by ", ".
    pub name: Private,
    pub group: bool,
    /// A Group DM's members, the user included; 2 for a DM.
    pub members: u32,
    /// The other person's avatar or the Group DM's icon, once fetched; None for none, or until it has come.
    pub picture: Option<Picture>,
    /// Unread messages, as Discord counts them.
    pub unread: u32,
    /// The last message's first line and time (Unix milliseconds), where Discord's stores hold them.
    pub last: Option<(Private, i64)>,
}

/// A DM call ringing the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ring {
    /// The DM's channel.
    pub channel: Id,
    pub caller: Private,
    pub picture: Option<Picture>,
}

/// The open conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub id: Id,
    pub name: Private,
    pub group: bool,
    pub picture: Option<Picture>,
    /// Oldest first.
    pub messages: Vec<Message>,
    /// Older messages were asked for ([`DirectAsk::Older`]) and have not yet come.
    pub loading_older: bool,
    /// The oldest loaded message is the conversation's first.
    pub at_start: bool,
    /// The first message that was unread when the conversation opened: the unread line stands before it.
    pub unread_from: Option<Id>,
}

/// Whether a message of the user's own has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Delivery {
    /// Discord's store holds it as sent (everyone else's are always this).
    #[default]
    Sent,
    Sending,
    /// Discord's page shows it failed, or nothing showed within 15 s.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Discord's id; for one going out or failed, the nonce the Dashboard sent it under ([`DirectAsk::Send`]).
    pub id: Id,
    pub author: Id,
    pub author_name: Private,
    /// The author's avatar, shown in a Group DM.
    pub author_picture: Option<Picture>,
    pub own: bool,
    /// Unix milliseconds.
    pub at: i64,
    pub edited: bool,
    pub body: Body,
    pub delivery: Delivery,
}

/// What a message is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// Someone's message: a bubble.
    Said {
        text: Vec<Span>,
        reply: Option<Reply>,
        /// What isn't text, a line each (the spec's 5.3).
        extras: Vec<Extra>,
        reactions: Vec<Reaction>,
    },
    /// One of Discord's own lines: centred, no bubble.
    Line { kind: LineKind, text: Private },
}

/// A run of a message's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    /// Plain text, emoji as Unicode included.
    Text(Private),
    /// `@name`, with the `@`.
    Mention(Private),
    /// A link, as text.
    Link(Private),
    /// A server's own emoji, by name without colons.
    ServerEmoji(Private),
}

/// The message another answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    To {
        author: Private,
        line: Private,
    },
    /// "Original message was deleted".
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extra {
    Images(u32),
    Video,
    Gif,
    /// Its file name.
    File(Private),
    VoiceMessage,
    /// Its name.
    Sticker(Private),
    /// A link preview: its title, else its site.
    Embed(Private),
    /// Its question.
    Poll(Private),
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub emoji: Emoji,
    pub count: u32,
    /// The user made it too.
    pub mine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Emoji {
    /// The emoji itself.
    Unicode(Private),
    /// A server's own, by name.
    Server(Private),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    CallStarted,
    Added,
    Pinned,
    Other,
}

/// The six reactions the Dashboard offers (the spec's 6.3, decision 9), with the Dashboard's names for them.
pub const REACTIONS: [(&str, &str); 6] = [
    ("\u{1F44D}", "Thumbs up"),
    ("\u{2764}\u{FE0F}", "Heart"),
    ("\u{1F602}", "Laughing"),
    ("\u{1F62E}", "Surprised"),
    ("\u{1F622}", "Sad"),
    ("\u{1F525}", "Fire"),
];

/// Discord's limit for a message.
pub const MESSAGE_LIMIT: usize = 2000;

/// What the screens ask of the web session about Direct Messages. Each is shown done only once Discord's own stores
/// show it; the session does one at a time, paced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectAsk {
    /// Open this DM's conversation: its messages are read, and [`Direct::open`] follows it.
    Open(Id),
    /// The conversation closed: its messages are forgotten.
    Close,
    /// Older messages of the open conversation.
    Older,
    /// Send `text` to a DM, as a reply to `reply` if any. `nonce` names it until Discord gives it an id.
    Send {
        channel: Id,
        text: Private,
        reply: Option<Id>,
        nonce: u64,
    },
    /// A failed message dropped from the view (it was never sent), by its nonce id.
    Forget {
        channel: Id,
        message: Id,
    },
    /// One of [`REACTIONS`] added to a message, or taken off.
    React {
        channel: Id,
        message: Id,
        emoji: Private,
        on: bool,
    },
    MarkRead(Id),
    /// Start a call in this DM (its menu's Call).
    Call(Id),
    /// Answer or decline the call ringing in this DM.
    Answer(Id),
    Decline(Id),
}

/// What the web session tells the screens once, as it happens: the Toasts (the spec's 7) and a refusal's prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectEvent {
    /// A message came in a DM that is not the open one.
    Message { channel: Id, name: Private, group: Option<Private>, line: Private, picture: Option<Picture> },
    /// The user was mentioned in a server's channel.
    Mention { name: Private, channel_name: Private },
    /// A DM call began ringing (it is in [`Direct::rings`] while it does).
    Ring(Ring),
    /// The ring in this DM stopped (answered here or elsewhere, declined, or given up).
    RingEnded(Id),
    /// Discord wants a captcha before it sends: "Discord wants to check it's you".
    Captcha,
    /// Discord refused to send: "Discord didn't send it", with its own reason as its page gives it.
    Refused(Private),
    /// A message sent under this nonce ([`DirectAsk::Send`]) is now Discord's, with this id (#201): the bubble kept
    /// under the nonce becomes that message. Raised once, after the store showed it sent.
    Sent { channel: Id, nonce: u64, message: Id },
    /// A message sent under this nonce was not sent: refused, a captcha, a failure Discord's page showed, or no answer
    /// within 15 s (#201). Its bubble stays under the nonce, [`Delivery::Failed`], until [`DirectAsk::Forget`].
    NotSent { channel: Id, nonce: u64 },
}

impl DirectEvent {
    /// The log's word for the Toast it raises (the spec's 10), if it raises one.
    pub fn toast(&self) -> Option<&'static str> {
        match self {
            DirectEvent::Message { .. } => Some("DiscordDm"),
            DirectEvent::Mention { .. } => Some("DiscordMention"),
            DirectEvent::Ring(_) => Some("DiscordCall"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dm(id: &str, unread: u32) -> Dm {
        Dm { id: id.into(), name: "Ana Example".into(), group: false, members: 2, picture: None, unread, last: None }
    }

    #[test]
    fn nothing_personal_is_in_a_debug_line() {
        let direct = Direct {
            state: DirectState::Ready,
            dms: vec![Dm { last: Some(("are you on tonight?".into(), 1)), ..dm("123456789", 3) }],
            open: None,
            rings: vec![Ring { channel: "987654".into(), caller: "Bo Example".into(), picture: None }],
        };
        let said = format!("{direct:?}");
        for private in ["Ana", "Bo", "tonight", "123456789", "987654"] {
            assert!(!said.contains(private), "{private} is in {said}");
        }
        let ask = DirectAsk::Send { channel: "42424242".into(), text: "hello there".into(), reply: None, nonce: 7 };
        let said = format!("{ask:?}");
        assert!(!said.contains("hello") && !said.contains("42424242"), "{said}");
        assert!(said.contains("(11 chars)"));
    }

    #[test]
    fn the_sending_events_say_no_id_or_reason_in_a_debug_line() {
        let events = [
            DirectEvent::Sent { channel: "42424242".into(), nonce: 7, message: "555666777".into() },
            DirectEvent::NotSent { channel: "42424242".into(), nonce: 7 },
            DirectEvent::Refused("You are being rate limited".into()),
        ];
        for event in events {
            let said = format!("{event:?}");
            for private in ["42424242", "555666777", "rate limited"] {
                assert!(!said.contains(private), "{private} is in {said}");
            }
            assert_eq!(event.toast(), None);
        }
    }

    #[test]
    fn the_logs_counts_are_figures_only() {
        let direct =
            Direct { state: DirectState::Ready, dms: vec![dm("1", 3), dm("2", 0), dm("3", 2)], ..Direct::default() };
        assert_eq!(direct.counts(), "dms 3, unread 5 in 2");
        assert_eq!(direct.unread(), (5, 2));
        assert_eq!(DirectState::Ready.word(true), Some("Empty"));
        assert_eq!(DirectState::Ready.word(false), None);
    }
}
