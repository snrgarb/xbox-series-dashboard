//! Games: the installed games in the grid, by name, and under the grid's end the way on to Full library
//! (docs/spec/my-games.md, section 5).
use model::{Game, Library};
use ui::{Button, Press, Rect, Ui};

use super::grid::{self, Grid, Step};
use super::look::{self, BUTTON, LINE, TEXT};
use super::nav::Item;
use super::{open_game_hub, open_tile_menu, Look, Outcome, Pane};
use crate::hint_bar::{self, Hint};
use crate::pages::{Around, Open, PageAction};

/// "Looking for more?": 3 right of the content's edge, 15 under the last row's tiles; the button 12 under it, 52
/// tall, its label 26 in from each side.
const MORE_LEFT: f32 = 523.0;
const MORE_UNDER: f32 = 15.0;
const BUTTON_UNDER: f32 = 12.0;
const BUTTON_HEIGHT: f32 = 52.0;
const BUTTON_SIDE: f32 = 26.0;
/// The grid with no games in it. Playnite's own line in the Bundle; its style is kept.
pub const EMPTY: &str = "No games found";

/// What a name sorts by, as the library database sorts: lower case, without a leading article.
pub fn sort_key(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let rest = ["the ", "a ", "an "].iter().find_map(|article| lower.strip_prefix(article));
    rest.map_or(lower.clone(), |rest| rest.trim_start().to_string())
}

/// The installed games that are not hidden, A to Z.
pub fn installed(library: &Library) -> Vec<&Game> {
    let mut games: Vec<&Game> = library.games.iter().filter(|game| game.installed).collect();
    games.sort_by_cached_key(|game| (sort_key(&game.name), game.name.clone(), game.id));
    games
}

pub struct Games {
    grid: Grid,
    /// Focus is on SEE FULL LIBRARY, under the grid. The grid keeps the game Down left, for Up.
    on_button: bool,
}

impl Games {
    pub fn new() -> Self {
        Self { grid: Grid::games(), on_button: false }
    }
}

impl Pane for Games {
    fn nav_count(&self, around: Around) -> Option<usize> {
        Some(around.library.games.iter().filter(|game| game.installed).count())
    }

    /// The header's count. It reads "games" for any number, one included, as the Bundle's does.
    fn count(&self, around: Around) -> Option<String> {
        self.nav_count(around).map(|games| format!("{games} games"))
    }

    fn enter(&mut self, around: Around) -> bool {
        self.on_button = false;
        self.grid.keep(&installed(around.library))
    }

    fn holds_focus(&mut self, around: Around) -> bool {
        self.grid.keep(&installed(around.library))
    }

    fn focus(&self) -> Option<String> {
        Some(if self.on_button { "SeeFullLibrary".into() } else { format!("Tile({})", self.grid.focus()) })
    }

    fn press(&mut self, press: Press, around: Around) -> Outcome {
        let games = installed(around.library);
        if !self.grid.keep(&games) {
            return Outcome::Nav;
        }
        let game = games[self.grid.focus()];
        let once = !press.repeat;
        match (self.on_button, press.button) {
            // SEE FULL LIBRARY: Up is the game Down came from, Left the nav; Right and Down are nothing.
            (true, Button::Up) => self.on_button = false,
            (true, Button::Left) => return Outcome::Nav,
            (true, Button::A) if once => return Outcome::Show(Item::FullLibrary),
            (true, _) => {}
            (false, button) if button.is_direction() => match self.grid.step(button, &games) {
                Step::Off(Button::Left) => return Outcome::Nav,
                Step::Off(Button::Down) => self.on_button = true,
                // Nothing is over the first row on Games.
                Step::Moved | Step::Stopped | Step::Off(_) => {}
            },
            // At once: the grid has no double tap to wait for.
            (false, Button::A) if once => return open_game_hub(game).map_or(Outcome::Stay, Outcome::Action),
            (false, Button::Menu | Button::X) if once => {
                return open_tile_menu(game).map_or(Outcome::Stay, Outcome::Action)
            }
            // Search, over the page: B there comes back here, focus on this game still.
            (false, Button::Y) if once => return Outcome::Action(PageAction::Open(Open::Search)),
            _ => {}
        }
        Outcome::Stay
    }

    fn leave(&mut self) {
        self.grid.forget();
        self.on_button = false;
    }

    fn hints(&self, _around: Around) -> &'static [Hint] {
        if self.on_button {
            hint_bar::NONE
        } else {
            hint_bar::MORE_OPTIONS_AND_SEARCH
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, look: Look) {
        let games = installed(around.library);
        let running = around.playing.and_then(|at| around.library.games.get(at)).map(grid::key);
        // The grid slides in with the pane but does not fade.
        self.grid.draw(ui, &games, look.focused && !self.on_button, running, EMPTY);
        // Under the grid's end, and only while its last row is wholly in view.
        let Some(end) = self.grid.end(ui, games.len()) else { return };
        ui.with_alpha(look.fade, |ui| {
            let top = end + MORE_UNDER;
            ui.label(MORE_LEFT, top, "Looking for more?", &TEXT);
            let label = "SEE FULL LIBRARY";
            let width = ui.measure(&TEXT, label).width + 2.0 * BUTTON_SIDE;
            let button = Rect::new(MORE_LEFT, top + LINE + BUTTON_UNDER, width, BUTTON_HEIGHT);
            if look.focused && self.on_button {
                look::slab(ui, button, 2.0);
            } else {
                ui.fill(button, BUTTON, 2.0);
            }
            ui.label_centered(button.center().0, button.center().1, label, &TEXT);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_are_the_installed_ones_by_name_without_case_or_article() {
        let game = |name: &str, installed| Game { name: name.into(), installed, ..Game::default() };
        let library = Library {
            games: vec![
                game("The Zephyr", true),
                game("moss", true),
                game("Not here", false),
                game("An Orchard", true),
                game("Anchor", true),
            ],
            ..Library::default()
        };
        let names: Vec<&str> = installed(&library).iter().map(|game| game.name.as_str()).collect();
        assert_eq!(names, ["Anchor", "moss", "An Orchard", "The Zephyr"]);
    }
}
