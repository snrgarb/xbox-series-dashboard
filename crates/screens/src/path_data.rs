//! Path data as XAML and SVG write it ("M 10,2 L 34,2 C 1,2 3,4 5,6 A 3,3 0 0 1 8,9 Z"), read into a tiny-skia
//! path, so a drawing carried over from the Bundle's XAML keeps its numbers as they are written there. Absolute
//! commands only (M, L, H, V, C, Q, A, Z), as the Bundle writes them; a command's letter may be left out when it
//! repeats. A circular arc becomes cubics of a quarter turn at most; tiny-skia flattens and strokes them, anti-aliased
//! and with the joins it is asked for.
use tiny_skia as sk;

/// The path `data` draws. Panics on data it cannot read: path data is written in the code, never read from a file.
pub fn parse(data: &str) -> sk::Path {
    let mut tokens = Tokens { rest: data, pending: None };
    let mut path = sk::PathBuilder::new();
    let mut command = None;
    let mut at = (0.0f32, 0.0f32);
    while let Some(token) = tokens.next() {
        let letter = match token {
            Token::Command(letter) => letter,
            Token::Number(_) => {
                tokens.back(token);
                // A repeated command; after a move, lines.
                match command {
                    Some('M') => 'L',
                    Some(letter) => letter,
                    None => panic!("path data starts with a number: {data}"),
                }
            }
        };
        command = Some(letter);
        let mut number = || tokens.number().unwrap_or_else(|| panic!("path data is short after {letter}: {data}"));
        match letter {
            'M' => {
                at = (number(), number());
                path.move_to(at.0, at.1);
            }
            'L' => {
                at = (number(), number());
                path.line_to(at.0, at.1);
            }
            'H' => {
                at.0 = number();
                path.line_to(at.0, at.1);
            }
            'V' => {
                at.1 = number();
                path.line_to(at.0, at.1);
            }
            'C' => {
                let (x1, y1, x2, y2, x, y) = (number(), number(), number(), number(), number(), number());
                path.cubic_to(x1, y1, x2, y2, x, y);
                at = (x, y);
            }
            'Q' => {
                let (x1, y1, x, y) = (number(), number(), number(), number());
                path.quad_to(x1, y1, x, y);
                at = (x, y);
            }
            'A' => {
                let (rx, ry, _turn, large, sweep, x, y) =
                    (number(), number(), number(), number(), number(), number(), number());
                assert!((rx - ry).abs() < 1e-3, "only circular arcs: {data}");
                arc(&mut path, at, rx, large != 0.0, sweep != 0.0, (x, y));
                at = (x, y);
            }
            'Z' | 'z' => {
                path.close();
                command = None;
            }
            other => panic!("path data's {other} is not read here: {data}"),
        }
    }
    path.finish().unwrap_or_else(|| panic!("path data draws nothing: {data}"))
}

/// A circular arc of radius `r` from `from` to `to` (SVG's endpoint arc, its centre worked out as SVG's
/// implementation notes F.6.5 do, with the two radii equal), as cubics of a quarter turn at most.
fn arc(path: &mut sk::PathBuilder, from: (f32, f32), r: f32, large: bool, sweep: bool, to: (f32, f32)) {
    let (x0, y0) = (from.0 as f64, from.1 as f64);
    let (x, y) = (to.0 as f64, to.1 as f64);
    if r <= 0.0 || (x0 - x).abs() + (y0 - y).abs() < 1e-9 {
        path.line_to(to.0, to.1);
        return;
    }
    let (hx, hy) = ((x0 - x) / 2.0, (y0 - y) / 2.0);
    let r = (r as f64).max((hx * hx + hy * hy).sqrt());
    let square = hx * hx + hy * hy;
    let mut coef = ((r * r - square) / square).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let (cx, cy) = (coef * hy + (x0 + x) / 2.0, -coef * hx + (y0 + y) / 2.0);
    let start = (y0 - cy).atan2(x0 - cx);
    let mut turn = (y - cy).atan2(x - cx) - start;
    use std::f64::consts::{FRAC_PI_2, TAU};
    if sweep && turn < 0.0 {
        turn += TAU;
    } else if !sweep && turn > 0.0 {
        turn -= TAU;
    }
    let pieces = (turn.abs() / FRAC_PI_2 - 1e-9).ceil().max(1.0) as usize;
    let step = turn / pieces as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan() * r;
    for piece in 0..pieces {
        let a = start + step * piece as f64;
        let b = a + step;
        let (p0x, p0y) = (cx + r * a.cos(), cy + r * a.sin());
        let (p3x, p3y) = if piece + 1 == pieces { (x, y) } else { (cx + r * b.cos(), cy + r * b.sin()) };
        let (p1x, p1y) = (p0x - k * a.sin(), p0y + k * a.cos());
        let (p2x, p2y) = (p3x + k * b.sin(), p3y - k * b.cos());
        path.cubic_to(p1x as f32, p1y as f32, p2x as f32, p2y as f32, p3x as f32, p3y as f32);
    }
}

#[derive(Clone, Copy)]
enum Token {
    Command(char),
    Number(f32),
}

struct Tokens<'a> {
    rest: &'a str,
    pending: Option<Token>,
}

impl Tokens<'_> {
    fn next(&mut self) -> Option<Token> {
        if let Some(token) = self.pending.take() {
            return Some(token);
        }
        self.rest = self.rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        let first = self.rest.chars().next()?;
        if first.is_ascii_alphabetic() && first != 'e' && first != 'E' {
            self.rest = &self.rest[1..];
            return Some(Token::Command(first.to_ascii_uppercase()));
        }
        // A number: a sign, digits and one point, an exponent.
        let bytes = self.rest.as_bytes();
        let mut end = 0;
        let mut point = false;
        while end < bytes.len() {
            let c = bytes[end];
            let sign = (c == b'-' || c == b'+') && (end == 0 || matches!(bytes[end - 1], b'e' | b'E'));
            if c.is_ascii_digit() || sign || (c == b'.' && !point) || ((c == b'e' || c == b'E') && end > 0) {
                point |= c == b'.';
                end += 1;
            } else {
                break;
            }
        }
        let (number, rest) = self.rest.split_at(end.max(1));
        self.rest = rest;
        Some(Token::Number(number.parse().unwrap_or_else(|_| panic!("path data's {number:?} is not a number"))))
    }

    fn number(&mut self) -> Option<f32> {
        match self.next()? {
            Token::Number(n) => Some(n),
            Token::Command(_) => None,
        }
    }

    /// Puts a token back, to be the next.
    fn back(&mut self, token: Token) {
        self.pending = Some(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(path: &sk::Path) -> Vec<(f32, f32)> {
        path.points().iter().map(|p| ((p.x * 1000.0).round() / 1000.0, (p.y * 1000.0).round() / 1000.0)).collect()
    }

    #[test]
    fn reads_moves_lines_and_cubics_as_the_xaml_writes_them() {
        let path = parse("M 181.1,13.8 C 175.1,13.8 168.7,14.5 162.8,13.6 L 279.2,225.2 Z");
        assert_eq!(points(&path), [(181.1, 13.8), (175.1, 13.8), (168.7, 14.5), (162.8, 13.6), (279.2, 225.2)]);
        let kinds: Vec<&str> = path
            .segments()
            .map(|segment| match segment {
                sk::PathSegment::MoveTo(_) => "move",
                sk::PathSegment::LineTo(_) => "line",
                sk::PathSegment::QuadTo(..) => "quad",
                sk::PathSegment::CubicTo(..) => "cubic",
                sk::PathSegment::Close => "close",
            })
            .collect();
        assert_eq!(kinds, ["move", "cubic", "line", "close"]);
    }

    #[test]
    fn a_repeated_command_may_leave_its_letter_out() {
        let path = parse("M0 0 10 0 L 10 10 0 10 Q 1 2 3 4 5 6 7 8");
        assert_eq!(
            points(&path),
            [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0)]
        );
    }

    /// A quarter turn clockwise on the screen (sweep 1) from the top of a circle about (0, 10) to its right: one cubic
    /// with the circle's handles, 0.5523 of the radius long.
    #[test]
    fn a_quarter_arc_is_one_cubic_with_the_circles_handles() {
        let path = parse("M 0,0 A 10,10 0 0 1 10,10");
        assert_eq!(points(&path), [(0.0, 0.0), (5.523, 0.0), (10.0, 4.477), (10.0, 10.0)]);
        // The other way round the same two points bends the other way, about (10, 0).
        let path = parse("M 0,0 A 10,10 0 0 0 10,10");
        assert_eq!(points(&path), [(0.0, 0.0), (0.0, 5.523), (4.477, 10.0), (10.0, 10.0)]);
    }

    /// The Bundle's D-pad, its corners arcs: it closes where it began and keeps to the plus's box.
    #[test]
    fn the_d_pads_arcs_keep_to_its_box() {
        let path = parse(
            "M 155.2,127.3 L 169.2,127.3 A 3,3 0 0 1 172.2,130.3 L 172.2,147.6 A 2,2 0 0 0 174.2,149.6 L 191.5,149.6 \
             A 3,3 0 0 1 194.5,152.6 L 194.5,166.6 A 3,3 0 0 1 191.5,169.6 Z",
        );
        let bounds = path.bounds();
        assert!((bounds.left() - 155.2).abs() < 1e-3 && (bounds.top() - 127.3).abs() < 1e-3);
        assert!((bounds.right() - 194.5).abs() < 1e-3 && (bounds.bottom() - 169.6).abs() < 1e-3);
    }
}
