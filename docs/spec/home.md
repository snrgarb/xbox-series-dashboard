# Home page specification (as the Bundle draws it)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run.

Path shorthand used in citations:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`

All numbers are design units = px on a 1920x1080 canvas unless marked "authored" (inside a Viewbox, before scaling).
This spec describes Home **with the extension installed** (the full Bundle). The theme-only fallback (Playnite's own list as tiles, `T/DerivedStyles/ListGameItem*.xaml`, the `XboxThemeOnly` parts of `T/Views/Main.xaml`) is not specified here.

**The one scale factor.** The Recent row, the wide tiles and the Groups are each authored on a 1526-wide panel inside a `Viewbox Stretch="Uniform"` with `Margin="105,53,115,0"` (E/RecentRowControl.xaml:37,71; E/GroupsControl.xaml:44). At 1920 wide the Viewbox is 1920 - 105 - 115 = 1700 wide, so

    s = 1700 / 1526 = 1.114024

Everything inside those three Viewboxes (tile sizes, gaps, corner radii, focus ring, glow, fonts, badges) is multiplied by `s`. The top bar, the hero, the scrim and the hint bar are **not** inside a Viewbox and are 1:1. Confirmed by the comment at E/HomeTiles.xaml:31-40 ("1700 / 1526 = 1.1140").

**Pictures of the real thing.** `~/projects/xbox-series-theme/artifacts/e2e/` holds screenshots of the Bundle running in Playnite on the test VM (`see-all-*/0-home.jpg`, `1-play-history-tile.jpg`, `2-see-all.jpg`; `background-follows-selection-*/home.jpg`, `on-step-1.jpg`, `groups.jpg`). **They are of an older Bundle than this spec.** The newest, of 10 November, were taken with build 0.2.0 (each run's `boot.log` and `playnite.log` name `XboxSeries_0.2.0.pext`; commit `0911612`), and the Bundle at `f11e4f9` is 0.4.2. Between the two Home was changed on purpose, so a port that follows this spec does not look like the pictures in these ways, and should not be made to:

| In the pictures (0.2.0) | In the Bundle now (0.4.2), which this spec follows | Changed by |
|---|---|---|
| The Groups are a screen of their own that flies in under the top bar: a Group with focus has its header about 255 down, the top bar still showing (a clipped view 740 tall, `E/GroupsControl.xaml` at `0911612`) | Home is one page; on a Group its header is 64 down and the top bar has gone up off the screen (section 7; C/HomeRests.cs:95-108, 140-143; the theme's `CONTEXT.md`, **Home**: "the top bar gone") | `c159686` (#219) |
| Group header: Bahnschrift **SemiBold**, authored 24 (26.7 on screen), 12 over the cards | Bahnschrift **Regular**, authored 28.73 (32 on screen), 16 over the cards (E/GroupsControl.xaml:62-65; E/HomeTiles.xaml:39,43) | `65a7512` (#215), `4c69cf3` (#229) |
| A focused tile's title band: Segoe UI authored 18 (20 on screen), `MaxHeight` 52; one line's band is (8 + 18 x 1.33 + 8) s = **44.5** tall | Segoe UI authored 23.34 (26 on screen), `MaxHeight` 64; one line's band is (8 + 23.34 x 1.33 + 8) s = **52.4** tall, and a title wraps sooner (E/HomeTiles.xaml:41-42, 88-91) | `65a7512` (#215), `4c69cf3` (#229) |
| Clock: **Segoe UI** Regular 22 | **Bahnschrift SemiBold** 22, tabular figures (E/StatusArea.xaml:148) | `eae7059` (#213) |
| Wide tiles: title Segoe UI Semibold authored 22, subtitle Segoe UI authored 19 (line middles about 985 and 1015 on a tile with two lines) | Both authored 23.34 (26 on screen): line middles 976.9 and 1011.4, a lone title's 1011.4 (E/PromoRow.xaml:44-58) | `4c69cf3` (#229) |

They are good for what did not change: the top bar's parts, the Recent row with nothing focused, the badges, the wide tiles' faces and glyphs, the Play history and See all tiles' collages, and where focus starts. Pictures of 0.4.2 would have to be taken afresh (the theme repository's `e2e/run.sh`).

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite or font internals that are not in this repository.

---

## 1. Page structure and vertical layout

### 1.1 Layer order (back to front), all 1920x1080 unless noted

| # | Layer | Source |
|---|---|---|
| 1 | `PART_ViewHost` background `MainBackgourndBrush`: radial gradient, origin and centre (0.1, 1) relative, RadiusX 1.1, RadiusY 1.3; stops `#FF0B4A0B` @0, `#FF041A04` @0.55, `#FF000000` @1. Only seen if nothing covers it (the hero always does on Home). | T/Views/Main.xaml:374; T/Constants.xaml:239-243 |
| 2 | Hero (`XboxSeries_Hero`, RowSpan 2 = full screen): swirl, then two crossfading art layers (section 8) | T/Views/Main.xaml:390-391; E/HeroControl.cs:46-50 |
| 3 | Hero scrim rectangle, full screen, `XboxHeroScrimBrush` (section 8.4). Drawn over art **and** swirl, always. | T/Views/Main.xaml:392 |
| 4 | Top bar `XboxTopBar` | T/Views/Main.xaml:473-665 |
| 5 | `XboxSeries_RecentRow` (Recent row + wide tiles) and `XboxSeries_Groups`, both in the same grid slot | T/Views/Main.xaml:753-756 |
| 6 | Game hub (`PART_ElemGameDetails`), hint bar (`XboxSeries_Hints`), Tile menu, game status, text entry: over the whole page | T/Views/Main.xaml:761-781 |

### 1.2 The slot

Main grid rows: `200` then `*` (T/Views/Main.xaml:375-378). Inside row 1 a grid with rows `*`, `560`, `40` (T/Views/Main.xaml:678-682). At 1080 tall: row 1 = 880, so the inner rows are 280 / 560 / 40. The Recent row and Groups controls sit in the 560 row:

- **Slot: y 480 to 1040**, full width. (CT/HomePageXamlTests.cs:57 also states "The slot starts 480 down".)
- Content overflows the slot downward on purpose: `StackPanel Margin="0,0,0,-240"` (E/RecentRowControl.xaml:29) and `Margin="0,0,0,-400"` (E/GroupsControl.xaml:39) only stop WPF clipping. Nothing is clipped to the slot.

### 1.3 Resting layout (page offset 0, focus on the Recent row)

| Element | Authored | On screen (x, y, w, h) | Source |
|---|---|---|---|
| Top bar | Margin 105,84,105,0; Height 60 | x 105..1815, y 84..144 | T/Views/Main.xaml:473 |
| Hero gap | (nothing drawn) | y 144..533 | - |
| Recent row box | 1526 x 245, Viewbox margin top 53 | x 105..1805, y 533..805.94 (h 272.94) | E/RecentRowControl.xaml:37-39 |
| Recent tile, unfocused | 153 x 153, bottom-aligned in the box | 170.45 square, y 635.49..805.94 | E/HomeTiles.xaml:47-52; E/TileGrowth.cs:18 |
| Recent tile, focused | 245 x 245 | 272.94 square, y 533..805.94 | E/TileGrowth.cs:19 (153 + 92) |
| Gap, wide tiles under Recent row | `Grid Margin="0,24,0,0"` | 26.74 | E/PromoRow.xaml:75 |
| Wide tiles row | 1526 x 214 box (24 + 190) | x 105..1805, tiles y 832.67..1044.34 (h 211.66) | E/RecentRowControl.xaml:71-73; E/PromoRow.xaml:74-84 |
| Gap, first Group under wide tiles | Canvas `Margin="0,40,0,0"` | 44.56 | E/GroupsControl.xaml:57 |
| Group 0 top (header top) | `AboveGroups` Height 459 (= 245 + 214) + 40 = 499 below Viewbox top | y = 533 + 499 s = **1088.90** | E/GroupsControl.xaml:51,57 |
| Group header line | font 28.73, line height ~1.2 em = 34.48 (UNVERIFIED font metric, see below) | 38.41 tall | E/GroupsControl.xaml:63-65; CT/HomePageXamlTests.cs:59 |
| Gap header to cards | header `Margin="0,0,0,16"` | 17.82 | E/GroupsControl.xaml:63 |
| Group card | 194 x 296 | 216.12 x 329.75 | E/GroupsControl.xaml:25-31 |
| Gap between Groups | row `Margin="0,0,0,40"` | 44.56 | E/GroupsControl.xaml:61 |
| Group pitch | 34.48 + 16 + 296 + 40 = 386.48 | **430.54** | derived |
| Group i top | | 1088.90 + i x 430.54 (1088.90, 1519.44, 1949.98, 2380.52 ...) | derived |
| Cards top within a Group | 34.48 + 16 = 50.48 | 56.23 below the Group's top | derived |

Header line height: the Bundle measures it at runtime (E/GroupsControl.xaml.cs:252-278 measures real positions). The 1.2 em figure is what the repo's own test assumes for Bahnschrift (CT/HomePageXamlTests.cs:59: `headerSize * 1.2`). The unit test fixture CT/HomeRestsTests.cs:13-15 uses rounder, slightly different numbers (first Group 1092, pitch 435, cards 61 under the header, 330 tall): treat those as illustrative, and the derived numbers above as the XAML's.

Horizontal extents common to all three rows: left edge x = 105, right edge x = 1805 (the avatar's left edge to the Notifications button's right edge; E/RecentRowControl.xaml:30-36).

---

## 2. Top bar

Grid at x 105..1815, y 84..144, three columns `*`, `Auto`, `*` (T/Views/Main.xaml:473-480). Everything is vertically centred on y = 114. Its RenderTransform is the extension's `TopBarScroll` (section 7). It is hidden (collapsed) while My games & apps, Settings or Search is up (`XboxHomeOnly`, T/Views/Main.xaml:307-319).

Text baseline nudges: all Segoe UI text in the bar is drawn 2 px **up** (`TranslateTransform Y="-2"`), the Bahnschrift clock 2 px **down** (`Y="2"`), to sit on the icons' centre (T/Views/Main.xaml:208-215; E/StatusArea.xaml:147-152).

### 2.1 Left column: profile (focusable)

Style `XboxTopBarProfile`, T/Views/Main.xaml:179-295. Left-aligned at x = 105.

| Part | Value | Line |
|---|---|---|
| Picture | 56 x 56 circle (x 105..161, y 86..142). Under it: ellipse `#FF107C10` (GlyphBrush) with glyph U+E77B (Contact) in the icon font, 26 px, white. Over it: the profile picture, `UniformToFill`, clipped to a circle r = 28. A picture that fails to load leaves the glyph. A Profile with no picture of its own shows the Windows account's (C/ExtensionCore.Startup.cs:425), and an account with none shows Windows' default, `user-192.png` (E/WindowsUserAccount.cs:90-115): a grey figure, head and shoulders, on a pale disc. That, not the green disc, is what the pictures show. | 190-198 |
| Focus ring | Round border `Margin="-7"`, `BorderThickness="3"`, `CornerRadius="999"`, brush `#FF2CA243`, no fill. So ring outer diameter 70, inner 64: 4 px clear of the picture. Glow: `XboxFocusGlowRound` with `Margin="-7"` (section 4.3). Hidden unless focused. | 199-206 |
| Name block | 16 px right of the picture (`Margin="16,0,0,0"`), translated Y -2 | 212-215 |
| Gamertag | Segoe UI 26, white (`XboxText`, lines 23-27). Text = profile gamertag, fallback the Windows user name. The port's is the signed-in Profile's (profiles.md 3). | 218-219 |
| Score line (under the gamertag, same stack) | A 19 x 19 white disc with a black "G" (Segoe UI **Bold** 13, centred); then the score, Segoe UI 26 white, `Margin="6,0,0,0"`, formatted `{0:N0}` (thousands separators). Collapsed when there are no achievement totals. | 220-228, 243-246 |
| Real name | Segoe UI 26 white, same place, `Opacity="0"` at rest | 230-231 |

Note: the header comment says the gamertag is "made smaller" over the Score, but the code sets 26 in both states (lines 219 and 245). With the Score showing, the two 26 px lines (about 69 px) overflow the 60 px bar, centred.

Name swap (only if the profile has a real name, the bar is visible, Reduce motion off): 15 s loop at 30 fps. Gamertag+Score opacity 1 for 0..7.2 s, linear to 0 by 7.5 s, 0 until 14.7 s, linear to 1 by 15 s; Real name is the mirror image (T/Views/Main.xaml:255-284). Reduce motion: gamertag stays.

Action (A / click): `OpenProfileCommand` -> `core.OpenProfile()`: plays `OpenMenu.wav`, opens the Guide on Profile & system; nothing if the Guide is off or already open (E/StatusControl.cs:28; C/ExtensionCore.cs:933-943).

### 2.2 Centre column: three round buttons (focusable)

Style `XboxTopBarButton`, T/Views/Main.xaml:91-168: 60 x 60, `Margin="10,0,10,0"` (80 px pitch). The Auto column is 240 wide and centred at x = 960.

| Order | Button | x (y is 84..144) | Glyph (font: `Segoe Fluent Icons, Segoe MDL2 Assets`, 26 px, white) | Focused glyph | Action | Lines |
|---|---|---|---|---|---|---|
| 1 | My games & apps | 850..910 | U+E8F1 (Library), outline | The outline hides; a 26 x 26 filled vector path `XboxLibrarySolid` shows (path data at line 76) | `OpenGamesCommand`: My games & apps on Games | 514-529 |
| 2 | Search (`XboxTopBarSearch`) | 930..990 | U+E721 (Search) | Same glyph, `FontWeight=Bold` | `OpenSearchCommand`: the Search page | 540-552, 78-85 |
| 3 | Settings (`XboxTopBarSettings`) | 1010..1070 | U+E713 (Setting) | U+F8B0 (solid gear) | `OpenSettingsCommand`: the Settings page | 563-575, 41-48 |

Focused state of a round button (lines 102-164):

- Ring: the button's own 60 x 60 round border, `BorderThickness="3"`, brush `#FF2CA243`, **fill `#33FFFFFF`**, `CornerRadius="999"`; glow `XboxFocusGlowRound` with `Margin="-7"`.
- Label: the button's `Tag` text under it, Segoe UI 26 white, centred, `Margin="-150,70,-150,-40"` (top of the text box 70 below the button's top, i.e. 10 under the ring; up to 360 wide; does not move neighbours).
- Icon scale: 1 -> 1.2 about its centre in 120 ms, `CubicEase EaseOut`; back to 1 in 120 ms **linear** (no easing on exit). No scale with Reduce motion on (ring and label still appear).

### 2.3 Right column, right-aligned, in this left-to-right order

T/Views/Main.xaml:578-664.

1. **Progress indicator** (`PART_ElemProgressIndicator`, Playnite shows it only while it updates the library; not focusable): status text (`PART_TextProgressTooltip`, Segoe UI 26, `#B3FFFFFF`, Y -2, `Margin="0,0,12,0"`) then a 26 x 26 **Spinner** (section 10.1); the pair has `Margin="0,0,20,0"`. Lines 579-590.
2. **Update button** (`PART_ButtonProgramUpdate`, Playnite shows it only when a Playnite update exists; focusable `XboxTopBarButton`, Tag "Update"): glyph U+E896 in `#FF2CA243`. Not relevant to a standalone port. Lines 592-594. It is the green down arrow left of the pad's glyph in the pictures (about x 1541); it has nothing to do with the Queue.
3. **Status area** (`XboxSeries_Status`, `Margin="10,0,10,0"`, not focusable, not hit-testable): section 2.4. Its right edge is at x = 1725.
4. **Notifications button** (`PART_ButtonNotifications`, focusable `XboxTopBarButton`, Tag "Notifications"), x 1745..1805: glyph U+E91C (ActionCenter) outline; focused, the filled path `XboxActionCenterSolid` (line 77). A 10 x 10 dot `#FF2CA243` at the icon's top right (`Margin="0,-4,-6,0"`) while Playnite's notification count is not 0. Action: Playnite's own notifications menu. Lines 646-663.

### 2.4 Status area (row form)

E/StatusArea.xaml:139-166 (`RowTemplate`), horizontal, each part followed by a 16 px gap that collapses to 0 when the part is absent (lines 156-164). Left to right:

| Part | When | Drawing | Lines |
|---|---|---|---|
| Discord mark | Only in a Discord call | 30 x 24 box; the Discord logo path 24 x 18, fill `#B3FFFFFF`, or `#FF2CA243` while the user speaks. Badge 14 x 14 at bottom right when muted/deafened: disc `#FF1A1A1A` with 1 px white stroke, glyph U+E720 (mic) or U+E7F6 (headphones) at 8 px white, struck by a white 1.25 px line from (3.5,3.5) to (10.5,10.5), round caps. | 108-136 |
| PC battery | Only on a PC with a battery | Percent text ("57%"), Segoe UI 26 white, Y -2, 4 px gap; then an upright battery glyph, icon font 26 px white: U+F5F2 + tenths (0..10) discharging, U+F5FD + tenths charging (the glyph carries its own bolt), U+F608 unknown; tenths = (percent + 5) / 10 integer division. Percent hidden if unknown or if "Show battery percentages" is off. | 38-63; C/IPcBattery.cs:46-68 |
| Pad battery | Pad 1 connected | Wireless: percent text `{0}%`, Segoe UI 26 white, Y -2, 8 px gap; then the cell drawn by hand: a 30 x 16 border, white 2 px stroke, radius 3, padding 2, with a white fill bar (radius 1) left-aligned and as wide as percent of the inner width; then a 3 x 7 white nub with right corners rounded 1.5. Wired: only glyph U+E7FC (game controller), 24 px white. No pad: nothing. Percent text hidden if "Show battery percentages" is off (default on, C/StartupState.cs:197). **In the port** (#191, below the table): the cell only for a known percentage, else the glyph. | 14-30, 69-98 |
| Clock | Always | `Bahnschrift SemiBold` 22, white, tabular numerals, Y +2. Text = `DateTime.Now.ToString("t", CurrentCulture)` (the user's short time, e.g. "21:07" or "9:07 PM"). Refreshed on each minute boundary. | 147-153; C/ExtensionCore.cs:425-486 |

**The pad's battery in the port (#191).** What Windows says of the first pad, and who says it, is in `docs/research/pad-battery.md`. Three cases, the same here and in the Guide's Status area (guide.md 2.3); only the first draws a battery:

- **A percentage** (a pad on Bluetooth LE, from the percentage Windows keeps for it; or a pad with real battery reporting): the cell filled to it, and `{0}%` beside it while Show battery percentages is on.
- **A level only** (XInput's four, or Windows.Gaming.Input's 100/70/40/10, which are the same four; the Xbox Wireless Adapter's case): **the controller glyph**, as for nothing known. The user's call on #191: a rough level is not worth showing ("just don't show it"). The level is still read and logged (`gamepad: battery level 1 of 3`), so what a pad reported can be seen.
- **Nothing known**, or a cable (a virtual pad, a 2.4 GHz dongle, a pad on USB): the controller glyph U+E7FC, as the Bundle draws a wired pad. The Bundle draws nothing for a pad that says nothing; the user asked for the glyph (#191).

Read on the pad's battery thread (`input::battery`), never the pad's or the window's: as the first pad connects, when Windows.Gaming.Input finds a pad, and once a minute after (the Bundle's minute). The log says each change: `gamepad: battery 85%`, `level 1 of 3`, `wired`, `not known`. `--fake-pad-battery <85 | empty | low | medium | full | wired | none>` puts a made-up answer in Windows' place (the e2e payload `pad-battery`). The Bundle has **no low-battery Toast**, so neither has the port. Pictures: `pad-battery-{percent,level,none}-{home,guide}` in `tests/shots.rs` (the level's and none's alike: the glyph).

### 2.5 Top bar navigation

- Left/Right: to the next focusable, visible, enabled button in the bar by x position; stops at the ends, never wraps, never drops to the Recent row (E/StatusControl.cs:57-78). Focusable set: profile, My games & apps, Search, Settings, (Update), Notifications.
- Down: WPF's default directional navigation to the Recent row (nearest element). Up from the Recent row: `PredictFocus(Up)`, i.e. the geometrically nearest top-bar button (E/HomeRows.cs:216-232). The exact button chosen is WPF's heuristic: **UNVERIFIED**, not coded in the Bundle.
- With focus on the top bar the page rests as for the Recent row (C/HomeRests.cs:9-10) and the background is the swirl (section 8).
- B on the top bar: **NOT FOUND** (no handler; nothing happens on Home).

---

## 3. Recent row

### 3.1 Contents and count rules

C/ExtensionCore.cs:17-27, 1380-1435.

- Game tiles = the running game first (if any), then played games (`LastActivity != null`, not hidden) by last played descending, then padded with never-played, not-hidden games by date added descending; de-duplicated; take **7** (Show Play history on, the default) or **8** (off).
- Then the **Play history tile** (only when Show Play history is on). Its collage = the next up to 4 played games not already in the row.
- So the row is 8 tiles when the library has enough games. With fewer, the row is shorter and stays **left-aligned** at x = 105 (tiles are in a left-aligned horizontal StackPanel; E/RecentRowControl.xaml:39-63).
- The row list is only replaced when what it shows changed (C/ExtensionCore.cs:1402).

### 3.2 Tile geometry and growth

E/TileGrowth.cs (whole file), E/HomeTiles.xaml:46-52.

Authored constants: `TileSize = 153`, `FocusGrowth = 92`, `TileGap = 30`. Tiles are bottom-aligned in the 245-tall box, so a tile grows **up and to the right**: its bottom-left corner is the anchor; tiles to its right shift right, tiles to its left do not move.

Per frame, for each tile i with growth share g_i in [0,1] (target 1 if keyboard-focused, else 0):

    step  = 1 - exp(-dt_ms / 50)            (step = 1 with Reduce motion: one frame)
    g_i  += (target_i - g_i) * step
    size_i = 153 + 92 * g_i                  (width = height)
    gap    = 30 + 92 * max(0, 1 - sum(g)) / (n - 1)     (n = tile count; n > 1)
    right margin of every tile but the last = gap

When every |target - g| < 0.002 all snap to target and the animation stops. This is an exponential approach with a 50 ms time constant ("about 98% of the way in 0.2 s"), not a fixed-duration tween; it has no named easing function. Shares always sum to a constant so **the row's width never changes** (1526 authored when n = 8):

- One tile focused: 8 x 153 + 92 + 7 x 30 = 1526. Gaps 30 (33.42 on screen).
- No tile focused (focus on the top bar, wide tiles, etc.): tiles all 153, gaps widen to 30 + 92/7 = 43.14 (48.06 on screen), row still 1526.

On-screen x of tile i (0-based) with tile k focused, n = 8: `105 + s * (183 * i + (i > k ? 92 : 0))`. No layout rounding (`UseLayoutRounding="False"`, E/RecentRowControl.xaml:38-39).

### 3.3 Tile face (shared by Recent row tiles and Group cards)

All authored units (multiply by s). E/HomeTiles.xaml.

| Part | Value | Lines |
|---|---|---|
| Tile background | `#FF1A1A1A`, clipped to a rounded rect radius 6 (6.68 on screen) that follows the tile's size | 67; E/RoundedClip.cs |
| Cover | `UniformToFill`, horizontally centred, **top-aligned** (crops the bottom of a portrait cover, the sides of a wide one), no letterbox bands, Fant (high quality) scaling. Decoded at 300 px wide. Laid out once at 245 x 245 (194 x 296 on a Group card) and scaled with the tile. | 135-148, 159-163 |
| Square art (the port's, #110) | A Recent tile is square: it shows the game's **square** picture where it has one (the Microsoft Store's box art, a package's large tile logo, SteamGridDB's 1024x1024 or 512x512 grid: docs/sources.md, "Art"), whole, decoded and fitted as the cover is. **Without one (#190), a square tile never crops a portrait cover:** it shows the picture it has (the cover, else the header, else the background) **whole**, as large as fits and in the tile's middle (a cover is the tile's height and two thirds of its width), over the same picture blurred (the theme's blurred cover, `Kind::Blurred`, as Home's background has it) filling the tile, cropped about its middle and darkened to 55 % of its colour. What is shown whole is decided by the picture as decoded, not by where it came from: a picture is square when its width and height are within a tenth of each other, and one kept as square that is not is shown whole too. The Bundle crops here (`TileArt`: `UniformToFill`, top-aligned), since in Playnite the user chose square covers for a square layout; the port has the user's own covers from the stores, which are portrait, and the user asked for no cropped portrait on a square tile (#190), so this is the port's own. Every square tile does the same: the Play history tile's cells, the Guide's Recent and Running covers, Search's tiles, My games & apps' tiles and Play history rows, Manage Queue's rows and a Toast's game. A Group card, portrait, keeps the cover cropped from the top as the Bundle's, and a game with no cover shows its square cropped there, before its header or its background | `screens::home::{square_art, art_for, fit, face}` |
| No cover / cover fails to decode | The game's title: Segoe UI 23.34 (26 on screen), white, centred both ways, wrapping, `Margin="10"` | 165-167, 203-205 |
| Source badge | Bottom left, `Margin="10"`: a white 20 x 20 vector icon (Steam or Xbox path, lines 21-22) over a 36 x 36 pre-blurred shadow image (`shared/Images/source-steam-shadow.png`, `source-xbox-shadow.png`; equivalent to a drop shadow radius 6, 0.8 black, no offset). Only for Source = "Steam" or "Xbox". Opacity 0 (instantly) while the tile is focused. | 176-185, 206-218 |
| Running badge | Only on the running game. Top left, `Margin="6"`, height 24, background `#E62B2B2B`, radius 3. Inside: a 24-wide play mark box `#F0181818` (radius 3; 3,0,0,3 when focused) with a white triangle `M0,0 L9,5.5 L0,11 Z`. Focused: "CONTINUE" beside it, Segoe UI Semibold + Bold, 12, white, `Margin="7,0,8,0"`. | 190-199, 216-223 |
| Status band (unfocused) | Only when the tile has an install status. Bottom, full width, `#E6000000`; text "Queued" or "Installing..." Segoe UI 23.34 white, `Margin="8"`, one line, character ellipsis. Opacity 0 while focused. | 75-80, 116; C/ExtensionCore.Queue.cs:15-16 |
| Title band (focused) | Bottom, full width, `#E6000000`, at rest `Opacity="0"`. Stack with `Margin="8"`: the title, Segoe UI 23.34 (26 on screen), white, wraps, `MaxHeight="64"` (two lines; Segoe line = 1.33 em), character ellipsis on the second; then, if any, the install status on one line, same font. Fade in: opacity to 1 over 150 ms after a 50 ms delay, linear. Fade out: to 0 over 100 ms, linear. | 57-63, 86-97 |
| Focus ring | Section 4 | 106-111 |

The source icons are the stores' own logos: NOTICE.md lists `source-steam-shadow.png` as "The Steam logo", Valve's, and `source-xbox-shadow.png` as Microsoft's; the vector paths are Simple Icons' drawings of the same marks. In the pictures the Steam mark shows as a white disc about 22 across with the logo cut out of it. Neither the paths nor the shadow pictures are the theme author's to license, so a port does not copy them.

#### 3.3.1 The launchers' marks (the port's, #189)

The Bundle's two marks are Valve's and Microsoft's logos, and the port has a launcher for each of seven stores. The user's choice (#189): **the project's own flat drawings**, one a launcher, shipped in the code. Each is an original, single-colour drawing of an idea its launcher brings to mind, never its logo or a tracing of one; no company's image or icon file is in the repository. They are vector paths on a 24 x 24 grid (`screens::drawn`), painted with tiny-skia at the smallest of 32, 64, 128 or 256 pixels that is at least as large as they are shown, in white, and drawn in the colour asked for. Placement and size are the Bundle's (`SourceBadge`, above): 20 x 20 (`Stretch="Uniform"`: the drawing fills the square), 10 in from the tile's bottom left, on the shadow, gone while the tile has focus.

| Launcher (`Game::source`) | Drawing | Why |
|---|---|---|
| Steam (`steam`) | A white disc with three wisps of steam rising through it, cut out | Steam; the Bundle's mark shows as a white disc with a shape cut from it |
| Epic Games (`epic`) | A shield, a five-pointed star cut out of it | "Epic": a hero's shield |
| GOG (`gog`) | A planet with a ring round it, the ring's front crossing it with a gap either side | GOG GALAXY |
| Battle.net (`battlenet`) | Two swords crossed, point up, the front one cut clear of the back | "Battle" |
| EA (`ea`) | A lightning bolt | "Electronic" Arts |
| Ubisoft (`ubisoft`) | Two rings linked, each over the other once | Ubisoft "Connect" |
| Xbox and the Microsoft Store (`xbox`) | A pad: body and grips, a cross and two buttons cut out | The console's pad; never the Xbox sphere |
| A game added by hand (`manual`) | A folder, a plus cut out of it | Added from a folder on the PC |
| An App | A window: a rounded frame, its title bar solid | A program |
| Any other (`Launcher`) | A box seen corner on: a hexagon cut in three faces | A game from somewhere with no drawing yet |

Every launcher's games carry their mark, not only Steam's and Xbox's: the tiles on Home and in My games & apps (Games, Full library, Play history, Manage Queue's Recently installed and rows), the Guide's recent games (18 across), the Game hub's cover chip, and Settings > Launchers' rows (28 across). A game of a source with no drawing carries the box, where before it carried nothing. `crates/screens/tests/launcher_marks.rs` draws them all on tiles at 20, at 40 (a tile's 20 in a 4K screen's pixels) and enlarged; `screens::drawn`'s unit tests check each paints, at every size, and that no two are alike.

There is **no "installed" badge** on Home tiles: only the source icon, the Running badge and the Queued / Installing... status (NOT FOUND otherwise). A not-installed game looks like any other.

### 3.4 Play history tile

E/RecentRowControl.xaml:55-62; E/HomeTiles.xaml:227-265.

- A `HomeTile`-styled tile (same size, growth, ring, title band reading "Play history"), last in the row, right margin 0.
- Face: a 2 x 2 grid of up to 4 covers; grid inset 4 from the tile edge, each cover with a 3 margin (so 7 from the edge, 6 between covers), each `UniformToFill`, top-aligned, decoded at 150 wide. No per-cover rounding (only the tile's clip).
- The grid's inset and the covers' margins are not in the cover's Viewbox: they stay 4 and 3 (authored) as the tile grows, and the four cells share what is left.
- A game in the collage with no cover leaves its cell empty (the tile's `#FF1A1A1A` shows); the glyph is only for a collage with no games at all (`HasItems` false, E/HomeTiles.xaml:258-260).
- With no covers: glyph U+E81C (History), icon font 40, white, centred.
- No source badge, no Running badge, no status.

### 3.5 Buttons on a Recent-row tile

| Input | Game tile | Play history tile | Source |
|---|---|---|---|
| A | Starts a 300 ms double-tap window. No second A: opens the game's **Game hub** after the 300 ms. Second A on the same tile within 300 ms: **plays the game** (or returns to it if it is the running game). | Opens My games & apps on Play history at once | C/ExtensionCore.cs:35-36, 1327-1362; E/RecentRowControl.xaml.cs:287-300 |
| Menu (Start) | Opens the **Tile menu** (plays `PromptOpen.wav`), centred over the dimmed page; cancels a pending A | Nothing | C/ExtensionCore.cs:840-843; C/ExtensionCore.TileMenu.cs:59-71 |
| Y | Opens **Search** (anywhere on Home, any focus) | Same | E/PlayniteViewHost.cs:27-32; C/ExtensionCore.Search.cs:61-74 |
| X | **NOT FOUND**: no action on Home | - | - |
| View (Back) | Nothing on Home | - | C/ExtensionCore.cs:844-849 (Manage Queue only) |
| B | Nothing on the Recent row | - | - |
| Left / Right | Next tile; nothing at either end (no wrap) | | E/HomeRows.cs:216-232 |
| Down | To the wide tile whose centre x is nearest this tile's centre x; the tile left is remembered | | E/RecentRowControl.xaml.cs:228-234; E/PromoRow.xaml.cs:84-89 |
| Up | To the top bar | | E/HomeRows.cs:216-232 |

The hint bar is **not** shown on the Recent row (section 10.4).

---

## 4. Focus ring

There is one focus visual, per element, toggled **instantly** with focus (Hidden/Visible). It does not slide between elements, does not pulse, and has no animation of its own; on a Recent-row tile it simply grows with the tile. (Pulse / travel animation: **NOT FOUND**.)

Colour everywhere: `GlyphLightBrush` = **`#FF2CA243`** (T/Constants.xaml:7,10). The darker accent `#FF107C10` is not used for the ring.

### 4.1 Rectangular ring (Recent tiles, Group cards, wide tiles)

E/HomeTiles.xaml:100-111; E/PromoRow.xaml:25-31. Authored (multiply by s = 1.114 on Home):

- `Border BorderThickness="4" CornerRadius="14" Margin="-8"`: a 4-thick stroke whose inner edge is 4 outside the tile edge and outer edge 8 outside. No fill, no inner or second stroke.
- WPF corner semantics (a port will get this wrong): a Border's `CornerRadius` is the radius of the stroke's **centre line**. So the ring's outer radius is 16, centre-line 14, inner 12 (authored). The source comment intends "6 + 4 + 4 = 14" as the outer radius; what WPF draws is 16 outer.
- On screen: stroke 4.46, gap 4.46, centre-line radius 15.60.
- The tile under it has radius 6 (6.68). The neighbour tile at a 30 gap is left 22 clear.

### 4.2 Round ring (top bar)

1:1 units. Round buttons: the 60 x 60 border itself, 3 thick, fill `#33FFFFFF`. Profile: 3 thick, 7 outside the 56 picture (outer diameter 70), no fill. T/Views/Main.xaml:104-109, 201-206.

### 4.3 Glow

T/Constants.xaml:11-191. A static faint green halo centred on the ring's centre line; no blur effect, no animation. It is built from ten nested, very faint stroke bands plus four corner spots, and cached as a bitmap.

Let c = the ring's centre line (for the rectangular ring: 6 outside the tile edge, authored). Each band is a rounded-rect stroke centred on c, with WPF `CornerRadius="12"` at its own centre line (999 = fully round for the round variant), colour `#2CA243` at the given alpha, painted in this order (alphas accumulate where bands overlap):

| Band | Half-width either side of c | Total width | Alpha |
|---|---|---|---|
| 1 | 15.5 | 31 | 0.034 |
| 2 | 12.75 | 25.5 | 0.035 |
| 3 | 11 | 22 | 0.036 |
| 4 | 9.5 | 19 | 0.038 |
| 5 | 8.25 | 16.5 | 0.039 |
| 6 | 7.25 | 14.5 | 0.041 |
| 7 | 6.25 | 12.5 | 0.042 |
| 8 | 5 | 10 | 0.044 |
| 9 | 3.75 | 7.5 | 0.046 |
| 10 | 2 | 4 | 0.048 |

(Derived from the glow control's `Margin="-7.5"` inside the ring's 4-thick border, and each band's `Margin` / `BorderThickness`: -10/31, -7.25/25.5, -5.5/22, -4/19, -2.75/16.5, -1.75/14.5, -0.75/12.5, 0.5/10, 1.75/7.5, 3.5/4. T/Constants.xaml:41-90.) Peak alpha on the centre line is the sum, about 0.40; it falls off in ten steps to nothing 15.5 either side. The glow therefore reaches 21.5 outside the tile edge and 9.5 inside it (authored), over the tile's art.

Rectangular variant only: four corner spots, each an 18 x 18 square filled with a radial gradient (`#2CA243` at the centre to transparent at the edge) at brush opacity 0.075, centred 3 inside the tile's corner on both axes (T/Constants.xaml:36-39, 91-98).

The source says this stack reproduces "a band 11 wide centred on the ring, at half opacity, blurred by 17" to within 7/255 (T/Constants.xaml:11-25): a port may draw that blurred band in a shader instead.

Round variant (`XboxFocusGlowRound`, T/Constants.xaml:110-173): the same ten bands with `CornerRadius="999"`, no corner spots; in the top bar the glow control's margin is -7 rather than -7.5 (T/Views/Main.xaml:107, 204), so the glow control reaches 4 outside the 3-thick ring's outer edge and the bands' centre line (5.5 inside the control's edge) falls 1.5 inside the ring's outer edge: exactly on the ring stroke's centre.

On Home tiles the whole glow scales by s with the tile's Viewbox.

---

## 5. Wide tiles

E/PromoRow.xaml (whole file), E/PromoRow.xaml.cs, hosted at E/RecentRowControl.xaml:71-73.

Four tiles in a grid of columns `*, 30, *, 30, *, 30, *` across 1526, under a 24 top margin, 190 tall.

- Authored: 359 x 190 each, 30 apart. On screen: **399.93 x 211.66**, gap 33.42, pitch 433.36; left edges x = 105, 538.36, 971.71, 1405.07; y 832.67..1044.34 at rest.
- Corner radius 6 (clip). Focus: the rectangular ring + glow only (section 4). **No growth, no label change, no title band.**
- Background: a diagonal linear gradient, top-left (0,0) to bottom-right (1,1). No images.
- Caption: bottom-left stack, `Margin="18,0,18,14"`. Title: `Segoe UI Semibold` 23.34 (26 on screen), white, one line, character ellipsis. Subtitle (tiles 3 and 4 only): `Segoe UI` 23.34, `#B3FFFFFF`, one line, ellipsis.
- Icon: centred in the tile, then raised 12 (`Margin="0,0,0,24"`), white.

| # | Tile | Gradient (0 -> 1) | Icon | Title / subtitle | A does | Lines |
|---|---|---|---|---|---|---|
| 1 | Discord | `#FF7983F5` -> `#FF4752C4` | Discord logo vector path, 48 x 48 uniform, white (path at line 71) | "Discord" | With a Discord tab and the Guide on: plays `OpenMenu.wav`, opens the Guide on the Discord tab. Otherwise: Settings > Theme settings > Discord, focus on Client ID. | 87-101; C/ExtensionCore.cs:1706-1726 |
| 2 | Playnite settings | `#FF5F6B7A` -> `#FF2B323C` | U+E713 (gear), icon font 40 | "Playnite settings" | Settings on the Playnite settings category | 104-117; C/ExtensionCore.cs:1729 |
| 3 | Achievements | `#FF3FA34D` -> `#FF0E5A1C` | U+1F3C6 (trophy) in **Segoe UI** (not the icon font, which has no trophy), 40. Segoe UI has none either: WPF draws it from **Segoe UI Symbol**, in the text's one colour, white (E/OverlayWindow.xaml:152 says so of the same glyph) | "Achievements"; subtitle "`{Score:N0} Score`" when there are totals; else "Needs PlayniteAchievements" or "Nothing recorded yet"; nothing until that is known | Same as the profile: Guide on Profile & system (`OpenMenu.wav`); nothing with the Guide off | 124-159; C/ExtensionCore.Achievements.cs:25-28, 363-367 |
| 4 | Downloads | `#FF3A8EE6` -> `#FF1A4C8F` | U+E896 (Download), icon font 40 | "Downloads"; subtitle "`{n} in queue`" or "Nothing queued" | My games & apps on Manage Queue; B there returns to this tile | 162-187; C/ExtensionCore.cs:1731-1736 |

Navigation (E/RecentRowControl.xaml.cs:200-226): Left/Right step along the four and stop at the ends; Up returns to the Recent-row tile that Down left (else the last focused game's tile, else the first); Down enters the Groups on the first Group's first card (only if Groups exist); Menu, X, B: nothing. Y: Search.

---

## 6. Groups

E/GroupsControl.xaml, E/GroupsControl.xaml.cs; C/ExtensionCore.cs:29-33, 1437-1485.

### 6.1 Which Groups

- One Group per Playnite filter preset flagged "show in Fullscreen quick selection", in Playnite's order, each sorted by the preset's own sort and direction (default by name).
- If none is flagged: a single Group named **"Favourites"** = favourite, not-hidden games by name ascending. This Group exists even when it is empty.
- Each Group shows its first **6** games, then a **See all** card. See all's collage = the next up to 4 games after those 6.

### 6.2 Header

`TextBlock`: **Bahnschrift** (regular), authored 28.73 = **32 px on screen**, `#FFFFFFFF`, one line, character ellipsis, 16 (17.82) above the cards, left-aligned at x = 105 (E/GroupsControl.xaml:62-65; E/HomeTiles.xaml:39,43).

### 6.3 Cards

- `GroupCard` = `HomeTile` style with Width 194, Height 296, right margin 28 (E/GroupsControl.xaml:25-31). On screen **216.12 x 329.75**, gap 31.19, pitch 247.31; card i left edge x = 105 + 247.31 i. Seven cards (6 games + See all) span exactly 7 x 194 + 6 x 28 = 1526 authored = x 105..1805.
- **Never grow.** Focus shows the ring + glow and the title band (same fades as section 3.3); the source badge hides; Running badge gains "CONTINUE". Same face as a Recent tile, the cover laid out at 194 x 296: `UniformToFill`, top-aligned, centred across.
- See all card: same size, right margin 0, title band "See all" on focus; face is the 2 x 2 collage (section 3.4), or with nothing left to show the glyph U+E8F1 (Library), icon font 40, white.
- A Group with fewer than 6 games is shorter; See all follows the last game directly. An empty Group is its header and a See all card alone.

### 6.4 Horizontal behaviour

**There is no horizontal scrolling.** A Group is at most 7 cards, which fit the width exactly. Left/Right move along the row and stop at the ends (E/GroupsControl.xaml.cs:205-206, 232-245).

### 6.5 Buttons on a Group card

| Input | Game card | See all card | Source |
|---|---|---|---|
| A | As a Recent tile: Game hub after the 300 ms double-tap window, or play on a double tap | Opens My games & apps on Full library with this Group's preset tab selected (Favourites has no preset: plain Full library) | E/GroupsControl.xaml.cs:302-321; C/ExtensionCore.FullLibrary.cs:22-35 |
| Menu | Tile menu (`PromptOpen.wav`) | Nothing | C/ExtensionCore.cs:840-843 |
| Y | Search | Search | as section 3.5 |
| B | Back to the Recent row, on the game it last had focus | Same | E/GroupsControl.xaml.cs:216-229 |
| Up / Down | Same column index in the Group above/below, clamped to that Group's last card; Up from Group 0 goes to the wide tiles (the wide tile last focused); Down from the last Group does nothing | | E/GroupsControl.xaml.cs:194-214, 232-245 |

---

## 7. Scrolling, rest positions and navigation

### 7.1 Model

Home is one page. A single vertical translate (`HomePage.Scroll`) moves the Recent row, wide tiles and Groups together; a second translate (`TopBarScroll`) moves the top bar. Both are set from where focus is (E/HomeRows.cs:18-186; C/HomeRests.cs). The hero, scrim and hint bar never scroll.

Inputs measured from the layout at page offset 0 (C/HomeRests.cs:56-75): `ScreenHeight` (1080), `GroupTops[i]`, `CardsTop` (cards' offset under a Group's top), `CardHeight`. Constants: `GroupTop = 64`, `TopBarBottom = 144` (C/HomeRests.cs:105-108).

    wide      = no Groups ? 0 : max(0, GroupTops[0] + CardsTop + CardHeight / 3 - ScreenHeight)

    RecentRow (or top bar):  PageOffset = 0,                    TopBarOffset = 0
    WideTiles:               PageOffset = wide,                 TopBarOffset = 0
    Group i:                 PageOffset = GroupTops[i] - 64,    TopBarOffset = max(PageOffset - wide, 144)

Applied as `Scroll.Y = -PageOffset`, `TopBarScroll.Y = -TopBarOffset` (E/HomeRows.cs:128-129). A Group index past the last laid out rests on the last (C/HomeRests.cs:114-117).

### 7.2 Exact offsets at 1920x1080 (derived from section 1.3; header line-height caveat applies, about +-1 px)

| Focus | PageOffset | TopBarOffset | What is on screen |
|---|---|---|---|
| Top bar or Recent row | 0 | 0 | As section 1.3. First Group starts at 1088.90, just below the foot. |
| Wide tiles | **175.05** (= 1088.90 + 56.23 + 109.92 - 1080) | 0 | Top bar unmoved (84..144). Recent row box top 357.95, unfocused tiles 460.44..630.89. Wide tiles 657.62..869.29. Group 0 header at 913.85; its cards from 970.08, the top third (109.92) showing. |
| Group 0 | **1024.90** | 849.85 | Top bar far above the screen. Group 0 header at y 64, cards 120.23..449.98. Group 1 header 494.54, cards 550.77..880.52 (whole). Group 2 header 925.08, cards from 981.31 (cut by the foot). |
| Group i | 1024.90 + 430.54 i | 849.85 + 430.54 i | Same picture, one Group further down. |

### 7.3 Animation

- Slide: 300 ms, `CubicEase EaseOut`, from wherever the transform currently is (to-only animation, so interruptions continue smoothly). Reduce motion: duration 0 (jump). E/HomeRows.cs:21, 176-185.
- The Recent row + wide tiles (as one element) **fade** to opacity 0 over the same 300 ms `CubicEase EaseOut` when focus goes to any Group, and back to 1 when it leaves the Groups. This fade is not shortened by Reduce motion (E/HomeRows.cs:127, 151-160; C/HomeRests.cs:126-131). Reason: on Group 0 the wide tiles' foot would otherwise show over the header.
- The top bar does not fade; it just translates off the top.
- Focusability: what is coming on screen is enabled when the slide starts; what has left (top bar when TopBarOffset >= 144; Recent row + wide tiles on a Group; Groups when Group 0's top is at or below the foot) is disabled when the slide ends (E/HomeRows.cs:110-125; C/HomeRests.cs:119-124).
- A re-measure that moves a rest by less than 0.5 px does not restart the slide (E/HomeRows.cs:24, 101-104).

### 7.4 Navigation summary and focus memory

- Vertical order: top bar <-> Recent row <-> wide tiles <-> Group 0 <-> Group 1 ...
- Only a game's tile sets `lastFocusedGame` (E/RecentRowControl.xaml.cs `OnTileFocused`, which the Play history tile does not have): B on a Group after the Play history tile had focus goes to the game's tile focused before it. The tile Down left (`leftForPromos`) is whichever tile it was, the Play history tile included.
- Between Groups the place kept is the focused card's index now (E/GroupsControl.xaml.cs:201-209): after a shorter Group has clamped it, Up or Down carries the clamped place on, not the one before.
- Recent row remembers the last focused **game id** (`lastFocusedGame`); wide tiles remember the last focused **index** (`lastPromo`) and the Recent tile that Down left (`leftForPromos`); Groups remember the last Group index, game id and whether it was See all (E/RecentRowControl.xaml.cs:31-44; E/GroupsControl.xaml.cs:35-39).
- Down from the Recent row: nearest wide tile by centre x (not the remembered one). Up from the wide tiles: the remembered Recent tile.
- Down from the wide tiles: always Group 0, first card (memory reset; E/GroupsControl.xaml.cs:124-130). Up from Group 0: the wide tile last focused.
- B on any Group card: Recent row, last focused game's tile (page slides back to offset 0). B elsewhere on Home: nothing.
- Returning to Home from a Game hub, My games & apps, Settings or Search: focus goes back to whichever of the Recent row / wide tiles / Groups had it, at the remembered place, and the page is still at that rest (E/RecentRowControl.xaml.cs:249-265; E/GroupsControl.xaml.cs:136-151).
- A game exiting: Home comes to the front, the page rests on the Recent row, focus on that game's tile (C/ExtensionCore.cs:637-670; E/RecentRowControl.xaml.cs:159-167).
- First focus on Home: the code asks for the last focused game's tile, else the first tile, else the Play history tile (E/RecentRowControl.xaml.cs:333-346). What happens after Start-up is otherwise: **Home starts with focus on the profile**, the window's first focusable element, with the page at rest on the Recent row and no tile grown. The pictures show it (`0-home.jpg`, `home.jpg`) and the Bundle's own test expects it ("Down from the profile, where Home starts, onto the Recent row", e2e/payloads/seeded-home/run.ps1:8, 68). Down from there lands on the row's first tile.
- D-pad repeat on Home is Playnite's own, every 80 ms when held (stated at C/ExtensionCore.cs:45-46; the repeat itself is Playnite's, UNVERIFIED).

---

## 8. Background

### 8.1 What shows

C/ExtensionCore.cs:38-54, 1245-1325, 1957-1971; C/HomeState.cs:65-73.

- Focus on a **Recent-row game tile**: that game's art.
- Focus anywhere else (top bar, Play history tile, wide tiles, any Group card, See all): the **green swirl**.

Art choice rule (`HeroArtOf`, C/ExtensionCore.cs:1958-1971):

1. The game's **background image** if it has one: shown as it is.
2. Else its **cover**: shown blurred and darkened.
3. Else none: the hero is **left as it was** (the previous game's art, or the swirl); it is not blanked (C/ExtensionCore.cs:1305-1319).

### 8.2 Delay before it changes

- Entering the row from the swirl (no art up): the focused game's art starts **at once**, no delay (C/ExtensionCore.cs:1261-1267).
- Moving along the row with art already up: the art changes after focus has rested on a tile for **1.5 s** (Background delay on, the default) or **150 ms** (off); every focus move restarts the timer; the old art stays up meanwhile (C/ExtensionCore.cs:42, 48, 1269-1273). The setting is read at the moment of each focus change. Which of the two it is depends on the setting alone, never on how fast focus is moving: the 150 ms is there so that a held direction, repeating every 80 ms, passes over tiles without their art being read (C/ExtensionCore.cs:44-48).
- "Art already up" means a game's art has been asked for (`heroGame`), whether or not it has faded in yet. A game with no art at all asks for nothing and does not count: moving from it onto a game with art, over the swirl, still shows that art at once.
- Leaving a Recent tile for something that is not a Recent game tile: the swirl request is made **100 ms** after the tile loses focus, cancelled if another Recent tile gets focus in that time (C/ExtensionCore.cs:54, 1280-1296). Focusing a Group card requests the swirl immediately (C/ExtensionCore.cs:1255-1259).
- Art is read only when its wait is over, not while focus rests: the crossfade starts when the wait and the decoding are both done. Art from a file is decoded off the UI thread; if focus has moved on by the time it is decoded it is dropped (E/HeroControl.cs:82-109).
- The first background Home shows is put up with no fade (`ShowAtOnce`, E/HeroControl.cs:54, 67-74). In practice that is the swirl: Home starts with focus on the profile.

### 8.3 Art layers and crossfade

E/HeroControl.cs, E/HeroLayer.cs.

- Two full-screen art layers over the swirl; each clips to bounds.
- Background art: decoded at 1920 wide, `UniformToFill`, **horizontally centred, top-aligned**, high-quality scaling, no extra darkening of its own.
- Cover: decoded at **120 px wide**, Gaussian-blurred once with radius **15** px (120 / 8) at that size, then scaled `UniformToFill`, centred/top-aligned, into a box that overhangs the screen by **120 on every side** (2160 x 1320, so the blur's soft edges are off screen); over it a flat **`#99000000`** rectangle (E/HeroLayer.cs:25-31, 94-95, 124-134). The rectangle is in the layer with the cover, so the two fade as one picture: mid-fade the layer is the cover at 40% of its light, at the layer's opacity, not a cover and a sheet each faded on its own.
- Crossfade: the new art's layer goes on top and fades to opacity 1 while the old layer fades to 0, both **linear, 400 ms**; **150 ms** with Reduce motion (still a fade). To the swirl: both layers fade to 0 the same way. Animations are to-only, so an interrupted fade continues from its current opacity (E/HeroControl.cs:24-27, 111-141).
- There are only the two layers, and they swap places at each crossfade: the one that was at the back (usually faded out) takes the new art and goes to the front. A crossfade that starts before the last has ended therefore gives the new art to a layer that may still be part way out: the picture it was showing is replaced at that opacity, and the layer fades up from there over the full 400 ms.
- The swirl is made visible under the layers for the whole fade and hidden again (so it stops animating) once art fully covers it.

### 8.4 Scrim (always on, over art and swirl)

`XboxHeroScrimBrush`, vertical linear gradient top to bottom over the full 1080 (T/Constants.xaml:245-251):

| Offset | y | Colour |
|---|---|---|
| 0 | 0 | `#99000000` |
| 0.22 | 237.6 | `#00000000` |
| 0.4 | 432 | `#00000000` |
| 1 | 1080 | `#D9000000` |

It does not scroll and does not change with focus location (no extra dim or blur when down in the Groups: NOT FOUND).

### 8.5 The green swirl

**Drawn as vector XAML: no image, video or shader asset exists for it.** E/SwirlBackground.xaml, E/SwirlBackground.xaml.cs. Authored on a 1920 x 1080 canvas, `UniformToFill`, clipped.

1. Base fill: radial gradient, centre (0.5, 0.5), RadiusX = RadiusY = 0.75 (relative to the canvas, so an ellipse 1440 x 810 semi-axes): `#FF1E2C1C` @0 -> `#FF0C140B` @1 (lines 59-64).
2. A group scaled 1.2 about the canvas centre, then rotated by `SwirlTurn` about the centre (lines 65-71), holding two band sets, each translated by its own drift. The drift is inside the group: on screen it is 1.2 times as far and turned with the whole. A positive angle turns clockwise. All paths are open cubic Bezier strokes with no fill; each stroke's brush is a linear gradient from the top-left to the bottom-right of that path's bounding box.

   The bounding box is that of the curve itself, not of its stroke (WPF maps a pen's brush to the geometry's bounds), and it is the curve's tight box, not its control points': for these six paths, the box from the first point to the last. The brush is stretched with the box, so a stop's colour lies along a line parallel to the box's **other** diagonal, not square to the gradient's own line: at a point (x, y) the gradient is at `((x - left) / width + (y - top) / height) / 2`. Colours are mixed as sRGB values (WPF's default), and a path's `Opacity` multiplies its alpha before it is laid over the paths under it in the set's bitmap.

Top-right set (`TopDrift`), lines 74-109:

| Path data | Stroke | Opacity | Gradient |
|---|---|---|---|
| `M 520,-260 C 600,140 920,250 1360,250 C 1760,250 1960,420 2200,720` | 300 | 0.9 | `#FF14581A` @0 -> `#FF2E8B22` @1 |
| `M 600,-260 C 680,90 980,180 1380,180 C 1760,180 1980,330 2240,620` | 150 | 1 | `#FF3E9A24` @0, `#FF8FD13A` @0.6, `#FF5CB02A` @1 |
| `M 650,-260 C 730,60 1020,130 1400,130 C 1770,130 2000,270 2260,560` | 46 | 0.85 | `#FF9BE03A` @0 -> `#FFC6F25A` @1 |

Bottom-left set (`BottomDrift`), lines 112-147:

| Path data | Stroke | Opacity | Gradient |
|---|---|---|---|
| `M -300,520 C 60,640 160,880 560,940 C 900,990 1060,1060 1200,1400` | 320 | 0.9 | `#FF2E8B22` @0 -> `#FF14581A` @1 |
| `M -300,600 C 40,720 140,960 540,1020 C 860,1070 1000,1140 1120,1460` | 160 | 1 | `#FF5CB02A` @0, `#FFA6E040` @0.4, `#FF3E9A24` @1 |
| `M -300,660 C 20,780 120,1020 520,1080 C 830,1130 960,1200 1060,1500` | 50 | 0.85 | `#FFC6F25A` @0 -> `#FF9BE03A` @1 |

Stroke caps and joins are WPF defaults (flat caps, miter joins). Each set is rendered once to a bitmap (`BitmapCache RenderAtScale="1"`: one pixel to a canvas unit, whatever the screen) and then only transformed: at 1080p the bitmap is drawn 1.2 times its size, at 4K 2.4 times, smoothed. The top set is drawn first, the bottom set over it.

3. On top, not rotated: a centre darkening, radial gradient centre (0.5, 0.5), RadiusX 0.45, RadiusY 0.55: `#88101810` @0 -> `#00101810` @1 (lines 150-158).

Motion (lines 18-49): five endless back-and-forth animations, each `SineEase EaseInOut`, auto-reversing, at 30 fps. Durations are one way; a full cycle is double. Eased both ways, each is a cosine: t seconds into the movement a value is at `from + (to - from) * (1 - cos(pi * t / duration)) / 2`. All five start at their From together.

| Target | From -> To | One-way duration |
|---|---|---|
| Rotation (degrees) | -5 -> 5 | 22 s |
| Top set X | -60 -> 60 | 13 s |
| Top set Y | 30 -> -30 | 17 s |
| Bottom set X | 50 -> -50 | 15 s |
| Bottom set Y | -40 -> 40 | 19 s |

It moves only while visible, loaded, and its window is the active window; otherwise it is paused where it is (E/SwirlBackground.xaml.cs:85-121). Its clock is its own: time spent hidden under a game's art does not count, and it comes back where it left off. Until it first moves it stands as authored, at angle 0 with no drift, which is not where the movement starts (-5 degrees, the sets at their From).

The 30 fps is on purpose (lines 13-17): nothing in it moves faster than some tens of pixels a second, and each of its frames draws all of Home again. The fastest point is a screen corner as the turn and a drift pass their middles together: about 14 px/s from the turn and 17 px/s from the top set's drift at 1080p, a pixel a frame.

Checked against the Bundle's picture: `see-all-20261110-113608/0-home.jpg` is this drawing 7.1 s into its movement. The Dashboard's swirl (`crates/screens/src/home/swirl.rs`) at that moment, compared with the picture over the million pixels away from the top bar and the tiles, differs by 0.7 of a level in 255 on average; 0.4% of pixels differ by more than 8 levels, along the bands' edges, and none by more than 32.

### 8.6 Reduce motion on Home (Settings; off by default, C/StartupState.cs:191)

| Thing | Normal | Reduce motion |
|---|---|---|
| Swirl | Turns and drifts, redrawn 30 times a second | Stands still (paused where it is; at angle 0 / no drift if it never ran) and asks for no frames |
| Hero crossfade | 400 ms | 150 ms |
| Recent tile growth | Exponential, 50 ms time constant | Full size in one frame |
| Page slide | 300 ms cubic ease-out | Instant |
| Recent row / wide tiles fade on entering Groups | 300 ms | 300 ms (unchanged) |
| Top bar icon scale to 1.2 | 120 ms | No scale |
| Gamertag / real name swap | 15 s loop | Gamertag stays |
| Reveal after Start-up (section 10.2) | Fly up + top bar drop | Everything in place at once |
| Title band fades, Spinner | As specified | Unchanged |

Sources: E/Motion.cs; E/HeroControl.cs:129; E/TileGrowth.cs:79; E/HomeRows.cs:178, 256-259; T/Views/Main.xaml:130-134, 255-260.

---

## 9. Sounds

All wav files are stereo 44.1 kHz PCM. Two sets exist:

- Theme (Playnite plays these itself): `T/audio/navigation.wav` (0.38 s), `T/audio/activation.wav` (0.70 s).
- Extension: `E/Sounds/`: `Navigate1.wav` (0.38 s), `Navigate2.wav` (0.38), `Navigate3.wav` (0.37), `Navigate4.wav` (0.38), `Enter.wav` (0.70), `OpenMenu.wav` (0.74), `PromptOpen.wav` (0.91), `PromptClose.wav` (0.87), `Login.wav` (1.46), `PowerOff.wav` (1.33), `Notification.wav` (1.22), `RareAchievement.wav` (10.53). Mapping at E/PlayniteSounds.cs:23-34.
- `navigation.wav` is byte-identical to `Navigate1.wav`, and `activation.wav` to `Enter.wav` (checked with `cmp`).

**Volume:** there are no per-sound volumes. Every sound plays at Playnite's Fullscreen "interface volume" setting, read at each play; at volume 0 nothing is played (E/PlayniteSounds.cs:130-158).

Events on Home:

| Event | Sound | Played by | Source |
|---|---|---|---|
| Focus moves onto any Home button (top bar button, Recent tile, Play history, wide tile, Group card, See all) | `navigation.wav` (always the same file: **no rotation on Home**) | Playnite's own focus sound for buttons and lists; the extension does **not** call `Play(Navigate)` anywhere in Home's code | E/PlayniteSounds.cs:69-75; E/GameHubFocusSounds.cs:9-13. Exactly which focus changes Playnite sounds (e.g. programmatic refocus after a rebuild) is Playnite's behaviour: **UNVERIFIED** |
| A activates a button | `activation.wav`, by Playnite, on a button activation | Playnite | Same. Whether the pad's A on a game tile triggers it is **UNVERIFIED**: the pad's A reaches the core as a controller button and `ChooseHomeTile` plays nothing (C/ExtensionCore.cs:850-854, 1331-1362) |
| Menu on a game tile (Tile menu opens) | `PromptOpen.wav` | Extension | C/ExtensionCore.TileMenu.cs:59-71 |
| B closes the Tile menu / a prompt | `PromptClose.wav` | Extension | C/ExtensionCore.cs:803-807 |
| Moving within the Tile menu | `Navigate1..4.wav` in rotation (1, 2, 3, 4, 1 ...) | Extension | E/PlayniteSounds.cs:25, 121-128; C/ExtensionCore.cs:2184-2277 |
| Profile button, Achievements tile, Discord tile (Guide opens) | `OpenMenu.wav` | Extension | C/ExtensionCore.cs:940, 1723 |
| Page scroll / rest change, hero change, tile growth | none | - | NOT FOUND |
| Boundary bump (Left at the first tile, etc.) | none | - | NOT FOUND |

During Start-up Playnite's two theme sounds are muted (volume 0) so Home's first focus behind the Start-up window is silent, and restored when Start-up's window has faded out (E/PlayniteSounds.cs:69-101; E/XboxSeriesPlugin.cs:540-570).

---

## 10. Other things a port would get wrong

### 10.1 Spinner (top bar, while the library updates)

T/Media.xaml:15-367 (keyframes sampled from C/Spinner.cs). 26 x 26 on Home's top bar, 12 px right of its status text and 20 px left of whatever follows (T/Views/Main.xaml:579-590). Drawn on a 100-unit canvas: ring radius 46, stroke 8 (about 2.1 px at 26), **round caps**, no track ring. Head arc white, tail arc `#FF2CA243`. Period **1.2 s**, repeating; it rests as a 6 degree dot at the top between turns.

Angles in degrees clockwise from the top, t = seconds into the 1.2 s turn (C/Spinner.cs:60-110):

    p(t, a, b)   = clamp((t - a) / (b - a), 0, 1)
    head         = 6 + 360 * bez(p(t, 0.00, 0.80); 0.70, 0.20, 0.25, 0.95)
    whiteTail    = min(360 * bez(p(t, 0.15, 0.85); 0.75, 0.20, 0.15, 0.60), head - 6)
    greenTail    = min(360 * bez(p(t, 0.25, 0.90); 0.70, 0.10, 0.15, 0.75), whiteTail)

where `bez(x; x1, y1, x2, y2)` is CSS `cubic-bezier(x1, y1, x2, y2)` evaluated at x. White arc = whiteTail..head; green arc = greenTail..whiteTail. Unaffected by Reduce motion. The same indicator is repeated at `Margin="0,84,105,0"` top right when another page hides the top bar (T/Views/Main.xaml:405-441).

### 10.2 Reveal after Start-up

E/HomeRows.cs:199-286; trigger at E/RecentRowControl.xaml.cs:175-181. Only when the page rests above the Groups and no Game hub is up. Units are 1:1 px.

- Recent row + wide tiles (as one): translate Y starts at **+90**, eases to **-12** at 400 ms (`CubicEase EaseOut`), then to 0 at 650 ms (`SineEase EaseInOut`). Opacity 0 -> 1 over 300 ms (`QuadraticEase EaseOut`). The Groups fade in over the same 300 ms.
- Top bar's centre button stack (My games & apps, Search, Settings only): held at translate Y **-150**, opacity 0 and disabled until 900 ms (650 + 250); then drops to 0 over 400 ms (`CubicEase EaseOut`) and fades in over 300 ms (`QuadraticEase EaseOut`). Skipped if focus is already in it. The profile, status area and Notifications button do not animate.
- Reduce motion: none of it.

### 10.3 Loading, empty and error states

- **Covers have no fade-in and no placeholder shimmer.** A tile is `#FF1A1A1A` until its cover is decoded (synchronously, at 300 px wide; 150 for collage cells), then the cover is simply there (E/HomeTiles.xaml:67, 162; E/DecodedImageConverter.cs).
- No cover, or a cover that fails to decode: the title text on the dark tile (section 3.3).
- A hero cover still downloading from a URL shows nothing until it arrives (E/HeroLayer.cs:104-122).
- Empty library: the Recent row is just the Play history tile (showing its History glyph); with Show Play history off and no games the row is empty and nothing in it takes focus. The wide tiles always show. The "Favourites" Group always shows, possibly as a header and a lone See all card with the Library glyph. There is **no "no games" message on Home** with the extension (the `LOCNoGamesFound` text at T/Views/Main.xaml:704-717 belongs to the theme-only list).
- While Playnite updates the library the rows are rebuilt only when their content actually changes; focus is put back on the same game id after a rebuild (E/RecentRowControl.xaml.cs:136-150).
- Rows fewer than full: left-aligned, not centred or stretched; Recent tiles keep 153 and gaps 30 (+ the unfocused widening shared over n - 1 gaps).

### 10.4 Hint bar

E/HintBarControl.xaml:17-60; C/ExtensionCore.Hints.cs:65-91. Bottom-right, `Margin="0,0,60,52"`: right edge x 1860, bottom y 1028, height 63, padding 16 left / 20 right, radius 6, background **`#D9030303`**. Hints 26 apart; each a 36 x 36 ring then a label Segoe UI 26 white (`Margin="12,0,0,6"`).

- Menu hint: white 2 px ring with glyph U+E700 (three lines), icon font 17; label "More options".
- Y hint: ring and letter "Y" (Segoe UI 19) both `#FFFFB900`; label "Search".
- On Home it shows **only while a Group's game card has focus** ("More options", then "Search"). It is absent on the top bar, the Recent row, the Play history tile, the wide tiles and See all, even though Menu works on Recent tiles and Y works everywhere. Shown/hidden instantly, no fade. It does not scroll with the page.

### 10.5 Text rules

| Text | Font | Size on screen | Colour | Wrap / trim |
|---|---|---|---|---|
| Tile title (band) | Segoe UI Regular | 26 (authored 23.34) | `#FFFFFF` | Wrap, max 2 lines (MaxHeight 64 authored), character ellipsis |
| Tile status ("Queued", "Installing...") | Segoe UI Regular | 26 | `#FFFFFF` | 1 line, character ellipsis |
| No-cover title | Segoe UI Regular | 26 | `#FFFFFF` | Wrap, centred, no trim, margin 10 |
| "CONTINUE" | Segoe UI Semibold, bold | 13.4 (authored 12) | `#FFFFFF` | - |
| Group header | Bahnschrift Regular | 32 (authored 28.73) | `#FFFFFF` | 1 line, character ellipsis |
| Wide tile title | Segoe UI Semibold | 26 | `#FFFFFF` | 1 line, character ellipsis |
| Wide tile subtitle | Segoe UI Regular | 26 | `#B3FFFFFF` | 1 line, character ellipsis |
| Top bar labels, gamertag, score, battery percent | Segoe UI Regular | 26 | `#FFFFFF` | none |
| Progress status text | Segoe UI Regular | 26 | `#B3FFFFFF` | none |
| Clock | Bahnschrift SemiBold, tabular figures | 22 | `#FFFFFF` | none |
| Icons | `Segoe Fluent Icons`, fallback `Segoe MDL2 Assets` | 26 top bar, 40 on tiles | `#FFFFFF` | - |

Font resources: T/Constants.xaml:228-235. 26 px body text is deliberate (Xbox Accessibility Guideline 101's floor; E/HomeTiles.xaml:35-36).

### 10.6 Small traps

- The focus ring colour is `#2CA243`, not the brand `#107C10`; `#107C10` appears on Home only as the avatar placeholder disc.
- Scale: tile corner radii, ring thickness, glow, badges and tile fonts are all x 1.114; top bar and hint bar values are not.
- Recent tiles anchor bottom-left when growing; the row is 272.94 tall always, so nothing below it moves.
- The hero image is **top-aligned**, not centred vertically, and so are covers on tiles.
- The scrim's bottom is `#D9` black (85%), which is what makes tiles readable; it is fixed to the screen.
- The swirl is under the same scrim, so its on-screen look is darker at top and bottom than the raw colours.
- With focus on a Group the background is the swirl even though the Recent row has faded out.
- A on a tile is delayed 300 ms before the Game hub opens (double-tap detection).
- `IsRunning` tile is always first in the Recent row.
- The console's own reference notes (docs/research/xbox-dashboard-reference.md:142-172) record that focus ring metrics, motion timings, type sizes and the sound catalogue are **not publicly documented**; the Bundle's values come from its own console captures, and the only reference image of Home in the repo is docs/reference/startup/4-home.jpg (no notes file accompanies docs/reference/).

---

## 11. Where the Dashboard's Home differs, for now

What `crates/screens/src/home*` does that the sections above do not say, or does differently until the pages it needs exist.

- **Top bar, Up and Down.** The Bundle leaves these to WPF's directional navigation (section 2.5, UNVERIFIED). The Dashboard's rule: Up from a Recent tile goes to the top bar button whose centre x is nearest the focused tile's; Down from a button goes to the Recent tile whose centre x is nearest the button's, with the row laid out as it is with no tile focused.
- **A on a game** is as sections 3 and 6 have it: the game's Game hub opens when the double tap's 300 ms have passed with no second A, and a second A on the same tile within them plays the game with no hub ([game-hub.md](game-hub.md) 9.1 and 16.2). Menu within the 300 ms drops the pending hub, as does the Guide, a page or a launch taking the pad; moving focus does not. The first A plays the activation sound, and the second nothing more.
- **Home starts on the profile**, as the Bundle does in practice (section 7.4).
- **The profile's picture** is drawn: the pale disc and grey figure of Windows' default account picture, which is what the Bundle shows for an account with none. Real pictures come with Profiles.
- **The source badge** is the launcher's mark, the project's own drawing (section 3.3.1), on the theme's shadow, for every game: the stores' logos are not ours to ship.
- **No Update button.** The green down arrow left of the Status area in the pictures is Playnite's own, shown while a Playnite update waits (section 2.3): the Dashboard has no updates to offer yet, and the Queue has no mark on the top bar in the Bundle either.
- **The pad's glyph** (U+E7FC, left of the clock) shows while a pad is connected, as the input crate reports it (`Dashboard::set_pad`). A wireless pad shows its battery in the glyph's place, as section 2.4 has it: the cell, and its charge left of it unless Settings' Show battery percentages is off.
- **Wide tiles.** "Settings" stands for "Playnite settings", and opens Settings on General's first box ([settings.md](settings.md) 16.3). Downloads is always there, as the Bundle's is, and counts the Queue ("`{n} in queue`", "Nothing queued"); A on it opens Manage Queue ([manage-queue.md](manage-queue.md)) over Home, B back to the tile. A tile whose game is in the Queue has the status band (section 3) unfocused and the state under its title in the focused title band; there is no progress on a tile, as there is none on the Bundle's, and no picture of the Bundle's shows either. Achievements says "`{Score:N0} Score`" while there are totals (#141), and "Nothing recorded yet", the Bundle's line for no achievements, until then; "Needs PlayniteAchievements" is never said, there being no such dependency here. Its trophy is drawn from Segoe UI Symbol (`render::Font::Symbols`), as WPF draws it. Discord shows the chat glyph, not Discord's logo.
- **A on the profile** opens the Guide on Profile & system, with the Guide's opening sound (section 2.1). A on Notifications does nothing: Playnite's notifications menu has no counterpart yet.
- **The ways into a page** (the page stack, [ui.md](../ui.md)). A on My games & apps, Search and Settings on the top bar, Y anywhere, A on the Play history tile, on See all and on the Settings and Downloads tiles open their pages, at once. Until M3's other tickets the pages are stand-ins that show their name and close on B. See all hands Full library the Group's saved filter, Favourites' too: here Favourites is a filter like any other, where the Bundle's has no preset.
- **The Discord tile** opens Settings, there being no Discord tab: on Account > Discord ([settings.md](settings.md) 16.3); with one (M6) and the Guide on it opens the Guide on that tab. **The Achievements tile** opens the Guide as the profile does, and nothing with the Guide off.
- **Where B on a page leaves focus** is as section 7.4 and the pages' own specs have it, the Bundle's accidents included, since they are what its users know: Home is at the rest it was left at; focus on a wide tile, a Group's card or See all is there still; focus on the Recent row or the top bar is on the Recent row's game last focused, so not on the My games & apps button and not on the Play history tile; but Search and Settings opened with A on their top bar button give focus back to the button ([search.md](search.md) 4.2, [settings.md](settings.md) 8.2). A page the Guide opened leaves focus on Home's rows the same way.
- **The Guide's My games & apps, Settings and Search** close the Guide onto their page, which takes the place of any page that is up; the Guide's Home closes it. Search opens as A goes down, where the Bundle waits for A's release so that the press is not also the keyboard's first key: here a press is given to one screen only. With a game running the three do nothing, as Home does not, until the shell can bring Home in front of a game (#40).
- **The hint bar** (section 10.4) shows on a Group's game card, "More options" and "Search", and nowhere else on Home. Menu does nothing yet: the Tile menu is its own ticket. On a screen of another shape the bar keeps its 60 and 52 from the screen's own corner. It is gone at once under the Guide.
- **Sounds for the pages.** A that opens a page plays the activation sound, as A on a tile does; Y and B are silent. The Bundle plays nothing of its own for either, and what Playnite plays is unverified ([my-games.md](my-games.md) 10, [search.md](search.md) 6).
- **Groups** are the library's saved filters that are shown and have a game, in the user's order ([filters.md](filters.md)), in place of Playnite's flagged presets (section 6.1): on a first run Favourites, Steam's collections, Most played and Recently added. A Group with no games is left out rather than shown empty, where the Bundle shows it; with no Group left at all Favourites shows empty, as the Bundle's fallback does (sections 6.1 and 10.3). There is no page yet that shows, hides, orders or makes one (M3's Groups in Settings).
- **A hidden game's card.** The Hidden Group, where the user has shown it, has the hidden games' cards. A on one opens its Game hub; a double tap plays nothing, and nor does the hub's Play: a launch names a game by its place among the library's games, and a hidden game has none.
- **Recent row.** Seven games and the Play history tile, or eight games with Settings' Show Play history off ([settings.md](settings.md) 16.5); a library with no game keeps the tile. Never-played games pad it in the library's order, by name: there is no date added.
- **Focused top bar glyphs.** Settings swaps to U+F8B0. My games & apps and Notifications keep their outline glyph (the Bundle's filled ones are vector paths) and Search is not emboldened.
- **The Score line** (#141) is drawn as section 2.1 has it while there are totals: the gamertag and the Score line are centred together on the profile's middle, each line 34.58 tall; with no totals the gamertag alone is centred, as before.
- **Not drawn yet:** the Running badge, the install status band, the name swap, the PC's battery and the Discord mark in the status area.
- **The background.** As section 8, with these differences. Background delay is Settings' option ([settings.md](settings.md) 16.5). Focus moves in one step here, so the 100 ms before the swirl comes back is kept only for the look of it. The swirl holds still under the Guide, as the Bundle's does whenever its window is not the active one; behind a game Home is not drawn at all. Its pictures are drawn on a decoding thread: the first frame or two of a Home that starts on the swirl show the dark green alone. The blurred cover is blurred by a Gaussian of deviation 7.5 with its edges held, where WPF's blur of radius 15 fades them to clear; what of that reaches the screen has not been compared with a picture of the Bundle.
- **Another screen shape.** The rests use the screen's own height in design units (1200 at 16:10), so the wide tiles' rest rises less on a taller screen.
