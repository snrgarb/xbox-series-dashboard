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
