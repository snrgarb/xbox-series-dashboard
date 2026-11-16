//! The Discord tab (docs/spec/discord-tab.md): in the console's Parties & chats slot, there from the first Connect
//! until Disconnect. While it can't show Discord, a line saying why; signed in, who is, then out of a Call that they
//! aren't in a voice channel, or in one its heading, the people in it, the user first, then Mute, Deafen and Leave
//! under a divider. Mute, Deafen and Leave are asked of Discord (`GuideAction::Discord`) and drawn from what Discord
//! last said, never from the press. Measured from the Bundle's `OverlayWindow.xaml` (its `GuideEntry` template's
//! Discord kinds) and ported from `ExtensionCore.DiscordCall.cs`: there is no picture of the tab among the Bundle's
//! reference pictures.
//!
//! #145 adds the Shortcuts and Servers after the Call's entries ([`entries`]), and the first focus falls to the first
//! Shortcut, else Servers, out of a Call ([`first_focus`]).
use model::discord::{CallAsk, Discord, Person, NOT_IN_A_CALL};
use ui::{ArtKey, Color, Crop, HAlign, Kind as ArtKind, Rect, TextStyle, Ui, VAlign};

use super::entries::{Entry, Kind};
use super::{focus_ring, marks, GuideAction};
use crate::theme::{glyph, ACCENT_LIGHT, BODY, TEXT_DIM};

/// Segoe Fluent Icons, as the Bundle's XAML names them.
mod icon {
    /// The warning, beside why the tab can't show Discord.
    pub const WARNING: char = '\u{E7BA}';
    /// The headset: out of a Call, and Deafen.
    pub const HEADSET: char = '\u{E7F6}';
    /// The microphone: Mute.
    pub const MICROPHONE: char = '\u{E720}';
    /// The microphone struck through: someone muted.
    pub const MUTED: char = '\u{EC54}';
    /// The hang-up: Leave.
    pub const HANG_UP: char = '\u{E778}';
}

/// Discord's blurple, behind its mark on the account card, and its red, Leave's.
const BLURPLE: Color = Color::rgb(0x5865F2);
const RED: Color = Color::rgb(0xED4245);
/// The disc under a person's avatar while there is none, and the figure on it.
const DISC: Color = Color::rgb(0x4A4A4A);
/// A person's avatar is decoded at 96, as the Bundle's converter is asked.
const AVATAR_DECODED: u32 = 96;

/// One of the tab's entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// Why the tab can't show Discord: its only entry.
    Unavailable,
    /// Who is signed in.
    Account,
    /// "You're not in a voice channel".
    Idle,
    /// The Call's heading.
    Channel,
    /// Someone in the Call, by their place in it (the user first).
    Person(usize),
    Mute,
    Deafen,
    Leave,
}

/// The tab's entries, top to bottom.
pub fn entries(discord: &Discord) -> Vec<Entry> {
    let entry = |item: Item, title: &str| Entry::new(Kind::Discord(item), title);
    if let Some(why) = discord.tab_line() {
        return vec![entry(Item::Unavailable, why)];
    }
    let mut entries = Vec::new();
    match &discord.voice.call {
        None => {
            entries.push(entry(Item::Account, discord.user.as_deref().unwrap_or_default()));
            entries.push(entry(Item::Idle, NOT_IN_A_CALL));
        }
        Some(call) => {
            entries.push(entry(Item::Channel, &call.title));
            entries.extend(call.people.iter().enumerate().map(|(at, person)| entry(Item::Person(at), &person.name)));
            entries.push(entry(Item::Mute, "Mute").under_a_divider());
            entries.push(entry(Item::Deafen, "Deafen"));
            entries.push(entry(Item::Leave, "Leave"));
        }
    }
    // #145: the Shortcuts, then Servers under a divider.
    entries
}

fn item(entry: &Entry) -> Option<Item> {
    match entry.kind {
        Kind::Discord(item) => Some(item),
        _ => None,
    }
}

/// The tab's first focus: the first entry A does something on, Mute in a Call; else (#145: the first Shortcut,
/// else Servers) the first entry.
pub fn first_focus(entries: &[Entry]) -> usize {
    entries.iter().position(|entry| item(entry) == Some(Item::Mute)).unwrap_or(0)
}

/// Where focus goes as the tab's entries are made again from Discord as it is now: into a Call or out of one,
/// afresh; otherwise the same entry, or the same person, else the entry now at the same place.
pub fn focus_after(before: (&[Entry], &Discord), after: (&[Entry], &Discord), focus: usize) -> usize {
    let in_call = |entries: &[Entry]| entries.iter().any(|entry| item(entry) == Some(Item::Channel));
    if in_call(before.0) != in_call(after.0) || after.0.is_empty() {
        return first_focus(after.0);
    }
    fn person_id(discord: &Discord, at: usize) -> Option<&str> {
        discord.voice.call.as_ref().and_then(|call| call.people.get(at)).map(|p| p.id.as_str())
    }
    let same = match before.0.get(focus).and_then(item) {
        Some(Item::Person(at)) => {
            let id = person_id(before.1, at);
            after.0.iter().position(|entry| match item(entry) {
                Some(Item::Person(at)) => person_id(after.1, at) == id,
                _ => false,
            })
        }
        Some(focused) => after.0.iter().position(|entry| item(entry) == Some(focused)),
        None => None,
    };
    same.unwrap_or(focus.min(after.0.len() - 1))
}

/// What A does on an entry: Mute, Deafen and Leave ask Discord; on anything else, nothing.
pub fn select(item: Item) -> Option<GuideAction> {
    match item {
        Item::Mute => Some(GuideAction::Discord(CallAsk::Mute)),
        Item::Deafen => Some(GuideAction::Discord(CallAsk::Deafen)),
        Item::Leave => Some(GuideAction::Discord(CallAsk::Leave)),
        Item::Unavailable | Item::Account | Item::Idle | Item::Channel | Item::Person(_) => None,
    }
}

/// An entry's box height, and the room under it.
pub fn height(item: Item) -> (f32, f32) {
    let height = match item {
        Item::Account => 105.0,
        Item::Channel => 60.0,
        Item::Person(_) => 72.0,
        Item::Unavailable | Item::Idle | Item::Mute | Item::Deafen | Item::Leave => 75.0,
    };
    (height, 3.0)
}

/// What `--log-focus`'s Guide line says of the tab after its focus: the entry focus is on, then what Discord says,
/// `Mute; in "General" with E2E Tester, Ana (speaking), Bo (muted, deafened); mute off, deafen off`.
pub fn line(entries: &[Entry], focus: Option<usize>, discord: &Discord) -> String {
    let focused = match focus.and_then(|at| entries.get(at)).and_then(item) {
        Some(Item::Person(at)) => {
            let name = discord.voice.call.as_ref().and_then(|call| call.people.get(at)).map(|p| p.name.as_str());
            format!(" Person {}", name.unwrap_or_default())
        }
        Some(item) => format!(" {item:?}"),
        None => String::new(),
    };
    if let Some(why) = discord.tab_line() {
        return format!("{focused}; {why:?}");
    }
    let on = |on: bool| if on { "on" } else { "off" };
    let settings = format!("mute {}, deafen {}", on(discord.voice.mute), on(discord.voice.deaf));
    let Some(call) = &discord.voice.call else { return format!("{focused}; not in a call; {settings}") };
    let person = |p: &Person| {
        let marks: Vec<&str> = [(p.speaking, "speaking"), (p.muted, "muted"), (p.deafened, "deafened")]
            .iter()
            .filter_map(|&(on, mark)| on.then_some(mark))
            .collect();
        if marks.is_empty() {
            p.name.clone()
        } else {
            format!("{} ({})", p.name, marks.join(", "))
        }
    };
    let people: Vec<String> = call.people.iter().map(person).collect();
    format!("{focused}; in {:?} with {}; {settings}", call.title, people.join(", "))
}

/// Draws one of the tab's entries in `rect`: its fill and ring with focus, what stands at its left, its text, and
/// a person's marks or a switch at its right.
pub fn draw(ui: &mut Ui, item: Item, entry: &Entry, rect: Rect, focused: bool, discord: &Discord) {
    if focused {
        ui.fill(rect, Color::argb(0x26FFFFFF), 4.0);
    }
    let (_, cy) = rect.center();
    let glyph_at = |ui: &mut Ui, glyph: char, size: f32, color: Color| ui.icon(rect.x + 36.0, cy, glyph, size, color);
    // Where the text starts and ends, its colour, and how many lines it may take.
    let mut text = (rect.x + 69.0, rect.right() - 16.0, BODY, 1u8);
    let person = match item {
        Item::Person(at) => discord.voice.call.as_ref().and_then(|call| call.people.get(at)),
        _ => None,
    };
    match item {
        Item::Unavailable => {
            glyph_at(ui, icon::WARNING, 26.0, TEXT_DIM);
            text = (text.0, text.1, BODY.color(TEXT_DIM), 2);
        }
        Item::Account => {
            let disc = Rect::new(rect.x + 18.0, cy - 30.0, 60.0, 60.0);
            ui.fill(disc, BLURPLE, 30.0);
            marks::discord_mark(ui, Rect::new(disc.x + 14.0, disc.y + 14.0, 32.0, 32.0), Color::WHITE);
            text.0 = rect.x + 96.0;
        }
        Item::Idle => {
            glyph_at(ui, icon::HEADSET, 26.0, TEXT_DIM);
            text.2 = BODY.color(TEXT_DIM);
        }
        Item::Channel => glyph_at(ui, glyph::VOLUME, 24.0, TEXT_DIM),
        Item::Person(_) => {
            if let Some(person) = person {
                avatar(ui, Rect::new(rect.x + 18.0, cy - 28.0, 56.0, 56.0), person);
                call_marks(ui, rect.right() - 24.0, cy, person);
            }
            text = (rect.x + 88.0, rect.right() - 96.0, BODY, 1);
        }
        Item::Mute | Item::Deafen => {
            let (glyph, on) = if item == Item::Mute {
                (icon::MICROPHONE, discord.voice.mute)
            } else {
                (icon::HEADSET, discord.voice.deaf)
            };
            glyph_at(ui, glyph, 26.0, Color::WHITE);
            super::flyout::switch(ui, Rect::new(rect.right() - 24.0 - 48.0, cy - 12.0, 48.0, 24.0), on);
        }
        Item::Leave => glyph_at(ui, icon::HANG_UP, 26.0, RED),
    }
    if focused {
        focus_ring(ui, rect);
    }
    let (x, right, style, lines) = text;
    title(ui, Rect::new(x, rect.y, right - x, rect.h), &entry.title, &style, lines);
}

/// The title, at most `lines` lines and cut short with an ellipsis, in the middle of the box's height.
fn title(ui: &mut Ui, rect: Rect, title: &str, style: &TextStyle, lines: u8) {
    let fits = ui.measure_wrapped(style, title, rect.w, 0).lines <= lines as usize;
    let lines = if fits { 0 } else { lines };
    let size = ui.measure_wrapped(style, title, rect.w, lines);
    let top = rect.center().1 - size.height / 2.0;
    ui.with_clip(rect, |ui| {
        ui.paragraph(Rect::new(rect.x, top, rect.w, size.height), title, style, HAlign::Left, VAlign::Top, lines);
    });
}

/// A person's avatar on a disc 48 across in the middle of `rect` (56 across), a grey disc with a figure under it
/// while there is none, ringed in the accent while they speak.
fn avatar(ui: &mut Ui, rect: Rect, person: &Person) {
    let (cx, cy) = rect.center();
    let disc = Rect::new(cx - 24.0, cy - 24.0, 48.0, 48.0);
    ui.fill(disc, DISC, 24.0);
    ui.icon(cx, cy, glyph::CONTACT, 22.0, TEXT_DIM);
    if let Some(path) = &person.avatar {
        let key = ArtKey { path: path.clone(), kind: ArtKind::Thumb(AVATAR_DECODED) };
        ui.art(disc, &key, Crop::Center, 1.0, 24.0);
    }
    if person.speaking {
        // WPF's Ellipse strokes inside its bounds: 3 in from the 56.
        ui.ring(rect, ACCENT_LIGHT, 28.0, 3.0);
    }
}

/// A person's marks, ending at `right`: muted (the microphone struck through), then deafened (the headset with a
/// line across it), 26 across each with 10 between.
fn call_marks(ui: &mut Ui, right: f32, cy: f32, person: &Person) {
    let mut x = right;
    if person.deafened {
        let mark = Rect::new(x - 26.0, cy - 13.0, 26.0, 26.0);
        ui.icon(mark.center().0, cy, icon::HEADSET, 22.0, TEXT_DIM);
        // The line from (3, 3) to (23, 23), 2 wide with round ends.
        let length = 20.0 * std::f32::consts::SQRT_2 + 2.0;
        let (mx, my) = mark.center();
        let line = Rect::new(mx - length / 2.0, my - 1.0, length, 2.0);
        ui.fill_turned(line, TEXT_DIM, 1.0, std::f32::consts::FRAC_PI_4);
        x -= 26.0 + 10.0;
    }
    if person.muted {
        ui.icon(x - 13.0, cy, icon::MUTED, 22.0, TEXT_DIM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::discord::{Call, Voice};

    fn person(id: &str, name: &str) -> Person {
        Person {
            id: id.into(),
            name: name.into(),
            avatar: None,
            speaking: false,
            muted: false,
            deafened: false,
            me: id == "1",
        }
    }

    fn in_call(people: &[(&str, &str)]) -> Discord {
        let people = people.iter().map(|(id, name)| person(id, name)).collect();
        let call = Call { id: "500".into(), title: "Late night".into(), people };
        let voice = Voice { call: Some(call), mute: false, deaf: false };
        Discord { connected: true, has_tokens: true, user: Some("gabe".into()), voice, ..Discord::default() }
    }

    fn items(discord: &Discord) -> Vec<Item> {
        entries(discord).iter().filter_map(item).collect()
    }

    #[test]
    fn the_tab_says_why_it_cant_show_discord_or_who_is_signed_in_or_the_call() {
        let lost = Discord { has_tokens: true, retrying: true, ..Discord::default() };
        assert_eq!(items(&lost), [Item::Unavailable]);
        assert_eq!(entries(&lost)[0].title, "Discord isn't running");

        let idle = Discord { voice: Voice::default(), ..in_call(&[]) };
        assert_eq!(items(&idle), [Item::Account, Item::Idle]);
        assert_eq!(entries(&idle)[0].title, "gabe");

        let call = in_call(&[("1", "gabe"), ("2", "ana")]);
        let all = [Item::Channel, Item::Person(0), Item::Person(1), Item::Mute, Item::Deafen, Item::Leave];
        assert_eq!(items(&call), all);
        let dividers: Vec<bool> = entries(&call).iter().map(|e| e.divider).collect();
        assert_eq!(dividers, [false, false, false, true, false, false]);
        assert_eq!(first_focus(&entries(&call)), 3, "Mute");
    }

    #[test]
    fn focus_stays_on_its_entry_or_person_and_starts_afresh_into_or_out_of_a_call() {
        let three = in_call(&[("1", "gabe"), ("2", "ana"), ("3", "bo")]);
        let two = in_call(&[("1", "gabe"), ("3", "bo")]);
        let (e3, e2) = (entries(&three), entries(&two));
        // On bo, ana leaving: still bo. On ana, ana leaving: the entry now in her place, bo.
        assert_eq!(focus_after((&e3, &three), (&e2, &two), 3), 2);
        assert_eq!(focus_after((&e3, &three), (&e2, &two), 2), 2);
        // On Deafen, someone joining: still Deafen.
        assert_eq!(focus_after((&e2, &two), (&e3, &three), 5), 6);
        // Out of the call: afresh.
        let idle = Discord { voice: Voice::default(), ..two.clone() };
        assert_eq!(focus_after((&e2, &two), (&entries(&idle), &idle), 5), 0);
    }

    #[test]
    fn the_focus_line_names_the_entry_and_the_call() {
        let mut discord = in_call(&[("1", "gabe"), ("2", "ana")]);
        if let Some(call) = &mut discord.voice.call {
            call.people[1].speaking = true;
            call.people[1].muted = true;
        }
        discord.voice.mute = true;
        let line = line(&entries(&discord), Some(2), &discord);
        assert_eq!(line, " Person ana; in \"Late night\" with gabe, ana (speaking, muted); mute on, deafen off");
    }
}
