//! The Game hub (docs/spec/game-hub.md): the Dashboard's page for one game, over the screen it was opened from. A
//! band across the screen with the game's art behind it, and in it the game's name and a line of facts, a row of
//! cards, and its actions. It has two pages, Game hub and Game details, which differ only in the cards; the last
//! action turns from one to the other.
//!
//! This file is the page's state: which game, which of the two pages, where focus is and where it goes. The
//! modules beside it draw.
//!
//! Menu anywhere, and A on Manage, open the game's Tile menu (`tile_menu`, without its Manage game row); A on
//! Install is the Tile menu's Install; A on Queue opens Manage Queue over the hub (`manage_queue`), which B there
//! comes back from. A on Your game stats of a game with achievements opens its Achievements page in the hub's
//! place (`achievements`), which B there comes back from.
mod actions;
pub(crate) mod backdrop;
pub(crate) mod cards;
pub(crate) mod text;

use std::sync::atomic::{AtomicU64, Ordering};

use model::{Game, GameKey, Library};
use ui::{Button, Font, HAlign, Press, Rect, TextStyle, Ui, VAlign};

use self::actions::{Action, Main};
use self::cards::Card;
use crate::hint_bar::{self, Hint};
use crate::pages::{self, Around, Page, PageAction};
use crate::theme::TEXT_DIM;
use crate::{tile_menu, Effect};

/// The content's top: 40 under the band's.
const TOP: f32 = 128.0;
/// The content's width: the screen's less 80 each side.
const WIDTH: f32 = 1760.0;
const TITLE: TextStyle = TextStyle::new(Font::Display, 47.0, 400);
/// The title's line: 47 at Bahnschrift's 1.2.
const TITLE_HEIGHT: f32 = 56.4;
const FACTS: TextStyle = TextStyle::new(Font::Ui, 20.0, 400).color(TEXT_DIM);
/// The facts' line is 6 under the title's and 48 tall, whatever is in it.
const FACTS_HEIGHT: f32 = 48.0;

/// The game a hub is opened for: its row in the library database, and its store and its id there. (A library that
/// is in no database has no rows, and tells its games apart by the second.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub game: GameKey,
    /// Whether it was installed then: what the main action is until the page first hears of the library.
    pub installed: bool,
}

impl Entry {
    pub fn of(game: &Game) -> Self {
        Self { game: game.key(), installed: game.installed }
    }

    pub fn in_library(&self, library: &Library) -> bool {
        self.find(library).is_some()
    }

    /// The game, and its place in the library's games, which a hidden game has none in.
    fn find<'a>(&self, library: &'a Library) -> Option<(&'a Game, Option<usize>)> {
        let is = |game: &Game| game.is(&self.game);
        match library.games.iter().position(is) {
            Some(place) => Some((&library.games[place], Some(place))),
            None => library.hidden.iter().find(|game| is(game)).map(|game| (game, None)),
        }
    }
}

/// Which of the hub's two pages shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Hub,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Action(Action),
    Card(Card),
}

pub struct GameHub {
    entry: Entry,
    view: View,
    focus: Focus,
    /// What the main action is, as the library and the shell last had the game.
    main: Main,
    /// Whether the game is in the Queue, and so Queue shows.
    queued: bool,
    /// The toggle's width as it was last drawn: Up and Down go by where things are.
    toggle_width: f32,
    /// What tells this hub's picture of the screen behind it from another's.
    serial: u64,
}

/// The facts under the title: who published the game, the year it came out, what it takes on disk while it is
/// installed, and the store it is from. The Bundle's platform, features and age rating are not in Steam's files.
fn facts(game: &Game) -> String {
    let source = crate::home::source(game).map(|source| source.name().to_string());
    let facts = [
        (!game.publishers.is_empty()).then(|| game.publishers.join(", ")),
        (game.released > 0).then(|| text::year(game.released)),
        (game.installed && game.size_on_disk > 0).then(|| text::size(game.size_on_disk)),
        source,
    ];
    facts.into_iter().flatten().collect::<Vec<_>>().join(" \u{2022} ")
}

/// How far the game has got with its achievements, if it has any: a game in no library database has none.
fn progress<'a>(game: &Game, library: &'a Library) -> Option<&'a model::achievement::GameProgress> {
    library.achievements.of(game.id).filter(|_| game.id != 0)
}

/// The hub's header: the game's name, and a line under it (the facts on the hub, what the page is on the
/// Achievements page). Neither wraps.
pub(crate) fn header(ui: &mut Ui, title: &str, line: &str) {
    let title_rect = Rect::new(cards::LEFT, TOP, WIDTH, TITLE_HEIGHT * 1.2);
    ui.paragraph(title_rect, title, &TITLE, HAlign::Left, VAlign::Top, 1);
    let line_rect = Rect::new(cards::LEFT, TOP + TITLE_HEIGHT + 6.0, WIDTH, FACTS_HEIGHT);
    ui.paragraph(line_rect, line, &FACTS, HAlign::Left, VAlign::Center, 1);
}

impl GameHub {
    /// The hub for a game, on its Game hub page with focus on the main action: it remembers nothing from the last
    /// time one was up. The main action is what the game can do now, Queued or Installing for one in the Queue,
    /// from the moment it opens.
    pub fn open(entry: Entry, around: Around) -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let mut hub = Self {
            main: if entry.installed { Main::Play } else { Main::Install },
            queued: false,
            entry,
            view: View::Hub,
            focus: Focus::Action(Action::Main),
            toggle_width: 269.0,
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
        };
        if let Some((game, place)) = hub.entry.find(around.library) {
            hub.follow(game, place, around);
        }
        hub
    }

    /// Takes in the game as it is now: what the main action is, whether Queue shows, and focus off a card or Queue
    /// when it no longer shows (from Queue to Manage).
    fn follow(&mut self, game: &Game, place: Option<usize>, around: Around) -> Vec<Card> {
        let queued = around.library.queue.get(game.id).filter(|_| game.id != 0);
        self.main = match (game.installed, queued) {
            _ if place.is_some() && around.playing == place => Main::Running,
            (true, _) => Main::Play,
            (false, Some(entry)) if entry.state.downloading() => Main::Installing,
            (false, Some(_)) => Main::Queued,
            (false, None) => Main::Install,
        };
        self.queued = queued.is_some() && !game.installed;
        if self.focus == Focus::Action(Action::Queue) && !self.queued {
            self.focus = Focus::Action(Action::Manage);
        }
        let cards = cards::showing(game, self.view == View::Details, progress(game, around.library));
        if matches!(self.focus, Focus::Card(card) if !cards.contains(&card)) {
            self.focus = Focus::Action(Action::Main);
        }
        cards
    }

    /// Which of `middles` is nearest `x`.
    fn nearest(x: f32, middles: impl Iterator<Item = f32>) -> usize {
        let distances = middles.map(|middle| (middle - x).abs());
        distances.enumerate().min_by(|a, b| a.1.total_cmp(&b.1)).map_or(0, |(at, _)| at)
    }

    /// A direction: along the row focus is in, stopping at its ends; Up from an action to the card over it, the
    /// cover from the main action; Down from a card to the action under it. Nearest is by the middles, across.
    fn step(&mut self, button: Button, cards: &[Card]) {
        let middle = |rect: &Rect| rect.center().0;
        let action_rects = actions::rects(self.toggle_width, self.queued);
        let shown = actions::shown(self.queued);
        self.focus = match (self.focus, button) {
            (Focus::Action(action), Button::Left | Button::Right) => {
                let at = shown.iter().position(|&a| a == action).unwrap_or(0);
                let to = if button == Button::Left { at.saturating_sub(1) } else { (at + 1).min(shown.len() - 1) };
                Focus::Action(shown[to])
            }
            (Focus::Card(card), Button::Left | Button::Right) => {
                let at = cards.iter().position(|&c| c == card).unwrap_or(0);
                let to = if button == Button::Left { at.saturating_sub(1) } else { (at + 1).min(cards.len() - 1) };
                Focus::Card(cards[to])
            }
            (Focus::Action(Action::Main), Button::Up) => Focus::Card(Card::Cover),
            (Focus::Action(action), Button::Up) => {
                let at = shown.iter().position(|&a| a == action).unwrap_or(0);
                let card_rects = cards::rects(cards, None);
                Focus::Card(cards[Self::nearest(middle(&action_rects[at]), card_rects.iter().map(middle))])
            }
            (Focus::Card(card), Button::Down) => {
                let at = cards.iter().position(|&c| c == card).unwrap_or(0);
                let from = middle(&cards::rects(cards, Some(card))[at]);
                Focus::Action(shown[Self::nearest(from, action_rects.iter().map(middle))])
            }
            (focus, _) => focus,
        };
    }
}

impl Page for GameHub {
    fn name(&self) -> &'static str {
        "GameHub"
    }

    /// The game by its id at its store, the page, and what has focus, the main action by what it is now:
    /// `9100010 Hub Play`, `9100010 Details Toggle`, `9100050 Hub Install`, `9100010 Hub Cover`; for a game in the
    /// Queue `9100040 Hub Queued` or `9100040 Hub Installing`, and `9100040 Hub Queue` on its Queue action.
    fn focus(&self) -> String {
        let at = match self.focus {
            Focus::Action(Action::Main) => format!("{:?}", self.main),
            Focus::Action(action) => format!("{action:?}"),
            Focus::Card(card) => format!("{card:?}"),
        };
        format!("{} {:?} {at}", self.entry.game.source_id, self.view)
    }

    /// A new library, or a new Queue: the main action reads what the game can do now (Queued, Installing...)
    /// at once, not at the next press.
    fn library_changed(&mut self, around: Around) {
        if let Some((game, place)) = self.entry.find(around.library) {
            self.follow(game, place, around);
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Option<PageAction> {
        let Some((game, place)) = self.entry.find(around.library) else {
            // The game has left the library under the hub: there is nothing to do here but leave.
            return (press.button == Button::B && !press.repeat).then_some(PageAction::Close);
        };
        let cards = self.follow(game, place, around);
        match (press.button, self.focus) {
            (button, _) if button.is_direction() => self.step(button, &cards),
            _ if press.repeat => {}
            (Button::B, _) => return Some(PageAction::Close),
            // The game's Tile menu, without its Manage game row: the player is here already. Focus stays where
            // it is, and B there comes back to it.
            (Button::Menu, _) | (Button::A, Focus::Action(Action::Manage)) => {
                return Some(tile_menu::open(game, tile_menu::Opener::GameHub));
            }
            (Button::A, Focus::Action(Action::Main)) => match self.main {
                Main::Play => return place.map(|place| PageAction::Effect(Effect::Launch(place))),
                // Its store is asked for the game, and the hub closes, as the Bundle's does.
                Main::Install => return Some(tile_menu::install(game, around.library)),
                Main::Running | Main::Queued | Main::Installing => {}
            },
            // Manage Queue over the hub, which B there comes back to, focus still on Queue.
            (Button::A, Focus::Action(Action::Queue)) => {
                return Some(PageAction::Open(crate::manage_queue::open()));
            }
            // The other page's cards; focus stays on the toggle, which now names the page it left.
            (Button::A, Focus::Action(Action::Toggle)) => {
                self.view = if self.view == View::Hub { View::Details } else { View::Hub };
            }
            // Your game stats of a game with achievements: its Achievements page, in the hub's place, which B
            // there comes back from, focus still on the card.
            (Button::A, Focus::Card(Card::Stats)) if progress(game, around.library).is_some() => {
                let entry = crate::achievements::Entry { game: game.key(), id: game.id, behind: self.serial };
                return Some(PageAction::Open(pages::Open::Achievements(entry)));
            }
            // A on another card does nothing; nor do X, Y, View, the bumpers and the triggers.
            _ => {}
        }
        None
    }

    fn draw(&mut self, ui: &mut Ui, around: Around) {
        let Some((game, place)) = self.entry.find(around.library) else {
            ui.fill(ui.screen(), ui::Color::BLACK, 0.0);
            return;
        };
        let cards = self.follow(game, place, around);
        let (on_action, on_card) = match self.focus {
            Focus::Action(action) => (Some(action), None),
            Focus::Card(card) => (None, Some(card)),
        };

        let behind = pages::behind(ui, self.serial);
        backdrop::draw(ui, game, behind);

        header(ui, &game.name, &facts(game));

        let source = crate::home::source(game).map(|source| source.name());
        let details = self.view == View::Details;
        self.toggle_width = actions::draw(ui, self.main, source, details, self.queued, on_action);
        cards::draw(ui, game, &cards, on_card, progress(game, around.library));
        actions::name_chip(ui, on_action, self.toggle_width, self.queued);
    }

    /// More options wherever focus is, and never Search: Y does nothing in the hub.
    fn hints(&self, _around: Around) -> &'static [Hint] {
        hint_bar::MORE_OPTIONS
    }

    fn behind(&self) -> Option<u64> {
        Some(self.serial)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn library() -> Library {
        let game = |appid: u32, installed: bool, playtime_minutes: u32| Game {
            source_id: appid.to_string(),
            name: format!("Game {appid}"),
            installed,
            playtime_minutes,
            last_played: u64::from(playtime_minutes) * 1_000_000,
            ..Game::default()
        };
        Library {
            games: vec![game(10, true, 600), game(20, false, 0)],
            hidden: vec![game(30, true, 5)],
            ..Library::default()
        }
    }

    struct Hub {
        hub: GameHub,
        library: Library,
        playing: Option<usize>,
    }

    impl Hub {
        fn open(appid: u32) -> Self {
            let library = library();
            let game =
                library.games.iter().chain(&library.hidden).find(|game| game.source_id == appid.to_string()).unwrap();
            let around = Around {
                library: &library,
                playing: None,
                options: Default::default(),
                now: Instant::now(),
                now_playing: crate::now_playing::none(),
                discord: crate::no_discord(),
            };
            let hub = GameHub::open(Entry::of(game), around);
            Self { hub, library, playing: None }
        }

        /// The hub opened anew on `entry`, in this library.
        fn reopen(&mut self, entry: Entry) {
            let around = Around {
                library: &self.library,
                playing: self.playing,
                options: Default::default(),
                now: Instant::now(),
                now_playing: crate::now_playing::none(),
                discord: crate::no_discord(),
            };
            self.hub = GameHub::open(entry, around);
        }

        fn press(&mut self, button: Button) -> Option<PageAction> {
            let around = Around {
                library: &self.library,
                playing: self.playing,
                options: Default::default(),
                now: Instant::now(),
                now_playing: crate::now_playing::none(),
                discord: crate::no_discord(),
            };
            self.hub.press(Press { button, repeat: false }, around)
        }

        /// Presses each button, and says where focus is after.
        fn go(&mut self, buttons: &[Button]) -> String {
            for &button in buttons {
                assert_eq!(self.press(button), None, "{button:?}");
            }
            self.hub.focus()
        }
    }

    #[test]
    fn focus_starts_on_the_main_action_which_is_what_the_game_can_do() {
        assert_eq!(Hub::open(10).hub.focus(), "10 Hub Play");
        assert_eq!(Hub::open(20).hub.focus(), "20 Hub Install");
        let mut running = Hub::open(10);
        running.playing = Some(0);
        assert_eq!(running.go(&[Button::X]), "10 Hub Running");
        // Another game running leaves this one's Play.
        running.playing = Some(1);
        assert_eq!(running.go(&[Button::X]), "10 Hub Play");
    }

    #[test]
    fn left_and_right_go_along_a_row_and_stop_at_its_ends() {
        let mut hub = Hub::open(10);
        assert_eq!(hub.go(&[Button::Left]), "10 Hub Play");
        assert_eq!(hub.go(&[Button::Right]), "10 Hub Manage");
        assert_eq!(hub.go(&[Button::Right]), "10 Hub Toggle");
        assert_eq!(hub.go(&[Button::Right, Button::Down]), "10 Hub Toggle");
        assert_eq!(hub.go(&[Button::Left, Button::Left, Button::Up]), "10 Hub Cover");
        assert_eq!(hub.go(&[Button::Left, Button::Up]), "10 Hub Cover");
        assert_eq!(hub.go(&[Button::Right]), "10 Hub Stats");
        assert_eq!(hub.go(&[Button::Right]), "10 Hub Stats");
    }

    #[test]
    fn up_and_down_go_to_what_is_nearest_across() {
        let mut hub = Hub::open(10);
        // The cover is over the main action; the stats card over Manage and the toggle.
        assert_eq!(hub.go(&[Button::Up, Button::Down]), "10 Hub Play");
        assert_eq!(hub.go(&[Button::Right, Button::Up]), "10 Hub Stats");
        assert_eq!(hub.go(&[Button::Down]), "10 Hub Toggle");
        assert_eq!(hub.go(&[Button::Up]), "10 Hub Stats");
        // A game with the cover alone: every action's Up is the cover.
        let mut bare = Hub::open(20);
        assert_eq!(bare.go(&[Button::Right, Button::Right, Button::Up]), "20 Hub Cover");
        assert_eq!(bare.go(&[Button::Down]), "20 Hub Install");
    }

    #[test]
    fn the_toggle_turns_the_page_and_keeps_focus() {
        let mut hub = Hub::open(10);
        assert_eq!(hub.go(&[Button::Right, Button::Right, Button::A]), "10 Details Toggle");
        assert_eq!(hub.go(&[Button::Up]), "10 Details Trailer");
        assert_eq!(hub.go(&[Button::Left, Button::Right, Button::Right]), "10 Details Trailer");
        assert_eq!(hub.go(&[Button::Down, Button::A]), "10 Hub Toggle");
        assert_eq!(hub.go(&[Button::Up]), "10 Hub Stats");
    }

    #[test]
    fn a_plays_from_play_and_does_nothing_on_a_card_or_while_the_game_runs() {
        let mut hub = Hub::open(10);
        assert_eq!(hub.press(Button::A), Some(PageAction::Effect(Effect::Launch(0))));
        // A held A is one A.
        let around = Around {
            library: &hub.library,
            playing: None,
            options: Default::default(),
            now: Instant::now(),
            now_playing: crate::now_playing::none(),
            discord: crate::no_discord(),
        };
        assert_eq!(hub.hub.press(Press { button: Button::A, repeat: true }, around), None);
        assert_eq!(hub.go(&[Button::Up, Button::A]), "10 Hub Cover");
        hub.playing = Some(0);
        assert_eq!(hub.go(&[Button::Down, Button::A]), "10 Hub Running");
        // The other buttons have nothing to do here.
        let mut other = Hub::open(20);
        let idle = [Button::X, Button::Y, Button::View, Button::LeftBumper];
        assert_eq!(other.go(&idle), "20 Hub Install");
    }

    #[test]
    fn menu_anywhere_and_a_on_manage_open_the_tile_menu_and_install_is_the_tile_menus() {
        let mut hub = Hub::open(20);
        let menu = Some(tile_menu::open(&hub.library.games[1], tile_menu::Opener::GameHub));
        assert_eq!(hub.press(Button::Menu), menu);
        assert_eq!(hub.go(&[Button::Up]), "20 Hub Cover");
        assert_eq!(hub.press(Button::Menu), menu);
        assert_eq!(hub.go(&[Button::Down, Button::Right]), "20 Hub Manage");
        assert_eq!(hub.press(Button::A), menu);
        // A made-up library's game is in no library database: Install closes the hub and asks nothing.
        assert_eq!(hub.go(&[Button::Left]), "20 Hub Install");
        assert_eq!(hub.press(Button::A), Some(PageAction::Close));
        // One that is in one asks its store for it.
        hub.library.games[1].id = 7;
        hub.reopen(Entry::of(&hub.library.games[1]));
        assert_eq!(hub.press(Button::A), Some(PageAction::CloseWith(Effect::Install(7))));
    }

    #[test]
    fn a_on_your_game_stats_opens_the_achievements_page_of_a_game_with_achievements() {
        let mut hub = Hub::open(10);
        assert_eq!(hub.go(&[Button::Right, Button::Up, Button::A]), "10 Hub Stats");
        // With achievements, and so in a library database.
        hub.library.games[0].id = 7;
        let progress = model::achievement::GameProgress { game: 7, unlocked: 2, of: 6, score: 25 };
        hub.library.achievements.games.push(progress);
        hub.reopen(Entry::of(&hub.library.games[0]));
        assert_eq!(hub.go(&[Button::Right, Button::Up]), "10 Hub Stats");
        let entry = crate::achievements::Entry { game: hub.library.games[0].key(), id: 7, behind: hub.hub.serial };
        assert_eq!(hub.press(Button::A), Some(PageAction::Open(pages::Open::Achievements(entry))));
        // A game never played with achievements still has the card, for their row.
        let game = &mut hub.library.games[0];
        (game.playtime_minutes, game.last_played) = (0, 0);
        assert_eq!(hub.go(&[Button::X]), "10 Hub Stats");
    }

    #[test]
    fn b_closes_the_hub_from_anywhere() {
        let mut hub = Hub::open(10);
        assert_eq!(hub.press(Button::B), Some(PageAction::Close));
        hub.go(&[Button::Up, Button::Right]);
        assert_eq!(hub.press(Button::B), Some(PageAction::Close));
        hub.go(&[Button::Down, Button::A]);
        assert_eq!(hub.press(Button::B), Some(PageAction::Close));
    }

    #[test]
    fn a_hidden_game_has_a_hub_but_no_place_to_play_from() {
        let mut hub = Hub::open(30);
        assert_eq!(hub.go(&[Button::A]), "30 Hub Play");
        assert_eq!(hub.go(&[Button::Up, Button::Right]), "30 Hub Stats");
    }

    #[test]
    fn a_game_that_leaves_the_library_leaves_b() {
        let mut hub = Hub::open(10);
        hub.library.games.remove(0);
        assert_eq!(hub.go(&[Button::A, Button::Right]), "10 Hub Play");
        assert_eq!(hub.press(Button::B), Some(PageAction::Close));
    }

    #[test]
    fn focus_leaves_a_card_that_goes() {
        let mut hub = Hub::open(10);
        assert_eq!(hub.go(&[Button::Right, Button::Up]), "10 Hub Stats");
        let game = &mut hub.library.games[0];
        (game.playtime_minutes, game.last_played) = (0, 0);
        assert_eq!(hub.go(&[Button::X]), "10 Hub Play");
    }

    #[test]
    fn the_facts_are_those_the_library_has_joined_by_bullets() {
        let mut game = Game { name: "Brass Meridian".into(), ..Game::default() };
        assert_eq!(facts(&game), "Steam");
        game.publishers = vec!["Harbour & Hull".into(), "Ninth Bell Games".into()];
        game.released = 1_552_348_800;
        game.size_on_disk = 45 * (1 << 30);
        // The size is an installed game's.
        assert_eq!(facts(&game), "Harbour & Hull, Ninth Bell Games \u{2022} 2019 \u{2022} Steam");
        game.installed = true;
        assert_eq!(facts(&game), "Harbour & Hull, Ninth Bell Games \u{2022} 2019 \u{2022} 45.00 GB \u{2022} Steam");
        game.publishers.clear();
        assert_eq!(facts(&game), "2019 \u{2022} 45.00 GB \u{2022} Steam");
    }
}
