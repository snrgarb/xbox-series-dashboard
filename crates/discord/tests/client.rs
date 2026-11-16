//! The connection's logic against a fake transport, token endpoint and secret store: the Bundle's DiscordTests,
//! DiscordTokenTests and DiscordReconnectTests, as far as they are the connection's (the Discord tab's are #144's).
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord::client::{Client, Event, Secrets, Tokens, Transport};
use discord::secrets::MemorySecrets;
use discord::Opcode;
use model::discord::Discord;
use serde_json::{json, Value};

const CLIENT_ID: &str = "1290000000000000001";
const SECRET: &str = "s3cr3t-s3cr3t-s3cr3t-s3cr3t-0001";
const ME: &str = "100000000000000001";

#[derive(Default)]
struct FakeTransport {
    not_running: bool,
    open: bool,
    opens: usize,
    sent: Vec<(Opcode, String)>,
}

impl Transport for FakeTransport {
    fn open(&mut self) -> bool {
        self.open = false;
        self.opens += 1;
        self.open = !self.not_running;
        self.open
    }

    fn send(&mut self, opcode: Opcode, json: &str) {
        if self.open {
            self.sent.push((opcode, json.to_string()));
        }
    }

    fn close(&mut self) {
        self.open = false;
    }
}

impl FakeTransport {
    fn commands(&self) -> Vec<Value> {
        let frames = self.sent.iter().filter(|(opcode, _)| *opcode == Opcode::Frame);
        frames.map(|(_, json)| serde_json::from_str(json).unwrap()).collect()
    }

    fn last(&self, cmd: &str) -> Option<Value> {
        self.commands().into_iter().rev().find(|c| c["cmd"] == cmd)
    }

    fn count(&self, cmd: &str) -> usize {
        self.commands().iter().filter(|c| c["cmd"] == cmd).count()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Request {
    client_id: String,
    secret: String,
    form: Vec<(&'static str, String)>,
}

impl Request {
    fn get(&self, key: &str) -> Option<&str> {
        self.form.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str())
    }
}

#[derive(Default)]
struct FakeTokens {
    requests: Vec<Request>,
}

impl Tokens for FakeTokens {
    fn request(&mut self, client_id: &str, secret: &str, form: Vec<(&'static str, String)>) {
        self.requests.push(Request { client_id: client_id.into(), secret: secret.into(), form });
    }
}

type Core = Client<FakeTransport, FakeTokens, MemorySecrets>;

struct Test {
    core: Core,
    now: SystemTime,
}

fn start_time() -> SystemTime {
    // 2026-10-14 20:00 UTC.
    UNIX_EPOCH + Duration::from_secs(1_792_008_000)
}

impl Test {
    fn new() -> Self {
        let now = start_time();
        Test { core: Client::new(FakeTransport::default(), FakeTokens::default(), MemorySecrets::default(), now), now }
    }

    /// A core made afresh over the same secret store, as when the Dashboard starts again.
    fn new_core(&mut self) {
        let secrets = self.core.secrets().clone();
        self.core = Client::new(FakeTransport::default(), FakeTokens::default(), secrets, self.now);
    }

    fn view(&self) -> &Discord {
        self.core.discord()
    }

    fn transport(&mut self) -> &mut FakeTransport {
        self.core.transport()
    }

    fn requests(&mut self) -> &Vec<Request> {
        &self.core.tokens().requests
    }

    fn secret(&self, name: &str) -> Option<String> {
        self.core.secrets().read(name)
    }

    fn advance(&mut self, by: Duration) {
        self.now += by;
        self.core.advance(self.now);
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn enter_app(&mut self) {
        self.core.set_client_id(CLIENT_ID);
        self.core.set_client_secret(SECRET);
    }

    fn save_tokens(&mut self, expires_at: SystemTime) {
        let at = expires_at.duration_since(UNIX_EPOCH).unwrap().as_millis().to_string();
        let mut secrets = self.core.secrets().clone();
        for (name, value) in [
            ("Discord.ClientId", CLIENT_ID),
            ("Discord.ClientSecret", SECRET),
            ("Discord.AccessToken", "saved-access"),
            ("Discord.RefreshToken", "saved-refresh"),
            ("Discord.ExpiresAt", &at),
        ] {
            secrets.write(name, value);
        }
        self.core = Client::new(FakeTransport::default(), FakeTokens::default(), secrets, self.now);
    }

    fn ready(&mut self) {
        let ready = r#"{"cmd":"DISPATCH","evt":"READY","data":{"v":1,"config":{}},"nonce":null}"#;
        self.core.on_frame(Opcode::Frame, ready);
    }

    fn reply(&mut self, cmd: &str, data: Value) {
        let nonce = self.transport().last(cmd).unwrap()["nonce"].clone();
        let frame = json!({ "cmd": cmd, "evt": null, "data": data, "nonce": nonce });
        self.core.on_frame(Opcode::Frame, &frame.to_string());
    }

    fn error(&mut self, cmd: &str, message: &str) {
        let nonce = self.transport().last(cmd).unwrap()["nonce"].clone();
        let data = json!({ "code": 5000, "message": message });
        let frame = json!({ "cmd": cmd, "evt": "ERROR", "data": data, "nonce": nonce });
        self.core.on_frame(Opcode::Frame, &frame.to_string());
    }

    fn tokens(&mut self, status: u16, body: &str) {
        self.core.on_token_response(status, body);
    }

    fn sign_in(&mut self) {
        let user = json!({ "user": { "id": ME, "username": "gabe", "global_name": null }, "scopes": ["rpc"] });
        self.reply("AUTHENTICATE", user);
    }

    fn authorize_and_await_token(&mut self) {
        self.enter_app();
        self.core.connect();
        self.ready();
        self.reply("AUTHORIZE", json!({ "code": "the-code" }));
    }

    /// Connect, all the way through: the Authorize prompt approved, the tokens granted and signed in.
    fn connect_fully(&mut self) {
        if !self.view().connecting {
            self.enter_app();
            self.core.connect();
        }
        self.ready();
        self.reply("AUTHORIZE", json!({ "code": "the-code" }));
        self.tokens(200, &tokens("access-1", "refresh-1", Some(604800)));
        self.sign_in();
    }

    /// The Dashboard starting again over the saved tokens, as far as Discord's READY.
    fn start_up(&mut self) {
        self.new_core();
        self.core.start();
        self.ready();
    }

    fn start_up_with_discord_not_running(&mut self) {
        self.save_tokens(self.now + Duration::from_secs(6 * 86400));
        self.transport().not_running = true;
        self.core.start();
    }

    fn assert_failed(&mut self, status: &str) {
        assert_eq!(self.view().status, status);
        assert!(!self.transport().open);
        assert!(!self.view().connecting);
        assert!(!self.view().connected);
        assert!(self.view().can_connect());
    }

    fn assert_saved(&self, access: &str, refresh: &str, expires_at: SystemTime) {
        assert_eq!(self.secret("Discord.AccessToken").as_deref(), Some(access));
        assert_eq!(self.secret("Discord.RefreshToken").as_deref(), Some(refresh));
        let at = expires_at.duration_since(UNIX_EPOCH).unwrap().as_millis().to_string();
        assert_eq!(self.secret("Discord.ExpiresAt"), Some(at));
    }

    fn assert_needs_reconnect(&mut self) {
        assert!(self.view().needs_reconnect);
        assert_eq!(self.view().status, "Reconnect Discord in settings");
        assert!(!self.transport().open);
        assert!(!self.view().connecting && !self.view().connected);
        assert!(self.view().can_connect() && self.view().can_disconnect());
        assert!(self.secret("Discord.AccessToken").is_some());
        assert_eq!(self.view().tab_line(), Some("Reconnect Discord in settings"));
    }

    fn need_reconnect(&mut self) {
        self.save_tokens(self.now + Duration::from_secs(7 * 86400));
        self.start_up();
        self.error("AUTHENTICATE", "Invalid access token");
        self.tokens(400, r#"{"error":"invalid_grant"}"#);
        self.error("AUTHORIZE", "OAuth2 Authorization cancelled");
    }
}

fn tokens(access: &str, refresh: &str, expires_in: Option<u64>) -> String {
    let mut answer =
        json!({ "access_token": access, "token_type": "Bearer", "refresh_token": refresh, "scope": "rpc" });
    if let Some(expires_in) = expires_in {
        answer["expires_in"] = json!(expires_in);
    }
    answer.to_string()
}

const DAY: Duration = Duration::from_secs(86400);

// ---------------------------------------------------------------------------------------------- DiscordTests

#[test]
fn settings_start_not_connected_with_connect_unavailable() {
    let t = Test::new();
    assert_eq!(t.view().client_id, None);
    assert!(!t.view().has_secret);
    assert_eq!(t.view().status, "Not connected");
    assert!(!t.view().connected && !t.view().connecting);
    assert!(!t.view().can_connect() && !t.view().can_disconnect());
    assert!(model::discord::STEPS.iter().any(|s| s.contains("OAuth2") && s.contains("http://127.0.0.1")));
    assert!(model::discord::STEPS.iter().any(|s| s.contains("App Testers")));
}

#[test]
fn connect_needs_both_the_client_id_and_the_client_secret() {
    let mut t = Test::new();
    t.core.set_client_id(CLIENT_ID);
    assert!(!t.view().can_connect());
    t.core.connect();
    assert_eq!(t.transport().opens, 0);
    t.core.set_client_secret(SECRET);
    assert!(t.view().can_connect());
}

#[test]
fn the_client_id_and_secret_are_trimmed_and_saved_and_the_secret_is_never_shown() {
    let mut t = Test::new();
    t.core.set_client_id(&format!("  {CLIENT_ID}\r\n"));
    t.core.set_client_secret(&format!(" {SECRET}\n"));
    assert_eq!(t.view().client_id.as_deref(), Some(CLIENT_ID));
    assert!(t.view().has_secret);
    assert_eq!(t.secret("Discord.ClientId").as_deref(), Some(CLIENT_ID));
    assert_eq!(t.secret("Discord.ClientSecret").as_deref(), Some(SECRET));
    assert!(!format!("{:?}", t.view()).contains(SECRET));
    assert!(t.core.take_log().iter().all(|line| !line.contains(SECRET) && !line.contains(CLIENT_ID)));
}

#[test]
fn pasting_nothing_forgets_the_client_id_and_secret() {
    let mut t = Test::new();
    t.enter_app();
    t.core.set_client_id("  ");
    t.core.set_client_secret("");
    assert_eq!(t.view().client_id, None);
    assert!(!t.view().has_secret);
    assert!(t.core.secrets().values.is_empty());
}

#[test]
fn the_client_id_and_secret_come_back_from_the_secret_store_next_time() {
    let mut t = Test::new();
    t.enter_app();
    t.new_core();
    assert_eq!(t.view().client_id.as_deref(), Some(CLIENT_ID));
    assert!(t.view().has_secret && t.view().can_connect());
}

#[test]
fn disconnect_with_nothing_to_disconnect_leaves_the_status_line_as_it_was() {
    let mut t = Test::new();
    t.transport().not_running = true;
    t.enter_app();
    t.core.connect();
    t.core.disconnect();
    assert_eq!(t.view().status, "Discord isn't running");
}

#[test]
fn connect_with_discord_not_running_says_so() {
    let mut t = Test::new();
    t.transport().not_running = true;
    t.enter_app();
    t.core.connect();
    assert_eq!(t.view().status, "Discord isn't running");
    assert!(!t.view().connecting && t.view().can_connect());
    assert!(t.transport().sent.is_empty());
}

#[test]
fn connect_says_hello_with_the_client_id_then_asks_for_authorization_once_discord_is_ready() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    assert!(t.view().connecting);
    assert_eq!(t.view().status, "Connecting...");
    assert!(!t.view().can_connect() && t.view().can_disconnect());
    let (opcode, hello) = t.transport().sent[0].clone();
    assert_eq!(t.transport().sent.len(), 1);
    assert_eq!(opcode, Opcode::Handshake);
    assert_eq!(serde_json::from_str::<Value>(&hello).unwrap(), json!({ "v": 1, "client_id": CLIENT_ID }));

    t.ready();

    let authorize = t.transport().last("AUTHORIZE").unwrap();
    assert_eq!(authorize["args"]["client_id"], CLIENT_ID);
    assert_eq!(
        authorize["args"]["scopes"],
        json!(["rpc", "identify", "rpc.voice.read", "rpc.voice.write", "rpc.notifications.read"])
    );
    // Discord refuses a redirect in RPC's AUTHORIZE; it takes the one registered on the app.
    assert!(authorize["args"].get("redirect_uri").is_none());
    assert!(authorize["nonce"].as_str().is_some_and(|nonce| !nonce.is_empty()));
    assert_eq!(t.view().status, "Approve the request in Discord");
}

#[test]
fn approving_in_discord_exchanges_the_code_for_tokens_as_the_users_own_app() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    let request = t.requests()[0].clone();
    assert_eq!(t.requests().len(), 1);
    assert_eq!((request.client_id.as_str(), request.secret.as_str()), (CLIENT_ID, SECRET));
    let form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", "the-code".to_string()),
        ("redirect_uri", "http://127.0.0.1".to_string()),
    ];
    assert_eq!(request.form, form);
    assert!(t.view().connecting);
    assert_eq!(t.view().status, "Connecting...");
}

#[test]
fn the_tokens_are_saved_and_used_to_sign_in_then_the_status_says_who_is_connected() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    t.tokens(200, &tokens("access-1", "refresh-1", Some(604800)));
    t.assert_saved("access-1", "refresh-1", t.now + Duration::from_secs(604800));
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "access-1");
    assert!(!t.view().connected);

    let user = json!({ "user": { "id": "1", "username": "gabe", "global_name": "Gabe" } });
    t.reply("AUTHENTICATE", user);

    assert_eq!(t.view().status, "Connected as Gabe");
    assert!(t.view().connected && !t.view().connecting);
    assert!(!t.view().can_connect() && t.view().can_disconnect());
    assert_eq!(t.view().user.as_deref(), Some("Gabe"));
    let log = t.core.take_log();
    assert!(log.contains(&"discord: connected as \"Gabe\"".to_string()), "{log:?}");
    assert!(log.iter().all(|line| !line.contains("access-1") && !line.contains("refresh-1") && !line.contains(SECRET)));
}

#[test]
fn connected_as_falls_back_to_the_username_without_a_display_name() {
    let mut t = Test::new();
    t.connect_fully();
    assert_eq!(t.view().status, "Connected as gabe");
}

#[test]
fn once_signed_in_it_subscribes_to_the_voice_events_one_frame_each_and_says_so() {
    let mut t = Test::new();
    t.connect_fully();
    let subscribes: Vec<Value> = t.transport().commands().into_iter().filter(|c| c["cmd"] == "SUBSCRIBE").collect();
    let events: Vec<&str> = subscribes.iter().map(|c| c["evt"].as_str().unwrap()).collect();
    assert_eq!(events, ["VOICE_CHANNEL_SELECT", "VOICE_CONNECTION_STATUS", "VOICE_SETTINGS_UPDATE"]);
    let user = discord::User { id: ME.into(), name: "gabe".into() };
    assert_eq!(t.core.take_events(), [Event::SignedIn(user)]);
}

#[test]
fn every_command_carries_its_own_nonce() {
    let mut t = Test::new();
    t.connect_fully();
    let nonces: Vec<String> = t.transport().commands().iter().map(|c| c["nonce"].as_str().unwrap().into()).collect();
    let mut unique = nonces.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(nonces.len(), unique.len());
}

#[test]
fn discords_pings_are_answered_with_the_same_payload() {
    for connected in [false, true] {
        let mut t = Test::new();
        if connected {
            t.connect_fully();
        } else {
            t.enter_app();
            t.core.connect();
        }
        t.core.on_frame(Opcode::Ping, r#"{"t":42}"#);
        assert_eq!(t.transport().sent.last().unwrap(), &(Opcode::Pong, r#"{"t":42}"#.to_string()));
    }
}

#[test]
fn a_reply_to_a_command_never_sent_is_ignored() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.ready();
    let stray = r#"{"cmd":"AUTHORIZE","evt":null,"data":{"code":"stray"},"nonce":"not-ours"}"#;
    t.core.on_frame(Opcode::Frame, stray);
    assert!(t.requests().is_empty());
}

#[test]
fn cancelling_the_authorize_prompt_in_discord_shows_why_and_closes_the_connection() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.ready();
    t.error("AUTHORIZE", "OAuth2 Authorization cancelled");
    t.assert_failed("Couldn't connect to Discord: OAuth2 Authorization cancelled");
    assert!(t.requests().is_empty());
}

#[test]
fn a_refused_token_request_shows_discords_description_of_the_error() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    t.tokens(401, r#"{"error":"invalid_client","error_description":"Invalid client secret"}"#);
    t.assert_failed("Couldn't connect to Discord: Invalid client secret");
    assert!(t.secret("Discord.AccessToken").is_none());
}

#[test]
fn a_refused_token_request_without_a_description_shows_the_error_code() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    t.tokens(400, r#"{"error":"invalid_grant"}"#);
    t.assert_failed("Couldn't connect to Discord: invalid_grant");
}

#[test]
fn a_token_request_that_got_no_answer_says_it_failed() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    t.tokens(0, "No such host is known.");
    t.assert_failed("The token request failed: No such host is known.");
}

#[test]
fn discord_closing_the_pipe_mid_handshake_shows_it_and_stops() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.core.on_closed();
    t.assert_failed("Discord closed the connection");
    assert_eq!(t.core.take_events(), [Event::Closed]);
}

#[test]
fn a_close_frame_from_discord_shows_its_message() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.core.on_frame(Opcode::Close, r#"{"code":4000,"message":"Invalid Client ID"}"#);
    t.assert_failed("Discord closed the connection: Invalid Client ID");
}

#[test]
fn a_token_answer_after_discord_closed_is_ignored() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    t.core.on_closed();
    t.tokens(200, &tokens("access-1", "refresh-1", Some(604800)));
    assert!(t.secret("Discord.AccessToken").is_none());
    assert_eq!(t.view().status, "Discord closed the connection");
}

#[test]
fn disconnect_closes_the_connection_and_forgets_the_tokens_but_keeps_the_app() {
    let mut t = Test::new();
    t.connect_fully();
    t.core.disconnect();
    assert!(!t.transport().open);
    let names: Vec<&String> = t.core.secrets().values.keys().collect();
    assert_eq!(names, ["Discord.ClientId", "Discord.ClientSecret"]);
    assert_eq!(t.view().status, "Not connected");
    assert!(!t.view().connected && t.view().can_connect() && !t.view().can_disconnect());
    assert!(!t.view().has_tab());
}

#[test]
fn frames_after_disconnect_are_ignored() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.core.disconnect();
    t.ready();
    assert!(t.transport().last("AUTHORIZE").is_none());
    assert_eq!(t.view().status, "Not connected");
}

#[test]
fn start_up_with_saved_tokens_signs_in_with_them_without_asking_again() {
    let mut t = Test::new();
    t.save_tokens(t.now + 6 * DAY);
    assert_eq!(t.transport().opens, 0);
    t.core.start();
    assert!(t.view().connecting);
    assert_eq!(t.view().tab_line(), Some("Connecting to Discord"));
    assert_eq!(t.transport().sent.len(), 1);
    assert_eq!(t.transport().sent[0].0, Opcode::Handshake);
    t.ready();
    assert!(t.transport().last("AUTHORIZE").is_none());
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "saved-access");
    t.sign_in();
    assert_eq!(t.view().status, "Connected as gabe");
    assert_eq!(t.transport().count("SUBSCRIBE"), 3);
    assert!(t.requests().is_empty());
}

#[test]
fn start_up_without_saved_tokens_leaves_discord_alone() {
    let mut t = Test::new();
    t.enter_app();
    t.core.start();
    assert_eq!(t.transport().opens, 0);
    assert_eq!(t.view().status, "Not connected");
}

#[test]
fn start_up_with_discord_not_running_says_so_and_keeps_the_tokens_and_the_discord_tab() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    assert_eq!(t.view().status, "Discord isn't running");
    assert!(!t.view().connected);
    assert_eq!(t.secret("Discord.AccessToken").as_deref(), Some("saved-access"));
    assert!(t.view().can_disconnect() && t.view().has_tab());
    assert_eq!(t.view().tab_line(), Some("Discord isn't running"));
}

#[test]
fn start_up_with_a_token_discord_refuses_refreshes_it_and_keeps_the_tokens() {
    let mut t = Test::new();
    t.save_tokens(t.now + 6 * DAY);
    t.start_up();
    t.error("AUTHENTICATE", "Invalid access token");
    assert_eq!(t.requests().len(), 1);
    assert_eq!(t.requests()[0].get("grant_type"), Some("refresh_token"));
    assert_eq!(t.secret("Discord.AccessToken").as_deref(), Some("saved-access"));
    assert!(t.view().has_tab());
}

#[test]
fn disconnect_after_a_failed_start_up_forgets_the_tokens() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    t.core.disconnect();
    for name in ["Discord.AccessToken", "Discord.RefreshToken", "Discord.ExpiresAt"] {
        assert!(t.secret(name).is_none());
    }
    assert!(!t.view().can_disconnect());
}

// ----------------------------------------------------------------------------------------- DiscordTokenTests

#[test]
fn start_up_with_tokens_a_day_from_expiring_refreshes_them_first_then_signs_in_with_the_new_ones() {
    let mut t = Test::new();
    t.save_tokens(t.now + 7 * DAY);
    t.now += DAY * 13 / 2;
    t.start_up();
    assert!(t.transport().last("AUTHENTICATE").is_none());
    assert!(t.transport().last("AUTHORIZE").is_none());
    let request = t.requests()[0].clone();
    assert_eq!((request.client_id.as_str(), request.secret.as_str()), (CLIENT_ID, SECRET));
    assert_eq!(request.form, vec![("grant_type", "refresh_token".into()), ("refresh_token", "saved-refresh".into())]);
    assert!(t.view().connecting);

    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));

    t.assert_saved("access-2", "refresh-2", t.now + Duration::from_secs(604800));
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "access-2");
    t.sign_in();
    assert!(t.view().connected);
}

#[test]
fn start_up_with_tokens_already_expired_or_of_no_known_expiry_refreshes_them_first() {
    for how in ["past", "none", "not a time"] {
        let mut t = Test::new();
        t.save_tokens(t.now + 7 * DAY);
        let mut secrets = t.core.secrets().clone();
        match how {
            "past" => t.now += 8 * DAY,
            "none" => secrets.delete("Discord.ExpiresAt"),
            _ => secrets.write("Discord.ExpiresAt", how),
        }
        t.core = Client::new(FakeTransport::default(), FakeTokens::default(), secrets, t.now);
        t.start_up();
        assert_eq!(t.requests().len(), 1, "{how}");
        assert_eq!(t.requests()[0].get("grant_type"), Some("refresh_token"));
        assert!(t.transport().last("AUTHENTICATE").is_none());
    }
}

#[test]
fn start_up_with_tokens_good_for_days_yet_signs_in_without_refreshing() {
    let mut t = Test::new();
    t.save_tokens(t.now + 7 * DAY);
    t.now += 5 * DAY;
    t.start_up();
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "saved-access");
    assert!(t.requests().is_empty());
}

#[test]
fn once_signed_in_the_tokens_are_refreshed_a_day_before_they_expire_over_the_open_connection() {
    let mut t = Test::new();
    t.connect_fully();
    let authenticates = t.transport().count("AUTHENTICATE");

    t.advance(6 * DAY - Duration::from_secs(60));
    assert_eq!(t.requests().len(), 1);
    t.advance(Duration::from_secs(60));
    let last = t.requests().last().unwrap().form.clone();
    assert_eq!(last, vec![("grant_type", "refresh_token".into()), ("refresh_token", "refresh-1".into())]);

    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));

    t.assert_saved("access-2", "refresh-2", t.now + Duration::from_secs(604800));
    assert_eq!(t.transport().count("AUTHENTICATE"), authenticates);
    assert!(t.transport().open && t.view().connected);
    assert_eq!(t.view().status, "Connected as gabe");

    // The new tokens are refreshed in their turn, a day before they expire.
    t.advance(6 * DAY);
    assert_eq!(t.requests().len(), 3);
    assert_eq!(t.requests().last().unwrap().get("refresh_token"), Some("refresh-2"));
}

#[test]
fn signing_in_after_a_start_up_refresh_refreshes_again_a_day_before_the_new_tokens_expire() {
    let mut t = Test::new();
    t.save_tokens(t.now + Duration::from_secs(3600));
    t.start_up();
    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));
    t.sign_in();
    t.advance(6 * DAY);
    assert_eq!(t.requests().len(), 2);
    assert_eq!(t.requests().last().unwrap().get("refresh_token"), Some("refresh-2"));
}

#[test]
fn tokens_granted_while_signed_in_without_a_day_to_run_are_refreshed_again_no_sooner_than_15_minutes_later() {
    for expires_in in [None, Some(0), Some(3600)] {
        let mut t = Test::new();
        t.connect_fully();
        t.advance(6 * DAY);
        assert_eq!(t.requests().len(), 2);
        t.tokens(200, &tokens("access-2", "refresh-2", expires_in));
        t.advance(Duration::from_secs(15 * 60) - Test::ms(1));
        assert_eq!(t.requests().len(), 2, "{expires_in:?}");
        t.advance(Test::ms(1));
        assert_eq!(t.requests().len(), 3);
        assert_eq!(t.requests().last().unwrap().get("refresh_token"), Some("refresh-2"));
    }
}

#[test]
fn signing_in_after_a_start_up_refresh_granting_no_expiry_refreshes_again_no_sooner_than_15_minutes_later() {
    let mut t = Test::new();
    t.save_tokens(t.now + Duration::from_secs(3600));
    t.start_up();
    t.tokens(200, &tokens("access-2", "refresh-2", None));
    t.sign_in();
    t.advance(Duration::from_secs(15 * 60) - Test::ms(1));
    assert_eq!(t.requests().len(), 1);
    t.advance(Test::ms(1));
    assert_eq!(t.requests().len(), 2);
}

#[test]
fn disconnect_stops_the_refresh_and_a_refresh_answer_after_it_is_ignored() {
    let mut t = Test::new();
    t.connect_fully();
    t.advance(6 * DAY);
    assert_eq!(t.requests().len(), 2);
    t.core.disconnect();
    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));
    assert!(t.secret("Discord.AccessToken").is_none());
    assert_eq!(t.view().status, "Not connected");
    assert!(!t.view().connected);
}

#[test]
fn disconnect_or_discord_closing_cancels_the_refresh_waiting_to_come() {
    let mut t = Test::new();
    t.connect_fully();
    t.core.disconnect();
    t.advance(8 * DAY);
    assert_eq!(t.requests().len(), 1);

    let mut t = Test::new();
    t.connect_fully();
    t.core.on_closed();
    t.transport().not_running = true;
    t.advance(8 * DAY);
    assert_eq!(t.requests().len(), 1);
    assert_eq!(t.secret("Discord.AccessToken").as_deref(), Some("access-1"));
}

#[test]
fn a_refresh_that_fails_at_start_up_asks_discord_to_authorize_again_then_signs_in() {
    for (status, body) in [
        (400, r#"{"error":"invalid_grant"}"#),
        (401, r#"{"error":"invalid_client"}"#),
        (200, r#"{"token_type":"Bearer"}"#),
    ] {
        let mut t = Test::new();
        t.save_tokens(t.now + Duration::from_secs(3600));
        t.start_up();
        t.tokens(status, body);
        assert_eq!(t.transport().last("AUTHORIZE").unwrap()["args"]["client_id"], CLIENT_ID);
        assert_eq!(t.view().status, "Approve the request in Discord");
        assert!(t.view().connecting);

        t.reply("AUTHORIZE", json!({ "code": "code-2" }));
        assert_eq!(t.requests().last().unwrap().get("code"), Some("code-2"));
        assert_eq!(t.requests().last().unwrap().get("redirect_uri"), Some("http://127.0.0.1"));
        t.tokens(200, &tokens("access-3", "refresh-3", Some(604800)));
        t.assert_saved("access-3", "refresh-3", t.now + Duration::from_secs(604800));
        assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "access-3");
        t.sign_in();
        assert!(t.view().connected && !t.view().needs_reconnect);
    }
}

#[test]
fn a_refresh_not_answered_at_start_up_is_tried_again_a_minute_later_rather_than_authorizing_again() {
    for (status, body) in [(0, "No such host is known."), (429, r#"{"message":"slow down"}"#), (503, "upstream")] {
        // As after waking from sleep, Discord's pipe back before the network.
        let mut t = Test::new();
        t.save_tokens(t.now + Duration::from_secs(3600));
        t.start_up();
        t.tokens(status, body);
        assert!(t.transport().last("AUTHORIZE").is_none());
        assert_eq!(t.requests().len(), 1);
        assert!(t.view().connecting);

        t.advance(Duration::from_secs(60));
        assert_eq!(t.requests().len(), 2);
        assert_eq!(t.requests().last().unwrap().get("refresh_token"), Some("saved-refresh"));
        t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));
        t.assert_saved("access-2", "refresh-2", t.now + Duration::from_secs(604800));
        assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "access-2");
        assert!(t.transport().last("AUTHORIZE").is_none());
    }
}

#[test]
fn a_refresh_not_answered_while_connected_is_tried_again_a_minute_later_and_stays_connected() {
    let mut t = Test::new();
    t.connect_fully();
    let authorizes = t.transport().count("AUTHORIZE");
    t.advance(6 * DAY);
    let requests = t.requests().len();
    t.tokens(0, "No such host is known.");
    assert_eq!(t.transport().count("AUTHORIZE"), authorizes);
    assert!(t.view().connected);
    t.advance(Duration::from_secs(60));
    assert_eq!(t.requests().len(), requests + 1);
    t.tokens(200, &tokens("access-3", "refresh-3", Some(604800)));
    t.assert_saved("access-3", "refresh-3", t.now + Duration::from_secs(604800));
    assert_eq!(t.transport().count("AUTHORIZE"), authorizes);
}

#[test]
fn a_refresh_answered_after_discord_closed_keeps_the_new_tokens() {
    // Discord replaces the refresh token with each refresh: the one sent is dead once the answer grants new ones.
    let mut t = Test::new();
    t.connect_fully();
    t.advance(6 * DAY);
    t.core.on_closed();
    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));
    t.assert_saved("access-2", "refresh-2", t.now + Duration::from_secs(604800));
    assert!(!t.view().connected);
}

#[test]
fn discord_closing_cancels_a_refresh_waiting_to_be_tried_again() {
    let mut t = Test::new();
    t.save_tokens(t.now + Duration::from_secs(3600));
    t.start_up();
    t.tokens(0, "No such host is known.");
    t.core.on_closed();
    t.transport().not_running = true;
    t.advance(Duration::from_secs(300));
    assert_eq!(t.requests().len(), 1);
}

#[test]
fn a_refresh_that_fails_while_connected_asks_discord_to_authorize_again_and_stays_connected() {
    let mut t = Test::new();
    t.connect_fully();
    let authenticates = t.transport().count("AUTHENTICATE");
    t.advance(6 * DAY);
    t.tokens(400, r#"{"error":"invalid_grant"}"#);
    assert_eq!(t.transport().count("AUTHORIZE"), 2);
    assert!(t.view().connected);
    assert_eq!(t.view().status, "Connected as gabe");

    t.reply("AUTHORIZE", json!({ "code": "code-2" }));
    t.tokens(200, &tokens("access-3", "refresh-3", Some(604800)));
    t.assert_saved("access-3", "refresh-3", t.now + Duration::from_secs(604800));
    assert_eq!(t.transport().count("AUTHENTICATE"), authenticates);
    assert!(t.view().connected);
    assert_eq!(t.view().status, "Connected as gabe");
    t.advance(6 * DAY);
    assert_eq!(t.requests().last().unwrap().get("refresh_token"), Some("refresh-3"));
}

#[test]
fn a_saved_token_discord_refuses_is_refreshed_then_signed_in_with() {
    let mut t = Test::new();
    t.save_tokens(t.now + 7 * DAY);
    t.start_up();
    t.error("AUTHENTICATE", "Invalid access token");
    assert_eq!(t.requests()[0].get("grant_type"), Some("refresh_token"));
    assert!(t.transport().open && t.view().connecting);
    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "access-2");
    t.sign_in();
    assert!(t.view().connected);
}

#[test]
fn a_refreshed_token_discord_refuses_asks_discord_to_authorize_again_rather_than_refreshing_twice() {
    let mut t = Test::new();
    t.save_tokens(t.now + 7 * DAY);
    t.start_up();
    t.error("AUTHENTICATE", "Invalid access token");
    t.tokens(200, &tokens("access-2", "refresh-2", Some(604800)));
    t.error("AUTHENTICATE", "Invalid access token");
    assert_eq!(t.requests().len(), 1);
    assert!(t.transport().last("AUTHORIZE").is_some());
    assert_eq!(t.view().status, "Approve the request in Discord");
}

#[test]
fn when_authorizing_again_fails_too_the_user_is_asked_to_reconnect_in_settings() {
    for failing in ["authorize", "exchange", "authenticate"] {
        let mut t = Test::new();
        t.save_tokens(t.now + 7 * DAY);
        t.start_up();
        t.error("AUTHENTICATE", "Invalid access token");
        t.tokens(400, r#"{"error":"invalid_grant"}"#);
        match failing {
            "authorize" => t.error("AUTHORIZE", "OAuth2 Authorization cancelled"),
            _ => {
                t.reply("AUTHORIZE", json!({ "code": "code-2" }));
                if failing == "exchange" {
                    t.tokens(401, r#"{"error":"invalid_client"}"#);
                } else {
                    t.tokens(200, &tokens("access-3", "refresh-3", Some(604800)));
                    t.error("AUTHENTICATE", "Invalid access token");
                }
            }
        }
        t.assert_needs_reconnect();
    }
}

#[test]
fn when_authorizing_again_fails_while_connected_the_connection_closes_and_the_user_is_asked_to_reconnect() {
    let mut t = Test::new();
    t.connect_fully();
    t.advance(6 * DAY);
    t.tokens(400, r#"{"error":"invalid_grant"}"#);
    t.error("AUTHORIZE", "OAuth2 Authorization cancelled");
    t.assert_needs_reconnect();
    assert!(t.view().has_tab());
}

#[test]
fn connect_after_needing_to_reconnect_authorizes_afresh_and_clears_it() {
    let mut t = Test::new();
    t.need_reconnect();
    t.core.connect();
    assert!(t.view().needs_reconnect);
    t.connect_fully();
    assert!(!t.view().needs_reconnect);
    assert_eq!(t.view().status, "Connected as gabe");
}

#[test]
fn disconnect_after_needing_to_reconnect_clears_it_and_forgets_the_tokens() {
    let mut t = Test::new();
    t.need_reconnect();
    t.core.disconnect();
    assert!(!t.view().needs_reconnect);
    assert!(t.secret("Discord.AccessToken").is_none());
    assert_eq!(t.view().status, "Not connected");
}

#[test]
fn connect_refused_says_why_rather_than_asking_to_reconnect() {
    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.ready();
    t.error("AUTHORIZE", "OAuth2 Authorization cancelled");
    assert_eq!(t.view().status, "Couldn't connect to Discord: OAuth2 Authorization cancelled");
    assert!(!t.view().needs_reconnect);
}

#[test]
fn signing_in_with_tokens_from_connect_that_discord_refuses_says_why_rather_than_refreshing() {
    let mut t = Test::new();
    t.authorize_and_await_token();
    t.tokens(200, &tokens("access-1", "refresh-1", Some(604800)));
    t.error("AUTHENTICATE", "Invalid access token");
    assert_eq!(t.requests().len(), 1);
    assert_eq!(t.view().status, "Couldn't connect to Discord: Invalid access token");
    assert!(!t.view().needs_reconnect);
}

// ------------------------------------------------------------------------------------- DiscordReconnectTests

#[test]
fn discord_not_running_is_tried_again_after_2_4_8_16_and_30_seconds_then_every_minute() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    assert_eq!(t.transport().opens, 1);
    for seconds in [2, 4, 8, 16, 30, 60, 60, 60] {
        let opens = t.transport().opens;
        t.advance(Duration::from_secs(seconds) - Test::ms(1));
        assert_eq!(t.transport().opens, opens, "{seconds}");
        t.advance(Test::ms(1));
        assert_eq!(t.transport().opens, opens + 1, "{seconds}");
    }
    assert_eq!(t.view().status, "Discord isn't running");
    assert!(!t.view().connecting && t.view().retrying);
    let log = t.core.take_log();
    let retries: Vec<&String> = log.iter().filter(|line| line.starts_with("discord: retry in ")).collect();
    assert_eq!(retries.first().map(|s| s.as_str()), Some("discord: retry in 2 s"));
    assert_eq!(retries.last().map(|s| s.as_str()), Some("discord: retry in 60 s"));
}

#[test]
fn discord_starting_later_is_signed_in_with_the_saved_tokens_by_itself() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    t.transport().not_running = false;
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().sent.len(), 1);
    assert!(t.view().connecting);
    t.ready();
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "saved-access");
    t.sign_in();
    assert!(t.view().connected);
    assert!(t.transport().last("AUTHORIZE").is_none() && t.requests().is_empty());
    let opens = t.transport().opens;
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, opens);
}

#[test]
fn discord_restarting_is_signed_in_again_with_every_subscription_back() {
    let mut t = Test::new();
    t.connect_fully();
    t.core.take_events();
    t.core.on_closed();
    assert_eq!(t.core.take_events(), [Event::Closed]);
    assert_eq!(t.view().status, "Discord isn't running");
    let log = t.core.take_log();
    assert!(log.contains(&"discord: lost (Discord isn't running)".to_string()), "{log:?}");
    assert!(log.contains(&"discord: retry in 2 s".to_string()), "{log:?}");
    let sent = t.transport().commands().len();
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 2);
    t.ready();
    assert_eq!(t.transport().last("AUTHENTICATE").unwrap()["args"]["access_token"], "access-1");
    t.sign_in();
    let again: Vec<Value> =
        t.transport().commands()[sent..].iter().filter(|c| c["cmd"] == "SUBSCRIBE").cloned().collect();
    assert_eq!(again.len(), 3);
    assert!(matches!(t.core.take_events().as_slice(), [Event::SignedIn(_)]));
}

#[test]
fn signing_in_again_starts_the_backoff_afresh() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    t.advance(Duration::from_secs(2 + 4 + 8));
    assert_eq!(t.transport().opens, 4);
    t.transport().not_running = false;
    t.advance(Duration::from_secs(16));
    t.ready();
    t.sign_in();
    t.core.on_closed();
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 6);
}

#[test]
fn a_pipe_that_opens_but_closes_again_before_signing_in_is_tried_again_with_backoff() {
    let mut t = Test::new();
    t.save_tokens(t.now + 6 * DAY);
    t.core.start();
    t.core.on_closed();
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 2);
    t.core.on_closed();
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 2);
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 3);
    assert!(t.view().connecting);
}

#[test]
fn a_pipe_that_never_says_ready_is_closed_after_10_seconds_and_tried_again_with_backoff() {
    let mut t = Test::new();
    t.save_tokens(t.now + 6 * DAY);
    t.core.start();
    t.advance(Duration::from_secs(10) - Test::ms(1));
    assert!(t.transport().open && t.view().connecting);
    t.advance(Test::ms(1));
    assert!(!t.transport().open && !t.view().connecting);
    assert_eq!(t.view().status, "Discord isn't running");
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 2);
    assert!(t.transport().open);

    // A try that opens it and gets no READY: closed after 10 s, and the back-off goes on.
    t.advance(Duration::from_secs(10));
    assert!(!t.transport().open);
    t.advance(Duration::from_secs(4) - Test::ms(1));
    assert_eq!(t.transport().opens, 2);
    t.advance(Test::ms(1));
    assert_eq!(t.transport().opens, 3);
}

#[test]
fn ready_in_time_keeps_the_pipe_open_and_connect_waits_for_ready_however_long_it_takes() {
    let mut t = Test::new();
    t.save_tokens(t.now + 6 * DAY);
    t.core.start();
    t.advance(Duration::from_secs(5));
    t.ready();
    t.advance(Duration::from_secs(60));
    assert!(t.transport().open);
    assert_eq!(t.transport().opens, 1);
    assert!(t.transport().last("AUTHENTICATE").is_some());

    let mut t = Test::new();
    t.enter_app();
    t.core.connect();
    t.advance(Duration::from_secs(60));
    assert!(t.transport().open && t.view().connecting);
}

#[test]
fn the_wait_for_ready_is_forgotten_as_the_pipe_closes_or_disconnect_is_pressed() {
    let mut t = Test::new();
    t.save_tokens(t.now + 6 * DAY);
    t.core.start();
    t.advance(Duration::from_secs(5));
    t.core.on_closed();
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 2);
    // Past when the first pipe's wait would have ended, the second is still waited for.
    t.advance(Duration::from_secs(5));
    assert!(t.transport().open);
    t.core.disconnect();
    t.advance(Duration::from_secs(60));
    assert_eq!(t.transport().opens, 2);
    assert_eq!(t.view().status, "Not connected");
}

#[test]
fn connect_failing_with_tokens_saved_leaves_the_tab_saying_to_reconnect_in_settings() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    t.transport().not_running = false;
    t.core.connect();
    t.ready();
    t.error("AUTHORIZE", "OAuth2 Authorization cancelled");
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, 2);
    assert_eq!(t.view().tab_line(), Some("Reconnect Discord in settings"));
    assert!(!t.view().needs_reconnect);
}

#[test]
fn a_close_frame_with_tokens_saved_shows_its_message_and_tries_again() {
    let mut t = Test::new();
    t.connect_fully();
    t.core.on_frame(Opcode::Close, r#"{"code":1000,"message":"Shutting down"}"#);
    assert_eq!(t.view().status, "Discord closed the connection: Shutting down");
    assert!(!t.view().connected);
    t.advance(Duration::from_secs(2));
    assert_eq!(t.transport().opens, 2);
}

#[test]
fn disconnect_stops_trying() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    t.core.disconnect();
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, 1);
    assert_eq!(t.view().status, "Not connected");
    assert!(!t.view().has_tab());
}

#[test]
fn connect_while_waiting_for_discord_replaces_the_try_waiting_to_come() {
    let mut t = Test::new();
    t.start_up_with_discord_not_running();
    t.transport().not_running = false;
    t.core.connect();
    assert_eq!(t.transport().opens, 2);
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, 2);
    t.ready();
    assert!(t.transport().last("AUTHORIZE").is_some());
}

#[test]
fn connect_failing_without_tokens_saved_doesnt_try_again() {
    let mut t = Test::new();
    t.transport().not_running = true;
    t.enter_app();
    t.core.connect();
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, 1);
    assert_eq!(t.view().status, "Discord isn't running");
}

#[test]
fn needing_to_reconnect_stops_trying_and_discord_closing_after_it_doesnt_try_again() {
    let mut t = Test::new();
    t.need_reconnect();
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, 1);
    assert_eq!(t.view().status, "Reconnect Discord in settings");

    t.core.connect();
    t.core.on_closed();
    t.advance(Duration::from_secs(600));
    assert_eq!(t.transport().opens, 2);
    assert!(t.view().needs_reconnect);
}

// ------------------------------------------------------------------------------------- the API for the tab

#[test]
fn signed_in_commands_go_out_and_their_answers_and_discords_events_come_back() {
    let mut t = Test::new();
    assert_eq!(t.core.send("GET_SELECTED_VOICE_CHANNEL", json!({}), None), None);
    t.connect_fully();
    t.core.take_events();
    let nonce = t.core.send("GET_SELECTED_VOICE_CHANNEL", json!({}), None).unwrap();
    t.reply("GET_SELECTED_VOICE_CHANNEL", json!({ "id": "500" }));
    let reply =
        Event::Reply { cmd: "GET_SELECTED_VOICE_CHANNEL".into(), nonce: nonce.clone(), data: json!({ "id": "500" }) };
    assert_eq!(t.core.take_events(), [reply]);

    let nonce = t.core.send("SET_VOICE_SETTINGS", json!({ "mute": true }), None).unwrap();
    t.error("SET_VOICE_SETTINGS", "Locked");
    let refused = Event::Refused { cmd: "SET_VOICE_SETTINGS".into(), nonce, message: "Locked".into() };
    assert_eq!(t.core.take_events(), [refused]);
    assert!(t.view().connected, "a refused command leaves the connection up");

    let select =
        json!({ "cmd": "DISPATCH", "evt": "VOICE_CHANNEL_SELECT", "data": { "channel_id": "500" }, "nonce": null });
    t.core.on_frame(Opcode::Frame, &select.to_string());
    let dispatch = Event::Dispatch { evt: "VOICE_CHANNEL_SELECT".into(), data: json!({ "channel_id": "500" }) };
    assert_eq!(t.core.take_events(), [dispatch]);
}
