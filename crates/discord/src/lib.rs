//! Discord's local RPC, for the user's own Discord application (docs/discord.md): the [`Client`] that holds the
//! connection's logic, ported from the Bundle, over the real pipe ([`ipc::Pipe`]), token endpoint
//! ([`tokens::HttpTokens`]) and secret store ([`secrets::FileSecrets`], DPAPI on Windows), put together as a
//! [`Connection`] that the Dashboard's loop drives; and a fake Discord for tests ([`fake::FakeDiscord`]).
pub mod client;
pub mod fake;
pub mod frame;
pub mod ipc;
pub mod secrets;
pub mod tokens;
#[cfg(windows)]
mod win;

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::SystemTime;

pub use client::{Client, Event, Secrets, Tokens, Transport, User};
pub use frame::Opcode;
use ipc::{Incoming, Pipe};
use model::discord::{Ask, Discord, Field};
use secrets::{AnySecrets, FileSecrets, MemorySecrets};
use tokens::HttpTokens;

/// Where the Dashboard's Discord is: the real one, or a test's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Place {
    /// The one pipe to try, in place of Discord's `discord-ipc-0` to `-9`: a test's (`standin discord`).
    pub pipe: Option<String>,
    /// The token endpoint, in place of Discord's. With a test's pipe and none, there is none: a test never reaches
    /// the real Discord.
    pub token_url: Option<String>,
}

/// The connection as the Dashboard has it: the client over the real pipe, token endpoint and secrets, and what
/// their threads hand the loop.
pub struct Connection {
    client: Client<Pipe, HttpTokens, AnySecrets>,
    incoming: Receiver<Incoming>,
}

impl Connection {
    /// The connection over the secrets in `data_dir` (in memory with none), waking the loop with `wake` when
    /// something comes. Nothing is tried until [`Connection::start`].
    pub fn new(place: &Place, data_dir: Option<&Path>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let wake: ipc::Wake = Arc::new(wake);
        let (sender, incoming) = mpsc::channel();
        let names = match &place.pipe {
            Some(pipe) => {
                eprintln!("discord: a test's pipe, {pipe}, in place of Discord's");
                vec![pipe.clone()]
            }
            None => ipc::discord_names(),
        };
        let token_url = match (&place.pipe, &place.token_url) {
            (_, Some(url)) => Some(url.clone()),
            (Some(_), None) => None,
            (None, None) => Some(tokens::DISCORD.to_string()),
        };
        let secrets = match data_dir {
            Some(dir) => AnySecrets::File(FileSecrets::open(&dir.join(secrets::FILE), secrets::system())),
            None => AnySecrets::Memory(MemorySecrets::default()),
        };
        let pipe = Pipe::new(names, sender.clone(), wake.clone());
        let tokens = HttpTokens::new(token_url, sender, wake);
        let client = Client::new(pipe, tokens, secrets, SystemTime::now());
        let mut connection = Self { client, incoming };
        connection.say();
        connection
    }

    /// The Dashboard has started: Discord is signed in with the tokens saved before, if any.
    pub fn start(&mut self) {
        self.client.start();
        self.say();
    }

    /// What Settings' Discord page shows.
    pub fn discord(&self) -> &Discord {
        self.client.discord()
    }

    /// The client, for whoever follows the connection once signed in (the Discord tab, #144): its commands
    /// ([`Client::send`]) and events ([`Client::take_events`]).
    pub fn client(&mut self) -> &mut Client<Pipe, HttpTokens, AnySecrets> {
        &mut self.client
    }

    /// When the loop next has to [`Connection::pump`], whatever comes.
    pub fn next_timer(&self) -> Option<SystemTime> {
        self.client.next_timer()
    }

    /// Takes in what the pipe and the token endpoint have handed over, and the time: whether what Settings shows
    /// changed.
    pub fn pump(&mut self, now: SystemTime) -> bool {
        let before = self.client.discord().clone();
        while let Ok(incoming) = self.incoming.try_recv() {
            match incoming {
                Incoming::Frame(generation, opcode, json) => {
                    if self.client.transport().is_current(generation) {
                        self.client.on_frame(opcode, &json);
                    }
                }
                Incoming::Closed(generation) => {
                    if self.client.transport().is_current(generation) {
                        self.client.transport().ended(generation);
                        self.client.on_closed();
                    }
                }
                Incoming::Token(status, body) => self.client.on_token_response(status, &body),
            }
        }
        self.client.advance(now);
        self.say();
        *self.client.discord() != before
    }

    /// Does what Settings' Discord page asks; Paste reads the clipboard with `clipboard`, and a clipboard with no
    /// text pastes nothing.
    pub fn ask(&mut self, ask: Ask, clipboard: impl FnOnce() -> Option<String>) {
        match ask {
            Ask::ClientId(text) => self.client.set_client_id(&text),
            Ask::ClientSecret(text) => self.client.set_client_secret(&text),
            Ask::Paste(field) => match clipboard() {
                Some(text) if field == Field::ClientId => self.client.set_client_id(&text),
                Some(text) => self.client.set_client_secret(&text),
                None => eprintln!("discord: the clipboard has no text: nothing pasted"),
            },
            Ask::Connect => self.client.connect(),
            Ask::Disconnect => self.client.disconnect(),
        }
        self.say();
    }

    fn say(&mut self) {
        for line in self.client.take_log() {
            eprintln!("{line}");
        }
    }
}
