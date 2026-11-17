//! Which folder: the small question an Install asks when the game's store can put it in more than one folder
//! (docs/spec/storage.md, section 4; #179), in the console's manner: a prompt in the middle of the screen over the
//! dimmed page, a row a folder, focus on the folder installs go to unless the user says otherwise (Settings >
//! Storage), the focused folder's free space under the question. A installs there; B installs nothing.
//!
//! It opens in the place of whatever asked (the Tile menu's Install, the Game hub's), through
//! [`crate::tile_menu::install`], which asks it only where the library's [`model::Storage`] says the store has more
//! than one folder and can be told which.
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use model::storage::{Place, StoreFolders};
use model::{Game, GameKey, Library};
use ui::{Press, Ui};

use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, Open, Page, PageAction};
use crate::Effect;

/// How the question is opened: for which game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    game: GameKey,
}

/// The question for `game`, in the place of what asked.
pub fn open(game: &Game) -> PageAction {
    PageAction::Replace(Open::InstallWhere(Entry { game: game.key() }))
}

/// The game's store's folders, where its Install asks which.
pub fn asks<'a>(game: &Game, library: &'a Library) -> Option<&'a StoreFolders> {
    library.storage.of(&game.source).filter(|folders| folders.asks())
}

/// A folder's row, short enough for the panel: its own name and its drive's letter, `SteamLibrary (D:)`.
pub fn row_label(place: &Place) -> String {
    const MOST: usize = 22;
    let path = place.path.to_string_lossy().replace('/', "\\");
    let trimmed = path.trim_end_matches('\\');
    let letter = trimmed.split('\\').next().filter(|first| first.ends_with(':'));
    let name = trimmed.rsplit('\\').next().unwrap_or(trimmed);
    let name = if name.chars().count() > MOST {
        format!("{}...", name.chars().take(MOST - 3).collect::<String>())
    } else {
        name.to_string()
    };
    match letter {
        Some(letter) if letter != name => format!("{name} ({letter})"),
        _ => name,
    }
}

/// What the question says under itself of the focused folder: its free space, where its drive could be read.
fn message(place: Option<&Place>) -> Option<String> {
    place.and_then(|place| place.drive.as_ref()).map(|drive| drive.free_text())
}

static QUESTIONS: AtomicU32 = AtomicU32::new(0);

pub struct InstallWhere {
    game: GameKey,
    name: String,
    folders: Vec<Place>,
    /// The folder installs go to unless the user says otherwise, by its place.
    default: usize,
    panel: MenuPanel<usize>,
    serial: u32,
}

impl InstallWhere {
    /// The question for the game as the library has it now. None for a game that has gone, or whose store no
    /// longer asks.
    pub fn open(entry: Entry, around: Around) -> Option<Self> {
        let game = around.library.games.iter().chain(&around.library.hidden).find(|game| game.is(&entry.game))?;
        let store = asks(game, around.library)?;
        let rows = store.folders.iter().enumerate().map(|(at, place)| Row::new(icon::INSTALL, &row_label(place), at));
        let title = format!("Where do you want to install {}?", game.name);
        let default = store.default.min(store.folders.len() - 1);
        let mut panel = MenuPanel::prompt(&title, None, rows.collect());
        panel.set_focus(default);
        panel.set_message(message(store.folders.get(default)).as_deref());
        Some(Self {
            game: entry.game,
            name: game.name.clone(),
            folders: store.folders.clone(),
            default,
            panel,
            serial: QUESTIONS.fetch_add(1, Ordering::Relaxed),
        })
    }

    fn flight(&self, ui: &mut Ui, leaving: bool) -> (f32, f32) {
        menu_panel::flight(ui, ("install where", self.serial), leaving)
    }

    /// Install into `folder`, as it is chosen: the game still not installed and not already in the Queue.
    fn install(&self, folder: &Path, library: &Library) -> PageAction {
        let game = library.games.iter().chain(&library.hidden).find(|game| game.is(&self.game));
        match game {
            Some(game) if !game.installed && game.id != 0 && library.queue.get(game.id).is_none() => {
                PageAction::CloseWith(Effect::InstallTo { game: game.id, folder: folder.to_path_buf() })
            }
            _ => PageAction::Close,
        }
    }
}

impl Page for InstallWhere {
    fn name(&self) -> &'static str {
        "InstallWhere"
    }

    /// The focused folder, by its place and path, then the game: `Folder(1) D:\SteamLibrary of Halyard Point`, and
    /// `, the default` on the folder installs go to unless the user says otherwise.
    fn focus(&self) -> String {
        let at = self.panel.focus();
        let path = self.folders.get(at).map(|place| place.path.display().to_string()).unwrap_or_default();
        let default = if at == self.default { ", the default" } else { "" };
        format!("Folder({at}) {path}{default} of {}", self.name)
    }

    fn dialog(&self) -> Option<Dialog> {
        Some(Dialog::Prompt)
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        match self.panel.press(press) {
            Answer::Chose(at) => {
                let folder = self.folders.get(at)?.path.clone();
                Some(self.install(&folder, around.library))
            }
            Answer::Declined => Some(PageAction::Close),
            Answer::Moved => {
                let shown = message(self.folders.get(self.panel.focus()));
                self.panel.set_message(shown.as_deref());
                None
            }
            Answer::Nothing => None,
        }
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), DIM, 0.0);
        let (fly, shown) = self.flight(ui, false);
        self.panel.draw_flown(ui, fly, shown);
    }

    fn draw_leaving(&mut self, ui: &mut Ui, _around: Around) -> bool {
        let (fly, shown) = self.flight(ui, true);
        if shown <= 0.0 || fly >= FLY {
            return false;
        }
        self.panel.draw_flown(ui, fly, shown);
        true
    }

    fn covers(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(path: &str) -> Place {
        Place { path: path.into(), drive: None }
    }

    #[test]
    fn a_row_names_the_folder_and_its_drive_short_enough_for_the_panel() {
        assert_eq!(row_label(&place(r"D:\SteamLibrary")), "SteamLibrary (D:)");
        assert_eq!(row_label(&place(r"C:\Program Files (x86)\Steam\")), "Steam (C:)");
        assert_eq!(row_label(&place("E:/Games")), "Games (E:)");
        assert_eq!(row_label(&place(r"F:\")), "F:");
        assert_eq!(row_label(&place(r"D:\A folder with a very long name indeed")), "A folder with a ver... (D:)");
        assert_eq!(row_label(&place("/home/games")), "games");
    }
}
