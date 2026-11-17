# Game hub specification (as the Bundle draws it)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run.

Path shorthand used in citations:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`
- `PN/` = `~/projects/playnite-src/source/` (Playnite's own source, a checkout on this machine, **not** part of the theme repository). It is cited only where the hub leans on Playnite's code and the theme repository cannot say what happens. The checkout's commit was not compared with the Playnite the Bundle targets (10.62): treat `PN/` lines as strong evidence, not as the Bundle's source.

All numbers are design units = px on a 1920x1080 canvas. **The Game hub is not inside any Viewbox of the theme's**: every number is 1:1, unlike Home's rows (home.md's `s` does not apply anywhere on this page). The main file is `T/Views/GameDetails.xaml`; a bare line number below (for example "line 508") means that file.

This spec describes the hub **with the extension installed**. With the theme alone the strips above and below the band show the game's art instead of the screen behind, there is no Queue action and no silent Install, and Menu and Manage open Playnite's own game menu (lines 370-373, 468-475, 559-572).

**Two line-height assumptions.** Several vertical positions depend on a font's line height, which WPF takes from the font file:

- Segoe UI: 1.33 em (CT/ConsoleTypeSizesXamlTests.cs:23, and the source's own sum at lines 44-48, which only works with 1.33). So a 26 px line is 34.58, a 20 px line 26.6, a 14 px line 18.62.
- Bahnschrift: the repository's tests assume 1.2 em in one place (CT/HomePageXamlTests.cs:59) and 1.25 em in another (CT/ConsoleTypeSizesXamlTests.cs:24). This spec uses **1.2**, as home.md does: the title line (47 px) is then 56.4 tall. With 1.25 it is 58.75 and **everything under the title moves down 2.35**. **UNVERIFIED** font metric; measure on Windows.

**Pictures of the real thing.** `artifacts/e2e/achievements-20261110-105905/` (and the same names in `achievements-20261110-102705/`): `data-hub.jpg` (hub open, focus on Play), `data-hub-up.jpg` (Up: focus on the cover card), `data-hub-up-a.jpg` (A on the card: nothing changed), `data-hub-closed.jpg` (B: Home again, focus back on the tile). `data-tile-opened.jpg` and `nodata-tile-opened.jpg` show the Guide, not the hub; `achievements-20261110-101405/hub*.jpg` are misnamed shots of the Guide and Home. No other folder has a hub shot, and the payloads `hub-panel`, `hub-queue` and `cover-fit` left no pictures. The pictures were taken with an older build (the run's `steps.txt` installs `XboxSeries_0.1.0.pext`): the header is still the old one (title and "Steam" on one line, smaller type), the cards' text is smaller, and the strips above and below the band show the game's art, not the screen behind (no Home tiles show in the bottom strip), so the backdrop snapshot is not in them. They were measured with a JPEG decoder for what did not change; section 13 lists what they confirmed.

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite or font internals that are not in the theme repository.

---

## 1. What the page is

The Dashboard's page for one game: a panel over the screen it was opened from. A band across the screen shows the game's art, darkened; above and below it the screen behind shows, blurred and darkened. Inside the band: a header (title, one line of facts), a row of cards about the game, and its actions (CONTEXT.md:50-52; lines 1-12).

The hub has **two pages that share the header and the actions**: *Game hub* (cover, Your game stats, About, Info) and *Game details* (cover, trailer, Description, Info). The last action, the page toggle, switches between them; only the cards change (lines 3-6; CT/GameHubHeaderXamlTests.cs:208-224).

Technically it is Playnite's Game details view restyled (`Style TargetType="{x:Type GameDetails}"`, line 358), an element that fills the window over the page (`PART_ElemGameDetails`, T/Views/Main.xaml:761-762) and is made visible or hidden by Playnite. Its data context is the selected game of Playnite's main list (line 7).

### 1.1 Layer order (back to front), each 1920x1080 unless noted

| # | Layer | Source |
|---|---|---|
| 1 | Hub root `PART_ViewHost`, background `MainBackgourndBrush` (the radial green of home.md section 1.1, row 1). Seen only for a game with neither background art nor cover. | lines 362-363; T/Constants.xaml:239-243 |
| 2 | The game's art, blurred, full screen (section 2.1) | lines 388-440 |
| 3 | A diagonal darkening gradient, full screen (section 2.2) | lines 442-466 |
| 4 | The screen behind, as two strips: y 0..88 and y 992..1080 (section 2.3) | lines 476-501 |
| 5 | The band: `Margin="0,88,0,88"`, so **x 0..1920, y 88..992 (904 tall)**, fill `#B3000000`, square corners, no border | line 508 |
| 6 | The content: header, cards, actions, in a grid with `Margin="80,128,80,0"` (section 3) | lines 510-1010 |
| 7 | Hint bar (section 10), over the hub | T/Views/Main.xaml:768 |
| 8 | Tile menu with its dim (tile-menu.md), over the hub and the hint bar | T/Views/Main.xaml:771 |
| 9 | Game status view (section 11), over all of those | T/Views/Main.xaml:773-774 |
| 10 | Text entry (Console keyboard), last | T/Views/Main.xaml:781 |

Nothing in layers 1 to 5 takes input (`IsHitTestVisible="False"`, lines 388, 442, 476, 508).

---

## 2. The backdrop

### 2.1 The game's art (behind everything, shows through the band)

Art choice (lines 380-386, 403-416, 425-434):

1. The game has **background art**: that, blurred.
2. Else it has a **cover**: the cover, blurred.
3. Else neither: nothing is drawn, the gradient of section 2.2 is hidden too, and the root's green radial background shows.

Both variants are blurred small and stretched: laid out on a **128 x 72** box (a fifteenth of 1920x1080), drawn into a bitmap of that size, blurred there, then scaled by a `Viewbox Stretch="UniformToFill"`, centred both ways, to fill the screen (x15 at 16:9) (lines 389-393, 435-438). The upscale itself smooths further (line 386).

| | Background art | Cover |
|---|---|---|
| Element | `FadeImage PART_ImageBackground`, 128 x 72, `Stretch="UniformToFill"`, `IsBlurEnabled="True" BlurAmount="2"` | `Image`, in a 128 x 72 grid clipped to bounds, `Stretch="UniformToFill"`, high-quality scaling |
| Blur | Gaussian, radius **2** at 128 x 72 = **30** in screen units (`BlurAmount` is the `BlurEffect` radius: PN/Playnite/Controls/FadeImage.xaml.cs:224-232) | Gaussian, `Radius="4"` at 128 x 72 = **60** in screen units, `RenderingBias="Performance"` |
| Overscan | none | scaled 1.15 about its centre "so the blur's soft edges fall off screen" |
| Alignment of the art in the box | centred (the FadeImage's inner alignment is Playnite's: **UNVERIFIED**) | centred both ways (WPF default; no alignment set) |
| When | background art is not empty | background art empty **and** cover not empty |
| Lines | 425-439 | 390-418 |

Note the difference from Home's hero (home.md 8.3): Home shows background art **sharp** and top-aligned; the hub **blurs it** and centres it.

The background art's source is Playnite's `DisplayBackgroundImageObject`. On a change of game with the hub open (LB/RB) Playnite waits 100 ms and crossfades old to new over 0.5 s, linear (PN/Playnite.FullscreenApp/Controls/Views/GameDetails.cs:167-174; PN/Playnite/Controls/FadeImage.xaml:20-41). The cover variant has no fade.

How WPF's `BlurEffect` radius maps to a Gaussian sigma is WPF's internal: **UNVERIFIED** (a port should match by eye against a capture; the usual reading is sigma = radius / 3).

### 2.2 The darkening gradient (over the art, under the strips and band)

A full-screen rectangle with a linear gradient from the top-left corner (0,0) to the bottom-right corner (1,1) (lines 442-453):

| Offset | Colour |
|---|---|
| 0 (top left) | `#CC000000` |
| 0.55 | `#66000000` |
| 1 (bottom right) | `#99000000` |

Collapsed when the game has neither background art nor cover (lines 455-463).

Inside the band the art is therefore darkened twice: by this gradient, then by the band's `#B3000000`. On the picture the band reads about `#211D1D` over dark red art (section 13).

### 2.3 The screen behind (the two strips)

C/HubBackdropState.cs:9-67; E/HubBackdrop.cs:12-125; lines 468-501.

- **When it is taken.** Once, as the hub becomes visible, before the hub is drawn over the page; kept while the hub is open; let go as the hub hides. Opening again while one is held keeps the one held (C/HubBackdropState.cs:45-54; E/HubBackdrop.cs:66-92). Followed from the hint bar's control (E/HintBarControl.xaml.cs:47-53).
- **What it shows.** Main's `PART_ViewHost`: the page with its background, hero, scrim, top bar and rows, or My games & apps, Settings or Search in their place, **as it was on screen at that moment**. Not the hint bar, the Tile menu or the hub itself (E/HubBackdrop.cs:13-17). Exactly the page's own bounds, so nothing that is flown off the page is included (E/HubBackdrop.cs:94-99). Whether the opening tile's focus ring is still in it is **UNVERIFIED** (the snapshot is taken while the tile still has focus in the code's order, but no picture shows it).
- **Size and blur.** An eighth of the page each way: `floor(1920 / 8) x floor(1080 / 8)` = **240 x 135** pixels. Drawn through a Gaussian blur of radius **2** in the snapshot's pixels, "about 16 of the screen's at 1080p" (E/HubBackdrop.cs:22-26, 100-124). Frozen: it does not update while the hub is open.
- **How it is drawn.** Two strips, each 88 tall: top `y 0..88`, bottom `y 992..1080`, full width, each clipping a full-screen copy of the snapshot stretched (`Stretch="Fill"`, high-quality scaling) to 1920x1080 and aligned to its own edge, so both line up with what was on screen. Over each: a flat **`#99000000`** (60% black) (lines 487-500).
- **Without a snapshot** (none could be taken, or the theme alone): the strips are collapsed and the game's art under its gradient shows there (lines 479-483).
- **Port.** Render the page under the hub once into a 240 x 135 target (or blur the previous frame) at the moment the hub opens, keep it until the hub closes, draw it in the two strips under `#99000000`.

`E/PanelBlur.cs` is not the hub's: it is the Guide's panel blur (radius 12, E/PanelBlur.cs:13-20) and nothing in the hub uses it.

---

## 3. Layout of the content

The content grid has `Margin="80,128,80,0"`: **x 80..1840, top y 128**. Four rows, all `Auto` (lines 510-516): title, facts, cards, actions. With the Bahnschrift line at 1.2 em:

| Row | Element | Authored | On screen (x, y) | Source |
|---|---|---|---|---|
| 0 | Title | one line, 47 px Bahnschrift: 56.4 tall | x 80.., **y 128..184.4** | lines 614-615 |
| 1 | Facts line | `Margin="0,6,0,0"`, `MinHeight="48"` (content is at most 34.6 tall, so the row is 6 + 48 = 54) | x 80.., **y 190.4..238.4**; text centred on y 214.4 | line 617 |
| 2 | Cards | grid `Margin="-80,8,-80,-24"`; inner panel `Margin="80,24,64,24"`; cards `Height="500"` | card tops **y 270.4**, bottoms **y 770.4** (238.4 + 8 + 24 = 270.4); first card x 80 | lines 777-782, 212 |
| 3 | Actions | `Margin="0,32,0,0"`, `Height="80"` | **y 802.4..882.4**, from x 80 | lines 523, 118 |
| - | Band's foot | | y 992: 109.6 under the actions | line 508 |

The 24 above and below the cards exists only so the focus ring's glow stays inside the clip a wider-than-screen row gets; the -24 bottom margin takes it back, so the actions are exactly 32 under the cards (lines 780-781).

The picture's cards are at y 277..776 and its actions at 808..888: 6 lower, because that build's header was a different height. The 32 gap, the 500 and the 80 are confirmed.

---

## 4. Header

Lines 605-768; CT/GameHubHeaderXamlTests.cs (whole file). Above the cards, so both pages show the same header.

### 4.1 Title

`DisplayName` (the game's name). **Bahnschrift Regular 47**, `#FFFFFFFF`, one line, no wrap, character ellipsis at the grid's width (1760) (lines 614-615; base style lines 32-38). Left edge x 80, top y 128.

There is **no logo, no icon and no cover in the header**: NOT FOUND. The cover is the first card.

### 4.2 The line of facts

A `DockPanel`, left-aligned, as wide as its content and at most 1760 (line 617). Two children:

1. **The facts**, one `TextBlock`: **Segoe UI Regular 20**, `#B3FFFFFF` (`XboxHubDimText`), no wrap, character ellipsis, vertically centred, `Margin="0,0,24,0"` (lines 634-637, 40-42).
2. **The age rating's box**, docked right of the facts: a border `BorderThickness="1"` `#80FFFFFF`, `CornerRadius="3"`, `Padding="10,3"`, no fill, vertically centred; inside, the ratings in **Segoe UI Regular 20, `#FFFFFFFF`** (lines 618-632). Height 1 + 3 + 26.6 + 3 + 1 = 34.6: y 197.1..231.7.

The rating is measured first, so it always keeps its whole width and the facts get what is left: a long line of facts ends in an ellipsis **before** the box; a short one has the box 24 after its last character (lines 608-612; CT/GameHubHeaderXamlTests.cs:142-162). The line never wraps.

The facts, in order, joined by **" • "** (space, U+2022 bullet, space) (CT/GameHubHeaderXamlTests.cs:17-23):

| # | Fact | Text | Blank when | Lines |
|---|---|---|---|---|
| 1 | Publisher | publishers joined by ", " | none | 652 |
| 2 | Release year | the year, e.g. "2023" | no release date | 668 |
| 3 | Install size | bytes as a size (format below) | the game is **not installed**, or the size is null or 0 | 685-700 |
| 4 | Platform | platforms joined by ", " | none | 718 |
| 5 | Source | the source's name, e.g. "Steam" | the game has no source | 737-746 |
| 6 | Features | features joined by **" • "** too | none | 766 |

Separator rule: the bullet before a fact shows only while that fact has text **and** some fact before it has text; a blank fact takes its bullet with it, so the line never starts with a bullet or doubles one (lines 653-667 and the like; CT/GameHubHeaderXamlTests.cs:59-91). In effect: join the non-blank facts with " • ".

Install size format (PN/Playnite/Converters/NullableUlongBytesSizeToStringConverter.cs:21-61): the value in the largest unit of B, KB, MB, GB, TB (1024-based) with two or three decimals, `"0.00# "` then the unit: "45.20 GB", "512.00 MB", "1.234 GB". **UNVERIFIED** against a capture.

Ratings text: the age ratings joined by ", " (line 630). The box is collapsed when that text is empty (lines 620-628). With no facts at all the facts block (and its 24 gap) is collapsed and the box starts the line at x 80 (lines 638-650; CT/GameHubHeaderXamlTests.cs:181-192). With neither, the line is empty but **keeps its 54 of height** (`MinHeight="48"`): the cards do not move up.

---

## 5. Cards

### 5.1 The card

Style `XboxHubCard`, lines 186-216:

| Property | Value |
|---|---|
| Size | each card's own width x **500** |
| Fill | **`#4D2A2A2A`**, `CornerRadius="4"` |
| Padding | 26 left and right, 22 top and bottom (inner height 456); 0 on the cover and trailer cards |
| Gap | `Margin="0,0,16,0"`: 16 between cards |
| Focus | the ring of section 7 while the card **or anything in it** has keyboard focus. Nothing else changes: no fill change, no growth, no label. |
| A | **does nothing** (line 6; the picture `data-hub-up-a.jpg` is identical to `data-hub-up.jpg`) |

Any card with nothing to show is collapsed and the cards after it close up to the left (line 771-772). The cover card always shows.

Card titles: **Segoe UI Bold 20**, white, `Margin="0,0,0,12"` (lines 50-54): 26.6 + 12 = 38.6 of the card's inner height. Labels and values: **Segoe UI Regular 26**, white, one line, character ellipsis; a value has `Margin="0,2,16,10"` (lines 56-63). One label/value pair is 34.58 + 2 + 34.58 + 10 = **81.16** tall.

### 5.2 Game hub page (the page the hub first shows)

Strip: 80 + (375 + 16) + (416 + 16) + (480 + 16) + (436 + 16) + 64 = **1915**, less than 1920, so this page never scrolls (section 5.4).

| # | Card | Width | x (all cards showing) | Lines |
|---|---|---|---|---|
| 1 | Cover | 375 | 80..455 | 789-820 |
| 2 | Your game stats | 416 | 471..887 | 825-928 |
| 3 | About | 480 | 903..1383 | 931-952 |
| 4 | Info | 436 | 1399..1835 | 955-956 |

**Cover card** (375 x 500, a 3:4 box). Padding 0, contents clipped to the card's rectangle. The cover fills it as every game tile shows art (`XboxTileArt`, T/Constants.xaml:192-218): under it `#FF1A1A1A`; the image `UniformToFill`, **centred across, top-aligned**, Fant scaling, both with corner radius **4** (the card overrides the tile's 6, lines 792-796). No bands. A game with no cover shows Playnite's default cover image (PN/.../Views/GameDetails.cs:149-165), not the title text Home's tiles use.

Badge on the cover (lines 797-818, 1013-1027): bottom-left, `Margin="10"`, `Padding="8,4"`, `CornerRadius="3"`, fill **`#CC1A1A1A`** (`XboxChipBrush`, T/Constants.xaml:236). Inside:

- Source "Steam" or "Xbox": the store's mark, a white vector path 20 x 20 (paths at lines 25-26; the same marks as Home's tiles, and as home.md 3.3 says, not ours to ship). The badge is then 36 x 28: **x 90..126, y 732.4..760.4**.
- Any other source: its name as text, Segoe UI Regular **14**, white, `MaxWidth="180"`, character ellipsis.
- No source: the platforms joined by ", ", same text style.
- No source and no platform: no badge.

Unlike Home's tiles the badge does **not** hide while the card has focus.

**Your game stats** (416 x 500; content x 497..861, 364 wide, from y 292.4). Title "Your game stats", then these rows top to bottom, each collapsed when it has nothing (lines 842-927):

| Row | Label | Value | Collapsed when |
|---|---|---|---|
| 1 | "Time played" | playtime as text: under a minute "N seconds", under an hour "N minutes", else "Hh Mm" (e.g. "10h 0m") (PN/Playnite/Converters/PlayTimeToStringConverter.cs:39-76; PN/Playnite/Localization/LocSource.xaml:792-796; the picture reads "10h 0m") | playtime is 0 |
| 2 | "Times played" | the play count | count is 0 |
| 3 | "Last played" | the date last played, in Playnite's date format; the picture reads "11/10/2026" for a game played that day, so not "Today" (PN/Playnite/Converters/DateTimeToLastPlayedConverter.cs:18-32; the format string itself is Playnite's setting: **UNVERIFIED**) | never played |
| 4 | "Completion" | the completion status's name | none set |
| 5 | "Achievements" (**M6**) | see below | no achievement data |

With rows 1 to 4 showing, row k (0-based) has its label at y 331 + 81.16 k and its value 36.58 under that (331 = 292.4 + 38.6).

Achievements row (lines 899-926), **M6**: label "Achievements" (26, white); under it (`Margin="0,2,0,10"`) "`{unlocked} / {total}`" at the left in white 26 and "`{percent:0}%`" at the right in `#B3FFFFFF` 26; then a progress bar **6 tall**, 364 wide: track `#33FFFFFF`, fill `#FF2CA243` from the left to percent of the width, both `CornerRadius="3"` (lines 218-233). The row is 34.58 + 2 + 34.58 + 10 + 6 = 87.16 tall. It is always the last row. **Placeholder state:** without achievement data the row is simply collapsed (lines 902-909) and nothing stands in for it: no "No achievements" text, no empty bar. M3 leaves this slot at the foot of the stats card and draws nothing in it.

All five rows fit: 38.6 + 4 x 81.16 + 87.16 = 450.4 of 456 (lines 44-48; CT/ConsoleTypeSizesXamlTests.cs:210-234).

The whole card is collapsed when all five rows are (a game never played, with no completion status and no achievements) (lines 826-840).

**About** (480 x 500; content x 929..1357, 428 wide). Title "About"; at the foot the features as pills (section 5.5); between them the **start of the description**, filling what is left, `Margin="0,0,0,12"`, clipped to its box, not scrollable, taking no focus (lines 941-951). Text: Segoe UI 26, **`#E6FFFFFF`**, links `#FF2CA243`. The description is HTML, drawn by Playnite's `HtmlTextView` through its `DescriptionView.html` template (the theme ships none, so Playnite's default; PN/.../Views/GameDetails.cs:176-188): how paragraphs, lists and images in it are laid out, and whether the clip cuts a line in half, is **UNVERIFIED**. No ellipsis and no "more" marker: the full text is on the other page. Collapsed when the game has no description (lines 933-938).

**Info** (436 x 500; content 384 wide). Title "Info", then a grid of **two equal columns** (192 each) filled row by row with these label/value pairs, each collapsed when empty (lines 255-339):

| Order | Label | Value | Collapsed when |
|---|---|---|---|
| 1 | "Genres" | genres joined by ", " | none |
| 2 | "Developer" | developers joined by ", " | none |
| 3 | "Series" | series joined by ", " | none |
| 4 | "Critic score" | the number | null |
| 5 | "Community score" | the number | null |
| 6 | "Your score" | the number | null |

A value is one line with an ellipsis in a 176-wide box (192 less its 16 right margin), so a list of genres is usually cut short. Collapsed pairs leave no hole: the pairs that show take the cells in order (WPF's `UniformGrid` skips collapsed children: **UNVERIFIED**, WPF's behaviour). Every grid row is 81.16 tall. The card is collapsed when all six are empty (lines 341-356).

**Recent Play sessions: NOT FOUND.** CONTEXT.md:184 says "the Game hub shows a game's most recent ones", but no card, row or binding for Play sessions exists in `GameDetails.xaml` or in the extension at `f11e4f9`, and the extension records none it could show (the only "Play session" in its code is a comment, C/ExtensionCore.LibraryUpdate.cs:184). "Times played" and "Last played" above are Playnite's own totals.

### 5.3 Game details page

Lines 959-1005. Strip: 80 + (375 + 16) + (889 + 16) + (480 + 16) + (436 + 16) + 64 = **2388**, wider than the screen (section 5.4).

| # | Card | Width | x at rest (strip left-aligned) | x with the strip's end lined up |
|---|---|---|---|---|
| 1 | Cover (the same card) | 375 | 80..455 | -388..-13 (off screen) |
| 2 | Trailer | 889 | 471..1360 | 3..892 |
| 3 | Description | 480 | 1376..1856 | 908..1388 |
| 4 | Info (as section 5.2) | 436 | 1872..2308 (48 showing) | 1404..1840 |

**Trailer** (889 x 500 = 16:9; lines 965-973). Padding 0, clipped to the card's rectangle (square corners over the card's rounded fill). Back to front: the cover, `UniformToFill`, centred; the background art, `UniformToFill`, centred, Fant scaling, covering the cover when there is one; then Extra Metadata Loader's video player, muted, without controls (another Playnite add-on; without it or without a video only the art shows). Focusable; A does nothing.

**Description** (480 x 500; lines 976-1000). Title "Description"; the features as pills at the foot; between them the **whole description** in a vertical scroller (`Margin="0,0,0,12"`, no scroll bar shown). Same text style as About. The card itself is not focusable; the scroller is, and the card's ring shows while the scroller has focus (lines 186-189, 202-204). Scrolling: section 8.3. Collapsed when there is no description.

### 5.4 Scrolling of the card row

Lines 770-782, 1048-1057. There is **no free horizontal scrolling and no animation**. The row has two positions:

- **Left**: the first card at x 80. The default.
- **Right**: the row's end 64 + 16 from the screen's right edge, i.e. the last card ending at x 1840. Taken while focus is on (or in) **Info on either page or Description on the details page**; left again as soon as focus is anywhere else.

The switch is a change of alignment, applied instantly. A row that fits the screen (1920 or less with its margins: the hub page always, the details page when Description or Info is collapsed and what is left fits) never moves, because the frame is never narrower than the screen (`MinWidth` bound to the screen's width, lines 778-779).

A row wider than the screen is clipped at the screen's left and right edges, and 24 above and below the cards (y 246.4..794.4).

Port: `offset = fits ? 0 : (focus in {Description, Info} ? 1920 - strip_width : 0)`, with `strip_width = 144 + sum(width + 16)` over the cards showing.

### 5.5 Feature pills (About and Description cards)

Lines 235-252. The game's features in a wrapping row, at the foot of the card: each a pill, fill **`#40FFFFFF`**, `CornerRadius="16"`, `Padding="14,4"`, `Margin="0,8,8,0"`, text Segoe UI Regular 26 white. A pill is 34.58 + 8 = 42.58 tall, so a row of pills takes 50.58 with its top margin. The block has `MaxHeight="80"` and clips.

Trap: the source's comment says "Two rows at most; the rest are cut, never half a pill", but at 26 px two rows need 101.16. By the arithmetic the first row shows whole and the **second row is cut 21.4 into its pills** (80 - 50.58 - 8). The 80 predates the 26 px type (#229). A port following the source reproduces the cut; see Open questions.

With no features the block is empty and takes no room.

---

## 6. Actions

Lines 518-603. A horizontal row at y 802.4..882.4 from x 80, in this order: **main action**, **Manage**, **Queue** (only while the game is in the Queue; M4), **page toggle**. Each is 80 tall with 12 after it.

### 6.1 Drawing

Style `XboxHubAction` and its template, lines 79-124:

| Part | Rest | Focused |
|---|---|---|
| Slab | **`#33FFFFFF`**, `CornerRadius="4"` | **`#FF2CA243`** (the ring's green), instantly |
| Ring and glow | hidden | section 7 |
| Label text | **Bahnschrift Bold 26**, `#FFFFFFFF` | unchanged (stays white) |
| Name chip (icon actions only) | hidden | shown above the action (below) |

Pressed, disabled and checked states: **NOT FOUND** (the template has only the focus trigger; the page toggle looks the same on both pages but for its icon and label). No scale, no animation on focus.

| Action | Width | x (no Queue) | x (Queue showing) | Content | Lines |
|---|---|---|---|---|---|
| Main action | **380** | 80..460 | 80..460 | Label in capitals, left-aligned with `Padding="28,0"` (text from x 108), vertically centred. For a game that is not installed and not installing, a second line under it: the source's name ("Steam"), **Segoe UI Regular 26, `#B3FFFFFF`**; none for a game with no source. | 126-171 |
| Manage | **80** | 472..552 | 472..552 | Glyph **U+E713** (Setting), icon font, **28 px**, Normal weight, white, centred. `Tag="Manage"`. | 173-184, 565-566 |
| Queue (**M4**) | **80** | - | 564..644 | Glyph **U+E896** (Download), same style. `Tag="Queue"`. | E/QueueActionControl.xaml:11-14 |
| Page toggle | content + 56 (`Padding="28,0"`, `MinWidth="80"`) | from 564 | from 656 | Icon, icon font **24 px**, Normal weight, `Margin="0,0,14,0"`, then the label, Bahnschrift Bold 26. On the hub page: **U+E946** (Info) "GAME DETAILS". On the details page: **U+E80F** (Home) "GAME HUB". It names the page it turns to. | 573-602 |

The page toggle's width depends on the label's measured width (**UNVERIFIED**: 56 + icon advance + 14 + text). In the picture, with that build's smaller label, it is 244 wide.

**The name chip** ("a focused action names itself above"; lines 90-95, 98-105). Only for an action with a `Tag`: Manage and Queue. While the action has focus, a chip centred on the action's x: fill `#CC1A1A1A`, border 1 px `#80FFFFFF`, `CornerRadius="3"`, `Padding="10,4"`, text = the tag in **Segoe UI Regular 26**, white. `Margin="-150,-48,-150,0"` puts its top **48 above the action's top**; it is 1 + 4 + 34.58 + 4 + 1 = 44.58 tall, so it spans **y 754.4..799.0**: its foot 3.4 above the action, over the top stroke of the focus ring (which stands 4..8 above the action) across the chip's width, and its top 16 into the cards' row. The actions are drawn **before** the cards (the Actions panel is the grid's first child, line 523), so where the chip overlaps a card the card's 30% fill is drawn over it. Its width is the text's plus 22, up to 380. The main action and the page toggle have no tag and no chip.

### 6.2 The main action, by the game's state

Lines 518-564; E/InstallActionControl.xaml(.cs); C/ExtensionCore.Queue.cs:84-108, 254-289; PN/Playnite.FullscreenApp/ViewModels/GameDetailsViewModel.cs:31-123.

| Game's state | Label (drawn in capitals) | Second line | A does |
|---|---|---|---|
| Installed, not running | "Play" | - | Plays the game: the Launch screen (M1). With another game running it asks "Quit X and start Y?" first; while a game is quitting or launching it does nothing (C/ExtensionCore.cs:506-531, 717-746). |
| Running | "Running" (a separate button that takes the main action's place and focus) | - | **Nothing** (lines 519-535). |
| Launching (between Play and the game being up) | "Launching" (Playnite's label, PN/.../LocSource.xaml:645) | - | Playnite's "Cancel game monitoring?" prompt (PN/.../GameDetailsViewModel.cs:110-112, 136-145). Normally unseen: the Launch screen and the game status view are over the hub. |
| Not installed, the Dashboard can install it silently (a Steam game: C/ExtensionCore.Queue.cs:88-92) | "Install" (the extension's own button, drawn exactly as the main action, in its place) | the source's name | **M4.** Asks Steam to install it without Steam's windows, puts it in the Queue as Queued, and **closes the hub**, back to where it was opened from, as the console does (C/ExtensionCore.Queue.cs:271-289). |
| Not installed, any other game | "Install" | the source's name, if any | Playnite's own install (the client's dialog). The hub closes as the install is seen to start (C/ExtensionCore.Queue.cs:471-486). |
| In the Queue through the extension, waiting | "Queued" | - | **Nothing** (E/InstallActionControl.xaml.cs:224-231). |
| In the Queue through the extension, downloading | "Installing..." (three full stops; C/ExtensionCore.Queue.cs:15-16) | - | **Nothing.** |
| Installing through Playnite | "Installing" (Playnite's label, no dots, PN/.../LocSource.xaml:643; the theme's comment at lines 538-540 says "Installing...") | - | Playnite's "Cancel game monitoring?" prompt; it never starts a second install. |
| Uninstalling | "Uninstalling" (PN/.../LocSource.xaml:644) | - | The same prompt (PN/.../GameDetailsViewModel.cs:106-109). |

The label is upper-cased by the template (lines 133-139, 151), so the screen reads PLAY, INSTALL, QUEUED, INSTALLING..., RUNNING. The state can change with the hub open (an install finishing turns Install into Play); the action is re-evaluated on each change of the game's installed/installing state and of the Queue (E/InstallActionControl.xaml.cs:113-128, 175-194).

Focus hand-offs: when Running replaces Play (or the reverse) focus follows to the one now showing (lines 1029-1038). When LB/RB moves to a game whose main action is the other kind, focus stays on the main action's slot (E/InstallActionControl.xaml.cs:157-173, 189-193).

### 6.3 Manage

A on Manage, and **Menu** anywhere in the hub, open the **Tile menu** for this game **without its Manage game row**, "as the player is there already" (line 372; E/GameMenuTakeover.cs:12-25, 118-124; C/ExtensionCore.TileMenu.cs:24-41). It plays `PromptOpen.wav`. Focus stays on what opened it, so closing the menu leaves focus there. Nothing opens while the Guide, text entry or another menu is up. While the menu is up the hub hears no button (E/TileMenuControl.xaml.cs:118-123; C/ExtensionCore.TileMenu.cs:43-50). The menu itself is tile-menu.md's.

Rows of that menu from the hub: Play game (or Install, or Quit while it runs), Add to / Remove from Favourites, Uninstall (installed, not added by hand, not running), Customise Home (or Customise games & apps when the hub is over My games & apps), More options (C/ExtensionCore.TileMenu.cs:73-109).

### 6.4 Queue (#78)

- **Slot:** between Manage and the page toggle, an 80 x 80 icon action (U+E896), name chip "Queue". When it shows, the page toggle moves 92 to the right.
- **Rule:** shown only while the game is in the Queue, Queued or Installing; it goes as the game leaves the Queue with the hub still open (C/ExtensionCore.Queue.cs:120-125; E/QueueActionControl.xaml.cs:132-150). If it goes while it has focus, focus moves to Manage.
- **A:** closes the hub and opens My games & apps on Manage Queue; B there opens the hub again with focus on Queue (C/ExtensionCore.Queue.cs:127-151, 173-252; CT/HubQueueActionTests.cs:80-119).

### 6.5 Page toggle

A flips the page: the hub page's cards (stats, About, Info) are replaced by the details page's (trailer, Description, Info); the cover card, header and actions stay; focus stays on the toggle; its icon and label change (lines 573-602, 1040-1046). Instant: no slide or fade (NOT FOUND). The toggle has no name chip.

The page is **not reset** when the hub closes or the game changes: no trigger or code sets the toggle back (NOT FOUND), and the hub is one element reused for every game, so by the source a hub opened after the details page was shown opens on the details page. **UNVERIFIED** on screen. The description's scroll position is put back to the top on every change of game (PN/.../Views/GameDetails.cs:52-56).

### 6.6 Achievements page (M6: entry point only; the port's is [achievements-page.md](achievements-page.md))

CONTEXT.md:54-56 defines it as the hub's view of one game's achievements, "opened from the hub and shown in its place". At `f11e4f9` it is **not built** (docs/playnite-achievements.md:14, issue #34): there is no action, card behaviour or binding that opens it (NOT FOUND; A on the stats card does nothing). `E/AchievementPages.cs` is unrelated: it pages the achievement Toast's text. For M3: nothing to draw; M6 decides its entry point (see Open questions).

---

## 7. Focus ring

The same resources as Home's (home.md section 4), at 1:1, with one number different. `XboxHubFocusRing`, lines 65-77, used by the actions (lines 85-89) and the cards (lines 195-199):

- `Border BorderThickness="4" CornerRadius="12" Margin="-8"`, brush `#FF2CA243`, no fill: a 4-thick stroke whose inner edge is 4 outside the element and outer edge 8 outside. Home's tiles use 14 where this uses **12**: by WPF's centre-line rule (home.md 4.1) the ring's outer radius is 14, centre line 12, inner 10. The element under it has radius 4.
- Glow: the **rectangular** variant `XboxFocusGlow` (ten bands and four corner spots, home.md 4.3) under an `XboxFocusGlowCache`, with its own `Margin="-7.5"` inside the ring's border, exactly as on Home's tiles: centred on the ring's centre line, reaching 21.5 outside the element and 9.5 inside it. Not scaled.
- Toggled instantly with focus (Hidden/Visible). It does not slide between elements and has no animation.
- An action shows it while the action itself has keyboard focus; a card while it **or anything inside it** has (lines 98-102, 201-205).

Neighbours: actions are 12 apart, so a focused action's ring (8 out) comes within 4 of the next action and its glow overlaps it; cards are 16 apart, the ring leaving 8.

---

## 8. Navigation

The hub root has `KeyboardNavigation.DirectionalNavigation="Contained"` (lines 367-369): directions move focus by WPF's geometric nearest-element rule and **stop at the hub's edges**, never wrapping and never leaving the hub. Which element WPF picks is its heuristic: **UNVERIFIED** except where a picture shows it.

### 8.1 Buttons

| Input | On an action | On a card | Source |
|---|---|---|---|
| A | Section 6: Play / Install / nothing (Running, Queued, Installing...); Manage opens the Tile menu; Queue opens Manage Queue (M4); the toggle flips the page | **Nothing** | lines 6, 518-603 |
| B | Closes the hub (section 9.3) | Same | line 8; PN/.../Views/GameDetails.cs:108-120 |
| Menu (Start) | Tile menu without Manage game, `PromptOpen.wav`; focus stays where it was | Same | line 372; E/GameMenuTakeover.cs:118-124 |
| LB / RB | The hub shows the **previous / next game** of the list it was opened over (Playnite's main list as it is filtered and sorted then); stops at the first and last, no wrap. Header, cards, actions and the art change in place; the backdrop snapshot does not. | Same | line 8; PN/.../Views/GameDetails.cs:110-113; PN/.../ViewModels/FullscreenAppViewModel_Commands.cs:310-328 |
| X | **NOT FOUND**: nothing | Nothing | - |
| Y | **Nothing**: the hub sits outside the view host that answers Y with Search, "so it never hears them" | Nothing | E/PlayniteViewHost.cs:10-15 |
| View (Back) | **NOT FOUND**: nothing in the hub | Nothing | - |
| LT / RT, stick clicks | **NOT FOUND**: nothing | Nothing | - |
| Left / Right | Previous / next action in the row (main, Manage, [Queue], toggle); nothing at either end | Previous / next card; nothing at either end. The row jumps as section 5.4 says when focus reaches Description or Info on a wide row. | line 367 |
| Up | To the cards. From Play it lands on the **cover card** (picture `data-hub-up.jpg`). From Manage, Queue or the toggle: the card WPF finds nearest above, **UNVERIFIED** (by position the stats card, or the trailer on the details page, is above them). | Nothing (nothing above the cards takes focus) | line 367 |
| Down | Nothing (nothing below) | To the actions; which one is WPF's choice, **UNVERIFIED** (the main action is under the cover card) | line 367 |
| Held direction | Repeats as Playnite repeats it (home.md 7.4) | Same | UNVERIFIED |

Keyboard equivalents Playnite binds: Backspace and Escape close; F2 / F3 are LB / RB (PN/.../Views/GameDetails.cs:108-111).

While the Tile menu is over the hub every press belongs to the menu, and a button held as the menu closes is ignored until released (C/ExtensionCore.TileMenu.cs:43-50).

### 8.2 Focus on entry, and memory

- **On opening:** the **main action** (Play, Install, Queued, Installing... or Running), always, whatever was focused the last time the hub was open (lines 8-9, 519-521; E/InstallActionControl.xaml.cs:130-155). There is no memory of the last focused action or card across openings.
- **On opening again from Manage Queue's B** (M4): **Queue**, if it still shows; else what Playnite focused (E/QueueActionControl.xaml.cs:94-130; E/InstallActionControl.xaml.cs:144-145).
- **After flipping the page:** the toggle keeps focus (lines 1044-1045).
- **After the Tile menu closes:** where it was (the menu never takes the hub's focus).
- **After LB / RB:** focus stays where it was; if that was a card the new game hides, WPF moves it: **UNVERIFIED**; if it was a main action that swapped kind, the new main action (section 6.2).
- **Within the hub** there is no remembered column between the cards and the actions: each Up or Down is a fresh nearest-element search.

### 8.3 Scrolling inside the hub

The only scrolling content is the **Description card's text** on the details page (lines 989-998). The page itself never scrolls vertically.

- Focus on the card's scroller; **Up / Down scroll the text**. The step is Playnite's `CustomScrollAmount="42"` (line 989). By Playnite's code a press first scrolls WPF's own line and is then pushed on by 42, and reaching the top or the end by a scroll moves focus on Up or Down at once (PN/Playnite.FullscreenApp/Controls/ScrollViewerEx.cs:80-112): the exact distance per press is **UNVERIFIED**.
- At the top, Up tries to move focus up (nothing there). At the end, Down moves focus **down to the actions**. A description that fits does both at once (PN/.../ScrollViewerEx.cs:66-77).
- Left / Right leave the scroller for the neighbouring cards (PN/.../ScrollViewerEx.cs:50-63).
- No scroll bar, no fade at the edges, no "more" hint (line 991; NOT FOUND otherwise). The scroll position resets to the top when the game changes (PN/.../Views/GameDetails.cs:52-56).

---

## 9. Opening and closing

### 9.1 Where it opens from

| From | How | Delay | Source |
|---|---|---|---|
| Home: a Recent-row tile or a Group card | A, when no second A follows on the same tile | **300 ms** (the double-tap window; a second A within it plays the game instead and no hub opens) | C/ExtensionCore.cs:35-36, 1327-1362 |
| My games & apps: Play history's rows, Manage Queue's rows and its Recently installed covers (M4) | A, through the same rule | 300 ms | E/MyGamesControl.xaml.cs:1472, 1788, 1819; C/ExtensionCore.cs:850-854 |
| My games & apps: Games and Full library (Playnite's own grid) | A on a game, by Playnite's own binding | see my-games.md | **UNVERIFIED** here |
| Search: a game found or recently added | A | **None**: "at once, as there is no double-tap to play here" | C/ExtensionCore.Search.cs:184-197 |
| Tile menu: Manage game | A on the row; the menu closes first | None | C/ExtensionCore.TileMenu.cs:87-94 |
| Idle screen (**M5**): the card's game | Y on a card; closes text entry, Settings and the Tile menu first, brings the Dashboard to the front | None | C/ExtensionCore.Idle.cs:165-196 |
| Manage Queue (**M4**): B, when it was opened from a hub's Queue | The hub opens again over what it was over, focus on Queue | None | C/ExtensionCore.Queue.cs:173-227 |

A Menu press during the 300 ms cancels the pending hub (C/ExtensionCore.TileMenu.cs:66-67). The hub opens for one game at a time; if the game cannot be shown the request does nothing (E/PlayniteHost.cs:188-199).

### 9.2 Opening transition

**There is none.** The hub is made visible in one frame: NOT FOUND any storyboard, opacity or transform in `GameDetails.xaml`, and Playnite only flips a visibility flag (PN/.../FullscreenAppViewModel_Commands.cs:83-98). In order, within that frame: the snapshot of the page is taken (section 2.3), the hub is drawn over the page, focus goes to the main action, the hint bar shows "More options". The page under the hub stays as it was (Home keeps its rest position and its hero) but is fully covered.

The only motion is Playnite's own 0.5 s crossfade of the blurred background art when its picture changes (section 2.1); whether it is seen as the hub first opens is **UNVERIFIED**.

**Reduce motion** changes nothing on this page: nothing in the hub asks `E/Motion.cs`, and there is nothing that slides, grows or scales.

### 9.3 Closing

| Cause | What happens | Source |
|---|---|---|
| B (or Backspace, Escape) | The hub hides in one frame; the snapshot is freed; focus returns (below) | PN/.../Views/GameDetails.cs:108-120; E/HubBackdrop.cs:86-92 |
| A on Install (silent or Playnite's, once it starts) | The hub closes, back to where it was opened from; the game's tiles read Queued | C/ExtensionCore.Queue.cs:271-289, 471-486 |
| A on Queue (M4) | The hub closes; Manage Queue opens | C/ExtensionCore.Queue.cs:134-151 |
| The game played from the hub exits | Home comes to the front out of any hub: the page rests on the Recent row, focus on that game's tile | C/ExtensionCore.cs:637-671; E/PlayniteHost.cs:79-103 |
| The hub's game leaves the list behind it (removed, filtered out) | Playnite shows a neighbouring game, or closes the hub if the list is empty | PN/.../Views/GameDetails.cs:73-98 |

No closing animation (NOT FOUND).

**Where focus returns after B:**

- **Home:** the row that last had focus takes it back: the Recent row on its last focused game's tile, or the Groups on the remembered Group and card; the page is still at that rest (E/RecentRowControl.xaml.cs:258-265; E/GroupsControl.xaml.cs:144-151; home.md 7.4). The picture `data-hub-closed.jpg` shows the tile focused and grown again.
- **My games & apps, Play history or Manage Queue:** the content takes focus back (E/MyGamesControl.xaml.cs:1291-1299). Games / Full library: Playnite puts focus back on its grid itself, on the hub's game. After LB / RB that is the game the hub ended on.
- **Search:** the tile of the game that was chosen; if it is no longer found, the keyboard's key while the keyboard is up, else the first tile, else the search box (E/SearchControl.xaml.cs:390-420).
- **Tile menu's Manage game:** the menu closed before the hub opened, so focus returns to the tile the menu was opened on.

---

## 10. Hint bar

C/ExtensionCore.Hints.cs:48-91; E/HintBarControl.xaml.cs:103-143; drawing as home.md section 10.4 (bottom right, right edge x 1860, bottom y 1028, 63 tall, `#D9030303`).

| State | Hints |
|---|---|
| Focus anywhere in the hub (any action, any card) | **"More options"** alone (the Menu glyph: white ring, U+E700) |
| Tile menu up, a prompt up, the Guide up, Start-up, the Idle screen's dim | none |
| Focus has left the hub (a window or dialog took it; the hub closed) | whatever the page behind asks for |

Never "Search" in the hub, whatever page is behind it (C/ExtensionCore.Hints.cs:87). The bar is drawn **over** the hub, inside the band's lower right corner and across the band's foot: it spans y 965..1028 while the band ends at 992 (picture: x 1627..1860). Shown and hidden instantly.

---

## 11. Game status view (what shows while a game starts and runs)

`T/Views/GameStatus.xaml` (whole file); CT/GameStatusXamlTests.cs. Playnite's own view, shown over the page, the hub, the hint bar and the Tile menu (T/Views/Main.xaml:773) from the moment a launch begins until it is closed. With the extension the **Launch screen** (M1) is shown above it in its own window, and the view is drawn to match it "so the Launch screen's fade-out reveals the same picture" (lines 1-12 of that file). It is what is on the Dashboard's window if the user looks at it while the game runs.

| Layer (back to front) | Value | Lines |
|---|---|---|
| Background | Black | 34 |
| Art | As Home's hero chooses (home.md 8.1): background art as it is, `UniformToFill`, **centred across, top-aligned**, no effect. Without it, the cover: laid out 128 x 72 with `Margin="-8"` (so it overhangs by 120 on every side at x15), top-aligned, Gaussian blur radius 4 there (60 on screen), stretched x15, under a flat `#99000000`. With neither: black. | 69-92, 120-131 |
| Scrim | Home's hero scrim (`XboxHeroScrimBrush`, home.md 8.4) | 95-96 |
| Shade | Radial gradient, centre and origin (0.5, 0.55), RadiusX 0.45, RadiusY 0.6 (864 x 648 px): `#B3000000` @0, `#80000000` @0.55, `#00000000` @1 | 97-105 |
| Cover | The game's cover, `Height="360"`, width by its own shape (240 for a 2:3 cover), Fant scaling, centred at x 960. No rounding, no border. Playnite's default cover when the game has none. | 107-109 |
| Status text | **Bahnschrift Regular 28**, white, centred, wrapping within the middle column (822.86 wide: columns 2 : 3 : 2), `Margin="0,28,0,28"` | 111-114; T/Constants.xaml:293-299 |

Rows are `*`, Auto, Auto, `*`, so cover and text are centred as a block: with a one-line status (33.6 at 1.2 em) the block is 360 + 28 + 33.6 + 28 = 449.6 tall: **cover y 315.2..675.2, text y 703.2..736.8**.

Text: "**{Game} is starting…**" from the launch beginning, then "**{Game} is running…**" once the game has started (the ellipsis is the single character U+2026) (line 2; PN/.../ViewModels/FullscreenAppViewModel.cs:509-542; PN/Playnite/Localization/LocSource.xaml:1040-1041).

Buttons: **B** (or Escape) closes the view and focus goes back to Home (lines 51-55 of that file). No Close button shows: Playnite's is there at 1 x 1, opacity 0, only as somewhere for focus to return to; A on it closes the view too (lines 18-28, 116-118). No hint bar. No animation in the view. The view goes by itself when the game stops (Playnite's behaviour: **UNVERIFIED** in this repository).

The hub under it keeps its state: once the game runs, its main action is "Running".

---

## 12. Sounds

The catalogue and volume rule are home.md section 9. Events in the hub:

| Event | Sound | Played by | Source |
|---|---|---|---|
| Focus lands on an action (main, Manage, Queue) or on a focusable card (cover, stats, About, Info, trailer) | `navigation.wav` (always the same file) | Playnite, for focus landing on any button; the cards are buttons for that reason | lines 187-189; E/GameHubFocusSounds.cs:9-13; E/InstallActionControl.xaml.cs:18-20; PN/.../ViewModels/FullscreenAppViewModel.cs:408, 544-576 |
| Focus lands on the **page toggle** or the **Description's scroller** | `Navigate1..4.wav` in rotation | Extension: Playnite leaves a toggle and a scroller silent, so the extension sounds them | E/GameHubFocusSounds.cs:19-34; E/PlayniteSounds.cs:25, 121-128 |
| The hub opens | No sound of its own (NOT FOUND). Focus landing on the main action is a focus change as any other; whether it sounds is **UNVERIFIED**. | - | - |
| A on an action (Play, Install, toggle, Queue) | **None.** Playnite's `activation.wav` is played only when a checkbox or slider changes; the toggle is neither. | - | PN/.../ViewModels/FullscreenAppViewModel.cs:405-407, 578-584 |
| Menu, or A on Manage (Tile menu opens) | `PromptOpen.wav` | Extension | C/ExtensionCore.TileMenu.cs:38-40 |
| B closes the Tile menu | `PromptClose.wav` | Extension | C/ExtensionCore.cs:803-807 |
| B closes the hub | None of its own (NOT FOUND); focus landing back on the tile behind may sound as a focus change (UNVERIFIED) | - | - |
| Page flip, the card row jumping, the description scrolling, LB / RB | None (NOT FOUND) | - | - |
| Edge bump (Left at the first action, and so on) | None | - | NOT FOUND |

A port that plays one navigation sound on every focus move inside the hub, the toggle and scroller included, is indistinguishable to the ear from the above except for the rotation.

---

## 13. States, text rules, traps, and what the pictures confirmed

### 13.1 Empty, loading and error states

| Case | What shows | Source |
|---|---|---|
| No background art, cover present | The cover, blurred (radius 60 on screen, x1.15), behind the band and, without a snapshot, in the strips | lines 390-418 |
| No background art, no cover | The green radial background, **no darkening gradient**; the band's `#B3000000` still over it. Cover card: Playnite's default cover. Trailer card: empty, just the card's fill. | lines 362-363, 455-463 |
| No description | About (hub page) and Description (details page) are collapsed | lines 933-938, 978-983 |
| Never played, no completion status, no achievements | Your game stats is collapsed | lines 826-840 |
| No genres, developers, series or scores | Info is collapsed on both pages | lines 341-356 |
| A game with nothing but a name and a cover | Hub page: the cover card alone. Details page: cover and trailer. Header: the title and an empty facts line that keeps its height. | sections 4.2, 5 |
| No publisher, year, ... in the header | Each blank fact drops out with its bullet; no facts and no rating leaves the line empty | section 4.2 |
| No source | No second line on Install; badge shows the platform, or no badge | lines 163-169, 805-817 |
| Cover still loading | The card's `#FF1A1A1A` until the picture is there; no spinner, shimmer or fade (NOT FOUND) | T/Constants.xaml:207 |
| Background art loading or changing | Old picture until the new one fades in over 0.5 s (Playnite) | section 2.1 |
| No snapshot could be taken | The strips show the game's art instead | lines 479-483 |
| The hub cannot open (the game is not in the list behind) | Nothing happens; no message | E/PlayniteHost.cs:188-199 |
| A launch that fails | Playnite's own error dialog; the Launch screen goes (M1) | C/ExtensionCore.cs:585-597 |

There is no loading state for the hub as a whole and no error text anywhere in it.

### 13.2 Text rules

| Text | Font | Size | Colour | Wrap / trim |
|---|---|---|---|---|
| Title | Bahnschrift Regular | 47 | `#FFFFFFFF` | 1 line, character ellipsis at 1760 |
| Facts | Segoe UI Regular | 20 | `#B3FFFFFF` | 1 line, character ellipsis, before the rating box |
| Age rating | Segoe UI Regular | 20 | `#FFFFFFFF` | 1 line, never trimmed (measured first) |
| Card title | Segoe UI Bold | 20 | `#FFFFFFFF` | 1 line, ellipsis |
| Card label, card value | Segoe UI Regular | 26 | `#FFFFFFFF` | 1 line, character ellipsis |
| Achievements percent | Segoe UI Regular | 26 | `#B3FFFFFF` | - |
| About / Description text | Segoe UI Regular | 26 | `#E6FFFFFF`, links `#FF2CA243` | Wraps; About is clipped at its box, Description scrolls |
| Feature pill | Segoe UI Regular | 26 | `#FFFFFFFF` | 1 line per pill; pills wrap; block clipped at 80 |
| Cover badge text | Segoe UI Regular | 14 | `#FFFFFFFF` | 1 line, max 180 wide, ellipsis |
| Action label (PLAY, GAME DETAILS ...) | Bahnschrift **Bold** | 26 | `#FFFFFFFF` | 1 line, capitals; the main action's is trimmed with an ellipsis inside 324 (380 - 56) |
| Main action's second line (source) | Segoe UI Regular | 26 | `#B3FFFFFF` | 1 line, ellipsis |
| Name chip over an icon action | Segoe UI Regular | 26 | `#FFFFFFFF` | 1 line |
| Action icons | `Segoe Fluent Icons`, fallback `Segoe MDL2 Assets`, Normal weight | 28 (Manage, Queue), 24 (toggle) | `#FFFFFFFF` | - |
| Game status text | Bahnschrift Regular | 28 | `#FFFFFFFF` | Wraps, centred, ellipsis |

Sizes are stated by the Bundle's own tests: CT/ConsoleTypeSizesXamlTests.cs:159-208. Font resources: T/Constants.xaml:228-235. All hub text is Regular unless the table says otherwise, even inside a Bold action (lines 28-38).

### 13.3 Small traps

- Nothing here is scaled by Home's 1.114. The ring is 4 thick and 8 out in real units; its `CornerRadius` is 12, not Home's 14.
- The hub **blurs** background art and centres it; Home shows it sharp and top-aligned. The game status view shows it sharp and top-aligned again.
- Art is darkened twice inside the band (diagonal gradient, then `#B3000000`); the strips are the **page behind** under `#99000000`, not the art.
- The band is full width with square corners; the content starts 80 in and 40 under the band's top.
- A focused action's whole slab turns `#2CA243`; a focused card does not change fill.
- Only Manage and Queue name themselves above; Play and the page toggle carry their own labels. The chip overlaps the ring's top stroke and the cards' foot (section 6.1).
- The main action's label is the state, in capitals; "Installing..." (extension, with dots) and "Installing" (Playnite's own) are different states.
- Running, Queued and Installing... are focusable buttons that do nothing on A; they are not greyed.
- Install closes the hub; Play does not (the Launch screen covers it and Home comes forward when the game exits).
- A on a card does nothing, but cards take focus, sound, and show the ring.
- Up from Play lands on the cover card, not on the stats card.
- The details page is wider than the screen and jumps, not slides, between two positions; the hub page never moves.
- The feature pills' second row is cut by the 80 limit at 26 px type.
- The cover card's cover is top-aligned (crops the foot of a tall cover), as Home's tiles; the trailer card's pictures are centred.
- The hint bar says only "More options" and overlaps the band's foot.
- Y does nothing in the hub even though it opens Search on the page behind.
- LB / RB change the game without closing the hub; B then returns to the page with that game selected behind it where the page follows the list (My games & apps' grid), while Home's rows keep their own remembered tile.
- The 300 ms wait before the hub opens is only on tiles where a double tap plays (Home, Play history); Search and Manage game open it at once.

### 13.4 What the pictures confirmed, and what they could not

Measured on `achievements-20261110-105905/data-hub.jpg` and `data-hub-up.jpg` (JPEG, so edges are good to about 1 px):

| Measure | Source says | Picture |
|---|---|---|
| Band | y 88..992 | edges at y 88 and 992, full width |
| Cover card | x 80..455 (375 wide), 500 tall | x 80..455; y 277..776 |
| Gap between cards | 16 | cover ends 455, stats starts 471 |
| Stats card | 416 wide | x 471..887 |
| Stats card padding, progress bar | content x 497..861; bar 6 tall; 60% of 364 = 218.4 | text and bar from x 497 to 861; bar y 565..571; fill ends at x 715 |
| Cards to actions | 32 | 776 to 808 |
| Actions | 80 tall; Play 380 wide; 12 apart; Manage 80 wide | y 808..888; Play x 80..460; Manage x 472..552; toggle from x 564 |
| Play's label inset | 28 | "PLAY" starts at x 109 |
| Focus ring | 4 thick, inner edge 4 out, outer 8 out | stroke x 72..76, gap 76..80 (cover card and Play) |
| Focused action fill | `#2CA243` | (44, 162, 66) |
| Resting action fill | `#33FFFFFF` over the band | (76, 74, 73) over (32, 29, 28): 20% white |
| Card fill | `#4D2A2A2A` over the band | (36, 34, 33) over (33, 29, 29) |
| Cover badge | 36 x 28, 10 in from the card's left and foot | x 90..126, y 738..766 with the card's foot at 776 |
| Up from Play | (WPF) | the cover card takes focus |
| A on a card | nothing | identical picture |
| Hint bar | "More options", right edge 1860, bottom 1028 | x 1627..1860, bottom 1028 |
| B | back to the tile | `data-hub-closed.jpg`: Home, the tile focused |

Not confirmed by any picture (older build, or no picture exists): the header as it is at `f11e4f9` (title at 47, the facts line, the rating box) and therefore every y under it (the picture's cards sit 6 lower than this spec's 270.4); the Bahnschrift line height; the 26 px card text and the stats rows' y; the strips showing the page behind (the picture's strips show the game's art); the name chip over Manage and Queue; the Queue action; Install and its second line; Running; the details page, its trailer, the scroller and the row's jump; the About and Info cards (the test game has no description or genres); the feature pills; the game status view.

---

## 14. What is Playnite-specific, and what to do about it

### 14.1 The fields the hub binds, and where the port gets them

(Written before the hub was built: section 16.1 says which of these the library has since gained.) The hub reads Playnite's game model. The Dashboard's library (`crates/library/src/lib.rs`, schema version 1; `crates/model` `Game`) holds per game: `source`, `source_id`, `name`, `sort_name`, `installed`, `install_dir`, `last_played`, `playtime_minutes`, `added`, `hidden`, `favourite`, `gone`; art by kind (`cover`, `background`, `header`; `logo` and `icon` are allowed kinds but not filled); and `play_sessions` (started, ended). Nothing else. "Steam has it" below means Steam's local files are believed to carry it (`appinfo.vdf`, `appmanifest_*.acf`); that is from general knowledge of those files, not from this repository, and the `source-steam` crate does not read any of them yet: **to be confirmed when the column is added**.

| Bundle's binding | Where shown | In our library now | From Steam's files | Without it |
|---|---|---|---|---|
| `DisplayName` (line 614) | Title | Yes: `name` | - | - |
| `Publishers` (652) | Fact 1 | No | Steam has it (appinfo: associations / extended publisher) | Drop the fact and its bullet |
| `ReleaseYear` (668) | Fact 2 | No | Steam has it (appinfo: release date as Unix time) | Drop the fact |
| `InstallSize`, `IsInstalled` (685-700) | Fact 3, installed games only | `installed` yes; size no | Steam has it (the game's `appmanifest` SizeOnDisk) | Drop the fact |
| `Platforms` (718; badge 1024-1027) | Fact 4; badge when there is no source | No | A constant for a PC store (what Playnite's Steam library writes there is **UNVERIFIED**) | Drop the fact (every game has a source, so the badge never needs it) |
| `Source.Name`, `SourceId` (737-746, 153, 167, 803, 1014-1023) | Fact 5; Install's second line; the badge | Yes: `source` ("steam"), to be shown as "Steam" | - | - |
| `Features` (766; pills 238-248) | Fact 6 and the pills | No | Steam has store categories in appinfo as numeric ids (single-player, controller support, ...) that need a name table | Drop the fact; no pills |
| `AgeRatings` (630) | Rating box | No | Not reliably in Steam's local files (**UNVERIFIED**) | No box |
| `CoverImageObject` / `CoverImage` (394, 410, 791-792, 967) | Cover card; blurred backdrop; trailer card | Yes: art `cover` | - | Bundle shows Playnite's default cover; port: the title on `#FF1A1A1A` as Home's tiles (home.md 3.3) |
| `BackgroundImage` / `DisplayBackgroundImageObject` (409, 429, 436, 969) | Backdrop; trailer card | Yes: art `background` | - | Cover blurred, else the green background |
| `IsRunning` (529, 547, 1033-1038) | Running in place of Play | Not a column: the shell knows the running game | - | - |
| `IsInstalling`, uninstalling, launching (159; Playnite's label) | Main action's label | No (installs are M4) | Steam's appmanifest state flags | M3: only Play / Install / Running |
| `Playtime` (848, 855) | Stats: Time played | Yes: `playtime_minutes` (Playnite's is seconds; format from minutes: under an hour "N minutes", else "Hh Mm") | - | Row collapsed at 0 |
| `PlayCount` (862, 869) | Stats: Times played | Derivable: count of `play_sessions` for the game. Steam keeps no launch count, so games played before the Dashboard existed count from 0. | No | Row collapsed at 0 |
| `LastActivity` (875, 882) | Stats: Last played | Yes: `last_played` | - | Row collapsed when 0 |
| `CompletionStatusId`, `CompletionStatus.Name` (889, 896) | Stats: Completion | No | No (a Playnite notion the user sets) | Row always collapsed, unless the port adds the notion |
| `Description` (935, 945, 980; Playnite binds the scroller's text) | About, Description | No | **No**: the store description is not in Steam's local files; it needs the store's web API (or another metadata source, plan.md open question 3) | Both cards collapsed |
| `Genres` (263, 270) | Info | No | Steam has genre ids in appinfo (need a name table) | Pair collapsed |
| `Developers` (276, 283) | Info | No | Steam has it (appinfo) | Pair collapsed |
| `Series` (289, 296) | Info | No | Not as such (franchise association at best: **UNVERIFIED**) | Pair collapsed |
| `CriticScore` (302, 309) | Info | No | Steam has a Metacritic score for some games (appinfo) | Pair collapsed |
| `CommunityScore` (315, 322) | Info | No | Steam has a review percentage (appinfo) | Pair collapsed |
| `UserScore` (328, 335) | Info: "Your score" | No | No (the user's own rating in Playnite) | Pair collapsed |
| PlayniteAchievements `HasAchievements`, `Unlocked`, `Total`, `Percent` (906-925) | Stats: Achievements row | No | **M6** | Row collapsed |
| Extra Metadata Loader's video (971) | Trailer card | No | Steam's library cache may hold microtrailers for some games (**UNVERIFIED**) | The art alone |
| Extension `HubBackdrop.HasSnapshot`, `Snapshot` (481, 489, 496) | The strips | Not library data: a render target | - | The game's art in the strips |

Consequence for M3 with today's schema: the header is the title and "Steam"; the hub page is the cover card and, for a played game, Your game stats with Time played, Times played (if sessions exist) and Last played; About and Info never show; the details page is cover and trailer art. The page is built to collapse to exactly that. Filling it out is a schema migration (publisher, developer, release date, size on disk, genres, features, scores) plus a description source.

### 14.2 Behaviour that is Playnite's

| Playnite-owned thing | What the hub leans on | Recommendation |
|---|---|---|
| The Game details view itself: visibility, data context, one instance for every game | The hub is this view restyled; the extension opens it by selecting the game in Playnite's main list and toggling the view (E/PlayniteHost.cs:188-199), which is why lists are refiltered before a hub opens (C/ExtensionCore.cs:1603-1615; C/ExtensionCore.Search.cs:184-197) | **Replace**: a `GameHub` page screen pushed on the page stack with a game id (ui.md "Screens and what is over what"). None of the main-list juggling is needed: drop it. |
| Focus on the first action as the view shows; WPF directional navigation between cards and actions | Sections 8.1, 8.2 | **Port the outcome with explicit rules**: focus starts on the main action; Left/Right along a row with stops; Down from any card to the action nearest by centre x; Up from the main action to the cover card, from another action to the card nearest by centre x. Record the rule in this file's "differs" notes when built, as home.md section 11 does. |
| B / Backspace / Escape through Playnite's cancellation binding | Closing | **Port**: B pops the page. |
| LB / RB: previous / next game of the main list | Section 8.1 | **Decide** (Open question 4). If kept: step through the list the hub was opened from (the Recent row, the Group, Play history, the search results), stopping at the ends. |
| The main action's label and command (Play, Install, Launching, Installing, Uninstalling) and the "Cancel game monitoring?" prompt | Section 6.2 | **Replace**: the port's own states. M3: Play, Install (handing to Steam, M4 for the Queue), Running. Drop "Launching" (the Launch screen covers it) and the monitoring prompt. |
| Playnite's game menu, taken over by the extension so Menu and Manage open the Tile menu | Section 6.3 | **Port the result**: Menu and Manage open the Tile menu without Manage game. Nothing to take over. |
| `HtmlTextView` and `DescriptionView.html` (HTML descriptions) | About, Description | **Replace**: store descriptions as plain text (strip tags, keep paragraphs) and draw with `ui.paragraph`; About clips to whole lines with an ellipsis on the last (a deliberate improvement on a hard clip: note it as a difference). |
| `ScrollViewerEx` (42 per step, focus hand-off at the ends) | Section 8.3 | **Port simply**: Up/Down scroll by 42 per press, instantly (the Bundle has no animation here), Down at the end moves to the actions, Up at the top does nothing. |
| `FadeImage` (blur by `BlurAmount`, 0.5 s crossfade, 100 ms delay) | Backdrop art | **Port**: `Kind::Blurred` art; a 0.5 s linear crossfade when the game changes under LB/RB, if LB/RB is kept. |
| Playnite's converters: play time, last played date, size, list joins | Texts in sections 4.2 and 5.2 | **Port the formats** given there; the date in the user's short date format. |
| Playnite's default cover for a game with none | Cover card, game status view | **Replace** with Home's no-cover tile (title text on `#FF1A1A1A`). |
| Focus sounds for buttons (`navigation.wav`), none for the toggle and scroller, which the extension adds | Section 12 | **Port**: one navigation sound per focus move in the hub. (Whether the console's sounds come over is plan.md open question 4.) |
| The game status view and its "is starting… / is running…" text | Section 11 | **Decide** (Open question 6). The Launch screen (M1) already does its job while a game starts. |
| PlayniteAchievements, Extra Metadata Loader | Achievements row, trailer video | Achievements: **M6**. Trailer video: **drop for M3**, show the art; revisit with video in the renderer (M5). |
| The store marks on the badge (Steam, Xbox paths) | Cover badge | **Do not copy** (home.md 3.3): the launcher's mark, the project's own drawing (home.md 3.3.1), in the same chip. |
| The extension's silent Install and Queue action | Sections 6.2, 6.4 | **M4.** M3 leaves the Queue slot and gives Install to Steam (`steam://install/<appid>` or nothing: Open question 7). |

---

## 15. Open questions

Each was decided for M3 when the hub was built: section 16 says how, under the question's number.

1. **Bahnschrift's line height.** 1.2 or 1.25 em (the Bundle's tests use both)? It moves the facts line, the cards and the actions by 2.35. Measure a 47 px Bahnschrift line on Windows, or accept 1.2 as home.md does.
2. **The feature pills' second row** is cut in half by `MaxHeight="80"` at 26 px type, against the source's own comment. Reproduce the cut, show one row only (limit 50.58), or two whole rows (limit 101.16, taking 21 from the description)?
3. **The name chip over Manage and Queue** overlaps the focus ring's top stroke and sits partly under the stats card's foot, by the source's arithmetic (48 above, 44.58 tall). Reproduce exactly, or lift it clear of the ring (for example 56 above)? No picture exists to say how it looks on the real thing.
4. **LB / RB.** In the Bundle they step through Playnite's main list, whose contents depend on how the hub was opened. Keep them (stepping through the row or list the hub came from), or drop them? If kept, what does B return focus to after stepping: the original tile (Home's behaviour) or the game now shown?
5. **Which page a hub opens on.** By the source the page toggle is never reset, so a hub can open on Game details if that was showing last. Is that wanted, or does every opening start on the Game hub page? (Unverified on the real thing.)
6. **The game status view.** Port it as the Dashboard's face while a game runs ("X is running…", B back to Home), fold it into the Launch screen, or drop it and show Home with the Running badge?
7. **Metadata the library lacks.** Which of publisher, developer, release year, install size, genres, features, scores does M3 add to the schema from Steam's files, and is a description source (Steam's store API, or another) in M3 or left to M4's "art and metadata" work? Until then About, Info and most of the facts never show.
8. **Times played.** Count our own Play sessions (0 for everything played before the Dashboard), or hide the row until there is a better source?
9. **Completion and Your score.** Playnite notions with no Steam source. Drop the two rows for good, or add them as user-set fields later?
10. **Recent Play sessions.** The glossary says the hub shows a game's most recent Play sessions; the Bundle does not. The Dashboard records sessions (`play_sessions`). Add a card (which slot: after Your game stats?), add rows to the stats card, or leave it out as the Bundle does?
11. **Install in M3.** Before the Queue exists (M4), what does A on Install do: hand to Steam and close the hub as the Bundle does, or stay disabled?
12. **The Achievements page's entry point (M6).** The Bundle never built it. A on the stats card (where the Achievements row is), a fifth action, or a card of its own? M3 only needs to know whether to keep A on the stats card free.
13. **The trailer card without video.** With no Extra Metadata Loader the details page's first card is 889 wide of plain art. Keep the page as it is, or leave the Game details page out of M3 until there is a description to put on it (with today's schema it holds nothing the hub page does not)?
14. **Up and Down between cards and actions.** The Bundle leaves the choice to WPF; only Up from Play is known (the cover card). Accept the nearest-by-centre-x rule of section 14.2?

---

## 16. The Dashboard's Game hub: what was decided, and where it differs

What `crates/screens/src/game_hub/` does (ticket #51), where the sections above left a choice or where it does otherwise. "Q" is an open question of section 15.

### 16.1 Decided

- **Only what Steam's own files on the machine hold; nothing is fetched** (Q7). The library gained (schema version 3; `model::Game`) `released`, `developers`, `publishers`, `size_on_disk`, `critic_score`, `community_score`, read by `source-steam` from `appinfo.vdf` (`common/associations` by type, else `extended/developer` and `extended/publisher`; `common/original_release_date`, else `steam_release_date`; `common/metacritic_score`; `common/review_percentage`) and from the game's `appmanifest` (`SizeOnDisk`). A fact Steam does not have for a game is missing, and what would show it is left out. Not added: genres, store categories (the Bundle's features) and the franchise (its series), which are in `appinfo.vdf` as numbers that need a table of names; the platform; the age rating; and the description, which no local file holds.
- **A card with nothing to show is left out**, as in the Bundle (5.1). With today's library: the hub page is Cover, Your game stats (a game that has been played) and Info (a game Steam names a developer or a score for); Game details is Cover, the wide card and Info. **About and Description never show**, and so no feature pills (Q2 does not arise).
- **The Game details page ships** with what the library has (Q13): its wide card shows the game's art where the Bundle plays a trailer (background art about its middle, else the cover; with neither, the card's fill).
- **The facts line** is the publishers, the release year, the size on disk of an installed game, and "Steam". The Bundle's platform, features and age rating box are not drawn.
- **Bahnschrift's line is 1.2 em** (Q1), as on Home: the title's line is 56.4 and the cards' top 270.4, which is where the Bundle's picture `19-game-hub-play-focused.jpg` has it.
- **The name chip is as the source's arithmetic has it** (Q3): its top 48 above the action, over the ring's top stroke. The picture `20-game-hub-manage-focused.jpg` shows it **over** the cards' foot, not under it as section 6.1 reasons from the element order: it is drawn last.
- **LB and RB are not built** (Q4). In the Bundle they walk Playnite's main list, which is whatever was filtered when the hub opened; here a hub is opened for one game from a row of Home, and nothing says what "next" would be. They do nothing, like X, Y and View.
- **A hub always opens on the Game hub page** with focus on the main action (Q5): the page is made anew each time it opens, and remembers nothing.
- **The game status view is not ported** (Q6). The Launch screen covers the hub while a game starts; the hub stays under it, its main action Running, and goes when the game ends, which is Home again with focus on the game's tile (9.3).
- **Times played** counts the Play sessions the Dashboard has recorded (Q8): 0, and so no row, for a game only ever played before the Dashboard.
- **Completion and Your score are dropped** (Q9): Playnite's notions, with no store behind them.
- **Recent Play sessions are left out**, as the Bundle leaves them out (Q10).
- **Install is the Tile menu's Install** (Q11). The action is drawn, with the store's name under it, and takes focus; A on it closes the hub and asks Steam for the game with `steam://install/<appid>` (`tile_menu::install`, which the menu's row answers with too). Steam's own window does the rest: there is no Queue until M4. A game in no library database asks nothing, and the hub closes all the same.
- **The Achievements row** (#141) is the Bundle's (section 5.2, row 5), from `Library::achievements` (the game's `GameProgress`): the last row of Your game stats, which shows for a game with achievements though never played. A game with none has no row, and nothing stands in for it.
- **A on Your game stats opens the Achievements page** (Q12, #141) of a game that has achievements, in the hub's place; B there comes back to the card ([achievements-page.md](achievements-page.md)). For a game with none, A on the card does nothing, as in the Bundle.
- **Up and Down go by the nearest middle, across** (Q14): Up from the main action is the cover card; from Manage or the toggle the card whose middle is nearest the action's; Down from a card the action whose middle is nearest the card's. No column is remembered.

### 16.2 Differs

- **Opening** (9.1). A on a Recent tile or a Group's game card starts the double tap's 300 ms: with no second A the hub opens when they are up, timed on the Dashboard's clock (`Dashboard::advance`, docs/ui.md); a second A on the same tile within them plays the game and no hub opens. Menu within them, or anything that takes the pad from Home (the Guide, a page, a launch), drops the pending hub; moving focus does not, and the hub of the tile that was chosen opens. A hidden game's card opens its hub too, where Play does nothing: a launch names a game by its place among the library's games, and a hidden one has none. My games & apps' grid opens it at once on A, and Play history after the same double tap; the Tile menu's Manage game opens it in the menu's place, so B there goes back to what the menu was over. Search's way in is its ticket's: it opens `pages::Open::GameHub(Entry::of(game))`.
- **The screen behind** (2.3) is a picture of what was under the hub the first time the hub is drawn: Home (or the page under it) drawn once more to a texture, halved down to an eighth each way and blurred there by 2 (`Ui::snapshot`). It has what Home showed then, the focused tile's ring included, and not the hint bar. It is kept while the hub is drawn and freed after. A picture that cannot be taken leaves the art in the strips.
- **The art behind the band** (2.1) is cut about its middle to 128 x 72 and blurred there by a Gaussian of deviation a third of the Bundle's radius (2, or 4 for a cover), then stretched over the screen (`Kind::Soft`). A cover is drawn 1.15 times the screen. For want of a cover the header stands in for it, as on Home. **Its reading starts at the tile's A** (#214), with the cover card's, while the double tap's 300 ms are waited out, so it is mostly there in the hub's first frame and is drawn at once. Art that comes after the first frame (a hub opened at once, from My games & apps or Search, or a large picture read slowly) **fades in over a quarter of a second** over black, with its darkening, as other art that arrives does (docs/ui.md, "Art"); the Bundle's has no fade but its 0.5 s crossfade under LB/RB.
- **Opening is timed in the log** (#214): every hub says, once it has come to rest, `hub: opened <game> in N ms: first frame N ms, last moving frame N ms after A`, then the frame that answered A, the double tap's wait, the page's data, what its first frame spent its time on, each picture (a cache hit, or how long it queued, decoded and uploaded, and when it came), the transition (none) and its longest frame. The game-hub payload fails when the first frame is done more than 100 ms (on a GPU) after the hub is put up. The double tap's 300 ms is the Bundle's and is kept: it is most of the time from A to the hub on Home's tiles.
- **The darkening gradient and the green** (2.2, 1.1) have three stops, where the renderer's gradients have two: each is a small picture painted once (`Kind::Painted`) and stretched over the screen.
- **A game with no cover** shows its name on the dark tile, as Home's tiles do, where the Bundle shows Playnite's default cover.
- **The source's mark** on the cover is the launcher's mark Home's tiles carry (home.md 3.3.1), the project's own drawing, on the Bundle's chip: the stores' marks are not ours to ship.
- **The date last played** is always month/day/year, as the Bundle's pictures have it, whatever the user's short date format.
- **The row of cards never needs its second place** (5.4): without Description, Game details fits the screen. The rule is built all the same (`cards::rects`).
- **Main action.** Play, Running, Install, Queued or Installing... Running shows from the moment the game is launched, there being no "Launching". Queued is a game in the Queue waiting its turn or paused, Installing... one downloading (#78); there is no Uninstalling: an uninstall asked for is Steam's own until the game reads uninstalled. The focus line names them `Queued` and `Installing`, and the Queue action `Queue`.
- **Queue.** A on it opens Manage Queue over the hub ([manage-queue.md](manage-queue.md)), so B there comes back to the hub with focus on Queue: the Bundle closes the hub and opens it again on the way back, which comes to the same.
- **Manage, and Menu anywhere in the hub,** open the game's Tile menu (`tile_menu::open(game, Opener::GameHub)`), without its Manage game row, over the hub; focus stays where it was, and B on the menu comes back to it. Its Customise row says "Customise Home" wherever the hub was opened from (section 7's "Customise games & apps" over My games & apps is not followed). The hint bar says "More options", as it does on Home's cards.
- **The Queue action** (6.4) is not drawn; its place is between Manage and the toggle (`actions::ALL`).
- **Sounds** (12). Every move of focus in the hub is the move sound, and a move into an edge is silent. The hub opens and closes in silence. **A on the toggle plays the move sound**, where the Bundle is silent: the Dashboard takes a press that changed where focus is said to be for a move. **A on Play plays the activation sound**, as A on any tile that chose something does; the Bundle plays none. A on a tile plays the activation sound once, at the first A, whether a hub or a double tap's launch follows.
- **`--log-focus`** says `focus: GameHub: <appid> <Hub|Details> <where>`, where is `Play`, `Running` or `Install` for the main action (what it is now), `Manage`, `Toggle`, `Cover`, `Stats`, `Info` or `Trailer`.
