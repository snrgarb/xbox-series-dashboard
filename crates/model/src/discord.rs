//! Discord as the screens see it: the user's own application and the connection to the Discord running on this PC
//! (docs/discord.md). The `discord` crate keeps it; Settings > Account > Discord shows it and asks for
//! changes with [`Ask`]; the Guide's Discord tab names why it can't show Discord with [`Discord::tab_line`], and shows
//! the [`Call`] the user is in ([`Voice`]), asking Mute, Deafen and Leave of Discord with [`CallAsk`], the
//! [`Shortcut`]s to the channels joined before, and the user's servers and their voice channels ([`Browse`]).
//! Nothing secret is ever in it: the Client Secret only as whether one is saved, the tokens not at all.
use std::path::PathBuf;

/// Discord's developer portal, where the user makes the application: the page names it and shows it as a QR code.
pub const PORTAL_URL: &str = "https://discord.com/developers/applications";

/// The Connect checklist, in order.
pub const STEPS: [&str; 4] = [
    "Create an application at discord.com/developers/applications.",
    "Under OAuth2, add the redirect http://127.0.0.1.",
    "Under App Testers, add yourself.",
    "Paste its Client ID and Client Secret here, then select Connect.",
];

/// The status line's words, and the Discord tab's, as the Bundle has them.
pub const NOT_CONNECTED: &str = "Not connected";
pub const CONNECTING: &str = "Connecting...";
pub const NOT_RUNNING: &str = "Discord isn't running";
pub const APPROVE: &str = "Approve the request in Discord";
pub const RECONNECT_IN_SETTINGS: &str = "Reconnect Discord in settings";
/// The Discord tab's line while a connect is under way; Settings' status line says [`CONNECTING`].
pub const CONNECTING_TO_DISCORD: &str = "Connecting to Discord";
/// The Discord tab's line under who is signed in, out of a Call.
pub const NOT_IN_A_CALL: &str = "You're not in a voice channel";
/// A DM's heading while nobody else has been seen in it.
pub const CALL: &str = "Call";
/// The Discord tab's last entry, and the Flyout it opens.
pub const SERVERS: &str = "Servers";
/// What the Servers Flyout says once Discord has answered with none, and a server's with no voice channel.
pub const NO_SERVERS: &str = "No servers";
pub const NO_VOICE_CHANNELS: &str = "No voice channels";
/// A voice channel nobody is in.
pub const EMPTY: &str = "Empty";
/// A Shortcut's one menu option.
pub const REMOVE_SHORTCUT: &str = "Remove shortcut";

/// What Settings' Discord page shows, and whether the Guide has a Discord tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discord {
    /// The application's Client ID, or None for none yet. It is public: the page shows it.
    pub client_id: Option<String>,
    /// Whether a Client Secret is saved. The Secret itself is never here.
    pub has_secret: bool,
    /// Whether tokens are saved, from a Connect until Disconnect.
    pub has_tokens: bool,
    /// The status line: "Not connected", "Connected as X", or what went wrong.
    pub status: String,
    /// Discord has signed the user in.
    pub connected: bool,
    /// A connect is under way: the handshake, a refresh, the Authorize prompt, the token exchange or signing in.
    pub connecting: bool,
    /// Discord stopped taking the saved tokens and would not authorize again either: the user has to Connect once
    /// more. Connect signing in, or Disconnect, clears it.
    pub needs_reconnect: bool,
    /// Discord is out of reach and is tried again later by itself (a try is waiting to come).
    pub retrying: bool,
    /// Who is signed in, by display name, else username, while connected.
    pub user: Option<String>,
    /// The Call the user is in and their own mute and deafen, as Discord last said: what the Discord tab shows.
    pub voice: Voice,
    /// The six most recently joined Known channels, most recently first: the Discord tab's Shortcuts. Kept while
    /// Discord is away, and across Disconnect.
    pub shortcuts: Vec<Shortcut>,
    /// The Servers Flyout, or a server's, as Discord last answered it.
    pub browse: Browse,
}

/// A tile on the Discord tab for a Known channel: A rejoins it, Remove shortcut forgets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    /// The channel's id, which joining it names.
    pub id: String,
    /// The Call's heading when last joined: the channel's name, a DM's people, else "Call".
    pub name: String,
    /// A voice channel's server's name, once the server has said it; None for a DM.
    pub server: Option<String>,
    /// A DM's first other person's avatar, or a voice channel's server's icon, once fetched into the cache.
    pub picture: Option<PathBuf>,
    /// A DM or group DM: its picture is a disc, and Discord's mark stands in for none.
    pub dm: bool,
}

/// One of the user's Discord servers, in the Servers Flyout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub id: String,
    pub name: String,
    /// Its icon, once fetched into the cache; None for a server without one, or until it has come.
    pub icon: Option<PathBuf>,
}

/// One of a server's voice (or stage) channels, in its Flyout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChannel {
    pub id: String,
    pub name: String,
    /// The avatars of the people in it, each once fetched; empty for nobody (Empty); None until Discord has said.
    pub occupants: Option<Vec<Option<PathBuf>>>,
}

/// The Flyout the Discord tab's Servers opens, as Discord last answered: the servers, then one server's voice
/// channels. Loading while Discord is asked and its spinner has not yet shown a second.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Browse {
    #[default]
    Closed,
    Servers {
        loading: bool,
        servers: Vec<Server>,
    },
    Server {
        id: String,
        name: String,
        loading: bool,
        channels: Vec<VoiceChannel>,
    },
}

impl Browse {
    pub fn is_open(&self) -> bool {
        *self != Browse::Closed
    }

    pub fn loading(&self) -> bool {
        matches!(self, Browse::Servers { loading: true, .. } | Browse::Server { loading: true, .. })
    }

    /// The Flyout's heading: "Servers", or the server's name.
    pub fn title(&self) -> &str {
        match self {
            Browse::Closed => "",
            Browse::Servers { .. } => SERVERS,
            Browse::Server { name, .. } => name,
        }
    }

    /// How many entries it lists.
    pub fn len(&self) -> usize {
        match self {
            Browse::Closed => 0,
            Browse::Servers { servers, .. } => servers.len(),
            Browse::Server { channels, .. } => channels.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The id of the entry at `at`: a server's, or a channel's.
    pub fn id(&self, at: usize) -> Option<&str> {
        match self {
            Browse::Closed => None,
            Browse::Servers { servers, .. } => servers.get(at).map(|s| s.id.as_str()),
            Browse::Server { channels, .. } => channels.get(at).map(|c| c.id.as_str()),
        }
    }

    /// The name of the entry at `at`.
    pub fn name(&self, at: usize) -> Option<&str> {
        match self {
            Browse::Closed => None,
            Browse::Servers { servers, .. } => servers.get(at).map(|s| s.name.as_str()),
            Browse::Server { channels, .. } => channels.get(at).map(|c| c.name.as_str()),
        }
    }
}

/// What the Discord tab's Servers asks of Discord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseAsk {
    /// A on Servers: the servers, those answered before at once, asked afresh.
    Servers,
    /// A on a server: its voice channels, likewise, and who is in each.
    Server { id: String, name: String },
    /// B in a server's Flyout: back to the servers answered before, not asked again.
    Back,
    /// B in the Servers Flyout: closed.
    Close,
}

/// The Status area's Discord mark, in a Call: lit while the user speaks, badged muted or deafened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallMark {
    pub speaking: bool,
    pub muted: bool,
    /// Deafened, which wins over muted on the badge.
    pub deafened: bool,
}

impl Voice {
    /// The Status area's mark, in a Call; None out of one.
    pub fn mark(&self) -> Option<CallMark> {
        let call = self.call.as_ref()?;
        let speaking = call.people.iter().any(|p| p.me && p.speaking);
        Some(CallMark { speaking, muted: self.mute, deafened: self.deaf })
    }
}

/// The user's voice on Discord, as Discord last said (docs/discord.md, "The Call").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Voice {
    /// The Call the user is in, once its channel has arrived; None out of one.
    pub call: Option<Call>,
    /// The user's own Discord mute and deafen, as Discord's voice settings last said, in a Call or out of one.
    pub mute: bool,
    pub deaf: bool,
}

/// The voice channel, DM or group DM the user is in (the Bundle's Call): its heading and the people in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The channel's id.
    pub id: String,
    /// The channel's name; a DM's, which has none, the other people in it, comma-joined, else those last seen in it,
    /// else "Call".
    pub title: String,
    /// The people in it, the user first, then the others in Discord's order.
    pub people: Vec<Person>,
}

/// Someone in a Call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// Their Discord user id, which the Call's events name them by.
    pub id: String,
    /// Their nickname on the server, else their display name, else their username.
    pub name: String,
    /// Their avatar, once fetched into the cache (a plain disc stands in until then).
    pub avatar: Option<PathBuf>,
    /// Speaking, as Discord last said.
    pub speaking: bool,
    /// Muted or deafened, by themselves or the server.
    pub muted: bool,
    pub deafened: bool,
    /// Whether this is the user signed in.
    pub me: bool,
}

/// What the Discord tab asks of Discord in a Call. Each is shown done only once Discord says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallAsk {
    /// The user's Discord mute turned over, from what Discord last said.
    Mute,
    /// The user's Discord deafen turned over, likewise.
    Deafen,
    /// Leave the Call.
    Leave,
}

impl Default for Discord {
    fn default() -> Self {
        Self {
            client_id: None,
            has_secret: false,
            has_tokens: false,
            status: NOT_CONNECTED.into(),
            connected: false,
            connecting: false,
            needs_reconnect: false,
            retrying: false,
            user: None,
            voice: Voice::default(),
            shortcuts: Vec::new(),
            browse: Browse::Closed,
        }
    }
}

impl Discord {
    /// Whether Connect does anything: with a Client ID and Secret, and not connected or connecting already.
    pub fn can_connect(&self) -> bool {
        self.client_id.is_some() && self.has_secret && !self.connected && !self.connecting
    }

    /// Whether Disconnect does anything: while connected or connecting, or with tokens saved from before.
    pub fn can_disconnect(&self) -> bool {
        self.connected || self.connecting || self.has_tokens
    }

    /// Whether the Guide has a Discord tab: signed in, or with tokens saved, from the first Connect until
    /// Disconnect.
    pub fn has_tab(&self) -> bool {
        self.connected || self.has_tokens
    }

    /// The page's state for the focus log, naming neither the Client ID nor the Secret: `client id set, secret
    /// saved, "Connected as E2E Tester"` (`not set` for either; the status line in quotes).
    pub fn state_line(&self) -> String {
        let id = if self.client_id.is_some() { "set" } else { "not set" };
        let secret = if self.has_secret { "saved" } else { "not set" };
        format!("client id {id}, secret {secret}, {:?}", self.status)
    }

    /// What the Discord tab says in Discord's place while it can't show it, or None while signed in: connecting,
    /// Discord isn't running while it is tried again, else the user has to reconnect in Settings.
    pub fn tab_line(&self) -> Option<&'static str> {
        if self.connected {
            None
        } else if self.needs_reconnect {
            Some(RECONNECT_IN_SETTINGS)
        } else if self.connecting {
            Some(CONNECTING_TO_DISCORD)
        } else if self.retrying {
            Some(NOT_RUNNING)
        } else {
            Some(RECONNECT_IN_SETTINGS)
        }
    }
}

/// Which of the application's two fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    ClientId,
    ClientSecret,
}

/// What Settings' Discord page asks of the connection.
#[derive(Clone, PartialEq, Eq)]
pub enum Ask {
    /// The Client ID typed: saved at once, trimmed; empty forgets it.
    ClientId(String),
    /// The Client Secret typed: saved at once, trimmed. Never shown, never in a log line or `Debug`.
    ClientSecret(String),
    /// Paste: the clipboard's text into the field, as typed. A clipboard with no text pastes nothing.
    Paste(Field),
    Connect,
    Disconnect,
    /// The Discord tab's Mute, Deafen or Leave.
    Call(CallAsk),
    /// A on a Shortcut or a server's voice channel: join it, by its id (nothing if it is the Call's already).
    Join(String),
    /// Remove shortcut: the Known channel, by its id, forgotten.
    RemoveShortcut(String),
    /// The Servers Flyouts.
    Browse(BrowseAsk),
}

impl std::fmt::Debug for Ask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ask::ClientId(_) => f.write_str("ClientId(..)"),
            Ask::ClientSecret(_) => f.write_str("ClientSecret(..)"),
            Ask::Paste(field) => write!(f, "Paste({field:?})"),
            Ask::Connect => f.write_str("Connect"),
            Ask::Disconnect => f.write_str("Disconnect"),
            Ask::Call(ask) => write!(f, "Call({ask:?})"),
            Ask::Join(id) => write!(f, "Join({id:?})"),
            Ask::RemoveShortcut(id) => write!(f, "RemoveShortcut({id:?})"),
            Ask::Browse(ask) => write!(f, "Browse({ask:?})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_needs_both_fields_and_nothing_under_way() {
        let mut discord = Discord { client_id: Some("1".into()), ..Discord::default() };
        assert!(!discord.can_connect());
        discord.has_secret = true;
        assert!(discord.can_connect());
        discord.connecting = true;
        assert!(!discord.can_connect());
        assert!(discord.can_disconnect());
    }

    #[test]
    fn the_tab_says_why_it_cant_show_discord() {
        let saved = Discord { has_tokens: true, ..Discord::default() };
        assert!(saved.has_tab());
        assert_eq!(saved.tab_line(), Some(RECONNECT_IN_SETTINGS));
        assert_eq!(Discord { retrying: true, ..saved.clone() }.tab_line(), Some(NOT_RUNNING));
        assert_eq!(Discord { connecting: true, ..saved.clone() }.tab_line(), Some(CONNECTING_TO_DISCORD));
        assert_eq!(Discord { connected: true, ..saved }.tab_line(), None);
        assert!(!Discord::default().has_tab());
    }

    #[test]
    fn a_secret_is_never_in_debug() {
        let said = format!("{:?}", Ask::ClientSecret("s3cr3t".into()));
        assert!(!said.contains("s3cr3t"));
    }
}
