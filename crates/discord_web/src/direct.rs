//! What the page script pushes (`js/direct.js`), and what the screens get of it (#198; docs/discord-web.md,
//! "Reading"): the JSON snapshots, parsed; their mapping to `model::discord_dms`; and the events (a message in a DM
//! that is not open, a ring starting or ending) found by comparing successive snapshots.
//!
//! Nothing here logs a name, an id, a text or an address: [`Reader::take`] hands back log lines made of counts and of
//! our own store keys and Discord's method names, which the session says.
use std::collections::{BTreeMap, HashMap};

use model::discord_dms::*;
use serde::Deserialize;

/// One message from the page, tagged by `k`.
#[derive(Deserialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum Page {
    Health(Health),
    Dms {
        dms: Vec<RawDm>,
    },
    Conv(RawConv),
    Rings {
        rings: Vec<RawRing>,
    },
    /// The user is gone from the page (signed out there).
    Reset,
    /// How the action the host asked for is going (`js/act.js`).
    Act(Act),
    /// A conversation the host opened had unread messages: whether the page marked it read by itself.
    Opened(Opened),
    /// The user's Call and own mute and deafen (`js/voice.js`).
    Voice(crate::voice::RawVoice),
    /// The user's servers.
    Guilds {
        ordered: bool,
        servers: Vec<crate::voice::RawGuild>,
    },
    /// The voice channels of the server being browsed.
    Server(crate::voice::RawServer),
}

/// One word from the page about the action it was asked for (#201): `typing` (the message box is focused, type now),
/// `sending` (Discord's optimistic message, `mid`), `sent` (the real `mid`), `done` (a reaction or a read), `captcha`,
/// or `failed` with `why`, one of a fixed set of words, and `reason`, Discord's own words where its page gave any.
/// No `Debug`: a reason is Discord's text.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Act {
    pub seq: u64,
    pub stage: String,
    pub mid: String,
    pub why: String,
    pub reason: String,
    /// With `typing`: the box held a draft, to be cleared first.
    pub draft: bool,
    /// With a mute or deafen `done`: the switch as it now stands.
    pub on: bool,
    /// With a join `done` that was to ring the DM: `yes`, `no` (Discord's call action was not found or threw), or empty.
    pub ring: String,
}

#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Opened {
    /// The page's own count of the conversation's unread messages went to 0 within a few seconds.
    pub read: bool,
    /// The page's own read action was called, once, because it did not.
    pub acked: bool,
}

/// Which stores and methods the script found: our keys, Discord's method names.
#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Health {
    pub ok: bool,
    pub found: BTreeMap<String, Vec<String>>,
    pub missing: Vec<String>,
    /// What voice lacks of its required stores and methods (`js/voice.js`): empty is ready. Never a break of the DMs.
    pub voice_missing: Vec<String>,
    /// Discord's own older-messages action was found.
    pub older: bool,
    pub tries: u32,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawDm {
    pub id: String,
    pub name: String,
    pub group: bool,
    pub members: u32,
    /// A path on the CDN (`avatars/<user>/<hash>.png?size=128`); empty for none.
    pub pic: String,
    pub unread: u32,
    pub mentions: u32,
    pub last: Option<RawLast>,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawLast {
    pub at: i64,
    /// Absent where the stores hold no message of the channel: only its time (from the id) is known.
    pub text: Option<String>,
    pub who: String,
    pub names: HashMap<String, String>,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawRing {
    pub channel: String,
    pub caller: String,
    pub pic: String,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawConv {
    pub id: String,
    pub loaded: bool,
    /// Discord's store says older messages exist (`hasMoreBefore`).
    pub more: bool,
    /// Discord's older-messages action is running.
    pub older: bool,
    pub unread_from: String,
    pub msgs: Vec<RawMsg>,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawAuthor {
    pub id: String,
    pub name: String,
    pub pic: String,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawReply {
    /// `to` (a message, loaded), `deleted`, or `none` (not loaded).
    pub s: String,
    pub who: String,
    pub tx: String,
    pub names: HashMap<String, String>,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawFile {
    pub ct: String,
    pub name: String,
    pub voice: bool,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawEmbed {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub site: String,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawReaction {
    /// A server emoji's id; empty for a Unicode one.
    pub id: String,
    pub name: String,
    pub count: u32,
    pub me: bool,
}

#[derive(Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RawMsg {
    pub id: String,
    pub author: RawAuthor,
    pub own: bool,
    pub at: i64,
    pub edited: bool,
    #[serde(rename = "type")]
    pub kind: u32,
    pub text: String,
    pub names: HashMap<String, String>,
    /// Discord's message state: `SENT`, `SENDING`, `SEND_FAILED`.
    pub state: String,
    pub reply: Option<RawReply>,
    pub mentioned: Vec<String>,
    pub files: Vec<RawFile>,
    pub stickers: Vec<String>,
    pub embeds: Vec<RawEmbed>,
    pub poll: Option<String>,
    pub reactions: Vec<RawReaction>,
}

/// A run of text as spans: users' mentions by the names the page resolved, links, a server's emoji; the rest as text.
pub fn spans(text: &str, names: &HashMap<String, String>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span::Text(Private(std::mem::take(plain))));
        }
    };
    let mut rest = text;
    while !rest.is_empty() {
        if let Some((len, token)) = token(rest, names) {
            match token {
                Token::Text(t) => plain.push_str(&t),
                other => {
                    flush(&mut plain, &mut out);
                    out.push(match other {
                        Token::Mention(m) => Span::Mention(Private(m)),
                        Token::Link(l) => Span::Link(Private(l)),
                        Token::Emoji(e) => Span::ServerEmoji(Private(e)),
                        Token::Text(_) => unreachable!(),
                    });
                }
            }
            rest = &rest[len..];
        } else {
            let c = rest.chars().next().unwrap_or(' ');
            plain.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    flush(&mut plain, &mut out);
    out
}

enum Token {
    Text(String),
    Mention(String),
    Link(String),
    Emoji(String),
}

/// A mention, a channel's, a server emoji or a link at the start of `rest`: its length in bytes, and what it is.
fn token(rest: &str, names: &HashMap<String, String>) -> Option<(usize, Token)> {
    let digits = |s: &str| s.chars().take_while(char::is_ascii_digit).count();
    if let Some(after) = rest.strip_prefix("<@") {
        let (role, after) = match after.strip_prefix('&') {
            Some(a) => (true, a),
            None => (false, after.strip_prefix('!').unwrap_or(after)),
        };
        let n = digits(after);
        if n > 0 && after[n..].starts_with('>') {
            let id = &after[..n];
            let len = rest.len() - after.len() + n + 1;
            let name = if role {
                "role".to_string()
            } else {
                names.get(id).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| "user".into())
            };
            return Some((len, Token::Mention(format!("@{name}"))));
        }
    }
    if let Some(after) = rest.strip_prefix("<#") {
        let n = digits(after);
        if n > 0 && after[n..].starts_with('>') {
            let id = &after[..n];
            let name =
                names.get(&format!("#{id}")).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| "channel".into());
            return Some((2 + n + 1, Token::Text(format!("#{name}"))));
        }
    }
    if let Some(after) = rest.strip_prefix("<a:").or_else(|| rest.strip_prefix("<:")) {
        if let Some((name, tail)) = after.split_once(':') {
            let n = digits(tail);
            if !name.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                && n > 0
                && tail[n..].starts_with('>')
            {
                return Some((rest.len() - tail.len() + n + 1, Token::Emoji(name.to_string())));
            }
        }
    }
    let (wrapped, url) = match rest.strip_prefix('<') {
        Some(inner) if inner.starts_with("http://") || inner.starts_with("https://") => (true, inner),
        _ if rest.starts_with("http://") || rest.starts_with("https://") => (false, rest),
        _ => return None,
    };
    let end = url.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(url.len());
    let mut link = &url[..end];
    if !wrapped {
        link = link.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '\'', '"']);
    }
    if link.len() <= "https://".len() {
        return None;
    }
    let mut len = link.len() + usize::from(wrapped);
    if wrapped && url[end..].starts_with('>') {
        len += 1;
    }
    Some((len, Token::Link(link.to_string())))
}

/// The text of spans, as plain words.
pub fn plain(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) | Span::Mention(t) | Span::Link(t) => t.0.clone(),
            Span::ServerEmoji(e) => format!(":{}:", e.0),
        })
        .collect()
}

/// A message's first non-empty line, as plain words, at most 200 characters.
pub fn first_line(text: &str, names: &HashMap<String, String>) -> String {
    let line = plain(&spans(text, names)).lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
    line.chars().take(200).collect()
}

/// What isn't text in a message, a line each, in the order the message holds them.
pub fn extras(raw: &RawMsg) -> Vec<Extra> {
    let mut out: Vec<Extra> = Vec::new();
    let mut images = 0u32;
    let mut images_at: Option<usize> = None;
    let mut image = |out: &mut Vec<Extra>| {
        images += 1;
        match images_at {
            Some(i) => out[i] = Extra::Images(images),
            None => {
                images_at = Some(out.len());
                out.push(Extra::Images(images));
            }
        }
    };
    for f in &raw.files {
        let ct = f.ct.to_ascii_lowercase();
        if f.voice {
            out.push(Extra::VoiceMessage);
        } else if ct == "image/gif" {
            out.push(Extra::Gif);
        } else if ct.starts_with("image/") {
            image(&mut out);
        } else if ct.starts_with("video/") {
            out.push(Extra::Video);
        } else {
            out.push(Extra::File(Private(f.name.clone())));
        }
    }
    for s in &raw.stickers {
        out.push(Extra::Sticker(Private(s.clone())));
    }
    for e in &raw.embeds {
        match e.kind.as_str() {
            "gifv" => out.push(Extra::Gif),
            "image" => image(&mut out),
            _ if !e.title.is_empty() => out.push(Extra::Embed(Private(e.title.clone()))),
            _ if !e.site.is_empty() => out.push(Extra::Embed(Private(e.site.clone()))),
            _ => out.push(Extra::Other),
        }
    }
    if let Some(q) = &raw.poll {
        out.push(Extra::Poll(Private(q.clone())));
    }
    out
}

/// Whether a message of this Discord type is someone's own words (a bubble), rather than one of Discord's lines:
/// default (0), reply (19), a command's (20, 23).
fn said(kind: u32) -> bool {
    matches!(kind, 0 | 19 | 20 | 23)
}

/// The message as the screens draw it. `pictures` gives a picture for a CDN path, where it has come.
pub fn message(raw: &RawMsg, pictures: &dyn Fn(&str) -> Option<Picture>) -> Message {
    let author = if raw.author.name.is_empty() { "Someone" } else { raw.author.name.as_str() };
    let body = if said(raw.kind) {
        let text = spans(&raw.text, &raw.names);
        let reply = raw.reply.as_ref().and_then(|r| match r.s.as_str() {
            "to" => Some(Reply::To { author: Private(r.who.clone()), line: Private(first_line(&r.tx, &r.names)) }),
            "deleted" => Some(Reply::Deleted),
            _ => None,
        });
        let mut extras = extras(raw);
        if text.is_empty() && extras.is_empty() {
            extras.push(Extra::Other);
        }
        let reactions = raw
            .reactions
            .iter()
            .map(|r| Reaction {
                emoji: if r.id.is_empty() {
                    Emoji::Unicode(Private(r.name.clone()))
                } else {
                    Emoji::Server(Private(r.name.clone()))
                },
                count: r.count,
                mine: r.me,
            })
            .collect();
        Body::Said { text, reply, extras, reactions }
    } else {
        let (kind, text) = match raw.kind {
            3 => (LineKind::CallStarted, format!("{author} started a call")),
            1 => (
                LineKind::Added,
                match raw.mentioned.first() {
                    Some(who) => format!("{author} added {who}"),
                    None => format!("{author} added someone"),
                },
            ),
            6 => (LineKind::Pinned, format!("{author} pinned a message")),
            _ => (LineKind::Other, first_line(&raw.text, &raw.names)),
        };
        Body::Line { kind, text: Private(text) }
    };
    Message {
        id: Id(raw.id.clone()),
        author: Id(raw.author.id.clone()),
        author_name: Private(raw.author.name.clone()),
        author_picture: pictures(&raw.author.pic),
        own: raw.own,
        at: raw.at,
        edited: raw.edited,
        body,
        delivery: match raw.state.as_str() {
            "SENDING" => Delivery::Sending,
            "SEND_FAILED" => Delivery::Failed,
            _ => Delivery::Sent,
        },
    }
}

/// What taking one page message did.
#[derive(Default)]
pub struct Taken {
    /// What the screens show may have changed.
    pub changed: bool,
    pub events: Vec<DirectEvent>,
    /// Log lines (counts, our keys, Discord's method names).
    pub log: Vec<String>,
    /// The user is gone from the page.
    pub reset: bool,
    /// The page's word about the action it was asked for.
    pub act: Option<Act>,
    /// The Call, the servers or the browsed server's channels changed.
    pub voice: bool,
}

/// What the page has said, as the session keeps it until it is forgotten.
#[derive(Default)]
pub struct Reader {
    pub health: Option<Health>,
    pub dms: Option<Vec<RawDm>>,
    pub conv: Option<RawConv>,
    pub rings: Vec<RawRing>,
    /// Voice (#297): the Call, the servers, and the browsed server's channels, as the page last said.
    pub voice: Option<crate::voice::RawVoice>,
    pub guilds: Option<(bool, Vec<crate::voice::RawGuild>)>,
    pub server: Option<crate::voice::RawServer>,
    /// Each DM's unread count in the last list: what a rise is measured against. A rise is raised only once a list has
    /// been seen, so what was unread when the session began raises nothing.
    unread: HashMap<String, u32>,
    listed: bool,
    last_counts: String,
    last_conv: usize,
}

impl Reader {
    /// A reader that has seen nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets everything the page said.
    pub fn forget(&mut self) {
        *self = Reader::default();
    }

    /// Whether the health probe found a store or method missing.
    pub fn page_changed(&self) -> bool {
        self.health.as_ref().is_some_and(|h| !h.ok)
    }

    /// Whether voice can be read and acted: the page has said, and lacks nothing voice requires.
    pub fn voice_ready(&self) -> bool {
        self.health
            .as_ref()
            .is_some_and(|h| h.voice_missing.is_empty() && h.found.get("voice").is_some_and(|v| !v.is_empty()))
    }

    pub fn older_found(&self) -> bool {
        self.health.as_ref().is_some_and(|h| h.older)
    }

    pub fn dm(&self, id: &str) -> Option<&RawDm> {
        self.dms.as_ref()?.iter().find(|d| d.id == id)
    }

    /// Takes one message from the page; `open` is the conversation the screens asked for. A message that does not
    /// parse changes nothing.
    pub fn take(&mut self, json: &str, open: Option<&str>) -> Taken {
        let mut taken = Taken::default();
        let Ok(page) = serde_json::from_str::<Page>(json) else { return taken };
        match page {
            Page::Health(health) => {
                if self.health.as_ref() != Some(&health) {
                    taken.log.extend(health_lines(self.health.as_ref(), &health));
                    self.health = Some(health);
                    taken.changed = true;
                }
            }
            Page::Dms { dms } => {
                let mut events = Vec::new();
                if self.listed {
                    for dm in &dms {
                        let was = self.unread.get(&dm.id).copied().unwrap_or(0);
                        if dm.unread > was && open != Some(dm.id.as_str()) {
                            events.push(message_event(dm));
                        }
                    }
                }
                self.unread = dms.iter().map(|d| (d.id.clone(), d.unread)).collect();
                self.listed = true;
                let counts = counts(&dms);
                if counts != self.last_counts {
                    taken.log.push(format!("discord web: {counts}"));
                    self.last_counts = counts;
                }
                taken.events = events;
                taken.changed = self.dms.as_ref() != Some(&dms);
                self.dms = Some(dms);
            }
            Page::Conv(conv) => {
                if open != Some(conv.id.as_str()) {
                    return taken;
                }
                if conv.loaded && (self.last_conv != conv.msgs.len() || self.conv.as_ref().is_none_or(|c| !c.loaded)) {
                    taken.log.push(format!("discord web: conversation open, {} messages", conv.msgs.len()));
                    self.last_conv = conv.msgs.len();
                }
                taken.changed = self.conv.as_ref() != Some(&conv);
                self.conv = Some(conv);
            }
            Page::Rings { rings } => {
                for r in &rings {
                    if !self.rings.iter().any(|p| p.channel == r.channel) {
                        taken.events.push(DirectEvent::Ring(Ring {
                            channel: Id(r.channel.clone()),
                            caller: Private(r.caller.clone()),
                            picture: None,
                        }));
                    }
                }
                for p in &self.rings {
                    if !rings.iter().any(|r| r.channel == p.channel) {
                        taken.events.push(DirectEvent::RingEnded(Id(p.channel.clone())));
                    }
                }
                taken.changed = self.rings != rings;
                self.rings = rings;
            }
            Page::Reset => {
                taken.reset = true;
                taken.changed = true;
            }
            Page::Voice(voice) => {
                if self.voice.as_ref() != Some(&voice) {
                    taken.log.extend(crate::voice::log_lines(self.voice.as_ref(), &voice));
                    self.voice = Some(voice);
                    taken.voice = true;
                }
            }
            Page::Guilds { ordered, servers } => {
                if self.guilds.as_ref() != Some(&(ordered, servers.clone())) {
                    self.guilds = Some((ordered, servers));
                    taken.voice = true;
                }
            }
            Page::Server(server) => {
                if self.server.as_ref() != Some(&server) {
                    self.server = Some(server);
                    taken.voice = true;
                }
            }
            Page::Act(act) => taken.act = Some(act),
            Page::Opened(opened) => {
                taken.log.push(format!(
                    "discord web: conversation open, read by the page {}",
                    if opened.read { "yes" } else { "no" }
                ));
            }
        }
        taken
    }
}

/// `25 DMs, 5 unread in 2`: the spec's log line.
pub fn counts(dms: &[RawDm]) -> String {
    let unread = dms.iter().filter(|d| d.unread > 0);
    format!("{} DMs, {} unread in {}", dms.len(), unread.clone().map(|d| d.unread).sum::<u32>(), unread.count())
}

fn message_event(dm: &RawDm) -> DirectEvent {
    let (line, from) = match &dm.last {
        Some(last) => (last.text.as_deref().map(|t| first_line(t, &last.names)).unwrap_or_default(), last.who.clone()),
        None => (String::new(), String::new()),
    };
    DirectEvent::Message {
        channel: Id(dm.id.clone()),
        name: Private(if dm.group && !from.is_empty() { from } else { dm.name.clone() }),
        group: dm.group.then(|| Private(dm.name.clone())),
        line: Private(line),
        picture: None,
    }
}

/// The log lines a health report makes: the stores found when it is first seen or changes, and each that no longer
/// answers. Our keys and Discord's method names only. Voice's stores have their own lines.
fn health_lines(before: Option<&Health>, now: &Health) -> Vec<String> {
    use crate::voice::is_voice_key;
    let mut out = Vec::new();
    let keys = |h: &Health, voice: bool| {
        h.found
            .iter()
            .filter(|(k, v)| !v.is_empty() && is_voice_key(k) == voice)
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>()
    };
    if before.map(|h| keys(h, false)).as_ref() != Some(&keys(now, false)) {
        out.push(format!("discord web: stores found: {}", keys(now, false).join(", ")));
    }
    if before.map(|h| keys(h, true)).unwrap_or_default() != keys(now, true) {
        out.push(format!("discord web: voice stores found: {}", keys(now, true).join(", ")));
    }
    for m in &now.missing {
        if before.is_some_and(|b| b.missing.contains(m)) {
            continue;
        }
        out.push(match m.split_once('.') {
            Some((key, method)) => format!("discord web: store \"{key}\" no longer answers ({method})"),
            None if m.contains(' ') => format!("discord web: the page's scripts: {m}"),
            None => format!("discord web: store \"{m}\" no longer answers"),
        });
    }
    for m in &now.voice_missing {
        if before.is_some_and(|b| b.voice_missing.contains(m)) {
            continue;
        }
        out.push(match m.split_once('.') {
            Some((key, method)) => format!("discord web: voice store \"{key}\" no longer answers ({method})"),
            None => format!("discord web: voice store \"{m}\" no longer answers"),
        });
    }
    if now.ok && before.is_some_and(|b| !b.ok) {
        out.push("discord web: the stores answer again".into());
    }
    if now.voice_missing.is_empty() && before.is_some_and(|b| !b.voice_missing.is_empty()) {
        out.push("discord web: the voice stores answer again".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    fn text(s: &str) -> Span {
        Span::Text(s.into())
    }

    #[test]
    fn text_becomes_spans() {
        let n = names(&[("42", "Ana"), ("#7", "general")]);
        assert_eq!(
            spans("hi <@42> and <@!42>, <@&9> see https://x.test/a?b=1. ok <:wave:123> <a:dance:5> <#7>", &n),
            vec![
                text("hi "),
                Span::Mention("@Ana".into()),
                text(" and "),
                Span::Mention("@Ana".into()),
                text(", "),
                Span::Mention("@role".into()),
                text(" see "),
                Span::Link("https://x.test/a?b=1".into()),
                text(". ok "),
                Span::ServerEmoji("wave".into()),
                text(" "),
                Span::ServerEmoji("dance".into()),
                text(" #general"),
            ]
        );
        assert_eq!(spans("<@1>", &HashMap::new()), vec![Span::Mention("@user".into())], "an unknown user");
        assert_eq!(spans("<https://x.test/p> end", &n), vec![Span::Link("https://x.test/p".into()), text(" end")]);
        assert_eq!(spans("a < b <@> <:x:> http:// \u{1F44D}", &n), vec![text("a < b <@> <:x:> http:// \u{1F44D}")]);
        assert!(spans("", &n).is_empty());
        assert_eq!(first_line("\n  second\nthird", &n), "second");
        assert_eq!(first_line("<@42>: yes", &n), "@Ana: yes");
        assert_eq!(first_line(&"x".repeat(500), &n).chars().count(), 200);
    }

    fn raw(json: &str) -> RawMsg {
        serde_json::from_str(json).unwrap()
    }

    fn map(json: &str) -> Message {
        message(&raw(json), &|_| None)
    }

    fn said_parts(m: &Message) -> (&[Span], Option<&Reply>, &[Extra], &[Reaction]) {
        match &m.body {
            Body::Said { text, reply, extras, reactions } => (text, reply.as_ref(), extras, reactions),
            other => panic!("not said: {other:?}"),
        }
    }

    #[test]
    fn every_extra() {
        let m = map(r#"{"id":"1","author":{"id":"2","name":"Ana"},"own":false,"at":5,"text":"look","files":[
                {"ct":"image/png","name":"a.png"},{"ct":"image/gif","name":"b.gif"},{"ct":"image/jpeg","name":"c.jpg"},
                {"ct":"video/mp4","name":"d.mp4"},{"ct":"audio/ogg","name":"e.ogg","voice":true},{"ct":"application/pdf","name":"f.pdf"}],
              "stickers":["Wave"],
              "embeds":[{"type":"article","title":"A Title","site":"Site"},{"type":"link","title":"","site":"Only Site"},
                        {"type":"gifv"},{"type":"image"},{"type":"rich"}],
              "poll":"Which game?"}"#);
        let (text_, reply, extras, reactions) = said_parts(&m);
        assert_eq!(text_, &[text("look")]);
        assert!(reply.is_none() && reactions.is_empty());
        assert_eq!(
            extras,
            &[
                Extra::Images(3),
                Extra::Gif,
                Extra::Video,
                Extra::VoiceMessage,
                Extra::File("f.pdf".into()),
                Extra::Sticker("Wave".into()),
                Extra::Embed("A Title".into()),
                Extra::Embed("Only Site".into()),
                Extra::Gif,
                Extra::Other,
                Extra::Poll("Which game?".into()),
            ]
        );
        // Nothing but something unshowable: "Something the Dashboard can't show".
        let empty = map(r#"{"id":"1","author":{"id":"2","name":"Ana"}}"#);
        assert_eq!(said_parts(&empty).2, &[Extra::Other]);
    }

    #[test]
    fn replies_reactions_edits_delivery_and_ownership() {
        let m = map(
            r#"{"id":"9","author":{"id":"2","name":"Me","pic":"avatars/2/h.png?size=128"},"own":true,"at":7,"edited":true,
                "type":19,"text":"yes","state":"SENDING",
                "reply":{"s":"to","who":"Ana","tx":"are you on <@5>\nsecond","names":{"5":"Bo"}},
                "reactions":[{"id":"","name":"👍","count":2,"me":true},{"id":"77","name":"party","count":1,"me":false}]}"#,
        );
        let (_, reply, _, reactions) = said_parts(&m);
        assert_eq!(reply, Some(&Reply::To { author: "Ana".into(), line: "are you on @Bo".into() }));
        assert_eq!(
            reactions,
            &[
                Reaction { emoji: Emoji::Unicode("\u{1F44D}".into()), count: 2, mine: true },
                Reaction { emoji: Emoji::Server("party".into()), count: 1, mine: false },
            ]
        );
        assert!(m.own && m.edited && m.at == 7);
        assert_eq!(m.delivery, Delivery::Sending);
        assert_eq!(m.id, Id::from("9"));
        let pics = |p: &str| (p == "avatars/2/h.png?size=128").then(|| Picture(vec![1u8].into()));
        assert!(message(&raw(r#"{"author":{"pic":"avatars/2/h.png?size=128"}}"#), &pics).author_picture.is_some());
        assert_eq!(map(r#"{"state":"SEND_FAILED","author":{}}"#).delivery, Delivery::Failed);
        assert_eq!(map(r#"{"state":"SENT","author":{}}"#).delivery, Delivery::Sent);
        let deleted = map(r#"{"type":19,"text":"x","author":{},"reply":{"s":"deleted"}}"#);
        assert_eq!(said_parts(&deleted).1, Some(&Reply::Deleted));
        let unknown = map(r#"{"type":19,"text":"x","author":{},"reply":{"s":"none"}}"#);
        assert_eq!(said_parts(&unknown).1, None, "a reply to a message not loaded shows no line");
    }

    #[test]
    fn discords_own_lines() {
        let line = |json: &str| match map(json).body {
            Body::Line { kind, text } => (kind, text.0),
            other => panic!("{other:?}"),
        };
        assert_eq!(line(r#"{"type":3,"author":{"name":"Ana"}}"#), (LineKind::CallStarted, "Ana started a call".into()));
        assert_eq!(
            line(r#"{"type":1,"author":{"name":"Bo"},"mentioned":["Cy"]}"#),
            (LineKind::Added, "Bo added Cy".into())
        );
        assert_eq!(line(r#"{"type":1,"author":{"name":"Bo"}}"#), (LineKind::Added, "Bo added someone".into()));
        assert_eq!(line(r#"{"type":6,"author":{"name":"Ana"}}"#), (LineKind::Pinned, "Ana pinned a message".into()));
        assert_eq!(
            line(r#"{"type":4,"author":{"name":"Ana"},"text":"New Name"}"#),
            (LineKind::Other, "New Name".into())
        );
    }

    fn dm(id: &str, name: &str, unread: u32) -> String {
        format!(r#"{{"id":"{id}","name":"{name}","members":2,"unread":{unread}}}"#)
    }

    fn dms(list: &[String]) -> String {
        format!(r#"{{"k":"dms","dms":[{}]}}"#, list.join(","))
    }

    #[test]
    fn events_come_from_successive_lists_and_never_for_what_was_unread_at_the_start() {
        let mut r = Reader::new();
        let first = r.take(&dms(&[dm("1", "Ana", 3), dm("2", "Bo", 0)]), None);
        assert!(first.events.is_empty(), "three unread at the start raise nothing");
        assert_eq!(first.log, vec!["discord web: 2 DMs, 3 unread in 1".to_string()]);
        assert!(r.take(&dms(&[dm("1", "Ana", 3), dm("2", "Bo", 0)]), None).events.is_empty());
        // A rise in a DM that is not open.
        let t = r.take(&dms(&[dm("1", "Ana", 3), dm("2", "Bo", 1)]), Some("1"));
        assert_eq!(t.events.len(), 1);
        assert!(matches!(&t.events[0], DirectEvent::Message { channel, name, group: None, line, .. }
            if channel.as_str() == "2" && name.as_str() == "Bo" && line.as_str().is_empty()));
        // A rise in the open one raises nothing; a fall raises nothing.
        assert!(r.take(&dms(&[dm("1", "Ana", 4), dm("2", "Bo", 0)]), Some("1")).events.is_empty());
        // A new DM with unread messages is a message.
        let t = r.take(&dms(&[dm("1", "Ana", 4), dm("3", "Cy", 1), dm("2", "Bo", 0)]), None);
        assert_eq!(t.events.len(), 1);
    }

    #[test]
    fn a_message_event_says_the_first_line_where_the_store_holds_it_and_the_group() {
        let mut r = Reader::new();
        r.take(&dms(&[dm("1", "Ana", 0)]), None);
        let group = r#"{"id":"1","name":"The Group","group":true,"members":3,"unread":1,
                        "last":{"at":9,"text":"\nare you <@5> on tonight?\nmore","who":"Ana","names":{"5":"Bo"}}}"#;
        let t = r.take(&format!(r#"{{"k":"dms","dms":[{group}]}}"#), None);
        match &t.events[0] {
            DirectEvent::Message { name, group, line, .. } => {
                assert_eq!(name.as_str(), "Ana");
                assert_eq!(group.as_ref().map(|g| g.as_str()), Some("The Group"));
                assert_eq!(line.as_str(), "are you @Bo on tonight?");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rings_begin_and_end() {
        let mut r = Reader::new();
        let ring = |ch: &str| format!(r#"{{"k":"rings","rings":[{{"channel":"{ch}","caller":"Ana"}}]}}"#);
        assert!(r.take(r#"{"k":"rings","rings":[]}"#, None).events.is_empty());
        let t = r.take(&ring("5"), None);
        assert!(
            matches!(&t.events[..], [DirectEvent::Ring(ring)] if ring.channel.as_str() == "5" && ring.caller.as_str() == "Ana")
        );
        assert!(r.take(&ring("5"), None).events.is_empty(), "the same ring again is nothing");
        let t = r.take(r#"{"k":"rings","rings":[]}"#, None);
        assert!(matches!(&t.events[..], [DirectEvent::RingEnded(id)] if id.as_str() == "5"));
    }

    #[test]
    fn health_says_what_was_found_and_what_no_longer_answers_by_our_keys() {
        let mut r = Reader::new();
        let ok = r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore","getChannel"],"calls":[]},"missing":[],"older":true}"#;
        let t = r.take(ok, None);
        assert_eq!(t.log, vec!["discord web: stores found: channels".to_string()]);
        assert!(!r.page_changed() && r.older_found());
        assert!(r.take(ok, None).log.is_empty(), "said once");
        let bad = r#"{"k":"health","ok":false,"found":{"channels":["ChannelStore"]},"missing":["readState.getUnreadCount","messages","no chunk global"]}"#;
        let t = r.take(bad, None);
        assert_eq!(
            t.log,
            vec![
                "discord web: store \"readState\" no longer answers (getUnreadCount)".to_string(),
                "discord web: store \"messages\" no longer answers".to_string(),
                "discord web: the page's scripts: no chunk global".to_string(),
            ]
        );
        assert!(r.page_changed());
        assert_eq!(r.take(ok, None).log, vec!["discord web: the stores answer again".to_string()]);
    }

    #[test]
    fn health_lists_the_optional_actions_and_the_box_and_a_missing_one_is_no_break() {
        let mut r = Reader::new();
        let ok = r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore"],"reply":["(by shape)","createPendingReply"],"react":["(by shape)"],"ack":[],"composer":[]},"missing":[]}"#;
        let t = r.take(ok, None);
        assert_eq!(t.log, vec!["discord web: stores found: channels, react, reply".to_string()]);
        assert!(!r.page_changed(), "no ack and no box is not a break");
        let seen = r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore"],"reply":["(by shape)"],"react":["(by shape)"],"ack":[],"composer":["textbox"]},"missing":[]}"#;
        assert_eq!(
            r.take(seen, None).log,
            vec!["discord web: stores found: channels, composer, react, reply".to_string()]
        );
    }

    #[test]
    fn the_pages_words_about_an_action_are_taken_for_the_queue() {
        let mut r = Reader::new();
        let t = r.take(r#"{"k":"act","seq":4,"stage":"failed","why":"not shown","reason":"x"}"#, None);
        let act = t.act.unwrap();
        assert_eq!(
            (act.seq, act.stage.as_str(), act.why.as_str(), act.reason.as_str()),
            (4, "failed", "not shown", "x")
        );
        assert!(!t.changed && t.log.is_empty());
        let t = r.take(r#"{"k":"opened","read":true}"#, None);
        assert_eq!(t.log, vec!["discord web: conversation open, read by the page yes".to_string()]);
    }

    #[test]
    fn a_conversation_is_taken_only_for_the_one_asked_for() {
        let mut r = Reader::new();
        let conv = r#"{"k":"conv","id":"7","loaded":true,"more":false,"msgs":[{"id":"1","author":{}},{"id":"2","author":{}}]}"#;
        assert!(!r.take(conv, Some("8")).changed);
        assert!(r.conv.is_none());
        assert!(!r.take(conv, None).changed);
        let t = r.take(conv, Some("7"));
        assert!(t.changed);
        assert_eq!(t.log, vec!["discord web: conversation open, 2 messages".to_string()]);
        assert!(!r.take(conv, Some("7")).changed, "the same again");
        assert!(r.take("not json", Some("7")).log.is_empty());
        assert!(r.take(r#"{"k":"nothing"}"#, Some("7")).log.is_empty());
        assert!(r.take(r#"{"k":"reset"}"#, None).reset);
    }
}
