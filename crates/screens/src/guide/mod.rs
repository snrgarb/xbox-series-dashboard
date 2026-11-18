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
mod chat;
mod direct;
mod discord;
mod entries;
mod flyout;
mod hints;
pub(crate) mod marks;
mod panel;
mod servers;
mod status;
mod utilities;
mod when;

use model::discord::{Discord, REMOVE_SHORTCUT};
use model::discord_dms::{Direct, DirectAsk, DirectState, Id, REACTIONS};
use model::{Audio, Flow, Library, MediaCommand, PowerCommand};
use ui::{Button, Color, Ease, Gradient, Press, Rect, TextStyle, Ui};

pub use self::direct::{All, Back, Chat, Dms, Draft, Place, View};
pub use self::discord::{DiscordAsk, Item as DiscordItem, Note as DirectNote};
pub use self::entries::{Entry, Kind};
pub use self::flyout::{AudioItem, Flyout, FlyoutFocus, MediaControl};
pub(crate) use self::status::{call_mark, pad_battery};
pub use self::status::{PcBattery, Status};
pub use self::utilities::Utility;
pub use self::when::When;
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
    /// My games & apps, Settings, the Search Utility or the Downloads Utility was chosen: close, onto that page.
    MyGames,
    Settings,
    Search,
    ManageQueue,
    /// The running game was chosen: close, back to it.
    Resume,
    /// A game played before was chosen, by its place in the library: launched, or with another game running asked
    /// about first (`Guide::ask_quit_and_start`).
    Launch(usize),
    /// An Achievement unlocked notification was chosen: close, onto that game's Achievements page (its row in the
    /// library database).
    Achievements(i64),
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
    /// What the Discord tab or its Flyout asks of Discord: shown done only once Discord says so.
    Discord(DiscordAsk),
    /// A on "Sign in to Discord in Settings > Account > Direct Messages": the Guide closes onto that page.
    DirectSettings,
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
    /// A Shortcut's menu's one option: the Shortcut the menu is on (`Guide::shortcut_menu`) forgotten.
    RemoveShortcut,
    /// A row of a Direct Messages menu (`Guide::direct_menu` says what it is about).
    Dm(DmRow),
}

/// The rows of the DM's menu, a message's menu and the Add Reaction panel (docs/spec/party-chats.md, 3.6 and 6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmRow {
    MarkRead,
    Call,
    Reply,
    AddReaction,
    Retry,
    Delete,
    /// One of the six reactions, by its place in `REACTIONS`.
    React(usize),
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
    /// The channel of the Shortcut whose menu is up: the menu stays while that Shortcut does.
    shortcut_menu: Option<String>,
    /// Direct Messages: what the session said, the views over the panel, drafts and messages going out.
    dms: Dms,
    /// What Direct Messages was when the lists were last made.
    listed_direct: Direct,
    /// What the DM or message menu (or Add Reaction) up over the Guide is about.
    direct_menu: Option<direct::Target>,
    /// The disc of the Direct Messages row focus was last on: where Up from the entry under it lands.
    row_memory: usize,
    /// Text entry is up over a conversation: its view ends at y 556.
    writing: bool,
    /// The DM focus was last on in the row, by its conversation: where All Direct Messages starts.
    last_dm: Option<Id>,
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
            shortcut_menu: None,
            dms: Dms::default(),
            listed_direct: Direct::default(),
            direct_menu: None,
            row_memory: 0,
            writing: false,
            last_dm: None,
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
        self.direct_menu = None;
        self.dms.close_all();
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
        self.direct_menu = None;
        self.writing = false;
        self.dms.close_all();
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
            // Not a Utility's: Servers opens it, on the Discord tab.
            Flyout::Discord => return,
        };
        if let Some(at) = self.utilities.iter().position(|&u| u == utility) {
            self.focus = Focus::Utility(at);
            self.flyouts += 1;
            self.flyout = Some(flyout::State::new(which, self.flyouts, &self.discord));
        }
    }

    /// The Flyout open over the panel, and where focus is in it.
    pub fn flyout(&self, around: Around) -> Option<(Flyout, FlyoutFocus)> {
        self.flyout.as_ref().map(|flyout| (flyout.which, flyout.focus(around, &self.discord)))
    }

    /// What `--log-focus`'s Guide line says of the Discord Flyout, when it is open: its title and what focus is on
    /// (`"Stand-in Server" "Late night" of 2: Empty`).
    pub fn discord_flyout_line(&self) -> Option<String> {
        let flyout = self.flyout.as_ref().filter(|flyout| flyout.which == Flyout::Discord)?;
        Some(servers::line(flyout, &self.discord.browse))
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
            // A name or the first words of a message: never in the log.
            Dialog::Menu(_) if self.direct_menu.is_some() => None,
            Dialog::Menu(menu) => Some(menu.title()),
            Dialog::Power(_) => None,
        }
    }

    /// What a Direct Messages menu up over the Guide is, for `--log-focus`: `dm`, `message` or `reaction`.
    pub fn dialog_tag(&self) -> Option<&'static str> {
        Some(match self.direct_menu.as_ref()? {
            direct::Target::Dm(_) => "dm",
            direct::Target::Message(_) => "message",
            direct::Target::Reaction(_) => "reaction",
        })
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
        self.shortcut_menu = None;
        self.direct_menu = None;
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

    /// Opens this DM's conversation over the tab, focus on its reply box, as A on its disc does: a Toast's tap
    /// (docs/spec/party-chats.md, 7.1). B comes back to the tab. `DirectAsk::Open` is asked of the session.
    pub fn open_conversation(&mut self, id: Id) {
        self.dms.open_chat(id, direct::Back::Tab);
    }

    /// Whether this DM's conversation is open over the tab (arrived or still loading).
    pub fn conversation_open(&self, id: &Id) -> bool {
        self.open && self.dms.chat().is_some_and(|chat| &chat.id == id)
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
        (self.row_memory, self.last_dm) = (0, None);
        self.remember_row();
        self.scroll_to_focus();
    }

    /// Discord as it says now. The Discord tab follows it as `refresh` makes the lists again.
    pub fn set_discord(&mut self, discord: &Discord) {
        self.discord = discord.clone();
    }

    /// Direct Messages as the web session says now (docs/spec/party-chats.md). The tab, a Flyout and a conversation
    /// follow it as `refresh` makes the lists again; the Dashboard calls that.
    pub fn set_direct(&mut self, direct: &Direct) {
        self.dms.set(direct);
        // What a menu is about may be gone.
        if self.dialog.is_some() && !self.direct_menu_alive() {
            self.dialog = None;
            self.direct_menu = None;
        }
    }

    /// Direct Messages as last said.
    pub fn direct(&self) -> &Direct {
        &self.dms.direct
    }

    /// What the Guide keeps of Direct Messages: what is open, the drafts, what is going out.
    pub fn dms(&self) -> &Dms {
        &self.dms
    }

    /// What has been asked of the web session since this was last called.
    pub fn take_asks(&mut self) -> Vec<DirectAsk> {
        std::mem::take(&mut self.dms.asks)
    }

    /// Text entry the Guide wants over a conversation's reply box, once.
    pub fn take_write(&mut self) -> Option<direct::Write> {
        self.dms.write.take()
    }

    /// The web session says a message went, or did not.
    pub fn sent(&mut self, channel: &Id, nonce: u64, message: &Id) {
        self.dms.sent(channel, nonce, message);
    }

    pub fn not_sent(&mut self, channel: &Id, nonce: u64) {
        self.dms.not_sent(channel, nonce);
    }

    /// Text entry is up (or gone) over the conversation.
    pub fn set_writing(&mut self, writing: bool) {
        self.writing = writing;
    }

    /// The text typed so far is the reply box's draft.
    pub fn set_draft(&mut self, channel: &Id, text: &str) {
        self.dms.set_draft(channel, text);
    }

    /// Menu or Enter on text entry: the draft goes out.
    pub fn send_draft(&mut self, channel: &Id) {
        self.dms.send(channel);
    }

    /// Time passes: messages going out that are not seen sent become Not sent.
    pub fn pass(&mut self, seconds: f32) {
        self.dms.pass(seconds);
    }

    /// How soon something here has to be looked at again, in seconds.
    pub fn timer(&self) -> Option<f32> {
        let loading =
            self.dms.loading_since.map(|since| (1.0 - (self.dms.age - since)).max(0.0)).filter(|&left| left > 0.0);
        [loading].into_iter().flatten().reduce(f32::min)
    }

    /// What time it is, for the times of messages: the machine's unless told.
    pub fn set_when(&mut self, when: When) {
        self.dms.when = when;
    }

    /// "Discord wants to check it's you": a prompt over the Guide (docs/spec/party-chats.md, 6.4).
    pub fn ask_captcha(&mut self) {
        let message = "Sign in again in Settings > Account > Direct Messages to carry on.";
        self.ask_ok("Discord wants to check it's you", message);
    }

    /// "Discord didn't send it", with Discord's own reason as its page gives it.
    pub fn ask_refused(&mut self, reason: &str) {
        self.ask_ok("Discord didn't send it", reason);
    }

    fn ask_ok(&mut self, title: &str, message: &str) {
        let rows = vec![Row::new(menu_panel::icon::CANCEL, "OK", MenuChoice::Cancel)];
        self.ask(MenuPanel::prompt(title, Some(message), rows));
    }

    /// Whether what the open Direct Messages menu is about is still there.
    fn direct_menu_alive(&self) -> bool {
        match &self.direct_menu {
            None => true,
            Some(direct::Target::Dm(id)) => self.dms.direct.dm(id).is_some(),
            Some(direct::Target::Message(id) | direct::Target::Reaction(id)) => match self.dms.chat() {
                Some(chat) => self.dms.message(&chat.id, id).is_some(),
                None => false,
            },
        }
    }

    /// The Discord tab's part of `--log-focus`'s Guide line: the entry focus is on, and what Discord says.
    pub fn discord_line(&self) -> Option<String> {
        let focus = match self.focus {
            Focus::Entry(at) => Some(at),
            Focus::Utility(_) => None,
        };
        (self.tab == Tab::Discord).then(|| discord::line(&self.entries, focus, &self.discord, &self.listed_direct))
    }

    /// What `--log-focus` says of an open All Direct Messages or conversation (`, flyout Conversation ReplyBox;
    /// draft 12 chars`): places and counts only.
    pub fn direct_flyout_line(&self) -> Option<String> {
        self.dms.line()
    }

    /// Whether All Direct Messages or a conversation is open over the panel.
    pub fn direct_open(&self) -> bool {
        self.dms.view.is_some()
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
        self.entries = entries::of(self.tab, around, &self.discord, &self.dms.direct);
        self.listed_discord = self.discord.clone();
        self.listed_direct = self.dms.direct.clone();
        // Only the landing tab has Utilities.
        self.utilities = if self.tab == Tab::Landing { Utility::LANDING.to_vec() } else { Vec::new() };
    }

    /// What is around changed while the Guide is open (a game started or stopped, a new snapshot of the library):
    /// the lists are made again, and focus stays at the same place in them.
    /// On the Discord tab focus follows its entry through Discord's changes (`discord::focus_after`).
    pub fn refresh(&mut self, around: Around) {
        let before = (
            self.tab,
            std::mem::take(&mut self.entries),
            std::mem::take(&mut self.listed_discord),
            std::mem::take(&mut self.listed_direct),
        );
        self.list(around);
        if let Focus::Entry(at) = self.focus {
            self.focus = Focus::Entry(if before.0 == Tab::Discord && self.tab == Tab::Discord {
                discord::focus_after(
                    (&before.1, &before.2, &before.3),
                    (&self.entries, &self.discord, &self.listed_direct),
                    at,
                )
            } else {
                at.min(self.entries.len().saturating_sub(1))
            });
        }

        self.remember_row();
        // A menu or a prompt is about the running game: when that has stopped it goes, in silence; a Shortcut's,
        // when the Shortcut has. Power is about the PC, and stays.
        let gone = match &self.shortcut_menu {
            Some(id) => self.tab != Tab::Discord || !self.discord.shortcuts.iter().any(|s| s.id == *id),
            None => around.playing.is_none(),
        };
        if self.direct_menu.is_some() {
            if !self.direct_menu_alive() {
                self.dialog = None;
                self.direct_menu = None;
            }
        } else if matches!(self.dialog, Some(Dialog::Menu(_))) && gone {
            self.dialog = None;
            self.shortcut_menu = None;
        }
        // An open Flyout follows what it shows: the notifications, the devices, Discord's servers; the Discord
        // Flyout closes as Discord closes it, or with its tab.
        if let Some(flyout) = &mut self.flyout {
            let tab_gone = flyout.which == Flyout::Discord && self.tab != Tab::Discord;
            if tab_gone || !flyout.follow(around, &self.discord) {
                self.flyout = None;
            }
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
        // All Direct Messages or a conversation takes the pad: B goes back to where it was opened from.
        if self.dms.view.is_some() {
            if press.button == Button::Guide {
                return Some(GuideAction::Close);
            }
            return match self.dms.press(press) {
                direct::Step::Menu(target) => {
                    self.open_direct_menu(target);
                    Some(GuideAction::Asked)
                }
                direct::Step::Chose => Some(GuideAction::Asked),
                direct::Step::Moved | direct::Step::Left | direct::Step::Nothing => None,
            };
        }
        // An open Flyout takes the pad: B closes a dropdown's list, then the Flyout, focus back on its Utility.
        // The bumpers do nothing meanwhile.
        if let Some(flyout) = &mut self.flyout {
            if press.button == Button::Guide {
                return Some(GuideAction::Close);
            }
            return match flyout.press(press, around, &self.discord) {
                flyout::Outcome::Chose(action) => Some(action),
                flyout::Outcome::Close => {
                    self.flyout = None;
                    None
                }
                flyout::Outcome::CloseChose(action) => {
                    self.flyout = None;
                    Some(action)
                }
                flyout::Outcome::Nothing => None,
            };
        }
        // On the Discord tab the Shortcuts' tiles stand in rows.
        if let (Tab::Discord, Focus::Entry(at)) = (self.tab, self.focus) {
            if let Some(to) = discord::grid_move(&self.entries, at, press.button, self.row_memory) {
                self.focus = Focus::Entry(to);
                self.remember_row();
                self.scroll_to_focus();
                return None;
            }
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
            (Button::A, Focus::Entry(at)) if self.direct_item(at).is_some() => return self.select_direct(at),
            (Button::A, Focus::Entry(at)) => {
                let action = self.entries.get(at).and_then(select);
                if action == Some(GuideAction::Asked) {
                    self.ask_power();
                }
                // Servers opens its Flyout at once, a spinner in it until Discord answers.
                if action == Some(GuideAction::Discord(DiscordAsk::Servers)) {
                    self.flyouts += 1;
                    self.flyout = Some(flyout::State::new(Flyout::Discord, self.flyouts, &self.discord));
                }
                return action;
            }
            (Button::A, Focus::Utility(at)) => {
                return match self.utilities.get(at)? {
                    Utility::Search => Some(GuideAction::Search),
                    Utility::Downloads => Some(GuideAction::ManageQueue),
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
            // A Shortcut's menu: its name, and Remove shortcut.
            (Button::Menu, Focus::Entry(at)) if self.entries.get(at).is_some_and(Entry::has_menu) => {
                if let Kind::Discord(DiscordItem::DM(place)) = self.entries[at].kind {
                    if let Some(dm) = self.dms.direct.dms.get(place) {
                        self.open_direct_menu(direct::Target::Dm(dm.id.clone()));
                    }
                } else if let Kind::Discord(DiscordItem::Shortcut(shortcut)) = self.entries[at].kind {
                    let rows = vec![Row::new(crate::theme::glyph::DELETE, REMOVE_SHORTCUT, MenuChoice::RemoveShortcut)];
                    self.ask(MenuPanel::menu(&self.entries[at].title, rows));
                    self.shortcut_menu = self.discord.shortcuts.get(shortcut).map(|s| s.id.clone());
                } else {
                    // The running game's menu: its name, and Quit.
                    let rows = vec![Row::new(menu_panel::icon::QUIT, "Quit", MenuChoice::QuitGame)];
                    self.ask(MenuPanel::menu(&self.entries[at].title, rows));
                }
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
        if self.direct_menu.is_some() {
            return self.answer_direct(press);
        }
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
                Answer::Chose(MenuChoice::RemoveShortcut) => {
                    let id = self.shortcut_menu.as_deref();
                    let at = self.discord.shortcuts.iter().position(|s| Some(s.id.as_str()) == id);
                    at.map(|at| GuideAction::Discord(DiscordAsk::RemoveShortcut(at)))
                }
                Answer::Chose(MenuChoice::Dm(_)) => None,
                Answer::Chose(MenuChoice::Cancel) | Answer::Declined => (!self.panel).then_some(GuideAction::Close),
                Answer::Moved | Answer::Nothing => return None,
            },
        };
        self.dialog = None;
        self.shortcut_menu = None;
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
                if self.flyout.is_none() && self.dms.view.is_none() {
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
                    // A conversation or All Direct Messages stays under its menu, out of focus with the panel.
                    if self.dms.view.is_some() {
                        let flyout = flyout::FLYOUT.grow(BEHIND_BLUR);
                        let bounds =
                            Rect::new(bounds.x, bounds.y, flyout.right() - bounds.x, flyout.bottom() - bounds.y);
                        ui.group(bounds, BEHIND_BLUR, 1.0, |ui| self.dms.draw(ui, self.writing, true));
                    }
                } else {
                    self.panel(ui, behind, around, face);
                    if let Some(flyout) = &mut self.flyout {
                        flyout.draw(ui, around, &self.discord);
                    }
                    self.dms.draw(ui, self.writing, false);
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

    /// The Direct Messages item the entry at `at` is, if it is one A has its own use for.
    fn direct_item(&self, at: usize) -> Option<DiscordItem> {
        match self.entries.get(at)?.kind {
            Kind::Discord(
                item @ (DiscordItem::DM(_) | DiscordItem::AllDMs | DiscordItem::Answer | DiscordItem::DMsState(_)),
            ) => Some(item),
            _ => None,
        }
    }

    /// A on a disc, the sixth circle, Answer or the section's state.
    fn select_direct(&mut self, at: usize) -> Option<GuideAction> {
        match self.direct_item(at)? {
            DiscordItem::DM(place) => {
                let id = self.dms.direct.dms.get(place)?.id.clone();
                self.dms.open_chat(id, Back::Tab);
                Some(GuideAction::Asked)
            }
            DiscordItem::AllDMs => {
                let focused = self.last_dm.clone();
                self.dms.open_all(focused.as_ref());
                Some(GuideAction::Asked)
            }
            DiscordItem::Answer => {
                let channel = self.dms.direct.rings.first()?.channel.clone();
                self.dms.asks.push(DirectAsk::Answer(channel));
                Some(GuideAction::Asked)
            }
            DiscordItem::DMsState(DirectNote::SignedOut) => Some(GuideAction::DirectSettings),
            _ => None,
        }
    }

    /// Remembers the disc focus is on, for Up from the entry under the row.
    fn remember_row(&mut self) {
        let (Focus::Entry(at), Some((first, count))) = (self.focus, discord::dm_row(&self.entries)) else { return };
        if (first..first + count).contains(&at) {
            self.row_memory = at - first;
            if let Kind::Discord(DiscordItem::DM(place)) = self.entries[at].kind {
                self.last_dm = self.dms.direct.dms.get(place).map(|dm| dm.id.clone());
            }
        }
    }

    /// A DM's, a message's or Add Reaction's menu over the Guide (docs/spec/party-chats.md, 3.6 and 6.3).
    fn open_direct_menu(&mut self, target: direct::Target) {
        let menu = match &target {
            direct::Target::Dm(id) => {
                let Some(dm) = self.dms.direct.dm(id) else { return };
                let mut rows = Vec::new();
                if dm.unread > 0 {
                    rows.push(Row::new('\u{E8FB}', "Mark As Read", MenuChoice::Dm(DmRow::MarkRead)));
                }
                if self.dms.direct.state == DirectState::Ready {
                    rows.push(Row::new('\u{E717}', "Call", MenuChoice::Dm(DmRow::Call)));
                }
                rows.push(Row::new(menu_panel::icon::CANCEL, "Cancel", MenuChoice::Cancel));
                MenuPanel::menu(dm.name.as_str(), rows)
            }
            direct::Target::Message(id) => {
                let Some(chat) = self.dms.chat() else { return };
                let Some(message) = self.dms.message(&chat.id, id) else { return };
                let mut rows = vec![
                    Row::new('\u{E97A}', "Reply", MenuChoice::Dm(DmRow::Reply)),
                    Row::new('\u{E76E}', "Add Reaction", MenuChoice::Dm(DmRow::AddReaction)),
                ];
                if message.own && message.delivery == model::discord_dms::Delivery::Failed {
                    rows.push(Row::new('\u{E72C}', "Retry", MenuChoice::Dm(DmRow::Retry)));
                    rows.push(Row::new('\u{E74D}', "Delete", MenuChoice::Dm(DmRow::Delete)));
                }
                rows.push(Row::new(menu_panel::icon::CANCEL, "Cancel", MenuChoice::Cancel));
                MenuPanel::menu(&direct::menu_title(&Dms::text_of(&message)), rows)
            }
            direct::Target::Reaction(id) => {
                let Some(chat) = self.dms.chat() else { return };
                let Some(message) = self.dms.message(&chat.id, id) else { return };
                let mine = |emoji: &str| match &message.body {
                    model::discord_dms::Body::Said { reactions, .. } => reactions.iter().any(|reaction| {
                        reaction.mine
                            && matches!(&reaction.emoji, model::discord_dms::Emoji::Unicode(e) if e.as_str() == emoji)
                    }),
                    _ => false,
                };
                let rows = REACTIONS
                    .iter()
                    .enumerate()
                    .map(|(at, (emoji, name))| {
                        let icon = emoji.chars().next().unwrap_or(' ');
                        let label = if mine(emoji) { format!("Remove {name}") } else { (*name).to_string() };
                        Row::new(icon, &label, MenuChoice::Dm(DmRow::React(at)))
                    })
                    .collect();
                MenuPanel::menu("Add Reaction", rows)
            }
        };
        self.ask(menu);
        self.direct_menu = Some(target);
    }

    /// A press while a Direct Messages menu is up. Add Reaction takes the message's menu's place, and B on it goes
    /// back there.
    fn answer_direct(&mut self, press: Press) -> Option<GuideAction> {
        let Some(Dialog::Menu(menu)) = self.dialog.as_mut() else { return None };
        let target = self.direct_menu.clone()?;
        let channel = self.dms.chat().map(|chat| chat.id.clone());
        let row = match menu.press(press) {
            Answer::Moved | Answer::Nothing => return None,
            Answer::Chose(MenuChoice::Dm(row)) => Some(row),
            Answer::Chose(_) | Answer::Declined => None,
        };
        let done = |me: &mut Self| {
            me.dialog = None;
            me.direct_menu = None;
        };
        let Some(row) = row else {
            // Cancel or B: Add Reaction goes back to the message's menu; the others close.
            return match target {
                direct::Target::Reaction(id) => {
                    self.open_direct_menu(direct::Target::Message(id));
                    None
                }
                _ => {
                    done(self);
                    None
                }
            };
        };
        match (row, target) {
            (DmRow::MarkRead, direct::Target::Dm(id)) => self.dms.asks.push(DirectAsk::MarkRead(id)),
            (DmRow::Call, direct::Target::Dm(id)) => self.dms.asks.push(DirectAsk::Call(id)),
            (DmRow::Reply, direct::Target::Message(id)) => {
                if let Some(channel) = &channel {
                    self.dms.reply_to(channel, &id);
                }
            }
            (DmRow::AddReaction, direct::Target::Message(id)) => {
                self.open_direct_menu(direct::Target::Reaction(id));
                return None;
            }
            (DmRow::Retry, direct::Target::Message(id)) => {
                if let Some(channel) = &channel {
                    self.dms.retry(channel, &id);
                }
            }
            (DmRow::Delete, direct::Target::Message(id)) => {
                if let Some(channel) = &channel {
                    self.dms.forget(channel, &id);
                }
            }
            (DmRow::React(which), direct::Target::Reaction(id)) => {
                if let Some(channel) = &channel {
                    self.dms.react(channel, &id, which);
                }
            }
            _ => {}
        }
        done(self);
        Some(GuideAction::Asked)
    }

    /// The panel and what is on it, which is `behind` to its left while it comes in.
    fn panel(&self, ui: &mut Ui, behind: f32, around: Around, face: &Face) {
        panel::background(ui);
        ui.with_offset(behind, 0.0, |ui| {
            panel::tabs(ui, &self.tabs, self.tab, &face.profile.picture, self.listed_direct.unread().1 > 0);
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
                let focused_item = match focus.and_then(|at| self.entries.get(at)) {
                    Some(Entry { kind: Kind::Discord(item), .. }) => Some(item.to_owned()),
                    _ => None,
                };
                let faces = entries::Faces {
                    library: around.library,
                    profile: face.profile,
                    discord: &self.listed_discord,
                    direct: discord::DirectFace {
                        direct: &self.listed_direct,
                        focus: focused_item,
                        loading_for: self.dms.loading_since.map(|since| self.dms.age - since),
                    },
                };
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
                // Notifications' badge counts what the Flyout lists, or what the shell says there is, whichever is
                // more; Downloads' the games in the Queue.
                let counts = utilities::Counts {
                    notifications: face.status.notifications.max(around.notifications.len() as u32),
                    queued: around.library.queue.len() as u32,
                };
                utilities::draw(ui, &self.utilities, focus, counts);
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
