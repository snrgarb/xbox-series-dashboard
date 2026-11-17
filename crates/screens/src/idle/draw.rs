//! The dim and the idle panel, as the Bundle's DimWindow and IdleCardControl draw them (docs/spec/idle-screen.md,
//! 3 and 4). Drawn last on the Overlay, over the Launch screen and the Guide, so only Toasts (a window above it) are
//! over it.
use model::Library;
use ui::{Art, ArtKey, Color, Crop, Ease, Font, Gradient, HAlign, Kind, Rect, TextStyle, Ui, VAlign};

use super::cards::{Card, Kind as CardKind};
use super::pad;
use super::{Idle, Stage};
use crate::home::background::{hero_art, BLURRED_LIGHT};

/// The dim: black, 70%, as dark as the console's.
const DIM: Color = Color::argb(0xB3000000);
/// It fades in slowly, so the screen settles rather than blinks, and out quickly as the screen wakes.
const FADE_IN: f32 = 0.8;
const FADE_OUT: f32 = 0.25;
/// The panel flies in from the right with the first card, slowing as it lands; the time fades in with it.
const FLY_IN: f32 = 0.5;
/// From one card to the next, slowly.
const CROSSFADE: f32 = 0.8;

/// A game's panel, and a tip's, slate blue (the console's, measured from its captures).
const GAME_PANEL: Color = Color::rgb(0x303030);
const TIP_PANEL: Color = Color::rgb(0x647184);

/// The time at the bottom left over the dim: 64 bold, light grey, 80 in and 57 up.
const CLOCK: TextStyle = TextStyle::new(Font::Ui, 64.0, 700).color(Color::rgb(0xCCCCCC));
const CLOCK_AT: (f32, f32) = (80.0, 57.0);

/// The card's text: 72 in from the panel's left, 78 from its right, from 75 down.
const TEXT_LEFT: f32 = 72.0;
const TEXT_RIGHT: f32 = 78.0;
const TEXT_TOP: f32 = 75.0;
/// The top line, 46 tall: the header, light.
const TOP_LINE: f32 = 46.0;
const HEADER: TextStyle = TextStyle::new(Font::Ui, 46.0, 350).color(Color::argb(0xE6FFFFFF));
/// A game's name, 64 bold, three lines at most; a tip's text, 46 bold, read whole. 37 short of the column's right.
const TITLE: TextStyle = TextStyle::new(Font::Ui, 64.0, 700);
const TIP_TITLE: TextStyle = TextStyle::new(Font::Ui, 46.0, 700);
const TITLE_SHORT: f32 = 37.0;
const LABEL: TextStyle = TextStyle::new(Font::Ui, 46.0, 700);
const DESCRIPTION: TextStyle = TextStyle::new(Font::Ui, 46.0, 400);
/// The art: the panel's bottom 540, rising out of the panel's colour from a tenth of the way down to six tenths.
const ART_HEIGHT: f32 = 540.0;
/// A cover standing in for background art, whole, 320 tall from 600 down.
const PICTURE_TOP: f32 = 600.0;
const PICTURE_HEIGHT: f32 = 320.0;
/// The Y prompt: its icon 72 round, its words 26 after it, 62 up from the panel's foot.
const PROMPT_ICON: f32 = 72.0;
const PROMPT_UP: f32 = 62.0;
const PROMPT: TextStyle = TextStyle::new(Font::Ui, 46.0, 700);

/// The dim and, while Showing things, the panel and the time over it; both fade as the screen wakes. Nothing at all
/// once the dim has faded out.
pub fn draw(ui: &mut Ui, idle: &mut Idle, library: &Library, clock: &str) {
    let awake = idle.stage() == Stage::Awake;
    let target = if awake { 0.0 } else { 1.0 };
    let dim = ui.tween_from("idle dim", 0.0, target, if awake { FADE_OUT } else { FADE_IN }, Ease::CubicOut);
    idle.showing = dim;
    if dim <= 0.0 {
        return;
    }
    let screen = ui.screen();
    // Over everything drawn so far, its text included.
    ui.layer();
    ui.fill(screen, DIM.fade(dim), 0.0);

    let Some((number, card)) = idle.card() else { return };
    let card = card.clone();
    // The panel and the time go with the dim as the screen wakes, from wherever they are; they come at once.
    let shown = ui.tween("idle cards", target, if awake { FADE_OUT } else { 0.0 }, Ease::CubicOut);
    if shown <= 0.0 {
        return;
    }
    let session = idle.session;
    // It flies in under Reduce motion as ever, as the Bundle's does.
    let out = ui.tween_from(("idle panel", session), 1.0, 0.0, FLY_IN, Ease::CubicOut);
    let time = ui.tween_from(("idle clock", session), 0.0, 1.0, FLY_IN, Ease::Linear);
    ui.with_alpha(shown, |ui| {
        let size = ui.measure(&CLOCK, clock);
        let (x, up) = CLOCK_AT;
        ui.with_alpha(time, |ui| ui.label(screen.x + x, screen.bottom() - up - size.height, clock, &CLOCK));
    });

    // Over the right half of the screen and a little more, the whole of its height.
    let width = (screen.w / 2.0).round() + 2.0;
    let panel = Rect::new(screen.right() - width + width * out, screen.y, width, screen.h);
    // The card before, fading out under this one; this one fading in over it, or on the panel as it flies in.
    if number > 1 {
        let before = ui.tween(("idle card", number - 1), 0.0, CROSSFADE, Ease::Linear);
        if before > 0.0 && !idle.first_card() {
            if let Some(previous) = idle.previous.clone() {
                ui.group(panel, 0.0, before * shown, |ui| card_at(ui, panel, &previous, library));
            }
        }
    }
    // The first card is on the panel whole as it flies in; a later one fades in.
    let from = if idle.first_card() { 1.0 } else { 0.0 };
    let alpha = ui.tween_from(("idle card", number), from, 1.0, CROSSFADE, Ease::Linear);
    ui.group(panel, 0.0, alpha * shown, |ui| card_at(ui, panel, &card, library));
}

/// One card filling `panel`.
fn card_at(ui: &mut Ui, panel: Rect, card: &Card, library: &Library) {
    let tip = card.kind == CardKind::Tip;
    let colour = if tip { TIP_PANEL } else { GAME_PANEL };
    ui.fill(panel, colour, 0.0);

    let game = card.game.as_ref().and_then(|key| library.games.iter().find(|game| game.is(key)));
    let hero = game.and_then(|game| hero_art(game, ui.px()));
    if let Some((key, blurred)) = &hero {
        let art = Rect::new(panel.x, panel.bottom() - ART_HEIGHT, panel.w, ART_HEIGHT);
        ui.with_clip(art, |ui| {
            if *blurred {
                ui.art_darkened(art.grow(120.0), key, Crop::Top, 1.0, BLURRED_LIGHT);
            } else {
                ui.art(art, key, Crop::Top, 1.0, 0.0);
            }
        });
        // A little shade under the prompt, so it reads over bright art.
        let half = art.h / 2.0;
        let shade = Rect::new(art.x, art.y + half, art.w, half);
        ui.gradient(shade, Color::CLEAR, Color::argb(0x59000000), Gradient::Vertical, 0.0);
        // The art rises out of the panel's colour: none of it a tenth of the way down, half at 35%, all at 60%.
        let at = |share: f32| art.y + art.h * share;
        let solid = Rect::new(art.x, art.y, art.w, at(0.1) - art.y);
        ui.fill(solid, colour, 0.0);
        let half_seen = colour.fade(0.5);
        let band = |from: f32, to: f32| Rect::new(art.x, at(from), art.w, at(to) - at(from));
        ui.gradient(band(0.1, 0.35), colour, half_seen, Gradient::Vertical, 0.0);
        ui.gradient(band(0.35, 0.6), half_seen, colour.fade(0.0), Gradient::Vertical, 0.0);
        // Over the blurred cover, the cover itself, whole, with a soft shadow round it.
        if let (true, Some(cover)) = (*blurred, game.and_then(|game| game.cover.as_ref().or(game.header.as_ref()))) {
            let key = ArtKey { path: cover.clone(), kind: Kind::Full((640.0 * ui.px()).ceil() as u32) };
            let room = Rect::new(panel.x, panel.y + PICTURE_TOP, panel.w, PICTURE_HEIGHT);
            if let Art::Ready { width, height, .. } = ui.art.get(&key) {
                let scale = PICTURE_HEIGHT / height.max(1) as f32;
                let w = (width as f32 * scale).min(panel.w);
                let fitted = Rect::new(room.center().0 - w / 2.0, room.y, w, PICTURE_HEIGHT);
                ui.shadow(fitted, Color::argb(0x80000000), 4.0, 24.0);
            }
            ui.art_fitted(room, &key, 1.0, 1.0);
        }
    } else if tip {
        // Centred across the panel, 4 short of its right as the Bundle's margin has it, its top 506 down.
        let at = (panel.x + (panel.w - 4.0 - pad::SIZE.0) / 2.0, panel.y + 506.0);
        pad::draw(ui, at, card.buttons, colour);
    }
    // An achievement's icon, large over the art's middle, the art darkened by half again under it, its corners
    // rounded as a tile's, a soft shadow round it.
    if let Some(icon) = &card.icon {
        if hero.is_some() {
            let art = Rect::new(panel.x, panel.bottom() - ART_HEIGHT, panel.w, ART_HEIGHT);
            ui.fill(art, Color::argb(0x80000000), 0.0);
        }
        let side = PICTURE_HEIGHT;
        let picture = Rect::new(panel.center().0 - side / 2.0, panel.y + PICTURE_TOP, side, side);
        let key = ArtKey { path: icon.clone(), kind: Kind::Thumb((side * ui.px()).ceil() as u32) };
        if matches!(ui.art.get(&key), Art::Ready { .. }) {
            ui.shadow(picture, Color::argb(0x80000000), 4.0, 24.0);
        }
        ui.art(picture, &key, Crop::Center, 1.0, 4.0);
    }

    // The text: the top line, the title, the label, the description.
    let left = panel.x + TEXT_LEFT;
    let width = panel.w - TEXT_LEFT - TEXT_RIGHT;
    let mut y = panel.y + TEXT_TOP;
    let header_left = card.score.is_none();
    if let Some(score) = card.score {
        score_row(ui, left, y, score, card.rare, colour);
    }
    if let Some(progress) = card.progress {
        let share = format!("{}%", (progress.clamp(0.0, 1.0) * 100.0).round());
        let room = Rect::new(left, y - 12.0, width, TOP_LINE + 18.0);
        ui.paragraph(room, &share, &HEADER, HAlign::Right, VAlign::Center, 1);
    }
    let header = card.header.as_ref().filter(|_| !(card.score.is_some() && card.progress.is_some()));
    if let Some(header) = header {
        let room = Rect::new(left, y - 12.0, width, TOP_LINE + 18.0);
        let align = if header_left { HAlign::Left } else { HAlign::Right };
        ui.paragraph(room, header, &HEADER, align, VAlign::Center, 1);
    }
    if header.is_some() || card.score.is_some() || card.progress.is_some() {
        y += TOP_LINE;
    }
    // The bar under the top line, 10 tall: the progress in teal on a grey track.
    if let Some(progress) = card.progress {
        let bar = Rect::new(left, y + 29.0, width, 10.0);
        ui.fill(bar, Color::rgb(0x838383), 0.0);
        ui.fill(Rect::new(bar.x, bar.y, bar.w * progress.clamp(0.0, 1.0), bar.h), Color::rgb(0x008080), 0.0);
        y += 29.0 + 10.0 + 40.0;
    } else {
        y += 29.0;
    }
    let (style, lines) = if tip { (TIP_TITLE, 10) } else { (TITLE, 3) };
    let title = Rect::new(left, y, width - TITLE_SHORT, 1000.0);
    y += ui.paragraph(title, &card.title, &style, HAlign::Left, VAlign::Top, lines).height;
    if let Some(label) = &card.label {
        y += 26.0;
        let room = Rect::new(left, y, width - TITLE_SHORT, 200.0);
        y += ui.paragraph(room, label, &LABEL, HAlign::Left, VAlign::Top, 1).height;
    }
    if let Some(description) = &card.description {
        y += 30.0;
        let room = Rect::new(left, y, width - TITLE_SHORT, 400.0);
        ui.paragraph(room, description, &DESCRIPTION, HAlign::Left, VAlign::Top, 3);
    }

    // The Y prompt at the foot: a tip's quieter.
    let quiet = if tip { 0.6 } else { 1.0 };
    ui.with_alpha(quiet, |ui| {
        let icon = Rect::new(left, panel.bottom() - PROMPT_UP - PROMPT_ICON, PROMPT_ICON, PROMPT_ICON);
        letter_ring(ui, icon, "Y", Color::WHITE);
        let (_, middle) = icon.center();
        ui.label_middle(icon.right() + 26.0, middle - 3.0, card.prompt, &PROMPT);
    });
}

/// The top line's Score from (`left`, `top`): for a Rare one the diamond first, 44 wide and 12 before the rest; the
/// G knocked out of a white disc 46 across in the panel's colour; the Score, 46 bold, 13 after it.
fn score_row(ui: &mut Ui, left: f32, top: f32, score: u32, rare: bool, panel: Color) {
    let middle = top + TOP_LINE / 2.0;
    let mut x = left;
    if rare {
        crate::achievement::rare_diamond(ui, x, middle);
        x += 44.0 + 12.0;
    }
    crate::achievement::g_disc(ui, x + 23.0, middle, 46.0, panel);
    ui.label_middle(x + 46.0 + 13.0, middle - 3.0, &score.to_string(), &SCORE);
}

/// The top line's Score: 46 bold, white.
const SCORE: TextStyle = TextStyle::new(Font::Ui, 46.0, 700);

/// A button as the console writes it in its text: its letter in a ring, the ring's line a sixteenth of its size.
fn letter_ring(ui: &mut Ui, rect: Rect, letter: &str, colour: Color) {
    let unit = rect.w / 64.0;
    ui.ring(rect.grow(-2.0 * unit), colour, 999.0, 4.0 * unit);
    let style = TextStyle::new(Font::Ui, 36.0 * unit, 700).color(colour);
    let (cx, cy) = rect.center();
    ui.label_centered(cx, cy - 3.0 * unit, letter, &style);
}
