//! Direct Messages in the Guide (docs/spec/party-chats.md): what the Discord tab's section, All Direct Messages and a
//! conversation keep between frames, and what a press does in them. The web session's reading arrives whole
//! (`Dms::set`); what the user does is asked of it (`DirectAsk`, queued in `Dms::asks` for the Dashboard to send) and is
//! shown done only once it says so, but for one's own message going out, which shows at once, Sending, and is
//! matched with what the session reports (`Dms::reconcile`) or marked Not sent after fifteen seconds.
//!
//! Nothing here is written anywhere, and nothing of a name or a message reaches a log: `line` says places and
//! counts only (spec 9 and 10).
use std::collections::HashMap;

use model::discord_dms::{
    Body, Conversation, Delivery, Direct, DirectAsk, DirectState, Id, LineKind, Message, Private, Reply, Span,
    MESSAGE_LIMIT, REACTIONS,
};
use ui::{Button, Press};

use super::when::When;

/// A message not seen on its way out within this long is Not sent (spec 6.4).
pub const SEND_SECONDS: f32 = 15.0;
/// How many messages a page of LT or RT moves before the view has said how many it shows.
const PAGE: usize = 6;

/// Where a conversation was opened from, which B goes back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Back {
    Tab,
    AllDirectMessages,
}

/// Where focus is in a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Message(Id),
    Reply,
}

/// What is open over the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    All(All),
    Chat(Chat),
}

/// All Direct Messages: the focused row and how far the rows are scrolled.
#[derive(Debug, Clone, PartialEq)]
pub struct All {
    pub item: usize,
    pub scroll: f32,
}

/// An open conversation, as far as the screens keep it. Its messages are the session's (`Direct::open`).
#[derive(Debug, Clone, PartialEq)]
pub struct Chat {
    pub id: Id,
    pub back: Back,
    pub focus: Place,
    /// How far the messages are scrolled, drawn: set by the frame, from the focus.
    pub scroll: f32,
    /// Whether the frame has yet put the view where opening puts it (spec 5.4).
    pub placed: bool,
    /// The first message that was unread when it opened: the unread line stands before it, and stays.
    pub unread_from: Option<Id>,
    /// The newest message that was in view with focus at the foot: those after it are "new".
    pub seen: Option<Id>,
    /// Older messages were asked for and have not come.
    pub asked_older: bool,
    /// How many messages the view showed when last drawn: a page of LT or RT.
    pub shown: usize,
    /// Whether the conversation has come (`Direct::open` is this one).
    pub arrived: bool,
    /// The oldest message loaded when the conversation last changed: older ones arriving move it.
    pub oldest: Option<Id>,
    /// The messages that took focus, oldest first, as they last were: where focus goes when one is deleted.
    pub known: Vec<Id>,
}

/// What a reply box has: the draft, and the message being answered.
#[derive(Clone, Default, PartialEq)]
pub struct Draft {
    pub text: String,
    pub reply: Option<Id>,
}

impl std::fmt::Debug for Draft {
    /// Its length only: never what is written.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Draft({} chars{})", self.text.chars().count(), if self.reply.is_some() { ", replying" } else { "" })
    }
}

/// A message of the user's own that has gone out and is not yet seen sent.
#[derive(Clone, PartialEq)]
pub struct Out {
    pub channel: Id,
    /// The nonce, as the session knows it until Discord gives the message an id.
    pub id: Id,
    pub text: String,
    pub reply: Option<Id>,
    /// Unix milliseconds, at the moment of sending.
    pub at: i64,
    /// `Dms::age` then.
    pub sent_at: f32,
    pub failed: bool,
    /// The messages the conversation had then: a message that is not one of them, of the user's, with this text,
    /// is this one sent.
    pub known: Vec<Id>,
}

impl std::fmt::Debug for Out {
    /// Its length and state only: never what is written.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Out({} chars, {})", self.text.chars().count(), if self.failed { "not sent" } else { "sending" })
    }
}

/// Text entry wanted for a reply box: the Dashboard opens it.
#[derive(Clone, PartialEq)]
pub struct Write {
    pub channel: Id,
    pub caption: String,
    pub text: String,
}

impl std::fmt::Debug for Write {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Write({} chars)", self.text.chars().count())
    }
}

/// What a press of the view wants of the Guide beyond itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Nothing,
    /// Something moved.
    Moved,
    /// A menu over the view.
    Menu(Target),
    /// The view's own B has gone back or closed it.
    Left,
    /// Chose something: the sound of A.
    Chose,
}

/// What a menu is about.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A DM's menu: Mark As Read, Call.
    Dm(Id),
    /// A message's (by id) menu.
    Message(Id),
    /// Add Reaction, on that message.
    Reaction(Id),
}

/// Direct Messages as the Guide keeps them.
#[derive(Debug, Clone, Default)]
pub struct Dms {
    /// As the session last said.
    pub direct: Direct,
    pub view: Option<View>,
    /// One draft per conversation, in memory while the Dashboard runs (decision 8).
    pub drafts: HashMap<Id, Draft>,
    pub outs: Vec<Out>,
    nonce: u64,
    /// Asked of the session, not yet taken by the Dashboard.
    pub asks: Vec<DirectAsk>,
    /// Seconds the Dashboard has told the Guide have passed.
    pub age: f32,
    pub when: When,
    /// Text entry wanted.
    pub write: Option<Write>,
    /// When (by `age`) the section began to say Loading.
    pub loading_since: Option<f32>,
    /// How many times a view has opened: each comes in once.
    pub opening: u32,
}

impl Dms {
    /// The session says how it stands. A switched-off section forgets everything (spec 8.2); a conversation that
    /// cannot be shown any more closes.
    pub fn set(&mut self, direct: &Direct) {
        let was_loading = self.direct.state == DirectState::Loading;
        self.direct = direct.clone();
        match direct.state {
            DirectState::Off => {
                *self = Self { when: self.when, age: self.age, ..Self::default() };
                return;
            }
            DirectState::Loading if !was_loading => self.loading_since = Some(self.age),
            DirectState::Loading => {}
            _ => self.loading_since = None,
        }
        if direct.state != DirectState::Ready && matches!(self.view, Some(View::Chat(_))) {
            self.view = None;
        }
        self.reconcile();
        self.follow();
    }

    /// Time passes: a message not seen sent in fifteen seconds is Not sent.
    pub fn pass(&mut self, seconds: f32) {
        self.age += seconds;
        for out in &mut self.outs {
            if !out.failed && self.age - out.sent_at >= SEND_SECONDS {
                out.failed = true;
            }
        }
    }

    /// Every message still going out is Not sent: Discord asked for a check or refused.
    pub fn fail_pending(&mut self) {
        for out in &mut self.outs {
            out.failed = true;
        }
    }

    /// Whether anything is still waiting on the clock.
    pub fn timer(&self) -> Option<f32> {
        self.outs
            .iter()
            .filter(|out| !out.failed)
            .map(|out| (SEND_SECONDS - (self.age - out.sent_at)).max(0.0))
            .reduce(f32::min)
    }

    /// The conversation the view is on, as the session has it.
    pub fn conversation(&self) -> Option<&Conversation> {
        let Some(View::Chat(chat)) = &self.view else { return None };
        self.direct.open.as_ref().filter(|open| open.id == chat.id)
    }

    pub fn chat(&self) -> Option<&Chat> {
        match &self.view {
            Some(View::Chat(chat)) => Some(chat),
            _ => None,
        }
    }

    /// The text a message says, as one string (its spans joined).
    pub fn text_of(message: &Message) -> String {
        match &message.body {
            Body::Said { text, .. } => text
                .iter()
                .map(|span| match span {
                    Span::Text(text) | Span::Mention(text) | Span::Link(text) => text.as_str().to_string(),
                    Span::ServerEmoji(name) => format!(":{}:", name.as_str()),
                })
                .collect(),
            Body::Line { text, .. } => text.as_str().to_string(),
        }
    }

    /// The conversation's messages with the user's own that are still going out, oldest first. A message the
    /// session reports under a nonce's id replaces the local one; one that is matched by what it says (the session's
    /// real id, for what the user sent) is the local one sent.
    pub fn messages(&self, channel: &Id) -> Vec<Message> {
        let mut messages: Vec<Message> = match self.direct.open.as_ref().filter(|open| &open.id == channel) {
            Some(open) => open.messages.clone(),
            None => Vec::new(),
        };
        for out in self.outs.iter().filter(|out| &out.channel == channel) {
            if messages.iter().any(|message| message.id == out.id) {
                continue;
            }
            let reply = out.reply.as_ref().and_then(|id| {
                let to = messages.iter().find(|message| &message.id == id)?;
                Some(Reply::To { author: to.author_name.clone(), line: Private(first_line(&Self::text_of(to))) })
            });
            messages.push(Message {
                id: out.id.clone(),
                author: Id::default(),
                author_name: Private::default(),
                author_picture: None,
                own: true,
                at: out.at,
                edited: false,
                body: Body::Said {
                    text: vec![Span::Text(out.text.as_str().into())],
                    reply,
                    extras: Vec::new(),
                    reactions: Vec::new(),
                },
                delivery: if out.failed { Delivery::Failed } else { Delivery::Sending },
            });
        }
        messages
    }

    /// The messages of the open conversation that take focus: bubbles, not Discord's own lines.
    pub fn stops(&self, channel: &Id) -> Vec<Message> {
        self.messages(channel).into_iter().filter(|message| matches!(message.body, Body::Said { .. })).collect()
    }

    /// Drops the local messages the session now shows, and moves focus onto the message that took a local one's
    /// place.
    fn reconcile(&mut self) {
        let Some(open) = self.direct.open.clone() else { return };
        let mut moved: Vec<(Id, Id)> = Vec::new();
        let mut claimed: Vec<Id> = Vec::new();
        self.outs.retain(|out| {
            if out.channel != open.id {
                return true;
            }
            let by_nonce = open.messages.iter().find(|message| message.id == out.id);
            let by_text = || {
                open.messages.iter().find(|message| {
                    message.own
                        && message.delivery == Delivery::Sent
                        && !out.known.contains(&message.id)
                        && !claimed.contains(&message.id)
                        && Self::text_of(message).trim() == out.text.trim()
                })
            };
            match by_nonce.or_else(by_text) {
                // Still going, or failed, as the session says: the session's message is shown in its place.
                Some(message) if message.delivery != Delivery::Sent && message.id == out.id => false,
                Some(message) => {
                    claimed.push(message.id.clone());
                    moved.push((out.id.clone(), message.id.clone()));
                    false
                }
                None => true,
            }
        });
        if let Some(View::Chat(chat)) = &mut self.view {
            if let Place::Message(id) = &chat.focus {
                if let Some((_, to)) = moved.iter().find(|(from, _)| from == id) {
                    chat.focus = Place::Message(to.clone());
                }
            }
        }
    }

    /// The open view follows what the session says: a conversation that has come, focus on a message that has
    /// gone, the older messages that have arrived.
    fn follow(&mut self) {
        let Some(View::Chat(chat)) = &self.view else { return };
        let (id, arrived) = (chat.id.clone(), self.direct.open.as_ref().is_some_and(|open| open.id == chat.id));
        let stops = self.stops(&id);
        let oldest = stops.first().map(|message| message.id.clone());
        let unread = self.direct.open.as_ref().filter(|_| arrived).and_then(|open| open.unread_from.clone());
        let at_start = self.direct.open.as_ref().is_some_and(|open| open.at_start);
        let Some(View::Chat(chat)) = &mut self.view else { return };
        if arrived && !chat.arrived {
            chat.unread_from = unread;
            chat.seen = stops.last().map(|message| message.id.clone());
        }
        chat.arrived = arrived;
        // Older messages came, or there are none to come: ask again if focus reaches the oldest.
        if chat.oldest != oldest {
            chat.oldest = oldest;
            chat.asked_older = false;
        }
        if at_start {
            chat.asked_older = false;
        }
        // Focus on a message that has gone: the next newer one still there, else the reply box.
        if let Place::Message(focus) = chat.focus.clone() {
            if !stops.iter().any(|message| message.id == focus) {
                let after = chat.known.iter().position(|id| *id == focus).map_or(0, |at| at + 1);
                let next = chat.known[after.min(chat.known.len())..]
                    .iter()
                    .find(|id| stops.iter().any(|message| &message.id == *id));
                chat.focus = next.map_or(Place::Reply, |id| Place::Message(id.clone()));
            }
        }
        chat.known = stops.iter().map(|message| message.id.clone()).collect();
        self.check_older();
    }

    /// Asks for older messages, once, when focus is on the oldest loaded.
    pub(super) fn check_older(&mut self) {
        let Some(View::Chat(chat)) = &self.view else { return };
        let Some(open) = self.direct.open.as_ref().filter(|open| open.id == chat.id) else { return };
        let on_oldest = matches!(&chat.focus, Place::Message(id) if Some(id) == chat.oldest.as_ref());
        if on_oldest && !open.at_start && !open.loading_older && !chat.asked_older {
            if let Some(View::Chat(chat)) = &mut self.view {
                chat.asked_older = true;
            }
            self.asks.push(DirectAsk::Older);
        }
    }

    // --- Opening and leaving ---

    /// A on a DM: its conversation opens, loading until the session has it.
    pub fn open_chat(&mut self, id: Id, back: Back) {
        self.asks.push(DirectAsk::Open(id.clone()));
        if self.view.is_none() {
            self.opening += 1;
        }
        let arrived = self.direct.open.as_ref().is_some_and(|open| open.id == id);
        self.view = Some(View::Chat(Chat {
            id,
            back,
            focus: Place::Reply,
            scroll: 0.0,
            placed: false,
            unread_from: None,
            seen: None,
            asked_older: false,
            shown: PAGE,
            arrived: false,
            oldest: None,
            known: Vec::new(),
        }));
        if arrived {
            self.follow();
        }
    }

    /// A on the sixth circle: All Direct Messages, focus on the DM that had focus if it is in the list, else the first.
    pub fn open_all(&mut self, focused: Option<&Id>) {
        let item = focused.and_then(|id| self.direct.dms.iter().position(|dm| &dm.id == id)).unwrap_or(0);
        if self.view.is_none() {
            self.opening += 1;
        }
        self.view = Some(View::All(All { item, scroll: 0.0 }));
    }

    /// The Guide closes: the conversation is let go of (its messages forgotten by the session), drafts kept.
    pub fn close_all(&mut self) {
        if matches!(self.view, Some(View::Chat(_))) {
            self.asks.push(DirectAsk::Close);
        }
        self.view = None;
    }

    // --- Writing ---

    pub fn draft(&self, channel: &Id) -> Option<&Draft> {
        self.drafts.get(channel)
    }

    /// Each change of the text typed is the draft at once.
    pub fn set_draft(&mut self, channel: &Id, text: &str) {
        let text: String = text.chars().take(MESSAGE_LIMIT).collect();
        let draft = self.drafts.entry(channel.clone()).or_default();
        draft.text = text;
        if draft.text.is_empty() && draft.reply.is_none() {
            self.drafts.remove(channel);
        }
    }

    /// Menu or Enter: the draft goes out, shown at once as Sending, and is emptied; blank text sends nothing and
    /// empties the draft (spec 6.1). Focus is on the reply box.
    pub fn send(&mut self, channel: &Id) {
        let draft = self.drafts.remove(channel).unwrap_or_default();
        if let Some(View::Chat(chat)) = &mut self.view {
            if &chat.id == channel {
                chat.focus = Place::Reply;
            }
        }
        let text = draft.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.nonce += 1;
        let id = Id(self.nonce.to_string());
        let known = self.messages(channel).into_iter().map(|message| message.id).collect();
        self.outs.push(Out {
            channel: channel.clone(),
            id,
            text: text.clone(),
            reply: draft.reply.clone(),
            at: self.when.now_ms(),
            sent_at: self.age,
            failed: false,
            known,
        });
        self.asks.push(DirectAsk::Send {
            channel: channel.clone(),
            text: Private(text),
            reply: draft.reply,
            nonce: self.nonce,
        });
    }

    /// Text entry for the reply box of this conversation: its caption and what it starts with.
    pub fn writing(&self, channel: &Id) -> Write {
        let name = self.direct.dm(channel).map(|dm| (dm.name.clone(), dm.group)).or_else(|| {
            self.direct.open.as_ref().filter(|open| &open.id == channel).map(|open| (open.name.clone(), open.group))
        });
        let (name, group) = name.unwrap_or_default();
        let draft = self.draft(channel);
        let caption = match draft.and_then(|draft| draft.reply.as_ref()) {
            Some(reply) => format!("Replying to {}", self.reply_name(channel, reply)),
            None if group => format!("Message {}", name.as_str()),
            None => format!("Message @{}", name.as_str()),
        };
        Write { channel: channel.clone(), caption, text: draft.map(|draft| draft.text.clone()).unwrap_or_default() }
    }

    /// Who a reply is to: the author's name, or "yourself" for one's own message.
    pub fn reply_name(&self, channel: &Id, reply: &Id) -> String {
        match self.messages(channel).iter().find(|message| &message.id == reply) {
            Some(message) if !message.own && !message.author_name.is_empty() => {
                message.author_name.as_str().to_string()
            }
            Some(_) => "yourself".into(),
            None => String::new(),
        }
    }

    /// Reply to this message: set with the draft, and the keyboard comes up at once (spec 6.2).
    pub fn reply_to(&mut self, channel: &Id, message: &Id) {
        self.drafts.entry(channel.clone()).or_default().reply = Some(message.clone());
        if let Some(View::Chat(chat)) = &mut self.view {
            chat.focus = Place::Reply;
        }
        self.write = Some(self.writing(channel));
    }

    // --- A message's menu ---

    pub fn message(&self, channel: &Id, id: &Id) -> Option<Message> {
        self.messages(channel).into_iter().find(|message| &message.id == id)
    }

    /// Retry: the failed message goes out again, as a new one, and the old one is dropped from the view.
    pub fn retry(&mut self, channel: &Id, id: &Id) {
        let Some(message) = self.message(channel, id) else { return };
        let reply = match &message.body {
            Body::Said { reply: Some(_), .. } => {
                self.outs.iter().find(|out| &out.id == id).and_then(|out| out.reply.clone())
            }
            _ => None,
        };
        let text = Self::text_of(&message);
        self.forget(channel, id);
        let saved = self.drafts.remove(channel);
        self.drafts.insert(channel.clone(), Draft { text, reply });
        self.send(channel);
        if let Some(saved) = saved {
            self.drafts.insert(channel.clone(), saved);
        }
    }

    /// Delete: a message that was never sent is dropped from the view.
    pub fn forget(&mut self, channel: &Id, id: &Id) {
        self.outs.retain(|out| &out.id != id);
        self.asks.push(DirectAsk::Forget { channel: channel.clone(), message: id.clone() });
        self.follow();
    }

    /// One of the six reactions on a message: added, or taken off if the user has made it.
    pub fn react(&mut self, channel: &Id, id: &Id, which: usize) {
        let Some((emoji, _)) = REACTIONS.get(which) else { return };
        let mine = self.message(channel, id).is_some_and(|message| match message.body {
            Body::Said { reactions, .. } => reactions.iter().any(|reaction| {
                reaction.mine
                    && matches!(&reaction.emoji, model::discord_dms::Emoji::Unicode(e) if e.as_str() == *emoji)
            }),
            _ => false,
        });
        self.asks.push(DirectAsk::React {
            channel: channel.clone(),
            message: id.clone(),
            emoji: Private((*emoji).to_string()),
            on: !mine,
        });
    }

    // --- The pad ---

    /// A press while the view is up.
    pub fn press(&mut self, press: Press) -> Step {
        match self.view.clone() {
            Some(View::All(all)) => self.press_all(all, press),
            Some(View::Chat(chat)) => self.press_chat(chat, press),
            None => Step::Nothing,
        }
    }

    fn press_all(&mut self, all: All, press: Press) -> Step {
        let last = self.direct.dms.len().saturating_sub(1);
        let set = |me: &mut Self, item: usize| {
            if let Some(View::All(all)) = &mut me.view {
                all.item = item;
            }
        };
        match press.button {
            Button::Up if all.item > 0 => {
                set(self, all.item - 1);
                Step::Moved
            }
            Button::Down if all.item < last => {
                set(self, all.item + 1);
                Step::Moved
            }
            Button::A => match self.direct.dms.get(all.item).map(|dm| dm.id.clone()) {
                Some(id) => {
                    self.open_chat(id, Back::AllDirectMessages);
                    Step::Chose
                }
                None => Step::Nothing,
            },
            Button::Menu => match self.direct.dms.get(all.item) {
                Some(dm) => Step::Menu(Target::Dm(dm.id.clone())),
                None => Step::Nothing,
            },
            Button::B => {
                self.view = None;
                Step::Left
            }
            _ => Step::Nothing,
        }
    }

    fn press_chat(&mut self, chat: Chat, press: Press) -> Step {
        let stops = self.stops(&chat.id);
        let at = match &chat.focus {
            Place::Message(id) => stops.iter().position(|message| &message.id == id),
            Place::Reply => None,
        };
        let go = |me: &mut Self, place: Place| {
            if let Some(View::Chat(chat)) = &mut me.view {
                chat.focus = place;
                if chat.focus == Place::Reply {
                    chat.seen = stops.last().map(|message| message.id.clone());
                }
            }
            me.check_older();
            Step::Moved
        };
        let to_stop = |me: &mut Self, at: usize| go(me, Place::Message(stops[at].id.clone()));
        let page = chat.shown.max(1);
        match (press.button, &chat.focus) {
            (Button::Up, Place::Reply) if !stops.is_empty() => to_stop(self, stops.len() - 1),
            (Button::Up, Place::Message(_)) if at.is_some_and(|at| at > 0) => to_stop(self, at.unwrap_or(0) - 1),
            // Up on the oldest asks for older ones.
            (Button::Up, Place::Message(_)) => {
                self.check_older();
                Step::Nothing
            }
            (Button::Down, Place::Message(_)) => match at {
                Some(at) if at + 1 < stops.len() => to_stop(self, at + 1),
                _ => go(self, Place::Reply),
            },
            (Button::LeftTrigger, Place::Reply) if !stops.is_empty() => to_stop(self, stops.len().saturating_sub(page)),
            (Button::LeftTrigger, Place::Message(_)) if at.is_some() => {
                to_stop(self, at.unwrap_or(0).saturating_sub(page))
            }
            (Button::RightTrigger, Place::Message(_)) => match at {
                Some(at) if at + 1 < stops.len() => to_stop(self, (at + page).min(stops.len() - 1)),
                _ => go(self, Place::Reply),
            },
            (Button::A, Place::Message(id)) => Step::Menu(Target::Message(id.clone())),
            (Button::A, Place::Reply) => {
                self.write = Some(self.writing(&chat.id));
                Step::Chose
            }
            (Button::Menu, Place::Message(id)) => Step::Menu(Target::Message(id.clone())),
            (Button::Menu, Place::Reply) => {
                if self.draft(&chat.id).is_some_and(|draft| !draft.text.trim().is_empty()) {
                    self.send(&chat.id);
                    Step::Chose
                } else {
                    Step::Nothing
                }
            }
            (Button::X, Place::Message(id)) => {
                self.reply_to(&chat.id, &id.clone());
                Step::Chose
            }
            (Button::X, Place::Reply) => {
                let had = self.draft(&chat.id).is_some_and(|draft| draft.reply.is_some());
                if had {
                    if let Some(draft) = self.drafts.get_mut(&chat.id) {
                        draft.reply = None;
                    }
                    self.set_draft(&chat.id, &self.draft(&chat.id).map(|draft| draft.text.clone()).unwrap_or_default());
                }
                if had {
                    Step::Moved
                } else {
                    Step::Nothing
                }
            }
            (Button::Y, Place::Message(id)) => Step::Menu(Target::Reaction(id.clone())),
            (Button::B, _) => {
                self.leave_chat(&chat);
                Step::Left
            }
            _ => Step::Nothing,
        }
    }

    /// B: the conversation closes, back to where it was opened from.
    fn leave_chat(&mut self, chat: &Chat) {
        self.asks.push(DirectAsk::Close);
        self.view = match chat.back {
            Back::Tab => None,
            Back::AllDirectMessages => {
                let item = self.direct.dms.iter().position(|dm| dm.id == chat.id).unwrap_or(0);
                Some(View::All(All { item, scroll: 0.0 }))
            }
        };
    }

    // --- The log ---

    /// `--log-focus`'s part for what is open (spec 10): places and counts, never a name, an id or a word of a
    /// message. `, flyout DirectMessages DM(7) of 25, unread 2` or `, flyout Conversation Message(3) of 50, theirs`.
    pub fn line(&self) -> Option<String> {
        match self.view.as_ref()? {
            View::All(all) => {
                let total = self.direct.dms.len();
                let Some(dm) = self.direct.dms.get(all.item) else {
                    return Some(", flyout DirectMessages empty".into());
                };
                let unread = if dm.unread > 0 { format!(", unread {}", dm.unread) } else { String::new() };
                let group = if dm.group { ", group" } else { "" };
                Some(format!(", flyout DirectMessages DM({}) of {total}{unread}{group}", all.item))
            }
            View::Chat(chat) => {
                if !chat.arrived {
                    return Some(", flyout Conversation loading".into());
                }
                let stops = self.stops(&chat.id);
                let (place, new) = match &chat.focus {
                    Place::Reply => {
                        let draft = self.draft(&chat.id);
                        let chars = draft.map_or(0, |draft| draft.text.chars().count());
                        let replying = if draft.is_some_and(|draft| draft.reply.is_some()) { ", replying" } else { "" };
                        (format!("ReplyBox; draft {chars} chars{replying}"), String::new())
                    }
                    Place::Message(id) => {
                        let at = stops.iter().position(|message| &message.id == id);
                        let Some(at) = at else { return Some(", flyout Conversation".into()) };
                        let message = &stops[at];
                        let mut line =
                            format!("Message({}) of {}, {}", stops.len() - 1 - at, stops.len(), side(message));
                        if chat.unread_from.as_ref() == Some(id) {
                            line.push_str(", unread line above");
                        }
                        match message.delivery {
                            Delivery::Sending => line.push_str(", sending"),
                            Delivery::Failed => line.push_str(", not sent"),
                            Delivery::Sent => {}
                        }
                        (line, self.new_below(chat, &stops))
                    }
                };
                let new = if new.is_empty() { new } else { format!(", {new}") };
                Some(format!(", flyout Conversation {place}{new}"))
            }
        }
    }

    /// "2 new below": how many messages came since focus was last at the foot, while it is not.
    fn new_below(&self, chat: &Chat, stops: &[Message]) -> String {
        match self.new_count(chat, stops) {
            0 => String::new(),
            n => format!("{n} new below"),
        }
    }

    /// How many messages are newer than the last in view with focus at the foot.
    pub fn new_count(&self, chat: &Chat, stops: &[Message]) -> usize {
        let Some(seen) = &chat.seen else { return 0 };
        // Only what others wrote counts as new.
        match stops.iter().position(|message| &message.id == seen) {
            Some(at) => stops[at + 1..].iter().filter(|message| !message.own).count(),
            None => 0,
        }
    }
}

fn side(message: &Message) -> &'static str {
    if message.own {
        "own"
    } else {
        "theirs"
    }
}

/// A message's first line.
pub fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_string()
}

/// A message's menu title: its first line, cut short.
pub fn menu_title(text: &str) -> String {
    let line = first_line(text);
    if line.chars().count() > 40 {
        format!("{}...", line.chars().take(40).collect::<String>().trim_end())
    } else if line.is_empty() {
        "Message".into()
    } else {
        line
    }
}

/// The glyph for one of Discord's own lines.
pub fn line_glyph(kind: LineKind) -> char {
    match kind {
        LineKind::CallStarted => '\u{E717}',
        LineKind::Added => '\u{E8FA}',
        LineKind::Pinned => '\u{E718}',
        LineKind::Other => '\u{E946}',
    }
}
