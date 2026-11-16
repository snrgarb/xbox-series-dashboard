//! The Call (docs/discord.md, "The Call"): which voice channel or DM the user is in and who is there, followed from
//! Discord's replies and events once signed in, for the Discord tab; and Mute, Deafen and Leave, asked of Discord,
//! which shows them done only once its events say so. Ported from the Bundle's `DiscordCall.cs` and
//! `ExtensionCore.DiscordCall.cs`; its tests are `DiscordCallTests` (here `tests/voice.rs`).
//!
//! Pure: it is told what the [`Client`] gave ([`Follower::on_event`]) and sends through [`Commands`]. Known channels,
//! Known channels and Shortcuts ([`crate::shortcuts`]) learn from it ([`Follower::take_joined`]); Servers
//! ([`crate::servers`]) shares its GET_CHANNEL replies ([`crate::following`]). Incoming calls (rings) are not built:
//! the places they plug in are marked "Rings".
use std::collections::HashSet;
use std::path::PathBuf;

use model::discord::{Call, CallAsk, Person, Voice, CALL};
use serde_json::{json, Value};

use crate::client::{Client, Event, Secrets, Tokens, Transport};

/// The events of the Call's channel that are followed, one SUBSCRIBE each with its `channel_id`.
pub const CALL_EVENTS: [&str; 5] =
    ["VOICE_STATE_CREATE", "VOICE_STATE_UPDATE", "VOICE_STATE_DELETE", "SPEAKING_START", "SPEAKING_STOP"];

/// Discord's CDN, where avatars are.
pub const CDN: &str = "https://cdn.discordapp.com";

/// Whoever sends commands to Discord: the [`Client`], or a test's.
pub trait Commands {
    /// Sends `cmd` while signed in; its nonce, None while not.
    fn send(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Option<String>;
}

impl<T: Transport, O: Tokens, S: Secrets> Commands for Client<T, O, S> {
    fn send(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Option<String> {
        Client::send(self, cmd, args, evt)
    }
}

/// A picture on Discord's CDN: a person's avatar, their own or one of Discord's six default ones for a user with
/// none, or a server's icon.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Avatar {
    Own { user: String, hash: String },
    Default(u64),
    Icon { guild: String, hash: String },
}

impl Avatar {
    /// The avatar of the user `id` whose avatar hash is `hash`: their own, else the default one Discord gives them,
    /// picked by their id (the Bundle's `AvatarUrl`).
    pub fn of(id: &str, hash: Option<&str>) -> Avatar {
        match hash.filter(|hash| !hash.is_empty()) {
            Some(hash) => Avatar::Own { user: id.to_string(), hash: hash.to_string() },
            None => Avatar::Default((id.parse::<u64>().unwrap_or(0) >> 22) % 6),
        }
    }

    /// Its address on the CDN at `cdn` (Discord's, [`CDN`], or a test's): an avatar 64 across, as the Bundle asks.
    pub fn url(&self, cdn: &str) -> String {
        let cdn = cdn.trim_end_matches('/');
        match self {
            Avatar::Own { user, hash } => format!("{cdn}/avatars/{user}/{hash}.png?size=64"),
            Avatar::Default(n) => format!("{cdn}/embed/avatars/{n}.png"),
            Avatar::Icon { guild, hash } => format!("{cdn}/icons/{guild}/{hash}.png"),
        }
    }

    /// The picture at an address on Discord's CDN, as Discord gives a server's `icon_url` and as a Known channel
    /// keeps its picture: `.../avatars/<user>/<hash>.png`, `.../embed/avatars/<n>.png`, `.../icons/<guild>/<hash>.png`
    /// (any size asked, any picture type). It is fetched from the CDN the Dashboard was given, never from the host
    /// named: a test's Discord can't send the Dashboard anywhere.
    pub fn from_url(url: &str) -> Option<Avatar> {
        let path = url.split_once("://").map_or(url, |(_, rest)| rest.split_once('/').map_or("", |(_, path)| path));
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let stem = |file: &str| file.rsplit_once('.').map_or(file, |(stem, _)| stem).to_string();
        let parts: Vec<&str> = path.split('/').collect();
        let named = |part: &str| !part.is_empty();
        match parts.as_slice() {
            ["avatars", user, file] if named(user) && named(file) => {
                Some(Avatar::Own { user: user.to_string(), hash: stem(file) })
            }
            ["embed", "avatars", file] => stem(file).parse().ok().map(Avatar::Default),
            ["icons", guild, file] if named(guild) && named(file) => {
                Some(Avatar::Icon { guild: guild.to_string(), hash: stem(file) })
            }
            _ => None,
        }
    }

    /// Where it is kept in the avatars' cache: `avatars/<user>/<hash>.png`, `embed/<n>.png`, `icons/<guild>/<hash>.png`.
    /// A picture's hash changes with the picture, so a kept one is never stale. Anything but letters, digits, `-`
    /// and `_` in a part is written `%XX`, so a part can never climb out of the folder.
    pub fn file(&self) -> PathBuf {
        match self {
            Avatar::Own { user, hash } => PathBuf::from("avatars").join(safe(user)).join(format!("{}.png", safe(hash))),
            Avatar::Default(n) => PathBuf::from("embed").join(format!("{n}.png")),
            Avatar::Icon { guild, hash } => PathBuf::from("icons").join(safe(guild)).join(format!("{}.png", safe(hash))),
        }
    }
}

fn safe(part: &str) -> String {
    let mut out = String::new();
    for byte in part.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// One person in a Call, as their voice state has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Participant {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) avatar: Avatar,
    muted: bool,
    deafened: bool,
}

impl Participant {
    /// The person a voice state stands for; None without a user. Named by their nickname, else their display
    /// name, else their username; muted or deafened by themselves or the server.
    pub(crate) fn from_voice_state(state: &Value) -> Option<Participant> {
        let user = &state["user"];
        let id = user["id"].as_str()?.to_string();
        let names = [&state["nick"], &user["global_name"], &user["username"]];
        let name = names.iter().filter_map(|name| name.as_str()).find(|name| !name.is_empty()).unwrap_or_default();
        let voice = &state["voice_state"];
        let set = |flag: &str| voice[flag].as_bool() == Some(true);
        Some(Participant {
            avatar: Avatar::of(&id, user["avatar"].as_str()),
            id,
            name: name.to_string(),
            muted: set("mute") || set("self_mute"),
            deafened: set("deaf") || set("self_deaf"),
        })
    }
}

/// The Call as it is followed: the channel Discord reported and the people in it as its voice states and speaking
/// events have them, in Discord's order.
#[derive(Debug, Clone)]
struct CallState {
    id: String,
    /// The server a voice channel is on; None for a DM or group DM.
    guild: Option<String>,
    /// The channel's name; None for a DM, which has none.
    name: Option<String>,
    /// The user signed in, who is told among the people by their id.
    me: String,
    people: Vec<Participant>,
    speaking: HashSet<String>,
    /// A DM's name from the people last seen in it, kept while nobody else is there.
    people_name: Option<String>,
}

impl CallState {
    /// The Call made from a channel object, as GET_CHANNEL and GET_SELECTED_VOICE_CHANNEL answer; None for none.
    fn from_channel(channel: &Value, me: &str) -> Option<CallState> {
        let id = channel["id"].as_str()?.to_string();
        let name = channel["name"].as_str().filter(|name| !name.is_empty()).map(str::to_string);
        let guild = channel["guild_id"].as_str().filter(|guild| !guild.is_empty()).map(str::to_string);
        let mut call = CallState {
            id,
            guild,
            name,
            me: me.to_string(),
            people: Vec::new(),
            speaking: HashSet::new(),
            people_name: None,
        };
        for state in channel["voice_states"].as_array().into_iter().flatten() {
            call.upsert(state);
        }
        Some(call)
    }

    /// The heading: the channel's name; a DM's, from the other people in it, else those last seen there, else
    /// "Call".
    fn title(&self) -> String {
        self.name.clone().or_else(|| self.people_name.clone()).unwrap_or_else(|| CALL.to_string())
    }

    /// A voice state come or changed: the person is updated in place, or added after the others. Whether they are
    /// new to the Call.
    fn upsert(&mut self, state: &Value) -> Option<&Participant> {
        let person = Participant::from_voice_state(state)?;
        let new = match self.people.iter().position(|p| p.id == person.id) {
            Some(at) => {
                self.people[at] = person;
                false
            }
            None => {
                self.people.push(person);
                true
            }
        };
        self.remember_people_name();
        new.then(|| self.people.last()).flatten()
    }

    /// A voice state gone: the person has left. Who left, if they were there.
    fn remove(&mut self, state: &Value) -> Option<Participant> {
        let id = state["user"]["id"].as_str()?;
        let at = self.people.iter().position(|p| p.id == id)?;
        let gone = self.people.remove(at);
        self.speaking.remove(id);
        self.remember_people_name();
        Some(gone)
    }

    fn remember_people_name(&mut self) {
        let others: Vec<&str> = self.people.iter().filter(|p| p.id != self.me).map(|p| p.name.as_str()).collect();
        if !others.is_empty() {
            self.people_name = Some(others.join(", "));
        }
    }

    fn set_speaking(&mut self, id: &str, speaking: bool) {
        if speaking {
            self.speaking.insert(id.to_string());
        } else {
            self.speaking.remove(id);
        }
    }

    fn has(&self, id: &str) -> bool {
        self.people.iter().any(|p| p.id == id)
    }

    /// The same channel fetched afresh: those still in it who were speaking still are, as Discord says so only as
    /// they start and stop.
    fn keep_speaking_from(&mut self, before: &CallState) {
        let still: Vec<String> = before.speaking.iter().filter(|id| self.has(id)).cloned().collect();
        self.speaking.extend(still);
    }

    /// The user first, then the others in Discord's order.
    fn user_first(&self) -> impl Iterator<Item = &Participant> {
        let me = self.people.iter().filter(|p| p.id == self.me);
        me.chain(self.people.iter().filter(|p| p.id != self.me))
    }

    fn names(&self) -> String {
        self.user_first().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
    }

    /// What a Known channel learns of it (the Bundle's `HasName`, `Title` and `OtherAvatar`).
    fn joined(&self) -> Joined {
        Joined {
            id: self.id.clone(),
            guild: self.guild.clone(),
            name: self.name.clone().or_else(|| self.people_name.clone()),
            other_avatar: self.people.iter().find(|p| p.id != self.me).map(|p| p.avatar.clone()),
        }
    }
}

/// The Call's channel as a Known channel learns it (docs/discord.md, "Known channels").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub id: String,
    /// The server a voice channel is on; None for a DM or group DM.
    pub guild: Option<String>,
    /// Its heading, if it has a real one: the channel's name, or a DM's from the people seen in it.
    pub name: Option<String>,
    /// The first person in it who isn't the user: their avatar.
    pub other_avatar: Option<Avatar>,
}

/// The Call and the user's voice settings, followed from Discord.
#[derive(Debug, Default)]
pub struct Follower {
    /// The signed-in user's id, which tells the user among the people in a Call.
    me: Option<String>,
    /// The channel Discord last said the user selected, fetched or being fetched; None out of a Call.
    selected: Option<String>,
    /// The Call shown, once its channel has arrived.
    call: Option<CallState>,
    /// Whether Discord has said which channel the user selected since signing in, which outdates the sign-in's
    /// GET_SELECTED_VOICE_CHANNEL answer should it come later.
    select_seen: bool,
    /// The user's own Discord mute and deafen, as Discord's voice settings last said.
    mute: bool,
    deaf: bool,
    /// The channels a Call has begun in since last asked: each becomes a Known channel ([`crate::shortcuts`]).
    joined: Vec<Joined>,
    log: Vec<String>,
}

impl Follower {
    /// What Discord said or did ([`Client::take_events`]), with whatever it calls for sent through `commands`.
    pub fn on_event(&mut self, event: Event, commands: &mut impl Commands) {
        match event {
            Event::SignedIn(user) => {
                self.forget();
                self.me = Some(user.id);
                // Signed in: which channel the user is already in, and their voice settings.
                commands.send("GET_SELECTED_VOICE_CHANNEL", json!({}), None);
                commands.send("GET_VOICE_SETTINGS", json!({}), None);
            }
            Event::Closed => self.forget(),
            Event::Reply { cmd, data, .. } => self.on_reply(&cmd, &data, commands),
            Event::Refused { cmd, message, .. } => self.on_refused(&cmd, &message, commands),
            Event::Dispatch { evt, data } => self.on_dispatch(&evt, &data, commands),
        }
    }

    /// Discord has gone: no Call, and its voice settings forgotten until it says them again.
    fn forget(&mut self) {
        if self.call.is_some() {
            self.say("out of the call: Discord has gone".into());
        }
        *self = Follower {
            log: std::mem::take(&mut self.log),
            joined: std::mem::take(&mut self.joined),
            ..Follower::default()
        };
    }

    fn on_reply(&mut self, cmd: &str, data: &Value, commands: &mut impl Commands) {
        match cmd {
            // Answered after Discord has said the user moved, it's stale.
            "GET_SELECTED_VOICE_CHANNEL" if !self.select_seen => {
                self.selected = data["id"].as_str().map(str::to_string);
                let call = self.call_of(data);
                self.show(call, commands);
            }
            // One sent for a server's Flyout never comes here: the `Following` tells it by its nonce first (Rings: a
            // known DM's would be told the same way). Fetched for a channel the user has since left, it's stale.
            "GET_CHANNEL" if self.selected.is_some() && data["id"].as_str() == self.selected.as_deref() => {
                let call = self.call_of(data);
                self.show(call, commands);
            }
            "GET_VOICE_SETTINGS" => self.on_voice_settings(data),
            _ => {}
        }
    }

    /// Discord refused a command; the connection stays up. Refused the channel the user moved to, the Call shown
    /// is no longer theirs, so it goes.
    fn on_refused(&mut self, cmd: &str, message: &str, commands: &mut impl Commands) {
        self.say(format!("Discord refused {cmd}: {message}"));
        if cmd == "GET_CHANNEL" && self.call.as_ref().is_some_and(|call| Some(&call.id) != self.selected.as_ref()) {
            self.show(None, commands);
        }
    }

    fn on_dispatch(&mut self, evt: &str, data: &Value, commands: &mut impl Commands) {
        match evt {
            "VOICE_CHANNEL_SELECT" => {
                self.select_seen = true;
                self.selected = data["channel_id"].as_str().map(str::to_string);
                match &self.selected {
                    None => self.show(None, commands),
                    // The Call moves over once the channel arrives, with the people in it.
                    Some(id) if self.call.as_ref().map(|call| &call.id) != Some(id) => {
                        commands.send("GET_CHANNEL", json!({ "channel_id": id }), None);
                    }
                    Some(_) => {}
                }
            }
            "VOICE_SETTINGS_UPDATE" => self.on_voice_settings(data),
            "VOICE_STATE_CREATE" | "VOICE_STATE_UPDATE" => {
                // Rings: someone coming into a channel followed for rings would be asked about first.
                let Some(call) = &mut self.call else { return };
                if let Some(joined) = call.upsert(data) {
                    let line = format!("{} joined the call", joined.name);
                    self.say(line);
                }
            }
            "VOICE_STATE_DELETE" => {
                let Some(call) = &mut self.call else { return };
                if let Some(left) = call.remove(data) {
                    self.say(format!("{} left the call", left.name));
                }
            }
            "SPEAKING_START" | "SPEAKING_STOP" => {
                let Some(call) = &mut self.call else { return };
                let channel = data["channel_id"].as_str();
                if let (Some(id), true) = (data["user_id"].as_str(), channel.is_none_or(|channel| channel == call.id)) {
                    call.set_speaking(id, evt == "SPEAKING_START");
                }
            }
            _ => {}
        }
    }

    fn on_voice_settings(&mut self, data: &Value) {
        let (mute, deaf) = (data["mute"].as_bool().unwrap_or(self.mute), data["deaf"].as_bool().unwrap_or(self.deaf));
        if (mute, deaf) != (self.mute, self.deaf) {
            let on = |on: bool| if on { "on" } else { "off" };
            self.say(format!("Discord says mute {}, deafen {}", on(mute), on(deaf)));
        }
        (self.mute, self.deaf) = (mute, deaf);
    }

    fn call_of(&self, channel: &Value) -> Option<CallState> {
        CallState::from_channel(channel, self.me.as_deref().unwrap_or_default())
    }

    /// Shows `next`, or no Call for None: the old channel's events are no longer followed and the new one's are.
    /// The same channel fetched afresh keeps who was speaking.
    fn show(&mut self, mut next: Option<CallState>, commands: &mut impl Commands) {
        let next_id = next.as_ref().map(|call| call.id.clone());
        if let Some(call) = self.call.as_ref().filter(|call| Some(&call.id) != next_id.as_ref()) {
            // Rings: a known DM's comings and goings would stay followed for rings.
            for evt in CALL_EVENTS {
                commands.send("UNSUBSCRIBE", json!({ "channel_id": call.id }), Some(evt));
            }
        }
        match (&mut next, &self.call) {
            (Some(next), Some(call)) if call.id == next.id => next.keep_speaking_from(call),
            (Some(next), _) => {
                // The channel becomes a Known channel, at the front.
                self.joined.push(next.joined());
                for evt in CALL_EVENTS {
                    commands.send("SUBSCRIBE", json!({ "channel_id": next.id }), Some(evt));
                }
            }
            (None, _) => {}
        }
        match (&next, &self.call) {
            (Some(next), Some(call)) if call.id == next.id => {}
            (Some(next), _) => self.say(format!("in a call in {:?} with {}", next.title(), next.names())),
            (None, Some(_)) => self.say("out of the call".into()),
            (None, None) => {}
        }
        self.call = next;
    }

    /// The Discord tab's Mute, Deafen or Leave: asked of Discord, whose events then show it done. Mute and Deafen
    /// turn over what Discord last said.
    pub fn ask(&mut self, ask: CallAsk, commands: &mut impl Commands) {
        let (cmd, args, what) = match ask {
            CallAsk::Mute => {
                ("SET_VOICE_SETTINGS", json!({ "mute": !self.mute }), if self.mute { "unmute" } else { "mute" })
            }
            CallAsk::Deafen => {
                ("SET_VOICE_SETTINGS", json!({ "deaf": !self.deaf }), if self.deaf { "undeafen" } else { "deafen" })
            }
            CallAsk::Leave => ("SELECT_VOICE_CHANNEL", json!({ "channel_id": null }), "leave the call"),
        };
        if commands.send(cmd, args, None).is_some() {
            self.say(format!("asking Discord to {what}"));
        }
    }

    /// A Shortcut or a server's voice channel: Discord asked to join it, unless it is the Call's already (the
    /// Bundle's `JoinChannel`). `name` is what it is called, for the log.
    pub fn join(&mut self, id: &str, name: &str, commands: &mut impl Commands) {
        if self.call.as_ref().is_some_and(|call| call.id == id) {
            self.say(format!("{name:?} is the call's already: nothing asked"));
            return;
        }
        let args = json!({ "channel_id": id, "force": true });
        if commands.send("SELECT_VOICE_CHANNEL", args, None).is_some() {
            self.say(format!("asking Discord to join {name:?}"));
        }
    }

    /// The channels a Call has begun in since this was last asked, oldest first.
    pub fn take_joined(&mut self) -> Vec<Joined> {
        std::mem::take(&mut self.joined)
    }

    /// The Call's channel as it is now, as a Known channel learns it; None out of a Call.
    pub fn call_joined(&self) -> Option<Joined> {
        self.call.as_ref().map(CallState::joined)
    }

    /// The Call's channel's id; None out of a Call.
    pub fn call_id(&self) -> Option<&str> {
        self.call.as_ref().map(|call| call.id.as_str())
    }

    /// The Call and the voice settings as the screens show them, each person's avatar as `avatar` has it: the
    /// cached file, or None until it is.
    pub fn voice(&self, mut avatar: impl FnMut(&Avatar, &str) -> Option<PathBuf>) -> Voice {
        let call = self.call.as_ref().map(|call| Call {
            id: call.id.clone(),
            title: call.title(),
            people: call
                .user_first()
                .map(|p| Person {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    avatar: avatar(&p.avatar, &p.name),
                    speaking: call.speaking.contains(&p.id),
                    muted: p.muted,
                    deafened: p.deafened,
                    me: p.id == call.me,
                })
                .collect(),
        });
        Voice { call, mute: self.mute, deaf: self.deaf }
    }

    /// The log's lines since this was last asked (`discord: Ana joined the call`).
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    fn say(&mut self, line: String) {
        self.log.push(format!("discord: {line}"));
    }
}
