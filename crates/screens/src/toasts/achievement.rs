//! What the Achievement unlocked Toast tells of (docs/spec/achievement-toast.md), from the unlocks the achievements'
//! thread finds (docs/achievements.md): one unlock is "Achievement unlocked", with its icon, Score, name and
//! description; one Rare unlock told of alone is the rare form, "Rare Achievement unlocked - <n>%"; several found
//! together are "<n> achievements unlocked", Rare ones among them or not (the Bundle's `UnlocksToast`).
//!
//! Unlocks found within a second of each other are told of together, as the Bundle gathers those it hears of live
//! (`UnlocksGatherFor`): the first starts the second, and every one found by its end joins it.
use std::path::PathBuf;

use model::achievement::Unlock;
use model::Library;

use super::{Toast, ToastKind};

/// How long the first unlock waits for others found with it.
pub const GATHER_SECONDS: f32 = 1.0;

/// The pill's line for one unlock, and for one Rare unlock told of alone (the view adds " - <n>%").
pub const UNLOCKED: &str = "Achievement unlocked";
pub const RARE_UNLOCKED: &str = "Rare Achievement unlocked";

/// What an Achievement unlocked Toast tells of beyond its line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Told {
    /// The achievement's name: none for several, whose line is alone in the pill.
    pub name: Option<String>,
    /// Its Score: none for several.
    pub score: Option<u32>,
    /// The share of players who have it, 0 to 100: the rare form's alone, which writes it after its line.
    pub rarity: Option<f64>,
    /// What it asks of the player, the pill's pages after its name; none for several.
    pub description: Option<String>,
    /// Its icon in our cache, once fetched: the pill's circle shows it, the trophy until then or without one.
    pub icon: Option<PathBuf>,
}

impl Told {
    /// What the log says of it after the Toast's line: ` "No Detours", score 30, 7.4%`.
    pub fn line(&self) -> String {
        let mut line = String::new();
        if let Some(name) = &self.name {
            line.push_str(&format!(" {name:?}"));
        }
        if let Some(score) = self.score {
            line.push_str(&format!(", score {score}"));
        }
        if let Some(rarity) = self.rarity {
            line.push_str(&format!(", {}%", rarity_text(rarity)));
        }
        line
    }
}

/// A rarity as the rare form writes it, `{0:0.#}`: a whole number bare, else to a tenth, halves away from 0.
pub fn rarity_text(rarity: f64) -> String {
    let tenths = (rarity * 10.0).round();
    if tenths % 10.0 == 0.0 {
        format!("{:.0}", tenths / 10.0)
    } else {
        format!("{:.1}", tenths / 10.0)
    }
}

/// The Toast telling of `unlocks`, found together (at least one). `prompt`: its last page says to press the Guide
/// button, which with the Guide off does nothing, so it does not say so.
pub fn toast(unlocks: &[Unlock], prompt: bool) -> Toast {
    let game = unlocks.first().map(|unlock| unlock.game);
    let (kind, text, told) = match unlocks {
        [one] => {
            let achievement = &one.achievement;
            let rare = achievement.is_rare();
            let told = Told {
                name: Some(achievement.name.clone()).filter(|name| !name.trim().is_empty()),
                score: Some(achievement.score()),
                rarity: achievement.rarity.filter(|_| rare),
                description: achievement.description.clone().filter(|text| !text.trim().is_empty()),
                icon: achievement.icon_file.clone(),
            };
            if rare {
                (ToastKind::RareAchievement, RARE_UNLOCKED.to_string(), told)
            } else {
                (ToastKind::Achievement, UNLOCKED.to_string(), told)
            }
        }
        several => (ToastKind::Achievement, format!("{} achievements unlocked", several.len()), Told::default()),
    };
    Toast { game, told: Some(Box::new(told)), ..Toast::new(kind, text, prompt) }
}

/// The notification the Toast leaves in the Notifications Flyout: what the pill said, and of which game.
/// "Achievement unlocked in Ashfall Courier: Night Route", "Rare Achievement unlocked in Ashfall Courier: No
/// Detours", "3 achievements unlocked in Ashfall Courier".
pub fn notification(toast: &Toast, library: &Library) -> String {
    let game = toast.game.and_then(|id| library.games.iter().chain(&library.hidden).find(|game| game.id == id));
    let mut text = toast.text.clone();
    if let Some(game) = game {
        text.push_str(&format!(" in {}", game.name));
    }
    if let Some(name) = toast.told.as_ref().and_then(|told| told.name.as_deref()) {
        text.push_str(&format!(": {name}"));
    }
    text
}

/// Unlocks waiting for others found with them.
#[derive(Debug, Default)]
pub struct Gather {
    unlocks: Vec<Unlock>,
    /// How long until they are told of; none while none waits.
    left: Option<f32>,
}

impl Gather {
    /// Unlocks found: the first starts the wait, the rest join it.
    pub fn add(&mut self, unlocks: Vec<Unlock>) {
        if unlocks.is_empty() {
            return;
        }
        self.unlocks.extend(unlocks);
        self.left.get_or_insert(GATHER_SECONDS);
    }

    /// This much time has passed.
    pub fn pass(&mut self, seconds: f32) {
        if let Some(left) = &mut self.left {
            *left -= seconds.max(0.0);
        }
    }

    /// The unlocks to tell of in one Toast, once their wait is over.
    pub fn take(&mut self) -> Option<Vec<Unlock>> {
        if self.left? > 0.0 {
            return None;
        }
        self.left = None;
        Some(std::mem::take(&mut self.unlocks))
    }

    /// How long until the wait is over: none while none waits.
    pub fn timer(&self) -> Option<f32> {
        self.left.map(|left| left.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use model::achievement::Achievement;

    use super::*;

    fn unlock(key: &str, rarity: Option<f64>) -> Unlock {
        let achievement = Achievement {
            key: key.into(),
            name: format!("Name of {key}"),
            description: Some(format!("Do {key}.")),
            rarity,
            unlocked: Some(1),
            icon_file: Some(PathBuf::from(format!("{key}.jpg"))),
            ..Default::default()
        };
        Unlock { game: 7, achievement }
    }

    #[test]
    fn one_unlock_is_its_name_score_and_description_with_its_icon() {
        let toast = toast(&[unlock("A", Some(18.2))], true);
        assert_eq!(
            (toast.kind, toast.text.as_str(), toast.prompt, toast.game),
            (ToastKind::Achievement, UNLOCKED, true, Some(7))
        );
        let told = toast.told.as_deref().unwrap();
        assert_eq!(told.name.as_deref(), Some("Name of A"));
        assert_eq!((told.score, told.rarity), (Some(20), None));
        assert_eq!(told.description.as_deref(), Some("Do A."));
        assert_eq!(told.icon, Some(PathBuf::from("A.jpg")));
        assert_eq!(toast.told_line(), " \"Name of A\", score 20");
    }

    #[test]
    fn a_rare_one_alone_is_the_rare_form_with_its_rarity_and_unknown_is_never_rare() {
        let rare = toast(&[unlock("R", Some(7.4))], true);
        assert_eq!((rare.kind, rare.text.as_str()), (ToastKind::RareAchievement, RARE_UNLOCKED));
        assert_eq!(rare.told.as_ref().unwrap().rarity, Some(7.4));
        assert_eq!(rare.told_line(), " \"Name of R\", score 30, 7.4%");
        assert_eq!(toast(&[unlock("U", None)], true).kind, ToastKind::Achievement);
        assert_eq!(toast(&[unlock("T", Some(10.0))], true).kind, ToastKind::Achievement);
    }

    #[test]
    fn several_are_one_ordinary_toast_rare_ones_among_them_or_not() {
        let toast = toast(&[unlock("A", Some(40.0)), unlock("R", Some(0.8)), unlock("B", None)], false);
        assert_eq!(
            (toast.kind, toast.text.as_str(), toast.prompt),
            (ToastKind::Achievement, "3 achievements unlocked", false)
        );
        assert_eq!(toast.told.as_deref(), Some(&Told::default()));
        assert_eq!(toast.told_line(), "");
    }

    #[test]
    fn the_rarity_is_whole_or_to_a_tenth() {
        assert_eq!(rarity_text(1.0), "1");
        assert_eq!(rarity_text(0.3), "0.3");
        assert_eq!(rarity_text(0.25), "0.3");
        assert_eq!(rarity_text(4.71), "4.7");
        assert_eq!(rarity_text(12.0), "12");
        assert_eq!(rarity_text(9.96), "10");
    }

    #[test]
    fn its_notification_names_the_game() {
        let mut library = Library::default();
        library.games.push(model::Game { id: 7, name: "Ashfall Courier".into(), ..Default::default() });
        let one = toast(&[unlock("A", Some(40.0))], true);
        assert_eq!(notification(&one, &library), "Achievement unlocked in Ashfall Courier: Name of A");
        let rare = toast(&[unlock("R", Some(4.0))], true);
        assert_eq!(notification(&rare, &library), "Rare Achievement unlocked in Ashfall Courier: Name of R");
        let several = toast(&[unlock("A", None), unlock("B", None)], true);
        assert_eq!(notification(&several, &library), "2 achievements unlocked in Ashfall Courier");
        assert_eq!(notification(&one, &Library::default()), "Achievement unlocked: Name of A");
    }

    #[test]
    fn unlocks_found_within_a_second_are_told_of_together() {
        let mut gather = Gather::default();
        gather.pass(5.0);
        assert_eq!((gather.take(), gather.timer()), (None, None));
        gather.add(vec![unlock("A", None)]);
        assert_eq!(gather.timer(), Some(1.0));
        gather.pass(0.6);
        assert_eq!(gather.take(), None);
        gather.add(vec![unlock("B", None)]);
        // The second waits out the first's second, not one of its own.
        gather.pass(0.4);
        let told = gather.take().unwrap();
        assert_eq!(told.iter().map(|u| u.achievement.key.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        gather.pass(5.0);
        assert_eq!((gather.take(), gather.timer()), (None, None));
        gather.add(Vec::new());
        assert_eq!(gather.timer(), None);
    }
}
