# Import from Playnite, once

Ticket #157. Someone moving from the Bundle (Playnite with the Xbox Series Theme) to the Dashboard has years of play
time, favourites, hidden games and Groups in Playnite. The Dashboard can bring them over once, from Settings > Import
from Playnite, and offers to the first time it starts on a PC with a Playnite library. It only ever reads Playnite's
files, and an import run twice brings nothing the second time.

Built in `crates/playnite` (reading Playnite), `crates/library/src/import.rs` (carrying it out), the Settings page
`crates/screens/src/settings/import.rs` and the executable (`crates/app/src/store.rs`, `window.rs`). The plan is
`model::playnite::Plan`; the screens see only `model::playnite::Import`.

Sources are Playnite's own code at `JosefNemec/Playnite` master (version 10.64, `source/Playnite/Playnite.csproj`),
its library plugins at `JosefNemec/PlayniteExtensions` master, LiteDB's at `mbdavid/LiteDB` (tag v4.1.4, and the v5
branch's `FileReaderV7`), and the Bundle at `~/projects/xbox-series-theme`. Paths below are within those.

## 1. Where Playnite keeps its library

| What | Where | Source |
|---|---|---|
| The user data folder, installed | `%APPDATA%\Playnite` | `source/Playnite/Settings/PlaynitePaths.cs`: `UpdateUserDataDir(IsPortable ? ProgramPath : Path.Combine(ApplicationData, "Playnite"))` |
| The program, installed | `%LOCALAPPDATA%\Playnite` (the installer's default) | Playnite's installer |
| Portable | The data folder is the program's folder | `PlaynitePaths.cs`: `IsPortable` is true when there is no `unins000.exe` beside the program |
| The library | `<data folder>\library`, unless `config.json`'s `DatabasePath` says otherwise | `source/Playnite/Settings/PlayniteSettings.cs`, `DatabasePath`; `GameDatabase.GetFullDbPath` expands `%AppData%` and `{PlayniteDir}` |
| The games | `library\games.db` | `source/Playnite/Database/GameDatabase.cs`: one LiteDB file a collection, named after it |
| The filter presets | `library\filterpresets.db` | `source/Playnite/Database/Collections/FilterPresetsCollection.cs` |

`playnite::find` looks, in order: the folder given (`--playnite <dir>`), as the library itself if it has `games.db`,
else as a data folder (its `config.json`'s `DatabasePath`, else its `library`); with none given, on Windows,
`%APPDATA%\Playnite`, then `%LOCALAPPDATA%\Playnite` (a portable copy unpacked there). A portable install anywhere
else is not found by itself: `--playnite <its folder>` reads it, and Settings says "No Playnite library was found".
Not searching the disk for one is deliberate: a guess is slow and can find someone else's.

## 2. The file format, and reading it read-only

Playnite 10 uses **LiteDB 4.1.4** (`Playnite.csproj`: `<PackageReference Include="LiteDB" Version="4.1.4" />`), whose
files are **format version 7** (LiteDB 5 writes 8). It opens every file `Mode=Exclusive;Cache Size=0`
(`source/Playnite/Database/Collections/ItemCollection.cs`), so:

- **While Playnite runs, its files cannot be opened**: Windows refuses with a sharing violation (os error 32). The
  Dashboard says "Playnite has its library open. Close Playnite, then try again." and does nothing.
- **A `<name>-journal.db` beside a file** means a write Playnite did not finish. LiteDB 4 would replay it on the next
  open; the Dashboard does not write, so it refuses ("Playnite stopped while it was writing its library. Start
  Playnite and close it again, then try again.") rather than read a half-written file.
- Reading takes nothing else: the file is read whole into memory and never opened for writing. No lock file is made.

The format, from LiteDB 5's reader of version 7 files, `FileReaderV7`, as Playnite itself carries a copy of it
(`source/Playnite/Database/Collections/LiteDBFileReaderV7.cs`, used to rescue damaged files), and checked against
files LiteDB 4.1.4 wrote (section 3):

- 4096-byte pages, each with a 25-byte header: page id (u32), type (u8: 1 header, 2 collection, 3 index, 4 data, 5
  extend), previous and next page (u32 each), item count (u16), free bytes (u16), 8 reserved.
- Page 0: `** This is a LiteDB file **`, the version byte (7), the change id, the free list, the last page, the user
  version, a 20-byte password hash and 16-byte salt (all zero unless encrypted: Playnite never sets a password), then
  the collections by name, each with its collection page.
- A collection page: its name, document count, free data page, and 16 index slots; slot 0 is `_id`. Each has its
  head and tail node's address (page, index).
- An index node: its levels, the key (a BSON value with LiteDB's own type numbers), the address of the document's
  first data block, and for each level the previous and next node. Level 0 is every document in key order.
- A data block: its extend page, its length, and the bytes; a document longer than its block goes on in the extend
  pages, each holding `item count` bytes.
- Documents are BSON as the standard has it; a `Guid` is binary subtype 4 in .NET's byte order, a `DateTime` is UTC
  milliseconds.

`playnite::litedb` reads the header, then each collection's `_id` index along level 0 from its head node, each
document's data block and its extend pages. It refuses a file that is not LiteDB, a version other than 7 (so a future
Playnite on LiteDB 5 is said to be unreadable, never misread), an encrypted one, and any chain that loops or points
outside the file.

## 3. A Rust reader

**None that is maintained was found** for LiteDB 4 (format 7). On crates.io (searched November 2026) the one LiteDB
crate is `vrc-get-litedb` (0.3.0-beta), "a Rust reimplementation of LiteDB made for vrc-get": made for the VRChat
Creator Companion's LiteDB 5 files (format 8), in beta, and for one program's needs. The Dashboard has its own, `crates/playnite` (`bson.rs`, about 300
lines, and `litedb.rs`, about 350 with a writer for tests), with no dependency but `anyhow` and `serde_json`.

**How sure I am about real files: fairly, not wholly.**

- It follows LiteDB's own reader of version 7 files step for step, and was checked against files the **real LiteDB
  4.1.4** wrote with **Playnite's own models and mapper settings** (`tools/playnite-litedb`, a .NET tool using the
  LiteDB 4.1.4 and PlayniteSDK 6.18 packages): 64 games, one 9600 characters long across extend pages, one deleted,
  two stores, presets and their order (`crates/playnite/tests/litedb4/`, `tests/litedb4.rs`). Every value read is
  what was written.
- The other way round, what the Dashboard's writer makes (the made-up library the tests and `standin make-playnite`
  use) was read by the real LiteDB 4.1.4, which found every document by its id, inserted, updated and deleted in it,
  and read the presets' order back (`tools/playnite-litedb dump` and `touch`).
- **Not checked: a library Playnite itself wrote over years of use.** A file that has had thousands of updates and
  deletes has free lists, reused pages and index nodes spread across many pages, which the made-up files have only
  a little of. Nothing in the format differs; but that is where a reader's mistake would show. The rare corruption
  Playnite's own comment describes (a hard reset while it is open) would make the reader refuse, not misread.
- **What a human should check**, on a PC with a real Playnite library (Playnite closed): Settings > Import from
  Playnite, and see that the page's count of games is Playnite's (its library view's total, hidden ones included),
  and that "N of them in your library" and the preview's play time agree with a few games looked up in Playnite.
  Nothing is changed by looking. The log has every line (`import: ...`).

## 4. The fields

Game (`source/PlayniteSDK/Models/Game.cs`), as Playnite's mapper writes it (`BsonMapper` with
`SerializeNullValues = false`, enums as strings):

| Field | BSON | What the Dashboard does with it |
|---|---|---|
| `_id` | Guid | The key it is read by; not kept |
| `PluginId` | Guid | The store, by the library plugin's id (below). `Guid.Empty` is a game added by hand: left |
| `GameId` | String | The store's own id: Steam's appid, Epic's AppName, GOG's product id. Matched without regard to case |
| `Name` | String | For the log and the page, **never to match by** |
| `Playtime` | Int64 | Seconds; whole minutes in the Dashboard |
| `LastActivity` | DateTime, or absent | Last played; absent is never |
| `Favorite`, `Hidden` | Boolean | A favourite, hidden |
| `IsInstalled` | Boolean | For a preset that filters by it |
| `CategoryIds`, `TagIds`, `GenreIds`, `FeatureIds`, `PlatformIds`, `DeveloperIds`, `PublisherIds`, `SeriesIds`, `AgeRatingIds`, `RegionIds` | Array of Guid | For a preset that filters by them |
| `SourceId`, `CompletionStatusId` | Guid | For a preset that filters by them |

Library plugins (each plugin's `extension.yaml` and `<Name>Library.cs` in PlayniteExtensions):

| Store | Plugin id | `GameId` | The Dashboard's `source` |
|---|---|---|---|
| Steam | `CB91DFC9-B977-43BF-8E70-55F46E410FAB` | appid | `steam` |
| Epic | `00000002-DBD1-46C6-B5D0-B1BA559D10E4` | AppName | `epic` |
| GOG | `AEBE8B7C-6DC3-4A66-AF31-E7375C6B5E9E` | product id | `gog` |
| Ubisoft Connect (Uplay) | `C2F038E5-8B92-4877-91F1-DA9094155FC5` | uplay id | `ubisoft` |
| Battle.net | `E3C26A3D-D695-4CB7-A769-5FF7612C7EDD` | product code (`WoW`, `D3`) | `battlenet` |
| Xbox | `7E4FBB5E-2AE3-48D4-8BA0-6B30E7A4E287` | package family name | `xbox` |

Amazon, itch.io, Humble, Rockstar and every third-party plugin have no store in the Dashboard: their games are
counted as "of other stores" and left. **EA has no built-in plugin in Playnite 10** (Origin's went with EA's own
app), and a third-party one's ids are its own: EA games are not matched.

## 5. The Bundle's Groups

The Bundle keeps no Groups of its own. A Group is a **Playnite filter preset with "Show in Fullscreen quick
selection"** on, in Playnite's order: `extension/XboxSeries.Extension/PlayniteLibrary.cs` reads
`api.MainView.GetSortedFilterPresets()` and keeps those with `ShowInFullscreeQuickSelection` (Playnite's spelling).
The extension's data folder holds settings, never Groups.

`filterpresets.db` has the collection `FilterPreset` (`source/PlayniteSDK/Models/FilterPreset.cs`: `Name`,
`Settings`, `SortingOrder`, `SortingOrderDirection`, `ShowInFullscreeQuickSelection`, which defaults to true when
absent) and `FilterPresetsSettings`, one document whose `SortingOrder` lists preset ids. The order is
`GetSortedFilterPresets`'s: those listed, in that order, then the rest by name.

A preset's `Settings` (`FilterPresetSettings`) is matched as Playnite's `GameDatabase_Filters.cs` matches it:
`Hidden` must equal the game's, the id lists (`{ "Ids": [...] }`; `Guid.Empty` among them is "none") are each any-of,
and together all-of; `UseAndFilteringStyle` makes a list all-of. Each preset becomes, in the Dashboard:

| The preset filters by | Becomes | Why |
|---|---|---|
| Only installed, uninstalled, favourite, hidden and libraries the Dashboard reads | A **custom Group with that rule** (`model::Rule`), sorted as the preset was | The library keeps it up to date as games change, as a Group made in Settings |
| Categories, tags, genres, features, platforms, developers, publishers, series, age ratings, regions, sources, completion status (by id) | A **Group of the games it picked** in Playnite on the day, a collection of the source `playnite` | The Dashboard has none of those; the games Playnite picked are what the user saw in that Group |
| Name, release year, play time, version, a user score, dates, install size, or anything filtered by text rather than id | **Not brought**, and the page says why ("it filters by release year") | These cannot be worked out faithfully from what is read |
| A preset not flagged for Fullscreen | Not brought: it was never a Group | |

Sorting: Name, Last activity, Date added and Play time map to the Dashboard's; any other sort becomes by name.

## 6. What the import does

`library::import`, on the library database in one transaction:

- A game is the library's game of the **same store and store id**, matched without regard to case (Battle.net's
  `WoW`; the library's own may be `wow`). Never by name: the Epic game of a Steam game's name is another game.
- **Play time** becomes Playnite's where Playnite's is more, and **last played** where Playnite's is later. Never
  added together: Playnite's play time for a Steam game is mostly Steam's own, which the library may have already.
- A game becomes a **favourite** or **hidden** where Playnite says so. Nothing that is one stops being one.
- A **Group** comes over once, last on Home in Playnite's order and shown. A rule's Group is remembered by
  `playnite.group.<preset id>` in the settings table: once brought it is the user's, to change or delete, and is not
  brought back. A Group of games is a collection `(playnite, <preset id>)`, whose games a later import puts right. A
  Group whose name is already a Group's here is left, and said.
- `playnite.imported` (Unix seconds) is set. The preview (`Database::import_preview`) is the same work rolled back.

Running it twice: everything is "more", "later" or "not yet", so the second run changes nothing and says
"nothing new". Games of Playnite's that the library does not have are left: there is nothing to put them on.

## 7. Settings, the offer, and the log

- **Theme settings > Import from Playnite** (icon U+E8B5), on a row of its own under Games & apps. Its page, laid out
  as Art's: "Playnite library" and what it does; where the library was found and how many games, of them how many
  are here; **What comes over** (`Preview::describe`, e.g. "Play time for 1 game, last played for 1 game, 1
  favourite, 1 game hidden, Groups: Couch picks, Installed on Steam."); **What stays** (games not here, of other
  stores, and each Group that cannot come over, with why); and one pane button, **Import**, dimmed while there is
  nothing to bring. The page looks when it opens (`Effect::Playnite(Ask::Look)`), so the preview is always what is
  there now; Import (`Ask::Import`) reads Playnite again and brings it.
- **The focus log**: `Settings: Import from Playnite: Import; <state>`, the state `looking`, `not found`,
  `unreadable`, `ready`, `nothing new` or `done`.
- **The offer**: the first time the stores have been read with a library found and never imported, one Toast,
  which also stays in Notifications: "Playnite is on this PC. Bring over its play time, favourites and Groups in
  Settings > Import from Playnite." A Toast rather than a prompt, because a prompt would stand between the user and
  Home on a first start, and asks for a choice before they have seen anything; the page shows what would come over
  before anything is done. Said once ever (`playnite.offered`).
- **`--playnite <dir>`** names the folder to read in place of the user's. With any test's fake store (`--steam`,
  `--library` and the rest) or `--screenshot` and no `--playnite`, the user's own Playnite is never read; safe mode on
  the TV PC gives every test `--playnite` (an empty folder unless it made a fake), as for the stores.
- **The log**: `import: found Playnite's library at <dir>: N games, M of them here, ...`, `import: would bring ...`,
  `import: Group "<name>" cannot come over: <why>`, then for an import one line a change (`import: Dune Cartographer
  (steam 9100080): play time 754 min, was 0 min`, `... : a favourite`, `... : hidden`, `import: Group "Couch picks"
  made: 2 games as Playnite picked them`) and `import: done: ...` or `import: done: nothing new`. `import: offered:
  ...` for the Toast; `import: no Playnite library found`, and `import: <dir> could not be read: <why>`.

## 8. The fallback, weighed

If reading LiteDB proves wrong on real files, there are two other ways in:

| | Playnite's own export | The Bundle extension writing a file |
|---|---|---|
| What | Playnite has no export of its library to a file of its own; third-party add-ons write a view's columns as CSV, and Playnite's backup is a zip of these same LiteDB files | The Bundle's extension, running inside Playnite, writes the games and the Groups as JSON with Playnite's own API |
| Needs | An add-on the user installs, and steps in Playnite | The user to run Playnite once more with an updated Bundle |
| Faithful | Columns only: no store ids unless chosen, no presets | Exactly what Playnite knows, Groups included |
| Against | Another add-on, steps, and still no Groups | A new Bundle release, and an import that only works for those who update it before leaving |

**Picked: read the LiteDB files directly.** It needs nothing of the user but to close Playnite, works on a PC where
Playnite is no longer used, and the format is fixed for Playnite 10, which keeps LiteDB 4 on purpose (the comment
atop `LiteDBFileReaderV7.cs` points to `ItemCollection`'s for why). Should a real library be misread, the extension's file is the
fallback: a `playnite::Playnite` built from JSON instead of LiteDB, everything after it unchanged.

## 9. Tests

- `crates/playnite/tests/litedb4.rs`: the reader on files the real LiteDB 4.1.4 wrote (`tools/playnite-litedb write`).
- `crates/playnite/src/fixture.rs`: the made-up library (`standin make-playnite`), Steam games shared with the fake
  Steam's: Dune Cartographer (12 h 34 min, played last), Fathom Nine (a favourite), Hollow Pines Motel (hidden),
  Ashfall Courier (less play time than Steam's), Ghost Pier (not in the fake Steam), an Epic Ashfall Courier (the same
  name in another store), a game added by hand; presets Couch picks (by category), Installed on Steam (a rule),
  Released in 2019 (cannot come over) and one not flagged.
- `crates/playnite/tests/import.rs`: that library imported into the fake Steam's, previewed, imported, and again.
- `crates/library/src/import.rs`: matching, merging, Groups, and running twice.
- `crates/screens/tests/settings.rs`, `settings_import_from_playnite`: the page, its focus line and its pictures.
- `e2e/payloads/playnite-import/run.ps1`: on Windows, the offer, the preview and the import with the pad, judged by
  the `import:` lines, `--log-focus` and the `groups:` line, Dune Cartographer's hub from Home's first recent tile,
  and Playnite's files unchanged. Written, not yet run.

`data/` (the user's own Steam files) is in none of them, and no file from anyone's Playnite is in the repository.
