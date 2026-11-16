//! The Tile menu: one menu for a game, a dark panel in the middle of the screen over the dimmed page, whatever
//! opened it (docs/spec/tile-menu.md). Its rows are what can be done with the game as the library has it when the
//! menu opens, and do not change while it is up. More options is a second panel over the first, the flyout; a
//! question ("Uninstall X?") takes the menu's place until it is answered.
//!
//! It is a page that does not fill the screen: what opened it is drawn under it, its focus where it was, and has
//! the pad again the moment the menu closes. **To open it for a game, from any page:** answer a press with
//! [`open`], `Some(tile_menu::open(game, Opener::MyGames))`. Home does the same through `HomeAction::Open`.
//!
//! The panels are [`MenuPanel`]s, the drawing the Guide's menus share. What is this file's own: which rows a game
//! has, the three levels and where B goes from each, the dim, and how the panels come and go.
use std::sync::atomic::{AtomicU32, Ordering};

use model::{Game, GameKey, Library};
use ui::{Ease, Press, Ui};

use crate::menu_panel::{self, icon, Answer, MenuPanel, Row, DIM, FLY};
use crate::pages::{Around, Dialog, Open, Page, PageAction};
use crate::{game_hub, settings, Effect};

/// The flyout hangs from the menu's top left corner, this far in, and comes in from this far to its left.
const FLYOUT_IN: f32 = 77.0;
const FLYOUT_TRAVEL: f32 = -48.0;
const FLYOUT_SECONDS: f32 = 0.12;
/// The menu behind its flyout, as one picture.
const BEHIND: f32 = 0.45;

/// What the menu was opened from, which decides two of its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opener {
    /// A game's tile on Home.
    Home,
    /// A game on My games & apps: the menu's Customise row is "Customise games & apps".
    MyGames,
    /// The Game hub's Manage button: the menu has no Manage game row, the player being there already.
    GameHub,
}

/// How the menu is opened: for which game, and from where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The game's id in the library database and its store and id there: a made-up library's games all have id 0.
    game: GameKey,
    opener: Opener,
}

/// The Tile menu for `game`, as a page asks for it: `Some(tile_menu::open(game, Opener::MyGames))` from
/// `Page::press`. Nothing opens for a game that has left the library by then.
pub fn open(game: &Game, opener: Opener) -> PageAction {
    PageAction::Open(entry(game, opener))
}

/// The same for whoever opens pages itself: Home, through `HomeAction::Open`.
pub fn entry(game: &Game, opener: Opener) -> Open {
    Open::TileMenu(Entry { game: game.key(), opener })
}

/// The Game hub of a game, which Manage game opens in the menu's place, over whatever the menu was over.
fn game_hub(game: &Game) -> Option<Open> {
    Some(Open::GameHub(game_hub::Entry::of(game)))
}

/// Install, as the menu's row and the Game hub's main action both answer it: what asked closes, and the game's
/// store is asked for it. A game that is installed by now, or is in no library database, asks nothing, and what
/// asked closes all the same.
pub fn install(game: &Game) -> PageAction {
    match game {
        game if !game.installed && game.id != 0 => PageAction::CloseWith(Effect::Install(game.id)),
        _ => PageAction::Close,
    }
}

/// What a row of the menu means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Play,
    Install,
    Quit,
    /// Make the game a favourite, or no longer one.
    Favourite(bool),
    Manage,
    Uninstall,
    Customise,
    MoreOptions,
}

/// What a row of the flyout means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum More {
    /// Hide the game, or show it again.
    Hide(bool),
}

/// The answers to "Uninstall X?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Uninstall,
    Cancel,
}

/// The menu's rows for a game (docs/spec/tile-menu.md, section 3): the play row, which is Quit for the running
/// game and Install for one that is not installed; the Favourites row; Manage game, unless the hub opened the
/// menu; Uninstall for an installed game its store can uninstall, unless it is running; then, after a gap,
/// Customise and More options.
fn rows(game: &Game, running: bool, opener: Opener) -> Vec<Row<Choice>> {
    let mut rows = vec![if running {
        Row::new(icon::QUIT, "Quit", Choice::Quit)
    } else if !game.installed {
        Row::new(icon::INSTALL, "Install", Choice::Install)
    } else {
        Row::new(icon::PLAY, "Play game", Choice::Play)
    }];
    rows.push(if game.favourite {
        Row::new(icon::UNFAVOURITE, "Remove from Favourites", Choice::Favourite(false))
    } else {
        Row::new(icon::FAVOURITE, "Add to Favourites", Choice::Favourite(true))
    });
    if opener != Opener::GameHub {
        rows.push(Row::new(icon::MANAGE, "Manage game", Choice::Manage));
    }
    if game.installed && has_uninstaller(game) && !running {
        rows.push(Row::new(icon::UNINSTALL, "Uninstall", Choice::Uninstall));
    }
    let customise = if opener == Opener::MyGames { "Customise games & apps" } else { "Customise Home" };
    rows.push(Row::new(icon::CUSTOMISE, customise, Choice::Customise).after_a_gap());
    rows.push(Row::new(icon::MORE_OPTIONS, "More options", Choice::MoreOptions));
    rows
}

/// Whether the game's store can be asked to uninstall it: any store's can, its source giving the address. A game
/// from no store (a made-up one) has no Uninstall row, nor one added by hand, which Settings removes.
fn has_uninstaller(game: &Game) -> bool {
    !game.source.is_empty() && game.source != model::by_hand::MANUAL
}

/// The flyout's rows (section 4): what is left of the Bundle's once Playnite's own are gone (section 13).
fn more_rows(game: &Game) -> Vec<Row<More>> {
    vec![if game.hidden {
        Row::new(icon::UNHIDE, "Unhide this game", More::Hide(false))
    } else {
        Row::new(icon::HIDE, "Hide this game", More::Hide(true))
    }]
}

/// The game with this key, hidden or not, and whether it is among the hidden.
fn find(library: &Library, key: &GameKey) -> Option<Game> {
    let is = |game: &&Game| game.is(key);
    let hidden = library.hidden.iter().find(is).map(|game| Game { hidden: true, ..game.clone() });
    library.games.iter().find(is).cloned().or(hidden)
}

/// The game's place among the library's games, which is how a launch names it. A hidden game has none.
fn place(library: &Library, key: &GameKey) -> Option<usize> {
    library.games.iter().position(|game| game.is(key))
}

/// Each menu has animated values of its own, so one opened as another flies out starts from its own start.
static MENUS: AtomicU32 = AtomicU32::new(0);

pub struct TileMenu {
    game: GameKey,
    name: String,
    menu: MenuPanel<Choice>,
    /// More options, over the menu.
    flyout: Option<MenuPanel<More>>,
    /// A question in the menu's place.
    prompt: Option<MenuPanel<Confirm>>,
    serial: u32,
    /// How many times a panel has taken the menu's place, the menu coming back included: each flies up afresh.
    flights: u32,
    /// How many times the flyout has opened: each comes in afresh.
    flyouts: u32,
}

impl TileMenu {
    /// The menu for the game as the library has it now. None for a game that is not in it.
    pub fn open(entry: Entry, around: Around) -> Option<Self> {
        let game = find(around.library, &entry.game)?;
        let running = around.playing.is_some() && around.playing == place(around.library, &entry.game);
        Some(Self {
            game: entry.game,
            menu: MenuPanel::menu(&game.name, rows(&game, running, entry.opener)),
            name: game.name,
            flyout: None,
            prompt: None,
            serial: MENUS.fetch_add(1, Ordering::Relaxed),
            flights: 0,
            flyouts: 0,
        })
    }

    /// A on a row of the menu.
    fn chose(&mut self, choice: Choice, around: Around) -> Option<PageAction> {
        let game = find(around.library, &self.game);
        let place = place(around.library, &self.game);
        // Every row but Uninstall and More options closes the menu first and then acts. What a row would do is
        // looked at again as it is chosen: one that no longer applies does nothing, and the menu closes all the
        // same (section 3.3).
        let then = |effect: Option<Effect>| Some(effect.map_or(PageAction::Close, PageAction::CloseWith));
        match choice {
            // With a game running nothing more is started. The Guide's "Quit X and start Y?" is #40's.
            Choice::Play => then(place.filter(|_| around.playing.is_none()).map(Effect::Launch)),
            Choice::Quit => then((around.playing.is_some() && around.playing == place).then_some(Effect::QuitGame)),
            Choice::Install => Some(game.as_ref().map_or(PageAction::Close, install)),
            Choice::Favourite(on) => then(Some(Effect::Favourite { game: self.game.id, on })),
            Choice::Manage => Some(game.as_ref().and_then(game_hub).map_or(PageAction::Close, PageAction::Replace)),
            Choice::Uninstall => {
                let answers = vec![
                    Row::new(icon::UNINSTALL, "Uninstall", Confirm::Uninstall),
                    Row::new(icon::CANCEL, "Cancel", Confirm::Cancel),
                ];
                self.prompt = Some(MenuPanel::prompt(&format!("Uninstall {}?", self.name), None, answers));
                self.flights += 1;
                None
            }
            Choice::Customise => Some(PageAction::Replace(Open::Settings(settings::Entry::Personalisation))),
            // The flyout is made as it opens, from the library as it is then. A game that has gone meanwhile
            // closes the whole menu.
            Choice::MoreOptions => match game {
                Some(game) => {
                    self.flyout = Some(MenuPanel::menu("More options", more_rows(&game)));
                    self.flyouts += 1;
                    None
                }
                None => Some(PageAction::Close),
            },
        }
    }

    /// The panel in the menu's place: the question, while one is asked.
    fn in_the_middle(&self) -> Middle<'_> {
        match &self.prompt {
            Some(prompt) => Middle::Prompt(prompt),
            None => Middle::Menu(&self.menu),
        }
    }

    /// Where the panel in the menu's place flies and how much of it shows, on its way in or, `leaving`, out.
    /// Under Reduce motion it is in its place, or gone, at once.
    fn flight(&self, ui: &mut Ui, leaving: bool) -> (f32, f32) {
        menu_panel::flight(ui, ("tile menu", self.serial, self.flights), leaving)
    }
}

/// The panel in the menu's place.
enum Middle<'a> {
    Menu(&'a MenuPanel<Choice>),
    Prompt(&'a MenuPanel<Confirm>),
}

impl Middle<'_> {
    /// Draws it centred on the screen by its own height, `fly` under its place, at `opacity`.
    fn draw(&self, ui: &mut Ui, fly: f32, opacity: f32) -> (f32, f32) {
        match self {
            Middle::Menu(panel) => panel.draw_flown(ui, fly, opacity),
            Middle::Prompt(panel) => panel.draw_flown(ui, fly, opacity),
        }
    }
}

impl Page for TileMenu {
    fn name(&self) -> &'static str {
        "TileMenu"
    }

    /// The panel that has the pad and its focused row, then the game: `Menu(Play game) of Halo Infinite`,
    /// `MoreOptions(Hide this game) of ...`, `Prompt(Uninstall) of ...`.
    fn focus(&self) -> String {
        fn row<T: Copy>(panel: &MenuPanel<T>) -> &str {
            panel.rows().get(panel.focus()).map_or("", |row| row.label.as_str())
        }
        let (level, label) = match (&self.flyout, &self.prompt) {
            (Some(flyout), _) => ("MoreOptions", row(flyout)),
            (None, Some(prompt)) => ("Prompt", row(prompt)),
            (None, None) => ("Menu", row(&self.menu)),
        };
        format!("{level}({label}) of {}", self.name)
    }

    fn dialog(&self) -> Option<Dialog> {
        Some(if self.flyout.is_none() && self.prompt.is_some() { Dialog::Prompt } else { Dialog::Menu })
    }

    /// The panel on top has the pad: the flyout, else the question, else the menu. B is one level at a time, and
    /// each level under keeps its focus: on More options behind the flyout, on Uninstall behind its question.
    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        if let Some(flyout) = &mut self.flyout {
            return match flyout.press(press) {
                // The whole menu closes, then the game is hidden or shown again.
                Answer::Chose(More::Hide(on)) => Some(PageAction::CloseWith(Effect::Hidden { game: self.game.id, on })),
                Answer::Declined => {
                    self.flyout = None;
                    None
                }
                Answer::Moved | Answer::Nothing => None,
            };
        }
        if let Some(prompt) = &mut self.prompt {
            return match prompt.press(press) {
                Answer::Chose(Confirm::Uninstall) => {
                    let game = find(around.library, &self.game).filter(|game| game.installed && game.id != 0);
                    Some(game.map_or(PageAction::Close, |game| PageAction::CloseWith(Effect::Uninstall(game.id))))
                }
                // Back to the menu, which flies up again as the question did.
                Answer::Chose(Confirm::Cancel) | Answer::Declined => {
                    self.prompt = None;
                    self.flights += 1;
                    None
                }
                Answer::Moved | Answer::Nothing => None,
            };
        }
        match self.menu.press(press) {
            Answer::Chose(choice) => self.chose(choice, around),
            Answer::Declined => Some(PageAction::Close),
            Answer::Moved | Answer::Nothing => None,
        }
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around) {
        ui.fill(ui.screen(), DIM, 0.0);
        let (fly, shown) = self.flight(ui, false);
        // Behind the flyout the menu is one faded picture, its focused row with it.
        let behind = if self.flyout.is_some() { BEHIND } else { 1.0 };
        let at = self.in_the_middle().draw(ui, fly, shown * behind);
        if let Some(flyout) = &self.flyout {
            let id = |what: &'static str| (what, self.serial, self.flyouts);
            let slide = ui.slide_from(id("tile menu flyout"), FLYOUT_TRAVEL, 0.0, FLYOUT_SECONDS, Ease::CubicOut);
            let seconds = if ui.store.reduce_motion { 0.0 } else { FLYOUT_SECONDS };
            let shown = ui.tween_from(id("tile menu flyout fade"), 0.0, 1.0, seconds, Ease::Linear);
            let at = (at.0 + FLYOUT_IN + slide, at.1);
            if shown >= 1.0 {
                // Over the menu's text.
                ui.layer();
                flyout.draw(ui, at);
            } else if shown > 0.0 {
                flyout.draw_faded(ui, at, shown);
            }
        }
    }

    /// The menu has closed: the dim went with it, at once, and so did the flyout; the panel that was in the
    /// menu's place flies down and out with the rows it held.
    fn draw_leaving(&mut self, ui: &mut Ui, _around: Around) -> bool {
        let (fly, shown) = self.flight(ui, true);
        if shown <= 0.0 || fly >= FLY {
            return false;
        }
        self.in_the_middle().draw(ui, fly, shown);
        true
    }

    fn covers(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game {
            id: 7,
            source: "steam".into(),
            source_id: "440".into(),
            name: "Halo Infinite".into(),
            installed: true,
            ..Game::default()
        }
    }

    fn labels(game: &Game, running: bool, opener: Opener) -> Vec<String> {
        rows(game, running, opener).into_iter().map(|row| row.label).collect()
    }

    /// docs/spec/tile-menu.md, section 3.1.
    #[test]
    fn the_rows_follow_the_games_state() {
        let installed = game();
        assert_eq!(
            labels(&installed, false, Opener::Home),
            ["Play game", "Add to Favourites", "Manage game", "Uninstall", "Customise Home", "More options"]
        );
        let absent = Game { installed: false, favourite: true, ..game() };
        assert_eq!(
            labels(&absent, false, Opener::Home),
            ["Install", "Remove from Favourites", "Manage game", "Customise Home", "More options"]
        );
        // Running is looked at first, and a running game is not uninstalled.
        for game in [&installed, &absent] {
            assert_eq!(
                labels(game, true, Opener::Home),
                [
                    "Quit",
                    labels(game, false, Opener::Home)[1].as_str(),
                    "Manage game",
                    "Customise Home",
                    "More options"
                ]
            );
        }
        // A game no store brought in has no uninstaller.
        let by_hand = Game { source: String::new(), ..game() };
        assert_eq!(labels(&by_hand, false, Opener::Home).len(), 5);
        assert!(!labels(&by_hand, false, Opener::Home).contains(&"Uninstall".to_string()));
        let manual = Game { source: model::by_hand::MANUAL.into(), ..game() };
        assert!(!labels(&manual, false, Opener::Home).contains(&"Uninstall".to_string()));
    }

    #[test]
    fn the_opener_decides_manage_game_and_what_customise_is_called() {
        assert_eq!(
            labels(&game(), false, Opener::GameHub),
            ["Play game", "Add to Favourites", "Uninstall", "Customise Home", "More options"]
        );
        assert_eq!(labels(&game(), false, Opener::MyGames)[4], "Customise games & apps");
        // The gap is over Customise, wherever that is.
        for opener in [Opener::Home, Opener::MyGames, Opener::GameHub] {
            let gaps: Vec<bool> = rows(&game(), false, opener).iter().map(|row| row.gap).collect();
            assert_eq!(gaps.iter().filter(|&&gap| gap).count(), 1);
            assert!(gaps[gaps.len() - 2]);
        }
    }

    #[test]
    fn the_flyout_hides_a_game_and_shows_a_hidden_one_again() {
        assert_eq!(more_rows(&game())[0].label, "Hide this game");
        let hidden = Game { hidden: true, ..game() };
        assert_eq!(
            (more_rows(&hidden)[0].label.as_str(), more_rows(&hidden)[0].value),
            ("Unhide this game", More::Hide(false))
        );
    }

    #[test]
    fn a_game_is_found_among_the_hidden_too_and_only_a_shown_one_has_a_place() {
        let other = Game { id: 8, source_id: "441".into(), ..game() };
        let library = Library { games: vec![other.clone()], hidden: vec![game()], ..Library::default() };
        assert!(find(&library, &game().key()).is_some_and(|game| game.hidden));
        assert!(find(&library, &other.key()).is_some_and(|game| !game.hidden));
        assert_eq!((place(&library, &other.key()), place(&library, &game().key())), (Some(0), None));
        let absent = Game { id: 9, source_id: "442".into(), ..game() }.key();
        assert!(find(&library, &absent).is_none());
        // The same store's id from another store is another game.
        assert!(find(&library, &Game { source: "gog".into(), ..other.clone() }.key()).is_none());
        let around =
            Around { library: &library, playing: None, options: Default::default(), now: std::time::Instant::now() };
        assert!(TileMenu::open(Entry { game: absent, opener: Opener::Home }, around).is_none());
    }
}
