//! Theme settings > About (docs/spec/settings.md, 16.10): which Dashboard this is, its version and the commit it
//! was built from, and what comes with it and what does not. The Bundle has no such page (Playnite's own About
//! said Playnite's version): its words are the port's own, laid out as System's, a label and its text a group, the
//! next group 40 under. Nothing on it takes focus but the page itself; B or Left goes back to the boxes.
use ui::{Button, Press, Ui};

use super::controls::{self, LEFT, ROW_WIDTH, TOP};
use super::{Pane, Step};
use crate::pages::Around;

pub const TITLE: &str = "About";
const NAME: &str = "Xbox Series Dashboard";
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

/// The page, with the build it was opened on.
pub struct About {
    build: Build,
}

impl About {
    pub fn open(around: Around) -> Self {
        Self { build: around.options.build }
    }
}

impl Pane for About {
    fn header(&self) -> String {
        TITLE.into()
    }

    /// `About: Version 0.1.0 (0123abc)`: the page has no control, so its line is what it says.
    fn focus(&self) -> String {
        format!("{TITLE}: {}", self.build.says())
    }

    fn press(&mut self, press: Press, _around: Around) -> Step {
        match press.button {
            Button::B | Button::Left if !press.repeat => Step::Back,
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, _around: Around, _focused: bool) {
        let mut y = TOP;
        y += controls::label(ui, LEFT, y, NAME) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, &self.build.says()) + 40.0;
        y += controls::label(ui, LEFT, y, LICENCES_LABEL) + 4.0;
        y += controls::description(ui, LEFT, y, ROW_WIDTH, LICENCES) + 40.0;
        y += controls::label(ui, LEFT, y, MEDIA_LABEL) + 4.0;
        controls::description(ui, LEFT, y, ROW_WIDTH, MEDIA);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_with_its_commit_or_alone() {
        assert_eq!(Build { version: "1.2.3", commit: "0123abc" }.says(), "Version 1.2.3 (0123abc)");
        assert_eq!(Build::version("1.2.3").says(), "Version 1.2.3");
        assert_eq!(Build::default().version, env!("CARGO_PKG_VERSION"));
    }
}
