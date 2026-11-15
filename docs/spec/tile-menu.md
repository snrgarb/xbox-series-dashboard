# Tile menu specification (as the Bundle draws it)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run.

Path shorthand used in citations:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`

All numbers are design units = px on a 1920x1080 canvas. **Nothing on this page is inside a Viewbox**: the Tile menu is hosted 1:1 over the whole Dashboard page (T/Views/Main.xaml:770-771), so authored and on-screen numbers are the same, and Home's scale factor `s` (home.md) does **not** apply to it. The one exception is the "Quit X and start Y?" prompt, which is the Guide's and is drawn in the Overlay's own 1920x1080 `Viewbox Stretch="Uniform"` (E/OverlayWindow.xaml:904-905): 1:1 on a 1080p screen.

This spec describes the Tile menu **with the extension installed**. Without it, Menu and the Game hub's Manage open Playnite's own game menu, which the theme restyles to the same numbers (`T/Views/GameMenu.xaml`); that fallback is not specified here and a port has no use for it.

**The one font metric.** The panel's height depends on the title's line height, Bahnschrift at 33. As home.md does for the Group header, this spec takes a line of Bahnschrift as 1.2 em:

    L = 33 x 1.2 = 39.6          (UNVERIFIED font metric; the pictures give 39 to 40, see below)
    header = 36 + L + 24 = 99.6  (title's top margin, its line, its bottom margin; E/MenuPanel.xaml:41)

Everything that says "99.6" below is this. On screen the Bundle's edges land on whole pixels (the pictures show the panel at y 300 and the first row at y 399 where the arithmetic gives 299.7 and 399.3), so treat every y on this page as good to +-1 px.

**Pictures of the real thing** (`~/projects/xbox-series-theme/artifacts/e2e/`), all of 10 November, build 0.2.0:

- `achievements-20261110-105905/data-tilemenu.jpg`: the menu on a Recent row tile, 5 rows, focus on Play game.
- `achievements-20261110-105905/data-more-options.jpg`: More options' flyout over the dimmed menu.
- `achievements-20261110-105905/data-extensions.jpg`: Playnite's own Extensions picker, which the flyout's Extensions opens (not the Tile menu's drawing).
- `background-follows-selection-20261110-111334/tile-menu.jpg`, `tile-menu-customise.jpg` and `see-all-20261110-113608/settings-0-tile-menu.jpg`: the menu with focus on Play game and on Customise Home.

Measured in them (section 12 lists what they confirmed): the panel's rectangle, the rows' pitch, the gap before Customise Home, the ring's overhang, the fill, the dim, the flyout's offset. What they could **not** confirm: the row labels' size. In the pictures the labels are about 22 px (the ink of "Remove from Favourites" is 231 wide, which is Segoe UI at about 21 to 22); the source at `f11e4f9` says 26 (E/MenuPanel.xaml:78; CT/MenuPanelXamlTests.cs:119). This is home.md's caveat (type set at 26 after the pictures, #229): **follow the source, 26**. No picture exists of a prompt (Uninstall?, Remove?, "Quit X and start Y?"), of a 4-row or 6-row menu, of an Install, Quit or Uninstall row, or of a flyout with a game's own actions or HDR.

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite, WPF or font internals that are not in this repository.

**A note on "Manage".** Three things carry the word and they are not the same:

- **Manage** (glossary term, M3 parity list): the page under My games & apps' nav, whose Queue box opens Manage Queue. Specified in `my-games.md`. Not part of this page.
- **Manage game**: the Tile menu's third row. It opens the game's **Game hub**. Specified here (section 3).
- The Game hub's **Manage** button: it opens the **Tile menu** (without the Manage game row). The button is `game-hub.md`'s; what it opens is this page.

So "Manage game" leads to the hub and the hub's "Manage" leads back to this menu, one row shorter. Neither has anything to do with the Manage page.

---

## 1. What it is and what opens it

One menu for a game: a dark panel, **centred on the screen** over the dimmed page, wherever the tile that opened it is. It is not anchored to the tile. Its rows are built once, as it opens, from the library as it is then (C/ExtensionCore.TileMenu.cs:73-109); they do not change while it is up.

| Opened by | From | Manage game row | Source |
|---|---|---|---|
| Menu (Start) on a game tile | Home's Recent row and Groups; My games & apps' Play history, Manage Queue rows and Recently installed covers (every tile that reports focus to the core) | Yes | C/ExtensionCore.cs:840-843, 868-869; E/RecentRowControl.xaml.cs:274; E/GroupsControl.xaml.cs:289; E/MyGamesControl.xaml.cs:1440, 1627, 1772, 1798 |
| View (Back) on a game tile | Manage Queue only (its rows and Recently installed covers) (M4) | Yes | C/ExtensionCore.cs:844-849 |
| Menu on a game in Playnite's own grid | My games & apps' Games and Full library | Yes | E/GameMenuTakeover.cs:12-25, 37; C/ExtensionCore.TileMenu.cs:30-41; CT/TileMenuTests.cs:687-693 |
| The Game hub's Manage button, and Menu anywhere on the hub | Game hub | **No** ("the player is there already") | E/GameMenuTakeover.cs:38, 57-58; T/Views/GameDetails.xaml:372; CT/TileMenuTests.cs:677-685 |

It does **not** open:

- on Home's Play history tile, See all cards, wide tiles or top bar (none is a game tile; home.md sections 3.5, 5, 6.5);
- on Search's result tiles: Search's tiles never report focus to the core (no `FocusHomeTile` call outside the three controls above), so Menu there does nothing. NOT FOUND otherwise;
- on Apps (CT/MoreOptionsHintTests.cs:258);
- while the Guide is open or still handing focus back, while text entry (the Console keyboard) is up, or while a Tile menu is already up (C/ExtensionCore.TileMenu.cs:33; C/ExtensionCore.cs:840, 869; CT/TileMenuTests.cs:150-170, 695-707);
- for a game no longer in the library (C/ExtensionCore.TileMenu.cs:32-36, 60-64).

Opening from a tile also cancels a pending A on that tile (the 300 ms double-tap window), so the Game hub does not open under the menu (C/ExtensionCore.TileMenu.cs:66-67).

Focus **stays on what opened it**: the tile keeps its ring, its grown size on the Recent row and its title band, dimmed with the rest of the page; the hub's button keeps its focus. Closing the menu therefore needs no refocus (E/TileMenuControl.xaml.cs:15-22). Home's background keeps doing whatever it was doing.

---

## 2. Layout

### 2.1 Layers (back to front)

| # | Layer | Source |
|---|---|---|
| 1 | The page as it was (Home, My games & apps, or the Game hub over either), the focused tile still focused, and the hint bar's slot (the bar itself is hidden while the menu is up, section 9) | T/Views/Main.xaml:761-768 |
| 2 | **Dim**: one rectangle over the whole page, `#99000000` (60% black) | E/TileMenuControl.xaml:17 |
| 3 | **Menu panel** (`Panel`, in `MenuLayer`), centred both ways | E/TileMenuControl.xaml:18-22 |
| 4 | **Flyout panel** (`FlyoutPanel`): More options, or the Remove prompt in its place. On a zero-size Canvas at the menu's top-left corner, `Canvas.Left="77"`, so it never moves the menu | E/TileMenuControl.xaml:23-25 |
| 5 | Above the Tile menu's host, in this order: Playnite's game status, then text entry. The Idle screen's dim (M5) is a window of its own over everything | T/Views/Main.xaml:773-781; E/DimWindow.xaml:17-25 |

The dim is the Tile menu's own rectangle. `E/DimWindow.xaml` is **not** it: that is the Idle screen's (M5).

The dim covers the top bar, the page, the Game hub and the hint bar's place alike. Measured: page pixels under it are 0.40 of their undimmed value (blue of the Discord tile 225 -> 89, green of a cover 217 -> 87), which is `#99` black.

### 2.2 The panel (shared by the menu, the flyout and the prompts)

`E/MenuPanel.xaml`, one drawing "shared by the Guide and the tile menu, so the two can't drift apart" (lines 6-17). Measured by its author on the console's context menu at 720p and scaled by 1.5.

| Part | Value | Line |
|---|---|---|
| Width | **450**, fixed | 36 |
| Height | Its content: header, then rows (section 2.4). No minimum, no maximum, no scrolling (NOT FOUND) | 39-160 |
| Background | `#FF2B2B2B`, opaque | 36 |
| Corner radius | **6** (no border, so 6 is the outline's radius) | 36 |
| Border | None (NOT FOUND) | 36 |
| Padding | 0 on every side: rows run edge to edge and the panel ends at the last row's foot | 36; CT/MenuPanelXamlTests.cs:74-82 |
| Shadow | A soft black shadow all round, no offset: what WPF's `DropShadowEffect` draws with `ShadowDepth=0`, `BlurRadius=24`, `Opacity=0.5`, `Color=Black` round a rounded rectangle of the panel's size and radius 6. Drawn **only outside** the panel (the panel's own area is cut out of it), reaching **25** out from every edge (`ceil(24) + 1`). Baked once to a bitmap; never animated. The falloff curve is WPF's effect's own: **UNVERIFIED** (a Gaussian-like blur of radius 24; in the pictures the page 1 to 10 px outside the panel's edge is visibly darker, fading by about 25) | 33-38; E/SoftShadow.cs:11-21, 105, 115-119 |
| Clipping | None, here or in any parent: the focus ring and its glow hang outside the panel | 6-17; CT/MenuPanelXamlTests.cs:160-180 |

### 2.3 Header

| Part | Value | Line |
|---|---|---|
| Title | `Margin="28,36,28,24"`: its box starts 28 in and 36 down, is 450 - 56 = **394** wide, and leaves 24 under it. **Bahnschrift** (regular), **33**, `#FFFFFFFF` | 41-42; T/Constants.xaml:230; CT/MenuPanelXamlTests.cs:123-131 |
| Title, on a menu or the flyout | One line, `NoWrap`, `CharacterEllipsis` at 394 wide | 44-46; CT/MenuPanelXamlTests.cs:133-143 |
| Title, on a prompt (`IsPrompt`) | Wraps on words at 394 wide, as many lines as it takes, no maximum (the ellipsis setting stays but never applies with no height limit) | 47-50; CT/MenuPanelXamlTests.cs:145-153 |
| Message | A second text under the title, only when the Dialog has one: `Margin="28,-14,28,20"` (so 10 under the title's line instead of 24, and 20 above the rows), **Segoe UI 26**, `#B3FFFFFF`, wraps. **None of the Tile menu's menus or prompts has a message** (every one passes `null`); only the Guide's "Force quit X?" does. Collapsed when null | 56-67; C/ExtensionCore.TileMenu.cs:107, 121, 160, 181; C/ExtensionCore.cs:733, 2429-2430 |
| Game cover, icon, subtitle, divider | **NOT FOUND**: the header is the title alone | 39-67 |

The title's text is the game's name on the menu (C/ExtensionCore.TileMenu.cs:107), the literal **"More options"** on the flyout (line 160), and the question on a prompt.

Header height, one title line: 36 + 39.6 + 24 = **99.6**. Each further line of a prompt's title adds 39.6. In the pictures the title's capitals span y 342..364 with the panel's top at 300, i.e. the ink of "Amber Circuit" starts 42 under the panel's top and 28 in (x 764).

### 2.4 Rows

One row per option, in a vertical stack, each a `Grid Height="72"` the panel's full width (E/MenuPanel.xaml:68-71).

| Part | Value (x from the panel's left edge, y from the row's top) | Line |
|---|---|---|
| Row | 450 x **72**; rows are 72 apart with nothing between them | 71; CT/MenuPanelXamlTests.cs:84-91 |
| Fill | `Margin="0,8"`: x 0..450, y 8..64 (**56** tall), radius **2**. `Transparent` at rest, `#FF404040` focused | 73, 93; CT/MenuPanelXamlTests.cs:39-47, 60-72 |
| Content | A horizontal stack, `Margin="32,0,20,0"`, vertically centred in the 72 (centre line y = 36) | 74 |
| Icon | A box **24** wide at x 32..56 (**centre x = 44**), glyph centred in it; font `Segoe Fluent Icons, Segoe MDL2 Assets`, **22**, `#FFFFFFFF` | 75-76; T/Constants.xaml:232; CT/MenuPanelXamlTests.cs:106-118 |
| Label | `Margin="32,0,0,0"` after the icon: starts at **x = 88**. **Segoe UI** (regular) **26**, `#FFFFFFFF`, one line | 77-78; T/Constants.xaml:228 |
| Label overflow | No wrap, no trimming set. A label wider than 450 - 88 - 20 = **342** is not shortened; whether WPF cuts it at the content's right edge (x 430) or lets it run on is **UNVERIFIED** (layout-clip internals). Every fixed label fits; only a game's own action name can be too long | 74-78 |
| Focus ring | Section 2.6 | 84-89 |
| Divider lines, chevrons, right-hand value text, checkmarks | **NOT FOUND**. More options has no arrow; nothing is drawn right of a label | 71-90 |

**The gap.** The row whose icon is `Customise` gets `Margin="0,21,0,0"`: the whole row, its fill and its ring included, starts **21 further down**, with no line drawn in the gap (E/MenuPanel.xaml:119-127; CT/MenuPanelXamlTests.cs:93-104). On the menu that is the next-to-last row, Customise Home / Customise games & apps; More options follows it directly, 72 on. The flyout and the prompts have no such row and so no gap.

Measured on the 5-row menu (panel top 300): ring tops and feet at y 399/471 for row 0, so rows 0..2 at 399, 471, 543, then Customise Home's ring at 636..708 (543 + 72 + 21) and More options' at 708..780, the panel's foot. Icon ink centred on x 779 = 735 + 44; label ink starting at x 825 (823 + the R's side bearing).

Glyphs (E/MenuPanel.xaml:97-156), all in the icon font:

| `DialogIcon` | Code point | Fluent name | Used by |
|---|---|---|---|
| Play | U+E768 | Play (outline triangle) | Play game; "Quit and start" |
| Quit | U+E711 | Cancel (X) | Quit |
| Cancel | U+E72B | Back (left arrow) | Cancel on every prompt |
| Favourite | U+E734 | FavoriteStar (outline star) | Add to Favourites |
| Unfavourite | U+E8D9 | Unfavorite (struck star) | Remove from Favourites |
| Manage | U+E7FC | Game (controller) | Manage game |
| Install | U+E896 | Download | Install |
| Uninstall | U+E74D | Delete (bin) | Uninstall, and the prompt's Uninstall |
| Customise | U+E771 | Personalize (brush) | Customise Home / Customise games & apps |
| MoreOptions | U+E712 | More (three dots) | More options |
| GameAction | U+E945 | LightningBolt | Each of the game's own actions |
| Hide | U+ED1A | Hide (struck eye) | Hide this game |
| Unhide | U+E7B3 | RedEye (eye) | Unhide this game |
| Hdr | U+E793 | Brightness (sun) | Enable HDR / Disable HDR |
| Extensions | U+EA86 | Puzzle | Extensions |
| RemoveGame | U+E738 | Remove (minus) | Remove, and the prompt's Remove |

(`Remove` U+E74D, `TurnOff`, `Restart`, `Sleep`, `Exit` belong to the Guide's menus, not this page: C/Dialog.cs:178-223.)

### 2.5 Rectangles

Panel x is always **735..1185** for the menu and for a prompt in the menu's place ((1920 - 450) / 2), and **812..1262** for the flyout and for a prompt in the flyout's place (735 + 77; E/TileMenuControl.xaml:24). Measured: 735..1184 and 812..1261 inclusive.

**The menu.** n rows, one of them Customise:

    H(n)  = 99.6 + 72 n + 21
    top   = (1080 - H) / 2,   bottom = 1080 - top

n is 4, 5 or 6: two rows always (the play row, the Favourites row), Manage game unless opened from the hub, Uninstall when its rule holds (section 3), then Customise and More options always.

| n | When | H | Panel (x, y, w, h) | Row tops (y), in order |
|---|---|---|---|---|
| 4 | From the hub, no Uninstall | 408.6 | 735, 335.7, 450, 408.6 | 435.3, 507.3, *gap*, 600.3, 672.3 |
| 5 | From a tile with no Uninstall; or from the hub with Uninstall | 480.6 | 735, 299.7, 450, 480.6 | 399.3, 471.3, 543.3, *gap*, 636.3, 708.3 |
| 6 | From a tile, with Uninstall | 552.6 | 735, 263.7, 450, 552.6 | 363.3, 435.3, 507.3, 579.3, *gap*, 672.3, 744.3 |

Within any of them: title box at (763, top + 36, 394 x 39.6); row i's fill at (735, rowTop + 8, 450 x 56); icon centre x **779**; label from x **823**; ring (727, rowTop, 466 x 72).

**The flyout.** m rows, no gap: `H = 99.6 + 72 m`, at x 812, **top = the menu's top** (it is not centred on its own; it hangs from the menu's top-left corner, 77 in). m is at least 3 (Hide, Extensions, Remove): 315.6 tall. Row j's top = menuTop + 99.6 + 72 j.

| m | H | With a 5-row menu behind (top 299.7) | With a 6-row menu behind (top 263.7) |
|---|---|---|---|
| 3 | 315.6 | y 299.7..615.3 | y 263.7..579.3 |
| 4 | 387.6 | ..687.3 | ..651.3 |
| 5 | 459.6 | ..759.3 | ..723.3 |
| 6 | 531.6 | ..831.3 | ..795.3 |
| each more | +72 | | |

The flyout does not scroll and is not clamped: with enough of a game's own actions it runs off the screen's foot (past 1080 from m = 10 over a 6-row menu, m = 10 over a 5-row one, m = 9 over a 4-row one), its last rows unreachable by eye though focus still goes to them. **NOT FOUND**: any handling of that.

**Prompts** (two rows, no gap), title of k lines: `H = 36 + 39.6 k + 24 + 144`.

| k | H | "Uninstall X?" (in the menu's place, centred) | "Remove X?" (in the flyout's place) |
|---|---|---|---|
| 1 | 243.6 | 735, 418.2, 450, 243.6 | 812, menuTop, 450, 243.6 |
| 2 | 283.2 | 735, 398.4, 450, 283.2 | 812, menuTop, 450, 283.2 |
| 3 | 322.8 | 735, 378.6, 450, 322.8 | 812, menuTop, 450, 322.8 |

The Remove prompt keeps the flyout's place exactly (same x, same top), only its height changes; the menu stays behind it, dimmed (E/TileMenuControl.xaml.cs:171-174). The Uninstall prompt **replaces** the menu: there is one centred panel, re-centred for its own height.

### 2.6 Focus

Exactly one row of the panel that has the pad is focused; there is no unfocused-but-selected state. Focus shows **instantly** (no fade, no slide between rows; NOT FOUND otherwise).

| Thing | Rest | Focused | Line |
|---|---|---|---|
| Fill | `Transparent` | `#FF404040`, x 0..450, 56 tall, radius 2 | 73, 93 |
| Icon, label | `#FFFFFFFF` | `#FFFFFFFF` (no change of colour, weight or size) | 75-78 |
| Ring | Hidden | `Border Margin="-8,0" BorderThickness="4" CornerRadius="7"`, brush `#FF2CA243` (`GlyphLightBrush`), no fill | 84-85; T/Constants.xaml:10 |
| Glow | Hidden | home.md section 4.3's **rectangular** variant (`XboxFocusGlow` under `XboxFocusGlowCache`, the ring's child, its own `Margin="-7.5"`), at 1:1, not scaled by s | 86-88; T/Constants.xaml:26-99, 184-191 |

Ring geometry: its outer edge is the row's full height (y 0..72) and stands **8 out past the panel** on the left and right (x -8..458, i.e. 727..1193 on screen); 4 thick; its inner edge is therefore 4 off the fill on all four sides, exactly as a Home tile's ring stands 4 off its tile. The fill plays the tile's part: everything in home.md section 4.3 that is said relative to "the tile edge" is relative to the **fill's** edge here (centre line 6 outside the fill; the glow reaches 21.5 outside the fill and 9.5 inside it; the four corner spots are centred 3 inside the fill's corners). WPF corner semantics as home.md 4.1: `CornerRadius="7"` is the stroke's centre line, so the ring is 9 round outside and 5 inside. (CT/MenuPanelXamlTests.cs:30-37, 49-72.)

Consequences a port must keep: the ring and glow of the **last** row hang below the panel's foot by nothing (ring) and 13.5 (glow); the ring of any row hangs 8 and the glow 21.5 past the panel's sides, over the dim; in the flyout the ring's left end lies over the dimmed menu. Measured: ring strokes at x 727..730 and 1189..1192, y 399..403 and 467..471; fill `#404040`.

A focused row behind another panel keeps its fill, ring and glow: with the flyout up, the menu's More options row is still drawn focused, at the menu's 0.45 opacity (visible in `data-more-options.jpg`).

**Disabled rows: NOT FOUND.** There is no disabled drawing and no row is ever disabled. A row that cannot apply is left out (section 3); a row that can no longer act when chosen simply does nothing (section 3.3).
**Pressed state: NOT FOUND.** A acts on the press; nothing is drawn for it.
**Mouse:** release builds draw no hover; a click on the dim closes the menu, a click on a row does nothing. Pointing and clicking rows is Debug builds only (E/MenuPanel.xaml.cs:19-32; E/TileMenuControl.xaml.cs:279-282).

---

## 3. The menu's rows

Built in this order by `TileMenuFor` (C/ExtensionCore.TileMenu.cs:73-109). Title = the game's name. Focus starts on the **first row** every time (C/Dialog.cs:29; CT/TileMenuTests.cs:31-41).

| # | Label | Icon | Shown when | Renamed when | Source (C/ExtensionCore.TileMenu.cs) |
|---|---|---|---|---|---|
| 1 | **Play game** | Play | Always one of the three | **Quit** (icon Quit) when this game is the running game; else **Install** (icon Install) when the game is not installed; else Play game. Running is tested first, so a running game reads Quit whatever its installed flag says | 79-81 |
| 2 | **Add to Favourites** | Favourite | Always | **Remove from Favourites** (icon Unfavourite) when the game is a favourite | 82-84 |
| 3 | **Manage game** | Manage | Unless the menu was opened from the Game hub | - | 87-94 |
| 4 | **Uninstall** | Uninstall | Only when the game is installed **and** a library brought it in (not added by hand, `!IsCustom`) **and** it is not the running game (M4, section 13) | - | 96-100 |
| | *gap of 21* | | | | E/MenuPanel.xaml:124-127 |
| 5 | **Customise Home** | Customise | Always | **Customise games & apps** while My games & apps is the page showing (the rule is `MyGames.IsShown` alone, read as the menu opens; whether a Game hub opened from My games & apps still counts as "shown" is the page's state, UNVERIFIED here) | 102-104 |
| 6 | **More options** | MoreOptions | Always | - | 105 |

### 3.1 Rows per state

"T" = opened on a tile or grid (Manage game present); "H" = opened from the Game hub (no Manage game). The Favourites row and the two last rows are in every menu.

| Game state | Row 1 | Favourites row | Manage game | Uninstall | Rows (T / H) | Source |
|---|---|---|---|---|---|---|
| Installed, from a library, not running | Play game | per favourite | T only | **Yes** | 6 / 5 | CT/TileMenuTests.cs:173-180, 677-693 |
| Not installed | **Install** | per favourite | T only | No | 5 / 4 | CT/TileMenuTests.cs:246-255 |
| Installing or queued (M4) | **Install** (still: the game is not installed yet; choosing it does nothing more) | per favourite | T only | No | 5 / 4 | C/ExtensionCore.Queue.cs:258-269; CT/TileMenuTests.cs:283-293 |
| Running | **Quit** | per favourite | T only | No | 5 / 4 | CT/TileMenuTests.cs:206-215 |
| Being uninstalled, not finished (M4) | Play game (still installed until the client is done) | per favourite | T only | **Yes** (asks again; confirming asks the client nothing more) | 6 / 5 | C/ExtensionCore.Uninstall.cs:36-53, 76-86; CT/TileMenuTests.cs:345-357 |
| Added by hand, installed | Play game | per favourite | T only | **No** | 5 / 4 | CT/TileMenuTests.cs:295-304 |
| Added by hand, not installed | Install | per favourite | T only | No | 5 / 4 | 79-81, 97 |
| Hidden | As its other state says | per favourite | T only | As its other state says | - | Hidden changes only the flyout (section 4) |
| Favourite | As its other state says | **Remove from Favourites** | T only | As its other state says | - | CT/TileMenuTests.cs:551-575 |

The pictures show the 5-row T menu of a seed game no library owns: Play game, Add to / Remove from Favourites, Manage game, Customise Home, More options.

### 3.2 What A on each row does (in brief; section 10 has the flows)

Every row but Uninstall and More options **closes the menu first, then acts** (`FromTileMenu`, C/ExtensionCore.TileMenu.cs:188-192), so focus is back on the tile before anything else opens.

| Row | A | Source |
|---|---|---|
| Play game | Menu closes; the game is launched (Launch screen), or with another game running the Guide asks "Quit X and start Y?" (section 5.3) | 81; C/ExtensionCore.cs:717-746 |
| Install | Menu closes; the game is installed through its store client (M4) | 80; C/ExtensionCore.Queue.cs:258-269 |
| Quit | Menu closes; the shared quit flow starts | 79; C/ExtensionCore.cs:693-710 |
| Add to / Remove from Favourites | Menu closes; the game's favourite flag is set / cleared | 82-84 |
| Manage game | Menu closes; the game's Game hub opens, focus on its Play | 89-93 |
| Uninstall | The menu is **replaced** by the prompt "Uninstall <game>?" (section 5.1). Nothing is uninstalled yet | 99, 115-126 |
| Customise Home / Customise games & apps | Menu closes; Settings opens on Theme settings' Personalisation, focus in its boxes | 102-104; C/ExtensionCore.cs:1746-1748 |
| More options | The flyout opens over the menu; the menu stays up, its focus still on More options | 105, 133-162 |

### 3.3 Stale rows

The rows are a snapshot. If the game starts, stops, finishes installing or is unfavourited elsewhere while the menu is up, the menu still shows what it was built with (NOT FOUND: any rebuild). Each action checks again when chosen and does nothing if it no longer applies: Quit on a game that has stopped (C/ExtensionCore.cs:695-698), Play game while a game is quitting or launching or on the game now running (719-722), Install on a game already installing or queued (C/ExtensionCore.Queue.cs:261-268), setting a favourite or hidden flag to what it already is (E/PlayniteHost.cs:236, 501). The menu still closes.

---

## 4. More options: the flyout

A second `MenuPanel`, "drawn alike, over the menu and as the Guide's Flyout sits beside its tab" (E/TileMenuControl.xaml:11-13). Built by `ShowMoreOptions` as it opens, from the library as it is then (C/ExtensionCore.TileMenu.cs:133-162). If the game has gone from the library meanwhile, the whole Tile menu closes instead (135-140).

**Rectangle:** x 812..1262 (the menu's left edge + 77), top = the menu's top, 450 wide, `99.6 + 72 m` tall (section 2.5). It covers the menu's right 373 of 450 and stands 77 past the menu's right edge; the menu's left 77 stays visible, dimmed, with the left ends of its title and icons (`data-more-options.jpg`: flyout at x 812..1261, y 300..615 for 3 rows).

**The menu behind it:** the whole menu (panel, shadow, text, its focused More options row with ring and glow) is drawn at **opacity 0.45**, as one group, set at once with no fade (E/TileMenuControl.xaml.cs:38, 179). The page's dim is unchanged; there is no second dim. Where the flyout's own shadow falls on the menu it darkens it further.

**Title:** "More options" (one line). **Focus** starts on its first row (C/Dialog.cs:29; CT/TileMenuFlyoutTests.cs:49).

Rows, in this order:

| # | Label | Icon | Shown when | A | Source (C/ExtensionCore.TileMenu.cs) |
|---|---|---|---|---|---|
| 1..k | The name of each of the game's actions that do not play it, in the game's own order (e.g. "Launch config tool", "Open mods folder") | GameAction | One row per such action; none for a game with none | Whole Tile menu closes; that action is run as Playnite's game menu runs it | 143-145; E/PlayniteLibrary.cs:97; E/PlayniteHost.cs:201-231 |
| k+1 | **Hide this game** | Hide | Always | Whole menu closes; the game is hidden | 146-148 |
| | renamed **Unhide this game** (icon Unhide) | | when the game is hidden | Whole menu closes; shown again | 147 |
| k+2 | **Enable HDR** | Hdr | Only when the display can show HDR (the primary display, asked as the flyout opens) | Whole menu closes; the game's "turn Windows' HDR on while it runs" flag is set | 149-154; E/PlayniteHost.cs:243-272 |
| | renamed **Disable HDR** (same icon) | | when that flag is already on for the game | Whole menu closes; flag cleared | 151-152 |
| next | **Extensions** | Extensions | Always | Whole menu closes; Playnite's own Extensions picker opens for the game | 156; E/PlayniteHost.cs:274-302 |
| last | **Remove** | RemoveGame | Always (for a running, installing or uninstalling game too; refused later) | The flyout is **replaced** by the prompt "Remove <game>?" (section 5.2) | 157, 175-186 |

Not offered: Playnite's Edit fields (C/ExtensionCore.TileMenu.cs:130-131), nor anything else of Playnite's game menu (category, completion status, "open install location": NOT FOUND).

Row counts: m = k + 3 without HDR, k + 4 with. Smallest flyout: Hide this game, Extensions, Remove (CT/TileMenuFlyoutTests.cs:40-50). With two actions on an HDR display: "Launch config tool", "Open mods folder", "Hide this game", "Disable HDR", "Extensions", "Remove" (52-62).

Like the menu's, the flyout's rows are a snapshot and its panel does not scroll (section 2.5).

---

## 5. Prompts

All three are the same `MenuPanel`, with `IsPrompt` true: the question as the title, **wrapping** so it is read whole; no message; two rows drawn exactly as menu rows (icon, label, fill, ring); **focus starts on the first row**, which is the confirming one. There are no separate button shapes, no default-button highlight other than focus, and no "X" to close.

`E/WideDialogPanel.xaml` is **not** used by any of them: that is the band across the screen for the Guide's Power dialog ("What do you want to do?", C/Dialog.cs:42-44, 57), the only wide Dialog. `T/Views/MessageBox.xaml` is Playnite's own message box restyled (560 minimum width, radius 8, tile buttons in a row); the Tile menu meets it only as Playnite's error when Remove is refused (section 10, item 9).

### 5.1 "Uninstall <game>?" (M4 with the Uninstall row)

| | | Source |
|---|---|---|
| Where | In the **menu's place**: the menu's Dialog is swapped for the prompt's, so one panel is centred on the screen at the prompt's own height (section 2.5). The flyout is never up at this point | C/ExtensionCore.TileMenu.cs:115-126 |
| Title | `Uninstall {name}?` | 120 |
| Row 1 (focused) | **Uninstall**, icon Uninstall (bin): the whole Tile menu closes, then the game is uninstalled (section 10, item 6) | 124 |
| Row 2 | **Cancel**, icon Cancel (back arrow): back to the menu, **focus on its Uninstall row** | 125, 117 |
| B | Same as Cancel | 123; CT/TileMenuTests.cs:359-390 |

The menu comes back as the same object, so its focus is where it was left: on Uninstall. Down from there steps over the gap to Customise (CT/TileMenuTests.cs:392-404).

### 5.2 "Remove <game>?"

| | | Source |
|---|---|---|
| Where | In the **flyout's place** (x 812, the menu's top), the menu still dimmed behind it | C/ExtensionCore.TileMenu.cs:175-186; E/TileMenuControl.xaml.cs:171-174 |
| Title | `Remove {name}?` | 180 |
| Row 1 (focused) | **Remove**, icon RemoveGame (minus): the whole Tile menu closes, then the game is taken out of the library (section 10, item 9) | 184 |
| Row 2 | **Cancel**, icon Cancel: back to the flyout, **focus on its Remove row** | 185, 177 |
| B | Same as Cancel | 183; CT/TileMenuFlyoutTests.cs:165-198 |

### 5.3 "Quit X and start Y?" (the Guide's prompt; reached from Play game)

Asked by `PlayGame` when another game is running, from the Tile menu, the Game hub and the Guide alike (C/ExtensionCore.cs:717-746). It is **not** part of the Tile menu: the Tile menu has already closed (and is flying out) when it shows, and it is drawn by the **Guide** in the Overlay. Its full drawing belongs with the Guide (M2); what the Tile menu needs of it:

| | | Source |
|---|---|---|
| Where | The same `MenuPanel`, centred in the Overlay's 1920x1080 canvas: x 735..1185, y (1080 - H) / 2, H from section 2.5's prompt formula | E/OverlayWindow.xaml:1382-1383 |
| Behind it | A flat `#B3000000` over everything in the Overlay, the Guide's panel included, shown and hidden at once with the prompt; under that the Guide's panel as a blurred picture of itself. If the Guide was closed it opens for the prompt (its panel sliding in behind); with Settings' Guide off the prompt shows on its own over the Overlay's dim | E/OverlayWindow.xaml:1370-1381; E/OverlayWindow.xaml.cs:150-192; C/ExtensionCore.cs:2406-2416 |
| Title | `Quit {running} and start {this}?`, wrapping | C/ExtensionCore.cs:732; CT/TileMenuTests.cs:193-204 |
| Row 1 (focused) | **Quit and start**, icon Play: the prompt goes; the running game is quit through the shared quit flow and this game is launched once it has stopped | 736-744 |
| Row 2 | **Cancel**, icon Cancel: the prompt goes | 745 |
| B | Same as Cancel | 735, 803-808 |
| After | With the Guide on, the Guide stays open behind (B closes it); with the Guide off, the Overlay closes with the answer | 1131-1141, 2419-2422 |
| Opening animation of the panel | **NOT FOUND**: in the Overlay the prompt and its dim appear at once (only the Tile menu's host flies its panel) | E/OverlayWindow.xaml.cs:106-109 |

The quit flow's own follow-up, "Force quit X?" with the message "It hasn't closed. Anything unsaved will be lost." after 10 s, is likewise the Guide's (C/ExtensionCore.cs:88, 2425-2433).

---

## 6. Navigation

While a Tile menu is up it has the pad: every button goes to it and none reaches the page (C/ExtensionCore.TileMenu.cs:49-50). The "active" panel is the flyout (or the prompt in its place) if there is one, else the menu (or the prompt in its place) (C/ExtensionCore.cs:2289).

| Input | On the menu | On the flyout | On a prompt | Source |
|---|---|---|---|---|
| D-pad / stick Up | Previous row; **stops** at the first row (no wrap, no sound) | Same | Same (two rows) | C/Dialog.cs:104-111; C/ExtensionCore.cs:2186-2199; CT/TileMenuTests.cs:577-593 |
| Down | Next row; **stops** at the last. The gap before Customise is not a stop | Same | Same | Same; CT/TileMenuTests.cs:392-404 |
| Left / Right | Nothing (no sound). Right on More options does **not** open the flyout; Left in the flyout does **not** close it | Nothing | Nothing | C/Dialog.cs:109; CT/TileMenuFlyoutTests.cs:200-219 |
| Held direction | Repeats: first repeat **700 ms** after the press, then every **80 ms**, until release; stops dead at an end | Same | Same | C/ExtensionCore.cs:84-85, 859-865, 2169-2180; CT/TileMenuTests.cs:595-606 |
| A | Chooses the focused row (section 3.2) | Chooses (section 4) | Answers | C/ExtensionCore.cs:827-831, 2353-2357 |
| B | Closes the Tile menu; focus is where it always was, on what opened it | Closes the flyout only: back to the menu, **focus on More options** | Back to the panel that asked, **focus on the row that asked** (Uninstall, or Remove) | C/ExtensionCore.cs:803-808; C/ExtensionCore.TileMenu.cs:107, 117, 160, 165-169, 177; CT/TileMenuFlyoutTests.cs:178-198 |
| Menu (Start) | Nothing (it does not close the menu, and opens no second one) | Nothing | Nothing | C/ExtensionCore.cs:836-843 (both need no dialog up) |
| View (Back) | Nothing | Nothing | Nothing | 844-849 |
| X, Y, LB, RB, LT, RT, stick clicks | Nothing. Y does not open Search while a menu is up | Nothing | Nothing | C/ExtensionCore.Search.cs:74-80; C/ExtensionCore.cs:855-858 |
| Guide button | The Tile menu (flyout and prompt with it) closes without an answer as the Guide opens: on a tap, 0.3 s after the press; on a 0.7 s hold, as Power opens. No close sound of the menu's own | Same | Same (the prompt's question is dropped, nothing is uninstalled or removed) | C/ExtensionCore.cs:79-83, 1977-1979; CT/TileMenuTests.cs:642-651 |
| Keyboard Esc | Closes the whole Tile menu at once, from any level, no sound. Every other key is swallowed | Same | Same | E/TileMenuControl.xaml.cs:233-253 |
| Mouse click on the dim | Closes the whole Tile menu, no sound | Same | Same | E/TileMenuControl.xaml.cs:279-282; E/TileMenuControl.xaml:17 |

**B is one level at a time:** Remove prompt -> flyout (on Remove) -> menu (on More options) -> closed; Uninstall prompt -> menu (on Uninstall) -> closed. Each level keeps its own focus because each panel's Dialog object is kept and put back, not rebuilt (C/ExtensionCore.TileMenu.cs:117, 177).

**Focus memory:** none across openings. Every opening of the menu starts on its first row; every opening of the flyout starts on its first row, even straight after B from it (it is rebuilt each time, C/ExtensionCore.TileMenu.cs:133-162).

**The button that closed it is held.** A press made while the menu was up is ignored by the page until that button is released, even though the press itself closed the menu: the A that chose Play game must not also press the Game hub's Play under it or the tile, and the B that closed the menu must not also close the hub or leave the Groups (C/ExtensionCore.cs:190-194, 793, 797-800, 1333; CT/TileMenuTests.cs:608-628, 727-757). A direction held as the menu closes does not move focus on the page either (the repeat is stopped as it closes, C/ExtensionCore.TileMenu.cs:18).

**Other ways it closes, all without an answer and without a sound** (`CloseTileMenu`, C/ExtensionCore.TileMenu.cs:14-22): the Dashboard's window losing the foreground (E/TileMenuControl.xaml.cs:274-277); Start-up covering the Dashboard (C/ExtensionCore.Startup.cs:294); Y on an Idle screen game card, "See game" (M5; C/ExtensionCore.Idle.cs:183). The Idle screen's dim shows over an open Tile menu and the wake press is swallowed, the menu still up after (C/ExtensionCore.cs:757-761).

**A game exiting while the menu is up:** NOT FOUND, no handling; the menu stays (only the Guide's game prompts are taken away, C/ExtensionCore.cs:677-687).

---

## 7. Animation

`E/TileMenuControl.xaml.cs`. All durations go through `Motion.Of`, so with **Reduce motion** on each is 0: things are in place, or gone, at once, fades included (E/Motion.cs:46-47; E/TileMenuControl.xaml.cs:211-212).

| What | Normal | Reduce motion | Lines |
|---|---|---|---|
| Dim, opening | Shown **at once**, full `#99000000`. No fade | Same | 164-166; E/TileMenuControl.xaml:8-10 |
| Menu panel, opening | Translate Y from **+80** to 0 over **250 ms**, `CubicEase EaseOut`; opacity 0 -> 1 over the same 250 ms, **linear**. It "flies up" | In place at once | 26, 30, 168, 209-231 |
| Dim, closing | Hidden **at once**; the page under it takes input again at once | Same | 196-197 |
| Menu panel, closing | Translate Y from where it is to **+80** over **150 ms**, `CubicEase EaseIn`; opacity to 0 over 150 ms, linear. It keeps showing the rows it held; it is gone (collapsed) when the move ends | Gone at once | 27, 198-206 |
| Close interrupted by a reopen | The open animation takes over, starting again from +80 and opacity 0 (it sets its From) | - | 200-205, 218-222 |
| Open interrupted by a close | The close continues from wherever the panel is (to-only), "so a quick close never jumps" | - | 217-218 |
| Flyout, opening | Translate X from **-48** to 0 over **120 ms**, `CubicEase EaseOut`; opacity 0 -> 1 over 120 ms, linear. "In from the left quickly, as the Guide's" | In place at once | 33-35, 185-187 |
| Menu behind the flyout | Opacity 1 -> **0.45** at once as the flyout opens; back to 1 at once as it goes | Same | 38, 179 |
| Flyout, closing (B) | Gone **at once**, no animation | Same | 177-183; E/MenuPanel.xaml:22-25 |
| Remove prompt in the flyout's place, and back | Swapped **at once**: no slide, no fade (the flyout was already showing) | Same | 171-174, 180-183 |
| Uninstall prompt in the menu's place, and Cancel / B back to the menu | The menu's Dialog changes to another non-null one, which runs the **opening fly-up again** (from +80, opacity 0, 250 ms) for the prompt, and again for the menu coming back. The dim stays up throughout. Read from the code (every change of `HomeState.TileMenu` to a Dialog calls `Open`); no picture or recording shows it | In place at once | 137-159, 162-169; C/HomeState.cs:78-82, 109-118 |
| A row chosen in the flyout or on the Remove prompt | The flyout goes at once and the menu is lit again (opacity 1) in the same instant, then the menu flies down and out as on any close | Gone at once | C/ExtensionCore.TileMenu.cs:19-20; 139-159, 177-183 |
| Row focus moving | Instant; nothing animates | Same | E/MenuPanel.xaml:92-95 |

The fly moves the **panel only** (its shadow and its rows with it); the dim never moves. The flyout does not fly with the menu on opening (it cannot be open yet) and is already gone when the menu flies out.

Opening and closing are the same from every opener: Home, My games & apps and the Game hub do not move, fade or blur when the menu opens over them, beyond the dim.

---

## 8. Sounds

Files and volume as home.md section 9 (`E/Sounds/`, Playnite's interface volume). All are played by the extension; Playnite's own focus sounds are not involved, since focus never moves.

| Event | Sound | Source |
|---|---|---|
| The Tile menu opens (any opener) | `PromptOpen.wav` | C/ExtensionCore.TileMenu.cs:39, 69 |
| Focus moves to another row (menu, flyout or prompt) | `Navigate1..4.wav`, the next in turn each time (1, 2, 3, 4, 1 ...); the turn is shared with every other Navigate in the Bundle | C/ExtensionCore.cs:2191-2196; E/PlayniteSounds.cs:25, 121-128 |
| Up at the first row, Down at the last, Left, Right | None | C/ExtensionCore.cs:2182-2185, 2192 |
| A on a row of the **menu** or the **flyout** (every row: Play game, Favourites, Manage game, Customise, also Uninstall, More options and Remove, which only open the next level) | `Enter.wav` | C/Dialog.cs:72; C/ExtensionCore.cs:2353-2357 |
| A on a row of a **prompt** (Uninstall, Remove, Quit and start, Cancel alike) | `PromptClose.wav` | Same |
| B, at every level (prompt -> panel, flyout -> menu, menu -> closed) | `PromptClose.wav` | C/ExtensionCore.cs:803-806 |
| The flyout opening; a prompt taking a panel's place | No sound of its own beyond the `Enter.wav` of the A that asked (no `PromptOpen.wav`) | C/ExtensionCore.TileMenu.cs:115-126, 159-161 |
| Play game with another game running | `Enter.wav` (the row), then `PromptOpen.wav` as the Guide's prompt shows | C/ExtensionCore.cs:2413-2415 |
| Closed by Esc, a click on the dim, the Guide button, losing the foreground, Start-up | None from the menu (the Guide plays its own as it opens) | C/ExtensionCore.TileMenu.cs:14-22 |

---

## 9. Hint bar

**Nothing while a Tile menu is up**, at any level: the bar is hidden the moment the menu opens and comes back, unchanged, the moment it closes (it follows `Home.TileMenu`, not the fly-out's end). The menu has no hints of its own: no "A Select", no "B Back" (NOT FOUND). (C/ExtensionCore.Hints.cs:70-76, 88; CT/MoreOptionsHintTests.cs:115-130, 290-301.) It is also hidden while the Guide's "Quit X and start Y?" is up (the Guide covers the Dashboard, line 76).

What the bar says **before** the menu opens is the opener's page's business, but it is how the player learns of the menu, so for reference (drawing: home.md section 10.4):

| Focus | Hint bar | Source |
|---|---|---|
| A Recent row tile on Home | No bar (Menu still opens the menu) | C/ExtensionCore.Hints.cs:83; CT/MoreOptionsHintTests.cs:76-83 |
| A Group's game card on Home | Menu "More options", then Y "Search" | 85-89; CT/MoreOptionsHintTests.cs:40-51 |
| A game on My games & apps (Play history, Manage Queue, Games' grid, Full library's grid) | Menu "More options" (plus Y "Search" on Games' grid only). On Manage Queue the pad's **View** opens the menu as well as Menu | 83-90; CT/MoreOptionsHintTests.cs:203-244; C/ExtensionCore.cs:844-849 |
| The Game hub | Menu "More options" | 84-87; CT/MoreOptionsHintTests.cs:315-322 |

The hint's label is "More options" although the button opens the Tile menu; the menu's last row has the same name. They are different things.

---

## 10. What each action then does

Brief, with the source; each store-facing flow is specified fully where it belongs (Launch screen: M1; Queue and installs: M4; Toasts: M2).

1. **Play game.** `PlayGame` (C/ExtensionCore.cs:717-746): nothing if a game is quitting or launching, or if it is this game that runs; with no game running, launches it through the library (Playnite's `StartGame`, C/ExtensionCore.cs:2499-2518; E/PlayniteHost.cs:105-108) under the Launch screen; with another running, the Guide's prompt (section 5.3).
2. **Quit.** `QuitGame` (C/ExtensionCore.cs:693-710): asks the running game's window to close gracefully; if it has not gone in 10 s the Guide asks "Force quit X?" (88, 2425-2433). When the game stops, the Dashboard returns to Home with the Recent row on that game (home.md 7.4).
3. **Install** (M4). `InstallThroughTheme` (C/ExtensionCore.Queue.cs:258-269): if the store client can install it without its windows (Steam: `app_install` typed into the running client's console, else `-silent +app_install` handed to steam.exe; E/SteamDownloads.Install.cs:81-97), it is asked, the game joins the Queue as "Queued" with a Queued Toast, and an open Game hub for it closes (C/ExtensionCore.Queue.cs:275-289); otherwise Playnite's own Install runs, client windows and all (E/PlayniteHost.cs:110); a game already installing or queued: nothing.
4. **Add to / Remove from Favourites.** Sets the flag in the library (E/PlayniteHost.cs:498-508). Nothing else happens at once: no Toast, no sound beyond the row's `Enter.wav`, no animation. The library's change comes back as a new snapshot, and Home's rows are rebuilt from it (C/ExtensionCore.cs:277-295): the default **Favourites** Group (favourite, not hidden, by name; home.md 6.1) gains or loses the game, as does any preset Group that filters on favourites. If the focused Group card's game left its Group, focus is put back "in the same place" in the rebuilt Groups (E/GroupsControl.xaml.cs:114-121); which card that is when the game is gone is home.md's to say (**UNVERIFIED** here).
5. **Manage game.** Opens the game's Game hub with focus on Play (C/ExtensionCore.TileMenu.cs:89-93; C/ExtensionCore.Queue.cs:233-237; C/IPlayniteHost.cs:21-25). B on the hub returns to the tile, as for a hub opened by A. (`ListPlayHistoryInMainList` before it is Playnite plumbing, C/ExtensionCore.cs:1603-1615.) Unlike A on a tile there is no 300 ms wait.
6. **Uninstall, confirmed** (M4). `UninstallThroughTheme` (C/ExtensionCore.Uninstall.cs:59-69). If the client can uninstall silently (an installed Steam game with a manifest, not installing, not running; lines 43-53; E/SteamDownloads.Uninstall.cs:26-44): Steam is asked (`app_uninstall` over its console, else `-silent -console +app_uninstall`; E/SteamDownloads.Uninstall.cs:15-22), once; the game is then checked **every 1 s** until a library snapshot shows it not installed, when the **Uninstalled Toast** "<Game> was uninstalled" with the game's cover is queued behind any Toast showing (lines 17, 91-126, 133-148). After **10 min** unfinished it is given up on, logged, no Toast (23, 115-120). A game that leaves the library meanwhile is dropped silently (135-139). Any other game: Playnite's own Uninstall, the library plugin's route, client windows and all, and no Toast (67; CT/UninstallTests.cs:216-232). Nothing shows on the tile while it uninstalls (no "Uninstalling..." status: NOT FOUND).
7. **Customise Home / Customise games & apps.** Settings opens on Theme settings, page Personalisation, focus in the page's boxes, remembering the page and the game it came from (C/ExtensionCore.cs:1691-1700, 1746-1748). B steps back Personalisation -> Theme settings' boxes -> out: to Home with focus on the tile whose menu opened it (Recent row or Group), or to My games & apps on the same nav item (Manage Queue included) and that game's tile (C/ExtensionCore.cs:1815-1834; CT/TileMenuTests.cs:406-511). The row does nothing with the game itself; the game only says where to come back to.
8. **Hide / Unhide this game.** Sets the library's hidden flag (E/PlayniteHost.cs:233-241). A hidden game leaves the Recent row, the default Favourites Group, Play history and Search with the next snapshot (C/ExtensionCore.cs:1393, 1457, 2533; C/ExtensionCore.Search.cs:256-264). Unhide can only be reached where hidden games are listed: Full library under a filter that shows them (Playnite's; `filters.md`).
9. **Remove, confirmed.** `RemoveGame` (E/PlayniteHost.cs:304-331): a game installing, running, launching or uninstalling is refused with Playnite's own error box ("LOCGameRemoveRunningError"); otherwise a library's game is first put on Playnite's import exclusion list, so its library's next update does not bring it back, and then the game is deleted from the database. A game added by hand is just deleted. No Toast.
10. **Enable / Disable HDR.** Sets the game's `EnableSystemHdr` (E/PlayniteHost.cs:264-272): Playnite then turns Windows' HDR on while that game runs. Nothing visible at once.
11. **Extensions.** Opens Playnite's Extensions picker window for the game, listing every extension's game-menu items (E/PlayniteHost.cs:274-302). It is Playnite's window, drawn by the theme as a list of 450 x 72 rows centred over a dim (`data-extensions.jpg`: one row "Playnite Achievements" with a chevron, at x 735..1185, y 504..576), not by the Tile menu.
12. **A game's own action.** Runs the action (a file, URL, script or emulator action) as Playnite's game menu would (E/PlayniteHost.cs:201-231). No Launch screen, no play-session tracking: it is not a play action.

---

## 11. Empty, loading and error states

- **Empty:** cannot be. The menu always has at least 4 rows, the flyout at least 3.
- **Loading:** none. Rows are built synchronously from the library snapshot in memory; nothing is fetched, there is no Spinner and no placeholder (NOT FOUND).
- **A game that has left the library:** Menu on its tile, or the hub's Manage, opens nothing, silently (C/ExtensionCore.TileMenu.cs:32-36, 60-64; CT/TileMenuTests.cs:709-715). If it leaves between the menu opening and More options being chosen, the whole menu closes with no message (135-140).
- **An action that fails:** the menu has already closed. Errors are whoever's the action is: Playnite's error box for a refused Remove or a failed game action; the log alone for an uninstall given up on after 10 min, a missing Extensions picker or a failed Steam command (E/PlayniteHost.cs:214-229, 284-300; C/ExtensionCore.Uninstall.cs:114-120). The Tile menu draws no error of its own (NOT FOUND).
- **A very long game name:** the menu's title is cut with an ellipsis at 394 wide; a prompt's title wraps to as many lines as it needs and the panel grows.
- **Too many rows for the screen:** the flyout runs off the foot (section 2.5). The menu cannot (6 rows at most).

---

## 12. Text rules, what the pictures confirmed, and small traps

### 12.1 Text rules

| Text | Font | Size | Colour | Wrap / trim |
|---|---|---|---|---|
| Menu title (the game's name) | Bahnschrift Regular | 33 | `#FFFFFFFF` | 1 line, character ellipsis, 394 wide |
| Flyout title ("More options") | Bahnschrift Regular | 33 | `#FFFFFFFF` | 1 line (it fits) |
| Prompt title (the question) | Bahnschrift Regular | 33 | `#FFFFFFFF` | Wraps on words at 394 wide, no line limit, left-aligned |
| Message (Guide's Force quit only) | Segoe UI Regular | 26 | `#B3FFFFFF` | Wraps at 394 wide |
| Row label | Segoe UI Regular | 26 | `#FFFFFFFF`, focused or not | 1 line, no trim (overflow UNVERIFIED, section 2.4) |
| Row icon | `Segoe Fluent Icons`, fallback `Segoe MDL2 Assets` | 22, centred in a 24 box | `#FFFFFFFF` | - |

Sources: E/MenuPanel.xaml:41-67, 75-78; T/Constants.xaml:228-232. Nothing is bold, upper-cased or dimmed for state. Labels are fixed English strings in the core (C/ExtensionCore.TileMenu.cs:79-105, 143-157), not localised.

Strings, exactly: "Play game", "Install", "Quit", "Add to Favourites", "Remove from Favourites", "Manage game", "Uninstall", "Customise Home", "Customise games & apps", "More options", "Hide this game", "Unhide this game", "Enable HDR", "Disable HDR", "Extensions", "Remove", "Cancel", "Quit and start"; titles "More options", "Uninstall {name}?", "Remove {name}?", "Quit {running} and start {name}?"; Toast "{name} was uninstalled" (C/ExtensionCore.Uninstall.cs:14).

### 12.2 What the pictures confirmed

Measured with a script on the 1920x1080 JPEGs named in the header (edges found by colour runs along a row or column of pixels):

| Number | Source says | Picture shows | Verdict |
|---|---|---|---|
| Panel x | 735..1185 | 735..1184 inclusive (450 px) | Confirmed |
| Panel background | `#2B2B2B` | `#2A2C2B` (JPEG) | Confirmed |
| 5-row panel y | 299.7..780.3 | 300..about 780 | Confirmed to 1 px |
| Header height | 99.6 (Bahnschrift line 39.6) | first row's ring starts at y 399, 99 under the panel's top | Confirmed to 1 px; the 1.2 em line holds |
| Row pitch | 72 | ring tops 399, 471; rows at 543, 636, 708 | Confirmed |
| Gap before Customise | 21 | Customise's ring at 636 = 543 + 72 + 21 | Confirmed |
| Ring | 4 thick, 8 past the panel each side, row's full 72 | x 727..730 and 1189..1192; y 399..403 and 467..471 | Confirmed |
| Fill | `#404040`, 56 tall, edge to edge | `#404040` from x 735 to 1184 inside the ring | Confirmed |
| Icon centre, label start | 44 and 88 in | icon ink centred on x 779; label ink from x 825 | Confirmed |
| Title | Bahnschrift 33 at 28, 36 | ink x 764..964 for "Amber Circuit", capitals y 342..364 (23 tall = 0.7 em of 33) | Confirmed |
| Dim | `#99000000` | page pixels at 0.40 of their undimmed value | Confirmed |
| Flyout | 77 in from the menu's left, same top, 99.6 + 72 m tall | x 812..1261, y 300..615 for m = 3 | Confirmed |
| Menu behind the flyout | opacity 0.45, still drawn focused on More options | menu visibly translucent, More options' ring and fill dimmed | Confirmed by eye (0.45 itself not measured) |
| Focused tile behind the menu | keeps ring and size | the Recent row tile is grown, ringed, title band up | Confirmed |
| Row label size | 26 | about 22 | **Not confirmed**: pictures predate #229; follow the source |
| Shadow | blur 24, 0.5 | a darker band about 10 px wide is plain outside the panel's edge; its full reach is lost in the JPEG | Present; curve not confirmed |
| Prompts, 4- and 6-row menus, Install / Quit / Uninstall rows, flyout with actions or HDR, every animation, every sound | - | No picture or recording | **Not confirmed** (source and tests only) |

### 12.3 Small traps

- **Not Home's scale.** Nothing here is multiplied by s = 1.114. The ring is 4 thick here and 4.46 on a Home tile.
- **Centred on the screen, not on the tile.** And centred by its own height: a 4-row menu's top is 36 lower than a 5-row one's.
- **The flyout hangs from the menu's top-left corner (+77, +0).** It is not centred vertically and not beside the More options row.
- **The ring is wider than the panel.** 8 out each side, with the glow a further 13.5: draw the row's focus on a layer that the panel does not clip, and the last row's glow below the panel's foot.
- **The fill is not the row.** Row 72, fill 56 (8 clear above and below), ring on the row's outline. Fill corners 2, ring corners 7 (centre line), panel corners 6.
- **The gap is a margin on the Customise row**, not a separator: no line, and Down crosses it in one press.
- **No wrap-around** at the ends, no sound at the ends, and Left/Right do nothing: the flyout opens only with A and closes only with B.
- **Menu does not close the menu.** Only B (one level), Esc, a click on the dim, or something taking the Dashboard away.
- **Default focus on a prompt is the confirming row** (Uninstall, Remove, Quit and start), not Cancel. That is the Bundle's behaviour; A, A uninstalls.
- **Three different sounds for A:** `Enter.wav` on a menu or flyout row, `PromptClose.wav` on a prompt's row, and nothing extra when a row merely opens the next level. B is always `PromptClose.wav`.
- **Open is `PromptOpen.wav`, not `OpenMenu.wav`** (that one is the Guide's).
- **The dim does not fade.** It is there on the first frame and gone on the first frame of closing, while the panel takes 250 / 150 ms. During the 150 ms fly-out the page is undimmed and already has the pad.
- **Reduce motion removes the fades too** (duration 0), unlike most fades in the Bundle.
- **0.45 is a group opacity** on the whole menu (panel, text, shadow, ring, glow together), multiplied with the panel's own fade; do not apply it to each quad separately or the overlaps will show.
- **Focus on the page never moves.** The tile behind stays drawn focused; do not "restore" focus on closing, and do not let the closing press act on the page (hold the button until release).
- **"More options" is three things:** the hint bar's label for the Menu button, the menu's last row, and the flyout's title.
- **Uninstall and Remove are not the same.** Uninstall deletes the game's files and keeps it in the library; Remove takes it out of the library and leaves its files.
- **The rows are a snapshot**; labels do not flip while the menu is open.
- **Quit replaces Play game and removes Uninstall**; Install replaces Play game and (being not installed) has no Uninstall either. Only one state has 6 rows.

---

## 13. What is Playnite-specific, and what to do about it

| Playnite-owned thing | What the Bundle leans on | Recommendation for the standalone port |
|---|---|---|
| **Taking over Playnite's game menu** (`GameMenuTakeover`, `GameMenuUninstall`, the theme's `GameMenu.xaml`) | Playnite's grid, hub buttons and bindings open Playnite's own menu; the extension swaps each binding's command, and patches the old menu's Uninstall row | **Drop.** Every opener is the port's own screen and calls one function. Nothing to take over |
| **Holding Playnite's routed copy of a press** (`TileMenuHolds`, `OnPreProcessInput`, the swallowed keys) | Playnite delivers each pad press twice (to the core and as a routed key), and resends held ones | **Replace** with the simple rule: while the menu is up it gets every press; a button that was pressed while it was up is ignored by the page until released (section 6). The double delivery itself goes |
| **Play game** (`StartGame`) | Playnite's launch, library plugins, play actions | **Port** on the Dashboard's own launch (`Effect::Launch`, M1) and Launch screen |
| **Quit**, "Quit X and start Y?", "Force quit X?" | The extension's own flow; only the prompts are hosted by the Guide's Overlay | **Port** as they are (the prompts on the Guide's layer, M2's `MenuPanel` drawing shared with this page) |
| **Install** through Playnite's library plugins, or Steam silently | `playnite.InstallGame` (plugin route, client windows) as the fallback; the extension's own silent Steam route first; the Queue, the Queued and Ready to play Toasts | **M4.** The silent Steam route (`app_install`) is already the Bundle's own code, not Playnite's: port it in M4 with the Queue. **In M3** show the Install row (a not-installed game must have a first row) and have it close the menu and hand `steam://install/<appid>` to Steam, which is exactly what the Bundle's fallback amounts to for a Steam game ("its library integration's route, client windows and all", C/IPlayniteHost.cs:30-31); no Queue, no Toast, no tile status until M4. Open question 1 |
| **Uninstall** through Playnite's library plugins, or Steam silently | `playnite.UninstallGame` as the fallback; the silent Steam route, the 1 s follow and the Uninstalled Toast are the extension's | **M4.** In M3 **leave the row out**: the row's own rule is "an installed game a library can uninstall", and in M3 no source can uninstall yet, so by that rule no game has it. A destructive row should not arrive before its confirmation's follow-up and Toast do. The menu is then at most 5 rows in M3; lay it out by the formula so 6 comes for free. Open question 1 |
| **Added by hand** (`IsCustomGame`) | Playnite's manually added games: no Uninstall, Remove without an exclusion | **Later** (plan.md open question 6). Keep the rule in the row builder ("has an uninstaller") so hand-added games fall out of Uninstall naturally when they exist |
| **Favourites** (`game.Favorite`) | Playnite's database flag; the Favourites Group; favourite filters in presets | **Port.** The `library` crate keeps favourites (plan.md). Needs a Favourites Group or filter to show for it: today's Dashboard Home has only Installed and Most played (home.md section 11). Open question 5 |
| **Manage game** (Playnite's Game details view as the Game hub) | `OpenGameHub` selects the game in Playnite's main list and toggles the details view; `ListPlayHistoryInMainList` works round the list's filter | **Port** the row; **drop** the list plumbing. It pushes the port's Game hub screen for the game |
| **Customise Home / Customise games & apps** | The extension's own Settings page | **Port** as is: Settings on Personalisation, B back to the tile (`settings.md`) |
| **A game's own actions** ("Launch config tool": Playnite's per-game `GameActions` that are not play actions) | Playnite's game editor data and `GamesEditor.ActivateAction` | **Drop for M3**, and build the flyout so rows can lead it later. A standalone equivalent would be Steam's extra launch entries from `appinfo.vdf` (a game's "Launch configuration tool", "Play in VR" and so on) or user-defined actions, both of which belong to plan.md open question 6. Open question 3 |
| **Hide / Unhide this game** (`game.Hidden`) | Playnite's hidden flag, honoured by every list; Unhide reachable through Playnite filters that show hidden games | **Port** the flag in the library database and the row. It needs one place that lists hidden games, or a hidden game can never be unhidden: a "Hidden" filter in Full library (`filters.md`). Open question 4 |
| **Enable / Disable HDR** (`EnableSystemHdr`, `HdrUtilities.IsHdrSupported`) | Playnite switches Windows' HDR on for the game's run and back after | **Drop for M3** (the row is simply absent, as it is on a display without HDR). Port later only if wanted: it is a per-game flag plus two Windows calls around launch and exit, and belongs with launching. Open question 6 |
| **Extensions** (Playnite's Extensions picker window) | Other Playnite add-ons' per-game menu items | **Drop.** There are no extensions. The flyout loses the row |
| **Remove** (database delete plus Playnite's import exclusion list; Playnite's error box when refused) | A library the user curates by deleting; exclusions so imports do not bring a game back | **Drop for store games; keep the prompt's drawing.** In a library read from each store's own files a removed Steam game would be back on the next read unless an exclusion list is built, and an excluded game is a hidden game that can never be unhidden: Hide already does the job. Remove returns, with its prompt, for hand-added games when they exist (plan.md question 6). Open question 2 |
| **Playnite's message box** (`T/Views/MessageBox.xaml`) for refused removals and action errors | Playnite's dialogs | **Drop.** Rows that cannot act are left out or do nothing; the port has no such error to show from this menu |
| **The Uninstalled Toast** | The extension's own Toast window | **M4**, on the port's Toasts (M2) |
| **Sounds at Playnite's interface volume** | Playnite's audio engine and setting | **Replace** with the port's own sound player and volume setting (home.md section 9; plan.md open question 4 on shipping the files) |
| **Debug-only mouse and keyboard stand-ins** (M for Menu, hover and click on rows) | Development aids | **Drop**, or keep Esc and a click on the dim to close, which release builds have |

With those recommendations the M3 Tile menu is: Play game / Install / Quit; Add to / Remove from Favourites; Manage game (not from the hub); gap; Customise Home / Customise games & apps; More options. The M3 flyout is: Hide this game / Unhide this game, and nothing else. A one-row flyout is a poor panel; see open question 7.

---

## 14. Open questions

1. **Install and Uninstall before M4.** Recommended above: in M3 Install is shown and hands the game to `steam://install/<appid>` (Steam's own window), and Uninstall is absent until M4 brings the silent routes, the Queue and the Toast. The alternatives are to hide Install too (a not-installed game's menu then starts on the Favourites row, and its hub has the same problem), or to draw both rows disabled (the Bundle has no disabled row to copy: its look would have to be invented). Which?
2. **Remove.** Drop it for store games and keep it only for hand-added ones (recommended), or port it with an exclusion list of the port's own?
3. **A game's own actions.** Nothing in M3 (recommended), Steam's extra launch entries from `appinfo.vdf` as rows, or user-defined actions? Ties to plan.md open question 6.
4. **Where hidden games can be seen**, so that Unhide this game is reachable: a Hidden filter in Full library (recommended), a list in Settings, or nowhere (hide becomes one-way)?
5. **Favourites on Home.** The Bundle's default Group is Favourites; the Dashboard's Home currently builds Installed and Most played instead (home.md section 11). Does a Favourites Group come back in M3 so the row has something to show for itself (plan.md open question 2)?
6. **HDR.** Drop the row for good, or port per-game HDR switching with launching in a later milestone?
7. **A flyout of one row.** With Extensions, Remove, HDR and game actions gone, More options would open a panel holding only Hide this game. Keep the two levels exactly as the Bundle has them (the flyout fills again as rows return), or, until then, put Hide this game on the menu in More options' place? The first keeps parity of structure; the second is the better menu today.
8. **A flyout taller than the screen.** The Bundle lets it run off the foot. If game actions ever return: scroll it, cap it, or keep the Bundle's behaviour?
9. **Default focus on the destructive prompts.** The Bundle focuses Uninstall and Remove, so A twice goes through. Keep for parity (recommended, as the requirement is the Bundle's behaviour exactly), or start on Cancel?
10. **The fly-up replayed for the Uninstall prompt** and again on Cancel (section 7) reads as a side effect of how the control is written rather than a decision. Reproduce it, or swap the prompt in place as the Remove prompt is swapped?
11. **Labels that do not fit** (section 2.4): with game actions dropped nothing can overflow in M3. If they return: ellipsis at the content's right edge (x 430 in the panel)?
12. **Menu on Search's tiles.** The Bundle opens no Tile menu from Search (section 1). Parity, or should every game tile in the port open it, since every opener is now the same code?
13. **Bahnschrift's line height.** This spec assumes 1.2 em (39.6 at 33), which the pictures bear out to 1 px. The port should take the real metric from the font at runtime and centre the panel by the height that gives.
