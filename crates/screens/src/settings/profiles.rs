//! Account > Profiles, a Profile's page and its Picture page (docs/spec/settings.md 6.5 and 6.6;
//! docs/spec/profiles.md, section 4), as the Bundle's `PersonalisationPane` has them.
//!
//! - **Profiles**: a box a Profile, two wide, its picture and gamertag, then Add profile. A on a Profile opens its
//!   page; Add profile opens text entry for a gamertag, and the Profile is added once it is kept, focus on its box.
//! - **A Profile** (its gamertag the header): its picture, then Gamertag and Real name (text entry, from what they
//!   are), Picture, and Remove profile, which asks first and is not there while it is the only Profile.
//! - **Picture**: the pictures to wear, four wide: the Windows account picture and the Gamerpics folder's, then the
//!   plain ones, then From a file (text entry for a picture's path). The one worn is ticked. A wears one and steps
//!   back.
//!
//! On every page Left and Right move within the row, and do nothing at its ends; Up at the top does nothing; Down
//! goes a row down, or to the last box from the row above it (the Bundle's quirk, 9.3). Every change goes out as
//! `Effect::Profile`, and the pages draw what the next snapshot says.
use std::sync::atomic::{AtomicU32, Ordering};

use model::profile::SHAPES;
use model::{Library, Picture, Profile, ProfileChange};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, Face, BOXES_TOP, BOX_GAP, BOX_HEIGHT, BOX_WIDTH, LEFT};
use super::{Pane, Step};
use crate::keyboard::{self, Event};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, Open, PageAction};
use crate::profile;
use crate::theme::{ACCENT, ACCENT_LIGHT, BODY, TEXT_DIM};
use crate::Effect;

const ADD_PROFILE: &str = "Add profile";
const GAMERTAG: keyboard::Entry = keyboard::Entry::new("Gamertag");
const REAL_NAME: keyboard::Entry = keyboard::Entry::new("Real name");
const FILE: keyboard::Entry = keyboard::Entry::new("A picture's full path").limit(260);
const HINT: &str = "Add images to the Gamerpics folder to choose from more.";

/// Where focus goes from `at` among `count` places laid out `columns` wide (docs/spec/settings.md, 9.3).
fn step(at: usize, count: usize, columns: usize, button: Button) -> usize {
    match button {
        Button::Left if !at.is_multiple_of(columns) => at - 1,
        Button::Right if at % columns < columns - 1 && at + 1 < count => at + 1,
        Button::Up if at >= columns => at - columns,
        // From the last row too: there it jumps to the last box, sideways (the Bundle's quirk, 13.2).
        Button::Down => (at + columns).min(count - 1),
        _ => at,
    }
}

/// A box's place on a pane of boxes two wide from `top`.
fn box_rect(top: f32, at: usize) -> Rect {
    let (column, row) = ((at % 2) as f32, (at / 2) as f32);
    Rect::new(LEFT + (BOX_WIDTH + BOX_GAP) * column, top + (BOX_HEIGHT + BOX_GAP) * row, BOX_WIDTH, BOX_HEIGHT)
}

/// A Profile's box: its picture, 80 across 30 in, and its gamertag 22 right of it. No icon.
fn profile_box(ui: &mut Ui, rect: Rect, profile: &Profile, focused: bool) {
    // A Box's back, lit with focus, with no icon: the picture and the name.
    ui.fill(rect, Color::rgb(if focused { 0x4A4A4A } else { 0x2D2D2D }), 0.0);
    let picture = Rect::new(rect.x + 30.0, rect.center().1 - 40.0, 80.0, 80.0);
    profile::picture(ui, picture, &profile.picture);
    let name = Rect::new(picture.right() + 22.0, rect.y, rect.right() - 24.0 - picture.right() - 22.0, rect.h);
    ui.paragraph(name, &profile.gamertag, &BODY, HAlign::Left, VAlign::Center, 1);
    if focused {
        controls::focus_ring(ui, rect, controls::RING_BOX);
    }
}

/// Account > Profiles.
pub struct Profiles {
    /// The box focus is on: a Profile's place, or Add profile's after the last.
    focus: usize,
    /// The Profiles' gamertags as last drawn, for the focus line.
    names: Vec<String>,
    scroll: f32,
    asks: Vec<Effect>,
}

impl Profiles {
    /// Opens with focus on the first box.
    pub fn open(around: Around) -> Self {
        Self { focus: 0, names: names(around.library), scroll: 0.0, asks: Vec::new() }
    }

    fn take_in(&mut self, library: &Library) {
        self.names = names(library);
        self.focus = self.focus.min(self.names.len());
    }
}

fn names(library: &Library) -> Vec<String> {
    library.profiles.list.iter().map(|p| p.gamertag.clone()).collect()
}

impl Pane for Profiles {
    fn header(&self) -> String {
        "Profiles".into()
    }

    /// `Profiles: Alpha`, `Profiles: Add profile`.
    fn focus(&self) -> String {
        format!("Profiles: {}", self.names.get(self.focus).map_or(ADD_PROFILE, String::as_str))
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        self.take_in(around.library);
        let list = &around.library.profiles.list;
        match press.button {
            Button::B => Step::Back,
            Button::A if press.repeat => Step::Stay,
            Button::A => match list.get(self.focus) {
                Some(profile) => Step::Open(Box::new(ProfilePage::open(profile.id, around))),
                None => Step::Do(PageAction::Open(Open::TextEntry(GAMERTAG))),
            },
            button => {
                self.focus = step(self.focus, list.len() + 1, 2, button);
                Step::Stay
            }
        }
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        // A Profile added is where Add profile was, which focus stays on: on the new Profile's box, as the Bundle
        // puts it. One removed leaves focus on the one now in its place, or Add profile.
        self.take_in(around.library);
        None
    }

    /// Add profile's text entry starts empty.
    fn entry_text(&self) -> String {
        String::new()
    }

    fn entry(&mut self, event: Event, _around: Around) {
        if let Event::Kept(gamertag) = event {
            if !gamertag.trim().is_empty() {
                self.asks.push(Effect::Profile(ProfileChange::Add { gamertag: gamertag.trim().to_string() }));
            }
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        self.take_in(around.library);
        let list = &around.library.profiles.list;
        let at = self.focus;
        let rect = box_rect(BOXES_TOP, at);
        controls::scrolled(ui, &mut self.scroll, BOXES_TOP, rect.y, rect.bottom(), |ui| {
            let draw = |ui: &mut Ui, i: usize, focused: bool| match list.get(i) {
                Some(profile) => profile_box(ui, box_rect(BOXES_TOP, i), profile, focused),
                None => {
                    controls::settings_box(ui, box_rect(BOXES_TOP, i), ADD_PROFILE, Face::Icon('\u{E8FA}'), focused)
                }
            };
            for i in (0..=list.len()).filter(|&i| !focused || i != at) {
                draw(ui, i, false);
            }
            if focused {
                draw(ui, at, true);
            }
        });
    }
}

/// The boxes of a Profile's page, in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Gamertag,
    RealName,
    Picture,
    Remove,
}

impl Control {
    fn name(self) -> &'static str {
        match self {
            Control::Gamertag => "Gamertag",
            Control::RealName => "Real name",
            Control::Picture => "Picture",
            Control::Remove => "Remove profile",
        }
    }

    fn icon(self) -> char {
        match self {
            Control::Gamertag => '\u{E8EC}',
            Control::RealName => '\u{E8AC}',
            Control::Picture => '\u{E8B9}',
            Control::Remove => '\u{E74D}',
        }
    }
}

/// The answers to "Remove X?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Remove,
    Cancel,
}

/// Each prompt flies up afresh: a number for every one.
static FLIGHTS: AtomicU32 = AtomicU32::new(0);

/// The picture over the boxes: 120 across, its boxes 30 under it.
const PICTURE: Rect = Rect::new(LEFT, BOXES_TOP, 120.0, 120.0);
const PROFILE_BOXES_TOP: f32 = 360.0;

/// A Profile's page.
pub struct ProfilePage {
    id: i64,
    /// The Profile as last seen in the library: what the header and the focus line say, which are asked without it.
    profile: Option<Profile>,
    /// Whether it is the only one: then there is no Remove profile.
    only: bool,
    focus: usize,
    prompt: Option<MenuPanel<Confirm>>,
    flying: u32,
    leaving: Option<(MenuPanel<Confirm>, u32)>,
    asks: Vec<Effect>,
}

impl ProfilePage {
    /// Opens with focus on Gamertag.
    pub fn open(id: i64, around: Around) -> Self {
        let mut page =
            Self { id, profile: None, only: true, focus: 0, prompt: None, flying: 0, leaving: None, asks: Vec::new() };
        page.take_in(around.library);
        page
    }

    fn take_in(&mut self, library: &Library) {
        self.profile = library.profiles.get(self.id).cloned();
        self.only = library.profiles.list.len() < 2;
        self.focus = self.focus.min(self.controls().len() - 1);
    }

    fn controls(&self) -> Vec<Control> {
        let all = [Control::Gamertag, Control::RealName, Control::Picture, Control::Remove];
        all.into_iter().filter(|&c| c != Control::Remove || !self.only).collect()
    }

    fn focused(&self) -> Control {
        self.controls()[self.focus]
    }

    fn gamertag(&self) -> &str {
        self.profile.as_ref().map_or("", |p| p.gamertag.as_str())
    }

    fn prompt_press(&mut self, press: Press) -> Step {
        let Some(prompt) = self.prompt.as_mut() else { return Step::Stay };
        let chose = match prompt.press(press) {
            Answer::Chose(confirm) => confirm,
            Answer::Declined => Confirm::Cancel,
            Answer::Moved | Answer::Nothing => return Step::Stay,
        };
        if let Some(prompt) = self.prompt.take() {
            self.leaving = Some((prompt, self.flying));
        }
        match chose {
            // Back to Profiles, focus on the Profile now in this one's place.
            Confirm::Remove => Step::BackWith(PageAction::Effect(Effect::Profile(ProfileChange::Remove(self.id)))),
            Confirm::Cancel => Step::Stay,
        }
    }
}

impl Pane for ProfilePage {
    /// Its gamertag.
    fn header(&self) -> String {
        self.gamertag().into()
    }

    /// `Profile: Gamertag`; with the question up, `Profile: Prompt(Remove) of Alpha`.
    fn focus(&self) -> String {
        match &self.prompt {
            Some(prompt) => {
                let row = prompt.rows().get(prompt.focus()).map_or("", |row| row.label.as_str());
                format!("Profile: Prompt({row}) of {}", self.gamertag())
            }
            None => format!("Profile: {}", self.focused().name()),
        }
    }

    /// What it is now, which a change made here shows: `"Alpha", real name "Ann Smith", shape 3`. Nothing while
    /// Remove's question is up.
    fn state(&self) -> String {
        if self.prompt.is_some() {
            return String::new();
        }
        match &self.profile {
            Some(p) => format!("{:?}, real name {:?}, {}", p.gamertag, p.real_name, p.picture.describe()),
            None => "gone".into(),
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        self.take_in(around.library);
        if self.prompt.is_some() {
            return self.prompt_press(press);
        }
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        let Some(profile) = self.profile.clone() else { return Step::Back };
        match press.button {
            Button::B => return Step::Back,
            Button::A => match self.focused() {
                Control::Gamertag => return Step::Do(PageAction::Open(Open::TextEntry(GAMERTAG))),
                Control::RealName => return Step::Do(PageAction::Open(Open::TextEntry(REAL_NAME))),
                Control::Picture => return Step::Open(Box::new(Pictures::open(&profile, around))),
                Control::Remove => {
                    let answers = vec![
                        Row::new(icon::DELETE, "Remove", Confirm::Remove),
                        Row::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                    ];
                    let title = format!("Remove {}?", profile.gamertag);
                    self.prompt = Some(MenuPanel::prompt(&title, None, answers));
                    self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
                    self.leaving = None;
                }
            },
            button => self.focus = step(self.focus, self.controls().len(), 2, button),
        }
        Step::Stay
    }

    fn dialog(&self) -> Option<Dialog> {
        self.prompt.as_ref().map(|_| Dialog::Prompt)
    }

    fn dismiss_dialog(&mut self) {
        self.prompt = None;
        self.leaving = None;
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        self.take_in(around.library);
        None
    }

    /// Gamertag and Real name start from what they are.
    fn entry_text(&self) -> String {
        match (&self.profile, self.focused()) {
            (Some(p), Control::Gamertag) => p.gamertag.clone(),
            (Some(p), Control::RealName) => p.real_name.clone(),
            _ => String::new(),
        }
    }

    /// A gamertag kept is the Profile's, unless it is empty; a real name kept is, empty or not.
    fn entry(&mut self, event: Event, _around: Around) {
        let Event::Kept(text) = event else { return };
        let profile = self.id;
        let text = text.trim().to_string();
        match self.focused() {
            Control::Gamertag if !text.is_empty() => {
                self.asks.push(Effect::Profile(ProfileChange::Gamertag { profile, gamertag: text }));
            }
            Control::RealName => self.asks.push(Effect::Profile(ProfileChange::RealName { profile, real_name: text })),
            _ => {}
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        self.take_in(around.library);
        let Some(profile) = &self.profile else { return };
        profile::picture(ui, PICTURE, &profile.picture);
        let controls = self.controls();
        let at = self.focus;
        for (i, control) in controls.iter().enumerate().filter(|&(i, _)| !focused || i != at) {
            controls::settings_box(
                ui,
                box_rect(PROFILE_BOXES_TOP, i),
                control.name(),
                Face::Icon(control.icon()),
                false,
            );
        }
        if focused {
            let control = controls[at];
            controls::settings_box(
                ui,
                box_rect(PROFILE_BOXES_TOP, at),
                control.name(),
                Face::Icon(control.icon()),
                true,
            );
        }
        // The question over the dimmed page; or on its way out, the dim gone at once.
        if let Some(prompt) = &self.prompt {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("profile prompt", self.flying), false);
            prompt.draw_flown(ui, fly, shown);
        } else if let Some((prompt, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("profile prompt", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.leaving = None;
            } else {
                prompt.draw_flown(ui, fly, shown);
            }
        }
    }
}

/// A picture on the Picture page.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Offered {
    Wear(Picture),
    /// From a file: text entry for its path.
    File,
}

impl Offered {
    fn name(&self) -> String {
        match self {
            Offered::Wear(Picture::Shape(n)) => format!("Shape {n}"),
            Offered::Wear(picture) => picture.describe(),
            Offered::File => "From a file".into(),
        }
    }
}

/// A gamerpic button: 172 square, 30 apart.
const BUTTON: f32 = 172.0;
const BUTTON_PITCH: f32 = 202.0;
const COLUMNS: usize = 4;
/// The hint's line and the 26 under it.
const HINT_HEIGHT: f32 = 34.58 + 26.0;

/// Account > Profiles > a Profile > Picture.
pub struct Pictures {
    id: i64,
    offered: Vec<Offered>,
    /// Which of them the Profile wears, as last seen.
    worn: Option<usize>,
    /// Whether the hint shows: there is at most one picture file to choose from.
    hint: bool,
    focus: usize,
    scroll: f32,
    asks: Vec<Effect>,
    /// A path was kept: the page steps back.
    done: bool,
}

impl Pictures {
    /// Opens with focus on the picture worn, or the first.
    fn open(profile: &Profile, around: Around) -> Self {
        let files = &around.library.profiles.pictures;
        let offered: Vec<Offered> = files
            .iter()
            .map(|path| Offered::Wear(Picture::File(path.clone())))
            .chain((0..SHAPES).map(|n| Offered::Wear(Picture::Shape(n))))
            .chain(Some(Offered::File))
            .collect();
        let mut page = Self {
            id: profile.id,
            offered,
            worn: None,
            hint: files.len() <= 1,
            focus: 0,
            scroll: 0.0,
            asks: Vec::new(),
            done: false,
        };
        page.take_in(around.library);
        page.focus = page.worn.unwrap_or(0);
        page
    }

    fn take_in(&mut self, library: &Library) {
        let profile = library.profiles.get(self.id);
        self.worn = profile.and_then(|p| {
            self.offered.iter().position(|offered| matches!(offered, Offered::Wear(picture) if p.wears(picture)))
        });
    }

    fn top(&self) -> f32 {
        BOXES_TOP + if self.hint { HINT_HEIGHT } else { 0.0 }
    }

    fn button(&self, at: usize) -> Rect {
        let (column, row) = ((at % COLUMNS) as f32, (at / COLUMNS) as f32);
        Rect::new(LEFT + BUTTON_PITCH * column, self.top() + BUTTON_PITCH * row, BUTTON, BUTTON)
    }
}

/// A gamerpic button: the picture 148 across, 12 in; with focus a 6-wide ring at its edge and the round glow on the
/// ring's middle; the one worn with a tick badge at its foot's right.
fn picture_button(ui: &mut Ui, rect: Rect, offered: &Offered, worn: bool, focused: bool) {
    let inner = rect.grow(-12.0);
    match offered {
        Offered::Wear(picture) => profile::picture(ui, inner, picture),
        Offered::File => {
            ui.fill(inner, Color::rgb(0x2D2D2D), 999.0);
            ui.icon(inner.center().0, inner.center().1, '\u{E8E5}', 48.0, Color::WHITE);
        }
    }
    if worn {
        let badge = Rect::new(rect.right() - 44.0, rect.bottom() - 44.0, 44.0, 44.0);
        ui.fill(badge, Color::rgb(0x1F1F1F), 999.0);
        ui.fill(badge.grow(-3.0), ACCENT, 999.0);
        ui.icon(badge.center().0, badge.center().1, controls::glyph::TICK, 20.0, Color::WHITE);
    }
    if focused {
        ui.glow(rect.grow(2.5), ACCENT_LIGHT.fade(0.5), 999.0, 11.0, 17.0);
        ui.ring(rect, ACCENT_LIGHT, 999.0, 6.0);
    }
}

impl Pane for Pictures {
    fn header(&self) -> String {
        "Picture".into()
    }

    /// `Picture: Shape 3`, `Picture: user-192.png`, `Picture: From a file`; `; worn` on the one worn.
    fn focus(&self) -> String {
        format!("Picture: {}", self.offered[self.focus].name())
    }

    fn state(&self) -> String {
        if self.worn == Some(self.focus) {
            "worn".into()
        } else {
            String::new()
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        self.take_in(around.library);
        match press.button {
            Button::B => Step::Back,
            Button::A if press.repeat => Step::Stay,
            Button::A => match &self.offered[self.focus] {
                Offered::Wear(picture) => {
                    let change = ProfileChange::Picture { profile: self.id, picture: picture.clone() };
                    Step::BackWith(PageAction::Effect(Effect::Profile(change)))
                }
                Offered::File => Step::Do(PageAction::Open(Open::TextEntry(FILE))),
            },
            button => {
                self.focus = step(self.focus, self.offered.len(), COLUMNS, button);
                Step::Stay
            }
        }
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        self.take_in(around.library);
        None
    }

    /// A path kept is worn, if there is a picture there (the shell says if not), and the page steps back.
    fn entry(&mut self, event: Event, _around: Around) {
        let Event::Kept(text) = event else { return };
        if let Some(path) = model::by_hand::typed_path(&text) {
            let change = ProfileChange::Picture { profile: self.id, picture: Picture::File(path) };
            self.asks.push(Effect::Profile(change));
            self.done = true;
        }
    }

    fn asks(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.asks)
    }

    fn closes(&mut self) -> bool {
        std::mem::take(&mut self.done)
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        self.take_in(around.library);
        if self.hint {
            let style = BODY.color(TEXT_DIM);
            ui.paragraph(Rect::new(LEFT, BOXES_TOP, 1230.0, 34.58), HINT, &style, HAlign::Left, VAlign::Top, 1);
        }
        let at = self.focus;
        let rect = self.button(at);
        let top = self.top();
        let Self { offered, worn, scroll, .. } = self;
        let worn = *worn;
        let place = |i: usize| {
            let (column, row) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
            Rect::new(LEFT + BUTTON_PITCH * column, top + BUTTON_PITCH * row, BUTTON, BUTTON)
        };
        controls::scrolled(ui, scroll, top, rect.y, rect.bottom(), |ui| {
            for (i, offered) in offered.iter().enumerate().filter(|&(i, _)| !focused || i != at) {
                picture_button(ui, place(i), offered, worn == Some(i), false);
            }
            if focused {
                picture_button(ui, place(at), &offered[at], worn == Some(at), true);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_steps_within_rows_and_down_to_the_last_box() {
        use Button::*;
        // Three Profiles and Add profile, two wide.
        assert_eq!(step(0, 4, 2, Left), 0, "Left at the first column does nothing");
        assert_eq!(step(0, 4, 2, Right), 1);
        assert_eq!(step(1, 4, 2, Right), 1, "the row's end stops");
        assert_eq!(step(1, 4, 2, Down), 3);
        assert_eq!(step(3, 4, 2, Down), 3, "the foot stops");
        assert_eq!(step(2, 4, 2, Down), 3, "Down on the last row jumps to its last box");
        assert_eq!(step(0, 4, 2, Up), 0, "the top stops");
        // Down onto a shorter last row lands on its last box, even across.
        assert_eq!(step(1, 3, 2, Down), 2);
        assert_eq!(step(2, 3, 2, Right), 2);
        // Four wide: from the first row's third to a last row of two.
        assert_eq!(step(2, 6, 4, Down), 5);
    }
}
