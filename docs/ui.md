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
| `shell` | Windows itself: the Overlay window, the Toast strip above it, launching a game and following it, and holding it still under the Guide. |
| `audio` | Short sounds: WAV files decoded once, mixed on a thread of its own, played through WASAPI. Silent anywhere but Windows. |
| `media` | Video: a file decoded by Media Foundation (loaded at run time) into frames for a texture, its sound through WASAPI, the picture paced by the sound's clock. The Boot video's ([spec/start-up.md](spec/start-up.md)); fails at once anywhere but Windows. |
| `model` | The domain, with no I/O: a game, the library, a filter, a Play session; and `Source`, the one interface every store is behind ([sources.md](sources.md)). |
| `library` | The library database: every source merged into one SQLite file, and the snapshots the screens are given. |
| `source-steam` | Steam, as a `Source`: its install read, its games started and asked of it through `steam://` addresses. The other stores' crates come beside it. |
| `app` | The two windows and the wiring. |

## Coordinates

Screens are laid out in **design units on a 1920x1080 canvas**, the numbers the theme's XAML uses, so a measure from `docs/spec/` is typed in as it is. `Ui` scales the canvas to fit the output whole (`ui::scale_for`). On an output of another shape the canvas is centred across and sits at the top; `ui.screen()` is the whole output in design units, which is what a background or a dim fills.

Three stacks apply to everything drawn inside them: `with_alpha` (a fading page), `with_offset` (a sliding panel, a scrolled row) and `with_clip` (a list's viewport).

## Colour and blending

Colours are sRGB with straight alpha (`Color::rgb(0x2CA243)`, `Color::argb(0xE6000000)` as XAML writes them) and are **blended in gamma space on a non-sRGB target, as WPF blends**. The theme's colours and alphas therefore mean here exactly what they mean in the Bundle: a `#E6000000` band is as dark as the Bundle's. (The proof of concept blended in linear light and had to lay its dark bands twice.) Textures are uploaded as plain `Rgba8Unorm` for the same reason.

## Layers

Within a layer, quads are drawn in order and the layer's text is drawn over all of them. `ui.layer()` starts a new layer that covers everything before it, text included. A page is one layer; anything that must cover text (the Guide's panel, a dim, the Launch screen, a menu) starts its own. Layers cost one text pass each, so they are for covering, not for grouping.

A layer can be a **group**: `ui.group(bounds, blur, opacity, |ui| ...)` draws its body to a texture of its own and lays that over what is under it as one picture, blurred as WPF's `BlurEffect` blurs (a radius; 0 for none), faded as a whole, or both. It is what WPF's `Opacity` and `Effect` on an element are: the parts of a faded group do not show through each other, and a blurred one blurs its own picture, not what is behind it. The Guide's panel behind a dialog is one (blur 12), as are Power's contents while they come in; the Tile menu's menu behind its flyout will be (opacity 0.45). `bounds` is all the group draws, and the blur's reach round that; nothing of it shows outside. A group costs two more passes and a texture its size, kept from frame to frame: for the few things that need it. `ui.shadow(rect, colour, radius, blur)` is WPF's `DropShadowEffect` with no offset, for a panel that casts one.

A whole screen can be kept as a small picture: `ui.snapshot(key, shrink, blur, |ui| ...)` draws its body to a texture of its own the first frame it is asked for, `shrink` times smaller each way and blurred there, and gives the same picture back for as long as some frame asks for `key` again (`ui.snapshot_taken(key)` asks without a body); the body is not run again, and the texture is freed the first frame nobody asks. Draw it with `ui.image`. That is how the Game hub shows the screen it was opened over in the strips above and below its band, as it was then, without Home being drawn under it: the page stack takes the picture for a page whose `Page::behind` asks for one, and the page fetches it with `pages::behind`.

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

Everything timed about the pad is timed in `input`, by `Presses`, which is told the time (`update(buttons, now)`) and so is tested with a clock of the test's own. The Dashboard's own clock is told the time too (below). The pad's thread reads the pad every 4 ms whatever is being drawn and wakes the loop when it has something to tell, which is what a timer would have to do anyway: a tap of the Guide button opens the Guide 300 ms after the press, with nothing pressed and nothing drawn at that moment.

- **Repeat.** A held direction repeats at the pace of whatever has the pad: 700 ms then every 80 ms in the Guide (`screens::GUIDE_REPEAT`), 400 ms then every 120 ms elsewhere (`Repeat::default()`). The Dashboard says which (`Dashboard::repeat`), since only it knows what is open; `app` passes it on (`Pad::set_repeat`) each turn of the loop. A pace that changes while a direction is held applies from the next repeat.
- **The Guide button** (docs/spec/guide.md, 7.1) sends no `Press` as it goes down. `input` says what the press turned out to be, whatever is open: `Release(Guide)` as it is let go, then `GuideTap` 300 ms after the press or at the release if that is later; or `GuideHeld` at 700 ms, then its `Release` and no tap. `Dashboard::pad_event` gives them their meaning: a release closes an open Guide (and the tap that follows does nothing), a tap opens a closed one, a hold opens Power with no panel (docs/spec/guide.md, 6.2). With Settings' Use the Guide off (`guide.enabled` in the settings table) none of them does anything.
- **The Dashboard's one clock is the time it is told has passed**: `Dashboard::advance(seconds)`, and `toasts_presented(seconds)` (Toasts, below), which move the same clock. It reads no system clock: whoever drives it says how much time has passed (the window, at the start of each turn of its loop and after a frame of the Toasts; a test, by as much as it likes, and nothing waits), and a press is at the time it has been brought to. `Dashboard::timer()` says how soon `advance` next has something to do (and `Toasts::wake()` for the Toasts), which is how long the loop may sleep; `take_timed()` whether what it did changed the screen. Everything the Dashboard waits on to act is timed so, and nothing else: Toasts; and a game being quit has its ten seconds before "Force quit?" (docs/spec/guide.md, 7.4). A on a game's tile waits for nothing: its Game hub is up in the turn that took the press (there is no double tap since 2026-11-17, docs/spec/game-hub.md 16.2). A page is told after each press, while it is on top (`Page::tick`), for what it follows. What only shows (a fade, a slide, Home's background delay, My games & apps' settle wait) is timed by the frames, on the ui's clock (`store.time()`), as the section above says. `--screenshot --press a` draws the screen to rest before each A, since a Game hub it opens keeps a picture of the screen it opened over.
- `Dashboard::press` with `Button::Guide` is a whole tap at once: the keyboard's, `--press guide`, a test's. Ctrl+Alt+G is read with the pad and is a tap however long it is held.
- The keyboard's arrows repeat as Windows repeats keys, in the Guide too.

## Screens and what is over what

`Dashboard` holds the screens and decides who gets a press, top down: the Guide if open, nothing while a Launch screen is up, else the top page of the page stack, else Home.

### The page stack

Home is always there. Over it is a stack of **pages** (`screens::pages::Stack`): My games & apps, Search, Settings, and what M3's other tickets add. The top page takes the pad; a page under it keeps its struct, so it is as it was when the one over it goes.

- **Opening and closing are instant**, as in the Bundle: a page is there in the next frame and gone in the frame after B. Nothing fades or slides between Home and a page, or between pages.
- **A page that fills the screen is there in Home's place.** Home is not drawn under it, top bar and all, so it asks for no frames and its animated values are forgotten: when the page goes Home is drawn at its rest at once, at the rest it was left at, since its focus never moved.
- **My games & apps, Search and Settings are never up together.** A page opened from outside the stack (Home's entry points, the Guide's entries) takes the place of every page that is up, and B on it goes to Home. A page the top page itself asks for (`PageAction::Open`: Y on Games opening Search) goes over it, and B there comes back to the page under it with its focus where it was. That is where the Bundle closes the first page and opens it again on the same game; here it never went.
- **B** that a page has no use of its own for answers `PageAction::Close`. With no page left, Home's rows take focus back by the Bundle's rules (`Home::uncovered`; docs/spec/home.md, 7.4 and 11).
- A game ending closes every page, and so does the Guide's Home.

A page that does not fill the screen (`covers() == false`) is drawn over the page under it, or over Home, which goes on as it was. The **Tile menu** (`screens::tile_menu`, docs/spec/tile-menu.md) is one, and a **dialog** (`Page::dialog`): it has a dialog's sounds, a held direction repeats in it at the Guide's pace, a button pressed while it is up does nothing under it until it is let go, it closes as the Guide opens, and once closed it is still drawn until it has flown out (`Page::draw_leaving`), taking no input. Any page opens it for a game with `tile_menu::open(game, opener)`. A page that closes and acts answers `PageAction::CloseWith(effect)`, and one that hands over to another page `PageAction::Replace(open)`. A page is made from the library as it is when it opens (`Open::page`), and one with nothing to open on is not made.

A page is a module of its own under `crates/screens/src/` that implements the `Page` trait: `name` and `focus` (what `--log-focus` says, and what the move sound is decided by), `press`, `draw`, `hints`, `covers` for one that does not fill the screen and has what is under it drawn first, and `behind` for one that fills it and yet shows a picture of what it was opened over (the Game hub). It is told the library and the game playing each time (`pages::Around`) and keeps neither. **To add a page:** the module, a variant of `pages::Open` carrying how it is opened (each page's own `Entry`), and that variant's line in `Open::page`. Nothing in `Dashboard` changes. A page that shows games as Home's tiles draws them with `home::tile_at`, `status_band_at` and `title_band_at` at its own scale (Manage Queue's Recently installed, 200 x 200). A page whose focus moves as the library or the Queue changes (`Page::library_changed`, called on every Queue update while it is up) keeps its focus on its game and says so in `focus`: `--log-focus` logs the line when it changes, press or none.

### Settings and its options

Settings (`screens::settings`, docs/spec/settings.md section 16) is a page like the others, and inside it a stack of its own: the rail, a category's boxes, and the pages a box opens in the boxes' place, each a struct that implements `settings::Pane`. B climbs a level at a time. Its controls (`settings::controls`) are functions that draw a control in a rectangle, with or without focus; a pane keeps the state.

The options it holds are `screens::Options`, typed, each with its default. The Dashboard has them (`Dashboard::options`), hands them to every page with the library (`pages::Around::options`), and applies a change the moment a checkbox asks for it: Home's Recent row and background, the Guide button, the batteries' figures, and `Store::reduce_motion`, which it sets at the start of each picture it draws. It never saves one: a change goes out as `Effect::Set`, and the executable, which owns the library database, writes the settings row and reads them all back at the next start (`Options::read`, `Dashboard::set_options`).

### Text entry, and the Console keyboard

The Console keyboard (`screens::keyboard::Board`, docs/spec/search.md sections 7 to 9) is a **component, not a page**: whoever owns one gives it the presses while its keys have focus (`press`; `shortcut` while the keyboard is up and focus is elsewhere, as on Search's results), tells it which pad buttons are down (`hold`, `release`), gives it a physical keyboard's text (`typed`), and draws it (`draw` for the keys, `draw_text` for the text either side of the caret in the owner's own box). An owner whose focus can leave the keys says so (`let_go` as it leaves, `enter` as it comes back, onto the key it left). It stands at the foot of the screen whoever draws it (`Board::rect`). What a press did comes back as an `Outcome`: `Edited` when the text changed, `Enter` and `LeftTop` for the owner to answer, `Ignored` for B and the buttons no key carries.

**Text entry** (`keyboard::TextEntry`, section 10) is a page that does not fill the screen: the page under it is drawn first and dimmed. A page opens it and hears from it like this:

- It answers a press with `PageAction::Open(Open::TextEntry(Entry::new("Filter by name")))`. `Entry` is the caption, `.masked()` for a secret shown as dots, `.limit(40)` for a most.
- The text it starts from is asked of the page then, with its focus where it is: `Page::entry_text`.
- While it is up it is the top page and takes every button. What it does reaches the page under it through `Page::entry`, in the same press: `Event::Typed(text)` each time the text changes (Full library narrows on it), `Event::Kept(text)` as Menu or the Enter key keeps it, `Event::Dropped(start)` as B drops it, carrying the text it started from for a page that followed the typing to put back. After `Kept` and `Dropped` text entry has gone and the page has the pad, its focus where it was unless `entry` moved it.
- A page that opens it for more than one box knows which by its own focus, which does not move while text entry is up.

`keyboard::stand_in` is a page that opens it three ways (a name followed as it is typed, a Group's name with a most, a secret) until Full library and Settings' Groups do: `dashboard --page keyboard` starts on it, and it goes when they are built.

**Text entry over the Guide** (Direct Messages, docs/spec/party-chats.md 6): the Guide is not a page, so the page stack cannot put text entry over it. The Dashboard keeps the `TextEntry` itself (`Dashboard::writing`) when the Guide asks for one (`Guide::take_write`), gives it every press and a physical keyboard's text while the Guide is open, draws it on a layer over the Guide, and hears from it as a page does (`Event::Typed` is the reply box's draft at once, `Kept` sends it, `Dropped` keeps the draft as typed). `Entry::private()` makes the focus log say how long the text is and nothing of it (`TextEntry: Q; 12 chars, lower letters`); `TextEntry::captioned` gives a caption made as it opens.

Four more things a page can say or be told, each with a default that is right for a page with no text:

- `Page::state`: what `--log-focus` adds after the page's focus, for what a press changes without moving focus: `focus: TextEntry: A; "Ha|" lower letters`. A change in it alone is not a move and makes no sound.
- `Page::typed`: a physical keyboard's text and its Backspace (`keyboard::Typed`), which `Dashboard::typed` gives the top page. A page that takes text answers true; where none does, the window presses B for Backspace, as before. Arrows, Enter and Escape are still the D-pad, A and B.
- `Page::pad`: a pad button that the page was just given is down, or was let go, for what is drawn only while it is held (a key of the keyboard). Only a pad says so (`Dashboard::pad_event`): a keyboard's key and a test's `press` hold nothing.
- `Page::repeat`: a held direction's pace on the page. Text entry's is the Guide's, 700 ms then every 80 ms.

### The hint bar

The bar in the screen's bottom right corner (docs/spec/home.md, 10.4) is drawn **once, by the `Dashboard`**, over whatever has the pad, on a layer of its own. No page draws it: a page says which hints it wants with focus where it is (`Page::hints`, `Home::hints`), one of `hint_bar::NONE`, `MORE_OPTIONS` and `MORE_OPTIONS_AND_SEARCH`, the Bundle's three. It is anchored to the screen's corner whatever the screen's shape, is there or gone at once, and is not drawn while the Guide is open.

### Two pictures

It declares **two pictures**, one for each window (next section):

- `draw(ui)` is Home's window: Home or the pages over it, and the hint bar. The Guide is in it only where there is no Overlay to draw it on.
- `draw_overlay(ui)` is the Overlay: the Launch screen, and the Guide, **over a running game and over Home alike, so there is one Guide**, as in the Bundle. It is drawn over nothing, so wherever it draws nothing the game shows, or Home, and its dim is a dim over that.
- `overlay()` says what the Overlay is wanted for: `Nothing` (hidden), `Passive` (something to look at that takes no input: the Launch screen, the Guide sliding away) or `Guide` (it takes the foreground). The answer comes from the screens' own state, which is why each keeps how much of it showed when last drawn (`LaunchScreen::showing`, `Guide::showing`): the Overlay stays up until the fade or the slide has ended. It stays `Guide` after the Guide has closed for as long as the pad's button that closed it is down (`awaiting_release`): the foreground goes back as the button is let go, so the window that gets it never sees the press, or a second later (`RELEASE_FALLBACK`, which `app` times and reports with `release_wait_over`). A tap of the Guide button and a key hand back at once.
- `guide_over_game()` says the Guide is open over a game: the game is then held still (below).
- `home_covered()` says Home has nothing to show: a game is in front of it, or its Launch screen has faded in over it. `draw` then draws the game's backdrop in Home's place (the Launch screen without the Spinner), which fades to Home when the game ends (`backdrop_showing`, `backdrop_fading`; docs/spec/launch-screen.md).
- `draw_toasts(ui)` is the Toast on screen and nothing else: the Toast strip's picture, above both. Where there is no strip (`set_toast_strip(false)`, as until told otherwise) `draw` ends with it.
- `draw_all(ui)` is all of them in one frame, the Overlay's over Home's and the Toast over both: what `--screenshot` and the screenshot tests draw.

A press can ask the shell for something, returned as an `Effect` (`Launch(game)`, `QuitGame`, `GuideClosed`, `Exit`, `Sound(sound)`, `Power(command)`, `Favourite` and `Hidden` for the user's marks in the library database, `Install(game)` (`InstallTo` with the folder chosen in "Where do you want to install X?", docs/spec/storage.md), `Uninstall(game)` and `Steer { game, steer }` for the game's store) that the window carries out; the shell reports back through methods (`launch_started`, `game_booted`, `game_ended`). The core never calls Windows, so the same core runs under a test. Where there is no Overlay (`set_overlay(false)`: it could not be made, or was lost) the core shows nothing that needs one: no Launch screen, and no Guide over a game. The Guide at Home is then drawn in Home's window.

The games the screens show are a **snapshot** of the library database (`model::Library`, read by the `library` crate): the core never sees the database. When the database changes (the sources read in the background after the first frame, a Play session ending) the window hands the core a new snapshot with `set_library`, which keeps focus on the game it was on. A game is known across snapshots by its `id` (`Game::key`, with its store and the store's id for it); its place in the list is good for one snapshot only.

## Toasts and time

The Dashboard reads no clock of its own ("The pad's timing"), and Toasts are timed: five seconds each, half a second between (docs/spec/guide.md, section 9). So it is told how much time has passed, `Dashboard::advance(seconds)`, by the window every turn of its loop and by a test as it likes. `Toasts::wake()` says how long until the queue next changes with nothing posted, and the loop sleeps no longer than that.

It is also told what has reached the screen: `Dashboard::toasts_presented(seconds)`, after each frame with the Toasts' picture in it is presented and after the strip is hidden, with the time since it was last told the time. The queue goes by that. A Toast's five seconds begin with its first presented frame, its going with the first presented frame of that, and the next waits for a frame without the last. `Toasts::seen()` is what the screen has by those frames, which is what the log's `toasts: shown`, `going` and `gone` lines say. So whoever draws Toasts must say so, or the queue stands still (`Toasts::awaits_frame`): a test that steps time calls it as a loop would, before and after each step, with no seconds. One `advance` does one thing to a Toast (going, gone, the next shown), however much time it is told of.

On a slow renderer Home gives way to a Toast (docs/spec/guide.md, 9.4). The window says which it is (`Dashboard::set_slow_renderer`: the adapter is the processor, or Home's last frame took longer than `SLOW_FRAME`). While `Dashboard::home_gives_way()`, the swirl holds still (`set_in_front`, as under the Guide) and the loop does not ask for Home's frame while the strip is moving or about to. On a GPU none of this happens.

The banner is not animated through the store. It is drawn where its age puts it (`screens::toasts::banner::pose`: its size, its opacity, how far its tile has slid, which of its line and its prompt shows), so its coming, its turns of text and its going are one function of two numbers, tested without a frame. While it moves it asks for frames (`keep_animating`); at rest between its turns of text it asks to be woken for the next (`wake_in`) and nothing is drawn meanwhile. A settling frame shows it at rest.

`Dashboard::notify(text)` posts a notification: it joins `Dashboard::notifications()`, the list the Guide's Notifications Flyout shows, and tells of itself in a Toast. `Dashboard::greet()` is the "Hi <user>!" Toast.

## Sounds

A sound is an effect like any other: `Effect::Sound(Sound::Move)`. No screen asks for one. `Dashboard::press` decides it for all of them, by what the press changed: focus somewhere new is the move sound, A that chose something the activation sound, the Guide opening its own. A move that went nowhere, B and the Guide closing are silent, as in the Bundle (`docs/spec/home.md`, section 9). On a page focus has moved when the page's `focus()` says something else than before the press; a page opening or closing is not a move, so Y and B are silent, and A that opens a page is a choice. A screen gets its sounds right by getting its focus right. The sound comes first among a press's effects. A dialog over the Guide is told the same way, by what the press did to `Guide::dialog()`: coming up is `PromptOpen` (after `GuideSelect` if A brought it), a move `GuideMove`, A on an option `GuideSelect` or, in a prompt, `PromptClose`, B `PromptClose`. A page that is a dialog (the Tile menu) has the same sounds, told by `Page::dialog()` before the press and the page's focus after it.

## Dialogs

Two pieces, in `screens`, for every dialog there is: `menu_panel::MenuPanel<T>`, the dark panel of a title and rows (a menu, or a prompt whose title wraps and may have a message), and `wide_dialog::WideDialog<T>`, the band across the screen with tiles and Close (Power). Each holds its options and its focus, takes the pad (`press` gives an `Answer`: `Nothing`, `Moved`, `Chose(value)`, `Declined`) and draws itself where it is told; `T` is whatever its owner wants a row to mean. What is behind it, how it comes and goes, and its sounds are its owner's: the Guide dims the screen and blurs its panel; the Tile menu dims the page and flies the panel up.

Turning the machine off is never the core's to do: it asks with `Effect::Power(command)`, and the window gives that to a `shell::power::Power`. Only `SystemPower` does anything, only on Windows, and the window makes it only when it is not started with `--dry-power`. Asking a store to install or uninstall a game, or to pause, resume or cancel an install (`Effect::Steer`), is behind `shell::store::Addresses` in the same way: the window's opens what the game's store's installer opens (`model::Installer`, docs/sources.md "Installs"), else the address its source gives (`model::Source::ask`); a test's keeps what it was asked.

`model::Sound` is the one table of which sound is which of the Bundle's files. The files are Microsoft's and never in this repository (#10): the window looks for them at start (`crates/app/src/sounds.rs` says where, in what order), and a sound with no file is silent. Offscreen modes and tests never take the effects to a device, so they play nothing. Volume and mute are the settings `sound.volume` (0 to 1) and `sound.muted` (`true`) in the library database.

## Two windows

Home's window is winit's: borderless, opaque, an ordinary swap chain. It is never made topmost, layered or clipped (that was the proof of concept's Launch screen, and #23). The **Overlay** (`shell::Overlay`) is a second window above every other, a running game's included, drawn with premultiplied alpha through a composition swap chain; `docs/research/overlay-window.md` has why it is made the way it is. The **Toast strip** (`shell::ToastStrip`) is a third, made the same way, above the Overlay along the foot of the screen: only ever passive, shown only while a Toast is on screen.

| | Home's window | The Overlay | The Toast strip |
|---|---|---|---|
| Shows | `Dashboard::draw` | `Dashboard::draw_overlay` | `Dashboard::draw_toasts` |
| Shown | Always. Behind a running game it is the game's backdrop, directly below the game's window (docs/spec/launch-screen.md) | Only while `Dashboard::overlay()` wants it | Only while a Toast is on screen |
| Where | The primary monitor | Each time it is shown, the monitor of the window that has the foreground then: the Guide opens on the screen the game is on | The foot of that same monitor, each time it is shown |
| Input | The keyboard, while it has the foreground | Passive: none, clicked through, never activated. Guide: it takes the foreground from the game (an Alt tap, then `SetForegroundWindow`) or from Home's window (no tap: the foreground is ours to move), and gives it back to that window when the Guide closes | None, ever: clicked through, never activated |
| Cleared to | Black | Nothing (transparent) | Nothing (transparent) |

The strip covers the foot of the screen, the bottom 240 of 1080, and its frame is declared as the whole screen's: `Host::draw_part` lays the frame out for the whole output and draws the part the target is, so a Toast is at the same design units wherever it is drawn. The Overlay comes to be above the strip whenever it is shown, raised or given the foreground (of two topmost windows the one made so last is the upper), so each turn the strip looks for a window of the Dashboard's above it and goes back on top (`ToastStrip::keep_above`). The strip's window is also where another program posts a notification to (`model::post`, a `WM_COPYDATA` message), which is how a test makes a Toast in a running Dashboard: `standin toast <text>`. It takes one only in a Dashboard started with `--allow-toast-posts`.

The pad is read through XInput whichever window is in front, so the Guide button works over a game.

**One GPU, one renderer, one art cache.** All the windows are surfaces of the same `wgpu` instance and device, in the same format, and are drawn by the same `Host`: its renderer, its fonts and glyphs, and its `ArtCache`, so a piece of art is decoded once and is one texture whichever window shows it (the Launch screen's art is the art Home already has up behind the focused game). What the Overlay and the strip each have of their own is a `Store`: animated values are forgotten when a frame doesn't ask for them, so two pictures can't share one. `Host::with_store` draws a frame with another store in place of the Host's.

The Overlay's composition tree (device, target, visual) is made by `shell`, and wgpu is handed the visual. wgpu can make it itself, but only for every window of an instance at once, which would change Home's swap chain too.

**Who decides what.** The `Dashboard` decides what each window shows and whether the Overlay is wanted. `app` follows it every turn of the loop: shows, hides or changes the Overlay's mode, draws it while something on it is moving, and draws Home's window unless it is covered, when it draws the backdrop once and leaves it. `shell::Launch` follows the game (asked of its store, its window up, booted, a window that wants the user, given up, ended), says when the Launch screen should go, puts Home's window directly behind the game's as it does, and brings Home to the front when the game ends. Hidden, the Overlay is out of the compositor's scene and nothing is drawn or presented on it.

**The game held.** While the Guide is open over a game the game is frozen, as the Bundle freezes it (`docs/spec/guide.md`, 7.2): `shell::Freezer` suspends every process of a program under the game's install folder, and the one that owns its window, with `NtSuspendProcess`. `app` orders it as the Bundle does: the Overlay takes the foreground, then the game is frozen; the game is thawed, then handed the foreground. A game that is quitting is thawed and stays so, and a game that ends is let go of. The Bundle has no setting for it; the port has one of its own, Settings > General > Guide's **Pause the game while the Guide is open** (`guide.pause_game`, on unless set; #270), which `app` reads each turn (`window::Hold`): off, nothing is frozen and the log says `guide: the game is not frozen (Settings)` once a Guide, and a game frozen as it is turned off is thawed at once. What is frozen is written first to `frozen-game.txt` in the data folder (a line a process: its id, when it started, its name) and the file is deleted at the thaw, so a Dashboard that was killed with the Guide open thaws the game at its next start, before anything else (`freeze: the Dashboard stopped last time with a game frozen`). Everything of Windows in it is behind the `Processes` trait, and the rest is tested on Linux.

**What it cannot do** is what the Bundle cannot, and it says so in the log rather than work around it: a game in true exclusive fullscreen is not covered (`overlay: Windows says a Direct3D program has the screen to itself`), and a game that runs elevated keeps the foreground and is not frozen (`overlay: Windows did not give the Overlay the foreground`, `freeze: Windows won't let ... be frozen`).

**What it costs.** Where there is no GPU every frame is drawn by the processor (WARP on the test VM), and an integrated GPU pays for one too, while a game is trying to start. So Home's window draws nothing once its Launch screen has faded in over it, nor behind a game, whoever asks for a frame. The Overlay draws only while something on it moves; the Launch screen's Spinner always does, so a frame that took longer than 50 ms (a slow renderer: with vsync a GPU's takes a refresh at most) is followed by three times as long with none, the loop turning meanwhile. A frame's time is all the loop stood still for it, the wait for the swap chain to hand over a frame to draw on included: that wait is where a software renderer's time shows, and timed without it every frame looked quick, the loop did nothing but frames, and a launch's looks at the desktop all came late. A fade or a slide is not paced so, and moves by the clock: a frame is as far on as it is later than the last, so a slow renderer shows fewer frames of it, not a slower one. `shell::Launch` looks at the desktop by the clock, before anything is drawn in a turn. The Overlay is made once Home has come to rest after its first frame, or two seconds after it, or at a launch if that is sooner; `overlay: made in N ms` says what it took, and `start-up:` when each part of the start-up was ready.

Anywhere but Windows there is no Overlay and no Toast strip (`Overlay::new` and `ToastStrip::new` make none): Toasts are then drawn in Home's window.

## Art

`ui.art(rect, &ArtKey { path, kind }, crop, alpha, radius)` draws a piece of art if it is decoded and asks for it if not. Decoding (and scaling, and a cover's mips) runs on a few threads, newest request first, since that is what is on screen; the drawing thread only uploads, and a frame uploads what 2 ms and 8 MB allow past its first piece, leaving the rest for the next frame (#99: a frame that took in every cover at once missed two refreshes at 120 Hz). `Kind::Thumb(width)` is a tile's cover with mips, `Kind::Full(width)` a background, `Kind::Blurred` the theme's blurred cover behind a game with no background art, `Kind::Soft(radius)` the Game hub's: any picture cut about its middle to a fifteenth of the screen and blurred there. `Kind::Painted(painter)` is a picture a function draws rather than a file: the swirl's bands, drawn once with tiny-skia on a decoding thread and then only moved, and every line drawing with curves (`screens::drawn`'s marks, the Idle screen's pad). A drawing carried over from the Bundle's XAML keeps its path data as written there (`M 10,2 C ... A 3,3 0 0 1 ... Z`), read by `screens::path_data::parse` into a tiny-skia path, which tiny-skia flattens and strokes anti-aliased with round joins. Art nothing has asked for in a while is freed once the cache is over its budget. Images are decoded by content, not extension: Steam stores some PNGs as `.jpg`. A cover a game is given while it is on screen (art downloaded while the Dashboard runs, docs/sources.md "Art") fades in over a quarter of a second where a tile draws it with `ui.art_arriving(id, rect, Option<&ArtKey>, ..)`, over the game's name, which fades out under it; one the game had from the first frame is drawn as soon as it is decoded, as `ui.art` does.

A picture that must never reach the disk (a DM's avatar, docs/spec/party-chats.md 9) is **memory art**: `ui::memory_art::key(&bytes, max)` registers the encoded bytes (an `Arc<[u8]>`, held weakly by the registry, so it keeps nothing alive) and gives an `ArtKey` named `memory/<n>`, the number never reused; the decoding threads find the bytes by that name and decode them like a file's, so it is cached, uploaded and drawn round like any other art.

## Tests

Every screen has screenshot tests in `crates/screens/tests/shots.rs`: build a `Dashboard` on a fake library (some games given generated, banded art so a crop or stretch shows; passed through a library database in memory, `from_database`, which is what gives it the saved filters that are Home's Groups and where a test seeds favourites, collections and filters of its own), press buttons, and compare the frame at rest with a committed reference (`crates/shots`), at 1080p and at other shapes. The frame is `draw_all`: the Overlay's picture over Home's window's, so the Guide over a game is seen over Home there, where on screen it is over the game; frames over time (`Stage::run`) are `draw_all` too. Each is `#[ignore = "screenshot"]`: run by hand (`cargo nextest run -p screens --run-ignored only`), never in CI (#343), where `SHOTS=forbid` fails a picture compared by a test not so marked; what a screen looks like is checked on the TV PC. Tests of behaviour that need a frame drawn still run in CI, on lavapipe. Behaviour (where focus goes, what effects a press asks for) is asserted in the same file with no picture. What happens over time is tested on the ui's clock: `Stage::run` draws frames a fixed step apart, `check_frame` compares the screen as it is mid-flight, and a pixel of the background says which art is up. `SHOTS=bless cargo test -p screens` accepts what is drawn now; **look at the image before blessing it**. The Game hub's focus rules are unit tests in its own folder (`src/game_hub/`), which need no picture and no Dashboard. Where the pages go and where B leaves focus is `tests/pages.rs`, with no picture; the stack's own rules are unit tests beside it (`pages.rs`).

On the VM a test judges by `--log-focus`, which says where focus is after every press that moved it: `focus: Recent(2)` on Home, as Home's `Focus` is written, and `focus: Settings: Rail: General` with a page on top, the page's `name()` then its `focus()` (`Dashboard::focus_line`). A page with more to say than where focus is adds its `state()` after a semicolon, and a press that changes that is a line too: `focus: TextEntry: A; "Ha|" lower letters`. `focus: GameHub: 9100010 Hub Play` is the hub of the game with that appid, on its Game hub page, focus on Play.
