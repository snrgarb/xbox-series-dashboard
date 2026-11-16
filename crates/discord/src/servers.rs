//! Servers (docs/discord.md, "Servers"): the Discord tab's last entry opens a Flyout of the user's servers, those
//! the user has joined a Call in first, most recently first, then the rest by name; a server opens onto its voice and
//! stage channels in Discord's order, each with the avatars of the people in it, or Empty; A on one joins it. What
//! Discord answered is kept until it goes, so each opens at once while it is asked again; one never answered shows
//! the console's spinner, for a second at least however quickly Discord answers. Answers for a Flyout no longer
//! showing are ignored.
//!
//! Ported from the Bundle's `ExtensionCore.DiscordServers.cs`; its tests are `DiscordServersTests` (here
//! `tests/servers.rs`). Pure: it sends through [`Commands`] and is told the time.
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use model::discord::{Browse, Server, VoiceChannel};
use serde_json::{json, Value};

use crate::shortcuts::Known;
use crate::voice::{Avatar, Commands, Participant};

/// How long a Flyout's spinner shows at least, however quickly Discord answers.
pub const SPINNER_SHOWN: Duration = Duration::from_secs(1);

/// Discord's channel types listed among a server's: voice, and stage.
const VOICE: [u64; 2] = [2, 13];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerEntry {
    id: String,
    name: String,
    /// Its icon's address, as Discord gave it.
    icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChannelEntry {
    id: String,
    name: String,
    /// The avatars of the people in it; None until Discord has said.
    occupants: Option<Vec<(Avatar, String)>>,
}

/// The Flyout showing, and what it lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Showing {
    #[default]
    Closed,
    Servers {
        loading: bool,
        items: Vec<ServerEntry>,
    },
    Server {
        id: String,
        name: String,
        loading: bool,
        items: Vec<ChannelEntry>,
    },
}

/// An answer held until the spinner has shown its second.
#[derive(Debug, Clone)]
enum Held {
    Servers(Vec<ServerEntry>),
    Channels(Value),
}

#[derive(Debug, Default)]
pub struct Browser {
    showing: Showing,
    /// The servers, kept from Discord's last answer until it goes; None until it has answered.
    servers: Option<Vec<ServerEntry>>,
    /// Each server's voice channels with who is in them, kept from Discord's last answers until it goes.
    channels: HashMap<String, Vec<ChannelEntry>>,
    /// The nonce of the GET_GUILDS or GET_CHANNELS the Flyout showing waits for; a reply under any other is stale.
    request: Option<String>,
    /// When the loading Flyout showing put its spinner up.
    spinner_since: Option<SystemTime>,
    /// An answer waiting for the spinner, and when it shows.
    held: Option<(SystemTime, Held)>,
    /// The GET_CHANNELs sent to learn who is in a server's voice channels, by nonce.
    occupant_requests: HashSet<String>,
    log: Vec<String>,
}

impl Browser {
    /// A on Servers: the servers Discord answered before at once, or else a Flyout showing they're coming; then
    /// Discord is asked for them.
    pub fn open_servers(&mut self, known: &Known, now: SystemTime, commands: &mut impl Commands) {
        self.held = None;
        self.showing = match &self.servers {
            Some(servers) => Showing::Servers { loading: false, items: in_order(servers.clone(), known) },
            None => {
                self.spinner_since = Some(now);
                Showing::Servers { loading: true, items: Vec::new() }
            }
        };
        self.request = commands.send("GET_GUILDS", json!({}), None);
        self.say("asking Discord for the servers".into());
    }

    /// A on a server: a Flyout under its name showing the channels Discord answered before at once, or else that
    /// they're coming; then Discord is asked for them.
    pub fn open_server(&mut self, id: &str, name: &str, now: SystemTime, commands: &mut impl Commands) {
        self.held = None;
        let (loading, items) = match self.channels.get(id) {
            Some(channels) => (false, channels.clone()),
            None => {
                self.spinner_since = Some(now);
                (true, Vec::new())
            }
        };
        self.showing = Showing::Server { id: id.to_string(), name: name.to_string(), loading, items };
        self.request = commands.send("GET_CHANNELS", json!({ "guild_id": id }), None);
        self.say(format!("asking Discord for {name:?}'s voice channels"));
    }

    /// B in a server's Flyout: back to the servers, as answered before, not asked again.
    pub fn back(&mut self, known: &Known) {
        (self.request, self.held) = (None, None);
        let servers = self.servers.clone().unwrap_or_default();
        self.showing = Showing::Servers { loading: false, items: in_order(servers, known) };
    }

    /// The Flyout closed: B in the servers', or a channel chosen.
    pub fn close(&mut self) {
        (self.request, self.held) = (None, None);
        self.showing = Showing::Closed;
    }

    /// Discord has gone: the servers and their channels are forgotten, to be asked afresh once it's back, and the
    /// Flyout closes.
    pub fn forget(&mut self) {
        let log = std::mem::take(&mut self.log);
        *self = Browser { log, ..Browser::default() };
    }

    /// Whether a GET_CHANNEL reply under `nonce` was asked to learn who is in a server's voice channel: then it only
    /// shows who is there, and the Call comes from its own.
    pub fn take_occupant_request(&mut self, nonce: &str) -> bool {
        self.occupant_requests.remove(nonce)
    }

    fn awaited(&self, servers: bool, nonce: &str) -> bool {
        let kind = match self.showing {
            Showing::Servers { .. } => Some(true),
            Showing::Server { .. } => Some(false),
            Showing::Closed => None,
        };
        self.request.as_deref() == Some(nonce) && kind == Some(servers)
    }

    /// GET_GUILDS answered: a server each, in the Flyout's order, kept for reopening.
    pub fn on_guilds(&mut self, nonce: &str, data: &Value, known: &Known, now: SystemTime) {
        if !self.awaited(true, nonce) {
            return;
        }
        self.request = None;
        let servers = data["guilds"].as_array().into_iter().flatten().filter_map(|guild| {
            Some(ServerEntry {
                id: guild["id"].as_str()?.to_string(),
                name: guild["name"].as_str().unwrap_or_default().to_string(),
                icon: guild["icon_url"].as_str().filter(|url| !url.is_empty()).map(str::to_string),
            })
        });
        let servers = in_order(servers.collect(), known);
        let names: Vec<String> = servers.iter().map(|s| format!("{:?}", s.name)).collect();
        self.say(format!("servers: {}", if names.is_empty() { "none".into() } else { names.join(", ") }));
        self.after_spinner(Held::Servers(servers), now, None::<&mut NoCommands>);
    }

    /// GET_CHANNELS answered: a channel each for the voice and stage ones, in Discord's order, kept for reopening,
    /// those shown before keeping who is in them; then who is in each is asked.
    pub fn on_channels(&mut self, nonce: &str, data: &Value, now: SystemTime, commands: &mut impl Commands) {
        if !self.awaited(false, nonce) {
            return;
        }
        self.request = None;
        self.after_spinner(Held::Channels(data.clone()), now, Some(commands));
    }

    /// Shows `held` once the loading Flyout showing has had its spinner up a second, at once if it has or isn't
    /// loading.
    fn after_spinner<C: Commands>(&mut self, held: Held, now: SystemTime, commands: Option<&mut C>) {
        let loading =
            matches!(self.showing, Showing::Servers { loading: true, .. } | Showing::Server { loading: true, .. });
        let due = self.spinner_since.map_or(now, |since| since + SPINNER_SHOWN);
        if loading && due > now {
            self.held = Some((due, held));
            return;
        }
        self.show(held, commands);
    }

    /// The time: an answer held for the spinner shows once its second is up. Whether anything showed.
    pub fn advance(&mut self, now: SystemTime, commands: &mut impl Commands) -> bool {
        match self.held.take() {
            Some((due, held)) if due <= now => {
                self.show(held, Some(commands));
                true
            }
            held => {
                self.held = held;
                false
            }
        }
    }

    /// When an answer held for the spinner is to show.
    pub fn next_timer(&self) -> Option<SystemTime> {
        self.held.as_ref().map(|(due, _)| *due)
    }

    fn show<C: Commands>(&mut self, held: Held, commands: Option<&mut C>) {
        match held {
            Held::Servers(servers) => {
                if let Showing::Servers { .. } = self.showing {
                    self.servers = Some(servers.clone());
                    self.showing = Showing::Servers { loading: false, items: servers };
                }
            }
            Held::Channels(data) => {
                let Showing::Server { id, name, items: shown, .. } = &self.showing else { return };
                let (id, name) = (id.clone(), name.clone());
                let channels: Vec<ChannelEntry> = data["channels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["type"].as_u64().is_some_and(|kind| VOICE.contains(&kind)))
                    .filter_map(|c| {
                        let id = c["id"].as_str()?.to_string();
                        let occupants = shown.iter().find(|k| k.id == id).and_then(|k| k.occupants.clone());
                        Some(ChannelEntry { name: c["name"].as_str().unwrap_or_default().to_string(), id, occupants })
                    })
                    .collect();
                let names: Vec<String> = channels.iter().map(|c| format!("{:?}", c.name)).collect();
                let names = if names.is_empty() { "none".into() } else { names.join(", ") };
                self.say(format!("voice channels of {name:?}: {names}"));
                self.channels.insert(id.clone(), channels.clone());
                if let Some(commands) = commands {
                    for channel in &channels {
                        if let Some(nonce) = commands.send("GET_CHANNEL", json!({ "channel_id": channel.id }), None) {
                            self.occupant_requests.insert(nonce);
                        }
                    }
                }
                self.showing = Showing::Server { id, name, loading: false, items: channels };
            }
        }
    }

    /// A channel Discord sent, for whichever reason: if a voice channel showing is it, it shows the avatars of the
    /// people in it, none when it's empty.
    pub fn show_occupants(&mut self, channel: &Value) {
        let Some(id) = channel["id"].as_str() else { return };
        let Showing::Server { id: server, items, .. } = &mut self.showing else { return };
        let Some(shown) = items.iter_mut().find(|c| c.id == id) else { return };
        let people: Vec<Participant> = channel["voice_states"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Participant::from_voice_state)
            .collect();
        let occupants: Vec<(Avatar, String)> = people.iter().map(|p| (p.avatar.clone(), p.name.clone())).collect();
        if shown.occupants.as_ref() != Some(&occupants) {
            let who: Vec<&str> = people.iter().map(|p| p.name.as_str()).collect();
            let who = if who.is_empty() { "nobody".to_string() } else { who.join(", ") };
            let line = format!("in {:?}: {who}", shown.name);
            shown.occupants = Some(occupants.clone());
            if let Some(kept) = self.channels.get_mut(server.as_str()).and_then(|c| c.iter_mut().find(|c| c.id == id)) {
                kept.occupants = Some(occupants);
            }
            self.say(line);
        }
    }

    /// Discord refused the servers or a server's channels: a Flyout loading them stops, with none; one showing those
    /// answered before keeps them.
    pub fn on_refused(&mut self, cmd: &str, nonce: &str) {
        let servers = match cmd {
            "GET_GUILDS" => true,
            "GET_CHANNELS" => false,
            _ => return,
        };
        if !self.awaited(servers, nonce) {
            return;
        }
        self.request = None;
        match &mut self.showing {
            Showing::Servers { loading, .. } | Showing::Server { loading, .. } => *loading = false,
            Showing::Closed => {}
        }
    }

    /// The name of the voice channel `id` in the server's Flyout showing, for the log.
    pub fn name_of(&self, id: &str) -> Option<String> {
        match &self.showing {
            Showing::Server { items, .. } => items.iter().find(|c| c.id == id).map(|c| c.name.clone()),
            _ => None,
        }
    }

    /// What the Flyout shows, each picture as `avatar` has it: the cached file, or None until it is.
    pub fn view(&self, mut avatar: impl FnMut(&Avatar, &str) -> Option<std::path::PathBuf>) -> Browse {
        match &self.showing {
            Showing::Closed => Browse::Closed,
            Showing::Servers { loading, items } => Browse::Servers {
                loading: *loading,
                servers: items
                    .iter()
                    .map(|s| Server {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        icon: s.icon.as_deref().and_then(Avatar::from_url).and_then(|icon| avatar(&icon, &s.name)),
                    })
                    .collect(),
            },
            Showing::Server { id, name, loading, items } => Browse::Server {
                id: id.clone(),
                name: name.clone(),
                loading: *loading,
                channels: items
                    .iter()
                    .map(|c| VoiceChannel {
                        id: c.id.clone(),
                        name: c.name.clone(),
                        occupants: c
                            .occupants
                            .as_ref()
                            .map(|people| people.iter().map(|(face, name)| avatar(face, name)).collect()),
                    })
                    .collect(),
            },
        }
    }

    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    fn say(&mut self, line: String) {
        self.log.push(format!("discord: {line}"));
    }
}

/// No commands to send: an answer of the servers asks nothing more.
struct NoCommands;

impl Commands for NoCommands {
    fn send(&mut self, _: &str, _: Value, _: Option<&str>) -> Option<String> {
        None
    }
}

/// The servers in the Flyout's order: those joined in, most recently first, then the rest by name, whatever its
/// case.
fn in_order(mut servers: Vec<ServerEntry>, known: &Known) -> Vec<ServerEntry> {
    servers.sort_by_cached_key(|s| match known.server_rank(&s.id) {
        Some(rank) => (rank, String::new()),
        None => (usize::MAX, s.name.to_lowercase()),
    });
    servers
}
