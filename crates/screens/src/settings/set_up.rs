//! Games & apps > Set up this PC (docs/spec/set-up.md, #178): the launchers' own apps, installed and removed from
//! here. A Row an app, as Launchers' Rows: its launcher's mark, its name, and how it stands on this PC. A on an app
//! that is not installed asks first, the vendor, the address and the size in the question, and only Install
//! downloads, checks and runs its installer; on an installed one it asks first too, with what removing it does to
//! its games, and only Remove runs its own uninstaller. Windows' permission prompt, which the controller can't answer,
//! is said in both questions. The Bundle has nothing of the kind: the page and its words are the port's.
//!
//! The page shows the apps as the last snapshot has them (`Library::clients`) and asks with `Effect::SetUp`.
use std::sync::atomic::{AtomicU32, Ordering};

use model::setup::{self, Client, ClientState, SetUpAsk};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, BOXES_TOP, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use super::{Pane, Step};
use crate::home::{self, Source};
use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

/// The box on Games & apps, and the page's header.
pub const TITLE: &str = "Set up this PC";
const ABOUT: &str = "Install the apps your games come from, or remove them. Each app's installer comes from its \
                     maker's own address and is checked before it runs. Windows asks for permission on screen \
                     each time: answer it with a mouse or keyboard, as the controller can't.";
const NONE: &str = "There are no apps to set up.";
const INSTALL: &str = "Install";
const REMOVE: &str = "Remove";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Yes,
    No,
}

/// The question up, of the app focus is on.
struct Question {
    key: String,
    remove: bool,
    panel: MenuPanel<Reply>,
}

static FLIGHTS: AtomicU32 = AtomicU32::new(0);

/// The launcher's mark, as the tiles and Launchers' Rows draw it.
fn mark(key: &str) -> Option<Source> {
    home::source(&model::Game { source: key.to_string(), ..model::Game::default() }).filter(|_| !key.is_empty())
}

/// The address without its scheme, as it is said.
fn address(client: &Client) -> &str {
    client.address.trim_start_matches("https://").trim_start_matches("http://")
}

/// What a Row says of its app, beside its name.
pub fn status(client: &Client) -> String {
    match &client.state {
        ClientState::NotInstalled => "Not installed".into(),
        ClientState::Installed => "Installed".into(),
        ClientState::InstalledFixed => "Installed".into(),
        ClientState::Downloading { done } => {
            format!("Downloading: {}%", (done.saturating_mul(100) / client.size.max(1)).min(100))
        }
        ClientState::Installing => "Installing...".into(),
        ClientState::Removing => "Removing...".into(),
        ClientState::Failed { installed: true, .. } => "Installed".into(),
        ClientState::Failed { installed: false, .. } => "Not installed".into(),
    }
}

/// The installed games of the app's launcher, and their count as it is said.
fn games(around: Around, key: &str) -> usize {
    around
        .library
        .games
        .iter()
        .chain(&around.library.hidden)
        .filter(|game| game.source == key && game.installed)
        .count()
}

fn count(n: usize, name: &str) -> String {
    match n {
        0 => format!("No {name} games are installed."),
        1 => format!("1 {name} game is installed."),
        n => format!("{n} {name} games are installed."),
    }
}

/// The launcher's name for its games: `Steam`, `Epic Games`.
fn games_of(key: &str) -> &'static str {
    mark(key).map_or("its", Source::name)
}

/// The line under the Rows, of the app focus is on: what A does to it, or why the last thing asked failed.
fn detail(client: &Client, around: Around) -> String {
    match &client.state {
        ClientState::Failed { why, .. } => why.clone(),
        ClientState::NotInstalled => format!(
            "A installs {} {} from {}'s own address ({}).",
            client.name,
            client.version,
            client.vendor,
            setup::size(client.size)
        ),
        ClientState::Installed => {
            format!("{} {}", count(games(around, &client.key), games_of(&client.key)), client.removing)
        }
        ClientState::InstalledFixed => "Windows lists no uninstaller for it: remove it in Windows' Settings.".into(),
        ClientState::Downloading { .. } => {
            format!("Downloading its installer from {}'s own address and checking it.", client.vendor)
        }
        ClientState::Installing => "Its installer is running. Answer Windows' prompt with a mouse or keyboard if one \
                                    shows."
            .into(),
        ClientState::Removing => "Its uninstaller is running. Answer Windows' prompt with a mouse or keyboard if one \
                                  shows."
            .into(),
    }
}

/// The question Install asks: the vendor, the address and the size in front of the user before anything is
/// downloaded, and Windows' prompt said.
fn install_question(client: &Client) -> MenuPanel<Reply> {
    let rows = vec![Row::new(icon::INSTALL, INSTALL, Reply::Yes), Row::new(icon::CANCEL, "Cancel", Reply::No)];
    MenuPanel::prompt(&format!("Install {}?", client.name), Some(&install_message(client)), rows)
}

fn install_message(client: &Client) -> String {
    let window = if client.window { " Its installer shows a window of its own." } else { "" };
    format!(
        "The Dashboard downloads {}'s installer ({}) from {}'s own address, {}, checks it and runs it. Windows \
         then asks for permission on screen: answer it with a mouse or keyboard, as the controller can't.{window} \
         Once it's installed, sign in to {} in its own window.",
        client.name,
        setup::size(client.size),
        client.vendor,
        address(client),
        client.name
    )
}

/// The question Remove asks: what happens to its games, and Windows' prompt said.
fn remove_question(client: &Client, games: usize) -> MenuPanel<Reply> {
    let rows = vec![Row::new(icon::UNINSTALL, REMOVE, Reply::Yes), Row::new(icon::CANCEL, "Cancel", Reply::No)];
    MenuPanel::prompt(&format!("Remove {}?", client.name), Some(&remove_message(client, games)), rows)
}

fn remove_message(client: &Client, games: usize) -> String {
    let window = if client.uninstall_window { ", in a window of its own" } else { "" };
    format!(
        "{} {} {}'s own uninstaller runs{window}; Windows may ask for permission on screen, which needs a mouse or \
         keyboard.",
        count(games, games_of(&client.key)),
        client.removing,
        client.name
    )
}

pub struct SetUp {
    clients: Vec<Client>,
    at: usize,
    question: Option<Question>,
    /// Why the last A did nothing, said under the Rows until focus moves.
    note: Option<String>,
    flying: u32,
    leaving: Option<(MenuPanel<Reply>, u32)>,
    scroll: f32,
}

impl SetUp {
    pub fn open(around: Around) -> Self {
        let clients = around.library.clients.clone();
        Self { clients, at: 0, question: None, note: None, flying: 0, leaving: None, scroll: 0.0 }
    }

    fn close_question(&mut self) {
        if let Some(question) = self.question.take() {
            self.leaving = Some((question.panel, self.flying));
        }
    }

    /// Why the app can't be removed now, if it can't: a game of its launcher playing or installing.
    fn held(client: &Client, around: Around) -> Option<String> {
        let playing = around.playing.and_then(|game| around.library.games.get(game));
        if let Some(game) = playing.filter(|game| game.source == client.key) {
            return Some(format!("Quit {} before removing {}.", game.name, client.name));
        }
        let installing = around.library.queue.entries.iter().filter_map(|entry| around.library.game(entry.game));
        installing.into_iter().find(|game| game.source == client.key).map(|game| {
            format!("{} is installing: let it finish, or cancel it, before removing {}.", game.name, client.name)
        })
    }

    /// The Rows' top, under the description: as drawn.
    fn rows_top(ui: &mut Ui) -> f32 {
        BOXES_TOP + ui.measure_wrapped(&BODY, ABOUT, ROW_WIDTH, 0).height + 26.0
    }
}

impl Pane for SetUp {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `Set up this PC: Steam`, or `Set up this PC: Nothing`.
    fn focus(&self) -> String {
        match self.clients.get(self.at) {
            Some(client) => format!("{TITLE}: {}", client.name),
            None => format!("{TITLE}: Nothing"),
        }
    }

    /// How the focused app stands (`not installed`, `installed`, `downloading`, `installing`, `removing`,
    /// `failed: <why>`), then the question while one is up, or why the last A did nothing.
    fn state(&self) -> String {
        let Some(client) = self.clients.get(self.at) else { return String::new() };
        let state = match &client.state {
            ClientState::NotInstalled => "not installed".to_string(),
            ClientState::Installed => "installed".into(),
            ClientState::InstalledFixed => "installed, no uninstaller".into(),
            ClientState::Downloading { .. } => "downloading".into(),
            ClientState::Installing => "installing".into(),
            ClientState::Removing => "removing".into(),
            ClientState::Failed { why, installed } => {
                format!("{}; failed: {why}", if *installed { "installed" } else { "not installed" })
            }
        };
        match (&self.question, &self.note) {
            (Some(question), _) => format!("{state}; asking {}", question.panel.title()),
            (None, Some(note)) => format!("{state}; refused: {note}"),
            (None, None) => state,
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if let Some(question) = self.question.as_mut() {
            let (key, remove) = (question.key.clone(), question.remove);
            match question.panel.press(press) {
                Answer::Chose(Reply::Yes) => {
                    self.close_question();
                    let ask = if remove { SetUpAsk::Remove(key) } else { SetUpAsk::Install(key) };
                    return Step::Do(PageAction::Effect(Effect::SetUp(ask)));
                }
                Answer::Chose(Reply::No) | Answer::Declined => self.close_question(),
                Answer::Moved | Answer::Nothing => {}
            }
            return Step::Stay;
        }
        match press.button {
            Button::B | Button::Left if !press.repeat => Step::Back,
            Button::Up | Button::Down => {
                let to = if press.button == Button::Up {
                    self.at.saturating_sub(1)
                } else {
                    (self.at + 1).min(self.clients.len().saturating_sub(1))
                };
                if to != self.at {
                    self.note = None;
                }
                self.at = to;
                Step::Stay
            }
            Button::A if !press.repeat => {
                let Some(client) = self.clients.get(self.at) else { return Step::Stay };
                self.note = None;
                if let Some(busy) = self.clients.iter().find(|other| other.state.busy()) {
                    if busy.key != client.key {
                        self.note = Some(format!("Wait until {} is done.", busy.name));
                    }
                    return Step::Stay;
                }
                let panel = match &client.state {
                    ClientState::InstalledFixed => {
                        self.note = Some("Windows lists no uninstaller for it.".into());
                        return Step::Stay;
                    }
                    state if state.installed() => {
                        if let Some(why) = Self::held(client, around) {
                            self.note = Some(why);
                            return Step::Stay;
                        }
                        remove_question(client, games(around, &client.key))
                    }
                    _ => install_question(client),
                };
                let remove = client.state.installed();
                self.question = Some(Question { key: client.key.clone(), remove, panel });
                self.flying = FLIGHTS.fetch_add(1, Ordering::Relaxed);
                self.leaving = None;
                Step::Stay
            }
            _ => Step::Stay,
        }
    }

    fn dialog(&self) -> Option<Dialog> {
        self.question.as_ref().map(|_| Dialog::Prompt)
    }

    fn dismiss_dialog(&mut self) {
        self.question = None;
        self.leaving = None;
    }

    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        let key = self.clients.get(self.at).map(|client| client.key.clone());
        self.clients = around.library.clients.clone();
        self.at = key
            .and_then(|key| self.clients.iter().position(|client| client.key == key))
            .unwrap_or(self.at)
            .min(self.clients.len().saturating_sub(1));
        None
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let rows_top = Self::rows_top(ui);
        let row = |i: usize| Rect::new(LEFT, rows_top + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT);
        let below = rows_top + ROW_PITCH * self.clients.len().saturating_sub(1) as f32 + ROW_HEIGHT + 30.0;
        let focused_rect = row(self.at);
        let detail_text = match (&self.note, self.clients.get(self.at)) {
            (Some(note), _) => Some((note.clone(), Color::WHITE)),
            (None, Some(client)) => Some((detail(client, around), TEXT_DIM)),
            (None, None) => None,
        };
        let detail_height =
            detail_text.as_ref().map_or(0.0, |(text, _)| ui.measure_wrapped(&BODY, text, ROW_WIDTH, 0).height);
        let Self { clients, at, scroll, .. } = self;
        controls::scrolled(
            ui,
            scroll,
            BOXES_TOP,
            focused_rect.y,
            focused_rect.bottom().max(below + detail_height),
            |ui| {
                controls::description(ui, LEFT, BOXES_TOP, ROW_WIDTH, ABOUT);
                if clients.is_empty() {
                    controls::description(ui, LEFT, rows_top, ROW_WIDTH, NONE);
                }
                // The focused Row last: its ring and glow lie over its neighbours.
                let has = |i: usize| focused && i == *at;
                for (i, client) in clients.iter().enumerate().filter(|(i, _)| !has(*i)) {
                    client_row(ui, row(i), client, false);
                }
                for (i, client) in clients.iter().enumerate().filter(|(i, _)| has(*i)) {
                    client_row(ui, row(i), client, true);
                }
                if let Some((text, colour)) = &detail_text {
                    let rect = Rect::new(LEFT, below, ROW_WIDTH, 2000.0);
                    ui.paragraph(rect, text, &BODY.color(*colour), HAlign::Left, VAlign::Top, 0);
                }
            },
        );
        // The question over the dimmed page; or on its way out, the dim gone at once.
        if let Some(question) = &self.question {
            ui.layer();
            ui.fill(ui.screen(), DIM, 0.0);
            let (fly, shown) = menu_panel::flight(ui, ("set-up question", self.flying), false);
            question.panel.draw_flown(ui, fly, shown);
        } else if let Some((panel, flying)) = &self.leaving {
            ui.layer();
            let (fly, shown) = menu_panel::flight(ui, ("set-up question", *flying), true);
            if shown <= 0.0 || fly >= FLY {
                self.leaving = None;
            } else {
                panel.draw_flown(ui, fly, shown);
            }
        }
    }
}

/// An app's Row: a Row (settings.md 4.3) with its launcher's mark in the tick's slot, its name, and how it stands
/// right-aligned at its end, a spinner before it while something is under way.
fn client_row(ui: &mut Ui, rect: Rect, client: &Client, focused: bool) {
    controls::row_marked(ui, rect, &client.name, None, focused);
    let (_, cy) = rect.center();
    if let Some(source) = mark(&client.key) {
        home::source_mark(ui, Rect::new(rect.x + 45.0 - 14.0, cy - 14.0, 28.0, 28.0), source);
    }
    let status_box = Rect::new(rect.x + 340.0, rect.y, rect.w - 340.0 - 24.0, rect.h);
    let text = status(client);
    if client.state.busy() {
        let width = ui.measure(&BODY, &text).width;
        crate::spinner::spinner(ui, status_box.right() - width - 24.0, cy, 28.0);
    }
    ui.paragraph(status_box, &text, &BODY.color(TEXT_DIM), HAlign::Right, VAlign::Center, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(state: ClientState) -> Client {
        Client {
            key: "steam".into(),
            name: "Steam".into(),
            vendor: "Valve".into(),
            version: "2.10.91.91".into(),
            address: "https://cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe".into(),
            size: 2_380_800,
            window: false,
            uninstall_window: false,
            removing: "Steam's uninstaller deletes the games in its own folder with it.".into(),
            state,
        }
    }

    #[test]
    fn the_install_question_names_the_vendor_the_address_the_size_and_windows_prompt() {
        let question = install_question(&client(ClientState::NotInstalled));
        assert_eq!(question.title(), "Install Steam?");
        let message = install_message(&client(ClientState::NotInstalled));
        for said in [
            "Valve's own address",
            "cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe",
            "2.4 MB",
            "mouse or keyboard",
        ] {
            assert!(message.contains(said), "{said}: {message}");
        }
        assert_eq!(question.rows()[0].label, INSTALL);
    }

    #[test]
    fn the_remove_question_says_what_happens_to_the_games() {
        let question = remove_question(&client(ClientState::Installed), 8);
        assert_eq!(question.title(), "Remove Steam?");
        let message = remove_message(&client(ClientState::Installed), 8);
        assert!(message.starts_with("8 Steam games are installed. Steam's uninstaller deletes"), "{message}");
        assert_eq!(question.rows()[0].label, REMOVE);
    }

    #[test]
    fn a_row_says_how_its_app_stands() {
        assert_eq!(status(&client(ClientState::NotInstalled)), "Not installed");
        assert_eq!(status(&client(ClientState::Downloading { done: 1_190_400 })), "Downloading: 50%");
        assert_eq!(status(&client(ClientState::Installing)), "Installing...");
        assert_eq!(status(&client(ClientState::Failed { why: "x".into(), installed: true })), "Installed");
    }
}
