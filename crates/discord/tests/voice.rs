//! The Call, as the Bundle's DiscordCallTests have it: what each of Discord's replies and events does to it, what is
//! subscribed as it comes and goes, and Mute, Deafen and Leave asked of Discord and shown done only once Discord
//! says so. The Discord tab's own part (its entries, focus, sounds) is the screens' (crates/screens/tests/discord.rs).
use discord::voice::{Avatar, Commands, Follower, CALL_EVENTS};
use discord::{Event, User};
use model::discord::{CallAsk, Voice};
use serde_json::{json, Value};

const ME: &str = "100000000000000001";
const ANA: &str = "200000000000000002";
const BO: &str = "300000000000000003";
const CY: &str = "400000000000000004";

/// Records what is sent, as Discord's pipe would carry it, and answers with a nonce of its own.
#[derive(Default)]
struct Sent {
    commands: Vec<Value>,
}

impl Commands for Sent {
    fn send(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Option<String> {
        let nonce = format!("n{}", self.commands.len());
        self.commands.push(json!({ "cmd": cmd, "args": args, "evt": evt, "nonce": nonce }));
        Some(nonce)
    }
}

impl Sent {
    fn last(&self, cmd: &str) -> &Value {
        self.commands.iter().rev().find(|c| c["cmd"] == cmd).unwrap_or_else(|| panic!("no {cmd} sent"))
    }

    fn of(&self, cmd: &str) -> Vec<&Value> {
        self.commands.iter().filter(|c| c["cmd"] == cmd).collect()
    }
}

struct Test {
    voice: Follower,
    sent: Sent,
}

impl Test {
    fn signed_in() -> Self {
        let mut test = Test { voice: Follower::default(), sent: Sent::default() };
        test.event(Event::SignedIn(User { id: ME.into(), name: "gabe".into() }));
        test
    }

    fn not_in_call() -> Self {
        let mut test = Test::signed_in();
        test.reply("GET_SELECTED_VOICE_CHANNEL", Value::Null);
        test.reply("GET_VOICE_SETTINGS", settings(false, false));
        test
    }

    fn in_call(channel: Value) -> Self {
        Test::in_call_with(channel, false, false)
    }

    fn in_call_with(channel: Value, mute: bool, deaf: bool) -> Self {
        let mut test = Test::signed_in();
        test.reply("GET_VOICE_SETTINGS", settings(mute, deaf));
        test.reply("GET_SELECTED_VOICE_CHANNEL", channel);
        test
    }

    fn event(&mut self, event: Event) {
        self.voice.on_event(event, &mut self.sent);
    }

    /// Discord's answer to the last `cmd` sent.
    fn reply(&mut self, cmd: &str, data: Value) {
        let nonce = self.sent.last(cmd)["nonce"].as_str().unwrap().to_string();
        self.event(Event::Reply { cmd: cmd.into(), nonce, data });
    }

    fn refuse(&mut self, cmd: &str, message: &str) {
        let nonce = self.sent.last(cmd)["nonce"].as_str().unwrap().to_string();
        self.event(Event::Refused { cmd: cmd.into(), nonce, message: message.into() });
    }

    fn dispatch(&mut self, evt: &str, data: Value) {
        self.event(Event::Dispatch { evt: evt.into(), data });
    }

    fn view(&self) -> Voice {
        self.voice.voice(|_, _| None)
    }

    /// The people's names, the user first.
    fn names(&self) -> Vec<String> {
        self.view().call.map(|call| call.people.into_iter().map(|p| p.name).collect()).unwrap_or_default()
    }

    fn title(&self) -> Option<String> {
        self.view().call.map(|call| call.title)
    }

    /// The channels each of `cmd`'s per-channel commands named, with its event, in order.
    fn per_channel(&self, cmd: &str) -> Vec<(String, String)> {
        let named = self.sent.of(cmd).into_iter().filter(|c| c["args"]["channel_id"].is_string());
        named.map(|c| (c["args"]["channel_id"].as_str().unwrap().into(), c["evt"].as_str().unwrap().into())).collect()
    }
}

fn settings(mute: bool, deaf: bool) -> Value {
    json!({ "mute": mute, "deaf": deaf, "input": { "volume": 100 }, "output": { "volume": 100 } })
}

fn channel(id: &str, name: &str, kind: u32, states: &[Value]) -> Value {
    let guild = if kind == 2 { json!("9") } else { Value::Null };
    json!({ "id": id, "name": name, "type": kind, "guild_id": guild, "voice_states": states })
}

#[derive(Default)]
struct Marks {
    global_name: Option<&'static str>,
    avatar: Option<&'static str>,
    self_mute: bool,
    self_deaf: bool,
    server_mute: bool,
}

fn state(id: &str, username: &str) -> Value {
    state_with(id, username, Marks::default())
}

fn state_with(id: &str, username: &str, marks: Marks) -> Value {
    json!({
        "nick": null,
        "mute": false,
        "volume": 100,
        "pan": { "left": 1.0, "right": 1.0 },
        "user": { "id": id, "username": username, "global_name": marks.global_name, "avatar": marks.avatar, "bot": false },
        "voice_state": {
            "mute": marks.server_mute, "deaf": false, "self_mute": marks.self_mute, "self_deaf": marks.self_deaf,
            "suppress": false,
        },
    })
}

fn late_night(states: &[Value]) -> Value {
    channel("500", "Late night", 2, states)
}

#[test]
fn signing_in_asks_discord_which_voice_channel_the_user_is_in_and_their_voice_settings() {
    let test = Test::signed_in();
    assert_eq!(test.sent.last("GET_SELECTED_VOICE_CHANNEL")["args"], json!({}));
    assert_eq!(test.sent.last("GET_VOICE_SETTINGS")["args"], json!({}));
}

#[test]
fn not_in_a_call_at_sign_in_there_is_no_call_and_the_voice_settings_are_kept() {
    let mut test = Test::signed_in();
    test.reply("GET_SELECTED_VOICE_CHANNEL", Value::Null);
    test.reply("GET_VOICE_SETTINGS", settings(true, false));
    assert_eq!(test.view(), Voice { call: None, mute: true, deaf: false });
}

#[test]
fn in_a_call_at_sign_in_the_people_come_self_first_then_the_others_in_discords_order() {
    let test = Test::in_call(late_night(&[state(ANA, "ana"), state(ME, "gabe"), state(BO, "bo")]));
    assert_eq!(test.title().as_deref(), Some("Late night"));
    assert_eq!(test.names(), ["gabe", "ana", "bo"]);
    let view = test.view();
    let me: Vec<bool> = view.call.unwrap().people.iter().map(|p| p.me).collect();
    assert_eq!(me, [true, false, false]);
}

#[test]
fn a_participant_is_named_by_nickname_then_display_name_then_username_and_wears_their_avatar_or_discords_default() {
    let mut nicked =
        state_with(ANA, "ana", Marks { global_name: Some("Ana G"), avatar: Some("a1b2"), ..Marks::default() });
    nicked["nick"] = json!("Annie");
    let bo = state_with(BO, "bo", Marks { global_name: Some("Bo B"), ..Marks::default() });
    let test = Test::in_call(late_night(&[state(ME, "gabe"), nicked, bo, state(CY, "cy")]));
    assert_eq!(test.names(), ["gabe", "Annie", "Bo B", "cy"]);

    let mut asked = Vec::new();
    test.voice.voice(|avatar, _| {
        asked.push(avatar.url(discord::voice::CDN));
        None
    });
    let default = (BO.parse::<u64>().unwrap() >> 22) % 6;
    assert_eq!(asked[1], format!("https://cdn.discordapp.com/avatars/{ANA}/a1b2.png?size=64"));
    assert_eq!(asked[2], format!("https://cdn.discordapp.com/embed/avatars/{default}.png"));
}

#[test]
fn an_avatar_is_kept_by_whose_and_which_it_is_and_no_part_climbs_out_of_the_cache() {
    let own = Avatar::of(ANA, Some("a1b2"));
    assert_eq!(own.file(), std::path::Path::new("avatars").join(ANA).join("a1b2.png"));
    assert_eq!(Avatar::of(BO, Some("")), Avatar::of(BO, None));
    assert_eq!(
        Avatar::of(BO, None).file(),
        std::path::Path::new("embed").join(format!("{}.png", (BO.parse::<u64>().unwrap() >> 22) % 6))
    );
    let sly = Avatar::of("../..", Some("../x"));
    assert!(!sly.file().to_string_lossy().contains(".."), "{:?}", sly.file());
    assert_eq!(own.url("http://127.0.0.1:9/"), format!("http://127.0.0.1:9/avatars/{ANA}/a1b2.png?size=64"));
}

#[test]
fn a_participant_shows_muted_or_deafened_whether_they_did_it_or_the_server_did() {
    let test = Test::in_call(late_night(&[
        state(ME, "gabe"),
        state_with(ANA, "ana", Marks { self_mute: true, ..Marks::default() }),
        state_with(BO, "bo", Marks { server_mute: true, ..Marks::default() }),
        state_with(CY, "cy", Marks { self_mute: true, self_deaf: true, ..Marks::default() }),
    ]));
    let people = test.view().call.unwrap().people;
    assert_eq!(people.iter().map(|p| p.muted).collect::<Vec<_>>(), [false, true, true, true]);
    assert_eq!(people.iter().map(|p| p.deafened).collect::<Vec<_>>(), [false, false, false, true]);
}

#[test]
fn a_dm_without_a_name_is_named_after_the_other_people_in_it() {
    for kind in [1, 3] {
        let test = Test::in_call(channel("600", "", kind, &[state(ME, "gabe"), state(ANA, "ana"), state(BO, "bo")]));
        assert_eq!(test.title().as_deref(), Some("ana, bo"));
    }
    // Nobody else seen there yet: "Call".
    let test = Test::in_call(channel("600", "", 1, &[state(ME, "gabe")]));
    assert_eq!(test.title().as_deref(), Some("Call"));
}

#[test]
fn a_dm_without_a_name_is_renamed_as_people_join_and_keeps_the_last_seen_once_alone() {
    let mut test = Test::in_call(channel("600", "", 3, &[state(ME, "gabe"), state(ANA, "ana")]));
    test.dispatch("VOICE_STATE_CREATE", state(BO, "bo"));
    assert_eq!(test.title().as_deref(), Some("ana, bo"));
    test.dispatch("VOICE_STATE_DELETE", state(ANA, "ana"));
    test.dispatch("VOICE_STATE_DELETE", state(BO, "bo"));
    assert_eq!(test.title().as_deref(), Some("bo"));
}

#[test]
fn joining_a_channel_fetches_it_and_the_call_begins_once_it_arrives_following_its_voice_events() {
    let mut test = Test::not_in_call();
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "500", "guild_id": "9" }));
    assert_eq!(test.sent.last("GET_CHANNEL")["args"]["channel_id"], "500");
    assert!(test.view().call.is_none());

    test.reply("GET_CHANNEL", late_night(&[state(ME, "gabe")]));
    assert!(test.view().call.is_some());
    let subscribed: Vec<(String, String)> = CALL_EVENTS.iter().map(|evt| ("500".into(), evt.to_string())).collect();
    assert_eq!(test.per_channel("SUBSCRIBE"), subscribed);
}

#[test]
fn leaving_the_call_stops_following_its_voice_events() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe")]));
    test.dispatch("VOICE_SETTINGS_UPDATE", settings(true, true));
    test.dispatch("SPEAKING_START", json!({ "user_id": ME }));

    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": null, "guild_id": null }));
    let unsubscribed: Vec<(String, String)> = CALL_EVENTS.iter().map(|evt| ("500".into(), evt.to_string())).collect();
    assert_eq!(test.per_channel("UNSUBSCRIBE"), unsubscribed);
    assert!(test.view().call.is_none());
}

#[test]
fn switching_channels_moves_the_voice_events_from_the_old_channel_to_the_new_one() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe")]));
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "501", "guild_id": "9" }));
    test.reply("GET_CHANNEL", channel("501", "Raid", 2, &[state(ME, "gabe"), state(ANA, "ana")]));

    let unsubscribed: Vec<String> = test.per_channel("UNSUBSCRIBE").into_iter().map(|(channel, _)| channel).collect();
    assert_eq!(unsubscribed, ["500"; 5]);
    assert_eq!(test.per_channel("SUBSCRIBE").iter().filter(|(channel, _)| channel == "501").count(), 5);
    assert_eq!(test.title().as_deref(), Some("Raid"));
    assert_eq!(test.names(), ["gabe", "ana"]);
}

#[test]
fn a_channel_that_arrives_after_the_user_has_moved_on_is_ignored() {
    let mut test = Test::not_in_call();
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "500" }));
    let stale = test.sent.last("GET_CHANNEL")["nonce"].as_str().unwrap().to_string();
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": null }));
    test.event(Event::Reply { cmd: "GET_CHANNEL".into(), nonce: stale, data: late_night(&[state(ME, "gabe")]) });
    assert!(test.view().call.is_none());
}

#[test]
fn a_sign_in_answer_arriving_after_the_user_has_left_doesnt_bring_the_call_back() {
    let mut test = Test::signed_in();
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": null }));
    test.reply("GET_SELECTED_VOICE_CHANNEL", late_night(&[state(ME, "gabe")]));
    assert!(test.view().call.is_none());
}

#[test]
fn someone_joining_is_added_after_the_others_and_someone_leaving_goes() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe"), state(ANA, "ana")]));
    test.dispatch("VOICE_STATE_CREATE", state(BO, "bo"));
    assert_eq!(test.names(), ["gabe", "ana", "bo"]);
    test.dispatch("VOICE_STATE_DELETE", state(ANA, "ana"));
    assert_eq!(test.names(), ["gabe", "bo"]);
    assert_eq!(
        test.voice.take_log(),
        [
            "discord: in a call in \"Late night\" with gabe, ana",
            "discord: bo joined the call",
            "discord: ana left the call"
        ]
    );
}

#[test]
fn speaking_lights_the_person_and_stops_and_leaving_forgets_it() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe"), state(ANA, "ana")]));
    let speaking = |test: &Test| -> Vec<bool> { test.view().call.unwrap().people.iter().map(|p| p.speaking).collect() };
    test.dispatch("SPEAKING_START", json!({ "user_id": ANA, "channel_id": "500" }));
    assert_eq!(speaking(&test), [false, true]);
    // Another channel's speaking is not the Call's.
    test.dispatch("SPEAKING_START", json!({ "user_id": ME, "channel_id": "777" }));
    assert_eq!(speaking(&test), [false, true]);
    test.dispatch("SPEAKING_STOP", json!({ "user_id": ANA }));
    assert_eq!(speaking(&test), [false, false]);
    test.dispatch("SPEAKING_START", json!({ "user_id": ANA }));
    test.dispatch("VOICE_STATE_DELETE", state(ANA, "ana"));
    test.dispatch("VOICE_STATE_CREATE", state(ANA, "ana"));
    assert_eq!(speaking(&test), [false, false]);
}

#[test]
fn the_same_channel_fetched_afresh_keeps_who_was_speaking() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe"), state(ANA, "ana")]));
    test.dispatch("SPEAKING_START", json!({ "user_id": ANA }));
    // Discord asked afresh, as the channel is selected again.
    test.reply("GET_SELECTED_VOICE_CHANNEL", late_night(&[state(ME, "gabe"), state(ANA, "ana")]));
    assert!(test.view().call.unwrap().people[1].speaking);
    assert!(test.per_channel("UNSUBSCRIBE").is_empty(), "the same channel is not followed twice");
}

#[test]
fn someone_muting_or_deafening_shows_on_them() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe"), state(ANA, "ana")]));
    test.dispatch("VOICE_STATE_UPDATE", state_with(ANA, "ana", Marks { self_mute: true, ..Marks::default() }));
    test.dispatch("VOICE_STATE_UPDATE", state_with(ME, "gabe", Marks { self_deaf: true, ..Marks::default() }));
    let people = test.view().call.unwrap().people;
    assert!(people[1].muted && people[0].deafened);
    assert_eq!(test.names(), ["gabe", "ana"]);
}

#[test]
fn mute_asks_discord_and_shows_muted_only_once_discord_says_so() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe")]));
    test.voice.ask(CallAsk::Mute, &mut test.sent);
    assert_eq!(test.sent.last("SET_VOICE_SETTINGS")["args"], json!({ "mute": true }));
    assert!(!test.view().mute);

    test.dispatch("VOICE_SETTINGS_UPDATE", settings(true, false));
    assert!(test.view().mute);

    test.voice.ask(CallAsk::Mute, &mut test.sent);
    assert_eq!(test.sent.last("SET_VOICE_SETTINGS")["args"], json!({ "mute": false }));
}

#[test]
fn deafen_asks_discord_and_shows_deafened_only_once_discord_says_so() {
    let mut test = Test::in_call_with(late_night(&[state(ME, "gabe")]), true, false);
    test.voice.ask(CallAsk::Deafen, &mut test.sent);
    assert_eq!(test.sent.last("SET_VOICE_SETTINGS")["args"], json!({ "deaf": true }));
    assert!(!test.view().deaf);
    test.dispatch("VOICE_SETTINGS_UPDATE", settings(true, true));
    assert!(test.view().deaf && test.view().mute);
}

#[test]
fn leave_asks_discord_to_leave_and_the_call_ends_only_once_discord_says_so() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe")]));
    test.voice.ask(CallAsk::Leave, &mut test.sent);
    assert_eq!(test.sent.last("SELECT_VOICE_CHANNEL")["args"], json!({ "channel_id": null }));
    assert!(test.view().call.is_some());
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": null }));
    assert!(test.view().call.is_none());
}

#[test]
fn a_refused_voice_command_changes_nothing() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe")]));
    test.voice.ask(CallAsk::Mute, &mut test.sent);
    test.voice.take_log();
    test.refuse("SET_VOICE_SETTINGS", "Nope");
    assert!(test.view().call.is_some() && !test.view().mute);
    assert_eq!(test.voice.take_log(), ["discord: Discord refused SET_VOICE_SETTINGS: Nope"]);
}

#[test]
fn discord_refusing_the_new_channel_drops_the_old_call_rather_than_showing_it_as_current() {
    let mut test = Test::in_call(late_night(&[state(ME, "gabe")]));
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "501" }));
    test.refuse("GET_CHANNEL", "No access");
    assert!(test.view().call.is_none());
    let last = test.sent.commands.last().unwrap();
    assert_eq!((last["cmd"].as_str(), last["args"]["channel_id"].as_str()), (Some("UNSUBSCRIBE"), Some("500")));
}

#[test]
fn discord_closing_mid_call_ends_it_and_forgets_the_voice_settings() {
    let mut test = Test::in_call_with(late_night(&[state(ME, "gabe")]), true, false);
    test.dispatch("SPEAKING_START", json!({ "user_id": ME }));
    test.event(Event::Closed);
    assert_eq!(test.view(), Voice::default());
    // Signed in again, it is all asked afresh.
    test.event(Event::SignedIn(User { id: ME.into(), name: "gabe".into() }));
    assert_eq!(test.sent.of("GET_SELECTED_VOICE_CHANNEL").len(), 2);
}

#[test]
fn muted_out_of_a_call_is_kept_for_when_a_call_begins() {
    let mut test = Test::not_in_call();
    test.dispatch("VOICE_SETTINGS_UPDATE", settings(true, false));
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "500" }));
    test.reply("GET_CHANNEL", late_night(&[state_with(ME, "gabe", Marks { self_mute: true, ..Marks::default() })]));
    let view = test.view();
    assert!(view.mute && view.call.unwrap().people[0].muted);
}
