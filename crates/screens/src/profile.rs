//! A Profile's picture, wherever one is drawn (docs/spec/profiles.md, section 3): Home's top bar, the Guide's
//! Profile & system tab and its profile entry, the greeting's Toast, My games & apps, and Settings' Profiles pages.
//!
//! A picture file is drawn cropped to a circle over the first plain picture, which shows while it is decoded and in
//! place of one that will not load. The plain pictures are drawn, not read: shapes of the theme's own in flat
//! colours, never Microsoft's or anyone's art. The first is Windows' look for an account with no picture, a grey
//! figure on a pale disc, which is what the Bundle's pictures show for the user it ran as.
use model::{Library, Picture, Profile};
use ui::art::{ArtKey, Kind};
use ui::{Color, Crop, Rect, Ui};

/// What is drawn for one picture file, whatever size it is shown at up to the Picture page's, 148.
const DECODED: u32 = 192;
/// What is drawn for one shown larger: Start-up's, 380 (docs/spec/start-up.md).
const DECODED_LARGE: u32 = 400;

/// The picture, filling the circle `rect` is.
pub fn picture(ui: &mut Ui, rect: Rect, picture: &Picture) {
    match picture {
        Picture::Shape(n) => shape(ui, rect, *n),
        Picture::File(path) => {
            shape(ui, rect, 0);
            let decoded = if rect.w > DECODED as f32 { DECODED_LARGE } else { DECODED };
            let key = ArtKey { path: path.clone(), kind: Kind::Thumb(decoded) };
            ui.art(rect, &key, Crop::Center, 1.0, rect.w / 2.0);
        }
    }
}

/// The plain pictures' colours: a disc each, and the motif on it.
const DISCS: [Color; 8] = [
    Color::rgb(0xF2F2F2),
    Color::rgb(0x107C10),
    Color::rgb(0x0063B1),
    Color::rgb(0xCA5010),
    Color::rgb(0x6B69D6),
    Color::rgb(0xC30052),
    Color::rgb(0x038387),
    Color::rgb(0x4C4A48),
];

/// A plain picture: its disc, and a motif of the disc's own (a figure, a ring, a square, a dot, bars, a plus, three
/// dots, a figure again in white). Drawn for a picture 56 across and in proportion for any other.
pub fn shape(ui: &mut Ui, rect: Rect, n: u8) {
    const FIGURE: Color = Color::rgb(0x9B9B9B);
    let n = usize::from(n) % DISCS.len();
    let unit = rect.w / 56.0;
    let (cx, cy) = rect.center();
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::new(cx + x * unit, cy + y * unit, w * unit, h * unit);
    let white = Color::WHITE;
    ui.fill(rect, DISCS[n], 999.0);
    match n {
        // Windows' own look: head and shoulders.
        0 => {
            ui.fill(at(-8.0, -16.0, 16.0, 16.0), FIGURE, 999.0);
            ui.fill(at(-14.0, 3.0, 28.0, 14.0), FIGURE, 999.0);
        }
        1 => ui.ring(at(-14.0, -14.0, 28.0, 28.0), white, 999.0, 5.0 * unit),
        2 => ui.fill(at(-11.0, -11.0, 22.0, 22.0), white, 4.0 * unit),
        3 => ui.fill(at(-8.0, -8.0, 16.0, 16.0), white, 999.0),
        4 => {
            ui.fill(at(-14.0, -9.0, 28.0, 6.0), white, 3.0 * unit);
            ui.fill(at(-14.0, 3.0, 28.0, 6.0), white, 3.0 * unit);
        }
        5 => {
            ui.fill(at(-14.0, -3.0, 28.0, 6.0), white, 3.0 * unit);
            ui.fill(at(-3.0, -14.0, 6.0, 28.0), white, 3.0 * unit);
        }
        6 => {
            for x in [-15.0, -4.0, 7.0] {
                ui.fill(at(x, -4.0, 8.0, 8.0), white, 999.0);
            }
        }
        _ => {
            ui.fill(at(-7.0, -15.0, 14.0, 14.0), white, 999.0);
            ui.fill(at(-13.0, 3.0, 26.0, 13.0), white, 999.0);
        }
    }
}

/// The Profile signed in, as the library has it; where none is (a library from no database, as a test's or
/// `--library`'s), one whose gamertag is the store's user, or "Player", wearing the first plain picture.
pub fn signed_in(library: &Library) -> Profile {
    match library.profiles.signed_in() {
        Some(profile) => profile.clone(),
        None => Profile { gamertag: library.user.as_deref().unwrap_or("Player").to_string(), ..Profile::default() },
    }
}
