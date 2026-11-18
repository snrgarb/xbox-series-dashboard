//! The index (#292): every server, every channel with its id and kind, every DM with its id, and who is in each
//! voice channel, as Discord's in-page stores give it (`js/index_store.js`), kept in memory. Names are resolved to
//! ids here (exact, then ignoring case, then contained; several is an answer too) so the voice steps go straight to
//! a channel by its route. The log gets counts only; names and ids go to `index.txt` (`--index-file`).
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Guild {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub guild: String,
    pub id: String,
    pub name: String,
    pub kind: u32,
    pub parent: String,
    pub position: i64,
    pub users: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dm {
    pub id: String,
    pub kind: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Index {
    pub guilds: Vec<Guild>,
    pub channels: Vec<Channel>,
    pub dms: Vec<Dm>,
}

/// Discord's channel types, as its API documents them.
pub fn kind_name(kind: u32) -> &'static str {
    match kind {
        0 => "text",
        1 => "dm",
        2 => "voice",
        3 => "group dm",
        4 => "category",
        5 => "announcement",
        13 => "stage",
        15 => "forum",
        16 => "media",
        10..=12 => "thread",
        _ => "other",
    }
}

pub fn is_voice(kind: u32) -> bool {
    kind == 2 || kind == 13
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

impl Index {
    /// From `js/index_store.js`'s answer.
    pub fn from_json(v: &Value) -> Index {
        let list = |k: &str| v[k].as_array().cloned().unwrap_or_default();
        Index {
            guilds: list("guilds").iter().map(|g| Guild { id: s(&g["id"]), name: s(&g["name"]) }).collect(),
            channels: list("channels")
                .iter()
                .map(|c| Channel {
                    guild: s(&c["guild"]),
                    id: s(&c["id"]),
                    name: s(&c["name"]),
                    kind: c["type"].as_u64().unwrap_or(99) as u32,
                    parent: s(&c["parent"]),
                    position: c["position"].as_i64().unwrap_or(0),
                    users: c["users"].as_u64().unwrap_or(0),
                })
                .collect(),
            dms: list("dms")
                .iter()
                .map(|d| Dm { id: s(&d["id"]), kind: d["type"].as_u64().unwrap_or(1) as u32, name: s(&d["name"]) })
                .collect(),
        }
    }

    /// The log's line: counts by kind, never a name.
    pub fn counts(&self) -> String {
        let mut kinds: std::collections::BTreeMap<&str, usize> = Default::default();
        for c in &self.channels {
            *kinds.entry(kind_name(c.kind)).or_default() += 1;
        }
        let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let in_voice: u64 = self.channels.iter().filter(|c| is_voice(c.kind)).map(|c| c.users).sum();
        let busy = self.channels.iter().filter(|c| is_voice(c.kind) && c.users > 0).count();
        format!(
            "servers {}, channels {} ({}), DMs {} (group {}), users in voice channels now {in_voice} (in {busy} channels)",
            self.guilds.len(),
            self.channels.len(),
            kinds.join(", "),
            self.dms.len(),
            self.dms.iter().filter(|d| d.kind == 3).count()
        )
    }

    /// For `index.txt`: one line per server, channel and DM, with ids. Private.
    pub fn text(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let mut out = String::from(
            "# discord-spike index. PRIVATE: keep it out of the repository.\n# server\tid\tname\n# channel\tserver id\tid\tkind\tparent id\tposition\tusers in it\tname\n# dm\tid\tkind\tname\n",
        );
        for g in &self.guilds {
            out.push_str(&format!("server\t{}\t{}\n", g.id, clean(&g.name)));
            let mut chans: Vec<&Channel> = self.channels.iter().filter(|c| c.guild == g.id).collect();
            chans.sort_by_key(|c| (c.position, c.kind));
            for c in chans {
                out.push_str(&format!(
                    "channel\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    c.guild,
                    c.id,
                    kind_name(c.kind),
                    c.parent,
                    c.position,
                    c.users,
                    clean(&c.name)
                ));
            }
        }
        for d in &self.dms {
            out.push_str(&format!("dm\t{}\t{}\t{}\n", d.id, kind_name(d.kind), clean(&d.name)));
        }
        out
    }
}

/// How a name was matched.
#[derive(Debug, PartialEq)]
pub enum Found<'a, T> {
    One(&'a T, &'static str),
    Many(Vec<&'a T>, &'static str),
    None,
}

/// A name to one item: exact, then ignoring case, then contained (ignoring case). Several at the first level that
/// matches any is `Many`.
pub fn resolve<'a, T>(items: &'a [T], name: impl Fn(&T) -> &str, target: &str) -> Found<'a, T> {
    let t = target.trim();
    let lower = t.to_lowercase();
    type Test<'t> = Box<dyn Fn(&str) -> bool + 't>;
    let levels: [(&'static str, Test); 3] = [
        ("exactly", Box::new(|n: &str| n == t)),
        ("ignoring case", Box::new(|n: &str| n.to_lowercase() == lower)),
        ("by a part of it", Box::new(|n: &str| !lower.is_empty() && n.to_lowercase().contains(&lower))),
    ];
    for (how, test) in levels.iter() {
        let hits: Vec<&T> = items.iter().filter(|i| test(name(i))).collect();
        match hits.len() {
            0 => continue,
            1 => return Found::One(hits[0], how),
            _ => return Found::Many(hits, how),
        }
    }
    Found::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn index() -> Index {
        Index::from_json(&json!({
            "guilds": [{ "id": "1", "name": "Games" }, { "id": "2", "name": "Work" }],
            "channels": [
                { "guild": "1", "id": "10", "name": "general", "type": 0, "parent": "", "position": 0, "users": 0 },
                { "guild": "1", "id": "11", "name": "Lounge", "type": 2, "parent": "", "position": 1, "users": 2 },
                { "guild": "1", "id": "12", "name": "Lounge 2", "type": 2, "parent": "", "position": 2, "users": 0 },
                { "guild": "2", "id": "20", "name": "Lounge", "type": 2, "parent": "", "position": 0, "users": 0 },
                { "guild": "1", "id": "13", "name": "Voice", "type": 4, "parent": "", "position": 3, "users": 0 }
            ],
            "dms": [{ "id": "30", "type": 1, "name": "Alex" }, { "id": "31", "type": 3, "name": "Alex, Sam" }]
        }))
    }

    #[test]
    fn counts_never_names() {
        let c = index().counts();
        assert_eq!(
            c,
            "servers 2, channels 5 (category 1, text 1, voice 3), DMs 2 (group 1), users in voice channels now 2 (in 1 channels)"
        );
        assert!(!c.contains("Lounge") && !c.contains("Alex"));
    }

    #[test]
    fn names_resolve_exact_then_case_then_part() {
        let i = index();
        let in_games: Vec<Channel> = i.channels.iter().filter(|c| c.guild == "1").cloned().collect();
        assert!(matches!(resolve(&in_games, |c| &c.name, "Lounge"), Found::One(c, "exactly") if c.id == "11"));
        assert!(matches!(resolve(&in_games, |c| &c.name, "lounge 2"), Found::One(c, "ignoring case") if c.id == "12"));
        assert!(matches!(resolve(&in_games, |c| &c.name, "ounge"), Found::Many(v, "by a part of it") if v.len() == 2));
        assert!(matches!(resolve(&in_games, |c| &c.name, "nothing"), Found::None));
        assert!(matches!(resolve(&i.guilds, |g| &g.name, "work"), Found::One(g, _) if g.id == "2"));
        assert!(matches!(resolve(&i.dms, |d| &d.name, "alex"), Found::One(d, "ignoring case") if d.id == "30"));
    }

    #[test]
    fn the_text_has_ids_and_kinds() {
        let t = index().text();
        assert!(t.contains("server\t1\tGames\n"));
        assert!(t.contains("channel\t1\t11\tvoice\t\t1\t2\tLounge\n"));
        assert!(t.contains("dm\t31\tgroup dm\tAlex, Sam\n"));
    }

    /// The in-page store scripts never touch anything credential-like.
    #[test]
    fn the_store_scripts_hold_no_credential_words() {
        for (name, js) in [
            ("store_find.js", include_str!("js/store_find.js")),
            ("index_store.js", include_str!("js/index_store.js")),
            ("store_voice.js", include_str!("js/store_voice.js")),
            ("store_join.js", include_str!("js/store_join.js")),
        ] {
            let lower = js.to_lowercase();
            for word in [
                "token",
                "localstorage",
                "authorization",
                "cookie",
                "password",
                "sessionstorage",
                "indexeddb",
                "fetch(",
                "xmlhttprequest",
            ] {
                assert!(!lower.contains(word), "{name} mentions {word}");
            }
        }
    }
}
