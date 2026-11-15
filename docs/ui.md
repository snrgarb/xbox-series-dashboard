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
| `ui` | `Ui` (drawing in design units), `Store` (animated values), `ArtCache`, `Button`/`Press`, `Host` (renderer + cache + store) and `Offscreen`. |
| `screens` | The screens and the `Dashboard` core that owns them. The theme's colours, type and measures (`theme`). |
| `input` | The pad through XInput, turned into `PadEvent`s: presses (with direction repeat), releases, the Guide button's tap and hold. Whether a pad is connected and what powers it. |
| `shell` | Windows itself: the Overlay window, launching a game and following it, and holding it still under the Guide. |
| `audio` | Short sounds: WAV files decoded once, mixed on a thread of its own, played through WASAPI. Silent anywhere but Windows. |
| `app` | The two windows and the wiring. |

## Coordinates

Screens are laid out in **design units on a 1920x1080 canvas**, the numbers the theme's XAML uses, so a measure from `docs/spec/` is typed in as it is. `Ui` scales the canvas to fit the output whole (`ui::scale_for`). On an output of another shape the canvas is centred across and sits at the top; `ui.screen()` is the whole output in design units, which is what a background or a dim fills.

Three stacks apply to everything drawn inside them: `with_alpha` (a fading page), `with_offset` (a sliding panel, a scrolled row) and `with_clip` (a list's viewport).

## Colour and blending

Colours are sRGB with straight alpha (`Color::rgb(0x2CA243)`, `Color::argb(0xE6000000)` as XAML writes them) and are **blended in gamma space on a non-sRGB target, as WPF blends**. The theme's colours and alphas therefore mean here exactly what they mean in the Bundle: a `#E6000000` band is as dark as the Bundle's. (The proof of concept blended in linear light and had to lay its dark bands twice.) Textures are uploaded as plain `Rgba8Unorm` for the same reason.

## Layers

Within a layer, quads are drawn in order and the layer's text is drawn over all of them. `ui.layer()` starts a new layer that covers everything before it, text included. A page is one layer; anything that must cover text (the Guide's panel, a dim, the Launch screen, a menu) starts its own. Layers cost one text pass each, so they are for covering, not for grouping.

## Drawing

- `fill`, `gradient` (vertical, horizontal, diagonal, radial), `ring`, `glow` (a soft band): rounded rectangles, antialiased by a signed-distance shader. `arc` is part of a circle's ring with round ends, which is what the Spinner is made of (`screens::spinner`).
- `image` / `art`: a texture filling a box, cropped on the GPU to the box's shape (`Crop::Top` is the theme's rule for covers and backgrounds). `art_darkened` is the same under a black sheet that fades with it; `art_turned` stretches the whole picture over a box turned about its middle.
- `label*` for one line, `paragraph` for text in a box (wrapped, a maximum number of lines, an ellipsis, never drawn outside the box), `icon` for a Segoe Fluent Icons glyph, or a symbol that font lacks (the trophy), which comes from Segoe UI Symbol as it does in the Bundle.
- Text styles are consts in `screens::theme` (`BODY` is Segoe UI 26, the Xbox accessibility floor).

## Fonts

Loaded from Windows at runtime, never bundled: Segoe UI, Bahnschrift, Segoe Fluent Icons and Segoe UI Symbol (`render::Font::{Ui, Display, Icons, Symbols}`). On Linux, where development and CI run, the folder `DASHBOARD_FONTS` names stands in with **Selawik**, Microsoft's open font with Segoe UI's metrics, fetched by `tools/test-fonts.sh` into `target/fonts/` (never committed); text there takes the room it will on Windows. There is no icon or symbol font on Linux: `ui.icon` draws a small outlined square in its place, so screenshots show where an icon is, and what it looks like is checked on Windows.

## Animation

A screen never starts or stops an animation. It says where a thing should be and draws it where it is:

```rust
let shown = ui.tween("guide", if self.open { 1.0 } else { 0.0 }, 0.2, Ease::CubicOut);
let share = ui.approach(("recent tile", i), if focused { 1.0 } else { 0.0 }, 0.05);
```

- `tween(id, target, seconds, ease)`: when the target changes, the value goes there from wherever it is, as WPF's to-only animations do. It sets off in the frame the target changes and has moved by the next, so a frame drawn after the window slept does not start an animation part way through. The first time an id is seen it is at its target; `tween_from` starts it somewhere else (a fade-in).
- `slide` / `slide_from`: the same for movement, which is instant under Settings' **Reduce motion**; fades stay.
- `approach(id, target, tau)`: exponential, for the Recent row's tile growth (63% of the way every 50 ms).
- `time()`: for what turns endlessly (the Spinner). Asking keeps frames coming, and Reduce motion leaves it turning.
- `store.time()` is the same clock without asking for frames, and `store.wake_in(seconds)` asks for one frame that long from now: a delay is timed on the two (Home's background waits 1.5 s for focus to rest, drawing nothing meanwhile), and so is what moves too slowly to need every frame (the swirl, 30 times a second). Home's window gives animations a tenth of a second at most per frame and the clock the rest too (`Store::slept`), so a wait ends when it should however long the window slept.
- A value nothing asked for this frame is forgotten, so something that leaves and comes back starts afresh.
- `Store::animating()` says whether another frame is wanted and `Store::wake()` when one is; the window draws only when something changed, is moving, asked to be woken, or the clock's second is up. A screen that is not drawn asks for nothing.
- `set_settle(true)` ends every animation at once, and every wait on the clock: a screenshot is of the screen at rest.

## Focus

Focus is **state the screen owns**: an enum such as `Focus::Recent(usize) | Focus::Wide(usize)`, moved by the screen's own `press`. Focus memory is more fields of the same struct (`left_recent`: the tile Down left). Because a covered screen keeps its struct, coming back to it finds focus where it was.

The focus visual is the theme's: a ring that appears at once on the focused element (`theme::focus_ring`), with its glow; it does not travel between elements. A Recent tile's ring grows with the tile.

A held direction repeats (`Press::repeat`); a screen ignores a repeat for anything but movement.

## The pad's timing

Everything timed about the pad is timed in `input`, by `Presses`, which is told the time (`update(buttons, now)`) and so is tested with a clock of the test's own. The Dashboard has no clock. The pad's thread reads the pad every 4 ms whatever is being drawn and wakes the loop when it has something to tell, which is what a timer would have to do anyway: a tap of the Guide button opens the Guide 300 ms after the press, with nothing pressed and nothing drawn at that moment.

- **Repeat.** A held direction repeats at the pace of whatever has the pad: 700 ms then every 80 ms in the Guide (`screens::GUIDE_REPEAT`), 400 ms then every 120 ms elsewhere (`Repeat::default()`). The Dashboard says which (`Dashboard::repeat`), since only it knows what is open; `app` passes it on (`Pad::set_repeat`) each turn of the loop. A pace that changes while a direction is held applies from the next repeat.
- **The Guide button** (docs/spec/guide.md, 7.1) sends no `Press` as it goes down. `input` says what the press turned out to be, whatever is open: `Release(Guide)` as it is let go, then `GuideTap` 300 ms after the press or at the release if that is later; or `GuideHeld` at 700 ms, then its `Release` and no tap. `Dashboard::pad_event` gives them their meaning: a release closes an open Guide (and the tap that follows does nothing), a tap opens a closed one, a hold asks for `Effect::PowerHold`. With the Guide setting off (`guide.enabled` in the settings table) none of them does anything.
- `Dashboard::press` with `Button::Guide` is a whole tap at once: the keyboard's, `--press guide`, a test's. Ctrl+Alt+G is read with the pad and is a tap however long it is held.
- The keyboard's arrows repeat as Windows repeats keys, in the Guide too.

## Screens and what is over what

`Dashboard` holds the screens and decides who gets a press, top down: the Guide if open, nothing while a Launch screen is up, else the page. As pages arrive in M3 the page becomes a stack (`Vec` of page screens, the top one taking input, each keeping its state while covered); the rule is the same.

It declares **two pictures**, one for each window (next section):

- `draw(ui)` is Home's window: the page. The Guide is in it only where there is no Overlay to draw it on.
- `draw_overlay(ui)` is the Overlay: the Launch screen, and the Guide, **over a running game and over Home alike, so there is one Guide**, as in the Bundle. It is drawn over nothing, so wherever it draws nothing the game shows, or Home, and its dim is a dim over that.
- `overlay()` says what the Overlay is wanted for: `Nothing` (hidden), `Passive` (something to look at that takes no input: the Launch screen, the Guide sliding away) or `Guide` (it takes the foreground). The answer comes from the screens' own state, which is why each keeps how much of it showed when last drawn (`LaunchScreen::showing`, `Guide::showing`): the Overlay stays up until the fade or the slide has ended. It stays `Guide` after the Guide has closed for as long as the pad's button that closed it is down (`awaiting_release`): the foreground goes back as the button is let go, so the window that gets it never sees the press, or a second later (`RELEASE_FALLBACK`, which `app` times and reports with `release_wait_over`). A tap of the Guide button and a key hand back at once.
- `guide_over_game()` says the Guide is open over a game: the game is then held still (below).
- `home_covered()` says Home's window has nothing to show: a game is in front of it, or its Launch screen has faded in over it.
- `draw_all(ui)` is both in one frame, the Overlay's over Home's: what `--screenshot` and the screenshot tests draw.

A press can ask the shell for something, returned as an `Effect` (`Launch(game)`, `QuitGame`, `GuideClosed`, `Exit`, `Sound(sound)`) that the window carries out; the shell reports back through methods (`launch_started`, `game_booted`, `game_ended`). The core never calls Windows, so the same core runs under a test. Where there is no Overlay (`set_overlay(false)`: it could not be made, or was lost) the core shows nothing that needs one: no Launch screen, and no Guide over a game. The Guide at Home is then drawn in Home's window.

The games the screens show are a **snapshot** of the library database (`model::Library`, read by the `library` crate): the core never sees the database. When the database changes (Steam read in the background after the first frame, a Play session ending) the window hands the core a new snapshot with `set_library`, which keeps focus on the game it was on. A game is known across snapshots by its `id`; its place in the list is good for one snapshot only.

## Sounds

A sound is an effect like any other: `Effect::Sound(Sound::Move)`. No screen asks for one. `Dashboard::press` decides it for all of them, by what the press changed: focus somewhere new is the move sound, A that chose something the activation sound, the Guide opening its own. A move that went nowhere, B and the Guide closing are silent, as in the Bundle (`docs/spec/home.md`, section 9). A screen gets its sounds right by getting its focus right. The sound comes first among a press's effects.

`model::Sound` is the one table of which sound is which of the Bundle's files. The files are Microsoft's and never in this repository (#10): the window looks for them at start (`crates/app/src/sounds.rs` says where, in what order), and a sound with no file is silent. Offscreen modes and tests never take the effects to a device, so they play nothing. Volume and mute are the settings `sound.volume` (0 to 1) and `sound.muted` (`true`) in the library database.

## Two windows

Home's window is winit's: borderless, opaque, an ordinary swap chain. It is never made topmost, layered or clipped (that was the proof of concept's Launch screen, and #23). The **Overlay** (`shell::Overlay`) is a second window above every other, a running game's included, drawn with premultiplied alpha through a composition swap chain; `docs/research/overlay-window.md` has why it is made the way it is. Toasts will be drawn on it too.

| | Home's window | The Overlay |
|---|---|---|
| Shows | `Dashboard::draw` | `Dashboard::draw_overlay` |
| Shown | Always, but hidden behind a running game | Only while `Dashboard::overlay()` wants it |
| Where | The primary monitor | Each time it is shown, the monitor of the window that has the foreground then: the Guide opens on the screen the game is on |
| Input | The keyboard, while it has the foreground | Passive: none, clicked through, never activated. Guide: it takes the foreground from the game (an Alt tap, then `SetForegroundWindow`) or from Home's window (no tap: the foreground is ours to move), and gives it back to that window when the Guide closes |
| Cleared to | Black | Nothing (transparent) |

The pad is read through XInput whichever window is in front, so the Guide button works over a game.

**One GPU, one renderer, one art cache.** Both windows are surfaces of the same `wgpu` instance and device, in the same format, and are drawn by the same `Host`: its renderer, its fonts and glyphs, and its `ArtCache`, so a piece of art is decoded once and is one texture whichever window shows it (the Launch screen's art is the art Home already has up behind the focused game). What the Overlay has of its own is a `Store`: animated values are forgotten when a frame doesn't ask for them, so two pictures can't share one. `Host::with_store` draws a frame with another store in place of the Host's.

The Overlay's composition tree (device, target, visual) is made by `shell`, and wgpu is handed the visual. wgpu can make it itself, but only for every window of an instance at once, which would change Home's swap chain too.

**Who decides what.** The `Dashboard` decides what each window shows and whether the Overlay is wanted. `app` follows it every turn of the loop: shows, hides or changes the Overlay's mode, draws it while something on it is moving, and draws Home's window unless it is covered. `shell::Launch` follows the game (asked of Steam, its window up, booted, a window that wants the user, given up, ended), says when the Launch screen should go, hides Home's window as it does and brings it back when the game ends. Hidden, the Overlay is out of the compositor's scene and nothing is drawn or presented on it.

**The game held.** While the Guide is open over a game the game is frozen, as the Bundle freezes it (`docs/spec/guide.md`, 7.2): `shell::Freezer` suspends every process of a program under the game's install folder, and the one that owns its window, with `NtSuspendProcess`. `app` orders it as the Bundle does: the Overlay takes the foreground, then the game is frozen; the game is thawed, then handed the foreground. A game that is quitting is thawed and stays so, and a game that ends is let go of. There is no setting for it but the Guide's own (`guide.enabled`, on unless set): with the Guide there is the freeze. What is frozen is written first to `frozen-game.txt` in the data folder (a line a process: its id, when it started, its name) and the file is deleted at the thaw, so a Dashboard that was killed with the Guide open thaws the game at its next start, before anything else (`freeze: the Dashboard stopped last time with a game frozen`). Everything of Windows in it is behind the `Processes` trait, and the rest is tested on Linux.

**What it cannot do** is what the Bundle cannot, and it says so in the log rather than work around it: a game in true exclusive fullscreen is not covered (`overlay: Windows says a Direct3D program has the screen to itself`), and a game that runs elevated keeps the foreground and is not frozen (`overlay: Windows did not give the Overlay the foreground`, `freeze: Windows won't let ... be frozen`).

**What it costs.** Where there is no GPU every frame is drawn by the processor (WARP on the test VM), and an integrated GPU pays for one too, while a game is trying to start. So Home's window draws nothing once its Launch screen has faded in over it, nor behind a game, whoever asks for a frame. The Overlay draws only while something on it moves; the Launch screen's Spinner always does, so a frame that took longer than 50 ms (a slow renderer: with vsync a GPU's takes a refresh at most) is followed by three times as long with none, the loop turning meanwhile. A fade or a slide is not paced so, and moves by the clock: a frame is as far on as it is later than the last, so a slow renderer shows fewer frames of it, not a slower one. `shell::Launch` looks at the desktop by the clock, before anything is drawn in a turn. The Overlay is made once Home has come to rest after its first frame, or two seconds after it, or at a launch if that is sooner; `overlay: made in N ms` says what it took, and `start-up:` when each part of the start-up was ready.

Anywhere but Windows there is no Overlay (`Overlay::new` makes none).

## Art

`ui.art(rect, &ArtKey { path, kind }, crop, alpha, radius)` draws a piece of art if it is decoded and asks for it if not. Decoding runs on a few threads, newest request first, since that is what is on screen. `Kind::Thumb(width)` is a tile's cover with mips, `Kind::Full(width)` a background, `Kind::Blurred` the theme's blurred cover behind a game with no background art. `Kind::Painted(painter)` is a picture a function draws rather than a file: the swirl's bands, drawn once with tiny-skia on a decoding thread and then only moved. Art nothing has asked for in a while is freed once the cache is over its budget. Images are decoded by content, not extension: Steam stores some PNGs as `.jpg`.

## Tests

Every screen has screenshot tests in `crates/screens/tests/shots.rs`: build a `Dashboard` on a fake library (some games given generated, banded art so a crop or stretch shows; passed through a library database in memory, `from_database`, which is what gives it the saved filters that are Home's Groups and where a test seeds favourites, collections and filters of its own), press buttons, and compare the frame at rest with a committed reference (`crates/shots`), at 1080p and at other shapes. The frame is `draw_all`: the Overlay's picture over Home's window's, so the Guide over a game is seen over Home there, where on screen it is over the game; frames over time (`Stage::run`) are `draw_all` too. They run on lavapipe in CI. Behaviour (where focus goes, what effects a press asks for) is asserted in the same file with no picture. What happens over time is tested on the ui's clock: `Stage::run` draws frames a fixed step apart, `check_frame` compares the screen as it is mid-flight, and a pixel of the background says which art is up. `SHOTS=bless cargo test -p screens` accepts what is drawn now; **look at the image before blessing it**.
