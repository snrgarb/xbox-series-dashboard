use model::art_match::Hit;
use model::ArtOrder;
use ui::Press;

use super::*;
use crate::settings::Options;

fn game() -> Game {
    Game {
        id: 4,
        source: "battlenet".into(),
        source_id: "wow".into(),
        name: "Made-up Realm: Forever".into(),
        ..Game::default()
    }
}

fn library(matches: Vec<Match>) -> Library {
    Library { games: vec![Game { matches, ..game() }], ..Library::default() }
}

fn options(key: bool) -> Options {
    Options { art_key: key, art_online: true, ..Options::default() }
}

fn around(library: &Library, options: Options) -> Around<'_> {
    Around {
        library,
        playing: None,
        options,
        now: std::time::Instant::now(),
        now_playing: crate::now_playing::none(),
        discord: crate::no_discord(),
    }
}

fn page(library: &Library, options: Options) -> MatchGame {
    MatchGame::open(Entry { game: game().key() }, around(library, options)).unwrap()
}

fn press(page: &mut MatchGame, library: &Library, button: Button) -> Option<PageAction> {
    page.press(Press { button, repeat: false }, around(library, options(true)))
}

fn hit(id: &str, name: &str) -> Hit {
    Hit { id: id.into(), name: name.into(), year: None }
}

/// Answers the search the page asks for with these hits; gives the words it was asked for.
fn serve(page: &mut MatchGame, hits: Vec<Hit>) -> Vec<(ArtSource, String)> {
    let asks = page.art_asks();
    let mut asked = Vec::new();
    for ask in asks {
        let Ask::Search { game, source, words } = ask else { panic!("only searches: {ask:?}") };
        asked.push((source, words.clone()));
        page.art_answer(Answer::Results { game, source, words, result: Ok(hits.clone()) });
    }
    asked
}

#[test]
fn each_source_is_searched_for_the_games_name_once_as_its_tab_is_first_shown() {
    let library = library(vec![]);
    let mut page = page(&library, options(true));
    assert_eq!(page.focus(), "Steam: Automatic");
    assert_eq!(page.state(), "matched: Automatic; loading");
    assert_eq!(serve(&mut page, vec![]), [(ArtSource::Steam, "Made-up Realm: Forever".to_string())]);
    assert_eq!(page.state(), "matched: Automatic; none", "found nothing");
    assert!(page.art_asks().is_empty(), "asked once");
    press(&mut page, &library, Button::RightBumper);
    press(&mut page, &library, Button::RightBumper);
    press(&mut page, &library, Button::RightBumper);
    assert_eq!(page.focus(), "SteamGridDB: Automatic");
    assert_eq!(
        serve(&mut page, vec![hit("9", "Made-up Realm")]),
        [(ArtSource::SteamGridDb, "Made-up Realm: Forever".into())]
    );
    assert_eq!(page.state(), "matched: Automatic; 1 found");
    // The tabs stop at the ends.
    press(&mut page, &library, Button::RightBumper);
    assert_eq!(page.focus(), "SteamGridDB: Automatic");
}

#[test]
fn other_words_are_searched_for_and_a_result_matches_the_game_to_it() {
    let library = library(vec![]);
    let mut page = page(&library, options(true));
    for _ in 0..3 {
        press(&mut page, &library, Button::RightBumper);
    }
    serve(&mut page, vec![]);
    // Y opens the keyboard on the words; what it keeps is searched for.
    assert_eq!(press(&mut page, &library, Button::Y), Some(PageAction::Open(Open::TextEntry(WORDS))));
    assert_eq!(page.focus(), "SteamGridDB: Search");
    assert_eq!(page.entry_text(), "Made-up Realm: Forever");
    page.entry(keyboard::Event::Kept("  Made-up Realm ".into()), around(&library, options(true)));
    // An answer for the old words, come late, is passed over.
    let game = game().key();
    let late = Answer::Results {
        game: game.clone(),
        source: ArtSource::SteamGridDb,
        words: "Made-up Realm: Forever".into(),
        result: Ok(vec![hit("1", "Wrong")]),
    };
    page.art_answer(late);
    let asked = serve(&mut page, vec![hit("9", "Made-up Realm"), hit("10", "Made-up Realm Classic")]);
    assert_eq!(asked, [(ArtSource::SteamGridDb, "Made-up Realm".to_string())]);
    press(&mut page, &library, Button::Down);
    press(&mut page, &library, Button::Down);
    press(&mut page, &library, Button::Down);
    assert_eq!(page.focus(), "SteamGridDB: Result 2 (10)");
    press(&mut page, &library, Button::Down);
    assert_eq!(page.focus(), "SteamGridDB: Result 2 (10)", "the list ends");
    press(&mut page, &library, Button::Up);
    let matched = Match { source: ArtSource::SteamGridDb, id: "9".into(), name: "Made-up Realm".into() };
    assert_eq!(
        press(&mut page, &library, Button::A),
        Some(PageAction::CloseWith(Effect::MatchGame { game, source: ArtSource::SteamGridDb, matched: Some(matched) }))
    );
}

#[test]
fn automatic_forgets_a_match_and_choosing_what_is_in_use_changes_nothing() {
    let steam = Match { source: ArtSource::Steam, id: "70".into(), name: "Made-up Realm Classic".into() };
    let library = library(vec![steam.clone()]);
    let mut page = page(&library, options(true));
    serve(&mut page, vec![hit("70", "Made-up Realm Classic")]);
    assert_eq!(page.state(), "matched: 70; 1 found");
    press(&mut page, &library, Button::Down);
    assert_eq!(press(&mut page, &library, Button::A), Some(PageAction::Close), "already its match");
    press(&mut page, &library, Button::Up);
    let game = game().key();
    assert_eq!(
        press(&mut page, &library, Button::A),
        Some(PageAction::CloseWith(Effect::MatchGame { game, source: ArtSource::Steam, matched: None }))
    );
}

#[test]
fn a_source_that_cant_be_searched_asks_nothing_and_says_why() {
    let library = library(vec![]);
    let mut off = ArtOrder::default().switched(ArtSource::Xbox);
    off = off.moved(ArtSource::SteamGridDb, -3);
    let mut page = page(&library, Options { art_order: off, ..options(false) });
    // SteamGridDB first, as Settings' order says, with no key.
    assert_eq!(page.focus(), "SteamGridDB: Automatic");
    assert!(page.art_asks().is_empty());
    assert_eq!(page.state(), "matched: Automatic; no key");
    press(&mut page, &library, Button::RightBumper);
    press(&mut page, &library, Button::RightBumper);
    assert_eq!(page.focus(), "Xbox: Automatic");
    assert!(page.art_asks().is_empty());
    assert_eq!(page.state(), "matched: Automatic; source off");

    let mut page = page_offline(&library);
    assert!(page.art_asks().is_empty());
    assert_eq!(page.state(), "matched: Automatic; offline");
}

fn page_offline(library: &Library) -> MatchGame {
    page(library, Options { art_online: false, ..options(true) })
}
