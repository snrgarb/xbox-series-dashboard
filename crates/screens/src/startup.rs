//! Start-up (docs/spec/start-up.md): the console's start-up sequence, full screen before Home. Its first part is
//! built: the **Boot video**, one console generation's boot animation, chosen on Settings > Personalisation >
//! Boot video, or Off. Who's playing today? and Signing you in (#127) come after it, at the hook
//! `Dashboard::after_boot_video`; until they do, the video is followed by Home and the greeting's Toast.
//!
//! The video itself is played by the executable (the `media` crate): it tells the Dashboard which texture holds
//! the picture (`Dashboard::boot_video_picture`) and when the video has ended (`Dashboard::boot_video_ended`).
//! The Dashboard owns the pad and the Guide button while Start-up shows, and a press skips the video, as the Bundle
//! does (`StartupSequence.OnButton`).
use ui::{Color, Crop, Rect, TexId, Ui};

/// One console generation's boot animation, or none: the Bundle's list, oldest first after Off
/// (`XboxSeries.Core.BootVideo`), and its names (`PersonalisationPane.BootVideoNames`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BootVideo {
    Off,
    Xbox,
    Xbox360,
    Xbox360Slim,
    Xbox360SpecialEdition,
    XboxOne,
    XboxOneX,
    XboxSeriesS,
    XboxSeriesX,
}

impl BootVideo {
    /// Every choice, in the order Settings lists them.
    pub const ALL: [BootVideo; 9] = [
        BootVideo::Off,
        BootVideo::Xbox,
        BootVideo::Xbox360,
        BootVideo::Xbox360Slim,
        BootVideo::Xbox360SpecialEdition,
        BootVideo::XboxOne,
        BootVideo::XboxOneX,
        BootVideo::XboxSeriesS,
        BootVideo::XboxSeriesX,
    ];

    /// The choice until one is made: the Bundle's.
    pub const DEFAULT: BootVideo = BootVideo::XboxSeriesX;

    /// What the settings table holds for it, and its file's name without `.mp4`: the Bundle's enum names.
    pub fn key(self) -> &'static str {
        match self {
            BootVideo::Off => "Off",
            BootVideo::Xbox => "Xbox",
            BootVideo::Xbox360 => "Xbox360",
            BootVideo::Xbox360Slim => "Xbox360Slim",
            BootVideo::Xbox360SpecialEdition => "Xbox360SpecialEdition",
            BootVideo::XboxOne => "XboxOne",
            BootVideo::XboxOneX => "XboxOneX",
            BootVideo::XboxSeriesS => "XboxSeriesS",
            BootVideo::XboxSeriesX => "XboxSeriesX",
        }
    }

    /// The name Settings shows.
    pub fn name(self) -> &'static str {
        match self {
            BootVideo::Off => "Off",
            BootVideo::Xbox => "Xbox (2001)",
            BootVideo::Xbox360 => "Xbox 360 (2005)",
            BootVideo::Xbox360Slim => "Xbox 360 Slim (2010)",
            BootVideo::Xbox360SpecialEdition => "Xbox 360 Special Edition (2012)",
            BootVideo::XboxOne => "Xbox One (2013)",
            BootVideo::XboxOneX => "Xbox One X (2017)",
            BootVideo::XboxSeriesS => "Xbox Series S (2020)",
            BootVideo::XboxSeriesX => "Xbox Series X (2020)",
        }
    }

    /// Its file in the media folder's videos, as the Bundle names it in its `Videos` folder: none for Off.
    pub fn file(self) -> Option<String> {
        (self != BootVideo::Off).then(|| format!("{}.mp4", self.key()))
    }

    /// The choice the settings table names, written by this program or by hand: none for what names none.
    pub fn from_key(text: &str) -> Option<BootVideo> {
        let text = text.trim();
        BootVideo::ALL.into_iter().find(|video| video.key().eq_ignore_ascii_case(text))
    }
}

/// Which boot videos this PC has, in the media folder: what Settings lists after Off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Found(u16);

impl Found {
    pub const NONE: Found = Found(0);

    fn bit(video: BootVideo) -> u16 {
        1 << BootVideo::ALL.iter().position(|v| *v == video).unwrap_or(0)
    }

    /// The same, with this video too. Off is never found: it is always there.
    pub fn with(self, video: BootVideo) -> Found {
        if video == BootVideo::Off {
            self
        } else {
            Found(self.0 | Found::bit(video))
        }
    }

    pub fn has(self, video: BootVideo) -> bool {
        video != BootVideo::Off && self.0 & Found::bit(video) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The videos found, in the Bundle's order.
    pub fn videos(self) -> impl Iterator<Item = BootVideo> {
        BootVideo::ALL.into_iter().filter(move |video| self.has(*video))
    }

    /// What plays at start with `chosen` chosen: it, if it is here; else nothing.
    pub fn plays(self, chosen: BootVideo) -> BootVideo {
        if self.has(chosen) {
            chosen
        } else {
            BootVideo::Off
        }
    }
}

/// Where Start-up is. Who's playing today? and Signing you in (#127) are its next parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The Boot video, full screen; a press skips it.
    BootVideo,
}

/// A press counts as a skip once the video's picture has been on screen this long: one pressed for something else
/// as the window opens is not (the `video` spike's run on the test VM had a stray key end the video at once).
pub const SKIP_AFTER: f32 = 0.3;

/// Start-up while it shows.
pub struct Startup {
    phase: Phase,
    /// The texture the executable writes the video's frames into, and the picture's shape, once its first frame is
    /// in it.
    picture: Option<(TexId, f32)>,
    /// Seconds the picture has been on screen, by the Dashboard's clock.
    shown_for: f32,
    /// Home has been drawn once under the video, unseen, so its art is asked for while the video plays.
    home_warmed: bool,
}

impl Startup {
    /// Start-up beginning with the Boot video.
    pub fn boot_video() -> Startup {
        Startup { phase: Phase::BootVideo, picture: None, shown_for: 0.0, home_warmed: false }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The video's first frame is in this texture: the picture shows from the next frame on, `aspect` wide to high.
    pub fn set_picture(&mut self, tex: TexId, aspect: f32) {
        self.picture = Some((tex, aspect));
    }

    /// Time has passed, by the Dashboard's clock.
    pub fn advance(&mut self, seconds: f32) {
        if self.picture.is_some() {
            self.shown_for += seconds.max(0.0);
        }
    }

    /// Whether a press now skips the video.
    pub fn takes_presses(&self) -> bool {
        self.picture.is_some() && self.shown_for >= SKIP_AFTER
    }

    /// Whether Home is to be drawn this frame, unseen, under the picture: once, the frame after the picture first
    /// showed, so that its art is decoded behind the video and Home shows whole as it ends.
    pub fn warm_home(&mut self) -> bool {
        let warm = self.picture.is_some() && !self.home_warmed;
        self.home_warmed |= warm;
        warm
    }

    /// What `--log-focus` says while Start-up shows.
    pub fn focus(&self) -> String {
        format!("Startup: {:?}", self.phase)
    }

    /// The screen: black, and the video's picture letterboxed to its own shape, as the Bundle's `Stretch="Uniform"`
    /// on black.
    pub fn draw(&self, ui: &mut Ui) {
        let screen = ui.screen();
        ui.layer();
        ui.fill(screen, Color::rgb(0x000000), 0.0);
        if let Some((tex, aspect)) = self.picture {
            let rect = letterbox(screen, aspect);
            // The box has the picture's shape, so nothing is cropped.
            let size = ((aspect * 1000.0).round().max(1.0) as u32, 1000);
            ui.image(rect, tex, size, Crop::Center, 1.0, 0.0);
        }
    }
}

/// The largest rectangle of shape `aspect` (width over height) inside `screen`, centred in it.
pub fn letterbox(screen: Rect, aspect: f32) -> Rect {
    if aspect.is_nan() || aspect <= 0.0 || screen.h <= 0.0 {
        return screen;
    }
    let (w, h) =
        if screen.w / screen.h > aspect { (screen.h * aspect, screen.h) } else { (screen.w, screen.w / aspect) };
    Rect::new(screen.x + (screen.w - w) / 2.0, screen.y + (screen.h - h) / 2.0, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choices_are_the_bundles_in_its_order_with_its_names_and_files() {
        let names: Vec<&str> = BootVideo::ALL.iter().map(|video| video.name()).collect();
        assert_eq!(
            names,
            [
                "Off",
                "Xbox (2001)",
                "Xbox 360 (2005)",
                "Xbox 360 Slim (2010)",
                "Xbox 360 Special Edition (2012)",
                "Xbox One (2013)",
                "Xbox One X (2017)",
                "Xbox Series S (2020)",
                "Xbox Series X (2020)",
            ]
        );
        assert_eq!(BootVideo::DEFAULT, BootVideo::XboxSeriesX);
        assert_eq!(BootVideo::XboxSeriesX.file().as_deref(), Some("XboxSeriesX.mp4"));
        assert_eq!(BootVideo::Xbox360SpecialEdition.file().as_deref(), Some("Xbox360SpecialEdition.mp4"));
        assert_eq!(BootVideo::Off.file(), None);
    }

    #[test]
    fn a_choice_is_read_back_as_it_was_written_and_nonsense_is_none() {
        for video in BootVideo::ALL {
            assert_eq!(BootVideo::from_key(video.key()), Some(video));
        }
        assert_eq!(BootVideo::from_key(" xboxone "), Some(BootVideo::XboxOne));
        assert_eq!(BootVideo::from_key("Xbox One (2013)"), None);
        assert_eq!(BootVideo::from_key(""), None);
    }

    #[test]
    fn what_plays_is_the_choice_if_it_is_found_and_else_nothing() {
        let found = Found::NONE.with(BootVideo::XboxOne).with(BootVideo::Xbox).with(BootVideo::Off);
        assert_eq!(found.videos().collect::<Vec<_>>(), [BootVideo::Xbox, BootVideo::XboxOne]);
        assert!(!found.has(BootVideo::Off) && !found.is_empty() && Found::NONE.is_empty());
        assert_eq!(found.plays(BootVideo::XboxOne), BootVideo::XboxOne);
        assert_eq!(found.plays(BootVideo::XboxSeriesX), BootVideo::Off, "chosen, and not on this PC");
        assert_eq!(found.plays(BootVideo::Off), BootVideo::Off);
        assert_eq!(Found::NONE.plays(BootVideo::DEFAULT), BootVideo::Off);
    }

    #[test]
    fn presses_count_once_the_picture_has_been_up_a_moment() {
        let mut startup = Startup::boot_video();
        startup.advance(5.0);
        assert!(!startup.takes_presses(), "nothing on screen yet");
        startup.set_picture(render::WHITE, 16.0 / 9.0);
        startup.advance(0.2);
        assert!(!startup.takes_presses());
        startup.advance(0.1);
        assert!(startup.takes_presses());
    }

    #[test]
    fn home_is_warmed_once_and_only_once_the_picture_shows() {
        let mut startup = Startup::boot_video();
        assert!(!startup.warm_home());
        startup.set_picture(render::WHITE, 16.0 / 9.0);
        assert!(startup.warm_home());
        assert!(!startup.warm_home());
    }

    #[test]
    fn the_picture_is_letterboxed_to_its_shape() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(letterbox(screen, 16.0 / 9.0), screen);
        assert_eq!(letterbox(screen, 4.0 / 3.0), Rect::new(240.0, 0.0, 1440.0, 1080.0));
        assert_eq!(letterbox(screen, 2.0), Rect::new(0.0, 60.0, 1920.0, 960.0));
        // A wider screen than the canvas: bars left and right.
        let wide = Rect::new(-240.0, 0.0, 2400.0, 1080.0);
        assert_eq!(letterbox(wide, 16.0 / 9.0), Rect::new(0.0, 0.0, 1920.0, 1080.0));
    }
}
