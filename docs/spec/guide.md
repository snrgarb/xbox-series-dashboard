# The Guide, the Overlay and Toasts: specification (as the Bundle draws them)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run. This is the specification for milestone M2 (`docs/plan.md`).

Path shorthand used in citations, as in `home.md`:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`

A bare line number in a table's Source column refers to the file named in that section's heading.

All numbers are design units = px on a 1920x1080 canvas.

**No scale factor.** The Guide's whole view is inside a `Viewbox Stretch="Uniform"` whose root is 1080 tall and 1080 x (screen aspect) wide (E/OverlayWindow.xaml:904-905; E/OverlayWindow.xaml.cs:91-97). At 1920x1080 that is 1:1: unlike Home's tiles there is no 1.114. On a screen of another shape everything scales with the screen's height and the canvas is wider or narrower; the panel stays at the left, the Status area at the right.

**In scope for M2** (plan): the Overlay, the Guide's landing tab, Profile & system, the Utilities and their Flyouts (Audio, Notifications), Power on a hold, Toasts, the Status area in the Guide, the Guide button. Things the Bundle's Guide also holds that belong to later milestones are listed with their milestone and measured only as far as M2 must leave room for them: Search and Settings (M3), the Queue's Toasts (M4), Switch profile, the greeting Toast and Now Playing (M5), the Discord tab, Calls, achievements (M6).

**Pictures of the real thing.** `~/projects/xbox-series-theme/artifacts/e2e/`:

- `guide-over-game-20261104-165208/guide-open.jpg`: the landing tab over a stand-in game (no games listed).
- `achievements-20261110-105905/data-guide-recent.jpg`, `nodata-tile-opened.jpg`: Profile & system over Home.
- `achievements-20261110-105905/data-tilemenu.jpg`: the menu panel (as the Tile menu; the same control shows over the Guide).

They were taken with builds of 4 and 10 November, **before** the type was set at 26 (#229, commit `4c69cf3`): in them titles are 24 or 25, detail lines 22, hints 24, the badge 16, menu labels 22, and the clock sits 12 higher. Geometry is otherwise unchanged and was measured against the XAML (panel, tab strip, underline, entries, rings, Utilities, hints, Status area, dim, menu panel all agree to the pixel). Where pictures and source disagree, this spec follows the source.

**There is no picture of the Bundle showing** a Flyout, a Toast, Power, a prompt over the Guide, or the landing tab with games in it. Those sections are from the XAML alone. (Since then `port-reference-20261115-200355/` has pictures of the Flyouts, Power and the greeting's Toast: `docs/spec/pictures.md`. The Toast's is measured in 9.2.) The console captures the XAML's comments cite (`docs/reference/console/G-guide`) are not in the repository; only `docs/reference/notifications/IMG_2336.jpg`, `IMG_2337.jpg` (photos of a console's Toast) are.

The later set `port-reference-20261115-200355/` (Bundle 0.4.2) has the Guide at 50 to 58. Two of its names say more than its pictures show: `56-guide-profile-power-focused.jpg` has focus on **Settings**, and `57-guide-power.jpg` is the **Settings page** that A opened from there (the run was one Down short). The set after it, `port-reference-20261115-215324/`, has them right: `56-guide-profile-power-focused.jpg` (focus on Power), `57-guide-power.jpg` (Power from its entry, focus on Close, the panel out of focus behind), `59-guide-power-option-focused.jpg` (focus on Turn off PC) and `62-guide-power-held.jpg` (by the hold: no panel). **Power is measured against those** (section 13). There is no picture of a menu or prompt over the Guide; the menu panel itself is in `port-reference-20261115-192720/23-tile-menu-installed.jpg` and `26-tile-menu-remove-prompt.jpg`.

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite, WPF or font internals that are not in the repository.

---

## 1. The Overlay

The Overlay is one borderless, topmost window covering the whole monitor that holds the foreground window when the Guide opens (E/OverlayWindow.xaml:1-13; E/OverlayWindow.xaml.cs:268-281). It is see-through where nothing is drawn. It is shown only while the Guide (or a dialog on its own) is up, and hidden, not drawn empty, otherwise.

### 1.1 Layers, back to front

| # | Layer | Rect | Look | Source (E/OverlayWindow.xaml) |
|---|---|---|---|---|
| 1 | Dim | whole screen | `#8F000000` (black at 56%), times the dim's opacity 0..1 | 897-899 |
| 2 | Top shade | x 0..1920, y 0..420 | vertical gradient `#FA000000` @0, `#EB000000` @0.3, `#A6000000` @0.6, `#00000000` @1; opacity = the dim's. Not drawn when the panel is not shown (Power alone) | 912-931 |
| 3 | Hints | from (675, 168), section 3.6 | opacity = the dim's; not drawn without the panel or with a Flyout open | 936-973 |
| 4 | Panel | x 78..614, y 33..1045 | section 1.2; slides | 985-1023 |
| 5 | Panel contents | same rect | tabs, heading, entries, Utilities; slides on their own offset; not clipped by the panel | 1031-1206 |
| 6 | Flyout dim | the panel's rect, radius 12 | `#73000000`, only while a Flyout is open | 1218-1228 |
| 7 | Flyout | x 155..691, y 33..1080 | section 5 | 1235-1353 |
| 8 | Status area | right-aligned to x 1770, top y 86 | section 2.3; opacity = the dim's; not drawn without the panel | 1358-1369 |
| 9 | Dialog dim | whole screen | `#B3000000`, only while a menu, prompt or Power is up | 1371-1381 |
| 10 | Menu panel or Power | sections 6.1, 6.2 | | 1383-1385 |

Layers 4 to 7 share one horizontal offset, `Slide`; layer 5 adds its own, `ContentSlide` (981-984, 1031-1038).

Measured: a game pixel `#102050` under the dim alone reads `#060d23` (0.44 x), as 1 - 0x8F/255 gives.

### 1.2 The panel

| Part | Value | Source (E/OverlayWindow.xaml) |
|---|---|---|
| Rect | Width 536, `Margin="78,33,0,35"`: x 78..614, y 33..1045 (1012 tall) | 985-987 |
| Corner radius | 12, all four corners; the tab strip is clipped to it | 987, 1000-1004 |
| Background | vertical gradient over the whole panel: `#FF1F1F1F` @0, `#FF1E201C` @0.5, `#FF172116` @0.71, `#FF152611` @0.86, `#FF132A0E` @1 (grey turning dark green at the foot) | 988-999 |
| Tab strip | the top 102 (y 33..135), its own vertical gradient: `#FF1F1F1F` @0 and @0.52, `#FF1B1B1B` @0.6, `#FF191919` @0.74, `#FF171717` @0.95 and @1 | 1004, 1012-1023 |
| Border, shadow | **NOT FOUND**: the panel has neither | |

Opaque: nothing of the game shows through the panel.

### 1.3 Opening and closing

Constants: open 120 ms, close 150 ms, contents lag 50 ms (E/OverlayWindow.xaml.cs:22-25).

| | What moves | From, to | Time | Easing | Source (E/OverlayWindow.xaml.cs) |
|---|---|---|---|---|---|
| Open | Panel offset x | -700 to 0 | 120 ms | cubic ease-out | 129-136, 315-368 |
| | Contents' extra offset x | -700 to 0, starting 50 ms later | 120 ms | cubic ease-out | 289-304 |
| | Dim opacity (and with it the top shade, hints, Status area) | 0 to 1 | 120 ms | linear (the animation is given no easing) | 342 |
| Close | Panel offset x | to -700 | 150 ms | cubic ease-in | 144, 342 |
| | Contents | ride the panel, no lag: their own offset is held where it is, so a close before they have set off never brings them in after | | | 346-367 |
| | Dim opacity | to 0 | 150 ms | linear | 342 |

- The contents trail the panel on the way in: for the first 50 ms the panel slides in empty.
- All are "to" animations: reopening in the middle of a close goes on from where it is.
- The open animation starts only once the Overlay's first frame has been drawn, so no frame of it is lost to the window appearing (129-140). A port that draws the first frame itself has this for free.
- The window is hidden only once the slide-out is done **and** the foreground has been handed back (242-248). A 250 ms deadline finishes a slide-out that stalled (54, 250-266): a WPF workaround, drop.
- **Reduce motion** (Settings): all of these take 0 ms: the Guide appears and goes at once (321). Note the Bundle drops the dim's fade too.
- Power on a hold, with the Guide open: the panel slides out (150 ms, ease-in) while the dim stays; with the Guide closed the dim fades in with nothing sliding (137-140).

### 1.4 Tab change

LB and RB change tab (section 4). The old tab's content (heading, entries and Utilities: everything under the tab strip, which is the `TabContent` panel at E/OverlayWindow.xaml:1080; not the tab strip) goes at once. The new content starts 50 to the side moved toward (RB: from the right; LB: from the left) at opacity 0, and settles: offset to 0 over 120 ms cubic ease-out, opacity to 1 over 120 ms quadratic ease-out (E/PaneFlyIn.cs:20-23, 213-222; E/OverlayWindow.xaml.cs:114). None when the Guide opens onto a tab, none under Reduce motion (E/PaneFlyIn.cs:71-76).

### 1.5 Behind a dialog

While a menu, prompt or Power is up, the panel and its contents are replaced by a picture of themselves blurred (Gaussian, radius 12) under the `#B3000000` dialog dim (E/OverlayWindow.xaml:1215-1216; E/OverlayWindow.xaml.cs:157-192; E/PanelBlur.cs:20). The picture is of the panel and its contents alone, on clear, its bounds grown by the blur's radius (E/PanelBlur.cs:20-70): it is the panel that is out of focus, not what is behind it, and what is beside it (the hints, the Status area, the page) stays sharp under the dim. The picture is a WPF economy; the port draws the panel's layer live into a texture of its own and blurs that (`render::Group`, `Ui::group`; section 13).

---

## 2. The Guide's structure

All in E/OverlayWindow.xaml unless said.

### 2.1 Type and colours used throughout

| Name | Value | Source |
|---|---|---|
| Body text | Segoe UI 26, white `#FFFFFFFF` | 30-34; T/Constants.xaml:228-232 |
| Secondary text | Segoe UI 26, `#B3FFFFFF` | 299-303 |
| Headings | Bahnschrift 33, white, one line, ellipsis | 39-42 |
| Icons | `Segoe Fluent Icons` (fallback `Segoe MDL2 Assets`), white, centred in their box | 43-48 |
| Accent | `#FF2CA243` (focus ring, tab underline, badge, slider, switch on) | T/Constants.xaml (GlyphLightBrush) |
| Focused fill | `#26FFFFFF` (white 15%) | 829-832 |

Line heights are WPF's for the font (**UNVERIFIED**; `home.md` takes Segoe UI as 1.33 em, so 26 gives about 34.6, and Bahnschrift as 1.2 em, so 33 gives about 39.6). Positions below that depend on them are marked "about".

**There is no pressed state anywhere in the Guide (NOT FOUND):** an item is at rest, focused, or disabled. Nothing animates on focus: the ring and fill appear at once.

### 2.2 Tab strip

A row of tabs starting 21 into the panel: tab *i* (from 0) is the box x 99+84*i*..183+84*i*, y 33..135 (84 x 102) (1039-1046).

| Tab | Order | Icon | Source |
|---|---|---|---|
| Landing | 0 | The Xbox mark: the 24-unit path at line 27, drawn 33 x 33 (uniform), centred in the tab. `#FF8C8C8C` unselected, white selected | 27, 1050-1056 |
| Discord (M6; only when connected) | 1 when present | The Discord mark (path at line 29), 36 wide, same colours | 29, 1057-1062; C/GuideState.cs:215-223 |
| Profile & system | last | The profile picture in a 54 x 54 circle, centred; `#FF4A4A4A` disc under it (shows when there is no picture); picture fills the circle, cropped (`UniformToFill`). Same selected or not | 1063-1070 |

- Selected mark: a bar 84 wide, 6 high, radius 2, `#FF2CA243`, at the tab's foot hanging 4 below it: y 133..139. No glow (1071-1077). Measured x 99..182, y 133..138.
- In M2 without the Discord tab: Landing at x 99..183, Profile & system at x 183..267 (measured: mark x 124..157, picture x 198..251, y 57..110).
- Tabs are not focusable and have no focus state; they change with LB/RB only. The underline moves at once.

### 2.3 Status area (beside the panel, stacked form)

Right-aligned, its right edge 150 from the screen's (x 1770), top y 86 (1358-1369). Two lines, each right-aligned (E/StatusArea.xaml:172-193):

| Line | Contents | Source (E/StatusArea.xaml) |
|---|---|---|
| 1 | Height 28, a further 4 in from the right (right edge x 1766). Left to right: Discord mark (M6), PC battery, pad battery; 12 between | 174-186 |
| 2 | The clock: `Bahnschrift SemiBold` 56, tabular figures, white, 10 below line 1 | 187-192 |

The parts are Home's (`home.md` 2.4): pad battery = optional percent text "{0}%" (Segoe UI 26, raised 2, 8 to the glyph) and a cell 30 x 16, white border 2, radius 3, padding 2, white fill radius 1 as wide as the charge, with a nub 3 x 7 radius `0,1.5,1.5,0`; a wired pad shows icon U+E7FC at 24; no pad shows nothing (E/StatusArea.xaml:14-30, 69-98). PC battery: text Segoe UI 26 raised 2, 4 to an icon-font glyph at 26; only on a PC with a battery (38-63). Measured: pad cell x 1742..1765, y about 91..107.

Data and refresh (C/ExtensionCore.cs:425-486, 60-63):

| Value | Source | Refreshed |
|---|---|---|
| Clock | the system's short time format in the user's locale | on each minute boundary, and when the Guide opens |
| Pad battery | the first pad (E/GamingInputController.cs:89-134): wired, or a percent | each minute, on a pad connecting or leaving, and 500 ms after the Guide opens |
| PC battery | Windows power status | every 30 s and on a power change |
| Percent text | shown only if Settings' "show battery percent" is on (C/StatusState.cs:78-80) | |

### 2.4 Content area

Below the tab strip the content starts 30 down (y 165) (1080). It holds, top to bottom: a heading (not on Landing), the entries (scrolling), and the Utilities fixed at the foot.

| Part | Value | Source |
|---|---|---|
| Heading | Bahnschrift 33 white, `Margin="28,-4,0,12"`: left x 106, top y 161. "Profile & system" (and "Discord", M6). None on Landing | 1148-1164 |
| Entries viewport | from under the heading to above the Utilities; scrolls vertically; no scrollbar | 1172-1180 |
| Entry width | 536 (the whole panel); the entry's own margins bring its fill to x 87..605 | 1183-1190, 63-65 |
| First entry, Landing | top of the entry's box y 183 (165 + 42 - 24) | 1196-1200 |
| First entry, Profile & system | about y 213 (heading 161 + about 39.6 + 12) | 1148, 1196 |
| Utilities | row at the foot: y 942..1029, from x 93. On the landing tab only: the other tabs have none (C/ExtensionCore.cs:1170), and their entries' viewport reaches to 28 above the panel's foot (the empty row's margins), y 1017 | 1081-1092 |

Measured: Landing's Home entry y 183..288 with its ring's outer edge 175..296, x 79..613; Profile & system's first ring 204..316.

Scrolling: when focus moves, the focused entry is brought into view by the least movement; focus on the first entry scrolls to the very top (so the heading's gap shows) (E/OverlayWindow.xaml.cs:199-220). The scroll is not animated (**NOT FOUND**). The focus ring of an entry at the viewport's edge is not clipped: the viewport is 24 larger than its content on the top and sides (1172, 1196). A focused entry draws over its neighbours (1201-1206).

### 2.5 An entry (the list item)

Every entry is, top to bottom: an optional divider, then the entry's box (61-65).

| Part | Value | Source |
|---|---|---|
| Divider (only on entries that have one above) | 1 high, `#33FFFFFF`, x 108..584, 14 above it and 26 below | 63 |
| Box | x 87..605 (`Margin="9,0,9,3"`): 518 wide, default 105 high, 3 below | 65 |
| Fill | radius 4; transparent at rest, `#26FFFFFF` focused | 66, 829-832 |
| Focus ring | 4 thick, `#FF2CA243`, standing 4 clear of the box: outer edge 8 outside the box, corner radius 12 (WPF's, on the stroke's centre line, as `home.md` 4.1). With the glow of `home.md` 4.3 at 1:1 | 420-425 |
| Icon | icon font 30 in a box 36 wide, 33 from the box's left (x 120..156), centred vertically | 67-68 |
| Text | from 112 in (x 199) to 16 short of the right (x 589), centred vertically as a stack | 296 |
| Title | Segoe UI 26 white; wraps to at most 2 lines (max height 68), then ellipsis | 296-298 |
| Second line (some kinds) | Segoe UI 26 `#B3FFFFFF`, one line, ellipsis | 299-303 |

Kinds in M2 scope and how they differ from that default:

| Kind | Icon | Height, below | Differences | Source |
|---|---|---|---|---|
| Home | U+E80F | 105, 3 | | 505-507 |
| My games & apps | U+E8F1 | 105, 3 | | 508-510 |
| Running game | cover | 96, 12 | Cover 90 x 90, 6 from the box's left, centred vertically (3 above and below), cropped to fill, no radius. Without a cover: a `#FF2CA243` square with icon U+E7FC at 36. Store badge 18 x 18 at the cover's bottom left, 6 in (as Home's badge). Second line "Running". Text stays at 112 | 74-95, 299-300, 491-527 |
| Recent game | cover | 96, 12 | As Running game without the second line | 491-527 |
| Profile | picture | 96, 3 | Picture 60 x 60 round, 18 from the left, over a `#FF4A4A4A` disc; text from 96 in; one line | 96-103, 528-536 |
| Switch profile (M5) | U+E748 | 75, 3 | Icon 26, 18 from the left; text from 69 in. Disabled while a game runs: icon and text at opacity 0.35, still focusable, A does nothing | 538-555 |
| Settings (M3) | U+E713 | 75, 3 | Icon 26, 18 from the left; text from 69 in | 557-564 |
| Power | U+E7E8 | 75, 3 | Same | 565-572 |

Text: "Home", "My games & apps", the game's name, the gamertag, "Switch profile", "Settings", "Power" (C/ExtensionCore.cs:1228-1243, 2554-2573).

Kinds of later milestones that appear in these lists (leave their rows out until then): the achievements section and recent achievements under Switch profile (M6; 154-164, 434-464, 881-891), Discord's kinds (M6; 663-694, 728-875).

### 2.6 Landing tab

Entries (C/ExtensionCore.cs:2554-2573, 15):

1. Home
2. My games & apps
3. Up to 4 games: the running game first (as Running game), then games played and not hidden, most recently played first (as Recent game). The first game has a divider above it.

Utilities: Notifications, Audio, Search, and the port's Downloads (section 16). Focus starts on Home.

With the layout above: Home y 183..288, My games & apps y 291..396, divider y 413, first game y 440..536, then every 108. With 4 games the last ends at y 860, clear of the Utilities' ring (y 934): the landing tab never scrolls at 1080.

While the Guide is open the list is rebuilt when a game starts, stops or the library changes; focus stays on the same position in the list (2539-2551).

### 2.7 Profile & system

Heading "Profile & system". Entries (C/ExtensionCore.cs:1228-1243):

1. Profile: the picture and the gamertag of the Profile signed in (profiles.md 3). Focusable, A does nothing.
2. Switch profile (M5).
3. Recent achievements (M6): with totals, a divider, the totals' line and the last three unlocks (built in #141, below).
4. Settings, divider above (M3).
5. Power, divider above.

Focus starts on the Profile entry. **There are no Utilities on this tab**: the row is the landing tab's alone (C/ExtensionCore.cs:1170; the comment at E/OverlayWindow.xaml:1081-1087; `data-guide-recent.jpg` shows none), so Down from Power does nothing.

Measured (old type): the first divider at y 403, x 108..583. With nothing between Switch profile and Settings (no achievements, as in M2) the second is at y 522; in the picture the achievements stand between them.

**The achievements (#141)**, only while there are totals (`Library::achievements.totals`), as the Bundle's `AchievementsSection` (E/OverlayWindow.xaml 434-464, 881-891):

- Under Switch profile: a divider (margin 30,14,30,26), then a line 34.58 tall: the trophy (26, in a 36-wide box), "`{score:N0} G`" 12 after it, and "`{n} achievements`" ("1 achievement") in the dim colour 24 after that; 12 under it the entries. The whole is 33 in.
- Each of the last three unlocks (`model::achievement::GUIDE_RECENT`), newest first, is an entry 96 tall with no divider of its own: the icon 60 square, 18 in, radius 4 (the grey `#4A4A4A` square with the trophy, `#B3FFFFFF` 26, while not fetched), and from 96 in the achievement's name over its game's name in grey. Focusable; A does nothing.
- `--log-focus`'s `guide:` line names the entry focus is on in Profile & system after its place, as the Discord tab's does: `ProfileAndSystem Entry(0) Profile`, `Entry(1) SwitchProfile`, `Entry(2) Achievement "No Detours"`, `Entry(6) Power`. The places move with the unlocks; tests go by the names.
- With no totals neither shows, and Settings follows Switch profile as in M2.

### 2.8 Utilities

A row at the panel's foot, left-aligned from x 93: each a square 87 x 87, 18 apart: x 93..180, 198..285, 303..390; y 942..1029 (1088-1096). Measured the same. The port's fourth, Downloads (section 16), follows at x 408..495: the row stays left-aligned, as the Bundle's `StackPanel` lays it, and is not centred.

| Part | Value | Source |
|---|---|---|
| Fill | radius 4; `#73000000` at rest, `#26FFFFFF` focused | 1097-1100, 1135-1140 |
| Icon | icon font 33, centred: Notifications U+E91C, Audio U+E767, Search U+E721 (M3); the port's Downloads U+E896, Home's Downloads tile's | 1101-1112 |
| Focus ring | as an entry's: 4 thick, 4 clear, radius 12, with glow; the focused square draws over its neighbours | 1118-1124, 1141-1145 |
| Badge (Notifications; the port's Downloads) | the count of notifications (of games in the Queue on Downloads): a pill at least 24 wide, 24 high, radius 12, `#FF2CA243`, padding 6 each side, at the square's top right 6 in from both edges; text Segoe UI 26 white. Not drawn at 0 | 1125-1133 |

A 26-px figure in a 24-high pill overflows it; that is what the source says since #229 (**UNVERIFIED** how WPF clips it; in the pictures, with 16-px type, it fits). Flag for the look review rather than guess.

---

## 3. Navigation

### 3.1 Buttons, with the Guide open and no dialog

C/ExtensionCore.cs:755-866 unless said. The left stick acts as the D-pad (E/PlayniteInput.cs:33-37).

| Button | Does | Sound |
|---|---|---|
| Up / Down | Moves focus along the entries. At the first entry Up does nothing; Down from the tab's last entry goes to the **first** Utility, on the landing tab, which alone has any; on the other tabs it does nothing. No wrap (2186-2251) | Navigate, only when focus moved |
| Up from a Utility | To the tab's last entry, whichever Utility it was | Navigate |
| Left / Right on a Utility | Along the Utilities; stops at the ends | Navigate when moved |
| Left / Right on an entry | Nothing (except Volume, section 5.3) | none |
| A | Selects (section 3.3) | Enter, or none if the item does nothing |
| B | Closes: an open device list, else the Flyout, else the Guide | none (section 8) |
| LB / RB | Previous / next tab. Stops at the ends, no wrap. Not while a Flyout or dialog is up (855-858, 1144-1156) | Navigate when it changed |
| Menu (Start) | On a Running game: its menu (section 6.3). Elsewhere nothing (2376-2400) | Prompt open |
| Guide | Tap: closes the Guide. Hold 0.7 s: Power (section 7.1) | |
| X, Y, View, stick clicks, triggers, right stick | Nothing (**NOT FOUND**) | |
| Keyboard Esc | Closes the Guide (E/OverlayWindow.xaml.cs:402-417) | |
| Mouse | A click outside the panels closes the Guide (431-455). Entries are not clickable | |

**Key repeat:** a held direction repeats after 700 ms, then every 80 ms (C/ExtensionCore.cs:84-85, 859-865, 2169-2173). Only directions repeat. (Home's repeat in the port is `input`'s; the Guide's figures are these.)

### 3.2 Focus memory

There is very little, on purpose:

- Opening the Guide always shows the landing tab with focus on Home (C/ExtensionCore.cs:1977-2008), except when a Toast was showing (section 9.5).
- Changing tab always focuses the new tab's first entry (1159-1179).
- Going down to the Utilities always lands on the first; coming up always lands on the last entry.
- Closing a Flyout returns focus to its Utility. Closing a device list returns focus to its dropdown.
- A dialog remembers nothing: closing it leaves focus where it was in the Guide.

### 3.3 A (select)

C/ExtensionCore.cs:2040-2154.

| On | Does |
|---|---|
| Home | Closes the Guide, brings the Dashboard to the front on Home (closing My games & apps, Settings, Search) |
| My games & apps | Closes the Guide, brings the Dashboard to the front on My games & apps (M3) |
| Running game | Closes the Guide, gives the game the foreground |
| Recent game | With nothing running: closes the Guide and launches it (Launch screen as from Home). With a game running, quitting or launching: the prompt of section 6.4 |
| Profile | Nothing, no sound |
| Switch profile | Closes the Guide at once, with no slide, and Who's playing today? asks, without the Boot video, whatever Settings says of asking (start-up.md 7). Disabled while a game runs: nothing, no sound |
| Settings | Closes the Guide, opens Settings (M3) |
| Power | Opens Power (section 6.2) |
| Notifications, Audio | Opens the Flyout |
| Search | Closes the Guide, opens Search (M3) |
| Downloads (the port's) | Closes the Guide, opens Manage Queue, as Search opens Search (section 16) |

### 3.4 What closes the Guide

- B with nothing else open; a tap of the Guide button; Esc; a click on the dim.
- Selecting anything that leaves it (above).
- The game being quit from the Guide stopping (section 7.4).
- Carrying out a Power option.

It does **not** close when the game exits on its own while the Guide is open: it stays, and closing it then lands on Home (C/ExtensionCore.cs:645-671).

### 3.5 Where the foreground goes on close

On B, the window that had the foreground when the Guide opened gets it back, on the button's release (so the game does not see the B press), or after 1 s if no release is seen (C/ExtensionCore.cs:77, 2160-2166). On a tap of the Guide button, at once (881-927).

### 3.6 Hints

Beside the panel, a column from (675, 168), each row 45 high (E/OverlayWindow.xaml:936-973):

| Part | Value |
|---|---|
| Glyph box | 27 x 27, centred vertically in the row: x 675..702, first row y 177..204 |
| Text | Segoe UI 26 white, 14 after the glyph (x 716) |

| Context | Hints, top to bottom | Source |
|---|---|---|
| Always, while the panel shows and no Flyout is open | Guide glyph (the Xbox mark, white, filling the box) "Hold for power options" | C/GuideState.cs:72-81 |
| Focus on an item with a menu (Running game; Discord shortcut in M6) | adds: Menu glyph (a circle stroked white 2 around icon U+E700 at 12) "More options" | C/GuideState.cs:704 |
| Flyout open; Power alone, with no panel | none | E/OverlayWindow.xaml:936-952 |
| A dialog over the panel | as without it, under the dialog's dim: only a Flyout and the panel's going hide them | E/OverlayWindow.xaml:936-952 |

There are no A/B hints in the Guide (**NOT FOUND**). Measured: glyph x 675..701, y 177..203; text from x 716.

---

## 4. Tabs in scope

Covered in 2.6 and 2.7. The tab order in M2 is Landing, Profile & system; LB on Landing and RB on Profile & system do nothing and make no sound.

---

## 5. Flyouts

A Flyout is a second panel opened from a Utility, over the first. All in E/OverlayWindow.xaml.

### 5.1 The panel

| Part | Value | Source |
|---|---|---|
| Rect | Width 536, `Margin="155,33,0,0"`: x 155..691, y 33 to the screen's foot (1080) | 1235-1238 |
| Corner radius | 12 top left and top right, 0 at the foot | 1238 |
| Background | `#FF1F1F1F`, flat | 1238 |
| Shadow | a soft shadow outside the panel only: black, blur 24, opacity 0.7, no offset | 1239-1245 |
| Under it | the main panel is dimmed by `#73000000` over its own rounded rect (not the whole screen) | 1218-1228 |
| Title | Bahnschrift 33 white, `Margin="28,36,0,24"`: left x 183, top y 69. "Notifications" / "Audio" | 1270; C/ExtensionCore.cs:1199-1221 |
| Entries | below the title (from about y 133), scrolling, as wide as the Flyout (entry boxes x 164..682); 24 of room under the last | 1318-1338 |
| Empty text | Segoe UI 26 `#B3FFFFFF` at x 183, under the title | 1275-1312 |

Open: the Flyout starts 48 to the left at opacity 0 and settles: offset to 0 over 120 ms cubic ease-out, opacity to 1 over 120 ms linear (1246-1267). Close: at once, **no animation** (NOT FOUND). Under Reduce motion: **UNVERIFIED** whether the open slide is dropped (it is a XAML storyboard with no check); a port drops the slide and keeps the fade, as `ui.slide` does.

Hints hide while a Flyout is open. LB/RB do nothing. Focus starts on the Flyout's first entry; if it has none, nothing is focused and only B does anything.

### 5.2 Notifications

Title "Notifications". Entries: every current notification, newest first (C/ExtensionCore.cs:1199-1210). Empty: "No new notifications".

| Part | Value | Source |
|---|---|---|
| Box | x 164..682, height to fit its text, at least 87; 9 below | 577-580 |
| Fill | `#14FFFFFF` at rest, `#26FFFFFF` focused, radius 4 | 581-583 |
| Icon | U+E91C at 26, 18 from the left, top-aligned 24 down | 584-586 |
| Text | Segoe UI 26 white, from 69 in to 24 short of the right, 18 above and below; wraps, at most 120 high (3 lines), then ellipsis | 587-590 |
| Focus ring | as any entry's | 420-425 |

Entries are focusable so the list can be scrolled; A does nothing and makes no sound. **There is no way to clear or dismiss a notification from the Guide (NOT FOUND).** The list updates live while open; no Toast shows for a notification that arrives while this Flyout is open (313-326).

### 5.3 Audio

Title "Audio". Entries in M2 (C/ExtensionCore.DiscordCall.cs:362-375):

1. Output device (dropdown)
2. Input device (dropdown)
3. Volume
4. Mute
5. (M6, in a Call: "Mute in Discord", "Deafen in Discord")
6. Now Playing, with a divider above; "No music app is running" when there is none (#129: docs/spec/now-playing.md. C/ExtensionCore.NowPlaying.cs:68-85; card 173-295, 699-723)

With no media session to show, the list ends, under the divider, with the line the Bundle shows when no music app runs: "No music app is running" (icon U+E8D6 and text both `#B3FFFFFF`, box 75 high), as in its picture of the Flyout. It takes focus, and A does nothing on it (C/ExtensionCore.cs:2045). With one, the Now Playing card is there in its place, 218 high, with its row of controls under it: now-playing.md says how it looks, what it shows and how focus moves on it.

**Dropdown entry** (Output: icon U+E7F5, title "Output"; Input: icon U+E720, title "Input", as the Bundle's picture has them) (310-323, 617-645):

| Part | Value |
|---|---|
| Box | 128 high, 3 below |
| Icon | 26, 18 from the left, top-aligned 22 down |
| Text column | from 69 in to 24 short of the right; title one line; under it, 10 down, the drop box |
| Drop box | 48 high, radius 4, border 2 `#80FFFFFF`, fill `#14FFFFFF`; the default device's name (or "None") Segoe UI 26 white, 14 in, ellipsis 46 short of the right; chevron U+E70D at 16, 16 from the right (U+E70E while open) |
| Focused | the entry's fill and ring as usual |
| Open | the entry's fill stays `#26FFFFFF`; the entry's ring goes; the drop box gets its own ring: 2 thick `#FF2CA243`, radius 4, in place of the grey border, with the glow set 6.5 outside |

**Device list** (open dropdown) (472-488, 646-661): a box hanging from the drop box, overlaying the entries below without moving them: from 78 in to 33 short of the right of the Flyout (x 233..658), its top 8 above the entry's foot; fill `#FF2E2E2E`, border 1 `#33FFFFFF`, radius 4, padding 6 top and bottom. Rows:

| Part | Value |
|---|---|
| Row | 60 high, 6 in from each side, 2 below |
| Tick | U+E73E at 20, 14 from the left, only on the default device |
| Name | Segoe UI 26 white, from 52 in to 16 short of the right, one line, ellipsis |
| Focused | fill `#26FFFFFF` radius 4, the usual ring |

Opening it (A on the dropdown) focuses the default device. Up/Down move within it and stop at its ends; Left/Right do nothing. A makes that device Windows' default (for output: playback; for input: recording), closes the list and returns to the dropdown. B closes it without change. The list and the selection follow devices coming and going while open (C/ExtensionCore.AudioDevices.cs). A dropdown with no devices shows "None"; A on it does nothing, silently.

**Volume** (icon U+E767, title "Volume") (325-336, 376-377, 591-600): box 96 high; icon 26, 18 from the left; text column from 69 in. Under the title, 12 down: a bar 6 high, radius 3, track `#40FFFFFF`, filled `#FF2CA243` from the left to the level, ending 56 short of the column's right; the level as a number 0..100, Segoe UI 26 white, right-aligned 24 from the box's right, level with the bar. Left/Right change the system's master volume by 2, within 0..100, with the Navigate sound only when it changed (C/ExtensionCore.cs:97, 2257-2268); a held direction repeats. A does nothing.

**Mute** (icon U+E74F, title "Mute") (379-382, 601-609, 824-828): box 75 high; icon 26, 18 from the left; text from 69 in. A switch 48 x 24 at the right, 24 in: off = track radius 12, border 2 white, no fill, knob 12 x 12 white 6 from the left; on = track filled and bordered `#FF2CA243`, knob at the right. A toggles the system's master mute (Enter sound). No animation of the knob (**NOT FOUND**).

Volume and mute are read from the system when the Flyout opens and follow changes made elsewhere while open (**UNVERIFIED** for the port: the Bundle listens to the device's volume callback).

---

## 6. Dialogs over the Guide

A dialog takes all input. Under it: the `#B3000000` dim over the whole screen and the blurred panel (1.5).

### 6.1 Menu panel (menus and prompts)

E/MenuPanel.xaml. Centred on the screen, 450 wide, as tall as its content.

| Part | Value | Source |
|---|---|---|
| Panel | `#FF2B2B2B`, radius 6, soft shadow black blur 24 opacity 0.5 | 36-38 |
| Title | Bahnschrift 33 white, `Margin="28,36,28,24"`; one line with ellipsis in a menu, wraps in a prompt | 41-54 |
| Message (prompts) | Segoe UI 26 `#B3FFFFFF`, wraps, sides 28, pulled 14 up toward the title, 20 below | 56-57 |
| Option row | 72 high, the panel's width | 71-74 |
| Row, focused | fill `#FF404040`, radius 2, the row's width, 56 high (8 clear above and below); ring 4 thick `#FF2CA243` radius 7, the row's whole 72 high and standing 8 outside the panel left and right, so 8 outside the fill all round, with glow | 75-90 |
| Row content | from 32 in: icon in a box 24 wide (icon font 22, white), label Segoe UI 26 white 32 after it | 76-84 |
| Icons | Quit U+E711, Cancel U+E72B, Play U+E768 (others belong to the Tile menu, M3) | 97-156 |

Measured (Tile menu picture): panel x 735..1185, fill `#2b2b2b`, focused row `#404040`, ring x 727..1193. `tile-menu.md` sections 2 and 5 have the panel and its rows to the pixel; the port draws both from the one `screens::menu_panel`.

Navigation: Up/Down along the options, stopping at the ends; A chooses; B closes (declines). No animation in or out (**NOT FOUND**). Sounds: opening = Prompt open; choosing in a menu = Enter; answering or closing a prompt = Prompt close; B on a menu = Prompt close (C/Dialog.cs:72; C/ExtensionCore.cs:2406-2416).

### 6.2 Power

E/WideDialogPanel.xaml. A band across the whole screen, y 134..942 (808 high), `#FF2D2D2D` (36).

| Part | Value | Source |
|---|---|---|
| Contents | from x 132 to x 1691 (229 short of the right), starting 78 down (y 212) | 40 |
| Title | "What do you want to do?" Bahnschrift 54 white | 42 |
| Tiles | 79 under the title; a grid of 3 columns over the contents' width (each column about 519.7); each tile fills its cell less 16 at the right and 18 below: about 503.7 x 129 | 43-48 |
| Tile | fill radius 2, `#FF3B3B3B`, focused `#FF4A4A4A`; icon font 30 in a box 36 wide, 48 from the left; label Segoe UI 26 white 42 after it | 49-84 |
| Tile icons, in order | "Turn off PC" U+E7E8, "Restart PC" U+E72C, "Sleep" U+E708, "Exit Playnite" U+F3B1 (so 3 in the first row, 1 in the second) | 66-84; C/ExtensionCore.cs:1086-1128 |
| Close | a button 312 x 104, 80 under the tiles, 6 left of the contents' edge (x 126); `#FF3B3B3B` / focused `#FF4A4A4A`, radius 2; "CLOSE" Segoe UI 26 bold, white, centred | 89-104 |
| Focus ring | 4 thick `#FF2CA243`, 4 clear, radius 10, with glow | 25-32, 61-62, 115-116 |

Opening (E/WideDialogPanel.xaml.cs:21-74): the band is there at once; its contents start 200 to the left at opacity 0 and, 40 ms after the first frame, settle: offset to 0 over 120 ms cubic ease-out, opacity to 1 over 120 ms linear. Reduce motion: at once. Closing: at once.

Navigation (C/Dialog.cs:17, 39, 104-137): focus starts on **Close**. Left/Right within a row of tiles, stopping at the ends. Up to the tile above; from the first row, nothing. Down to the tile below; if the next row has none below, to its last tile; from the last row, to Close. Up from Close goes to the last row's first tile; Left, Right and Down on Close do nothing. A carries the option out; B or A on Close closes Power (and the Overlay, if the panel was not showing). Closed from its entry, focus is on the Power entry still. LB, RB and Menu do nothing while it is up; a tap of the Guide button closes the Guide and Power with it, in silence. A hold while Power is up puts it up anew, focus on Close, without its contents coming in again, and takes the panel away if it was showing.

Carried out after the Guide has closed (E/PlayniteHost.cs:384-428): Turn off = `shutdown.exe /s /hybrid /t 0`; Restart = `shutdown.exe -r -t 0`; Sleep = `SetSuspendState(false, true, false)`; Exit = quit the program.

**The hold has no visual (NOT FOUND).** Holding the Guide button shows nothing for 0.7 s, then Power appears. There is no ring, bar or countdown; the pad's own buzz is the pad's (C/ExtensionCore.cs:875-880).

### 6.3 A running game's menu

Menu on the Running game entry: a menu panel titled with the game's name, one option "Quit" (icon U+E711) (C/ExtensionCore.cs:2376-2400). B closes it.

### 6.4 Prompts

| Prompt | Title | Message | Options (focus on the first) | Source (C/ExtensionCore.cs) |
|---|---|---|---|---|
| Starting a game while another runs | "Quit {X} and start {Y}?" | none | "Quit and start", "Cancel" | 717-746 |
| A quit that did not finish in 10 s | "Force quit {X}?" | "It hasn't closed. Anything unsaved will be lost." | "Force quit", "Cancel" | 693-710, 2425-2476 |

A prompt that comes up with the Guide closed opens the Overlay first, with the panel (2406-2416). A prompt whose game stops by itself closes silently.

---

## 7. Behaviour

### 7.1 The Guide button

C/ExtensionCore.cs:80-83, 881-927, 1060-1080.

| Press | Guide closed | Guide open |
|---|---|---|
| Tap (released before 700 ms) | The Guide opens **300 ms after the press** (at once on release if that is later than 300 ms). Sound: Open menu | The Guide closes at once on release, the foreground returning to where it was |
| Hold (700 ms, judged by timestamps, the button still down) | Power opens on its own: the dim and the band, no panel | The panel slides out and Power opens |

- The 300 ms wait exists so a hold does not flash the Guide first. A tap shorter than 300 ms therefore still waits until 300 ms after the press.
- With the "Guide" setting off the button does nothing, a hold too (C/SettingsState.cs:155-171).
- During Start-up and while waking from the Idle screen the press is swallowed (757-768) (M5).

**How the Bundle reads it** (`docs/research/guide-button-capture.md` in the theme repo): Playnite's controller events, which come from SDL polling `XInputGetStateEx` (ordinal 100, bit 0x0400) about every 16 ms. The press cannot be consumed: whatever else listens (Game Bar, Steam) also sees it, so Game Bar's binding should be off. The port already reads the same call in `crates/input`; the tap/hold machine above is what M2 adds.

### 7.2 Over a game and over Home

The Guide is the same in both; what differs is what is behind it and what the landing tab lists.

On opening (C/ExtensionCore.cs:1977-2008):

1. The foreground window is recorded.
2. The clock is refreshed now, the pad's battery 500 ms later.
3. All Toasts, showing and waiting, are dropped.
4. The Overlay is shown over the monitor of the foreground window, topmost, and **takes the foreground** so the game stops receiving the pad (an Alt tap, then `SetForegroundWindow`; E/Win32WindowSystem.cs:61-86).
5. The running game is **frozen**: every process under its install folder, and the process owning its window, is suspended (`NtSuspendProcess`; E/GameFreezer.cs:11-21; E/NativeMethods.cs:95-96). Not while it is being quit. A record of what was frozen is written so a crash cannot leave a game frozen: leftovers are thawed at the next start.

On closing: the game is thawed, the foreground handed back (3.5), the Overlay hidden after the slide-out.

Over a game the game's last frame stays on screen, dimmed. Over Home, Home is behind the dim and keeps drawing.

Limits the Bundle accepts (`docs/research/topmost-overlay.md`): windowed, borderless and "fullscreen optimised" games work. A game in true exclusive fullscreen is not covered (it may minimise or the Overlay may not show), and a game running elevated cannot be covered, given focus back or frozen by an unelevated program.

A game that starts while the Guide is open is frozen at once and appears as Running (552-556).

**The freeze in full** (E/GameFreezer.cs; C/ExtensionCore.cs:2014-2034, 693-710, 2576-2587; CT/GuideFreezeTests.cs):

- **No setting.** The Bundle has no switch for the freeze: it comes with the Guide, whose own setting (`IsGuideOn`, on by default, C/SettingsState.cs:146-171) is the only one. The port is the same: `guide.enabled`, on unless set, and no other.
- **Which processes.** Every process whose program is under the game's install folder (the folder as given and as it is past junctions and `subst` drives), and the process that owns the game's window. Never the Bundle's own process, a process of Windows' (a path under the Windows folder: Explorer, or a host owning a store game's window), or one whose path can't be read. A game installed at a drive's root is not frozen at all.
- **How.** `NtSuspendProcess` on a handle opened with `PROCESS_SUSPEND_RESUME | PROCESS_QUERY_LIMITED_INFORMATION`; `NtResumeProcess` to thaw. A process Windows won't open or suspend, or one that has ended, is skipped and logged.
- **Order.** Opening: the Overlay takes the foreground, then the game is frozen. Closing: the game is thawed, then handed the foreground, when the button that closed the Guide is let go (3.5).
- **Not frozen:** a game being quit (it is thawed before `WM_CLOSE` and stays thawed until it has gone), a game with no install folder, and nothing at all with no game running. A game that exits under the Guide is let go of. The program leaving thaws the game.
- **The record.** Before anything is suspended, `frozen-game.txt` in the extension's data folder is written: a line a process, `<id> <start time as a FILETIME> <name>`. It is deleted once all are resumed. What it still lists at the next start is resumed then, a process being touched only if it has that id and that start time. Without the record nothing is frozen.

**In the port:** `shell::Freezer`, the same in each of these, the record in the Dashboard's data folder under the same name and in the same form. The order is `app`'s (`crates/app/src/window.rs`).

### 7.3 Launching from the Guide

A on a Recent game with nothing running: the Guide closes, the recorded window gets the foreground back, then the game launches exactly as from Home (Launch screen, `home.md`). With a game running: the prompt "Quit {X} and start {Y}?"; on "Quit and start", X is quit (7.4), and when it has stopped the Guide closes and Y launches (2479-2493). If X does not stop, the Force quit prompt follows.

### 7.4 Quitting a game

From the running game's menu, "Quit" (C/ExtensionCore.cs:693-710, 2425-2476). **There is no confirmation before a graceful quit.**

1. The game is thawed and its window sent `WM_CLOSE`.
2. The Guide stays open. The Running game entry stays until the game stops.
3. When the game stops: the Guide closes 500 ms later, onto Home (645-671, 75).
4. If it has not stopped after 10 s: the prompt "Force quit {X}?". "Force quit" kills its processes and waits another 10 s; "Cancel" leaves it running.

### 7.5 Dialogs with the Guide off or closed

With the Guide setting off, a prompt shows on the Overlay by itself: dim and menu panel, no panel (C/SettingsState.cs:155-171).

---

## 8. Sounds

By the Bundle's file names (E/PlayniteSounds.cs:23-34); in the port they are asked for by name and come from the media folder (#10, #28), silent without it.

| Action | Sound | Source |
|---|---|---|
| Guide opens (tap) | `OpenMenu.wav` | C/ExtensionCore.cs:968 |
| Guide closes, by any means | none | CT/SoundTests.cs:41, 52 |
| Focus moves (entries, Utilities, device list, dialog options) | `Navigate1.wav`..`Navigate4.wav` in rotation, as Home | C/ExtensionCore.cs:2186-2251 |
| A direction at an end, or on something that does not move | none | |
| Tab changes | Navigate | 1144-1156 |
| Volume changes by a step | Navigate | 2257-2268 |
| A on something that acts (including opening a Flyout, a device list, toggling Mute, choosing a device) | `Enter.wav` | 2040-2106 |
| A on something inert | none | |
| B closing a Flyout or a device list | none | 755-866 |
| A menu, a prompt or Power opens | `PromptOpen.wav` | 2406-2416 |
| A menu option chosen; a Power option chosen | `Enter.wav` | C/Dialog.cs:72 |
| A prompt answered; any dialog closed with B | `PromptClose.wav` | C/Dialog.cs:72 |
| Close in Power, with A | `Enter.wav`: Power is not a prompt, and A on any of its options, Close among them, is `ChooseSound` | C/Dialog.cs:72 |
| A on the Power entry | `Enter.wav`, then `PromptOpen.wav` | C/ExtensionCore.cs:2040-2106, 2406-2416 |
| A Toast shows | `Notification.wav` | C/ExtensionCore.cs:313-423 |
| A Toast goes | none | |
| The program exits from Power | `PowerOff.wav`, played to its end before exit | E/XboxSeriesPlugin.cs:406-410 |

Volume: the Bundle uses Playnite's interface volume; the port uses its own sound setting (#28). The hold for Power makes no sound of its own before Prompt open.

---

## 9. Toasts

E/ToastWindow.xaml, E/ToastWindow.xaml.cs, C/ToastState.cs.

### 9.1 The window

A separate window from the Overlay: topmost, never activated, click-through (`WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`), covering only the bottom 240 of the 1080 (y 840..1080) of the monitor holding the foreground window, shown without taking focus (E/ToastWindow.xaml:6-17; E/ToastWindow.xaml.cs:26, 79-87, 233-246). It never takes the pad from a game and never freezes one. Z-order: Toast above the Idle dim above the Overlay and the Launch screen (E/XboxSeriesPlugin.cs:228-233).

**In the port (#42).** Two windows, as the Bundle (open question 7): `shell::ToastStrip` is a second window made as the Overlay is (no redirection bitmap, a composition swap chain, premultiplied alpha), always passive, along the foot of the monitor of the window that has the foreground when it is shown, as the Bundle's is and as the Overlay is (open question 9). It is shown only while a Toast is on screen. Its frame is laid out as the whole screen's and shows the foot of it (`Host::draw_part`), so a Toast is placed by the same numbers wherever it is drawn. The Overlay comes to be above it whenever the Overlay is shown, raised or given the foreground, so each turn of the loop the strip looks for a window of the Dashboard's above it and, finding one, goes back on top. Where the strip can't be made (and anywhere but Windows) Toasts are drawn in Home's window, and so are not seen over a game.

### 9.2 The banner

One banner, centred horizontally, its foot 64 above the screen's (E/ToastWindow.xaml:74-154):

| Part | Value | Source |
|---|---|---|
| Card | `#FF107C10` (the console's green; not the accent), radius 6, no shadow, no border. As wide as its content, at most 640. Height 128: y 888..1016 | 74-82; T/Constants.xaml:224 |
| Padding | 16 left, top and bottom; 32 right | 83 |
| Tile | 96 x 96 at the left, 24 to the text. Without an image of its own: `#26FFFFFF` square radius 4 with the kind's icon (icon font 44, white) centred. With one: the image fills the tile, cropped, radius 4 | 84-154 |
| Line | Segoe UI 34 white, one line, ellipsis, centred vertically | 168-180 |
| Detail (Library updated only) | Segoe UI 26 under the line | 181-186 |
| Prompt | "Press", the Xbox mark 32 x 32 (12 either side, 2 down), "to view"; Segoe UI 34 white | 187-214 |

**Against the Bundle's picture** (`port-reference-20261115-200355/60-toast-greeting.jpg`, the greeting over Home): the card is x 756..1164, y 888..1016. It is 408 wide for a line about 140 wide: the card is as wide as the longer of the line and the prompt even for a Toast whose prompt never shows, since the prompt is laid out, unseen, in the line's place. The greeting's tile is the profile's picture, round and 80 across, bare on the card with no lighter square (the square is only under a kind's own icon); the port draws the same stand-in picture Home's top bar has until Profiles have pictures (M5). The Guide button's mark in the prompt is the plain disc the Guide's hint has (open question 5).

**Text cycle.** When the Guide is on, the banner alternates between its line and the prompt, in place: line for 2.2 s, a 0.3 s linear cross-fade, prompt for 2.2 s, 0.3 s back; a 5 s loop, so a Toast shows each once (48-63). With the Guide off there is only the line.

### 9.3 Animation

E/ToastWindow.xaml.cs:29-58, 249-335.

| | What | Time | Easing |
|---|---|---|---|
| In | scale 0.3 to 1 about the card's centre, opacity 0 to 1 | 300 ms | cubic ease-out |
| In, sliding kinds (Screenshot; Greeting, M5) | the tile and the text start 120 to the left (tile at opacity 0) and, 1000 ms in, slide to place over 450 ms | | cubic ease-out |
| Out | scale 1 to 1.1 over 120 ms (cubic ease-out), then to 0.9 over 180 ms (ease-in-out); opacity to 0 over 200 ms starting at 210 ms | 410 ms | |
| Reduce motion | no scale, no slide; the fades only | | |

A sliding kind's text starts where its tile will be, at the card's left padding, and the card is cut to its bounds so the tile comes out from behind its left edge. The text cycle runs on from the Toast's start through its going, so a Toast that has had its five seconds goes on its line. In the port the banner is where its age puts it (`screens::toasts::banner`): nothing is started or stopped, and a frame at rest (a screenshot) shows it in, in place and on its line.

### 9.4 Queue and timing

C/ExtensionCore.cs:100-101, 313-423.

- One Toast at a time. Each shows for **5 s** (counted from when it starts appearing), then goes; **0.5 s** later the next shows.
- `Notification.wav` plays as each shows.
- A notification that already existed at start-up makes no Toast. One that arrives while the Notifications Flyout is open makes none.
- **Opening the Guide drops the Toast showing and all waiting** (1977-2008). Toasts are dropped only at that moment: a notification arriving while the Guide is open, with the Notifications Flyout closed, is queued as usual and shows over the Guide, in the Toast window above the Overlay (from the code at 313-337 and the z-order; **UNVERIFIED** by picture).
- Over a fullscreen game: the same, with the limits of 7.2 (nothing shows over true exclusive fullscreen).

Decided for the port (#42):

- **The greeting is in M2**, since the Bundle shows it as it starts (`60-toast-greeting.jpg`): "Hi <first word of the user's name>!", asked for once the Toast strip has been made, which is when Home has come to rest after its first frame. Until there is a Start-up (M5) that is the moment a profile "signs in". It is the queue's first Toast like any other: notifications posted meanwhile wait behind it and follow it half a second after it goes. It is not a notification and is not listed.
- **A Toast that goes with none waiting leaves no gap**, as in the Bundle: one posted a moment later shows at once, over the going of the last.
- **The queue goes by what is on the screen, not by what it has asked for.** Whoever shows the Toasts says when their picture has been presented (`Dashboard::toasts_presented`, after each frame and after the strip is hidden). A Toast's five seconds begin when its first frame is presented. Its going begins with the first presented frame of its going, once its time is seen to be up, so it is seen to go however late that was. The next does not show until the last is gone and a frame without it has been presented (or the strip hidden), and no sooner than half a second after the last began to go. The time told with a presented frame is the time then, not when that turn of the loop began: the turn that makes the Overlay and the strip takes over a second. One step of time does one of these things to a Toast, so a loop that stood still (a game starting, a slow renderer) swallows none of them. This was found on the VM, where Home's first seconds take the software renderer most of a second a frame: the queue ran on the clock alone and a Toast "showed" and went that was never drawn.
- **On a slow renderer Home gives way to a Toast.** The strip is drawn first each turn of the loop, but a frame of Home, once begun, keeps the renderer for as long as it takes: most of a second with the swirl moving where the renderer is the processor. So where the renderer is slow (the adapter is the processor, or Home's last frame took more than 50 ms, the Overlay's test) and a Toast is on screen or coming:
  - Home's swirl holds still, as it does under the Guide, so Home has nothing to draw but what the player does;
  - a frame of Home that is wanted (a press, the clock, art coming in) is not begun while the Toast is moving, nor when the Toast would move or the queue change before that frame was done (1.25 times what Home's last frame took). It is begun in the Toast's next rest, and is never kept more than 1.5 s after Home's last frame.

  On a GPU nothing changes: the swirl moves under a Toast and Home is drawn when it asks.
- **The queue is timed by time the Dashboard is told has passed** (`Dashboard::advance`), not by a clock of its own, so tests step it. `Toasts::wake` says when it next changes, which the window's loop sleeps until.
- **Where Toasts have no strip** they are in Home's frame, which says it was presented. Behind a game Home presents nothing: Toasts then pass unseen, in their time.

### 9.5 The Guide button while a Toast shows

A tap opens the Guide on what the Toast was about (C/ExtensionCore.cs:953-983). The press records which Toast was showing, since opening drops it.

In the port the Guide button says nothing as it goes down: a tap is told 300 ms after the press (7.1). So the Toast a tap opens the Guide on is the one **on screen** when the tap is told, one that began to go meanwhile included (its going takes 410 ms, longer than the wait). Until the Notifications Flyout exists (#41) a notification's tap opens the Guide with focus on the Notifications Utility; `Dashboard::open_notifications` is where the Flyout opens instead. Opening the Guide any other way (the profile on Home) drops the Toasts and opens it as ever.

| Toast | Tap opens |
|---|---|
| Notification, Screenshot | the Guide with the Notifications Flyout open |
| Greeting (M2 in the port), Achievement (M6) | Profile & system |
| Call (M6) | the Discord tab |
| Queued (M4) | Manage Queue (not the Guide), in front of a running game too; the other Queued Toasts waiting are let go; the Activate sound |
| Installed (M4) | starts the game ("Press ... to start"); the Activate sound |
| Uninstalled (M4) | the Guide |

In the port these are `Dashboard::tell(kind, game)`, from the Queue's thread (`library::queue::Tell`, #132): Queued as the Dashboard asks a store to install a game, and as a game joins the Queue from its store's client while the Dashboard runs (not for those already in it as it starts); Installed as a game the Queue followed is installed; Uninstalled once a game the Dashboard asked to uninstall has really gone, never on a give-up. The `queue` payload checks each by its `toasts: shown "<line>" (<Kind>)` line, and both taps.

### 9.6 Kinds

| Kind | Tile | Milestone | Source |
|---|---|---|---|
| Notification | icon U+E91C | **M2** | E/ToastImages.xaml |
| Screenshot (a notification whose id or text mentions "screenshot") | icon U+E722; slides | Dropped (#42): the port has no such notification | C/Notification.cs:18 |
| Queued / Installed / Uninstalled | "Queued", "Ready to play", "<Name> was uninstalled". The game's square art, else its cover, cut from its middle to the tile's square, radius 4, bare on the card; until it is decoded, or with none, U+E896 / U+E768 / U+E74D on the lighter square. None slides | **M4** (#132) | C/ExtensionCore.Queue.cs |
| Library updated | U+E895, with a detail line | M4 or drop | |
| Greeting | round picture 80 x 80; slides | **M2** (#42); the picture itself M5 | |
| Call | the Discord mark at 48 | M6 | |
| Achievement | its own pill (E/AchievementToast.xaml), 8.5 s; its rare form | **M6** (#142): [achievement-toast.md](achievement-toast.md) | |

### 9.7 Notifications in the port

Decided in #42. The Dashboard core keeps the notifications itself (`screens::toasts`):

- `Dashboard::notify(text) -> id` adds one to the list and shows its Toast (not while the Notifications Flyout is open, once there is one: `Dashboard::notifications_open` is the hook). `Dashboard::notifications()` is the list, oldest first, each an `id` (1 for the first, counting up) and its `text`. There is no time: the Flyout (5.2) shows none. The list lives as long as the Dashboard runs; nothing clears it, as nothing does in the Bundle. The Notifications Utility's badge counts it.
- **Where they come from.** Nothing in the Dashboard posts one yet (open question 1 stays open for what should). Two ways in exist, for tests: `dashboard --toast <text>` (repeatable) posts as the Dashboard starts, behind the greeting; and another program posts to a running Dashboard with a `WM_COPYDATA` message to the Toast strip's window (`model::post`: the window's class, the message's id, the text as UTF-8, one line of 200 characters at most), which is what `standin toast <text>` does.
- **Posts from other programs are off unless asked for.** The strip's window takes such a message only in a Dashboard started with `--allow-toast-posts`, which the tests pass. Started any other way, as it is on the TV, it answers that it took nothing and nothing is listed or shown: no other program can put text on the screen through the Dashboard. `e2e/payloads/toast` checks both.
- The sound is `Sound::Notification` (`Notification.wav`), asked for as each Toast begins to show, the greeting's too.

---

## 10. Playnite-specific things

| In the Bundle | In the port |
|---|---|
| Playnite's notification list feeds the Notifications Flyout and Toasts | A notification list of our own in the Dashboard core: an id and a text (9.7). What posts to it in M2 is an open question (1) |
| Playnite's controller events for the Guide button | Our own `XInputGetStateEx` polling in `input` (exists); add the tap/hold timing |
| "Exit Playnite" in Power | "Exit to desktop": the Dashboard itself exits (section 13) |
| Playnite's interface volume for sounds | Our own setting (#28) |
| `BringToFront(page)` to show Home, My games & apps, Settings, Search | The main window to the front and the page asked of the core; pages that do not exist yet: open question (2) |
| A game known by Playnite's id and install folder; its window found by install folder | The library's id and what launch tracking already knows (process, window) |
| Playnite's "Guide button focuses Playnite" setting, turned off by the Bundle | Drop |
| The Guide on/off setting, Reduce motion, show battery percent | Config values now, Settings in M3 |
| Library updated Toast; "Update after downloads" | M4 or drop |
| PlayniteAchievements for the achievements section | M6 (#7) |
| Gamertag and picture from the Bundle's Profiles | From the port's Profiles (profiles.md), as Home's top bar |
| Prewarming the window offscreen; pictures cached as bitmaps; the close deadline timer; the blurred picture of the panel; the shadow as a nine-patch | Drop: WPF workarounds. Draw live |
| Debug-only mouse clicks on entries | Drop. Keep Esc and the keyboard stand-in for the Guide button |
| The Xbox mark, Discord mark and store marks as vector paths in the XAML | Not ours to ship: as `home.md`, drawn at runtime from what Windows has or a plain stand-in (open question 5) |

---

## 11. Open questions

1. **What makes a notification in M2.** Without Playnite nothing posts any. Candidates: a library read finishing with changes, a launch failing, a force quit. Without a source the Flyout always says "No new notifications" and no Toast ever shows outside tests.
2. **Entries whose pages do not exist yet.** My games & apps, Search (M3), Settings (M3), Switch profile (M5). Draw them and make A inert, or leave them out until their page exists? Leaving out changes the layout the screenshots are compared with.
3. **Freezing the game.** **Decided (#38): kept, as the Bundle has it**, with no setting of its own, since the Bundle has none (7.2). A game an anti-cheat guards refuses to be opened and is left running, which is said in the log.
4. **Exclusive fullscreen and elevated games.** **Decided (#38): the Bundle's limits are accepted** and said in the log when they are met: `overlay: Windows says a Direct3D program has the screen to itself (exclusive fullscreen)` as the Guide opens over such a game, `overlay: Windows did not give the Overlay the foreground` over a game that runs elevated, and `freeze: Windows won't let <name> (<id>) be frozen` for a process it can't suspend. Nothing is done to work around either.
5. **The Xbox mark** on the landing tab, the Guide hint and the Toast prompt, and the store badges: what is drawn in a repository that may go public?
6. ~~**Power.** The fourth option's name ("Exit", "Exit to desktop"); whether "Turn off PC" keeps the hybrid shutdown; whether Exit plays `PowerOff.wav` when the media folder has it.~~ Decided in #39: section 13.
7. **One window or two.** The Bundle has an Overlay window (activatable) and a Toast window (passive, a strip). The port's Overlay window has both modes (#27) but a Toast must stay passive while the Guide is active and must show while the Guide is closed. Two windows as the Bundle is the recommendation. **One Guide (#38):** the Guide is on the Overlay over Home as it is over a game, as in the Bundle; only where no Overlay can be made is it drawn in Home's window. **Two windows (#42):** the Toast has its own, as the Bundle (9.1).
8. **Layered window not updating on the VM** (#23) blocks checking Toasts there if the passive window is the layered kind. The strip is made as the Overlay is, whose picture does show on the VM; `e2e/payloads/toast` checks the Toast by the screen's pixels.
9. **Which monitor.** The Bundle covers the monitor of the foreground window. **Decided (#38): the same.** Each time the Overlay is shown it goes over the monitor of the window that has the foreground then (`overlay: over the monitor at (x, y), WxH`), the Launch screen's showing included, as the Bundle's Launch screen does. Home's own window stays on the primary monitor.
10. **Game Bar and Steam** also react to the Guide button. Document turning them off, or detect and warn?
11. **The 300 ms wait before a tap opens the Guide.** Kept as is? It is the price of the hold not flashing the Guide.
12. ~~**Blur behind dialogs.** `render` has no blur of a layer yet; is unblurred-under-dim acceptable for M2?~~ `render` has it (#39): section 13.
13. **Font metrics.** Line heights are assumed (2.1). The first screenshot test on Windows should be compared with the Bundle's pictures at the entries' text baselines.
14. **The badge's 26-px figure in a 24-high pill** (2.8): confirm on the TV PC how the Bundle draws it today.
15. **A Toast over the open Guide** (9.4): confirm on the Bundle. The port does as the Bundle's code reads, and `e2e/payloads/toast` checks it.

---

## 12. Proposed tickets for M2

In order. Each can be tested on its own; 1 to 3 need nothing of each other.

1. **The Guide button: tap and hold.** (parallel with 2, 3)
   - In `input`/the core: press, release and time become "tap" (opens 300 ms after the press, closes on release) and "hold 700 ms".
   - Direction repeat for the Guide: 700 ms, then 80 ms.
   - The Guide setting off: the button does nothing.
   - Tested with a fake clock, no window.

2. **The Guide's panel, offscreen.** (parallel with 1, 3)
   - Replace `crates/screens/src/guide.rs`: dim, top shade, panel, tab strip, landing tab, entries of every M2 kind, hints, the stacked Status area, Utilities with the badge.
   - Navigation of section 3, focus rules of 3.2, Navigate/Enter sounds as effects.
   - Open/close and tab-change animation (1.3, 1.4), Reduce motion.
   - Screenshot tests against the measures here: landing with 0 and 4 games, focus on a Utility, a Running game.

3. **The Overlay window in guide mode.** (parallel with 1, 2)
   - The Guide drawn in the Overlay window (#27) over the foreground window's monitor; hidden when closed.
   - Taking the foreground, recording and handing it back on button release or after 1 s.
   - Freezing and thawing the running game, with the crash record (subject to open question 3).
   - Tested on the VM over a stand-in game: the game is dimmed, gets no pad input, and gets focus back.

4. **Profile & system, menus, prompts and Power.** (after 2; parallel with 5, 6)
   - The Profile & system tab.
   - The menu panel and the wide dialog (section 6), with the dialog dim and Prompt sounds.
   - Power on a hold and from its entry: turn off, restart, sleep, exit, carried out after the Guide closes.
   - Screenshot tests of each dialog; the power commands behind a trait so tests assert the request.

5. **Games from the Guide.** (after 2 and 3; parallel with 4, 6)
   - A on a Running game and a Recent game; the running game's menu and Quit.
   - The 10 s quit timeout and Force quit; "Quit X and start Y?"; the Guide closing 500 ms after a quit game stops.
   - A game starting or stopping while the Guide is open.
   - Behaviour tests in the core; one end-to-end run on the VM.

6. **Utilities and Flyouts: Audio and Notifications.** (after 2; parallel with 4, 5)
   - The Flyout panel, its dim over the main panel, its open animation.
   - `shell`: output and input devices, default device, master volume and mute (Core Audio), following changes.
   - Audio: the two dropdowns with their lists, Volume, Mute.
   - The notification list in `services` and the Notifications Flyout with its empty state.
   - Screenshot tests with a fake device list.

7. **Toasts.** (after 6 for the notification list; the window part parallel with 4 to 6)
   - The passive Toast window, bottom strip, above the Overlay.
   - The banner, the text cycle, in and out animation, Reduce motion.
   - The queue: 5 s, 0.5 s gap, dropped when the Guide opens, none while the Notifications Flyout is open.
   - A tap of the Guide button while a Toast shows opens Notifications.
   - Screenshot tests of the banner; queue tested with a fake clock.

8. **M2 on the TV PC.**
   - An end-to-end run on the VM: Guide over a game, Flyouts, Power (cancelled), a Toast over a game.
   - Side-by-side pictures with the Bundle for the views it has pictures of; new Bundle pictures of a Flyout, a Toast and Power to close the gaps named at the top.
   - The M2 report, the port's `CONTEXT.md` entries for Guide, Overlay, Status area, Utility, Flyout, Toast, Guide button.

---

## 13. Where the Dashboard's dialogs differ, and what was decided (#39)

What `crates/screens/src/guide/`, `menu_panel.rs` and `wide_dialog.rs` do that the sections above leave open, or do differently.

- **Measured against the Bundle's pictures of Power** (`port-reference-20261115-215324/57`, `59`, `62`), pixel for pixel: the band y 134..941 at `#2d2d2d`; tiles at x 132..635, 652..1155, 1171..1675 and y 356..484, 503..631 at `#3b3b3b`; Close x 126..437, y 730..833 at `#4a4a4a` with focus, its ring `#2ca243` and the glow's tint inside its edge; the panel behind, whose top edge goes from black at y 24 to its dimmed grey at y 39 in both (the blur's 12 either side of y 33); the dimmed page under the band. Nothing was changed for them. What they cannot settle here is type: the tests draw with stand-in fonts, so the title's and labels' own shapes and baselines wait for a picture taken on Windows.
- **The blur is of a layer, not of the backdrop.** As the Bundle: the panel and what is on it are drawn to a texture of their own, 12 larger than the panel all round, blurred there (a Gaussian of deviation 4, which is WPF's radius 12) and laid over the dim; the dialog's `#B3000000` goes over that. Nothing beside the panel is blurred. `render::Group` is that texture: a layer drawn as one picture, blurred, faded as a whole, or both. The Tile menu's menu behind its flyout (opacity 0.45 as one group) and its fly-in are the same thing with no blur.
- **The menu panel's shadow** is drawn by the quad shader's soft edge, 36 wide (one and a half times WPF's `BlurRadius` of 24), which is what the picture of the Tile menu measures: about a quarter black at the panel's edge, a sixth 5 out, a twelfth 10 out, gone by 18.
- **Power's fourth option is "Exit to desktop"**, with the Bundle's glyph (U+F3B1). It exits the Dashboard; whatever game is running is left running.
- **"Turn off PC" keeps the hybrid shutdown** (`shutdown.exe /s /hybrid /t 0`), Restart is `shutdown.exe -r -t 0`, Sleep is `SetSuspendState(false, true, false)`: the Bundle's, unchanged. Nothing is asked first: the Bundle has no "are you sure".
- **Exit plays `PowerOff.wav` to its end** when the media folder has it, then exits; the wait is the clip's own length, 4 s at most, and none at all with no file, no device or no volume. The window stands still meanwhile, as the Bundle's does.
- **The machine's power is behind a trait** (`shell::power::Power`). The core only ever asks: `Effect::Power(TurnOff | Restart | Sleep)` and `Effect::Exit`. The one implementation that does anything (`SystemPower`, Windows only) is made in one place, the window's start, and not there with `--dry-power`, which logs `power: would run ...` instead. A test, a `--screenshot` or `--press` run never carries effects out at all.
- **Power's contents fade as one picture** while they come in (a group), and a second hold does not bring them in again.
- **A on Close in Power is `Enter.wav`**, as the Bundle's code has it; section 8 said `PromptClose.wav` before.
- **The hints stay beside the panel under a dialog**; section 3.6 said none before. With no panel (Power by a hold) there are none, and no top shade or Status area, at once.
- **A hold does nothing** with the Guide setting off, while a game is starting (the Dashboard takes no input then), or over a game with no Overlay to show Power on.
- **The running game's menu** (6.3) is here as the menu panel's first user: Menu on the Running game opens it, its one row Quit asks the shell to quit the game and leaves the Guide open. The prompts of 6.4, the 10 s timeout and the Guide closing after a quit are #40's: section 14.
- ~~**A game stopping closes the Guide, and any dialog with it**~~: no longer. Since #40 the Guide stays, and Power with it, as in the Bundle (section 14).
- **Not here:** a click on the dim.

---

## 14. Games from the Guide: what was built, and what was decided (#40)

From the Bundle's C# (`C/ExtensionCore.cs` 500-746, 2040-2154, 2376-2493, 2576-2632; `E/Win32WindowSystem.cs` 126-151; `CT/GameExitTests.cs`). **No picture of the Bundle shows a running game in the Guide**, a prompt over it, or Home in front of a game: the reference runs could not start one. The prompts are the menu panel of 6.1, which was measured against the Tile menu's pictures; everything else here is from source.

**What the core does** (`crates/screens/src/lib.rs`; `crates/screens/tests/guide_games.rs` has each line as a test):

- **A on the Running game**: the Guide closes and the game's own window gets the foreground, whatever the Guide opened over (`Effect::ShowGame`; the Bundle's `ReturnToRunningGame`).
- **A on a Recent game** with nothing running launches it as from Home. **With a game running**: the prompt "Quit {X} and start {Y}?", "Quit and start" (U+E768) and "Cancel" (U+E72B), over the Guide, which stays. "Quit and start" quits X as Quit does and, when X has stopped, closes the Guide at once and launches Y. Cancel, B and the Guide button drop it. **While a game is quitting or starting** A on another game plays `Enter.wav` and does nothing, as the Bundle's does (its `Select` sounds before `PlayGame` refuses).
- **Quit**, from the running game's menu: no confirmation; the game is thawed and its window sent `WM_CLOSE`; the Guide stays open with the game still listed as Running. A second Quit while it is quitting asks nothing more.
- **Ten seconds on** (`QUIT_TIMEOUT`) with the game still there: "Force quit {X}?", "It hasn't closed. Anything unsaved will be lost.", "Force quit" (U+E711) and "Cancel". It opens the Guide if that was closed meanwhile, with its panel, and with `PromptOpen.wav` only. Nothing is counted while the question stands. **Force quit** ends the process that owns the game's window (`TerminateProcess`: the Bundle's `Process.Kill` on that one process, not its tree) and asks again after another ten seconds if the game is somehow still there; with no window to find there is nothing to end and the quit is given up (`Dashboard::quit_given_up`). **Declining** (Cancel, B, the Guide closing, Power taking the prompt's place by a hold) leaves the game running, ends the quit and drops whatever was to start after it; the game is then held under the Guide again like any other.
- **A game being quit is not frozen** (`Dashboard::quitting`, which the shell hands to `Launch::quitting`).
- **The Guide closes 500 ms after a game quit from it has stopped** (`CLOSE_AFTER_QUIT`), in silence and with no button to wait for, onto Home. Not if it was closed and opened again meanwhile, nor if another game has started.
- **A game that ends by itself under the open Guide leaves the Guide open.** Its list is made again without the game, focus at the same place in it; a menu or a prompt about the game goes in silence; **Power stays up**, from its entry or by a hold; the foreground stays the Guide's. Closing the Guide then lands on Home (the Bundle's `homeOwed`). The Bundle leaves it at that and so does the port: nothing closes the Guide for the user.
- **Home from the Guide over a game** brings the Dashboard in front of the game, which runs on behind it (`Effect::ShowHome`; the Bundle's `BringToFront(Home)`): Home draws and takes the pad again, the Guide still lists the game as Running, and A on it there, or on its own tile on Home, goes back to it. **My games & apps, Settings and Search** do the same onto their pages. At Home in front of a game, A on another game's tile asks "Quit {X} and start {Y}?" too, the Guide opening for the prompt as the Bundle's `ShowDialog` opens it.
- **With the Guide setting off** a prompt shows on the Overlay by itself, the dim and the menu panel with no panel, hints or Status area, and answering it closes the Overlay (7.5).
- **Time is told, not read**: `Dashboard::advance(seconds)` counts the ten seconds and the half second, and `Dashboard::timer()` says how soon it has something to do, so the window's loop wakes for it and a test passes ten seconds in one call.

**What the shell does** (`crates/shell/src/launch.rs`, `overlay.rs`; `crates/app/src/window.rs`):

- Who gets the foreground as the Guide lets go of it is the Overlay's to carry out, once the button that closed the Guide is up (3.5): `Overlay::give_back_to` names Home's window (Home chosen, or a game that ended under the Guide) or the game's (the Running entry) in place of the window recorded as the Guide opened. Thawed first, then handed the foreground, as ever.
- Home's window, behind a running game as its backdrop since the Launch screen went (docs/spec/launch-screen.md), is brought in front of it for Home (`Launch::show_home`) and put directly behind it again when the game is gone back to (`Launch::show_game`); behind a game it draws the backdrop once and nothing more.
- A game ending under the open Guide brings Home's window back **without** the foreground (`launch: the game has ended, Home back behind the Guide`): the Guide keeps it, as the Bundle's test `A_game_exiting_under_the_open_Guide_leaves_the_foreground_with_the_Guide` has it.
- The library is read again before the core is told a game ended, so the game that starts next is named by its place in the library as it then is.

**The log**, for the VM test (`e2e/payloads/guide-games/run.ps1`): `guide:` lines now come for what time and a game ending change, not only presses; a menu or a prompt has its title after its focus (`dialog Menu { prompt: true, row: 0 } "Force quit Brass Meridian?"`), and the line ends `, quitting` or `, quitting, then {Y}` while a quit is under way. The stand-in game takes `--ignore-close` to be a game that does not close when asked.

**Where the port differs from the Bundle, knowingly:**

- "Launching" is the Launch screen being up: the port has no separate begin-of-launch state, so there is no 15 s `LaunchBeginTimeout`.
- The Bundle's Home tile opens the Game hub and plays on a second A; the port's tile plays on A until the Game hub (M3) is there. What Play does with a game running is the same either way.
- A prompt wanted over a game with no Overlay to show it on is not asked: the quit is given up, or the game not started.

**Hooks for later tickets:** a page that asks for `Effect::Launch` or `Effect::QuitGame` (the Game hub's and the Tile menu's Play and Quit) already gets all of the above: the core routes them through `Dashboard::play` and `Dashboard::quit_game`. A launch that fails or needs the user is where #41's notifications post.

---

## 15. The Flyouts: what was built, and what was decided (#41)

Matched against the Bundle's pictures `52-guide-utility-focused.jpg`, `53-guide-notifications-flyout.jpg` and `54-guide-audio-flyout.jpg` (set `port-reference-20261115-215324`), with section 5's values from its XAML.

- **Where it is.** `crates/screens/src/guide/flyout.rs` is which Flyout is open, where focus is in it, and how it is drawn. The Guide's panel is dimmed under it (`#73000000` over the panel's own shape), the hints hide, the Status area stays. The bumpers do nothing; B closes an open list, then the Flyout (focus back on its Utility), then the Guide, all in silence; the Guide button closes the Guide whatever is open. A Flyout remembers nothing: opened again, focus is on its first entry.
- **A dialog takes a Flyout's place.** Power by a hold, and a prompt that comes by itself ("Force quit X?"), close the open Flyout as they come up. The Bundle keeps the Flyout under its dialog; here the panel behind a dialog is one blurred picture of the panel alone, and a Flyout under it is not worth a second one.
- **Notifications** are the Dashboard's own list: `Dashboard::notify(text) -> id`, `Dashboard::notifications() -> &[Notification { id, text }]` (oldest first; the Flyout lists them newest first), and `Dashboard::open_notifications()`, which opens the Flyout in an open Guide. These are the names and shapes Toasts (#42, PR #66) has, so that the two meet: #66's Toast strip posts through `notify`, and its "Guide opened while a Toast showed" calls `open_notifications`. Nothing posts a notification yet but a test and `dashboard --notify <text>` (repeatable), which is how the VM test fills the Flyout.
- **The badge** on the Notifications Utility counts the list, or what the shell's `Status` says there is, whichever is more.
- **A notification's card**: three lines of text at most, then an ellipsis: the Bundle's 120 of height is three lines of its type and part of a fourth, which WPF does not draw.
- **Audio's titles are "Output" and "Input"**, as in the Bundle's picture, and the list ends with the divider and Now Playing (section 5.3, now-playing.md): "No music app is running" while there is nothing to show.
- **The system's sound is the shell's.** The core holds an `Audio` (`model::audio`: the output and input devices, the default of each, the volume 0..100, mute), which the shell gives it with `Dashboard::set_audio`, and asks for changes with `Effect::Volume(level)`, `Effect::Mute(on)` and `Effect::DefaultDevice(flow, place)`. It shows a change at once and takes the system's word at the next reading. `shell::audio::AudioControl` is the trait; `SystemAudio` is Windows' Core Audio (`IMMDeviceEnumerator`, `IAudioEndpointVolume` on the default multimedia output, and for the default device the same undocumented `IPolicyConfig::SetDefaultEndpoint` the Bundle uses, for the console and multimedia roles); `FakeAudio` is a system of its own for tests, and what any machine but Windows has is no devices.
- **Following the system** (section 5.3's UNVERIFIED): the app reads the system's sound as the Audio Flyout opens, straight after each change it makes, and every 250 ms while the Flyout is open (`Dashboard::audio_shown`). The Bundle listens to Core Audio's callbacks; a reading four times a second while one Flyout is open is the same to look at and needs no COM object of ours called on another thread. Nothing is read while the Flyout is closed.
- **With no output device** Volume shows 0 and Mute off, and Left, Right and A on them do nothing, in silence; a dropdown with no devices says "None" and A on it does nothing.
- **Sounds**: A on a Utility, on a dropdown, on a device and on Mute is the Guide's select; a move in a Flyout or a list, and the volume changing, is the Guide's move; a press that changed nothing, and B, are silent.
- **The device list** is drawn over the entries below it and is as long as its devices; with more than fit the screen it is cut at the foot (the Bundle scrolls it; a machine with a dozen outputs is for when one turns up).
- **Reduce motion**: the Flyout's slide is dropped and its fade kept, as `ui.slide` does everywhere.
- **The log** (`--log-focus`): the `guide:` line has `, flyout Notifications Item(0)` or `, flyout Audio Item(2), volume 67` (and `, muted`), `Device(Output, 1)` while a list is open and `Nothing` in an empty Flyout; `audio: volume 67, muted false, output "Speakers" of 2, input None of 0` says what was read from Windows as the Flyout opens, after each change, and whenever a reading differs. Now Playing's lines (`Control(PlayPause)` in the `guide:` line, `now playing: ...`) are in now-playing.md.
- **The VM test** is `e2e/payloads/flyouts/run.ps1`: both Flyouts over a stand-in game; where the VM has an output device the real volume moves by 2 and back and mute goes on and off, each read back from Windows; where it has none, that nothing can be changed.

---

## 16. The Downloads Utility: what was built, and what was decided (#193)

Asked for by the user (2026-11-17): "add a download/queue icon in the bottom bar of the guide". **The Bundle has no such Utility**: its row is Notifications, Audio and Search. Everything here is the port's own, made of the parts the row already has.

- **Where.** The fourth square, after Search: x 408..495, y 942..1029. The row stays left-aligned from x 93 (2.8), as the Bundle's is, rather than centred: the three squares it has were never centred, and a fourth ends well inside the panel (x 605). Last, so the three that were there keep their places and the Toast that opens the Guide on Notifications still lands on the first.
- **What it looks like.** As the others: the 87 square, `#73000000` at rest and `#26FFFFFF` with focus, the ring, and the download arrow U+E896 at 33 (`theme::glyph::DOWNLOAD`, Home's Downloads tile's).
- **The badge.** While the Queue has anything in it, the count of games in it on the Notifications badge's pill: green, at the square's top right, the figure in the body type (2.8). Nothing at 0. It counts what Home's Downloads tile counts (`{n} in queue`), the Queue's entries, paused ones included. The issue also asked for the active download's progress "where the row's manner allows": the row has no such manner (no Utility shows anything but its glyph and a count), so there is none; Manage Queue, one press away, has the bar.
- **Focus.** Right from Search reaches it; Right on it does nothing (the row stops at its ends); Up from it is the tab's last entry, as from any Utility; Down from the last entry is still the first Utility, Notifications (3.2).
- **A** closes the Guide and opens Manage Queue (`manage_queue::open()`, the hook Home's Downloads tile and the Game hub's Queue action call) **as Search opens Search** (3.3, 14): the Guide closes, the page takes the place of any page that was up, and over a running game the Dashboard comes in front of the game with the page on it (`Effect::ShowHome`), the game running on behind. B on Manage Queue is Home, still in front of the game; the Guide's Running entry goes back to it. Not a page drawn on the Overlay over the game: the Guide has no pages of its own, and Manage Queue, which plays games and opens the Tile menu, is Home's window's.
- **Sounds.** As Search's: a move onto it is the Guide's move, A the Guide's select.
- **Hints.** None of its own: the Guide's hints are the Guide button's and, on an entry with a menu, Menu's (3.6).
- **The log.** `--log-focus`'s `guide:` line says `Landing Utility(3)` with focus on it; after A, the focus line is Manage Queue's (`ManageQueue: Nothing`, `ManageQueue: Row(0) of ...`).
- **Tests.** `tests/guide.rs` (the row's ends), `tests/pages.rs` (A at Home, B back to Home's rows), `tests/guide_games.rs` (A over a game), and the pictures `guide-downloads` (empty) and `guide-downloads-queued` (two queued, one downloading) in `tests/shots.rs`. The VM test is `e2e/payloads/guide-downloads/run.ps1`: from the Guide with the pad, at Home and over the stand-in game.

