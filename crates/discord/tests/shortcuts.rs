//! Known channels and Shortcuts, as the Bundle's DiscordShortcutsTests have them: which channels are remembered, in
//! which order and for how long, what each Shortcut shows, kept in the secret store, and joined in one press. The
//! tab's own part (the tiles, moving among them, the menu) is the screens' (crates/screens/tests/discord.rs).
mod common;

use common::*;
use model::discord::Ask;
use serde_json::json;

#[test]
fn a_voice_channel_joined_becomes_a_shortcut_named_after_it_with_its_servers_icon_and_name() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[state(ME, "gabe")]));
    assert_eq!(test.sent.last("GET_GUILD").unwrap()["args"], json!({ "guild_id": "9" }));
    test.reply("GET_GUILD", json!({ "id": "9", "name": "The Lads", "icon_url": LADS_ICON }));
    test.leave();

    let shortcuts = test.shortcuts();
    assert_eq!(shortcuts.len(), 1);
    assert_eq!(shortcuts[0].name, "Late night");
    assert_eq!(shortcuts[0].server.as_deref(), Some("The Lads"));
    assert_eq!(Test::picture(&shortcuts[0]).as_deref(), Some(LADS_ICON));
    assert!(!shortcuts[0].dm);
}

#[test]
fn a_voice_channel_whose_server_hasnt_answered_is_still_no_dm() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[]));
    test.refuse("GET_GUILD", "Nope");
    test.leave();
    let shortcut = &test.shortcuts()[0];
    assert_eq!(shortcut.server, None);
    assert!(!shortcut.dm);
}

#[test]
fn the_servers_name_and_icon_arriving_unchanged_save_nothing() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[]));
    let guild = json!({ "id": "9", "name": "The Lads", "icon_url": LADS_ICON });
    test.reply("GET_GUILD", guild.clone());
    test.leave();
    test.join(voice_channel("500", "Late night", "9", &[]));
    let writes = test.writes;
    test.reply("GET_GUILD", guild);
    assert_eq!(test.writes, writes);
}

#[test]
fn a_dm_joined_becomes_a_shortcut_named_after_the_people_in_it_wearing_the_first_ones_avatar() {
    for kind in [1, 3] {
        let mut test = Test::not_in_call();
        test.join(dm("600", kind, &[state(ME, "gabe"), state_with(ANA, "ana", Some("a1")), state(BO, "bo")]));
        test.leave();
        let shortcut = &test.shortcuts()[0];
        assert_eq!(shortcut.name, "ana, bo");
        assert_eq!(Test::picture(shortcut), Some(avatar_of(ANA, "a1")));
        assert_eq!(shortcut.server, None);
        assert!(shortcut.dm);
        assert!(test.sent.last("GET_GUILD").is_none(), "a DM has no server to ask");
    }
}

#[test]
fn a_named_dm_keeps_its_name_and_avatar_as_people_come_and_go_until_it_is_joined_again() {
    let mut test = Test::not_in_call();
    test.join(dm("600", 3, &[state(ME, "gabe"), state_with(ANA, "ana", Some("a1")), state(BO, "bo")]));
    let writes = test.writes;
    test.dispatch("VOICE_STATE_DELETE", state(ANA, "ana"));
    assert_eq!(test.shortcut_names(), ["ana, bo"]);
    assert_eq!(Test::picture(&test.shortcuts()[0]), Some(avatar_of(ANA, "a1")));
    assert_eq!(test.writes, writes, "nothing saved");

    test.leave();
    test.join(dm("600", 3, &[state(ME, "gabe"), state_with(BO, "bo", Some("b2"))]));
    assert_eq!(test.shortcut_names(), ["bo"]);
    assert_eq!(Test::picture(&test.shortcuts()[0]), Some(avatar_of(BO, "b2")));
}

#[test]
fn a_dm_rung_before_anyone_answers_is_named_once_they_join() {
    let mut test = Test::not_in_call();
    test.join(dm("600", 1, &[state(ME, "gabe")]));
    assert_eq!(test.shortcut_names(), ["Call"]);
    test.dispatch("VOICE_STATE_CREATE", state_with(ANA, "ana", Some("a1")));
    test.leave();
    assert_eq!(test.shortcut_names(), ["ana"]);
    assert_eq!(Test::picture(&test.shortcuts()[0]), Some(avatar_of(ANA, "a1")));
}

#[test]
fn rejoining_a_dm_alone_keeps_the_name_and_avatar_learned_before() {
    let mut test = Test::not_in_call();
    test.join(dm("600", 1, &[state(ME, "gabe"), state_with(ANA, "ana", Some("a1"))]));
    test.leave();
    test.join(dm("600", 1, &[state(ME, "gabe")]));
    assert_eq!(test.shortcut_names(), ["ana"]);
    assert_eq!(Test::picture(&test.shortcuts()[0]), Some(avatar_of(ANA, "a1")));
}

#[test]
fn the_call_the_user_is_in_at_sign_in_is_learned_too() {
    let mut test = Test::new();
    test.event(discord::Event::SignedIn(discord::User { id: ME.into(), name: "gabe".into() }));
    test.reply("GET_SELECTED_VOICE_CHANNEL", dm("600", 1, &[state(ME, "gabe"), state(ANA, "ana")]));
    assert_eq!(test.shortcut_names(), ["ana"]);
}

#[test]
fn shortcuts_are_most_recently_joined_first_and_rejoining_one_moves_it_to_the_front() {
    let mut test = Test::not_in_call();
    for (id, name) in [("500", "A"), ("501", "B"), ("502", "C")] {
        test.join(voice_channel(id, name, "9", &[]));
    }
    assert_eq!(test.shortcut_names(), ["C", "B", "A"]);
    test.join(voice_channel("500", "A", "9", &[]));
    assert_eq!(test.shortcut_names(), ["A", "C", "B"]);
}

#[test]
fn a_servers_icon_arriving_later_puts_it_on_the_tile() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "A", "9", &[]));
    let get_guild = test.sent.last("GET_GUILD").unwrap().clone();
    test.leave();
    test.reply_to(&get_guild, json!({ "id": "9", "name": "The Lads", "icon_url": LADS_ICON }));
    assert_eq!(Test::picture(&test.shortcuts()[0]).as_deref(), Some(LADS_ICON));
}

#[test]
fn at_most_six_shortcuts_show() {
    let mut test = Test::not_in_call();
    for n in 1..=8 {
        test.join(voice_channel(&format!("50{n}"), &format!("Ch{n}"), "9", &[]));
    }
    test.leave();
    assert_eq!(test.shortcut_names(), ["Ch8", "Ch7", "Ch6", "Ch5", "Ch4", "Ch3"]);
}

#[test]
fn voice_channels_beyond_the_six_most_recent_are_forgotten_but_dms_are_kept() {
    let mut test = Test::not_in_call();
    test.join(dm("600", 1, &[state(ME, "gabe"), state(ANA, "ana")]));
    test.join(voice_channel("500", "Old channel", "9", &[]));
    for n in 1..=6 {
        test.join(dm(&format!("60{n}"), 1, &[state(ME, "gabe"), state(BO, &format!("bo{n}"))]));
    }
    test.leave();
    assert_eq!(test.shortcut_names(), ["bo6", "bo5", "bo4", "bo3", "bo2", "bo1"]);

    test.ask(Ask::RemoveShortcut("606".into()));
    assert_eq!(test.shortcut_names(), ["bo5", "bo4", "bo3", "bo2", "bo1", "ana"]);
}

#[test]
fn shortcuts_are_kept_in_the_secret_store_and_come_back_when_the_dashboard_starts_again() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[]));
    test.reply("GET_GUILD", json!({ "id": "9", "name": "The Lads", "icon_url": LADS_ICON }));
    test.join(dm("600", 1, &[state(ME, "gabe"), state_with(ANA, "ana", Some("a1"))]));
    assert!(test.saved.iter().any(|(name, _)| name == "Discord.Shortcuts"));

    let mut again = test.new_core();
    again.sign_in();
    assert_eq!(again.shortcut_names(), ["ana", "Late night"]);
    let pictures: Vec<Option<String>> = again.shortcuts().iter().map(Test::picture).collect();
    assert_eq!(pictures, [Some(avatar_of(ANA, "a1")), Some(LADS_ICON.into())]);
    let servers: Vec<Option<String>> = again.shortcuts().into_iter().map(|s| s.server).collect();
    assert_eq!(servers, [None, Some("The Lads".into())]);
}

#[test]
fn discord_going_and_disconnecting_keep_the_shortcuts() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "Late night", "9", &[]));
    test.closed();
    test.sign_in();
    assert_eq!(test.shortcut_names(), ["Late night"]);
}

#[test]
fn joining_a_shortcut_asks_discord_and_nothing_shows_until_discord_says_so() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "A", "9", &[]));
    test.join(voice_channel("501", "B", "9", &[]));
    test.leave();
    test.ask(Ask::Join("500".into()));
    assert_eq!(
        test.sent.last("SELECT_VOICE_CHANNEL").unwrap()["args"],
        json!({ "channel_id": "500", "force": true })
    );
    assert!(!test.in_call());
    test.join(voice_channel("500", "A", "9", &[state(ME, "gabe")]));
    assert!(test.in_call());
    assert_eq!(test.shortcut_names(), ["A", "B"]);
}

#[test]
fn joining_the_shortcut_of_the_call_the_user_is_in_sends_nothing() {
    let mut test = Test::not_in_call();
    test.join(voice_channel("500", "A", "9", &[state(ME, "gabe")]));
    let sent = test.sent.commands.len();
    test.ask(Ask::Join("500".into()));
    assert_eq!(test.sent.commands.len(), sent);
}

#[test]
fn remove_shortcut_forgets_it_for_good() {
    let mut test = Test::not_in_call();
    for (id, name) in [("500", "A"), ("501", "B"), ("502", "C")] {
        test.join(voice_channel(id, name, "9", &[]));
    }
    test.leave();
    test.ask(Ask::RemoveShortcut("501".into()));
    assert_eq!(test.shortcut_names(), ["C", "A"]);
    let mut again = test.new_core();
    again.sign_in();
    assert_eq!(again.shortcut_names(), ["C", "A"]);
}

#[test]
fn a_shortcut_removed_during_its_call_is_not_brought_back_by_the_call() {
    let mut test = Test::not_in_call();
    test.join(dm("600", 1, &[state(ME, "gabe"), state(ANA, "ana")]));
    test.ask(Ask::RemoveShortcut("600".into()));
    test.dispatch("VOICE_STATE_CREATE", state(BO, "bo"));
    assert!(test.shortcuts().is_empty());
}
