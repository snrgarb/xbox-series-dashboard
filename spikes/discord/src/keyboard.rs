//! The spike's own pad keyboard: a grid of keys in three layers, moved over with the D-pad, the key under focus
//! typed with A. A stand-in for the Dashboard's Console keyboard (docs/spec/search.md), which needs the renderer and
//! the fonts to draw, too much for a throwaway spike; the question here is whether text reaches Discord's fields
//! from the pad at all (it goes in by DevTools' `Input.insertText`). Pure; drawn in `win::sign_in`.
pub const LAYERS: [[&str; 4]; 3] = [
    ["1234567890", "qwertyuiop", "asdfghjkl'", "zxcvbnm.-@"],
    ["1234567890", "QWERTYUIOP", "ASDFGHJKL\"", "ZXCVBNM,_@"],
    ["!?#$%&*()=", "+-/\\:;<>[]", "{}^~`|'\",.", "_@£€¥§°±×÷"],
];
pub const LAYER_NAMES: [&str; 3] = ["abc", "ABC", "#+="];

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Board {
    pub layer: usize,
    pub row: usize,
    pub col: usize,
}

impl Board {
    pub fn rows(&self) -> Vec<Vec<char>> {
        LAYERS[self.layer].iter().map(|r| r.chars().collect()).collect()
    }

    pub fn key(&self) -> char {
        self.rows()[self.row][self.col]
    }

    /// Moves focus one key, wrapping round the edges as the Console keyboard does.
    pub fn step(&mut self, dx: i32, dy: i32) {
        let rows = self.rows();
        self.row = (self.row as i32 + dy).rem_euclid(rows.len() as i32) as usize;
        let width = rows[self.row].len() as i32;
        self.col = (self.col as i32 + dx).rem_euclid(width) as usize;
    }

    pub fn next_layer(&mut self) {
        self.layer = (self.layer + 1) % LAYERS.len();
        self.step(0, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_ten_keys() {
        for layer in LAYERS {
            for row in layer {
                assert_eq!(row.chars().count(), 10, "{row}");
            }
        }
    }

    #[test]
    fn focus_wraps_and_types_its_key() {
        let mut b = Board::default();
        assert_eq!(b.key(), '1');
        b.step(-1, 0);
        assert_eq!(b.key(), '0');
        b.step(0, -1);
        assert_eq!(b.key(), '@');
        b.next_layer();
        assert_eq!(b.key(), '@');
        b.next_layer();
        assert_eq!(b.key(), '÷');
        b.next_layer();
        assert_eq!((b.layer, b.key()), (0, '@'));
    }
}
