# M3, the library pages: a proposed breakdown into tickets

A proposal, to be turned into GitHub issues as M2 ends ([plan.md](../plan.md): "M2 onwards get their tickets as the milestone before each one ends"). M3 ends with "every Dashboard page navigable".

The specifications each ticket builds from:

| Spec | Page |
|---|---|
| [my-games.md](my-games.md) | My games & apps: the nav, Games, Play history, Full library, Manage |
| [game-hub.md](game-hub.md) | The Game hub |
| [tile-menu.md](tile-menu.md) | The Tile menu, More options and the prompts |
| [search.md](search.md) | Search, the Console keyboard and text entry |
| [settings.md](settings.md) | Settings |
| [filters.md](filters.md) | Saved filters and Steam collections in place of Playnite's presets |
| [home.md](home.md) | Home, which gains what M3 unlocks (section 11 there lists what waits) |

Every ticket is one branch and one pull request, with screenshot tests against committed references and behaviour tests in `crates/screens/tests/shots.rs`, as [ui.md](../ui.md) sets out. "Test" below is what proves the ticket alone, with no later ticket merged.

## Order and what can run in parallel

```
1 Page stack, hint bar ──┬── 3 Menu panel, Tile menu ──┐
                         ├── 4 Game hub ───────────────┤
                         ├── 5 My games & apps ────────┼── 6 Full library ──┐
                         ├── 7 Console keyboard ───────┼── 8 Search         ├── 10 Groups in Settings
                         └── 9 Settings ───────────────┘                    │
2 Saved filters (library) ──────────────────────────────── 6 ───────────────┘
```

- **At once, in parallel:** 1 and 2 (2 touches only `library`, `model`, `source-steam` and Home's Groups).
- **After 1, in parallel:** 3, 4, 5, 7 and 9. They share only the page stack; each lands with the others' entry points doing nothing (as Home's do today) and wires them up when they exist.
- **Then:** 6 (needs 2, 5, 7), 8 (needs 7; A on a result needs 4), 10 (needs 2, 3, 7, 9).
- The box has room for one cargo build at a time, so "in parallel" means separate worktrees taking turns at the build lock, not faster wall-clock builds.

## The tickets

### 1. The page stack, and the hint bar

- `Dashboard`'s one page becomes a stack of pages over Home: the top one takes the pad, each keeps its state while covered ([ui.md](../ui.md), "Screens and what is over what"). My games & apps, Search and Settings are never up together: opening one closes the others ([settings.md](settings.md) section 8; [search.md](search.md) section 4).
- Opening and closing are instant, as in the Bundle: no page transition anywhere in M3's pages ([my-games.md](my-games.md) section 3; [settings.md](settings.md) section 10; [search.md](search.md) section 6).
- B returns to what opened the page with focus where it was, and Home still at its rest ([home.md](home.md) section 7.4). The top bar hides while a page is up ([home.md](home.md) section 2).
- The hint bar ([home.md](home.md) section 10.4), drawn once for every page, with the Menu and Y hints; Home shows it on a Group's game card.
- Home's top bar buttons, Y, the Play history tile, See all and the wide tiles open their pages (empty stand-ins until tickets 5, 8 and 9).
- **Test:** behaviour tests press through Home > stand-in page > B for every entry point and assert focus and rest; a screenshot of the hint bar on a Group card.

### 2. Saved filters in the library, and Steam's collections

- Migration 2: `collections`, `collection_games`, `filters`, the four built-ins ([filters.md](filters.md) section 7). `model::Game` gains `source` and `added`; `Library` gains `filters`, each with its games in its sort ([filters.md](filters.md) section 3).
- Rule evaluation for the eight clauses and four sorts, never-played last either way, ties by name ([filters.md](filters.md) sections 3.1 and 3.2).
- `source-steam` reads the user's collections; the first step is to check the file format against a real install ([filters.md](filters.md) section 5.1, open question 8). The sync adds, renames and removes collection filters and leaves the user's shown and order alone (section 5.2).
- The `Database` API to show, move, add, change and delete a filter (section 7).
- Home's Groups come from `Library::filters`; the stand-in Groups worked out in the screen go ([filters.md](filters.md) sections 4.1 and 8).
- **Test:** unit tests in `library` on an in-memory database and the Steam fixture (which gains a collections file); Home's Groups screenshot from seeded filters. No UI of its own.

### 3. The menu panel, the Tile menu and its prompts

- The shared panel: 450 wide, header, rows 72, the focus fill and ring, the dim, the fly-up and its Reduce motion form ([tile-menu.md](tile-menu.md) sections 2 and 7). It is also the Guide's prompt panel and ticket 10's menu.
- The Tile menu on Menu from a Home game tile: rows per game state ([tile-menu.md](tile-menu.md) section 3), Up/Down, B, the held-button rule on close (section 6), sounds through #28's player (section 8).
- What M3 wires: Play game / Quit, Add to / Remove from Favourites (`set_favourite`, then a new snapshot), Manage game (the Game hub, once ticket 4 is in), Customise Home (Settings on Personalisation, once ticket 9 is in), More options with Hide this game.
- Install, Uninstall, Remove, a game's own actions, HDR and Extensions follow the recommendations in [tile-menu.md](tile-menu.md) section 13: decide open questions 1, 2, 3 and 7 there before starting.
- The More options flyout and the "Remove <game>?" style prompt as a reusable prompt ([tile-menu.md](tile-menu.md) sections 4 and 5).
- **Test:** screenshots of the menu at each row count, the flyout and a prompt over Home; behaviour tests for each row's effect and for B at every level.

### 4. The Game hub

- The backdrop: a 1/8-size snapshot of the page behind, blurred and darkened, in the two strips; the game's art blurred in the band ([game-hub.md](game-hub.md) section 2). This needs a render-to-texture and blur pass in `render`.
- The header, the facts line, the cards with what the library can fill, and the page toggle ([game-hub.md](game-hub.md) sections 3 to 5). Decide open questions 7, 9, 10 and 13 there first: which metadata M3 adds, and whether the Game details page ships without it.
- The actions: Play / Running, Manage (the Tile menu without Manage game), the Queue slot left empty for M4, focus, the name chip ([game-hub.md](game-hub.md) sections 6 to 8).
- Opening: A on a Home tile after the 300 ms double-tap window, a double tap playing; at once from Search and Manage game; B back with focus on the tile ([game-hub.md](game-hub.md) section 9). Home's "A launches at once" ([home.md](home.md) section 11) goes.
- The achievements card is a placeholder for M6; the Achievements page is not built ([game-hub.md](game-hub.md) section 5).
- **Test:** screenshots of the hub over Home for a game with background art, with a cover only and with neither; behaviour tests for the double-tap window, navigation and B.

### 5. My games & apps: the shell, Games, Play history and Manage

- The shell: the page fill, the nav with its counts and selected and focused states, focus on a nav item showing its content, the divider, the header, the progress indicator's place ([my-games.md](my-games.md) sections 2 to 4).
- Games: the five-column grid of installed games, its tile (not Home's), scrolling, edges, Y for Search, Menu for the Tile menu, A for the Game hub ([my-games.md](my-games.md) section 5). Decide open questions 1, 2, 4 and 5 there first: the grid's scroll and edge rules are Playnite's and unrecorded.
- Play history: the dated sections and rows, the 300 ms rule on A, focus memory ([my-games.md](my-games.md) section 6).
- Manage with its Queue box, inert until M4; the Apps nav item present and empty until M4 ([my-games.md](my-games.md) sections 8 and 9, open question 6).
- The pane fly-in along the nav and its Reduce motion form ([my-games.md](my-games.md) section 3.3); empty states (section 11).
- **Test:** screenshots of Games (first rows, scrolled, empty), Play history (several sections, empty) and Manage; behaviour tests for the nav, the grid's edges and B from each entry point.

### 6. Full library: tabs, Sort, the name filter and See all

- The tabs from the shown filters, the active tab's fill and glow, Left/Right and LB/RB stepping, the invisible focus stop under them, the grid's fly-in across tabs ([my-games.md](my-games.md) sections 3.4 and 7.2; [filters.md](filters.md) section 8).
- The Sort box and its list, four options until there is a release date ([my-games.md](my-games.md) sections 7.3 and 7.4; [filters.md](filters.md) section 3.2).
- The name box and "Filter by name" through text entry, narrowing as it is typed ([my-games.md](my-games.md) section 7.5).
- See all on a Home Group opens Full library on that Group's tab; B back to See all with Home still at that Group's rest ([my-games.md](my-games.md) section 3; [home.md](home.md) section 6.5).
- The grid lifted under the row with its fade, the counts, the empty state ([my-games.md](my-games.md) sections 7.1, 7.6 and 7.8).
- **Needs:** 2, 5, 7. **Test:** screenshots of each tab state, the open Sort list, a name narrowing the grid, an empty tab; behaviour tests for See all and back from each Group.

### 7. The Console keyboard and text entry

- The board: 5 by 12 cells, 47 keys, the three layers, key colours, the one ring ([search.md](search.md) sections 7 and 8).
- Navigation by cell with the remembered row and column, no wrap; the pad's shortcuts (X, Y, LB, RB, LS, LT, Menu); the caret model; a physical keyboard ([search.md](search.md) section 9).
- Text entry over the dimmed page: caption, field, masking, Start keeps and B drops, typed-as-you-go callbacks ([search.md](search.md) section 10). Built once for Full library's name (ticket 6), Groups (ticket 10), and M5's and M6's fields.
- Decide [search.md](search.md) open questions 4, 7 and 8 first (held repeat, lit Shift, long text).
- **Test:** screenshots of each layer and of text entry over a stand-in page; behaviour tests typing a string by navigation and by shortcuts, and for every edge of the board.

### 8. Search

- The page, the search box, the keyboard's place, results with the keyboard up and away ([search.md](search.md) sections 1 to 3).
- Matching and ranking, no debounce, never hidden games; "Recently added" while nothing is typed; the Games row; the Apps row's place for M4 ([search.md](search.md) section 3).
- The sliding rows, 300 ms cubic ease-out, instant under Reduce motion ([search.md](search.md) section 3).
- Every entry point (top bar, Y on Home, Y on Games, the Guide's Search Utility) and where B returns; A on a result opens the Game hub at once ([search.md](search.md) sections 4 and 5).
- **Needs:** 7; 4 for A on a result. **Test:** screenshots of empty Search, results for a typed name, no results, the keyboard away; behaviour tests for ranking and for B's two steps.

### 9. Settings: the shell, the controls and the Bundle's own options

- The shell: the rail, the header, panes of boxes, pages in the boxes' place, B one level at a time, the five entry points and where each returns ([settings.md](settings.md) sections 1 to 3, 8 and 9). Decide open questions 1 and 2 there first: Personalisation's overflow, and the rail's shape once Playnite's categories are gone.
- The controls: Box, checkbox box with its info text, Row, pane button, dropdown box and list, checkbox row ([settings.md](settings.md) section 4).
- A settings store of our own ([settings.md](settings.md) open question 6), and the options M3 owns wired to what they change: Background delay, Reduce motion, Show Play history, Show battery percentages on Home; Use the Guide.
- Boxes for M5 and M6 (Boot video, Profiles, Who's playing today?, Idle, Now Playing source, Discord) as open question 2 decides: absent or inert. Playnite settings, More Playnite options and the Debug menu are not built (section 14).
- Home's "Settings" wide tile and the Tile menu's Customise Home land where [settings.md](settings.md) section 8 says.
- **Test:** screenshots of Theme settings, Personalisation with a checkbox box focused, the Guide page, an open dropdown; behaviour tests that each option changes Home and survives a restart of the core.

### 10. Groups in Settings

- Personalisation's Groups box and the Groups page: a Row per filter, the tick for shown, New Group ([filters.md](filters.md) sections 6.1 and 6.2).
- The Group's menu on the menu panel: Move up, Move down, Edit, Delete and its prompt ([filters.md](filters.md) section 6.2).
- A custom Group's page: Rename through text entry, the dropdowns, the checkbox row, the live count ([filters.md](filters.md) section 6.3).
- Home's Groups and Full library's tabs follow each change with the next snapshot, focus kept.
- **Needs:** 2, 3, 7, 9. **Test:** screenshots of the Groups page, the menu and a Group's page; a behaviour test that makes a Group with the pad, moves it first, and finds it as Home's first Group and Full library's first tab.

## After the tickets

As M1 ends with #29, M3 ends with a run on the win11-e2e VM and a report in the map issue (#14): every page reached with the virtual pad from Home and back, with screenshots. That closing ticket is the milestone's own and is not counted above.

## Decisions to take before the tickets start

Each spec ends with its own numbered open questions. These are the ones that block a ticket; the rest can be settled inside it.

| Decision | Blocks | Where |
|---|---|---|
| The Games grid's scroll and edge rules (Playnite's own, not in the Bundle's source) | 5, 6 | [my-games.md](my-games.md) questions 2, 5 |
| A on a grid tile: at once, as the Bundle, or the 300 ms rule as everywhere else | 5 | [my-games.md](my-games.md) question 4 |
| What Install, Uninstall and Remove do before M4 | 3, 4 | [tile-menu.md](tile-menu.md) questions 1, 2; [game-hub.md](game-hub.md) question 11 |
| Which metadata the library gains in M3 (description, release date, publisher, features) and from where | 4, 6 | [game-hub.md](game-hub.md) question 7; [filters.md](filters.md) question 2 |
| Whether the Game details page ships in M3 | 4 | [game-hub.md](game-hub.md) question 13 |
| The rail with one category, and boxes for later milestones | 9 | [settings.md](settings.md) question 2 |
| Personalisation's layout, which overflows 1080 in the Bundle | 9, 10 | [settings.md](settings.md) question 1 |
| Where settings are stored | 9 | [settings.md](settings.md) question 6 |
| Empty Groups, and one set for Home and the tabs | 2, 6 | [filters.md](filters.md) questions 1, 6 |
| Where hidden games can be found again | 2, 3 | [filters.md](filters.md) question 10; [tile-menu.md](tile-menu.md) question 4 |
| Font line heights (Bahnschrift 1.2 or 1.25 em), which move every page's text by a pixel or two | all | [game-hub.md](game-hub.md) question 1; measure once on Windows |
| No picture exists of Search, the keyboard, text entry, Games, Apps, Manage, the dropdowns or the hub at `f11e4f9`: capture them from the Bundle on the VM before building, or build from source alone | 4 to 9 | [search.md](search.md) question 1 |
