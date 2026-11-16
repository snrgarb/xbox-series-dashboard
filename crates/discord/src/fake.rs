//! A fake Discord for tests (`standin discord`, and this crate's own tests): it listens on a pipe of the test's
//! naming, never Discord's own, and answers the handshake, AUTHORIZE (approved at once, as if the user had pressed
//! Authorize), AUTHENTICATE, SUBSCRIBE and the voice commands; with its own OAuth2 token endpoint on 127.0.0.1, which
//! grants what it gave codes and refresh tokens for. It can be told (`control`, or over the pipe `<name>-control`
//! with `standin discord-say`) to put the user in a Call in a server's voice channel, a DM or a group DM, put people
//! in a voice channel the user isn't in, change who is in the Call, who speaks and who is muted or deafened, hold the
//! voice commands until told to carry them out or refuse them, close the connection, and go away and come back as
//! Discord quitting and starting again does. It answers the servers (GET_GUILDS, GET_GUILD), their channels
//! (GET_CHANNELS) and who is in each (GET_CHANNEL), and joins any voice channel or DM it knows on
//! SELECT_VOICE_CHANNEL. Its HTTP server is also the CDN the avatars and server icons come from
//! ([`FakeDiscord::cdn_url`]): a plain picture of one colour for each address asked.
//!
//! The made-up world: the user "E2E Tester" (`e2e-tester`, with no avatar of their own: Discord's default one); three
//! servers ([`SERVERS`]): "Stand-in Server", with a text channel and two voice channels, General and Late night;
//! "Another Server", with no icon, a voice channel (Lobby), a text channel and a stage (Town hall); and "art club",
//! with text only; DMs and group DMs made as the user is put in them; and people met by name as they are first
//! named, each with an avatar.
use std::collections::BTreeMap;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::frame::{encode, Opcode};
use crate::ipc::{read_frames, Stream};

/// What the fake says as it goes.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

pub const USER_ID: &str = "100000000000000001";
pub const USERNAME: &str = "e2e-tester";
pub const DISPLAY_NAME: &str = "E2E Tester";
pub const GUILD_ID: &str = "900000000000000001";
pub const GUILD_NAME: &str = "Stand-in Server";
/// Stand-in Server's voice channels: id and name.
pub const CHANNELS: [(&str, &str); 2] = [("500000000000000001", "General"), ("500000000000000002", "Late night")];

/// A made-up server: its id, name, whether it has an icon, and its channels in Discord's order, each an id, a name
/// and Discord's type (0 text, 2 voice, 13 stage).
pub type FakeServer = (&'static str, &'static str, bool, &'static [(&'static str, &'static str, u64)]);

/// The servers, in no order of the Bundle's.
pub const SERVERS: [FakeServer; 3] = [
    (
        GUILD_ID,
        GUILD_NAME,
        true,
        &[
            ("500000000000000009", "chat", 0),
            ("500000000000000001", "General", 2),
            ("500000000000000002", "Late night", 2),
        ],
    ),
    (
        "900000000000000002",
        "Another Server",
        false,
        &[
            ("510000000000000001", "Lobby", 2),
            ("510000000000000009", "notes", 0),
            ("510000000000000002", "Town hall", 13),
        ],
    ),
    ("900000000000000003", "art club", true, &[("520000000000000009", "drawings", 0)]),
];

/// A server's icon, on Discord's CDN: its hash made up from its id, as a person's avatar's is.
fn icon_url(guild: &str) -> String {
    format!("https://cdn.discordapp.com/icons/{guild}/{}.png", World::avatar(guild))
}

/// A server, as GET_GUILDS and GET_GUILD give it.
fn guild(server: &FakeServer) -> Value {
    let (id, name, icon, _) = server;
    json!({ "id": id, "name": name, "icon_url": icon.then(|| icon_url(id)) })
}

/// The control pipe's name for the fake on `name`.
pub fn control_name(name: &str) -> String {
    format!("{name}-control")
}

#[derive(Default)]
struct World {
    /// People by id: their names. The user is not among them.
    people: BTreeMap<String, String>,
    /// Who is in each voice channel and DM, by id (the user only as they are in it: `selected`).
    members: BTreeMap<String, Vec<String>>,
    /// The DMs and group DMs made: id, their people (the user aside), and whether a group.
    dms: Vec<(String, Vec<String>, bool)>,
    /// The channel the user is in.
    selected: Option<String>,
    mute: bool,
    deaf: bool,
    /// The others' mute and deafen, by id: set by the server, as far as the user can tell.
    marks: BTreeMap<String, (bool, bool)>,
    /// What the client subscribed to: the event, and the channel for the per-channel ones.
    subscriptions: Vec<(String, Option<String>)>,
    /// Codes, access tokens and refresh tokens given out.
    issued: u64,
    /// The voice commands (SET_VOICE_SETTINGS, SELECT_VOICE_CHANNEL) wait to be confirmed or refused.
    holding: bool,
    /// Those waiting: the command, its args and its nonce.
    held: Vec<(String, Value, Value)>,
}

/// The commands `hold` keeps waiting.
const VOICE_COMMANDS: [&str; 2] = ["SET_VOICE_SETTINGS", "SELECT_VOICE_CHANNEL"];

impl World {
    fn person(&mut self, name: &str) -> String {
        if let Some((id, _)) = self.people.iter().find(|(_, known)| known.as_str() == name) {
            return id.clone();
        }
        let id = format!("{}", 200000000000000000u64 + self.people.len() as u64 + 2);
        self.people.insert(id.clone(), name.to_string());
        id
    }

    /// Who `who` is: `me` the user, anyone else by name.
    fn who(&mut self, who: &str) -> String {
        if who == "me" {
            USER_ID.to_string()
        } else {
            self.person(who)
        }
    }

    /// A person's avatar hash: made up from their id, 32 hex digits as Discord's are.
    fn avatar(id: &str) -> String {
        format!("{:032x}", id.parse::<u128>().unwrap_or(0))
    }

    fn subscribed(&self, evt: &str, channel: Option<&str>) -> bool {
        self.subscriptions.iter().any(|(e, c)| e == evt && (c.is_none() || c.as_deref() == channel))
    }

    fn user(&self) -> Value {
        json!({ "id": USER_ID, "username": USERNAME, "global_name": DISPLAY_NAME, "avatar": null, "bot": false })
    }

    fn voice_state(&self, id: &str) -> Value {
        let (name, user) = if id == USER_ID {
            (DISPLAY_NAME.to_string(), self.user())
        } else {
            let name = self.people.get(id).cloned().unwrap_or_default();
            let avatar = World::avatar(id);
            (name.clone(), json!({ "id": id, "username": name.to_lowercase(), "global_name": name, "avatar": avatar }))
        };
        let me = id == USER_ID;
        let (mute, deaf) = self.marks.get(id).copied().unwrap_or_default();
        json!({
            "nick": name,
            "mute": false,
            "volume": 100,
            "user": user,
            "voice_state": { "mute": mute, "deaf": deaf, "self_mute": me && self.mute, "self_deaf": me && self.deaf },
        })
    }

    /// A server's voice or stage channel by id: its name, type and server.
    fn server_channel(id: &str) -> Option<(&'static str, u64, &'static str)> {
        SERVERS.iter().find_map(|(guild, _, _, channels)| {
            let found = channels.iter().find(|(known, _, kind)| *known == id && *kind != 0);
            found.map(|(_, name, kind)| (*name, *kind, *guild))
        })
    }

    /// A voice channel or DM, as GET_CHANNEL answers: with everyone in it, the user first if they are.
    fn channel(&self, id: &str) -> Option<Value> {
        let (name, kind, guild) = match Self::server_channel(id) {
            Some((name, kind, guild)) => (name, kind, Some(guild)),
            None => {
                let (_, _, group) = self.dms.iter().find(|(known, _, _)| known == id)?;
                ("", if *group { 3 } else { 1 }, None)
            }
        };
        let mut members = self.members.get(id).cloned().unwrap_or_default();
        if self.selected.as_deref() == Some(id) {
            members.insert(0, USER_ID.to_string());
        }
        let states: Vec<Value> = members.iter().map(|member| self.voice_state(member)).collect();
        Some(json!({ "id": id, "name": name, "type": kind, "guild_id": guild, "voice_states": states }))
    }

    /// A server's voice or stage channel by its name (`_` for a space, any case).
    fn find(name: &str) -> Option<&'static str> {
        let name = name.replace('_', " ");
        let mut voice =
            SERVERS.iter().flat_map(|(_, _, _, channels)| channels.iter()).filter(|(_, _, kind)| *kind != 0);
        voice.find(|(_, known, _)| known.eq_ignore_ascii_case(&name)).map(|(id, _, _)| *id)
    }

    fn voice_settings(&self) -> Value {
        json!({ "mute": self.mute, "deaf": self.deaf })
    }
}

struct Shared {
    name: String,
    world: Mutex<World>,
    /// The client connected now; writes to it go one at a time under the lock.
    client: Mutex<Option<Arc<dyn Stream>>>,
    /// Gone away (or stopping): nothing listens on the pipe.
    quiet: AtomicBool,
    stop: AtomicBool,
    /// The pipe, and the control pipe, have been listened on once: a client can come.
    listening: AtomicBool,
    control_listening: AtomicBool,
    log: Log,
}

impl Shared {
    fn say(&self, line: &str) {
        (self.log)(&format!("discord fake: {line}"));
    }

    fn write(&self, opcode: Opcode, payload: &Value) {
        if let Some(client) = self.client.lock().unwrap().as_ref() {
            if client.write_all(&encode(opcode, &payload.to_string())).is_err() {
                client.shut();
            }
        }
    }

    fn reply(&self, cmd: &str, nonce: &Value, data: Value, evt: Option<&str>) {
        self.write(Opcode::Frame, &json!({ "cmd": cmd, "data": data, "evt": evt, "nonce": nonce }));
    }

    fn dispatch(&self, evt: &str, data: Value) {
        self.write(Opcode::Frame, &json!({ "cmd": "DISPATCH", "data": data, "evt": evt, "nonce": null }));
    }

    fn error(&self, cmd: &str, nonce: &Value, code: u32, message: &str) {
        self.reply(cmd, nonce, json!({ "code": code, "message": message }), Some("ERROR"));
    }

    /// A frame from the client.
    fn frame(&self, opcode: Opcode, text: &str, said_hello: &mut bool) {
        let payload: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        match opcode {
            Opcode::Handshake => {
                if payload["v"] != 1 || !payload["client_id"].is_string() {
                    self.say("a handshake with no Client ID: closing");
                    self.write(Opcode::Close, &json!({ "code": 4000, "message": "Invalid Client ID" }));
                    if let Some(client) = self.client.lock().unwrap().as_ref() {
                        client.shut();
                    }
                    return;
                }
                *said_hello = true;
                self.say("handshake: READY");
                let config = json!({ "cdn_host": "cdn.discordapp.com", "api_endpoint": "//discord.com/api" });
                let user = self.world.lock().unwrap().user();
                self.dispatch("READY", json!({ "v": 1, "config": config, "user": user }));
            }
            Opcode::Ping => self.write(Opcode::Pong, &payload),
            Opcode::Frame if *said_hello => self.command(&payload),
            _ => {}
        }
    }

    fn command(&self, frame: &Value) {
        let cmd = frame["cmd"].as_str().unwrap_or_default();
        let (args, nonce) = (&frame["args"], &frame["nonce"]);
        self.say(&format!("{cmd}{}", frame["evt"].as_str().map(|evt| format!(" {evt}")).unwrap_or_default()));
        let mut world = self.world.lock().unwrap();
        match cmd {
            "AUTHORIZE" => {
                world.issued += 1;
                let code = format!("fake-code-{}", world.issued);
                drop(world);
                self.reply(cmd, nonce, json!({ "code": code }), None);
            }
            "AUTHENTICATE" => {
                let token = args["access_token"].as_str().unwrap_or_default();
                if token.starts_with("fake-access-") {
                    let user = world.user();
                    drop(world);
                    let scopes = crate::client::SCOPES;
                    let application = json!({ "id": "1", "name": "Stand-in application" });
                    let data = json!({ "user": user, "scopes": scopes, "application": application });
                    self.reply(cmd, nonce, data, None);
                } else {
                    drop(world);
                    self.error(cmd, nonce, 4009, "Invalid access token");
                }
            }
            "SUBSCRIBE" | "UNSUBSCRIBE" => {
                let evt = frame["evt"].as_str().unwrap_or_default().to_string();
                let channel = args["channel_id"].as_str().map(str::to_string);
                if cmd == "SUBSCRIBE" {
                    world.subscriptions.push((evt.clone(), channel));
                } else {
                    world.subscriptions.retain(|(e, c)| !(*e == evt && *c == channel));
                }
                drop(world);
                self.reply(cmd, nonce, json!({ "evt": evt }), None);
            }
            "GET_SELECTED_VOICE_CHANNEL" => {
                let channel = world.selected.clone().and_then(|id| world.channel(&id)).unwrap_or(Value::Null);
                drop(world);
                self.reply(cmd, nonce, channel, None);
            }
            "GET_CHANNEL" => {
                let channel = args["channel_id"].as_str().and_then(|id| world.channel(id));
                drop(world);
                match channel {
                    Some(channel) => self.reply(cmd, nonce, channel, None),
                    None => self.error(cmd, nonce, 4005, "Invalid channel"),
                }
            }
            "GET_GUILDS" => {
                drop(world);
                let guilds: Vec<Value> = SERVERS.iter().map(guild).collect();
                self.reply(cmd, nonce, json!({ "guilds": guilds }), None);
            }
            "GET_GUILD" | "GET_CHANNELS" => {
                drop(world);
                let id = args["guild_id"].as_str().unwrap_or_default();
                let Some(server) = SERVERS.iter().find(|(known, _, _, _)| *known == id) else {
                    self.error(cmd, nonce, 4004, "Invalid guild");
                    return;
                };
                let data = if cmd == "GET_GUILD" {
                    let mut data = guild(server);
                    data["members"] = json!([]);
                    data
                } else {
                    let channels: Vec<Value> = (server.3)
                        .iter()
                        .map(|(channel, name, kind)| json!({ "id": channel, "name": name, "type": kind, "guild_id": id }))
                        .collect();
                    json!({ "channels": channels })
                };
                self.reply(cmd, nonce, data, None);
            }
            "GET_VOICE_SETTINGS" => {
                let settings = world.voice_settings();
                drop(world);
                self.reply(cmd, nonce, settings, None);
            }
            _ if VOICE_COMMANDS.contains(&cmd) && world.holding => {
                world.held.push((cmd.to_string(), args.clone(), nonce.clone()));
                drop(world);
                self.say(&format!("{cmd} held: waiting for confirm or refuse"));
            }
            _ if VOICE_COMMANDS.contains(&cmd) => {
                drop(world);
                self.voice_command(cmd, args, nonce);
            }
            _ => {
                drop(world);
                self.error(cmd, nonce, 4000, "Unknown command");
            }
        }
    }

    /// SET_VOICE_SETTINGS or SELECT_VOICE_CHANNEL carried out: Discord answers, and tells whoever subscribed.
    fn voice_command(&self, cmd: &str, args: &Value, nonce: &Value) {
        let mut world = self.world.lock().unwrap();
        if cmd == "SET_VOICE_SETTINGS" {
            if let Some(mute) = args["mute"].as_bool() {
                world.mute = mute;
            }
            if let Some(deaf) = args["deaf"].as_bool() {
                world.deaf = deaf;
            }
            drop(world);
            let settings = self.world.lock().unwrap().voice_settings();
            self.reply(cmd, nonce, settings, None);
            self.settings_changed(USER_ID);
            return;
        }
        let id = args["channel_id"].as_str().map(str::to_string);
        if id.as_deref().is_some_and(|id| world.channel(id).is_none()) {
            drop(world);
            self.error(cmd, nonce, 4005, "Invalid channel");
            return;
        }
        drop(world);
        let channel = self.select(id);
        self.reply(cmd, nonce, channel, None);
    }

    /// The user's voice settings, or someone's marks, changed: VOICE_SETTINGS_UPDATE for the user's own, and the
    /// person's VOICE_STATE_UPDATE in the Call they are in, to whoever subscribed.
    fn settings_changed(&self, id: &str) {
        let world = self.world.lock().unwrap();
        let settings =
            (id == USER_ID && world.subscribed("VOICE_SETTINGS_UPDATE", None)).then(|| world.voice_settings());
        let channel = world.selected.clone().filter(|channel| {
            id == USER_ID || world.members.get(channel).is_some_and(|members| members.iter().any(|m| m == id))
        });
        let state = channel
            .filter(|channel| world.subscribed("VOICE_STATE_UPDATE", Some(channel)))
            .map(|_| world.voice_state(id));
        drop(world);
        if let Some(settings) = settings {
            self.dispatch("VOICE_SETTINGS_UPDATE", settings);
        }
        if let Some(state) = state {
            self.dispatch("VOICE_STATE_UPDATE", state);
        }
    }

    /// The user is now in `channel`, or in none: Discord says so to whoever subscribed. The channel, as
    /// GET_CHANNEL gives it.
    fn select(&self, channel: Option<String>) -> Value {
        let mut world = self.world.lock().unwrap();
        world.selected = channel.clone();
        let tell = world.subscribed("VOICE_CHANNEL_SELECT", None);
        let found = channel.as_deref().and_then(|id| world.channel(id)).unwrap_or(Value::Null);
        drop(world);
        if tell {
            let guild = channel.as_deref().and_then(World::server_channel).map(|(_, _, guild)| guild);
            self.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": channel, "guild_id": guild }));
        }
        found
    }

    /// Someone joins or leaves the channel the user is in.
    fn member(&self, name: &str, joins: bool) -> Result<String, String> {
        let mut world = self.world.lock().unwrap();
        let Some(channel) = world.selected.clone() else { return Err("the user is in no Call".into()) };
        let id = world.person(name);
        let members = world.members.entry(channel.clone()).or_default();
        let was = members.contains(&id);
        if joins && !was {
            members.push(id.clone());
        } else if !joins && was {
            members.retain(|member| *member != id);
        } else {
            return Err(format!("{name} is {}in the Call already", if joins { "" } else { "not " }));
        }
        let evt = if joins { "VOICE_STATE_CREATE" } else { "VOICE_STATE_DELETE" };
        let (tell, state) = (world.subscribed(evt, Some(&channel)), world.voice_state(&id));
        drop(world);
        if tell {
            self.dispatch(evt, state);
        }
        Ok(format!("{name} {} the Call", if joins { "joined" } else { "left" }))
    }

    /// The client's connection ends, as Discord dropping it.
    fn drop_client(&self) {
        if let Some(client) = self.client.lock().unwrap().take() {
            client.shut();
        }
    }

    /// A control command: what it did, or why it didn't.
    fn control(&self, line: &str) -> Result<String, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [verb @ ("call" | "in"), channel, people @ ..] => {
                let Some(id) = World::find(channel) else {
                    return Err(format!("no voice channel {:?}", channel.replace('_', " ")));
                };
                {
                    let mut world = self.world.lock().unwrap();
                    let ids: Vec<String> = people.iter().map(|person| world.person(person)).collect();
                    world.members.insert(id.to_string(), ids);
                }
                let (name, everyone) = (channel.replace('_', " "), people.join(", "));
                if *verb == "in" {
                    // People in a voice channel the user may not be in: who GET_CHANNEL says is there.
                    let everyone = if people.is_empty() { "nobody".to_string() } else { everyone };
                    return Ok(format!("in {name}: {everyone}"));
                }
                self.select(Some(id.to_string()));
                Ok(format!("the user is in {name} with {everyone}"))
            }
            // A DM with one person, a group DM with more (or named `group`): made if new, the user in its Call.
            [verb @ ("dm" | "group"), people @ ..] if !people.is_empty() => {
                let id = {
                    let mut world = self.world.lock().unwrap();
                    let ids: Vec<String> = people.iter().map(|person| world.person(person)).collect();
                    let id = match world.dms.iter().find(|(_, known, _)| *known == ids) {
                        Some((id, _, _)) => id.clone(),
                        None => {
                            let id = format!("{}", 600000000000000001u64 + world.dms.len() as u64);
                            world.dms.push((id.clone(), ids.clone(), *verb == "group" || ids.len() > 1));
                            id
                        }
                    };
                    world.members.insert(id.clone(), ids);
                    id
                };
                self.select(Some(id));
                Ok(format!(
                    "the user is in a {} with {}",
                    if *verb == "dm" { "DM" } else { "group DM" },
                    people.join(", ")
                ))
            }
            // Joining by command, as SELECT_VOICE_CHANNEL from another device: the voice channel by name.
            ["select", channel] => {
                let Some(id) = World::find(channel) else {
                    return Err(format!("no voice channel {:?}", channel.replace('_', " ")));
                };
                self.select(Some(id.to_string()));
                Ok(format!("the user is in {}", channel.replace('_', " ")))
            }
            ["join", person] => self.member(person, true),
            ["leave", person] => self.member(person, false),
            [verb @ ("speak" | "hush"), who] => {
                let mut world = self.world.lock().unwrap();
                let Some(channel) = world.selected.clone() else { return Err("the user is in no Call".into()) };
                let id = world.who(who);
                let present = id == USER_ID || world.members.get(&channel).is_some_and(|m| m.contains(&id));
                if !present {
                    return Err(format!("{who} is not in the Call"));
                }
                let evt = if *verb == "speak" { "SPEAKING_START" } else { "SPEAKING_STOP" };
                let tell = world.subscribed(evt, Some(&channel));
                drop(world);
                if tell {
                    self.dispatch(evt, json!({ "user_id": id, "channel_id": channel }));
                }
                Ok(format!("{who} {}", if *verb == "speak" { "speaks" } else { "stopped speaking" }))
            }
            [verb @ ("mute" | "unmute" | "deafen" | "undeafen"), who] => {
                let on = !verb.starts_with("un");
                let mut world = self.world.lock().unwrap();
                let id = world.who(who);
                if id == USER_ID {
                    if verb.ends_with("mute") {
                        world.mute = on;
                    } else {
                        world.deaf = on;
                    }
                } else {
                    let marks = world.marks.entry(id.clone()).or_default();
                    if verb.ends_with("mute") {
                        marks.0 = on;
                    } else {
                        marks.1 = on;
                    }
                }
                drop(world);
                self.settings_changed(&id);
                let done = if verb.ends_with("mute") { format!("{verb}d") } else { format!("{verb}ed") };
                Ok(format!("{who} {done}"))
            }
            ["hold"] => {
                self.world.lock().unwrap().holding = true;
                Ok("the voice commands wait for confirm or refuse".into())
            }
            ["confirm"] => {
                let held = {
                    let mut world = self.world.lock().unwrap();
                    world.holding = false;
                    std::mem::take(&mut world.held)
                };
                for (cmd, args, nonce) in &held {
                    self.voice_command(cmd, args, nonce);
                }
                Ok(format!("{} carried out", held.len()))
            }
            ["refuse", message @ ..] => {
                let held = {
                    let mut world = self.world.lock().unwrap();
                    world.holding = false;
                    std::mem::take(&mut world.held)
                };
                let message = if message.is_empty() { "Refused".to_string() } else { message.join(" ") };
                for (cmd, _, nonce) in &held {
                    self.error(cmd, nonce, 4000, &message);
                }
                Ok(format!("{} refused", held.len()))
            }
            ["hang-up"] => {
                self.select(None);
                Ok("the user left the Call".into())
            }
            ["go-away"] => {
                self.quiet.store(true, Ordering::SeqCst);
                self.drop_client();
                Ok("gone: nothing listens on the pipe".into())
            }
            ["come-back"] => {
                self.quiet.store(false, Ordering::SeqCst);
                Ok("back: listening on the pipe".into())
            }
            ["close", message @ ..] => {
                self.write(Opcode::Close, &json!({ "code": 1000, "message": message.join(" ") }));
                self.drop_client();
                Ok("closed the connection".into())
            }
            _ => Err(format!("unknown command {line:?}")),
        }
    }
}

/// A fake Discord, running until dropped.
pub struct FakeDiscord {
    shared: Arc<Shared>,
    token_port: u16,
}

impl FakeDiscord {
    /// Listens on the pipe `name` (a test's, never Discord's own) and its control pipe, and serves the token
    /// endpoint on 127.0.0.1 at `port` (any free one for 0).
    pub fn start(name: &str, port: u16, log: Log) -> io::Result<FakeDiscord> {
        if name.starts_with("discord-ipc-") {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "that is Discord's own pipe"));
        }
        let shared = Arc::new(Shared {
            name: name.to_string(),
            world: Mutex::new(World::default()),
            client: Mutex::new(None),
            quiet: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            listening: AtomicBool::new(false),
            control_listening: AtomicBool::new(false),
            log,
        });
        let http = TcpListener::bind(("127.0.0.1", port))?;
        let token_port = http.local_addr()?.port();
        http.set_nonblocking(true)?;
        let (rpc, control, tokens) = (shared.clone(), shared.clone(), shared.clone());
        std::thread::Builder::new().name("discord fake".into()).spawn(move || serve(&rpc))?;
        std::thread::Builder::new().name("discord fake control".into()).spawn(move || serve_control(&control))?;
        std::thread::Builder::new().name("discord fake tokens".into()).spawn(move || serve_tokens(&tokens, http))?;
        // Started when both pipes can be connected to: a test connecting at once finds it there.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !(shared.listening.load(Ordering::SeqCst) && shared.control_listening.load(Ordering::SeqCst)) {
            if std::time::Instant::now() > deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, format!("couldn't listen on {name}")));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        shared.say(&format!(
            "listening on {name}, tokens at http://127.0.0.1:{token_port}/api/oauth2/token, avatars at http://127.0.0.1:{token_port}"
        ));
        Ok(FakeDiscord { shared, token_port })
    }

    /// The token endpoint's address, for `dashboard --discord-token-url`.
    pub fn token_url(&self) -> String {
        format!("http://127.0.0.1:{}/api/oauth2/token", self.token_port)
    }

    /// The stand-in CDN's address, for `dashboard --discord-cdn-url`: the avatars are there.
    pub fn cdn_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.token_port)
    }

    /// Does a control command (`call General Ana Bo`, `in Late_night Cy Di`, `select Lobby`, `dm Ana`,
    /// `group Ana Bo`, `join Cy`, `leave Ana`, `speak Ana`, `hush Ana`, `mute me`, `undeafen Bo`, `hold`, `confirm`,
    /// `refuse <message>`, `hang-up`, `go-away`, `come-back`, `close <message>`): what it did, or why it didn't.
    pub fn control(&self, line: &str) -> Result<String, String> {
        let done = self.shared.control(line);
        self.shared.say(&format!("{line}: {}", done.as_ref().unwrap_or_else(|e| e)));
        done
    }

    /// Whether a client is connected.
    pub fn connected(&self) -> bool {
        self.shared.client.lock().unwrap().is_some()
    }

    /// What the client has subscribed to: each event, and its channel for a per-channel one.
    pub fn subscriptions(&self) -> Vec<(String, Option<String>)> {
        self.shared.world.lock().unwrap().subscriptions.clone()
    }
}

impl Drop for FakeDiscord {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.quiet.store(true, Ordering::SeqCst);
        self.shared.drop_client();
    }
}

/// Serves one client at a time on the pipe, until stopped; nothing listens while it is away.
fn serve(shared: &Shared) {
    while !shared.stop.load(Ordering::SeqCst) {
        if shared.quiet.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        let client = match listen_once(&shared.name, &shared.quiet, &shared.listening) {
            Ok(Some(client)) => client,
            Ok(None) => continue,
            Err(e) => {
                shared.say(&format!("the pipe: {e}"));
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        shared.say("a client connected");
        *shared.client.lock().unwrap() = Some(client.clone());
        shared.world.lock().unwrap().subscriptions.clear();
        let mut said_hello = false;
        read_frames(&*client, |opcode, json| shared.frame(opcode, &json, &mut said_hello));
        client.shut();
        let mut current = shared.client.lock().unwrap();
        if current.as_ref().is_some_and(|open| Arc::ptr_eq(open, &client)) {
            *current = None;
        }
        drop(current);
        shared.say("the client's connection ended");
    }
}

/// Waits for one client on the pipe `name`, setting `listening` once it can come: None if `quiet` is set first.
#[cfg(windows)]
fn listen_once(name: &str, quiet: &AtomicBool, listening: &AtomicBool) -> io::Result<Option<Arc<dyn Stream>>> {
    let pipe = crate::win::Pipe::listen(name)?;
    listening.store(true, Ordering::SeqCst);
    Ok(pipe.accept(quiet)?.then(|| Arc::new(pipe) as Arc<dyn Stream>))
}

#[cfg(unix)]
fn listen_once(name: &str, quiet: &AtomicBool, listening: &AtomicBool) -> io::Result<Option<Arc<dyn Stream>>> {
    use std::os::unix::net::UnixListener;
    let path = crate::ipc::socket_path(name);
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    listening.store(true, Ordering::SeqCst);
    let accepted = loop {
        if quiet.load(Ordering::SeqCst) {
            break None;
        }
        match listener.accept() {
            Ok((stream, _)) => break Some(stream),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(e),
        }
    };
    // Nothing listens while a client is served, nor while away: as on Windows, where the one instance is taken.
    drop(listener);
    let _ = std::fs::remove_file(&path);
    match accepted {
        Some(stream) => {
            stream.set_nonblocking(false)?;
            Ok(Some(Arc::new(stream)))
        }
        None => Ok(None),
    }
}

/// The control pipe: a line in, a line back, one connection each.
fn serve_control(shared: &Shared) {
    let name = control_name(&shared.name);
    while !shared.stop.load(Ordering::SeqCst) {
        let stream = match listen_once(&name, &shared.stop, &shared.control_listening) {
            Ok(Some(stream)) => stream,
            Ok(None) => continue,
            Err(e) => {
                shared.say(&format!("the control pipe: {e}"));
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        let mut line = Vec::new();
        let mut buf = [0u8; 512];
        while !line.contains(&b'\n') {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => line.extend_from_slice(&buf[..n]),
            }
        }
        let line = String::from_utf8_lossy(&line).trim().to_string();
        let done = shared.control(&line);
        shared.say(&format!("{line}: {}", done.as_ref().unwrap_or_else(|e| e)));
        let answer = match done {
            Ok(said) => format!("ok: {said}\n"),
            Err(why) => format!("error: {why}\n"),
        };
        let _ = stream.write_all(answer.as_bytes());
        stream.shut();
    }
}

/// Sends `line` to the fake on `name` over its control pipe: its answer.
pub fn say_to(name: &str, line: &str) -> io::Result<String> {
    let stream = crate::ipc::connect(&control_name(name))?;
    stream.write_all(format!("{line}\n").as_bytes())?;
    let mut answer = Vec::new();
    let mut buf = [0u8; 512];
    while !answer.contains(&b'\n') {
        match stream.read(&mut buf)? {
            0 => break,
            n => answer.extend_from_slice(&buf[..n]),
        }
    }
    stream.shut();
    Ok(String::from_utf8_lossy(&answer).trim().to_string())
}

// ------------------------------------------------------------------------------------------------ token endpoint

/// An avatar for `path`: 64 across, of one colour picked by the address, with a lighter disc in its middle, so a
/// picture shows where one is drawn and two people's differ. Nobody's real avatar.
pub fn avatar_picture(path: &str) -> Vec<u8> {
    let hash = path.bytes().fold(2166136261u32, |h, b| (h ^ u32::from(b)).wrapping_mul(16777619));
    let channel = |shift: u32| 60 + ((hash >> shift) & 0x7F) as u8;
    let (r, g, b) = (channel(0), channel(8), channel(16));
    let picture = image::RgbImage::from_fn(64, 64, |x, y| {
        let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
        if dx * dx + dy * dy < 14.0 * 14.0 {
            image::Rgb([r.saturating_add(90), g.saturating_add(90), b.saturating_add(90)])
        } else {
            image::Rgb([r, g, b])
        }
    });
    let mut png = Vec::new();
    picture.write_to(&mut io::Cursor::new(&mut png), image::ImageFormat::Png).expect("a PNG in memory");
    png
}

fn serve_tokens(shared: &Shared, listener: TcpListener) {
    while !shared.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(e) = token_request(shared, stream) {
                    shared.say(&format!("token endpoint: {e}"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                shared.say(&format!("token endpoint: {e}"));
                return;
            }
        }
    }
}

/// `a=1&b=2`, each part `%XX` and `+` read back.
fn form(body: &str) -> BTreeMap<String, String> {
    let decode = |text: &str| {
        let bytes = text.replace('+', " ").into_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let hex = match bytes.get(i + 1..i + 3) {
                Some(digits) if bytes[i] == b'%' => {
                    std::str::from_utf8(digits).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
                }
                _ => None,
            };
            match hex {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                }
                None => {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    };
    body.split('&').filter_map(|pair| pair.split_once('=')).map(|(key, value)| (decode(key), decode(value))).collect()
}

fn token_request(shared: &Shared, stream: TcpStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = io::BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let (mut length, mut authorized) = (0usize, false);
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else { continue };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().unwrap_or(0),
            "authorization" => {
                let credentials = value.trim().strip_prefix("Basic ").and_then(crate::tokens::unbase64);
                let credentials = credentials.map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                authorized = credentials.is_some_and(|both| {
                    both.split_once(':').is_some_and(|(id, secret)| !id.is_empty() && !secret.is_empty())
                });
            }
            _ => {}
        }
    }
    let mut body = vec![0u8; length.min(64 * 1024)];
    reader.read_exact(&mut body)?;
    // The stand-in CDN: an avatar, a plain picture of one colour for each address.
    let path = request_line.strip_prefix("GET ").and_then(|rest| rest.split_whitespace().next());
    let pictures = ["/avatars/", "/embed/avatars/", "/icons/"];
    if let Some(path) = path.filter(|path| pictures.iter().any(|prefix| path.starts_with(prefix))) {
        let picture = avatar_picture(path);
        shared.say(&format!("cdn: {path} served"));
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            picture.len()
        )?;
        stream.write_all(&picture)?;
        return stream.flush();
    }
    let form = form(&String::from_utf8_lossy(&body));
    let grant = form.get("grant_type").cloned().unwrap_or_default();
    let (status, answer) = if !request_line.starts_with("POST /api/oauth2/token") {
        (404, json!({ "error": "not_found" }))
    } else if !authorized {
        (401, json!({ "error": "invalid_client", "error_description": "Invalid client secret" }))
    } else {
        let good = match grant.as_str() {
            "authorization_code" => {
                form.get("code").is_some_and(|code| code.starts_with("fake-code-"))
                    && form.get("redirect_uri").map(String::as_str) == Some(crate::client::REDIRECT_URI)
            }
            "refresh_token" => form.get("refresh_token").is_some_and(|token| token.starts_with("fake-refresh-")),
            _ => false,
        };
        if good {
            let mut world = shared.world.lock().unwrap();
            world.issued += 1;
            let n = world.issued;
            let answer = json!({
                "access_token": format!("fake-access-{n}"),
                "token_type": "Bearer",
                "expires_in": 604800,
                "refresh_token": format!("fake-refresh-{n}"),
                "scope": crate::client::SCOPES.join(" "),
            });
            (200, answer)
        } else {
            (400, json!({ "error": "invalid_grant" }))
        }
    };
    shared.say(&format!("token endpoint: {grant} answered {status}"));
    let body = answer.to_string();
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        _ => "Not Found",
    };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}
