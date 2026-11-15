//! The Guide: a panel from the left over the dimmed screen, as the Bundle draws it (docs/spec/guide.md). Tabs along
//! its top, the showing tab's entries down it, the Utilities at the landing tab's foot; beside it the hints and, at
//! the screen's top right, the Status area. Over Home it is drawn in Home's window; over a running game on the
//! Overlay, where its dim is over the game. The same code draws both.
//!
//! This file is where focus is and where it goes, and how the Guide comes and goes; each part draws itself in its
//! own module.
mod entries;
mod hints;
mod marks;
mod panel;
mod status;
mod utilities;

use model::Library;
use ui::{Button, Color, Ease, Gradient, Press, Rect, TextStyle, Ui};

pub use self::entries::{Entry, Kind};
pub use self::status::{PcBattery, Status};
pub use self::utilities::Utility;
use crate::theme::ACCENT_LIGHT;

/// The panel slides in over 120 ms and out over 150; what is on it comes in 50 ms behind it.
const OPEN_SECONDS: f32 = 0.12;
const CLOSE_SECONDS: f32 = 0.15;
const CONTENT_LAG_SECONDS: f32 = 0.05;
/// Off the screen's left: far enough to take a Flyout with it.
const OFFSCREEN: f32 = -700.0;
/// A new tab's content starts this far to the side it was reached from, and settles in this long.
const TAB_TRAVEL: f32 = 50.0;
const TAB_SECONDS: f32 = 0.12;
/// The dim over everything behind the Guide: black at 56%.
const DIM: Color = Color::argb(0x8F000000);
const HEADING: TextStyle = TextStyle::new(ui::Font::Display, 33.0, 400);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Landing,
    /// Only while Discord is connected (M6). Its entries come with it.
    Discord,
    ProfileAndSystem,
}

/// Where focus is on the showing tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// An entry, from the top.
    Entry(usize),
    /// A Utility, from the left.
    Utility(usize),
}

/// What a press in the Guide asks the Dashboard to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuideAction {
    /// B or the Guide button: close, in silence.
    Close,
    /// Home was chosen: close, onto Home.
    Home,
    /// The running game was chosen: close, back to it.
    Resume,
    /// Launch the game at this place in the library.
    Launch(usize),
    /// Quit the running game.
    QuitGame,
    Exit,
}

/// What the Guide's lists are made from.
#[derive(Clone, Copy)]
pub struct Around<'a> {
    pub library: &'a Library,
    /// The game running, by its place in the library.
    pub playing: Option<usize>,
    /// Whether the Discord tab is there.
    pub discord: bool,
}

/// What the Guide shows that is not its own: the user's name, the clock, the Status area's values.
pub struct Face<'a> {
    pub user: &'a str,
    pub clock: &'a str,
    /// Whether a pad is connected.
    pub pad: bool,
    pub status: &'a Status,
}

pub struct Guide {
    tabs: Vec<Tab>,
    tab: Tab,
    entries: Vec<Entry>,
    utilities: Vec<Utility>,
    focus: Focus,
    open: bool,
    /// Whether any of it showed when last drawn: the panel still sliding away, or the dim still fading.
    shown: bool,
    /// How many times the tab has changed since the Guide opened, and which way the last change went: -1 for the
    /// left bumper, 1 for the right. The showing tab's content comes in from that side.
    turns: (u32, f32),
    /// How far through its lagging slide the panel's content was when last drawn, 0 to 1.
    lag: f32,
    /// How far the entries are scrolled.
    scroll: f32,
}

impl Default for Guide {
    fn default() -> Self {
        Self {
            tabs: vec![Tab::Landing, Tab::ProfileAndSystem],
            tab: Tab::Landing,
            entries: Vec::new(),
            utilities: Vec::new(),
            focus: Focus::Entry(0),
            open: false,
            shown: false,
            turns: (0, 0.0),
            lag: 0.0,
            scroll: 0.0,
        }
    }
}

impl Guide {
    /// Opens on the landing tab with focus on Home, always: the Guide remembers nothing from the last time.
    pub fn open(&mut self, around: Around) {
        self.open = true;
        self.turns = (0, 0.0);
        self.show(Tab::Landing, around);
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The tab showing and where focus is on it.
    pub fn focus(&self) -> (Tab, Focus) {
        (self.tab, self.focus)
    }

    /// The showing tab's entries, top to bottom.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The tabs along the top, left to right.
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Whether there is anything of it to draw: open, or still sliding away.
    pub fn showing(&self) -> bool {
        self.open || self.shown
    }

    /// Shows a tab from its top, focus on its first entry.
    fn show(&mut self, tab: Tab, around: Around) {
        self.tab = tab;
        self.list(around);
        self.focus = Focus::Entry(0);
        self.scroll = 0.0;
    }

    /// Makes the tabs and the showing tab's lists from what is around now.
    fn list(&mut self, around: Around) {
        self.tabs = if around.discord {
            vec![Tab::Landing, Tab::Discord, Tab::ProfileAndSystem]
        } else {
            vec![Tab::Landing, Tab::ProfileAndSystem]
        };
        if !self.tabs.contains(&self.tab) {
            self.tab = Tab::Landing;
        }
        self.entries = entries::of(self.tab, around);
        // Only the landing tab has Utilities.
        self.utilities = if self.tab == Tab::Landing { Utility::LANDING.to_vec() } else { Vec::new() };
    }

    /// What is around changed while the Guide is open (a game started or stopped, a new snapshot of the library):
    /// the lists are made again, and focus stays at the same place in them.
    pub fn refresh(&mut self, around: Around) {
        self.list(around);
        if let Focus::Entry(at) = self.focus {
            self.focus = Focus::Entry(at.min(self.entries.len().saturating_sub(1)));
        }
        self.scroll_to_focus();
    }

    /// The pad while the Guide is open. What a press asks for beyond the Guide itself is returned for the Dashboard
    /// to carry out. Only a direction held down repeats.
    pub fn press(&mut self, press: Press, around: Around) -> Option<GuideAction> {
        if press.repeat && !press.button.is_direction() {
            return None;
        }
        let last = self.entries.len().checked_sub(1);
        match (press.button, self.focus) {
            (Button::Up, Focus::Entry(at)) => self.focus = Focus::Entry(at.saturating_sub(1)),
            // Up from any Utility is the tab's last entry.
            (Button::Up, Focus::Utility(_)) => self.focus = last.map_or(self.focus, Focus::Entry),
            // Down from the last entry is the first Utility, where the tab has any.
            (Button::Down, Focus::Entry(at)) if Some(at) >= last => {
                if !self.utilities.is_empty() {
                    self.focus = Focus::Utility(0);
                }
            }
            (Button::Down, Focus::Entry(at)) => self.focus = Focus::Entry(at + 1),
            (Button::Left, Focus::Utility(at)) => self.focus = Focus::Utility(at.saturating_sub(1)),
            (Button::Right, Focus::Utility(at)) => {
                self.focus = Focus::Utility((at + 1).min(self.utilities.len().saturating_sub(1)));
            }
            (Button::LeftBumper, _) => self.turn(-1, around),
            (Button::RightBumper, _) => self.turn(1, around),
            (Button::A, Focus::Entry(at)) => return self.entries.get(at).and_then(|entry| select(entry, around)),
            // The Utilities' Flyouts (#41) and Search (M3) are yet to come.
            (Button::A, Focus::Utility(_)) => {}
            // The running game's menu (#40) is yet to come: until it does, Menu is its one option, Quit.
            (Button::Menu, Focus::Entry(at)) if self.entries.get(at).is_some_and(Entry::has_menu) => {
                return Some(GuideAction::QuitGame);
            }
            (Button::B | Button::Guide, _) => return Some(GuideAction::Close),
            _ => {}
        }
        self.scroll_to_focus();
        None
    }

    /// The tab `by` places along, stopping at either end; focus goes to its first entry.
    fn turn(&mut self, by: i32, around: Around) {
        let at = self.tabs.iter().position(|&tab| tab == self.tab).unwrap_or(0) as i32;
        let next = self.tabs[(at + by).clamp(0, self.tabs.len() as i32 - 1) as usize];
        if next != self.tab {
            self.turns = (self.turns.0 + 1, by as f32);
            self.show(next, around);
        }
    }

    fn scroll_to_focus(&mut self) {
        let Focus::Entry(at) = self.focus else { return };
        let placed = entries::layout(self.tab, &self.entries);
        if let Some(placed) = placed.get(at) {
            self.scroll = entries::scroll_to(self.scroll, at == 0, placed.extent, entries::viewport(self.tab));
        }
    }

    pub fn draw(&mut self, ui: &mut Ui, around: Around, face: &Face) {
        if !self.showing() {
            return;
        }
        // The panel slides in fast and settling, and out slowly then fast; the dim fades evenly with it, and not
        // at all under Reduce motion, as in the Bundle.
        let (to, seconds, ease) =
            if self.open { (0.0, OPEN_SECONDS, Ease::CubicOut) } else { (OFFSCREEN, CLOSE_SECONDS, Ease::CubicIn) };
        let slide = ui.slide_from("guide slide", OFFSCREEN, to, seconds, ease);
        let fade = if ui.store.reduce_motion { 0.0 } else { seconds };
        let dim = ui.tween_from("guide dim", 0.0, if self.open { 1.0 } else { 0.0 }, fade, Ease::Linear);
        // What is on the panel comes in a moment behind it, and goes out with it, held where it is: a close
        // before it has started never brings it in after.
        let whole = CONTENT_LAG_SECONDS + OPEN_SECONDS;
        self.lag = if self.open {
            ui.slide_from("guide contents", 0.0, 1.0, whole, Ease::Linear)
        } else {
            ui.tween("guide contents", self.lag, 0.0, Ease::Linear)
        };
        let behind = OFFSCREEN * (1.0 - Ease::CubicOut.at((self.lag * whole - CONTENT_LAG_SECONDS) / OPEN_SECONDS));
        self.shown = slide > OFFSCREEN || dim > 0.0;
        if !self.showing() {
            return;
        }

        let screen = ui.screen();
        let focused = match self.focus {
            Focus::Entry(at) => self.entries.get(at),
            Focus::Utility(_) => None,
        };
        ui.layer();
        ui.with_alpha(dim, |ui| {
            ui.fill(screen, DIM, 0.0);
            top_shade(ui, screen);
            ui.with_offset(screen.x, 0.0, |ui| hints::draw(ui, focused.is_some_and(Entry::has_menu)));
            status::draw(ui, face.status, face.clock, face.pad);
        });
        // The panel stays at the screen's left, whatever the screen's shape.
        ui.with_offset(screen.x + slide, 0.0, |ui| {
            panel::background(ui);
            ui.with_offset(behind, 0.0, |ui| {
                panel::tabs(ui, &self.tabs, self.tab);
                self.content(ui, around, face);
            });
        });
    }

    /// The showing tab's content: its heading, its entries and its Utilities. After a change of tab it comes in
    /// from the side the change went toward, fading in; not as the Guide opens, and not under Reduce motion.
    fn content(&self, ui: &mut Ui, around: Around, face: &Face) {
        let (turn, toward) = self.turns;
        let (offset, alpha) = if turn == 0 || ui.store.reduce_motion {
            (0.0, 1.0)
        } else {
            let offset = ui.slide_from(("guide tab", turn), toward * TAB_TRAVEL, 0.0, TAB_SECONDS, Ease::CubicOut);
            (offset, ui.tween_from(("guide tab fade", turn), 0.0, 1.0, TAB_SECONDS, Ease::QuadOut))
        };
        ui.with_offset(offset, 0.0, |ui| {
            ui.with_alpha(alpha, |ui| {
                let heading = match self.tab {
                    Tab::Landing => None,
                    Tab::Discord => Some("Discord"),
                    Tab::ProfileAndSystem => Some("Profile & system"),
                };
                if let Some(heading) = heading {
                    let line = Rect::new(panel::PANEL.x + 28.0, entries::HEADING_TOP, panel::PANEL.w - 28.0, 44.0);
                    ui.paragraph(line, heading, &HEADING, ui::HAlign::Left, ui::VAlign::Top, 1);
                }

                // The entries, cut to their part of the panel, the focused one last so that it is over its
                // neighbours. The cut is wider than the panel: a ring at its edge is not cut.
                let (top, foot) = entries::viewport(self.tab);
                let viewport = Rect::new(panel::PANEL.x - 24.0, top, panel::PANEL.w + 48.0, foot - top);
                let placed = entries::layout(self.tab, &self.entries);
                let focus = match self.focus {
                    Focus::Entry(at) => Some(at),
                    Focus::Utility(_) => None,
                };
                let faces = entries::Faces { library: around.library, user: face.user };
                ui.with_clip(viewport, |ui| {
                    ui.with_offset(0.0, -self.scroll, |ui| {
                        let unfocused = (0..placed.len()).filter(|&at| Some(at) != focus);
                        for at in unfocused.chain(focus.filter(|&at| at < placed.len())) {
                            entries::draw(ui, &self.entries[at], &placed[at], focus == Some(at), &faces);
                        }
                    });
                });

                let focus = match self.focus {
                    Focus::Utility(at) => Some(at),
                    Focus::Entry(_) => None,
                };
                utilities::draw(ui, &self.utilities, focus, face.status.notifications);
            });
        });
    }
}

/// What A does on an entry, if anything. Entries whose pages or dialogs are yet to come are drawn and do nothing:
/// My games & apps and Settings (M3), Switch profile (M5), the Profile entry (which never does).
fn select(entry: &Entry, around: Around) -> Option<GuideAction> {
    let playing = around.playing.is_some();
    match entry.kind {
        // Bringing Home in front of a running game is the shell's to do (#40): until then Home is chosen at Home.
        Kind::Home => (!playing).then_some(GuideAction::Home),
        Kind::Running(_) => Some(GuideAction::Resume),
        // With a game running, a recent game asks "Quit X and start Y?" first (#40).
        Kind::Recent(game) => (!playing).then_some(GuideAction::Launch(game)),
        // Power's dialog (#39) is yet to come: until it does, Power at Home is its last option, the way out.
        Kind::Power => (!playing).then_some(GuideAction::Exit),
        Kind::MyGames | Kind::Profile | Kind::SwitchProfile | Kind::Settings => None,
    }
}

/// The shade across the screen's top, behind the panel and the Status area: near-opaque black at the very top,
/// gone by y 420, so the clock reads over whatever is behind.
fn top_shade(ui: &mut Ui, screen: Rect) {
    const STOPS: [(f32, u32); 4] = [(0.0, 0xFA000000), (0.3, 0xEB000000), (0.6, 0xA6000000), (1.0, 0x00000000)];
    for pair in STOPS.windows(2) {
        let ((from, top), (to, bottom)) = (pair[0], pair[1]);
        let part = Rect::new(screen.x, from * 420.0, screen.w, (to - from) * 420.0);
        ui.gradient(part, Color::argb(top), Color::argb(bottom), Gradient::Vertical, 0.0);
    }
}

/// The focus ring round an entry or a Utility, with its glow: a 4-wide stroke 4 clear of the box. Its corners are
/// WPF's 12 on the stroke's centre line, so 14 at its outer edge. It appears at once and does not travel.
fn focus_ring(ui: &mut Ui, rect: Rect) {
    // The theme's glow, as Home's tiles have it: a band 11 wide on the ring's centre line, at half opacity,
    // blurred by 17.
    ui.glow(rect.grow(11.5), ACCENT_LIGHT.fade(0.5), 15.5, 11.0, 17.0);
    ui.ring(rect.grow(8.0), ACCENT_LIGHT, 14.0, 4.0);
}
