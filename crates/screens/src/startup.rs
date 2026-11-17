//! Start-up (docs/spec/start-up.md): the console's start-up sequence, full screen before Home. The **Boot video**,
//! one console generation's boot animation chosen on Settings > General > Boot video, or Off; then
//! **Who's playing today?**, the Profiles in a row, focus on the one signed in last, A signing the focused one in;
//! then **Signing you in**, the Profile's picture alone with the Spinner turning round it for three seconds; then
//! Home, and the greeting's Toast. With Who's playing today? turned off on Settings, the Profile signed in last goes
//! straight to Signing you in. Switch profile on the Guide, and waking from sleep, ask again without the video.
//!
//! The video itself is played by the executable (the `media` crate): it tells the Dashboard which texture holds
//! the picture (`Dashboard::boot_video_picture`) and when the video has ended (`Dashboard::boot_video_ended`).
//! The Dashboard owns the pad and the Guide button while Start-up shows, as the Bundle's `StartupSequence` does
//! (C/StartupSequence.cs): a press skips the video; on Who's playing today? Left and Right move along the Profiles,
//! stopping at the ends, A signs the focused one in, and nothing else does anything, B and the Guide button
//! included: someone has to sign in.
use model::Profile;
use ui::{Button, Color, Crop, Ease, Font, HAlign, Press, Rect, TexId, TextStyle, Ui, VAlign};

use crate::home::swirl::Swirl;
use crate::{profile, spinner, theme};

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

/// Where Start-up is, in its order (the Bundle's `StartupPhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The Boot video, full screen; a press skips it.
    BootVideo,
    /// Who's playing today?: the Profiles in a row, one focused, over the swirl.
    WhosPlaying,
    /// Signing you in: the Profile chosen, alone, the Spinner round its picture.
    SigningIn,
}

/// A press counts as a skip once the video's picture has been on screen this long: one pressed for something else
/// as the window opens is not (the `video` spike's run on the test VM had a stray key end the video at once).
pub const SKIP_AFTER: f32 = 0.3;

/// How long Signing you in shows before Home and the greeting (the Bundle's `SigningInDuration`).
pub const SIGNING_IN: f32 = 3.0;

/// The swirl's fade in, as Who's playing today? or Signing you in first shows (the Bundle's `SwirlInDuration`).
pub const SWIRL_IN: f32 = 0.3;

/// The swirl's fade off Home as Start-up ends (the Bundle's `FadeDuration`). Everything else goes at once.
pub const FADE_OUT: f32 = 0.25;

/// The row's slide to centre a newly focused Profile: a touch longer than the ring's growth, easing out, so the
/// two land together (the Bundle's `SlideDuration`).
pub const SLIDE: f32 = 0.22;

/// A Profile's growth into focus, and its ring's fade, and the same back (StartupWindow.xaml's storyboards).
pub const GROW: f32 = 0.15;

/// Each Profile's width in Who's playing today?'s row.
pub const PROFILE_WIDTH: f32 = 420.0;
/// A Profile's picture, round, and how much of that it is drawn at out of focus.
const PICTURE: f32 = 380.0;
const UNFOCUSED: f32 = 0.79;
/// Where the row and Signing you in hang from.
const TOP: f32 = 340.0;

/// The title over Who's playing today? and Signing you in alike.
pub const WHOS_PLAYING: &str = "Who's playing today?";

const TITLE: TextStyle = TextStyle::new(Font::Display, 60.0, 400);
const GAMERTAG: TextStyle = TextStyle::new(Font::Ui, 30.0, 400);
const GAMERTAG_SIGNING_IN: TextStyle = TextStyle::new(Font::Ui, 30.0, 600);
const SIGNING_IN_TEXT: TextStyle = TextStyle::new(Font::Ui, 26.0, 400);

/// What a press on Who's playing today? came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Nothing: a button with no use here, a repeat, or a move at an end.
    Nothing,
    /// Focus moved along the row.
    Moved,
    /// A on this Profile: Signing you in shows it.
    Chose(Profile),
}

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
    /// Who's playing today?'s Profiles, left to right: the library's as Start-up came to ask.
    profiles: Vec<Profile>,
    /// The Profile focused, by its place in `profiles`.
    focus: usize,
    /// The Profile Signing you in shows.
    signing_in: Option<Profile>,
    /// Seconds Signing you in has shown, by the Dashboard's clock.
    signing_for: f32,
    /// The green swirl behind Who's playing today? and Signing you in: Home's own, turning as Home's does.
    swirl: Swirl,
}

impl Startup {
    /// Start-up beginning with the Boot video.
    pub fn boot_video() -> Startup {
        Startup {
            phase: Phase::BootVideo,
            picture: None,
            shown_for: 0.0,
            home_warmed: false,
            profiles: Vec::new(),
            focus: 0,
            signing_in: None,
            signing_for: 0.0,
            swirl: Swirl::default(),
        }
    }

    /// Start-up beginning after the video, or with none: asking, or with Who's playing today? turned off signing
    /// `profiles[last]` in at once.
    pub fn after_video(profiles: Vec<Profile>, last: usize, ask: bool) -> Startup {
        let mut startup = Startup::boot_video();
        startup.go_on(profiles, last, ask);
        startup
    }

    /// The video is over, or there is none: Who's playing today?, focus on `profiles[last]`, the one signed in
    /// last; or, with it not asking, Signing you in for that one (the Bundle's `AfterBootVideo`). With no Profile
    /// at all (a library from no database) it stays where it is, and the Dashboard ends it.
    pub fn go_on(&mut self, profiles: Vec<Profile>, last: usize, ask: bool) {
        self.picture = None;
        self.focus = last.min(profiles.len().saturating_sub(1));
        self.profiles = profiles;
        if ask && !self.profiles.is_empty() {
            self.phase = Phase::WhosPlaying;
        } else if let Some(profile) = self.profiles.get(self.focus).cloned() {
            self.sign_in(profile);
        }
    }

    fn sign_in(&mut self, profile: Profile) {
        self.signing_in = Some(profile);
        self.signing_for = 0.0;
        self.phase = Phase::SigningIn;
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Who's playing today?'s Profiles, left to right.
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// The Profile focused on Who's playing today?, or shown on Signing you in.
    pub fn profile(&self) -> Option<&Profile> {
        match self.phase {
            Phase::BootVideo => None,
            Phase::WhosPlaying => self.profiles.get(self.focus),
            Phase::SigningIn => self.signing_in.as_ref(),
        }
    }

    /// The video's first frame is in this texture: the picture shows from the next frame on, `aspect` wide to high.
    pub fn set_picture(&mut self, tex: TexId, aspect: f32) {
        self.picture = Some((tex, aspect));
    }

    /// Time has passed, by the Dashboard's clock. True when Signing you in has shown its time: Start-up is over.
    pub fn advance(&mut self, seconds: f32) -> bool {
        let seconds = seconds.max(0.0);
        match self.phase {
            Phase::BootVideo if self.picture.is_some() => self.shown_for += seconds,
            Phase::SigningIn => self.signing_for += seconds,
            _ => {}
        }
        self.over()
    }

    /// Whether Signing you in has shown its time.
    pub fn over(&self) -> bool {
        self.phase == Phase::SigningIn && self.signing_for >= SIGNING_IN
    }

    /// How long until Signing you in has shown its time, while it shows.
    pub fn timer(&self) -> Option<f32> {
        (self.phase == Phase::SigningIn).then(|| (SIGNING_IN - self.signing_for).max(0.0))
    }

    /// Whether a press now skips the video.
    pub fn takes_presses(&self) -> bool {
        self.picture.is_some() && self.shown_for >= SKIP_AFTER
    }

    /// A press on Who's playing today?: Left and Right move along the row, stopping at its ends, and A chooses the
    /// focused Profile, which Signing you in then shows. A held direction's repeats do nothing: the Bundle reads
    /// presses alone. Anything else, B and the Guide button too, does nothing.
    pub fn press(&mut self, press: Press) -> Answer {
        if self.phase != Phase::WhosPlaying || press.repeat {
            return Answer::Nothing;
        }
        let last = self.profiles.len().saturating_sub(1);
        let to = match press.button {
            Button::Left => self.focus.saturating_sub(1),
            Button::Right => (self.focus + 1).min(last),
            Button::A => {
                let Some(profile) = self.profiles.get(self.focus).cloned() else { return Answer::Nothing };
                self.sign_in(profile.clone());
                return Answer::Chose(profile);
            }
            _ => return Answer::Nothing,
        };
        if to == self.focus {
            return Answer::Nothing;
        }
        self.focus = to;
        Answer::Moved
    }

    /// Whether Home is to be drawn this frame, unseen, under Start-up: once, the frame after the video's picture
    /// first showed, or Who's playing today?'s first frame with no video, so that Home's art is decoded behind
    /// Start-up and Home shows whole as it ends.
    pub fn warm_home(&mut self) -> bool {
        let warm = (self.picture.is_some() || self.phase != Phase::BootVideo) && !self.home_warmed;
        self.home_warmed |= warm;
        warm
    }

    /// What `--log-focus` says while Start-up shows: `Startup: BootVideo`, `Startup: WhosPlaying "Alpha"` with the
    /// Profile focused, `Startup: SigningIn "Alpha"`.
    pub fn focus(&self) -> String {
        match self.profile() {
            Some(profile) => format!("Startup: {:?} {:?}", self.phase, profile.gamertag),
            None => format!("Startup: {:?}", self.phase),
        }
    }

    /// The screen. The Boot video: black, and the video's picture letterboxed to its own shape, as the Bundle's
    /// `Stretch="Uniform"` on black. Who's playing today? and Signing you in: the swirl fading in over black, with
    /// "Who's playing today?" over it and the row or the Profile signing in, fading in with it.
    pub fn draw(&mut self, ui: &mut Ui) {
        let screen = ui.screen();
        ui.layer();
        ui.fill(screen, Color::rgb(0x000000), 0.0);
        if self.phase == Phase::BootVideo {
            if let Some((tex, aspect)) = self.picture {
                let rect = letterbox(screen, aspect);
                // The box has the picture's shape, so nothing is cropped.
                let size = ((aspect * 1000.0).round().max(1.0) as u32, 1000);
                ui.image(rect, tex, size, Crop::Center, 1.0, 0.0);
            }
            return;
        }
        let shown = ui.tween_from("startup shown", 0.0, 1.0, SWIRL_IN, Ease::Linear);
        let Startup { phase, profiles, focus, signing_in, swirl, .. } = self;
        // The swirl fades as one picture; what is over it, which overlaps little, part by part.
        ui.group(screen, 0.0, shown, |ui| swirl.draw(ui, true));
        ui.with_alpha(shown, |ui| {
            let title = ui.measure(&TITLE, WHOS_PLAYING);
            ui.label(960.0 - title.width / 2.0, 150.0, WHOS_PLAYING, &TITLE);
            match phase {
                Phase::WhosPlaying => draw_row(ui, profiles, *focus),
                Phase::SigningIn => {
                    if let Some(profile) = signing_in {
                        draw_signing_in(ui, profile);
                    }
                }
                Phase::BootVideo => {}
            }
        });
    }

    /// The swirl alone over black, still turning, `alpha` of the way in: what lifts off Home as Start-up ends.
    pub fn draw_leaving(&mut self, ui: &mut Ui, alpha: f32) {
        let screen = ui.screen();
        let swirl = &mut self.swirl;
        ui.group(screen, 0.0, alpha, |ui| {
            ui.fill(screen, Color::rgb(0x000000), 0.0);
            swirl.draw(ui, true);
        });
    }
}

/// Who's playing today?'s row: each Profile 420 wide, the focused one at the screen's centre and the others either
/// side, running off the screen's edges if need be; the row slides to centre the one focused. Each is its picture,
/// 380 round, drawn at 300 unless focused, when it grows to its whole size in its green ring 12 out from it; its
/// gamertag under it.
fn draw_row(ui: &mut Ui, profiles: &[Profile], focus: usize) {
    let along = ui.slide("startup row", focus as f32, SLIDE, Ease::CubicOut);
    let cy = TOP + PROFILE_WIDTH / 2.0;
    // The focused one last, so its ring's glow is over its neighbours.
    let order = (0..profiles.len()).filter(|&i| i != focus).chain((focus < profiles.len()).then_some(focus));
    for i in order {
        let profile = &profiles[i];
        let focused = i == focus;
        let grown = ui.tween(("startup grown", i), if focused { 1.0 } else { 0.0 }, GROW, Ease::Linear);
        let cx = 960.0 + PROFILE_WIDTH * (i as f32 - along);
        let scale = UNFOCUSED + (1.0 - UNFOCUSED) * grown;
        let size = PICTURE * scale;
        let picture = Rect::new(cx - size / 2.0, cy - size / 2.0, size, size);
        if grown > 0.0 {
            // The ring, 7 wide and 12 out from the picture, with the theme's glow about it, fading with it.
            let glow = theme::ACCENT_LIGHT.fade(0.5 * grown);
            ui.glow(picture.grow(14.0 * scale), glow, 999.0, 11.0 * scale, 17.0 * scale);
            ui.ring(picture.grow(12.0 * scale), theme::ACCENT_LIGHT.fade(grown), 999.0, 7.0 * scale);
        }
        profile::picture(ui, picture, &profile.picture);
        let name = Rect::new(cx - PROFILE_WIDTH / 2.0 + 10.0, TOP + PROFILE_WIDTH + 6.0, PROFILE_WIDTH - 20.0, 60.0);
        ui.paragraph(name, &profile.gamertag, &GAMERTAG, HAlign::Center, VAlign::Top, 1);
    }
}

/// Signing you in: the Profile's picture alone at the centre, the Spinner turning round it where its ring was, 404
/// across and 7 wide, its ends square; its gamertag under it, semibold, and "Signing you in" under that.
fn draw_signing_in(ui: &mut Ui, profile: &Profile) {
    let cy = TOP + 200.0;
    let picture = Rect::new(960.0 - PICTURE / 2.0, cy - PICTURE / 2.0, PICTURE, PICTURE);
    profile::picture(ui, picture, &profile.picture);
    spinner::unbroken_ring(ui, Rect::new(960.0 - 202.0, cy - 202.0, 404.0, 404.0), 7.0);
    let width = PROFILE_WIDTH - 20.0;
    let name = Rect::new(960.0 - width / 2.0, TOP + 400.0 + 6.0, width, 60.0);
    let size = ui.paragraph(name, &profile.gamertag, &GAMERTAG_SIGNING_IN, HAlign::Center, VAlign::Top, 1);
    let line = Rect::new(960.0 - width / 2.0, name.y + size.height + 8.0, width, 60.0);
    ui.paragraph(line, "Signing you in", &SIGNING_IN_TEXT, HAlign::Center, VAlign::Top, 1);
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

    fn profiles(names: &[&str]) -> Vec<Profile> {
        let profile =
            |(i, name): (usize, &&str)| Profile { id: i as i64 + 1, gamertag: name.to_string(), ..Profile::default() };
        names.iter().enumerate().map(profile).collect()
    }

    fn press(startup: &mut Startup, button: Button) -> Answer {
        startup.press(Press { button, repeat: false })
    }

    fn focused(startup: &Startup) -> &str {
        startup.profile().map_or("", |profile| profile.gamertag.as_str())
    }

    #[test]
    fn whos_playing_starts_on_the_last_and_moves_along_stopping_at_the_ends() {
        let mut startup = Startup::after_video(profiles(&["Alpha", "Beta", "Gamma"]), 1, true);
        assert_eq!((startup.phase(), focused(&startup)), (Phase::WhosPlaying, "Beta"));
        assert_eq!(startup.focus(), "Startup: WhosPlaying \"Beta\"");
        assert_eq!(press(&mut startup, Button::Right), Answer::Moved);
        assert_eq!(focused(&startup), "Gamma");
        assert_eq!(press(&mut startup, Button::Right), Answer::Nothing, "the end stops");
        assert_eq!(startup.press(Press { button: Button::Left, repeat: true }), Answer::Nothing, "a repeat");
        press(&mut startup, Button::Left);
        press(&mut startup, Button::Left);
        assert_eq!(press(&mut startup, Button::Left), Answer::Nothing);
        assert_eq!(focused(&startup), "Alpha");
        // Nothing else does anything: someone has to sign in.
        for button in [Button::B, Button::Up, Button::Down, Button::Guide, Button::Menu, Button::Y] {
            assert_eq!(press(&mut startup, button), Answer::Nothing, "{button:?}");
        }
        assert_eq!((startup.phase(), focused(&startup)), (Phase::WhosPlaying, "Alpha"));
    }

    #[test]
    fn a_signs_the_focused_one_in_and_signing_in_shows_three_seconds() {
        let mut startup = Startup::after_video(profiles(&["Alpha", "Beta"]), 0, true);
        press(&mut startup, Button::Right);
        let Answer::Chose(chosen) = press(&mut startup, Button::A) else { panic!("A chooses") };
        assert_eq!(chosen.gamertag, "Beta");
        assert_eq!(startup.focus(), "Startup: SigningIn \"Beta\"");
        assert_eq!(press(&mut startup, Button::Left), Answer::Nothing, "nothing moves it now");
        assert_eq!(startup.timer(), Some(SIGNING_IN));
        assert!(!startup.advance(2.9));
        assert!(startup.advance(0.1));
    }

    #[test]
    fn not_asking_signs_the_last_in_at_once_and_with_one_profile_it_asks_all_the_same() {
        let startup = Startup::after_video(profiles(&["Alpha", "Beta"]), 1, false);
        assert_eq!((startup.phase(), focused(&startup)), (Phase::SigningIn, "Beta"));
        // One Profile: asked as ever, as the Bundle does.
        let startup = Startup::after_video(profiles(&["Alpha"]), 0, true);
        assert_eq!((startup.phase(), focused(&startup)), (Phase::WhosPlaying, "Alpha"));
        // One that has gone: the nearest there is.
        let startup = Startup::after_video(profiles(&["Alpha"]), 4, true);
        assert_eq!(focused(&startup), "Alpha");
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
