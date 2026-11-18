//! Voice as the page pushes it (`js/voice.js`) and as the screens see it (docs/discord-web.md, "Voice"): the JSON, parsed;
//! its mapping to `model::discord` (the Call, the Servers Flyout, the Shortcuts); and the log lines its changes make,
//! which are counts and states, never a name or an id.
use std::path::PathBuf;

use model::discord::{Call, Person, Server, Shortcut, VoiceChannel, CALL};
use serde::Deserialize;

use crate::art::{Art, Pic};
use crate::shortcuts::{Joined, Known};

/// Our keys for what voice reads and does, in the order the log lists them. Required for voice: `channels`, `voice`,
/// `media`, `guilds`, `voiceActions`; the rest is optional.
pub const KEYS: [&str; 14] = [
    "channels",
    "voice",
    "media",
    "guilds",
    "voiceActions",
    "speaking",
    "selected",
    "rtc",
    "members",
    "guildOrder",
    "permissions",
    "mediaActions",
    "callActions",
    "guildChannels",
];

/// Whether `key` is one of voice's, which the Direct Messages' log line leaves out (`channels` is the DMs' too).
pub fn is_voice_key(key: &str) -> bool {
    key != "channels" && KEYS.contains(&key)
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawGuild {
    pub id: String,
    pub name: String,
    /// The icon's hash; empty for none.
    pub icon: String,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawPerson {
    pub id: String,
    pub name: String,
    /// The avatar's hash; empty for Discord's default one.
    pub avatar: String,
    pub muted: bool,
    pub deafened: bool,
    pub me: bool,
}

/// The user's Call (`channel` empty out of one) and their own mute and deafen.
#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawVoice {
    /// The signed-in user's display name.
    pub me: String,
    pub channel: String,
    pub title: String,
    pub guild: Option<RawGuild>,
    pub dm: bool,
    pub people: Vec<RawPerson>,
    pub speaking: Vec<String>,
    pub mute: bool,
    pub deaf: bool,
    /// RTC's own word on whether the connection is up; None where that store was not found.
    pub rtc: Option<bool>,
    /// The user's own voice state is in the channel.
    #[serde(rename = "in")]
    pub joined: bool,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawOccupant {
    pub id: String,
    pub avatar: String,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawChannel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u32,
    pub users: Vec<RawOccupant>,
}

/// The voice and stage channels of the server being browsed.
#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawServer {
    pub id: String,
    pub channels: Vec<RawChannel>,
}

/// The Call's heading: the channel's name, a DM's other people comma-joined, else those in it, else "Call".
pub fn title(raw: &RawVoice) -> String {
    if !raw.title.is_empty() {
        return raw.title.clone();
    }
    let others: Vec<&str> =
        raw.people.iter().filter(|p| !p.me && !p.name.is_empty()).map(|p| p.name.as_str()).collect();
    if raw.dm && !others.is_empty() {
        others.join(", ")
    } else {
        CALL.to_string()
    }
}

/// The Call as the screens show it: the user first, then the others in Discord's order; None out of one.
pub fn call(raw: &RawVoice, art: &mut Art) -> Option<Call> {
    if raw.channel.is_empty() {
        return None;
    }
    let mut people: Vec<Person> = raw
        .people
        .iter()
        .map(|p| Person {
            id: p.id.clone(),
            name: p.name.clone(),
            avatar: Pic::avatar(&p.id, &p.avatar).and_then(|pic| art.get(&pic)),
            speaking: raw.speaking.contains(&p.id),
            muted: p.muted,
            deafened: p.deafened,
            me: p.me,
        })
        .collect();
    // The user first (a stable partition: the others keep Discord's order).
    people.sort_by_key(|p| !p.me);
    Some(Call { id: raw.channel.clone(), title: title(raw), people })
}

/// What a Known channel learns of the Call, once the user is really in it.
pub fn joined(raw: &RawVoice) -> Option<Joined> {
    if raw.channel.is_empty() || !(raw.joined || raw.rtc == Some(true)) {
        return None;
    }
    let heading = title(raw);
    let guild = raw.guild.as_ref().filter(|g| !g.id.is_empty());
    let image = match guild {
        Some(g) => Pic::icon(&g.id, &g.icon),
        None => raw.people.iter().find(|p| !p.me).and_then(|p| Pic::avatar(&p.id, &p.avatar)),
    };
    Some(Joined {
        id: raw.channel.clone(),
        guild: guild.map(|g| g.id.clone()),
        name: (heading != CALL).then_some(heading),
        server_name: guild.map(|g| g.name.clone()).filter(|n| !n.is_empty()),
        image,
    })
}

/// The Servers Flyout's servers: in the user's own order where Discord gave one, else the Bundle's (the servers joined
/// in most recent first, then by name ignoring case).
pub fn servers(guilds: &[RawGuild], ordered: bool, known: &Known, art: &mut Art) -> Vec<Server> {
    let mut list: Vec<&RawGuild> = guilds.iter().collect();
    if !ordered {
        list.sort_by_cached_key(|g| match known.server_rank(&g.id) {
            Some(rank) => (0, rank, String::new()),
            None => (1, 0, g.name.to_lowercase()),
        });
    }
    list.into_iter()
        .map(|g| Server {
            id: g.id.clone(),
            name: g.name.clone(),
            icon: Pic::icon(&g.id, &g.icon).and_then(|pic| art.get(&pic)),
        })
        .collect()
}

/// A server's voice and stage channels in the order the page gave, with who is in each.
pub fn channels(raw: &RawServer, art: &mut Art) -> Vec<VoiceChannel> {
    raw.channels
        .iter()
        .map(|c| VoiceChannel {
            id: c.id.clone(),
            name: c.name.clone(),
            occupants: Some(
                c.users
                    .iter()
                    .map(|u| Pic::avatar(&u.id, &u.avatar).and_then(|pic| art.get(&pic)))
                    .collect::<Vec<Option<PathBuf>>>(),
            ),
        })
        .collect()
}

pub fn shortcuts(known: &Known, art: &mut Art) -> Vec<Shortcut> {
    known
        .shortcuts()
        .iter()
        .map(|k| Shortcut {
            id: k.id.clone(),
            name: k.name.clone().unwrap_or_else(|| CALL.to_string()),
            server: k.server_name.clone(),
            picture: k.picture().and_then(|pic| art.get(&pic)),
            dm: k.guild.is_none(),
        })
        .collect()
}

/// The log lines a new voice report makes: counts and states only.
pub fn log_lines(before: Option<&RawVoice>, now: &RawVoice) -> Vec<String> {
    let mut out = Vec::new();
    let was_in = before.is_some_and(|b| !b.channel.is_empty());
    let on = |b: bool| if b { "on" } else { "off" };
    if !now.channel.is_empty() {
        let changed = before.is_none_or(|b| b.channel != now.channel || b.people.len() != now.people.len());
        if changed {
            let n = now.people.len();
            out.push(format!("discord web: in a call, {n} {}", if n == 1 { "person" } else { "people" }));
        }
    } else if was_in {
        out.push("discord web: left the call".into());
    }
    if let Some(b) = before {
        if b.mute != now.mute {
            out.push(format!("discord web: mute {}", on(now.mute)));
        }
        if b.deaf != now.deaf {
            out.push(format!("discord web: deafen {}", on(now.deaf)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(json: &str) -> RawVoice {
        serde_json::from_str(json).unwrap()
    }

    const CALL_JSON: &str = r#"{"me":"Me","channel":"50","title":"Lounge","guild":{"id":"4","name":"Place","icon":"gh"},"dm":false,
        "people":[{"id":"7","name":"Ana","avatar":"a1","muted":true,"deafened":false,"me":false},
                  {"id":"9","name":"Me","avatar":"","muted":false,"deafened":false,"me":true}],
        "speaking":["7"],"mute":false,"deaf":false,"rtc":true,"in":true}"#;

    fn art() -> Art {
        Art::new(None, None, std::sync::Arc::new(|| {}))
    }

    #[test]
    fn the_call_has_the_user_first_then_discords_order_with_who_speaks() {
        let c = call(&raw(CALL_JSON), &mut art()).unwrap();
        assert_eq!((c.id.as_str(), c.title.as_str()), ("50", "Lounge"));
        assert_eq!(
            c.people.iter().map(|p| (p.name.as_str(), p.me, p.speaking, p.muted)).collect::<Vec<_>>(),
            [("Me", true, false, false), ("Ana", false, true, true)]
        );
        assert!(call(&raw(r#"{"channel":"","mute":true}"#), &mut art()).is_none(), "out of a call");
    }

    #[test]
    fn a_dm_is_headed_by_the_others_else_call() {
        let dm = raw(
            r#"{"channel":"5","dm":true,"people":[{"id":"1","name":"Ana"},{"id":"2","name":"Bo"},{"id":"3","name":"Me","me":true}]}"#,
        );
        assert_eq!(title(&dm), "Ana, Bo");
        assert_eq!(title(&raw(r#"{"channel":"5","dm":true,"people":[{"id":"3","name":"Me","me":true}]}"#)), CALL);
        assert_eq!(title(&raw(r#"{"channel":"5","dm":true,"title":"Group"}"#)), "Group");
    }

    #[test]
    fn a_call_teaches_the_known_channels_only_once_the_user_is_in_it() {
        let j = joined(&raw(CALL_JSON)).unwrap();
        assert_eq!(
            (j.id.as_str(), j.guild.as_deref(), j.name.as_deref(), j.server_name.as_deref()),
            ("50", Some("4"), Some("Lounge"), Some("Place"))
        );
        assert_eq!(j.image, Pic::icon("4", "gh"));
        let connecting = raw(r#"{"channel":"50","in":false,"rtc":false}"#);
        assert!(joined(&connecting).is_none());
        let dm = joined(&raw(r#"{"channel":"5","dm":true,"in":true,"people":[{"id":"1","name":"Ana","avatar":"x"},{"id":"3","name":"Me","me":true}]}"#)).unwrap();
        assert_eq!((dm.guild, dm.name.as_deref(), dm.image), (None, Some("Ana"), Pic::avatar("1", "x")));
        let unnamed =
            joined(&raw(r#"{"channel":"5","dm":true,"in":true,"people":[{"id":"3","name":"Me","me":true}]}"#)).unwrap();
        assert_eq!(unnamed.name, None, "a DM with nobody else has no real heading yet");
    }

    #[test]
    fn servers_keep_the_users_order_else_the_bundles() {
        let g = |id: &str, name: &str| RawGuild { id: id.into(), name: name.into(), icon: String::new() };
        let list = [g("1", "beta"), g("2", "Alpha"), g("3", "gamma")];
        let names = |v: Vec<Server>| v.into_iter().map(|s| s.name).collect::<Vec<_>>();
        let mut known = Known::default();
        assert_eq!(names(servers(&list, true, &known, &mut art())), ["beta", "Alpha", "gamma"], "the user's own order");
        assert_eq!(
            names(servers(&list, false, &known, &mut art())),
            ["Alpha", "beta", "gamma"],
            "by name ignoring case"
        );
        known.learn(&Joined { id: "9".into(), guild: Some("3".into()), name: None, server_name: None, image: None });
        assert_eq!(names(servers(&list, false, &known, &mut art())), ["gamma", "Alpha", "beta"], "joined in first");
    }

    #[test]
    fn channels_are_listed_with_who_is_in_them() {
        let server: RawServer = serde_json::from_str(r#"{"id":"4","channels":[{"id":"1","name":"Lounge","type":2,"users":[{"id":"7","avatar":""}]},{"id":"2","name":"Stage","type":13,"users":[]}]}"#).unwrap();
        let c = channels(&server, &mut art());
        assert_eq!((c[0].name.as_str(), c[0].occupants.as_ref().map(Vec::len)), ("Lounge", Some(1)));
        assert_eq!(c[1].occupants, Some(Vec::new()), "nobody: Empty");
    }

    #[test]
    fn the_log_says_counts_and_states_only() {
        let a = raw(CALL_JSON);
        let lines = log_lines(None, &a);
        assert_eq!(lines, ["discord web: in a call, 2 people"]);
        let mut b = a.clone();
        b.mute = true;
        b.deaf = true;
        assert_eq!(log_lines(Some(&a), &b), ["discord web: mute on", "discord web: deafen on"]);
        let out = RawVoice { mute: true, deaf: true, ..RawVoice::default() };
        assert_eq!(log_lines(Some(&b), &out), ["discord web: left the call"]);
        assert!(log_lines(Some(&a), &a).is_empty());
        for line in lines.iter().chain(&log_lines(Some(&a), &b)) {
            assert!(!line.contains("Ana") && !line.contains("Lounge") && !line.contains("Place"));
        }
    }
}
