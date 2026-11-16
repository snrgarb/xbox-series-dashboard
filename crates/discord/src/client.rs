//! The connection's logic, ported from the Bundle's `ExtensionCore.Discord.cs`, `ExtensionCore.DiscordTokens.cs` and
//! `ExtensionCore.DiscordReconnect.cs`: the user's own application kept in the secret store; Connect (the handshake,
//! AUTHORIZE, the token exchange, AUTHENTICATE, SUBSCRIBE); the tokens refreshed a day before they expire, Discord
//! asked to authorize again when a refresh won't do, the user asked to reconnect when that fails too; and Discord out
//! of reach tried again with the Bundle's back-off (2, 4, 8, 16, 30 s, then every minute).
//!
//! It does no I/O and reads no clock. Frames reach it through [`Client::on_frame`] and the pipe closing through
//! [`Client::on_closed`], the token endpoint's answers through [`Client::on_token_response`], and the time through
//! [`Client::advance`]; it acts through the [`Transport`], [`Tokens`] and [`Secrets`] it is given. What happens on the
//! connection once signed in (the Call, Shortcuts, Servers) is for whoever takes its [`Event`]s and sends its
//! commands ([`Client::send`]).
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use model::discord::{self as words, Discord};
use serde_json::{json, Value};

use crate::frame::Opcode;

/// Discord's local RPC pipe. It only moves raw frames: the protocol, PING included, is the client's.
pub trait Transport {
    /// Connects to the first of Discord's pipes that answers, closing any connection already open. False when none
    /// does: Discord isn't running.
    fn open(&mut self) -> bool;
    /// Sends one frame. Ignored while closed. A send that fails ends the connection as Discord closing it would.
    fn send(&mut self, opcode: Opcode, json: &str);
    /// Closes the connection, if open. [`Client::on_closed`] is not called for it.
    fn close(&mut self);
}

/// Discord's OAuth2 token endpoint. The answer, its HTTP status and body, comes back through
/// [`Client::on_token_response`]: status 0, with what went wrong as the body, when no answer came.
pub trait Tokens {
    /// POSTs `form` form-encoded, authenticated as the user's own application with HTTP Basic `client_id:secret`.
    fn request(&mut self, client_id: &str, secret: &str, form: Vec<(&'static str, String)>);
}

/// Values kept encrypted for the Windows user (DPAPI), never in the library database: the Client ID and Secret and
/// the tokens. Named values; each write is saved at once.
pub trait Secrets {
    fn read(&self, name: &str) -> Option<String>;
    fn write(&mut self, name: &str, value: &str);
    fn delete(&mut self, name: &str);
}

// The secret store's names, the Bundle's.
pub const CLIENT_ID: &str = "Discord.ClientId";
pub const CLIENT_SECRET: &str = "Discord.ClientSecret";
pub const ACCESS_TOKEN: &str = "Discord.AccessToken";
pub const REFRESH_TOKEN: &str = "Discord.RefreshToken";
/// When the tokens expire, in milliseconds since 1970.
pub const EXPIRES_AT: &str = "Discord.ExpiresAt";

/// The redirect the user adds under OAuth2, which the token exchange has to name. AUTHORIZE leaves it out, as
/// Discord refuses one there and takes the app's own. Nothing listens there: AUTHORIZE hands the code back itself.
pub const REDIRECT_URI: &str = "http://127.0.0.1";

pub const SCOPES: [&str; 5] = ["rpc", "identify", "rpc.voice.read", "rpc.voice.write", "rpc.notifications.read"];

/// The voice events followed from signing in, one SUBSCRIBE each.
pub const VOICE_EVENTS: [&str; 3] = ["VOICE_CHANNEL_SELECT", "VOICE_CONNECTION_STATUS", "VOICE_SETTINGS_UPDATE"];

/// How long after Discord is found out of reach the pipe is tried again: soon at first, as when Discord is only
/// restarting, then less often, holding at a minute for as long as it stays away. Signing in starts it afresh.
pub const PROBE_DELAYS: [Duration; 6] = [
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(16),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

/// How long, signing in with the saved tokens, Discord has to say READY once the pipe opens, before it is closed
/// and tried again later: Discord still starting, or hung, may never say it. Connect waits as long as it takes.
pub const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long before the tokens expire they are refreshed. Discord's last a week.
pub const REFRESH_MARGIN: Duration = Duration::from_secs(24 * 60 * 60);

/// The soonest the tokens are refreshed again after signing in or renewing them: tokens granted with less than a
/// day to run, or of no known expiry, would otherwise be refreshed at once, again and again.
pub const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// How long a refresh the token endpoint didn't answer, or answered busy or failing, waits to be tried again.
pub const REFRESH_RETRY: Duration = Duration::from_secs(60);

/// The Discord user signed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: String,
    /// The display name, else the username.
    pub name: String,
}

/// What happened on the connection, for whoever follows what Discord says once signed in (the Discord tab, #144).
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// AUTHENTICATE answered: signed in, the first time or again after Discord went. The voice events are
    /// subscribed to already; everything else that belongs to the connection (the selected voice channel, the
    /// per-channel events, the known DMs) is to be asked for afresh.
    SignedIn(User),
    /// An event Discord sent while signed in: `evt` and its `data`.
    Dispatch { evt: String, data: Value },
    /// Discord's answer to a command sent with [`Client::send`].
    Reply { cmd: String, nonce: String, data: Value },
    /// Discord refused a command sent with [`Client::send`]; the connection stays up.
    Refused { cmd: String, nonce: String, message: String },
    /// The connection ended, or the connect under way: everything that belonged to it is gone.
    Closed,
}

/// Where Connect has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Not connected, nor connecting: frames and token answers are ignored.
    Closed,
    /// The handshake sent, waiting for Discord's READY.
    Handshake,
    /// AUTHORIZE sent: the user approves the request in Discord.
    Authorize,
    /// The code sent to the token endpoint, waiting for its answer.
    TokenExchange,
    /// The saved refresh token sent to the token endpoint, waiting for its answer.
    Refresh,
    /// AUTHENTICATE sent.
    Authenticate,
    /// Signed in.
    Connected,
}

/// The one timer the tokens have: the refresh due a day before they expire, or one tried again after it went
/// unanswered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshTimer {
    Due,
    Retry,
}

/// Milliseconds since 1970: the client's time.
type Ms = i64;

fn ms(time: SystemTime) -> Ms {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_millis() as Ms,
        Err(before) => -(before.duration().as_millis() as Ms),
    }
}

fn dur(duration: Duration) -> Ms {
    duration.as_millis() as Ms
}

pub struct Client<T, O, S> {
    transport: T,
    tokens: O,
    secrets: S,
    now: Ms,
    view: Discord,
    step: Step,
    /// The saved access token to sign in with once Discord is READY, or None for Connect, which AUTHORIZEs first.
    /// It stays set through the sign-in, refreshed or not, marking it as one with saved tokens.
    sign_in_token: Option<String>,
    /// The commands sent and not yet answered, by nonce, which Discord's replies carry back.
    awaiting: HashMap<String, String>,
    last_nonce: u64,
    user: Option<User>,
    /// Signed in, how far renewing the tokens in the background has got: Refresh, Authorize or TokenExchange, and
    /// Connected while nothing is under way. The connection stays up throughout.
    renewal: Step,
    /// Whether this sign-in, or this renewal, has refreshed the tokens already, so a refused token isn't refreshed
    /// twice.
    refresh_tried: bool,
    /// Whether AUTHORIZE was sent to stand in for tokens that wouldn't do, rather than by Connect: failing, the user
    /// has to reconnect.
    reauthorizing: bool,
    refresh_timer: Option<(Ms, RefreshTimer)>,
    /// The refresh token the last refresh sent, until its answer comes. Discord replaces the refresh token with each
    /// refresh, so an answer granting new tokens is kept even when it comes after the connection closed.
    refresh_sent_for: Option<String>,
    /// The next try of the pipe, while Discord is out of reach; never more than one.
    probe_at: Option<Ms>,
    /// The end of the wait for READY, signing in with the saved tokens.
    ready_by: Option<Ms>,
    /// The tries since Discord last signed in, which pick the next one's delay.
    probes: usize,
    events: Vec<Event>,
    log: Vec<String>,
}

impl<T: Transport, O: Tokens, S: Secrets> Client<T, O, S> {
    /// The client over the application and tokens saved before. Nothing is tried until [`Client::start`].
    pub fn new(transport: T, tokens: O, secrets: S, now: SystemTime) -> Self {
        let view = Discord {
            client_id: secrets.read(CLIENT_ID),
            has_secret: secrets.read(CLIENT_SECRET).is_some(),
            has_tokens: secrets.read(ACCESS_TOKEN).is_some(),
            ..Discord::default()
        };
        Self {
            transport,
            tokens,
            secrets,
            now: ms(now),
            view,
            step: Step::Closed,
            sign_in_token: None,
            awaiting: HashMap::new(),
            last_nonce: 0,
            user: None,
            renewal: Step::Connected,
            refresh_tried: false,
            reauthorizing: false,
            refresh_timer: None,
            refresh_sent_for: None,
            probe_at: None,
            ready_by: None,
            probes: 0,
            events: Vec::new(),
            log: Vec::new(),
        }
    }

    /// What Settings' Discord page shows.
    pub fn discord(&self) -> &Discord {
        &self.view
    }

    /// The user signed in, while connected.
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref().filter(|_| self.step == Step::Connected)
    }

    /// Whether Discord has signed the user in: commands may be sent.
    pub fn connected(&self) -> bool {
        self.step == Step::Connected
    }

    pub fn transport(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn tokens(&mut self) -> &mut O {
        &mut self.tokens
    }

    pub fn secrets(&self) -> &S {
        &self.secrets
    }

    /// The secret store, for what else is kept in it: the Known channels (docs/discord.md).
    pub fn secrets_mut(&mut self) -> &mut S {
        &mut self.secrets
    }

    /// What happened on the connection since this was last asked.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// The log's lines since this was last asked (`discord: connecting`, ...). Nothing secret is ever in one.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    fn say(&mut self, line: String) {
        self.log.push(format!("discord: {line}"));
    }

    /// The Dashboard has started: with tokens saved from before, Discord is signed in with them, without asking
    /// again.
    pub fn start(&mut self) {
        self.sign_in_with_saved_tokens();
    }

    /// The Client ID typed or pasted: saved at once, trimmed; empty forgets it.
    pub fn set_client_id(&mut self, text: &str) {
        self.view.client_id = self.save(CLIENT_ID, text);
        let what = if self.view.client_id.is_some() { "set" } else { "forgotten" };
        self.say(format!("client id {what}"));
    }

    /// The Client Secret typed or pasted: saved at once, trimmed, and never shown; empty forgets it.
    pub fn set_client_secret(&mut self, text: &str) {
        self.view.has_secret = self.save(CLIENT_SECRET, text).is_some();
        let what = if self.view.has_secret { "saved" } else { "forgotten" };
        self.say(format!("client secret {what}"));
    }

    fn save(&mut self, name: &str, text: &str) -> Option<String> {
        let value = text.trim();
        if value.is_empty() {
            self.secrets.delete(name);
            None
        } else {
            self.secrets.write(name, value);
            Some(value.to_string())
        }
    }

    /// Settings' Connect: the handshake with Discord, then its Authorize prompt for the user to approve, the token
    /// exchange and signing in. Does nothing without a Client ID and Secret, or with a connect under way.
    pub fn connect(&mut self) {
        if self.view.can_connect() {
            self.start_discord(None);
        }
    }

    /// Settings' Disconnect: closes the connection and forgets the tokens, and with them any need to reconnect.
    /// The Client ID and Secret stay. With nothing to disconnect it leaves the status line saying what last went
    /// wrong.
    pub fn disconnect(&mut self) {
        if !self.view.can_disconnect() {
            return;
        }
        for name in [ACCESS_TOKEN, REFRESH_TOKEN, EXPIRES_AT] {
            self.secrets.delete(name);
        }
        self.view.has_tokens = false;
        self.view.needs_reconnect = false;
        self.say("disconnected: the tokens are forgotten".into());
        self.close_discord(words::NOT_CONNECTED, false);
    }

    /// Sends a command while signed in, under a nonce of its own that Discord's reply carries back, and gives the
    /// nonce: its answer comes as [`Event::Reply`] or [`Event::Refused`]. None while not signed in.
    pub fn send(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Option<String> {
        (self.step == Step::Connected).then(|| self.send_command(cmd, args, evt))
    }

    /// The soonest moment something is timed for: a try of the pipe, the end of the wait for READY, a refresh.
    pub fn next_timer(&self) -> Option<SystemTime> {
        let at = [self.probe_at, self.ready_by, self.refresh_timer.map(|(at, _)| at)].into_iter().flatten().min()?;
        Some(UNIX_EPOCH + Duration::from_millis(at.max(0) as u64))
    }

    /// The time is now `now`: what was timed for until then is done, in order, each at its own moment.
    pub fn advance(&mut self, now: SystemTime) {
        let now = ms(now);
        loop {
            let due = [
                self.probe_at.map(|at| (at, 0)),
                self.ready_by.map(|at| (at, 1)),
                self.refresh_timer.map(|(at, _)| (at, 2)),
            ];
            let Some((at, which)) = due.into_iter().flatten().filter(|&(at, _)| at <= now).min() else { break };
            self.now = self.now.max(at);
            match which {
                0 => {
                    self.probe_at = None;
                    self.sign_in_with_saved_tokens();
                }
                1 => {
                    self.ready_by = None;
                    self.say("no READY in 10 s".into());
                    self.close_discord(words::NOT_RUNNING, true);
                }
                _ => {
                    let (_, kind) = self.refresh_timer.take().expect("due");
                    match kind {
                        RefreshTimer::Due if self.step == Step::Connected && self.renewal == Step::Connected => {
                            self.say("refreshing the tokens, a day before they expire".into());
                            self.refresh_tokens();
                        }
                        RefreshTimer::Retry if self.token_step() == Step::Refresh => self.refresh_tokens(),
                        _ => {}
                    }
                }
            }
        }
        self.now = self.now.max(now);
    }

    /// Opens the pipe and says hello; once Discord is READY, signs in with the saved `token`, refreshed first when
    /// near expiring, or AUTHORIZEs first with none. Discord not running, or, signing in with the saved token, not
    /// READY in time, it is tried again later while tokens are saved.
    fn start_discord(&mut self, token: Option<String>) {
        self.forget_renewal();
        self.probe_at = None;
        self.ready_by = None;
        self.view.retrying = false;
        if !self.transport.open() {
            self.say("not running".into());
            self.close_discord(words::NOT_RUNNING, true);
            return;
        }
        self.say("connecting".into());
        let waits = token.is_some();
        self.sign_in_token = token;
        self.awaiting.clear();
        self.step = Step::Handshake;
        self.view.connecting = true;
        self.view.status = words::CONNECTING.into();
        let hello = json!({ "v": 1, "client_id": self.view.client_id });
        self.transport.send(Opcode::Handshake, &hello.to_string());
        if waits {
            self.ready_by = Some(self.now + dur(READY_TIMEOUT));
        }
    }

    /// One frame from Discord's pipe, raw: its opcode and JSON.
    pub fn on_frame(&mut self, opcode: Opcode, json: &str) {
        if self.step == Step::Closed {
            return;
        }
        if opcode == Opcode::Ping {
            self.transport.send(Opcode::Pong, json);
            return;
        }
        let frame: Option<Value> = serde_json::from_str(json).ok().filter(Value::is_object);
        if opcode == Opcode::Close {
            let message = frame.as_ref().and_then(|f| f["message"].as_str()).unwrap_or_default();
            let status = if message.is_empty() {
                "Discord closed the connection".to_string()
            } else {
                format!("Discord closed the connection: {message}")
            };
            self.close_discord(&status, true);
            return;
        }
        let Some(mut frame) = frame.filter(|_| opcode == Opcode::Frame) else { return };
        let cmd = frame["cmd"].as_str().unwrap_or_default().to_string();
        let evt = frame["evt"].as_str().map(str::to_string);
        let data = frame["data"].take();
        if cmd == "DISPATCH" {
            if evt.as_deref() == Some("READY") && self.step == Step::Handshake {
                self.on_ready();
            } else if self.step == Step::Connected {
                self.events.push(Event::Dispatch { evt: evt.unwrap_or_default(), data });
            }
            return;
        }
        let nonce = frame["nonce"].as_str().unwrap_or_default().to_string();
        let Some(sent) = self.awaiting.remove(&nonce) else { return };
        if evt.as_deref() == Some("ERROR") {
            let message = data["message"].as_str().unwrap_or_default().to_string();
            // Refused while connecting, or authorizing again while connected, signing in has failed; once signed in,
            // as a voice setting Discord won't change, the connection stays up.
            if self.step != Step::Connected || (sent == "AUTHORIZE" && self.renewal == Step::Authorize) {
                self.on_sign_in_refused(&sent, &message);
            } else {
                self.events.push(Event::Refused { cmd: sent, nonce, message });
            }
            return;
        }
        if sent == "AUTHORIZE" && self.token_step() == Step::Authorize {
            self.set_token_step(Step::TokenExchange);
            if self.step != Step::Connected {
                self.view.status = words::CONNECTING.into();
            }
            let code = data["code"].as_str().unwrap_or_default().to_string();
            let form = vec![
                ("grant_type", "authorization_code".to_string()),
                ("code", code),
                ("redirect_uri", REDIRECT_URI.to_string()),
            ];
            self.say("approved in Discord: exchanging the code for tokens".into());
            self.request_tokens(form);
        } else if sent == "AUTHENTICATE" && self.step == Step::Authenticate {
            self.on_signed_in(&data["user"]);
        } else if self.step == Step::Connected {
            self.events.push(Event::Reply { cmd: sent, nonce, data });
        }
    }

    fn request_tokens(&mut self, form: Vec<(&'static str, String)>) {
        let id = self.view.client_id.clone().unwrap_or_default();
        let secret = self.secrets.read(CLIENT_SECRET).unwrap_or_default();
        self.tokens.request(&id, &secret, form);
    }

    fn on_ready(&mut self) {
        self.ready_by = None;
        if self.sign_in_token.is_some() {
            self.on_ready_with_saved_token();
        } else {
            self.send_authorize();
        }
    }

    /// The token endpoint's answer: its HTTP status and body, or status 0 with what went wrong when no answer came.
    /// Granted, the tokens are saved and Discord signed in with them; an answer nothing is waiting for, as after
    /// Disconnect, is ignored.
    pub fn on_token_response(&mut self, status: u16, body: &str) {
        let step = self.token_step();
        if step == Step::Refresh {
            let answer = if status == 0 { None } else { parse(body) };
            self.on_refresh_answer(status, answer);
            return;
        }
        if step != Step::TokenExchange {
            self.keep_late_refresh(status, body);
            return;
        }
        if status == 0 {
            self.on_tokens_refused(&format!("The token request failed: {body}"));
            return;
        }
        let answer = parse(body).unwrap_or(Value::Null);
        if !(200..=299).contains(&status) {
            let error = answer["error_description"].as_str().or(answer["error"].as_str());
            let error = error.map_or_else(|| format!("HTTP {status}"), str::to_string);
            self.on_tokens_refused(&format!("Couldn't connect to Discord: {error}"));
            return;
        }
        if answer["access_token"].as_str().is_none() {
            self.on_tokens_refused("Couldn't connect to Discord: no access token came back");
            return;
        }
        self.on_tokens_granted(&answer);
    }

    fn authenticate(&mut self, token: &str) {
        self.step = Step::Authenticate;
        self.send_command("AUTHENTICATE", json!({ "access_token": token }), None);
    }

    /// AUTHENTICATE answered: the status line says who, any need to reconnect is gone, the voice events are
    /// followed, and the tokens' refresh is set. Signed in again after Discord went, it is all done afresh.
    fn on_signed_in(&mut self, user: &Value) {
        self.step = Step::Connected;
        self.view.needs_reconnect = false;
        self.probe_at = None;
        self.probes = 0;
        self.view.retrying = false;
        self.schedule_refresh();
        let id = user["id"].as_str().unwrap_or_default().to_string();
        let name = user["global_name"].as_str().filter(|name| !name.is_empty()).or(user["username"].as_str());
        let name = name.unwrap_or_default().to_string();
        self.view.connecting = false;
        self.view.connected = true;
        self.view.status = format!("Connected as {name}");
        self.view.user = Some(name.clone());
        self.say(format!("connected as {name:?}"));
        for event in VOICE_EVENTS {
            self.send_command("SUBSCRIBE", json!({}), Some(event));
        }
        let user = User { id, name };
        self.user = Some(user.clone());
        self.events.push(Event::SignedIn(user));
    }

    /// Discord's pipe closed by itself: Discord quit, or dropped the connection. With tokens saved, Discord is
    /// waited for, and signed in again once back.
    pub fn on_closed(&mut self) {
        if self.step != Step::Closed {
            let status = if self.reconnects_by_itself() { words::NOT_RUNNING } else { "Discord closed the connection" };
            self.close_discord(status, true);
        }
    }

    /// Ends the connection, or the connect under way, with `status` on the status line, and stops trying Discord
    /// again, unless `try_again`, Discord being out of reach, when it is tried again later while tokens are saved.
    fn close_discord(&mut self, status: &str, try_again: bool) {
        let was = self.step;
        self.transport.close();
        self.awaiting.clear();
        self.probe_at = None;
        self.ready_by = None;
        self.view.retrying = false;
        self.step = Step::Closed;
        self.sign_in_token = None;
        self.forget_renewal();
        self.view.connecting = false;
        self.view.status = status.to_string();
        self.view.connected = false;
        self.view.user = None;
        if was != Step::Closed {
            self.events.push(Event::Closed);
            if status != words::NOT_CONNECTED {
                self.say(format!("lost ({status})"));
            }
        }
        if try_again {
            self.probe_later();
        }
    }

    /// Sends a command, under a nonce of its own that Discord's reply carries back, and returns the nonce.
    fn send_command(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> String {
        self.last_nonce += 1;
        let nonce = self.last_nonce.to_string();
        self.awaiting.insert(nonce.clone(), cmd.to_string());
        let mut frame = json!({ "cmd": cmd, "args": args, "nonce": nonce });
        if let Some(evt) = evt {
            frame["evt"] = Value::String(evt.to_string());
        }
        self.transport.send(Opcode::Frame, &frame.to_string());
        nonce
    }

    // ---------------------------------------------------------------------------------------------------- tokens

    /// Where the tokens have got to: the step of signing in, or, once signed in, of renewing them.
    fn token_step(&self) -> Step {
        if self.step == Step::Connected {
            self.renewal
        } else {
            self.step
        }
    }

    fn set_token_step(&mut self, step: Step) {
        if self.step == Step::Connected {
            self.renewal = step;
        } else {
            self.step = step;
        }
    }

    /// Signs in with the tokens saved from before, without asking the user again; does nothing without them or a
    /// Client ID.
    fn sign_in_with_saved_tokens(&mut self) {
        if let (Some(token), Some(_)) = (self.secrets.read(ACCESS_TOKEN), &self.view.client_id) {
            self.start_discord(Some(token));
        }
    }

    /// Discord READY for the saved token: signed in with it, refreshed first when it is a day or less from
    /// expiring, or of no known expiry.
    fn on_ready_with_saved_token(&mut self) {
        match self.expires_at() {
            Some(at) if at - self.now > dur(REFRESH_MARGIN) => {
                let token = self.sign_in_token.clone().unwrap_or_default();
                self.authenticate(&token);
            }
            _ => {
                self.say("the tokens are a day or less from expiring: refreshing them first".into());
                self.refresh_tokens();
            }
        }
    }

    /// The saved tokens' expiry, or None for none, or none that reads as a time.
    fn expires_at(&self) -> Option<Ms> {
        self.secrets.read(EXPIRES_AT)?.trim().parse().ok()
    }

    /// Asks the token endpoint for new tokens for the saved refresh token; without one, Discord is asked to
    /// authorize again.
    fn refresh_tokens(&mut self) {
        self.refresh_tried = true;
        let refresh = self.secrets.read(REFRESH_TOKEN).filter(|token| !token.is_empty());
        let Some(refresh) = refresh else {
            self.reauthorize();
            return;
        };
        self.set_token_step(Step::Refresh);
        self.refresh_sent_for = Some(refresh.clone());
        self.request_tokens(vec![("grant_type", "refresh_token".to_string()), ("refresh_token", refresh)]);
    }

    /// Sends AUTHORIZE: Discord shows its prompt for the user to approve, and the code it gives back is exchanged
    /// for tokens.
    fn send_authorize(&mut self) {
        self.set_token_step(Step::Authorize);
        if self.step != Step::Connected {
            self.view.status = words::APPROVE.into();
        }
        self.say("asking Discord to authorize: approve the request in Discord".into());
        let client_id = self.view.client_id.clone();
        self.send_command("AUTHORIZE", json!({ "client_id": client_id, "scopes": SCOPES }), None);
    }

    /// The tokens won't do and a refresh hasn't helped: Discord is asked to authorize again, as Connect does.
    fn reauthorize(&mut self) {
        self.reauthorizing = true;
        self.send_authorize();
    }

    /// Signing in, or AUTHORIZE while connected, refused with `message`. A saved token refused is refreshed, or,
    /// refreshed already, authorized again; authorizing again refused, the user has to reconnect.
    fn on_sign_in_refused(&mut self, sent: &str, message: &str) {
        if self.reauthorizing {
            self.need_reconnect();
        } else if sent == "AUTHENTICATE" && self.sign_in_token.is_some() {
            self.say(format!("Discord refused the saved token ({message})"));
            if self.refresh_tried {
                self.reauthorize();
            } else {
                self.refresh_tokens();
            }
        } else {
            self.close_discord(&format!("Couldn't connect to Discord: {message}"), false);
        }
    }

    /// The token endpoint's answer to a refresh. Granted, the tokens are saved and Discord signed in with them, or,
    /// signed in already, the next refresh set; not answered, or answered busy or failing, tried again a minute
    /// later, as the refresh token may well still be good; refused, Discord is asked to authorize again.
    fn on_refresh_answer(&mut self, status: u16, answer: Option<Value>) {
        self.refresh_sent_for = None;
        if is_transient(status) {
            self.say(format!("the token refresh went unanswered ({}): trying again in 60 s", describe(status)));
            self.refresh_timer = Some((self.now + dur(REFRESH_RETRY), RefreshTimer::Retry));
            return;
        }
        let answer = answer.filter(|answer| (200..=299).contains(&status) && answer["access_token"].is_string());
        match answer {
            Some(answer) => self.on_tokens_granted(&answer),
            None => {
                self.say(format!("the token refresh was refused ({})", describe(status)));
                self.reauthorize();
            }
        }
    }

    /// An answer that came with no refresh waiting for it. One granting new tokens for the refresh token still
    /// saved, sent before the connection closed, is kept: Discord has replaced that refresh token, so dropping the
    /// answer would leave a dead one saved. After Disconnect, or a Connect that saved others, the saved one differs.
    fn keep_late_refresh(&mut self, status: u16, body: &str) {
        let Some(sent) = self.refresh_sent_for.take() else { return };
        if !(200..=299).contains(&status) || self.secrets.read(REFRESH_TOKEN).as_deref() != Some(sent.as_str()) {
            return;
        }
        if let Some(answer) = parse(body).filter(|answer| answer["access_token"].is_string()) {
            self.save_tokens(&answer);
        }
    }

    fn save_tokens(&mut self, answer: &Value) {
        let access = answer["access_token"].as_str().unwrap_or_default().to_string();
        self.secrets.write(ACCESS_TOKEN, &access);
        let refresh = answer["refresh_token"].as_str().map(str::to_string);
        let refresh = refresh.or_else(|| self.secrets.read(REFRESH_TOKEN)).unwrap_or_default();
        self.secrets.write(REFRESH_TOKEN, &refresh);
        let expires_in = answer["expires_in"].as_f64().unwrap_or(0.0);
        let at = self.now + (expires_in * 1000.0) as Ms;
        self.secrets.write(EXPIRES_AT, &at.to_string());
        self.view.has_tokens = true;
        self.say("tokens saved".into());
    }

    /// Tokens granted, by the code exchange or a refresh: saved, then signed in with, or, signed in already,
    /// refreshed in their turn.
    fn on_tokens_granted(&mut self, answer: &Value) {
        self.save_tokens(answer);
        if self.step == Step::Connected {
            self.renewal = Step::Connected;
            self.schedule_refresh();
        } else {
            let access = answer["access_token"].as_str().unwrap_or_default().to_string();
            self.authenticate(&access);
        }
    }

    /// The token exchange, or authorizing again, failed with `status`: standing in for the tokens, the user has to
    /// reconnect.
    fn on_tokens_refused(&mut self, status: &str) {
        if self.reauthorizing {
            self.need_reconnect();
        } else {
            self.close_discord(status, false);
        }
    }

    /// Signed in, or the tokens renewed: sets the refresh due a day before the saved tokens expire, or, when they
    /// are nearer than that, [`MIN_REFRESH_INTERVAL`] from now.
    fn schedule_refresh(&mut self) {
        self.refresh_tried = false;
        self.reauthorizing = false;
        let due = self.expires_at().unwrap_or(self.now) - dur(REFRESH_MARGIN) - self.now;
        let due = due.max(dur(MIN_REFRESH_INTERVAL));
        self.refresh_timer = Some((self.now + due, RefreshTimer::Due));
    }

    /// Neither the saved tokens nor authorizing again would do: the connection closes, Discord isn't tried again by
    /// itself, and Settings and the Discord tab ask the user to reconnect. The tokens stay saved until Connect
    /// replaces them or Disconnect forgets them.
    fn need_reconnect(&mut self) {
        self.view.needs_reconnect = true;
        self.say("the saved tokens won't do and Discord won't authorize again: reconnect in Settings".into());
        self.close_discord(words::RECONNECT_IN_SETTINGS, false);
    }

    /// Forgets the renewal under way and the refresh due, as the connection closes.
    fn forget_renewal(&mut self) {
        self.refresh_timer = None;
        self.renewal = Step::Connected;
        self.refresh_tried = false;
        self.reauthorizing = false;
    }

    // ------------------------------------------------------------------------------------------------- reconnect

    /// Whether Discord out of reach is tried again: with tokens saved, so not after Disconnect nor before the
    /// first Connect, and while they may still do, so not once the user has to reconnect.
    fn reconnects_by_itself(&self) -> bool {
        self.view.has_tokens && !self.view.needs_reconnect
    }

    /// Discord is out of reach: the pipe is tried again after the next delay, signing in with the saved tokens.
    fn probe_later(&mut self) {
        self.probe_at = None;
        if !self.reconnects_by_itself() {
            return;
        }
        let delay = PROBE_DELAYS[self.probes.min(PROBE_DELAYS.len() - 1)];
        self.probes += 1;
        self.probe_at = Some(self.now + dur(delay));
        self.view.retrying = true;
        self.say(format!("retry in {} s", delay.as_secs()));
    }
}

fn parse(body: &str) -> Option<Value> {
    serde_json::from_str::<Value>(body).ok().filter(Value::is_object)
}

/// No answer (status 0), a timeout, too many requests, or the server failing: none says the refresh token is bad.
fn is_transient(status: u16) -> bool {
    status == 0 || status == 408 || status == 429 || status >= 500
}

fn describe(status: u16) -> String {
    if status == 0 {
        "no answer".into()
    } else {
        format!("HTTP {status}")
    }
}
