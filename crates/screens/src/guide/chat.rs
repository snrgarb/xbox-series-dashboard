//! What All Direct Messages and a conversation look like (docs/spec/party-chats.md, sections 4 and 5): the Flyout's
//! rows, the bubbles of a conversation laid out down a column with their runs, times, day dividers, unread line,
//! replies, reactions and the lines for what isn't text, and the reply box under them.
//!
//! A frame lays the conversation out whole (the text wrapped by hand, so a mention, a link and an emoji can each be
//! drawn in its own style), then places the view by the least movement that shows the focused bubble with its ring
//! (spec 5.4), and draws what is in view. The view's place and how many messages it shows are kept in the `Chat`.
use model::discord_dms::{Body, Delivery, Dm, Emoji, Extra, Id, Message, Picture, Reaction, Reply, Span};
use ui::{Color, Ease, Font, HAlign, Rect, TextStyle, Ui, VAlign};

use super::direct::{line_glyph, Chat, Dms, Place, View};
use super::discord::{dm_picture, Note};
use super::flyout::{
    BACK, FIRST_TOP, FLYOUT, FOCUSED, FOOT, INSET, OPEN_SECONDS, PANEL_DIM, RADIUS, TITLE_TOP, TRAVEL,
};
use super::panel::PANEL;
use super::when::When;
use super::{focus_ring, HEADING};
use crate::theme::{ACCENT, ACCENT_LIGHT, BODY, BODY_SEMIBOLD, TEXT_DIM};

const DIVIDER: Color = Color::argb(0x33FFFFFF);
const THEIRS: Color = Color::rgb(0x2E2E2E);
const FIELD: Color = Color::rgb(0x2E3136);
const RED: Color = Color::rgb(0xED4245);
const PILL: Color = Color::argb(0x14FFFFFF);
const BAR: Color = Color::argb(0x80FFFFFF);
const DECODED: u32 = 128;

/// The content column, x 183..663.
const X0: f32 = 183.0;
const X1: f32 = 663.0;
const WIDTH: f32 = X1 - X0;
/// The message view, y 134 to 970; the reply box under it.
const VIEW_TOP: f32 = 134.0;
const VIEW_FOOT: f32 = 970.0;
const VIEW_FOOT_REPLYING: f32 = 940.0;
const VIEW_FOOT_WRITING: f32 = 556.0;
const BOX: Rect = Rect::new(X0, 986.0, WIDTH, 54.0);
const REPLYING: Rect = Rect::new(X0, 948.0, WIDTH, 34.0);
const MAX_BUBBLE: f32 = 400.0;
const MAX_BUBBLE_GROUP: f32 = 384.0;
const AVATAR: f32 = 36.0;
/// A bubble's padding, and its room from the ring to the view's edge.
const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 10.0;
const RING_ROOM: f32 = 12.0;
/// Reactions: shown at most this many.
const REACTIONS_SHOWN: usize = 8;
const SEMIBOLD: TextStyle = BODY_SEMIBOLD;
const EMOJI: TextStyle = TextStyle::new(Font::Ui, 24.0, 400);

// --- The Flyout's frame ---

/// The Flyout over the Guide's dimmed panel, with a title: its body is drawn by `body`.
fn frame(ui: &mut Ui, opening: u32, title: &str, grouped: bool, body: impl FnOnce(&mut Ui)) {
    // A group is one layer: out of focus behind a menu, it is the group's own.
    if !grouped {
        ui.layer();
    }
    ui.fill(PANEL, PANEL_DIM, RADIUS);
    let slide = ui.slide_from(("direct flyout", opening), TRAVEL, 0.0, OPEN_SECONDS, Ease::CubicOut);
    let alpha = ui.tween_from(("direct flyout fade", opening), 0.0, 1.0, OPEN_SECONDS, Ease::Linear);
    ui.with_offset(slide, 0.0, |ui| {
        ui.with_alpha(alpha, |ui| {
            ui.shadow(FLYOUT, Color::BLACK.fade(0.7), RADIUS, 24.0);
            ui.fill(FLYOUT, BACK, RADIUS);
            if !title.is_empty() {
                let line = Rect::new(FLYOUT.x + 28.0, TITLE_TOP, FLYOUT.w - 56.0, 44.0);
                ui.paragraph(line, title, &HEADING, HAlign::Left, VAlign::Top, 1);
            }
            body(ui);
        });
    });
}

fn line_height(ui: &mut Ui) -> f32 {
    ui.measure(&BODY, "Ag").height
}

impl Dms {
    /// Draws what is open over the panel: All Direct Messages or the conversation.
    pub fn draw(&mut self, ui: &mut Ui, writing: bool, grouped: bool) {
        let opening = self.opening;
        match self.view.clone() {
            Some(View::All(all)) => {
                let (mut item, mut scroll) = (all.item, all.scroll);
                frame(ui, opening, "Direct Messages", grouped, |ui| {
                    scroll = self.draw_all(ui, &mut item, scroll);
                });
                if let Some(View::All(all)) = &mut self.view {
                    all.scroll = scroll;
                }
            }
            Some(View::Chat(chat)) => self.draw_chat(ui, opening, chat, writing, grouped),
            None => {}
        }
    }

    // --- All Direct Messages ---

    fn draw_all(&self, ui: &mut Ui, item: &mut usize, scroll: f32) -> f32 {
        let direct = &self.direct;
        if let Some(note) = Note::of(direct) {
            // States are section 3.5's lines at x 183 under the title.
            let line = Rect::new(X0, FIRST_TOP, WIDTH, FOOT - FIRST_TOP);
            ui.paragraph(line, note.text(), &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 3);
            return 0.0;
        }
        let rows = direct.dms.len();
        *item = (*item).min(rows.saturating_sub(1));
        let (height, below) = (96.0, 3.0);
        let top = |at: usize| FIRST_TOP + at as f32 * (height + below);
        let extent = (top(*item), top(*item) + height + below);
        let scroll = super::entries::scroll_to(scroll, *item == 0, extent, (FIRST_TOP, FOOT));
        let clip = Rect::new(FLYOUT.x - 24.0, FIRST_TOP - 24.0, FLYOUT.w + 48.0, FOOT - FIRST_TOP + 24.0);
        ui.with_clip(clip, |ui| {
            ui.with_offset(0.0, -scroll, |ui| {
                let unfocused = (0..rows).filter(|&at| at != *item);
                for at in unfocused.chain((rows > 0).then_some(*item)) {
                    let rect = Rect::new(FLYOUT.x + INSET, top(at), FLYOUT.w - 2.0 * INSET, height);
                    if rect.bottom() - scroll < FIRST_TOP - 30.0 || rect.y - scroll > FOOT + 30.0 {
                        continue;
                    }
                    self.dm_row(ui, rect, &direct.dms[at], at == *item);
                }
            });
        });
        scroll
    }

    /// A row: the avatar 60 across 18 in, the name (semibold while unread) with the time right-aligned on its line,
    /// the last message dim under it, and the unread count in a pill at the right of that line.
    fn dm_row(&self, ui: &mut Ui, rect: Rect, dm: &Dm, focused: bool) {
        let (_, cy) = rect.center();
        if focused {
            ui.fill(rect, FOCUSED, 4.0);
        }
        dm_picture(
            ui,
            Rect::new(rect.x + 18.0, cy - 30.0, 60.0, 60.0),
            &dm.name,
            dm.group,
            dm.picture.as_ref(),
            DECODED,
        );
        if focused {
            focus_ring(ui, rect);
        }
        let lh = line_height(ui);
        let second: Option<String> = match (&dm.last, dm.group) {
            (Some((text, _)), _) => Some(text.as_str().lines().next().unwrap_or_default().to_string()),
            (None, true) => Some(format!("{} members", dm.members)),
            (None, false) => None,
        };
        let time = dm.last.as_ref().map(|(_, at)| self.when.stamp(*at));
        let stacked = second.is_some() || time.is_some() || dm.unread > 0;
        let left = rect.x + 96.0;
        let right = rect.right() - 24.0;
        let top = if stacked { cy - lh } else { cy - lh / 2.0 };
        let dim = BODY.color(TEXT_DIM);
        let style = if dm.unread > 0 { SEMIBOLD } else { BODY };
        let time_w = time.as_ref().map_or(0.0, |time| ui.measure(&dim, time).width + 16.0);
        ui.paragraph(
            Rect::new(left, top, (right - left - time_w).max(0.0), lh),
            &dm.name,
            &style,
            HAlign::Left,
            VAlign::Top,
            1,
        );
        if let Some(time) = &time {
            ui.label_right(right, top + lh / 2.0, time, &dim);
        }
        let mut pill = 0.0;
        if dm.unread > 0 {
            let count = dm.unread.to_string();
            let w = (ui.measure(&BODY, &count).width + 20.0).max(34.0);
            let at = Rect::new(right - w, cy + lh / 2.0 - 17.0, w, 34.0);
            ui.fill(at, ACCENT_LIGHT, 17.0);
            ui.label_centered(at.center().0, at.center().1, &count, &BODY);
            pill = w + 12.0;
        }
        if let Some(second) = &second {
            let line = Rect::new(left, top + lh, (right - left - pill).max(0.0), lh);
            ui.paragraph(line, second, &dim, HAlign::Left, VAlign::Top, 1);
        }
    }

    // --- The conversation ---

    fn title_of(&self, id: &Id) -> (String, bool, Option<Picture>) {
        if let Some(dm) = self.direct.dm(id) {
            return (dm.name.as_str().to_string(), dm.group, dm.picture.clone());
        }
        match self.direct.open.as_ref().filter(|open| &open.id == id) {
            Some(open) => (open.name.as_str().to_string(), open.group, open.picture.clone()),
            None => (String::new(), false, None),
        }
    }

    fn draw_chat(&mut self, ui: &mut Ui, opening: u32, chat: Chat, writing: bool, grouped: bool) {
        let (name, group, picture) = self.title_of(&chat.id);
        let messages = self.messages(&chat.id);
        let draft = self.draft(&chat.id).cloned().unwrap_or_default();
        let replying = draft.reply.as_ref().map(|id| self.reply_name(&chat.id, id));
        let open = self.direct.open.clone().filter(|open| open.id == chat.id);
        let (at_start, loading_older) =
            open.as_ref().map_or((false, false), |open| (open.at_start, open.loading_older));
        let when = self.when;
        let new = {
            let stops: Vec<Message> =
                messages.iter().filter(|message| matches!(message.body, Body::Said { .. })).cloned().collect();
            self.new_count(&chat, &stops)
        };
        let mut placed = None;
        frame(ui, opening, "", grouped, |ui| {
            // The title: the avatar 48 across, the name 16 after it, a divider under.
            let lh = ui.measure(&HEADING, "Ag").height;
            let middle = TITLE_TOP + lh / 2.0;
            dm_picture(ui, Rect::new(X0, middle - 24.0, 48.0, 48.0), &name, group, picture.as_ref(), DECODED);
            let line = Rect::new(X0 + 48.0 + 16.0, TITLE_TOP, X1 - (X0 + 64.0), 44.0);
            ui.paragraph(line, &name, &HEADING, HAlign::Left, VAlign::Top, 1);
            ui.fill(Rect::new(X0, 133.0, WIDTH, 1.0), DIVIDER, 0.0);
            if !chat.arrived {
                crate::spinner::spinner(ui, FLYOUT.center().0, (VIEW_TOP + VIEW_FOOT) / 2.0, 102.0);
                return;
            }
            let foot = if writing {
                VIEW_FOOT_WRITING
            } else if replying.is_some() {
                VIEW_FOOT_REPLYING
            } else {
                VIEW_FOOT
            };
            let built = build(ui, &messages, &chat, group, &name, at_start, loading_older, &when);
            placed = Some(view(ui, &built, &chat, foot, new));
            reply_box(ui, &draft, &name, group, replying.as_deref(), chat.focus == Place::Reply && !writing);
        });
        if let (Some(placed), Some(View::Chat(live))) = (placed, &mut self.view) {
            live.scroll = placed.scroll;
            live.shown = placed.shown.max(1);
            if let Some(focus) = placed.focus {
                live.focus = focus;
            }
            live.placed = true;
        }
        self.check_older();
    }
}

// --- Laying a conversation out ---

/// What a run of text is drawn as.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Look {
    Plain,
    Mention,
    Link,
    Dim,
}

impl Look {
    fn style(self) -> TextStyle {
        match self {
            Look::Plain | Look::Link => BODY,
            Look::Mention => SEMIBOLD.color(ACCENT_LIGHT),
            Look::Dim => BODY.color(TEXT_DIM),
        }
    }
}

struct Run {
    x: f32,
    text: String,
    look: Look,
}

/// Text wrapped by hand to a width: the runs of each line, and the widest line.
struct Wrapped {
    lines: Vec<Vec<Run>>,
    width: f32,
}

fn space_width(ui: &mut Ui) -> f32 {
    ui.measure(&BODY, "a a").width - 2.0 * ui.measure(&BODY, "a").width
}

/// Wraps spans to `max` wide: greedy by words, a word too long for a line broken by characters, new lines kept. A
/// space between two words is one space wide (the first of a run of them), and none starts or ends a line.
fn wrap(ui: &mut Ui, spans: &[Span], max: f32) -> Wrapped {
    let sp = space_width(ui);
    let mut lines: Vec<Vec<Run>> = vec![Vec::new()];
    let (mut x, mut width) = (0.0f32, 0.0f32);
    // A space has been seen since the last word.
    let mut space = false;
    for span in spans {
        let (text, look) = match span {
            Span::Text(text) => (text.as_str().to_string(), Look::Plain),
            Span::Mention(text) => (text.as_str().to_string(), Look::Mention),
            Span::Link(text) => (text.as_str().to_string(), Look::Link),
            Span::ServerEmoji(name) => (format!(":{}:", name.as_str()), Look::Dim),
        };
        let style = look.style();
        for (at, line) in text.split('\n').enumerate() {
            if at > 0 {
                lines.push(Vec::new());
                (x, space) = (0.0, false);
            }
            for (place, piece) in line.split(' ').enumerate() {
                // Between two pieces there was a space.
                space |= place > 0;
                if piece.is_empty() {
                    continue;
                }
                let w = ui.measure(&style, piece).width;
                let gap = if space && x > 0.0 { sp } else { 0.0 };
                if x > 0.0 && x + gap + w > max {
                    lines.push(Vec::new());
                    x = 0.0;
                } else {
                    x += gap;
                }
                space = false;
                let last = lines.last_mut().expect("a line");
                if w <= max {
                    last.push(Run { x, text: piece.to_string(), look });
                    x += w;
                    width = width.max(x);
                    continue;
                }
                // A word wider than the line is broken by characters.
                let mut chunk = String::new();
                for c in piece.chars() {
                    let mut tried = chunk.clone();
                    tried.push(c);
                    if !chunk.is_empty() && x + ui.measure(&style, &tried).width > max {
                        let w = ui.measure(&style, &chunk).width;
                        lines.last_mut().expect("a line").push(Run { x, text: std::mem::take(&mut chunk), look });
                        width = width.max(x + w);
                        lines.push(Vec::new());
                        x = 0.0;
                    }
                    chunk.push(c);
                }
                let w = ui.measure(&style, &chunk).width;
                lines.last_mut().expect("a line").push(Run { x, text: chunk, look });
                x += w;
                width = width.max(x);
            }
        }
    }
    // Trailing empty lines of a message that ends in new lines are dropped.
    while lines.len() > 1 && lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    Wrapped { lines, width }
}

/// One thing in the column.
enum What {
    Loading,
    Start(String),
    Day(String),
    Unread,
    System(char, String),
    Name(String, f32),
    /// The time under a run, with a mark if some message in it was edited: its side.
    Time {
        text: String,
        own: bool,
        right: f32,
        left: f32,
        failed: bool,
    },
    Bubble(Box<Bubble>),
}

struct Bubble {
    id: Id,
    rect: Rect,
    own: bool,
    opacity: f32,
    reply: Option<String>,
    extras: Vec<(char, String)>,
    text: Wrapped,
    avatar: Option<(Rect, String, Option<Picture>)>,
    pills: Vec<PillAt>,
    more: Option<(f32, f32, String)>,
}

struct PillAt {
    rect: Rect,
    emoji: String,
    server: bool,
    count: String,
    mine: bool,
}

struct Row {
    top: f32,
    height: f32,
    what: What,
}

struct Built {
    rows: Vec<Row>,
    height: f32,
    /// A stop's message id, and the column's y of its top and foot.
    stops: Vec<(Id, f32, f32)>,
    /// The column's y of the unread line.
    unread: Option<f32>,
}

fn extra_line(extra: &Extra) -> (char, String) {
    match extra {
        Extra::Images(1) => ('\u{EB9F}', "Image".into()),
        Extra::Images(n) => ('\u{EB9F}', format!("{n} images")),
        Extra::Video => ('\u{E714}', "Video".into()),
        Extra::Gif => ('\u{EB9F}', "GIF".into()),
        Extra::File(name) => ('\u{E8A5}', format!("File: {}", name.as_str())),
        Extra::VoiceMessage => ('\u{E720}', "Voice message".into()),
        Extra::Sticker(name) => ('\u{E76E}', format!("Sticker: {}", name.as_str())),
        Extra::Embed(title) => ('\u{E71B}', title.as_str().to_string()),
        Extra::Poll(question) => ('\u{E9D9}', format!("Poll: {}", question.as_str())),
        Extra::Other => ('\u{E946}', "Something the Dashboard can't show".into()),
    }
}

const RUN_MS: i64 = 7 * 60 * 1000;

/// The run of one author's messages being laid out.
struct RunState {
    author: String,
    own: bool,
    at: i64,
    day: chrono::NaiveDate,
    edited: bool,
    side: (f32, f32),
    failed: bool,
    sending: bool,
}

/// Ends a run: the time under its last bubble (none while it is going out), then the gap to the next run.
fn close_run(rows: &mut Vec<Row>, y: &mut f32, run: &mut Option<RunState>, lh: f32, when: &When) {
    let Some(state) = run.take() else { return };
    if !state.sending {
        let text = if state.failed {
            "Message failed to send".to_string()
        } else if state.edited {
            format!("{} (edited)", when.time(state.at))
        } else {
            when.time(state.at)
        };
        *y += 4.0;
        let what = What::Time { text, own: state.own, left: state.side.0, right: state.side.1, failed: state.failed };
        rows.push(Row { top: *y, height: lh, what });
        *y += lh;
    }
    *y += 16.0;
}

/// Lays the messages down the column from y 0.
#[allow(clippy::too_many_arguments)]
fn build(
    ui: &mut Ui,
    messages: &[Message],
    chat: &Chat,
    group: bool,
    name: &str,
    at_start: bool,
    loading_older: bool,
    when: &When,
) -> Built {
    let lh = line_height(ui);
    let mut rows: Vec<Row> = Vec::new();
    let mut stops = Vec::new();
    let mut unread = None;
    let mut y = 0.0f32;
    if loading_older {
        rows.push(Row { top: y, height: 60.0, what: What::Loading });
        y += 60.0;
    } else if at_start {
        let who = if group { name.to_string() } else { format!("@{name}") };
        let text = format!("This is the beginning of your direct message history with {who}.");
        let h = ui.measure_wrapped(&BODY, &text, WIDTH, 0).height;
        rows.push(Row { top: y, height: h, what: What::Start(text) });
        y += h + 16.0;
    }
    let mut run: Option<RunState> = None;
    let mut last_day = None;
    for message in messages {
        let day = when.day_of(message.at);
        if last_day != Some(day) {
            close_run(&mut rows, &mut y, &mut run, lh, when);
            let h = 24.0 + lh + 24.0;
            rows.push(Row { top: y, height: h, what: What::Day(when.day(message.at)) });
            y += h;
            last_day = Some(day);
        }
        if chat.unread_from.as_ref() == Some(&message.id) {
            close_run(&mut rows, &mut y, &mut run, lh, when);
            let h = 16.0 + lh + 16.0;
            unread = Some(y);
            rows.push(Row { top: y, height: h, what: What::Unread });
            y += h;
        }
        let Body::Said { text, reply, extras, reactions } = &message.body else {
            close_run(&mut rows, &mut y, &mut run, lh, when);
            let Body::Line { kind, text } = &message.body else { continue };
            let h = lh + 16.0;
            rows.push(Row { top: y, height: h, what: What::System(line_glyph(*kind), text.as_str().to_string()) });
            y += h;
            continue;
        };
        let author = if message.own { String::new() } else { message.author.as_str().to_string() };
        let same = run.as_ref().is_some_and(|state| {
            state.own == message.own
                && state.author == author
                && state.day == day
                && (message.at - state.at).abs() <= RUN_MS
        });
        if same {
            y += 4.0;
        } else {
            close_run(&mut rows, &mut y, &mut run, lh, when);
        }
        let theirs_group = group && !message.own;
        if !same && theirs_group && !message.author_name.is_empty() {
            let who = message.author_name.as_str().to_string();
            rows.push(Row { top: y, height: lh, what: What::Name(who, X0 + AVATAR + 12.0) });
            y += lh + 4.0;
        }
        let max_bubble = if theirs_group { MAX_BUBBLE_GROUP } else { MAX_BUBBLE };
        let inner = max_bubble - 2.0 * PAD_X;
        let wrapped = if text.is_empty() { Wrapped { lines: Vec::new(), width: 0.0 } } else { wrap(ui, text, inner) };
        let reply_line = reply.as_ref().map(|reply| match reply {
            Reply::To { author, line } => format!("{}: {}", author.as_str(), line.as_str()),
            Reply::Deleted => "Original message was deleted".to_string(),
        });
        let extra_lines: Vec<(char, String)> = extras.iter().map(extra_line).collect();
        let mut content_w = wrapped.width;
        if let Some(line) = &reply_line {
            content_w = content_w.max((ui.measure(&BODY, line).width + 11.0).min(inner));
        }
        for (_, line) in &extra_lines {
            content_w = content_w.max((ui.measure(&BODY, line).width + 36.0).min(inner));
        }
        let w = (content_w + 2.0 * PAD_X).clamp(2.0 * PAD_X + 10.0, max_bubble);
        let mut h = 2.0 * PAD_Y + (wrapped.lines.len() + extra_lines.len()) as f32 * lh;
        if reply_line.is_some() {
            h += lh + 6.0;
        }
        let x = if message.own {
            X1 - w
        } else if theirs_group {
            X0 + AVATAR + 12.0
        } else {
            X0
        };
        let rect = Rect::new(x, y, w, h);
        let avatar = (theirs_group && !same).then(|| {
            let who = message.author_name.as_str().to_string();
            (Rect::new(X0, y, AVATAR, AVATAR), who, message.author_picture.clone())
        });
        let (pills, more, below) = reaction_pills(ui, reactions, message.own, x, y + h + 6.0);
        let sending = message.delivery == Delivery::Sending;
        rows.push(Row {
            top: y,
            height: h + below,
            what: What::Bubble(Box::new(Bubble {
                id: message.id.clone(),
                rect,
                own: message.own,
                opacity: if sending { 0.6 } else { 1.0 },
                reply: reply_line,
                extras: extra_lines,
                text: wrapped,
                avatar,
                pills,
                more,
            })),
        });
        stops.push((message.id.clone(), y, y + h));
        y += h + below;
        let state = run.get_or_insert(RunState {
            author,
            own: message.own,
            at: message.at,
            day,
            edited: false,
            side: (rect.x, rect.right()),
            failed: false,
            sending: false,
        });
        state.at = message.at;
        state.edited |= message.edited;
        state.side = (rect.x, rect.right());
        state.failed = message.delivery == Delivery::Failed;
        state.sending = sending;
    }
    close_run(&mut rows, &mut y, &mut run, lh, when);
    Built { rows, height: y, stops, unread }
}

/// The reactions under a bubble from `top`: pills 36 high, 8 apart, wrapping, from the bubble's side (a user's own
/// message's end at the column's right); at most eight, then "+n". Returns the pills, the "+n" and the height they
/// take with the 6 above them.
fn reaction_pills(
    ui: &mut Ui,
    reactions: &[Reaction],
    own: bool,
    x: f32,
    top: f32,
) -> (Vec<PillAt>, Option<(f32, f32, String)>, f32) {
    if reactions.is_empty() {
        return (Vec::new(), None, 0.0);
    }
    let room = if own { WIDTH } else { X1 - x };
    // Pills by row: (emoji, server?, count, mine, width).
    let mut rows: Vec<Vec<(String, bool, String, bool, f32)>> = vec![Vec::new()];
    let mut used = 0.0f32;
    for reaction in reactions.iter().take(REACTIONS_SHOWN) {
        let (emoji, server) = match &reaction.emoji {
            Emoji::Unicode(e) => (e.as_str().to_string(), false),
            Emoji::Server(n) => (format!(":{}:", n.as_str()), true),
        };
        let ew =
            if server { ui.measure(&BODY.color(TEXT_DIM), &emoji).width } else { ui.measure(&EMOJI, &emoji).width };
        let count = reaction.count.to_string();
        let w = 10.0 + ew + 6.0 + ui.measure(&BODY, &count).width + 10.0;
        if used > 0.0 && used + w > room {
            rows.push(Vec::new());
            used = 0.0;
        }
        used += w + 8.0;
        rows.last_mut().expect("a row").push((emoji, server, count, reaction.mine, w));
    }
    let more = reactions.len().saturating_sub(REACTIONS_SHOWN);
    let more_text = (more > 0).then(|| format!("+{more}"));
    let more_w = more_text.as_ref().map_or(0.0, |text| ui.measure(&BODY, text).width);
    let mut pills = Vec::new();
    let mut more_at = None;
    for (r, row) in rows.iter().enumerate() {
        let y = top + r as f32 * 44.0;
        let total: f32 = row.iter().map(|p| p.4 + 8.0).sum::<f32>() - 8.0;
        let last = r + 1 == rows.len();
        let total = if last && more_text.is_some() { total + 8.0 + more_w } else { total };
        let mut px = if own { X1 - total } else { x };
        for (emoji, server, count, mine, w) in row {
            pills.push(PillAt {
                rect: Rect::new(px, y, *w, 36.0),
                emoji: emoji.clone(),
                server: *server,
                count: count.clone(),
                mine: *mine,
            });
            px += w + 8.0;
        }
        if last {
            if let Some(text) = &more_text {
                more_at = Some((px, y, text.clone()));
            }
        }
    }
    (pills, more_at, 6.0 + rows.len() as f32 * 36.0 + (rows.len() - 1) as f32 * 8.0)
}

// --- Placing the view and drawing ---

/// Where the frame put the view, and what it decided.
struct Placed {
    scroll: f32,
    /// How many bubbles are whole in view.
    shown: usize,
    /// Focus, if the frame decided it (opening on the first unread message).
    focus: Option<Place>,
}

/// Places the view by the least movement that shows the focused bubble with its ring (spec 5.4), draws what is in
/// it, and the pill for new messages.
fn view(ui: &mut Ui, built: &Built, chat: &Chat, foot: f32, new: usize) -> Placed {
    let lh = line_height(ui);
    let height = foot - VIEW_TOP;
    let max_scroll = (built.height - height).max(0.0);
    let mut focus = None;
    let mut scroll = chat.scroll;
    let mut at = chat.focus.clone();
    if !chat.placed {
        // Opening: with unread that don't all fit, focus is on the first unread message with the unread line 12
        // under the view's top; else on the reply box with the newest at the foot.
        match built.unread {
            Some(line) if built.height - line > height => {
                let first = chat.unread_from.clone().filter(|id| built.stops.iter().any(|stop| &stop.0 == id));
                if let Some(id) = first {
                    scroll = line - 12.0;
                    at = Place::Message(id.clone());
                    focus = Some(Place::Message(id));
                }
            }
            _ => scroll = max_scroll,
        }
    } else if at == Place::Reply {
        scroll = max_scroll;
    }
    if let Place::Message(id) = &at {
        if let Some(&(_, top, bottom)) = built.stops.iter().find(|stop| &stop.0 == id) {
            let (top, bottom) = (top - RING_ROOM, bottom + RING_ROOM);
            if bottom - top > height || top < scroll {
                scroll = top;
            } else if bottom > scroll + height {
                scroll = bottom - height;
            }
        }
    }
    let scroll = scroll.clamp(0.0, max_scroll);
    // A conversation shorter than its view stands at its foot.
    let lift = (height - built.height).max(0.0);
    let origin = VIEW_TOP + lift - scroll;
    let clip = Rect::new(X0 - 16.0, VIEW_TOP, WIDTH + 32.0, height);
    let mut shown = 0;
    let focused_id = match &at {
        Place::Message(id) => Some(id),
        Place::Reply => None,
    };
    ui.with_clip(clip, |ui| {
        ui.with_offset(0.0, origin, |ui| {
            for row in &built.rows {
                let (top, bottom) = (origin + row.top, origin + row.top + row.height);
                if bottom < VIEW_TOP - 20.0 || top > foot + 20.0 {
                    continue;
                }
                match &row.what {
                    What::Bubble(bubble) if Some(&bubble.id) == focused_id => {}
                    What::Bubble(bubble) => {
                        let (top, bottom) = (origin + bubble.rect.y, origin + bubble.rect.bottom());
                        if top >= VIEW_TOP && bottom <= foot {
                            shown += 1;
                        }
                        draw_bubble(ui, bubble, false);
                    }
                    other => draw_other(ui, other, row, lh),
                }
            }
            // The focused bubble last, over its neighbours, with its ring.
            for row in &built.rows {
                if let What::Bubble(bubble) = &row.what {
                    if Some(&bubble.id) == focused_id {
                        let (top, bottom) = (origin + bubble.rect.y, origin + bubble.rect.bottom());
                        if top >= VIEW_TOP && bottom <= foot {
                            shown += 1;
                        }
                        draw_bubble(ui, bubble, true);
                    }
                }
            }
        });
    });
    // "{n} new messages", while focus is higher up and some came since it was at the foot.
    if new > 0 && !matches!(at, Place::Reply) {
        let text = if new == 1 { "1 new message".to_string() } else { format!("{new} new messages") };
        let w = ui.measure(&BODY, &text).width + 20.0 + 8.0 + 20.0 + 20.0;
        let pill = Rect::new(X0 + (WIDTH - w) / 2.0, foot - 12.0 - 36.0, w, 36.0);
        ui.fill(pill, ACCENT_LIGHT, 18.0);
        let cy = pill.center().1;
        ui.label_middle(pill.x + 20.0, cy, &text, &BODY);
        ui.icon(pill.right() - 20.0 - 10.0, cy, '\u{E74B}', 20.0, Color::WHITE);
    }
    Placed { scroll, shown, focus }
}

fn draw_other(ui: &mut Ui, what: &What, row: &Row, lh: f32) {
    let dim = BODY.color(TEXT_DIM);
    let cy = row.top + row.height / 2.0;
    match what {
        What::Loading => crate::spinner::spinner(ui, X0 + WIDTH / 2.0, cy, 36.0),
        What::Start(text) => {
            ui.paragraph(Rect::new(X0, row.top, WIDTH, row.height), text, &dim, HAlign::Center, VAlign::Top, 0);
        }
        What::Day(text) => {
            let w = ui.measure(&dim, text).width;
            let mid = X0 + WIDTH / 2.0;
            ui.label_centered(mid, cy, text, &dim);
            ui.fill(Rect::new(X0, cy - 0.5, mid - w / 2.0 - 12.0 - X0, 1.0), DIVIDER, 0.0);
            ui.fill(Rect::new(mid + w / 2.0 + 12.0, cy - 0.5, X1 - (mid + w / 2.0 + 12.0), 1.0), DIVIDER, 0.0);
        }
        What::Unread => {
            let w = ui.measure(&SEMIBOLD, "New").width;
            ui.fill(Rect::new(X0, cy - 1.0, WIDTH - w - 12.0, 2.0), ACCENT_LIGHT, 0.0);
            ui.label_right(X1, cy, "New", &SEMIBOLD.color(ACCENT_LIGHT));
        }
        What::System(glyph, text) => {
            let w = ui.measure(&dim, text).width;
            let total = 26.0 + 10.0 + w;
            let left = X0 + (WIDTH - total) / 2.0;
            ui.icon(left + 13.0, cy, *glyph, 26.0, TEXT_DIM);
            ui.label_middle(left + 36.0, cy, text, &dim);
        }
        What::Name(text, x) => {
            ui.paragraph(Rect::new(*x, row.top, X1 - x, lh), text, &dim, HAlign::Left, VAlign::Top, 1);
        }
        What::Time { text, own, left, right, failed } => {
            if *failed {
                let w = ui.measure(&BODY, text).width;
                let (start, mid) = if *own { (right - w - 36.0, 0.0) } else { (*left, 0.0) };
                let _ = mid;
                ui.icon(start + 13.0, cy, '\u{E783}', 26.0, RED);
                ui.label_middle(start + 36.0, cy, text, &BODY);
            } else if *own {
                ui.label_right(*right, cy, text, &dim);
            } else {
                ui.label_middle(*left, cy, text, &dim);
            }
        }
        What::Bubble(_) => {}
    }
}

fn draw_bubble(ui: &mut Ui, bubble: &Bubble, focused: bool) {
    let lh = line_height(ui);
    let rect = bubble.rect;
    if let Some((disc, name, picture)) = &bubble.avatar {
        dm_picture(ui, *disc, name, false, picture.as_ref(), DECODED);
    }
    ui.with_alpha(bubble.opacity, |ui| {
        ui.fill(rect, if bubble.own { ACCENT } else { THEIRS }, 12.0);
        let mut y = rect.y + PAD_Y;
        let x = rect.x + PAD_X;
        if let Some(reply) = &bubble.reply {
            ui.fill(Rect::new(x, y + 2.0, 3.0, lh - 4.0), BAR, 1.5);
            ui.paragraph(
                Rect::new(x + 11.0, y, rect.w - 2.0 * PAD_X - 11.0, lh),
                reply,
                &BODY.color(TEXT_DIM),
                HAlign::Left,
                VAlign::Top,
                1,
            );
            y += lh + 6.0;
        }
        for (glyph, line) in &bubble.extras {
            ui.icon(x + 13.0, y + lh / 2.0, *glyph, 26.0, TEXT_DIM);
            let at = Rect::new(x + 36.0, y, rect.w - 2.0 * PAD_X - 36.0, lh);
            ui.paragraph(at, line, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 1);
            y += lh;
        }
        for line in &bubble.text.lines {
            for run in line {
                let style = run.look.style();
                ui.label(x + run.x, y, &run.text, &style);
                if run.look == Look::Link {
                    let w = ui.measure(&style, &run.text).width;
                    ui.fill(Rect::new(x + run.x, y + lh - 5.0, w, 1.0), Color::WHITE, 0.0);
                }
            }
            y += lh;
        }
    });
    for pill in &bubble.pills {
        ui.fill(pill.rect, PILL, 18.0);
        if pill.mine {
            ui.ring(pill.rect, ACCENT_LIGHT, 18.0, 2.0);
        }
        let (cx, cy) = (pill.rect.x + 10.0, pill.rect.center().1);
        let used = if pill.server {
            ui.label_middle(cx, cy, &pill.emoji, &BODY.color(TEXT_DIM)).width
        } else {
            ui.label_middle(cx, cy, &pill.emoji, &EMOJI).width
        };
        ui.label_middle(cx + used + 6.0, cy, &pill.count, &BODY);
    }
    if let Some((x, y, text)) = &bubble.more {
        ui.label_middle(*x, y + 18.0, text, &BODY.color(TEXT_DIM));
    }
    if focused {
        focus_ring(ui, rect);
    }
}

/// The reply box: Search's pill, the draft's end or Discord's placeholder in it; over it, while a reply is set, the
/// line that says to whom.
fn reply_box(
    ui: &mut Ui,
    draft: &super::direct::Draft,
    name: &str,
    group: bool,
    replying: Option<&str>,
    focused: bool,
) {
    if let Some(to) = replying {
        ui.fill(Rect::new(REPLYING.x, REPLYING.y + 2.0, 3.0, REPLYING.h - 4.0), BAR, 1.5);
        let text = format!("Replying to {to}");
        let line = Rect::new(X0 + 16.0, REPLYING.y, WIDTH - 16.0, REPLYING.h);
        ui.paragraph(line, &text, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Center, 1);
    }
    ui.fill(BOX, FIELD, 27.0);
    let room = WIDTH - 56.0;
    let text_at = |ui: &mut Ui, text: &str, style: &TextStyle| {
        ui.paragraph(Rect::new(X0 + 28.0, BOX.y, room, BOX.h), text, style, HAlign::Left, VAlign::Center, 1);
    };
    if draft.text.is_empty() {
        let placeholder = if group { format!("Message {name}") } else { format!("Message @{name}") };
        text_at(ui, &placeholder, &BODY.color(TEXT_DIM));
    } else {
        // One line, its end showing, an ellipsis at the start when it is long.
        let flat: String = draft.text.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
        let shown = if ui.measure(&BODY, &flat).width <= room {
            flat
        } else {
            let chars: Vec<char> = flat.chars().collect();
            let (mut low, mut high) = (0, chars.len());
            while low < high {
                let mid = (low + high) / 2;
                let tail: String = std::iter::once('\u{2026}').chain(chars[mid..].iter().copied()).collect();
                if ui.measure(&BODY, &tail).width <= room {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            std::iter::once('\u{2026}').chain(chars[low..].iter().copied()).collect()
        };
        text_at(ui, &shown, &BODY);
    }
    if focused {
        ui.glow(BOX.grow(11.5), ACCENT_LIGHT.fade(0.5), 38.5, 11.0, 17.0);
        ui.ring(BOX.grow(8.0), ACCENT_LIGHT, 35.0, 4.0);
    }
}
