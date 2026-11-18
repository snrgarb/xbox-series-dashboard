# Settings specification (as the Bundle draws it)

Source: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2). Read-only research; nothing was built or run. Why the page exists at all is docs/adr/0001-bundle-settings-page.md in that repository: Playnite's Fullscreen Settings window takes no extension sections, so the Bundle's own options got a page of their own.

Path shorthand used in citations:

- `T/` = `theme/XboxSeries.Dashboard/`
- `E/` = `extension/XboxSeries.Extension/`
- `C/` = `extension/XboxSeries.Core/`
- `CT/` = `extension/XboxSeries.Core.Tests/`

All numbers are design units = px on a 1920x1080 canvas. **Nothing on Settings is inside a Viewbox: every measure is 1:1** ("In Settings' palette and 1080p units", E/SettingsBoxes.xaml:6). Home's scale factor s = 1.114 (home.md) does not apply anywhere on this page, the focus ring and glow included.

**Glossary.** Settings is the Dashboard's page for the Bundle's own options: a left **rail** (Theme settings, Playnite settings, then under a divider More Playnite options, an action; in Debug builds a divider and the Debug menu) beside the selected **category**. A category is a grid of **boxes**; a box opens a **page** in the boxes' place, turns a checkbox over, or acts. (CONTEXT.md, "Settings" and "Playnite settings".)

**Pictures of the real thing.** `artifacts/e2e/see-all-20261110-113608/settings-1-personalisation.jpg` (Personalisation, focus on Boot video, a Release build's rail) and `settings-2-after-b.jpg` (Home after B); `artifacts/e2e/background-follows-selection-20261110-111334/settings-opened.jpg`, `background-delay-focused.jpg`, `background-delay-off.jpg` (a Debug build's rail; the checkbox in both states, its info beside it). All are of build 0.2.0, of 10 November, **before** type was set at 26 (#229: body text in them is 24) and before Reduce motion, Show Play history and Show battery percentages were added to Personalisation. Section 13.3 lists what they confirmed. There is no picture of Theme settings' own boxes, of Idle, Guide, Discord, Playnite settings, a dropdown open, Boot video, Profiles or the Debug menu's pages: those are from the source alone.

Things marked **NOT FOUND** were looked for and are absent; things marked **UNVERIFIED** depend on Playnite, WPF or font internals that are not in this repository. Line counts of wrapped text were estimated with Selawik's advance widths (Segoe UI's metrics; the same estimate reproduces the five line breaks of the info text in `background-delay-focused.jpg` at 24 px), without kerning: treat a line count as UNVERIFIED by one line. A Segoe UI line is taken as 1.33 em (34.58 at 26), a Bahnschrift line as 1.2 em, as home.md does.

Milestones are `docs/plan.md`'s. This page is **M3**; parts of it belong to features of later milestones and are marked so.

**Where the port differs first**: in place of Theme settings its rail has four generic categories, General, Account, Games & apps and System, each with its boxes in rows or in titled sections, as the console's Settings has them (section 16.1; #194). Sections 1 to 15 are the Bundle's.

---

## 1. Page structure

### 1.1 Layers and hosting

Settings is an extension-drawn element (`XboxSeries_Settings`) spanning the whole Dashboard page, over Home's hero and scrim and under the Game hub, hint bar, Tile menu and text entry (T/Views/Main.xaml:390-398, 761-781). Its root grid has an **opaque** background, so nothing of Home shows through, and is `Visible` exactly while `SettingsState.IsShown` (E/SettingsControl.xaml:293). While it is up Home's top bar, Recent row, wide tiles, Groups and hero are collapsed (`XboxHomeOnly`, T/Views/Main.xaml:307-319), and My games & apps and Search are closed: the pages are never up together (C/ExtensionCore.cs:1691-1704, 1544-1545; C/ExtensionCore.Search.cs:38-39).

| # | Layer (back to front) | Rectangle (x, y, w, h) | Fill | Source |
|---|---|---|---|---|
| 1 | Page background | 0, 0, 1920, 1080 | `#FF1F1F1F` flat | E/SettingsControl.xaml:17, 293 |
| 2 | Rail column | 0, 0, 441, 1080 | (page colour) | E/SettingsControl.xaml:295, 305 |
| 3 | Column divider | 441, 0, 2, 1080 | `#FF303030` | E/SettingsControl.xaml:18, 296, 328 |
| 4 | Content host (`CategoryHost`): header row then the category's pane | 520, 72, 1230, 1008 | none | E/SettingsControl.xaml:334-338 |
| 5 | Progress copy (only while a library task runs, section 11.1) | right edge 1815, y 84..144 | - | T/Views/Main.xaml:405-441 |
| 6 | Text entry (Console keyboard over the dimmed page; section 9.6) | whole screen | - | T/Views/Main.xaml:781 |

Content host arithmetic: column 2 starts at 441 + 2 = 443 and runs to 1920; `Margin="77,72,170,0"` gives x 443 + 77 = **520** to 1920 - 170 = **1750** (1230 wide) and y 72 to 1080. Its rows are `118` then `*`: the header row is y 72..190, the pane row **y 190..1080** (890 tall) (E/SettingsControl.xaml:334-338).

### 1.2 Everything at rest (opened from the top bar: rail focus on Theme settings)

| Element | Rectangle (x, y, w, h) | Source |
|---|---|---|
| Title "Settings" | text box 75, 78, to the rail's edge, 54 | E/SettingsControl.xaml:305-306 |
| Rail item 1, Theme settings | 0, 190, 441, 58 | E/SettingsControl.xaml:48-49, 308 |
| Rail item 2, Playnite settings | 0, 262, 441, 58 | :311 |
| Rail divider | 75, 334, 323, 2 | :315 |
| Rail item 3, More Playnite options | 0, 350, 441, 58 | :316 |
| Rail divider 2 (Debug builds) | 75, 422, 323, 2 | :320 |
| Rail item 4, Debug menu (Debug builds) | 0, 438, 441, 58 | :322 |
| Category header | text top-left at 520, 72 | :341 |
| Box, column 0 / column 1 | x 520..957 / x 975..1412 | E/SettingsBoxes.xaml:36-38 |
| Box, row r | y 210 + 149 r .. 341 + 149 r | E/ThemeSettingsPane.xaml:19 |

Rail arithmetic: the rail stack has `Margin="0,78,0,0"`; the title is 54 tall with `Margin="75,0,0,58"`, so items start at 78 + 54 + 58 = **190**; each item is 58 tall with 14 under it (pitch 72); a divider is 2 tall with `Margin="75,0,43,14"` (x 75 to 441 - 43 = 398; pitch 16). So: 190, 262, [334], 350, [422], 438 (T/Views/SettingsMenus.xaml:125 also states "78 down, the items starting at 190").

Box arithmetic: every pane of boxes has `Margin="0,20,0,0"` (y 190 + 20 = **210**); a box is 437 x 131 with `Margin="0,0,18,18"`, so column pitch 455 and row pitch 149; two columns span 437 + 18 + 437 = **892** (x 520..1412).

---

## 2. The rail

E/SettingsControl.xaml:33-82, 305-326.

### 2.1 Title

"Settings": **Bahnschrift** regular, 30, `#FFFFFFFF`, left edge x 75, in a 54-tall box from y 78, character ellipsis (HeaderText, E/SettingsControl.xaml:21-31, 306; font keys T/Constants.xaml:228-232). Not focusable.

### 2.2 Items

| Property | Value | Line |
|---|---|---|
| Rectangle | The rail's full width, 441 x 58, edge to edge (x 0..441), no corner radius | 48, 53 |
| Name | **Bahnschrift**, 24, **weight 350** (semilight), `#FFFFFFFF`, vertically centred, left edge x 75, right limit 441 - 43 = 398 (323 wide), one line, character ellipsis | 34-38, 59 |
| Rest | No fill (page colour shows) | 54 |
| Selected (the category showing; focus is in its content, or on More Playnite options) | Fill `#FF262626` over the whole 441 x 58; a 6-wide bar `#FF107C10` on its right edge (x 435..441, full 58) | 60-67 |
| Focused | Fill **`#FF107C10`** (GlyphBrush, the darker accent: not the ring's `#2CA243`) over the whole 441 x 58, and the glow (section 5.3). The bar is still there, same colour, so unseen. No ring. | 68-71 |
| Focused, z-order | Drawn over its neighbours, so their fills do not cover its glow | 77-81 |
| Pressed / disabled | NOT FOUND (no such state) | - |

Order and names (lines 308-324): **Theme settings**, **Playnite settings**, divider, **More Playnite options**, and in Debug builds only (shown by code, E/SettingsControl.xaml.cs:811-822) a second divider and **Debug menu**. Dividers are `#FF303030`.

### 2.3 What focus on a rail item does

- Focus landing on **Theme settings**, **Playnite settings** or **Debug menu** shows that category at once: no A needed (`OnCategoryFocused` -> `FocusSettingsCategory`, E/SettingsControl.xaml.cs:347-358; C/ExtensionCore.cs:1754-1761). Theme settings always shows its own boxes then, never one of its pages ("focus on the rail is never in a page of them"; CT/ThemeSettingsTests.cs:199).
- **More Playnite options** is not a category: focusing it changes nothing on the right. The category last shown stays, and its rail item stays Selected (lit, with the bar) while More Playnite options is green (E/SettingsControl.xaml:314-318; E/SettingsControl.xaml.cs:335-338).
- The new category flies in (section 10.2).

---

## 3. Header and content area

- Header: **Bahnschrift**, **42**, weight 350, `#FFFFFFFF`, top-left at (520, 72), one line, character ellipsis at the host's width 1230 (E/SettingsControl.xaml:341). About 50 tall (1.2 em, UNVERIFIED), in the 118 row, which leaves 68 clear above the pane row.
- Header text follows what shows (E/SettingsControl.xaml.cs:295-301, 328-331; E/PersonalisationPane.xaml.cs:79-93):

| Showing | Header |
|---|---|
| Theme settings' boxes | Theme settings |
| Theme settings > Personalisation | Personalisation |
| ... > Boot video / Who's playing today? / Profiles / Picture | Boot video / Who's playing today? / Profiles / Picture |
| ... > Profiles > one Profile | that Profile's gamertag |
| Theme settings > Idle / Guide / Discord | Idle / Guide / Discord |
| Playnite settings | Playnite settings |
| Debug menu and its pages | Debug menu / Notifications / Steam / Console installs / CEF installs / Stub installs |

- There is no breadcrumb, no back arrow and no subtitle: the header is the only sign of depth.
- Exactly one pane shows in the pane row at a time (E/SettingsControl.xaml.cs:315-320, 333).
- **Nothing scrolls** except the Profiles page, the Picture page and an open dropdown's list (sections 6.5, 6.6, 4.6). The panes are not clipped by anything but the screen.

---

## 4. Controls

Text styles shared by all of them: **Text** = Segoe UI 26, `#FFFFFFFF`, character ellipsis; **MutedText** = the same in `#B3FFFFFF`; **Icon** = `Segoe Fluent Icons`, fallback `Segoe MDL2 Assets`, white, centred (E/SettingsBoxes.xaml:8-26; E/SettingsControl.xaml:21-26, 84-86).

### 4.1 Box (E/SettingsBoxes.xaml:34-72)

| Part | Value | Line |
|---|---|---|
| Size | **437 x 131**, 18 to the right and 18 below (in the grid) | 36-38 |
| Back | `#FF2D2D2D`, **square corners** (no radius) | 44 |
| Icon | Glyph from the icon font, **34 px**, white, centred in the first 100 px of the box (centre 50 from its left edge) and vertically | 46-47 |
| Name | Text (Segoe UI 26 white), vertically centred, from 100 in to 24 short of the right edge (`Padding="100,0,24,0"`): 313 wide, one line, character ellipsis | 39, 48 |
| Second line (value, subtitle) | **None, ever**: "the name alone, never a second line saying what it's set to". What a box is set to shows on the page it opens. | 29-30 |
| Focused | Back `#FF4A4A4A` (the whole box alike), the focus ring (section 5.1) and its glow; drawn over its neighbours | 57-71 |
| Pressed, disabled | NOT FOUND | - |

### 4.2 Checkbox box

A Box with no icon: `Padding="30,0,24,0"`, so its content runs from 30 in to 24 short of the right edge (E/PersonalisationPane.xaml:154-172; E/PlaynitePane.xaml:59-77).

- Name: Text, vertically centred, from x + 30, with 50 kept clear at its right (`Margin="0,0,50,0"`): at most 437 - 30 - 24 - 50 = 333 wide, ellipsis.
- Checkbox: a glyph of the icon font, **34 px**, white, right-aligned in the content (its right edge 24 from the box's right edge, so it occupies x + 379..x + 413 of the box), vertically centred. **Off: U+E739** (an empty square outline). **On: U+E73A** (the same square with a tick in it). There is no fill, no accent colour and no knob: on and off differ only by the tick.
- A turns it over **in place**, at once, saved; no page opens, focus stays, nothing animates.
- **Info text**: while (and only while) the box has keyboard focus, a paragraph shows beside it in the second column's place: Text style (Segoe UI 26, **white**, not muted), wrapping, no trimming, **400 wide**, its top-left at box x + 437 + 18 + 30 = **x 1005**, box top + 8 (`Margin="30,8,0,0"`, top-aligned) (E/PersonalisationPane.xaml:178-182).

### 4.3 Row (a choice in a list: Boot video, Who's playing today?) (E/PersonalisationPane.xaml:25-65)

- **892 x 66** ("the width of two boxes"), 8 below (pitch 74), left-aligned at x 520.
- Back `#FF2D2D2D`, square corners; focused `#FF4A4A4A` with ring and glow (ring `CornerRadius="8"`, as a Box's).
- The one set now: tick **U+E73E**, icon font 24, white, centred in a 30-wide slot that starts 30 in (x 550..580). Others: the slot is empty.
- Name: Text, from 84 in (x 604) to 24 short of the right edge, one line, ellipsis.
- A sets that choice at once and saved; the tick moves; the page stays and focus stays on the row.

### 4.4 Pane button (Discord's Type, Paste, Connect, Disconnect) (E/SettingsControl.xaml:95-133)

- Height 58, `MinWidth` 180, wider if its label + 60 is (content `Margin="30,0"`, centred).
- Back `#FF333333`, radius 2. Label Text, centred.
- Focused: back `#FF4A4A4A`, ring (`Margin="-8"`, 4 thick, `CornerRadius="10"`) and glow; drawn over its neighbours.
- "Does nothing now" (Tag False: Connect without a Client ID and Secret, Disconnect with nothing to disconnect): label at **opacity 0.4**, back unchanged, **still focusable** ("keeps its place in the pad's path").

### 4.5 Dropdown box (E/SettingsControl.xaml:153-192)

- Height 58; as wide as its pane (**560** on Idle and Guide).
- Back: fill `#14FFFFFF`, a 2-thick edge `#80FFFFFF`, radius 4.
- Value: Text, vertically centred, 18 in from the left, 50 kept at the right, one line, ellipsis.
- Chevron: **U+E70D** (down), icon font 16, white, right-aligned 18 from the right edge, vertically centred.
- Focused (closed): fill `#26FFFFFF`; ring `Margin="-8"`, 4 thick, `CornerRadius="12"`, with glow.
- **Open** (Tag "Open"; focus is down in its list): fill `#26FFFFFF`; its own 2-thick edge turns `#FF2CA243` (radius 4) with the glow centred on that edge (section 5.3); the chevron is **U+E70E** (up). No ring.

### 4.6 Dropdown list and its choices (E/SettingsControl.xaml:226-289; E/SettingsControl.xaml.cs:38-45, 562-628)

It is **not a flyout, popup or dialog**: a panel that opens **in the page, directly under its box and as wide**, laid over whatever is below without moving it (it sits in a zero-height canvas), the page not dimmed.

| Part | Value | Line |
|---|---|---|
| Panel | **560 wide**, left edge the box's (x 520), top **8 under the box**; fill `#FF2E2E2E`, 1-thick edge `#33FFFFFF`, radius 4 | xaml 232-238, 372; cs 39 |
| Inside | 8 above the first choice and 8 under the last | xaml 375 |
| Choice | Height **60**, 2 under each (pitch **62**), 6 in from the panel's inner sides: x 527..1073 (546 wide); radius 4; no fill at rest | xaml 252-260 |
| Choice i (0-based) | y = panel top + 1 + 8 + 62 i | derived |
| Selected mark | Tick **U+E73E**, icon font 20, white, 14 in from the choice's left edge, vertically centred; only on the one set now | xaml 261-263, 273-275 |
| Label | Text, from 52 in to 16 short of the right edge, one line, ellipsis | xaml 264 |
| Focused choice | Fill `#26FFFFFF`; ring `Margin="-8"`, 4 thick, `CornerRadius="12"`, with glow; drawn over the rows round it | xaml 265-287 |
| Panel height | 2 + 16 + 62 n for n choices, cut to `max(202, 1080 - panelTop - 32)`: at least three rows (3 x 62 + 16), and never nearer the screen's foot than 32 | cs 39-45, 585-586 |
| Scrolling | Cut short, the choices scroll inside it, no scrollbar drawn; the focused choice is kept in view with 10 above and below it (so its ring is not cut). The scroll area reaches 16 past the panel's sides so a ring's glow is not clipped there. No animation: it jumps. | xaml 240-245; cs 41-42, 597-603 |

Opening: A on the box. The list is rebuilt from the core's choices **as it opens**, the one set now ticked and **focused**. A on a choice sets it (the core may refuse it: the box shows what the core then holds), closes the list and returns focus to the box. B closes it unchanged, focus back on the box. Opening and closing are instant (cs 562-628).

While a box or its list has focus the whole box-and-list group is raised over the controls after it, so Dim after's open list lies over Show things after (E/SettingsControl.xaml:139-145, 366, 381).

### 4.7 Checkbox row (the Guide page's "Use the Guide") (E/SettingsControl.xaml:194-224, 410-427)

Drawn as a dropdown box, not as a Box: height 58, 560 wide, fill `#14FFFFFF`, 2-thick edge `#80FFFFFF`, radius 4; focused, fill `#26FFFFFF` and the ring (`Margin="-8"`, 4 thick, `CornerRadius="12"`) with glow. Content 18 in from each side: the name (Text, 50 kept at its right), and at the right, "in the chevron's place", the checkbox glyph: **U+E739** off, **U+E73A** on, icon font **28**, white. A turns it over in place.

### 4.8 Gamerpic button (Picture page; M5) (E/PersonalisationPane.xaml:71-115, 362-370)

172 x 172, 30 to the right and below (pitch 202). The picture is a 148 circle (12 in). Focus: a **6-thick round ring** `#FF2CA243` at the button's edge (outer diameter 172, so 6 clear of the picture), with the round glow (section 5.3). The one worn: a 44 x 44 badge at the picture's bottom right: a disc `#FF107C10` with a 3-thick stroke `#FF1F1F1F`, and the tick U+E73E at 20, white. A Profile's picture anywhere on Settings: the picture `UniformToFill` in a circle, over a disc **`#FF2CA243`** with the person glyph U+E77B (scaled to the disc less 22 each side), which shows for none or one that will not load.

---

## 5. Focus visuals

One visual per element, shown and hidden **instantly** with keyboard focus; it does not travel, pulse or animate (as home.md section 4). Colour `#FF2CA243` (T/Constants.xaml:10), except the rail's fill.

### 5.1 The ring

`Border Margin="-8" BorderThickness="4"`: a 4-thick stroke whose inner edge is 4 outside the element and outer edge 8 outside; no fill. It is home.md section 4.1's ring **at scale 1** ("4 of gap and 4 of ring out from the box, as Home's tiles", E/SettingsBoxes.xaml:49-51). Only the corner radius differs by control (WPF's `CornerRadius` is the stroke's centre line, home.md 4.1):

| Control | `CornerRadius` (centre line) | Outer / inner radius | Element's own radius | Source |
|---|---|---|---|---|
| Box, Row | 8 | 10 / 6 | 0 | E/SettingsBoxes.xaml:50; E/PersonalisationPane.xaml:37 |
| Pane button | 10 | 12 / 8 | 2 | E/SettingsControl.xaml:106 |
| Dropdown box, checkbox row, dropdown choice | 12 | 14 / 10 | 4 | E/SettingsControl.xaml:171, 208, 265 |

Two boxes are 18 apart, so a focused box's ring (8 out) leaves 10 to its neighbour; its glow overlaps the neighbour, which is why the focused control is always drawn on top (E/SettingsBoxes.xaml:66-84).

The comment "the accent ring stands 3 off it" (E/SettingsBoxes.xaml:31) is out of date: the markup is 4 of gap.

### 5.2 Along with the ring

The focused element's back also lightens (Box and Row `#2D2D2D` -> `#4A4A4A`; pane button `#333333` -> `#4A4A4A`; dropdown box, checkbox row and choice -> `#26FFFFFF`). The info text beside a checkbox box appears (section 4.2). Nothing grows or moves.

### 5.3 The glow

The same resource as Home's rectangular glow, `XboxFocusGlow` (T/Constants.xaml:26-103; home.md section 4.3: ten faint nested bands of `#2CA243`, widths 31 to 4, alphas 0.034 to 0.048, each `CornerRadius="12"`, plus four corner spots), at scale 1. Where its centre line c falls, from each use's margins (band centre = 5.5 inside the glow control's edge):

| Use | Glow control's `Margin` | Centre line c | Source |
|---|---|---|---|
| Any ring of section 5.1 | -7.5 (the default) inside the 4-thick ring | **6 outside the element's edge** (the ring's centre), as Home | T/Constants.xaml:27 |
| Rail item, focused | -7.5 inside the fill (no ring, no stroke) | **2 outside the item's edge**: the halo hugs the green slab, reaching 17.5 past it. Its left part is off screen; on the right it crosses the column divider, which is drawn over it (the divider is a later sibling), and shows again from x 443 to 458.5 | E/SettingsControl.xaml:54-58 |
| Dropdown box, open | -6.5 inside the 2-thick accent edge | **1 inside the box's edge** (the edge's centre) | E/SettingsControl.xaml:164-170 |
| Gamerpic button | `XboxFocusGlowRound`, -8.5 inside the 6-thick ring | **3 inside the button's edge** (the ring's centre); round, no corner spots | E/PersonalisationPane.xaml:100-105 |

Derivation: with the carrier's outer edge e outside the element, stroke t and glow margin -m, c = e - t + m - 5.5 outside the element (8 - 4 + 7.5 - 5.5 = 6; 0 - 0 + 7.5 - 5.5 = 2; 0 - 2 + 6.5 - 5.5 = -1; 0 - 6 + 8.5 - 5.5 = -3).

---

## 6. The pages, one by one

All panes sit in the pane row (from y 190). Boxes are section 4.1's; "(c, r)" is column, row; a box's rectangle is x 520 + 455 c, y 210 + 149 r, 437 x 131 unless a page says otherwise.

### 6.1 Theme settings (the rail's first category) - M3

E/ThemeSettingsPane.xaml:19-32. Four boxes, two wide, each a line icon and a name, each opening its page in the boxes' place:

| Box | (c, r) | Rectangle | Icon (34 px) | Opens |
|---|---|---|---|---|
| Personalisation | (0, 0) | 520, 210, 437, 131 | U+E771 | section 6.2 |
| Idle | (1, 0) | 975, 210, 437, 131 | U+E708 | section 6.7 |
| Guide | (0, 1) | 520, 359, 437, 131 | U+E7FC | section 6.8 |
| Discord | (1, 1) | 975, 359, 437, 131 | U+E8F2 | section 6.9 |

**The port has no Theme settings** (#194): its rail lists four generic categories, General, Account, Games & apps and System, as the console's Settings does, and these four boxes, with Personalisation's that open pages and the port's own (Games & apps, Launchers, Art, Storage, Helpers, System, About), are placed among them: section 16.1.

### 6.2 Theme settings > Personalisation - M3 (three of its boxes open M5 pages)

E/PersonalisationPane.xaml:119-278; a grid of two auto columns and six auto rows.

| Box | (c, r) | Rectangle at rest | Kind | Icon / checkbox |
|---|---|---|---|---|
| Boot video | (0, 0) | 520, 210, 437, 131 | opens a page (6.3, built in #126) | U+E714 |
| Profiles | (1, 0) | 975, 210, 437, 131 | opens a page (6.5, built in #125) | U+E716 |
| Who's playing today? | (0, 1) | 520, 359, 437, 131 | opens a page (6.4, built in #127) | U+E748 |
| Background delay | (0, 2) | 520, 508, 437, 131 | checkbox box | U+E73A on (default) |
| Reduce motion | (0, 3) | 520, 657, 437, 131 | checkbox box | U+E739 off (default) |
| Show Play history | (0, 4) | 520, 806, 437, 131 | checkbox box | U+E73A on (default) |
| Show battery percentages | (0, 5) | 520, 955, 437, **131: bottom at 1086** | checkbox box | U+E73A on (default) |

(1, 1) is empty; the checkbox boxes are each on a row of their own, the second column's place kept for their info.

Info texts (section 4.2; x 1005, top = box top + 8, 400 wide; line counts estimated, see the header):

| Box | Text | Lines at 26 | Height |
|---|---|---|---|
| Background delay | "Waits a moment on a game before Home's background shows its art. Turn it off to change the background as you move along your recent games." | 5 | 172.9 |
| Reduce motion | "Shows pages, rows and tiles in place and holds the background still. Turn it on if the Dashboard feels slow on this PC." | 4 | 138.3 |
| Show Play history | "Ends Home's recent games with the Play history tile. Turn it off to show another recent game in its place; Play history stays in My games & apps." | 5 | 172.9 |
| Show battery percentages | "Shows how much charge is left beside the controller's battery, and this PC's on a laptop or handheld. Turn it off to show the batteries alone." | 5 | 172.9 |

The first three span their own row and the next (`Grid.RowSpan="2"`), so that text taller than the box does not push the next box down as it shows (E/PersonalisationPane.xaml:174-182, 209-213, 241-245): they overlap the empty second column of the row below and move nothing.

**A defect of the Bundle here, at 1080 tall, not to be copied.** Six rows need 6 x 149 = 894 from y 210, to 1104; the screen ends at 1080 and nothing scrolls (the page is in no scroller). So:

- Show battery percentages' box is cut by the screen's foot at rest: its last 6 px, and the lower stroke of its focus ring, are off screen.
- Its info has **no** row span (E/PersonalisationPane.xaml:273-277). Shown, it is taller (8 + 172.9 = 180.9) than its 149 row, so the row grows to 180.9 and the box, centred in its cell above its 18 margin, drops by (180.9 - 18 - 131) / 2 = **15.95** to y 970.95..1101.95 while it has focus; the info (y 963 on) loses its last lines below the screen. (Derived from the markup; there is no picture of this build's page.)
- The same drop is in the pictures for Background delay, which had no row span in 0.2.0: its box rests at y 508 and is at about 517.5 focused, (8 + 5 x 31.92 - 149) / 2 = 9.3 with the 24 px type of that build (`background-delay-focused.jpg`). The row span added since fixed rows 2 to 4 only.

Open question 1 is what the port does instead.

### 6.3 ... > Personalisation > Boot video - built in #126 (start-up.md 2)

E/PersonalisationPane.xaml:281-286; names at E/PersonalisationPane.xaml.cs:29-40.

- y 210: "Plays as Playnite starts in Fullscreen. Any button skips it." (MutedText, one line, 34.58 tall), 26 under it.
- Then nine Rows (section 4.3), x 520..1412, row i at y 270.58 + 74 i (the last ends at 928.58): **Off**, **Xbox (2001)**, **Xbox 360 (2005)**, **Xbox 360 Slim (2010)**, **Xbox 360 Special Edition (2012)**, **Xbox One (2013)**, **Xbox One X (2017)**, **Xbox Series S (2020)**, **Xbox Series X (2020)** (the default, C/ExtensionCore.Startup.cs:360).
- The one that plays is ticked. The page opens with focus **on the ticked row** (E/PersonalisationPane.xaml.cs:355-359). A sets and saves; no preview plays.

The port's page (start-up.md 2): the line is "Plays as the Dashboard starts. Any button skips it.", and the Rows are Off and the videos **found** in the media folder, in this order; with none found the line says so and Off is alone.

### 6.4 ... > Personalisation > Who's playing today? - built in #127 (start-up.md 9)

E/PersonalisationPane.xaml:289-294; E/PersonalisationPane.xaml.cs:194-200. As Boot video: "Choose who's playing as Playnite starts in Fullscreen and wakes from sleep." then two Rows at y 270.58 and 344.58: **Ask every time** (the default) and **Sign in the last profile**. Opens with focus on the ticked one.

The port's line is "Choose who's playing as the Dashboard starts and wakes from sleep."

### 6.5 ... > Personalisation > Profiles, and one Profile - built in #125 (profiles.md 4)

E/PersonalisationPane.xaml:300-351; E/PersonalisationPane.xaml.cs:139-153, 419-443.

- **Profiles**: boxes two wide from (520, 210), one per Profile, then **Add profile** (icon U+E8FA) last. A Profile's box has no icon: `Padding="30,0,24,0"`, its picture an 80 circle (box x + 30..110, vertically centred) and its gamertag (Text) 22 right of it (from box x + 132), one line, ellipsis. The page scrolls vertically, no scrollbar; the scroller is set 32 up and left of the boxes so a ring's glow is not clipped. Opens with focus on the first box.
- A on a Profile opens **its page** (header: its gamertag): its picture, a 120 circle at (520, 210), 30 under it, then boxes from y 360: **Gamertag** (0, 0) U+E8EC, **Real name** (1, 0) U+E8AC, **Picture** (0, 1) U+E8B9, **Remove profile** (1, 1) U+E74D. Remove profile is absent while there is only one Profile. Opens with focus on Gamertag.
- Gamertag and Real name open text entry from what they are now; Add profile opens text entry for a gamertag (caption "Gamertag", empty) and adds the Profile once it is kept, focus going to the new Profile's box (C/ExtensionCore.TextEntry.cs:59-85).
- Remove profile asks "Remove {gamertag}?" (title "Remove profile", Yes / No) in **Playnite's own message box**.

### 6.6 ... > Profiles > a Profile > Picture - built in #125 (profiles.md 4)

E/PersonalisationPane.xaml:354-381; E/PersonalisationPane.xaml.cs:419-449. Gamerpic buttons (section 4.8) four wide from (520, 210), pitch 202, the Windows account picture first, then the Gamerpics folder's (C/ExtensionCore.Startup.cs:387-400); scrolls as Profiles does (set 24 up and left). The one worn carries the tick badge and has focus as the page opens. With at most one picture to choose from, a line shows above them, "Add images to the Gamerpics folder to choose from more." (MutedText, 26 under it, so the buttons start at y 270.58). A wears the picture and steps back to the Profile's page. With no pictures at all the Picture box opens no page: Playnite's message box says "Add images to the Gamerpics folder to choose from." (title "Picture").

### 6.7 Theme settings > Idle - built (#128); the Idle screen itself is [idle-screen.md](idle-screen.md)

E/SettingsControl.xaml:364-396; C/IdleState.cs:30-91; C/ExtensionCore.Idle.cs:214-248.

A 560-wide column at x 520, **from y 190** (this pane has no 20 top margin), two groups 40 apart:

| Element | Rectangle (x, y, w, h) |
|---|---|
| Label "Dim after" (Text) | 520, 190, -, 34.58 |
| Dim after's dropdown box | 520, 238.58, 560, 58 (14 under the label) |
| Its list, open | 520, 304.58, 560, 700 (eleven choices; fits: the cut is 1080 - 304.58 - 32 = 743.42) |
| Label "Show things after" | 520, 344.58, -, 34.58 |
| Show things after's dropdown box | 520, 393.16, 560, 58 |
| Its list, open | 520, 459.16, 560, min(18 + 62 n, 588.84) |

- **Steps**, the same for both, in this order: `1 min`, `2 min`, `5 min`, `10 min`, `15 min`, `30 min`, `1 h`, `2 h`, `3 h`, `4 h`, `Never` (written exactly so). Defaults: Dim after **2 min**, Show things after **5 min**.
- **The coupling rule.** Show things after is never shorter than Dim after (Never counts as longer than any time):
  - Show things after's list **leaves out** every step shorter than Dim after; Never is always in it. With the defaults it offers ten (2 min to Never: 638 tall, so cut to 588.84 and scrolling); with Dim after on Never it offers Never alone.
  - Raising Dim after past Show things after **raises Show things after to the same step**, at once (both boxes follow). Lowering Dim after leaves Show things after as it was.
  - A choice the rule forbids changes nothing and saves nothing.
- Both are shared by every Profile, saved at once, and count from the moment they are set (CT/IdleSettingsTests.cs:35-189).
- **The port:** the rows `idle.dim_after` and `idle.show_things_after` in the settings table, in seconds or `never` (16.4), saved by `Effect::IdleTimes`. The focus log: `Idle: Dim after`, `Idle: Show things after`, and with a list open `Idle: Dim after, list on "2 min"`. A time that is not a step (`--idle-after`'s) shows as seconds, `3 s`, and its list opens on its first step.

### 6.8 Theme settings > Guide - the checkbox is **M3** (the Guide is M2); the Now Playing source is #129's

E/SettingsControl.xaml:405-441; C/SettingsState.cs:141-195; C/ExtensionCore.NowPlayingSource.cs:48-70. A 560-wide column at x 520 from y 190:

| Element | y (with the two descriptions at three lines each, estimated) | Height |
|---|---|---|
| "Guide" (Text) | 190 | 34.58 |
| "Opens the Guide with the Guide button and from Home. Turn it off to leave the Guide button to Steam or another app." (MutedText, wrapping at 560, 4 under the label) | 228.58 | 103.75 |
| Checkbox row "Use the Guide" (section 4.7), 18 under the description | 350.33 | 58 |
| "Now Playing source" (Text), 40 under the checkbox | 448.33 | 34.58 |
| "The app the Audio flyout's Now Playing follows while it plays or is paused, else whatever Windows shows." (MutedText, wrapping) | 486.91 | 103.75 |
| Dropdown box (section 4.5), showing the source's name | 608.66 | 58 |
| Its list, open | 674.66 | 18 + 62 n, cut to 373.34 |

With L1 and L2 lines in the two descriptions: checkbox top = 246.58 + 34.58 L1; dropdown top = checkbox top + 154.58 + 34.58 L2.

- **Use the Guide**: on by default. Off, the Guide never shows: the Guide button is left alone, Home's profile and Achievements tile do nothing, its Discord tile opens Settings on Discord (section 8.2), and a Toast has no "Press ... to view" (C/SettingsState.cs:155-160). In force at once, saved, shared by every Profile.
- **Now Playing source** choices: **Any**, **Spotify** (the default), then every other app that has had a media session this run (C/ExtensionCore.NowPlayingSource.cs:48-59, 107-108). A source saved in an earlier run whose app has had no session yet is among them, first after Spotify, so that the list always holds what the box shows. What it does: docs/spec/now-playing.md.

### 6.9 Theme settings > Discord - built in #143 ([../discord.md](../discord.md))

As built: the y positions below are the Bundle's picture's (`artifacts/e2e/port-reference-20261115-192720/36-settings-discord.jpg` in the Bundle): the steps and the code from y 255; the address under the code in four centred lines, broken after each slash ("https://", "discord.com/", "developers/", "applications"); the Client ID row's buttons at y 645, the Secret's at 734, Connect and Disconnect at 833, the status line 24 under them. The QR code is a plain stand-in (a white square, the corner marks' places outlined): the Bundle's picture is not copied, and a real code is for later (discord.md). The value under "Client ID" and the status line are white, as in the picture. The focus log: `Settings: Discord: <button>; client id <set|not set>, secret <saved|not set>, "<status>"`.

E/SettingsControl.xaml:449-543; C/DiscordSettingsState.cs:26-32, 80-83. The pane is the host's full width (x 520..1750) from y 190, top to bottom:

1. "Connect your own Discord application, so the Guide can show you on Discord." (MutedText), 30 under it.
2. A two-column block, 36 under it. Left: four numbered steps, each a 40-wide number and wrapping Text, 16 between: "Create an application at discord.com/developers/applications." / "Under OAuth2, add the redirect http://127.0.0.1." / "Under App Testers, add yourself." / "Paste its Client ID and Client Secret here, then select Connect." Right, a 220-wide column (x 1530..1750): a **200 x 200 QR code** of the developer portal (`DiscordPortalQr.png`, nearest-neighbour scaling) and under it the address (MutedText, wrapping, centred; where it breaks is UNVERIFIED).
3. **Client ID** row, 20 under it: a 420-wide name column ("Client ID" and its value, or "Not set") then two pane buttons: **Type** (x 940..1120) and **Paste** (x 1140..1320).
4. **Client Secret** row, 36 under it: the same; its value reads only "Saved" or "Not set" (the Secret is never shown).
5. **Connect** and **Disconnect**, pane buttons 20 apart from x 520, 24 under them.
6. The status line (Text, wrapping): "Not connected", "Connecting...", "Approve the request in Discord", "Connected as {user}", "Discord isn't running", "Discord closed the connection[: {message}]", "Couldn't connect to Discord: {message}" (C/ExtensionCore.Discord.cs:29-31, 181, 208, 361, 380; C/ExtensionCore.DiscordTokens.cs:125, 165).

Type opens text entry (captions "Client ID", from what is saved; "Client Secret", from empty and shown as dots, an empty one kept leaving the saved Secret alone). Paste saves the Windows clipboard's text at once; an empty clipboard forgets the field; a clipboard another program holds open pastes nothing (E/SettingsControl.xaml.cs:759-795, 826-836; C/ExtensionCore.TextEntry.cs:88-112). Exact y positions were not derived (they hang on how the address wraps; M6).

### 6.10 Playnite settings (the rail's second category) - **goes away** in the port (plan.md, "Goes away")

E/PlaynitePane.xaml:23-85; C/ExtensionCore.LibraryUpdate.cs.

| Box | (c, r) | Rectangle | Kind | Icon |
|---|---|---|---|---|
| Download metadata (reads **Cancel** while it runs) | (0, 0) | 520, 210, 437, 131 | action | U+E896 |
| Playnite settings | (1, 0) | 975, 210, 437, 131 | action: opens Playnite's own Fullscreen Settings window (section 12.1) over the page | U+E713 |
| Update after downloads | (0, 1) | 520, 359, 437, 131 | checkbox box, on by default | U+E73A / U+E739 |

Info for Update after downloads (x 1005, y 367, 400 wide, four lines, 8 + 138.3 = 146.3 < 149, so nothing moves): "Updates every library and downloads missing metadata for new games after a download finishes."

### 6.11 More Playnite options (the rail's third item) - **goes away**

An action: A opens Playnite's restyled main menu window over the page (C/ExtensionCore.cs:1792; E/PlayniteHost.cs:371-375). Section 12.2.

### 6.12 Debug menu (Debug builds only)

E/DebugPane.xaml:20-38; E/DebugPane.xaml.cs:35-99, 166-206. Under an optional line (MutedText, wrapping at 892, 26 under it) a grid of Boxes two wide. With a one-line hint the boxes start at y 270.58; without, at y 210.

| Page | Line over the boxes | Boxes (icon) |
|---|---|---|
| Debug menu | none | Notifications (U+EA8F), Steam (U+E896) |
| Notifications | "Each puts up a Toast, as Playnite's own would." | Notification (U+EA8F), Screenshot notification (U+E722), Long notification (U+E8BD), Three notifications (U+E8FD), Achievement, Rare achievement, Three achievements (only while achievements are read; their glyphs were not extracted), Call (U+E717), Call with avatar (U+E77B), Call, avatar fails (U+E783) |
| Steam | "Target: {name}. CEF debugging: on/off. Each install reports what Steam showed as a Toast, 20 seconds on." | Change target (U+E8AB), Request licence (U+E8D7), Uninstall target (U+E74D), CEF debugging (U+EBE8), Console installs (U+E756), CEF installs (U+E774), Stub installs (U+E8A5) |
| Console installs, CEF installs, Stub installs | the Steam line | install trials of the Steam adapter (three to six boxes each) |

A box either opens its page (focus on its first box) or acts and stays.

---

## 7. Inventory: every page and every entry

"Stored" names the entry in the Bundle's one settings store: a DPAPI-encrypted file, `discord-secrets.dat` in the extension's data folder, per Windows user, holding named strings (E/DpapiSecretStore.cs:17-30), most of them JSON. Every setting is shared by every Profile. Every change is applied and saved at once: there is no Save, Apply or Cancel anywhere. Options saved "only once set" are absent from the file until the player changes them, so a later change of default reaches those who never chose.

| Path | Control | Values (**default**) | What it does | Stored | Port |
|---|---|---|---|---|---|
| Theme settings | rail category | - | Shows its four boxes | - | **Bundle's own: M3** |
| Theme settings > Personalisation | box, opens a page | - | Its boxes | - | **Bundle's own: M3** |
| ... > Personalisation > Boot video | box, opens a picker | Off, Xbox (2001), Xbox 360 (2005), Xbox 360 Slim (2010), Xbox 360 Special Edition (2012), Xbox One (2013), Xbox One X (2017), Xbox Series S (2020), **Xbox Series X (2020)** | Which generation's boot animation plays at start; any button skips it | `StartUp`.BootVideo (C/ExtensionCore.Startup.cs:147-215, 337-384) | **Bundle's own: M5** (the videos are read from a media folder at run time, issue #10; with none the Boot video is skipped) |
| ... > Personalisation > Profiles | box, opens the Profiles editor | list of Profiles | Add, rename, picture, remove | `StartUp`.Profiles [{Id, Gamertag, RealName, Picture}], .LastProfileId | **Bundle's own: M5** |
| ... > Profiles > Add profile | action, text entry | gamertag | Adds a Profile | as above | **M5** |
| ... > Profiles > {Profile} > Gamertag | text entry | text | The Profile's gamertag | as above | **M5** |
| ... > Profiles > {Profile} > Real name | text entry | text | The Profile's real name | as above | **M5** |
| ... > Profiles > {Profile} > Picture | box, opens a picker | the Windows account picture, then the Gamerpics folder's images (the Bundle fills it with the 100 most liked Xbox 360 gamerpics from xboxgamer.pics, E/PopularGamerpics.cs; the user may add more) | The Profile's picture | as above | **M5**; fetching those gamerpics is open question 8 |
| ... > Profiles > {Profile} > Remove profile | action, with a prompt | Yes / No | Forgets the Profile; never the last one | as above | **M5**; the prompt becomes ours |
| ... > Personalisation > Who's playing today? | box, opens a two-row picker | **Ask every time**, Sign in the last profile | Whether Start-up and waking from sleep ask who is playing | `StartUp`.AskWhosPlaying | **Bundle's own: M5** |
| ... > Personalisation > Background delay | checkbox box | **on**, off | On: Home's background waits 1.5 s on a game before showing its art; off: 150 ms (home.md) | `StartUp`.BackgroundDelay, only once set | **Bundle's own: M3** |
| ... > Personalisation > Reduce motion | checkbox box | on, **off** | Section 10.4; in force at once (C/ExtensionCore.Startup.cs:175-180) | `StartUp`.ReduceMotion, only once set | **Bundle's own: M3** (`ui`'s `slide` already honours it, docs/ui.md) |
| ... > Personalisation > Show Play history | checkbox box | **on**, off | On: the Recent row is seven games and the Play history tile; off: eight games. The row is rebuilt at once | `StartUp`.ShowPlayHistory, only once set | **Bundle's own: M3** |
| ... > Personalisation > Show battery percentages | checkbox box | **on**, off | Whether the percent is written beside the controller's and the PC's battery | `StartUp`.ShowBatteryPercent, only once set | **Bundle's own: M3** |
| Theme settings > Idle | box, opens a page | - | The Idle screen's two times | - | **built (#128)** |
| ... > Idle > Dim after | dropdown | 1, **2**, 5, 10, 15, 30 min, 1, 2, 3, 4 h, Never | How long with no input before the screen dims | `Idle` as "00:02:00" or "Never" (C/ExtensionCore.Idle.cs:55, 255, 293); the port's `idle.dim_after` | **Bundle's own: built (#128)** |
| ... > Idle > Show things after | dropdown | the same steps not shorter than Dim after (**5 min**) | How long before the Idle screen shows things over the dim | `Idle`; the port's `idle.show_things_after` | **Bundle's own: built (#128)** |
| Theme settings > Guide | box, opens a page | - | - | - | **M3** |
| ... > Guide > Use the Guide | checkbox row | **on**, off | Section 6.8 | `Guide`.IsGuideOn, only once set (C/SettingsState.cs:19, 155-161) | **Bundle's own: M3** |
| ... > Guide > Now Playing source | dropdown | Any, **Spotify**, other apps seen | Which app the Audio flyout's Now Playing follows | `Guide`.NowPlayingSource | **Bundle's own: built (#129)** |
| Theme settings > Discord | box, opens a page | - | Connect one's own Discord application | - | **M6** |
| ... > Discord > Client ID: Type / Paste | text entry / action | text (**Not set**) | The application's Client ID | `Discord.ClientId` (C/ExtensionCore.Discord.cs:17-21) | **Bundle's own: M6** |
| ... > Discord > Client Secret: Type / Paste | text entry (dots) / action | text (**Not set**) | The application's Secret, never shown | `Discord.ClientSecret` | **Bundle's own: M6** |
| ... > Discord > Connect | action (dimmed when it can do nothing) | - | Connects; Discord asks for approval | `Discord.AccessToken`, `.RefreshToken`, `.ExpiresAt` | **Bundle's own: M6** |
| ... > Discord > Disconnect | action (dimmed with nothing to disconnect) | - | Closes the connection | the tokens | **Bundle's own: M6** |
| Playnite settings | rail category | - | What the Bundle does with Playnite itself | - | **Playnite's: the category goes**; section 14 says what takes its place |
| Playnite settings > Download metadata | action box (reads Cancel while running) | - | Metadata for every not-hidden game missing some, under Playnite's progress bar (C/ExtensionCore.LibraryUpdate.cs:68-89; E/PlayniteLibraryTasks.cs:187-214) | - | **Playnite's: replace** with our own "fetch missing art and details" action, **M4** |
| Playnite settings > Update after downloads | checkbox box | **on**, off | After a download finishes: wait 10 s of quiet, update every library, fetch metadata for the new games, then the Library updated Toast (C/ExtensionCore.LibraryUpdate.cs:28, 34, 95-109, 196) | `PlayniteSettings` (C/ExtensionCore.LibraryUpdate.cs:19, 49-61) | **Playnite's: drop** as a setting: the port's library re-reads the stores itself after an install (M4) (open question 5) |
| Playnite settings > Playnite settings | action box | - | Opens Playnite's Fullscreen Settings window (E/PlayniteHost.cs:333-359) | Playnite's own file | **Playnite's: replace** entry by entry (section 12.1) |
| More Playnite options | rail action | - | Opens Playnite's main menu | - | **Playnite's: replace** with the few actions worth keeping (section 12.2) |
| Debug menu and its pages | rail category (Debug builds) | - | Try Toasts, calls and Steam install trials | - | **drop** from the product; a developer aid at most (open question 9) |

---

## 8. Opening, closing and where B returns

### 8.1 Opening and closing

Both are **instant**: the page is simply there or gone (visibility bound to the state; any fly-in under way is stopped; E/SettingsControl.xaml.cs:175-181). Opening closes My games & apps and Search and re-reads the gamerpics (C/ExtensionCore.cs:1691-1704). Closing drops any text entry, closes any open dropdown and resets Theme settings and Personalisation to their boxes (C/ExtensionCore.cs:1838-1843; E/SettingsControl.xaml.cs:290-313), so **Settings remembers nothing between visits**: it always opens as the entry point says.

Settings is also closed, without B, by a game exiting back to Home, by the Guide's Home entry, and by opening My games & apps or Search (C/ExtensionCore.cs:648, 1545, 2120).

### 8.2 Entry points

| From | Opens on | Focus lands on | B, level by level |
|---|---|---|---|
| Home's top bar, Settings button (T/Views/Main.xaml:563-575; C/ExtensionCore.cs:1683) | Theme settings' boxes | the rail's **Theme settings** | rail -> Home, focus on the top bar's Settings button (E/SettingsControl.xaml.cs:862-880, 917-926) |
| The Guide's Settings entry (C/ExtensionCore.cs:2139-2144; the Guide closes, the Dashboard comes to the front) | the same | the rail's Theme settings | rail -> Home, its rows taking focus where they last had it |
| Tile menu, **Customise Home** (a Home tile) or **Customise games & apps** (a My games & apps tile) (C/ExtensionCore.TileMenu.cs:103-104; C/ExtensionCore.cs:1746-1748) | Theme settings > **Personalisation** | the **Boot video** box | Personalisation -> Theme settings' boxes (focus on Personalisation) -> rail -> back to the tile the menu was opened on: on Home, or My games & apps reopened on that game (C/ExtensionCore.cs:1815-1835) |
| Home's **Discord** tile, when there is no Discord tab to show or the Guide is off (C/ExtensionCore.cs:1710-1726; E/PromoRow.xaml.cs:65) (built in #143; until #144 the tile always comes here) | Theme settings > **Discord** | **Type** (Client ID) | Discord -> Theme settings' boxes (focus on Discord) -> rail -> the tile |
| Home's **Playnite settings** tile (C/ExtensionCore.cs:1729; E/PromoRow.xaml.cs:69) | the **Playnite settings** category | its **Playnite settings** box (E/SettingsControl.xaml.cs:887-911) | boxes -> rail -> the tile |

B never skips a level: an entry point that opens deep still climbs page, boxes, rail, out. "Opened in a pane" is forgotten as soon as focus leaves that first page (C/ExtensionCore.cs:1779-1786). CT/ThemeSettingsTests.cs:30-288 states these.

---

## 9. Navigation

D-pad and left stick move focus; A selects; B goes back. Auto-repeat on a held direction is Playnite's (UNVERIFIED, as home.md).

### 9.1 The rail (E/SettingsControl.xaml.cs:379-415)

| Button | Does |
|---|---|
| Up / Down | Previous / next item, passing over dividers; stops at the ends (no wrap). The category follows focus (section 2.3) |
| Right, or A, on Theme settings / Playnite settings / Debug menu | Into the category, on its **first box** always (Personalisation / Download metadata / Notifications): no memory of where focus was |
| A on More Playnite options | Opens Playnite's main menu. Right does nothing there |
| Left | Nothing |
| B | Leaves Settings (section 8.2) |

### 9.2 A pane of boxes (Theme settings, Personalisation, Playnite settings, Debug)

E/ThemeSettingsPane.xaml.cs:60-113; E/PersonalisationPane.xaml.cs:101-109, 318-351; E/PlaynitePane.xaml.cs:62-131.

| Button | Does |
|---|---|
| Up / Down | The row above / below, same column, clamped to that row's length (from Profiles down goes to Who's playing today?); stops at the top and bottom |
| Right | Next box in the row; stops at its end |
| Left | Previous box in the row; **from column 0, back one level** exactly as B |
| A | Opens the box's page, turns its checkbox over, or acts |
| B | Back one level: Theme settings' or Playnite settings' boxes -> the rail, on that category's item; Personalisation's boxes -> Theme settings' boxes, focus on **Personalisation**; a Debug page -> the page before, the Debug menu's own page -> the rail |

Stepping back from a page always puts focus on **the box that opened it**. In the Debug menu Left at column 0 goes back only on its menu page.

### 9.3 Personalisation's pages (Boot video, Who's playing today?, Profiles, a Profile, Picture) (E/PersonalisationPane.xaml.cs:264-315, 459-477)

Grids of 1, 1, 2, 2 and 4 columns. Left / Right within the row only: **Left at column 0 does nothing** (unlike 9.2). Up at the top does nothing. Down goes to min(here + columns, last): on the last row it **jumps to the last element** (a quirk, section 13.2). A chooses (rows, gamerpics) or opens (boxes). B steps back one page, to the box that opened it; the pages opened are kept as a stack (Personalisation > Profiles > a Profile > Picture is four B presses from Theme settings' boxes).

### 9.4 Idle and Guide (E/SettingsControl.xaml.cs:422-503)

Two controls each, top to bottom. Up / Down move between them, clamped. A opens the dropdown or turns the checkbox over. Right does nothing. **Left or B** returns to Theme settings' boxes, focus on that page's box. Each page opens with focus on its first control (E/SettingsControl.xaml.cs:235-255).

In an open list (E/SettingsControl.xaml.cs:510-541): Up / Down move, clamped; A chooses and closes; B closes without choosing; Left / Right do nothing. Either way focus returns to the dropdown box.

### 9.5 Discord (E/SettingsControl.xaml.cs:708-747)

Three rows of two: [Type, Paste], [Type, Paste], [Connect, Disconnect]. Up / Down keep the column; Left / Right within the row; Left from column 0, or B, returns to Theme settings' boxes with focus on Discord. Dimmed buttons take focus all the same.

### 9.6 Text entry over the page

Gamertag, Real name, Add profile and Discord's two Type buttons put the Console keyboard (search.md) over the page. While it is up every button is its own: B cancels the entry, not the page (E/SettingsControl.xaml.cs:711-714, 865). Kept or cancelled, focus returns to the button that opened it (to the new Profile's box after Add profile).

### 9.7 Every other button

| Button | On Settings |
|---|---|
| LB, RB, Y, right stick | Swallowed: nothing, and nothing behind the page acts (E/SettingsControl.xaml.cs:843-852) |
| X, Menu | Nothing |
| View | The Bundle handles nothing; whether Playnite's own binding on the view host fires (T/Views/Main.xaml:373) is UNVERIFIED |
| LT, RT | NOT FOUND |
| Guide | Opens the Guide over the page as anywhere (unless Use the Guide is off) |

### 9.8 Focus memory, in one place

None across visits (8.1). Within a visit: a page remembers which box opened it, and nothing else. Entering a category from the rail always lands on its first box; a picker opens on the choice now set; a dropdown list opens on the choice now set.

---

## 10. Motion, the hint bar and sounds

### 10.1 What moves

Only one thing: the category fly-in. Opening and closing the page, a box opening its page, stepping back, a dropdown opening, a checkbox turning, the info text showing, focus moving: all instant (E/SettingsControl.xaml.cs:191-195).

### 10.2 The category fly-in (E/PaneFlyIn.cs:20-47, 70-75, 195-229)

When focus moves **along the rail** to another category, the whole content host (header and pane together) is hidden at once, and two rendered frames later comes in: offset **50** vertically (from **below** when focus moved down the rail, from **above** when it moved up) sliding to rest in **120 ms**, cubic ease-out, while fading 0 -> 1 in 120 ms, quadratic ease-out. It does not play when Settings opens, nor on More Playnite options (nothing changes on the right).

### 10.3 What does not exist

No page transition, no cross-fade between pages, no focus-ring travel, no scroll animation (lists and scrollers jump), no press animation.

### 10.4 Reduce motion

On, the fly-in does not play: the category is simply there, **no slide and no fade** (E/Motion.cs:9-13, 47). (The port's `ui` layer makes `slide` instant and keeps fades under Reduce motion, docs/ui.md; here the Bundle drops both. Following the `ui` rule is fine: 120 ms of fade is not motion.) Elsewhere it shows pages, rows and tiles in place and holds Home's background still (the other pages' specs). Off by default; takes effect on the next animation.

### 10.5 The hint bar

**Never shows on Settings**, in any state: it needs a focused tile, grid or hub, and the focused tile is forgotten as the page opens (C/ExtensionCore.Hints.cs:74-91). There is no legend of A and B anywhere on the page.

### 10.6 Sounds

The extension plays nothing of its own on Settings (CT/ThemeSettingsTests.cs:120-127). What is heard is Playnite's: its `navigation.wav` as focus lands on any of the page's buttons (E/GameHubFocusSounds.cs:10-13; E/PlayniteSounds.cs:69-75), at Playnite's interface volume (E/PlayniteSounds.cs:130-158). Whether its `activation.wav` plays on A is UNVERIFIED (A reaches the page as a key press, not a click). No sound for B, for opening or closing, for a checkbox or a refused choice. The Guide's Settings entry plays the Guide's own Enter sound before the page shows (C/ExtensionCore.cs:2050).

---

## 11. Loading, empty and error states

### 11.1 Progress copy

While Playnite reports a library task (an update, Download metadata) and Settings is up, the page shows the same progress as Home, top right (T/Views/Main.xaml:405-441): a 60-tall strip, right edge at x 1815, top y 84: the status text (Segoe UI 26, `#B3FFFFFF`) then, 12 on, a 26 x 26 spinner. It overlays the header row; nothing else changes.

### 11.2 The rest

| Case | What shows |
|---|---|
| Settings itself loading | No such state: every value is in memory before the page can open |
| Download metadata while a library task already runs | Nothing: the press is ignored. While its own run goes the box reads Cancel; A or B then cancels it, focus staying on the box (C/ExtensionCore.LibraryUpdate.cs:68-89) |
| Show things after, with Dim after on Never | A list of one choice, Never |
| Now Playing source, nothing else seen this run | Any and Spotify alone |
| Profiles, one Profile | Its box and Add profile; its page has no Remove profile |
| Picture, no gamerpics | Playnite's message box (6.6); one or none to add: the hint line |
| Discord | Its status line (6.9); Connect and Disconnect dimmed but focusable (4.4) |
| Paste, clipboard empty / held by another program | The field is forgotten / nothing happens; no message |
| Playnite's Settings window or main menu fails to open | Nothing on screen; a log line only (E/PlayniteHost.cs:342-357) |
| A setting fails to save | NOT FOUND: no message is drawn |

Changes are logged as "[Settings] ..." lines (E/SettingsLog.cs:8-39, 72-94).

---

## 12. Playnite's own windows, as the Bundle restyles them

These are Playnite's windows, reached from Settings; the theme can only restyle them. **None of them comes over as it is.** They are recorded for what the port must replace.

### 12.1 Playnite's Fullscreen Settings window (T/Views/SettingsMenus.xaml)

Restyled to the same look as the Bundle's page, so the two read as one:

| Part | Value | Line |
|---|---|---|
| Page | `#1F1F1F`, whole screen; divider `#303030` at x 441..443 | 24-48 |
| Rail | 441 wide; title (Playnite's "Settings" string) Bahnschrift 30 at (75, 78); buttons 58 tall, 14 apart, name 75 in; focused `#107C10` with glow; mouse-over `#262626`; **disabled while a category is open** | 70-133 |
| Categories (fixed, six) | General, Visuals, Audio, Layout, Menus, Input | 282-305 |
| Category content | x 520..1750, y 170..1040 (CT/DashboardSettingsTests.cs:70-88) | 55-62 |
| Before a category is chosen | At (520, 72): gear U+E713 at 40 and "Settings" at 42 / weight 350; a 56 disc `#107C10` with U+E77B at 24 and the Windows user's name; the line "Choose a category to change Playnite's own options." | 135-166 |
| Toggle row | 56 tall; "On" / "Off" in dim text and a 50 x 26 pill switch (knob 12; `#107C10` when on); focused fill `#26FFFFFF` and a ring (`Margin="-6"`, 4 thick, radius 10) | 219-280 |
| Combo box, slider rows | 56 tall, control 360 wide | 202-212 |
| Combo box (T/DefaultControls/ComboBox.xaml:12-153) | 52 tall, the dropdown box's look (4.5); list 8 under it, items 52 + 4, tick U+E73E at 20 | - |
| Slider (T/DefaultControls/Slider.xaml) | 48 tall; 4-thick track `#66FFFFFF`, filled part `#2CA243`; 22 white knob | - |
| Check box (T/DefaultControls/CheckBox.xaml:10-69) | 26 square; `#107C10` with a tick when on | - |

Note the two looks for on/off: the Bundle's page uses a tick-in-a-square glyph (4.2), Playnite's window a pill switch. The port needs one; the Bundle's own is the one to keep.

Which entry sits in which category is compiled into Playnite and not in this repository (docs/research/fullscreen-theming-limits.md:116): **UNVERIFIED**. What Playnite saves is known from a captured `fullscreenConfig.json` (artifacts/e2e/achievements-20261110-105905/). By key, and what a standalone port needs instead:

| Playnite's keys | What they are | Port |
|---|---|---|
| InterfaceVolume (0.5), BackgroundVolume (0.3), MuteInBackground | Volume of interface sounds and background music | **Replace**: one "Interface volume" entry of our own (the sounds play from a media folder at run time, issue #10); background music: drop |
| Monitor, UsePrimaryDisplay | Which display | **Replace**: a display entry only if more than one display is ever supported; else drop |
| Theme, Rows, Columns, HorizontalLayout, ShowGameTitles, DarkenUninstalledGamesGrid, EnableMainBackgroundImage, MainBackgroundImageBlurAmount, MainBackgroundImageDarkAmount, FontSize, FontSizeSmall, ImageScalerMode, AsyncImageLoading, SmoothScrolling, ListNavigationSpeed, HideMouserCursor | Playnite's own layout and drawing | **Drop**: the Dashboard has one layout |
| ShowBattery, ShowClock, ShowBatteryPercentage | Playnite's own top bar | **Drop**: ours is Show battery percentages |
| ButtonPrompts | Glyph set for prompts | **Drop** unless other pads' glyphs are wanted |
| FilterSettings, ViewSettings, SelectedFilterPreset, ClearFiltersOnStartup | Library filters and sort | filters.md and my-games.md |
| EnableGameControllerSupport, DisabledGameControllers | Pads | **Drop**: the input layer's business |
| SwapConfirmCancelButtons, SwapStartDetailsAction | A/B and Menu/View swapped | Open question 10 |
| GuideButtonFocus, GuideButtonFocusOnlyWhenNoGameRunning | Guide button brings Playnite to the front | **Replaced** already by Use the Guide |
| MainMenuShow{Restart, Shutdown, Suspend, Hibernate, Minimize, Logout, Lock, Tools, Extensions, Clients} | Which main-menu rows show | **Drop**: power is in the Guide |
| MinimizeAfterGameStartup | Window behaviour on launch | **Drop**: the port decides |

Outside that file, Playnite's Settings also carries its language, library integrations (accounts for stores) and metadata sources: language is open question 3; store sign-in and sources are **M4**'s to design.

### 12.2 Playnite's main menu ("More Playnite options") (T/Views/MainMenu.xaml:9-158)

A right-hand panel `#2B2B2B`, title "Playnite" (Bahnschrift 28; margins 30, 48, 30, 24), rows at least 48 tall (margin 12, 2; padding 20, 0), a 22 px icon in a 24 column and 20 to the name. Rows, with Playnite deciding which show: Settings (U+E713), Update library (U+E895), Cancel update (U+E711), Exit Playnite (U+F3B1), Switch to Desktop (U+E7F4), Random game (U+E8B1), Shut down (U+E7E8), Suspend (U+E708), Hibernate (U+E823), Restart (U+E72C), Help (U+E897), Patreon / Ko-fi (U+EB51), Minimise (U+E921), Lock (U+E72E), Log out (U+E748), Clients (U+E7FC), Tools (U+E90F), Extensions (U+EA86).

Port: the power rows are the Guide's; Help, Patreon, Clients, Tools, Extensions, Switch to Desktop, Random game: drop. Worth keeping, as entries of our own (open question 4): **Update library** (re-read the stores now) and **Exit** (leave the Dashboard for the desktop).

### 12.3 Playnite's custom menus (T/Views/CustomMenus.xaml)

Game clients, software tools and extension commands: a 450-wide panel, radius 6, `#2B2B2B`, rows 72 tall, focused fill `#404040` with the ring, at most eleven rows (792). **Drop.**

---

## 13. Text rules, traps, and what the pictures confirmed

### 13.1 Text rules

| Text | Font, size, weight | Colour | Lines | Overflow |
|---|---|---|---|---|
| Rail title "Settings" | Bahnschrift 30 | `#FFFFFF` | 1 | character ellipsis |
| Rail item | Bahnschrift 24, weight 350 | `#FFFFFF` | 1 | ellipsis at 323 wide |
| Header | Bahnschrift 42, weight 350 | `#FFFFFF` | 1 | ellipsis at 1230 |
| Box name | Segoe UI 26 | `#FFFFFF` | 1 | ellipsis at 313 (333 in a checkbox box) |
| Row, dropdown value, choice, pane button, label | Segoe UI 26 | `#FFFFFF` | 1 | ellipsis |
| Info text beside a checkbox box | Segoe UI 26 | `#FFFFFF` | wraps at 400 | never trimmed |
| Descriptions, hints, "Not set", the Discord address | Segoe UI 26 | `#B3FFFFFF` | wrap at the pane's width | never trimmed |
| Icons | Segoe Fluent Icons (fallback Segoe MDL2 Assets): 34 in a box, 34 a box's checkbox, 28 a checkbox row's, 24 a Row's tick, 20 a choice's tick, 16 a chevron | `#FFFFFF` | - | - |

Everything is left-aligned except pane-button labels and the Discord address (centred). No text is upper-cased. Names are sentence case ("Show battery percentages"). Times read "2 min", "1 h", "Never". There is no second line on a box, no value preview, no "On"/"Off" word on the Bundle's page.

### 13.2 Small traps

1. Nothing on this page is scaled: do not apply Home's 1.114 to the ring, the glow or the type.
2. The Idle, Guide and Discord panes start at y **190**; panes of boxes at **210**.
3. The rail's focused fill is `#107C10`, the ring everywhere else `#2CA243`. A focused rail item has **no ring**.
4. The selected rail item's bar is on its **right** edge (x 435..441), against the divider.
5. More Playnite options takes focus without changing the category; the category before it keeps its Selected look.
6. Boxes and Rows have **square** corners under a rounded ring.
7. A box never shows its value. The only place a value shows before a page is opened is a checkbox box's glyph.
8. The info text is white, not muted, and shows only with focus: with the mouse or no focus, none shows.
9. Left from column 0 goes back in panes of boxes, Idle, Guide and Discord, but **not** in Personalisation's pages.
10. Down on the last row of a Personalisation page jumps to its last element (9.3); on Profiles with an odd count this moves focus sideways to Add profile.
11. The dropdown list overlays; it does not push Show things after down, and it is not modal: the page is not dimmed.
12. Show things after's list is rebuilt every time it opens; its length depends on Dim after.
13. The sixth Personalisation row overflows the screen and shifts when focused (6.2): a defect.
14. Entering a category always lands on its first box, even straight after leaving another.
15. Settings forgets everything on closing; an entry point decides where it opens.
16. Connect and Disconnect stay focusable when dimmed; only the label dims (0.4).
17. A setting takes effect at once; several are saved only once set.
18. E/SettingsBoxes.xaml:31 says the ring stands 3 off the box; the markup, and the pictures, say 4.

### 13.3 What the pictures confirmed

Measured in the 0.2.0 pictures (JPEG; colours within a few levels):

| Fact | Measured |
|---|---|
| Page colour | `#1F1F1F` |
| Rail, selected item | y 190..247, fill `#262626`, bar x 435..440 in `#107C10` |
| Rail divider | y 334..335, x 75..397 |
| Column divider | x 441..442 |
| Boxes | x 520..956 and 975..1411; y 210..340, 359..489, 508..638; `#2D2D2D` |
| Focused box | fill `#4A4A4A`; ring 4 thick with 4 of gap (y 202..205 over a box at 210), about `#379F4A` (JPEG of `#2CA243`) |
| Box icon and name | icon centre x 570; name from x 620 |
| Checkbox box | name from x 550; glyph x 899..933 (34 wide), with and without its tick |
| Info text | from x 1005, 400 wide, five lines (at 24 px, 32 pitch, in that build) |
| The focus shift | Background delay's box at about y 517.5 while focused in 0.2.0 (6.2) |
| B from the rail | Home, focus on the top bar's Settings button (`settings-2-after-b.jpg`) |

Not confirmable from any picture: type at 26; Theme settings' four boxes; Reduce motion, Show Play history and Show battery percentages; Idle, Guide, Discord, Playnite settings; a dropdown open or closed; Boot video, Who's playing today?, Profiles, Picture; the Debug menu's pages; the rail's focused (green) state; the fly-in.

---

## 14. What is Playnite-specific, and what to do about it

| Bundle | Why it is Playnite's | Port |
|---|---|---|
| The **Playnite settings** category, its three boxes and the tile on Home that opens it | It exists to reach Playnite | **Goes** (plan.md). The rail then has one category; open question 2 is its shape. Home's Playnite settings tile is home.md's to replace |
| **More Playnite options** | Playnite's main menu | **Replace** by our own entries for Update library and Exit, if wanted (12.2; open question 4) |
| Playnite's Fullscreen Settings window, main menu, custom menus | Playnite's windows | **Not ported.** Section 12.1's table lists the few options that need an entry of ours: interface volume, perhaps display, language, A/B swap (open questions 3, 10) |
| Download metadata; Update after downloads | Playnite's metadata and library plugins | **M4**: our own fetch of missing art and details; the library refreshing itself after an install (open question 5) |
| Wording: "Plays as Playnite starts in Fullscreen", "... as Playnite starts in Fullscreen and wakes from sleep", "Each puts up a Toast, as Playnite's own would" | Names the host | Reword: "Plays as the Dashboard starts. Any button skips it."; "Choose who's playing as the Dashboard starts and wakes from sleep." |
| Guide page's "Turn it off to leave the Guide button to Steam or another app." | True of the port too | Keep |
| The store: a DPAPI file named `discord-secrets.dat` in the extension's data folder | The extension had no settings file of its own that Playnite would not own | A plain settings file of the port's own for options; an encrypted store only for Discord's secrets and tokens (open question 6) |
| Message boxes (Remove profile, no gamerpics) | Playnite's dialog | Our own prompt, in the look of the port's other prompts (open question 12) |
| Sounds: Playnite's navigation sound on focus, at Playnite's interface volume | Playnite's files and setting | Decided in issue #10: the recordings never come into the repository; the exe plays them from a media folder at run time and is silent without them (ticket #28). The page must work silent, and needs a volume of our own (12.1) |
| The progress copy | Playnite's `ProgressActive` and status string | The port's own library-task state, same place and look (11.1) |
| Hosting as a theme element; focus handed between theme and extension with a delay (`FocusRailSoon`) | WPF and Playnite's focus | Not needed: the screen owns its focus (docs/ui.md) |
| Fonts | Segoe UI, Bahnschrift, Segoe Fluent Icons from Windows | The same, loaded from Windows at runtime, never bundled (CLAUDE.md) |
| Debug menu | Tools for the extension's Steam trials | Drop, or a developer-only page (open question 9) |
| The sixth row running off the screen | A WPF layout accident | Do not copy (open question 1) |

**A new entry the port needs: which Groups show on Home, and in what order** (dashboard issue #8). The Bundle has no such entry (its Home rows come from Playnite's filter presets). The entry itself, its page and its rules are designed in filters.md; here only its place: a box on **Theme settings > Personalisation**, which is where Customise Home lands, in the slot the Bundle leaves empty, column 1 of row 1 (x 975..1412, y 359..490), beside Who's playing today?. It opens a page in the boxes' place like any other box, B returning to it. Its name and icon are filters.md's (open question 7).

What M3 builds, then: the shell (rail, header, pane of boxes, pages, B levels, entry points), Theme settings with Personalisation's four checkbox boxes and the Guide page's Use the Guide, the Groups-on-Home entry, and the dropdown control (needed by Idle in M5 and likely by filters.md's page). Boxes whose feature belongs to M5 or M6 (Boot video, Profiles, Who's playing today?, Idle, Discord, Now Playing source) are either left out until their milestone or shown and inert: open question 2.

---

## 15. Open questions

Questions 1, 2, 6 and 13 are answered in section 16, with what else building the page (#56) settled.

1. **The sixth Personalisation row.** The Bundle's page runs off a 1080 screen and its last box shifts when focused (6.2). Options: scroll the pane (as Profiles does); put the checkbox boxes two wide and move the info elsewhere; or give every info text the row span. With the Groups entry added and M5's boxes possibly absent in M3, the page's rows change anyway: which layout?
2. **The rail with one category.** With Playnite settings, More Playnite options and the Debug menu gone, the rail holds only Theme settings. Keep a one-item rail (and the name "Theme settings", which names a Playnite theme), promote Personalisation, Idle, Guide and Discord to rail categories, or something else? And are boxes for features of later milestones shown before those milestones? **Decided (#194)**: four generic categories, General, Account, Games & apps and System, as the console's (16.1).
3. **Options that were Playnite's and have no entry of ours**: interface volume, display, language. Which exist in the port, and on which page?
4. **Update library and Exit.** They lived in Playnite's main menu. Do they become boxes on Settings, entries in the Guide, or neither?
5. **Update after downloads / Download metadata.** Confirm that the port refreshes its library after an install with no setting, and that fetching missing art is an M4 action (and where its box sits).
6. **Where settings are stored** in the port: file, format and location; whether anything other than Discord's secrets needs encrypting; whether "saved only once set" is kept.
7. **The Groups-on-Home entry** (issue #8): confirm its place on Personalisation (section 14), and its name. **Decided (#57)**: Personalisation, "Groups"; in M3 its first box (16.1).
8. **Gamerpics.** The Bundle downloads the 100 most liked Xbox 360 gamerpics from xboxgamer.pics into its Gamerpics folder. Does the port fetch them too, or offer only the Windows account picture and the user's own images? (M5.)
9. **Debug menu**: dropped, or a developer-only page behind a build flag?
10. **Swapped A/B and the View button.** Playnite offers a confirm/cancel swap; View's effect on this page is UNVERIFIED. Does the port offer the swap, and does View do anything on Settings?
11. **Sounds.** Whether A plays Playnite's activation sound here is UNVERIFIED; the sounds themselves are settled (issue #10: played from a media folder, silent without one), so what is open is only which events on Settings sound: focus moves alone, as Playnite's own source suggests (game-hub.md section 12), or A too.
12. **Prompts.** Remove profile and "no gamerpics" use Playnite's message box: what is the port's prompt? (M5; the same question arises in tile-menu.md for its confirmations.)
13. **A hint bar on Settings.** The Bundle shows none. Keep it so, or show A Select / B Back as on the other pages?
14. **Held D-pad repeat** on the rail and in lists is Playnite's and unmeasured (as home.md): the port uses its own repeat everywhere?
15. **Line counts** of the wrapped texts (6.2, 6.8) are estimates (one line either way), and the Discord page's exact y positions were not derived: to be checked against the Bundle on the TV PC when those pages are built.

---

## 16. Settings in the Dashboard

What `crates/screens/src/settings/` does that the sections above leave open, or does differently. Decided in #56.

### 16.1 The rail's categories and their boxes (#194)

The Bundle's rail has one category of its own, Theme settings, and the port had every box under it in the order the boxes came. There is no theme here: since #194 the rail lists **generic categories, as the console's Settings does** (General, Account, System, Devices & connections, Preferences, Accessibility on an Xbox Series X; the reference is a 1920 x 1080 capture of its General > TV & display options and General > Power mode & startup pages, kept out of the repository). Four, the ones the port has boxes for, in this order: **General**, **Account**, **Games & apps**, **System**. Nothing is named "Theme".

**Where it follows the Bundle**: the rail's drawing (2.1, 2.2: the title, items 441 x 58 at y 190 + 72 i, Bahnschrift 24 at weight 350, the focused item's green and glow, the selected item's lit fill and bar), the boxes (4.1, 4.2: 437 x 131, two columns at x 520 and 975, row pitch 149 from y 210), every page, and B one level at a time (8.2, 9.2). No dividers, there being nothing under the categories. The fly-in (10.2) is not built: categories change at once.

**Where it follows the console**:

- **The rail's Up and Down** go from category to category, stopping at the ends, and the category focus lands on shows beside the rail at once (as the Bundle's 2.3). Right or A goes into it; B leaves Settings; Left does nothing.
- **Each category keeps where its focus was** while Settings is up: back to the rail and into the category again is the box focus was on, not its first (the Bundle's 9.1 always goes to the first). A page opened and closed keeps it too. Nothing is kept between visits: Settings opens as its entry point says, every category on its first box (8.1).
- **The header** reads the category in **bold** (Bahnschrift 42, weight 700), then, where a page shows, the page's name 20 after it in the Bundle's weight 350: "**General** Idle", as the console's "**General** TV & display options". On a category's own boxes it is the category alone.
- **A category with more than a handful of boxes** has them in columns, each under a section title, as the console's pages: the title in Bahnschrift 30 at weight 400 (the rail title's) at (x of the column, 210), the column's boxes from 52 under it (y 262 + 149 i), the columns at the grid's x (520, 975). Down and Up go along a column, stopping at its ends; Right and Left go to the column beside, as far down as it goes (min of the place and its length); Left from the first column is B, as on rows. Only Games & apps has sections now; the others are rows of two.

| Category | Boxes | Layout |
|---|---|---|
| **General** | Personalisation (0, 0), Idle (1, 0), Guide (0, 1), **Boot video** (1, 1) | rows; the Bundle's Theme settings' first three at their places, Boot video in Discord's |
| **Account** | **Profiles** (0, 0), **Who's playing today?** (1, 0), **Discord** (0, 1) | rows |
| **Games & apps** | **Library**: Launchers, Games & apps, Groups, Art. **Installs**: Storage, Helpers, Set up this PC ([set-up.md](set-up.md), #178) | sections |
| **System** | System (0, 0), About (1, 0) | rows |

- **Personalisation** keeps its four checkbox boxes, a row each, and has a fifth of the port's own (#248): Background delay (y 210), Reduce motion (359), Show Play history (508), **Show launcher icons** (657), Show battery percentages (806). The new one is beside Show Play history, the other that changes the Recent row; its name says the user's word for the mark ("the game launcher icon"), "Show launcher on recent games" being cut short in the box, and its info says where: "Marks each of Home's recent games with the launcher it comes from, at the tile's foot. Turn it off to show the covers alone." Five rows fit the screen with every info text (the last, 806 + 8 + 172.9, ends at 987), so the pane does not scroll (16.2). Its boxes that opened pages went to their categories: Boot video to General, Profiles and Who's playing today? to Account, Groups to Games & apps.
- **Choices #194 made where the ticket did not say**: Games & apps (the box, games added by hand and Apps: apps.md 4) in Library, under Launchers, the ticket's "Recently added" being taken for it; Helpers under Storage in Installs, being what some stores install through; the System box (shell mode, the ticket's "Start-up as the shell") and About (with Updates, updates.md 2) in System. The sections are Library (where games come from, how Home groups them and their art) and Installs (where installs go and what installs them).
- **The focus log**: `Settings: Rail: General` (`Account`, `Games & apps`, `System`); on a category's boxes `Settings: General: Idle`, `Settings: Games & apps: Storage`; a page's lines are as they were (`Settings: Idle: Dim after`). The Games & apps box on its category reads `Settings: Games & apps: Games & apps`; its page's lines start `Settings: GamesAndApps:`, so they are never taken for the category's.
- **Playnite settings, More Playnite options and the Debug menu are not built** (section 14).
- **No hint bar** (open question 13): as the Bundle.

| Page | What it holds |
|---|---|
| Idle | "Dim after" and "Show things after", each a dropdown, as 6.7 (#128). |
| Guide | "Guide", its description and Use the Guide, and the Now Playing source under them (#129), as 6.8. Down and Up move between the two; A on the source opens its list on the source set now, and A in the list chooses, at once and saved (`guide.now_playing_source` in the settings table). The log: `Guide: Use the Guide`, `Guide: Now Playing source`, and with the list open `Guide: Now Playing source, list on "Spotify"`. |

### 16.2 A pane that would run off the screen scrolls

Open question 1. A pane of boxes in rows scrolls, as Profiles does: only as far as shows the focused box whole with its ring, and under a checkbox box the whole of its info text, 32 clear of the screen's foot; back down as focus goes up, until the first row is at its place again. It jumps: nothing on Settings scrolls with an animation (10.3). Scrolled, the boxes are cut 32 over the first row's place (y 178) and 32 left of the first column, so they pass under the header's row and a ring is never cut where it rests. The info text never moves its box: every one overlaps the empty second column, as the first three do in the Bundle. Personalisation, four rows since #194 and five since #248, fits the screen with every info text; the test pane of seven rows (`a_pane_of_boxes_that_runs_off_the_screen_scrolls_to_the_focused_box`) scrolls. A pane in sections does not scroll: its columns are four boxes at most.

The Bundle's picture 31 has the focused box's ring cut at the pane's left edge (x 520), which its picture 29 of Theme settings has not: a clipping accident of that pane's grid, not copied.

### 16.3 Entry points

| From | Opens on | Focus | B |
|---|---|---|---|
| Home's top bar button; the Guide's Settings (`Entry::Rail`) | General's boxes | the rail's General | Home: the button; from the Guide, Home's rows |
| Home's **Settings** wide tile (the Bundle's "Playnite settings" tile, whose category is gone; `Entry::Tile`) | General's boxes | its first box, Personalisation | the rail, then the tile |
| The Tile menu's Customise Home (`Entry::CustomiseHome`) | Games & apps' boxes | **Groups** (#57), the box that customises Home; the Bundle's is Personalisation's first box, Boot video | the rail on Games & apps, then the tile |
| Home's **Discord** tile with no Discord tab or the Guide off (`Entry::Discord`) | Account > Discord | Type | Account's boxes on Discord, the rail, then the tile |
| Manage Queue's row of a game stopped for want of space (`Entry::Storage`, #230; not the Bundle's) | Games & apps > Storage | its first folder | **straight back to the row** (the lead's decision on #235). The Bundle's own entries above keep 8.2's rule, every level climbed; a link from a place the Bundle has none goes back to that place |

Left from a pane's first column is B, on the boxes and on the Guide page. From the boxes to the rail and into the category again is the box focus was on (16.1).

Where a line the user reads names a place in Settings it names the category: the SteamGridDB Toast's "Check it in Settings, Games & apps, Art.", the update's "Install it in Settings > System > About.", the art picker's "in Settings > Games & apps > Art", the Idle screen's tips "Settings › Games & apps › Groups".

### 16.4 The settings store

Open question 6. Settings are rows of the library database's `settings` table, a key and a value, beside what the library keeps there of its own (`steam.user`, `steam.marks`). The core never sees the database: `screens::Options` is every option's value, typed, each with its default; the executable reads it at start-up (`Options::read`) and hands it to the Dashboard (`Dashboard::set_options`), `--screenshot` too where `--data-dir` names a database. A checkbox turned over is in force at once in the core, which asks for it to be saved with `Effect::Set(Change)`; the executable writes the row then, so a Dashboard that is killed has lost nothing.

| Option | Key | Default |
|---|---|---|
| Background delay | `home.background_delay` | on |
| Reduce motion | `motion.reduce` | off |
| Show Play history | `home.play_history` | on |
| Show launcher icons (#248) | `home.recent_launcher` | on |
| Show battery percentages | `status.battery_percent` | on |
| Use the Guide | `guide.enabled` | on |
| Download art online (16.8) | `art.online` | on |
| Art sources (16.8) | `art.order` | `steam,xbox,playstation,steamgriddb` |
| Boot video (start-up.md 2) | `startup.boot_video` | `XboxSeriesX`; a name, not `true` or `false`, saved by `Effect::BootVideo` |
| Idle > Dim after (6.7) | `idle.dim_after` | `120` (seconds, or `never`) |
| Idle > Show things after (6.7) | `idle.show_things_after` | `300` |
| Who's playing today? (start-up.md 9) | `startup.ask_whos_playing` | on: Ask every time; off is Sign in the last profile |
| About's Check for updates every day (updates.md 3) | `updates.daily_check` | off |
| Launchers > a launcher ([launchers.md](launchers.md) 3) | `launcher.<key>` (`launcher.steam`, `launcher.epic`, ...) | on; saved by `Effect::Launcher`, read by the library itself, never by `Options` |

The library keeps `launcher.<key>.found` (`installed`, `not installed`, `failed`) and `launcher.<key>.read` (Unix seconds) beside them, what the launcher's last read found and when: launchers.md, 2.

The same table keeps `profile.last`, the id of the Profile signed in last (profiles.md 1); the Profiles themselves are the `profiles` table's (profiles.md 2).

Values are `true` and `false` (`1`, `on`, `yes` and their opposites are read too). **Saved only once set** is kept: an option has no row until it is changed, so a later change of default reaches those who never chose. Nothing here is secret and nothing is encrypted; Discord's secret and tokens are M6's, and do not go in this table as they are. Every option is shared by every Profile. The log says `settings: <key> is <value>` for each option that is set as the Dashboard starts, and `settings: <key> = <value>` as one is saved.

### 16.5 What each option changes

| Option | In force |
|---|---|
| Background delay | Home's background waits 1.5 s or 150 ms on a Recent tile, from the next move of focus |
| Reduce motion | `ui`'s rule (docs/ui.md): what slides is in place, fades stay, the swirl holds still. From the next frame, in Home's window and on the Overlay |
| Show Play history | The Recent row is seven games and the Play history tile, or eight games, made again at once. Focus that was on the tile is on the game in its place, and the other way about. A library with no game keeps the tile, so the row is never empty |
| Show launcher icons | The launcher's mark at the foot of each of the Recent row's tiles (home.md, 3.3.1), from the next frame. Off, none of them carries it; the Groups' cards, My games & apps, the Guide and Manage Queue keep theirs |
| Show battery percentages | The figure beside the pad's battery and the PC's, on Home's top bar and in the Guide's Status area. Home's top bar draws a wireless pad's battery for it (home.md, 2.4): the cell, and the figure left of it |
| Use the Guide | As 6.8: the Guide button and a hold of it do nothing, nor Home's profile and Achievements tile, and the Discord tile opens Settings. A Guide that is open stays open until it is closed |

### 16.6 Sounds and the focus log

The Dashboard's one rule for every screen (docs/ui.md, "Sounds") holds here: focus landing on another control is the move sound, a level up or down included, as Playnite's navigation sound is in the Bundle; A that turns a checkbox over is the activation sound, where the Bundle's is unverified (open question 11); B out of Settings is silent.

`--log-focus` says `focus: Settings: ` and then the rail (`Rail: General`) or the pane and its control (`General: Guide`, `Personalisation: Reduce motion`, `Personalisation: Show launcher icons`, `Guide: Use the Guide`). A checkbox turned over leaves focus where it is and so says nothing there: its line is the `settings:` one above. A pane may add what a press changes without moving focus after a `; `, which is no move and makes no sound: the Groups page says whether the Group focus is on is shown (`Groups: Most played; shown`), a Group's page its name, count and the focused control's value (filters.md 6.2, 6.3), a Profile's page the Profile and the Picture page `worn` (profiles.md 5).

The Groups page's menu is a dialog the page shows over itself: while it is up `Page::dialog` says so, it has a dialog's sounds and the Guide puts it away (filters.md 6.2).

### 16.7 The controls, for the pages still to come

`settings::controls` has every control of section 4 as a function that draws it in a rectangle, with or without focus: `settings_box` (a Box with an icon, or a checkbox box with its info text), `row`, `pane_button`, `checkbox_row`, `label` and `description`; `Dropdown` (the box, and its list while open, with the list's own presses and scrolling); `Grid` (a pane of boxes' places, its steps and its scrolling); and `focus_ring` and `glow`. A page is a struct that implements `settings::Pane` (its header, its focus line, `press`, `draw`) and is opened by a box (`boxes::Item::Opens`) of the pane it belongs to; `boxes::Boxes` is a whole pane of boxes made from a list of them. The dropdown is on Guide's and Idle's pages; the Row and the pane button are on no page yet: `crates/screens/tests/settings.rs` builds a list of Rows from them, for their pictures.

### 16.8 Art (the port's own; #80)

The Bundle has no such page: Playnite's metadata sources fetched missing art (section 14, "Download metadata"). The port's is the user's own SteamGridDB key, a way to take away what was downloaded, a switch for all art online (#110), and the order of the online art sources, Playnite's by default (#246) (docs/sources.md, "Art"), and is decided here.

- **Its box**: **Art**, icon U+EB9F, the last of Games & apps' Library section (x 520..957, y 709..840), under Groups (16.1; #194). Right from it is Set up this PC, the foot of Installs (#178).
- **Its page** is laid out as Discord's (6.9), a 560..892-wide column from y 190:

| Element | y | Notes |
|---|---|---|
| "SteamGridDB key" (Text) | 190 | |
| "Art that no store has for a game comes from SteamGridDB, with your own API key: make one at steamgriddb.com, under Preferences and API. Only a game's store id or name is sent." (MutedText, wrapping at 892) | 228.58 | |
| The value, a 420-wide column at x 520 (Text): eight dots while a key is kept, whatever its length; "Not set" (MutedText) while none is. Then pane buttons **Type** (x 940..1120) and **Clear** (x 1140..1320), 58 tall | 30 under the description | Clear is dimmed (0.4) with no key, and focusable all the same (4.4) |
| "Downloaded art" (Text) | 40 under the buttons | |
| "Art and details downloaded for your games are kept on this PC. Clearing removes them; refreshing fetches them all again. A store's own art, and your games, play time and Groups, stay." (MutedText) | 4 under | #190 |
| Pane buttons **Clear downloaded art**, then **Refresh all art and details** 20 to its right (as Discord's Connect and Disconnect) | 18 under | each as wide as its label and 30 each side |
| The refresh's line (Text, one line, 892 wide): at rest "A refresh goes on in the background: Home can be used meanwhile." (MutedText); working, the Spinner (32, centred 16 in) and 48 in "Refreshing: 3 of 14 games..."; "Refresh paused: it goes on once Download art online is on."; "Refreshed: 9 pictures downloaded." (white) | 24 under the buttons, as Discord's status line | #190 |
| "Art online" (Text) | 40 under the refresh's line | #110 |
| "Downloads art for games that have none on this PC, from the game's own store and the sources below. Turn it off to use only what is on this PC." (MutedText) | 4 under | |
| Checkbox row **Download art online** (4.3's checkbox row, 560 wide, as Use the Guide's) | 18 under | on by default |
| "Art sources" (Text) | 40 under the checkbox | #246 |
| "Each picture a game lacks is looked for in this order, the first source that has it giving it. Only a game's name or store id is sent. A turns a source on or off; Menu moves it." (MutedText) | 4 under | |
| A Row each (Groups' Row, 16.1 and filters.md 6.2: 892 x 66, 74 apart) for **Steam**, **Xbox**, **PlayStation**, **SteamGridDB**, in the order kept, a tick (U+E73E) in the slot while it is on | 26 under | Playnite's order, all on, until changed |

The page scrolls, at once, as every Settings pane does (`controls::scrolled`), to keep the control with focus whole on screen: the sources' Rows run past the screen's foot.

- **Focus**: three rows, [Type, Clear], [Clear downloaded art, Refresh all art and details] and [Download art online], then the four sources' Rows; the page opens on Type. Up and Down go between the rows, onto a row's first (Down from Clear is Clear downloaded art; Up from either of the second row is Type; Down from Download art online is the first source, Up from it Download art online; Down stops at the last source), Left and Right along the first two; Left from the first column (a source's Row among them), or B, goes back to Games & apps' boxes on Art.
- **Art sources** (#246, `model::ArtOrder`), as Settings > Groups orders Groups (filters.md 6.2): **A** on a source turns it on or off, in force at once; **Menu** opens its menu, the Tile menu's panel over the dimmed page, titled with its name: **Move up** (U+E74A) but for the first, **Move down** (U+E74B) but for the last. A move is in force at once; the menu stays up with its rows as they are for the new place, focus on the same row if it is still there, else the first; focus on the page follows the source. B closes the menu. The hint bar shows Menu's "More options" on a source's Row, on or off, as on Settings > Groups' Rows: that is where the page says how a source is moved. **Each Row is numbered by its place in the order** (#262): the number of the sources that are on, 1 to 4, in Segoe UI 26 (`BODY`) white, right-aligned in a box 60 wide whose right edge is 33 in from the Row's (as far in as the tick is from the left), on the Row's middle; a move renumbers at once. A source that is off has no number and no tick, and those after it number on from the one before it (Steam 1, Xbox 2, PlayStation off, SteamGridDB 3): the numbers are the order the lookup asks in (`ArtOrder::number`). Each change is `Effect::ArtOrder`, saved as the settings row `art.order` (the sources' keys in order, `-` before one that is off: `steam,steamgriddb,xbox,-playstation`; a row that is garbled or misses a source is mended: unknown words passed over, the missing sources after the others and on), and starts a lookup under way again in the new order (docs/sources.md, "Art"). The lookup asks only the sources that are on, in that order, after what the PC has and the game's own store; the Art picker lists SteamGridDB's pictures only while SteamGridDB is on, and else says "Turn on SteamGridDB in Settings > Games & apps > Art's sources to choose from its pictures." Turning SteamGridDB on does not need a key, but nothing is asked of it without one.
- **Download art online** (`art.online` in the settings table, `Setting::DownloadArtOnline`; on until it is turned off, as every option is saved only once set): A turns it over, in force at once. Off, nothing is asked of any service, neither the stores' own art online nor SteamGridDB, and a lookup under way stops; the tiles keep what the stores' files and our cache have. On again, art is looked up at once. The key is kept either way.
- **Type** opens text entry over the page (caption "SteamGridDB key", from empty, shown as dots, 64 at most). A kept key is in force at once: saved, and art is looked up with it; an empty one kept leaves the key as it was. **Clear** forgets the key, and lookups stop.
- **Clear downloaded art** asks first, a prompt over the dimmed page as the Groups page's Delete asks (filters.md 6.2): "Clear downloaded art?", "The art downloaded for your games is removed from this PC. A store's own art stays.", **Clear** and **Cancel**, focus on Clear. Clear removes the data folder's `art` folder and nothing else; the tiles show the stores' own art, or their names, at once. B or Cancel does nothing. Art is not looked up again until the next start, or a new key.
- **Refresh all art and details** (#190) asks first, as Clear does: "Refresh all art and details?", "Everything downloaded for your games is removed and fetched again, in the background. Your games, play time, Groups and choices stay.", **Refresh** (icon U+E895) and **Cancel**, focus on Refresh. Refresh (`Effect::RefreshArt(None)`):
  - **Forgets everything fetched from outside** (`Database::forget_fetched`, `library::refresh`): the data folder's `art` folder (every picture downloaded, and every miss remembered, so nothing waits out its seven days), the achievements' icons (`achievements` folder; fetched again as they are wanted), and when each game's rarity was asked (asked again at once; the figures kept stand until then). **Never** the user's own or a store's: the games, play time and Play sessions, favourites, hidden games, Groups and pins, Profiles, settings, the SteamGridDB key, a store's own pictures on the PC. There are no other details fetched from outside: a game's name, dates, developers and scores come from its store's files, which Launchers' Update now reads again (launchers.md 4).
  - **Fetches again in the background**: the art lookup starts over for every game, most recently played first, as at a start (docs/sources.md, "Art": the game's own store, then the art sources in their order, SteamGridDB only with a key; its manners, two at a time, 250 ms between requests to a host, waiting when asked to wait), and the achievements are read again (`Ask::ReadAll`). Home and every page can be used meanwhile; tiles fill in as pictures come.
  - **Says how it goes** on the line under the buttons (above), and in the focus log after a `;` (below). Pressed while it works, Refresh does nothing (drawn dimmed, focusable all the same).
  - **Safe to interrupt, and goes on after a restart.** The settings row `art.refresh` is written before anything is forgotten and removed only when a lookup has looked at every game. A lookup cut short (Download art online turned off, SteamGridDB refusing the key, the Dashboard stopped or killed) leaves it: the line says "Refresh paused", and the next lookup (art online on again, the next start) is the refresh's, going on from where it stopped, since what came in is in the cache and what did not is looked up. The log says `art: refresh: under way since an earlier start` at such a start.
  - The log names no game: `art: refresh: forgot 120 pictures, the achievements' icons and 30 games' rarity`, `art: refresh: paused`, `art: refresh: done, 118 pictures downloaded` (the pictures of the lookup that finished it), and each lookup's own line.
- **One game's art** is refreshed from the Tile menu's More options, **Refresh art** (tile-menu.md 15): that game's folder of the cache goes (its pictures and misses), and the lookup starts over with that game first. Not a refresh of all: Settings' line is not changed by it. The log: `art: refresh: one game's art: forgot 4 pictures`.
- **A key SteamGridDB refuses** (401) is said once, as a Toast and in Notifications: "SteamGridDB didn't accept your key, so no art is being downloaded. Check it in Settings, Art." Nothing more is asked of it until another key is kept.
- **Stored**: `art.steamgriddb_key` in the settings table, as the other options; the core is told only that there is one (`Options::art_key`), never what it is, and the key is in no log, focus line or `Debug`. `art.steamgriddb_refused` says the service refused the key in it.
- **The focus log**: `Settings: Art: Type`, `Settings: Art: Clear`, `Settings: Art: Clear downloaded art`, `Settings: Art: Refresh all art and details`, `Settings: Art: Download art online`, on a source its Row among the four and, while it is on, the number its Row shows: `Settings: Art: Source Xbox, 2 of 4, on, number 2`, `Settings: Art: Source PlayStation, 3 of 4, off`, with its menu up `Settings: Art: Menu(Move up) of Xbox`, and with a question up `Settings: Art: Prompt(Clear) of Clear downloaded art`, `Settings: Art: Prompt(Refresh) of Refresh all art and details`. Once a refresh has been asked for, since the Dashboard started or at an earlier start, how it goes follows after a `;`, and changes without a press: `Settings: Art: Type; refreshing 3 of 14`, `...; refresh paused`, `...; refreshed: 9 pictures`. Turned over, Download art online says `settings: art.online = false` (or `true`); a change to the sources says `settings: art.order = steam,xbox,-playstation,steamgriddb`. Text entry's line shows the key as stars (`TextEntry: Q; "****|" lower letters`).

### 16.9 System (the port's own; #154)

The System category's first box, at (0, 0): **System** (icon U+E770), the page that turns shell mode on and off,
the Dashboard in place of the Windows desktop at sign-in. The Bundle set its Boot shell up from a terminal and has no
such page; the words, the layout and what each state shows are in shell-mode.md, section 2. One control, the checkbox
row **Start with Windows as the shell**: A asks before turning it on (Turn on, Cancel), turns it off at once, and does
nothing while it is refused. The focus log: `Settings: System: Start with Windows as the shell`, and with the question
up `Settings: System: Prompt(Turn on) of Start with Windows as the shell`; the executable's answer is
`shell: Settings > System shows on` (`off`, `refused <why>`, `failed`, `unknown`).

### 16.10 About (the port's own; #155)

The System category's second box, beside System: **About** (icon U+E946) at (1, 0) (x 975..1412, y 210..341). The Bundle has
no such page (Playnite's own About said Playnite's version). Right from System is About; Up and Down from it
do nothing. Its page is laid out as System's (shell-mode.md, 2), groups of a label and its text, 40
apart, from y 190; the second, **Updates**, is updates.md's (section 2), between the version and the licences:

| Label | Text |
|---|---|
| "Xbox Series Dashboard" | "Version 0.1.0 (0123abc)": the workspace's one version (`Cargo.toml`, `[workspace.package]`) and the commit the executable was built from, seven characters; "Version 0.1.0" alone where the build had no commit (a source archive) |
| "Updates" | where updates stand, Check for updates and Install, and the checkbox row Check for updates every day (updates.md, 2) |
| "Third-party licences" | "The Dashboard is built from open-source parts. Their licences are in THIRD-PARTY-LICENSES.txt, beside dashboard.exe." |
| "Sounds and boot videos" | "The console's sounds and boot videos don't come with the Dashboard. It plays them from the Xbox Series Theme for Playnite, where that is installed, or from the media folder in %LOCALAPPDATA%\XboxSeriesDashboard. The README beside dashboard.exe says how to fill it." |

- **Focus** is on Updates' three controls, from Check for updates (updates.md, 2). B, or Left from the first column,
  goes back to System's boxes on About.
- **Where the version comes from**: the executable's `BUILD` (`crates/app`: `CARGO_PKG_VERSION`, and the commit its
  build script reads, `DASHBOARD_COMMIT` from the environment where CI gives one, else `git rev-parse --short=7
  HEAD`), handed over in `Options::build`. The core's own default is the version with no commit; the screenshot test
  gives a made-up `1.2.3 (0123abc)`, so its picture never changes with a release.
- **The same anywhere else**: `dashboard --version` prints `dashboard 0.1.0 (0123abc)` and ends 0, and every start of
  the Dashboard says `dashboard 0.1.0 (0123abc)` in its log before it reads the library.
- **The focus log**: `Settings: About: Version 0.1.0 (0123abc); Check for updates; unchecked`, the line a test reads
  the version by (with
  `--screenshot --page settings --press down,down,down,right,right,a --log-focus`, the offscreen frame logs where
  its presses left focus).
