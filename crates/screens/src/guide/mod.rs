//! The Guide: a panel from the left over the dimmed screen, as the Bundle draws it (docs/spec/guide.md). Tabs along
//! its top, the showing tab's entries down it, the Utilities at the landing tab's foot; beside it the hints and, at
//! the screen's top right, the Status area. Over Home it is drawn in Home's window; over a running game on the
//! Overlay, where its dim is over the game. The same code draws both.
//!
//! Over it come its dialogs, which take the pad while they are up: Power, a band across the screen, and the menu
//! panel, the running game's menu. Behind one the panel is out of focus under a second dim.
//!
//! This file is where focus is and where it goes, and how the Guide comes and goes; each part draws itself in its
//! own module.
mod discord;
mod entries;
mod flyout;
mod hints;
pub(crate) mod marks;
mod panel;
mod status;
mod utilities;

use model::discord::{CallAsk, Discord};
use model::{Audio, Flow, Library, MediaCommand, PowerCommand};
use ui::{Button, Color, Ease, Gradient, Press, Rect, TextStyle, Ui};

pub use self::discord::Item as DiscordItem;
pub use self::entries::{Entry, Kind};
pub use self::flyout::{AudioItem, Flyout, FlyoutFocus, MediaControl};
pub(crate) use self::status::pad_battery;
pub use self::status::{PcBattery, Status};
pub use self::utilities::Utility;
use crate::menu_panel::{self, Answer, MenuPanel, Row};
use crate::theme::ACCENT_LIGHT;
use crate::wide_dialog::{self, Tile, WideDialog};
use crate::Notification;

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
/// The dim under a dialog, over everything else of the Guide: black at 70%.
const DIALOG_DIM: Color = Color::argb(0xB3000000);
/// How far out of focus the panel is behind a dialog: WPF's blur radius. Nothing of it shows further than that
/// outside the panel.
const BEHIND_BLUR: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Landing,
    /// From the first Connect until Disconnect (docs/spec/discord-tab.md).
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
    /// My games & apps, Settings or the Search Utility was chosen: close, onto that page.
    MyGames,
    Settings,
    Search,
    /// The running game was chosen: close, back to it.
    Resume,
    /// A game played before was chosen, by its place in the library: launched, or with another game running asked
    /// about first (`Guide::ask_quit_and_start`).
    Launch(usize),
    /// Switch profile was chosen: the Guide goes at once, and Who's playing today? asks.
    SwitchProfile,
    /// Quit the running game.
    QuitGame,
    /// "Quit and start": quit the running game, and start this one when it has stopped.
    QuitAndStart(usize),
    /// "Force quit": the running game did not close when asked.
    ForceQuit,
    /// A opened a dialog over the Guide, a Flyout or a dropdown's list: there is nothing to carry out.
    Asked,
    /// Audio's Volume: the system's master volume to this, 0 to 100.
    Volume(u8),
    /// Audio's Mute: the system's master mute on or off.
    Mute(bool),
    /// A device chosen in one of Audio's dropdowns, by its place among its flow's: Windows' default from now.
    Device(Flow, usize),
    /// One of the Now Playing card's controls: the app playing is asked, and nothing shows changed until it answers.
    Media(MediaCommand),
    /// A Power option was chosen: close, then carry it out.
    Power(PowerChoice),
    /// Mute, Deafen or Leave on the Discord tab: asked of Discord, and shown done only once Discord says so.
    Discord(CallAsk),
}

/// Power's options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerChoice {
    /// Something asked of the machine.
    Machine(PowerCommand),
    /// Leave to the desktop: the Bundle's Exit Playnite.
    Exit,
}

/// What a row of one of the Guide's menus stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuChoice {
    QuitGame,
    /// "Quit and start", with the game to start.
    QuitAndStart(usize),
    ForceQuit,
    /// A prompt's "Cancel": as B.
    Cancel,
}

/// A dialog over the Guide. It takes every press but the Guide button's.
#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    Power(WideDialog<PowerChoice>),
    Menu(MenuPanel<MenuChoice>),
}

/// Which dialog is up over the Guide and where focus is in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogFocus {
    Power(wide_dialog::Focus),
    /// A menu panel: whether it is a prompt, and its focused row from the top.
    Menu {
        prompt: bool,
        row: usize,
    },
}

fn power() -> WideDialog<PowerChoice> {
    let machine = PowerChoice::Machine;
    let tiles = vec![
        Tile::new('\u{E7E8}', "Turn off PC", machine(PowerCommand::TurnOff)),
        Tile::new('\u{E72C}', "Restart PC", machine(PowerCommand::Restart)),
        Tile::new('\u{E708}', "Sleep", machine(PowerCommand::Sleep)),
        Tile::new('\u{F3B1}', "Exit to desktop", PowerChoice::Exit),
    ];
    WideDialog::new("What do you want to do?", tiles)
}

/// What the Guide's lists are made from.
#[derive(Clone, Copy)]
pub struct Around<'a> {
    pub library: &'a Library,
    /// The game running, by its place in the library.
    pub playing: Option<usize>,
    /// Whether the Discord tab is there.
    pub discord: bool,
    /// The system's sound, which the Audio Flyout shows.
    pub audio: &'a Audio,
    /// What has been posted, oldest first, which the Notifications Flyout lists.
    pub notifications: &'a [Notification],
    /// The media the Audio Flyout's Now Playing card shows, if any plays.
    pub now_playing: Option<crate::now_playing::Card<'a>>,
}

/// What the Guide shows that is not its own: the Profile signed in, the clock, the Status area's values.
pub struct Face<'a> {
    pub profile: &'a model::Profile,
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
    /// Whether the panel shows while the Guide is open. It does not when Power came by a hold of the Guide button:
    /// then there is the dim and Power, and closing Power closes the Guide.
    panel: bool,
    /// The dialog up over the Guide, and how many have come up: each comes in once.
    dialog: Option<Dialog>,
    asked: u32,
    /// The Flyout open over the panel, and how many have opened.
    flyout: Option<flyout::State>,
    flyouts: u32,
    /// Whether any of it showed when last drawn: the panel still sliding away, or the dim still fading.
    shown: bool,
    /// How many times the tab has changed since the Guide opened, and which way the last change went: -1 for the
    /// left bumper, 1 for the right. The showing tab's content comes in from that side.
    turns: (u32, f32),
    /// How far through its lagging slide the panel's content was when last drawn, 0 to 1.
    lag: f32,
    /// How far the entries are scrolled.
    scroll: f32,
    /// Discord as it last said, and as it was when the lists were last made: the Discord tab's entries.
    discord: Discord,
    listed_discord: Discord,
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
            panel: true,
            dialog: None,
            asked: 0,
            flyout: None,
            flyouts: 0,
            shown: false,
            turns: (0, 0.0),
            lag: 0.0,
            scroll: 0.0,
            discord: Discord::default(),
            listed_discord: Discord::default(),
        }
    }
}

impl Guide {
    /// Opens on the landing tab with focus on Home, always: the Guide remembers nothing from the last time.
    pub fn open(&mut self, around: Around) {
        self.open_on(Tab::Landing, around);
    }

    /// Opens on a tab, focus on its first entry: Home's profile opens the Guide on Profile & system.
    pub fn open_on(&mut self, tab: Tab, around: Around) {
        self.open = true;
        self.panel = true;
        self.dialog = None;
        self.flyout = None;
        self.turns = (0, 0.0);
        self.show(tab, around);
    }

    /// Power, by a hold of the Guide button: over the dim alone. An open Guide's panel slides away under it and a
    /// dialog that was up is gone; a closed Guide opens with no panel. Closing Power then closes the Guide.
    pub fn open_power(&mut self, around: Around) {
        if !self.open {
            self.open(around);
        }
        self.panel = false;
        self.flyout = None;
        self.ask_power();
    }

    /// Puts Power up, focus on Close. Power taking its own place is the same showing of it: it does not come in
    /// again.
    fn ask_power(&mut self) {
        if !matches!(self.dialog, Some(Dialog::Power(_))) {
            self.asked += 1;
        }
        self.dialog = Some(Dialog::Power(power()));
    }

    pub fn close(&mut self) {
        self.open = false;
        self.dialog = None;
        self.flyout = None;
    }

    /// Closes at once, with no slide away: Start-up is coming over everything (the Bundle's `DismissAtOnce`).
    pub fn close_at_once(&mut self) {
        self.close();
        self.shown = false;
    }

    /// Opens a Utility's Flyout over the panel, focus on its first entry: A on the Utility, or the Guide opened
    /// on what a Toast told of. Only on a tab that has the Utility.
    pub fn open_flyout(&mut self, which: Flyout) {
        let utility = match which {
            Flyout::Notifications => Utility::Notifications,
            Flyout::Audio => Utility::Audio,
        };
        if let Some(at) = self.utilities.iter().position(|&u| u == utility) {
            self.focus = Focus::Utility(at);
            self.flyouts += 1;
            self.flyout = Some(flyout::State::new(which, self.flyouts));
        }
    }

    /// The Flyout open over the panel, and where focus is in it.
    pub fn flyout(&self, around: Around) -> Option<(Flyout, FlyoutFocus)> {
        self.flyout.as_ref().map(|flyout| (flyout.which, flyout.focus(around)))
    }

    /// The dialog up over the Guide, if one is, and where its focus is.
    pub fn dialog(&self) -> Option<DialogFocus> {
        match self.dialog.as_ref()? {
            Dialog::Power(power) => Some(DialogFocus::Power(power.focus())),
            Dialog::Menu(menu) => Some(DialogFocus::Menu { prompt: menu.is_prompt(), row: menu.focus() }),
        }
    }

    /// The title of the menu or prompt up over the Guide.
    pub fn dialog_title(&self) -> Option<&str> {
        match self.dialog.as_ref()? {
            Dialog::Menu(menu) => Some(menu.title()),
            Dialog::Power(_) => None,
        }
    }

    /// Whether the panel shows: not when Power came by a hold, nor under a prompt that came with the Guide off.
    pub fn panel_shown(&self) -> bool {
        self.panel
    }

    /// The panel does not show: what is up is a prompt on its own, and answering it closes the Guide.
    pub fn without_panel(&mut self) {
        self.panel = false;
    }

    /// Asks "Quit {running} and start {name}?" (docs/spec/guide.md, 6.4): A on a game while another runs.
    pub fn ask_quit_and_start(&mut self, running: &str, game: usize, name: &str) {
        let rows = vec![
            Row::new(menu_panel::icon::PLAY, "Quit and start", MenuChoice::QuitAndStart(game)),
            Row::new(menu_panel::icon::CANCEL, "Cancel", MenuChoice::Cancel),
        ];
        self.ask(MenuPanel::prompt(&format!("Quit {running} and start {name}?"), None, rows));
    }

    /// Asks "Force quit {name}?": the game was asked to close and has not.
    pub fn ask_force_quit(&mut self, name: &str) {
        let rows = vec![
            Row::new(menu_panel::icon::QUIT, "Force quit", MenuChoice::ForceQuit),
            Row::new(menu_panel::icon::CANCEL, "Cancel", MenuChoice::Cancel),
        ];
        let message = "It hasn't closed. Anything unsaved will be lost.";
        self.ask(MenuPanel::prompt(&format!("Force quit {name}?"), Some(message), rows));
    }

    /// Puts a menu or a prompt up, in place of whatever dialog was. A Flyout that was open closes under it.
    fn ask(&mut self, menu: MenuPanel<MenuChoice>) {
        self.flyout = None;
        self.dialog = Some(Dialog::Menu(menu));
        self.asked += 1;
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

    /// Shows this tab, if the Guide has it, as it shows at once on opening: what a Toast about something on it
    /// opens the Guide at.
    pub fn show_tab(&mut self, tab: Tab, around: Around) {
        if self.tabs.contains(&tab) {
            self.show(tab, around);
        }
    }

    /// Puts focus on this Utility, if the showing tab has it.
    pub fn focus_utility(&mut self, utility: Utility) {
        if let Some(at) = self.utilities.iter().position(|&u| u == utility) {
            self.focus = Focus::Utility(at);
        }
    }

    /// Shows a tab from its top, focus on its first entry: on the Discord tab, the first that A does something on.
    fn show(&mut self, tab: Tab, around: Around) {
        self.tab = tab;
        self.list(around);
        self.scroll = 0.0;
        let first = if self.tab == Tab::Discord { discord::first_focus(&self.entries) } else { 0 };
        self.focus = Focus::Entry(first);
        self.scroll_to_focus();
    }

    /// Discord as it says now. The Discord tab follows it as `refresh` makes the lists again.
    pub fn set_discord(&mut self, discord: &Discord) {
        self.discord = discord.clone();
    }

    /// The Discord tab's part of `--log-focus`'s Guide line: the entry focus is on, and what Discord says.
    pub fn discord_line(&self) -> Option<String> {
        let focus = match self.focus {
            Focus::Entry(at) => Some(at),
            Focus::Utility(_) => None,
        };
        (self.tab == Tab::Discord).then(|| discord::line(&self.entries, focus, &self.discord))
    }

    /// On Profile & system, the entry focus is on by name, after its place: ` Power`, ` SwitchProfile`, ` Achievement
    /// "Ash Runner"`. Its places move with the achievements under Switch profile; its names do not.
    pub fn profile_line(&self) -> Option<String> {
        let Focus::Entry(at) = self.focus else { return None };
        let entry = self.entries.get(at).filter(|_| self.tab == Tab::ProfileAndSystem)?;
        Some(match entry.kind {
            Kind::RecentAchievement(_) => format!(" Achievement {:?}", entry.title),
            kind => format!(" {kind:?}"),
        })
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
        self.entries = entries::of(self.tab, around, &self.discord);
        self.listed_discord = self.discord.clone();
        // Only the landing tab has Utilities.
        self.utilities = if self.tab == Tab::Landing { Utility::LANDING.to_vec() } else { Vec::new() };
    }

    /// What is around changed while the Guide is open (a game started or stopped, a new snapshot of the library):
    /// the lists are made again, and focus stays at the same place in them.
    /// On the Discord tab focus follows its entry through Discord's changes (`discord::focus_after`).
    pub fn refresh(&mut self, around: Around) {
        let before = (self.tab, std::mem::take(&mut self.entries), std::mem::take(&mut self.listed_discord));
        self.list(around);
        if let Focus::Entry(at) = self.focus {
            self.focus = Focus::Entry(if before.0 == Tab::Discord && self.tab == Tab::Discord {
                discord::focus_after((&before.1, &before.2), (&self.entries, &self.discord), at)
            } else {
                at.min(self.entries.len().saturating_sub(1))
            });
        }

        // A menu or a prompt is about the running game: when that has stopped it goes, in silence. Power is about
        // the PC, and stays.
        if matches!(self.dialog, Some(Dialog::Menu(_))) && around.playing.is_none() {
            self.dialog = None;
        }
        // An open Flyout follows what it shows: the notifications, the devices.
        if let Some(flyout) = &mut self.flyout {
            flyout.follow(around);
        }
        self.scroll_to_focus();
    }

    /// The pad while the Guide is open. What a press asks for beyond the Guide itself is returned for the Dashboard
    /// to carry out. Only a direction held down repeats. A dialog that is up takes every press but the Guide
    /// button's, which closes the Guide whatever is up.
    pub fn press(&mut self, press: Press, around: Around) -> Option<GuideAction> {
        if press.repeat && !press.button.is_direction() {
            return None;
        }
        if self.dialog.is_some() {
            return if press.button == Button::Guide { Some(GuideAction::Close) } else { self.answer(press) };
        }
        // An open Flyout takes the pad: B closes a dropdown's list, then the Flyout, focus back on its Utility.
        // The bumpers do nothing meanwhile.
        if let Some(flyout) = &mut self.flyout {
            if press.button == Button::Guide {
                return Some(GuideAction::Close);
            }
            return match flyout.press(press, around) {
                flyout::Outcome::Chose(action) => Some(action),
                flyout::Outcome::Close => {
                    self.flyout = None;
                    None
                }
                flyout::Outcome::Nothing => None,
            };
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
            (Button::A, Focus::Entry(at)) => {
                let action = self.entries.get(at).and_then(select);
                if action == Some(GuideAction::Asked) {
                    self.ask_power();
                }
                return action;
            }
            (Button::A, Focus::Utility(at)) => {
                return match self.utilities.get(at)? {
                    Utility::Search => Some(GuideAction::Search),
                    Utility::Notifications => {
                        self.open_flyout(Flyout::Notifications);
                        Some(GuideAction::Asked)
                    }
                    Utility::Audio => {
                        self.open_flyout(Flyout::Audio);
                        Some(GuideAction::Asked)
                    }
                };
            }
            // The running game's menu: its name, and Quit.
            (Button::Menu, Focus::Entry(at)) if self.entries.get(at).is_some_and(Entry::has_menu) => {
                let rows = vec![Row::new(menu_panel::icon::QUIT, "Quit", MenuChoice::QuitGame)];
                self.ask(MenuPanel::menu(&self.entries[at].title, rows));
            }
            (Button::B | Button::Guide, _) => return Some(GuideAction::Close),
            _ => {}
        }
        self.scroll_to_focus();
        None
    }

    /// A press while a dialog is up. Choosing an option closes the dialog and is the Dashboard's to carry out.
    /// B, and Close in Power, close it onto the Guide where it was, or close the Guide with it when there is no
    /// panel to go back to.
    fn answer(&mut self, press: Press) -> Option<GuideAction> {
        let action = match self.dialog.as_mut()? {
            Dialog::Power(power) => match power.press(press) {
                Answer::Chose(Some(choice)) => Some(GuideAction::Power(choice)),
                Answer::Chose(None) | Answer::Declined => (!self.panel).then_some(GuideAction::Close),
                Answer::Moved | Answer::Nothing => return None,
            },
            Dialog::Menu(menu) => match menu.press(press) {
                Answer::Chose(MenuChoice::QuitGame) => Some(GuideAction::QuitGame),
                Answer::Chose(MenuChoice::QuitAndStart(game)) => Some(GuideAction::QuitAndStart(game)),
                Answer::Chose(MenuChoice::ForceQuit) => Some(GuideAction::ForceQuit),
                Answer::Chose(MenuChoice::Cancel) | Answer::Declined => (!self.panel).then_some(GuideAction::Close),
                Answer::Moved | Answer::Nothing => return None,
            },
        };
        self.dialog = None;
        action
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
        // The panel is in while the Guide is open, unless Power came by a hold: then it slides away, or never
        // comes, under a dim that stays.
        let panel = self.open && self.panel;
        let (to, seconds, ease) =
            if panel { (0.0, OPEN_SECONDS, Ease::CubicOut) } else { (OFFSCREEN, CLOSE_SECONDS, Ease::CubicIn) };
        let slide = ui.slide_from("guide slide", OFFSCREEN, to, seconds, ease);
        let fade = match (ui.store.reduce_motion, self.open) {
            (true, _) => 0.0,
            (false, true) => OPEN_SECONDS,
            (false, false) => CLOSE_SECONDS,
        };
        let dim = ui.tween_from("guide dim", 0.0, if self.open { 1.0 } else { 0.0 }, fade, Ease::Linear);
        // What is on the panel comes in a moment behind it, and goes out with it, held where it is: a close
        // before it has started never brings it in after.
        let whole = CONTENT_LAG_SECONDS + OPEN_SECONDS;
        self.lag = if panel {
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
            // What stands beside the panel goes with it, at once. A dialog over the panel leaves it be.
            if self.panel {
                top_shade(ui, screen);
                // The hints hide for a Flyout.
                if self.flyout.is_none() {
                    ui.with_offset(screen.x, 0.0, |ui| hints::draw(ui, focused.is_some_and(Entry::has_menu)));
                }
                status::draw(ui, face.status, face.clock, face.pad);
            }
        });
        // The panel stays at the screen's left, whatever the screen's shape. Behind a dialog it is one picture,
        // out of focus.
        if slide > OFFSCREEN {
            ui.with_offset(screen.x + slide, 0.0, |ui| {
                if self.dialog.is_some() {
                    let bounds = panel::PANEL.grow(BEHIND_BLUR);
                    ui.group(bounds, BEHIND_BLUR, 1.0, |ui| self.panel(ui, behind, around, face));
                } else {
                    self.panel(ui, behind, around, face);
                    if let Some(flyout) = &mut self.flyout {
                        flyout.draw(ui, around);
                    }
                }
            });
        }
        // The dialog, over a dim of its own that is there at once, as the dialog is.
        if let Some(dialog) = self.dialog.as_ref().filter(|_| self.open) {
            ui.layer();
            ui.fill(screen, DIALOG_DIM, 0.0);
            match dialog {
                Dialog::Power(power) => power.draw(ui, self.asked),
                Dialog::Menu(menu) => {
                    let at = menu.centred(ui);
                    menu.draw(ui, at);
                }
            }
        }
    }

    /// The panel and what is on it, which is `behind` to its left while it comes in.
    fn panel(&self, ui: &mut Ui, behind: f32, around: Around, face: &Face) {
        panel::background(ui);
        ui.with_offset(behind, 0.0, |ui| {
            panel::tabs(ui, &self.tabs, self.tab, &face.profile.picture);
            self.content(ui, around, face);
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
                let faces =
                    entries::Faces { library: around.library, profile: face.profile, discord: &self.listed_discord };
                ui.with_clip(viewport, |ui| {
                    ui.with_offset(0.0, -self.scroll, |ui| {
                        let unfocused = (0..placed.len()).filter(|&at| Some(at) != focus);
                        for at in unfocused.chain(focus.filter(|&at| at < placed.len())) {
                            entries::draw(ui, &self.entries[at], &placed[at], focus == Some(at), &faces);
                        }
                    });
                });

                // Focus is in the open Flyout, not on the Utility that opened it.
                let focus = match self.focus {
                    Focus::Utility(at) if self.flyout.is_none() => Some(at),
                    _ => None,
                };
                // The badge counts what the Flyout lists, or what the shell says there is, whichever is more.
                let count = face.status.notifications.max(around.notifications.len() as u32);
                utilities::draw(ui, &self.utilities, focus, count);
            });
        });
    }
}

/// What A does on an entry, if anything: the Profile entry never does anything, nor does Switch profile while it is
/// greyed out, over a running game. Over a running game Home and the pages bring the Dashboard in front of it, and a
/// recent game is asked about first: both are the Dashboard's to do.
fn select(entry: &Entry) -> Option<GuideAction> {
    match entry.kind {
        Kind::Home => Some(GuideAction::Home),
        Kind::Running(_) => Some(GuideAction::Resume),
        Kind::Recent(game) => Some(GuideAction::Launch(game)),
        Kind::Power => Some(GuideAction::Asked),
        Kind::MyGames => Some(GuideAction::MyGames),
        Kind::Settings => Some(GuideAction::Settings),
        // Switch profile closes the Guide and asks Who's playing today? (docs/spec/start-up.md, 7).
        Kind::SwitchProfile if entry.enabled => Some(GuideAction::SwitchProfile),
        Kind::Profile | Kind::SwitchProfile | Kind::RecentAchievement(_) => None,
        Kind::Discord(item) => discord::select(item),
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
