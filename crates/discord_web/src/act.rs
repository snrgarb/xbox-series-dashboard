//! Acting through the web session (#201; docs/discord-web.md, "Acting"): what the screens ask (send, react, mark read,
//! forget) as a queue of ONE action at a time, paced, each done only when Discord's own stores show it.
//!
//! The page does the acting (`js/act.js`: the route, the message box focused, Discord's own functions where the page has
//! no control); the host types as trusted input. This is the part that decides when: it starts an action at most every
//! [`PACE`] after the last one ended, gives each [`LIMIT`] to finish, tells the screens what happened as events, and
//! keeps the user's outgoing messages (what is queued, what is in flight, what failed) to put into the open
//! conversation under the nonce the screens gave, until Discord has given an id ([`Acts::overlay`]).
//!
//! Pure: the clock is given, nothing is sent from here, so the tests run it by hand. Nothing here logs a text, an id or
//! an emoji: [`Out::Log`] lines are made of words from a fixed set and of milliseconds.
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use model::discord_dms::{Body, Delivery, DirectEvent, Id, Message, Private};

use crate::direct::{spans, Act};

/// At most one action per this long (the spec's 6.4; docs/research/discord-bridge.md, 4).
pub const PACE: Duration = Duration::from_millis(700);
/// How long an action has to finish before it is called not done.
pub const LIMIT: Duration = Duration::from_secs(15);
/// Discord's limit for one message without Nitro.
pub const LONGEST: usize = 2000;

/// What the host is told to do with the page. Its `Debug` says kinds and sizes only.
#[derive(Clone, PartialEq, Eq)]
pub enum ActOp {
    Send {
        seq: u64,
        channel: String,
        text: String,
        reply: Option<String>,
    },
    React {
        seq: u64,
        channel: String,
        message: String,
        emoji: String,
        on: bool,
    },
    Ack {
        seq: u64,
        channel: String,
    },
    /// Give up what is running: the page lets go of it and says nothing more of it.
    Cancel,
    /// Drop Discord's own failed message of ours from its store, locally (a fire and forget).
    Forget {
        channel: String,
        message: String,
    },
}

impl std::fmt::Debug for ActOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActOp::Send { seq, text, reply, .. } => {
                write!(
                    f,
                    "Send(#{seq}, {} chars{})",
                    text.chars().count(),
                    if reply.is_some() { ", reply" } else { "" }
                )
            }
            ActOp::React { seq, on, .. } => write!(f, "React(#{seq}, {})", if *on { "on" } else { "off" }),
            ActOp::Ack { seq, .. } => write!(f, "Ack(#{seq})"),
            ActOp::Cancel => write!(f, "Cancel"),
            ActOp::Forget { .. } => write!(f, "Forget"),
        }
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg_attr(not(windows), allow(dead_code))]
impl ActOp {
    /// The kind, as a word for the log.
    pub fn kind(&self) -> &'static str {
        match self {
            ActOp::Send { .. } => "send",
            ActOp::React { .. } => "react",
            ActOp::Ack { .. } => "mark read",
            ActOp::Cancel => "cancel",
            ActOp::Forget { .. } => "forget",
        }
    }

    pub fn seq(&self) -> Option<u64> {
        match self {
            ActOp::Send { seq, .. } | ActOp::React { seq, .. } | ActOp::Ack { seq, .. } => Some(*seq),
            ActOp::Cancel | ActOp::Forget { .. } => None,
        }
    }

    /// The script that asks the page (`window.__xsdDirect`), or None for an ask that is not well formed: ids are digits
    /// only, an emoji is one short word without control characters. Everything else goes in as a JSON value, so
    /// nothing typed can become code.
    pub fn script(&self) -> Option<String> {
        use serde_json::json;
        let call =
            |name: &str, args: String| Some(format!("window.__xsdDirect ? window.__xsdDirect.{name}({args}) : false"));
        match self {
            ActOp::Send { seq, channel, text, reply } => {
                if !digits(channel) || reply.as_deref().is_some_and(|r| !digits(r)) || text.trim().is_empty() {
                    return None;
                }
                call("send", json!({ "seq": seq, "channel": channel, "text": text, "reply": reply }).to_string())
            }
            ActOp::React { seq, channel, message, emoji, on } => {
                if !digits(channel) || !digits(message) || emoji.is_empty() || emoji.chars().count() > 32 {
                    return None;
                }
                if emoji.chars().any(char::is_control) {
                    return None;
                }
                call(
                    "react",
                    json!({ "seq": seq, "channel": channel, "message": message, "emoji": emoji, "on": on }).to_string(),
                )
            }
            ActOp::Ack { seq, channel } => {
                if !digits(channel) {
                    return None;
                }
                call("ack", json!({ "seq": seq, "channel": channel }).to_string())
            }
            ActOp::Cancel => call("cancel", String::new()),
            ActOp::Forget { channel, message } => {
                if !digits(channel) || !digits(message) {
                    return None;
                }
                call("forget", format!("{}, {}", json!(channel), json!(message)))
            }
        }
    }
}

/// What a step of the queue asks of the session.
pub enum Out {
    /// For the host: ask the page.
    Op(ActOp),
    /// For the host: type this into the focused message box and press Enter; `clear` first empties a draft.
    Type { text: String, clear: bool },
    /// For the screens (`Session::take_events`).
    Event(DirectEvent),
    /// A log line, in words.
    Log(String),
    /// What the screens show may have changed.
    Changed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Queued,
    Sending,
    Failed,
}

/// A message of the user's own that the Dashboard sent and Discord has not yet shown as sent, or that failed.
struct Outgoing {
    nonce: u64,
    channel: String,
    text: String,
    reply: Option<String>,
    /// Unix milliseconds the Dashboard sent it.
    at: i64,
    /// The id of Discord's own optimistic message in the store, once the page has tied this send to it.
    tied: Option<String>,
    phase: Phase,
}

enum Work {
    Send { nonce: u64 },
    React { channel: String, message: String, emoji: String, on: bool },
    Read { channel: String },
}

struct Job {
    seq: u64,
    work: Work,
}

struct Running {
    job: Job,
    started: Instant,
    typed: bool,
}

/// The queue, and the user's outgoing messages.
#[derive(Default)]
pub struct Acts {
    queue: VecDeque<Job>,
    running: Option<Running>,
    last_end: Option<Instant>,
    seq: u64,
    outgoing: Vec<Outgoing>,
}

/// The log's word for a failure the page names; anything else is "page error".
fn word(why: &str) -> &'static str {
    match why {
        "reply action not found" => "reply action not found",
        "reply target not found" => "reply target not found",
        "composer not found" => "composer not found",
        "composer not focused" => "composer not focused",
        "react action not found" => "react action not found",
        "react action failed" => "react action failed",
        "ack action not found" => "ack action not found",
        "ack action failed" => "ack action failed",
        "message not loaded" => "message not loaded",
        "not shown" => "not shown",
        "send refused" => "refused",
        _ => "page error",
    }
}

/// Whether Discord's own words about a refusal say it is the rate limit.
fn limited(reason: &str) -> bool {
    let r = reason.to_lowercase();
    ["slow down", "rate limit", "too fast", "too many"].iter().any(|w| r.contains(w))
}

fn ms(since: Instant, now: Instant) -> u128 {
    now.saturating_duration_since(since).as_millis()
}

impl Acts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether nothing is queued or running.
    #[cfg(test)]
    pub fn idle(&self) -> bool {
        self.queue.is_empty() && self.running.is_none()
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// A message the Dashboard will not send (the session is not up, or it is not a message): at once not sent, and its
    /// bubble stays failed, so the screens show what they would for any other.
    pub fn refuse(&mut self, channel: &str, text: &str, nonce: u64, at: i64, why: &str) -> Vec<Out> {
        self.outgoing.push(Outgoing {
            nonce,
            channel: channel.to_string(),
            text: text.to_string(),
            reply: None,
            at,
            tied: None,
            phase: Phase::Failed,
        });
        vec![
            Out::Log(format!("discord web: not sent: {why}")),
            Out::Event(DirectEvent::NotSent { channel: Id(channel.to_string()), nonce }),
            Out::Changed,
        ]
    }

    /// A message to send: queued, shown at once as Sending, sent when its turn comes.
    pub fn send(&mut self, channel: &Id, text: &str, reply: Option<&Id>, nonce: u64, at: i64) -> Vec<Out> {
        let channel = channel.as_str();
        if !digits(channel) || reply.is_some_and(|r| !digits(r.as_str())) {
            return self.refuse(channel, text, nonce, at, "not a conversation");
        }
        if text.trim().is_empty() || text.chars().count() > LONGEST {
            return self.refuse(channel, text, nonce, at, "not a message");
        }
        let seq = self.next_seq();
        self.outgoing.push(Outgoing {
            nonce,
            channel: channel.to_string(),
            text: text.to_string(),
            reply: reply.map(|r| r.as_str().to_string()),
            at,
            tied: None,
            phase: Phase::Queued,
        });
        self.queue.push_back(Job { seq, work: Work::Send { nonce } });
        vec![Out::Changed]
    }

    pub fn react(&mut self, channel: &Id, message: &Id, emoji: &str, on: bool) {
        if !digits(channel.as_str()) || !digits(message.as_str()) || emoji.is_empty() {
            return;
        }
        let seq = self.next_seq();
        let work = Work::React {
            channel: channel.as_str().to_string(),
            message: message.as_str().to_string(),
            emoji: emoji.to_string(),
            on,
        };
        self.queue.push_back(Job { seq, work });
    }

    pub fn mark_read(&mut self, channel: &Id) {
        if !digits(channel.as_str()) {
            return;
        }
        let seq = self.next_seq();
        self.queue.push_back(Job { seq, work: Work::Read { channel: channel.as_str().to_string() } });
    }

    /// A failed message dropped from the view (by the nonce the screens know it by). Only a failed one: what is queued or
    /// in flight is not forgotten. Discord's own store drops its failed message too, locally.
    pub fn forget(&mut self, channel: &Id, message: &Id) -> Vec<Out> {
        let at = self.outgoing.iter().position(|o| {
            o.phase == Phase::Failed && o.channel == channel.as_str() && o.nonce.to_string() == message.as_str()
        });
        let Some(at) = at else { return Vec::new() };
        let gone = self.outgoing.remove(at);
        let mut out = vec![Out::Changed];
        if let Some(tied) = gone.tied {
            out.push(Out::Op(ActOp::Forget { channel: gone.channel, message: tied }));
        }
        out
    }

    fn outgoing_mut(&mut self, nonce: u64) -> Option<&mut Outgoing> {
        self.outgoing.iter_mut().find(|o| o.nonce == nonce)
    }

    /// The clock: a timeout, or the next action's turn.
    pub fn tick(&mut self, now: Instant) -> Vec<Out> {
        let mut out = Vec::new();
        if let Some(run) = &self.running {
            if now.saturating_duration_since(run.started) >= LIMIT {
                out.push(Out::Op(ActOp::Cancel));
                out.extend(self.end(now, Err("no answer in 15 s")));
            }
        }
        if self.running.is_some() || self.last_end.is_some_and(|end| now.saturating_duration_since(end) < PACE) {
            return out;
        }
        let Some(job) = self.queue.pop_front() else { return out };
        let seq = job.seq;
        let op = match &job.work {
            Work::Send { nonce } => match self.outgoing_mut(*nonce) {
                Some(o) => {
                    o.phase = Phase::Sending;
                    ActOp::Send { seq, channel: o.channel.clone(), text: o.text.clone(), reply: o.reply.clone() }
                }
                None => return out,
            },
            Work::React { channel, message, emoji, on } => {
                ActOp::React { seq, channel: channel.clone(), message: message.clone(), emoji: emoji.clone(), on: *on }
            }
            Work::Read { channel } => ActOp::Ack { seq, channel: channel.clone() },
        };
        self.running = Some(Running { job, started: now, typed: false });
        out.push(Out::Op(op));
        out
    }

    /// What the page said of the action running. Anything about another one is ignored.
    pub fn page(&mut self, act: &Act, now: Instant) -> Vec<Out> {
        let Some(run) = &mut self.running else { return Vec::new() };
        if run.job.seq != act.seq {
            return Vec::new();
        }
        let nonce = match run.job.work {
            Work::Send { nonce } => Some(nonce),
            _ => None,
        };
        match act.stage.as_str() {
            "typing" => {
                let Some(nonce) = nonce.filter(|_| !run.typed) else { return Vec::new() };
                run.typed = true;
                match self.outgoing.iter().find(|o| o.nonce == nonce) {
                    Some(o) => vec![Out::Type { text: o.text.clone(), clear: act.draft }],
                    None => Vec::new(),
                }
            }
            "sending" => {
                let Some(nonce) = nonce else { return Vec::new() };
                if let Some(o) = self.outgoing_mut(nonce) {
                    o.tied = Some(act.mid.clone());
                }
                vec![Out::Changed]
            }
            "sent" => {
                let Some(nonce) = nonce else { return Vec::new() };
                let channel = self.outgoing.iter().find(|o| o.nonce == nonce).map(|o| o.channel.clone());
                self.outgoing.retain(|o| o.nonce != nonce);
                let mut out = self.end(now, Ok(()));
                if let Some(channel) = channel {
                    out.push(Out::Event(DirectEvent::Sent {
                        channel: Id(channel),
                        nonce,
                        message: Id(act.mid.clone()),
                    }));
                }
                out
            }
            "done" => self.end(now, Ok(())),
            "captcha" => {
                let mut out = vec![Out::Event(DirectEvent::Captcha)];
                out.extend(self.end(now, Err("captcha")));
                out
            }
            "failed" => {
                let mut out = Vec::new();
                let mut why = word(&act.why);
                if why == "refused" && limited(&act.reason) {
                    why = "rate limited";
                }
                if nonce.is_some() && !act.reason.is_empty() {
                    out.push(Out::Event(DirectEvent::Refused(Private(act.reason.clone()))));
                }
                if let (Some(nonce), true) = (nonce, !act.mid.is_empty()) {
                    // The store's own failed message: the one the bubble stays on.
                    if let Some(o) = self.outgoing_mut(nonce) {
                        o.tied = Some(act.mid.clone());
                    }
                }
                out.extend(self.end(now, Err(why)));
                out
            }
            _ => Vec::new(),
        }
    }

    /// The running action ended: how, in words. A send that did not go stays as a failed message.
    fn end(&mut self, now: Instant, how: Result<(), &'static str>) -> Vec<Out> {
        let Some(run) = self.running.take() else { return Vec::new() };
        self.last_end = Some(now);
        let took = ms(run.started, now);
        let mut out = vec![Out::Changed];
        match (run.job.work, how) {
            (Work::Send { .. }, Ok(())) => out.push(Out::Log(format!("discord web: sent in {took} ms"))),
            (Work::React { .. }, Ok(())) => out.push(Out::Log(format!("discord web: reaction shown in {took} ms"))),
            (Work::Read { .. }, Ok(())) => out.push(Out::Log(format!("discord web: marked read in {took} ms"))),
            (Work::Send { nonce }, Err(why)) => {
                out.push(Out::Log(format!("discord web: not sent: {why}")));
                if let Some(o) = self.outgoing_mut(nonce) {
                    o.phase = Phase::Failed;
                    let channel = Id(o.channel.clone());
                    out.push(Out::Event(DirectEvent::NotSent { channel, nonce }));
                }
            }
            (Work::React { .. }, Err(why)) => out.push(Out::Log(format!("discord web: reaction not done: {why}"))),
            (Work::Read { .. }, Err(why)) => out.push(Out::Log(format!("discord web: mark read not done: {why}"))),
        }
        out
    }

    /// The session ended (switched off, signed out, the page's user gone): the queue is emptied, each send that had not
    /// gone is not sent, what ran is cancelled. Nothing outgoing is kept.
    pub fn reset(&mut self) -> Vec<Out> {
        let mut out = Vec::new();
        if self.running.take().is_some() {
            out.push(Out::Op(ActOp::Cancel));
        }
        let pending: Vec<(String, u64)> =
            self.outgoing.iter().filter(|o| o.phase != Phase::Failed).map(|o| (o.channel.clone(), o.nonce)).collect();
        for (channel, nonce) in pending {
            out.push(Out::Log("discord web: not sent: the session ended".into()));
            out.push(Out::Event(DirectEvent::NotSent { channel: Id(channel), nonce }));
        }
        *self = Acts { seq: self.seq, ..Acts::default() };
        out
    }

    /// The open conversation's messages with the user's outgoing ones in them: Discord's own optimistic message (or
    /// its failed one) carries the nonce as its id while the send is not done; one the store does not hold (not
    /// asked yet, or gone) is made from what was sent, last, so a message sent shows at once as Sending.
    pub fn overlay(&self, channel: &str, messages: &mut Vec<Message>) {
        for o in self.outgoing.iter().filter(|o| o.channel == channel) {
            let delivery = if o.phase == Phase::Failed { Delivery::Failed } else { Delivery::Sending };
            let id = Id(o.nonce.to_string());
            let held = o.tied.as_ref().and_then(|t| messages.iter_mut().find(|m| m.id.as_str() == t));
            match held {
                Some(m) => {
                    m.id = id;
                    m.delivery = delivery;
                }
                None => messages.push(Message {
                    id,
                    author: Id(String::new()),
                    author_name: Private(String::new()),
                    author_picture: None,
                    own: true,
                    at: o.at,
                    edited: false,
                    body: Body::Said {
                        text: spans(o.text.trim(), &Default::default()),
                        reply: None,
                        extras: Vec::new(),
                        reactions: Vec::new(),
                    },
                    delivery,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    fn act(seq: u64, stage: &str, mid: &str) -> Act {
        Act { seq, stage: stage.into(), mid: mid.into(), ..Act::default() }
    }

    fn ops(outs: &[Out]) -> Vec<&ActOp> {
        outs.iter().filter_map(|o| if let Out::Op(op) = o { Some(op) } else { None }).collect()
    }

    fn events(outs: Vec<Out>) -> Vec<DirectEvent> {
        outs.into_iter().filter_map(|o| if let Out::Event(e) = o { Some(e) } else { None }).collect()
    }

    fn logs(outs: &[Out]) -> Vec<String> {
        outs.iter().filter_map(|o| if let Out::Log(l) = o { Some(l.clone()) } else { None }).collect()
    }

    fn id(s: &str) -> Id {
        Id(s.to_string())
    }

    #[test]
    fn a_send_is_shown_sending_at_once_and_goes_when_its_turn_comes() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 1000);
        let mut messages = Vec::new();
        a.overlay("42", &mut messages);
        assert_eq!(
            (messages.len(), messages[0].id.as_str(), messages[0].delivery, messages[0].own),
            (1, "7", Delivery::Sending, true)
        );
        a.overlay("43", &mut messages);
        assert_eq!(messages.len(), 1, "another conversation's is not shown here");
        let outs = a.tick(base);
        assert!(
            matches!(ops(&outs)[..], [ActOp::Send { seq: 1, channel, text, reply: None }] if channel == "42" && text == "hello")
        );
    }

    #[test]
    fn the_page_asks_for_typing_once_and_the_text_goes_to_the_host_with_whether_to_clear_a_draft() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 0);
        a.tick(base);
        let outs = a.page(&Act { draft: true, ..act(1, "typing", "") }, at(base, 100));
        assert!(matches!(&outs[..], [Out::Type { text, clear: true }] if text == "hello"));
        assert!(a.page(&act(1, "typing", ""), at(base, 110)).is_empty(), "once");
        assert!(a.page(&act(9, "typing", ""), at(base, 110)).is_empty(), "another action's");
    }

    #[test]
    fn one_at_a_time_and_each_paced_by_the_last_ones_end() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "one", None, 1, 0);
        a.send(&id("42"), "two", None, 2, 0);
        a.react(&id("42"), &id("5"), "x", true);
        a.mark_read(&id("42"));
        assert_eq!(ops(&a.tick(base)).len(), 1);
        assert!(a.tick(at(base, 5000)).is_empty(), "the first is still running: nothing else starts");
        a.page(&act(1, "typing", ""), at(base, 10));
        a.page(&act(1, "sending", "9001"), at(base, 20));
        a.page(&act(1, "sent", "9002"), at(base, 800));
        // Ended at 800: the next not before 1500.
        assert!(a.tick(at(base, 800)).is_empty());
        assert!(a.tick(at(base, 1499)).is_empty());
        let outs = a.tick(at(base, 1500));
        assert!(matches!(ops(&outs)[..], [ActOp::Send { seq: 2, .. }]));
        a.page(&act(2, "sent", "9003"), at(base, 1600));
        assert!(a.tick(at(base, 2299)).is_empty());
        assert!(matches!(ops(&a.tick(at(base, 2300)))[..], [ActOp::React { seq: 3, .. }]));
        a.page(&act(3, "done", ""), at(base, 2400));
        assert!(matches!(ops(&a.tick(at(base, 3100)))[..], [ActOp::Ack { seq: 4, .. }]));
    }

    #[test]
    fn several_queued_sends_are_each_shown_sending_in_order() {
        let (base, mut a) = (t0(), Acts::new());
        for n in 1..=3u64 {
            a.send(&id("42"), &format!("m{n}"), None, n, n as i64);
        }
        a.tick(base);
        let mut messages = Vec::new();
        a.overlay("42", &mut messages);
        assert_eq!(
            messages.iter().map(|m| (m.id.as_str().to_string(), m.delivery)).collect::<Vec<_>>(),
            vec![
                ("1".to_string(), Delivery::Sending),
                ("2".to_string(), Delivery::Sending),
                ("3".to_string(), Delivery::Sending),
            ]
        );
    }

    #[test]
    fn the_stores_optimistic_message_is_reported_under_the_nonce_until_it_has_a_real_id() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 5);
        a.tick(base);
        a.page(&act(1, "sending", "900"), at(base, 50));
        let store = |id: &str, delivery| Message {
            id: Id(id.to_string()),
            author: Id("me".into()),
            author_name: Private("Me".into()),
            author_picture: None,
            own: true,
            at: 9,
            edited: false,
            body: Body::Line { kind: model::discord_dms::LineKind::Other, text: Private("hello".into()) },
            delivery,
        };
        let mut messages = vec![store("1", Delivery::Sent), store("900", Delivery::Sent)];
        a.overlay("42", &mut messages);
        assert_eq!(messages.len(), 2, "the store's own message is the one shown, not another");
        assert_eq!(
            (messages[1].id.as_str(), messages[1].delivery),
            ("7", Delivery::Sending),
            "even where the store says sent"
        );
        // Sent: the real id is Discord's, the nonce is told once, nothing is overlaid any more.
        let outs = a.page(&act(1, "sent", "901"), at(base, 300));
        assert_eq!(logs(&outs), vec!["discord web: sent in 300 ms".to_string()]);
        assert_eq!(events(outs), vec![DirectEvent::Sent { channel: id("42"), nonce: 7, message: id("901") }]);
        let mut messages = vec![store("1", Delivery::Sent), store("901", Delivery::Sent)];
        a.overlay("42", &mut messages);
        assert_eq!(messages[1].id.as_str(), "901");
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn a_failure_keeps_the_bubble_under_the_nonce_until_it_is_forgotten() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 5);
        a.tick(base);
        a.page(&act(1, "sending", "900"), at(base, 50));
        let outs =
            a.page(&Act { why: "send refused".into(), mid: "900".into(), ..act(1, "failed", "") }, at(base, 400));
        assert_eq!(logs(&outs), vec!["discord web: not sent: refused".to_string()]);
        assert_eq!(
            events(outs),
            vec![DirectEvent::NotSent { channel: id("42"), nonce: 7 }],
            "no reason: not sent, no prompt"
        );
        let mut messages = Vec::new();
        a.overlay("42", &mut messages);
        assert_eq!((messages[0].id.as_str(), messages[0].delivery), ("7", Delivery::Failed), "made from what was sent");
        // Forget by the nonce: dropped here, and the page is asked to drop Discord's own failed message.
        assert!(a.forget(&id("42"), &id("8")).is_empty(), "another nonce");
        let outs = a.forget(&id("42"), &id("7"));
        assert!(matches!(ops(&outs)[..], [ActOp::Forget { channel, message }] if channel == "42" && message == "900"));
        let mut messages = Vec::new();
        a.overlay("42", &mut messages);
        assert!(messages.is_empty());
        // A send still in flight is not forgotten.
        a.send(&id("42"), "again", None, 8, 6);
        assert!(a.forget(&id("42"), &id("8")).is_empty());
    }

    #[test]
    fn a_reason_discord_gives_is_passed_on_and_the_rate_limit_is_named() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 5);
        a.tick(base);
        let failed = Act {
            why: "send refused".into(),
            reason: "You are sending messages too fast. Slow down.".into(),
            ..act(1, "failed", "")
        };
        let outs = a.page(&failed, at(base, 400));
        assert_eq!(logs(&outs), vec!["discord web: not sent: rate limited".to_string()]);
        let events = events(outs);
        assert!(matches!(&events[0], DirectEvent::Refused(r) if r.as_str().starts_with("You are sending")));
        assert!(matches!(events[1], DirectEvent::NotSent { nonce: 7, .. }));
    }

    #[test]
    fn a_captcha_is_told_once_and_the_message_is_not_sent() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 5);
        a.tick(base);
        let outs = a.page(&act(1, "captcha", ""), at(base, 400));
        assert_eq!(logs(&outs), vec!["discord web: not sent: captcha".to_string()]);
        let events = events(outs);
        assert!(matches!(events[..], [DirectEvent::Captcha, DirectEvent::NotSent { nonce: 7, .. }]));
    }

    #[test]
    fn nothing_within_fifteen_seconds_is_not_sent_and_the_page_is_told_to_let_go() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", None, 7, 5);
        a.send(&id("42"), "next", None, 8, 5);
        a.tick(base);
        assert!(a.tick(at(base, 14_999)).is_empty());
        let outs = a.tick(at(base, 15_000));
        assert!(matches!(outs[0], Out::Op(ActOp::Cancel)));
        assert_eq!(logs(&outs), vec!["discord web: not sent: no answer in 15 s".to_string()]);
        assert!(matches!(events(outs)[..], [DirectEvent::NotSent { nonce: 7, .. }]));
        // The next waits its pace, then goes.
        assert!(ops(&a.tick(at(base, 15_100))).is_empty());
        assert!(matches!(ops(&a.tick(at(base, 15_700)))[..], [ActOp::Send { seq: 2, .. }]));
        // A word from the page about the first, late, is nothing.
        assert!(a.page(&act(1, "sent", "1"), at(base, 15_800)).is_empty());
    }

    #[test]
    fn a_reaction_or_a_read_not_done_is_said_in_words_and_raises_nothing() {
        let (base, mut a) = (t0(), Acts::new());
        a.react(&id("42"), &id("5"), "\u{1F525}", true);
        a.mark_read(&id("42"));
        a.tick(base);
        let outs = a.page(&Act { why: "react action not found".into(), ..act(1, "failed", "") }, at(base, 10));
        assert_eq!(logs(&outs), vec!["discord web: reaction not done: react action not found".to_string()]);
        assert!(events(outs).is_empty());
        a.tick(at(base, 710));
        let outs = a.page(&act(2, "done", ""), at(base, 800));
        assert_eq!(logs(&outs), vec!["discord web: marked read in 90 ms".to_string()]);
    }

    #[test]
    fn a_reply_the_page_cannot_make_is_not_sent_with_its_words() {
        let (base, mut a) = (t0(), Acts::new());
        a.send(&id("42"), "hello", Some(&id("5")), 7, 5);
        let outs = a.tick(base);
        assert!(matches!(ops(&outs)[..], [ActOp::Send { reply: Some(r), .. }] if r == "5"));
        let outs = a.page(&Act { why: "reply action not found".into(), ..act(1, "failed", "") }, at(base, 10));
        assert_eq!(logs(&outs), vec!["discord web: not sent: reply action not found".to_string()]);
        assert!(matches!(events(outs)[..], [DirectEvent::NotSent { nonce: 7, .. }]));
    }

    #[test]
    fn a_reset_empties_the_queue_and_every_pending_send_is_not_sent() {
        let (base, mut a) = (t0(), Acts::new());
        for n in 1..=3u64 {
            a.send(&id("42"), "x", None, n, 0);
        }
        a.react(&id("42"), &id("5"), "x", true);
        a.tick(base);
        a.page(&act(1, "sent", "1"), at(base, 10));
        let outs = a.reset();
        assert_eq!(logs(&outs).len(), 2, "the two sends that had not gone");
        let nonces: Vec<u64> = events(outs)
            .into_iter()
            .filter_map(|e| if let DirectEvent::NotSent { nonce, .. } = e { Some(nonce) } else { None })
            .collect();
        assert_eq!(nonces, vec![2, 3]);
        assert!(a.idle() && a.tick(at(base, 10_000)).is_empty());
        let mut messages = Vec::new();
        a.overlay("42", &mut messages);
        assert!(messages.is_empty());
        // One running is cancelled.
        a.send(&id("42"), "y", None, 9, 0);
        a.tick(at(base, 20_000));
        assert!(matches!(a.reset()[0], Out::Op(ActOp::Cancel)));
    }

    #[test]
    fn what_is_not_a_message_or_a_conversation_is_not_sent_at_once() {
        let mut a = Acts::new();
        for (channel, text) in [("42", "   "), ("42", ""), ("4x", "hi"), ("42", &"x".repeat(2001)[..])] {
            let outs = a.send(&id(channel), text, None, 1, 0);
            assert!(matches!(events(outs)[..], [DirectEvent::NotSent { nonce: 1, .. }]));
        }
        assert!(a.idle());
        let outs = a.send(&id("42"), &"x".repeat(2000), None, 2, 0);
        assert!(events(outs).is_empty());
        assert!(!a.idle());
    }

    #[test]
    fn the_script_for_the_page_holds_ids_as_digits_and_text_as_a_value() {
        let send = ActOp::Send { seq: 1, channel: "42".into(), text: "say \"hi\"\n</script> $(x)".into(), reply: None };
        let js = send.script().unwrap();
        assert!(js.starts_with("window.__xsdDirect ? window.__xsdDirect.send({"), "{js}");
        assert!(js.contains(r#""text":"say \"hi\"\n</script> $(x)""#), "{js}");
        for bad in [
            ActOp::Send { seq: 1, channel: "4\"; alert(1); \"".into(), text: "x".into(), reply: None },
            ActOp::Send { seq: 1, channel: "42".into(), text: "x".into(), reply: Some("a".into()) },
            ActOp::Send { seq: 1, channel: "42".into(), text: " ".into(), reply: None },
            ActOp::React { seq: 1, channel: "42".into(), message: "1".into(), emoji: String::new(), on: true },
            ActOp::React { seq: 1, channel: "42".into(), message: "x".into(), emoji: "a".into(), on: true },
            ActOp::Ack { seq: 1, channel: "-1".into() },
            ActOp::Forget { channel: "1".into(), message: "1 2".into() },
        ] {
            assert!(bad.script().is_none(), "{bad:?}");
        }
        assert_eq!(
            ActOp::Forget { channel: "1".into(), message: "2".into() }.script().unwrap(),
            r#"window.__xsdDirect ? window.__xsdDirect.forget("1", "2") : false"#
        );
        assert!(ActOp::Cancel.script().unwrap().contains("cancel()"));
        let react =
            ActOp::React { seq: 2, channel: "1".into(), message: "2".into(), emoji: "\u{1F525}".into(), on: false };
        assert!(react.script().unwrap().contains(r#""on":false"#));
    }

    #[test]
    fn nothing_private_is_in_a_debug_line_or_a_log_line() {
        let (base, mut a) = (t0(), Acts::new());
        let mut lines = Vec::new();
        let mut all = |outs: Vec<Out>| {
            for o in &outs {
                match o {
                    Out::Log(l) => lines.push(l.clone()),
                    Out::Op(op) => lines.push(format!("{op:?}")),
                    Out::Event(e) => lines.push(format!("{e:?}")),
                    _ => {}
                }
            }
        };
        all(a.send(&id("424242"), "secret words", Some(&id("777")), 7, 1));
        all(a.tick(base));
        all(a.page(
            &Act {
                why: "send refused".into(),
                reason: "private reason".into(),
                mid: "555".into(),
                ..act(1, "failed", "")
            },
            at(base, 9),
        ));
        a.react(&id("424242"), &id("777"), "\u{1F525}", true);
        all(a.tick(at(base, 900)));
        all(a.page(&Act { why: "message not loaded".into(), ..act(2, "failed", "") }, at(base, 950)));
        all(a.forget(&id("424242"), &id("7")));
        all(a.refuse("424242", "secret words", 8, 1, "not signed in"));
        let said = lines.join("\n");
        assert!(lines.len() >= 6, "{said}");
        for private in ["secret", "424242", "777", "555", "private reason", "\u{1F525}"] {
            assert!(!said.contains(private), "{private} is in:\n{said}");
        }
    }
}
