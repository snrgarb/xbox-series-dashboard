//! General > Boot video (docs/spec/settings.md, 6.3; docs/spec/start-up.md): a Row for Off and
//! one for each of the Bundle's boot videos the media folder has, oldest first, the one that plays ticked. A sets a
//! Row at once and saved; the tick moves, focus stays, and no preview plays.
//!
//! The Bundle lists every generation whatever is on the PC, its videos being in its own package. Here they are the
//! user's to put in the media folder (#10), so only those found are listed; with none found a line over the Rows
//! says so and only Off is there. The Bundle's line over the Rows ("Plays as the Dashboard starts...") is gone
//! (#261). A choice whose video has gone is kept, and ticks Off, which is what then plays.
use ui::{Button, Press, Rect, Ui};

use super::controls::{self, BOXES_TOP, LEFT, ROW_HEIGHT, ROW_PITCH, ROW_WIDTH};
use super::{Pane, Step};
use crate::pages::{Around, PageAction};
use crate::startup::BootVideo;
use crate::Effect;

/// The line over the Rows with no video found.
pub const NONE_FOUND: &str =
    "No boot videos were found on this PC, so none plays as the Dashboard starts. Put them in the media folder's \
     videos folder to choose one here.";

pub struct BootVideoPage {
    /// Off, then the videos found.
    rows: Vec<BootVideo>,
    focus: usize,
    /// Whether any video was found.
    found: bool,
    /// The Row ticked, as this page last set it: what the focus log says of the focused Row.
    ticked: BootVideo,
}

impl BootVideoPage {
    /// The page as the media folder is and the choice stands, focus on the Row ticked.
    pub fn open(around: Around) -> Self {
        let found = around.options.boot_videos;
        let rows: Vec<BootVideo> = std::iter::once(BootVideo::Off).chain(found.videos()).collect();
        let ticked = found.plays(around.options.boot_video);
        let focus = rows.iter().position(|video| *video == ticked).unwrap_or(0);
        Self { rows, focus, found: !found.is_empty(), ticked }
    }

    fn ticked(&self, around: Around) -> BootVideo {
        around.options.boot_videos.plays(around.options.boot_video)
    }
}

impl Pane for BootVideoPage {
    fn header(&self) -> String {
        "Boot video".into()
    }

    fn focus(&self) -> String {
        format!("Boot video: {}", self.rows[self.focus].name())
    }

    /// Whether the focused Row is the one ticked: `focus: Settings: Boot video: Off; ticked`.
    fn state(&self) -> String {
        if self.rows[self.focus] == self.ticked {
            "ticked".into()
        } else {
            String::new()
        }
    }

    fn press(&mut self, press: Press, around: Around) -> Step {
        match press.button {
            Button::B => Step::Back,
            Button::Up => {
                self.focus = self.focus.saturating_sub(1);
                Step::Stay
            }
            Button::Down => {
                self.focus = (self.focus + 1).min(self.rows.len() - 1);
                Step::Stay
            }
            Button::A if !press.repeat => {
                let video = self.rows[self.focus];
                if video == self.ticked(around) && video == around.options.boot_video {
                    return Step::Stay;
                }
                self.ticked = video;
                Step::Do(PageAction::Effect(Effect::BootVideo(video)))
            }
            // One column: Left at it does nothing (section 9.3), and Right has nowhere to go.
            _ => Step::Stay,
        }
    }

    fn draw(&mut self, ui: &mut Ui, around: Around, focused: bool) {
        let top = match self.found {
            true => BOXES_TOP,
            false => BOXES_TOP + controls::description(ui, LEFT, BOXES_TOP, ROW_WIDTH, NONE_FOUND) + 26.0,
        };
        let ticked = self.ticked(around);
        let row = |i: usize| Rect::new(LEFT, top + ROW_PITCH * i as f32, ROW_WIDTH, ROW_HEIGHT);
        for (i, video) in self.rows.iter().enumerate() {
            if !focused || i != self.focus {
                controls::row(ui, row(i), video.name(), *video == ticked, false);
            }
        }
        // The focused Row last, its ring over its neighbours.
        if focused {
            let video = self.rows[self.focus];
            controls::row(ui, row(self.focus), video.name(), video == ticked, true);
        }
    }
}
