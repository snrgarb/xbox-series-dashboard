# Art picker specification (#213)

The Bundle has no art picker: Playnite's own metadata dialogs chose a game's pictures, and those are not ported (tile-menu.md 13). This page is the port's own, drawn in the manner of the Bundle's dialogs (Power's band, guide.md 6.2) with the Guide's tab strip (guide.md 2.2). All numbers are design units = px on the 1920x1080 canvas, 1:1 like the Tile menu (nothing is in a Viewbox). Code: `crates/screens/src/art_picker/` (the page), `crates/model/src/art_picker.rs` (tabs, paging), `crates/library/src/art/picker.rs` (the network), `crates/library/src/choices.rs` (what is kept).

## 1. What opens it

- The Tile menu's More options, **Change art**, the flyout's third row under Refresh art and shown with it: a game of a store (tile-menu.md 15). The picker opens **in the menu's place** (`PageAction::Replace`), over what the menu was over.
- The Game hub's **Manage**, which is the Tile menu (game-hub.md), the same way.

B closes it with the picture as it was, focus back on the tile (or the hub's Manage).

## 2. SteamGridDB's paging (what the service says)

From the service's own OpenAPI description, <https://www.steamgriddb.com/static/openapi.yml>, rendered at <https://www.steamgriddb.com/api/v2> (read 2026-11-17):

- `/grids/game/{gameId}`, `/heroes/game/{gameId}`, `/logos/game/{gameId}` and `/icons/game/{gameId}` all take `page` and `limit`, with the filters (`styles`, `dimensions`, `mimes`, `types`, `nsfw`, `humor`, `epilepsy`, `oneoftag`).
- `page` (`PageParam`): an integer, **default 0**: pages count from 0.
- `limit` (`LimitParam`): an integer, **default 50**, "Values > `50` will be ignored": 50 a page at most.
- The reply (`Grids`, and the same for heroes, logos and icons): `{success, page, total, limit, data[]}`. The description does not say whether `total` counts candidates or pages; the port takes it as **candidates** (the count of the whole list), and never trusts it alone (section 4).
- Each candidate has `id`, `score`, `style`, `url` (the full picture), `thumb` ("Path to 380x178 thumbnail image"), `width`, `height`, `tags`, `author`.
- Grid sizes it knows: 460x215, 920x430, 600x900, 342x482, 660x930, 512x512, 1024x1024. Hero sizes: 1920x620, 3840x1240, 1600x650.
- **No rate limit is documented**, and the real service sent no rate-limit headers when it was checked (sources.md "Art"). The port keeps its own politeness: 250 ms between any two requests to a host, `Retry-After` (or 30 s doubled, 10 minutes at most) on 429 or 503, given up after five.
- The official Node client (<https://github.com/SteamGridDB/node-steamgriddb>) sends neither `page` nor `limit` and keeps only `data`: it shows only the first 50, so it is no guide to paging.

Not checked against the real service: the picker was built against the stand-in alone (`standin serve-art`), as the ticket's rules ask. **UNVERIFIED**: that `total` counts candidates, and that a filtered list's `total` is the filtered count.

## 3. Layout

### 3.1 The band

Over the page that opened it, dimmed as the Tile menu dims (`menu_panel::DIM`): a band the screen's width, **y 134 to 942** (808 high), `#2D2D2D`, Power's (guide.md 6.2). Its contents span **x 132 to 1691**.

### 3.2 The tab strip

Across the band's top, **72 high**, `#1F1F1F` (the Guide's strip). One tab a kind (section 5), **200 wide** from x 132, the label centred, Selawik 26: the selected tab semibold and white, with the Guide's **bar 6 high** in the accent colour along the strip's foot under it; the others `#8C8C8C`. The game's name sits right-aligned at x 1691, dim.

Under the strip, its middle **38** under it (y 244): left, the one line saying where that kind of picture shows (`Tab::shows`); right, aligned at x 1691, the count: **"Page n of N · T from SteamGridDB"**, or "Page n" while N is not known.

### 3.3 The grid

From **86 under the strip** (y 292), **568 high**, cells **16 apart**, centred across x 132..1691. Each tab has its own grid, whole pages fitted to its kind's shape (rows x columns; the cell as large as the height allows, else the width):

| Tab | Shape | Grid | Cell | A page |
|---|---|---|---|---|
| Square | 1:1 | 2 x 5 | 276 x 276 | 10 |
| Cover | 600:900 | 2 x 7 | 184 x 276 | 14 |
| Background | 1920:620 | 4 x 3 | 403 x 130 | 12 |

A cell is `#3B3B3B` (`#4A4A4A` focused) under its picture. The candidates' thumbnails are drawn **at the cell's shape**: Square as the square tiles draw it (`home::face`: a portrait picture whole over its own blur), Cover and Background cropped from the top. Focus is the Dashboard's ring (`theme::focus_ring_cornered`, 12). A cell whose thumbnail has not come is the plain fill; one whose page has not come is a Spinner's place.

**The fixed entries**, first on page 1, in this order:

1. **Automatic**, "The Dashboard's pick": no choice; what the lookup finds (sources.md "Art").
2. **The store's own**, its picture with the caption on a dark strip at its foot: where the store's client keeps one on the PC (Steam's cover and hero; none of Steam's is square, so Square has none for Steam).

**In use:** the cell of what is in use carries a disc **36 across**, accent green, **10 in** from its top right corner, a tick (U+E73E) on it.

### 3.4 The foot

Its middle **44 above the band's foot** (y 898): what the buttons do ("LB RB kind of picture   LT RT page   A choose   B leave it as it is"), or, in its place, what went wrong (section 6). **Retry**, 200 x 52 at the right (x 1491..1691), Selawik 26 bold, is there only with a failure that a retry could mend, or an empty list.

## 4. Paging

The ticket: "Make sure you correct paginate the art per page". The rule (`model::art_picker::Pager`):

- The candidates are one list, in SteamGridDB's order, that the fixed entries come before. Grid page `p` shows cells `p x per_page .. (p + 1) x per_page` of it.
- Service pages are asked **strictly in order, from 0, one at a time**, `limit=50`, and only as far as the grid page shown needs. Each is asked **once** for the picker's life; what came is kept until it closes.
- The list ends at a page shorter than its limit, or when `(next page) x limit` reaches the `total` said. Nothing past that is asked.
- `total` may change between requests (a candidate added or taken away meanwhile): candidates are kept by id, so one seen twice is shown once and taken off the count; the count is the last reply's `total` less those, and never less than the candidates got.
- N (the page count) is `ceil((fixed + total) / per_page)`, at least 1.
- Thumbnails are fetched only for the page shown, each once; the full picture only for the one chosen, as it is chosen.

Tested by `model` (`art_picker::tests`: 0, 1, 50, 51 and 170 candidates; a page size that does not divide 50; a total that changes; late and out-of-turn replies), by `library` (`art::picker::tests`, against the stand-in service: 0, 1, 50, 51 and 140, each `page=N` asked once and in order) and by `screens` (`art_picker::tests`).

## 5. Tabs: which kinds, and why

A tab for each kind the Dashboard draws **at its own shape**:

| Tab | Shows on | Asks SteamGridDB for |
|---|---|---|
| **Square** | square tiles: Home's Recent row, Search, the Guide, the Queue | `/grids/game/{id}?dimensions=1024x1024,512x512` |
| **Cover** | portrait tiles: My games & apps, Full library | `/grids/game/{id}?dimensions=600x900,342x482,660x930` |
| **Background** | behind Home and the Game hub when the game is focused | `/heroes/game/{id}` |

each with `types=static&nsfw=false&humor=false&epilepsy=false&mimes=image/png,image/jpeg,image/webp&limit=50&page=<n>`.

**No Banner (header) tab:** the header is never drawn at its own shape; it is only a fallback cropped into a cover's place when a game has no cover. A tab for it would offer pictures nobody sees as chosen. Logos and icons are not drawn at all.

## 6. Pad, and what it says

| Button | Does |
|---|---|
| LB / RB | The tab before / after; stops at the ends. Each tab keeps its own page and focus. |
| LT / RT | The page before / after, focus staying on the same cell where the new page has it, else on its last cell. Nothing past the first or the last. |
| Left / Right past the grid's edge | The page before / after, the same row's last / first cell. |
| Down from the last row | Retry, when it is shown. |
| A | On a cell: that one is chosen and the picker closes. On Retry: the failed page is asked again. On a cell whose page has not come: nothing. |
| B | Closes it; nothing changes. |

Said in the foot:

- No key: "Enter your SteamGridDB API key in Settings > Games & apps > Art to choose from its pictures." (the fixed entries alone, no Retry). The same once SteamGridDB has refused the key.
- Art online turned off: "Turn on Download art online in Settings > Games & apps > Art to choose from SteamGridDB's pictures."
- A game the service does not know: "SteamGridDB doesn't know this game."
- Asked to wait: "SteamGridDB is busy. Try again in a minute." with Retry.
- Anything else: "These pictures couldn't be fetched: <why>." with Retry.
- An empty list: "SteamGridDB has no pictures of this kind for this game." with Retry.

A Spinner shows where the candidates will be while a page comes.

## 7. What is kept

A choice is the database's row `art_choices(source, source_id, kind, chosen)`, `chosen` being `store` or `steamgriddb:<id>`; Automatic is no row. A chosen picture's file is downloaded once, by the full `url`, into `art-chosen/<source>/<source_id>/<kind>.<ext>` beside the art cache, not in it. So:

- **Refresh all art and details**, a game's **Refresh art** and **Clear downloaded art** never replace it (they touch only the cache).
- It is keyed by the store and its id, as the library's games are, so it **follows the game across launcher updates** and re-syncs.
- `Database::snapshot` puts it over whatever the lookup found; Automatic gives the game back to the lookup, which is started again for it.

A download that fails keeps nothing and says so in a Toast.

**Privacy:** only the store id (or the name, to search) goes to SteamGridDB; to a thumbnail's or picture's host nothing but its address. The log names no game: `art: picker: square page 1: 50 of 60 candidates`, `art: picker: square is SteamGridDB picture 123`, `art: picker: SteamGridDB refused the key`.

## 8. The focus log

`--log-focus` says `focus: ArtPicker: <Tab>: <page>: <cell>; in use: <what>[; <state>]`:

- `<page>`: `Page n of N`, or `Page n` while N is not known.
- `<cell>`: `Automatic`, `Store`, `Picture <id>`, `Coming <i>` (a cell whose page has not come), or `Retry`.
- `<what>`: `Automatic`, `Store` or `Picture <id>`.
- `<state>`, when there is one: `loading`, `no key`, `offline`, `no game`, `busy`, `failed`, `none`.

E.g. `focus: ArtPicker: Cover: Page 3 of 5: Picture 2027; in use: Automatic`.

## 9. Tests

- Screenshots of each tab, a failed page and no key: `crates/screens/tests/art_picker.rs` (references `art-picker-*.png`).
- On the TV PC: `e2e/payloads/art-picker/run.ps1`, against `standin serve-art --candidates 60`: to the Square tab's last page by Right, a candidate chosen there, one on Background's third page, the stand-in's log showing the square list's pages 0 and 1 asked once each and in order; both still in use after Refresh all art and details and after the game's Refresh art.
