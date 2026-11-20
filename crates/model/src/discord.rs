//! Discord's voice as the screens see it, from the Dashboard's own web session (docs/discord-web.md, "Voice"): the
//! Guide's Discord tab shows the [`Call`] the user is in ([`Voice`]), asking Mute, Deafen and Leave of Discord with
//! [`CallAsk`], the [`Shortcut`]s to the channels joined before, and the user's servers and their voice channels
//! ([`Browse`]). The session itself, and Direct Messages, are `discord_web` and `discord_dms`.
use std::path::PathBuf;

/// What the Discord tab says while the web session's voice is not ready.
pub const NOT_CONNECTED: &str = "Discord isn't connected";
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

/// What the Discord tab shows: whether the web session's voice is ready, and what it says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discord {
    /// The web session's voice is ready: its stores are found and the Discord tab can show a Call.
    pub connected: bool,
    /// Who is signed in, by display name, else username, while connected.
    pub user: Option<String>,
    /// The Call the user is in and their own mute and deafen, as Discord last said: what the Discord tab shows.
    pub voice: Voice,
    /// The six most recently joined Known channels, most recently first: the Discord tab's Shortcuts.
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

impl Discord {
    /// Whether the Guide has a Discord tab for the voice: while the session's voice is ready.
    pub fn has_tab(&self) -> bool {
        self.connected
    }

    /// What the Discord tab says in the voice's place while it can't show it, or None while it can.
    pub fn tab_line(&self) -> Option<&'static str> {
        if self.connected {
            None
        } else {
            Some(NOT_CONNECTED)
        }
    }
}

/// What the Discord tab asks of the web session's voice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// The Discord tab's Mute, Deafen or Leave.
    Call(CallAsk),
    /// A on a Shortcut or a server's voice channel: join it, by its id (nothing if it is the Call's already).
    Join(String),
    /// Remove shortcut: the Known channel, by its id, forgotten.
    RemoveShortcut(String),
    /// The Servers Flyouts.
    Browse(BrowseAsk),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tab_says_why_it_cant_show_discord() {
        assert!(!Discord::default().has_tab());
        assert_eq!(Discord::default().tab_line(), Some(NOT_CONNECTED));
        let ready = Discord { connected: true, ..Discord::default() };
        assert!(ready.has_tab());
        assert_eq!(ready.tab_line(), None);
    }
}
