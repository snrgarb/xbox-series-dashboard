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

There are four kinds:

| Kind | Made by | Rule | Name | Can be deleted |
|---|---|---|---|---|
| `builtin` | A migration | Fixed (section 4) | Fixed | No: only shown or not |
| `collection` | The sync, one per Steam collection (section 5) | "the games in this collection", fixed | Steam's, follows a rename | No: it goes when Steam's collection goes |
| `custom` | The user, in Settings (section 6) | The user's | The user's | Yes |
| `launcher` | The sync, one per launcher with games (section 4.2, #250) | `sources: [<the launcher>]`, fixed | The launcher's (Steam, Epic Games, ...) | No: it goes when the launcher has no game left |

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

Always, and not a clause: `hidden = 0 AND gone = 0`. A hidden game is in no Group and no tab, as the snapshot already leaves it out of its games (`Database::snapshot`). One rule lifts the first half: `hidden: true`, the built-in Hidden filter's (section 4), which picks the hidden games and only those. It is a key of the rule's JSON like the others, and the editor of section 6 does not offer it.

Each clause as built (`crates/library/src/filters.rs`): `played_within_days: n` is `last_played > 0` and no more than n x 86 400 seconds before now; `added_within_days` the same of `added`; `min_playtime_hours: n` is `playtime_minutes >= 60 n`; `favourite: false` and an empty `sources` are read as they stand (not a favourite; any source) though nothing writes them.

Not available, and so not offered: genre, tag, developer, publisher, release date, age rating, platform, completion status, scores. They come, if at all, with M4's metadata; a rule is stored as JSON (section 7) so a later clause needs no migration.

**`added`** is when the game first came into our database, not when the user bought it. After the first sync every game has the same `added`, so "Recently added" starts as the whole library in name order and only becomes meaningful as games arrive. Steam's own purchase time is not read (open question 3, decided: this is good enough).

### 3.2 Sorts

| Sort | Column | Default direction | Bundle's name for it |
|---|---|---|---|
| `name` | `games.sort_name`, then `name`, then `id` | Ascending | "A–Z" |
| `last_played` | `games.last_played` | Descending | "Last played" |
| `added` | `games.added` | Descending | "Recently added" |
| `playtime` | `games.playtime_minutes` | Descending | "Playtime" |

Rules kept from the Bundle (C/GameListFilter.cs:34-62): a game without the field (never played: `last_played = 0`, `playtime_minutes = 0`) comes **last in either direction**; ties are broken by name ascending, whichever way the sort goes. Every game has an `added`. Name order is by `sort_name` (lower case, no leading article), which stands in for Playnite's sorting name. A sort the database names that the program does not know is by name, as a preset's unsupported sort is in the Bundle (section 1.1).

"Release date", the fifth option of Full library's Sort dropdown, has no column. Until one exists the dropdown has four options (open question 2). Modified and PlayCount are dropped.

### 3.3 Where a rule is evaluated

In the `library` crate, when it builds the snapshot. The screens are handed the result, never the rule, as the Bundle's core is handed `FilterPreset.Games`:

```rust
// model
pub struct Filter {
    pub id: i64,
    pub kind: FilterKind,          // Builtin, Collection, Custom
    pub key: Option<String>,       // a built-in's: "favourites", "installed", "most-played", "recently-added", "hidden"
    pub name: String,
    pub shown: bool,
    pub rule: Rule,                // for Settings' editor to show; empty for a collection's
    pub sort: Sort,                // Name, LastPlayed, Added, Playtime
    pub descending: bool,
    /// Every game the rule picks, in the filter's sort: the games' ids, found with `Library::game`.
    pub games: Vec<i64>,
}
pub struct Library {
    pub user: Option<String>,
    pub games: Vec<Game>,          // not hidden
    pub hidden: Vec<Game>,         // the hidden ones: only the Hidden filter names them
    pub filters: Vec<Filter>,
    pub collections: Option<Vec<Collection>>, // a source's reading only (section 5.2)
}
```

`Library::filters` is in display order and holds **every** filter, shown or not (Settings lists them all); Home and Full library take those with `shown` and a game (`Library::groups()`). Home shows a Group's first 6 games and the next 4 in See all's collage; Full library shows all of them. Unlike the Bundle, every filter's games are evaluated, not only the shown ones: with one SQLite query each over a few hundred games that costs nothing, and Settings can then say how many games a Group holds.

The rule travels with the filter so that the editor of section 6.3 can show what is set; it is still only the library that works one out. `key` is how code knows a built-in whatever it comes to be called (Home's fallback, 4.1).

`model::Game` gains `source: String` and `added: u64`, which the editor's count and the sorts need, and `hidden: bool`: false for every game of `Library::games`, true for those of `Library::hidden`, and in a source's reading what the store says (section 5.2).

A Library that came from no database (`--library <file.json>`, the fake one, Steam read with nowhere to keep it) has no filters. `app` passes it through a database in memory (`store::with_filters`), so the built-ins and Steam's collections are Home's Groups there too; the screens' tests do the same.

---

## 4. The built-in filters

Seeded by the migration, in this order. The first four are the ones issue #8 names.

| Key | Name | Rule | Sort | Shown at first |
|---|---|---|---|---|
| `favourites` | Favourites | `favourite: true` | Name ascending | Yes |
| `installed` | Installed | `installed: true` | Name ascending | No |
| `most-played` | Most played | `played: "played"` | Playtime descending | Yes |
| `recently-added` | Recently added | (no clause) | Added descending | Yes |
| `hidden` | Hidden | `hidden: true` | Name ascending | No |

Steam's collections are placed after Favourites as they are first seen (section 5), those seen together in name order, so a first run reads: Favourites, the user's collections by name, Most played, Recently added.

Why Installed is not shown at first: My games & apps' Games is exactly that list (section 1.3), and Home's Recent row is mostly installed games. It exists so one press in Settings brings it to Home.

**Hidden** (decided, issue #49; open question 10) is how a hidden game is found again. It is a built-in like the others, last and not shown, and the user shows it from Settings > Groups as any other: then it is a Group on Home and a tab on Full library while a game is hidden, and gone again when none is (4.1). One flag still drives Home and the tabs alike. Its games are the snapshot's `Library::hidden`, which no other list reads. On Home a hidden game's card shows as any card does; A on it does nothing until the Game hub can open on it (ticket 4), since a launch names a game by its place in `Library::games`, where a hidden game has none ([home.md](home.md) section 11).

### 4.1 Empty Groups

The Bundle shows a flagged preset's Group even when it matches nothing, and shows the Favourites fallback even before anything is a favourite (section 1.1). Ours:

- A shown filter with **no games is left off Home** and off Full library's tabs. On a first run that is Favourites, and a collection whose games are all hidden or gone. It comes back by itself when it has a game. (This is what Home does today: [home.md](home.md) section 11.)
- If that leaves **no Group at all** (an empty library, or everything turned off), Home shows the Favourites Group empty, its header and a See all card alone: the Bundle's fallback, so the page under the wide tiles is never blank and Down from the wide tiles has somewhere to go. It is Favourites whether or not Favourites is shown, under the name the `favourites` filter has.

This is a choice, not a finding (open question 1), and it is decided so (issue #49).

### 4.2 Each launcher's whole library (#250)

The user (2026-11-18): "add full library filters for different game libraries to allow importing and downloading not installed games". So Full library has a tab for each launcher that has games, with that launcher's whole library, installed or not, from which a game that is not installed is installed by the routes that exist already: Install on its Tile menu and in its Game hub. Nothing in the Bundle backs it: Playnite's presets could filter by source, but the Bundle made none.

**What each launcher's tab can show is what its source reads** (`crates/source-*`, as of this ticket). A tab never lists a game its source does not list; where a source reads the installed games only, its tab is those, and says nothing of what else the account owns.

| Launcher | Games owned and not installed, read from | Without that | Install of one not installed |
|---|---|---|---|
| Steam | Yes: the apps the account's packages grant (`appcache/packageinfo.vdf`), else every app `localconfig.vdf` lists; games only (`appinfo.vdf` type `game`) | (always there with Steam) | Steam's client, over its debugger or `steam.exe +app_install` (sources.md) |
| Epic Games | Yes, once the launcher was signed in on the PC: its catalogue cache (`Data\Catalog\catcache.bin`), or the eglv2 launcher's library cache; and, with legendary added and signed in, legendary's `metadata\` (every game of the account it has listed) | Installed games only | The launcher's link (`?action=install`, its own dialog), or legendary where it is signed in |
| GOG | Yes, once Galaxy was signed in on the PC: Galaxy's database (`ProductPurchaseDates`, GOG's own release keys). Without it and with gogdl signed in, GOG's library service with gogdl's token, kept as `<data>\owned\gog.json` (#276) | Installed games only | Galaxy (`/command=installGame`) where it is installed, or gogdl where it is signed in |
| EA app | **No**: installed games only (the Uninstall keys). No local file lists what the account owns | | |
| Ubisoft Connect | Yes, once the client was signed in on the PC: its cache (`cache\configuration\configurations`) | Installed games only, named by their folders | `uplay://install/<id>` |
| Battle.net | **Yes**, from the client's own cache (`CachedData.db`'s licences run through the catalogue it keeps in `Cache`; #276), free to play ones among them; the catalogue's games not owned are not listed (docs/sources.md, "Which games can be installed") | Signed out, or no cache: installed and a built-in catalogue's others, whether the account owns one not known until the Agent answers its install (#172) | Its Agent, else the client's page |
| Xbox | **No**: the packages on the PC, and the PC Game Pass catalogue of the region Settings names (#277): its games not installed, ownership and subscription not known | Installed games only, with Show Game Pass games off | The Store's page (`ms-windows-store://pdp/?ProductId=<id>`) |

So the tab of EA app is its installed games (Battle.net's, its installed games and those it owns, or with no list of those the catalogue's; Xbox's, its installed games and the Game Pass catalogue's), the same as Games' for that launcher: it is there so that every launcher's library is one press away, and it gains games not installed if its source ever reads them.

**The Game Pass tab** (#277). Decided: **a tab, not a Group**, made as a launcher's is, of the `launcher` kind, its key `gamepass`, its name **Game Pass**, its rule `{"sources": ["xbox"], "game_pass": true}` (the clause `game_pass`, the column `games.game_pass`, migration 11; not offered by Settings' editor), its sort A–Z. It is the Xbox games of the PC Game Pass catalogue, installed or not. The library makes it while a game of the catalogue is in the library and removes it when none is (`filters::sync_game_pass`, after the launchers' filters: so, at first, after Xbox's tab); with Xbox switched off it is listed nowhere, as Xbox's own is; Show Game Pass games off removes the catalogue's games (they are marked gone, as a source's dropped games are) and so the tab. Like a launcher's, it is always a tab while it has a game, and a Group on Home only once ticked in Settings > Groups. Why not a Group by default: Home's Groups are the user's; Full library is where the whole libraries are. Search finds the catalogue's games as it finds every game of the library.

**Decided** (#250):

1. **A filter of its own kind**, `launcher`, one per launcher, its key `launcher.<source>` (`launcher.steam`), its name the launcher's as Settings > Launchers writes it (Steam, Epic Games, GOG, EA app, Ubisoft Connect, Battle.net, Xbox), its rule `{"sources": ["<source>"]}`, its sort A–Z. The rule is in the `filters` table like any other's; only the kind is new, so no migration (`launcher` is a kind's text, and a key is free text).
2. **Derived, not the user's.** The library makes one for each launcher that has a game its source still lists (hidden games count), and removes it when it has none: after every sync, and as the database opens (so a database from before this gains them). A new one is last in the order, not shown, those made together in the launchers' order. Its name follows the launcher's. Settings > Groups lists it as a Row like a built-in: its menu is Move up and Move down, never Edit or Delete (`set_filter`, `delete_filter` refuse it).
3. **Always a tab; a Group only when ticked.** Full library's tabs are the Groups (shown, with a game), and every launcher's filter that has a game whether shown or not, in the filters' order (`Library::tabs()`; Home keeps `Library::groups()`). So a launcher's tab comes after the tabs there were, at first; moving its Row in Settings moves its tab. Its tick in Settings > Groups is "a Group on Home too", not shown at first; See all on that Group lights its tab. This is the one exception to open question 6's "one set": a launcher's tab cannot be taken off Full library but by switching the launcher off.
4. **A launcher switched off keeps its filter**, as it keeps its games ([launchers.md](launchers.md) section 3, "Off hides, never deletes"): the row stays, with whether it is shown and its place, and is listed nowhere (not in Settings, not a tab, not a Group) until the launcher is on again, when it is back as it was. A move in Settings passes over it. A launcher whose source stops listing every game (uninstalled, or its last game gone) loses its filter, and the user's choice for it with it, as a Steam collection's goes.
5. **One launcher still has its tab.** With Steam alone the Steam tab lists what the page shows with no tab lit; it is kept, so that the rule is one and the user always finds a launcher's library in the same place.
6. **Hidden games** are not in it, as in no filter but Hidden (3.1). A launcher whose games are all hidden has a filter with no game, and so no tab (4.1).
7. **What a game not installed looks like there** is what it looks like on every Full library tab: the Download state badge at the tile's top left ([my-games.md](my-games.md) 5.3; not darkened, 17.9). An install under way has its Installing band, as anywhere. Nothing new is drawn.
8. **A game owned in two launchers** is two games ([../sources.md](../sources.md)), one in each launcher's tab. Which launcher Install asks when it is the same game is #249's, not this.

Tests: `crates/library/src/filters.rs` (made, named, by name, installed or not, hidden left out, a tab and not a Group, kept while off and back as it was, gone with its last game, made as an old database opens); `crates/screens/tests/my_games.rs` (the tabs stepped to the launchers', Install from a game not installed, a launcher switched off and on; the picture `my_games_full_library_launcher_tab`); `e2e/payloads/full-library/run.ps1` (the fakes' Steam and Epic Games tabs, and a Steam game not installed found on its tab and installed against the stand-in Steam).

---

## 5. Steam collections

### 5.1 Where Steam keeps them

**Still UNVERIFIED, from knowledge of Steam's client.** The ticket's first step was to check it against a real install, and it could not be done: the repository's copy of the user's Steam files under `data/` holds `appcache/`, `steamapps/` and `userdata/<id>/config/localconfig.vdf` and nothing else, so no `cloudstorage` folder. What that copy does show: `localconfig.vdf` carries no collections (its `WebStorage` block has the library's view state, which collection filter is ticked, and not the collections themselves), so it is not a second place to read them from. The reader (`crates/source-steam/src/collections.rs`) is written to the format below and to doubt it: a key or an entry it does not understand is passed over, an appid may be a number or a number in a string, a `value` may be JSON in a string or an object, and a file that is not a list at all reads as "no word", which changes nothing (5.2). The fixture writes the same format, so the tests prove the reader against the writer and not against Steam. **To do on the TV PC** (open question 8): look at the real file once, and at whether it lags the client.

The format:

- `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json`: a JSON array of `[key, entry]` pairs. A collection's key is `user-collections.<collection id>`; its entry has `timestamp`, `is_deleted` (true for a collection the user removed, with no value) and `value`, itself a JSON string: `{"id": "uc-...", "name": "...", "added": [appid, ...], "removed": [appid, ...]}`.
- Two ids are Steam's own: `favorite` (its Favourites) and `hidden` (its hidden games).
- A **dynamic collection** has a `filterSpec` in its value and no complete `added` list: Steam works its games out in the client from store tags and features.
- The file is the client's cloud-synced copy and is written as Steam syncs, not at once. Beside it the client is believed to keep `cloud-storage-namespace-1.modified.json`, the changes not yet sent; it is not read. Older clients kept categories as `tags` under each app in `userdata/<id>/7/remote/sharedconfig.vdf`; nothing current writes it and it is not read.
- The user whose file is read is the one whose `localconfig.vdf` was written last, as for the persona name and the play times.

### 5.2 How they map in

`source-steam` reads the file into `Library::collections: Option<Vec<Collection { source_id, name, appids }>>`, by name, and marks each game it read `favourite` or `hidden` if Steam's own two collections have it. `None` is "no word": no file, or one that does not parse. `Database::sync` merges them in the same transaction as the games:

| Steam has | The database does |
|---|---|
| A collection not seen before | Adds a `collections` row and a filter of kind `collection`: name = Steam's, rule = that collection, sort = name ascending, shown, placed after the last `collection` filter (or after Favourites if there is none) |
| A collection renamed | Renames the row and its filter |
| Games added or removed | Replaces the collection's `collection_games` rows. An appid with no row in `games` (a tool, DLC, a game no longer owned) is skipped |
| A collection deleted in Steam (`is_deleted`, or absent) | Deletes the `collections` row; its filter and its games go with it by cascade. The user's choice of shown and place for it goes too |
| A dynamic collection | Skipped: not listed at all (open question 4, decided) |
| A collection with no name | Skipped |
| The `favorite` collection | Not a collection filter. On the **first** sync of each game only, a game in it gets `favourite = 1`; after that the flag is ours and Steam's later changes do not reach it (open question 5, decided) |
| The `hidden` collection | The same, for `hidden = 1`, on the first sync of each game only |
| No file, or one that does not parse | Collections are left exactly as they are; nothing is deleted. A warning is logged (`steam: the collections were not read: ...`) |

What stays the user's across every sync: whether a collection's Group is shown, and where it is in the order. A sync never moves or re-shows one. A collection renamed in Steam keeps its filter's id, so a page open on it still names it.

**Games that were there first.** "The first sync of each game" would leave out every game already in the database when Steam could first say which are favourites: a database made before migration 2, or a first sync that found no collections file. So the first sync of a source that does carry collections also gives Steam's two marks to the games already there, once (the setting `<source>.marks` records that it has been done), only ever setting a mark and never clearing one. From then on only a game seen for the first time takes them.

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
| Text entry on the Console keyboard, over the dimmed page | [search.md](search.md) section 10 | Gamertag (the Client ID went with the RPC page, #299) |
| The menu panel and its prompt | [tile-menu.md](tile-menu.md) sections 2 and 5 | The Tile menu, "Remove <game>?" |

Focus, the ring and glow, the fly-in, B's levels and sounds are those of the Settings page ([settings.md](settings.md) sections 5, 8 to 10).

### 6.1 Where

**Settings > Games & apps > Groups**: a Box in the Games & apps category's Library section, under Games & apps and over Art ([settings.md](settings.md) 6.1; #194). Icon U+E8FD (BulletedList), 34 px; name "Groups". The Tile menu's Customise Home lands on it (CONTEXT.md:47 has it land on Personalisation, which held it until #194), so Customise Home on a Group's card reaches the page that edits Groups in one press: A.

**As built (#57).** Boot video, Profiles and Who's playing today? are M5's and not there yet, and the boxes that are close up row by row ([settings.md](settings.md) 16.1): Groups is Personalisation's **first box, alone on row 0** (520, 210), the four checkbox boxes under it from y 359, each still with a row to itself and the second column kept for its info. A box beside a checkbox box would be under its info text, so Groups has no other place in M3. Customise Home lands on it: one A from the Tile menu's Customise Home is the Groups page. When M5's boxes come, Groups goes to the Bundle's empty slot as above. Open question 7 is so decided.

### 6.2 The Groups page

Opens in Personalisation's boxes' place, as Boot video's page does, and is laid out as that page is ([settings.md](settings.md) section 6.3):

- No line over the Rows: "Choose which Groups show on Home and in Full library, and in what order." went in #261 (settings.md 16.11).
- One **Row** per filter, in order, x 520..1412, row i at y 210 + 74 i. The Row's name is the filter's name. **The tick means shown**: unlike Boot video's list, any number of Rows can be ticked.
- Last Row, always: **New Group**. In the tick's slot it has the glyph U+E710 (Add), icon font 24, white.
- Eleven Rows fit with room for the last one's ring and glow (the eleventh ends at 210 + 74 x 10 + 66 = 1016; a twelfth would end at 1090, past the screen's foot). With more than eleven, New Group included, the pane scrolls as the Profiles page does ([settings.md](settings.md) section 6.5): no scroll bar, it jumps, the focused Row kept whole with its ring.
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

B closes the menu, focus on the Row it was opened on (which may have moved). A built-in's, a collection's or a launcher's (4.2) menu therefore holds only the two Move rows. A launcher's Row's tick is whether it is a Group on Home: its tab on Full library is there either way.

**As built (#57).**

- The menu and its prompt are the Tile menu's: the same panel, `#99000000` dim over the whole screen (the rail too), the same fly-up and fly-down, the same sounds (a dialog's: PromptOpen, GuideMove, GuideSelect on a menu row, PromptClose on an answer and on B). The Row it was opened on keeps its focus and its ring under the dim, as a tile keeps its own under the Tile menu (the Bundle's picture 26). The Guide puts the menu away with no answer, and the Groups page is as it was.
- On the prompt, **Cancel and B go back to the menu**, focus on Delete, as "Remove <game>?" goes back to More options. Delete closes prompt and menu at once.
- A filter whose menu would have no row (the only filter there is, not a custom one) opens none.
- Every change is asked of the executable (`Effect::Filter`, `model::FilterChange`), which carries it out on the library database (`Database::change_filter`, by the setters of section 7) and gives the snapshot after; the page, Home's Groups and Full library's tabs are all drawn from that. The log says `library: filter <id> is shown`, `... is not shown`, `... moved by -1`, `... made: "New Group"`, `... set: "<name>"`, `... deleted`. A change the database refuses changes nothing and is logged.
- Focus stays on its filter wherever the snapshot puts it; a filter that has gone (deleted) leaves focus on the Row now in its place, the next filter or New Group.
- The hint bar shows More options while a filter's Row has focus, none on New Group and none while the menu is up. Open question 11 is so decided.
- Open question 9 is decided as recommended: the menu is enough.
- `--log-focus`: `focus: Settings: Groups: <name>; shown` (or `; not shown`), `Settings: Groups: New Group`, `Settings: Groups: Menu(Move up) of <name>`, `Settings: Groups: Prompt(Delete) of <name>`. With `--log-focus` the executable also says Home's Groups in order, `groups: Most played, Recently added`, with the first frame and whenever they change: how a test on the VM sees Home follow.

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

**As built (#57).** The header over the page is the filter's name, as a Profile's page has its gamertag ([settings.md](settings.md) section 3); it follows a rename. Rename's text entry keeps with Menu or the Enter key, as text entry does everywhere ([search.md](search.md) section 10); a name kept that is the same, empty or all spaces saves nothing. Each change saves the whole filter (`FilterChange::Set`: name, rule, sort, direction) and the count beside the name follows the snapshot. A store is named by its source id with a capital ("Steam"). The checkbox row reads "Favourites only" with the checkbox glyph of [settings.md](settings.md) 4.7. While a list is open the pane scrolls to show the box and as much of the list as three choices; the list itself is cut to the screen's foot as every list is. `--log-focus`: `Settings: Group: Installed; "Couch co-op", 4 games, Installed: Any`, the list open `Settings: Group: Installed > Not installed; ...`; Rename's state is the name and count alone.

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
    ('builtin', 'recently-added', 'Recently added', '{}',                  'added',    1, 1, 4000),
    ('builtin', 'hidden',         'Hidden',         '{"hidden":true}',     'name',     0, 0, 5000);
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

The built-in Hidden filter's rule has one key more, `"hidden": true` (3.1). A collection filter's rule is `{}`: its `collection` column is its rule. A key the program does not know, a value it does not know for one, or text that is no JSON object makes the filter match **nothing** and is logged, rather than being ignored and matching too much (a database from a newer version, opened by an older one, is refused anyway by `user_version`). The filter is still listed, with an empty rule.

**Position.** Built-ins are seeded 1000 apart. A collection first seen takes the position midway between the last collection filter (or Favourites) and the filter after it; a new custom filter takes the highest position plus 1000. Move up and Move down swap two filters' positions. When two neighbours' positions are 1 apart and something must go between them, or two that are to be swapped have the same position, every filter is renumbered 1000 apart in the same transaction.

**API**, on `Database`, beside `set_favourite` (`crates/library/src/filters.rs`):

```rust
pub fn filters(&self) -> Result<Vec<Filter>>;                      // every filter, in order, with its games
pub fn filters_at(&self, now: u64) -> Result<Vec<Filter>>;         // the same, the "within days" clauses counted from `now`
pub fn set_filter_shown(&self, filter: i64, shown: bool) -> Result<()>;
pub fn move_filter(&mut self, filter: i64, by: i32) -> Result<()>;  // -1 up, +1 down
pub fn add_filter(&mut self, name: &str) -> Result<i64>;            // custom, no clauses, shown, last
pub fn set_filter(&self, filter: i64, name: &str, rule: &Rule, sort: Sort, descending: bool) -> Result<()>; // custom only
pub fn delete_filter(&self, filter: i64) -> Result<()>;             // custom only
```

- `move_filter` counts places among **all** filters, shown or not, as Settings lists them; it stops at the ends, takes any `by` (the filters passed each move one place the other way), and is an error for an id that is no filter's.
- `set_filter` and `delete_filter` are an error for a built-in, a collection's filter or an id that is no filter's, and change nothing. `set_filter_shown` on an id that is no filter's does nothing, as `set_favourite` does.
- A name is kept without the spaces around it; an empty one is an error (`add_filter`, `set_filter`). The 40-character limit of 6.3 is the editor's.
- `model::Rule` is the clauses of 3.1 as a struct of options, `model::Sort` the four sorts with `Sort::default_descending()` for the direction 6.3 gives each.

`snapshot()` fills `Library::filters` from `filters()` (and `snapshot_at(now)` from `filters_at(now)`). A launcher's filter (kind `launcher`, key `launcher.<source>`, 4.2) is in the same table; `filters()` leaves out that of a launcher switched off, and `move_filter` counts places among the filters it lists. Each of the setters is followed by a new snapshot to the core (`set_library`), which keeps focus on the game or box it was on.

---

## 8. What changes on the pages

Only what follows from the filters; everything else is the page's own spec.

| Page | With Playnite's presets | With filters |
|---|---|---|
| Home's Groups | Flagged presets in Playnite's order; Favourites as a fallback | `Library::filters` with `shown` and at least one game, in order; Favourites empty as the fallback (4.1). The stand-in Groups of [home.md](home.md) section 11 (Installed, Most played, worked out in the screen) are deleted |
| A Group's See all | Full library on the preset's tab; plain Full library for the fallback | Full library on that filter's tab, always: Favourites is a real filter now and has a tab |
| Full library's tabs | The flagged presets | The same filters as Home's Groups, same order, and each launcher's whole library (4.2) |
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

Those marked **Decided** were settled with ticket 2 of M3 (issue #49), which built sections 3, 4, 5 and 7: by the issue where it says so, else as this spec recommended. The rest belong to the tickets that build what they ask about (6 and 10); 7, 9 and 11 were settled with ticket 10 (#57).

1. **Empty Groups.** The Bundle shows a Group that matches nothing as a header and a lone See all card. This design leaves an empty filter off Home and off the tabs, except Favourites when nothing else would show (4.1). Is that right, or should a shown Group always show, as in the Bundle? **Decided (issue #49): as this design.** Empty Groups are not shown on Home.
2. **Release date.** Full library's Sort dropdown has "Release date" in the Bundle. Steam's `appinfo.vdf` is believed to carry a release date for most games (UNVERIFIED: `source-steam` reads none today). Add `games.release_date` in M3 so the dropdown keeps five options, or leave it at four until M4's metadata?
3. **When a game was added.** `games.added` is when we first saw the game, so "Recently added" means little until the library has lived a while. Is Steam's purchase time worth finding (it is not in the files read today), or is this good enough? **Decided: good enough.** It is what this design does and what Playnite's own "Added" is (the day Playnite imported the game), so it is what the Bundle's "Recently added" meant too.
4. **Dynamic collections.** Steam does not store their games. Skip them (this design), or evaluate the parts of a `filterSpec` we could (installed, played), which would show a Group that does not match Steam's? **Decided: skipped**, as this design.
5. **Steam's Favourites and hidden games.** Seed our `favourite` and `hidden` once per game from Steam's `favorite` and `hidden` collections (this design), keep them in step with Steam both ways, or ignore Steam's? Seeding once means a game un-favourited in Steam later stays a favourite here. **Decided: seeded once**, as this design, with one addition so that games already in the database are not left out (5.2, "Games that were there first").
6. **One set for Home and the tabs.** In the Bundle one flag drives both. Should Full library's tabs instead list every filter, shown on Home or not, so a Group can be kept off Home but still be a tab? **Decided (issue #49): one set**, as in the Bundle. Since #250 the launchers' filters are the one exception: always a tab, a Group only when shown (4.2).
7. **Where the Groups box sits.** Personalisation's empty slot beside Who's playing today? (6.1), because Customise Home lands on that page. Or a box of its own on Theme settings beside Personalisation, Idle, Guide and Discord? Personalisation is already too tall for the screen in the Bundle ([settings.md](settings.md) open question 1); the slot chosen adds no row. **Decided (#57): Personalisation**, as recommended; until M5's boxes come it is that page's first box (6.1, "As built").
8. **Collections' file.** Section 5.1's format is from knowledge of Steam's client, not from a file read for this spec. If the cloud-storage file proves stale or absent on the TV PC, the live copy is in the client's LevelDB (`htmlcache/Local Storage/leveldb`), which is far more work to read. Decide once the real file has been looked at. **Still open.** Ticket 2 could not look: the copy of the user's Steam files under `data/` has no `cloudstorage` folder (5.1), and the TV PC is not to be touched without being asked. The reader is built to the format of 5.1 and changes nothing when it cannot read the file, so a wrong guess costs the collection Groups and nothing else. What settles it: one look at `userdata/<id>/config/cloudstorage/` on the TV PC (is the file there, is it this format, does a collection made in Steam a minute ago show in it), or a first run there that logs `steam: the collections were not read`.
9. **Reordering by Menu.** Move up and Move down in a menu is the least new drawing. The console itself reorders Groups by picking one up and moving it with the stick. Is the menu enough? **Decided (#57): the menu**, as designed.
10. **Hidden games.** No filter ever holds a hidden game (3.1), so once the Tile menu's Hide this game is used there is nowhere to find the game and unhide it ([tile-menu.md](tile-menu.md) open question 4). Recommended: a fifth built-in, "Hidden" (`hidden: true`, the one rule that lifts the always-clause), never a Group on Home, offered only as Full library's last tab and only while something is hidden. **Decided (issue #49), otherwise than recommended:** the fifth built-in, yes, but an ordinary one: not shown at first, and shown by the user from Settings > Groups, where it becomes a Group on Home and a tab alike (section 4). That keeps the rule that one flag drives both. Until ticket 10 builds that page there is no control that shows it.
11. **A hint bar on one Settings page.** The Groups page shows the Menu hint though the Bundle's Settings shows none (6.2). Keep the exception, or put a visible "..." affordance on the Row instead? **Decided (#57): the hint bar**, as designed, and only while a filter's Row has focus.
