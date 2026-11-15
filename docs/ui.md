# The ui layer

Decided in #3. How every screen is built: what a screen is, how it draws, how focus, animation, art and layering work, and how screens are tested. Home, the Guide and the Launch screen are built this way as the proof.

## The shape, in one paragraph

A screen is a plain struct that **keeps its own state** (where focus is, what is open, what it remembers) and has two jobs: take a button press, and **declare what it looks like, every frame**, to a `Ui`. Nothing about its appearance is retained between frames except animated values, which live in a store keyed by id, and decoded art, which lives in a cache. There is no element tree, no layout pass and no callbacks. The `Dashboard` core owns the screens, has no window and nothing of Windows in it, and asks the shell for effects (launch this, quit that) rather than doing them.

## Why not a retained tree

The Dashboard is a fixed set of 10-foot screens steered by a pad. Its navigation rules are specific and written down (the Bundle's `CONTEXT.md`, `docs/spec/`): Down from a Recent tile goes to the wide tile nearest it, Up goes back to the tile Down left, B on any Group goes to the Recent row. Those are a few lines of `match` on a screen's own focus state; in a tree with generic spatial navigation they are exceptions fought one at a time. Declaring the frame from state also makes every screen testable with no window: construct it, press buttons, look at the picture.

## Crates

| Crate | Holds |
|---|---|
| `render` | The GPU: a `Frame` of `Layer`s, each quads then text. Textures. Fonts and shaping. Offscreen read-back. |
| `ui` | `Ui` (drawing in design units), `Store` (animated values), `ArtCache`, `Button`/`Press`, `Host` (renderer + cache + store for one window) and `Offscreen`. |
| `screens` | The screens and the `Dashboard` core that owns them. The theme's colours, type and measures (`theme`). |
| `input` | The pad through XInput, turned into `Press`es (with direction repeat). |
| `shell` | Windows itself. |
| `app` | The window and the wiring. |

## Coordinates

Screens are laid out in **design units on a 1920x1080 canvas**, the numbers the theme's XAML uses, so a measure from `docs/spec/` is typed in as it is. `Ui` scales the canvas to fit the output whole (`ui::scale_for`). On an output of another shape the canvas is centred across and sits at the top; `ui.screen()` is the whole output in design units, which is what a background or a dim fills.

Three stacks apply to everything drawn inside them: `with_alpha` (a fading page), `with_offset` (a sliding panel, a scrolled row) and `with_clip` (a list's viewport).

## Colour and blending

Colours are sRGB with straight alpha (`Color::rgb(0x2CA243)`, `Color::argb(0xE6000000)` as XAML writes them) and are **blended in gamma space on a non-sRGB target, as WPF blends**. The theme's colours and alphas therefore mean here exactly what they mean in the Bundle: a `#E6000000` band is as dark as the Bundle's. (The proof of concept blended in linear light and had to lay its dark bands twice.) Textures are uploaded as plain `Rgba8Unorm` for the same reason.

## Layers

Within a layer, quads are drawn in order and the layer's text is drawn over all of them. `ui.layer()` starts a new layer that covers everything before it, text included. A page is one layer; anything that must cover text (the Guide's panel, a dim, the Launch screen, a menu) starts its own. Layers cost one text pass each, so they are for covering, not for grouping.

## Drawing

- `fill`, `gradient` (vertical, horizontal, diagonal, radial), `ring`, `glow` (a soft band): rounded rectangles, antialiased by a signed-distance shader.
- `image` / `art`: a texture filling a box, cropped on the GPU to the box's shape (`Crop::Top` is the theme's rule for covers and backgrounds).
- `label*` for one line, `paragraph` for text in a box (wrapped, a maximum number of lines, an ellipsis, never drawn outside the box), `icon` for a Segoe Fluent Icons glyph.
- Text styles are consts in `screens::theme` (`BODY` is Segoe UI 26, the Xbox accessibility floor).

## Fonts

Loaded from Windows at runtime, never bundled: Segoe UI, Bahnschrift and Segoe Fluent Icons (`render::Font::{Ui, Display, Icons}`). On Linux, where development and CI run, the folder `DASHBOARD_FONTS` names stands in with **Selawik**, Microsoft's open font with Segoe UI's metrics, fetched by `tools/test-fonts.sh` into `target/fonts/` (never committed); text there takes the room it will on Windows. There is no icon font on Linux: `ui.icon` draws a small outlined square in its place, so screenshots show where an icon is, and what it looks like is checked on Windows.

## Animation

A screen never starts or stops an animation. It says where a thing should be and draws it where it is:

```rust
let shown = ui.tween("guide", if self.open { 1.0 } else { 0.0 }, 0.2, Ease::CubicOut);
let share = ui.approach(("recent tile", i), if focused { 1.0 } else { 0.0 }, 0.05);
```

- `tween(id, target, seconds, ease)`: when the target changes, the value goes there from wherever it is, as WPF's to-only animations do. The first time an id is seen it is at its target; `tween_from` starts it somewhere else (a fade-in).
- `slide` / `slide_from`: the same for movement, which is instant under Settings' **Reduce motion**; fades stay.
- `approach(id, target, tau)`: exponential, for the Recent row's tile growth (63% of the way every 50 ms).
- `time()`: for what turns endlessly (the Spinner, the swirl).
- A value nothing asked for this frame is forgotten, so something that leaves and comes back starts afresh.
- `Store::animating()` says whether another frame is wanted; the window draws only when something changed, is moving, or the clock's second is up.
- `set_settle(true)` ends every animation at once: a screenshot is of the screen at rest.

## Focus

Focus is **state the screen owns**: an enum such as `Focus::Recent(usize) | Focus::Wide(usize)`, moved by the screen's own `press`. Focus memory is more fields of the same struct (`left_recent`: the tile Down left). Because a covered screen keeps its struct, coming back to it finds focus where it was.

The focus visual is the theme's: a ring that appears at once on the focused element (`theme::focus_ring`), with its glow; it does not travel between elements. A Recent tile's ring grows with the tile.

A held direction repeats (`Press::repeat`); a screen ignores a repeat for anything but movement.

## Screens and what is over what

`Dashboard` holds the screens and decides who gets a press, top down: the Guide if open, nothing while a Launch screen is up, else the page. It draws bottom up: the page, the Launch screen, the Guide, each on its own layer. As pages arrive in M3 the page becomes a stack (`Vec` of page screens, the top one taking input, each keeping its state while covered); the rule is the same.

A press can ask the shell for something, returned as an `Effect` (`Launch(game)`, `QuitGame`, `GuideClosed`, `Exit`) that the window carries out; the shell reports back through methods (`launch_started`, `game_booted`, `game_ended`). The core never calls Windows, so the same core runs under a test.

The Guide, Toasts and the Launch screen will be drawn into their own windows (the Overlay, M2) by the same `Ui` and the same core; offscreen, everything is composed into one frame.

## Art

`ui.art(rect, &ArtKey { path, kind }, crop, alpha, radius)` draws a piece of art if it is decoded and asks for it if not. Decoding runs on a few threads, newest request first, since that is what is on screen. `Kind::Thumb(width)` is a tile's cover with mips, `Kind::Full(width)` a background, `Kind::Blurred` the theme's blurred cover behind a game with no background art. Art nothing has asked for in a while is freed once the cache is over its budget. Images are decoded by content, not extension: Steam stores some PNGs as `.jpg`.

## Tests

Every screen has screenshot tests in `crates/screens/tests/shots.rs`: build a `Dashboard` on a fake library (some games given generated, banded art so a crop or stretch shows), press buttons, and compare the frame at rest with a committed reference (`crates/shots`), at 1080p and at other shapes. They run on lavapipe in CI. Behaviour (where focus goes, what effects a press asks for) is asserted in the same file with no picture. `SHOTS=bless cargo test -p screens` accepts what is drawn now; **look at the image before blessing it**.
