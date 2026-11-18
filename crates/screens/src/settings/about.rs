//! System > About (docs/spec/settings.md, 16.10; docs/spec/updates.md): which Dashboard this is, its version
//! and the commit it was built from; updates, checked by hand or once a day, and installed; and what comes with it
//! and what does not. The Bundle has no such page (Playnite's own About said Playnite's version): its words are the
//! port's own, laid out as System's, a label and its text a group, the next group 40 under.
//!
//! The page shows updates as the executable last said (`Options::updates`) and asks with `Effect::CheckForUpdates`
//! and `Effect::InstallUpdate`; the daily check is an option like any other (`Setting::DailyUpdateCheck`).
use model::update::Updates;
use ui::{Button, HAlign, Press, Rect, Ui, VAlign};

use super::controls::{self, FIELD_HEIGHT, FIELD_WIDTH, LEFT, ROW_WIDTH, TOP};
use super::options::{Change, Setting};
use super::{Pane, Step};
use crate::pages::{Around, PageAction};
use crate::theme::{BODY, TEXT_DIM};
use crate::Effect;

pub const TITLE: &str = "About";
const NAME: &str = "Xbox Series Dashboard";
const UPDATES_LABEL: &str = "Updates";
pub const CHECK: &str = "Check for updates";
pub const INSTALL: &str = "Install";
pub const DAILY: &str = "Check for updates every day";
const LICENCES_LABEL: &str = "Third-party licences";
const LICENCES: &str = "The Dashboard is built from open-source parts. Their licences are in \
                        THIRD-PARTY-LICENSES.txt, beside dashboard.exe.";
const MEDIA_LABEL: &str = "Sounds and boot videos";
const MEDIA: &str = "The console's sounds and boot videos don't come with the Dashboard. It plays them from the \
                     Xbox Series Theme for Playnite, where that is installed, or from the media folder in \
                     %LOCALAPPDATA%\\XboxSeriesDashboard. The README beside dashboard.exe says how to fill it.";

/// Which build of the Dashboard this is: the workspace's version, and the commit it was built from (empty when
/// that is not known). The executable says its own (`app`'s `BUILD`); a test gives one that never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    pub version: &'static str,
    pub commit: &'static str,
}

impl Build {
    /// The version alone, the commit not known.
    pub const fn version(version: &'static str) -> Self {
        Self { version, commit: "" }
    }

    /// "Version 0.1.0 (0123abc)", or "Version 0.1.0" with no commit.
    pub fn says(&self) -> String {
        format!("Version {}", self.says_short())
    }

    /// "0.1.0 (0123abc)", or "0.1.0" with no commit: what `dashboard --version` prints after its name.
    pub fn says_short(&self) -> String {
        if self.commit.is_empty() {
            self.version.to_string()
        } else {
            format!("{} ({})", self.version, self.commit)
        }
    }
}

impl Default for Build {
    fn default() -> Self {
        Self::version(env!("CARGO_PKG_VERSION"))
    }
}

/// What the line under Updates says of where they stand; `playing`, a game is running.
pub fn status(updates: Updates, playing: bool) -> String {
    match updates {
        // Nothing to say before the first check (#261): the button says it.
        Updates::Unchecked => String::new(),
        Updates::Checking => "Checking for updates...".into(),
        Updates::UpToDate(_) => "This is the latest version.".into(),
        Updates::NotPublic => "Updates can't be checked yet: the Dashboard's releases aren't public.".into(),
        Updates::Failed => "Couldn't check for updates. Is the PC online? The Dashboard's log says why.".into(),
        Updates::Downloading(v) => format!("Version {v} is available. Downloading it..."),
        Updates::Ready(v) if playing => format!("Version {v} is ready. Quit the game to install it."),
        Updates::Ready(v) => format!("Version {v} is ready to install. The Dashboard restarts with it."),
        Updates::BadDownload(v) => {
            format!("Version {v} didn't match its release's checksum, so it wasn't installed.")
        }
        Updates::Installing(v) => format!("Installing version {v}..."),
        Updates::RolledBack(v) => format!("Version {v} didn't start, so this version was kept."),
        Updates::InstallFailed(v) => {
            format!("Version {v} couldn't be put in place, so nothing was changed. The Dashboard's log says why.")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Check,
    Install,
    Daily,
}

impl Control {
    fn name(self) -> &'static str {
        match self {
            Control::Check => CHECK,
            Control::Install => INSTALL,
            Control::Daily => DAILY,
        }
    }
}

/// The page, with the build it was opened on.
pub struct About {
    build: Build,
    at: Control,
}

impl About {
    pub fn open(around: Around) -> Self {
        Self { build: around.options.build, at: Control::Check }
    }
}

impl Pane for About {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `About: Version 0.1.0 (0123abc); Check for updates`: the version and the control focus is on. The Dashboard's
    /// focus line adds where updates stand (`Updates::state`, as `; ready 0.2.0`) from the options.
    fn focus(&self) -> String {
        format!("{TITLE}: {}; {}", self.build.says(), self.at.name())
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        let updates = around.options.updates;
        let effect = |effect| Step::Do(PageAction::Effect(effect));
        match (press.button, self.at) {
            (Button::B, _) if !press.repeat => Step::Back,
            (Button::Left, Control::Check | Control::Daily) if !press.repeat => Step::Back,
            (Button::Left, Control::Install) => {
                self.at = Control::Check;
                Step::Stay
            }
            (Button::Right, Control::Check) => {
                self.at = Control::Install;
                Step::Stay
            }
            (Button::Down, Control::Check | Control::Install) => {
                self.at = Control::Daily;
                Step::Stay
            }
            (Button::Up, Control::Daily) => {
                self.at = Control::Check;
                Step::Stay
            }
            (Button::A, _) if press.repeat => Step::Stay,
            (Button::A, Control::Check) if !updates.busy() => effect(Effect::CheckForUpdates),
            // Only with an update ready, and never over a running game.
            (Button::A, Control::Install) if matches!(updates, Updates::Ready(_)) && around.playing.is_none() => {
                effect(Effect::InstallUpdate)
            }
            (Button::A, Control::Daily) => {
                let change = Change { setting: Setting::DailyUpdateCheck, on: !around.options.daily_update_check };
                effect(Effect::Set(change))
            }
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let updates = around.options.updates;
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, NAME) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, &self.build.says()) + 40.0;
        // Updates: where they stand, the two buttons, then the daily check.
        let line = status(updates, around.playing.is_some());
        if line.is_empty() {
            // Not checked yet: the buttons 14 under the label, as a control under its label.
            y += controls::label(ui, LEFT, y, UPDATES_LABEL) + 14.0;
        } else {
            y += controls::label(ui, LEFT, y, UPDATES_LABEL) + 4.0;
            let style = BODY.color(TEXT_DIM);
            y +=
                ui.paragraph(Rect::new(LEFT, y, ROW_WIDTH, 2000.0), &line, &style, HAlign::Left, VAlign::Top, 0).height
                    + 18.0;
        }
        let check = Rect::new(LEFT, y, controls::pane_button_width(ui, CHECK), FIELD_HEIGHT);
        let install = Rect::new(check.right() + 18.0, y, controls::pane_button_width(ui, INSTALL), FIELD_HEIGHT);
        let ready = matches!(updates, Updates::Ready(_)) && around.playing.is_none();
        controls::pane_button(ui, check, CHECK, !updates.busy(), focused && self.at == Control::Check);
        controls::pane_button(ui, install, INSTALL, ready, focused && self.at == Control::Install);
        if updates.busy() {
            crate::spinner::spinner(ui, install.right() + 30.0 + 16.0, install.y + install.h / 2.0, 32.0);
        }
        y += FIELD_HEIGHT + 18.0;
        let daily = Rect::new(LEFT, y, FIELD_WIDTH, FIELD_HEIGHT);
        let on = around.options.daily_update_check;
        controls::checkbox_row(ui, daily, DAILY, on, focused && self.at == Control::Daily);
        y += FIELD_HEIGHT + 40.0;
        y += controls::label(ui, LEFT, y, LICENCES_LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, LICENCES) + 40.0;
        y += controls::label(ui, LEFT, y, MEDIA_LABEL) + 4.0;
        controls::description(ui, LEFT, y, ROW_WIDTH, MEDIA);
    }
}

#[cfg(test)]
mod tests {
    use model::update::Version;

    use super::*;

    #[test]
    fn the_version_with_its_commit_or_alone() {
        assert_eq!(Build { version: "1.2.3", commit: "0123abc" }.says(), "Version 1.2.3 (0123abc)");
        assert_eq!(Build::version("1.2.3").says(), "Version 1.2.3");
        assert_eq!(Build::default().version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn every_state_of_updates_has_its_line() {
        let v = Version::new(0, 2, 0);
        assert!(status(Updates::NotPublic, false).contains("aren't public"));
        assert!(status(Updates::Ready(v), false).starts_with("Version 0.2.0 is ready to install"));
        assert!(status(Updates::Ready(v), true).contains("Quit the game"));
        assert!(status(Updates::BadDownload(v), false).contains("checksum"));
    }
}
