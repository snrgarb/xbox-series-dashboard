# Search and the Console keyboard specification (as the Bundle draws them)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run.

Path shorthand used in citations:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`

All numbers are design units = px on a 1920x1080 canvas. **Nothing on Search, the Console keyboard or text entry is inside a Viewbox: every authored number is the on-screen number (scale 1).** Home's `s = 1.114` does not apply anywhere in this file. The focus ring and glow are the same resources as `home.md` section 4, drawn at 1:1.

This file covers three things that share one keyboard:

1. **Search**: the page (sections 1 to 6).
2. **Console keyboard**: the keys, their layers, their navigation (sections 7 to 9). Built once, used by both of the others.
3. **Text entry**: the keyboard under a caption and a field over the dimmed page (section 10). M3 needs it for Full library's name filter; M5 and M6 reuse it.

**Pictures of the real thing: none.** Every `artifacts/e2e/*/` folder's newest stamp was listed (`achievements-20261110-105905`, `background-follows-selection-20261110-111334`, `fresh-install-20261110-113436`, `guide-over-game-20261104-165208`, `see-all-20261110-113608`) and every `steps.txt` and `*.txt` beside the shots was searched for "search" and "keyboard": no shot shows Search, the Console keyboard or text entry. The two tests that would take one (`e2e/payloads/cover-fit/run.ps1:12,118-120`, shot `search`; `e2e/payloads/hub-panel/run.ps1:170-177`, shots `search` and `search-hub`) have no pictures on disk. **Every number below is derived from the source only; no picture confirmed any of them.** The only checks available were the repo's own tests, cited where they state a number.

**The console's own reference: none either.** `docs/research/xbox-dashboard-reference.md:140` records that "the Search page layout (keyboard, suggestions, result categories) is not described publicly", and line 193 lists it as a gap. The Bundle's XAML says its sizes and colours were "sized from the console's capture at 1080p" and "sampled from its capture, so approximate" (E/SearchControl.xaml:10,22; E/ConsoleKeyboardControl.xaml:13,18); that capture is not in the repo (`docs/reference/` holds only `startup/` and `notifications/`).

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite, WPF or font internals that are not in this repository.

---

## 1. Search: page structure

### 1.1 Where it sits

Search is one full-screen, **opaque** control (`XboxSeries_Search`, `Grid.RowSpan="2"` of `PART_ViewHost`, so 1920 x 1080) drawn over Home (T/Views/Main.xaml:443-444). While it is up:

- Home's hero, top bar, Recent row and Groups are collapsed (`XboxHomeOnly`, T/Views/Main.xaml:306-319, 390-391, 474, 753-756). The hero scrim rectangle (T/Views/Main.xaml:392) is still there but under the opaque page, so it is not seen.
- My games & apps and Settings are never up with it: opening Search closes both, and opening either closes Search (C/ExtensionCore.Search.cs:38-39; C/ExtensionCore.cs:1511, 1695; T/Views/Main.xaml:11-13).
- Over it, in this order: the Game hub, the hint bar, the Tile menu, game status, text entry (T/Views/Main.xaml:761-781). The Guide is a window of its own, over everything.

Shown and hidden by `Visibility` bound to `SearchState.IsShown` (E/SearchControl.xaml:207): **instantly** (section 6).

### 1.2 Background

A linear gradient over the whole 1920 x 1080, `StartPoint="0,0" EndPoint="0.6,1"` in relative units, i.e. from (0, 0) to (1152, 1080) (E/SearchControl.xaml:209-215):

| Offset | Colour |
|---|---|
| 0 | `#FF123A2C` |
| 0.45 | `#FF07140F` |
| 0.8 | `#FF000000` |

Beyond offset 0.8 it stays black (WPF pads). The gradient's parameter at pixel (x, y) is `t = (1152 x + 1080 y) / (1152^2 + 1080^2)`: dark green at the top left, black towards the bottom right. It does not move, and nothing of Home shows through.

### 1.3 The three rows of the page grid

Rows `Auto`, `*`, `Auto` (E/SearchControl.xaml:216-220).

| Row | Holds | Height |
|---|---|---|
| 0 | The search box: 54 tall under a 77 top margin | 131 |
| 1 | The results (a vertical ScrollViewer) | what is left |
| 2 | The Console keyboard: 260 tall, margin 20 above and 110 below, while it is up; **nothing (0) while it is put away** (collapsed) | 390 or 0 |

Resolved rectangles:

| Element | Authored | On screen (x, y, w, h) | Source |
|---|---|---|---|
| Search box | 960 x 54, `Margin="0,77,0,0"`, centred | x 480..1440, y 77..131 | E/SearchControl.xaml:226-227 |
| Results viewport, keyboard up | row 1 less `Margin="0,36,0,0"` | x 0..1920, **y 167..690** (h 523) | E/SearchControl.xaml:244-245; 1080 - 390 = 690 |
| Results viewport, keyboard away | same | x 0..1920, **y 167..1080** (h 913) | same; row 2 is 0 |
| Results content | `StackPanel Margin="74,0,74,24"` | x 74..1846 (w 1772), from y 167, 24 clear under the last row | E/SearchControl.xaml:246 |
| Console keyboard | 928 x 260, `Margin="0,20,0,110"`, centred | **x 496..1424, y 710..970** | E/SearchControl.xaml:266; E/ConsoleKeyboardControl.xaml:74-92 (5 x 52; 114 + 10 x 70 + 114 = 928) |
| Foot | 110 under the keyboard | y 970..1080, empty | E/SearchControl.xaml:266 |

The keyboard does not slide over the results: the results viewport is simply shorter while it is up, and whatever does not fit is cut off at y 690 (the ScrollViewer clips) and scrolled to as focus moves (section 3.6).

---

## 2. The search box

`Button` with style `SearchBox` (E/SearchControl.xaml:46-70, 226-238). It is focusable; it is **not** a text box (it only shows the keyboard's text).

| Part | Value | Lines |
|---|---|---|
| Shape | 960 x 54 pill: `CornerRadius="27"` | 52, 226 |
| Fill, rest | `#FF2E3136` (`BoxBrush`) | 23, 52 |
| Fill, focused | `#FF44474E` | 63 |
| Content inset | `Margin="28,0"`, vertically centred: text area x 508..1412 (w 904) | 53 |
| Placeholder | "Search for games and apps", Segoe UI 26, `#FFB4B7BD` (`DimBrush`), `Margin="4,0,0,0"` (starts x 512), character ellipsis. Shown only while the text is empty (`text.Length == 0`, so a typed space hides it). | 229-230; E/SearchControl.xaml.cs:163 |
| Text | Segoe UI 26, `#FFFFFFFF`, one line, **no trimming**, in two runs: before the caret and after it | 27-32, 232, 235; E/SearchControl.xaml.cs:157-164 |
| Caret | A 2 x 30 white rectangle between the two runs, `Margin="1,0"` (1 clear each side, so the caret takes 4 of width), vertically centred. **Shown only while the keyboard is up** (`IsKeyboardShown`); with the keyboard away the two runs close up. **It does not blink** (no animation: NOT FOUND). | 233-234 |
| Focus ring | `Border Margin="-8" BorderThickness="4" CornerRadius="35"`, brush `#FF2CA243`, no fill: a 4-thick stroke 4 clear of the pill (ring box x 472..1448, y 69..139). Hidden unless the box has keyboard focus. | 54-59, 64 |
| Glow | `XboxFocusGlow`, the **rectangular** variant of `home.md` 4.3 (ten bands of corner radius 12 and four corner spots), inside that ring at 1:1. Its corners are therefore squarer than the pill's round ends: see trap 12.3. | 56-58; T/Constants.xaml:26-103 |

Vertical placement (derived): Segoe UI's line is 1.33 em (the Bundle's own figure, E/SearchControl.xaml:159), so a 26 px line is 34.58 tall, centred in the 54: text line y 86.71..121.29; caret y 89..119. Segoe's metric is **UNVERIFIED** here as in `home.md`.

With nothing typed and the keyboard up, the caret (x 509..511) stands just left of the placeholder (x 512).

Text longer than the 904 px text area: nothing scrolls. The runs are laid out left to right from x 508 and what passes x 1412 is not drawn (WPF clips a child that does not fit its slot: **UNVERIFIED**, by WPF's layout clip, not by anything written in the Bundle). The caret goes out of sight with it. **There is no maximum length** (NOT FOUND in C/ConsoleKeyboard.cs: `Insert` has no limit, lines 183-187).

Buttons on the search box (E/SearchControl.xaml.cs:183-195, 271-280):

| Input | Does |
|---|---|
| A | Brings the keyboard up again (`ShowSearchKeyboard`), the text and the cursor as they were; focus goes to the key last focused (section 9.1). Nothing if it is already up, other than moving focus to the keys. C/ExtensionCore.Search.cs:86-92; CT/SearchTests.cs:498 |
| Down | The first row showing, on the tile that row last had focus on (else its first); with no row showing and the keyboard up, the keyboard's key; else nothing |
| Up, Left, Right | Nothing (handled, focus stays) |
| B, X, Y, LB, RB, LS, Start | As anywhere on the page (section 5) |

---

## 3. Results

### 3.1 The three rows

A vertical stack inside the results ScrollViewer, each row a heading over a horizontal list of tiles, each **collapsed while it has nothing to show** (E/SearchControl.xaml:246-262):

| Row | Heading | Shows when | Tiles | Lines |
|---|---|---|---|---|
| Recently added | "Recently added" | **Nothing typed** (the text, trimmed, is empty). It shows even when the library has no games: then the heading stands alone. | Game tiles, up to 12 | 247-251; C/SearchState.cs:87, 96-103 |
| Games | "Games" | Something typed **and** at least one game found | Game tiles, no limit | 252-256; C/SearchState.cs:90 |
| Apps (M4) | "Apps" | Something typed **and** at least one App found (never when Apps are unavailable) | App tiles, no limit | 257-261; C/SearchState.cs:93; C/ExtensionCore.Search.cs:265-267 |

So the page is in one of three states: **empty** (Recently added alone), **found** (Games, Apps, or both, Games above Apps), **nothing found** (no row at all, section 3.8).

### 3.2 Heading

`HeaderText` (E/SearchControl.xaml:34-40): **Bahnschrift**, 30 px, `FontWeight="350"` (the variable font's SemiLight), `#FFFFFFFF`, one line, character ellipsis, `Margin="0,0,0,20"`, left edge x 74. Line height about 1.2 em = 36 (the Bundle's own assumption for Bahnschrift, CT/HomePageXamlTests.cs:59; the real metric is **UNVERIFIED**). The numbers below use 36.

### 3.3 Row geometry

The list is a `ListBox` with style `TileRow` (E/SearchControl.xaml:79-112) whose width is set in code to show **whole tiles only, 7 at most** (C/SearchRowWidth.cs:12-27; E/SearchControl.xaml.cs:506-517):

    TileWidth = 240, Gap = 15, MostTiles = 7, Gutter = 24
    available  = 1920 - 74 - 74 = 1772                       (the row's width)
    TilesShown = max(1, min(7, floor((available + 15) / 255))) = floor(1787 / 255) = 7
    ListWidth  = 24 + TilesShown x 255 = 24 + 1785 = 1809

At 1920 wide it is exactly 7 (7 x 240 + 6 x 15 = 1770 of the 1772). A narrower screen (the page is 1080 design units tall and as wide as the aspect makes it) shows fewer: 1440 wide (4:3) gives available 1292, 5 tiles.

Resolved, at 1920:

| Thing | Authored | On screen | Source |
|---|---|---|---|
| ListBox | `Margin="-24,-24,-24,6"`, left-aligned, width 1809 | x 50..1859 | E/SearchControl.xaml:80-81 |
| Its ScrollViewer (the horizontal clip) | `Margin="15,0,0,0"` | **x 65..1859** (w 1794) | E/SearchControl.xaml:106 |
| Tiles panel | `ItemsPresenter Margin="9,24,0,24"` | first tile's left edge x **74** (65 + 9); 24 clear above and below the tiles inside the clip | E/SearchControl.xaml:107 |
| Tile | Width 240, `Margin="0,0,15,0"`, top-aligned | pitch 255; tile i at x = 74 + 255 i: 74, 329, 584, 839, 1094, 1349, 1604 (right edge of the 7th: 1844) | E/SearchControl.xaml:120-122 |
| Right clip | the list ends where the 8th tile begins | x 1859 = 1844 + 15 | C/SearchRowWidth.cs:23-27 |

So the first tile lines up with the heading at x 74, the 8th tile is cut off wholly, and the clip stands 9 left of the first tile and 15 right of the last: the 8-out focus ring fits on both sides; the glow, which reaches 21.5 out (`home.md` 4.3), is **cut off by the clip at 9 on the left and 15 on the right** of a row's end tiles, and by nothing above and below (24 of room).

Vertical extent of a row, from the row's top (derived from the margins above):

| Thing | Game row (Recently added, Games) | Apps row (M4) |
|---|---|---|
| Heading | 0..36 | 0..36 |
| Tiles | **56..428** (h 372) | **56..296** (h 240) |
| Row ends (next row's heading starts) | **458** | **326** |

Arithmetic: heading 36 + margin 20 = 56; the list's -24 top margin and the panel's +24 cancel, so tiles start at 56. After the tiles: 24 (panel) + 6 (list's bottom margin) = 30. So 56 + 372 + 30 = 458, and 56 + 240 + 30 = 326. Rows follow each other with nothing else between them; 24 under the last (E/SearchControl.xaml:246).

On screen at scroll 0 (results viewport top y 167):

| State | Element | y |
|---|---|---|
| Empty, or found with Games first | Row 1 heading | 167..203 |
| | Row 1 tiles: cover | 223..463 |
| | Row 1 tiles: caption | 463..595 |
| Found, Games and Apps both | "Apps" heading | 625..661 |
| | App tiles | 681..921 |
| Found, Apps only | "Apps" heading 167..203, App tiles 223..463 | |

With the keyboard up (viewport to y 690) a Games row fits whole (ends 595 + ring 8 = 603); the Apps row under it shows its heading and the top 9 px of its tiles. With the keyboard away (viewport to y 1080) both rows fit whole: content 458 + 326 + 24 = 808 of the 913.

### 3.4 Game tile

`Tile` container (E/SearchControl.xaml:118-153) with the `GameTile` face (163-194). 240 x 372, **always the same size: no growth, no title band, the caption is always showing.**

| Part | Value | Lines |
|---|---|---|
| Clip | Rounded rect radius 6 (`RoundedClip.Radius="6"`) round the whole 240 x 372 | 130 |
| Cover area | 240 x 240, background `#FF1A1A1A` | 166, 169 |
| Cover | `TileArt`: `UniformToFill`, centred across, **top-aligned**, no bands, Fant scaling; decoded at **480 px wide** | 170-171; E/HomeTiles.xaml:135-148 |
| No cover, or one that will not decode | The game's title in the cover area: Segoe UI 26, white, wrapping, centred both ways, `Margin="12"`, no trimming | 173-185 |
| Caption | 240 x 132, background `#FF3A3D44` (`CaptionBrush`); content `Margin="18,14,18,14"` (text width 204) | 24, 164, 167, 187 |
| Title | Segoe UI 26, `#FFFFFFFF`, wraps, `MaxHeight="72"` = two lines (2 x 34.58 = 69.2), character ellipsis on the second | 188-189; comment 159-161; CT/ConsoleTypeSizesXamlTests.cs:80-125 |
| Status (M4) | Under the title, Segoe UI 26, `#FFB4B7BD`, one line, character ellipsis: "Queued" or "Installing..." while the game is queued or installing, else empty (the line then takes no room). | 190-191; C/HomeState.cs:168-169; C/ExtensionCore.Queue.cs:15-16 |
| Focus ring | `Border BorderThickness="4" CornerRadius="14" Margin="-8"`, `#FF2CA243`, with `XboxFocusGlow`: exactly the rectangular ring and glow of `home.md` 4.1 and 4.3, at scale 1 (stroke 4, gap 4, centre-line radius 14, outer 16). Hidden unless focused; toggled instantly. | 133-138, 141-143 |
| Draw order | The focused tile is drawn over its neighbours (`Panel.ZIndex=1`), so they do not cover its glow | 148-152 |

The caption's 132 is 14 + 69.2 + 34.6 + 14 (two title lines, one status line), so a one-line title with no status leaves the caption's foot empty: the caption does not shrink.

**Not on a Search tile** (the `GameTile` template has none of them): the source badge, the Running badge, "CONTINUE", a title band. The core marks a found game as running (CT/SearchTests.cs:340) but this page's template does not draw it.

### 3.5 App tile (M4)

`AppTile` = the same container, `Height="240"` (E/SearchControl.xaml:197-199), with the `AppTileArt` face shared with My games & apps' Apps (E/AppTiles.xaml:16-79). 240 x 240, same clip radius 6, same ring and glow. For M3, leave the row's slot (section 3.3); the face in brief: a white tile (the accent `#FF107C10` for a Microsoft Store app) with the App's icon inset 18 over a soft shadow (edge to edge for a Store app), glyph U+ECAA in the icon font when it has no icon, and, **unlike the game tile**, a `#E6000000` title band at the foot holding the name (Segoe UI 26, white, wrap, `MaxHeight="72"`, `Margin="8"`) that fades in on focus (to 1 over 150 ms after 50 ms; out over 100 ms; linear) (E/AppTiles.xaml:18-46, 49-73). Full detail belongs to the Apps specification.

### 3.6 Scrolling

**Sideways, along a row** (E/SlidingRow.cs, whole file; attached at E/SearchControl.xaml:82):

- A row longer than what fits slides to keep the focused tile wholly in view, and always **rests a whole number of tiles along** (offsets are multiples of 255).
- Going right: the row slides until the end of the focused tile's slot (its right edge + its 15 margin) meets the clip's right edge (x 1859). Going left: until the tile's left edge is 9 in from the clip's left edge (x 74). A tile already wholly in view moves nothing. Clamped to the row's ends. E/SlidingRow.cs:205-232.
- So with 7 showing, Right from the 7th tile slides one tile (255) and the 8th takes the 7th place; Left from the tile in first place slides one tile back.
- Slide: **300 ms, `CubicEase EaseOut`**, from where the row was when the button was pressed (E/SlidingRow.cs:26, 234-251, 258-272). A new move while sliding retargets from the current position. **Reduce motion: the row is there at once** (E/SlidingRow.cs:243).
- A new list in the row (the text changed) starts at offset 0 with no slide left over (E/SlidingRow.cs:278-285).
- No scroll bar, no arrows, no fade at the cut edge (`HorizontalScrollBarVisibility="Hidden"`; nothing else: NOT FOUND). Tiles off screen are not built until needed (virtualised; a port may ignore this).

**Up and down, between rows**: the results ScrollViewer (E/SearchControl.xaml:244-245; no scroll bar) is moved only by the focused tile asking to be brought into view (E/SlidingRow.cs:136-141). That is WPF's own minimal scroll, **with no animation** (a jump), and its exact result is **UNVERIFIED**. What it asks for, derived: the smallest scroll that puts the focused tile's own rectangle (not its ring, not its row's heading) inside the viewport. With Games and Apps both found and the keyboard up, that gives:

- Down onto an App tile (y 681..921 at scroll 0, viewport foot 690): scroll by 231, so the App tiles sit at y 450..690 with their foot on the clip (the ring's bottom 8 px cut off), and the Games row's tiles at y -8..364, cut by the viewport's top at 167.
- Up again onto a Games tile: scroll back to 56, so the Games tiles' top is exactly on the viewport's top (y 167..539) and the "Games" heading is scrolled off.

The scroll range there is 0..285 (content 808 - viewport 523). With the keyboard away nothing scrolls. This is open question 3.

### 3.7 How the rows follow the text

C/ExtensionCore.Search.cs:244-322.

- **When:** on every key that edits the text, at once and synchronously: a key of the Console keyboard, a pad shortcut, a letter or Backspace on a physical keyboard (lines 117, 162, 180). **No debounce, no delay, no minimum length** (NOT FOUND). Also whenever the library changes while the page is up (C/ExtensionCore.cs:300, 1377; CT/SearchTests.cs:326).
- **The text matched** is the keyboard's text with spaces trimmed from both ends; only spaces typed counts as nothing typed (line 251; CT/SearchTests.cs:222-234).
- **Never hidden games** (lines 256, 264). Games that are not installed are found like any other (CT/SearchTests.cs:202, 215-216).
- **Rule:** a game is found when its **name** contains the text anywhere (substring), compared culture-aware with **case ignored and accents ignored** (`CompareOptions.IgnoreCase | IgnoreNonSpace`: "pokemon" finds "Pokémon", "HALO" finds "Halo Infinite", "izon" finds "Forza Horizon 5") (lines 19, 301-311; CT/SearchTests.cs:197-219). The sorting name is not searched, only sorted by.
- **Order:** by rank, then by sorting name (current culture, case ignored) (lines 289-298):
  - rank 0: the name **starts with** the text;
  - rank 1: a **word** in the name starts with it (a letter or digit that follows a character that is neither, lines 313-319);
  - rank 2: the text is elsewhere in the name.
  - Example, typing "halo" (CT/SearchTests.cs:237-254): "Halo Infinite", "Halo Wars" (rank 0); "The Halo Collection" (sorting name "Halo Collection"), "Super Halo" (rank 1); "Ahalo", "Unhalo" (rank 2).
- **Limits:** none on Games or Apps (NOT FOUND). Recently added is 12 (line 16).
- **Apps (M4)** are found by the same rule on their name and ordered by rank then name (lines 265-267).
- **Recently added** (nothing typed): games not hidden, those with a date added first, newest first, the first **12**; games with no date added come after, in the library's order (lines 253-261; CT/SearchTests.cs:161-194).
- A row is replaced only when what it shows changed (same games, each reading the same), so typing a letter that leaves the Games row as it was rebuilds nothing and moves no focus (lines 269-282, 325-326; CT/SearchTests.cs:353).

When a row is replaced: it starts again at its first tile (offset 0), each row's remembered tile is forgotten, and, **only if focus was on a tile** (not on the keyboard or the search box), focus is placed again: on the keyboard's key if the keyboard is up, else on the first row's first tile, else on the search box (E/SearchControl.xaml.cs:132-144, 403-429). While typing on the keys, focus never leaves the keys.

### 3.8 Empty, nothing found, loading, errors

- **Empty** (page just opened, or the text deleted): "Recently added" and up to 12 tiles. An empty library leaves the heading alone over nothing.
- **Nothing found:** both rows collapse and **nothing is drawn in their place: there is no "No results" text, no picture** (NOT FOUND in E/SearchControl.xaml). With the keyboard away, focus is on the search box.
- **Loading:** none. The rows are worked out synchronously; a cover is decoded as its tile is built and simply appears (no fade, no shimmer; the cover area is `#FF1A1A1A` until then), as on Home (`home.md` 10.3).
- **Errors:** a cover that will not decode shows the title in its place (section 3.4). Nothing else can fail on the page.
- Playnite's progress indicator (`home.md` 10.1) is meant to show at the top right over Search too (`XboxPageProgress`, `Margin="0,84,105,0"`, T/Views/Main.xaml:401-441), but that panel is declared **before** the opaque Search page in the same grid (line 405 against 444), so by WPF's draw order the page covers it. Whether it is ever seen on Search is **UNVERIFIED**; a port has no such indicator to draw.

---

## 4. Opening Search, and where focus is

### 4.1 Every time it opens

`ShowSearch` (C/ExtensionCore.Search.cs:31-45; CT/SearchTests.cs:30-61): **empty, every time**: nothing typed, cursor 0, Shift on, the letters layer, the keyboard up, Recently added showing. Focus goes to the keyboard's first letter, **Q** (E/SearchControl.xaml.cs:113-121; E/ConsoleKeyboardControl.xaml.cs:68-89). Nothing is remembered from the last visit: not the text, not the key, not a row's tile.

Home's focused tile is told it lost focus first, so the pad's A and Menu cannot reach it under the page (lines 33-36; CT/SearchTests.cs:842-856).

### 4.2 Entry points and where B returns

| Opened by | Origin | Guard | B (keyboard already away) goes to | Source |
|---|---|---|---|---|
| A on Home's top bar Search button | `TopBar` | - | Home, focus on the **top bar's Search button** | T/Views/Main.xaml:540-541; C/ExtensionCore.Search.cs:24; E/SearchControl.xaml.cs:381-387, 464-473 |
| **Y anywhere on Home** (any focus: top bar, Recent row, wide tiles, Groups), also the Y key of a keyboard | `Home` | Not with My games & apps, Settings or Search in Home's place; not with text entry, a Tile menu or prompt, or the Guide over it | Home; Home's own rows take focus back as they reappear: the Recent row / wide tile last focused if the page rests above the Groups, else the Group card last focused. Y pressed from a top bar button therefore comes back on the Recent row, not on that button. | E/PlayniteViewHost.cs:29-32; C/ExtensionCore.Search.cs:65-80; E/RecentRowControl.xaml.cs:249-256; E/GroupsControl.xaml.cs:136-142; CT/SearchTests.cs:669-768 |
| **Y on a game in My games & apps' Games grid** | `MyGames` | Only on Games (not Full library, where Y opens the name box, nor Play history, where it does nothing); not with text entry or the Guide up | **My games & apps on Games, focus on the game Y was pressed on** | E/MyGamesControl.xaml.cs:704-729, 1496-1505; C/ExtensionCore.Search.cs:52-59, 77, 228-232; CT/SearchTests.cs:608-667 |
| The **Guide's Search Utility** (third of Notifications, Audio, Search; glyph U+E721) | `Guide` | On A's **release**: the Guide closes, the Dashboard comes to the front on Search, replacing Settings or My games & apps if up | Home, as for Y on Home | C/ExtensionCore.cs:2145-2150, 2566-2571; E/OverlayWindow.xaml:1138-1141; CT/SearchTests.cs:99-119, 770 |

Y while Search is already up does not reopen it (C/ExtensionCore.Search.cs:74).

### 4.3 What takes Search away without B

All of them drop the page with no trace (the text is not kept): the Guide's Home; the Guide's My games & apps; opening My games & apps or Settings by any route; a game exiting (Home comes back on that game's Recent tile) (C/ExtensionCore.cs:649, 1511, 1695, 2116-2129; CT/SearchTests.cs:85-159).

---

## 5. Search: buttons and navigation

### 5.1 Focus stops

Three kinds: the search box; the tiles of each row showing; the keys (only while the keyboard is up). Order top to bottom: search box, row 1, row 2, keyboard.

| From | Up | Down | Left / Right |
|---|---|---|---|
| Search box | nothing | first row showing (its remembered tile, else its first); no rows: the keyboard's key if up; else nothing | nothing |
| Tile in a row | the row above (its remembered tile, else its first); from the top row: the search box | the row below (remembered tile, else first); from the last row: the keyboard's key if it is up, else nothing | next tile along the row; **nothing at either end, no wrap** |
| Key on the keyboard's top row | the **last** row showing (remembered tile, else first); no rows: the search box | (within the keyboard, section 9) | (within the keyboard) |
| Other keys | (within the keyboard; Down from the bottom row does nothing) | | |

Source: E/SearchControl.xaml.cs:173-180, 243-318; E/ConsoleKeyboardControl.xaml.cs:277-281.

Moving between rows does **not** keep the column: each row remembers the index of the tile it last had focus on (`lastIndex`), and a row not yet visited gives its first tile. The memory is cleared when the page opens and whenever any row is replaced (E/SearchControl.xaml.cs:33, 118, 137, 197-204, 447-458).

Held D-pad: repeated by Playnite, every 80 ms by the core's own account (C/ExtensionCore.cs:45-46); the repeat itself is Playnite's: **UNVERIFIED**.

### 5.2 Buttons

| Input | Keyboard up | Keyboard away | Source |
|---|---|---|---|
| A on a key | Presses it (section 8) | - | E/ConsoleKeyboardControl.xaml.cs:193-205 |
| A on the search box | (focus to the keys) | Keyboard up again | section 2 |
| **A on a game tile** | Opens the game's **Game hub at once**: no 300 ms wait, no double tap (a second A is the hub's own business and plays nothing here). Search stays up behind the hub. Works with the keyboard up or away. | same | C/ExtensionCore.Search.cs:190-197; CT/SearchTests.cs:798-838 |
| **A on an App tile (M4)** | Starts the App with its arguments, with no Launch screen, whatever game is running; the page stays as it is | same | C/ExtensionCore.Search.cs:203-210; CT/SearchTests.cs:1002-1015 |
| **B** | **Puts the keyboard away**; the text stays; focus goes to the first row's first tile, or to the search box when no row shows | **Leaves the page** to where it was opened from (section 4.2) | C/ExtensionCore.Search.cs:217-237; E/SearchControl.xaml.cs:128-131, 374-388, 417-427; CT/SearchTests.cs:590-606 |
| X | Backspace | nothing (swallowed) | C/ConsoleKeyboard.cs:165-180; E/SearchControl.xaml.cs:325-355 |
| Y | Space | nothing (swallowed; it does **not** reopen or reset Search) | same |
| LB / RB | Cursor left / right one character | nothing (swallowed) | same |
| Left stick click | Shift | nothing | same |
| Menu (Start) | Enter: **puts the keyboard away**, focus to the first tile found (or the search box) | **Nothing**: there is no Tile menu on a Search result (the page never tells the core a tile has focus, so Menu has nothing to open) | C/ExtensionCore.Search.cs:112-115; C/ExtensionCore.cs:840-843, 869; CT/SearchTests.cs:485, 842-856 |
| LT | The symbols key, **only while a key has focus** | nothing | E/ConsoleKeyboardControl.xaml.cs:219-234 |
| RT | nothing | nothing | NOT FOUND |
| Right stick click | nothing (swallowed) | nothing (swallowed) | E/SearchControl.xaml.cs:350-354 |
| View (Back) | Not handled by the page. Playnite binds Back to its own menu on the view host (T/Views/Main.xaml:373); whether that menu opens over Search is **UNVERIFIED**. | same | - |
| Guide | Opens the Guide over the page; while it is up it has the pad and the page hears nothing | same | C/ExtensionCore.Search.cs:332; CT/SearchTests.cs:783 |

The pad shortcuts X, Y, LB, RB, LS and Start act **wherever focus is on the page** while the keyboard is up (on a key, on a result tile, on the search box): they are taken at the page, not at the keys (E/SearchControl.xaml.cs:325-348). LT is taken at the keys only.

Enter, Start and B all put the keyboard away the same way; the difference is only that B, pressed again, leaves.

### 5.3 The Game hub from Search, and back

A on a game opens its hub over the page (the hub's own specification says how). **B in the hub closes it and focus returns to that game's tile on Search** (E/SearchControl.xaml.cs:36, 228-229, 390-397, 403-444):

1. the tile of that game, in the first row that still holds it (Recently added or Games), the row slid to show it;
2. if the game is no longer in a row: the keyboard's key if the keyboard is up; else the first row's first tile; else the search box.

The keyboard is as it was left (it can still be up if the game was chosen by going Up from the keys). The rows are not rebuilt while a hub is open over the page (E/SearchControl.xaml.cs:136-142).

---

## 6. Search: transitions, Reduce motion, sounds, hint bar

**Opening and closing: there is no transition.** The page appears and disappears in one frame (`Visibility`), from and to Home, My games & apps, Settings and the Guide alike; Home under it is simply collapsed and shown again where it was left (E/SearchControl.xaml:207; T/Views/Main.xaml:306-319). Home's fly-up reveal (`home.md` 10.2) runs only after Start-up, not on coming back from Search (E/RecentRowControl.xaml.cs:169-181). The keyboard too comes up and goes away in one frame, the results area changing height with it. Fade, slide or scale of the page or the keyboard: **NOT FOUND**.

Everything that moves on Search:

| Thing | Normal | Reduce motion (E/Motion.cs:46-50) |
|---|---|---|
| A row sliding sideways | 300 ms `CubicEase EaseOut` | Instant (E/SlidingRow.cs:243) |
| Results moving up/down between rows | Instant | Instant |
| App tile's title band (M4) | 150 ms in after 50 ms, 100 ms out | Unchanged (a fade) |
| Focus ring, key colours, caret, page, keyboard | Instant | Instant |

**Sounds.** The extension plays **no sound of its own** anywhere on Search, the keyboard or text entry: no `sounds.Play` call in C/ExtensionCore.Search.cs, C/ExtensionCore.TextEntry.cs, C/ConsoleKeyboard.cs, E/SearchControl.xaml.cs, E/ConsoleKeyboardControl.xaml.cs, E/TextEntryControl.xaml.cs or E/SlidingRow.cs (searched). No open sound, no close sound, no key-press sound, no bump at an edge. What is heard is Playnite's own: `navigation.wav` (= `Navigate1.wav`) as focus lands on its "buttons and lists" (E/PlayniteSounds.cs:69-75), and the search box, every key and every tile are buttons or list items, so each focus move on the page, key to key included, should sound it: **UNVERIFIED** (Playnite's behaviour). Whether Playnite sounds `activation.wav` when the pad's A presses a key or chooses a tile is **UNVERIFIED** too (the A arrives as a key event the page handles, not as a click). See open question 5.

**Hint bar: empty on Search**, in every state. The hints need a tile the core knows has focus, a game in Playnite's grid, or the Game hub (C/ExtensionCore.Hints.cs:74-91), and Search gives none of them. The keys carry their own pad glyphs instead (section 7.3). With a Game hub open over Search the bar shows "More options" alone, without "Y Search" (line 87: `!gameHubFocused`).

---

## 7. Console keyboard: layout and drawing

The model is C/ConsoleKeyboard.cs (the text, the cursor, Shift, the layer, the keys); the drawing is E/ConsoleKeyboardControl.xaml(.cs). The same control, at the same size and the same place on screen, serves Search and text entry.

### 7.1 Grid

A grid of **5 rows by 12 columns** (C/ConsoleKeyboard.cs:9-12, 316-338; E/ConsoleKeyboardControl.xaml:74-92):

- Rows: 5 x **52**. Total height **260**.
- Columns: **114**, then 10 x **70**, then **114**. Total width **928**.
- Each key fills its cells less **2 all round** (`Margin="2"`), so keys are **4 apart** and the board's outer 2 px is empty. Corner radius **2**. E/ConsoleKeyboardControl.xaml:33, 38.
- No panel, plate or background behind the keys: the page (or text entry's dim) shows between them.
- On screen, on Search and in text entry alike: the grid is **x 496..1424, y 710..970** (sections 1.3, 10.2). Column 0 is x 496..610; column k (1..10) is x 610 + 70 (k - 1) .. 610 + 70 k; column 11 is x 1310..1424. Row r is y 710 + 52 r .. 762 + 52 r.

There are **47 keys**: 12 on row 0; 10 letters on each of rows 1, 2 and 3; the two cursor keys, each spanning rows 1 and 2; Shift and Enter, each spanning rows 3 and 4; and the space bar across the middle of row 4 (1 + 10 + 1 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 1 = 47). In reading order, as the model lists them (C/ConsoleKeyboard.cs:318-337):

| Key | Kind | Row | Column | Spans | Key face on screen (x, y, w, h) |
|---|---|---|---|---|---|
| Symbols | `Symbols` | 0 | 0 | 1 x 1 | 498, 712, 110, 48 |
| Digits 1 2 3 4 5 6 7 8 9 0 | `Character` | 0 | 1..10 | 1 x 1 | 612 + 70 (k - 1), 712, 66, 48 |
| Backspace | `Backspace` | 0 | 11 | 1 x 1 | 1312, 712, 110, 48 |
| Cursor left | `Left` | 1 | 0 | **2 rows** | 498, 764, 110, **100** |
| Letter row 1 | `Character` | 1 | 1..10 | 1 x 1 | 612 + 70 (k - 1), 764, 66, 48 |
| Cursor right | `Right` | 1 | 11 | **2 rows** | 1312, 764, 110, 100 |
| Letter row 2 | `Character` | 2 | 1..10 | 1 x 1 | 612 + 70 (k - 1), 816, 66, 48 |
| Shift | `Shift` | 3 | 0 | **2 rows** | 498, 868, 110, 100 |
| Letter row 3 | `Character` | 3 | 1..10 | 1 x 1 | 612 + 70 (k - 1), 868, 66, 48 |
| Enter | `Enter` | 3 | 11 | **2 rows** | 1312, 868, 110, 100 |
| Space bar | `Space` | 4 | 1 | **10 columns** | 612, 920, **696**, 48 |

(Key face = cells less 2: a letter key 70 - 4 = 66 by 52 - 4 = 48; a side key 114 - 4 = 110; a two-row key 104 - 4 = 100 tall; the space bar 700 - 4 = 696.)

### 7.2 Key colours

E/ConsoleKeyboardControl.xaml:19-24, 31-63. All opaque, "sampled from the console's capture".

| State | Character keys and the space bar | Side keys (symbols, backspace, cursor left/right, Shift, Enter) |
|---|---|---|
| Rest | `#FF2F3238` (`KeyBrush`) | `#FF3A3D44` (`SideKeyBrush`), a shade lighter |
| Focused | `#FF555963` (`FocusedKeyBrush`) | same |
| Held down (the pad's A on the focused key; the pad button the key carries; a mouse press) | `#FF7A7E88` (`PressedKeyBrush`), for as long as the button is held; wins over focused | same |
| Latched / active (Shift on, symbols showing) | **No colour of its own: NOT FOUND.** Shift on shows only as capitals on the letter keys; the symbols layer shows only as the symbols key reading "abc". **The port lights Shift while it is on** (decided in #54): the Shift key takes the focused key's `#FF555963`, with no ring, on either layer, and goes back to the side keys' grey as Shift goes off. The symbols key is not lit. | - |
| Disabled | No key is ever disabled: NOT FOUND | - |

All changes are instant. "Held" is drawn only while focus is on the board: a pad button a key carries lights that key until it is let go, whichever key has focus, and everything held is let go when focus leaves the board (E/ConsoleKeyboardControl.xaml.cs:35, 146-185, 219-246).

### 7.3 Key faces

Every face is centred in its key (E/ConsoleKeyboardControl.xaml:39, 107-125). Text: **Segoe UI 26, white**. Glyphs: the icon font (`Segoe Fluent Icons, Segoe MDL2 Assets`, T/Constants.xaml:232) at **26, white**. Each key the pad has a shortcut for also carries the pad button, a small outlined badge: by default 24 x 24, radius 12, 1.5 stroke `#FFB4B7BD`, no fill, text Segoe UI `#FFB4B7BD` centred in it.

| Key | Face, left to right or top to bottom | Badge | Lines |
|---|---|---|---|
| Character | Its character (section 8) | none | 111-112 |
| Symbols | Text "#+=" (letters layer) or "abc" (symbols layer), 26; then, 12 to its right, the badge | **LT**: 28 x 22, corner radii 9, 9, 3, 3 (top-left, top-right, bottom-right, bottom-left), text "LT" at 10 | 133-141; C/ConsoleKeyboard.cs:228 |
| Backspace | Glyph **U+E750**; 12 to its right, the badge | **X**: round 24, stroke and letter **`#FF3F8EF0`**, "X" at 14 | 142-150 |
| Cursor left | Stacked: glyph **U+E76B** (chevron left); 16 under it, the badge | **LB**: 32 x 18, radius 5, "LB" at 10 | 151-162 |
| Cursor right | Stacked: glyph **U+E76C** (chevron right); 16 under it, the badge | **RB**: 32 x 18, radius 5, "RB" at 10 | 163-174 |
| Shift | Stacked: glyph **U+E74A** (arrow up); 14 under it, the badge | **LS**: round 24, "LS" at 10 | 175-183 |
| Enter | Stacked: glyph **U+E72A** (arrow right); 14 under it, the badge | **Menu**: round 24, holding glyph U+E700 (three lines) in the icon font at 11 | 184-194 |
| Space bar | The mark: an open box `M0,0 L0,6 L24,6 L24,0` (24 wide, 6 tall, open at the top), white stroke 2, no fill, pushed down by a 10 top margin; 12 to its right, the badge | **Y**: round 24, stroke and letter **`#FFFFB900`**, "Y" at 14 | 116-117, 195-203 |

The badges are part of the key at all times (not only when a pad is connected). There is no A badge and no B badge on the board. Glyph names are Segoe MDL2's; what each code point draws in the installed font is **UNVERIFIED** here.

Where each face stands in its key, worked out from the template's stacks and checked against pictures 13, 15 and 17 (key face = x, y, w, h; cx, cy its middle):

- Symbols: the text's left edge at cx - (text width + 12 + 28) / 2; the LT badge 12 after the text, y cy - 11 .. cy + 11.
- Backspace: glyph box 26 wide from cx - 31; X's ring from cx + 7 to cx + 31, both centred on cy.
- Cursor left / right: the stack is 26 + 16 + 18 = 60 tall, so the chevron's middle is at y + 33 and the pill's at y + 71 (pill x cx - 16 .. cx + 16).
- Shift / Enter: the stack is 26 + 14 + 24 = 64 tall, so the arrow's middle is at y + 31 and the badge's at y + 70.
- Space bar: the mark's stroke makes it 25 by 7 for the layout; mark, 12, ring is 61 wide, so the mark's left stroke is centred on cx - 30.5, its top is at cy + 1.5 and its foot at cy + 8.5; Y's ring is centred on cx + 18.5.

### 7.4 Focus ring

**One ring for the whole board**, moved onto the focused key's cells; no key has a ring of its own (E/ConsoleKeyboardControl.xaml:209-241; E/ConsoleKeyboardControl.xaml.cs:138-143, 146-153).

- `Border Margin="-1" BorderThickness="3" CornerRadius="4"`, brush `#FF2CA243`, no fill, placed on the key's cells (all of them, for a spanning key). So the ring's outer edge is 1 outside the cells = **3 outside the key's edge**, and its inner edge is **exactly on the key's edge**: a 3-thick ring hugging the key, with no gap. (Compare a tile's: 4 thick, 4 clear of the tile.)
- Corner radius 4 at the stroke's centre line (WPF), so outer 5.5, inner 2.5, over a key of radius 2.
- A letter key's ring box: 72 x 54. Neighbouring keys are 4 away, so the ring covers 3 of the 4 px gap.
- Inside it the same `XboxFocusGlow` (rectangular variant, `home.md` 4.3) at 1:1. With a 3-thick ring the glow's bands are centred **2 outside the key's edge** (1 inside the ring's outer edge; half a pixel off the stroke's centre) and reach 17.5 outside the key's edge, over the neighbouring keys, and 13.5 inside it; its four corner spots are centred 7 inside the key's corners. Derived: the glow's `Margin="-7.5"` is taken from the ring's inside, which is the key's edge (T/Constants.xaml:27, 41, 91-98).
- The ring is drawn over all keys (it is on a grid above them), shown and moved instantly, hidden while no key has focus. It does not slide from key to key (NOT FOUND).

---

## 8. Console keyboard: layers and typing

### 8.1 What every key reads

C/ConsoleKeyboard.cs:310, 313, 222-236. Three states of the same 47 keys. Only the three letter rows (rows 1 to 3, columns 1 to 10) ever change; the digit row, the side keys and the space bar are the same in all three, except the symbols key's own text.

Letters, **Shift on** (how the keyboard opens):

    row 0   [#+=]  1  2  3  4  5  6  7  8  9  0  [bksp]
    row 1   [ <  ] Q  W  E  R  T  Y  U  I  O  P  [ >  ]
    row 2   [ <  ] A  S  D  F  G  H  J  K  L  "  [ >  ]
    row 3   [Shft] Z  X  C  V  B  N  M  ;  :  !  [Entr]
    row 4   [Shft] [        space bar          ] [Entr]

Letters, **Shift off**:

    row 0   [#+=]  1  2  3  4  5  6  7  8  9  0  [bksp]
    row 1   [ <  ] q  w  e  r  t  y  u  i  o  p  [ >  ]
    row 2   [ <  ] a  s  d  f  g  h  j  k  l  "  [ >  ]
    row 3   [Shft] z  x  c  v  b  n  m  ;  :  !  [Entr]
    row 4   [Shft] [        space bar          ] [Entr]

**Symbols** (the same whatever Shift says):

    row 0   [abc]  1  2  3  4  5  6  7  8  9  0  [bksp]
    row 1   [ <  ] !  @  #  $  %  ^  &  *  (  )  [ >  ]
    row 2   [ <  ] -  _  =  +  [  ]  {  }  \  |  [ >  ]
    row 3   [Shft] ~  `  <  >  /  ?  ,  .  '  "  [Entr]
    row 4   [Shft] [        space bar          ] [Entr]

Exactly, as strings, columns 1 to 10: letter rows `QWERTYUIOP`, `ASDFGHJKL"`, `ZXCVBNM;:!`; symbol rows `!@#$%^&*()`, `-_=+[]{}\|`, ``~`<>/?,.'"``. The four punctuation keys on the letters layer (`"` `;` `:` `!`) and the digits do not change with Shift. Note what the letters layer lacks: comma, full stop, apostrophe, hyphen and @ are on the symbols layer only. `"` and `!` are on both layers.

Accented letters, a second symbols page, long-press alternates, emoji, other languages' layouts, a paste key: **NOT FOUND**.

### 8.2 What each key does

`Press`, C/ConsoleKeyboard.cs:114-145 (CT/SearchTests.cs:369-483, 859).

| Key | Does |
|---|---|
| Character | Inserts what it reads at the cursor; the cursor moves past it; **Shift goes off** |
| Space bar | Inserts one space at the cursor; Shift goes off |
| Shift | Turns Shift on if off, off if on |
| Symbols | Swaps the letter rows for the symbol rows, or back |
| Backspace | Deletes the one character before the cursor; nothing at the start of the text |
| Cursor left / right | Moves the cursor one character; stops at the start and at the end |
| Enter | Hands the text to the keyboard's owner: Search puts the keyboard away (section 5.2); text entry keeps the text (section 10.4) |

**Shift is one-shot, not a lock.** It is on when the keyboard opens empty; any character or space typed turns it off, whether typed by A on a key, by the pad's Y, or on a physical keyboard; the Shift key (or the left stick's click) turns it on again for the next one character. Pressing Shift twice just turns it on and off: **there is no caps lock** (NOT FOUND). Backspace and the cursor keys leave it as it is. It does not come back on by itself after the text is emptied or after a full stop (NOT FOUND). Typing a symbol turns it off as well, though the symbols layer does not show it.

**The symbols layer stays** until the symbols key (or LT) is pressed again: typing a symbol does not go back to letters. Opening the keyboard always starts on letters.

How it opens (C/ConsoleKeyboard.cs:88-107): on Search, always empty, cursor 0, Shift on, letters. In text entry, over the starting text with **the cursor at its end**, letters, and **Shift on only if the starting text is empty**.

### 8.3 The caret model

- The text is a plain string; the cursor is an index into it, 0..length (C/ConsoleKeyboard.cs:16-17, 42-54). Characters go in **at the cursor**, not at the end (lines 183-187; CT/SearchTests.cs:455).
- One caret, no selection, no word or line jumps, no Home/End, no forward delete: NOT FOUND.
- The owner draws the text in two runs either side of a 2 x 30 white caret (sections 2 and 10.2).
- **No maximum length** and no character filter on the keys (NOT FOUND). A physical keyboard's text goes in less any control characters (lines 151-162). The port's text entry can be opened with a most (a Group's name is 40 characters at most, [filters.md](filters.md) section 6): at the most, a key or a typed character puts nothing in, though a key pressed still turns Shift off; a text it opened over is left as long as it is.

### 8.4 A physical keyboard

"For testing on the desktop", but it works (E/SearchControl.xaml.cs:237-262, 357-367; E/TextEntryControl.xaml.cs:217-243, 286-296; C/ExtensionCore.Search.cs:151-182):

- Any printable character typed goes in at the cursor **exactly as typed** (its own case, whatever the on-screen Shift says), and turns Shift off. On Search this works with the on-screen keyboard up **or away** (CT/SearchTests.cs:471, 869).
- Space types a space (it never presses the focused key). Backspace deletes before the cursor.
- The arrow keys are the D-pad: they move focus, not the caret.
- Enter presses whatever has focus, as A does: the focused key, the search box, a tile.
- Escape: in text entry, cancels as B does. On Search it is not handled by the page: **UNVERIFIED** (Playnite's own).
- While Search is up, Playnite's own single-letter, digit and space shortcuts are lifted so the typing reaches the page (E/SearchControl.xaml.cs:475-523).

---

## 9. Console keyboard: navigation and pad shortcuts

### 9.1 Where focus starts

- First time on a page just opened: **Q**, the first key of letter row 1 (E/ConsoleKeyboardControl.xaml.cs:68-89).
- Search, keyboard put away and brought back in the same visit: **the key last focused** (it is the same key whatever it reads now) (lines 28-29, 68-78). Leaving and reopening Search starts on Q again (E/SearchControl.xaml.cs:117).
- Text entry: always Q on opening (E/TextEntryControl.xaml.cs:132-143).
- Coming Down onto the board from Search's results or search box: the key last focused, else Q (E/SearchControl.xaml.cs:274-277, 311-314). It is **not** the key under the tile focus came from.

### 9.2 D-pad and left stick

Focus moves **one grid cell at a time**, and the board remembers the cell (row, column) focus is "in", which for a key spanning several cells is the cell it was entered through (E/ConsoleKeyboardControl.xaml.cs:24-26, 91-105, 253-293):

    Left:  to the cell (remembered row, key's first column - 1)
    Right: to the cell (remembered row, key's last column + 1)
    Up:    to the cell (key's first row - 1, remembered column)
    Down:  to the cell (key's last row + 1, remembered column)

then focus goes to whichever key covers that cell, and that cell is remembered. A key focused any other way (first focus, coming back onto the board, the mouse) is "in" its first cell (its top row, its leftmost column).

**No wrap anywhere.** Past the left, right or bottom edge: nothing happens, focus stays. Past the top edge: the owner decides: on Search, focus leaves to the last row of results or the search box (section 5.1); in text entry, nothing.

What that gives at the wide and tall keys:

| From | Press | Lands on |
|---|---|---|
| A letter on row 3, column c | Down | The space bar (remembering column c) |
| Space bar | Up | The row-3 key in the column it was entered from; **Z** (column 1) if it was not entered by Down |
| Space bar | Left / Right | Shift / Enter (remembering row 4) |
| Shift or Enter, entered from the space bar | Right / Left | Back to the space bar |
| Shift, entered from Z (row 3) | Right | Z |
| Enter, entered from `!` (row 3) | Left | `!` (the last key of row 3) |
| Shift or Enter | Down | Nothing (bottom edge) |
| Shift / Enter | Up | Cursor left / cursor right, remembered row 2: from there Right / Left goes to **A** / `"` (row 2) |
| Cursor left, entered from Q (row 1) | Right | Q |
| Cursor left, entered from A (row 2) | Right | A |
| Cursor left, entered from the symbols key (Down) | Right | Q (row 1) |
| Cursor left / right | Down | Shift / Enter, remembered row 3: from there Right / Left goes to Z / `!` |
| Cursor left / right | Up | Symbols / backspace |
| Symbols, backspace | Left / Right outward | Nothing |
| Any row-0 key | Up | Leaves the board (Search) or nothing (text entry) |

Held D-pad repeat is Playnite's (every 80 ms by the core's account, C/ExtensionCore.cs:45-46): **UNVERIFIED**. **The port (decided in #54):** a held direction repeats after 700 ms, then every 80 ms, as in the Guide and the Tile menu, and stops dead at an edge.

### 9.3 Pad shortcuts

C/ConsoleKeyboard.cs:164-180; E/ConsoleKeyboardControl.xaml.cs:219-238; CT/SearchTests.cs:530-588; CT/TextEntryTests.cs:75.

| Button | Does | Notes |
|---|---|---|
| A | Presses the focused key | |
| **X** | Backspace | |
| **Y** | Space | |
| **LB** / **RB** | Cursor left / right | |
| **Left stick click** | Shift | |
| **Menu (Start)** | Enter | |
| **LT** | The symbols key | Handled by the board itself, so only while a key has focus; the model has no shortcut for it |
| RT | Nothing | NOT FOUND |
| Right stick click, View | Nothing on the keyboard | Swallowed by the owner where stated (sections 5.2, 10.4) |
| B | The owner's: Search puts the keyboard away; text entry drops the text | |

A shortcut does exactly what pressing its key does, and lights that key while the button is held (section 7.2).

**Held repeat on backspace and the cursor: NOT FOUND in the Bundle.** Neither the model nor the control starts a timer; a key is pressed once per button-down event that reaches it. Whether Playnite sends repeated button-down events for a held A, X, LB or RB is **UNVERIFIED**. **The port (decided in #54): only directions repeat.** A held A, X, Y, LB, RB, left stick click, LT or Menu presses its key once.

---

## 10. Text entry (the keyboard over the dimmed page)

C/TextEntryState.cs, C/ExtensionCore.TextEntry.cs, E/TextEntryControl.xaml(.cs).

### 10.1 What it is, and every place the Bundle uses it

Every text the Bundle asks for outside Search is typed here, in place of Playnite's own keyboards: the page dims, and at the foot stand a caption, the field, and the Console keyboard. It is the last element of the main host, over the pages, the top bar, the Game hub, the hint bar and the Tile menu (T/Views/Main.xaml:776-781). One at a time: nothing opens while it is up (C/ExtensionCore.TextEntry.cs:120-126).

| Caption | Opened from | Starts with | Masked | As it is typed | Kept (Start / Enter) | Dropped (B) | Milestone | Source |
|---|---|---|---|---|---|---|---|---|
| "Filter by name" | Full library's name box: the pad's A or Y on it, or a letter typed there on a physical keyboard (which goes in after the name) | The name the grid is narrowed by now | no | **The grid narrows on every key**, live, behind the dim | The grid stays narrowed (nothing more to do) | The name it had when text entry opened is put back, and the grid with it | **M3** | C/ExtensionCore.FullLibrary.cs:71-85; E/MyGamesControl.xaml.cs:1120-1136 |
| "Gamertag" | Settings, Add profile | Empty | no | - | A profile is added with that gamertag (a blank one adds nothing) | Nothing added | M5 | C/ExtensionCore.TextEntry.cs:59-65; CT/TextEntryTests.cs:41, 108, 121 |
| "Gamertag" | Settings, a Profile's Gamertag box | Its gamertag | no | - | The profile is renamed | Unchanged | M5 | C/ExtensionCore.TextEntry.cs:68-75 |
| "Real name" | Settings, a Profile's Real name box | Its real name | no | - | Saved | Unchanged | M5 | C/ExtensionCore.TextEntry.cs:78-85 |
| "Client ID" | Settings, Discord | The saved Client ID | no | - | Saved | Unchanged | M6 | C/ExtensionCore.TextEntry.cs:88-94 |
| "Client Secret" | Settings, Discord | **Empty** (the saved one is never shown) | **yes** | - | Saved; kept empty or blank, the saved one stays | Unchanged | M6 | C/ExtensionCore.TextEntry.cs:100-112; CT/TextEntryTests.cs:221-270 |

So the reusable piece takes: a caption, a starting text, a masked flag, and three callbacks: on every edit, on keep, on drop (`ShowTextEntry`, C/ExtensionCore.TextEntry.cs:120-136). It opens only over Settings or Full library (lines 28-32), and it goes, **dropping** the text, if that page goes under it (the Guide's Home, another page taking its place, a game exiting; lines 167-173; CT/TextEntryTests.cs:361-389).

### 10.2 Layout

Nothing is in a Viewbox. E/TextEntryControl.xaml:42-80.

| Element | Authored | On screen (x, y, w, h) | Lines |
|---|---|---|---|
| Dim | Full screen, vertical gradient (below) | 0, 0, 1920, 1080 | 51-60 |
| Stack | Width 928, centred, bottom-aligned, `Margin="0,0,0,110"` | x 496..1424, bottom y 970 | 63 |
| Caption | `Margin="28,0,28,14"`; one line of Bahnschrift 30 (about 36 tall, **UNVERIFIED** metric) | x 524..1396, **y 570..606** | 64, 29-34 |
| Field | Height 54, the stack's width | **x 496..1424, y 620..674** | 67 |
| Console keyboard | `Margin="0,36,0,0"`, 928 x 260 | **x 496..1424, y 710..970** (exactly where Search has it) | 78-79 |

Arithmetic, upward from the keyboard's foot at 1080 - 110 = 970: keyboard 260 (top 710), gap 36 (674), field 54 (620), gap 14 (606), caption 36 (570).

**Dim** (lighter at the top so Full library is seen narrowing behind it, darker where the caption, field and keys sit):

| Offset | y | Colour |
|---|---|---|
| 0 | 0 | `#59000000` (35% black) |
| 0.4 | 432 | `#59000000` |
| 0.62 | 669.6 | `#C8000000` (78% black) |
| 1 | 1080 | `#C8000000` |

**Caption:** Bahnschrift 30, weight 350 (SemiLight), white, one line, character ellipsis, left-aligned 28 in from the field's left edge (the same style as Search's row headings) (lines 29-34, 64).

**Field:** Search's pill: `#FF2E3136`, `CornerRadius="27"`, 928 x 54 (lines 19, 67-68). The text area is inset 28 each side (x 524..1396, w 872) and **clips** (`ClipToBounds="True"`): the text is left-aligned, in two runs either side of the caret, Segoe UI 26 white, vertically centred; a text longer than the area is cut on the right and does not scroll, the caret going out of sight with it (lines 69-75). **The port scrolls it (decided in #54):** when what is before the caret is wider than the area, the text is drawn that much to the left, so the caret stands at the area's right edge with the text's end before it; with the caret moved back until what is before it fits, the text is at rest again with its start at the area's left edge. Nothing is drawn outside the area either way. The caret is the same 2 x 30 white rectangle with 1 clear each side, **always shown** (the keyboard is always up), not blinking. The field has **no placeholder**, **no focus state and no ring**: focus is never on it (lines 66, 72; E/TextEntryControl.xaml.cs:39).

**Masked input:** while the entry is secret the field shows one **U+25CF** (black circle) per character typed, in the same font and colour, the caret among them at the cursor; the real text is never drawn (E/TextEntryControl.xaml.cs:24-25, 198-209). **The port draws the dots itself** (decided in #54: which font supplies U+25CF is unverified, and no picture shows a secret): white discs 12 across, one every 18, so they are the same on every machine. The keyboard forgets the text as text entry goes, kept or dropped, so a secret does not linger in it, and the next entry is plain again (C/ExtensionCore.TextEntry.cs:142-161; CT/TextEntryTests.cs:252-282). There is no "show" toggle (NOT FOUND).

### 10.3 Focus

- Focus is on the keys from the moment it opens (Q) until it goes, and cannot leave them: Up from the top row does nothing, the field and the caption are not focusable, and if the page underneath takes focus (Full library's grid does as it narrows) focus is put back on the key it was on (E/TextEntryControl.xaml:42-45; E/TextEntryControl.xaml.cs:70-98, 173-195).
- As it goes, focus returns to **what had it before** (the name box, the Gamertag box, the Type button), if that is still on screen; otherwise the page that opened it places focus (E/TextEntryControl.xaml.cs:129-171).

### 10.4 Buttons

While it is up it has the pad: every button is taken here, and neither the page under it nor a tile focused there hears one (E/TextEntryControl.xaml.cs:245-284; C/ExtensionCore.cs:869; CT/TextEntryTests.cs:284-343).

| Input | Does |
|---|---|
| D-pad / left stick | Moves along the keys (section 9.2) |
| A | Presses the focused key; **Enter keeps the text** |
| **Menu (Start)** | **Keeps the text**: text entry goes, the keep callback gets the text |
| **B** | **Drops the text**: text entry goes, whatever the typing changed is put back |
| X, Y, LB, RB, left stick click, LT | Backspace, space, cursor left / right, Shift, symbols |
| RT, right stick click, View, anything else | Nothing (swallowed) |
| Guide | The Guide opens over it and has the pad until it closes; text entry is still there after (CT/TextEntryTests.cs:345) |
| Physical keyboard | Section 8.4; Escape drops as B does |

Not while it is up: the Tile menu (C/ExtensionCore.TileMenu.cs:33), Y's Search (C/ExtensionCore.Search.cs:80), a tile's A (C/ExtensionCore.cs:1333). The Idle screen opening a game's hub drops it first (C/ExtensionCore.Idle.cs:171-176; M5).

### 10.5 Transitions, sounds, hint bar

It appears and goes **in one frame** (`Visibility`, E/TextEntryControl.xaml:42): no fade of the dim, no slide of the keyboard (NOT FOUND). No sound of the extension's (section 6). The hint bar is under its dim and shows nothing of text entry's; the keys' own badges say what the pad does. Reduce motion changes nothing here.

**The port (decided in #54):**

- Sounds: moving from key to key is the move sound, as Playnite's focus sound is in the Bundle; a move that goes nowhere at an edge is silent. Pressing a key, a shortcut, Shift or the layer turning, Menu and B are silent. A on the box that opens text entry is the activation sound, as A that opens any page is.
- The hint bar is not drawn while text entry is up, where the Bundle leaves the page's own bar dimmed under it: the Dashboard draws the bar for whatever has the pad, and text entry has no hints.
- On a screen of another shape (open question 11) the keyboard stays 110 off the screen's foot with the field and caption over it, and the dim's two stops stay at four tenths and 62 hundredths of the screen's height.
- A physical keyboard's Escape is B on every page, text entry among them, and leaves the Dashboard only at Home with nothing open.

---

## 11. Text rules

Font resources: T/Constants.xaml:228-232. Segoe UI's line is taken as 1.33 em and Bahnschrift's as 1.2 em, as the Bundle's own comments and tests take them; both are **UNVERIFIED** font metrics.

| Text | Font | Size | Colour | Wrap / trim | Source |
|---|---|---|---|---|---|
| Search box placeholder "Search for games and apps" | Segoe UI Regular | 26 | `#FFB4B7BD` | 1 line, character ellipsis | E/SearchControl.xaml:229-230 |
| Search box text; text entry's field text | Segoe UI Regular | 26 | `#FFFFFFFF` | 1 line, no trim, cut at the text area's right edge | E/SearchControl.xaml:232-235; E/TextEntryControl.xaml:69-75 |
| Masked field text | Segoe UI Regular (U+25CF per character; which font supplies the dot is **UNVERIFIED**) | 26 | `#FFFFFFFF` | as above | E/TextEntryControl.xaml.cs:25, 201-204 |
| Row headings "Recently added", "Games", "Apps" | Bahnschrift, weight 350 | 30 | `#FFFFFFFF` | 1 line, character ellipsis | E/SearchControl.xaml:34-40 |
| Text entry's caption | Bahnschrift, weight 350 | 30 | `#FFFFFFFF` | 1 line, character ellipsis | E/TextEntryControl.xaml:29-34 |
| Game tile title (caption) | Segoe UI Regular | 26 | `#FFFFFFFF` | Wrap, max 2 lines (MaxHeight 72), character ellipsis | E/SearchControl.xaml:188-189 |
| Game tile status (M4) | Segoe UI Regular | 26 | `#FFB4B7BD` | 1 line, character ellipsis | E/SearchControl.xaml:190-191 |
| Game tile title with no cover | Segoe UI Regular | 26 | `#FFFFFFFF` | Wrap, centred, no trim, margin 12 | E/SearchControl.xaml:173-174 |
| App tile name band (M4) | Segoe UI Regular | 26 | `#FFFFFFFF` | Wrap, max 2 lines (MaxHeight 72), character ellipsis, margin 8 | E/AppTiles.xaml:44-45 |
| Key character; "#+=", "abc" | Segoe UI Regular | 26 | `#FFFFFFFF` | - | E/ConsoleKeyboardControl.xaml:111-112, 134 |
| Key glyphs | Segoe Fluent Icons, fallback Segoe MDL2 Assets | 26 | `#FFFFFFFF` | - | E/ConsoleKeyboardControl.xaml:113-114 |
| Badge letters "X", "Y" | Segoe UI Regular | 14 | `#FF3F8EF0`, `#FFFFB900` | - | E/ConsoleKeyboardControl.xaml:146-149, 199-202 |
| Badge letters "LT", "LB", "RB", "LS" | Segoe UI Regular | 10 | `#FFB4B7BD` | - | E/ConsoleKeyboardControl.xaml:139-140, 160-161, 172-173, 181-182 |
| Enter's Menu badge glyph U+E700 | icon font | 11 | `#FFB4B7BD` | - | E/ConsoleKeyboardControl.xaml:191-193 |

The badge letters (10 to 14 px) are the only text here under the Bundle's 26 px floor. All colours on these pages, gathered: page gradient `#FF123A2C` / `#FF07140F` / `#FF000000`; box `#FF2E3136`, focused `#FF44474E`; tile cover ground `#FF1A1A1A`; caption `#FF3A3D44`; dim text `#FFB4B7BD`; keys `#FF2F3238` / `#FF3A3D44` / `#FF555963` / `#FF7A7E88`; ring `#FF2CA243`; X `#FF3F8EF0`; Y `#FFFFB900`; dim `#59000000` to `#C8000000`.

---

## 12. Small traps

1. **Scale is 1.** Nothing here is in Home's Viewbox: a tile's ring is 4 thick 4 clear at radius 14, not those times 1.114, and the tile is 240, not 153 or 245.
2. **Two different rings.** Tiles and the search box: 4 thick, 4 clear of the element (`Margin="-8"`). Keys: 3 thick, **touching** the key (`Margin="-1"` on cells that are 2 larger than the key), radius 4.
3. **The search box's glow is the rectangular one** (bands of corner radius 12, plus corner spots) inside a ring of radius 35, so at the pill's two ends the faint glow is squarer than the ring it surrounds. Playnite's own keyboard's field, by contrast, uses the round glow (T/Views/TextInput.xaml:78-83). As drawn, not as one would design it: open question 6.
4. **Rows show whole tiles only.** The 8th tile is not peeking at the right edge; it is wholly cut off, and the row slides a whole tile at a time. The first tile is at x 74, level with the heading, although the list itself starts at x 50.
5. **A row's end tiles lose part of their glow** to the row's clip (9 left of the first, 15 right of the last); the ring itself is whole.
6. **A Search game tile is not a Home tile.** It never grows, has no title band, no source badge, no Running badge; its title is always showing in a grey caption under a square cover. Only the cover's fit (fill, top-aligned) and the ring are shared.
7. **Up and Down between rows go to each row's remembered tile, not to the tile above or below.**
8. **Coming down onto the keyboard goes to the key last focused (first time: Q)**, not to the nearest key.
9. **The keyboard's navigation is by grid cell with a remembered row and column**, which is what brings Up from the space bar back to the letter it was left from, and Right from a tall side key back to the row it was entered from. Nearest-neighbour navigation gets these wrong.
10. **No wrap** on the keyboard, in a row of tiles, or anywhere else.
11. **Shift is one-shot and starts on**; in text entry it starts on only over an empty text. There is no caps lock and no lit state for Shift.
12. **The symbols layer is sticky**; the digit row never changes; `"` and `!` are on both layers; comma and full stop are on symbols only.
13. **B is two steps on Search**: keyboard away, then leave. Start and Enter are the first step only.
14. **Y on Search never reopens Search**: it is space with the keyboard up and nothing with it away. X, Y, LB, RB, LS and Start act as keyboard shortcuts even while focus is on a result tile or the search box, as long as the keyboard is up; LT only while a key has focus.
15. **A on a result opens the Game hub at once**: Home's 300 ms double-tap window does not apply, and a double tap does not play.
16. **Search opens empty every time**, on Q, with Shift on. Nothing is kept between visits; even B from a Game hub returns to the same visit, not a new one.
17. **The matched text is trimmed, the shown text is not.** With only spaces typed the placeholder is hidden (the text is not empty) while Recently added still shows (the trimmed text is).
18. **Typing never moves focus off the keys**, though the rows under them are replaced as each character goes in.
19. **The caret does not blink**, and on Search it is hidden while the keyboard is away.
20. **Nothing found draws nothing**: no message. An empty library still draws the "Recently added" heading.
21. **Menu on a result does nothing and the hint bar is empty on Search**: results have no Tile menu.
22. **Text entry's keyboard stands exactly where Search's does** (x 496..1424, y 710..970), and its field is the keyboard's width (928), not the search box's (960).
23. **Text entry over Full library filters live** and B puts the old name back; Start only closes it.
24. The page's greys are bluish (`#2E3136`, `#2F3238`, `#3A3D44`), not Home's neutral `#1A1A1A` / `#2B2B2B`. The caption grey and the side keys' grey are the same colour.
25. Search is **not** Full library's name filter: the glossary keeps "filter" for narrowing a list by name and "Search" for this page (CONTEXT.md:58-60).

---

## 13. What is Playnite-specific, and what to do about it

| Playnite-owned thing | Where | For the standalone port |
|---|---|---|
| Playnite's own bindings on its view host: Y opens its keyboard to filter by name, LB/RB switch filter presets, RS opens its filters, Back its menu; the Bundle rebinds Y to Search and removes or swallows the rest on each page | E/PlayniteViewHost.cs:19-42; E/SearchControl.xaml.cs:320-355; T/Views/Main.xaml:373 | **Drop.** There are no foreign bindings to fend off. Route Y to Search from Home and from Games in the screen's own input handling; X, RS, LB/RB, View do nothing on Search with the keyboard away. |
| Playnite's single-letter, digit and space key shortcuts, lifted while Search is up | E/SearchControl.xaml.cs:475-523 | **Drop.** Send a physical keyboard's text to the page while Search or text entry is up. |
| The pad arriving as key events with no Key, A never "clicking", focus slipping to Playnite's main list (the `FocusSoon` deferrals, text entry's `PreProcessInput` guard and refocusing) | E/SearchControl.xaml.cs:403-429; E/TextEntryControl.xaml.cs:70-98, 185-195 | **Drop.** The port's own focus model makes text entry modal and places focus directly. Keep only the rules: where focus goes on open, on keyboard away, on rows replaced, on hub closed. |
| The Game hub being "for the main list's selected game": Search first puts every game in Playnite's main list (`ShowInMainList(LastPlayed)`), and gives the user's filter and sort back on leaving (`GiveBackMainList`, `ReleaseUserFilter`) | C/ExtensionCore.Search.cs:190-197, 335-343; CT/SearchTests.cs:920-1000 | **Drop.** Open the Game hub by game id over Search; nothing to give back. |
| Playnite's library fields: `Name`, `SortingName`, `Hidden`, `Added` | C/ExtensionCore.Search.cs:255-264 | **Port the rules, on the Dashboard's own library.** It needs a hidden flag (never found), a sorting name (order within a rank; fall back to the name) and a date added (Recently added). The Dashboard's library has no date added yet (`home.md` section 11): open question 2. |
| .NET's culture-aware comparison (`CompareInfo.IsPrefix` / `IndexOf` with `IgnoreCase` and `IgnoreNonSpace`, current culture) and `StringComparer.CurrentCultureIgnoreCase` for the order | C/ExtensionCore.Search.cs:19, 289-322 | **Replace** with a fixed fold: Unicode case folding plus removal of nonspacing marks after NFD on both name and text, then plain substring / prefix tests, and order by the folded sorting name. Culture-specific tailoring (Turkish i, ligatures and the like) will differ; say so in the port's tests. |
| Playnite's sounds on focus and activation (`navigation.wav`, `activation.wav`), the only sounds on these pages | E/PlayniteSounds.cs:69-75 | **Decide** (open question 5). The port plays its own sounds; the Bundle gives no rule of its own here to copy. |
| Playnite's held D-pad repeat (80 ms) | C/ExtensionCore.cs:45-46 | **Replace** with the port's own D-pad repeat, the same one Home uses. |
| Playnite's own on-screen keyboard window, restyled by the theme to look like the Console keyboard (grey keys min 76 x 60, the same four greys, a 3-thick ring 3 outside each key; a pill field with a round ring; a `#FF2B2B2B` panel of radius 8). With the extension it no longer comes up for Y anywhere on the Dashboard; it **can still appear** from Playnite's own windows and other extensions' text prompts (which ones: **UNVERIFIED**), and it is what Y opens with the theme alone. | T/Views/TextInput.xaml:1-106; CONTEXT.md:59, 64 | **Drop.** The port has one keyboard, the Console keyboard, and nothing else asks for text. |
| Apps: the list (`MyGames.Apps`), their icons and Store-app flag, starting one through Playnite (`StartApp`) | C/ExtensionCore.Search.cs:203-210, 265-267; E/AppTiles.xaml | **M4.** In M3 the Apps row is always empty and so never shows; leave the row in the layout. |
| Install status on a tile ("Queued", "Installing...") | C/ExtensionCore.Queue.cs:15-16 | **M4.** In M3 the status line is always empty. |
| Bringing Playnite's window to the front on Search from the Guide (`BringToFront(DashboardPage.Search)`) | C/ExtensionCore.cs:2145-2150 | **Port** as the Dashboard's own "come to the front on a page" when the Guide exists; the rule (on A's release, Guide closes first) stays. |
| Playnite's library-update progress shown over the page | T/Views/Main.xaml:401-441 | **Drop.** |
| WPF's virtualised list and `BringIntoView` (the instant vertical scroll between rows; the layout clip that cuts a long text) | E/SlidingRow.cs:136-141; E/SearchControl.xaml:85-89 | **Replace** with an explicit rule (open questions 3 and 8). Virtualisation is not needed to match the look. |
| The mouse: clicking a key, a tile, the search box; clicking text entry's dim to cancel | E/TextEntryControl.xaml.cs:298-301; E/ConsoleKeyboardControl.xaml.cs:188-191 | **Drop**, unless the port supports a mouse elsewhere. |
| Discord's and Profiles' callers of text entry; the Idle screen dropping it | section 10.1 | **M5 / M6.** M3 builds text entry once, with the caption / start text / masked / on-edit / keep / drop interface, and uses it for "Filter by name" only. |

---

## 14. Open questions

1. **There is no picture of any of this.** Every number is from the XAML. Should someone run the Bundle's `cover-fit` or `hub-panel` test on the VM (both take a `search` shot; neither shows text entry or the symbols layer) and add shots of the keyboard's three layers and of text entry, to check this file before or after the port is built?
2. **Recently added needs a date added**, which the Dashboard's library does not record (`home.md` section 11). What stands for it with Steam alone: the time the library first saw the game, a file time from Steam's own files, or leave the row in the library's order until it exists?
3. **Scrolling between the Games and Apps rows with the keyboard up.** The Bundle leaves it to WPF: an instant jump that can leave a row's heading scrolled off and the focused tile's ring cut at the viewport's edge (section 3.6, UNVERIFIED). Copy that, or scroll by whole rows so the heading and the ring always show? It only matters once Apps exist (M4); in M3 one row at most shows and nothing scrolls.
4. **Held repeat.** The Bundle has no repeat of its own on backspace, the cursor keys or A; what happens on a held X in Playnite is unknown. Should the port repeat a held X / LB / RB (and A on a key) at the D-pad's rate, or press once per press? **Decided (#54):** held directions repeat as everywhere else (700 ms, then 80 ms); nothing else repeats (sections 9.2, 9.3).
5. **Sounds.** The Bundle adds none; Playnite's focus sound probably plays on every move between keys and tiles, and nothing is known to play on a key press, on Search opening or closing, or on the keyboard going away. Which of the port's sounds, if any, go on: moving between keys, pressing a key, opening and leaving Search, opening and closing text entry?
6. **The search box's glow.** Copy the Bundle's rectangular glow on the pill (squarer than the ring at both ends), or draw the round variant, which is what its shape calls for and what Playnite's own keyboard's field uses?
7. **Shift and symbols have no lit state.** Keep it exactly so (Shift on is seen only in the letters' case), or light the latched key? The requirement is "exactly as the theme", so the default is to keep it. **Decided (#54):** Shift lights while it is on, in the focused key's grey; the symbols key does not (section 7.2).
8. **A text longer than the field** is cut on the right and the caret goes out of sight, on Search (904 px of text) and in text entry (872 px). Copy that, or keep the caret in view by scrolling the text? A 32-character Client Secret in dots fits; a long name filter may not. **Decided (#54):** long text scrolls in the field with the caret kept in view (section 10.2); Search's box draws its text the same way.
9. **Menu on a result** does nothing in the Bundle and the hint bar is empty on Search. Keep it so, or give results the Tile menu that Home's and Games' tiles have?
10. **Search tiles show neither the Running badge nor the source badge**, though the core marks a found game as running. Keep it so (parity), or draw them as Home does?
11. **Other screen shapes.** The Bundle's page is always 1080 tall and as wide as the aspect makes it, so a narrower screen shows fewer whole tiles (section 3.3) while the search box (960) and the keyboard (928) stay their size, centred. The Dashboard instead keeps 1920 across and lets the height grow (1200 at 16:10, `home.md` section 11): then the row is always 7 tiles and the question is vertical. Proposal: the search box stays 77 from the top, the keyboard 110 off the foot, and the results take the extra height. Agreed?
12. **View (Back) and Escape on Search** are left to Playnite in the Bundle (unknown result). The port's proposal: both do nothing, or Escape acts as B. Which?
13. **Matching without .NET's cultures** (section 13): is a fixed case-and-accent fold acceptable, or must the user's locale tailor it as the Bundle's does?

