//! Discord's local RPC, for the user's own Discord application (docs/discord.md): the [`Client`] that holds the
//! connection's logic, ported from the Bundle, over the real pipe ([`ipc::Pipe`]), token endpoint
//! ([`tokens::HttpTokens`]) and secret store ([`secrets::FileSecrets`], DPAPI on Windows), put together as a
//! [`Connection`] that the Dashboard's loop drives, following the Call ([`voice::Follower`]), the Known channels
//! ([`shortcuts::Known`]) and the Servers Flyouts ([`servers::Browser`]) together ([`following::Following`]), and
//! fetching their pictures ([`avatars::Avatars`]); and a fake Discord for tests ([`fake::FakeDiscord`]).
pub mod avatars;
pub mod client;
pub mod fake;
pub mod following;
pub mod frame;
pub mod ipc;
pub mod secrets;
pub mod servers;
pub mod shortcuts;
pub mod tokens;
pub mod voice;
#[cfg(windows)]
mod win;

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::SystemTime;

use avatars::Avatars;
pub use client::{Client, Event, Secrets, Tokens, Transport, User};
pub use frame::Opcode;
use ipc::{Incoming, Pipe};
use model::discord::{Ask, Discord, Field};
use secrets::{AnySecrets, FileSecrets, MemorySecrets};
use tokens::HttpTokens;
use following::Following;
use shortcuts::{Known, RECENT_SERVERS_SECRET, SHORTCUTS_SECRET};

/// Where the Dashboard's Discord is: the real one, or a test's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Place {
    /// The one pipe to try, in place of Discord's `discord-ipc-0` to `-9`: a test's (`standin discord`).
    pub pipe: Option<String>,
    /// The token endpoint, in place of Discord's. With a test's pipe and none, there is none: a test never reaches
    /// the real Discord.
    pub token_url: Option<String>,
    /// The CDN avatars are fetched from, in place of Discord's: a test's (`standin discord` serves plain pictures).
    /// With a test's pipe and none, there is none, and no avatar is fetched.
    pub cdn_url: Option<String>,
}

/// The connection as the Dashboard has it: the client over the real pipe, token endpoint and secrets, and what
/// their threads hand the loop.
pub struct Connection {
    client: Client<Pipe, HttpTokens, AnySecrets>,
    incoming: Receiver<Incoming>,
    /// The Call, the Known channels and the Servers Flyouts, followed from what the client hands over once signed
    /// in.
    following: Following,
    avatars: Avatars,
    /// The client's view with the Call in it: what the screens are given.
    view: Discord,
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
        // A test's pipe reaches no service of the real Discord's that the test did not name.
        let real = |given: &Option<String>, discords: &str| match (&place.pipe, given) {
            (_, Some(url)) => Some(url.clone()),
            (Some(_), None) => None,
            (None, None) => Some(discords.to_string()),
        };
        let token_url = real(&place.token_url, tokens::DISCORD);
        let cdn_url = real(&place.cdn_url, voice::CDN);
        let avatars = Avatars::new(data_dir.map(|dir| dir.join("art").join("discord")), cdn_url, wake.clone());
        let secrets = match data_dir {
            Some(dir) => AnySecrets::File(FileSecrets::open(&dir.join(secrets::FILE), secrets::system())),
            None => AnySecrets::Memory(MemorySecrets::default()),
        };
        let pipe = Pipe::new(names, sender.clone(), wake.clone());
        let tokens = HttpTokens::new(token_url, sender, wake);
        let client = Client::new(pipe, tokens, secrets, SystemTime::now());
        let saved = |name: &str| client.secrets().read(name);
        let known = Known::load(saved(SHORTCUTS_SECRET).as_deref(), saved(RECENT_SERVERS_SECRET).as_deref());
        let following = Following::new(known);
        let view = client.discord().clone();
        let mut connection = Self { client, incoming, following, avatars, view };
        connection.follow(SystemTime::now());
        connection
    }

    /// The Dashboard has started: Discord is signed in with the tokens saved before, if any.
    pub fn start(&mut self) {
        self.client.start();
        self.follow(SystemTime::now());
    }

    /// What Settings' Discord page and the Guide's Discord tab show: the connection, and the Call.
    pub fn discord(&self) -> &Discord {
        &self.view
    }

    /// The client, for whoever follows more of the connection once signed in: its commands ([`Client::send`]).
    /// Its events are the Connection's, which hands them to what it follows ([`Following`]).
    pub fn client(&mut self) -> &mut Client<Pipe, HttpTokens, AnySecrets> {
        &mut self.client
    }

    /// When the loop next has to [`Connection::pump`], whatever comes: the client's next try or refresh, or an
    /// answer held for a Flyout's spinner.
    pub fn next_timer(&self) -> Option<SystemTime> {
        [self.client.next_timer(), self.following.next_timer()].into_iter().flatten().min()
    }

    /// Takes in what the pipe, the token endpoint and the avatars' thread have handed over, and the time: whether
    /// what the screens show changed.
    pub fn pump(&mut self, now: SystemTime) -> bool {
        let before = self.view.clone();
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
        self.follow(now);
        self.view != before
    }

    /// Does what Settings' Discord page or the Discord tab asks; Paste reads the clipboard with `clipboard`, and a
    /// clipboard with no text pastes nothing.
    pub fn ask(&mut self, ask: Ask, clipboard: impl FnOnce() -> Option<String>) {
        let now = SystemTime::now();
        if self.following.ask(&ask, &mut self.client, now) {
            self.follow(now);
            return;
        }
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
            // The Discord tab's: carried out above.
            Ask::Call(_) | Ask::Join(_) | Ask::RemoveShortcut(_) | Ask::Browse(_) => {}
        }
        self.follow(now);
    }

    /// Hands what the client says to what is followed, saves the Known channels that changed, takes in the pictures
    /// that arrived, and makes the view afresh.
    fn follow(&mut self, now: SystemTime) {
        for event in self.client.take_events() {
            self.following.on_event(event, &mut self.client, now);
        }
        self.following.advance(now, &mut self.client);
        for (name, value) in self.following.take_unsaved() {
            self.client.secrets_mut().write(name, &value);
        }
        self.avatars.pump();
        let avatars = &mut self.avatars;
        let (voice, shortcuts, browse) = self.following.view(|avatar, name| avatars.get(avatar, name));
        self.view = Discord { voice, shortcuts, browse, ..self.client.discord().clone() };
        self.say();
    }

    fn say(&mut self) {
        let lines = [self.client.take_log(), self.following.take_log(), self.avatars.take_log()];
        for line in lines.into_iter().flatten() {
            eprintln!("{line}");
        }
    }
}
