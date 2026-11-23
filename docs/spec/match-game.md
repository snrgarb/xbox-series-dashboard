# Match game specification

The Bundle has no such page: Playnite's own metadata download did it. There the user searches a metadata source for a
game and picks the result that is the game, and Playnite then takes that entry's metadata. This page is the port's
own way to do the same for art. It is drawn in the manner of the art picker (art-picker.md), with the same band and
tab strip. All numbers are design units = px on the 1920x1080 canvas, 1:1. Code: `crates/screens/src/match_game/`
(the page), `crates/model/src/art_match.rs` (a match, a result), `crates/library/src/matches.rs` (what is kept),
`crates/library/src/art/picker.rs` (the searches), `crates/library/src/art/lookup.rs` (what a match changes).

## 1. Why

Art is looked up by the game's id where the source knows the game's store (a Steam game at Steam, an Epic game at
SteamGridDB), else **by its name**, which must be the same once written plainly (docs/sources.md, "Matching by name").
Some launchers name a game in a way no other source does. Battle.net's **World of Warcraft: Forever** (#339) is
"World of Warcraft" on SteamGridDB, and its Classic flavours have names of their own. So their names find nothing, or
find another game, and the game gets no art. A **match** is the user's word on which of a source's games this game is.
After a match the source is asked by the match's id, never by the game's name.

"Metadata" here means art: the Dashboard takes nothing else from the online sources. A game's details (developers,
release date, scores) come from its store's own files on the PC (game-hub.md 14.1).

## 2. What opens it

- The Tile menu's More options, **Match game**: the flyout's fourth row, under Change art and shown with it, for a
  game of a store (tile-menu.md 15). The page opens **in the menu's place** (`PageAction::Replace`), over what the
  menu was over.
- The Game hub's **Manage**, which is the Tile menu, the same way.

B closes the page and leaves the game as it was. Focus goes back to the tile (or to the hub's Manage).

## 3. Layout

The art picker's band (art-picker.md 3.1): y 134 to 942, `#2D2D2D`, contents from x 132 to 1691, over the page dimmed
by `menu_panel::DIM`.

- **The tab strip**, 72 high, `#1F1F1F`: one tab, 200 wide, for each online art source, in Settings > Art's order
  (`model::ArtOrder`), which by default is Steam, Xbox, PlayStation, SteamGridDB. A source turned off is still a tab,
  and the page says where to turn it on. The tabs are drawn as in the art picker: the selected one semibold and white,
  with the accent bar 6 high; the game's name right-aligned at x 1691, dim.
- **The line under it**, its middle 38 under the strip: "Which of <source>'s games is this?" on the left, dim.
  "Matched: <the match's name there>" or "Matched: Automatic" on the right.
- **The search field**, from 76 under the strip, 60 high, the contents' width, `#3B3B3B` (`#4A4A4A` focused): the
  search glyph (U+E721) 24 in, then the words searched for. These start as the game's name.
- **The list**, from 156 under the strip: rows 56 high and 4 apart, eight shown, which scroll to keep focus in view.
  - Row 0 is **Automatic**: no match, so the game is found by its name. It shows "Found by its name: <game>", dim.
  - Then each result the source's search found, in its order, at most 25 (`art_match::MOST_HITS`). Each result shows
    its name and, right-aligned and dim, its year where the source gives one (SteamGridDB does), then the source's
    name and its id there ("SteamGridDB 5263").
  - The row in use has the art picker's in-use disc: 36 across, accent green, with a tick, 24 in from the row's
    right.
- A Spinner shows in the second row's place while the search has not come back.
- **The foot**, its middle 44 above the band's foot: "LB RB where to look   Y search   A match   B leave it as it is",
  or, in its place, what went wrong (section 5).

## 4. Pad

| Button | Does |
|---|---|
| LB / RB | The source before / after; stops at the ends. Each tab keeps its own words, results and focus. |
| Up / Down | Between the field and the rows. The list ends at its last row. |
| A on the field, or Y anywhere | Text entry (the Console keyboard, search.md 7 to 10) over the page, captioned "Search for the game", starting from the words. Kept words (Menu or Enter) are searched for on that source, and focus goes to Automatic. Empty words change nothing. |
| A on Automatic | The match on this source is forgotten, and the page closes. If there was none, the page just closes. |
| A on a result | The game is matched to it on this source, and the page closes. Choosing the match already in use just closes. |
| B | Closes; nothing changes. |

Each source is searched **once per set of words**, the first time its tab is shown. A result that comes back for
words since changed is ignored.

## 5. What it says

- Art online off: "Turn on Download art online in Settings > Games & apps > Art to search the stores."
- The source off among the art sources: "Turn on <source> in Settings > Games & apps > Art's sources to search it."
- SteamGridDB with no key, or one it refused: "Enter your SteamGridDB API key in Settings > Games & apps > Art to
  search it."
- Nothing found: "<source> found nothing for these words. Press Y to search for others."
- Asked to wait: "<source> is busy. Press Y and search again in a minute."
- Anything else: "The search couldn't be done: <why>. Press Y to search again."

In each of these the source is not asked. Automatic can still be chosen, which forgets a match made earlier.

## 6. The searches

What each source is asked, only the words being sent (and SteamGridDB's key, to its API alone):

| Source | Request | Result |
|---|---|---|
| Steam | `store.steampowered.com/api/storesearch/?term=<words>&l=english&cc=US` | `items[]` of `type: app`: appid, name |
| Xbox | the Microsoft Store's `productFamilies/autosuggest?...&productFamilyNames=Games&query=<words>` | products of `Type: Game`: product id, title |
| PlayStation | the PlayStation Store's `chihiro` search, `<words>?suggested_size=10&mode=game` | full games and bundles: content id, name |
| SteamGridDB | `/search/autocomplete/<words>` | id, name, the year of `release_date` |

These are the requests the art lookup already makes to find a game by name (docs/sources.md, "Art"). The searches go
through the art picker's network (`library::art::Picker`), with the same politeness: 250 ms between two requests to a
source, and a source that asks to wait is waited for. The log names no game, only the source and a count:
`art: match: SteamGridDB found 4`, `art: match: Steam failed: Busy`.

## 7. What a match changes, and what is kept

A match is a row of `art_matches(source, source_id, online, id, name)`, with `online` being `steam`, `xbox`,
`playstation` or `steamgriddb`. Automatic means no row. It is keyed by the game's store and its id there, as the art
choices are (art-picker.md 7), so it **follows the game across launcher updates** and re-syncs. `Database::snapshot`
puts a game's matches on it (`Game::matches`).

Once the match is kept, the game's downloaded art and its remembered misses are forgotten, as the game's Refresh art
does. Its art is then looked up again, that game first (`Effect::MatchGame`). The log says
`art: match: SteamGridDB is 5263` or `art: match: Steam is Automatic`.

How the art lookup asks a matched source:

- **SteamGridDB**: by the match's id. The game is not looked for at all.
- **Steam, Xbox**: by the match's id there (Steam's `GetItems` by appid, the Microsoft Store's `products?bigIds=`), in
  place of a search by name. This works for any game, including another store's.
- **PlayStation**: its pictures come only with its search's results, so the store is searched again by the match's
  name and the result with the match's id is taken. If that result is no longer found, the source has nothing for the
  game, and the log says `art: <source> <id>: PlayStation: the game matched is not in its search`.
- **The art picker** lists the candidates of the game matched on SteamGridDB.

A picture the user chose in the art picker still shows over what the lookup finds (art-picker.md 7). A match changes
what the lookup and the picker find, not a choice already made.

## 8. The focus log

`--log-focus` says `focus: MatchGame: <source>: <row>; matched: <id or Automatic>; <state>`:

- `<row>`: `Search` (the field), `Automatic`, or `Result <n> (<id>)`.
- `<state>`: `loading`, `<n> found`, `none` (found nothing), `offline`, `source off`, `no key`, `busy`, `failed`.

For example: `focus: MatchGame: SteamGridDB: Result 2 (5264); matched: Automatic; 4 found`.

## 9. Tests

- `model`: none of its own; a match is data.
- `library`: `matches::tests` (kept, in the sources' order, across a sync and a reopen; Automatic forgets it);
  `art::tests` (a game matched on SteamGridDB asked for by the match and never by its name; matched on Steam by the
  appid with no search; matched on PlayStation by the id among its search's results, and said when it is gone);
  `art::picker::tests` (a search to each source with the words alone, the key to SteamGridDB's API alone; offline and
  no key ask nothing).
- `screens`: `match_game::tests` (one search for each source as its tab is shown; other words; a result's match; a
  late answer ignored; Automatic; sources that can't be searched) and `tests/match_game.rs` (opened from the Tile menu
  in its place, B back to the tile, a match made through text entry; pictures `match-game-results.png` and
  `match-game-off.png`).
