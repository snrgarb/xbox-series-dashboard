//! What the pad and the keyboard both come down to.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    /// Start, the three lines.
    Menu,
    /// Back, the two squares.
    View,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    /// The left stick's click.
    LeftStick,
    /// The Xbox logo button.
    Guide,
}

impl Button {
    pub fn is_direction(self) -> bool {
        matches!(self, Button::Up | Button::Down | Button::Left | Button::Right)
    }
}

/// A button going down, or a held direction repeating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub button: Button,
    pub repeat: bool,
}

/// What a pad says happened. Every button but the Guide button is a `Press` as it goes down; the Guide button does
/// nothing as it goes down, since only what follows says whether it was a tap or a hold (docs/spec/guide.md, 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadEvent {
    Press(Press),
    /// The button was let go.
    Release(Button),
    /// The Guide button was let go before it had been held: a tap. Told 300 ms after it went down, or as it is let
    /// go if that is later, and always after its `Release`.
    GuideTap,
    /// The Guide button has been down 700 ms. Its `Release` follows, and no tap.
    GuideHeld,
}

/// How a held direction repeats: after this long, then this often.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    pub after: std::time::Duration,
    pub every: std::time::Duration,
}

impl Repeat {
    pub const fn millis(after: u64, every: u64) -> Self {
        Self { after: std::time::Duration::from_millis(after), every: std::time::Duration::from_millis(every) }
    }
}

/// As Home has it.
impl Default for Repeat {
    fn default() -> Self {
        Self::millis(400, 120)
    }
}

/// What powers a pad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Battery {
    /// A cable.
    Wired,
    /// A battery this full, 0 to 1.
    Charge(f32),
}

/// The pad the Status area shows: the first one connected.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PadStatus {
    pub connected: bool,
    /// None with no pad, or when the pad does not say.
    pub battery: Option<Battery>,
}
