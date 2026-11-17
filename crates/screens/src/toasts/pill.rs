//! The Achievement unlocked Toast's pill (docs/spec/achievement-toast.md), as the Bundle draws it
//! (`AchievementToast.xaml` and its `.xaml.cs`, `AchievementPages.cs`): the console's pill, 778 by 128, fully
//! rounded, with a brighter circle flush at its left; green with the achievement's icon (or the trophy) for an
//! ordinary unlock and for several, blue with a spinning diamond for one Rare unlock told of alone.
//!
//! It comes in as a circle alone at the pill's centre, popping in with the Xbox mark, which gives way to the icon
//! zooming out to its place or the diamond flying out of the inside of the gem; the pill unfurls from behind the
//! circle, which rides its left end out, and its text fades in. It pages: the line ("Rare Achievement unlocked -
//! 7.4%" or "Achievement unlocked") over the Score badge, the Score and the name; pushed up for the description, two
//! lines to a page, whole; then cut to "Press (Guide) to view". A band of light sweeps across it on each of the first
//! two pages. It goes as the pill collapses back into the circle, which turns dark with the mark, rings bright and
//! shrinks to a dot.
//!
//! Every value is where the Bundle's timeline puts it at a moment (`intro`, `outro`): nothing is remembered between
//! frames, and a frame at rest (a screenshot) shows it with its first page in, at `REST_AGE`.
use ui::{ArtKey, Color, Font, Gradient, HAlign, Kind, Painter, Rect, TextStyle, Ui, VAlign};

use super::{Toast, ToastKind};
use crate::guide::marks;
use crate::theme::glyph;

/// The pill, its foot 64 above the screen's and in the middle across, where the banner sits.
pub const WIDTH: f32 = 778.0;
pub const HEIGHT: f32 = 128.0;
const LEFT: f32 = (ui::DESIGN_WIDTH - WIDTH) / 2.0;
const TOP: f32 = 888.0;
/// How far the circle and the pill's ends start in from their places: to the pill's centre.
const CENTRE: f32 = (WIDTH - HEIGHT) / 2.0;
/// The circle: 126 across, a unit inside the pill's left end.
const CIRCLE: f32 = 126.0;

// The intro, in seconds from the Toast showing, when its sound starts.
const POP_AT: f32 = 0.117;
const POP_END: f32 = 0.217;
const LOGO_IN: f32 = 0.167;
const SWAP_AT: f32 = 1.052;
const SWAP_END: f32 = 1.085;
const INSIDE_FADES: f32 = 1.25;
const FLY_END: f32 = 1.485;
const UNFURL_AT: f32 = 1.485;
const UNFURL_END: f32 = 2.385;
const TEXT_IN_AT: f32 = 1.786;
const TEXT_IN_END: f32 = 2.386;
const FIRST_SHINE: f32 = 2.954;
const SECOND_SHINE: f32 = 5.222;
const SHINE_FADES_IN: f32 = 0.3;
const SHINE_SECONDS: f32 = 1.47;

// The pages (AchievementPages.cs): the console's push from the header to the description, its cut to the prompt.
const PUSH_AT: f32 = 4.122;
const PUSH_OUT: f32 = 0.18;
const PUSH_IN_AT: f32 = 4.30;
const PUSH_IN: f32 = 0.38;
const CUT_AT: f32 = 7.091;
const PROMPT_AT: f32 = 7.158;
const PROMPT_END: f32 = 7.341;
/// How long the console shows its Toast, with a page of description or none.
const CONSOLE_SHOWN_FOR: f32 = 8.5;
const PUSH_SPAN: f32 = PUSH_IN_AT + PUSH_IN - PUSH_AT;
const HEADER_HOLD: f32 = PUSH_AT - TEXT_IN_END;
const PAGE_HOLD: f32 = CUT_AT - PUSH_IN_AT - PUSH_IN;
const PROMPT_HOLD: f32 = CONSOLE_SHOWN_FOR - PROMPT_END;
/// How far a page goes up as it leaves, and how far below the next starts.
const PUSH_UP: f32 = -24.0;
const PUSH_FROM: f32 = 58.0;
/// The lines on a page of description, 45 apart.
const LINES_PER_PAGE: usize = 2;
const LINE: f32 = 45.0;

// The going, in seconds from the Toast beginning to go.
const COLLAPSE_END: f32 = 0.5;
const TEXT_OUT_AT: f32 = 0.1;
const TEXT_OUT: f32 = 0.27;
const BRIGHT_OUT_AT: f32 = 0.4;
const LOGO_BACK_END: f32 = 0.57;
const SWELL_AT: f32 = 0.633;
const SWELL_END: f32 = 0.813;
const CLOSE_END: f32 = 0.947;
const DOT_END: f32 = 1.097;
/// How long it takes to go: the next Toast waits for it.
pub const OUT_SECONDS: f32 = 1.12;

/// The shine's middle from 185 to 590 across the pill, and its band, 560 wide, from (93, -34) to (467, 162).
const SHINE_FROM: f32 = -95.0;
const SHINE_TO: f32 = 310.0;

/// One facet's turn to the next: the gem's eight-fold symmetry repeats every 0.66 s.
const SPIN_PERIOD: f32 = 0.66;
/// The spin is drawn as this many pictures of the diamond a turn.
const SPIN_FRAMES: usize = 24;

/// How long the core shows an ordinary unlock, and a page more of description.
const SHOWN_FOR: f32 = 8.5;
const RARE_SHOWN_FOR: f32 = 8.45;
const DESCRIPTION_PAGE_CHARS: usize = 80;
const DESCRIPTION_PAGE_SECONDS: f32 = 3.0;

/// The moment a frame at rest shows: its first page in, before the light sweeps across it.
pub const REST_AGE: f32 = 2.7;

/// The body text: Segoe UI 28.5, white.
const TEXT: TextStyle = TextStyle::new(Font::Ui, 28.5, 400);
/// The text column: from 26 past the circle to the pill's radius short of its right end, 4 in from top and foot.
const COLUMN: Rect = Rect::new(153.0, 4.0, WIDTH - 153.0 - 62.0, HEIGHT - 8.0);

/// How long a Toast tells of an unlock before it goes (the Bundle's `AchievementShownFor`): a Rare one's 8.45 s,
/// as its sound's closing flourish starts, whatever its description; any other's 8.5 s and 3 s more for each page
/// of a long description after the first, about 80 characters a page.
pub fn shown_for(rare: bool, description: Option<&str>) -> f32 {
    if rare {
        return RARE_SHOWN_FOR;
    }
    let chars = description.map_or(0, |text| text.trim().chars().count());
    let pages = chars.div_ceil(DESCRIPTION_PAGE_CHARS);
    SHOWN_FOR + DESCRIPTION_PAGE_SECONDS * pages.saturating_sub(1) as f32
}

/// The two colourings: the console's blues for a Rare achievement, their greens for any other.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Palette {
    pill: Color,
    bright: Color,
    dark: Color,
    halo: Color,
}

const BLUE: Palette = Palette {
    pill: Color::rgb(0x0F4E7E),
    bright: Color::rgb(0x0D7FC9),
    dark: Color::rgb(0x0F5180),
    halo: Color::rgb(0x0B3456),
};
/// The Toast's own green, the circle a lighter one.
const GREEN: Palette = Palette {
    pill: Color::rgb(0x107C10),
    bright: Color::rgb(0x0FC60F),
    dark: Color::rgb(0x107C10),
    halo: Color::rgb(0x0B550B),
};

// --- The timeline ---

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ease {
    Linear,
    CubicOut,
    CubicIn,
    QuinticOut,
    QuinticIn,
}

impl Ease {
    fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::CubicOut => 1.0 - (1.0 - t).powi(3),
            Ease::CubicIn => t.powi(3),
            Ease::QuinticOut => 1.0 - (1.0 - t).powi(5),
            Ease::QuinticIn => t.powi(5),
        }
    }
}

/// A key of a WPF key-frame animation: at once at `at`, or eased to by `at` from the key before.
#[derive(Debug, Clone, Copy)]
struct Key {
    at: f32,
    value: f32,
    ease: Option<Ease>,
}

fn jump(at: f32, value: f32) -> Key {
    Key { at, value, ease: None }
}

fn to(at: f32, value: f32, ease: Ease) -> Key {
    Key { at, value, ease: Some(ease) }
}

fn lin(at: f32, value: f32) -> Key {
    to(at, value, Ease::Linear)
}

/// A value `t` seconds into its animation through `keys`, from `start`, which it holds until its first key.
fn track(start: f32, keys: &[Key], t: f32) -> f32 {
    let (mut value, mut since) = (start, 0.0);
    for key in keys {
        if t < key.at {
            return match key.ease {
                None => value,
                Some(ease) => {
                    let span = key.at - since;
                    let p = if span > 0.0 { (t - since) / span } else { 1.0 };
                    value + (key.value - value) * ease.at(p)
                }
            };
        }
        value = key.value;
        since = key.at;
    }
    value
}

/// When the pages change, from the Toast showing (`AchievementPages.Plan`).
#[derive(Debug, Clone, PartialEq)]
struct Times {
    /// When each page but the last starts to leave, pushed up for the next: the header's first.
    pushes: Vec<f32>,
    /// When the last is cut for the prompt, and the prompt's fade in.
    cut: f32,
    prompt_at: f32,
    prompt_end: f32,
}

/// For `pages` pages of description in a Toast shown `shown_for`: with one or none the console's own times; with
/// more each push takes the console's time and every page is held as the console holds one, as far as there is
/// room, else all held for less in the same proportion, so the prompt is in with time to read it.
fn plan(pages: usize, shown_for: f32) -> Times {
    if pages == 0 {
        return Times { pushes: Vec::new(), cut: CUT_AT, prompt_at: PROMPT_AT, prompt_end: PROMPT_END };
    }
    let holds = HEADER_HOLD + pages as f32 * PAGE_HOLD + PROMPT_HOLD;
    let room = shown_for - TEXT_IN_END - pages as f32 * PUSH_SPAN - (PROMPT_END - CUT_AT);
    let scale = (room / holds).min(1.0);
    let mut at = TEXT_IN_END + scale * HEADER_HOLD;
    let mut pushes = Vec::with_capacity(pages);
    for _ in 0..pages {
        pushes.push(at);
        at += PUSH_SPAN + scale * PAGE_HOLD;
    }
    Times { pushes, cut: at, prompt_at: at + (PROMPT_AT - CUT_AT), prompt_end: at + (PROMPT_END - CUT_AT) }
}

/// Everything that moves, at a moment: opacities, scales about their middles, and shifts across.
#[derive(Debug, Clone, PartialEq)]
struct Pose {
    circle: f32,
    circle_scale: f32,
    circle_shift: f32,
    halo: f32,
    halo_scale: f32,
    ring: f32,
    ring_scale: f32,
    dark: f32,
    dark_scale: f32,
    bright: f32,
    logo: f32,
    logo_scale: f32,
    /// A Rare one's: the gem, the pale inside it the diamond flies out of, the diamond's zoom, and the round
    /// window it is seen through, grown from the inside's.
    gem: f32,
    inside: f32,
    diamond_zoom: f32,
    gem_opening: f32,
    /// Any other's icon, or the trophy, and its zoom.
    icon: f32,
    icon_zoom: f32,
    /// The pill: shown at all, the span between its ends, and how far in each end and the circle are.
    body: f32,
    span: f32,
    left_end: f32,
    right_end: f32,
    /// The text: its opacity and its drift into place; each page's opacity and how far up or down it is; the prompt.
    text: f32,
    drift: f32,
    pages: Vec<(f32, f32)>,
    prompt: f32,
    shine: f32,
    shine_shift: f32,
}

/// The intro `t` seconds after the Toast showed, for a Toast whose pages change at `times`.
fn intro(t: f32, rare: bool, times: &Times, prompts: bool) -> Pose {
    let at = |keys: &[Key]| track(0.0, keys, t);
    let circle_scale = at(&[jump(0.0, 0.4), jump(POP_AT, 0.4), to(POP_END, 1.0, Ease::CubicOut)]);
    let swap = |zoom: bool| {
        if zoom {
            at(&[jump(0.0, 3.0), jump(SWAP_AT, 3.0), to(FLY_END, 1.0, Ease::QuinticOut)])
        } else {
            at(&[jump(0.0, 0.0), jump(SWAP_AT, 0.0), lin(SWAP_END, 1.0)])
        }
    };
    let unfurl = |from: f32| at(&[jump(0.0, from), jump(UNFURL_AT, from), to(UNFURL_END, 0.0, Ease::QuinticOut)]);

    // The header's page, then each of the description's, pushed up for the next; the last cut for the prompt.
    let count = times.pushes.len() + 1;
    let pages = (0..count)
        .map(|index| {
            let (mut opacity, mut place) = if index == 0 {
                (vec![jump(0.0, 1.0)], vec![jump(0.0, 0.0)])
            } else {
                let comes = times.pushes[index - 1] + (PUSH_IN_AT - PUSH_AT);
                (
                    vec![jump(0.0, 0.0), jump(comes, 1.0)],
                    vec![jump(0.0, PUSH_FROM), jump(comes, PUSH_FROM), to(comes + PUSH_IN, 0.0, Ease::CubicOut)],
                )
            };
            if let Some(&leaves) = times.pushes.get(index) {
                opacity.push(jump(leaves + PUSH_OUT, 0.0));
                place.extend([jump(leaves, 0.0), to(leaves + PUSH_OUT, PUSH_UP, Ease::CubicIn)]);
            } else if prompts {
                opacity.push(jump(times.cut, 0.0));
            }
            (at(&opacity), at(&place))
        })
        .collect();

    // The band of light, across the first page and across the description's first.
    let mut shines = vec![FIRST_SHINE];
    if let Some(&first) = times.pushes.first() {
        shines.push(first + (SECOND_SHINE - PUSH_AT));
    }
    let mut shine = vec![jump(0.0, 0.0)];
    let mut shift = vec![jump(0.0, SHINE_FROM)];
    for &start in &shines {
        let end = start + SHINE_SECONDS;
        shine.extend([
            jump(start, 0.0),
            lin(start + SHINE_FADES_IN, 1.0),
            jump(end - SHINE_FADES_IN, 1.0),
            lin(end, 0.0),
        ]);
        shift.extend([jump(start, SHINE_FROM), lin(end, SHINE_TO)]);
    }

    Pose {
        circle: at(&[jump(0.0, 0.0), jump(POP_AT, 0.0), lin(POP_END, 1.0)]),
        circle_scale,
        circle_shift: unfurl(CENTRE),
        halo: 0.0,
        halo_scale: 1.0,
        ring: 0.0,
        ring_scale: 1.0,
        dark: at(&[jump(0.0, 1.0), jump(POP_END, 0.0)]),
        dark_scale: 1.0,
        bright: at(&[jump(0.0, 0.0), jump(POP_AT, 0.0), lin(POP_END, 1.0)]),
        logo: at(&[jump(0.0, 0.0), jump(LOGO_IN, 0.0), lin(POP_END, 1.0), jump(SWAP_AT, 1.0), lin(SWAP_END, 0.0)]),
        logo_scale: 1.0,
        gem: if rare { swap(false) } else { 0.0 },
        inside: at(&[jump(0.0, 1.0), jump(INSIDE_FADES, 1.0), lin(FLY_END, 0.0)]),
        diamond_zoom: if rare { swap(true) } else { 3.0 },
        gem_opening: at(&[jump(0.0, 1.0), jump(SWAP_AT, 1.0), to(FLY_END, 1.5, Ease::QuinticOut)]),
        icon: if rare { 0.0 } else { swap(false) },
        icon_zoom: if rare { 3.0 } else { swap(true) },
        body: at(&[jump(0.0, 0.0), jump(UNFURL_AT, 1.0)]),
        span: 1.0 - unfurl(1.0),
        left_end: unfurl(CENTRE),
        right_end: -unfurl(CENTRE),
        text: at(&[jump(0.0, 0.0), jump(TEXT_IN_AT, 0.0), lin(TEXT_IN_END, 1.0)]),
        drift: at(&[jump(0.0, 4.0), jump(TEXT_IN_AT, 4.0), to(TEXT_IN_END, 0.0, Ease::CubicOut)]),
        pages,
        prompt: if prompts {
            at(&[jump(0.0, 0.0), jump(times.prompt_at, 0.0), lin(times.prompt_end, 1.0)])
        } else {
            0.0
        },
        shine: at(&shine),
        shine_shift: at(&shift),
    }
}

/// The going `g` seconds in, from `from`, where the intro was as it began to go; `now` is the intro carried on,
/// for what the going leaves as it was (the pages, the diamond's spin).
fn outro(from: &Pose, now: Pose, g: f32) -> Pose {
    let at = |start: f32, keys: &[Key]| track(start, keys, g);
    let collapse = |start: f32, end: f32| at(start, &[to(COLLAPSE_END, end, Ease::QuinticIn)]);
    Pose {
        span: collapse(from.span, 0.0),
        left_end: collapse(from.left_end, CENTRE),
        right_end: collapse(from.right_end, -CENTRE),
        circle_shift: collapse(from.circle_shift, CENTRE),
        body: at(from.body, &[jump(COLLAPSE_END, 0.0)]),
        text: at(from.text, &[jump(TEXT_OUT_AT, from.text), lin(TEXT_OUT_AT + TEXT_OUT, 0.0)]),
        shine: at(from.shine, &[lin(TEXT_OUT_AT, 0.0)]),
        dark: 1.0,
        bright: at(from.bright, &[jump(BRIGHT_OUT_AT, from.bright), lin(COLLAPSE_END, 0.0)]),
        gem: at(from.gem, &[jump(BRIGHT_OUT_AT, from.gem), lin(COLLAPSE_END, 0.0)]),
        icon: at(from.icon, &[jump(BRIGHT_OUT_AT, from.icon), lin(COLLAPSE_END, 0.0)]),
        logo: at(0.0, &[jump(COLLAPSE_END, 0.0), lin(LOGO_BACK_END, 1.0)]),
        ring: 1.0,
        circle_scale: at(
            from.circle_scale,
            &[jump(SWELL_AT, 1.0), to(SWELL_END, 1.11, Ease::CubicOut), lin(CLOSE_END, 1.0)],
        ),
        dark_scale: at(
            1.0,
            &[jump(SWELL_AT, 1.0), to(SWELL_END, 0.82, Ease::CubicOut), to(CLOSE_END, 0.0, Ease::CubicIn)],
        ),
        logo_scale: at(1.0, &[jump(SWELL_END, 1.0), to(CLOSE_END, 0.0, Ease::CubicIn)]),
        ring_scale: at(1.0, &[jump(SWELL_END, 1.0), lin(CLOSE_END, 0.8), to(DOT_END, 0.08, Ease::CubicIn)]),
        halo: at(0.0, &[jump(SWELL_END, 0.0), lin(CLOSE_END, 0.85), lin(DOT_END, 0.6)]),
        halo_scale: at(1.0, &[jump(CLOSE_END, 1.0), lin(DOT_END, 0.85)]),
        circle: at(from.circle, &[jump(DOT_END, from.circle), lin(OUT_SECONDS, 0.0)]),
        ..now
    }
}

/// When things move in the intro, each from when to when: what asks for frames, and what the Toast is woken for
/// between. Instants (the cut to the prompt) are from and to the same moment.
fn moves(rare: bool, times: &Times) -> Vec<(f32, f32)> {
    let mut moves = vec![(POP_AT, POP_END), (SWAP_AT, FLY_END), (UNFURL_AT, UNFURL_END), (TEXT_IN_AT, TEXT_IN_END)];
    // The diamond spins on for as long as it shows.
    if rare {
        moves.push((SWAP_AT, f32::INFINITY));
    }
    moves.extend(times.pushes.iter().map(|&push| (push, push + PUSH_SPAN)));
    moves.push((FIRST_SHINE, FIRST_SHINE + SHINE_SECONDS));
    if let Some(&first) = times.pushes.first() {
        let second = first + (SECOND_SHINE - PUSH_AT);
        moves.push((second, second + SHINE_SECONDS));
    }
    moves.push((times.cut, times.cut));
    moves.push((times.prompt_at, times.prompt_end));
    moves
}

// --- Drawing ---

/// The description in lines as the column wraps it, a hair narrower so a line measured to fit never wraps again,
/// in pages of two: all of it, never cut short. A word too long for a line is a line of its own.
fn description_pages(ui: &mut Ui, text: Option<&str>) -> Vec<Vec<String>> {
    let Some(text) = text else { return Vec::new() };
    let width = COLUMN.w - 1.0;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let tried = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if line.is_empty() || ui.measure(&TEXT, &tried).width <= width {
            line = tried;
        } else {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.chunks(LINES_PER_PAGE).map(<[String]>::to_vec).collect()
}

/// Which of the diamond's pictures shows `t` seconds after the Toast showed: the first until it spins.
fn spin_frame(t: f32) -> usize {
    if t < SWAP_AT {
        return 0;
    }
    let turn = ((t - SWAP_AT) / SPIN_PERIOD).fract();
    ((turn * SPIN_FRAMES as f32) as usize).min(SPIN_FRAMES - 1)
}

/// Draws the pill `age` seconds after it began to show and, if it is going, `going` seconds after it began to.
pub fn draw(ui: &mut Ui, toast: &Toast, age: f32, going: Option<f32>) {
    let rare = toast.kind == ToastKind::RareAchievement;
    let told = toast.told.as_deref().cloned().unwrap_or_default();
    let descriptions = description_pages(ui, told.description.as_deref());
    let times = plan(descriptions.len(), toast.shown_seconds() as f32);
    let settling = ui.store.settling();
    if settling && going.is_some() {
        return;
    }
    let age = if settling { REST_AGE } else { age };
    let now = intro(age, rare, &times, toast.prompt);
    let pose = match going {
        Some(g) => outro(&intro(age - g, rare, &times, toast.prompt), now, g),
        None => now,
    };
    if !settling {
        let moves = moves(rare, &times);
        let moving = going.is_some() || moves.iter().any(|&(from, to)| from < to && (from..to).contains(&age));
        if moving {
            ui.store.keep_animating();
        } else if let Some(wait) = moves.iter().map(|&(from, _)| from - age).filter(|&wait| wait > 0.0).reduce(f32::min)
        {
            ui.store.wake_in(wait);
        }
    }
    let palette = if rare { BLUE } else { GREEN };
    if rare {
        ask_for_diamond(ui);
    }

    ui.layer();
    ui.with_offset(LEFT, TOP, |ui| {
        body(ui, &pose, palette);
        shine(ui, &pose);
        text(ui, toast, &told, &descriptions, &pose, palette);
        // The circle over the text, as it rides out over the column's start.
        ui.layer();
        circle(ui, &pose, palette, rare, told.icon.as_ref(), spin_frame(age));
    });
}

/// The pill's ends and the span between them, unfurling from its centre.
fn body(ui: &mut Ui, pose: &Pose, palette: Palette) {
    if pose.body <= 0.0 {
        return;
    }
    ui.with_alpha(pose.body, |ui| {
        let span = (WIDTH - HEIGHT) * pose.span;
        ui.fill(Rect::new(WIDTH / 2.0 - span / 2.0, 0.0, span, HEIGHT), palette.pill, 0.0);
        let radius = HEIGHT / 2.0;
        ui.fill(Rect::new(pose.left_end, 0.0, HEIGHT, HEIGHT), palette.pill, radius);
        ui.fill(Rect::new(WIDTH - HEIGHT + pose.right_end, 0.0, HEIGHT, HEIGHT), palette.pill, radius);
    });
}

/// The band of light, white at 20% at its brightest, leaning "/", swept across the span between the pill's ends.
fn shine(ui: &mut Ui, pose: &Pose) {
    if pose.shine <= 0.0 {
        return;
    }
    // Its gradient runs from (93, -34) to (467, 162) of its rectangle, which starts `shine_shift` across: lines of
    // one colour lean across that. Its stops, out from its middle, drawn as a turned quad between each two.
    let (dx, dy) = (374.0f32, 196.0f32);
    let length = dx.hypot(dy);
    let turn = dy.atan2(dx);
    let middle = (93.0 + dx / 2.0 + pose.shine_shift, -34.0 + dy / 2.0);
    const STOPS: [(f32, u32); 7] =
        [(0.0, 0x00), (0.2, 0x06), (0.35, 0x1A), (0.5, 0x33), (0.65, 0x1A), (0.8, 0x06), (1.0, 0x00)];
    let white = |alpha: u32| Color::argb((alpha << 24) | 0xFFFFFF);
    ui.with_clip(Rect::new(HEIGHT / 2.0, 0.0, WIDTH - HEIGHT, HEIGHT), |ui| {
        ui.with_alpha(pose.shine, |ui| {
            for pair in STOPS.windows(2) {
                let ((from, a), (to, b)) = (pair[0], pair[1]);
                let along = ((from + to) / 2.0 - 0.5) * length;
                let (cx, cy) = (middle.0 + along * turn.cos(), middle.1 + along * turn.sin());
                // A hair longer each way, so the pieces meet with no seam.
                let w = (to - from) * length + 1.0;
                let rect = Rect::new(cx - w / 2.0, cy - 400.0, w, 800.0);
                ui.gradient_turned(rect, white(a), white(b), Gradient::Horizontal, turn);
            }
        });
    });
}

/// The pages: the line and the Score line; the description, two lines at a time; the prompt.
fn text(
    ui: &mut Ui,
    toast: &Toast,
    told: &super::achievement::Told,
    descriptions: &[Vec<String>],
    pose: &Pose,
    palette: Palette,
) {
    if pose.text <= 0.0 {
        return;
    }
    let column = Rect::new(COLUMN.x + pose.drift, COLUMN.y, COLUMN.w, COLUMN.h);
    let x = column.x;
    ui.with_alpha(pose.text, |ui| {
        ui.with_clip(column, |ui| {
            // The first page: the line, and under it the Score badge, the Score and the name; the line alone,
            // centred, without a name.
            if let Some(&(opacity, shift)) = pose.pages.first().filter(|page| page.0 > 0.0) {
                ui.with_alpha(opacity, |ui| {
                    let lines = if told.name.is_some() { 2.0 } else { 1.0 };
                    let top = page_top(lines) + shift;
                    let line = match told.rarity {
                        Some(rarity) => format!("{} - {}%", toast.text, super::achievement::rarity_text(rarity)),
                        None => toast.text.clone(),
                    };
                    ui.paragraph(Rect::new(x, top, column.w, LINE), &line, &TEXT, HAlign::Left, VAlign::Center, 1);
                    if let Some(name) = &told.name {
                        score_line(ui, x, top + LINE + LINE / 2.0, column.w, told.score, name, palette);
                    }
                });
            }
            for (page, lines) in descriptions.iter().enumerate() {
                let Some(&(opacity, shift)) = pose.pages.get(page + 1).filter(|page| page.0 > 0.0) else { continue };
                ui.with_alpha(opacity, |ui| {
                    let top = page_top(lines.len() as f32) + shift;
                    for (i, line) in lines.iter().enumerate() {
                        ui.label_middle(x, top + LINE * i as f32 + LINE / 2.0, line, &TEXT);
                    }
                });
            }
            if pose.prompt > 0.0 {
                ui.with_alpha(pose.prompt, |ui| prompt(ui, column, toast.kind.prompt()));
            }
        });
    });
}

/// Where a page of so many 45-high lines starts, in the middle of the column (over a margin of 2 at its foot).
fn page_top(lines: f32) -> f32 {
    COLUMN.y + (COLUMN.h - 2.0 - LINE * lines) / 2.0
}

/// The Score badge (a white disc with the "G" cut in, in the pill's colour), "<Score> -" and the name, cut short.
fn score_line(ui: &mut Ui, x: f32, cy: f32, width: f32, score: Option<u32>, name: &str, palette: Palette) {
    let mut at = x;
    if let Some(score) = score {
        const BADGE: f32 = 33.5;
        let (bx, by) = (at + BADGE / 2.0, cy + 0.5);
        ui.fill(Rect::new(bx - BADGE / 2.0, by - BADGE / 2.0, BADGE, BADGE), Color::rgb(0xFCFFFF), BADGE / 2.0);
        ui.label_centered(bx, by - 0.5, "G", &TextStyle::new(Font::Ui, 22.0, 600).color(palette.pill));
        at += BADGE + 11.0;
        at += ui.label_middle(at, cy, &format!("{score} -"), &TEXT).width + 9.0;
    }
    let room = Rect::new(at, cy - LINE / 2.0, (x + width - at).max(0.0), LINE);
    ui.paragraph(room, name, &TEXT, HAlign::Left, VAlign::Center, 1);
}

/// "Press (Guide) to view", centred in the column: the Guide button's mark 34 across, 16 before it and 22 after.
fn prompt(ui: &mut Ui, column: Rect, then: &str) {
    const MARK: f32 = 34.0;
    let press = ui.measure(&TEXT, "Press").width;
    let after = ui.measure(&TEXT, then).width;
    let whole = press + 16.0 + MARK + 22.0 + after;
    let x = column.x + (column.w - whole) / 2.0;
    let cy = column.y + column.h / 2.0;
    ui.label_middle(x, cy, "Press", &TEXT);
    marks::guide_button(ui, Rect::new(x + press + 16.0, cy + 1.0 - MARK / 2.0, MARK, MARK), Color::WHITE);
    ui.label_middle(x + press + 16.0 + MARK + 22.0, cy, then, &TEXT);
}

/// The circle, riding the pill's left end: from the bottom the dark halo, the bright ring and the dark disc the
/// going shows; the bright circle; the Xbox mark; and a Rare one's gem or any other's icon.
fn circle(ui: &mut Ui, pose: &Pose, palette: Palette, rare: bool, icon: Option<&std::path::PathBuf>, frame: usize) {
    if pose.circle <= 0.0 {
        return;
    }
    let (cx, cy) = (1.0 + CIRCLE / 2.0 + pose.circle_shift, HEIGHT / 2.0);
    let scale = pose.circle_scale;
    let disc =
        |across: f32| Rect::new(cx - across * scale / 2.0, cy - across * scale / 2.0, across * scale, across * scale);
    let round = |ui: &mut Ui, across: f32, colour: Color, alpha: f32| {
        if alpha > 0.0 && across > 0.0 {
            ui.fill(disc(across), colour.fade(alpha), across * scale / 2.0);
        }
    };
    ui.with_alpha(pose.circle, |ui| {
        round(ui, CIRCLE * pose.halo_scale, palette.halo, pose.halo);
        round(ui, CIRCLE * pose.ring_scale, palette.bright, pose.ring);
        round(ui, CIRCLE * pose.dark_scale, palette.dark, pose.dark);
        round(ui, CIRCLE, palette.bright, pose.bright);
        if pose.logo > 0.0 && pose.logo_scale > 0.0 {
            ui.with_alpha(pose.logo, |ui| marks::guide_button(ui, disc(70.0 * pose.logo_scale), Color::WHITE));
        }
        if rare && pose.gem > 0.0 {
            ui.with_alpha(pose.gem, |ui| gem(ui, (cx, cy), scale, pose, frame));
        }
        if !rare && pose.icon > 0.0 {
            ui.with_alpha(pose.icon, |ui| {
                // The icon, and the trophy only while there is none decoded: letters are drawn over pictures
                // whatever the order here, so the trophy drawn first still showed on the icon. Zoomed, the trophy
                // is cut to a square inside the circle, so nothing of it is outside.
                let drawn = icon.is_some_and(|path| {
                    let key = ArtKey { path: path.clone(), kind: Kind::Thumb(256) };
                    ui.art_zoomed(disc(CIRCLE), &key, pose.icon_zoom, 1.0, CIRCLE * scale / 2.0)
                });
                if !drawn {
                    let size = 60.0 * pose.icon_zoom * scale;
                    let inner = disc(CIRCLE / std::f32::consts::SQRT_2);
                    let clip = if pose.icon_zoom > 1.0 { inner } else { disc(CIRCLE) };
                    ui.with_clip(clip, |ui| ui.icon(cx, cy, glyph::TROPHY, size, Color::WHITE));
                }
            });
        }
    });
}

/// A Rare one's gem: the pale inside, close up, fading as the diamond flies out of it, the two seen through a round
/// window the inside's size that opens half as wide again.
fn gem(ui: &mut Ui, (cx, cy): (f32, f32), scale: f32, pose: &Pose, frame: usize) {
    if pose.inside > 0.0 {
        let half = 43.5 * scale;
        let inside = Rect::new(cx - half, cy - half, half * 2.0, half * 2.0);
        ui.gradient(
            inside,
            Color::rgb(0xF4F8FB).fade(pose.inside),
            Color::rgb(0xA9BBCB).fade(pose.inside),
            Gradient::Radial,
            half,
        );
    }
    // The diamond, 80 by 79 at rest, `diamond_zoom` times that about its middle, seen through the window: the part
    // of its picture the window covers, drawn round.
    let window = 43.5 * pose.gem_opening * scale;
    let (w, h) = (DIAMOND.0 * pose.diamond_zoom * scale, DIAMOND.1 * pose.diamond_zoom * scale);
    let (pw, ph) = (DIAMOND.0 * PAINT + 2.0 * MARGIN, DIAMOND.1 * PAINT + 2.0 * MARGIN);
    let u = |x: f32| (MARGIN + (x - (cx - w / 2.0)) / w * DIAMOND.0 * PAINT) / pw;
    let v = |y: f32| (MARGIN + (y - (cy - h / 2.0)) / h * DIAMOND.1 * PAINT) / ph;
    let rect = Rect::new(cx - window, cy - window, window * 2.0, window * 2.0);
    let uv = [u(rect.x), v(rect.y), u(rect.right()), v(rect.bottom())];
    ui.art_part(rect, &diamond_key(frame), uv, 1.0, window);
}

fn diamond_key(frame: usize) -> ArtKey {
    ArtKey { path: format!("achievement/diamond/{frame}").into(), kind: Kind::Painted(DIAMOND_FRAMES[frame]) }
}

/// Asks for every one of the diamond's pictures while a Rare one shows, from its first frame: each is ready before
/// the spin comes to it (none is skipped while it paints), and a frame at rest finds them all.
fn ask_for_diamond(ui: &mut Ui) {
    for frame in 0..SPIN_FRAMES {
        ui.art.get(&diamond_key(frame));
    }
}

// --- The diamond ---

/// The diamond's size at rest, and how many pixels a unit its pictures are painted at, with a clear margin.
const DIAMOND: (f32, f32) = (80.0, 79.0);
const PAINT: f32 = 4.0;
const MARGIN: f32 = 2.0;
/// The table's and girdle's half-widths, and the crown's and pavilion's heights.
const TABLE: f32 = 23.0;
const GIRDLE: f32 = 40.0;
const CROWN: f32 = 26.0;
const PAVILION: f32 = 53.0;

/// The diamond `F` of `SPIN_FRAMES` through a facet's turn.
fn paint_frame<const F: usize>() -> ui::art::RgbaImage {
    paint_diamond(F as f32 / SPIN_FRAMES as f32)
}

macro_rules! frames {
    ($($f:literal)*) => { [$(Painter(paint_frame::<$f>)),*] };
}

const DIAMOND_FRAMES: [Painter; SPIN_FRAMES] = frames!(0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23);

/// Where a slice of the gem is seen `turn` of the way through a facet's turn (the Bundle's `View`): its edges at
/// angles φ and φ + 45° round the gem are seen at sin φ across it, so the slice is squashed to half their distance,
/// skewed so its wider girdle sits under its narrower table (or over the pavilion's point), and moved to their
/// middle; one turned away is squashed to nothing. And how much its shade darkens it, the angle of its middle.
struct View {
    middle: f32,
    squash: f32,
    lean: f32,
    place: f32,
    shade: f32,
}

fn view(index: usize, pavilion: bool, turn: f32) -> View {
    let sin = |degrees: f32| degrees.to_radians().sin();
    let from = -135.0 + 45.0 * index as f32 + 45.0 * turn;
    let middle = from + 22.5;
    let half = (sin(from + 45.0) - sin(from)) / 2.0;
    let centre = (sin(from) + sin(from + 45.0)) / 2.0;
    let facing = middle.to_radians().cos() > 0.0;
    let (lean, place) = if pavilion {
        ((-GIRDLE * centre / PAVILION).atan(), GIRDLE * centre)
    } else {
        (((GIRDLE - TABLE) * centre / CROWN).atan(), TABLE * centre)
    };
    View {
        middle,
        squash: if facing { half } else { 0.0 },
        lean,
        place: GIRDLE + place,
        shade: 0.55 * (1.0 - middle.to_radians().cos()),
    }
}

/// A facet's glint, flashing as its slice turns past `angle` from facing us.
fn glint(middle: f32, angle: f32) -> f32 {
    0.06 + 0.8 * ((middle - angle).to_radians() * 2.0).cos().max(0.0).powi(10)
}

/// The diamond, table up, `turn` of the way through a facet's turn: five slices of crown over five of pavilion, each
/// its facets in the gem's whites, greys and pale blues, a white glint over each and a shade over the slice, and
/// its edges drawn fine (the Bundle's `CrownFacets` and `PavilionFacets`).
fn paint_diamond(turn: f32) -> ui::art::RgbaImage {
    use tiny_skia as sk;
    let (width, height) = ((DIAMOND.0 * PAINT + 2.0 * MARGIN) as u32, (DIAMOND.1 * PAINT + 2.0 * MARGIN) as u32);
    let mut pixmap = sk::Pixmap::new(width, height).expect("the diamond has a size");
    type Facet = (&'static [(f32, f32)], u32, Option<f32>);
    // Crown: from the table (y 0) to the girdle (26); pavilion: from the girdle (0) to the culet (53).
    const CROWN_FACETS: [Facet; 7] = [
        (&[(-23.0, 0.0), (0.0, 13.0), (0.0, 26.0), (-40.0, 26.0)], 0xCBD9E6, None),
        (&[(23.0, 0.0), (40.0, 26.0), (0.0, 26.0), (0.0, 13.0)], 0x9FBAD3, None),
        (&[(-23.0, 0.0), (23.0, 0.0), (0.0, 13.0)], 0x7FAEDB, None),
        (&[(-23.0, 0.0), (0.0, 13.0), (0.0, 26.0), (-40.0, 26.0)], 0xFFFFFF, Some(-30.0)),
        (&[(23.0, 0.0), (40.0, 26.0), (0.0, 26.0), (0.0, 13.0)], 0xFFFFFF, Some(35.0)),
        (&[(-23.0, 0.0), (23.0, 0.0), (0.0, 13.0)], 0xFFFFFF, Some(5.0)),
        (&[(-23.0, 0.0), (23.0, 0.0), (40.0, 26.0), (-40.0, 26.0)], 0x123F66, None),
    ];
    const PAVILION_FACETS: [Facet; 7] = [
        (&[(-40.0, 0.0), (0.0, 0.0), (0.0, 24.0)], 0xE2EAF2, None),
        (&[(0.0, 0.0), (40.0, 0.0), (0.0, 24.0)], 0xB4C8DA, None),
        (&[(-40.0, 0.0), (0.0, 24.0), (40.0, 0.0), (0.0, 53.0)], 0x4F8AC2, None),
        (&[(-40.0, 0.0), (0.0, 0.0), (0.0, 24.0)], 0xFFFFFF, Some(-20.0)),
        (&[(0.0, 0.0), (40.0, 0.0), (0.0, 24.0)], 0xFFFFFF, Some(25.0)),
        (&[(-40.0, 0.0), (0.0, 24.0), (40.0, 0.0), (0.0, 53.0)], 0xFFFFFF, Some(-5.0)),
        (&[(-40.0, 0.0), (40.0, 0.0), (0.0, 53.0)], 0x123F66, None),
    ];
    const CROWN_EDGES: [&[(f32, f32)]; 2] = [&[(-23.0, 0.0), (0.0, 13.0), (23.0, 0.0)], &[(0.0, 13.0), (0.0, 26.0)]];
    const PAVILION_EDGES: [&[(f32, f32)]; 2] = [&[(-40.0, 0.0), (0.0, 24.0), (40.0, 0.0)], &[(0.0, 0.0), (0.0, 24.0)]];
    let path = |points: &[(f32, f32)], close: bool| {
        let mut path = sk::PathBuilder::new();
        path.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            path.line_to(x, y);
        }
        if close {
            path.close();
        }
        path.finish()
    };
    for index in 0..5 {
        for pavilion in [false, true] {
            let view = view(index, pavilion, turn);
            if view.squash <= 0.0 {
                continue;
            }
            // Squashed, then skewed, then moved, as the Bundle's transform group; then to the picture's pixels.
            let slice = sk::Transform::from_row(
                view.squash,
                0.0,
                view.lean.tan(),
                1.0,
                view.place,
                if pavilion { CROWN } else { 0.0 },
            );
            let transform = slice.post_scale(PAINT, PAINT).post_translate(MARGIN, MARGIN);
            let (facets, edges) =
                if pavilion { (&PAVILION_FACETS, &PAVILION_EDGES) } else { (&CROWN_FACETS, &CROWN_EDGES) };
            for (i, &(points, colour, glint_at)) in facets.iter().enumerate() {
                let alpha = match glint_at {
                    Some(angle) => glint(view.middle, angle),
                    // The last is the shade over the slice.
                    None if i == facets.len() - 1 => view.shade,
                    None => 1.0,
                };
                let Some(path) = path(points, true) else { continue };
                let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
                let [_, r, g, b] = colour.to_be_bytes();
                paint.set_color_rgba8(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
                pixmap.fill_path(&path, &paint, sk::FillRule::Winding, transform, None);
            }
            let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
            paint.set_color_rgba8(255, 255, 255, 0xB3);
            // A stroke 0.7 wide in the slice's own units, as WPF strokes it before its transform.
            let stroke = sk::Stroke { width: 0.7, ..Default::default() };
            for edge in edges {
                if let Some(path) = path(edge, false) {
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
        }
    }
    let mut image = ui::art::RgbaImage::new(width, height);
    for (to, from) in image.pixels_mut().zip(pixmap.pixels()) {
        let from = from.demultiply();
        to.0 = [from.red(), from.green(), from.blue(), from.alpha()];
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn it_shows_for_the_consoles_time_and_longer_for_a_long_description() {
        assert_eq!(shown_for(false, None), 8.5);
        assert_eq!(shown_for(false, Some(&"x".repeat(80))), 8.5);
        assert_eq!(shown_for(false, Some(&"x".repeat(81))), 11.5);
        assert_eq!(shown_for(false, Some(&"x".repeat(200))), 14.5);
        // A Rare one's goes as its sound's flourish starts, however long its description.
        assert_eq!(shown_for(true, Some(&"x".repeat(400))), 8.45);
    }

    #[test]
    fn with_a_page_of_description_or_none_the_pages_change_on_the_consoles_times() {
        assert_eq!(plan(0, 8.5), Times { pushes: vec![], cut: CUT_AT, prompt_at: PROMPT_AT, prompt_end: PROMPT_END });
        let one = plan(1, 8.5);
        assert!(close(one.pushes[0], PUSH_AT) && close(one.cut, CUT_AT) && close(one.prompt_end, PROMPT_END));
        // With more, each page is held as the console holds one while there is room: three pages in 14.5 s.
        let three = plan(3, 14.5);
        assert!(close(three.pushes[0], PUSH_AT));
        assert!(close(three.pushes[1] - three.pushes[0], PUSH_SPAN + PAGE_HOLD));
        assert!(three.prompt_end < 14.5);
        // With too little room, all held for less, the prompt still in before the Toast goes.
        let squeezed = plan(3, 8.45);
        assert!(squeezed.pushes[0] < PUSH_AT && squeezed.prompt_end < 8.45);
        assert!(close(8.45 - squeezed.prompt_end, PROMPT_HOLD * (squeezed.pushes[0] - TEXT_IN_END) / HEADER_HOLD));
    }

    #[test]
    fn the_circle_pops_in_alone_at_the_pills_centre_then_the_pill_unfurls_and_the_text_comes() {
        let times = plan(1, 8.5);
        let at = |t: f32| intro(t, false, &times, true);
        // Nothing, then the circle at 0.4 of its size, at the centre.
        assert_eq!(at(0.1).circle, 0.0);
        let popped = at(POP_END);
        assert!(close(popped.circle, 1.0) && close(popped.circle_scale, 1.0) && popped.circle_shift == CENTRE);
        assert_eq!((popped.body, popped.text, popped.logo), (0.0, 0.0, 1.0));
        // The icon takes the mark's place, zooming out from three times its size.
        let swapped = at(FLY_END);
        assert!(close(swapped.icon, 1.0) && close(swapped.icon_zoom, 1.0) && swapped.logo == 0.0);
        assert_eq!(swapped.gem, 0.0);
        // The pill unfurls, the circle riding its left end out.
        let unfurled = at(UNFURL_END);
        assert!(unfurled.body == 1.0 && close(unfurled.span, 1.0) && close(unfurled.circle_shift, 0.0));
        assert!(close(unfurled.left_end, 0.0) && close(unfurled.right_end, 0.0));
        let rest = at(REST_AGE);
        assert!(close(rest.text, 1.0) && close(rest.drift, 0.0) && rest.pages[0] == (1.0, 0.0));
        assert_eq!((rest.pages[1].0, rest.prompt, rest.shine), (0.0, 0.0, 0.0));
    }

    #[test]
    fn a_rare_ones_diamond_flies_out_of_the_gem_and_spins() {
        let times = plan(0, 8.45);
        let swapped = intro(FLY_END, true, &times, true);
        assert!(close(swapped.gem, 1.0) && close(swapped.diamond_zoom, 1.0) && close(swapped.gem_opening, 1.5));
        assert_eq!((swapped.inside, swapped.icon), (0.0, 0.0));
        assert_eq!(spin_frame(0.5), 0);
        assert_eq!(spin_frame(SWAP_AT + SPIN_PERIOD / 2.0 + 0.001), SPIN_FRAMES / 2);
        assert_eq!(spin_frame(SWAP_AT + SPIN_PERIOD * 7.0 + 0.001), 0);
        // It spins for as long as it shows: frames are always wanted.
        let moves = moves(true, &times);
        assert!(moves.iter().any(|&(from, to)| (from..to).contains(&100.0)));
    }

    #[test]
    fn it_pages_from_the_header_through_the_description_to_the_prompt() {
        let times = plan(2, 11.5);
        let at = |t: f32| intro(t, false, &times, true);
        let first = times.pushes[0];
        // The header leaves up, the first page comes from below.
        let pushing = at(first + PUSH_OUT);
        assert_eq!(pushing.pages[0], (0.0, PUSH_UP));
        let comes = first + (PUSH_IN_AT - PUSH_AT);
        assert_eq!(at(comes - 0.01).pages[1].0, 0.0);
        assert!(at(comes + PUSH_IN / 2.0).pages[1].1 > 0.0);
        assert_eq!(at(comes + PUSH_IN).pages[1], (1.0, 0.0));
        // The last page is cut for the prompt, which fades in.
        let cut = at(times.cut + 0.001);
        assert_eq!(cut.pages[2].0, 0.0);
        assert!(close(at(times.prompt_end).prompt, 1.0));
        // With the Guide off there is no prompt: the last page stays.
        let alone = intro(times.cut + 1.0, false, &times, false);
        assert_eq!((alone.pages[2].0, alone.prompt), (1.0, 0.0));
        // The light sweeps across the first page and the description's first.
        let shine = |t: f32| at(t).shine;
        assert!(close(shine(FIRST_SHINE + 0.5), 1.0) && close(shine(first + SECOND_SHINE - PUSH_AT + 0.5), 1.0));
        assert_eq!(shine(FIRST_SHINE + SHINE_SECONDS + 0.01), 0.0);
    }

    #[test]
    fn it_goes_by_collapsing_into_the_circle_which_rings_and_shrinks_to_a_dot() {
        let times = plan(0, 8.5);
        let from = intro(8.5, false, &times, true);
        let at = |g: f32| outro(&from, intro(8.5 + g, false, &times, true), g);
        let collapsed = at(COLLAPSE_END);
        assert!(close(collapsed.span, 0.0) && close(collapsed.circle_shift, CENTRE) && collapsed.body == 0.0);
        assert!(close(collapsed.icon, 0.0) && collapsed.dark == 1.0 && collapsed.ring == 1.0);
        assert!(close(at(TEXT_OUT_AT + TEXT_OUT).text, 0.0));
        assert!(close(at(LOGO_BACK_END).logo, 1.0));
        assert!(close(at(SWELL_END).circle_scale, 1.11) && close(at(SWELL_END).dark_scale, 0.82));
        let dot = at(DOT_END);
        assert!(close(dot.ring_scale, 0.08) && close(dot.dark_scale, 0.0) && close(dot.halo, 0.6));
        assert!(close(at(OUT_SECONDS).circle, 0.0));
        // Sent away before it unfurled, it goes from where it was.
        let early = intro(0.5, false, &times, true);
        let going = outro(&early, intro(0.6, false, &times, true), 0.1);
        assert_eq!((going.body, going.span), (0.0, 0.0));
    }

    #[test]
    fn between_its_moves_it_asks_for_no_frames() {
        let times = plan(1, 8.5);
        let moves = moves(false, &times);
        let moving = |t: f32| moves.iter().any(|&(from, to)| from < to && (from..to).contains(&t));
        assert!(moving(0.15) && moving(2.0) && moving(PUSH_AT + 0.1) && moving(PROMPT_AT + 0.1));
        assert!(!moving(0.5) && !moving(REST_AGE) && !moving(8.0));
    }

    #[test]
    fn the_diamond_is_painted_with_a_clear_margin() {
        let image = paint_diamond(0.0);
        assert_eq!(image.dimensions(), (324, 320));
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert_eq!(image.get_pixel(323, 160).0[3], 0);
        // Its middle is the gem.
        assert_eq!(image.get_pixel(162, 160).0[3], 255);
    }
}
