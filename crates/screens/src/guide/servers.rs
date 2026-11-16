//! The Discord tab's Flyout (docs/spec/discord-tab.md, "Servers"): A on Servers opens it on the user's servers, A on
//! a server shows its voice channels in its place, each with who is in it, and A on a channel joins it and closes
//! the Flyout. B goes back to the servers, focus on the server it came from, then closes it. What it lists is
//! Discord's, as the `discord` crate's browser last had it (`Discord::browse`): a spinner while Discord is asked,
//! for a second at least. Measured from the Bundle's `OverlayWindow.xaml` and ported from
//! `ExtensionCore.DiscordServers.cs`.
use model::discord::{Browse, Discord, NO_SERVERS, NO_VOICE_CHANNELS};
use ui::{ArtKey, Button, Color, Crop, HAlign, Kind as ArtKind, Press, Rect, Ui, VAlign};

use super::discord::{DiscordAsk, DISC};
use super::flyout::{Outcome, State, FIRST_TOP, FLYOUT, FOCUSED, FOOT, INSET};
use super::{entries, focus_ring, marks, Around, GuideAction};
use crate::theme::{glyph, BODY, TEXT_DIM};

/// A server's row, 96 high; its icon 60 across, 18 in; its name 96 in.
const SERVER_ROW: f32 = 96.0;
const ICON: f32 = 60.0;
/// A server's icon is decoded at 120.
const ICON_DECODED: u32 = 120;
/// Between rows.
const BELOW: f32 = 3.0;
/// Who is in a channel: discs 34 across, each over the last by 8.
const OCCUPANT: f32 = 34.0;
const OVERLAP: f32 = 8.0;
const OCCUPANT_EDGE: Color = Color::rgb(0x1F1F1F);
/// The spinner while Discord is asked.
const SPINNER: f32 = 102.0;
/// Who is in a channel: each avatar decoded at 64.
const PERSON_DECODED: u32 = 64;

/// What Discord has done since: false when the Flyout is to close, as Discord closes it (a channel joined), goes,
/// or the tab can't show it. Focus lands on the server B went back from, else keeps its place.
pub(super) fn follow(state: &mut State, discord: &Discord) -> bool {
    if discord.tab_line().is_some() {
        return false;
    }
    let browse = &discord.browse;
    if browse.is_open() {
        state.seen = true;
    } else if state.seen {
        return false;
    }
    // Focus moves only once Discord shows where A or B went: until then it stays on the row pressed, and says so.
    match (&state.into, &state.back_to, browse) {
        (Some(into), _, Browse::Server { id, .. }) if id == into => {
            (state.item, state.scroll, state.into) = (0, 0.0, None);
        }
        (_, Some(back_to), Browse::Servers { loading: false, servers }) => {
            state.item = servers.iter().position(|server| server.id == *back_to).unwrap_or(0);
            (state.scroll, state.back_to) = (0.0, None);
        }
        _ => {}
    }
    state.item = state.item.min(browse.len().saturating_sub(1));
    true
}

/// A press in the Flyout. Up and down move among its rows; A on a server opens it, on a channel joins it (the
/// Flyout closing, focus on Servers); B goes back from a server, and closes the servers' Flyout.
pub(super) fn press(state: &mut State, press: Press, around: Around, discord: &Discord) -> Outcome {
    let browse = &discord.browse;
    let count = state.count(around, discord);
    let ask = |ask| Outcome::Chose(GuideAction::Discord(ask));
    match (press.button, browse) {
        (Button::Up, _) => state.item = state.item.saturating_sub(1),
        (Button::Down, _) => state.item = (state.item + 1).min(count.saturating_sub(1)),
        (Button::A, Browse::Servers { servers, .. }) if state.item < count => {
            (state.into, state.back_to) = (servers.get(state.item).map(|server| server.id.clone()), None);
            return ask(DiscordAsk::Server(state.item));
        }
        (Button::A, Browse::Server { .. }) if state.item < count => {
            return Outcome::CloseChose(GuideAction::Discord(DiscordAsk::Channel(state.item)));
        }
        (Button::B, Browse::Server { id, .. }) => {
            (state.back_to, state.into) = (Some(id.clone()), None);
            return ask(DiscordAsk::Back);
        }
        (Button::B, _) => return Outcome::CloseChose(GuideAction::Discord(DiscordAsk::Close)),
        _ => {}
    }
    Outcome::Nothing
}

/// What `--log-focus`'s Guide line says of the open Flyout after its focus: its title, then what it shows,
/// `"Servers" loading`, `"Servers" "Stand-in Server" of 3`, `"Stand-in Server" "Late night" of 2: Empty`.
pub fn line(state: &State, browse: &Browse) -> String {
    let title = browse.title();
    if browse.loading() {
        return format!("{title:?} loading");
    }
    let Some(name) = browse.name(state.item) else {
        let none = if matches!(browse, Browse::Servers { .. }) { NO_SERVERS } else { NO_VOICE_CHANNELS };
        return format!("{title:?} {none:?}");
    };
    let mut line = format!("{title:?} {name:?} of {}", browse.len());
    if let Browse::Server { channels, .. } = browse {
        match channels.get(state.item).and_then(|channel| channel.occupants.as_ref()) {
            Some(occupants) if occupants.is_empty() => line.push_str(": Empty"),
            Some(occupants) => line.push_str(&format!(": {} in it", occupants.len())),
            None => line.push_str(": not known yet"),
        }
    }
    line
}

/// Draws the Flyout's rows under its title: the spinner while Discord is asked, that there are none once it has
/// answered so, else the servers or a server's voice channels, the focused one ringed and scrolled into view.
pub(super) fn draw(state: &mut State, ui: &mut Ui, browse: &Browse) {
    if browse.loading() {
        let (cx, _) = FLYOUT.center();
        crate::spinner::spinner(ui, cx, (FIRST_TOP + FOOT) / 2.0, SPINNER);
        return;
    }
    if browse.is_empty() {
        let none = if matches!(browse, Browse::Servers { .. }) { NO_SERVERS } else { NO_VOICE_CHANNELS };
        ui.label(FLYOUT.x + 28.0, FIRST_TOP, none, &BODY.color(TEXT_DIM));
        return;
    }
    let line = ui.measure(&BODY, "Ag").height;
    let width = FLYOUT.w - 2.0 * INSET;
    let mut y = FIRST_TOP;
    let heights: Vec<f32> = match browse {
        Browse::Server { channels, .. } => channels
            .iter()
            .map(|channel| match &channel.occupants {
                Some(occupants) if occupants.is_empty() => 16.0 + line + 4.0 + line + 16.0,
                Some(_) => 16.0 + line + 8.0 + OCCUPANT + 16.0,
                None => 16.0 + line + 16.0,
            })
            .collect(),
        _ => vec![SERVER_ROW; browse.len()],
    };
    let placed: Vec<Rect> = heights
        .iter()
        .map(|&height| {
            let rect = Rect::new(FLYOUT.x + INSET, y, width, height);
            y += height + BELOW;
            rect
        })
        .collect();
    let focused = state.item.min(placed.len() - 1);
    let rect = placed[focused];
    state.scroll = entries::scroll_to(state.scroll, focused == 0, (rect.y, rect.bottom() + BELOW), (FIRST_TOP, FOOT));
    let scroll = state.scroll;
    ui.with_clip(State::viewport(), |ui| {
        ui.with_offset(0.0, -scroll, |ui| {
            let unfocused = (0..placed.len()).filter(|&at| at != focused);
            for at in unfocused.chain([focused]) {
                let rect = placed[at];
                if at == focused {
                    ui.fill(rect, FOCUSED, 4.0);
                    focus_ring(ui, rect);
                }
                match browse {
                    Browse::Servers { servers, .. } => {
                        let server = &servers[at];
                        server_icon(ui, Rect::new(rect.x + 18.0, rect.center().1 - ICON / 2.0, ICON, ICON), server);
                        let text = Rect::new(rect.x + 96.0, rect.y, rect.w - 96.0 - 16.0, rect.h);
                        ui.paragraph(text, &server.name, &BODY, HAlign::Left, VAlign::Center, 1);
                    }
                    Browse::Server { channels, .. } => voice_channel(ui, rect, &channels[at], line),
                    Browse::Closed => {}
                }
            }
        });
    });
}

/// A server's icon on a square rounded by 16; on grey under Discord's mark, 30 across, while there is none.
fn server_icon(ui: &mut Ui, rect: Rect, server: &model::discord::Server) {
    let (cx, cy) = rect.center();
    ui.fill(rect, DISC, 16.0);
    marks::discord_mark(ui, Rect::new(cx - 15.0, cy - 15.0, 30.0, 30.0), TEXT_DIM);
    if let Some(path) = &server.icon {
        let key = ArtKey { path: path.clone(), kind: ArtKind::Thumb(ICON_DECODED) };
        ui.art(rect, &key, Crop::Center, 1.0, 16.0);
    }
}

/// A voice channel's row: the speaker at its top left, its name, and under it who is in it, their avatars on discs
/// overlapping, or "Empty"; nothing there until Discord has said.
fn voice_channel(ui: &mut Ui, rect: Rect, channel: &model::discord::VoiceChannel, line: f32) {
    ui.icon(rect.x + 36.0, rect.y + 14.0 + 13.0, glyph::VOLUME, 26.0, Color::WHITE);
    let (x, width) = (rect.x + 69.0, rect.w - 69.0 - 16.0);
    let top = rect.y + 16.0;
    ui.paragraph(Rect::new(x, top, width, line), &channel.name, &BODY, HAlign::Left, VAlign::Top, 1);
    match &channel.occupants {
        Some(occupants) if occupants.is_empty() => {
            let empty = Rect::new(x, top + line + 4.0, width, line);
            ui.paragraph(empty, model::discord::EMPTY, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 1);
        }
        Some(occupants) => {
            let top = top + line + 8.0;
            // The first leftmost and under the next, as WPF stacks them.
            for (at, avatar) in occupants.iter().enumerate() {
                let disc = Rect::new(x + at as f32 * (OCCUPANT - OVERLAP), top, OCCUPANT, OCCUPANT);
                if disc.right() > rect.right() - 16.0 {
                    break;
                }
                ui.fill(disc, DISC, OCCUPANT / 2.0);
                if let Some(path) = avatar {
                    let key = ArtKey { path: path.clone(), kind: ArtKind::Thumb(PERSON_DECODED) };
                    let inside = disc.grow(-2.0);
                    ui.art(inside, &key, Crop::Center, 1.0, inside.w / 2.0);
                }
                ui.ring(disc, OCCUPANT_EDGE, OCCUPANT / 2.0, 2.0);
            }
        }
        None => {}
    }
}
