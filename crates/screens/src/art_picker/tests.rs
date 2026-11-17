use model::art_picker::{Candidate, Reply};
use ui::Press;

use super::*;
use crate::settings::Options;

fn game() -> Game {
    let mut game =
        Game { id: 3, source: "steam".into(), source_id: "30".into(), name: "Made-up Game".into(), ..Game::default() };
    game.art.store[Tab::Cover.index()] = Some("C:/Steam/librarycache/30/library_600x900.jpg".into());
    game
}

fn library() -> Library {
    Library { games: vec![game()], ..Library::default() }
}

fn around(library: &Library, key: bool) -> Around<'_> {
    Around {
        library,
        playing: None,
        options: Options { art_key: key, art_online: true, ..Options::default() },
        now: std::time::Instant::now(),
        now_playing: crate::now_playing::none(),
        discord: crate::no_discord(),
    }
}

fn picker(library: &Library, key: bool) -> ArtPicker {
    ArtPicker::open(Entry { game: game().key() }, around(library, key)).unwrap()
}

fn press(picker: &mut ArtPicker, library: &Library, button: Button) -> Option<PageAction> {
    picker.press(Press { button, repeat: false }, around(library, true))
}

/// Answers every page the picker asks for from a list of `total` candidates, served `limit` at a time; gives the
/// pages asked.
fn serve(picker: &mut ArtPicker, total: u64, limit: usize) -> Vec<u32> {
    let mut asked = Vec::new();
    loop {
        let pages: Vec<(Tab, u32)> = picker
            .art_asks()
            .into_iter()
            .filter_map(|ask| match ask {
                Ask::Page { tab, page, .. } => Some((tab, page)),
                Ask::Thumb { .. } => None,
            })
            .collect();
        if pages.is_empty() {
            return asked;
        }
        for (tab, page) in pages {
            asked.push(page);
            let start = page as u64 * limit as u64;
            let items = (start..(start + limit as u64).min(total))
                .map(|n| Candidate {
                    id: 100 + n,
                    url: format!("full/{n}"),
                    thumb: format!("thumb/{n}"),
                    width: 1,
                    height: 1,
                })
                .collect();
            let reply = Reply { page, total: total as usize, limit, items };
            picker.art_answer(Answer::Page { game: game().key(), tab, page, result: Ok(reply) });
        }
    }
}

#[test]
fn the_pages_are_whole_grids_and_lt_rt_keep_focus_on_the_same_cell() {
    let library = library();
    let mut picker = picker(&library, true);
    assert_eq!(picker.focus(), "Square: Page 1: Automatic");
    // Square: 10 cells a page, Automatic first (Steam has no square of its own): 23 candidates are 3 pages.
    assert_eq!(serve(&mut picker, 23, 50), [0]);
    assert_eq!(picker.focus(), "Square: Page 1 of 3: Automatic");
    assert_eq!(picker.state(), "in use: Automatic");
    for _ in 0..3 {
        press(&mut picker, &library, Button::Right);
    }
    press(&mut picker, &library, Button::Down);
    assert_eq!(picker.focus(), "Square: Page 1 of 3: Picture 107");
    press(&mut picker, &library, Button::RightTrigger);
    assert_eq!(picker.focus(), "Square: Page 2 of 3: Picture 117", "the same cell");
    press(&mut picker, &library, Button::RightTrigger);
    // The last page has 4 candidates (19 to 22): the same cell is not there, so its last.
    assert_eq!(picker.focus(), "Square: Page 3 of 3: Picture 122");
    press(&mut picker, &library, Button::RightTrigger);
    assert_eq!(picker.focus(), "Square: Page 3 of 3: Picture 122", "no page past the last");
    press(&mut picker, &library, Button::LeftTrigger);
    // Focus is on the fourth cell now, and stays there.
    assert_eq!(picker.focus(), "Square: Page 2 of 3: Picture 112");
    // Left past the grid's edge: the page before, the same row's last cell.
    for _ in 0..3 {
        press(&mut picker, &library, Button::Left);
    }
    assert_eq!(picker.focus(), "Square: Page 2 of 3: Picture 109");
    press(&mut picker, &library, Button::Left);
    assert_eq!(picker.focus(), "Square: Page 1 of 3: Picture 103");
    // And Right past it the next, the same row's first cell.
    press(&mut picker, &library, Button::Right);
    assert_eq!(picker.focus(), "Square: Page 2 of 3: Picture 109");
}

#[test]
fn each_tab_keeps_its_own_page_and_asks_only_as_it_is_shown() {
    let library = library();
    let mut picker = picker(&library, true);
    serve(&mut picker, 120, 50);
    press(&mut picker, &library, Button::RightTrigger);
    press(&mut picker, &library, Button::RightBumper);
    // Cover: 14 cells, Automatic and the store's own first.
    assert_eq!(picker.focus(), "Cover: Page 1: Automatic");
    assert_eq!(serve(&mut picker, 60, 50), [0]);
    assert_eq!(picker.focus(), "Cover: Page 1 of 5: Automatic");
    press(&mut picker, &library, Button::Right);
    assert_eq!(picker.focus(), "Cover: Page 1 of 5: Store");
    // Page 4 is candidates 40..54: it needs the second page of the service's.
    for _ in 0..3 {
        press(&mut picker, &library, Button::RightTrigger);
    }
    assert_eq!(picker.focus(), "Cover: Page 4 of 5: Picture 141");
    assert_eq!(serve(&mut picker, 60, 50), [1]);
    press(&mut picker, &library, Button::RightBumper);
    press(&mut picker, &library, Button::RightBumper);
    assert!(picker.focus().starts_with("Background: "), "stops at the last tab");
    press(&mut picker, &library, Button::LeftBumper);
    press(&mut picker, &library, Button::LeftBumper);
    assert_eq!(picker.focus(), "Square: Page 2 of 13: Picture 109");
}

#[test]
fn a_chooses_and_b_leaves_it_as_it_was() {
    let library = library();
    let mut picker = picker(&library, true);
    serve(&mut picker, 5, 50);
    press(&mut picker, &library, Button::Right);
    let Some(PageAction::CloseWith(Effect::ChooseArt(choice))) = press(&mut picker, &library, Button::A) else {
        panic!("a choice")
    };
    assert_eq!(
        choice,
        Choice { game: game().key(), tab: Tab::Square, chosen: Some(Chosen::Picture(100)), url: Some("full/0".into()) }
    );
    press(&mut picker, &library, Button::Left);
    let Some(PageAction::CloseWith(Effect::ChooseArt(choice))) = press(&mut picker, &library, Button::A) else {
        panic!("a choice")
    };
    assert_eq!((choice.chosen, choice.url), (None, None), "Automatic");
    assert_eq!(press(&mut picker, &library, Button::B), Some(PageAction::Close));

    // The thumbnails of what is shown are asked for as the page comes, each once.
    let mut picker = self::picker(&library, true);
    assert_eq!(picker.art_asks().len(), 1);
    let items = (0..5).map(|n| Candidate { id: n, url: String::new(), thumb: format!("t{n}"), width: 1, height: 1 });
    let reply = Reply { page: 0, total: 5, limit: 50, items: items.collect() };
    picker.art_answer(Answer::Page { game: game().key(), tab: Tab::Square, page: 0, result: Ok(reply) });
    let thumbs = picker.art_asks().into_iter().filter(|ask| matches!(ask, Ask::Thumb { .. })).count();
    assert_eq!(thumbs, 5);
    assert!(picker.art_asks().is_empty());
}

#[test]
fn without_a_key_nothing_is_asked_and_it_says_where_to_set_one() {
    let library = library();
    let mut picker = picker(&library, false);
    assert!(picker.art_asks().is_empty());
    assert_eq!(picker.state(), "in use: Automatic; no key");
    assert_eq!(picker.focus(), "Square: Page 1 of 1: Automatic");
    press(&mut picker, &library, Button::Down);
    assert_eq!(picker.focus(), "Square: Page 1 of 1: Automatic", "no Retry: there is nothing to retry");
}

#[test]
fn a_failed_page_is_said_with_retry_which_asks_it_again() {
    let library = library();
    let mut picker = picker(&library, true);
    let asks = picker.art_asks();
    assert!(matches!(asks[..], [Ask::Page { page: 0, .. }]));
    let failed = Failure::Failed("SteamGridDB could not be reached".into());
    picker.art_answer(Answer::Page { game: game().key(), tab: Tab::Square, page: 0, result: Err(failed) });
    assert_eq!(picker.state(), "in use: Automatic; failed");
    assert!(picker.art_asks().is_empty(), "not again until Retry");
    press(&mut picker, &library, Button::Down);
    assert_eq!(picker.focus(), "Square: Page 1: Retry", "how many pages there are is not known");
    assert_eq!(press(&mut picker, &library, Button::A), None);
    assert_eq!(picker.focus(), "Square: Page 1: Automatic");
    assert!(matches!(picker.art_asks()[..], [Ask::Page { page: 0, .. }]));
    // An empty list is said too, with Retry.
    let empty = Reply { page: 0, total: 0, limit: 50, items: Vec::new() };
    picker.art_answer(Answer::Page { game: game().key(), tab: Tab::Square, page: 0, result: Ok(empty) });
    assert_eq!(picker.state(), "in use: Automatic; none");
    press(&mut picker, &library, Button::Down);
    assert_eq!(picker.focus(), "Square: Page 1 of 1: Retry");
}

#[test]
fn what_is_in_use_is_marked() {
    let mut game = game();
    game.art.chosen[Tab::Square.index()] = Some(Chosen::Picture(104));
    game.art.chosen[Tab::Cover.index()] = Some(Chosen::Store);
    let library = Library { games: vec![game], ..Library::default() };
    let mut picker = picker(&library, true);
    assert_eq!(picker.state(), "in use: Picture 104");
    press(&mut picker, &library, Button::RightBumper);
    assert_eq!(picker.state(), "in use: Store");
}
