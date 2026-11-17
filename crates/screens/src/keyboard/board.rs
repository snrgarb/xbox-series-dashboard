//! The Console keyboard's model: its 47 keys on a grid of 5 rows by 12 columns, what each reads on each layer, the
//! text and the caret in it, where focus is and how it moves, and which pad button stands for which key
//! (docs/spec/search.md, sections 7.1, 8 and 9; the Bundle's `ConsoleKeyboard.cs` and `ConsoleKeyboardControl.xaml.cs`).
//! Nothing here draws: that is `draw.rs`.
use ui::{Button, Press};

pub const ROWS: u8 = 5;
pub const COLUMNS: u8 = 12;

/// What a key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Types what it reads.
    Character,
    /// "#+=": swaps the letter rows for symbols, and back ("abc"). LT on the pad.
    Symbols,
    /// Deletes the character before the caret. X on the pad.
    Backspace,
    /// Moves the caret one character left. LB on the pad.
    Left,
    /// Moves the caret one character right. RB on the pad.
    Right,
    /// Capitals for the next character. The left stick's click on the pad.
    Shift,
    /// Hands the text to the keyboard's owner. Menu on the pad.
    Enter,
    /// Types a space. Y on the pad.
    Space,
}

/// One key, at its cells on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub kind: Kind,
    /// Its first row and column, and how many of each it spans.
    pub row: u8,
    pub column: u8,
    pub rows: u8,
    pub columns: u8,
}

/// The digit row, the same on every layer.
const DIGITS: &[u8; 10] = b"1234567890";
/// The three letter rows, in capitals; the last two end in punctuation, which Shift leaves alone.
const LETTERS: [&[u8; 10]; 3] = [b"QWERTYUIOP", b"ASDFGHJKL\"", b"ZXCVBNM;:!"];
/// What the letter rows read with symbols showing.
const SYMBOLS: [&[u8; 10]; 3] = [b"!@#$%^&*()", b"-_=+[]{}\\|", b"~`<>/?,.'\""];

/// How many keys there are.
pub const COUNT: usize = 47;

/// Every key, in reading order: the symbols key, the digits and backspace; the caret's left and right keys either
/// side of two letter rows; Shift and Enter either side of the third; the space bar under it.
pub const KEYS: [Key; COUNT] = layout();

const fn layout() -> [Key; COUNT] {
    const fn key(kind: Kind, row: u8, column: u8, rows: u8, columns: u8) -> Key {
        Key { kind, row, column, rows, columns }
    }
    let mut keys = [key(Kind::Character, 0, 0, 1, 1); COUNT];
    let mut next = 0;
    let mut row = 0;
    while row < 4 {
        // The side key at the row's start: rows 1 and 2 share the caret's, rows 3 and 4 Shift.
        let (left, right, tall) = match row {
            0 => (Kind::Symbols, Kind::Backspace, 1),
            1 => (Kind::Left, Kind::Right, 2),
            _ => (Kind::Shift, Kind::Enter, 2),
        };
        if row != 2 {
            keys[next] = key(left, row, 0, tall, 1);
            next += 1;
        }
        let mut column = 1;
        while column <= 10 {
            keys[next] = key(Kind::Character, row, column, 1, 1);
            next += 1;
            column += 1;
        }
        if row != 2 {
            keys[next] = key(right, row, 11, tall, 1);
            next += 1;
        }
        row += 1;
    }
    keys[next] = key(Kind::Space, 4, 1, 1, 10);
    assert!(next + 1 == COUNT);
    keys
}

impl Key {
    /// Whether the key covers this cell.
    fn covers(self, row: u8, column: u8) -> bool {
        (self.row..self.row + self.rows).contains(&row) && (self.column..self.column + self.columns).contains(&column)
    }

    /// What a character key reads with Shift and the layer as they are; None for any other key.
    pub fn reads(self, shift: bool, symbols: bool) -> Option<char> {
        if self.kind != Kind::Character {
            return None;
        }
        let at = self.column as usize - 1;
        let reads = match self.row {
            0 => DIGITS[at],
            row if symbols => SYMBOLS[row as usize - 1][at],
            row => LETTERS[row as usize - 1][at],
        } as char;
        Some(if shift || symbols { reads } else { reads.to_ascii_lowercase() })
    }

    /// The key's name in the focus log: the same whatever it reads now, since it is the same key. A letter key is
    /// its capital and a digit itself; the four punctuation keys of the letter rows and the other keys are words.
    pub fn name(self) -> String {
        match self.reads(true, false) {
            Some('"') => "Quote".into(),
            Some(';') => "Semicolon".into(),
            Some(':') => "Colon".into(),
            Some('!') => "Exclamation".into(),
            Some(reads) => reads.to_string(),
            None => format!("{:?}", self.kind),
        }
    }
}

/// The key covering a cell.
fn key_at(row: u8, column: u8) -> Option<usize> {
    KEYS.iter().position(|key| key.covers(row, column))
}

/// The first key of this kind: the only one, for every kind but a character's.
fn key_of(kind: Kind) -> usize {
    KEYS.iter().position(|key| key.kind == kind).expect("the board has a key of every kind")
}

/// Q: where focus starts.
fn first_letter() -> usize {
    key_at(1, 1).expect("the board has a first letter")
}

/// What a physical keyboard typed that is text's: the keys the pad has no button for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed<'a> {
    /// Characters, as typed: in at the caret in their own case, whatever Shift says. Control characters are
    /// left out.
    Text(&'a str),
    /// The Backspace key: deletes before the caret.
    Backspace,
}

/// What a press did on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Not the board's: B, View, RT, a repeat of anything but a direction. The owner's to answer or to swallow.
    Ignored,
    /// The board's, and the text is as it was: focus moved or stayed at an edge, Shift or the layer turned, the
    /// caret moved, backspace at the start of the text.
    Taken,
    /// The text changed.
    Edited,
    /// Enter: the text is the owner's to take. The board does nothing more.
    Enter,
    /// Up from the top row: the owner may take focus above the board. Focus on the board is where it was.
    LeftTop,
}

/// The Console keyboard: built once, used by Search and by text entry. Whoever owns it gives it the presses while
/// its keys have focus, draws it (`draw`, `draw_text`), and answers [`Outcome::Enter`], [`Outcome::LeftTop`] and B.
#[derive(Debug, Clone)]
pub struct Board {
    text: Vec<char>,
    /// Where the next character goes: an index into the text, 0 to its length.
    cursor: usize,
    /// Shift is on: the letters read and type capitals, for one character.
    shift: bool,
    /// The letter rows show symbols.
    symbols: bool,
    /// At most this many characters, if the owner said so.
    limit: Option<usize>,
    /// The key focus is on, or was last on.
    key: usize,
    /// The cell focus is "in": the one it came through to reach the key, which is what takes a tall key back to
    /// the row it was entered from and the space bar back up the column.
    cell: (u8, u8),
    /// The keys held down by a pad button (A on the key focused then, or the button a key carries), until the
    /// button is let go.
    held: Vec<(Button, usize)>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    /// As the keyboard comes up on Search: nothing typed, Shift on, the letters showing, focus on Q.
    pub fn new() -> Self {
        let key = first_letter();
        Self {
            text: Vec::new(),
            cursor: 0,
            shift: true,
            symbols: false,
            limit: None,
            key,
            cell: (KEYS[key].row, KEYS[key].column),
            held: Vec::new(),
        }
    }

    /// As it comes up over text already there, as text entry's does: the caret at its end, Shift on only with
    /// nothing typed, the letters showing, focus on Q.
    pub fn over(text: &str) -> Self {
        let text: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
        Self { cursor: text.len(), shift: text.is_empty(), text, ..Self::new() }
    }

    /// No more than this many characters go in. Text it opened over is left as it is, however long.
    pub fn limited(mut self, limit: Option<usize>) -> Self {
        self.limit = limit;
        self
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// The text either side of the caret.
    pub fn around_caret(&self) -> (String, String) {
        (self.text[..self.cursor].iter().collect(), self.text[self.cursor..].iter().collect())
    }

    /// The caret's place: how many characters are before it.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// How many characters are typed.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn shifted(&self) -> bool {
        self.shift
    }

    pub fn shows_symbols(&self) -> bool {
        self.symbols
    }

    /// The key focus is on, or was last on while the owner has focus elsewhere.
    pub fn focused(&self) -> Key {
        KEYS[self.key]
    }

    pub(super) fn focused_index(&self) -> usize {
        self.key
    }

    /// Whether a pad button is holding this key down.
    pub(super) fn is_held(&self, key: usize) -> bool {
        self.held.iter().any(|&(_, held)| held == key)
    }

    /// What a key reads now.
    pub fn reads(&self, key: Key) -> Option<char> {
        key.reads(self.shift, self.symbols)
    }

    /// The focused key's name, for the focus log: `Q`, `Space`, `Shift`.
    pub fn focus(&self) -> String {
        self.focused().name()
    }

    /// The text with a bar where the caret is, Shift, and the layer, for the focus log: `"Ha|" lower letters`.
    /// A secret's characters are stars.
    pub fn state(&self, masked: bool) -> String {
        let (before, after) = self.around_caret();
        let shown = |text: String| if masked { "*".repeat(text.chars().count()) } else { text };
        let case = if self.shift { "upper" } else { "lower" };
        let layer = if self.symbols { "symbols" } else { "letters" };
        format!("\"{}|{}\" {case} {layer}", shown(before), shown(after))
    }

    /// A press of the pad while a key has focus. A direction moves focus a cell at a time and may repeat; A presses
    /// the focused key; X, Y, LB, RB, the left stick's click, Menu and LT press the key that carries them. B and
    /// everything else is [`Outcome::Ignored`].
    pub fn press(&mut self, press: Press) -> Outcome {
        if press.button.is_direction() {
            return self.step(press.button);
        }
        if press.repeat {
            return Outcome::Ignored;
        }
        match press.button {
            Button::A => self.act(self.key),
            Button::LeftTrigger => self.act(key_of(Kind::Symbols)),
            button => self.shortcut(button),
        }
    }

    /// A press of the pad while the keyboard is up and focus is elsewhere (a result on Search): X, Y, LB, RB, the
    /// left stick's click and Menu still press their keys. LT does not, nor A, nor a direction.
    pub fn shortcut(&mut self, button: Button) -> Outcome {
        match shortcut_kind(button) {
            Some(kind) => self.act(key_of(kind)),
            None => Outcome::Ignored,
        }
    }

    /// What a physical keyboard typed. Printable characters go in at the caret as typed and turn Shift off;
    /// Backspace deletes before the caret. [`Outcome::Ignored`] when there was nothing printable in it.
    pub fn typed(&mut self, typed: Typed) -> Outcome {
        match typed {
            Typed::Text(text) => {
                let printable: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
                if printable.is_empty() {
                    return Outcome::Ignored;
                }
                let went_in = printable.into_iter().filter(|&c| self.insert(c)).count();
                self.shift = false;
                if went_in > 0 {
                    Outcome::Edited
                } else {
                    Outcome::Taken
                }
            }
            Typed::Backspace => self.backspace(),
        }
    }

    /// A pad button that has just been given to `press` is down, and will be let go: the key it pressed is drawn
    /// held until `release`. A on the key focused, LT, or a button a key carries; any other button holds nothing.
    pub fn hold(&mut self, button: Button) {
        let key = match button {
            Button::A => Some(self.key),
            Button::LeftTrigger => Some(key_of(Kind::Symbols)),
            button => shortcut_kind(button).map(key_of),
        };
        if let Some(key) = key {
            self.release(button);
            self.held.push((button, key));
        }
    }

    /// The pad button was let go.
    pub fn release(&mut self, button: Button) {
        self.held.retain(|&(held, _)| held != button);
    }

    /// Focus left the board: nothing stays held, since a button let go elsewhere never reaches it.
    pub fn let_go(&mut self) {
        self.held.clear();
    }

    /// Focus comes back onto the board from elsewhere (Down from Search's results or its box, the keyboard brought
    /// back): it is on the key it was last on, whatever that reads now, and in that key's first cell, not the one
    /// it was once entered through.
    pub fn enter(&mut self) {
        self.cell = (KEYS[self.key].row, KEYS[self.key].column);
    }

    /// One cell along the grid from where focus is in the focused key. No wrap: past the left, right or bottom
    /// edge nothing happens, and past the top the owner is told.
    fn step(&mut self, direction: Button) -> Outcome {
        let key = KEYS[self.key];
        let (mut row, mut column) = (self.cell.0 as i16, self.cell.1 as i16);
        match direction {
            Button::Left => column = key.column as i16 - 1,
            Button::Right => column = (key.column + key.columns) as i16,
            Button::Up => row = key.row as i16 - 1,
            _ => row = (key.row + key.rows) as i16,
        }
        if row < 0 {
            return Outcome::LeftTop;
        }
        if row < ROWS as i16 && (0..COLUMNS as i16).contains(&column) {
            if let Some(next) = key_at(row as u8, column as u8) {
                self.key = next;
                self.cell = (row as u8, column as u8);
            }
        }
        Outcome::Taken
    }

    /// Presses a key.
    fn act(&mut self, key: usize) -> Outcome {
        let key = KEYS[key];
        match key.kind {
            Kind::Character | Kind::Space => {
                let character = self.reads(key).unwrap_or(' ');
                let went_in = self.insert(character);
                // Shift is for one character, whichever: a symbol and a space turn it off too.
                self.shift = false;
                if went_in {
                    return Outcome::Edited;
                }
            }
            Kind::Shift => self.shift = !self.shift,
            Kind::Symbols => self.symbols = !self.symbols,
            Kind::Backspace => return self.backspace(),
            Kind::Left => self.cursor = self.cursor.saturating_sub(1),
            Kind::Right => self.cursor = (self.cursor + 1).min(self.text.len()),
            Kind::Enter => return Outcome::Enter,
        }
        Outcome::Taken
    }

    /// Puts a character in at the caret, which moves past it. False when the text is as long as it may be.
    fn insert(&mut self, character: char) -> bool {
        if self.limit.is_some_and(|limit| self.text.len() >= limit) {
            return false;
        }
        self.text.insert(self.cursor, character);
        self.cursor += 1;
        true
    }

    fn backspace(&mut self) -> Outcome {
        if self.cursor == 0 {
            return Outcome::Taken;
        }
        self.cursor -= 1;
        self.text.remove(self.cursor);
        Outcome::Edited
    }
}

/// The key a pad button stands for whether or not a key has focus: X backspace, Y space, LB and RB the caret, the
/// left stick's click Shift, Menu Enter.
fn shortcut_kind(button: Button) -> Option<Kind> {
    Some(match button {
        Button::X => Kind::Backspace,
        Button::Y => Kind::Space,
        Button::LeftBumper => Kind::Left,
        Button::RightBumper => Kind::Right,
        Button::LeftStick => Kind::Shift,
        Button::Menu => Kind::Enter,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use ui::Button::{
        Down, Left, LeftBumper, LeftStick, LeftTrigger, Menu, Right, RightBumper, RightTrigger, Up, View, A, B, X, Y,
    };

    use super::*;

    fn press(board: &mut Board, buttons: &[Button]) -> Outcome {
        let mut last = Outcome::Ignored;
        for &button in buttons {
            last = board.press(Press { button, repeat: false });
        }
        last
    }

    /// Where focus is after these presses from Q.
    fn after(buttons: &[Button]) -> String {
        let mut board = Board::new();
        press(&mut board, buttons);
        board.focus()
    }

    /// Walks focus from where it is to the key that reads `wanted` now, along the rows, and presses A on it.
    fn type_by_walking(board: &mut Board, wanted: char) {
        let target = KEYS.iter().position(|&key| board.reads(key) == Some(wanted));
        let target = KEYS[target.unwrap_or_else(|| panic!("no key reads {wanted:?} on this layer"))];
        for _ in 0..ROWS + COLUMNS {
            let at = board.focused();
            // To a character key's row first by way of the letters, so a tall side key is never in the way.
            let button = if at.kind != Kind::Character && at.kind != Kind::Space && at.column == 0 {
                Right
            } else if at.kind != Kind::Character && at.kind != Kind::Space {
                Left
            } else if at.row > target.row {
                Up
            } else if at.row < target.row {
                Down
            } else if at.column > target.column {
                Left
            } else if at.column < target.column {
                Right
            } else {
                break;
            };
            press(board, &[button]);
        }
        assert_eq!(board.reads(board.focused()), Some(wanted), "walked to {}", board.focus());
        assert_eq!(press(board, &[A]), Outcome::Edited);
    }

    #[test]
    fn the_board_is_47_keys_that_cover_every_cell_once() {
        assert_eq!(KEYS.len(), 47);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let covering = KEYS.iter().filter(|key| key.covers(row, column)).count();
                assert_eq!(covering, 1, "cell {row}, {column}");
            }
        }
        let of = |kind| KEYS.iter().filter(|key| key.kind == kind).count();
        assert_eq!(of(Kind::Character), 40);
        for kind in [Kind::Symbols, Kind::Backspace, Kind::Left, Kind::Right, Kind::Shift, Kind::Enter, Kind::Space] {
            assert_eq!(of(kind), 1, "{kind:?}");
        }
        // The tall keys and the wide one, as the Bundle lays them out (C/ConsoleKeyboard.cs:318-337).
        let spans = |kind| {
            let key = KEYS[key_of(kind)];
            (key.row, key.column, key.rows, key.columns)
        };
        assert_eq!(spans(Kind::Symbols), (0, 0, 1, 1));
        assert_eq!(spans(Kind::Backspace), (0, 11, 1, 1));
        assert_eq!(spans(Kind::Left), (1, 0, 2, 1));
        assert_eq!(spans(Kind::Right), (1, 11, 2, 1));
        assert_eq!(spans(Kind::Shift), (3, 0, 2, 1));
        assert_eq!(spans(Kind::Enter), (3, 11, 2, 1));
        assert_eq!(spans(Kind::Space), (4, 1, 1, 10));
    }

    /// What the rows read on a layer, the side keys left out.
    fn rows(shift: bool, symbols: bool) -> Vec<String> {
        (0..4)
            .map(|row| KEYS.iter().filter(|key| key.row == row).filter_map(|key| key.reads(shift, symbols)).collect())
            .collect()
    }

    #[test]
    fn the_three_layers_read_as_the_bundles_do() {
        assert_eq!(rows(true, false), ["1234567890", "QWERTYUIOP", "ASDFGHJKL\"", "ZXCVBNM;:!"]);
        assert_eq!(rows(false, false), ["1234567890", "qwertyuiop", "asdfghjkl\"", "zxcvbnm;:!"]);
        // The symbols are the same whatever Shift says, and the digits never change.
        for shift in [true, false] {
            assert_eq!(rows(shift, true), ["1234567890", "!@#$%^&*()", "-_=+[]{}\\|", "~`<>/?,.'\""]);
        }
    }

    #[test]
    fn it_opens_empty_on_q_with_shift_on_and_over_a_text_with_the_caret_at_its_end_and_shift_off() {
        let board = Board::new();
        assert_eq!((board.focus().as_str(), board.state(false).as_str()), ("Q", "\"|\" upper letters"));
        let board = Board::over("Hull");
        assert_eq!((board.focus().as_str(), board.state(false).as_str()), ("Q", "\"Hull|\" lower letters"));
        assert_eq!(Board::over("").state(false), "\"|\" upper letters");
        assert_eq!(Board::over("Hull").state(true), "\"****|\" lower letters");
    }

    #[test]
    fn focus_moves_a_cell_at_a_time_along_the_rows() {
        assert_eq!(after(&[Right]), "W");
        assert_eq!(after(&[Right, Right, Down]), "D");
        assert_eq!(after(&[Up]), "1");
        assert_eq!(after(&[Down, Down]), "Z");
        assert_eq!(after(&[Down, Down, Down]), "Space");
        assert_eq!(after(&[Left]), "Left");
        assert_eq!(after(&[Up, Left]), "Symbols");
        assert_eq!(after(&[Right; 9]), "P");
        assert_eq!(after(&[Right; 10]), "Right");
        assert_eq!(after(&[Down, Right, Right, Right, Right, Right, Right, Right, Right, Right]), "Quote");
        let row_3 = [Down, Down, Right, Right, Right, Right, Right, Right, Right];
        assert_eq!(after(&row_3), "Semicolon");
        assert_eq!(after(&[&row_3[..], &[Right]].concat()), "Colon");
        assert_eq!(after(&[&row_3[..], &[Right, Right]].concat()), "Exclamation");
        assert_eq!(after(&[&row_3[..], &[Right, Right, Right]].concat()), "Enter");
    }

    /// Every edge of the board: nothing wraps. Left of the left column, right of the right one and under the
    /// bottom row focus stays; over the top row the owner is told and focus stays.
    #[test]
    fn nothing_wraps_at_any_edge() {
        // The left edge, from each of its three keys.
        for (to, key) in [(&[Up, Left][..], "Symbols"), (&[Left], "Left"), (&[Down, Down, Left], "Shift")] {
            assert_eq!(after(to), key);
            assert_eq!(after(&[to, &[Left]].concat()), key, "Left from {key}");
            assert_eq!(after(&[to, &[Left, Left]].concat()), key);
        }
        // The right edge.
        let far = [Right; 10];
        for (to, key) in [(&[Up][..], "Backspace"), (&[], "Right"), (&[Down, Down], "Enter")] {
            let to = [to, &far[..]].concat();
            assert_eq!(after(&to), key);
            assert_eq!(after(&[&to[..], &[Right]].concat()), key, "Right from {key}");
        }
        // The bottom edge: Shift, the space bar and Enter.
        for (to, key) in [(&[Down, Down, Left][..], "Shift"), (&[Down, Down, Down], "Space")] {
            assert_eq!(after(&[to, &[Down]].concat()), key, "Down from {key}");
        }
        assert_eq!(after(&[&[Down, Down][..], &far[..], &[Down]].concat()), "Enter");
        // The top edge, along the whole row: the owner is told, and focus is where it was.
        let mut board = Board::new();
        assert_eq!(press(&mut board, &[Up, Left]), Outcome::Taken);
        for column in 0..COLUMNS {
            let key = board.focus();
            assert_eq!(press(&mut board, &[Up]), Outcome::LeftTop, "Up from {key}");
            assert_eq!(board.focus(), key);
            let moved = press(&mut board, &[Right]);
            assert_eq!(moved, Outcome::Taken);
            assert_eq!(board.focus() != key, column < COLUMNS - 1, "Right from {key}");
        }
        assert_eq!(board.focus(), "Backspace");
    }

    /// The tall keys and the wide one remember the cell they were entered through
    /// (docs/spec/search.md, 9.2's table, row by row).
    #[test]
    fn a_spanning_key_goes_back_the_way_it_was_entered() {
        // Down from a letter on row 3 is the space bar, and Up from it is that letter again.
        assert_eq!(after(&[Down, Down, Right, Right, Right, Down]), "Space");
        assert_eq!(after(&[Down, Down, Right, Right, Right, Down, Up]), "V");
        // The space bar's Left and Right are Shift and Enter, and from them back is the space bar.
        assert_eq!(after(&[Down, Down, Down, Left]), "Shift");
        assert_eq!(after(&[Down, Down, Down, Left, Right]), "Space");
        assert_eq!(after(&[Down, Down, Down, Right]), "Enter");
        assert_eq!(after(&[Down, Down, Down, Right, Left]), "Space");
        // Shift entered from Z goes back to Z, and Enter entered from "!" to "!".
        assert_eq!(after(&[Down, Down, Left, Right]), "Z");
        let to_enter = [&[Down, Down][..], &[Right; 10][..]].concat();
        assert_eq!(after(&[&to_enter[..], &[Left]].concat()), "Exclamation");
        // Up from Shift and Enter is the caret's key, in its lower row: from there the letters' second row.
        assert_eq!(after(&[Down, Down, Left, Up]), "Left");
        assert_eq!(after(&[Down, Down, Left, Up, Right]), "A");
        assert_eq!(after(&[&to_enter[..], &[Up]].concat()), "Right");
        assert_eq!(after(&[&to_enter[..], &[Up, Left]].concat()), "Quote");
        // The caret's left key goes back to the row it was entered from: Q's, A's, and Q's from the symbols key.
        assert_eq!(after(&[Left, Right]), "Q");
        assert_eq!(after(&[Down, Left, Right]), "A");
        assert_eq!(after(&[Up, Left, Down]), "Left");
        assert_eq!(after(&[Up, Left, Down, Right]), "Q");
        // Down from the caret's keys is Shift and Enter, in their upper row: from there the letters' third row.
        assert_eq!(after(&[Left, Down]), "Shift");
        assert_eq!(after(&[Left, Down, Right]), "Z");
        let to_right = [Right; 10];
        assert_eq!(after(&[&to_right[..], &[Down]].concat()), "Enter");
        assert_eq!(after(&[&to_right[..], &[Down, Left]].concat()), "Exclamation");
        // Up from the caret's keys is the symbols key and backspace.
        assert_eq!(after(&[Down, Left, Up]), "Symbols");
        assert_eq!(after(&[&to_right[..], &[Up]].concat()), "Backspace");
        // The space bar entered from Enter is in the last letters' column: Up from it is "!".
        assert_eq!(after(&[&to_enter[..], &[Down]].concat()), "Enter");
        assert_eq!(after(&[&to_right[..], &[Down, Down]].concat()), "Enter");
        let by_the_foot = [&[Down, Down, Down][..], &[Right, Left, Up][..]].concat();
        assert_eq!(after(&by_the_foot), "Exclamation");
    }

    #[test]
    fn focus_coming_back_onto_the_board_is_on_the_key_it_left_in_its_first_cell() {
        let mut board = Board::new();
        // The space bar entered from V remembers V's column; left and come back to, it is in its own first
        // column, under Z.
        press(&mut board, &[Down, Down, Right, Right, Right, Down]);
        board.hold(A);
        board.let_go();
        board.enter();
        assert_eq!(board.focus(), "Space");
        assert!(!board.is_held(board.focused_index()));
        assert_eq!(press(&mut board, &[Up]), Outcome::Taken);
        assert_eq!(board.focus(), "Z");
    }

    #[test]
    fn a_string_is_typed_by_walking_to_each_key() {
        let mut board = Board::new();
        for wanted in "Hull 3".chars() {
            // Shift is on for the first character alone; a space is the space bar.
            if wanted == ' ' {
                while board.focused().kind != Kind::Space {
                    press(&mut board, &[Down]);
                }
                assert_eq!(press(&mut board, &[A]), Outcome::Edited);
            } else {
                type_by_walking(&mut board, wanted);
            }
        }
        assert_eq!(board.text(), "Hull 3");
        assert_eq!(board.state(false), "\"Hull 3|\" lower letters");
        // A symbol from the symbols layer, which stays until it is turned back; then a capital.
        press(&mut board, &[LeftTrigger]);
        type_by_walking(&mut board, ',');
        type_by_walking(&mut board, '@');
        assert_eq!(board.state(false), "\"Hull 3,@|\" lower symbols");
        press(&mut board, &[LeftTrigger, LeftStick]);
        type_by_walking(&mut board, 'X');
        assert_eq!(board.state(false), "\"Hull 3,@X|\" lower letters");
    }

    #[test]
    fn a_string_is_typed_and_mended_with_the_pads_shortcuts() {
        let mut board = Board::new();
        // "Q", a space, "w": Shift went off with the first character, and Y is the space bar.
        assert_eq!(press(&mut board, &[A]), Outcome::Edited);
        assert_eq!(press(&mut board, &[Y]), Outcome::Edited);
        assert_eq!(press(&mut board, &[Right, A]), Outcome::Edited);
        assert_eq!(board.state(false), "\"Q w|\" lower letters");
        // LB and RB move the caret and stop at the ends; a character goes in at the caret, not at the end.
        assert_eq!(press(&mut board, &[LeftBumper, LeftBumper]), Outcome::Taken);
        assert_eq!(board.state(false), "\"Q| w\" lower letters");
        press(&mut board, &[Right, A]);
        assert_eq!(board.state(false), "\"Qe| w\" lower letters");
        press(&mut board, &[LeftBumper; 5]);
        assert_eq!(board.cursor(), 0);
        // X at the start of the text deletes nothing.
        assert_eq!(press(&mut board, &[X]), Outcome::Taken);
        assert_eq!(board.text(), "Qe w");
        press(&mut board, &[RightBumper; 9]);
        assert_eq!(board.cursor(), 4);
        // X deletes before the caret.
        assert_eq!(press(&mut board, &[LeftBumper, X]), Outcome::Edited);
        assert_eq!(board.state(false), "\"Qe|w\" lower letters");
        // The left stick's click is Shift, for one character; twice is on and off, with no caps lock.
        press(&mut board, &[LeftStick]);
        assert!(board.shifted());
        press(&mut board, &[LeftStick]);
        assert!(!board.shifted());
        press(&mut board, &[LeftStick, A, A]);
        assert_eq!(board.state(false), "\"QeEe|w\" lower letters");
        // LT is the symbols key, and the layer stays through what is typed on it. Shift goes off after a symbol too.
        press(&mut board, &[LeftTrigger, LeftStick, A, A]);
        assert_eq!(board.state(false), "\"QeEe##|w\" lower symbols");
        assert!(board.shows_symbols());
        press(&mut board, &[LeftTrigger]);
        assert!(!board.shows_symbols());
        // Backspace and the caret's keys leave Shift as it is.
        press(&mut board, &[LeftStick, X, LeftBumper, RightBumper]);
        assert!(board.shifted());
        // Menu is Enter: the owner's, and the board does nothing.
        assert_eq!(press(&mut board, &[Menu]), Outcome::Enter);
        assert_eq!(board.text(), "QeEe#w");
    }

    #[test]
    fn a_on_each_side_key_does_what_its_shortcut_does() {
        let mut board = Board::over("ab");
        // Backspace: Up from Q, then along the digits to the row's end.
        press(&mut board, &[Up]);
        press(&mut board, &[Right; 10]);
        assert_eq!(board.focus(), "Backspace");
        assert_eq!(press(&mut board, &[A]), Outcome::Edited);
        assert_eq!(board.text(), "a");
        // The caret's keys.
        press(&mut board, &[Down]);
        assert_eq!(board.focus(), "Right");
        press(&mut board, &[A]);
        assert_eq!(board.cursor(), 1);
        press(&mut board, &[Left; 11]);
        assert_eq!(board.focus(), "Left");
        press(&mut board, &[A, A]);
        assert_eq!(board.cursor(), 0);
        // Shift, the symbols key, and Enter.
        press(&mut board, &[Down, A]);
        assert_eq!((board.focus().as_str(), board.shifted()), ("Shift", true));
        press(&mut board, &[Up, Up, A]);
        assert_eq!((board.focus().as_str(), board.shows_symbols()), ("Symbols", true));
        press(&mut board, &[Down, Down]);
        press(&mut board, &[Right; 11]);
        assert_eq!(board.focus(), "Enter");
        assert_eq!(press(&mut board, &[A]), Outcome::Enter);
    }

    #[test]
    fn b_and_the_buttons_no_key_carries_are_the_owners() {
        let mut board = Board::over("a");
        for button in [B, View, RightTrigger, Button::Guide] {
            assert_eq!(press(&mut board, &[button]), Outcome::Ignored, "{button:?}");
        }
        // Only a direction repeats: a held A, X or bumper presses its key once.
        for button in [A, X, Y, LeftBumper, RightBumper, LeftStick, LeftTrigger, Menu] {
            assert_eq!(board.press(Press { button, repeat: true }), Outcome::Ignored, "{button:?} held");
        }
        assert_eq!(board.press(Press { button: Right, repeat: true }), Outcome::Taken);
        assert_eq!((board.focus().as_str(), board.state(false).as_str()), ("W", "\"a|\" lower letters"));
    }

    #[test]
    fn with_focus_elsewhere_the_shortcuts_still_press_their_keys_but_lt_a_and_the_directions_do_not() {
        let mut board = Board::new();
        assert_eq!(board.shortcut(Y), Outcome::Edited);
        assert_eq!(board.shortcut(X), Outcome::Edited);
        assert_eq!(board.shortcut(LeftStick), Outcome::Taken);
        assert_eq!(board.shortcut(LeftBumper), Outcome::Taken);
        assert_eq!(board.shortcut(RightBumper), Outcome::Taken);
        assert_eq!(board.shortcut(Menu), Outcome::Enter);
        for button in [LeftTrigger, A, Down, B] {
            assert_eq!(board.shortcut(button), Outcome::Ignored, "{button:?}");
        }
        assert_eq!((board.focus().as_str(), board.state(false).as_str()), ("Q", "\"|\" upper letters"));
    }

    #[test]
    fn a_physical_keyboards_text_goes_in_as_typed_at_the_caret() {
        let mut board = Board::over("Hd");
        board.press(Press { button: LeftBumper, repeat: false });
        board.press(Press { button: LeftStick, repeat: false });
        // In its own case whatever Shift says, and Shift goes off; control characters are left out.
        assert_eq!(board.typed(Typed::Text("aL o\r\u{8}")), Outcome::Edited);
        assert_eq!(board.state(false), "\"HaL o|d\" lower letters");
        assert_eq!(board.typed(Typed::Text("\r")), Outcome::Ignored);
        assert_eq!(board.typed(Typed::Backspace), Outcome::Edited);
        assert_eq!(board.state(false), "\"HaL |d\" lower letters");
        // Focus stays on the key it was on.
        assert_eq!(board.focus(), "Q");
        let mut empty = Board::new();
        assert_eq!(empty.typed(Typed::Backspace), Outcome::Taken);
    }

    #[test]
    fn a_limit_stops_the_text_growing_by_the_keys_and_by_typing() {
        let mut board = Board::over("abcd").limited(Some(5));
        assert_eq!(press(&mut board, &[A]), Outcome::Edited);
        assert_eq!(press(&mut board, &[A]), Outcome::Taken);
        assert_eq!(press(&mut board, &[Y]), Outcome::Taken);
        assert_eq!(board.typed(Typed::Text("xyz")), Outcome::Taken);
        assert_eq!(board.text(), "abcdq");
        // What was typed over is kept, however long.
        assert_eq!(Board::over("abcdefgh").limited(Some(5)).text(), "abcdefgh");
        let mut board = Board::over("abc").limited(Some(5));
        assert_eq!(board.typed(Typed::Text("xyz")), Outcome::Edited);
        assert_eq!(board.text(), "abcxy");
    }

    #[test]
    fn a_pad_button_holds_its_key_down_until_it_is_let_go() {
        let mut board = Board::new();
        let held = |board: &Board| {
            KEYS.iter().enumerate().filter(|&(i, _)| board.is_held(i)).map(|(_, k)| k.name()).collect::<Vec<_>>()
        };
        board.hold(A);
        board.hold(X);
        board.hold(LeftTrigger);
        assert_eq!(held(&board), ["Symbols", "Backspace", "Q"]);
        // The key A went down on stays held as focus moves on.
        press(&mut board, &[Right]);
        board.hold(B);
        assert_eq!(held(&board), ["Symbols", "Backspace", "Q"]);
        board.release(A);
        board.release(X);
        assert_eq!(held(&board), ["Symbols"]);
        board.hold(Y);
        board.let_go();
        assert!(held(&board).is_empty());
    }
}
