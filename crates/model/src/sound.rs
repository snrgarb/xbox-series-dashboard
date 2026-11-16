//! The sounds the Dashboard makes, and the files they are: the Bundle's own names, so its folders can be played
//! from as they are. The files belong to Microsoft and are never in this repository (#10): they are looked for at
//! run time, and a sound with no file is silent.

/// A sound the Dashboard asks for, named by what happened. They are the Bundle's, played where it plays them
/// (`docs/spec/home.md`, section 9; `docs/spec/guide.md`, section 8). What the Bundle does in silence is silent
/// here too: B, the Guide closing, a game starting (beyond its tile's or entry's own activation) and a move that
/// has nowhere to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sound {
    /// Focus landed on another of a page's buttons or tiles.
    Move,
    /// A on a page's button or tile.
    Activate,
    /// The Guide opened.
    GuideOpen,
    /// Focus landed on another of the Guide's entries, or of a dialog's options. Its four files take turns, as on
    /// the console.
    GuideMove,
    /// A on one of the Guide's entries, or on an option of a menu or of Power.
    GuideSelect,
    /// A dialog opened: a menu, a prompt or Power.
    PromptOpen,
    /// A prompt was answered, or a dialog closed with B.
    PromptClose,
    /// The Dashboard is leaving to the desktop. Played to its end before it goes.
    PowerOff,
    /// A Toast shows.
    Notification,
    /// A Profile signs in: Signing you in shows (docs/spec/start-up.md).
    Login,
}

impl Sound {
    pub const ALL: [Sound; 10] = [
        Sound::Move,
        Sound::Activate,
        Sound::GuideOpen,
        Sound::GuideMove,
        Sound::GuideSelect,
        Sound::PromptOpen,
        Sound::PromptClose,
        Sound::PowerOff,
        Sound::Notification,
        Sound::Login,
    ];

    /// The sound's files, by the Bundle's names: one, or several that take turns in this order. The first two are
    /// the theme's (`audio\`), which Playnite plays itself; the rest the extension's (`Sounds\`).
    pub fn files(self) -> &'static [&'static str] {
        match self {
            Sound::Move => &["navigation.wav"],
            Sound::Activate => &["activation.wav"],
            Sound::GuideOpen => &["OpenMenu.wav"],
            Sound::GuideMove => &["Navigate1.wav", "Navigate2.wav", "Navigate3.wav", "Navigate4.wav"],
            Sound::GuideSelect => &["Enter.wav"],
            Sound::PromptOpen => &["PromptOpen.wav"],
            Sound::PromptClose => &["PromptClose.wav"],
            Sound::PowerOff => &["PowerOff.wav"],
            Sound::Notification => &["Notification.wav"],
            Sound::Login => &["Login.wav"],
        }
    }

    /// Every file any sound is.
    pub fn all_files() -> Vec<&'static str> {
        Sound::ALL.iter().flat_map(|sound| sound.files().iter().copied()).collect()
    }
}
