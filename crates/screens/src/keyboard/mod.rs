//! The Console keyboard, and text entry on it (docs/spec/search.md, sections 7 to 10).
//!
//! - [`Board`] is the keyboard: its keys, layers, text and caret, its navigation and the pad's shortcuts
//!   (`board.rs`), and how it is drawn (`draw.rs`). It is a component, not a page: whoever owns one gives it the
//!   presses while its keys have focus and draws it. Search owns one beside its results; text entry owns one.
//! - [`TextEntry`] is text entry: a page over the page that asked for it, which dims, with a caption, a field and
//!   the keyboard at the foot (`text_entry.rs`). A page opens it with `PageAction::Open(Open::TextEntry(entry))`
//!   and hears what is typed, kept or dropped through `Page::entry`.
//! - `stand_in.rs` is a page that opens text entry three ways, until the pages that will (Full library's name,
//!   Settings' Groups, Discord's Secret) are built: `dashboard --page keyboard`.
pub mod board;
mod draw;
pub mod stand_in;
pub mod text_entry;

pub use board::{Board, Key, Kind, Outcome, Typed, KEYS};
pub use text_entry::{Entry, Event, TextEntry};
use ui::Repeat;

/// A held direction's pace on the keys: after 700 ms, then every 80 ms, the Bundle's own repeat (#54).
pub const REPEAT: Repeat = Repeat::millis(700, 80);
