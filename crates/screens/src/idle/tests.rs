use super::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn press(button: Button) -> Press {
    Press { button, repeat: false }
}

fn tip(n: u8) -> Card {
    Card {
        kind: cards::Kind::Tip,
        header: Some("Tip".into()),
        title: format!("Tip {n}"),
        label: None,
        description: None,
        game: None,
        prompt: "Got it",
        buttons: &[],
        score: None,
        rare: false,
        progress: None,
        icon: None,
    }
}

fn pool() -> Vec<Card> {
    (0..4).map(tip).collect()
}

/// An Idle at the start, with times of 3 s and 6 s.
fn idle() -> (Idle, Instant) {
    let start = Instant::now();
    let mut idle = Idle::new(start);
    idle.set_times(Times { dim_after: Some(secs(3)), show_after: Some(secs(6)) }, start);
    (idle, start)
}

#[test]
fn it_dims_after_dim_after_then_shows_things_after_show_things_after_counted_from_the_last_input() {
    let (mut idle, start) = idle();
    assert!(!idle.tick(start + ms(2999), pool));
    assert_eq!(idle.stage(), Stage::Awake);
    assert_eq!(idle.due(start + ms(2999)), Some(ms(1)));
    assert!(idle.tick(start + secs(3), pool));
    assert_eq!(idle.stage(), Stage::Dimmed);
    assert_eq!(idle.take_news(), [News::Dimmed]);
    // Show things after counts from the last input too, not from the dim.
    assert_eq!(idle.due(start + secs(4)), Some(secs(2)));
    assert!(idle.tick(start + secs(6), pool));
    assert_eq!(idle.stage(), Stage::ShowingThings);
    assert_eq!(idle.take_news(), [News::ShowingThings]);
    assert!(idle.card().is_some());
}

#[test]
fn input_anywhere_counts_from_the_later_of_it_and_the_last() {
    let (mut idle, start) = idle();
    idle.input_at(start + secs(2), start + secs(2));
    idle.tick(start + secs(4), pool);
    assert_eq!(idle.stage(), Stage::Awake, "counted from the input at 2 s");
    // An older moment changes nothing.
    idle.input_at(start + secs(1), start + secs(4));
    idle.tick(start + secs(5), pool);
    assert_eq!(idle.stage(), Stage::Dimmed);
    // A press counts as input as well.
    let (mut idle, start) = self::idle();
    assert_eq!(idle.press(press(Button::Down), start + secs(2)), Pressed::Pass);
    idle.tick(start + ms(4999), pool);
    assert_eq!(idle.stage(), Stage::Awake);
}

#[test]
fn a_press_while_dimmed_only_wakes_it_and_its_repeats_and_release_do_nothing() {
    let (mut idle, start) = idle();
    idle.tick(start + secs(3), pool);
    idle.take_news();
    let at = start + secs(4);
    assert_eq!(idle.press(press(Button::A), at), Pressed::Woke(None));
    assert_eq!(idle.stage(), Stage::Awake);
    assert_eq!(idle.take_news(), [News::Woken(Woken::Button(Button::A))]);
    assert_eq!(idle.press(Press { button: Button::A, repeat: true }, at + ms(400)), Pressed::Swallowed);
    assert!(idle.release(Button::A));
    // The next press of it is a press.
    assert_eq!(idle.press(press(Button::A), at + secs(1)), Pressed::Pass);
    assert!(!idle.release(Button::A));
    // And the count starts again from the wake.
    idle.tick(at + ms(2999), pool);
    assert_eq!(idle.stage(), Stage::Awake);
}

#[test]
fn a_key_has_no_release_so_its_next_press_is_a_press() {
    let (mut idle, start) = idle();
    idle.tick(start + secs(3), pool);
    assert!(matches!(idle.press(press(Button::Down), start + secs(4)), Pressed::Woke(_)));
    assert_eq!(idle.press(press(Button::Down), start + secs(5)), Pressed::Pass);
}

#[test]
fn the_guide_buttons_wake_swallows_its_tap_or_its_hold() {
    let (mut idle, start) = idle();
    idle.tick(start + secs(3), pool);
    idle.take_news();
    // A tap: down, released, its tap.
    assert!(idle.guide_down(start + secs(4)));
    assert_eq!(idle.take_news(), [News::Woken(Woken::Guide)]);
    assert!(idle.guide_release());
    assert!(idle.guide_tap());
    // The next is the Guide's again.
    assert!(!idle.guide_down(start + secs(5)));
    assert!(!idle.guide_release());
    assert!(!idle.guide_tap());

    // A hold: down, held, released, and no tap.
    idle.tick(start + secs(8), pool);
    assert!(idle.guide_down(start + secs(9)));
    assert!(idle.guide_held());
    assert!(idle.guide_release());
    assert!(!idle.guide_tap());
    assert!(!idle.guide_held());
}

#[test]
fn input_only_the_clock_saw_wakes_it_after_its_grace_unless_a_press_comes_first() {
    let (mut idle, start) = idle();
    idle.tick(start + secs(3), pool);
    idle.take_news();
    // The mouse moved: the dim stays while a press may yet come.
    let at = start + secs(4);
    idle.input_at(at, at);
    assert!(!idle.tick(at + ms(100), pool));
    assert_eq!(idle.stage(), Stage::Dimmed);
    assert_eq!(idle.due(at + ms(100)), Some(GRACE - ms(100)));
    // None came: awake once the grace is up, by input.
    assert!(idle.tick(at + GRACE, pool));
    assert_eq!(idle.stage(), Stage::Awake);
    assert_eq!(idle.take_news(), [News::Woken(Woken::Input)]);

    // A press that comes in the grace is the wake press, swallowed.
    let (mut idle, start) = self::idle();
    idle.tick(start + secs(3), pool);
    idle.take_news();
    idle.input_at(start + secs(4), start + secs(4));
    assert_eq!(idle.press(press(Button::B), start + secs(4) + ms(20)), Pressed::Woke(None));
    assert_eq!(idle.take_news(), [News::Woken(Woken::Button(Button::B))]);
    assert!(!idle.tick(start + secs(5), pool));
}

#[test]
fn typing_while_dimmed_only_wakes_it() {
    let (mut idle, start) = idle();
    idle.tick(start + secs(3), pool);
    idle.take_news();
    assert!(idle.typed(start + secs(4)));
    assert_eq!(idle.take_news(), [News::Woken(Woken::Typing)]);
    assert!(!idle.typed(start + secs(5)));
}

#[test]
fn the_wake_press_on_a_card_says_which_card_showed() {
    let (mut idle, start) = idle();
    idle.tick(start + secs(6), pool);
    let showing = idle.card().map(|(_, card)| card.clone());
    assert!(showing.is_some());
    assert_eq!(idle.press(press(Button::Y), start + secs(7)), Pressed::Woke(showing.clone().map(Box::new)));
    // The card stays to fade out with the dim.
    assert_eq!(idle.card().map(|(_, card)| card.clone()), showing);
}

#[test]
fn each_card_shows_for_twenty_seconds_in_shuffled_turn_and_none_twice_running() {
    let (mut idle, start) = idle();
    idle.seed(7);
    idle.tick(start + secs(6), pool);
    let mut seen = vec![idle.card().unwrap().1.clone()];
    assert_eq!(idle.due(start + secs(6)), Some(CARD_FOR));
    for turn in 1..=12u32 {
        let at = start + secs(6) + CARD_FOR * turn;
        assert!(!idle.tick(at - ms(1), pool));
        assert!(idle.tick(at, pool));
        let (number, card) = idle.card().unwrap();
        assert_eq!(number, 1 + u64::from(turn));
        assert_ne!(card, seen.last().unwrap(), "turn {turn}");
        seen.push(card.clone());
    }
    // Each round of four shows every card once.
    for round in seen[..12].chunks(4) {
        let mut titles: Vec<_> = round.iter().map(|card| card.title.clone()).collect();
        titles.sort();
        assert_eq!(titles, ["Tip 0", "Tip 1", "Tip 2", "Tip 3"]);
    }
}

#[test]
fn equal_times_dim_and_show_things_together_and_never_switches_a_stage_off() {
    let start = Instant::now();
    let mut idle = Idle::new(start);
    idle.set_times(Times { dim_after: Some(secs(3)), show_after: Some(secs(3)) }, start);
    idle.tick(start + secs(3), pool);
    assert_eq!(idle.stage(), Stage::ShowingThings);
    assert_eq!(idle.take_news(), [News::Dimmed, News::ShowingThings]);

    let mut idle = Idle::new(start);
    idle.set_times(Times { dim_after: Some(secs(3)), show_after: None }, start);
    idle.tick(start + secs(3600), pool);
    assert_eq!(idle.stage(), Stage::Dimmed);
    assert_eq!(idle.due(start + secs(3600)), None);

    let mut idle = Idle::new(start);
    idle.set_times(Times { dim_after: None, show_after: None }, start);
    idle.tick(start + secs(100_000), pool);
    assert_eq!(idle.stage(), Stage::Awake);
    assert_eq!(idle.due(start), None);
}

#[test]
fn the_launch_screen_holds_it_off_and_the_count_starts_again_as_it_goes() {
    let (mut idle, start) = idle();
    idle.wait(true, start + secs(1));
    assert!(!idle.tick(start + secs(30), pool));
    assert_eq!(idle.stage(), Stage::Awake);
    assert_eq!(idle.due(start + secs(30)), None);
    idle.wait(false, start + secs(30));
    idle.tick(start + secs(32), pool);
    assert_eq!(idle.stage(), Stage::Awake);
    idle.tick(start + secs(33), pool);
    assert_eq!(idle.stage(), Stage::Dimmed);
}

#[test]
fn new_times_count_from_when_they_are_set() {
    let (mut idle, start) = idle();
    idle.set_times(Times { dim_after: Some(secs(10)), show_after: Some(secs(20)) }, start + secs(5));
    idle.tick(start + secs(14), pool);
    assert_eq!(idle.stage(), Stage::Awake);
    idle.tick(start + secs(15), pool);
    assert_eq!(idle.stage(), Stage::Dimmed);
}

#[test]
fn show_things_after_is_never_shorter_than_dim_after() {
    let times = Times::default();
    assert_eq!(times.show_steps().len(), 10);
    assert_eq!(times.show_steps()[0], minutes(2));
    // Raising Dim after past Show things after raises it too; lowering it leaves it.
    let raised = times.with_dim(minutes(10)).unwrap();
    assert_eq!(raised, Times { dim_after: minutes(10), show_after: minutes(10) });
    assert_eq!(raised.with_dim(minutes(1)).unwrap(), Times { dim_after: minutes(1), show_after: minutes(10) });
    let never = times.with_dim(None).unwrap();
    assert_eq!(never, Times { dim_after: None, show_after: None });
    assert_eq!(never.show_steps(), [None]);
    // Refused: shorter than Dim after, not a step, the one set.
    assert_eq!(times.with_show(minutes(1)), None);
    assert_eq!(times.with_show(Some(secs(90))), None);
    assert_eq!(times.with_show(minutes(5)), None);
    assert_eq!(times.with_dim(Some(secs(7))), None);
    assert_eq!(times.with_show(None), Some(Times { dim_after: minutes(2), show_after: None }));
}

#[test]
fn the_times_are_read_back_as_they_were_saved_and_a_bad_one_is_its_default() {
    let times = Times { dim_after: minutes(15), show_after: None };
    let saved = times.saved();
    assert_eq!(saved, [(DIM_KEY, "900".to_string()), (SHOW_KEY, "never".to_string())]);
    let read = Times::read(|key| saved.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()));
    assert_eq!(read, times);
    assert_eq!(Times::read(|_| None), Times::default());
    // Not a step: its default. Shorter than Dim after: raised to it.
    let read = Times::read(|key| Some(if key == DIM_KEY { "600" } else { "61" }.to_string()));
    assert_eq!(read, Times { dim_after: minutes(10), show_after: minutes(10) });
}

#[test]
fn the_test_switch_takes_seconds_and_never() {
    assert_eq!(Times::parse_seconds("3,6"), Ok(Times { dim_after: Some(secs(3)), show_after: Some(secs(6)) }));
    assert_eq!(Times::parse_seconds("4,never"), Ok(Times { dim_after: Some(secs(4)), show_after: None }));
    assert_eq!(Times::parse_seconds("5,2"), Ok(Times { dim_after: Some(secs(5)), show_after: Some(secs(5)) }));
    assert!(Times::parse_seconds("5").is_err());
    assert!(Times::parse_seconds("a,b").is_err());
}

#[test]
fn a_step_is_worded_as_its_dropdown_has_it() {
    let words: Vec<_> = STEPS.iter().map(|&step| describe(step)).collect();
    assert_eq!(words, ["1 min", "2 min", "5 min", "10 min", "15 min", "30 min", "1 h", "2 h", "3 h", "4 h", "Never"]);
    assert_eq!(describe(Some(secs(3))), "3 s");
    assert_eq!(Times::default().describe(), "dim after 2 min, show things after 5 min");
}
