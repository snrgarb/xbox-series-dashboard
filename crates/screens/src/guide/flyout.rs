//! The Flyouts: a second panel a Utility opens over the Guide's own (docs/spec/guide.md, section 5). Notifications
//! lists what has been posted, newest first; Audio has the system's sound: the output and input devices, each a
//! dropdown with its list, the master volume and mute, and last Now Playing: the card of the media Windows' own
//! flyout shows, with Previous, Play/Pause and Next under it (docs/spec/now-playing.md).
//!
//! This file is which Flyout is open and where focus is in it, and how it is drawn. What it shows is the
//! Dashboard's (`Around`), and what a press changes of the system's sound is asked of the Dashboard
//! (`GuideAction`), which asks the shell.
use std::path::PathBuf;

use model::discord::{CallAsk, Discord};
use model::{Audio, Flow, MediaCommand};
use ui::{ArtKey, Button, Color, Crop, Ease, HAlign, Kind, Press, Rect, Ui, VAlign};

use super::discord::DiscordAsk;
use super::panel::PANEL;
use super::{entries, focus_ring, Around, GuideAction, HEADING};
use crate::now_playing::Card;
use crate::theme::{glyph, ACCENT_LIGHT, BODY, TEXT_DIM};
use crate::Notification;

/// The Flyout: x 155..691, from y 33 to the screen's foot. Drawn a corner's worth past the foot, so that only its
/// top corners are round.
pub(super) const FLYOUT: Rect = Rect::new(155.0, 33.0, 536.0, 1047.0 + RADIUS);
const RADIUS: f32 = 12.0;
const BACK: Color = Color::rgb(0x1F1F1F);
/// The dim over the Guide's own panel while a Flyout is over it.
const PANEL_DIM: Color = Color::argb(0x73000000);
/// It comes in from this far to the left, fading in, over this long.
const TRAVEL: f32 = -48.0;
const OPEN_SECONDS: f32 = 0.12;
/// The title's top, and the entries' under it: the title's line and 24.
const TITLE_TOP: f32 = FLYOUT.y + 36.0;
pub(super) const FIRST_TOP: f32 = TITLE_TOP + 39.6 + 24.0;
/// The entries end 24 above the screen's foot.
pub(super) const FOOT: f32 = 1080.0 - 24.0;
/// An entry's box is 9 in from the Flyout's sides.
pub(super) const INSET: f32 = 9.0;
/// The volume moves by this much a press.
const VOLUME_STEP: i32 = 2;

pub(super) const FOCUSED: Color = Color::argb(0x26FFFFFF);
const CARD: Color = Color::argb(0x14FFFFFF);
const DIVIDER: Color = Color::argb(0x33FFFFFF);

mod icon {
    pub const OUTPUT: char = '\u{E7F5}';
    pub const INPUT: char = '\u{E720}';
    pub const MUTE: char = '\u{E74F}';
    pub const MUSIC: char = '\u{E8D6}';
    pub const CHEVRON_DOWN: char = '\u{E70D}';
    pub const CHEVRON_UP: char = '\u{E70E}';
    pub const TICK: char = '\u{E73E}';
    pub const PREVIOUS: char = '\u{E892}';
    pub const PLAY: char = '\u{E768}';
    pub const PAUSE: char = '\u{E769}';
    pub const NEXT: char = '\u{E893}';
    pub const HEADSET: char = '\u{E7F6}';
}

/// Which Flyout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flyout {
    Notifications,
    Audio,
    /// The Discord tab's Servers, then a server's voice channels (`super::servers`).
    Discord,
}

/// Where focus is in the open Flyout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyoutFocus {
    /// It has nothing to focus: Notifications with none.
    Nothing,
    /// An entry, from the top.
    Item(usize),
    /// A device in a dropdown's open list, from the top.
    Device(Flow, usize),
    /// One of the Now Playing card's controls, under it.
    Control(MediaControl),
}

/// The Now Playing card's controls, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaControl {
    Previous,
    PlayPause,
    Next,
}

impl MediaControl {
    pub const ALL: [MediaControl; 3] = [MediaControl::Previous, MediaControl::PlayPause, MediaControl::Next];

    fn command(self) -> MediaCommand {
        match self {
            MediaControl::Previous => MediaCommand::Previous,
            MediaControl::PlayPause => MediaCommand::PlayPause,
            MediaControl::Next => MediaCommand::Next,
        }
    }

    /// Whether the app has it on now. One it has off is dimmed, and A does nothing on it.
    fn enabled(self, session: &model::MediaSession) -> bool {
        match self {
            MediaControl::Previous => session.can_previous,
            MediaControl::PlayPause => session.can_play_pause,
            MediaControl::Next => session.can_next,
        }
    }

    /// Its glyph: Play/Pause is play while paused and pause while playing.
    fn glyph(self, playing: bool) -> char {
        match self {
            MediaControl::Previous => icon::PREVIOUS,
            MediaControl::PlayPause if playing => icon::PAUSE,
            MediaControl::PlayPause => icon::PLAY,
            MediaControl::Next => icon::NEXT,
        }
    }

    /// The one `by` along, held at either end.
    fn step(self, by: isize) -> Self {
        let at = Self::ALL.iter().position(|&control| control == self).unwrap_or(1) as isize;
        Self::ALL[(at + by).clamp(0, Self::ALL.len() as isize - 1) as usize]
    }
}

/// Audio's entries, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioItem {
    Output,
    Input,
    Volume,
    Mute,
    /// In a Discord Call, after Mute: the user's Mute and Deafen in Discord, as switches.
    DiscordMute,
    DiscordDeafen,
    /// Now Playing, under a divider: the card of the media the Now Playing source has playing, or with none "No
    /// music app is running". Either takes focus, and A on it does nothing; the card's controls are under it.
    NowPlaying,
}

impl AudioItem {
    pub const ALL: [AudioItem; 5] =
        [AudioItem::Output, AudioItem::Input, AudioItem::Volume, AudioItem::Mute, AudioItem::NowPlaying];

    /// Audio's entries, top to bottom: in a Discord Call, Mute and Deafen in Discord after Mute.
    pub fn of(discord: &Discord) -> Vec<AudioItem> {
        let mut items = AudioItem::ALL.to_vec();
        if discord.voice.call.is_some() {
            items.splice(4..4, [AudioItem::DiscordMute, AudioItem::DiscordDeafen]);
        }
        items
    }

    fn flow(self) -> Option<Flow> {
        match self {
            AudioItem::Output => Some(Flow::Output),
            AudioItem::Input => Some(Flow::Input),
            _ => None,
        }
    }

    /// The box's height, and the room under it. Now Playing's card is its 150, and under it its row of controls,
    /// 52 with 16 below.
    fn height(self, card: bool) -> (f32, f32) {
        match self {
            AudioItem::Output | AudioItem::Input => (128.0, 3.0),
            AudioItem::Volume => (96.0, 3.0),
            AudioItem::NowPlaying if card => (CARD_HEIGHT + CONTROLS_HEIGHT, 3.0),
            AudioItem::Mute | AudioItem::DiscordMute | AudioItem::DiscordDeafen | AudioItem::NowPlaying => (75.0, 3.0),
        }
    }
}

/// What a press in a Flyout came to.
pub(super) enum Outcome {
    Nothing,
    /// B on the Flyout itself: it closes, focus back on its Utility.
    Close,
    /// Something for the Dashboard to carry out, or to sound as a choice.
    Chose(GuideAction),
    /// The Flyout closes, and the Dashboard carries something out: the Discord Flyout's B, or a channel joined.
    CloseChose(GuideAction),
}

/// The open Flyout.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct State {
    pub which: Flyout,
    pub(super) item: usize,
    /// Audio's entries as they were last made: focus follows its entry as Mute and Deafen in Discord come and go.
    audio: Vec<AudioItem>,
    /// The Discord Flyout: whether Discord has had it open (it closes as Discord closes it), and the server B went
    /// back from, which focus lands on in the servers.
    pub(super) seen: bool,
    pub(super) back_to: Option<String>,
    /// The dropdown whose list is open, and the device focus is on in it.
    list: Option<(Flow, usize)>,
    /// The Now Playing card's control focus is on, under the card.
    control: Option<MediaControl>,
    /// The album art the card last drew: it stays up while the next track's is decoded, so a track change swaps it
    /// once.
    art: Option<PathBuf>,
    pub(super) scroll: f32,
    /// Which opening this is, of all the Guide's: each comes in once.
    pub(super) opening: u32,
}

impl State {
    /// Opens with focus on the first entry.
    pub fn new(which: Flyout, opening: u32, discord: &Discord) -> Self {
        let audio = AudioItem::of(discord);
        let (list, control, art, back_to) = (None, None, None, None);
        Self { which, item: 0, audio, seen: false, back_to, list, control, art, scroll: 0.0, opening }
    }

    pub(super) fn count(&self, around: Around, discord: &Discord) -> usize {
        match self.which {
            Flyout::Notifications => around.notifications.len(),
            Flyout::Audio => self.audio.len(),
            Flyout::Discord if discord.browse.loading() => 0,
            Flyout::Discord => discord.browse.len(),
        }
    }

    pub fn focus(&self, around: Around, discord: &Discord) -> FlyoutFocus {
        match self.list {
            Some((flow, at)) => FlyoutFocus::Device(flow, at),
            None if self.count(around, discord) == 0 => FlyoutFocus::Nothing,
            None => self.control.map_or(FlyoutFocus::Item(self.item), FlyoutFocus::Control),
        }
    }

    /// What it shows changed under it: focus stays at the same place, or the last there is now; on Audio, on its
    /// entry, Mute for Mute or Deafen in Discord gone with the Call. An open list follows its devices, and closes
    /// when there are none. False when the Flyout is to close: the Discord Flyout, as Discord closes it or goes.
    pub fn follow(&mut self, around: Around, discord: &Discord) -> bool {
        if self.which == Flyout::Discord {
            return super::servers::follow(self, discord);
        }
        let audio = AudioItem::of(discord);
        if self.which == Flyout::Audio && audio != self.audio {
            let focused = self.audio.get(self.item).copied().unwrap_or(AudioItem::Output);
            let mute = audio.iter().position(|&item| item == AudioItem::Mute).unwrap_or(0);
            self.item = audio.iter().position(|&item| item == focused).unwrap_or(mute);
            self.audio = audio;
        }
        self.item = self.item.min(self.count(around, discord).saturating_sub(1));
        if let Some((flow, at)) = self.list {
            let devices = around.audio.devices(flow).0.len();
            self.list = (devices > 0).then(|| (flow, at.min(devices - 1)));
        }
        // The card gone, focus on one of its controls is on "No music app is running" in its place; a card for
        // another session keeps it on the same control.
        if around.now_playing.is_none() {
            self.control = None;
        }
        true
    }

    pub fn press(&mut self, press: Press, around: Around, discord: &Discord) -> Outcome {
        if self.which == Flyout::Discord {
            return super::servers::press(self, press, around, discord);
        }
        if let Some((flow, at)) = self.list {
            let last = around.audio.devices(flow).0.len().saturating_sub(1);
            match press.button {
                Button::Up => self.list = Some((flow, at.saturating_sub(1))),
                Button::Down => self.list = Some((flow, (at + 1).min(last))),
                // That device is Windows' default from now; the list closes onto its dropdown.
                Button::A => {
                    self.list = None;
                    return Outcome::Chose(GuideAction::Device(flow, at));
                }
                Button::B => self.list = None,
                _ => {}
            }
            return Outcome::Nothing;
        }
        if let (Some(control), Some(card)) = (self.control, around.now_playing) {
            // Along the card's row and back up onto the card; down from it stays, at the Flyout's end.
            match press.button {
                Button::Up => self.control = None,
                Button::Left => self.control = Some(control.step(-1)),
                Button::Right => self.control = Some(control.step(1)),
                Button::A if control.enabled(card.session) => {
                    return Outcome::Chose(GuideAction::Media(control.command()));
                }
                Button::B => return Outcome::Close,
                _ => {}
            }
            return Outcome::Nothing;
        }
        let last = self.count(around, discord).saturating_sub(1);
        let audio = (self.which == Flyout::Audio).then(|| self.audio[self.item]);
        let call = |ask| Outcome::Chose(GuideAction::Discord(DiscordAsk::Call(ask)));
        match (press.button, audio) {
            // Asked of Discord, and shown done once Discord says so.
            (Button::A, Some(AudioItem::DiscordMute)) => return call(CallAsk::Mute),
            (Button::A, Some(AudioItem::DiscordDeafen)) => return call(CallAsk::Deafen),
            // Down from the card is Play/Pause, under it.
            (Button::Down, Some(AudioItem::NowPlaying)) if around.now_playing.is_some() => {
                self.control = Some(MediaControl::PlayPause);
            }
            (Button::Up, _) => self.item = self.item.saturating_sub(1),
            (Button::Down, _) => self.item = (self.item + 1).min(last),
            (Button::B, _) => return Outcome::Close,
            // With no output device there is nothing to turn up or to mute.
            (Button::Left | Button::Right | Button::A, Some(AudioItem::Volume | AudioItem::Mute))
                if around.audio.output.is_none() => {}
            (Button::Left | Button::Right, Some(AudioItem::Volume)) => {
                let by = if press.button == Button::Left { -VOLUME_STEP } else { VOLUME_STEP };
                let level = (around.audio.volume as i32 + by).clamp(0, 100) as u8;
                if level != around.audio.volume {
                    return Outcome::Chose(GuideAction::Volume(level));
                }
            }
            (Button::A, Some(AudioItem::Mute)) => return Outcome::Chose(GuideAction::Mute(!around.audio.muted)),
            // A dropdown opens its list on the default device. With no devices it says "None" and does nothing.
            (Button::A, Some(item)) => {
                if let Some(flow) = item.flow() {
                    let (devices, default) = around.audio.devices(flow);
                    if !devices.is_empty() {
                        self.list = Some((flow, default.unwrap_or(0).min(devices.len() - 1)));
                        return Outcome::Chose(GuideAction::Asked);
                    }
                }
            }
            _ => {}
        }
        Outcome::Nothing
    }

    /// Draws the Flyout over the Guide's panel, which is dimmed under it.
    pub fn draw(&mut self, ui: &mut Ui, around: Around, discord: &Discord) {
        // Over everything of the panel, its text too.
        ui.layer();
        ui.fill(PANEL, PANEL_DIM, RADIUS);
        let slide = ui.slide_from(("flyout", self.opening), TRAVEL, 0.0, OPEN_SECONDS, Ease::CubicOut);
        let alpha = ui.tween_from(("flyout fade", self.opening), 0.0, 1.0, OPEN_SECONDS, Ease::Linear);
        ui.with_offset(slide, 0.0, |ui| {
            ui.with_alpha(alpha, |ui| {
                ui.shadow(FLYOUT, Color::BLACK.fade(0.7), RADIUS, 24.0);
                ui.fill(FLYOUT, BACK, RADIUS);
                let title = match self.which {
                    Flyout::Notifications => "Notifications",
                    Flyout::Audio => "Audio",
                    Flyout::Discord => discord.browse.title(),
                };
                let line = Rect::new(FLYOUT.x + 28.0, TITLE_TOP, FLYOUT.w - 56.0, 44.0);
                ui.paragraph(line, title, &HEADING, HAlign::Left, VAlign::Top, 1);
                match self.which {
                    Flyout::Notifications => self.notifications(ui, around.notifications),
                    Flyout::Audio => self.audio(ui, around.audio, around.now_playing, discord),
                    Flyout::Discord => super::servers::draw(self, ui, &discord.browse),
                }
            });
        });
    }

    /// The part of the Flyout the entries show in, wide enough for a focused entry's glow.
    pub(super) fn viewport() -> Rect {
        Rect::new(FLYOUT.x - 24.0, FIRST_TOP - 24.0, FLYOUT.w + 48.0, FOOT - FIRST_TOP + 24.0)
    }

    /// Every notification, newest first, each a card as tall as its text; or that there are none.
    fn notifications(&mut self, ui: &mut Ui, notifications: &[Notification]) {
        if notifications.is_empty() {
            ui.label(FLYOUT.x + 28.0, FIRST_TOP, "No new notifications", &BODY.color(TEXT_DIM));
            return;
        }
        // The text is 69 in and 24 short of the box's right, 18 from its top and foot, three lines at most.
        let width = FLYOUT.w - 2.0 * INSET - 69.0 - 24.0;
        let mut y = FIRST_TOP;
        let mut placed = Vec::with_capacity(notifications.len());
        for notification in notifications.iter().rev() {
            let text = ui.measure_wrapped(&BODY, &notification.text, width, 3);
            let height = (text.height + 36.0).max(87.0);
            placed.push((Rect::new(FLYOUT.x + INSET, y, FLYOUT.w - 2.0 * INSET, height), notification));
            y += height + 9.0;
        }
        let focused = self.item.min(placed.len() - 1);
        let rect = placed[focused].0;
        self.scroll = entries::scroll_to(self.scroll, focused == 0, (rect.y, rect.bottom() + 9.0), (FIRST_TOP, FOOT));
        let scroll = self.scroll;
        ui.with_clip(Self::viewport(), |ui| {
            ui.with_offset(0.0, -scroll, |ui| {
                let unfocused = (0..placed.len()).filter(|&at| at != focused);
                for at in unfocused.chain([focused]) {
                    let (rect, notification) = placed[at];
                    ui.fill(rect, if at == focused { FOCUSED } else { CARD }, 4.0);
                    ui.icon(rect.x + 36.0, rect.y + 24.0 + 13.0, glyph::NOTIFICATIONS, 26.0, Color::WHITE);
                    if at == focused {
                        focus_ring(ui, rect);
                    }
                    let text = Rect::new(rect.x + 69.0, rect.y + 18.0, width, rect.h - 36.0);
                    ui.paragraph(text, &notification.text, &BODY, HAlign::Left, VAlign::Center, 3);
                }
            });
        });
    }

    /// Audio's entries, and over them the open dropdown's list.
    fn audio(&mut self, ui: &mut Ui, audio: &Audio, card: Option<Card>, discord: &Discord) {
        let mut y = FIRST_TOP;
        let mut placed = Vec::with_capacity(self.audio.len());
        for &item in &self.audio {
            // A divider is 1 high, 14 under what is above it and 26 over the box.
            if item == AudioItem::NowPlaying {
                ui.fill(Rect::new(FLYOUT.x + 30.0, y + 14.0, FLYOUT.w - 60.0, 1.0), DIVIDER, 0.0);
                y += 41.0;
            }
            let (height, below) = item.height(card.is_some());
            placed.push(Rect::new(FLYOUT.x + INSET, y, FLYOUT.w - 2.0 * INSET, height));
            y += height + below;
        }
        let open = self.list.map(|(flow, _)| flow);
        // The focused entry last, so its glow is over its neighbours.
        let unfocused = (0..placed.len()).filter(|&at| at != self.item);
        for at in unfocused.chain([self.item]) {
            let (item, rect) = (self.audio[at], placed[at]);
            let expanded = item.flow().is_some() && item.flow() == open;
            let focused = at == self.item && open.is_none() && self.control.is_none();
            if focused || expanded {
                ui.fill(rect, FOCUSED, 4.0);
            }
            if focused {
                focus_ring(ui, rect);
            }
            match (item, card) {
                (AudioItem::NowPlaying, Some(card)) => {
                    let focus = self.control.filter(|_| at == self.item);
                    self.art = now_playing(ui, rect, card, focus, self.art.take());
                }
                _ => audio_item(ui, item, rect, audio, expanded, discord),
            }
        }
        if let Some((flow, at)) = self.list {
            // Over the entries it hangs across, their text too.
            ui.layer();
            let dropdown = placed[if flow == Flow::Output { 0 } else { 1 }];
            device_list(ui, dropdown, audio, flow, at);
        }
    }
}

/// One of Audio's entries in its box.
fn audio_item(ui: &mut Ui, item: AudioItem, rect: Rect, audio: &Audio, expanded: bool, discord: &Discord) {
    let (_, cy) = rect.center();
    let (icon_x, text_x) = (rect.x + 36.0, rect.x + 69.0);
    let line = ui.measure(&BODY, "Volume").height;
    match item {
        AudioItem::Output | AudioItem::Input => {
            let flow = if item == AudioItem::Output { Flow::Output } else { Flow::Input };
            let (glyph, title) = match flow {
                Flow::Output => (icon::OUTPUT, "Output"),
                Flow::Input => (icon::INPUT, "Input"),
            };
            // The icon is at the box's top, 22 down; the title and the drop box under it are in its middle.
            ui.icon(icon_x, rect.y + 22.0 + 13.0, glyph, 26.0, Color::WHITE);
            let top = cy - (line + 10.0 + 48.0) / 2.0;
            ui.label(text_x, top, title, &BODY);
            let drop = Rect::new(text_x, top + line + 10.0, rect.right() - 24.0 - text_x, 48.0);
            drop_box(ui, drop, audio.selection(flow), expanded);
        }
        AudioItem::Volume => {
            ui.icon(icon_x, cy, glyph::VOLUME, 26.0, Color::WHITE);
            // The title, and 12 under it the bar, 6 high: together in the box's middle.
            let top = cy - (line + 12.0 + 6.0) / 2.0;
            ui.label(text_x, top, "Volume", &BODY);
            let bar = Rect::new(text_x, top + line + 12.0, rect.right() - 16.0 - 56.0 - text_x, 6.0);
            ui.fill(bar, Color::argb(0x40FFFFFF), 3.0);
            let filled = bar.w * audio.volume.min(100) as f32 / 100.0;
            if filled > 0.0 {
                ui.fill(Rect::new(bar.x, bar.y, filled, bar.h), ACCENT_LIGHT, 3.0);
            }
            ui.label_right(rect.right() - 24.0, cy, &audio.volume.to_string(), &BODY);
        }
        AudioItem::Mute => {
            ui.icon(icon_x, cy, icon::MUTE, 26.0, Color::WHITE);
            ui.label_middle(text_x, cy, "Mute", &BODY);
            switch(ui, Rect::new(rect.right() - 24.0 - 48.0, cy - 12.0, 48.0, 24.0), audio.muted);
        }
        AudioItem::DiscordMute | AudioItem::DiscordDeafen => {
            let (glyph, title, on) = if item == AudioItem::DiscordMute {
                (icon::INPUT, "Mute in Discord", discord.voice.mute)
            } else {
                (icon::HEADSET, "Deafen in Discord", discord.voice.deaf)
            };
            ui.icon(icon_x, cy, glyph, 26.0, Color::WHITE);
            ui.label_middle(text_x, cy, title, &BODY);
            switch(ui, Rect::new(rect.right() - 24.0 - 48.0, cy - 12.0, 48.0, 24.0), on);
        }
        AudioItem::NowPlaying => {
            ui.icon(icon_x, cy, icon::MUSIC, 26.0, TEXT_DIM);
            ui.label_middle(text_x, cy, "No music app is running", &BODY.color(TEXT_DIM));
        }
    }
}

/// A dropdown's box: the device set, a long name cut short, and a chevron at its end. Open, it is ringed in the
/// accent with the focus glow, and the chevron points up.
fn drop_box(ui: &mut Ui, rect: Rect, selection: &str, open: bool) {
    ui.fill(rect, CARD, 4.0);
    if open {
        ui.glow(rect.grow(4.5), ACCENT_LIGHT.fade(0.5), 8.5, 11.0, 17.0);
        ui.ring(rect, ACCENT_LIGHT, 4.0, 2.0);
    } else {
        ui.ring(rect, Color::argb(0x80FFFFFF), 4.0, 2.0);
    }
    let name = Rect::new(rect.x + 14.0, rect.y, rect.w - 14.0 - 46.0, rect.h);
    ui.paragraph(name, selection, &BODY, HAlign::Left, VAlign::Center, 1);
    let chevron = if open { icon::CHEVRON_UP } else { icon::CHEVRON_DOWN };
    ui.icon(rect.right() - 16.0 - 8.0, rect.center().1, chevron, 16.0, Color::WHITE);
}

/// The mute switch: a pill edged in white with its knob at the left, filled in the accent with the knob at the
/// right when on.
pub(super) fn switch(ui: &mut Ui, rect: Rect, on: bool) {
    let (_, cy) = rect.center();
    if on {
        ui.fill(rect, ACCENT_LIGHT, 12.0);
    } else {
        ui.ring(rect, Color::WHITE, 12.0, 2.0);
    }
    let x = if on { rect.right() - 6.0 - 12.0 } else { rect.x + 6.0 };
    ui.fill(Rect::new(x, cy - 6.0, 12.0, 12.0), Color::WHITE, 6.0);
}

/// An open dropdown's list: a box hanging from the dropdown over the entries below, a row a device, the default
/// one ticked.
fn device_list(ui: &mut Ui, dropdown: Rect, audio: &Audio, flow: Flow, focused: usize) {
    let (devices, default) = audio.devices(flow);
    // From 78 in to 33 short of the Flyout's right, its top 8 above the dropdown's foot; 6 of padding above and
    // below its rows, each 60 high with 2 under it, inside a border of 1.
    let height = 2.0 + 12.0 + devices.len() as f32 * 62.0;
    let list = Rect::new(FLYOUT.x + 78.0, dropdown.bottom() - 8.0, FLYOUT.w - 78.0 - 33.0, height);
    ui.fill(list, Color::rgb(0x2E2E2E), 4.0);
    ui.ring(list, DIVIDER, 4.0, 1.0);
    let row =
        |at: usize| Rect::new(list.x + 1.0 + 6.0, list.y + 1.0 + 6.0 + at as f32 * 62.0, list.w - 2.0 - 12.0, 60.0);
    let unfocused = (0..devices.len()).filter(|&at| at != focused);
    for at in unfocused.chain((focused < devices.len()).then_some(focused)) {
        let rect = row(at);
        let (_, cy) = rect.center();
        if at == focused {
            ui.fill(rect, FOCUSED, 4.0);
            focus_ring(ui, rect);
        }
        if default == Some(at) {
            ui.icon(rect.x + 14.0 + 10.0, cy, icon::TICK, 20.0, Color::WHITE);
        }
        let name = Rect::new(rect.x + 52.0, rect.y, rect.w - 52.0 - 16.0, rect.h);
        ui.paragraph(name, &devices[at].name, &BODY, HAlign::Left, VAlign::Center, 1);
    }
}

/// The card's own height, and its row of controls' with the room under it.
const CARD_HEIGHT: f32 = 150.0;
const CONTROLS_HEIGHT: f32 = 68.0;
/// The album art's square, and the grey one under it for none.
const ART: f32 = 120.0;
const NO_ART: Color = Color::rgb(0x4A4A4A);
/// A control: 72 by 52 with 10 either side, the three centred under the card.
const CONTROL: (f32, f32) = (72.0, 52.0);
const CONTROL_GAP: f32 = 20.0;

/// The Now Playing card in its entry's box, as Windows' media flyout shows the media (the Bundle's `Media` grid):
/// the album art on a rounded square, 18 in, and beside it the title, the artist in grey, a thin bar of how far in
/// it is, and the play or pause mark with the app and the time; under all of it the controls, `focus` on one of
/// them. `last` is the art drawn last time, kept up while this track's is decoded. Gives the art drawn now.
fn now_playing(
    ui: &mut Ui,
    rect: Rect,
    card: Card,
    focus: Option<MediaControl>,
    last: Option<PathBuf>,
) -> Option<PathBuf> {
    let session = card.session;
    let art = Rect::new(rect.x + 18.0, rect.y + (CARD_HEIGHT - ART) / 2.0, ART, ART);
    ui.fill(art, NO_ART, 4.0);
    ui.icon(art.center().0, art.center().1, icon::MUSIC, 44.0, TEXT_DIM);
    let key = |path: &PathBuf| ArtKey { path: path.clone(), kind: Kind::Thumb(240) };
    let shown = match &session.art {
        Some(path) if ui.art(art, &key(path), Crop::Center, 1.0, 4.0) => Some(path.clone()),
        // Still being decoded: the last track's stays up until it is.
        Some(_) => last.filter(|last| ui.art(art, &key(last), Crop::Center, 1.0, 4.0)),
        None => None,
    };

    // The text column: 22 right of the art, 16 short of the box's right, its lines together in the card's middle.
    let text_x = art.right() + 22.0;
    let width = rect.right() - 16.0 - text_x;
    // A title that fits two lines is set with no limit, as the Guide's entries set theirs.
    let lines = if ui.measure_wrapped(&BODY, &session.title, width, 0).lines <= 2 { 0 } else { 2 };
    let title = ui.measure_wrapped(&BODY, &session.title, width, lines);
    let line = title.line_height;
    let taken = title.height.min(68.0);
    let length = !session.length.is_zero();
    // The title, the artist, the bar (10 above it, 4 high) and the app's line (8 above it).
    let height = taken + line + if length { 14.0 } else { 0.0 } + 8.0 + line;
    // A column taller than the card (a title on two lines over all the rest) is centred all the same, and cut to
    // the card's 150, as WPF lays it out and clips it to its row.
    ui.with_clip(Rect::new(text_x, rect.y, width, CARD_HEIGHT), |ui| {
        let mut y = rect.y + (CARD_HEIGHT - height) / 2.0;
        let box_ = Rect::new(text_x, y, width, title.height);
        ui.with_clip(Rect::new(text_x, y, width, taken), |ui| {
            ui.paragraph(box_, &session.title, &BODY, HAlign::Left, VAlign::Top, lines);
        });
        y += taken;
        let artist = session.artist.as_deref().unwrap_or("");
        ui.paragraph(Rect::new(text_x, y, width, line), artist, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 1);
        y += line;
        if length {
            let bar = Rect::new(text_x, y + 10.0, width, 4.0);
            ui.fill(bar, Color::argb(0x40FFFFFF), 2.0);
            let share = (card.position.as_secs_f32() / session.length.as_secs_f32()).clamp(0.0, 1.0);
            if share > 0.0 {
                ui.fill(Rect::new(bar.x, bar.y, bar.w * share, bar.h), ACCENT_LIGHT, 2.0);
            }
            y += 14.0;
        }
        y += 8.0;
        // The play mark while it plays, the pause mark while paused, in a column 20 wide; the app 8 past it, cut short
        // before the time, which is at the right, 12 clear of it: "1:05 / 3:20".
        let mid = y + line / 2.0;
        ui.icon(text_x + 10.0, mid, if session.playing { icon::PLAY } else { icon::PAUSE }, 16.0, TEXT_DIM);
        let time =
            length.then(|| format!("{} / {}", model::media::time(card.position), model::media::time(session.length)));
        let time_width = time.as_ref().map_or(0.0, |time| ui.measure(&BODY, time).width + 12.0);
        if let Some(time) = &time {
            ui.label_right(text_x + width, mid, time, &BODY.color(TEXT_DIM));
        }
        let app = Rect::new(text_x + 28.0, y, (width - 28.0 - time_width).max(0.0), line);
        ui.paragraph(app, &session.app(), &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Top, 1);
    });

    // The controls, centred under the card across its width. The focused one last, so its glow is over the others.
    let row = CONTROL.0 * 3.0 + CONTROL_GAP * 2.0;
    let first = rect.x + 18.0 + (rect.w - 18.0 - 16.0 - row) / 2.0;
    let unfocused = MediaControl::ALL.into_iter().filter(|&control| Some(control) != focus);
    for control in unfocused.chain(focus) {
        let at = MediaControl::ALL.iter().position(|&c| c == control).unwrap_or(0) as f32;
        let place = Rect::new(first + at * (CONTROL.0 + CONTROL_GAP), rect.y + CARD_HEIGHT, CONTROL.0, CONTROL.1);
        let focused = Some(control) == focus;
        // One the app has off: its fill at half and its glyph at 0.35.
        let enabled = control.enabled(session);
        let fill = if focused { FOCUSED } else { CARD };
        ui.fill(place, if enabled { fill } else { fill.fade(0.5) }, 4.0);
        if focused {
            focus_ring(ui, place);
        }
        let colour = if enabled { Color::WHITE } else { Color::WHITE.fade(0.35) };
        let (cx, cy) = place.center();
        ui.icon(cx, cy, control.glyph(session.playing), 22.0, colour);
    }
    shown
}
