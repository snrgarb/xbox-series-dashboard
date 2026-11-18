//! Games & apps > Launchers (docs/spec/launchers.md): which launchers games come in from, and Update now, which
//! reads every one switched on again. The Bundle has no such page (Playnite's own library settings did this job):
//! it is the port's, made of Settings' Rows and pane button in the Bundle's look.
//!
//! A Row a launcher: its mark, its name, what the library has of it and when it was last read, and a checkbox. A
//! on it switches the launcher, at once: the shell is asked (`Effect::Launcher`) and the page shows what the next
//! snapshot says. A launcher a game is playing from is not switched off: the page says to quit the game first.
//! Update now asks the shell (`Effect::UpdateLaunchers`), which reads off the UI thread and says how it goes in the
//! snapshots it hands over (`Library::launchers`): working, then what changed. Pressed while it works, it does
//! nothing.
//!
//! Under them, the PC Game Pass catalogue's two controls (#277): Show Game Pass games, a checkbox row
//! (`Setting::ShowGamePass`), and Game Pass region, a dropdown of Windows' own region and every market where PC Game
//! Pass is sold (`Effect::GamePassRegion`). Either change has Xbox read again.
use std::time::{SystemTime, UNIX_EPOCH};

use model::game_pass::{self, Region};
use model::launcher::{self, Found, Launcher, Update};
use ui::{Button, Color, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{
    self, glyph, Dropdown, BOXES_TOP, FIELD_HEIGHT, FIELD_WIDTH, LEFT, RIGHT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH,
};
use super::{Change, Pane, Setting, Step};
use crate::home::{self, Source};
use crate::pages::{Around, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

/// The box on Games & apps, and the page's header.
pub const TITLE: &str = "Launchers";
const UPDATE: &str = "Update now";
const WORKING: &str = "Updating your library...";
const FAILED: &str = "Your library couldn't be updated. Try again later.";
const GAME_PASS: &str = "Game Pass";
const SHOW_GAME_PASS: &str = "Show Game Pass games";
const REGION: &str = "Game Pass region";
/// How tall an open region list is kept on screen at most, for the page's scroll: six choices.
const LIST_SHOWN: f32 = 8.0 + 18.0 + 62.0 * 6.0;
/// How dim a launcher that is not on this PC is drawn.
const ABSENT: f32 = 0.4;

/// Each launcher's mark, by its key.
fn mark(key: &str) -> Option<Source> {
    home::source(&model::Game { source: key.to_string(), ..model::Game::default() }).filter(|_| !key.is_empty())
}

/// The launcher's name, as the Game hub writes it.
fn name(key: &str) -> &'static str {
    mark(key).map_or("A launcher", Source::name)
}

/// What focus is on, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Launcher(String),
    Update,
    ShowGamePass,
    Region,
}

/// The region list's choices: Windows' own region first, then every market where PC Game Pass is sold, by name.
fn regions(windows: Option<Region>) -> Vec<String> {
    let windows = match windows {
        Some(region) => format!("Windows' region ({})", region.name()),
        None => "Windows' region".to_string(),
    };
    std::iter::once(windows).chain(game_pass::MARKETS.iter().map(|(_, name)| name.to_string())).collect()
}

/// The region chosen's place among [`regions`]' choices: 0 for Windows' own.
fn region_place(chosen: Option<Region>) -> usize {
    chosen.and_then(|region| game_pass::markets().position(|market| market == region)).map_or(0, |at| at + 1)
}

/// Every launcher as the library has them; with none (a library from no database), each on, its games counted.
fn launchers(around: Around) -> Vec<Launcher> {
    if !around.library.launchers.list.is_empty() {
        return around.library.launchers.list.clone();
    }
    let all = || around.library.games.iter().chain(&around.library.hidden);
    let launcher = |key: &str| Launcher {
        key: key.to_string(),
        on: true,
        found: Found::NotYet,
        read: None,
        games: all().filter(|game| game.source == key).count(),
    };
    launcher::ALL.iter().map(|key| launcher(key)).collect()
}

/// `17 games from 3 launchers`: what the library shows of the launchers switched on.
fn count_line(around: Around) -> String {
    let all = around.library.games.iter().chain(&around.library.hidden);
    let sources: Vec<&str> =
        all.map(|game| game.source.as_str()).filter(|source| launcher::ALL.contains(source)).collect();
    let mut keys = sources.clone();
    keys.sort_unstable();
    keys.dedup();
    let games = if sources.len() == 1 { "1 game".to_string() } else { format!("{} games", sources.len()) };
    match keys.len() {
        0 => "No games from your launchers".to_string(),
        1 => format!("{games} from 1 launcher"),
        n => format!("{games} from {n} launchers"),
    }
}

/// When it was read, `seconds` ago: `read just now`, `read 5 minutes ago`, `read 2 hours ago`, `read 3 days ago`.
fn ago(seconds: u64) -> String {
    let (n, unit) = match seconds {
        0..60 => return "read just now".to_string(),
        60..3600 => (seconds / 60, "minute"),
        3600..86_400 => (seconds / 3600, "hour"),
        _ => (seconds / 86_400, "day"),
    };
    format!("read {n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// What a Row says of its launcher, beside its name.
fn status(launcher: &Launcher, now: u64) -> String {
    let games = if launcher.games == 1 { "1 game".to_string() } else { format!("{} games", launcher.games) };
    match (launcher.on, launcher.found, launcher.read) {
        (false, ..) => format!("Off: {games} kept"),
        (true, Found::NotInstalled, _) if launcher.games == 0 => "Not installed".to_string(),
        (true, Found::NotInstalled, _) => format!("Not installed: {games} kept"),
        (true, Found::Failed, _) => format!("{games}, couldn't be read"),
        (true, _, Some(read)) => format!("{games}, {}", ago(now.saturating_sub(read))),
        (true, _, None) => "Not read yet".to_string(),
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// What the line beside Update now says once it is done: `Updated: 1 new game, play time for 1.`
fn done(changes: &str) -> String {
    if changes.is_empty() || changes == "nothing new" {
        "Your library is up to date.".to_string()
    } else {
        format!("Updated: {changes}.")
    }
}

pub struct Launchers {
    /// Every launcher, as the library had them when last told: what the focus log says comes from here.
    list: Vec<Launcher>,
    /// The count line, as of then.
    count: String,
    update: Update,
    /// The line focus is on.
    at: Line,
    /// Why the last press did nothing, said beside Update now until focus moves: a game playing from the launcher.
    note: Option<String>,
    scroll: f32,
    region: Dropdown,
    /// What the Game Pass controls are set to: shown, the region chosen, Windows' region. Taken from the options at
    /// each press, and changed at once by the press that asks for a change, so the focus line says it before the
    /// shell has carried it out.
    game_pass: (bool, Option<Region>, Option<Region>),
}

impl Launchers {
    /// The page, focus on its first launcher that can be switched.
    pub fn open(around: Around) -> Self {
        let mut page = Self {
            list: Vec::new(),
            count: String::new(),
            update: Update::Idle,
            at: Line::Update,
            note: None,
            scroll: 0.0,
            region: Dropdown::default(),
            game_pass: (true, None, None),
        };
        page.take(around);
        page.at = page.lines().swap_remove(0);
        page
    }

    /// Takes what the library says now.
    fn take(&mut self, around: Around) {
        self.list = launchers(around);
        self.count = count_line(around);
        self.update = around.library.launchers.update.clone();
        let options = around.options;
        self.game_pass = (options.show_game_pass, options.game_pass_region, options.windows_region);
    }

    /// What the Game Pass controls are set to, for the focus log: `Game Pass on, GB (Windows')`.
    fn game_pass_line(&self) -> String {
        let (shown, chosen, windows) = self.game_pass;
        let on = if shown { "on" } else { "off" };
        let region = match (chosen, windows) {
            (Some(region), _) => region.to_string(),
            (None, Some(windows)) => format!("{windows} (Windows')"),
            (None, None) => "Windows'".to_string(),
        };
        format!("{GAME_PASS} {on}, {region}")
    }

    /// The lines focus may be on: each launcher that can be switched, then Update now.
    fn lines(&self) -> Vec<Line> {
        let rows = self.list.iter().filter(|l| l.switchable()).map(|l| Line::Launcher(l.key.clone()));
        rows.chain([Line::Update, Line::ShowGamePass, Line::Region]).collect()
    }

    fn launcher(&self) -> Option<&Launcher> {
        match &self.at {
            Line::Launcher(key) => self.list.iter().find(|launcher| &launcher.key == key),
            _ => None,
        }
    }

    /// The game playing, or being launched, from this launcher, if one is.
    fn playing(around: Around, key: &str) -> Option<String> {
        let game = around.library.games.get(around.playing?)?;
        (game.source == key).then(|| game.name.clone())
    }

    /// Where the Rows start, under the count, and where Update now is: as drawn.
    fn layout(ui: &mut Ui) -> (f32, f32) {
        let rows = BOXES_TOP + ui.measure(&BODY, "0").height + 26.0;
        let update = rows + ROW_PITCH * (launcher::ALL.len() - 1) as f32 + ROW_HEIGHT + 40.0;
        (rows, update)
    }

    /// The Game Pass controls under Update now: its label's top, the checkbox row, the region's label's top and its
    /// dropdown. Each label's control 14 under it, the next label 48 under that, as on Guide's page.
    fn game_pass_layout(ui: &mut Ui, update_top: f32) -> (f32, Rect, f32, Rect) {
        let label = ui.measure(&BODY, "0").height;
        let heading = update_top + FIELD_HEIGHT + 40.0;
        let show = Rect::new(LEFT, heading + label + 14.0, FIELD_WIDTH, FIELD_HEIGHT);
        let region_label = show.bottom() + 48.0;
        let region = Rect::new(LEFT, region_label + label + 14.0, FIELD_WIDTH, FIELD_HEIGHT);
        (heading, show, region_label, region)
    }
}

impl Pane for Launchers {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `Launchers: Steam`, `Launchers: Update now`.
    fn focus(&self) -> String {
        match &self.at {
            Line::Launcher(key) => format!("Launchers: {}", name(key)),
            Line::Update => format!("Launchers: {UPDATE}"),
            Line::ShowGamePass => format!("Launchers: {SHOW_GAME_PASS}"),
            Line::Region => match self.region.focused() {
                None => format!("Launchers: {REGION}"),
                Some(choice) => format!("Launchers: {REGION}, list on {choice:?}"),
            },
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        if press.repeat && !press.button.is_direction() {
            return Step::Stay;
        }
        self.take(around);
        if self.region.is_open() {
            let markets: Vec<Region> = game_pass::markets().collect();
            let chosen = match self.region.press(press.button) {
                Some(0) => None,
                Some(at) => match markets.get(at - 1) {
                    Some(&region) => Some(region),
                    None => return Step::Stay,
                },
                None => return Step::Stay,
            };
            self.game_pass.1 = chosen;
            return Step::Do(PageAction::Effect(Effect::GamePassRegion(chosen)));
        }
        let lines = self.lines();
        let at = lines.iter().position(|line| *line == self.at).unwrap_or(0);
        match press.button {
            Button::Up | Button::Down => {
                let to = if press.button == Button::Up { at.saturating_sub(1) } else { (at + 1).min(lines.len() - 1) };
                if to != at {
                    self.note = None;
                }
                self.at = lines[to].clone();
            }
            Button::B | Button::Left => return Step::Back,
            Button::A => match self.at.clone() {
                // Once more while it works: nothing.
                Line::Update if self.update == Update::Working => {}
                Line::Update => {
                    self.note = None;
                    return Step::Do(PageAction::Effect(Effect::UpdateLaunchers));
                }
                Line::ShowGamePass => {
                    let change = Change { setting: Setting::ShowGamePass, on: !around.options.show_game_pass };
                    self.game_pass.0 = change.on;
                    return Step::Do(PageAction::Effect(Effect::Set(change)));
                }
                Line::Region => {
                    let options = around.options;
                    self.region.open(regions(options.windows_region), region_place(options.game_pass_region));
                }
                Line::Launcher(key) => {
                    let Some(on) = self.launcher().map(|launcher| launcher.on) else { return Step::Stay };
                    if on {
                        if let Some(game) = Self::playing(around, &key) {
                            self.note = Some(format!("Quit {game} before switching {} off.", name(&key)));
                            return Step::Stay;
                        }
                    }
                    self.note = None;
                    return Step::Do(PageAction::Effect(Effect::Launcher { key, on: !on }));
                }
            },
            _ => {}
        }
        Step::Stay
    }

    /// What the focused Row's launcher is (`on, 14 games`), the count line, and how the update goes (`ready`,
    /// `updating`, `updated: 1 new game`), or why the last press did nothing (`refused: Quit ...`).
    fn state(&self) -> String {
        let row = self.launcher().map(|l| {
            let found = match l.found {
                Found::NotInstalled => ", not installed",
                Found::Failed => ", unreadable",
                Found::Installed | Found::NotYet => "",
            };
            format!("{}, {} games{found}; ", if l.on { "on" } else { "off" }, l.games)
        });
        let update = match (&self.note, &self.update) {
            (Some(note), _) => format!("refused: {note}"),
            (None, Update::Idle) => "ready".to_string(),
            (None, Update::Working) => "updating".to_string(),
            (None, Update::Done(changes)) => format!("updated: {changes}"),
            (None, Update::Failed) => "update failed".to_string(),
        };
        let game_pass = match self.at {
            Line::ShowGamePass | Line::Region => format!("; {}", self.game_pass_line()),
            _ => String::new(),
        };
        format!("{}{}; {update}{game_pass}", row.unwrap_or_default(), self.count)
    }

    /// The launchers as the new snapshot has them; a launcher that can no longer be switched leaves focus on the
    /// first line.
    fn library_changed(&mut self, around: Around) -> Option<Box<dyn Pane>> {
        self.take(around);
        let mut lines = self.lines();
        if !lines.contains(&self.at) {
            self.at = lines.swap_remove(0);
        }
        None
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        self.take(around);
        let (rows_top, update_top) = Self::layout(ui);
        let (heading, show_row, region_label, region_box) = Self::game_pass_layout(ui, update_top);
        let row = |i: usize| Rect::new(LEFT, rows_top + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT);
        let update_box = Rect::new(LEFT, update_top, controls::pane_button_width(ui, UPDATE), FIELD_HEIGHT);
        let focused_rect = match &self.at {
            Line::Update => update_box,
            Line::Launcher(key) => row(self.list.iter().position(|l| &l.key == key).unwrap_or(0)),
            Line::ShowGamePass => show_row,
            // With its list open, as much of the list as six choices too.
            Line::Region if self.region.is_open() => {
                Rect::new(region_box.x, region_box.y, region_box.w, region_box.h + LIST_SHOWN)
            }
            Line::Region => region_box,
        };
        let now = now();
        let options = around.options;
        let region_value = match options.game_pass_region {
            Some(region) => region.name().to_string(),
            None => regions(options.windows_region).swap_remove(0),
        };
        let Self { list, count, update, at, note, scroll, region, .. } = self;
        controls::scrolled(ui, scroll, BOXES_TOP, focused_rect.y, focused_rect.bottom(), |ui| {
            controls::label(ui, LEFT, BOXES_TOP, count);
            let draw = |ui: &mut Ui, i: usize, launcher: &Launcher, focused: bool| {
                let alpha = if launcher.switchable() { 1.0 } else { ABSENT };
                ui.with_alpha(alpha, |ui| launcher_row(ui, row(i), launcher, now, focused));
            };
            // The focused Row last: its ring and glow lie over its neighbours.
            let has = |l: &Launcher| focused && *at == Line::Launcher(l.key.clone());
            for (i, launcher) in list.iter().enumerate().filter(|(_, l)| !has(l)) {
                draw(ui, i, launcher, false);
            }
            for (i, launcher) in list.iter().enumerate().filter(|(_, l)| has(l)) {
                draw(ui, i, launcher, true);
            }
            // Update now, and beside it how the update goes, or why the last press did nothing: nothing at rest.
            controls::pane_button(ui, update_box, UPDATE, true, focused && *at == Line::Update);
            let beside = update_box.right() + 30.0;
            let line = Rect::new(beside, update_box.y, RIGHT - beside, update_box.h);
            let said = |ui: &mut Ui, text: &str, colour: Color, line: Rect| {
                ui.paragraph(line, text, &BODY.color(colour), HAlign::Left, VAlign::Center, 1);
            };
            match (&*note, &*update) {
                (Some(note), _) => said(ui, note, Color::WHITE, line),
                (None, Update::Idle) => {}
                (None, Update::Working) => {
                    crate::spinner::spinner(ui, beside + 16.0, update_box.y + update_box.h / 2.0, 32.0);
                    let after = Rect::new(beside + 48.0, line.y, line.w - 48.0, line.h);
                    said(ui, WORKING, Color::WHITE, after);
                }
                (None, Update::Done(changes)) => said(ui, &done(changes), Color::WHITE, line),
                (None, Update::Failed) => said(ui, FAILED, Color::WHITE, line),
            }
            // The Game Pass catalogue's (#277): no line saying what they are for (settings.md 16.11).
            controls::label(ui, LEFT, heading, GAME_PASS);
            let on_show = focused && *at == Line::ShowGamePass;
            controls::checkbox_row(ui, show_row, SHOW_GAME_PASS, options.show_game_pass, on_show);
            // Beside it, whether the account has PC Game Pass, as far as this PC says (docs/spec/launchers.md 8).
            let beside = show_row.right() + 30.0;
            let status = Rect::new(beside, show_row.y, RIGHT - beside, show_row.h);
            let line = options.game_pass_subscription.line();
            ui.paragraph(status, line, &BODY.color(TEXT_DIM), HAlign::Left, VAlign::Center, 1);
            controls::label(ui, LEFT, region_label, REGION);
            region.draw(ui, region_box, &region_value, focused && *at == Line::Region);
        });
        // The list over everything, where the box is on screen now.
        let shown = Rect::new(region_box.x, region_box.y - *scroll, region_box.w, region_box.h);
        region.draw_list(ui, shown);
    }
}

/// A launcher's Row: a Row (settings.md 4.3) with the launcher's mark in the tick's slot, its name, what the
/// library has of it on the right, and a checkbox at the right end where it can be switched.
fn launcher_row(ui: &mut Ui, rect: Rect, launcher: &Launcher, now: u64, focused: bool) {
    controls::row_marked(ui, rect, name(&launcher.key), None, focused);
    let (_, cy) = rect.center();
    if let Some(source) = mark(&launcher.key) {
        home::source_mark(ui, Rect::new(rect.x + 45.0 - 14.0, cy - 14.0, 28.0, 28.0), source);
    }
    // The checkbox where a checkbox row has it, 18 in from the right; the status right-aligned 24 short of it.
    let check = rect.right() - 18.0 - 14.0;
    let status_right = check - 14.0 - 24.0;
    let status_box = Rect::new(rect.x + 340.0, rect.y, status_right - rect.x - 340.0, rect.h);
    ui.paragraph(status_box, &status(launcher, now), &BODY.color(TEXT_DIM), HAlign::Right, VAlign::Center, 1);
    if launcher.switchable() {
        let glyph = if launcher.on { glyph::CHECKBOX_TICKED } else { glyph::CHECKBOX };
        ui.icon(check, cy, glyph, 28.0, Color::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_says_what_the_library_has_of_its_launcher() {
        let launcher = |on, found, read, games| Launcher { key: "steam".into(), on, found, read, games };
        let now = 1_800_000_000;
        assert_eq!(status(&launcher(true, Found::Installed, Some(now - 30), 14), now), "14 games, read just now");
        assert_eq!(status(&launcher(true, Found::Installed, Some(now - 300), 1), now), "1 game, read 5 minutes ago");
        assert_eq!(status(&launcher(true, Found::Installed, Some(now - 7300), 3), now), "3 games, read 2 hours ago");
        assert_eq!(status(&launcher(true, Found::Installed, Some(now - 86_400), 3), now), "3 games, read 1 day ago");
        assert_eq!(status(&launcher(false, Found::Installed, Some(now), 14), now), "Off: 14 games kept");
        assert_eq!(status(&launcher(true, Found::NotInstalled, None, 0), now), "Not installed");
        assert_eq!(status(&launcher(true, Found::NotInstalled, Some(now), 2), now), "Not installed: 2 games kept");
        assert_eq!(status(&launcher(true, Found::Failed, Some(now), 2), now), "2 games, couldn't be read");
        assert_eq!(status(&launcher(true, Found::NotYet, None, 0), now), "Not read yet");
    }

    #[test]
    fn every_launcher_has_its_mark_and_its_name() {
        let names: Vec<&str> = launcher::ALL.iter().map(|key| name(key)).collect();
        assert_eq!(names, ["Steam", "Epic Games", "GOG", "EA app", "Ubisoft Connect", "Battle.net", "Xbox"]);
        assert!(launcher::ALL.iter().all(|key| mark(key).is_some()));
        // The library names each launcher's filter on Full library by the same names.
        assert!(launcher::ALL.iter().all(|key| launcher::name(key) == Some(name(key))));
    }

    #[test]
    fn the_region_list_is_windows_then_the_markets_and_opens_on_the_one_chosen() {
        let gb = Region::parse("GB");
        let choices = regions(gb);
        assert_eq!(choices[0], "Windows' region (United Kingdom)");
        assert_eq!(regions(None)[0], "Windows' region");
        assert_eq!(choices.len(), game_pass::MARKETS.len() + 1);
        assert_eq!(region_place(None), 0);
        assert_eq!(choices[region_place(gb)], "United Kingdom");
        assert_eq!(choices[region_place(Region::parse("AL"))], "Albania");
    }

    #[test]
    fn the_line_beside_update_now_says_what_changed() {
        assert_eq!(done("nothing new"), "Your library is up to date.");
        assert_eq!(done("1 new game, play time for 1"), "Updated: 1 new game, play time for 1.");
    }
}
