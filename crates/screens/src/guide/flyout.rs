//! The Flyouts: a second panel a Utility opens over the Guide's own (docs/spec/guide.md, section 5). Notifications
//! lists what has been posted, newest first; Audio has the system's sound: the output and input devices, each a
//! dropdown with its list, the master volume and mute.
//!
//! This file is which Flyout is open and where focus is in it, and how it is drawn. What it shows is the
//! Dashboard's (`Around`), and what a press changes of the system's sound is asked of the Dashboard
//! (`GuideAction`), which asks the shell.
use model::{Audio, Flow};
use ui::{Button, Color, Ease, HAlign, Press, Rect, Ui, VAlign};

use super::panel::PANEL;
use super::{entries, focus_ring, Around, GuideAction, HEADING};
use crate::theme::{glyph, ACCENT_LIGHT, BODY, TEXT_DIM};
use crate::Notification;

/// The Flyout: x 155..691, from y 33 to the screen's foot. Drawn a corner's worth past the foot, so that only its
/// top corners are round.
const FLYOUT: Rect = Rect::new(155.0, 33.0, 536.0, 1047.0 + RADIUS);
const RADIUS: f32 = 12.0;
const BACK: Color = Color::rgb(0x1F1F1F);
/// The dim over the Guide's own panel while a Flyout is over it.
const PANEL_DIM: Color = Color::argb(0x73000000);
/// It comes in from this far to the left, fading in, over this long.
const TRAVEL: f32 = -48.0;
const OPEN_SECONDS: f32 = 0.12;
/// The title's top, and the entries' under it: the title's line and 24.
const TITLE_TOP: f32 = FLYOUT.y + 36.0;
const FIRST_TOP: f32 = TITLE_TOP + 39.6 + 24.0;
/// The entries end 24 above the screen's foot.
const FOOT: f32 = 1080.0 - 24.0;
/// An entry's box is 9 in from the Flyout's sides.
const INSET: f32 = 9.0;
/// The volume moves by this much a press.
const VOLUME_STEP: i32 = 2;

const FOCUSED: Color = Color::argb(0x26FFFFFF);
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
}

/// Which Flyout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flyout {
    Notifications,
    Audio,
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
}

/// Audio's entries, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioItem {
    Output,
    Input,
    Volume,
    Mute,
    /// "No music app is running", under a divider: where Now Playing will be (M5). It takes focus and does
    /// nothing.
    NothingPlaying,
}

impl AudioItem {
    pub const ALL: [AudioItem; 5] =
        [AudioItem::Output, AudioItem::Input, AudioItem::Volume, AudioItem::Mute, AudioItem::NothingPlaying];

    fn flow(self) -> Option<Flow> {
        match self {
            AudioItem::Output => Some(Flow::Output),
            AudioItem::Input => Some(Flow::Input),
            _ => None,
        }
    }

    /// The box's height, and the room under it.
    fn height(self) -> (f32, f32) {
        match self {
            AudioItem::Output | AudioItem::Input => (128.0, 3.0),
            AudioItem::Volume => (96.0, 3.0),
            AudioItem::Mute | AudioItem::NothingPlaying => (75.0, 3.0),
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
}

/// The open Flyout.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct State {
    pub which: Flyout,
    item: usize,
    /// The dropdown whose list is open, and the device focus is on in it.
    list: Option<(Flow, usize)>,
    scroll: f32,
    /// Which opening this is, of all the Guide's: each comes in once.
    opening: u32,
}

impl State {
    /// Opens with focus on the first entry.
    pub fn new(which: Flyout, opening: u32) -> Self {
        Self { which, item: 0, list: None, scroll: 0.0, opening }
    }

    fn count(&self, around: Around) -> usize {
        match self.which {
            Flyout::Notifications => around.notifications.len(),
            Flyout::Audio => AudioItem::ALL.len(),
        }
    }

    pub fn focus(&self, around: Around) -> FlyoutFocus {
        match self.list {
            Some((flow, at)) => FlyoutFocus::Device(flow, at),
            None if self.count(around) == 0 => FlyoutFocus::Nothing,
            None => FlyoutFocus::Item(self.item),
        }
    }

    /// What it shows changed under it: focus stays at the same place, or the last there is now. An open list
    /// follows its devices, and closes when there are none.
    pub fn follow(&mut self, around: Around) {
        self.item = self.item.min(self.count(around).saturating_sub(1));
        if let Some((flow, at)) = self.list {
            let devices = around.audio.devices(flow).0.len();
            self.list = (devices > 0).then(|| (flow, at.min(devices - 1)));
        }
    }

    pub fn press(&mut self, press: Press, around: Around) -> Outcome {
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
        let last = self.count(around).saturating_sub(1);
        let audio = (self.which == Flyout::Audio).then(|| AudioItem::ALL[self.item]);
        match (press.button, audio) {
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
    pub fn draw(&mut self, ui: &mut Ui, around: Around) {
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
                };
                let line = Rect::new(FLYOUT.x + 28.0, TITLE_TOP, FLYOUT.w - 56.0, 44.0);
                ui.paragraph(line, title, &HEADING, HAlign::Left, VAlign::Top, 1);
                match self.which {
                    Flyout::Notifications => self.notifications(ui, around.notifications),
                    Flyout::Audio => self.audio(ui, around.audio),
                }
            });
        });
    }

    /// The part of the Flyout the entries show in, wide enough for a focused entry's glow.
    fn viewport() -> Rect {
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
    fn audio(&self, ui: &mut Ui, audio: &Audio) {
        let mut y = FIRST_TOP;
        let mut placed = Vec::with_capacity(AudioItem::ALL.len());
        for item in AudioItem::ALL {
            // A divider is 1 high, 14 under what is above it and 26 over the box.
            if item == AudioItem::NothingPlaying {
                ui.fill(Rect::new(FLYOUT.x + 30.0, y + 14.0, FLYOUT.w - 60.0, 1.0), DIVIDER, 0.0);
                y += 41.0;
            }
            let (height, below) = item.height();
            placed.push(Rect::new(FLYOUT.x + INSET, y, FLYOUT.w - 2.0 * INSET, height));
            y += height + below;
        }
        let open = self.list.map(|(flow, _)| flow);
        // The focused entry last, so its glow is over its neighbours.
        let unfocused = (0..placed.len()).filter(|&at| at != self.item);
        for at in unfocused.chain([self.item]) {
            let (item, rect) = (AudioItem::ALL[at], placed[at]);
            let expanded = item.flow().is_some() && item.flow() == open;
            let focused = at == self.item && open.is_none();
            if focused || expanded {
                ui.fill(rect, FOCUSED, 4.0);
            }
            if focused {
                focus_ring(ui, rect);
            }
            audio_item(ui, item, rect, audio, expanded);
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
fn audio_item(ui: &mut Ui, item: AudioItem, rect: Rect, audio: &Audio, expanded: bool) {
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
        AudioItem::NothingPlaying => {
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
fn switch(ui: &mut Ui, rect: Rect, on: bool) {
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
