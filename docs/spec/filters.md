# Filters: what replaces Playnite's filter presets

Source for what the Bundle does: `~/projects/xbox-series-theme` at commit `f11e4f9` (theme 0.4.2, extension 0.4.2), read only. Source for what we have: `crates/library/src/lib.rs` and `crates/model/src/lib.rs` on `origin/main` at `dba0850`. The decision this designs is issue #8.

Path shorthand, as in [home.md](home.md): `T/` = `theme/XboxSeries.Dashboard/`, `E/` = `extension/XboxSeries.Extension/`, `C/` = `extension/XboxSeries.Core/`, `CT/` = `extension/XboxSeries.Core.Tests/`.

Sections 1 and 2 are **findings**: what the Bundle does, each with its source. Sections 3 to 8 are a **design**, ours, which no XAML backs: where it draws something it reuses a control another spec measures and says which. Nothing here changes how a Group or a tab looks; that is [home.md](home.md) section 6 and [my-games.md](my-games.md).

Marks: **NOT FOUND** = looked for and absent; **UNVERIFIED** = rests on something outside both repositories (Playnite's SDK, Steam's file formats).

---

## 1. What Playnite's filter presets gave the Bundle

A Playnite filter preset is a saved filter with a name, an optional sort and a flag, "show in Fullscreen quick selection". The user makes and flags them in Playnite's Desktop mode; the Bundle has no screen for it ("Users don't create Groups in the Bundle; they flag filter presets in Playnite", CONTEXT.md:43). The Bundle reads them into its snapshot as `FilterPreset { Id, Name, ShowInFullscreenQuickSelection, Games, Sort, SortDirection }` (C/LibrarySnapshot.cs:134-169).

### 1.1 Groups on Home

| What | Rule | Source |
|---|---|---|
| Which presets are Groups | Every preset with the flag set | C/ExtensionCore.cs:1443-1444 |
| Their order | Playnite's own order of presets (`MainView.GetSortedFilterPresets()`, the order the user gave them in Playnite), or the database's order if that can't be read yet | E/PlayniteLibrary.cs:41, 200-213 |
| A Group's name | The preset's name | C/ExtensionCore.cs:1446 |
| A Group's games | Whatever Playnite's own filter evaluation returns for the preset's settings (`Database.GetFilteredGames(p.Settings)`). Only flagged presets are evaluated; another preset's list is left empty | E/PlayniteLibrary.cs:39-48; C/LibrarySnapshot.cs:159-163 |
| A Group's sort | The preset's own sort and direction; with none, or one the core does not support, by name ascending | C/ExtensionCore.cs:1449-1450; E/PlayniteLibrary.cs:49-50, 216-229 |
| Sorts supported | Name, LastActivity, Added, Modified, ReleaseDate, Playtime, PlayCount | C/LibrarySnapshot.cs:171-181 |
| Sort rules | By name: the game's sorting name (its name when it has none), current culture, ignoring case. By another field: games without the field (never played, no release date) come last in either direction; ties in name order | C/GameListFilter.cs:34-62; C/LibrarySnapshot.cs:70 |
| How many show | The first 6, then See all, whose collage is the next 4 | C/ExtensionCore.cs:27, 33, 1472-1485 |
| With no preset flagged | One Group, "Favourites": favourite, not-hidden games by name ascending. It has no preset id. It shows even when empty | C/ExtensionCore.cs:1452-1460; CT/GroupTests.cs:151-203 |
| Flagging any preset | Replaces the Favourites Group: it is a fallback, not a Group beside the others | CT/GroupTests.cs:192-203 |
| A preset that matches nothing | Still a Group: its header and a See all card alone ([home.md](home.md) section 6.3). Whether a hidden game can be in a Group is the preset's own doing (Playnite's filter decides): UNVERIFIED | C/ExtensionCore.cs:1445-1450 |
| When Groups are rebuilt | With every library snapshot; the list is replaced only if a Group's name, preset, tiles or collage changed | C/ExtensionCore.cs:1422-1426, 1465-1469 |

### 1.2 Tabs on My games & apps' Full library

| What | Rule | Source |
|---|---|---|
| The tabs | Playnite's own `FilterPresetSelector`, restyled: one tab per preset flagged for Fullscreen quick selection, the same set and order as Home's Groups | T/CustomControls/FilterPresetSelector.xaml:1-4, 42-70 |
| Where | A 56-tall strip at `Margin="520,190,170,0"` of the page (x 520..1750, y 190..246), shown only on Full library | T/Views/Main.xaml:446-464 |
| A tab | 56 tall, text Segoe UI 26 white with 22 either side, 8 between tabs, radius 2, no fill. The active one: fill `#FF107C10` and the rectangular focus glow ([home.md](home.md) section 4.3). Tabs do not take focus themselves | T/CustomControls/FilterPresetSelector.xaml:9-40; T/Constants.xaml:9, 228 |
| Selecting one | Applies the preset to Playnite's main list: its filter and, if it has one, its sort. Playnite keeps it selected and lit | E/PlayniteHost.cs:710-715; C/IPlayniteHost.cs:85-89 |
| See all on a Group | Opens Full library with that Group's preset selected and its tab lit. The Favourites fallback has no preset: plain Full library | C/ExtensionCore.FullLibrary.cs:16-37 |
| With no tab selected | Full library shows "the user's own filter": whatever filter and sort the user last left on Playnite's Fullscreen list, kept aside while the Dashboard puts its own on the list and given back as the page goes | C/IPlayniteHost.cs:64-80; C/ExtensionCore.FullLibrary.cs:5-10 |
| Sort dropdown | Five options, in this order: "A–Z" (name ascending), "Last played", "Recently added", "Playtime", "Release date" (each descending). The user's choice stays as they switch tabs, overriding a preset's own sort, until the page closes. The dropdown reads "Sorted by <name>", or "Sort" while the list is in none of the five | C/MyGamesState.cs:144-166, 365-399; C/ExtensionCore.FullLibrary.cs:39-52, 90-109 |
| Name filter | Narrows the grid to names holding the typed text, on top of the tab; gone when the page moves to another nav item or closes | C/ExtensionCore.FullLibrary.cs:54-88, 111-124 |

How the tabs are stepped with the pad, how focus reaches them and what the strip looks like when it overflows are in [my-games.md](my-games.md).

### 1.3 The Dashboard's own two filters

Not presets, and not the user's: built by the extension and put on Playnite's list (C/GameListFilter.cs:13-17; E/PlayniteHost.cs:697-708).

| Filter | Games | Sort | Used by |
|---|---|---|---|
| `Installed` | Installed, not hidden | Name ascending | My games & apps' Games, and its count |
| `LastPlayed` | Every game not hidden | Last played descending | Play history and Search, so that any game there can be selected for its Game hub |

"Hidden games never match" either (C/GameListFilter.cs:8-9, 31-32).

### 1.4 What a preset could filter on

A Playnite preset's settings (`FilterPresetSettings`) are Playnite's, not the Bundle's: the Bundle passes them to Playnite whole and never reads a field but `IsInstalled`, which it sets itself (E/PlayniteHost.cs:700-702). From Playnite's SDK they cover the name, installed and uninstalled, hidden, favourite, and lists of genres, platforms, publishers, developers, categories, tags, series, regions, sources, age ratings, features, completion statuses, plus ranges for release year, the three scores, playtime, install size and the last-played, added and modified dates. **UNVERIFIED**: the SDK is not in the repository.

Of those, our library can answer only what section 3 lists. The rest need metadata no source gives us yet.

### 1.5 What goes away with Playnite

- The presets themselves, their flag and their order: there is no Desktop mode to make them in.
- "The user's own filter" of section 1.2: there is no Playnite list for the user to have left a filter on. Full library with no tab is simply every game.
- Sorting by Modified, and by PlayCount as Playnite counts it.
- Sorting by Release date, until the library holds one (section 3).

---

## 2. What issue #8 decided

> Both. A Group is a saved filter of our own, kept in the library database: a few built in (Favourites, Installed, Most played, Recently added), plus one for each Steam collection, read from Steam's files and kept in step with them. Settings chooses which show on Home and in what order. Full library's tabs are the same filters.

The rest of this file is the concrete form of that.

---

## 3. A filter

A **filter** is a name, a rule that picks games, a sort, a place in the order and whether it shows. A filter that shows is a **Group** on Home and a tab on Full library, as a flagged preset was both. "Filter" is the stored thing and the word in code; the user only ever reads "Group".

There are three kinds:

| Kind | Made by | Rule | Name | Can be deleted |
|---|---|---|---|---|
| `builtin` | A migration | Fixed (section 4) | Fixed | No: only shown or not |
| `collection` | The sync, one per Steam collection (section 5) | "the games in this collection", fixed | Steam's, follows a rename | No: it goes when Steam's collection goes |
| `custom` | The user, in Settings (section 6) | The user's | The user's | Yes |

### 3.1 The fields a rule can use

Every field is a column the library database already has (`games`, migration 1, `crates/library/src/lib.rs`) or a table this design adds. A rule is the **AND** of the clauses that are set; a clause left unset matches every game. There is no OR, no NOT and no nesting: a second Group covers what OR would.

| Clause | Values | Column | Notes |
|---|---|---|---|
| `installed` | unset / `true` / `false` | `games.installed` | |
| `favourite` | unset / `true` | `games.favourite` | "not a favourite" is not offered |
| `played` | unset / `"played"` / `"never"` | `games.last_played` (0 = never) | |
| `played_within_days` | unset / 7 / 30 / 90 / 365 | `games.last_played` against now | Implies played |
| `added_within_days` | unset / 7 / 30 / 90 / 365 | `games.added` against now | See the note on `added` below |
| `min_playtime_hours` | unset / 1 / 10 / 50 / 100 | `games.playtime_minutes` | |
| `sources` | unset / a list of source ids (`"steam"`, later `"xbox"`, `"epic"`, ...) | `games.source` | Any of the listed |
| `collection` | unset / a collection's row id | `collection_games` | Only `collection` filters set it; a custom rule does not offer it in M3 |

Always, and not a clause: `hidden = 0 AND gone = 0`. A hidden game is in no Group and no tab, as the snapshot already leaves it out (`Database::snapshot`).

Not available, and so not offered: genre, tag, developer, publisher, release date, age rating, platform, completion status, scores. They come, if at all, with M4's metadata; a rule is stored as JSON (section 7) so a later clause needs no migration.

**`added`** is when the game first came into our database, not when the user bought it. After the first sync every game has the same `added`, so "Recently added" starts as the whole library in name order and only becomes meaningful as games arrive. Steam's own purchase time is not read today (open question 3).

### 3.2 Sorts

| Sort | Column | Default direction | Bundle's name for it |
|---|---|---|---|
| `name` | `games.sort_name`, then `name`, then `id` | Ascending | "A–Z" |
| `last_played` | `games.last_played` | Descending | "Last played" |
| `added` | `games.added` | Descending | "Recently added" |
| `playtime` | `games.playtime_minutes` | Descending | "Playtime" |

Rules kept from the Bundle (C/GameListFilter.cs:34-62): a game without the field (never played: `last_played = 0`, `playtime_minutes = 0`) comes **last in either direction**; ties are broken by name ascending. Name order is by `sort_name` (lower case, no leading article), which stands in for Playnite's sorting name.

"Release date", the fifth option of Full library's Sort dropdown, has no column. Until one exists the dropdown has four options (open question 2). Modified and PlayCount are dropped.

### 3.3 Where a rule is evaluated

In the `library` crate, when it builds the snapshot. The screens are handed the result, never the rule, as the Bundle's core is handed `FilterPreset.Games`:

```rust
// model
pub struct Filter {
    pub id: i64,
    pub kind: FilterKind,          // Builtin, Collection, Custom
    pub name: String,
    pub shown: bool,
    pub sort: Sort,                // Name, LastPlayed, Added, Playtime
    pub descending: bool,
    /// Every game the rule picks, in the filter's sort: ids into `Library::games`.
    pub games: Vec<i64>,
}
pub struct Library { pub user: Option<String>, pub games: Vec<Game>, pub filters: Vec<Filter> }
```

`Library::filters` is in display order and holds **every** filter, shown or not (Settings lists them all); Home and Full library take those with `shown`. Home shows a Group's first 6 games and the next 4 in See all's collage; Full library shows all of them. Unlike the Bundle, every filter's games are evaluated, not only the shown ones: with one SQLite query each over a few hundred games that costs nothing, and Settings can then say how many games a Group holds.

`model::Game` gains `source: String` and `added: u64`, which the editor's count and the sorts need.

---

## 4. The built-in filters

Seeded by the migration, in this order. The first four are the ones issue #8 names.

| Key | Name | Rule | Sort | Shown at first |
|---|---|---|---|---|
| `favourites` | Favourites | `favourite: true` | Name ascending | Yes |
| `installed` | Installed | `installed: true` | Name ascending | No |
| `most-played` | Most played | `played: "played"` | Playtime descending | Yes |
| `recently-added` | Recently added | (no clause) | Added descending | Yes |

Steam's collections are placed after Favourites as they are first seen (section 5), so a first run reads: Favourites, the user's collections, Most played, Recently added.

Why Installed is not shown at first: My games & apps' Games is exactly that list (section 1.3), and Home's Recent row is mostly installed games. It exists so one press in Settings brings it to Home.

### 4.1 Empty Groups

The Bundle shows a flagged preset's Group even when it matches nothing, and shows the Favourites fallback even before anything is a favourite (section 1.1). Ours:

- A shown filter with **no games is left off Home** and off Full library's tabs. On a first run that is Favourites, and a collection whose games are all hidden or gone. It comes back by itself when it has a game. (This is what Home does today: [home.md](home.md) section 11.)
- If that leaves **no Group at all** (an empty library, or everything turned off), Home shows the Favourites Group empty, its header and a See all card alone: the Bundle's fallback, so the page under the wide tiles is never blank and Down from the wide tiles has somewhere to go.

This is a choice, not a finding (open question 1).

---

## 5. Steam collections

### 5.1 Where Steam keeps them

**UNVERIFIED, from knowledge of Steam's client, to be checked against a real install as the ticket's first step** (the repository's copy of the user's Steam files under `data/` was not read for this spec):

- `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json`: a JSON array of `[key, entry]` pairs. A collection's key is `user-collections.<collection id>`; its entry has `timestamp`, `is_deleted` (true for a collection the user removed, with no value) and `value`, itself a JSON string: `{"id": "uc-...", "name": "...", "added": [appid, ...], "removed": [appid, ...]}`.
- Two ids are Steam's own: `favorite` (its Favourites) and `hidden` (its hidden games).
- A **dynamic collection** has a `filterSpec` in its value and no complete `added` list: Steam works its games out in the client from store tags and features.
- The file is the client's cloud-synced copy and is written as Steam syncs, not at once. Older clients kept categories as `tags` under each app in `userdata/<id>/7/remote/sharedconfig.vdf`; nothing current writes it and it is not read.

### 5.2 How they map in

`source-steam` reads the file into `Library::collections: Vec<Collection { source_id, name, appids }>`; `Database::sync` merges them in the same transaction as the games:

| Steam has | The database does |
|---|---|
| A collection not seen before | Adds a `collections` row and a filter of kind `collection`: name = Steam's, rule = that collection, sort = name ascending, shown, placed after the last `collection` filter (or after Favourites if there is none) |
| A collection renamed | Renames the row and its filter |
| Games added or removed | Replaces the collection's `collection_games` rows. An appid with no row in `games` (a tool, DLC, a game no longer owned) is skipped |
| A collection deleted in Steam (`is_deleted`, or absent) | Deletes the `collections` row; its filter and its games go with it by cascade. The user's choice of shown and place for it goes too |
| A dynamic collection | Skipped: not listed at all (open question 4) |
| The `favorite` collection | Not a collection filter. On the **first** sync of each game only, a game in it gets `favourite = 1`; after that the flag is ours and Steam's later changes do not reach it (open question 5) |
| The `hidden` collection | The same, for `hidden = 1`, on the first sync of each game only |
| No file, or one that does not parse | Collections are left exactly as they are; nothing is deleted. A warning is logged |

What stays the user's across every sync: whether a collection's Group is shown, and where it is in the order. A sync never moves or re-shows one.

Other stores have nothing like collections that we read. A `collections` row carries its `source` so one can be added.

---

## 6. Making and editing Groups with a controller

Nothing of this exists in the Bundle (NOT FOUND: section 1). It is built only from controls the Bundle's Settings already has, so it needs no new drawing. Each is measured in [settings.md](settings.md) section 4, and this section says only what goes in them:

| Control | settings.md | The Bundle uses it for |
|---|---|---|
| Box (437 x 131, a line icon and a name, never a second line) | 4.1 | Personalisation, Boot video |
| Row (892 x 66, 74 pitch, a tick U+E73E in a slot at its left for the one set) | 4.3 | Boot video's choices |
| Pane button (58 tall, at least 180 wide) | 4.4 | Discord's Type and Paste |
| Dropdown box and its list (560 x 58; the list opens in the page under it) | 4.5, 4.6 | Idle's two times |
| Checkbox row (560 x 58, U+E739 / U+E73A at its right) | 4.7 | The Guide page's Use the Guide |
| Text entry on the Console keyboard, over the dimmed page | [search.md](search.md) section 10 | Gamertag, Client ID |
| The menu panel and its prompt | [tile-menu.md](tile-menu.md) sections 2 and 5 | The Tile menu, "Remove <game>?" |

Focus, the ring and glow, the fly-in, B's levels and sounds are those of the Settings page ([settings.md](settings.md) sections 5, 8 to 10).

### 6.1 Where

**Settings > Theme settings > Personalisation > Groups**: a Box on Personalisation in the slot the Bundle leaves empty, column 1 of row 1, beside Who's playing today? (x 975..1412, y 359..490; [settings.md](settings.md) sections 6.2 and 14). Icon U+E8FD (BulletedList), 34 px; name "Groups". Personalisation is where the Tile menu's Customise Home already lands (CONTEXT.md:47), so Customise Home on a Group's card reaches the page that edits Groups in two presses: Right, Down, A from Boot video.

### 6.2 The Groups page

Opens in Personalisation's boxes' place, as Boot video's page does, and is laid out as that page is ([settings.md](settings.md) section 6.3):

- y 210: "Choose which Groups show on Home and in Full library, and in what order." (MutedText, one line), 26 under it.
- Then one **Row** per filter, in order, x 520..1412, row i at y 270.58 + 74 i. The Row's name is the filter's name. **The tick means shown**: unlike Boot video's list, any number of Rows can be ticked.
- Last Row, always: **New Group**. In the tick's slot it has the glyph U+E710 (Add), icon font 24, white.
- Ten Rows fit with room for the last one's ring and glow (the tenth ends at 270.58 + 74 x 9 + 66 = 1002.58; an eleventh would end at 1076.58, its ring cut by the screen's foot). With more than ten, New Group included, the pane scrolls as the Profiles page does ([settings.md](settings.md) section 6.5): no scroll bar, it jumps, the focused Row kept whole with its ring.
- The page opens with focus on the first Row.

| Input | On a filter's Row | On New Group |
|---|---|---|
| A | Turns the tick over in place, saved at once; Home and Full library follow with the next snapshot | Makes a custom filter named "New Group" with no clauses, sorted by name, shown, last in the order (just above New Group), and opens its page (6.3) |
| Menu | Opens the Group's menu (below) | Nothing |
| X, Y, View, LB, RB | Nothing | Nothing |
| B | Back to Personalisation, focus on the Groups box | Same |
| Up / Down | The Row above or below; stops at the ends | |
| Left, Right | Nothing, as on Personalisation's other pages ([settings.md](settings.md) section 9) | |

The Group's menu is the Tile menu's panel ([tile-menu.md](tile-menu.md) section 2: 450 wide, centred over the `#99000000` dim, rows 72 tall, its title the filter's name), with these rows. A row that does not apply is absent, never dimmed, as in the Tile menu.

| Row | Glyph | Shown for | Does |
|---|---|---|---|
| Move up | U+E74A | Every filter but the first | Swaps its place with the one before. The menu stays open with focus on the row, so A again moves it further; when it reaches the top the row goes and focus drops to the next row |
| Move down | U+E74B | Every filter but the last | The same, downward |
| Edit | U+E70F | `custom` | Closes the menu and opens the filter's page (6.3) |
| Delete | U+E74D | `custom` | The panel is replaced in place by the prompt "Delete <name>?" with rows Delete and Cancel, focus on Delete, as "Remove <game>?" ([tile-menu.md](tile-menu.md) section 5.2). Delete removes the filter, closes the prompt and puts focus on the Row that takes its place |

B closes the menu, focus on the Row it was opened on (which may have moved). A built-in's or a collection's menu therefore holds only the two Move rows.

The Bundle's Settings page shows no hint bar ([settings.md](settings.md) section 10). This page alone shows one while a filter's Row has focus: the Menu hint, "More options" ([home.md](home.md) section 10.4), since nothing else on the page says the menu is there.

### 6.3 A Group's page (custom filters only)

Opens in the Groups page's place. It is a 560-wide column at x 520 from y 190, laid out as the Guide page is ([settings.md](settings.md) section 6.8): each control has its name above it (Text, 34.58 tall), the control 4 under the name, and 40 between one control and the next name, so the pitch is 34.58 + 4 + 58 + 40 = 136.58.

| # | Name above | Control | Values (default first) | Sets |
|---|---|---|---|---|
| 0 | the filter's name, then "` · {n} games`" in MutedText on the same line, following every change | Pane button "Rename" | Opens text entry, caption "Group name", from the current name; Start keeps, B drops. A name that is empty or all spaces is not kept. At most 40 characters | `name` |
| 1 | Installed | Dropdown | Any, Installed, Not installed | `installed` |
| 2 | Favourites | Checkbox row "Favourites only" | off, on | `favourite` |
| 3 | Played | Dropdown | Any, Played, Never played, In the last 7 days, In the last 30 days, In the last 90 days, In the last year | `played`, `played_within_days` |
| 4 | Added | Dropdown | Any time, In the last 7 days, In the last 30 days, In the last 90 days, In the last year | `added_within_days` |
| 5 | Time played | Dropdown | Any, 1 hour or more, 10 hours or more, 50 hours or more, 100 hours or more | `min_playtime_hours` |
| 6 | Store | Dropdown, present only when the library has games from more than one source | Any, then each source that has a game, by name | `sources` (one source; the stored list allows more later) |
| 7 | Sort | Dropdown | A–Z, Last played, Recently added, Playtime | `sort`, `descending` (the direction is each sort's default, 3.2) |

Control k's name is at y 190 + 136.58 k and its control at y 228.58 + 136.58 k. Seven controls reach y 1106.06 and eight 1242.64, past the screen's foot, so this pane scrolls too, as the Groups page does, keeping the focused control and an open list whole (a list is cut to the screen's foot less 32 and scrolls inside itself: [settings.md](settings.md) section 4.6).

- Up and Down move between the controls and stop at the ends; A opens a dropdown, turns the checkbox over, or opens text entry; Right does nothing.
- B, or Left as on the Guide page, goes back to the Groups page, focus on the filter's Row. There is no Save or Cancel: each control saves as it changes, as every setting of the Bundle's does ([settings.md](settings.md) section 7).
- The page opens with focus on Rename for a Group just made, and on Installed otherwise.

A built-in or collection filter has no page: Edit is not in its menu. Its rule and name are not the user's.

### 6.4 What is deliberately left out

Kept minimal on purpose; each can be added without changing the storage.

- Picking games one by one into a Group. That is what Favourites and Steam's collections are for.
- A rule on a collection inside a custom filter ("installed games of collection X").
- Choosing a sort's direction.
- Renaming or re-sorting a built-in or a collection's Group.
- A different set or order for Full library's tabs than for Home.

---

## 7. Storage

Migration 2 of the library database (`MIGRATIONS[1]`). The comment at the head of `crates/library/src/lib.rs` already names both tables.

```sql
-- A store's own grouping of games: a Steam collection.
CREATE TABLE collections (
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    -- The store's own id for it (Steam's 'uc-...').
    source_id TEXT NOT NULL,
    name TEXT NOT NULL,
    UNIQUE (source, source_id)
);
CREATE TABLE collection_games (
    collection INTEGER NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
    game INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    PRIMARY KEY (collection, game)
) WITHOUT ROWID;

-- A saved filter: a Group on Home and a tab on Full library while it is shown.
CREATE TABLE filters (
    id INTEGER PRIMARY KEY,
    -- builtin, collection or custom.
    kind TEXT NOT NULL,
    -- A builtin's key ('favourites', 'installed', 'most-played', 'recently-added'); NULL otherwise.
    key TEXT UNIQUE,
    -- A collection filter's collection; NULL otherwise. The filter goes with it.
    collection INTEGER UNIQUE REFERENCES collections (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- The rule, as JSON: an object of the clauses that are set. '{}' matches every game.
    rule TEXT NOT NULL DEFAULT '{}',
    -- name, last_played, added or playtime.
    sort TEXT NOT NULL DEFAULT 'name',
    descending INTEGER NOT NULL DEFAULT 0,
    shown INTEGER NOT NULL DEFAULT 1,
    -- Its place among the filters, lowest first. Not unique: ties fall back to id.
    position INTEGER NOT NULL
);

INSERT INTO filters (kind, key, name, rule, sort, descending, shown, position) VALUES
    ('builtin', 'favourites',     'Favourites',     '{"favourite":true}',  'name',     0, 1, 1000),
    ('builtin', 'installed',      'Installed',      '{"installed":true}',  'name',     0, 0, 2000),
    ('builtin', 'most-played',    'Most played',    '{"played":"played"}', 'playtime', 1, 1, 3000),
    ('builtin', 'recently-added', 'Recently added', '{}',                  'added',    1, 1, 4000);
```

A rule's JSON, every key optional:

```json
{
  "installed": true,
  "favourite": true,
  "played": "played",
  "played_within_days": 30,
  "added_within_days": 90,
  "min_playtime_hours": 10,
  "sources": ["steam"]
}
```

A collection filter's rule is `{}`: its `collection` column is its rule. A key the program does not know makes the filter match **nothing** and is logged, rather than being ignored and matching too much (a database from a newer version, opened by an older one, is refused anyway by `user_version`).

**Position.** Built-ins are seeded 1000 apart. A collection first seen takes the position midway between the last collection filter (or Favourites) and the filter after it; a new custom filter takes the highest position plus 1000. Move up and Move down swap two filters' positions. When two neighbours' positions are 1 apart and something must go between them, every filter is renumbered 1000 apart in the same transaction.

**API**, on `Database`, beside `set_favourite`:

```rust
pub fn filters(&self) -> Result<Vec<Filter>>;                      // every filter, in order, with its games
pub fn set_filter_shown(&self, filter: i64, shown: bool) -> Result<()>;
pub fn move_filter(&mut self, filter: i64, by: i32) -> Result<()>;  // -1 up, +1 down
pub fn add_filter(&mut self, name: &str) -> Result<i64>;            // custom, no clauses, shown, last
pub fn set_filter(&self, filter: i64, name: &str, rule: &Rule, sort: Sort, descending: bool) -> Result<()>; // custom only
pub fn delete_filter(&self, filter: i64) -> Result<()>;             // custom only
```

`snapshot()` fills `Library::filters` from `filters()`. Each of the setters is followed by a new snapshot to the core (`set_library`), which keeps focus on the game or box it was on.

---

## 8. What changes on the pages

Only what follows from the filters; everything else is the page's own spec.

| Page | With Playnite's presets | With filters |
|---|---|---|
| Home's Groups | Flagged presets in Playnite's order; Favourites as a fallback | `Library::filters` with `shown` and at least one game, in order; Favourites empty as the fallback (4.1). The stand-in Groups of [home.md](home.md) section 11 (Installed, Most played, worked out in the screen) are deleted |
| A Group's See all | Full library on the preset's tab; plain Full library for the fallback | Full library on that filter's tab, always: Favourites is a real filter now and has a tab |
| Full library's tabs | The flagged presets | The same filters as Home's Groups, same order |
| Full library with no tab lit | The user's own Playnite filter | Every game not hidden, by name ascending |
| Full library's Sort dropdown | Five options | Four, until there is a release date (3.2). A choice there overrides the tab's own sort until the page closes, as in the Bundle |
| The Tile menu's Add to / Remove from Favourites | Sets Playnite's flag; the Favourites fallback follows | `set_favourite`, then a new snapshot: the Favourites Group and tab follow at once |
| Settings | Nothing: presets are flagged in Playnite's Desktop mode | Personalisation's Groups box and its pages (section 6) |

---

## 9. What is Playnite-specific, and what to do about it

| Playnite's | Recommendation |
|---|---|
| Filter presets as the store of Groups and tabs | **Replace** with the `filters` table (section 7) |
| The "show in Fullscreen quick selection" flag | **Replace** with `filters.shown`, set in Settings |
| Playnite's order of presets | **Replace** with `filters.position`, set by Move up / Move down |
| Playnite's filter evaluation (`GetFilteredGames`) | **Replace** with our own over the eight clauses of 3.1 |
| The many metadata fields a preset can filter on (1.4) | **Drop** for M3. Revisit with M4's metadata: a JSON rule takes a new clause without a migration |
| "The user's own filter" on Full library, and the save/restore/release dance around it (C/IPlayniteHost.cs:64-80) | **Drop**: it exists only because the Bundle borrows Playnite's one list. Our grid is given its games directly |
| The extension's `Installed` and `LastPlayed` list filters (1.3) | **Drop** as filters: Games and Play history ask the snapshot for what they show. The `installed` built-in is the same list, should the user want it on Home |
| Sort by Modified, PlayCount | **Drop** |
| Sort by Release date | **Port later**: needs a release date in the library (open question 2) |
| Making presets in Playnite's Desktop mode | **Replace** with section 6 |

---

## 10. Open questions

1. **Empty Groups.** The Bundle shows a Group that matches nothing as a header and a lone See all card. This design leaves an empty filter off Home and off the tabs, except Favourites when nothing else would show (4.1). Is that right, or should a shown Group always show, as in the Bundle?
2. **Release date.** Full library's Sort dropdown has "Release date" in the Bundle. Steam's `appinfo.vdf` is believed to carry a release date for most games (UNVERIFIED: `source-steam` reads none today). Add `games.release_date` in M3 so the dropdown keeps five options, or leave it at four until M4's metadata?
3. **When a game was added.** `games.added` is when we first saw the game, so "Recently added" means little until the library has lived a while. Is Steam's purchase time worth finding (it is not in the files read today), or is this good enough?
4. **Dynamic collections.** Steam does not store their games. Skip them (this design), or evaluate the parts of a `filterSpec` we could (installed, played), which would show a Group that does not match Steam's?
5. **Steam's Favourites and hidden games.** Seed our `favourite` and `hidden` once per game from Steam's `favorite` and `hidden` collections (this design), keep them in step with Steam both ways, or ignore Steam's? Seeding once means a game un-favourited in Steam later stays a favourite here.
6. **One set for Home and the tabs.** In the Bundle one flag drives both. Should Full library's tabs instead list every filter, shown on Home or not, so a Group can be kept off Home but still be a tab?
7. **Where the Groups box sits.** Personalisation's empty slot beside Who's playing today? (6.1), because Customise Home lands on that page. Or a box of its own on Theme settings beside Personalisation, Idle, Guide and Discord? Personalisation is already too tall for the screen in the Bundle ([settings.md](settings.md) open question 1); the slot chosen adds no row.
8. **Collections' file.** Section 5.1's format is from knowledge of Steam's client, not from a file read for this spec. If the cloud-storage file proves stale or absent on the TV PC, the live copy is in the client's LevelDB (`htmlcache/Local Storage/leveldb`), which is far more work to read. Decide once the real file has been looked at.
9. **Reordering by Menu.** Move up and Move down in a menu is the least new drawing. The console itself reorders Groups by picking one up and moving it with the stick. Is the menu enough?
10. **Hidden games.** No filter ever holds a hidden game (3.1), so once the Tile menu's Hide this game is used there is nowhere to find the game and unhide it ([tile-menu.md](tile-menu.md) open question 4). Recommended: a fifth built-in, "Hidden" (`hidden: true`, the one rule that lifts the always-clause), never a Group on Home, offered only as Full library's last tab and only while something is hidden. It is left out of section 4 until this is decided, since it breaks the rule that one flag drives Home and the tabs alike.
11. **A hint bar on one Settings page.** The Groups page shows the Menu hint though the Bundle's Settings shows none (6.2). Keep the exception, or put a visible "..." affordance on the Row instead?
