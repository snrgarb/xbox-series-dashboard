//! Everything followed of Discord once signed in, from the same events: the Call ([`voice::Follower`]), the Known
//! channels it teaches ([`Known`]) and the Servers Flyouts ([`Browser`]); and what the Discord tab asks of them. A
//! GET_CHANNEL sent to learn who is in a server's voice channel is told by its nonce, so it never shows as the Call;
//! every channel Discord sends shows who is in it in a server's Flyout.
//!
//! Pure, as its parts are: the [`crate::Connection`] hands it the client's events and the time, sends what it asks
//! through the client, and saves what it hands back ([`Following::take_unsaved`]) in the secret store.
use std::path::PathBuf;
use std::time::SystemTime;

use model::discord::{Ask, Browse, BrowseAsk, Shortcut, Voice, CALL};
use serde_json::json;

use crate::client::Event;
use crate::servers::Browser;
use crate::shortcuts::Known;
use crate::voice::{self, Avatar, Commands};

#[derive(Debug, Default)]
pub struct Following {
    pub voice: voice::Follower,
    pub known: Known,
    pub servers: Browser,
    log: Vec<String>,
}

impl Following {
    /// Following nothing yet, with the Known channels saved before.
    pub fn new(known: Known) -> Self {
        Following { known, ..Following::default() }
    }

    /// What Discord said or did, with whatever it calls for sent through `commands`.
    pub fn on_event(&mut self, event: Event, commands: &mut impl Commands, now: SystemTime) {
        match &event {
            Event::Reply { cmd, nonce, data } if cmd == "GET_CHANNEL" => {
                let occupants_only = self.servers.take_occupant_request(nonce);
                let data = data.clone();
                if !occupants_only {
                    self.voice.on_event(event, commands);
                }
                self.servers.show_occupants(&data);
            }
            Event::Refused { cmd, nonce, message } if cmd == "GET_CHANNEL" => {
                if self.servers.take_occupant_request(nonce) {
                    self.say(format!("Discord refused GET_CHANNEL: {message}"));
                } else {
                    self.voice.on_event(event, commands);
                }
            }
            Event::Reply { cmd, data, .. } if cmd == "GET_GUILD" => {
                self.known.on_guild(data);
            }
            Event::Reply { cmd, nonce, data } if cmd == "GET_GUILDS" => {
                self.servers.on_guilds(nonce, data, &self.known, now);
            }
            Event::Reply { cmd, nonce, data } if cmd == "GET_CHANNELS" => {
                let (nonce, data) = (nonce.clone(), data.clone());
                self.servers.on_channels(&nonce, &data, now, commands);
            }
            Event::Refused { cmd, nonce, .. } if cmd == "GET_GUILDS" || cmd == "GET_CHANNELS" => {
                let (cmd, nonce) = (cmd.clone(), nonce.clone());
                self.voice.on_event(event, commands);
                self.servers.on_refused(&cmd, &nonce);
            }
            Event::Closed => {
                self.voice.on_event(event, commands);
                self.servers.forget();
            }
            _ => self.voice.on_event(event, commands),
        }
        self.learn(commands);
    }

    /// Each Call begun becomes a Known channel, at the front, its server asked for its name and icon; the Call's
    /// Known channel takes the name and picture it lacked.
    fn learn(&mut self, commands: &mut impl Commands) {
        for joined in self.voice.take_joined() {
            if let Some(guild) = self.known.learn(&joined) {
                commands.send("GET_GUILD", json!({ "guild_id": guild }), None);
            }
        }
        self.known.remember_call(self.voice.call_joined().as_ref());
    }

    /// What the Discord tab asks that is Discord's to carry out: Mute, Deafen and Leave, joining a Shortcut or a
    /// server's voice channel, Remove shortcut, and the Servers Flyouts. False for anything else (Settings').
    pub fn ask(&mut self, ask: &Ask, commands: &mut impl Commands, now: SystemTime) -> bool {
        match ask {
            Ask::Call(ask) => self.voice.ask(*ask, commands),
            Ask::Join(id) => {
                let known = self.known.channels().iter().find(|c| c.id == *id).map(|c| c.name.clone());
                let name = self.servers.name_of(id).or(known.flatten()).unwrap_or_else(|| CALL.to_string());
                self.voice.join(id, &name, commands);
                self.servers.close();
            }
            Ask::RemoveShortcut(id) => {
                let known = self.known.channels().iter().find(|c| c.id == *id).map(|c| c.name.clone());
                let name = known.flatten().unwrap_or_else(|| CALL.to_string());
                if self.known.forget(id) {
                    self.say(format!("shortcut {name:?} removed"));
                }
            }
            Ask::Browse(BrowseAsk::Servers) => self.servers.open_servers(&self.known, now, commands),
            Ask::Browse(BrowseAsk::Server { id, name }) => self.servers.open_server(id, name, now, commands),
            Ask::Browse(BrowseAsk::Back) => self.servers.back(&self.known),
            Ask::Browse(BrowseAsk::Close) => self.servers.close(),
            Ask::ClientId(_) | Ask::ClientSecret(_) | Ask::Paste(_) | Ask::Connect | Ask::Disconnect => return false,
        }
        true
    }

    /// The time: an answer held for a Flyout's spinner shows once its second is up. Whether anything changed.
    pub fn advance(&mut self, now: SystemTime, commands: &mut impl Commands) -> bool {
        self.servers.advance(now, commands)
    }

    /// When the time is next wanted.
    pub fn next_timer(&self) -> Option<SystemTime> {
        self.servers.next_timer()
    }

    /// What has changed of the Known channels since last asked, for the secret store: each name with its value.
    pub fn take_unsaved(&mut self) -> Vec<(&'static str, String)> {
        self.known.take_unsaved()
    }

    /// The Call, the Shortcuts and the Servers Flyout as the screens show them, each picture as `avatar` has it: the
    /// cached file, or None until it is.
    pub fn view(&self, mut avatar: impl FnMut(&Avatar, &str) -> Option<PathBuf>) -> (Voice, Vec<Shortcut>, Browse) {
        let voice = self.voice.voice(&mut avatar);
        let shortcuts = self
            .known
            .shortcuts()
            .iter()
            .map(|known| {
                let name = known.name.clone().unwrap_or_else(|| CALL.to_string());
                Shortcut {
                    id: known.id.clone(),
                    picture: known.picture().and_then(|picture| avatar(&picture, &name)),
                    name,
                    server: known.server_name.clone(),
                    dm: known.guild.is_none(),
                }
            })
            .collect();
        let browse = self.servers.view(&mut avatar);
        (voice, shortcuts, browse)
    }

    /// The log's lines since this was last asked.
    pub fn take_log(&mut self) -> Vec<String> {
        let mut lines = self.voice.take_log();
        lines.append(&mut self.servers.take_log());
        lines.append(&mut self.log);
        lines
    }

    fn say(&mut self, line: String) {
        self.log.push(format!("discord: {line}"));
    }
}
