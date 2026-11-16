//! The Servers Flyouts, as the Bundle's DiscordServersTests have them: the user's servers in the Bundle's order, a
//! server's voice channels with who is in each, the spinner shown a second at least, answers no longer awaited
//! ignored, and joining a channel in one press. Where focus goes in the Flyouts is the screens'
//! (crates/screens/tests/discord.rs).
mod common;

use std::time::Duration;

use common::*;
use model::discord::{Ask, Browse, BrowseAsk};
use serde_json::{json, Value};

fn guilds() -> Value {
    json!({ "guilds": [
        { "id": "9", "name": "The Lads", "icon_url": LADS_ICON },
        { "id": "8", "name": "Wine club", "icon_url": null },
    ] })
}

/// The Lads and Wine club among three servers never joined, named in mixed case, in no order of Discord's.
fn five_guilds() -> Value {
    json!({ "guilds": [
        { "id": "5", "name": "Zeta" },
        { "id": "9", "name": "The Lads", "icon_url": LADS_ICON },
        { "id": "6", "name": "beta" },
        { "id": "8", "name": "Wine club" },
        { "id": "7", "name": "art" },
    ] })
}

/// The Lads' channels, as GET_CHANNELS answers: text channels among the voice ones.
fn channels() -> Value {
    json!({ "channels": [
        { "id": "490", "name": "general", "type": 0 },
        { "id": "500", "name": "Late night", "type": 2 },
        { "id": "491", "name": "memes", "type": 0 },
        { "id": "501", "name": "Raid", "type": 2 },
    ] })
}

/// Servers opened, its spinner shown its second, so Discord's answer shows when it comes.
fn open_servers() -> Test {
    let mut test = Test::not_in_call();
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_secs(1));
    test
}

fn lads() -> BrowseAsk {
    BrowseAsk::Server { id: "9".into(), name: "The Lads".into() }
}

fn open_lads() -> Test {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(lads());
    test.advance(Duration::from_secs(1));
    test
}

#[test]
fn servers_asks_discord_for_the_servers_and_opens_a_flyout_loading_them() {
    let mut test = Test::not_in_call();
    test.browse(BrowseAsk::Servers);
    assert_eq!(test.sent.last("GET_GUILDS").unwrap()["args"], json!({}));
    assert_eq!(test.browse_view(), Browse::Servers { loading: true, servers: vec![] });
    assert_eq!(test.browse_view().title(), "Servers");
}

#[test]
fn servers_answered_quickly_wait_for_the_spinner_to_have_shown_a_second() {
    let mut test = Test::not_in_call();
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_millis(300));
    test.reply("GET_GUILDS", guilds());
    assert!(test.loading());
    test.advance(Duration::from_millis(699));
    assert!(test.loading());
    assert!(test.following.next_timer().is_some(), "the loop is woken for it");
    test.advance(Duration::from_millis(1));
    assert!(!test.loading());
    assert_eq!(test.listed(), ["The Lads", "Wine club"]);
}

#[test]
fn servers_answered_after_a_second_show_at_once() {
    let mut test = Test::not_in_call();
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_millis(1500));
    test.reply("GET_GUILDS", guilds());
    assert!(!test.loading());
}

#[test]
fn a_servers_channels_answered_quickly_wait_for_the_spinner_too() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(lads());
    test.reply("GET_CHANNELS", channels());
    assert!(test.loading());
    test.advance(Duration::from_secs(1));
    assert_eq!(test.listed(), ["Late night", "Raid"]);
}

#[test]
fn servers_held_for_the_spinner_never_show_once_the_flyout_has_closed() {
    let mut test = Test::not_in_call();
    test.browse(BrowseAsk::Servers);
    test.reply("GET_GUILDS", guilds());
    test.browse(BrowseAsk::Close);
    test.advance(Duration::from_secs(1));
    assert_eq!(test.browse_view(), Browse::Closed);
    test.browse(BrowseAsk::Servers);
    assert!(test.loading(), "nothing was kept of them");
}

#[test]
fn the_servers_arrive_one_entry_each_with_its_icon_alphabetically() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    let Browse::Servers { loading: false, servers } = test.browse_view() else { panic!("{:?}", test.browse_view()) };
    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["The Lads", "Wine club"]);
    let icons: Vec<Option<String>> =
        servers.iter().map(|s| s.icon.as_ref().map(|p| p.to_string_lossy().into_owned())).collect();
    assert_eq!(icons, [Some(LADS_ICON.into()), None]);
}

#[test]
fn discord_refusing_the_servers_stops_loading_with_none() {
    let mut test = open_servers();
    test.refuse("GET_GUILDS", "Nope");
    assert_eq!(test.browse_view(), Browse::Servers { loading: false, servers: vec![] });
}

#[test]
fn servers_arriving_after_the_flyout_closed_are_ignored() {
    let mut test = open_servers();
    test.browse(BrowseAsk::Close);
    test.reply("GET_GUILDS", guilds());
    assert_eq!(test.browse_view(), Browse::Closed);
}

#[test]
fn a_server_asks_for_its_channels_and_opens_its_flyout_loading_them() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(lads());
    assert_eq!(test.sent.last("GET_CHANNELS").unwrap()["args"], json!({ "guild_id": "9" }));
    let view = test.browse_view();
    assert_eq!(view.title(), "The Lads");
    assert!(view.loading() && view.is_empty());
}

#[test]
fn a_servers_voice_channels_list_in_discords_order_then_each_is_fetched_for_who_is_in_it() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    assert!(!test.loading());
    assert_eq!(test.listed(), ["Late night", "Raid"]);
    assert_eq!(test.occupants(), [None, None]);
    let fetched: Vec<Value> = test.sent.of("GET_CHANNEL").iter().map(|c| c["args"]["channel_id"].clone()).collect();
    assert_eq!(fetched, [json!("500"), json!("501")]);
}

#[test]
fn a_voice_channel_shows_the_avatars_of_who_is_in_it_or_none_when_empty() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    let fetches = test.sent.of("GET_CHANNEL");
    test.reply_to(&fetches[1], voice_channel("501", "Raid", "9", &[]));
    test.reply_to(
        &fetches[0],
        voice_channel("500", "Late night", "9", &[state_with(ANA, "ana", Some("a1")), state_with(BO, "bo", Some("b2"))]),
    );
    assert_eq!(test.occupants(), [Some(vec![avatar_of(ANA, "a1"), avatar_of(BO, "b2")]), Some(vec![])]);
    assert!(!test.in_call(), "who is in a channel is not the Call");
}

#[test]
fn back_in_a_servers_flyout_goes_to_the_servers_without_asking_again() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(BrowseAsk::Server { id: "8".into(), name: "Wine club".into() });
    let asked = test.sent.of("GET_GUILDS").len();
    test.browse(BrowseAsk::Back);
    assert_eq!(test.browse_view().title(), "Servers");
    assert!(!test.loading());
    assert_eq!(test.listed(), ["The Lads", "Wine club"]);
    assert_eq!(test.sent.of("GET_GUILDS").len(), asked);
}

#[test]
fn stage_channels_list_among_the_voice_channels_in_discords_order() {
    let mut test = open_lads();
    test.reply(
        "GET_CHANNELS",
        json!({ "channels": [
            { "id": "490", "name": "general", "type": 0 },
            { "id": "510", "name": "Town hall", "type": 13 },
            { "id": "500", "name": "Late night", "type": 2 },
        ] }),
    );
    assert_eq!(test.listed(), ["Town hall", "Late night"]);
}

#[test]
fn who_is_in_a_channel_the_user_has_just_joined_arriving_first_fills_it_but_the_call_waits_for_its_own_reply() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    let fetch = test.sent.of("GET_CHANNEL")[0].clone();
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "500", "guild_id": "9" }));
    let own = test.sent.last("GET_CHANNEL").unwrap().clone();
    let both = voice_channel("500", "Late night", "9", &[state(ME, "gabe"), state(ANA, "ana")]);

    test.reply_to(&fetch, both.clone());
    assert!(!test.in_call());
    assert_eq!(test.occupants()[0].as_ref().map(Vec::len), Some(2));

    test.reply_to(&own, both);
    assert!(test.in_call());
}

#[test]
fn channels_arriving_after_the_user_backed_out_of_the_server_are_ignored() {
    let mut test = open_lads();
    let get_channels = test.sent.last("GET_CHANNELS").unwrap().clone();
    test.browse(BrowseAsk::Back);
    test.reply_to(&get_channels, channels());
    assert_eq!(test.browse_view().title(), "Servers");
    assert!(test.sent.of("GET_CHANNEL").is_empty());
}

#[test]
fn channels_of_a_server_left_for_another_are_ignored() {
    let mut test = open_lads();
    let lads = test.sent.last("GET_CHANNELS").unwrap().clone();
    test.browse(BrowseAsk::Back);
    test.browse(BrowseAsk::Server { id: "8".into(), name: "Wine club".into() });
    test.reply_to(&lads, channels());
    assert_eq!(test.browse_view().title(), "Wine club");
    assert!(test.loading() && test.browse_view().is_empty());
}

#[test]
fn who_is_in_a_channel_arriving_after_the_user_backed_out_is_ignored() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    let fetch = test.sent.of("GET_CHANNEL")[0].clone();
    test.browse(BrowseAsk::Back);
    test.reply_to(&fetch, voice_channel("500", "Late night", "9", &[state(ANA, "ana")]));
    assert_eq!(test.browse_view().title(), "Servers");
    test.browse(lads());
    assert_eq!(test.occupants()[0], None, "not known of the channel kept either");
}

#[test]
fn discord_refusing_a_servers_channels_stops_loading_with_none() {
    let mut test = open_lads();
    test.refuse("GET_CHANNELS", "Nope");
    assert!(!test.loading() && test.browse_view().is_empty());
}

#[test]
fn a_voice_channel_chosen_asks_discord_to_join_it_and_the_flyout_closes() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    test.ask(Ask::Join("501".into()));
    assert_eq!(test.sent.last("SELECT_VOICE_CHANNEL").unwrap()["args"], json!({ "channel_id": "501", "force": true }));
    assert_eq!(test.browse_view(), Browse::Closed);
}

#[test]
fn the_voice_channel_the_user_is_in_asks_nothing_and_closes_the_flyout() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("501", "Raid", "9", &[state(ME, "gabe")]));
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_secs(1));
    test.reply("GET_GUILDS", guilds());
    test.browse(lads());
    test.advance(Duration::from_secs(1));
    test.reply("GET_CHANNELS", channels());
    test.ask(Ask::Join("501".into()));
    assert!(test.sent.last("SELECT_VOICE_CHANNEL").is_none());
    assert_eq!(test.browse_view(), Browse::Closed);
}

#[test]
fn the_call_still_follows_the_channel_the_user_moved_to_while_a_server_is_open() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    test.dispatch("VOICE_CHANNEL_SELECT", json!({ "channel_id": "501", "guild_id": "9" }));
    test.reply("GET_CHANNEL", voice_channel("501", "Raid", "9", &[state(ME, "gabe")]));
    assert!(test.in_call());
    let occupants = test.occupants();
    assert_eq!(occupants[1].as_ref().map(Vec::len), Some(1));
    assert_eq!(occupants[0], None);
}

#[test]
fn discord_going_away_closes_the_flyout_and_forgets_what_it_answered() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    test.closed();
    assert_eq!(test.browse_view(), Browse::Closed);
    test.sign_in();
    test.browse(BrowseAsk::Servers);
    assert!(test.loading() && test.browse_view().is_empty());
    test.advance(Duration::from_secs(1));
    test.reply("GET_GUILDS", guilds());
    test.browse(lads());
    assert!(test.loading() && test.browse_view().is_empty());
}

#[test]
fn reopening_servers_shows_those_fetched_at_once_and_still_asks_again() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(BrowseAsk::Close);
    test.browse(BrowseAsk::Servers);
    assert!(!test.loading());
    assert_eq!(test.listed(), ["The Lads", "Wine club"]);
    assert_eq!(test.sent.of("GET_GUILDS").len(), 2);
}

#[test]
fn servers_answering_changed_are_listed_afresh() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(BrowseAsk::Close);
    test.browse(BrowseAsk::Servers);
    test.reply(
        "GET_GUILDS",
        json!({ "guilds": [
            { "id": "9", "name": "Lads", "icon_url": LADS_ICON },
            { "id": "8", "name": "Wine club", "icon_url": null },
            { "id": "7", "name": "Art", "icon_url": null },
        ] }),
    );
    assert_eq!(test.listed(), ["Art", "Lads", "Wine club"]);
}

#[test]
fn discord_refusing_the_servers_on_a_reopen_keeps_those_shown() {
    let mut test = open_servers();
    test.reply("GET_GUILDS", guilds());
    test.browse(BrowseAsk::Close);
    test.browse(BrowseAsk::Servers);
    test.refuse("GET_GUILDS", "Nope");
    assert_eq!(test.listed(), ["The Lads", "Wine club"]);
}

#[test]
fn reopening_a_server_shows_its_channels_and_who_is_in_them_at_once_then_asks_again() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    let fetches = test.sent.of("GET_CHANNEL");
    test.reply_to(&fetches[0], voice_channel("500", "Late night", "9", &[state_with(ANA, "ana", Some("a1"))]));
    test.reply_to(&fetches[1], voice_channel("501", "Raid", "9", &[]));
    test.browse(BrowseAsk::Back);
    test.browse(lads());
    assert!(!test.loading());
    assert_eq!(test.listed(), ["Late night", "Raid"]);
    assert_eq!(test.occupants(), [Some(vec![avatar_of(ANA, "a1")]), Some(vec![])]);
    assert_eq!(test.sent.of("GET_CHANNELS").len(), 2);

    test.reply("GET_CHANNELS", channels());
    assert_eq!(test.occupants()[0], Some(vec![avatar_of(ANA, "a1")]));
    assert_eq!(test.sent.of("GET_CHANNEL").len(), 4);
    let again = test.sent.of("GET_CHANNEL")[2].clone();
    test.reply_to(&again, voice_channel("500", "Late night", "9", &[]));
    assert_eq!(test.occupants()[0], Some(vec![]));
}

#[test]
fn a_servers_channels_answering_changed_keep_who_is_in_them() {
    let mut test = open_lads();
    test.reply("GET_CHANNELS", channels());
    let fetch = test.sent.of("GET_CHANNEL")[0].clone();
    test.reply_to(&fetch, voice_channel("500", "Late night", "9", &[state_with(ANA, "ana", Some("a1"))]));
    test.browse(BrowseAsk::Back);
    test.browse(lads());
    test.reply(
        "GET_CHANNELS",
        json!({ "channels": [
            { "id": "510", "name": "Town hall", "type": 13 },
            { "id": "500", "name": "Late night", "type": 2 },
            { "id": "501", "name": "Raid", "type": 2 },
        ] }),
    );
    assert_eq!(test.listed(), ["Town hall", "Late night", "Raid"]);
    let occupants = test.occupants();
    assert_eq!(occupants[0], None);
    assert_eq!(occupants[1], Some(vec![avatar_of(ANA, "a1")]));
}

#[test]
fn servers_list_the_most_recently_joined_first_then_the_rest_alphabetically() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("600", "Lounge", "8", &[]));
    test.join(voice_channel("500", "Late night", "9", &[]));
    test.join(voice_channel("601", "Lounge", "8", &[]));
    test.leave();
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_secs(1));
    test.reply("GET_GUILDS", five_guilds());
    assert_eq!(test.listed(), ["Wine club", "The Lads", "art", "beta", "Zeta"]);
}

#[test]
fn servers_keep_their_order_when_the_dashboard_starts_again() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[]));
    test.join(voice_channel("600", "Lounge", "8", &[]));
    test.leave();
    let mut again = test.new_core();
    again.sign_in();
    again.browse(BrowseAsk::Servers);
    again.advance(Duration::from_secs(1));
    again.reply("GET_GUILDS", five_guilds());
    assert_eq!(again.listed(), ["Wine club", "The Lads", "art", "beta", "Zeta"]);
}

#[test]
fn a_server_stays_among_the_recent_after_its_channels_drop_off_the_shortcuts() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("600", "Lounge", "8", &[]));
    for n in 0..6 {
        test.join(voice_channel(&format!("70{n}"), &format!("Room {n}"), "9", &[]));
    }
    test.leave();
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_secs(1));
    test.reply("GET_GUILDS", five_guilds());
    assert!(!test.shortcut_names().contains(&"Lounge".to_string()));
    assert_eq!(test.listed(), ["The Lads", "Wine club", "art", "beta", "Zeta"]);
}

#[test]
fn removing_a_shortcut_leaves_the_servers_order_alone() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[]));
    test.join(voice_channel("600", "Lounge", "8", &[]));
    test.leave();
    test.ask(Ask::RemoveShortcut("600".into()));
    assert!(!test.shortcut_names().contains(&"Lounge".to_string()));
    test.browse(BrowseAsk::Servers);
    test.advance(Duration::from_secs(1));
    test.reply("GET_GUILDS", five_guilds());
    assert_eq!(test.listed(), ["Wine club", "The Lads", "art", "beta", "Zeta"]);
}
