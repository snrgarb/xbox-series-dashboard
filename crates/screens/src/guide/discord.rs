//! The Discord tab (docs/spec/discord-tab.md): in the console's Parties & chats slot, there from the first Connect
//! until Disconnect. While it can't show Discord, a line saying why; signed in, who is, then out of a Call that they
//! aren't in a voice channel, or in one its heading, the people in it, the user first, then Mute, Deafen and Leave
//! under a divider; then the Shortcuts, tiles three to a row, and Servers, each under a divider. What A asks is
//! Discord's to carry out (`GuideAction::Discord`), and is drawn from what Discord last said, never from the press.
//! Measured from the Bundle's `OverlayWindow.xaml` (its `GuideEntry` template's Discord kinds) and ported from
//! `ExtensionCore.DiscordCall.cs`, `ExtensionCore.DiscordShortcuts.cs` and `ExtensionCore.DiscordServers.cs`:
//! there is no picture of the tab among the Bundle's reference pictures.
use model::discord::{Ask, BrowseAsk, CallAsk, Discord, Person, Shortcut, NOT_IN_A_CALL, SERVERS};
use ui::{ArtKey, Button, Color, Crop, HAlign, Kind as ArtKind, Rect, TextStyle, Ui, VAlign};

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
    /// The people: Servers.
    pub const SERVERS: char = '\u{E716}';
}

/// Discord's blurple, behind its mark on the account card, and its red, Leave's.
const BLURPLE: Color = Color::rgb(0x5865F2);
const RED: Color = Color::rgb(0xED4245);
/// The disc under a person's avatar while there is none, and the figure on it.
pub(super) const DISC: Color = Color::rgb(0x4A4A4A);
/// A person's avatar is decoded at 96, as the Bundle's converter is asked; a Shortcut's picture at 144.
const AVATAR_DECODED: u32 = 96;
const TILE_DECODED: u32 = 144;
/// A Shortcut's tile, 160 square, in a column 178 wide, 18 over the next row; three to a row.
pub const TILE: f32 = 160.0;
pub const TILE_COLUMN: f32 = 178.0;
pub const TILE_BELOW: f32 = 18.0;
pub const PER_ROW: usize = 3;

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
    /// A Known channel's tile, by its place among the Shortcuts, most recently joined first.
    Shortcut(usize),
    /// The tab's last entry: it opens the Servers Flyout.
    Servers,
}

/// What the Discord tab asks of Discord, by the places of what it names in what Discord last said: the Dashboard
/// makes it an [`Ask`] with [`DiscordAsk::ask`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscordAsk {
    /// Mute, Deafen or Leave, on the tab or in the Audio Flyout.
    Call(CallAsk),
    /// A on a Shortcut: join its channel.
    Join(usize),
    /// Remove shortcut, from a Shortcut's menu.
    RemoveShortcut(usize),
    /// A on Servers: the Servers Flyout.
    Servers,
    /// A on a server in it: its voice channels.
    Server(usize),
    /// A on a voice channel in a server's Flyout: join it.
    Channel(usize),
    /// B in a server's Flyout: back to the servers.
    Back,
    /// B in the Servers Flyout.
    Close,
}

impl DiscordAsk {
    /// The ask, with what it names as Discord said it: None when that is no longer there.
    pub fn ask(self, discord: &Discord) -> Option<Ask> {
        let browse = &discord.browse;
        Some(match self {
            DiscordAsk::Call(ask) => Ask::Call(ask),
            DiscordAsk::Join(at) => Ask::Join(discord.shortcuts.get(at)?.id.clone()),
            DiscordAsk::RemoveShortcut(at) => Ask::RemoveShortcut(discord.shortcuts.get(at)?.id.clone()),
            DiscordAsk::Servers => Ask::Browse(BrowseAsk::Servers),
            DiscordAsk::Server(at) => {
                let (id, name) = (browse.id(at)?.to_string(), browse.name(at)?.to_string());
                Ask::Browse(BrowseAsk::Server { id, name })
            }
            DiscordAsk::Channel(at) => Ask::Join(browse.id(at)?.to_string()),
            DiscordAsk::Back => Ask::Browse(BrowseAsk::Back),
            DiscordAsk::Close => Ask::Browse(BrowseAsk::Close),
        })
    }
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
    // The Shortcuts, the first under a divider across the row; then Servers under one of its own.
    for (at, shortcut) in discord.shortcuts.iter().enumerate() {
        let tile = entry(Item::Shortcut(at), &shortcut.name);
        entries.push(if at == 0 { tile.under_a_divider() } else { tile });
    }
    entries.push(entry(Item::Servers, SERVERS).under_a_divider());
    entries
}

pub(super) fn item(entry: &Entry) -> Option<Item> {
    match entry.kind {
        Kind::Discord(item) => Some(item),
        _ => None,
    }
}

/// The tab's first focus: the first entry A does something on: Mute in a Call (the Bundle's Answer, while a Call
/// rings, is not built), else the first Shortcut, else Servers; else the first entry.
pub fn first_focus(entries: &[Entry]) -> usize {
    let at = |wanted: fn(Item) -> bool| entries.iter().position(|entry| item(entry).is_some_and(wanted));
    at(|item| item == Item::Mute)
        .or_else(|| at(|item| matches!(item, Item::Shortcut(_))))
        .or_else(|| at(|item| item == Item::Servers))
        .unwrap_or(0)
}

/// Where focus goes as the tab's entries are made again from Discord as it is now: into a Call or out of one,
/// afresh; otherwise the same entry, the same person or the same Shortcut, else the entry now at the same place.
pub fn focus_after(before: (&[Entry], &Discord), after: (&[Entry], &Discord), focus: usize) -> usize {
    let in_call = |entries: &[Entry]| entries.iter().any(|entry| item(entry) == Some(Item::Channel));
    if in_call(before.0) != in_call(after.0) || after.0.is_empty() {
        return first_focus(after.0);
    }
    /// Who a Person entry is, or which channel a Shortcut is: what it stands for beyond its place.
    fn id(discord: &Discord, item: Item) -> Option<&str> {
        match item {
            Item::Person(at) => discord.voice.call.as_ref().and_then(|call| call.people.get(at)).map(|p| p.id.as_str()),
            Item::Shortcut(at) => discord.shortcuts.get(at).map(|s| s.id.as_str()),
            _ => None,
        }
    }
    let same = before.0.get(focus).and_then(item).and_then(|focused| {
        let wanted = id(before.1, focused);
        after.0.iter().position(|entry| match (item(entry), focused) {
            (Some(now @ Item::Person(_)), Item::Person(_)) | (Some(now @ Item::Shortcut(_)), Item::Shortcut(_)) => {
                id(after.1, now) == wanted
            }
            (now, _) => now == Some(focused) && wanted.is_none(),
        })
    });
    same.unwrap_or(focus.min(after.0.len() - 1))
}

/// What A does on an entry: Mute, Deafen and Leave, a Shortcut and Servers ask Discord; on anything else, nothing.
pub fn select(item: Item) -> Option<GuideAction> {
    let ask = match item {
        Item::Mute => DiscordAsk::Call(CallAsk::Mute),
        Item::Deafen => DiscordAsk::Call(CallAsk::Deafen),
        Item::Leave => DiscordAsk::Call(CallAsk::Leave),
        Item::Shortcut(at) => DiscordAsk::Join(at),
        Item::Servers => DiscordAsk::Servers,
        Item::Unavailable | Item::Account | Item::Idle | Item::Channel | Item::Person(_) => return None,
    };
    Some(GuideAction::Discord(ask))
}

/// Where a press moves focus among the Shortcuts' tiles, from the entry at `at`; None for a press they leave to the
/// list's own moves. Left and right go along a row, held at its ends. Down goes a row down, else onto the last tile
/// of a shorter row below, else on past the tiles; up goes a row up, else to the entry above them. Up onto the
/// tiles from the entry under them lands on the last row's first. Down onto them from above lands on the first, as
/// the list's own move does.
pub fn grid_move(entries: &[Entry], at: usize, button: Button) -> Option<usize> {
    let first = entries.iter().position(|entry| matches!(item(entry), Some(Item::Shortcut(_))))?;
    let count = entries[first..].iter().take_while(|entry| matches!(item(entry), Some(Item::Shortcut(_)))).count();
    let after = first + count;
    if at == after && button == Button::Up {
        return Some(first + (count - 1) / PER_ROW * PER_ROW);
    }
    if !(first..after).contains(&at) {
        return None;
    }
    let tile = at - first;
    let (row, column) = (tile / PER_ROW, tile % PER_ROW);
    let rows = count.div_ceil(PER_ROW);
    Some(match button {
        Button::Left => first + tile - column.min(1),
        Button::Right => first + (tile + 1).min(count - 1).min(row * PER_ROW + PER_ROW - 1),
        Button::Down if row + 1 < rows => first + (tile + PER_ROW).min(count - 1),
        Button::Down => after.min(entries.len() - 1),
        Button::Up if row > 0 => first + tile - PER_ROW,
        Button::Up => first.saturating_sub(1),
        _ => return None,
    })
}

/// An entry's box height, and the room under it.
pub fn height(item: Item) -> (f32, f32) {
    let height = match item {
        Item::Account => 105.0,
        Item::Channel => 60.0,
        Item::Person(_) => 72.0,
        Item::Shortcut(_) => return (TILE, TILE_BELOW),
        Item::Unavailable | Item::Idle | Item::Mute | Item::Deafen | Item::Leave | Item::Servers => 75.0,
    };
    (height, 3.0)
}

/// What `--log-focus`'s Guide line says of the tab after its focus: the entry focus is on, then what Discord says,
/// `Mute; in "General" with E2E Tester, Ana (speaking), Bo (muted, deafened); mute off, deafen off`, or
/// `Shortcut "Late night" (Stand-in Server); not in a call; mute off, deafen off`.
pub fn line(entries: &[Entry], focus: Option<usize>, discord: &Discord) -> String {
    let focused = match focus.and_then(|at| entries.get(at)).and_then(item) {
        Some(Item::Person(at)) => {
            let name = discord.voice.call.as_ref().and_then(|call| call.people.get(at)).map(|p| p.name.as_str());
            format!(" Person {}", name.unwrap_or_default())
        }
        Some(Item::Shortcut(at)) => match discord.shortcuts.get(at) {
            Some(Shortcut { name, server: Some(server), .. }) => format!(" Shortcut {name:?} ({server})"),
            Some(Shortcut { name, .. }) => format!(" Shortcut {name:?}"),
            None => " Shortcut".into(),
        },
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
/// a person's marks or a switch at its right; a Shortcut's tile its picture over its name.
pub fn draw(ui: &mut Ui, item: Item, entry: &Entry, rect: Rect, focused: bool, discord: &Discord) {
    if focused {
        ui.fill(rect, Color::argb(0x26FFFFFF), 4.0);
    }
    if let Item::Shortcut(at) = item {
        if focused {
            focus_ring(ui, rect);
        }
        if let Some(shortcut) = discord.shortcuts.get(at) {
            tile(ui, rect, shortcut);
        }
        return;
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
        Item::Servers => glyph_at(ui, icon::SERVERS, 26.0, Color::WHITE),
        Item::Shortcut(_) => {}
    }
    if focused {
        focus_ring(ui, rect);
    }
    let (x, right, style, lines) = text;
    title(ui, Rect::new(x, rect.y, right - x, rect.h), &entry.title, &style, lines);
}

/// A Shortcut's tile: in a column 10 in from its sides, in the tile's middle, its picture 72 across, and 10 under
/// it the channel's name on one line, then its server's in grey. A DM's picture is a disc, Discord's mark on grey
/// for none; a voice channel's a rounded square, the speaker on grey for none.
fn tile(ui: &mut Ui, rect: Rect, shortcut: &Shortcut) {
    let width = rect.w - 20.0;
    let line = ui.measure(&BODY, "Ag").height;
    let lines = if shortcut.server.is_some() { 2.0 } else { 1.0 };
    let top = rect.center().1 - (72.0 + 10.0 + line * lines) / 2.0;
    let picture = Rect::new(rect.center().0 - 36.0, top, 72.0, 72.0);
    picture_or(ui, picture, shortcut.picture.as_ref(), shortcut.dm, TILE_DECODED);
    let name = Rect::new(rect.x + 10.0, picture.bottom() + 10.0, width, line);
    ui.paragraph(name, &shortcut.name, &BODY, HAlign::Center, VAlign::Top, 1);
    if let Some(server) = &shortcut.server {
        let below = Rect::new(name.x, name.bottom(), width, line);
        ui.paragraph(below, server, &BODY.color(TEXT_DIM), HAlign::Center, VAlign::Top, 1);
    }
}

/// A picture in `rect`: a disc when `round`, else a square rounded by 16 (a server's icon, at 60 or 72); grey with
/// Discord's mark (round) or the speaker (square) under it while there is none.
pub(super) fn picture_or(ui: &mut Ui, rect: Rect, picture: Option<&std::path::PathBuf>, round: bool, decoded: u32) {
    let radius = if round { rect.w / 2.0 } else { 16.0 };
    let (cx, cy) = rect.center();
    ui.fill(rect, DISC, radius);
    if round {
        let mark = rect.w * 34.0 / 72.0;
        marks::discord_mark(ui, Rect::new(cx - mark / 2.0, cy - mark / 2.0, mark, mark), TEXT_DIM);
    } else {
        ui.icon(cx, cy, glyph::VOLUME, rect.w * 28.0 / 72.0, TEXT_DIM);
    }
    if let Some(path) = picture {
        let key = ArtKey { path: path.clone(), kind: ArtKind::Thumb(decoded) };
        ui.art(rect, &key, Crop::Center, 1.0, radius);
    }
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

    fn shortcut(id: &str, name: &str) -> Shortcut {
        Shortcut { id: id.into(), name: name.into(), server: Some("The Lads".into()), picture: None, dm: false }
    }

    fn with_shortcuts(discord: Discord, count: usize) -> Discord {
        let shortcuts = (0..count).map(|n| shortcut(&format!("{}", 600 + n), &format!("Ch{n}"))).collect();
        Discord { shortcuts, ..discord }
    }

    fn idle() -> Discord {
        Discord { voice: Voice::default(), ..in_call(&[]) }
    }

    fn items(discord: &Discord) -> Vec<Item> {
        entries(discord).iter().filter_map(item).collect()
    }

    #[test]
    fn the_tab_says_why_it_cant_show_discord_or_who_is_signed_in_or_the_call_then_servers() {
        let lost = Discord { has_tokens: true, retrying: true, ..Discord::default() };
        assert_eq!(items(&lost), [Item::Unavailable]);
        assert_eq!(entries(&lost)[0].title, "Discord isn't running");

        assert_eq!(items(&idle()), [Item::Account, Item::Idle, Item::Servers]);
        assert_eq!(entries(&idle())[0].title, "gabe");
        assert_eq!(first_focus(&entries(&idle())), 2, "Servers");

        let call = in_call(&[("1", "gabe"), ("2", "ana")]);
        let all =
            [Item::Channel, Item::Person(0), Item::Person(1), Item::Mute, Item::Deafen, Item::Leave, Item::Servers];
        assert_eq!(items(&call), all);
        let dividers: Vec<bool> = entries(&call).iter().map(|e| e.divider).collect();
        assert_eq!(dividers, [false, false, false, true, false, false, true]);
        assert_eq!(first_focus(&entries(&call)), 3, "Mute");
    }

    #[test]
    fn the_shortcuts_come_before_servers_the_first_under_a_divider_and_take_the_first_focus_out_of_a_call() {
        let discord = with_shortcuts(idle(), 2);
        let entries = entries(&discord);
        assert_eq!(items(&discord), [Item::Account, Item::Idle, Item::Shortcut(0), Item::Shortcut(1), Item::Servers]);
        let dividers: Vec<bool> = entries.iter().map(|e| e.divider).collect();
        assert_eq!(dividers, [false, false, true, false, true]);
        assert_eq!(first_focus(&entries), 2);
        assert_eq!(entries[3].title, "Ch1");
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
        // Out of the call: afresh, on Servers.
        let idle = Discord { voice: Voice::default(), ..two.clone() };
        assert_eq!(focus_after((&e2, &two), (&entries(&idle), &idle), 5), 2);
    }

    #[test]
    fn focus_follows_its_shortcut_as_another_is_removed_and_else_keeps_its_place() {
        let before = with_shortcuts(idle(), 3);
        let mut after = before.clone();
        after.shortcuts.remove(0);
        let (eb, ea) = (entries(&before), entries(&after));
        assert_eq!(focus_after((&eb, &before), (&ea, &after), 4), 3, "Ch2 moved up a place");
        assert_eq!(focus_after((&eb, &before), (&ea, &after), 2), 2, "Ch0 gone: Ch1 now in its place");
    }

    #[test]
    fn the_tiles_move_as_a_grid_three_to_a_row() {
        // Account, Idle, five tiles in rows of three and two, Servers.
        let discord = with_shortcuts(idle(), 5);
        let e = entries(&discord);
        let go = |at: usize, button: Button| grid_move(&e, at, button);
        assert_eq!(go(2, Button::Left), Some(2), "held at the row's start");
        assert_eq!(go(2, Button::Right), Some(3));
        assert_eq!(go(4, Button::Right), Some(4), "held at the row's end");
        assert_eq!(go(6, Button::Right), Some(6), "held at the last tile");
        assert_eq!(go(2, Button::Down), Some(5));
        assert_eq!(go(4, Button::Down), Some(6), "onto the last tile of the shorter row below");
        assert_eq!(go(5, Button::Down), Some(7), "on past the tiles to Servers");
        assert_eq!(go(5, Button::Up), Some(2));
        assert_eq!(go(3, Button::Up), Some(1), "to the entry above the tiles");
        assert_eq!(go(7, Button::Up), Some(5), "up from Servers: the last row's first");
        assert_eq!(go(1, Button::Down), None, "down onto the tiles is the list's own move: the first");
        assert_eq!(go(0, Button::Left), None);
    }

    #[test]
    fn a_and_the_asks_name_what_discord_said() {
        let mut discord = with_shortcuts(idle(), 2);
        assert_eq!(select(Item::Shortcut(1)), Some(GuideAction::Discord(DiscordAsk::Join(1))));
        assert_eq!(DiscordAsk::Join(1).ask(&discord), Some(Ask::Join("601".into())));
        assert_eq!(DiscordAsk::RemoveShortcut(0).ask(&discord), Some(Ask::RemoveShortcut("600".into())));
        assert_eq!(DiscordAsk::Join(5).ask(&discord), None);
        discord.browse = model::discord::Browse::Servers {
            loading: false,
            servers: vec![model::discord::Server { id: "9".into(), name: "The Lads".into(), icon: None }],
        };
        let server = BrowseAsk::Server { id: "9".into(), name: "The Lads".into() };
        assert_eq!(DiscordAsk::Server(0).ask(&discord), Some(Ask::Browse(server)));
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

        let discord = with_shortcuts(idle(), 1);
        let line = super::line(&entries(&discord), Some(2), &discord);
        assert_eq!(line, " Shortcut \"Ch0\" (The Lads); not in a call; mute off, deafen off");
    }
}
