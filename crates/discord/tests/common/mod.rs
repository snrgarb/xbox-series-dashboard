//! What tests/shortcuts.rs and tests/servers.rs share: the [`Following`] fed Discord's replies and events, a record
//! of what it sends, and a clock the test moves, as the Bundle's `DiscordTestsBase`.
#![allow(dead_code)]
use std::time::{Duration, SystemTime};

use discord::following::Following;
use discord::shortcuts::Known;
use discord::voice::{Commands, CDN};
use discord::{Event, User};
use model::discord::{Ask, Browse, BrowseAsk, Shortcut};
use serde_json::{json, Value};

pub const ME: &str = "100000000000000001";
pub const ANA: &str = "200000000000000002";
pub const BO: &str = "300000000000000003";
pub const LADS_ICON: &str = "https://cdn.discordapp.com/icons/9/abc.png";

/// Records what is sent, as Discord's pipe would carry it, and answers with a nonce of its own.
#[derive(Default)]
pub struct Sent {
    pub commands: Vec<Value>,
}

impl Commands for Sent {
    fn send(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Option<String> {
        let nonce = format!("n{}", self.commands.len());
        self.commands.push(json!({ "cmd": cmd, "args": args, "evt": evt, "nonce": nonce }));
        Some(nonce)
    }
}

impl Sent {
    pub fn last(&self, cmd: &str) -> Option<&Value> {
        self.commands.iter().rev().find(|c| c["cmd"] == cmd)
    }

    pub fn of(&self, cmd: &str) -> Vec<Value> {
        self.commands.iter().filter(|c| c["cmd"] == cmd).cloned().collect()
    }
}

pub struct Test {
    pub following: Following,
    pub sent: Sent,
    pub now: SystemTime,
    /// The secret store: what has been saved, by name, and how many writes.
    pub saved: Vec<(String, String)>,
    pub writes: usize,
}

impl Test {
    pub fn new() -> Self {
        Test {
            following: Following::default(),
            sent: Sent::default(),
            now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
            saved: Vec::new(),
            writes: 0,
        }
    }

    /// Signed in, Discord saying the user is in no Call.
    pub fn not_in_call() -> Self {
        let mut test = Test::new();
        test.sign_in();
        test
    }

    pub fn sign_in(&mut self) {
        self.event(Event::SignedIn(User { id: ME.into(), name: "gabe".into() }));
        self.reply("GET_SELECTED_VOICE_CHANNEL", Value::Null);
        self.reply("GET_VOICE_SETTINGS", json!({ "mute": false, "deaf": false }));
    }

    /// The Dashboard started again: the Known channels read back from what was saved.
    pub fn new_core(&self) -> Test {
        let read = |name: &str| self.saved.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone());
        let known = Known::load(
            read(discord::shortcuts::SHORTCUTS_SECRET).as_deref(),
            read(discord::shortcuts::RECENT_SERVERS_SECRET).as_deref(),
        );
        Test { following: Following::new(known), saved: self.saved.clone(), ..Test::new() }
    }

    pub fn event(&mut self, event: Event) {
        self.following.on_event(event, &mut self.sent, self.now);
        self.save();
    }

    fn save(&mut self) {
        for (name, value) in self.following.take_unsaved() {
            self.writes += 1;
            self.saved.push((name.to_string(), value));
        }
    }

    pub fn ask(&mut self, ask: Ask) {
        self.following.ask(&ask, &mut self.sent, self.now);
        self.save();
    }

    pub fn browse(&mut self, ask: BrowseAsk) {
        self.ask(Ask::Browse(ask));
    }

    pub fn advance(&mut self, by: Duration) {
        self.now += by;
        self.following.advance(self.now, &mut self.sent);
    }

    /// Discord's answer to `command`, as sent.
    pub fn reply_to(&mut self, command: &Value, data: Value) {
        let (cmd, nonce) = (command["cmd"].as_str().unwrap().to_string(), command["nonce"].as_str().unwrap().into());
        self.event(Event::Reply { cmd, nonce, data });
    }

    /// Discord's answer to the last `cmd` sent.
    pub fn reply(&mut self, cmd: &str, data: Value) {
        let command = self.sent.last(cmd).unwrap_or_else(|| panic!("no {cmd} sent")).clone();
        self.reply_to(&command, data);
    }

    pub fn refuse(&mut self, cmd: &str, message: &str) {
        let nonce = self.sent.last(cmd).unwrap()["nonce"].as_str().unwrap().to_string();
        self.event(Event::Refused { cmd: cmd.into(), nonce, message: message.into() });
    }

    pub fn dispatch(&mut self, evt: &str, data: Value) {
        self.event(Event::Dispatch { evt: evt.into(), data });
    }

    /// The user goes into `channel`, as Discord tells it: VOICE_CHANNEL_SELECT, then the channel fetched.
    pub fn join(&mut self, channel: Value) {
        let (id, guild) = (channel["id"].clone(), channel["guild_id"].clone());
        self.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": id, "guild_id": guild }));
        self.reply("GET_CHANNEL", channel);
    }

    pub fn leave(&mut self) {
        self.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": null, "guild_id": null }));
    }

    pub fn closed(&mut self) {
        self.event(Event::Closed);
    }

    /// The view, each picture named by its address on Discord's CDN.
    fn view(&self) -> (model::discord::Voice, Vec<Shortcut>, Browse) {
        self.following.view(|avatar, _| Some(avatar.url(CDN).into()))
    }

    pub fn shortcuts(&self) -> Vec<Shortcut> {
        self.view().1
    }

    pub fn shortcut_names(&self) -> Vec<String> {
        self.shortcuts().into_iter().map(|s| s.name).collect()
    }

    /// A shortcut's picture's address.
    pub fn picture(shortcut: &Shortcut) -> Option<String> {
        shortcut.picture.as_ref().map(|p| p.to_string_lossy().into_owned())
    }

    pub fn browse_view(&self) -> Browse {
        self.view().2
    }

    pub fn in_call(&self) -> bool {
        self.view().0.call.is_some()
    }

    /// The names the Flyout lists.
    pub fn listed(&self) -> Vec<String> {
        let browse = self.browse_view();
        (0..browse.len()).map(|at| browse.name(at).unwrap().to_string()).collect()
    }

    pub fn loading(&self) -> bool {
        self.browse_view().loading()
    }

    /// The avatars of who is in each voice channel listed, by address.
    pub fn occupants(&self) -> Vec<Option<Vec<String>>> {
        match self.browse_view() {
            Browse::Server { channels, .. } => channels
                .into_iter()
                .map(|c| {
                    c.occupants.map(|o| o.into_iter().map(|p| p.unwrap().to_string_lossy().into_owned()).collect())
                })
                .collect(),
            other => panic!("no server's Flyout: {other:?}"),
        }
    }
}

pub fn state(id: &str, username: &str) -> Value {
    state_with(id, username, None)
}

pub fn state_with(id: &str, username: &str, avatar: Option<&str>) -> Value {
    json!({
        "nick": null,
        "mute": false,
        "user": { "id": id, "username": username, "global_name": null, "avatar": avatar, "bot": false },
        "voice_state": { "mute": false, "deaf": false, "self_mute": false, "self_deaf": false },
    })
}

/// A server's voice channel, as GET_CHANNEL answers.
pub fn voice_channel(id: &str, name: &str, guild: &str, states: &[Value]) -> Value {
    json!({ "id": id, "name": name, "type": 2, "guild_id": guild, "voice_states": states })
}

/// A DM (type 1) or group DM (3), which has no name.
pub fn dm(id: &str, kind: u32, states: &[Value]) -> Value {
    json!({ "id": id, "name": "", "type": kind, "guild_id": null, "voice_states": states })
}

pub fn avatar_of(id: &str, hash: &str) -> String {
    format!("https://cdn.discordapp.com/avatars/{id}/{hash}.png?size=64")
}
