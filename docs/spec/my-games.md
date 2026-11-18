# My games & apps specification (as the Bundle draws it)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run.

Path shorthand used in citations:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`

All numbers are design units = px on a 1920x1080 canvas. **Nothing on this page is inside a Viewbox: every measure is 1:1**, unlike Home's rows (home.md's s = 1.114 does not apply here; E/MyGamesControl.xaml:24-29 says so of the one Home resource the page borrows).
This spec describes the page **with the extension installed** (the full Bundle). My games & apps exists only then (CONTEXT.md:22-23).

Milestones (docs/plan.md): the page shell, Games, Play history, Full library and Manage are **M3**. **Apps, the Queue, Manage Queue and everything about installing are M4**; they are marked so below, with their slot given so M3 leaves room.

**Pictures of the real thing.** `~/projects/xbox-series-theme/artifacts/e2e/see-all-20261110-113608/`: `history-1..4-*.jpg` (Play history, focus on its first and second rows), `first-1..4-*.jpg` and `2-see-all.jpg` (Full library on the preset "My Picks", 6 games), `second-1..4-*.jpg` and `3-see-all.jpg` (Full library on "All", 14 games, three rows). They were taken with build 0.2.0. Measured (edges found by pixel colour) and compared with the numbers derived from the source at `f11e4f9`; section 13 lists what they confirmed, what they corrected and what no picture shows. **There is no picture of Games, Apps, Manage, Manage Queue, the Sort list open, the name box, or a nav item with focus.**

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite or WPF internals that are not in this repository.

---

## 1. What draws the page

Three owners draw into the same 1920x1080, and a port must know which, because their numbers come from different files:

| Part | Drawn by | Source |
|---|---|---|
| Page background, nav, divider, header, Play history's list, Apps' tiles, Manage's box, Manage Queue, Full library's Sort and Filter row, the Sort list, "Looking for more?" | The extension's `MyGamesControl`, in the theme's `XboxSeries_MyGames` slot (full screen, `Grid.RowSpan="2"`) | E/MyGamesControl.xaml; T/Views/Main.xaml:395 |
| **The grid of games on Games and on Full library** | **Playnite's own main list** (`PART_ListGameItems`, a `ListBoxEx`), which the theme hides everywhere else and shows beside the nav while `ShowsGameGrid` is true. Its panel is Playnite's `FullscreenTilePanel`, which the extension re-parameterises (vertical, `Rows="3"`, `ItemAspectRatio="1:1"`); each tile's container is the theme's `ListGameItemStyle` and its face the theme's `ListGameItemTemplate`. | T/Views/Main.xaml:325-363, 699-702; E/MyGamesControl.xaml.cs:39-43, 505-520; T/DerivedStyles/ListGameItemStyle.xaml; T/DerivedStyles/ListGameItemTemplate.xaml; C/MyGamesState.cs:93 |
| Full library's tabs | Playnite's `FilterPresetSelector` (`PART_FilterPresetSelector`), styled by the theme and placed by it over the page | T/Views/Main.xaml:452-464; T/CustomControls/FilterPresetSelector.xaml |

So the answer to "which control draws the Games grid in the full Bundle" is: **Playnite's list, with the theme's tile, not the extension's `HomeTile`**. The grid tile's face therefore differs from Home's in several details (section 5.3). The cover-fit payload header's "the theme's tiles" means exactly this.

Layer order, back to front: page fill; nav and content (`MyGamesControl`); Playnite's progress copy (section 11.3); the tabs; the main list with its top fade; Game hub; hint bar; Tile menu; text entry (T/Views/Main.xaml:395-464, 699-746, 761-781). The main list is later in the tree than `MyGamesControl`, so **the grid draws over the page**, which is why the Sort list is a popup window and why the name box's ring is drawn inside its box (sections 7.4, 7.5).

While the page is up, Home's hero, top bar, Recent row and Groups are collapsed (`XboxHomeOnly`, T/Views/Main.xaml:307-319). The hero scrim rectangle is not collapsed but is under the page's opaque fill (T/Views/Main.xaml:392, 395).

---

## 2. Page shell

### 2.1 Colours and type used throughout

| Name | Value | Source |
|---|---|---|
| Page fill (`PageBrush`) | `#FF1F1F1F`, flat, whole screen | E/MyGamesControl.xaml:32, 451 |
| Divider (`DividerBrush`) | `#FF303030` | :33 |
| Selected nav item fill (`SelectedBrush`), storage ring track | `#FF262626` | :34 |
| Play history row back (`RowBrush`) | `#FF292929` | :35 |
| Dim text (`DimBrush`) | `#FF8F8F8F` (opaque grey, **not** Home's `#B3FFFFFF`) | :36 |
| Accent fill (`GlyphBrush`): focused nav item, focused Sort/Filter/footer button, active tab, selected bar, storage ring | `#FF107C10` | T/Constants.xaml:9 |
| Focus ring (`GlyphLightBrush`) | `#FF2CA243` | T/Constants.xaml:10 |
| Flat button rest | `#FF333333`, radius 2 | E/MyGamesControl.xaml:128, 317, 407 |
| Box / Queue row rest, focused box | `#FF2D2D2D`, `#FF4A4A4A` | :248, 263, 284 |
| Tile back before art | `#FF1A1A1A` | T/Constants.xaml:207; E/MyGamesControl.xaml:728 |

Text styles (E/MyGamesControl.xaml:38-68; fonts T/Constants.xaml:228-232):

- `Text`: Segoe UI 26, white, character ellipsis, one line.
- `HeaderText`: Bahnschrift, otherwise as `Text`.
- `NavText`: `Text`, vertically centred.
- `NavLabel`: Bahnschrift, `FontWeight="350"` (the SemiLight face), 26.
- `NavIcon`: `Segoe Fluent Icons, Segoe MDL2 Assets`, 24, white, in a box 44 wide (glyph left-aligned in it), vertically centred.

### 2.2 Columns

Root grid columns `441`, `2`, `*` (E/MyGamesControl.xaml:452-456):

| Column | x | What |
|---|---|---|
| Nav | 0..441 | Profile, nav items, foot |
| Divider | 441..443 | `#FF303030`, full height (`NavDivider`, :603) |
| Content | 443..1920 | `ItemHost`, `Margin="77,72,170,0"`: **x 520..1750 (1230 wide), y 72..1080** (:609) |

`ItemHost` rows: `118`, then `*` (:610-613): the **header row is y 72..190**, the **content row is y 190..1080** (890 tall).

### 2.3 Header

| Part | Value | Source |
|---|---|---|
| Title | `Header`: Bahnschrift SemiLight (weight 350) **42**, white, top-left of the header row: text box top-left at (520, 72). Text: "Games", "Apps", "Play history", "Full library", "Manage". | E/MyGamesControl.xaml:616; E/MyGamesControl.xaml.cs:366 |
| Count (Games only) | "`{GameCount} games`", Segoe UI 26, `#FF8F8F8F`, right-aligned to x 1750, top y 88 (`Margin="0,16,0,0"`). Collapsed on every other item. | E/MyGamesControl.xaml:617-618; E/MyGamesControl.xaml.cs:381 |

`GameCount` is the number of installed, not hidden games (C/ExtensionCore.cs:1428; C/GameListFilter.cs:14, 31-32). The text does not singularise ("1 games").

### 2.4 Nav: profile

`NavPane`, a vertical stack with `Margin="0,78,0,0"` (E/MyGamesControl.xaml:462).

| Part | Value | Source |
|---|---|---|
| Picture | 54 x 54 circle at **x 75..129, y 78..132** (`Margin="75,0,0,58"`). Under it an ellipse `#FF107C10` with glyph U+E77B (icon font 24, white, centred); over it the Profile's picture, `UniformToFill`, clipped to a circle r = 27, decoded 108 wide. Same fallbacks as Home's (home.md 2.1): a Profile with no picture shows the Windows account's. | :463-473 |
| Gamertag | 14 right of the picture (x 143), vertically centred on y 105: Segoe UI 26, **`#FF8F8F8F`**. Fallback the Windows user name. | :474-476 |

Not focusable; no Score, no real-name swap.

### 2.5 Nav: items

`Nav` stack starts at y = 78 + 54 + 58 = **190** (E/MyGamesControl.xaml:463, 478). Style `NavItem` (:78-114): a Button the full width of the column (x 0..441), `Height="58"`, `Margin="0,0,0,14"` (pitch 72).

| # | Item | y (Apps shown) | y (Apps hidden) | Icon | Right-hand count | Source |
|---|---|---|---|---|---|---|
| 0 | Games | 190..248 | 190..248 | U+E7FC | `GameCount` | :479-485 |
| 1 | Apps (**M4**) | 262..320 | (absent) | U+E71D | `Apps.Count` | :487-494 |
| 2 | Play history | 334..392 | 262..320 | U+E81C | `PlayHistoryCount` (all rows) | :495-501 |
| 3 | Full library | 406..464 | 334..392 | U+E8F1 | `FullLibraryCount` (section 7.6) | :502-508 |
| - | Line | y 482..484, **x 36..383** (`Height="2" Margin="36,4,58,32"`), `#FF303030` | y 410..412 | - | - | :511 |
| 4 | Manage | 516..574 | 444..502 | U+E821 | none | :517-523 |

Apps is collapsed (takes no room) when the library cannot give Apps at all (`HasApps` false, :487-488; C/ExtensionCore.cs:1622-1635); with zero Apps it still shows, counting "0", as in the pictures. **M3 has no Apps: the M3 nav is the "Apps hidden" column**, and M4 moves Play history, Full library, the line and Manage down 72.

Inside an item (`ContentPresenter Margin="93,0,43,0"`, a DockPanel; :91, 480-484):

- Icon: x 93 (its 44-wide box 93..137), icon font 24, white.
- Label: from x 137, `NavLabel` (Bahnschrift SemiLight 26, white).
- Count: `NavText` (Segoe UI 26, white), docked right: its right edge at **x 398**.
- All vertically centred in the 58.

Manage with something queued (**M4**): the button grows (`Height="Auto" MinHeight="58"`) by a second line, a DockPanel `Height="40" Margin="44,0,0,10"`: "Queue" at x 137 and the Queue's count right-aligned to x 398, both Segoe UI 26 `#FF8F8F8F`; the item is then 58 + 40 + 10 = **108** tall (y 516..624) and is lit and barred as one (:517-530). In M3 Manage is always 58.

States (:85-113):

| State | Drawing |
|---|---|
| Rest | No fill (transparent). |
| Showing (`Tag="Selected"`: the item whose content is up) | Fill `#FF262626` over the whole item (x 0..441), and a bar **6 wide at the item's right edge (x 435..441)**, full item height, `#FF107C10`. Manage is also "showing" while Manage Queue is up (E/MyGamesControl.xaml.cs:371). |
| Focused | Fill **`#FF107C10`** over the whole item, plus the glow (below). No ring. Drawn over its neighbours (`ZIndex 1`). Text and icon stay white. |
| Focused and showing | The focused fill wins (the later trigger); the bar is still drawn, the same green as the fill. |
| Pressed, disabled | **NOT FOUND** (no triggers). |

Glow on a focused slab: `XboxFocusGlow` (home.md 4.3: the same ten bands and four corner spots) inside the fill's Border, which has no border thickness. The glow control's `Margin="-7.5"` and its bands' centre line 5.5 inside the control's edge (T/Constants.xaml:27, 41) put **the bands' centre line 2 outside the fill's edge**; the bands keep their own `CornerRadius="12"` though the slab is square. The glow reaches 17.5 beyond the fill: above and below over the neighbouring items, and right over the divider column's place (the divider, later in the tree, is drawn over it at x 441..443) and on to x 458.5. The same construction is used by every "filled with the accent" control on the page (Sort box, Filter, SEE FULL LIBRARY, Sort options, the active tab).

### 2.6 Nav: foot

`NavFoot`: bottom-left, `Margin="75,0,0,66"`: left x 75, **bottom y 1014** (E/MyGamesControl.xaml:537). Top to bottom:

1. **Current download block (M4)**: `Width="301"`, `Margin="0,0,0,34"`, only while the Queue's first entry is downloading: title; state with the speed right-aligned; "`{amount}, {percent}`"; all Segoe UI 26 white, 2 between lines (:543-574; C/MyGamesState.cs:250). Slot: directly above the storage ring, x 75..376.
2. **Storage ring** (`StorageRing`, :581-600): a grid of columns `206`, `Auto`.
   - Left, vertically centred: "All storage", then 3 under it the free space ("354.6 GB free"), both Segoe UI 26 white (`NavText`).
   - Right: a ring 95 x 95, so **x 281..376, y 919..1014**: track ellipse stroke 4 `#FF262626`; over it the used arc, stroke 4 `#FF107C10`, starting at 12 o'clock (the ellipse is rotated -90 degrees) and running as far round as the used fraction (a dash of `used x pi x (95 - 4)` along the centre line). Direction: clockwise is WPF's ellipse outline direction, **UNVERIFIED** in this repo. Flat caps. Inside, centred: the used share ("55.8%"), Segoe UI 26 white.
   - Texts: used = `(total - free) / total`, as "0.0" + "%"; free = size to one decimal + " free", in GB, MB below 1 GB, TB from 1024 GB (C/MyGamesState.cs:283-309).
   - Which drive: the one holding most installed, not hidden games; ties to the first by name; hidden (collapsed, no placeholder) when there is none or it cannot be read (C/ExtensionCore.cs:1886-1897; E/MyGamesControl.xaml.cs:453-463). Refreshed when the page opens and on a library change while it is up (C/ExtensionCore.cs:303-306, 1537-1542). The pictures, whose library has no install folders, show no ring.

The nav, its foot and the divider are hidden only on Manage Queue (E/MyGamesControl.xaml.cs:375-380).

---

## 3. Opening, leaving and transitions

### 3.1 Ways in

| From | Opens on | Focus lands on | Source |
|---|---|---|---|
| Home's top bar button My games & apps (A) | Games | The grid's first game; with no games, the Games nav item | E/MyGamesControl.xaml.cs:137; C/ExtensionCore.cs:1501; E/MyGamesControl.xaml.cs:304-312, 1496-1506, 1555-1561 |
| Home's Play history tile (A; at once, no double-tap wait) | Play history | Its first row; with none, the Play history nav item | E/RecentRowControl.xaml.cs:296-299; E/MyGamesControl.xaml.cs:1521-1531 |
| See all on a Group (A) | Full library, **with that Group's preset applied**: its tab lit, its games in the grid in the preset's own sort; any sort chosen on the dropdown before is forgotten. The default Favourites Group has no preset: plain Full library, as the nav opens it. | The grid's first game | C/ExtensionCore.FullLibrary.cs:22-37; CT/SeeAllTests.cs:39-87 |
| Home's Downloads tile (A) | Manage Queue (**M4**), B returning to Home | (M4) | C/ExtensionCore.cs:1732-1736 |
| The Guide's "My games & apps" | Games, the Dashboard brought to the front | First game | C/ExtensionCore.cs:2124-2129; CT/MyGamesTests.cs:467 |
| B on Search that Y on Games opened | Games | **The game Y was pressed on** | C/ExtensionCore.Search.cs:228-232; E/MyGamesControl.xaml.cs:1498-1505 |
| B on Settings that the Tile menu's "Customise games & apps" opened | The item the page was on | That game's tile (grid, Play history row) | C/ExtensionCore.cs:1739-1747, 1826-1828; C/MyGamesState.cs:43-47 |
| A Game hub's Queue; the Guide button during a Queued Toast | Manage Queue (**M4**) | (M4) | C/ExtensionCore.Queue.cs:134-151; C/ExtensionCore.cs:1013-1014 |

The page takes the place of Settings or Search if one is up; the three are never up together (C/ExtensionCore.cs:1510-1545).

Focus is given to the content, not the nav, on every way in ("Opened from Home, the top bar or the Guide: focus goes to the first tile", E/MyGamesControl.xaml.cs:292-312). This runs after the grid has been refilled (`DispatcherPriority.ContextIdle`, :1480-1483): a frame or two after the page appears.

### 3.2 The opening and closing transitions

**There are none.** The page is shown and hidden by `Visibility` bound to `IsShown` (E/MyGamesControl.xaml:451) and Home's parts by `XboxHomeOnly`: one frame Home, the next the page, and the same back. No fade, slide or scale between Home and the page, nor between the page and Search or Settings: **NOT FOUND**. `PaneFlyIn` explicitly does nothing when the page opens ("Changed any other way, as the page opening, it is just there", E/MyGamesControl.xaml.cs:394-399; E/PaneFlyIn.cs:39-44).

`E/SlidingRow.cs` (a row sliding sideways to a focused tile over 300 ms) is **not used on this page**: its only user is Search (E/SearchControl.xaml:82). Manage Queue's Recently installed row is a plain horizontal ScrollViewer (E/MyGamesControl.xaml:850-851).

### 3.3 Pane fly-in along the nav

E/PaneFlyIn.cs (whole file); called from E/MyGamesControl.xaml.cs:400-408. Only when the item changes **because focus moved along the nav** (the page is up and focus is within the nav).

| Step | Value | Source |
|---|---|---|
| Direction | New item lower on the nav than the old: content comes **up from 50 below**. Higher: **down from 50 above**. Nav order for this: Games 0, Apps 1, Play history 2, Full library 3, Manage 5. | E/PaneFlyIn.cs:20, 46, 70-75; E/MyGamesControl.xaml.cs:404 |
| Old content | Gone at once (no fade out). | E/PaneFlyIn.cs:175-179 |
| Start | New content placed at translate Y +-50 with opacity 0, and held there for two rendered frames (so the slide is not spent on the first layout). | :178-179, 193-207 |
| Slide | Translate to 0 over **120 ms, `CubicEase EaseOut`**. | :23, 214-217 |
| Fade | Opacity 0 -> 1 over the same **120 ms, `QuadraticEase EaseOut`**: for `ItemHost` (header, count, Play history, Apps, Manage's box, the Sort and Filter row, "Looking for more?") and, on Full library, the tabs. | :218-223; E/MyGamesControl.xaml.cs:406-407; CT/FullLibraryTabsTests.cs:51-61 |
| The grid | The main list (Games, Full library) **slides the same 50 but does not fade**: it shows at full opacity as the slide starts. | E/PaneFlyIn.cs:181-190, 224-227 |
| Interruption | A newer slide cancels the older, which jumps to rest. | :168-171, 245-257 |
| Reduce motion | No slide and no fade: content is simply there. | :74 |

"SEE FULL LIBRARY" (section 5.5) moves focus to the Full library nav item before opening, so Full library flies up from below when reached that way.

### 3.4 Fly-in across Full library's tabs

Switching tab (LB, RB, or Left/Right on the tabs): **only the grid** comes in, from **50 to the right** for a tab further right, 50 to the left for one further left; 120 ms `CubicEase EaseOut`, no fade; the tabs, header and Sort row stay put. Not when a tab is lit any other way (See all applying its preset before the page shows). Reduce motion: none (E/MyGamesControl.xaml.cs:416-450; E/PaneFlyIn.cs:54-64; CT/FullLibraryTabsTests.cs:22-48).

### 3.5 Leaving (B)

B anywhere on the page, nav or content (it is Playnite's B on the view host, E/MyGamesControl.xaml.cs:1277-1289), does the first that applies (C/ExtensionCore.cs:1662-1680):

1. Text entry, a prompt, the Tile menu or the Guide over the page: that takes B; the page stays.
2. The Sort list is open: closes it, focus back on the Sort box (E/MyGamesControl.xaml.cs:978-981).
3. Focus in the name box: clears the name, closes the box, focus on Filter (:1113-1117, 1198-1204).
4. Manage Queue (**M4**): back to where it was opened from.
5. Full library with a name filter: **clears the name** (the whole tab shows again); the page stays and focus stays (CT/FullLibraryTests.cs:177).
6. Otherwise: **back to Home**, at once. A nav item still waiting to open (section 4.2) does not open (CT/MyGamesTests.cs:384).

Where focus returns on Home (home.md 7.4; E/RecentRowControl.xaml.cs:250-256, 312-346; E/GroupsControl.xaml.cs:136-142): Home is still at the rest it was left at. If it rests on a Group (See all), the Groups take focus at the remembered card, **See all itself**. Otherwise the Recent row / wide tiles take it: the wide tile last focused if the wide tiles had focus (Downloads), else the **last focused game's tile**, else the first tile, else the Play history tile. Consequences:

- Opened from the **Play history tile**: focus returns to the game tile focused before it, not to the Play history tile (the tile is not a "last focused game"; e2e/payloads/see-all/run.ps1:5-6).
- Opened from the **top bar button**: focus returns to the Recent row (last focused game's tile), **not to the button**. Nothing refocuses the top bar: NOT FOUND.

No sound is played by the extension for opening or leaving (section 10).

---

## 4. Nav behaviour

### 4.1 Buttons with focus on a nav item

| Input | Does | Source |
|---|---|---|
| Up / Down | The next nav item that way (WPF `PredictFocus`); at either end nothing; never wraps. Down from Full library crosses the line to Manage. | E/MyGamesControl.xaml.cs:1347-1350; E/HomeRows.cs:216-232 |
| Right, or A | Opens the focused item **at once** (even if its settle wait is pending) and moves focus into its content: the grid's game focus last left for the nav (else its first), the last focused App, the last focused Play history row (else the first), Manage's Queue box. With no content to focus (empty grid, no history, no Apps) focus stays on the nav item. | :1339-1346, 1361-1367, 1480-1533; CT/MyGamesTests.cs:368 |
| Left | Nothing. | :1351-1354 |
| B | Home (section 3.5). | |
| Y, RS click | Nothing (swallowed so Playnite's keyboard and filter panel never open). | :636-654 |
| LB / RB | Nothing, **except while Full library is showing**, where they switch its tab from anywhere on the page, the nav included (they are bound on the view host). | :636-638; E/PlayniteViewHost.cs:37-45; CT/MyGamesTests.cs:520 |
| Menu, View, X | Nothing: no tile has the pad (C/ExtensionCore.cs:840-849, 869). |
| LT / RT | **NOT FOUND** in the Bundle; whatever Playnite binds, UNVERIFIED. |

### 4.2 Focus on a nav item shows it (the settle wait)

Focusing an item lights it (and Playnite sounds it) at once; **its content follows** by this rule (C/ExtensionCore.cs:71, 1562-1577; CT/MyGamesTests.cs:286-411):

- `NavSettle` = **150 ms**.
- If no nav item took focus in the 150 ms before this one: the item **opens at once**. So a single step opens immediately.
- If one did (focus is passing along the nav, as with a held direction repeating every 80 ms): the item opens once focus has **stayed on it 150 ms**; moving on restarts the wait for the next item; items passed over never open.
- Back on the item already showing: nothing opens, and a pending one is dropped.
- Focus leaving the nav other than by Right/A (B, the Guide taking the foreground) drops a pending open (E/MyGamesControl.xaml.cs:1323-1329).

Opening an item from the nav: its header and content replace the old with the fly-in of section 3.3; focus stays on the nav. Moving along the nav does not rebuild Play history (CT/MyGamesTests.cs:420). Leaving Full library for another item drops its name filter (C/ExtensionCore.cs:1513-1517).

Left from the content's first column returns to **the nav item of what is showing**, and Right from there goes back to the game it left (`lastFocusedGridGame`; forgotten when the item or the tab changes; E/MyGamesControl.xaml.cs:79-84, 279-281, 436-440, 677-682).

---

## 5. Games

### 5.1 What it lists

Installed games that are not hidden, by name A to Z (Playnite's sorting name, current culture, case-insensitive) (C/GameListFilter.cs:14, 31-41; C/ExtensionCore.cs:1530-1533; CT/MyGamesTests.cs:25). **No sort or filter controls on Games: NOT FOUND.** No group headers in the grid: NOT FOUND. LB/RB do nothing here.

### 5.2 Grid geometry

Derivation. The theme places the list's grid at `Margin="436.5,-74,86.5,0"`, top-aligned, in a row that starts at y 200, and the list itself 80 in from each side (T/Views/Main.xaml:331-342, 376, 700): **list x 516.5..1753.5 (1237 wide), top y 126**. The extension sets the list's height to `floor(width / 5 x 3.5)` = floor(865.9) = **865** (E/MyGamesControl.xaml.cs:527-535), so the list is y 126..991. Playnite's panel, told `Rows="3"` and 1:1 cells, "sizes its cells from its height, 3.5 of them to it, and fits as many to a row as its width allows", and "always leaves a quarter cell above the first row" (E/MyGamesControl.xaml.cs:34-38, 538-541; T/Views/Main.xaml:336-340). Each tile sits 7.5 inside its cell on every side (T/DerivedStyles/ListGameItemStyle.xaml:67; E/MyGamesControl.xaml.cs:56).

| Measure | Value | How |
|---|---|---|
| Columns | **5** | by construction |
| Cell height | 865 / 3.5 = **247.14** | |
| Row pitch | 247.14 | |
| Tile height | 247.14 - 15 = **232.14** | |
| Gap between tiles | **15** both ways | 7.5 + 7.5 |
| Quarter cell above row 0 | 61.79 | 247.14 / 4 |
| Row r tile top (unscrolled) | 126 + 61.79 + 7.5 + 247.14 r = **195.29, 442.43, 689.57**; row 3 at 936.71 | |
| List bottom (clips the grid) | **991** | 126 + 865 |
| Rows visible | 3 whole, and the top **54.29** of a fourth (936.71..991) | |
| Tile left edges, as the source intends | 524 + 247.4 c, tile 232.4 wide (c = 0..4: 524, 771.4, 1018.8, 1266.2, 1513.6; last right edge 1746) | 1237 / 5; T/Views/Main.xaml:336-338 "x 524 to 174 from the right" |
| **Tile left edges, as measured** (Full library pictures; the same list at the same x) | **526 + 246.6 c, tile 231.6 wide** (526, 772.6, 1019.2, 1265.8, 1512.4; last right edge 1744) | section 13 |

The 2 px each side between intent and picture is Playnite's (the list or its panel insets its cells by 2 at left and right, not at top): **UNVERIFIED** cause, measured effect. Tiles are therefore 231.6 x 232.14, square to half a pixel. See open question 1.

Below the grid's third row nothing else is drawn but the fourth row's top and, at the grid's end, "Looking for more?" (section 5.5). The area x 1750..1920 right of the grid is empty.

### 5.3 The tile (T/DerivedStyles/ListGameItemTemplate.xaml, ListGameItemStyle.xaml)

All 1:1. This is **not** Home's tile face (home.md 3.3); differences are in bold.

| Part | Value | Source |
|---|---|---|
| Back and shape | `#FF1A1A1A`, corner radius 6 | T/Constants.xaml:199-208 |
| Cover | `UniformToFill`, centred across, **top-aligned**, Fant scaling, rounded 6. Whatever Playnite's cover-stretch setting says. Playnite loads the image (size and timing its own: UNVERIFIED). | T/Constants.xaml:208-213; T/DerivedStyles/ListGameItemTemplate.xaml:30-32 |
| No cover | **No title fallback in the theme**: the dark tile, or whatever default cover Playnite supplies (UNVERIFIED). | ListGameItemTemplate.xaml:30-32 |
| Darken | `#AA000000` over the cover, radius 6, when the game is not installed **and** Playnite's "darken uninstalled games" grid setting is on. The pictures (fresh Playnite) show it off. Games never has an uninstalled game. | :33, 76-82 |
| Source badge | Bottom-left, `Margin="10"`: white 20 x 20 icon over its 36 x 36 shadow picture (home.md 3.3; same paths and pictures). Only for Source "Steam" or "Xbox". Opacity 0 at once while the tile is focused. Collapsed while installing. | :41-50, 83-96, 104 |
| **State badge** | Top-left, `Margin="6"`, **24 x 24**, `#E62B2B2B`, radius 3. Holds either the play triangle `M0,0 L9,5.5 L0,11 Z`, white (the running game), or **glyph U+E896 (Download), icon font 14, white, on a game that is not installed**. No "CONTINUE" text when focused; no inner `#F0181818` box. Stays visible on focus. Collapsed while installing unless the game is running. | :65-73, 97-111 |
| Installing band (#78), unfocused | Bottom, full width, radius 0,0,6,6: a **vertical gradient `#00000000` (top) to `#E6000000` (bottom)**; text "Installing..." Segoe UI 26 white, `Margin="12,28,12,10"`, one line, ellipsis. Opacity 0 while focused. This list reads "Installing..." for a queued game too (Playnite has one flag). | :53-63, 95, 102-106; :1-6 of the file |
| Title band, focused | Bottom, full width, radius 0,0,6,6: the same **gradient `#00000000` -> `#E6000000`, not Home's flat band**. Stack `Margin="12,36,12,10"`: the title (Playnite's display name), Segoe UI 26 white, wraps, `MaxHeight="72"` (two lines), character ellipsis; then (M4) "Installing..." on one line. One-line band is 36 + 34.6 + 10 = 80.6 tall; two-line 115.2. | T/DerivedStyles/ListGameItemStyle.xaml:76-99 |
| Title band fade | To 1 over 150 ms after a 50 ms delay, linear; to 0 over 100 ms, linear. **Reduce motion: there and gone at once** (unlike Home's band, which still fades). | :41-61, 127-150 |
| Focus ring | `BorderThickness="4" CornerRadius="14" Margin="-8"`, `#FF2CA243`, with `XboxFocusGlow`: exactly home.md 4.1 and 4.3 (rectangular variant), at 1:1. Shown and hidden at once. | :107-112, 118-121 |
| Growth | **None** with the extension (the 1.12 scale is theme-only). The focused tile is drawn over its neighbours (`ZIndex 90`). | :21-40, 120, 151-162 |

### 5.4 Buttons with focus on a grid tile (Games)

| Input | Does | Source |
|---|---|---|
| A | Opens the game's **Game hub**. This is Playnite's own binding on its list (its details view, which the theme draws as the Game hub), so **there is no 300 ms double-tap window here** and A, A does not play: NOT FOUND in the Bundle. Whether a Playnite setting can make A start the game instead: UNVERIFIED. | docs/research/fullscreen-theming-limits.md:14, 92 (theme repo); E/MyGamesControl.xaml.cs:1563 |
| Menu | The **Tile menu** (in place of Playnite's game menu), with "Customise games & apps" rather than "Customise Home". | E/GameMenuTakeover.cs:12-16; C/ExtensionCore.TileMenu.cs:30, 103 |
| X | Also the Tile menu: Playnite binds X on its list to its game menu, which the extension takes over ("Menu (and X) on Playnite's own grid"). Not shown in the hint bar. | E/GameMenuTakeover.cs:14 |
| Y | **Search**, empty, the Console keyboard up; B there returns to Games on this game. | E/MyGamesControl.xaml.cs:704-729; C/ExtensionCore.Search.cs:52-59, 77 |
| B | Home. | |
| Left | Within the row; from the first column, to the Games nav item (the game is remembered for Right). | E/MyGamesControl.xaml.cs:670-682 |
| Right | Within the row. At the row's last tile: Playnite's list (UNVERIFIED: stop, or on to the next row). |
| Up / Down | The tile above / below (Playnite's list). **Down from the row that holds the last game goes to "SEE FULL LIBRARY"**; Up from the first row: no handler on Games (Playnite's default, UNVERIFIED; nothing is above it to take focus). | :683-689 |
| LB / RB, RS click | Nothing. | :636-654 |
| View, LT / RT | NOT FOUND. |
| Held direction | Playnite's repeat (80 ms per home.md 7.4, UNVERIFIED). |

Focus memory: Right from the nav returns to the game Left came from; the page reopened by B from Search or Settings returns to the game concerned; otherwise the first game. A game that has left the list (uninstalled) falls back to the first (E/MyGamesControl.xaml.cs:1496-1506). Back from a Game hub, Playnite itself refocuses its list's selected game (:1293).

### 5.5 Scrolling, and "Looking for more?"

The grid scrolls vertically inside the list (y 126..991 on Games); the header does not move. Scrolling is Playnite's panel's: it brings the focused tile's row into view, **animated or not by Playnite's own "smooth scrolling" Fullscreen setting** (`SmoothScrollEnabled="{Settings Fullscreen.SmoothScrolling}"`, E/MyGamesControl.xaml.cs:42). Its duration, easing and how far it moves per step are **UNVERIFIED**. When the extension itself scrolls to a game whose tile is not built (after a sort; returning focus), it puts that game's row where row 0 rests: offset = `floor(index / 5) x (viewport height / 3.5)` (:1578-1589). No scroll bar is shown (T/Views/Main.xaml:700).

Under the grid's last row, only while that row is wholly in view (E/MyGamesControl.xaml.cs:561-619; E/MyGamesControl.xaml:796-802):

| Part | Value |
|---|---|
| "Looking for more?" | Segoe UI 26 white, left x **523** (`ItemHost` + 3), top = the last tile's bottom + **15** (its container's bottom + 7.5). 12 below it: |
| Button "SEE FULL LIBRARY" | `FooterButton`: height 52, width = text + 26 + 26, fill `#FF333333`, radius 2; label Segoe UI 26 white. Focused: fill `#FF107C10` + the slab glow (section 2.5). |

It scrolls with the grid (it is re-placed on every scroll change) and is collapsed whenever the last row is not fully inside the list, when the grid is empty, and off Games. Example, 8 games (two rows), unscrolled: text top 689.57, button y 736.2..788.2.

On the button: A opens Full library (focus passes through the Full library nav item to its first game, so the pane flies in from below); Up returns to the game Down came from; Left goes to the Games nav item; Right and Down do nothing (E/MyGamesControl.xaml.cs:1239-1274).

### 5.6 Empty Games

No installed games: the list is empty, focus stays on the Games nav item, "Looking for more?" is hidden (so there is **no way to Full library from the content**; the nav has it), and Playnite's "no games found" text shows centred in the list's box: Segoe UI **32**, white, centred on (1135, 558.5) (T/Views/Main.xaml:704-717; the string is Playnite's `LOCNoGamesFound`, its English text UNVERIFIED here). The header's count reads "0 games".

---

## 6. Play history

E/MyGamesControl.xaml:151-188, 692-757; C/ExtensionCore.cs:1915-1955, 2531-2532; CT/MyGamesTests.cs:151-198.

### 6.1 What it lists

Every played game (`LastActivity` set) that is not hidden, **installed or not**, newest first, one row each, no limit. Rows are grouped into sections by the calendar day last played, relative to today (the first matching rule wins):

| Section title | Rule (day = date last played) |
|---|---|
| Today | day >= today |
| Yesterday | day = today - 1 |
| This week | day >= first day of this week (the culture's first day of week) |
| Last week | day >= that - 7 |
| This month | day >= the 1st of this month |
| Last month | day >= the 1st of last month |
| This year | same year as today |
| Older | anything else |

Sections appear in that order, only those with games; within a section newest first (C/ExtensionCore.cs:1941-1955). The sections are worked out when the page opens (not while moving along the nav) and on a library change, and replaced only if something shown changed (C/ExtensionCore.cs:1535-1542, 1924-1927; CT/KeptRowsTests.cs:131-169). The Settings option "Show Play history" only removes Home's tile: Play history stays in the nav (CONTEXT.md:67).

### 6.2 Layout

The list is a ListBox in the content row with `Margin="-24,0"` and its rows 24 in again, so glows are not clipped at its sides: **rows are laid out from x 520 to 1750, the list's viewport is x 496..1774, y 190 down** (E/MyGamesControl.xaml:692-706).

| Element | Value | Source |
|---|---|---|
| Section header | Bahnschrift (regular) **30**, white, one line; `Margin="32,0,0,18"`: **text x 552**; line height 36 (30 x 1.2, confirmed by the pictures), so a header takes **54** | :711 |
| Row (item) | `Height="176"`, `Margin="0,0,0,7"`: pitch **183** | :153-154 |
| Row back | `Margin="4"` inside the item: **168 tall**, `#FF292929`, square corners | :163 |
| Row x extent | The items are indented **5** by WPF's default group container (its `ItemsPresenter Margin="5,0,0,0"`; not in this repo, UNVERIFIED there, but measured): item x 525..1750, **back x 529..1746 (1217 wide)**. The source's own comment intends 524..1746. | measured; T/Views/Main.xaml:336 |
| Gap after a section | `GroupItem Margin="0,0,0,24"` | :714-718 |
| First section | header y 190..226 (text), first row item y 244..420, **back y 248..416** | derived, confirmed |
| A section of n rows takes | 54 + 183 n + 24. With one row each: section pitch **261** (backs at y 248, 509, 770, 1031 ...) | derived, confirmed |
| End padding | `Padding="0,0,0,80"` on the list's scroller | :693, 702 |

Inside a row (:721-756):

| Part | Value |
|---|---|
| Cover | Column 168: a **168 x 168 square at the row back's left** (x 529..697), `#FF1A1A1A` under the cover: `UniformToFill`, centred across, top-aligned, clipped square (**no rounded corners**), decoded 200 wide. No cover: the dark square (no title fallback; the title is beside it). (E/HomeTiles.xaml:135-148) |
| Text block | `Margin="22,0,24,0"`: **x 719..1722**, vertically centred in the 168 |
| Line 1 | Title: **Segoe UI Semibold** 26, white, one line, ellipsis |
| Line 2 | 18 under line 1: the date last played in the user's short date format (`d`, current culture: "11/10/2026"), then "`  •  {platform}`" (two spaces, U+2022, two spaces, the game's first platform name; E/PlayniteLibrary.cs:93), both Segoe UI 26 `#FF8F8F8F`. In the pictures, whose games have no platform, the date is followed by the bullet alone ("11/10/2026  •"). |
| Line 3 (#78) | 8 under line 2, only while the game is in the Queue: "Queued" or "Installing...", Segoe UI 26 white |
| No badges | No source badge, no state badge, no title band on the cover |

With two lines the block is 34.6 + 18 + 34.6 = 87.2 tall, its top 40.4 under the back's top.

Focus (:162-187): a ring round the row, `Border Margin="-4" CornerRadius="8" BorderThickness="4"`, `#FF2CA243`: the stroke occupies the 4 just **outside the item box**, leaving 4 clear between it and the row back (ring outer x 521..1754, y item top - 4 .. item bottom + 4). WPF centre-line radius 8 (outer 10, inner 6). With `XboxFocusGlow` (home.md 4.3). The back does not change colour; the focused row is drawn over the next (`ZIndex 1`). Shown and hidden at once.

### 6.3 Buttons with focus on a row

| Input | Does | Source |
|---|---|---|
| A | As a Home tile: the game's **Game hub** after the 300 ms double-tap window; a second A within it **plays** the game (or returns to it if running). The port: the hub at once, no double tap (section 14, question 4) | E/MyGamesControl.xaml.cs:1435-1442; C/ExtensionCore.cs:850-854, 1331-1362; CT/MyGamesTests.cs:433 |
| Menu | The Tile menu (`PromptOpen.wav`) | C/ExtensionCore.cs:840-843 |
| Up / Down | The row above / below, across sections; stops at the ends (`DirectionalNavigation="Contained"`) | E/MyGamesControl.xaml:697 |
| Left | The Play history nav item | E/MyGamesControl.xaml.cs:1374-1380 |
| Right | Nothing | :1381-1384 |
| Y, X, View, LB/RB, RS | Nothing | :636-654 |
| B | Home | |

Focus memory: the game whose row last had focus is refocused after a rebuild of the list, after its Game hub closes, and on Right from the nav; otherwise the first row (E/MyGamesControl.xaml.cs:70-73, 321-329, 1291-1299, 1521-1531).

### 6.4 Scrolling

The list scrolls by pixel (`ScrollUnit="Pixel"`), virtualised, no scroll bar, the header fixed. Scrolling to the focused row is WPF's own bring-into-view (the least movement that shows the row), with **no animation: NOT FOUND**. In `history-3-after-right-down.jpg` focus has moved to the second row, already in view, and nothing has scrolled. Exactly where a row rests after a scroll, and whether the 80 end padding shortens the viewport or only follows the last row, are WPF template internals: **UNVERIFIED** (the pictures show rows drawn right to the screen's foot, y 1080). See open question 3.

### 6.5 Empty

No played games: the list is collapsed and "Games you play show up here." shows at the content row's top, x 548 (`Margin="28,0,0,0"`), y 190: Segoe UI 26, `#FF8F8F8F`. Focus stays on the nav item (E/MyGamesControl.xaml:789-790; E/MyGamesControl.xaml.cs:386-388, 1523-1527).

---

## 7. Full library

### 7.1 What it is

The grid of section 5 (same list, same tile, same columns), showing **every game the user's current Playnite filter matches**, installed or not, under three things stacked above it: the tabs, the Sort and Filter row, and (clipped under that row) the grid (C/MyGamesState.cs:95-99; CT/MyGamesTests.cs:484).

Vertical stack at x 520..1750:

| Element | y | Source |
|---|---|---|
| Tabs | **190..246** (grid `Margin="520,190,170,0" Height="56"`) | T/Views/Main.xaml:452 |
| Invisible focus target under the tabs (`TabsFocus`) | 190..246, x 520..1750 | E/MyGamesControl.xaml:625-632 |
| Sort and Filter row (`LibraryBar`, `Margin="4,72,4,0" Height="60"`) | **262..322**, x 524..1746 | :639 |
| Grid's clip edge | **326** (list grid `Margin="436.5,126,86.5,0"`, `ClipToBounds`) | T/Views/Main.xaml:351-354 |
| Fade over the grid's top | y 326..338, x 516.5..1753.5: vertical gradient `#FF1F1F1F` -> `#001F1F1F` (the page colour, solid at the clip, clear 12 below) | T/Views/Main.xaml:727-745 |
| Row 0 tiles | top **350** = 24 under the clip | E/MyGamesControl.xaml.cs:59, 543-557 |

Grid arithmetic: the extension lifts the list by `61.79 + 7.5 - 24 = 45.29` (quarter cell + tile gap - padding), so the list's own top is 326 - 45.29 = 280.71 and row r's tiles are at **350 + 247.14 r: 350, 597.14, 844.29** (bottom 1076.43); row 3 would start at 1091.43, below the screen. Three rows show; there is no partial fourth. Horizontal positions are those of section 5.2. The list's own box runs on to y 1145.71, 65.71 below the screen's foot.

Scrolled tiles pass up under the fade and are cut at y 326, so they never draw over the Sort row. Scrolling is Playnite's as in section 5.5; note that its viewport ends below the screen, so how it keeps a focused fourth row on screen is **UNVERIFIED** (open question 2).

Not-installed games carry the Download state badge (section 5.3); the pictures show it.

### 7.2 Tabs

T/CustomControls/FilterPresetSelector.xaml:9-70. One tab per Playnite filter preset flagged "show in Fullscreen quick selection", in Playnite's order, left to right from x 520 (the pictures: "My Picks", "All", "Favorites", "Most Played", "Recently Played", the order of Home's Groups).

| Part | Value |
|---|---|
| Tab | Height 56; width = text + 22 + 22; `Margin="0,0,8,0"` (8 between tabs). Measured: "My Picks" x 520..656, the next tab from 664. |
| Label | The preset's name, Segoe UI 26, white (both states), vertically centred |
| Inactive | No fill |
| **Active** (the preset applied) | Fill `#FF107C10`, radius 2, with the slab glow (section 2.5). |
| Overflow | A horizontal scroller, no scroll bar, 24 of glow room all round; how it follows the active tab is Playnite's: UNVERIFIED |

**The tabs themselves never take focus.** Focus "on the tabs" is on `TabsFocus`, an invisible button laid under them, and **nothing changes on screen when it takes focus: the active tab looks the same focused or not** (E/MyGamesControl.xaml:620-632). No tab is lit when the user's current filter is not a preset (as Full library opened from the default Favourites Group, CT/SeeAllTests.cs:72). With no flagged presets the strip is empty and `TabsFocus` can still be focused, showing nothing.

Buttons with focus on the tabs (E/MyGamesControl.xaml.cs:764-825):

| Input | Does |
|---|---|
| Left / Right | Applies the preset of the tab before / after the active one (the grid flies in from that side, section 3.4). Right at the last tab: nothing. **Left at the first tab: to the Full library nav item.** With no tab active, Right does nothing and Left goes to the nav. |
| Down | To the Sort and Filter row, on what last had focus there, else Sort |
| Up | Nothing |
| A | Nothing (the tab focused is the one showing) |
| Y | Opens the name box |
| LB / RB | Previous / next preset (Playnite's command, from anywhere on Full library; whether it wraps at the ends: UNVERIFIED) |

Switching tab: the grid shows that preset's games in the preset's own sort, unless a sort was chosen on the dropdown since the page opened, which is put back at once (section 7.4); the name filter, if any, stays on top; the count follows; "Right from the nav" memory is reset (E/MyGamesControl.xaml.cs:429-440; C/ExtensionCore.cs:492-499).

### 7.3 The Sort and Filter row

E/MyGamesControl.xaml:639-664; CT/FullLibraryTests.cs:353-364. A DockPanel, y 262..322:

| Part | Rectangle | Drawing | Source |
|---|---|---|---|
| Sort box | **x 524..924** (400 x 60) | Fill `#FF333333`, radius 2. Content inset 22: label at x 546, Segoe UI 26 white: "**Sorted by {name}**" (e.g. "Sorted by A–Z", an en dash) or just "**Sort**" when the grid's sort is none of the five options; at the right (right edge x 902) the chevron U+E70D, icon font 18, white. Focused: fill `#FF107C10` + slab glow. | :309-333, 642-648; C/MyGamesState.cs:166 |
| Name box | **x 948..1348** (400 x 60), 24 right of Sort. **Only while open or while a name filters the grid**; otherwise collapsed and nothing is there. | Section 7.5 | :649-652 |
| Count | Right-aligned, its right edge 24 left of Filter (measured x 1588) | "`{n} games`", Segoe UI 26, `#FF8F8F8F`: the games the grid shows under the tab and any name | :661-663 |
| Filter button | Right edge **x 1746**; width = 26 + glyph + 14 + "Filter" + 26 (measured **x 1612..1746**), height 60 | `FooterButton`: fill `#FF333333`, radius 2; glyph U+E71C (Filter), icon font 20, white; "Filter" Segoe UI 26 white. Focused: fill `#FF107C10` + slab glow. | :121-144, 653-659 |

Buttons on the row (E/MyGamesControl.xaml.cs:827-891):

| Focus on | Left | Right | Up | Down | A | Y |
|---|---|---|---|---|---|---|
| Sort | Full library nav item | Name box if showing, else Filter | Tabs | Grid | Opens the Sort list | Opens the name box |
| Filter | Name box if showing, else Sort | Nothing | Tabs | Grid | Opens the name box | Opens the name box |

Down goes to the grid game focus last left (by Up from the first row, or Left to the nav), else the first; with an empty grid focus stays. Up from the grid's **first row** comes to this row, on what last had focus in it, else Sort (:690-697, 738-756). LB/RB switch tabs. B: section 3.5.

### 7.4 The Sort list

E/MyGamesControl.xaml:339-372, 671-686; E/MyGamesControl.xaml.cs:899-999; C/MyGamesState.cs:369-378.

- A popup window 8 under the Sort box: **x 524..924, y 330..646** (400 wide; `Padding="0,8"`; five options of 60). Opaque fill `#FF2B2B2B`, square corners, no border, no shadow. It is a window of its own because the grid draws over the page. **No open or close animation: NOT FOUND.**
- Options, top to bottom: **A–Z** (name ascending), **Last played**, **Recently added**, **Playtime**, **Release date** (each of those four descending: newest or most first). Games without the field come last; ties in name order (C/GameListFilter.cs:35-62; CT/FullLibraryTests.cs:33-59).
- An option: 60 tall, full width. Label Segoe UI 26 white at x 586 (`Margin="62,0,24,0"`). The option the grid is in shows a tick: U+E73E, icon font 20, white, at x 542 (`Margin="18,0,0,0"`). Focused: fill `#FF107C10` + slab glow (clipped by the popup's own edges at the sides).
- Opening puts focus on the ticked option, or the first if none is ticked. Up/Down move and stop at the ends; Left/Right nothing; **A sorts the grid by that option, closes the list, focus on the Sort box**; B closes it, focus on the Sort box. Every other pad button is swallowed while it is open (LB/RB do not switch tabs).
- Choosing: the grid is re-sorted in place, the label reads "Sorted by ...", and Down from the row starts again at the first game. The choice **survives tab switches and moving along the nav and back, and is forgotten when the page closes** (CT/FullLibraryTests.cs:84-126, 331). Until one is chosen the label names whatever sort the grid is in (the user's own, or a tab's preset's).

### 7.5 The name box and the name filter

E/MyGamesControl.xaml:385-434; E/MyGamesControl.xaml.cs:1001-1204; C/ExtensionCore.FullLibrary.cs:58-88.

Drawing (400 x 60 at x 948..1348, y 262..322): fill `#FF333333`, radius 2 (**it does not turn green on focus**, so the name stays legible). While empty: placeholder "**Filter by name**", Segoe UI 26, `#FF8F8F8F`, from x 970 (`Margin="22,0,14,0"`). Otherwise the name, Segoe UI 26 white, left-aligned from about x 968 (`Margin="20,0,14,0"` plus the text box's own 2 of padding, UNVERIFIED), clipped before the glyph. At the right, in its own column: glyph U+E721 (Search), icon font 20, white, right edge at x 1326 (`Margin="0,0,22,0"`). Focused: a ring **on the box's own edge, inside it** (`BorderThickness="4" CornerRadius="2"`, no negative margin), `#FF2CA243`, with `XboxFocusGlow` (so the glow's centre line is the ring's). It lies inside because the grid and its fade, only 4 below, would paint over a ring standing out (CT/FullLibraryTests.cs:432).

The box is **never typed into**; it only shows the name. Behaviour:

1. Y anywhere on Full library (grid, tabs, row), or A on Filter: the box **opens** and takes focus. Nothing is filtered yet.
2. A or Y on the box: **text entry over the dimmed page** on the Console keyboard, caption "**Filter by name**", starting from the current name (the Console keyboard and text entry are specified in search.md). **The grid narrows on every change as it is typed**, to games whose name contains the text, on top of the tab and in the current sort; the count follows (CT/FullLibraryTests.cs:140-163, 205).
3. Start keeps the name: if it is not empty, differs from the name before, and the grid has games, **focus goes to the narrowed grid's first game**; otherwise focus returns to the box, still open. B in text entry drops it: the name before is put back, focus on the box (E/MyGamesControl.xaml.cs:1164-1195; CT/FullLibraryTests.cs:222-249).
4. On the box: Left to Sort, Right to Filter, Up to the tabs, Down to the grid; **B clears the name, closes the box, focus on Filter**. Left empty, the box closes as focus leaves it. While a name is set the box stays showing with the name in it.
5. B elsewhere on Full library while a name is set clears it first (section 3.5). The name goes when the page moves to another nav item or closes; it never outlives the page (C/ExtensionCore.FullLibrary.cs:115-124; CT/FullLibraryTests.cs:274-329).

Matching is Playnite's name filter; the tests show a case-insensitive "contains" ("o" finds Forza Horizon 5 and Halo Infinite; "ha" finds Hades and Halo Infinite). Anything beyond that (accents, word order, fuzzy matching): UNVERIFIED.

### 7.6 Counts

- Nav "Full library" count and the row's "`{n} games`": the games the current filter matches, **name filter included** (so the nav's count changes with the tab and the name; in the pictures 14 on "All", 6 on "My Picks") (C/ExtensionCore.cs:1877-1880; E/PlayniteHost.cs:762-789; CT/FullLibraryTests.cs:261; CT/MyGamesTests.cs:68-88).
- Unknown (Playnite not ready): the count is null and the texts' number is blank: exact rendering UNVERIFIED.

### 7.7 Buttons with focus on a grid tile (Full library)

As Games (section 5.4), except: **Y opens the name box** (not Search); **Up from the first row goes to the Sort and Filter row**; Down from the last row does nothing ("Looking for more?" is Games' only); Left from the first column goes to the Full library nav item; LB/RB switch tabs.

### 7.8 Empty

A tab with no games, or a name that matches nothing: the grid is empty and the "no games found" text of section 5.6 shows centred in the list's box, here centred on about (1135, 736) (box y 326..1145.71). After typing a name that finds nothing, focus is on the name box (rule 3). Tabs, row and count ("0 games") stay.

---

## 8. Apps (M4)

Slot and essentials only. E/MyGamesControl.xaml:195-230, 763-787; E/AppTiles.xaml:16-79; CT/AppsTests.cs.

- **Slot**: nav item 1 (section 2.5), and the content row. A grid of **five columns** (`AppsPerRow = 5`, E/MyGamesControl.xaml.cs:46), tiles square: list `Margin="-24,-24,-24,0"`, items 16.5 in, each tile `Margin="7.5"`: **tile 234 x 234, pitch 249, first tile x 520..754, y 190..424**; column c at x 520 + 249 c (last right edge 1750), row r at y 190 + 249 r. (Flush with the content's edges, so 6 left of and 2.4 wider than a Games tile.) Vertical scrolling by WPF, not virtualised, 80 end padding.
- **Tile**: clipped to radius 6. Face: white `#FFFFFFFF` with the App's icon inset 18, `Uniform`, over a soft shadow of its own shape (blur 18, depth 3, opacity 0.3); a Microsoft Store app's tile art instead fills the tile edge to edge over `#FF107C10`, no shadow; no icon: glyph U+ECAA as large as the inset allows, `#FF333333` on white (white on the accent for a Store app). Focused: Home's ring (4 thick, radius 14, 8 out, with glow) and a flat `#E6000000` band at the foot with the name, Segoe UI 26 white, `Margin="8"`, wrap, two lines (`MaxHeight="72"`), fading as Home's title band (150 ms after 50; out 100).
- **Behaviour**: Apps are Playnite's Software tools, in Playnite's order. A starts the App (no Launch screen, no quit prompt, even while a game runs). Left from the first column to the nav; other edges stop. No Tile menu, no hint bar.
- **Empty**: "Software tools you add in Playnite show up here." at x 548, y 190, Segoe UI 26 `#FF8F8F8F`.
- **In the port** (#77): Apps are added in Settings, not Playnite, in the order added; the empty line says so ("Apps you add in Settings show up here."). Everything the port decided, and the focus log, is in [apps.md](apps.md).

---

## 9. Manage (M3) and Manage Queue (M4)

### 9.1 Manage

The page under the nav's line (CONTEXT.md:34-35). Header "Manage"; the content row holds **one box**, the Queue box, and nothing else: "The console's other boxes (updates, storage, its settings) have nothing to stand for here" (E/MyGamesControl.xaml:804-815).

Box (style `Box`, :237-270; the same box as Settings'):

| Part | Value |
|---|---|
| Rectangle | **437 x 131 at x 520..957, y 194..325** (`Margin="0,4,0,0"`) |
| Fill | `#FF2D2D2D`, square corners; **focused `#FF4A4A4A`** |
| Icon | Glyph U+E896 (Download), icon font **34**, white, centred in the box's first 100 (x 520..620), vertically centred |
| Label | "Queue", Segoe UI 26 white, from x 620 (`Padding="100,0,30,0"`), vertically centred |
| Count (#78) | The Queue's count, Segoe UI 26 white, right-aligned to x 927, only while something is queued |
| Focus ring | `Margin="-8" BorderThickness="4" CornerRadius="8"`, `#FF2CA243`, with `XboxFocusGlow`: stroke 4 to 8 outside the box (centre-line radius 8) |

Buttons (E/MyGamesControl.xaml.cs:1736-1759): A opens Manage Queue (M4); Left to the Manage nav item; Right, Up, Down nothing; B Home. Right or A on the Manage nav item puts focus on the box. No hint bar.

Since #79 A opens Manage Queue, empty or not ([manage-queue.md](manage-queue.md)).

### 9.2 Manage Queue (M4)

Its own document since #79: [manage-queue.md](manage-queue.md). Slot and entry points in short (E/MyGamesControl.xaml:818-950; CONTEXT.md:38-39).

- **Slot**: a page of its own over the whole of My games & apps: the same `#FF1F1F1F` fill across all 1920 x 1080, **the nav, its foot and the divider hidden**. Title "Manage Queue", Bahnschrift SemiLight 42, at x 96, y 72. Under it a vertical scroller with content x 96..1824: "Recently installed" (a row of 200 x 200 Home tiles), then "Installing" (rows 250 tall), or with nothing queued an outlined empty box 330 tall. The Manage nav item counts as showing while it is up.
- **Entry points**: Manage's Queue box (B returns to Manage, focus on the box); Home's Downloads tile (B returns to Home, on the tile); a Game hub's Queue action (B returns to the hub, focus on Queue); the Guide button during a Queued Toast (C/ExtensionCore.Queue.cs:110-171; C/ExtensionCore.cs:1013-1014, 1732-1736).
- View there opens the focused game's Tile menu (C/ExtensionCore.cs:844-849).

---

## 10. Hint bar and sounds

### 10.1 Hint bar

Drawing: home.md 10.4 (bottom-right, right edge x 1860, bottom y 1028, 63 tall, `#D9030303`). It is over the page and does not move; it **overlaps the grid's fifth column in the third row** (measured bar x 1627..1860, y 965..1028 with one hint). Shown and hidden at once. Contents (C/ExtensionCore.Hints.cs:19-22, 74-91; CT/MoreOptionsHintTests.cs:204-311; CT/YSearchHintTests.cs:63-99):

| Focus | Hints |
|---|---|
| Nav item; header; tabs; Sort box, name box, Filter; the Sort list; SEE FULL LIBRARY; Manage's box; an App | **None** (bar hidden) |
| A game in **Games'** grid | "More options" (Menu), then "Search" (Y) |
| A game in **Full library's** grid | "More options" only (Y opens the name box, which is not hinted) |
| A Play history row | "More options" only |
| Manage Queue row or cover (M4) | "More options" only (shown with the Menu glyph, though View also opens the menu there) |
| Tile menu, a prompt, the Guide, text entry over the page | None |

### 10.2 Sounds

Catalogue and volume: home.md section 9. On this page:

| Event | Sound | Played by | Source |
|---|---|---|---|
| Focus lands on a nav item, a grid tile, a Play history row, an App, the Sort box, Filter, a Sort option, SEE FULL LIBRARY, Manage's box, the (invisible) tabs focus | `navigation.wav` (one file, no rotation) | Playnite, for its buttons and lists; the extension plays no navigation sound on this page. ("The highlight and its sound follow focus at once", E/MyGamesControl.xaml.cs:1301-1302.) Whether the read-only name box sounds: UNVERIFIED. | E/PlayniteSounds.cs:69-75 |
| A activates a button (nav item by keyboard/mouse click, Sort, Filter, a Sort option, the box) | `activation.wav`, Playnite's, on a button's click. The pad's A reaches most of these as a key, not a click (e.g. E/MyGamesControl.xaml.cs:893, 1238): whether Playnite then sounds it is **UNVERIFIED** | Playnite | |
| Menu on a game (Tile menu opens) | `PromptOpen.wav` | Extension | C/ExtensionCore.TileMenu.cs:59-71 |
| B closes the Tile menu or a prompt | `PromptClose.wav` | Extension | C/ExtensionCore.cs:803-807 |
| The page opening, B leaving it, an item's content changing, the fly-in, a tab switch, a sort chosen, the name box opening, scrolling, an edge bump | **none: NOT FOUND** | - | - |

---

## 11. Empty, loading and error states

### 11.1 Summary

| Where | State | What shows |
|---|---|---|
| Games | no installed games | Section 5.6 |
| Full library | tab or name matches nothing | Section 7.8 |
| Play history | nothing played | Section 6.5 |
| Apps (M4) | none / unreadable | Section 8; the nav item hidden when unreadable, and a page left on Apps moves to Games (C/ExtensionCore.cs:1631-1634) |
| Storage ring | no drive | Hidden |
| Tabs | no flagged presets | Empty strip |

### 11.2 Covers

No fade-in, no shimmer. A grid tile is `#FF1A1A1A` until Playnite has loaded its cover; a Play history cover is decoded (200 wide) as its row is built. A cover that fails: the dark tile.

### 11.3 While the library updates

The top bar and its progress indicator are hidden with Home, so the theme repeats it on this page: status text (Segoe UI 26, `#B3FFFFFF`) then the 26 x 26 Spinner (home.md 10.1), right-aligned to x 1815 in a strip y 84..144 (`Margin="0,84,105,0" Height="60"`), 12 between them; not focusable (T/Views/Main.xaml:405-441). It sits right of the header, above the content row, and over the Games count's line (the count is at y 88, right edge 1750: the two can overlap; nothing prevents it: NOT FOUND).

During an update the grid follows Playnite's list as games arrive; Play history and Apps are replaced only when what they show changed, and focus is put back on the same game or App (E/MyGamesControl.xaml.cs:321-339). The Full library count is re-read once a burst of changes is over (:469-482).

### 11.4 Errors

None are drawn. A grid that cannot be laid out, a sort or name filter Playnite cannot apply, and an unreadable drive are logged and the page carries on without (E/MyGamesControl.xaml.cs:512-519; E/PlayniteHost.cs:729-733, 745-749).

---

## 12. Text rules, Reduce motion and small traps

### 12.1 Text rules

| Text | Font | Size | Colour | Wrap / trim |
|---|---|---|---|---|
| Page header; "Manage Queue" | Bahnschrift SemiLight (350) | 42 | `#FFFFFF` | 1 line, character ellipsis |
| Nav label | Bahnschrift SemiLight (350) | 26 | `#FFFFFF` | 1 line, ellipsis |
| Nav count | Segoe UI | 26 | `#FFFFFF` | 1 line |
| Nav "Queue" line (M4) | Segoe UI | 26 | `#8F8F8F` | 1 line |
| Gamertag in the nav | Segoe UI | 26 | `#8F8F8F` | 1 line, ellipsis |
| "All storage", free space, used share | Segoe UI | 26 | `#FFFFFF` | 1 line |
| Header count, row count ("n games") | Segoe UI | 26 | `#8F8F8F` | 1 line |
| Grid tile title (focused band) | Segoe UI | 26 | `#FFFFFF` | Wrap, 2 lines (MaxHeight 72), character ellipsis |
| "Installing..." on a tile (M4) | Segoe UI | 26 | `#FFFFFF` | 1 line, ellipsis |
| Play history section header | Bahnschrift Regular | 30 | `#FFFFFF` | 1 line, ellipsis |
| Play history title | Segoe UI Semibold | 26 | `#FFFFFF` | 1 line, ellipsis |
| Play history date and platform | Segoe UI | 26 | `#8F8F8F` | 1 line, ellipsis |
| Tab label | Segoe UI | 26 | `#FFFFFF` | 1 line, none |
| Sort label, Sort option, "Filter", "SEE FULL LIBRARY", "Looking for more?", "Queue" | Segoe UI | 26 | `#FFFFFF` | 1 line, ellipsis |
| Name box text / placeholder | Segoe UI | 26 | `#FFFFFF` / `#8F8F8F` | 1 line, clipped (no ellipsis) / ellipsis |
| Empty notes (Play history, Apps) | Segoe UI | 26 | `#8F8F8F` | 1 line, ellipsis |
| "No games found" (grid) | Segoe UI | 32 | `#FFFFFF` | centred |
| Icons | `Segoe Fluent Icons`, fallback `Segoe MDL2 Assets` | nav 24; box 34; Filter, tick, search 20; chevron 18; state badge 14 | `#FFFFFF` | - |

### 12.2 Reduce motion on this page

| Thing | Normal | Reduce motion | Source |
|---|---|---|---|
| Fly-in along the nav | 50 slide + fade, 120 ms | Nothing: in place | E/PaneFlyIn.cs:74 |
| Fly-in across tabs | 50 slide, 120 ms | Nothing | :74 |
| Grid tile title band | Fades (150 after 50; 100) | At once | T/DerivedStyles/ListGameItemStyle.xaml:127-150 |
| App tile title band (M4) | Fades | Fades (unchanged) | E/AppTiles.xaml:57-73 |
| Grid scrolling | Playnite's smooth scrolling setting | Unchanged (not the Bundle's) | E/MyGamesControl.xaml.cs:42 |
| Spinner | Turns | Turns | E/Motion.cs:8-14 |

### 12.3 Small traps

- The grid on Games and Full library is not the extension's tile: its title band is a **gradient**, inset 12/36/12/10; its state badge is 24 x 24 with a **Download mark for a not-installed game**; A has **no double-tap**. Play history rows and Home tiles do have the double-tap.
- Dim text here is opaque `#8F8F8F`, not Home's 70% white.
- Focus on this page has two looks: **a ring** (grid tiles, Play history rows, Manage's box, the name box) and **an accent fill** (nav items, Sort, Filter, Sort options, SEE FULL LIBRARY). Both carry the glow; a filled control has no ring.
- "Showing" (grey fill + green bar at the right) and "focused" (green fill) are different states of a nav item, and after Right both exist at once on different elements: the item stays grey-with-bar while focus is in the content.
- Focusing a nav item changes the content without A, after 0 or 150 ms (section 4.2); A or Right never lands in the previous item's content.
- The header is fixed; only the grid or list under it scrolls. On Full library the grid is cut at y 326 and fades over 12.
- Play history rows are 5 further right at the left than everything else (x 529 vs the Sort row's 524), and Games' tiles 2 (x 526): both are artefacts of Playnite/WPF that the source's comments do not intend (open question 1).
- The Sort box reads "Sort", not "Sorted by ...", when the sort is not one of the five (for example a preset sorted by name descending) (CT/FullLibraryTests.cs:73).
- Y means three things: Search on Games' grid, the name box anywhere on Full library, nothing elsewhere on the page.
- LB/RB act on the whole Full library page (nav included), and nowhere else on My games & apps.
- B with a name filter set clears the filter instead of leaving; a second B leaves.
- B from the page opened by the top bar button does not return to the button.
- The tabs' focus is invisible.
- The nav's counts are live: Full library's follows the tab and the name.
- The header count reads "games" for any number, 1 included.
- "A–Z" has an en dash (U+2013).

---

## 13. What the pictures confirmed

Measured in `see-all-20261110-113608/` (build 0.2.0), against the numbers above derived from `f11e4f9`.

Confirmed (to the pixel, within JPEG edges):

- Page fill `#1F1F1F`; divider x 441..443 `#303030`; nav line y 482..484, x 36..383.
- Profile picture x 75..129, y 78..132; gamertag from x 143.
- Nav items at 72 pitch from y 190; the showing item's fill `#262626` y 406..464 (Full library) and 334..392 (Play history); its bar x 435..441 `#107C10`; label x 137; count right edge x 398.
- Header text from x 520.
- Tabs y 190..246, first tab x 520..656 (accent fill, glow round it), 8 between tabs.
- Sort box x 524..924, y 262..322, `#333333`; Filter x 1612..1746; count's right edge about x 1588; "Sort" / "Sorted by A–Z" labels.
- Full library tiles: row 0 y 350..582, row 1 from about 597, row 2 ending about 1076; tile about 232 square; focus ring 4 thick, 4 clear of the tile; title band a gradient with the title 12 in; Download badge at top-left on not-installed games; source badge bottom-left, hidden on the focused tile.
- Play history: header text x 552; section pitch 261; row backs `#292929` y 248..416, 509..676, 770..937; cover 168 square; title at x 719; ring 4 thick, 4 clear of the back; the hint bar "More options" at x 1627..1860, y 965..1028.

Corrected by the pictures (the source alone gives a different number):

- Grid tiles' x: 526 + 246.6 c, not the 524 + 247.4 c the theme's comment intends.
- Play history rows' left edge: 529 (ring from 521), not 524.

Not shown by any picture (derived from the source only): everything about Games (its tile rows at 195.29 ..., the fourth row's sliver, the header count, "Looking for more?"); a focused nav item; the fly-ins; the Sort list; the name box; the storage ring; Apps; Manage's box; Manage Queue; every empty state; any scrolled position. The 0.2.0 caveat of home.md was not seen to change any position on this page; sizes of text were not measured to the pixel, so the 26 px type is the source's.

---

## 14. What is Playnite-specific, and what to do about it

### 14.1 What Playnite's filter presets give this page (stated exactly, for filters.md)

A Playnite filter preset is a named, saved filter (any of Playnite's filter criteria), with an optional sort order and direction, and a flag "show in Fullscreen quick selection" (C/LibrarySnapshot.cs:135-165; E/PlayniteLibrary.cs:41-51). The page gets from them:

1. **The tabs**: one per preset with the flag, **in the order the user gave the presets in Playnite** (`GetSortedFilterPresets`; the database's order if that cannot be read; E/PlayniteLibrary.cs:200-213). That the selector itself uses the same order is Playnite's doing (UNVERIFIED in code, seen in the pictures). The same set and order as Home's Groups (home.md 6.1). There is no built-in "All" tab: "All" in the pictures is a preset the test library made. With no flagged preset there are no tabs at all, while Home still shows its default Favourites Group.
2. **The active tab** = the preset Playnite currently has applied to its Fullscreen list. That state is Playnite's and **persists**: Full library opens on whatever filter the user last left (a preset, or a preset since edited, or no preset, in which case no tab is lit) (CT/MyGamesTests.cs:484-518).
3. **Tab = filter + sort**: applying a preset replaces the grid's filter and, if the preset has a sort, its sort and direction; a preset without a sort leaves the grid's sort as it was (CT/SeeAllTests.cs:39-70). A sort chosen on the page's dropdown overrides presets' sorts until the page closes (section 7.4).
4. **See all**: a Group knows its preset's id; See all applies it (C/ExtensionCore.FullLibrary.cs:22-37; E/PlayniteHost.cs:710-715).
5. **LB / RB**: Playnite's previous / next preset commands, allowed only on Full library (E/PlayniteViewHost.cs:37-45).
6. **The count**: Playnite's evaluation of the current filter over the library (E/PlayniteHost.cs:762-789).
7. **The name filter**: a field of Playnite's live Fullscreen filter, set on top of the active preset without un-lighting its tab (E/PlayniteHost.cs:739-752).
8. **Sort orders available**: the dropdown's five are a subset of Playnite's; presets may carry others (Modified, play count, or an unlisted direction), shown as "Sort" (C/GameListFilter.cs:44-55; C/MyGamesState.cs:394-396).

**Recommendation** (the design is filters.md's): keep the tab strip, its drawing, its keys and See all's contract exactly, and feed it from the port's own saved filters and Steam collections. Whatever replaces presets must provide, per tab: a name, a stable id a Group can point at, a game set, and an optional sort; plus an order, a "current tab" that is remembered between visits, and a decision on whether an always-present first tab ("All games") exists, since without presets the Bundle's Full library has no tabs and no way to say "everything". Recommend that it does.

### 14.2 Everything else

| Playnite-owned thing | What the page leans on it for | Recommendation |
|---|---|---|
| The main list (`PART_ListGameItems`, `FullscreenTilePanel`, `GameListItem`) | Games' and Full library's grid, its cell sizing (3.5 cells to the height, the quarter cell above), virtualisation, scrolling, D-pad movement inside it, cover loading | **Replace** with the port's own grid at the measured geometry (sections 5.2, 7.1). Decide its scrolling (open question 2) and its edge rules (open question 5). |
| Taking the list over and giving it back (`SaveUserFilter`, `ShowInMainList`, `RestoreUserFilter`, `ReleaseUserFilter`, the crash file) | Games needs "installed, A–Z" on a list the user's filter owns; Play history must put its games in the list before a Game hub can open (C/ExtensionCore.cs:1589-1615, 1856-1874; E/PlayniteHost.cs:510-600) | **Drop.** In the port Games, Full library and Play history are three queries of the library; a Game hub opens for a game id. |
| Playnite's Game details view and game menu | A and Menu/X on a grid tile | **Port** as the Game hub and the Tile menu (their own specs). Decide the double-tap on the grid (open question 4). |
| Playnite's keyboard, filter panel and preset picker (`T/Views/FiltersView.xaml`, `FiltersAdditional.xaml`, `T/CustomControls/Filter*Selection.xaml`) | Nothing. **They are not reachable in the full Bundle**: the extension removes the bindings that open the filter panel and the preset picker and rebinds Y (E/PlayniteViewHost.cs:19-41; T/Views/FiltersView.xaml:2-3 "without the extension"; CONTEXT.md:59). Only `FilterPresetSelector.xaml` is in use, as the tabs. Any other route to them inside Playnite's own menus: UNVERIFIED. | **Drop** all but the tab strip. |
| Hidden games; installed flag; sorting name; platform; source; last activity, added, playtime, release date | What each list contains, the sorts, Play history's second line, the badges | **Port** from the library database. Platform has no Steam equivalent: see open question 7. "Recently added" needs a date added, which the port's library does not record yet (home.md section 11). |
| Playnite settings: "darken uninstalled games", "smooth scrolling", cover stretch | The `#AA000000` darken; animated grid scrolling | **Drop the settings**; choose fixed behaviour (open questions 2 and 8). |
| Software tools | Apps | M4. |
| Playnite's install state, Steam's queue | Queue line, download block, Installing bands, Manage Queue | M4. |
| The Profile's picture and gamertag | Nav's top | **Port** as Home's (M5 for real Profiles; until then what Home shows). |
| Windows drive space for the storage ring | Nav's foot | **Port** (from Steam library folders); it can ship in M3 since installed games and their folders are known, or wait for M4 with the download block (open question 9). |
| Playnite's focus and activation sounds | Every focus move on the page | **Port** as on Home: `navigation.wav` on each focus move, nothing else coded here. |
| Playnite's progress indicator and status text | Section 11.3 | **Replace** with the port's own "library updating" state, or drop while there is nothing to report. |
| Playnite's localised "no games found" | Empty grid | **Replace** with the port's own line; consider the page's dim note style (section 6.5) instead (open question 10). |
| WPF focus workarounds: "focus from nothing" on the nav, suspended letter keys, the invisible tabs focus target, the popup window for the Sort list, the name box that is shown but typed elsewhere | Making Playnite's window behave | **Drop** the workarounds; keep the visible behaviour (focus goes to content on opening; the Sort list overlays the grid; the name is typed on the Console keyboard over the page). |
| Playnite's D-pad repeat (80 ms) | The settle wait's reason | **Port** the 150 ms settle with the port's own repeat. |

---

## 15. Open questions

Questions 1 to 7, 9, 10 and 12 are answered in section 16, 8 and 11 in section 17; 13 is still open.

1. **Intended or measured x?** The grid's tiles measure 526..1744 (2 in from the Sort row's 524..1746) and Play history's rows 529..1746, where the source's comments intend both flush at 524..1746. Build the artefacts as the pictures show them ("exactly as the theme"), or the alignment the author intended? (Tile width follows: 231.6 measured, 232.4 intended.)
2. **Grid scrolling.** Playnite's panel decides how far the grid moves when focus goes to a row out of view, and animates only if the user's Playnite setting says so; none of it is in the repo. Proposal to confirm: scroll by whole rows (247.14) so the focused row rests in one of the three row slots, with the page's own slide (home.md 7.3: 300 ms cubic ease-out, instant under Reduce motion). On Games, does the quarter row at the foot stay as the hint that more follows?
3. **Play history scrolling.** WPF moves the least needed and does not animate. Same question: animate it as the grid, and where does a row rest (is the 80 end padding kept clear at the foot while scrolling, or only after the last row)?
4. **A on a grid tile.** The Bundle has no double-tap there (Playnite's binding) but has it on Play history rows and Home tiles. Keep the inconsistency, or give the port's grid the same 300 ms double-tap-plays rule everywhere?
5. **Grid edges.** Right at a row's last tile, Up from Games' first row, Down onto a shorter last row: all Playnite's, unverified. Proposal: stop at every edge; Down onto a short last row goes to its last tile.
6. **Manage in M3.** With the Queue and Manage Queue in M4, does M3 show Manage with its one box doing nothing, show it opening an empty Manage Queue (the console's empty box, section 9.2), or leave Manage out of the nav until M4?
7. **Play history's second line.** Playnite's platform ("PC (Windows)") has no source in a Steam-only library; the pictures show a dangling bullet when it is missing. Show the store ("Steam") there, drop the bullet when there is nothing after it, or keep the bullet as pictured?
8. **Darkening not-installed games in Full library.** A Playnite setting, off in the pictures. Fixed off (the Download badge alone marks them)?
9. **Storage ring in M3 or M4?** And which drive when games span several Steam library folders (the Bundle: the drive with most installed games).
10. **Empty grid text.** Playnite's centred 32 px line, or a dim 26 px note in the page's own style, and its wording for Games ("no installed games") and for a name that matches nothing?
11. **Tabs with nothing to show.** If the replacement for presets yields no tabs, is the strip's row kept empty (Sort row stays at y 262) or does the content move up? And what does focus on the tabs look like, given that the Bundle shows nothing for it?
12. **The Games count's wording** ("1 games") and whether the port's own progress text may overlap it (section 11.3).
13. **Sound on the pad's A** for the page's buttons, and on tab switches: unverified in the Bundle; one rule is needed for the port (shared with home.md's open point on A).

---

## 16. Decided in #52 (the shell, Games, Play history and Manage)

What the port does where section 15 asked, each settled as this document recommends or, failing that, as the Bundle does it. Questions 8 and 11 are Full library's, answered in section 17; 13 is shared with Home's and stays open.

1. **Measured x (question 1).** The artefacts are built as the pictures show them: the grid's tiles at x 526 + 246.6 a column, 231.6 wide; Play history's row backs 529..1746 with the row's item 525..1750. Not the flush 524..1746 the source's comments intend.
2. **Grid scrolling (question 2, and the issue).** The grid moves by whole rows of 247.14, the least that brings the focused row into one of its three whole slots, so the focused row is whole on screen and the next row shows its top: the quarter row at the foot stays. It slides with the page's own slide (300 ms, cubic ease-out) and is simply there under Reduce motion. With focus on SEE FULL LIBRARY the grid is at its end, "Looking for more?" and the button under its last row.
3. **Play history scrolling (question 3).** As the Bundle: the least that brings the focused row in, not animated. A section's first row brings its header in with it, and the last row brings the 80 of end padding under it; a row in the middle rests against the viewport's edge with nothing kept clear. The viewport runs to the foot of the screen.
4. **A (question 4, and the issue).** A on a grid tile opens the Game hub at once: no double tap there. Play history's rows did keep the Bundle's rule (a second A within 300 ms played the game instead) until the user's decision of 2026-11-17 (#214), which departs from the Bundle: A on a row opens the Game hub at once, as everywhere, and A again is the hub's Play. The hub opens over the page, and B there is the page again with focus on the game.
5. **Edges (question 5, and the issue).** Every edge stops and nothing wraps. Right at a row's last tile and Up from the first row do nothing; Left from the first column is the nav; Down onto a shorter last row goes to its last tile; Down from the last row is SEE FULL LIBRARY, and Up from there is the last row's first tile. In Play history Up from the first row and Down from the last do nothing, Right nothing, Left the nav.
6. **Manage (question 6).** Manage is in the nav in M3 with its one box, Queue, which takes focus and does nothing on A until M4 brings Manage Queue. Home's Downloads tile opens the page on Manage with focus on the box. (Since #79 the box and the Downloads tile open Manage Queue: [manage-queue.md](manage-queue.md) 7.)
7. **Play history's second line (question 7).** The date, two spaces, a bullet, two spaces, the platform, as pictured. The platform is "PC (Windows)" for any game a source found, since every source the port has is a Windows store; a game with no source keeps the bullet alone, as the Bundle's games with no platform do.
8. **The date and the week.** The date is in the user's short date format, as their Windows writes it (`screens::date`, #237; US English's M/D/YYYY off Windows and in the tests, as in the Bundle's pictures). The week starts on Sunday, as in the Bundle's pictures (US English): the user's first day of the week is not read yet, and `library::history::WEEK_STARTS` is where it goes.
9. **What Play history lists.** Every game that is not hidden and has been played: the later of the time its source says it was last played and the newest of its Play sessions, newest first. Sections are worked out when the library is read, so a page left open over midnight keeps yesterday's sections until the library is next read, as the filters' day counts do.
10. **Storage ring (question 9).** Drawn since #179 from the drives Settings > Storage reads (storage.md, section 6; `my_games::nav::storage_ring`), the current download's block 34 above it.
11. **Empty grid (question 10).** Playnite's line as the Bundle shows it: "No games found", 32 px, white, centred in the grid's box at (1135, 558.5). Play history's and Apps' empty texts are section 6.5's and section 8's.
12. **Apps.** The item is always in the nav with a count of 0 until M4, and its pane says "Apps you add show up here." The Bundle's text names Playnite's Software tools, which the port does not have.
13. **A tile with no cover.** Its game's name, centred on the tile's dark fill as Home's tile writes it, and gone under the title band while the tile has focus. (In the Bundle Playnite draws its own default cover there.)
14. **The count (question 12).** "N games" for any N, one included, as the Bundle writes it. The progress text of section 11.3 is not drawn yet (`my_games::progress`).
15. **Left out on purpose.** The fly-in's two held frames, which work around WPF's layout pass, and the playing mark's path: the mark is the glyph U+F5B0.
16. **What the focus log says.** `MyGames: <item up>: <where>`: `Games: Tile(3)`, `Games: SeeFullLibrary`, `PlayHistory: Row(2)`, `Manage: Queue`, and `Games: Nav(Apps)` while focus is on the nav, where the item up is the one showing, which trails focus by the settle wait of section 4.2. Full library adds its state after a `;` (section 17).

---

## 17. Decided in #53 (Full library)

What the port does for Full library, settled as this document and [filters.md](filters.md) recommend or, failing that, as the Bundle does it. Compared with the Bundle's pictures 07 and 09 to 14 of `port-reference-20261115-192720/`.

1. **The tabs.** `Library::groups()`: the saved filters that are Home's Groups, shown and with a game, in the same order. See all lights its Group's tab; the nav and SEE FULL LIBRARY open the page with none lit, the grid then every game that is not hidden, by name. A filter See all names that is not a tab (the empty Favourites Home shows when there is no Group, filters.md 4.1) lights none. A lit tab whose filter stops being a tab (it lost its last game, or was hidden) goes dark, and the grid is every game, focus staying on its game.
2. **A tab's games** are its filter's, in the filter's own sort and direction; a hidden game is in the Hidden tab only. The name narrows them on top; a sort chosen on the Sort list orders them.
3. **Stepping the tabs.** Left and Right on the tabs, and LB and RB from anywhere on the page (the nav included, not under the open Sort list), stop at the first tab and the last: nothing wraps (the Bundle's is Playnite's and unverified). Left on the first tab is the nav. With no tab lit, Right on the tabs is nothing and Left the nav, as the Bundle; **RB lights the first tab** and LB is nothing. A tab switch starts the grid again at its first game, which Down from the row and Right from the nav then land on.
4. **The strip scrolls** at once, the least that shows the lit tab whole, when the tabs are wider than 520..1750.
5. **Tabs with nothing to show (question 11).** The strip is left empty and the Sort row stays at y 262: nothing moves up. Focus on the tabs shows nothing, as in the Bundle, and the focus log says `Tabs`.
6. **The Sort list** has four options until there is a release date (filters.md 3.2): A–Z, Last played, Recently added, Playtime, each in its own direction. The box reads "Sorted by" the option the grid is in, and "Sort" when it is in none of them (a filter sorted the other way up). The tick stands after the option's name, where the Bundle's picture 11 has it (x about 720), not at x 542 as the source's margin reads: the glyph is centred in what is right of its 18 of margin. Over a long name ("Recently added") it lies on the name, as it would in the Bundle. The name box shows no ring while text entry is over the page, as in picture 13. A chosen sort orders every tab until the page closes, and is kept across the nav.
7. **The name** is matched as a case-insensitive "contains" on the game's name. The count beside Filter and the nav's count follow the tab and the name at once. (The Bundle's pictures 13 and 14 show the count still at the tab's whole, "15 games" over two games, and the lit tab gone dark while a name is set: both are Playnite's filter refreshing late, and are not copied.)
8. **Focus with nothing in the grid.** The page opened, or Right from the nav, on a grid that is empty: focus on the name box if a name emptied it, else on Sort. The row is always there to hold it; focus never stays on the nav for want of somewhere to go.
9. **Darkening not-installed games (question 8).** Off: the Download badge alone marks them, as in the pictures.
10. **Sounds (question 13's part).** A tab switch, a sort chosen and the name box opening are silent, as the Bundle has none; a move of focus (onto the tabs, the row, the Sort list's options) is the move sound, as everywhere.
11. **What the focus log says.** `MyGames: FullLibrary: <where>; tab <n>/<tabs> "<name>", sort <Sort>[ reversed], name "<name>", <count> games`. Where is `Tabs`, `Sort`, `Name`, `Filter`, `SortList(Playtime)` or `Tile(3)`, or `Nav(Item)` with focus on the nav. The tab's place is 0, its name empty, with none lit; `reversed` marks a sort the other way up from the Sort list's. While text entry is over the page the line is text entry's.
12. **The fly-in across tabs** is section 3.4's: the grid only, from 50 on the side of the tab lit, 120 ms; none when See all lights the tab, none under Reduce motion.


---

## 18. Decided in #250 (a tab for each launcher)

Full library's tabs gain each launcher's whole library, installed or not, so that a game owned and not installed can be found and installed from the page. The filters behind them, what each launcher's source can list and why some list installed games only, are [filters.md](filters.md) section 4.2. On this page:

1. **The tabs** are the Groups' as section 17.1 says, then each launcher's that has a game, in the filters' order (`Library::tabs()`): at first after every Group's tab, in the order Steam, Epic Games, GOG, EA app, Ubisoft Connect, Battle.net, Xbox. A launcher's tab is there whether or not it is a Group on Home. Its label is the launcher's name, drawn as every tab's (section 7.2); no mark of the launcher's is added to it.
2. **A launcher's games** are every game of its that is not hidden, installed or not, A–Z. The Sort list, the name box and the counts act on it as on any tab (17.6, 17.7).
3. **A game not installed** reads as one on every tab: the Download state badge at its tile's top left (section 5.3), not darkened (17.9). A on it opens its Game hub, whose action is Install; Menu or X opens its Tile menu, whose first row is Install. Either asks the launcher as Install does anywhere (where to install first, where the launcher asks: `install_where`). Once asked, the tile shows its Installing band while the Queue has it, as on every page; the game stays on the tab, as it is still the launcher's.
4. **Stepping the tabs** is section 17.3's: the launchers' tabs are more tabs, and RB from the last Group's tab reaches the first launcher's. The focus log names them as any: `MyGames: FullLibrary: Tile(1); tab 4/4 "Epic Games", sort Name, name "", 7 games`.
5. **Switched off in Settings > Launchers**, a launcher's tab goes with its games; a lit one goes dark as any tab that stops being one (17.1), the grid every game left. Switched on again it is back where it was.
6. **The picture**: `crates/screens/tests/reference/my_games_full_library_launcher_tab.png`, Epic Games' tab with four games not installed, focus on one of them. The Full library pictures from before show the fake library's Steam tab at the strip's end.
