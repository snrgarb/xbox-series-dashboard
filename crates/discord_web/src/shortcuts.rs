//! Known channels and Shortcuts for the web session (docs/discord-web.md, "Voice"): the DMs, group DMs and server voice
//! channels the user has been in a Call in through this session, most recently joined first; the first six are the Discord
//! tab's Shortcuts. The same rules as the RPC connection's, which #299 removed (docs/discord.md, "Known
//! channels"): a DM is kept until removed, a server's voice channel only while among the six most recent, a removed
//! one is not brought back by its Call going on, the servers joined in are remembered most recent first.
//!
//! **Not the RPC crate's store** (its encrypted `discord-secrets.dat`, gone with that crate, #299). This is a small file of its
//! own, in the same shape (`{Id, GuildId, Name, Image, ServerName}`, `Image` the CDN path), at
//! `<data>/discord-web/known-profile-<id>.json`, plain text (names of channels and servers, never a message). It is
//! read when the session signs in, written when it changes, and **removed by a sign-out**; switching the session off
//! forgets it from memory only. Pure but for [`Known::load`] and [`Known::save`].
use std::path::Path;

use serde_json::{json, Value};

use crate::art::Pic;

/// How many Shortcuts the Discord tab shows.
pub const SHORTCUT_COUNT: usize = 6;

/// A channel the user has been in a Call in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownChannel {
    pub id: String,
    /// The server a voice channel is on; None for a DM or group DM.
    pub guild: Option<String>,
    /// The Call's heading when last joined; None while a DM has no real one.
    pub name: Option<String>,
    /// A DM's first other person's avatar, or a voice channel's server's icon, as a CDN path.
    pub image: Option<String>,
    pub server_name: Option<String>,
}

impl KnownChannel {
    fn to_json(&self) -> Value {
        json!({ "Id": self.id, "GuildId": self.guild, "Name": self.name, "Image": self.image, "ServerName": self.server_name })
    }

    fn from_json(value: &Value) -> Option<KnownChannel> {
        let text = |name: &str| value[name].as_str().map(str::to_string);
        Some(KnownChannel {
            id: text("Id").filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))?,
            guild: text("GuildId"),
            name: text("Name"),
            image: text("Image"),
            server_name: text("ServerName"),
        })
    }

    pub fn picture(&self) -> Option<Pic> {
        self.image.as_deref().and_then(Pic::from_path)
    }
}

/// What a Call tells of its channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub id: String,
    pub guild: Option<String>,
    pub name: Option<String>,
    pub server_name: Option<String>,
    /// A voice channel's server's icon, or a DM's first other person's avatar.
    pub image: Option<Pic>,
}

#[derive(Debug, Default)]
pub struct Known {
    channels: Vec<KnownChannel>,
    servers: Vec<String>,
    unsaved: bool,
}

impl Known {
    /// Those saved in `file`; a missing or unreadable file is none.
    pub fn load(file: &Path) -> Known {
        let Ok(text) = std::fs::read_to_string(file) else { return Known::default() };
        let Ok(value) = serde_json::from_str::<Value>(&text) else { return Known::default() };
        let channels = match &value["Shortcuts"] {
            Value::Array(list) => list.iter().filter_map(KnownChannel::from_json).collect(),
            _ => Vec::new(),
        };
        let servers = match &value["RecentServers"] {
            Value::Array(list) => list.iter().filter_map(|id| id.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        };
        Known { channels, servers, unsaved: false }
    }

    /// Writes what changed, whole to `.new` and renamed. Whether anything was written.
    pub fn save(&mut self, file: &Path) -> bool {
        if !std::mem::take(&mut self.unsaved) {
            return false;
        }
        let list: Vec<Value> = self.channels.iter().map(KnownChannel::to_json).collect();
        let text = json!({ "Shortcuts": list, "RecentServers": self.servers }).to_string();
        let new = file.with_extension("json.new");
        let wrote = file.parent().is_none_or(|dir| std::fs::create_dir_all(dir).is_ok())
            && std::fs::write(&new, text).is_ok()
            && std::fs::rename(&new, file).is_ok();
        wrote
    }

    pub fn channels(&self) -> &[KnownChannel] {
        &self.channels
    }

    /// The Shortcuts: the first six.
    pub fn shortcuts(&self) -> &[KnownChannel] {
        &self.channels[..self.channels.len().min(SHORTCUT_COUNT)]
    }

    /// Where the server is among those joined in, most recently first.
    pub fn server_rank(&self, id: &str) -> Option<usize> {
        self.servers.iter().position(|known| known == id)
    }

    /// A Call begun in a channel: it moves to the front, taking the name and picture now known, else keeping those
    /// learned before; voice channels past the sixth are forgotten; its server moves to the front of the servers.
    pub fn learn(&mut self, joined: &Joined) {
        let at = self.channels.iter().position(|c| c.id == joined.id);
        let mut known = match at {
            Some(at) => self.channels.remove(at),
            None => KnownChannel { id: joined.id.clone(), guild: None, name: None, image: None, server_name: None },
        };
        known.guild = joined.guild.clone();
        known.name = joined.name.clone().or(known.name);
        known.server_name = joined.server_name.clone().or(known.server_name);
        known.image = joined.image.as_ref().map(Pic::path).or(known.image);
        self.channels.insert(0, known);
        let mut at = self.channels.len();
        while at > SHORTCUT_COUNT {
            at -= 1;
            if self.channels[at].guild.is_some() {
                self.channels.remove(at);
            }
        }
        if let Some(guild) = &joined.guild {
            self.servers.retain(|id| id != guild);
            self.servers.insert(0, guild.clone());
        }
        self.unsaved = true;
    }

    /// The Call went on (people came and went): its Known channel takes the name and picture it lacked, and does not
    /// move. One removed during the Call is not brought back.
    pub fn remember_call(&mut self, call: &Joined) {
        let Some(known) = self.channels.iter_mut().find(|c| c.id == call.id) else { return };
        let name = known.name.clone().or_else(|| call.name.clone());
        let image = known.image.clone().or_else(|| call.image.as_ref().map(Pic::path));
        let server_name = known.server_name.clone().or_else(|| call.server_name.clone());
        if (&name, &image, &server_name) != (&known.name, &known.image, &known.server_name) {
            (known.name, known.image, known.server_name) = (name, image, server_name);
            self.unsaved = true;
        }
    }

    /// Remove shortcut. Whether it was known. The servers' order is left alone.
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.channels.len();
        self.channels.retain(|c| c.id != id);
        let forgotten = self.channels.len() != before;
        self.unsaved |= forgotten;
        forgotten
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, guild: &str) -> Joined {
        Joined {
            id: id.into(),
            guild: Some(guild.into()),
            name: Some(format!("room {id}")),
            server_name: Some("Place".into()),
            image: Pic::icon(guild, "h"),
        }
    }

    fn dm(id: &str, name: Option<&str>) -> Joined {
        Joined {
            id: id.into(),
            guild: None,
            name: name.map(str::to_string),
            server_name: None,
            image: Pic::avatar("5", "a"),
        }
    }

    #[test]
    fn the_most_recent_is_first_and_a_server_follows_its_channel() {
        let mut k = Known::default();
        k.learn(&voice("1", "10"));
        k.learn(&dm("2", Some("Ana")));
        k.learn(&voice("1", "10"));
        assert_eq!(k.channels().iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(k.server_rank("10"), Some(0));
        assert_eq!(k.channels()[0].image.as_deref(), Some("icons/10/h.png"));
        assert_eq!(k.channels()[1].image.as_deref(), Some("avatars/5/a.png"));
        assert_eq!(k.channels()[0].server_name.as_deref(), Some("Place"));
    }

    #[test]
    fn only_six_voice_channels_are_kept_but_a_dm_stays_until_removed() {
        let mut k = Known::default();
        k.learn(&dm("99", None));
        for i in 1..=8 {
            k.learn(&voice(&i.to_string(), "10"));
        }
        assert_eq!(k.channels().len(), 7, "six voice channels and the DM");
        assert!(k.channels().iter().any(|c| c.id == "99"));
        assert_eq!(k.shortcuts().len(), 6);
        assert_eq!(k.shortcuts()[0].id, "8");
        assert!(k.forget("99") && !k.forget("99"));
    }

    #[test]
    fn a_call_going_on_names_a_dm_without_moving_it_and_does_not_bring_back_a_removed_one() {
        let mut k = Known::default();
        k.learn(&dm("2", None));
        k.learn(&voice("3", "10"));
        k.remember_call(&dm("2", Some("Ana")));
        assert_eq!(k.channels()[1].name.as_deref(), Some("Ana"));
        assert_eq!(k.channels()[0].id, "3", "it did not move");
        k.remember_call(&dm("2", Some("Other")));
        assert_eq!(k.channels()[1].name.as_deref(), Some("Ana"), "already named: kept");
        k.forget("2");
        k.remember_call(&dm("2", Some("Ana")));
        assert!(k.channels().iter().all(|c| c.id != "2"));
    }

    #[test]
    fn what_is_saved_is_read_back_in_the_rpc_shape_and_only_when_it_changed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("discord-web").join("known-profile-7.json");
        let mut k = Known::default();
        assert!(!k.save(&file), "nothing changed: nothing written");
        k.learn(&voice("1", "10"));
        k.learn(&dm("2", Some("Ana")));
        assert!(k.save(&file) && !k.save(&file));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"Id\"") && text.contains("\"GuildId\"") && text.contains("\"ServerName\""));
        let back = Known::load(&file);
        assert_eq!(back.channels(), k.channels());
        assert_eq!(back.server_rank("10"), Some(0));
        assert!(Known::load(&dir.path().join("none")).channels().is_empty());
        std::fs::write(&file, "not json").unwrap();
        assert!(Known::load(&file).channels().is_empty());
    }
}
