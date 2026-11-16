//! Known channels and Shortcuts (docs/discord.md, "Known channels"): the DMs, group DMs and server voice channels
//! the user has been in a Call in, on this PC or another device, learned from the Calls Discord reports, most
//! recently joined first, kept in the encrypted secret store under the Bundle's names and in its form. Known DMs are
//! kept until removed; a server's voice channel only while among the six most recent. The first six are the Discord
//! tab's Shortcuts. The servers the user has joined a Call in are kept too, most recently first, every one, however
//! far its channels have fallen: the Servers Flyout lists them first.
//!
//! Ported from the Bundle's `ExtensionCore.DiscordShortcuts.cs` and `RememberServer` in
//! `ExtensionCore.DiscordServers.cs`; their tests are `DiscordShortcutsTests` and `DiscordServersTests` (here
//! `tests/shortcuts.rs` and `tests/servers.rs`). Pure: what is to be saved is handed out ([`Known::take_unsaved`]).
use serde_json::{json, Value};

use crate::voice::{Avatar, Joined, CDN};

/// The secret store's names, the Bundle's: the Known channels, and the servers joined in, each as JSON.
pub const SHORTCUTS_SECRET: &str = "Discord.Shortcuts";
pub const RECENT_SERVERS_SECRET: &str = "Discord.RecentServers";

/// How many Shortcuts the Discord tab shows, and how many of the servers' voice channels are kept.
pub const SHORTCUT_COUNT: usize = 6;

/// A DM, group DM or voice channel the user has joined, as saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownChannel {
    /// The channel's id, which SELECT_VOICE_CHANNEL takes.
    pub id: String,
    /// The server a voice channel is on; None for a DM or group DM.
    pub guild: Option<String>,
    /// The Call's heading when last joined: the channel's name, or a DM's from the people in it; None while a DM
    /// has no real one.
    pub name: Option<String>,
    /// A DM's first other person's avatar, or a voice channel's server's icon, as an address on Discord's CDN;
    /// None for none yet.
    pub image: Option<String>,
    /// A voice channel's server's name; None for a DM, or until the server has said.
    pub server_name: Option<String>,
}

impl KnownChannel {
    /// As the Bundle saves it (Newtonsoft's names): `{"Id":..,"GuildId":..,"Name":..,"Image":..,"ServerName":..}`.
    fn to_json(&self) -> Value {
        json!({
            "Id": self.id,
            "GuildId": self.guild,
            "Name": self.name,
            "Image": self.image,
            "ServerName": self.server_name,
        })
    }

    fn from_json(value: &Value) -> Option<KnownChannel> {
        let text = |name: &str| value[name].as_str().map(str::to_string);
        Some(KnownChannel {
            id: text("Id")?,
            guild: text("GuildId"),
            name: text("Name"),
            image: text("Image"),
            server_name: text("ServerName"),
        })
    }

    /// The picture the Shortcut wears: the address it keeps, as a picture of Discord's CDN.
    pub fn picture(&self) -> Option<Avatar> {
        self.image.as_deref().and_then(Avatar::from_url)
    }
}

/// The Known channels and the servers joined in.
#[derive(Debug, Default)]
pub struct Known {
    /// Most recently joined first.
    channels: Vec<KnownChannel>,
    /// The ids of the servers the user has joined a Call in, most recently first, every one kept.
    servers: Vec<String>,
    /// What changed since last saved.
    channels_unsaved: bool,
    servers_unsaved: bool,
}

impl Known {
    /// Those saved before, as the secret store has them (None for nothing saved). What can't be read is none.
    pub fn load(channels: Option<&str>, servers: Option<&str>) -> Known {
        let parse = |json: Option<&str>| json.and_then(|json| serde_json::from_str::<Value>(json).ok());
        let channels = match parse(channels) {
            Some(Value::Array(list)) => list.iter().filter_map(KnownChannel::from_json).collect(),
            _ => Vec::new(),
        };
        let servers = match parse(servers) {
            Some(Value::Array(list)) => list.iter().filter_map(|id| id.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        };
        Known { channels, servers, ..Known::default() }
    }

    /// Every Known channel, most recently joined first.
    pub fn channels(&self) -> &[KnownChannel] {
        &self.channels
    }

    /// The Shortcuts: the first six.
    pub fn shortcuts(&self) -> &[KnownChannel] {
        &self.channels[..self.channels.len().min(SHORTCUT_COUNT)]
    }

    /// Where the server is among those joined in, most recently first; None for one never joined in.
    pub fn server_rank(&self, id: &str) -> Option<usize> {
        self.servers.iter().position(|known| known == id)
    }

    /// A Call shown for a channel it wasn't showing before (the Bundle's `LearnChannel`): the channel moves to the
    /// front, taking the name and a DM's picture now known, else keeping those learned before; voice channels past
    /// the sixth are forgotten. A voice channel's server moves to the front of the servers: it is given back, to be
    /// asked for its name and icon (GET_GUILD).
    pub fn learn(&mut self, joined: &Joined) -> Option<String> {
        let at = self.channels.iter().position(|c| c.id == joined.id);
        let mut known = match at {
            Some(at) => self.channels.remove(at),
            None => KnownChannel { id: joined.id.clone(), guild: None, name: None, image: None, server_name: None },
        };
        known.guild = joined.guild.clone();
        known.name = joined.name.clone().or(known.name);
        if joined.guild.is_none() {
            known.image = joined.other_avatar.as_ref().map(|avatar| avatar.url(CDN)).or(known.image);
        }
        self.channels.insert(0, known);
        let mut at = self.channels.len();
        while at > SHORTCUT_COUNT {
            at -= 1;
            if self.channels[at].guild.is_some() {
                self.channels.remove(at);
            }
        }
        self.channels_unsaved = true;
        let guild = joined.guild.clone()?;
        self.servers.retain(|id| *id != guild);
        self.servers.insert(0, guild.clone());
        self.servers_unsaved = true;
        Some(guild)
    }

    /// The Call changed (the Bundle's `RememberCall`): its Known channel, if it still has one and lacks them, takes
    /// the name now known, and a DM the avatar of the first other person in it, as when someone answers a DM rung;
    /// one already named keeps its name and picture as people come and go. It doesn't move, and one removed during
    /// the Call isn't brought back.
    pub fn remember_call(&mut self, call: Option<&Joined>) {
        let Some(call) = call else { return };
        let Some(known) = self.channels.iter_mut().find(|c| c.id == call.id) else { return };
        let name = known.name.clone().or_else(|| call.name.clone());
        let image = known.image.clone().or_else(|| match call.guild {
            None => call.other_avatar.as_ref().map(|avatar| avatar.url(CDN)),
            Some(_) => None,
        });
        if name != known.name || image != known.image {
            (known.name, known.image) = (name, image);
            self.channels_unsaved = true;
        }
    }

    /// GET_GUILD answered: the server's voice channels take its icon and name, saved only if they changed. Whether
    /// any did.
    pub fn on_guild(&mut self, guild: &Value) -> bool {
        let Some(id) = guild["id"].as_str() else { return false };
        let image = guild["icon_url"].as_str().map(str::to_string);
        let server_name = guild["name"].as_str().map(str::to_string);
        let mut changed = false;
        for known in self.channels.iter_mut().filter(|c| c.guild.as_deref() == Some(id)) {
            if known.image != image || known.server_name != server_name {
                (known.image, known.server_name) = (image.clone(), server_name.clone());
                changed = true;
            }
        }
        self.channels_unsaved |= changed;
        changed
    }

    /// Remove shortcut: the channel forgotten, from the saved list too. Whether it was known. The servers' order is
    /// left alone.
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.channels.len();
        self.channels.retain(|c| c.id != id);
        let forgotten = self.channels.len() != before;
        self.channels_unsaved |= forgotten;
        forgotten
    }

    /// What has changed since last asked, to be written to the secret store: each name with its JSON.
    pub fn take_unsaved(&mut self) -> Vec<(&'static str, String)> {
        let mut unsaved = Vec::new();
        if std::mem::take(&mut self.channels_unsaved) {
            let list: Vec<Value> = self.channels.iter().map(KnownChannel::to_json).collect();
            unsaved.push((SHORTCUTS_SECRET, Value::Array(list).to_string()));
        }
        if std::mem::take(&mut self.servers_unsaved) {
            unsaved.push((RECENT_SERVERS_SECRET, json!(self.servers).to_string()));
        }
        unsaved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn they_are_saved_as_the_bundle_saves_them_and_read_back() {
        let mut known = Known::default();
        let joined =
            Joined { id: "500".into(), guild: Some("9".into()), name: Some("Late night".into()), other_avatar: None };
        assert_eq!(known.learn(&joined), Some("9".into()));
        known.on_guild(
            &json!({ "id": "9", "name": "The Lads", "icon_url": "https://cdn.discordapp.com/icons/9/abc.png" }),
        );
        let saved = known.take_unsaved();
        assert_eq!(
            saved,
            [
                (
                    SHORTCUTS_SECRET,
                    r#"[{"GuildId":"9","Id":"500","Image":"https://cdn.discordapp.com/icons/9/abc.png","Name":"Late night","ServerName":"The Lads"}]"#
                        .to_string()
                ),
                (RECENT_SERVERS_SECRET, r#"["9"]"#.to_string()),
            ]
        );
        assert!(known.take_unsaved().is_empty(), "nothing more to save");
        let again = Known::load(Some(&saved[0].1), Some(&saved[1].1));
        assert_eq!(again.channels(), known.channels());
        assert_eq!(again.server_rank("9"), Some(0));
    }

    #[test]
    fn what_cant_be_read_is_none() {
        let known = Known::load(Some("not json"), Some(r#"{"a":1}"#));
        assert!(known.channels().is_empty() && known.server_rank("9").is_none());
        let known = Known::load(Some(r#"[{"Name":"no id"},{"Id":"1"}]"#), None);
        assert_eq!(known.channels().len(), 1);
    }
}
