# Install from (a game owned in several launchers; #249)

The user (2026-11-18): "installing games from different game launchers, so if a game can be downloaded from steam,
epic, xbox then allow for a second modal to popup to download". When Install is pressed on a game that the library
also has, not installed, from other launchers that can install it now, a question comes first: which launcher to
install from. A game in one launcher only installs as before, with no question.

**The question is the port's own.** Neither the Bundle (Playnite keeps a game per library and installs from the one
its tile is) nor the console has one. It is made from the prompt the Tile menu and the folder question share
(`menu_panel`, [tile-menu.md](tile-menu.md) section 5, [storage.md](storage.md) section 5), so it looks and behaves as
they do. **Wording guessed**, with nothing to copy.

The library keeps one entry per launcher, as before: whether a game owned twice should be one tile is not decided
here, and tiles are left as they are.

## 1. When it is asked

Install on the Tile menu, on the Game hub's main action, and from Manage Queue's game menu all go through
`tile_menu::install`. Before the folder question, it asks `install_from::asks`: is there more than one launcher to
install the game from. The launchers are (`install_from::choices`):

- **the launcher of the game pressed**, if that game can be installed at all, as Install already required: not
  installed, in the library database (an id), not in the Queue. If it can't be, there is no question and Install
  does what it did before (nothing).
- **each other launcher's copy of the same game** (section 2) that:
  - is in the lists, not hidden (`Library::games`);
  - is not installed and not in the Queue;
  - comes from a launcher that is switched on in Settings > Launchers (`Launchers::is_on`): a launcher switched off
    never counts;
  - and that its store can install now: its id is in `Storage::installable` (section 3).
- **one copy a launcher**: where a launcher has the same name twice, the library's first.

With two or more, the question opens; with one, Install goes on as before.

**A copy installed elsewhere** (decided here): Install is still offered for this launcher's copy, as the Tile menu
already offers it for any game that is not installed. The question lists only the launchers where the game is not
installed and not being installed; if only the launcher pressed is left, there is no question. The user is not told
that a copy is installed elsewhere: the tiles say so already, as each copy is its own tile.

## 2. The same game: the rule

`model::names` (`normalised_name`, `same_name`, `same_game_elsewhere`; `Library::same_game_elsewhere`), for this and
for anything else that matches games by name (art looked up by name, #246, can take the same function).

**Two games are the same game when they come from two different stores and their normalised names are equal and not
empty. Exact equality only:** a wrong match would offer the wrong download, so a doubtful pair is left apart.

Normalising a name:

1. Lowercase it (Unicode's lowercase).
2. Drop the trademark signs `™ ® © ℠` and the apostrophes `' ’ ‘ ʼ ` ´` where they stand, joining what is on either
   side: "Assassin’s Creed®" is "assassins creed".
3. Every other character that is not a letter or a digit (punctuation, symbols, dashes, colons, any whitespace,
   no-break spaces) is a break between words.
4. Join the words with one space.

Nothing more:

| Same | Not the same |
|---|---|
| "DOOM Eternal", "Doom Eternal" | "Quiet Orbit", "Quiet Orbit Deluxe Edition" (edition words are kept) |
| "Hitman: Absolution", "Hitman Absolution" | "Quiet Orbit", "Quiet Orbit: Game of the Year Edition" |
| "Half-Life 2", "Half Life 2" | "Pokémon Quest", "Pokemon Quest" (accents are kept) |
| "Control™", "Control" | "Thimble & Thorn", "Thimble and Thorn" ("&" is not "and") |
| "Tom Clancy’s Ghost Recon®: Wildlands", "Tom Clancy's Ghost Recon Wildlands" | "Fathom II", "Fathom 2" (numerals are not Roman ones) |
| "Mr. Prepper", "Mr Prepper" | "Halflife", "Half-Life" (a break is not nothing) |

A name with no letter or digit (empty once normalised) is the same as nothing. A game added by hand, or of no store,
is never the same game as another. Two copies in the same store are not "elsewhere".

The tests: `crates/model/src/names.rs`.

## 3. Which stores can install it now

`Source::can_install(game)`: the store has an installer (`Source::installer`; its route, or its fallback to the
store's own way, takes the install), or its own way (`Source::ask(game, Ask::Install)`) gives something to open. So
today: Steam always; Epic always (legendary, or the launcher's link); GOG with gogdl or Galaxy; Ubisoft Connect by its
`uplay://` address; Battle.net with its client; Xbox only where the game's Store id is to hand (`MicrosoftGame.config`,
which a game not installed seldom has); the EA app never (installs.md 5).

`ask` may read a client's files, so it is not asked on the UI thread. The executable asks it with the rest of
Storage, on Storage's own thread (`app::storage::read`), for the games not installed that the library has from more
than one launcher (`app::storage::twins`, one pass over the library by normalised name), and carries the ids that can
in every snapshot (`Storage::installable`). It is read when Storage is (after the first frame, once the sources have
been read, when an install finishes or a game is uninstalled, as Settings > Storage opens) and after every update of
the launchers (Update now, a launcher switched on). Until it has been read, no other launcher is offered: Install
goes on as before.

The log says, for each game two or more stores can install:
`storage: Quiet Orbit can be installed from Steam, Epic Games`.

## 4. The question

| | |
|---|---|
| Where | In the place of what asked (`PageAction::Replace`): over whatever the Tile menu or the Game hub was over, the screen dimmed (`menu_panel::DIM`), the panel in the middle, flying up as the Tile menu's question does |
| Title | **Install from** (guessed) |
| Under it | The game's name, dim, in the prompt's message place: "Quiet Orbit" |
| Rows | A launcher each, in Settings > Launchers' order (Steam, Epic Games, GOG, EA app, Ubisoft Connect, Battle.net, Xbox): its mark (the store's own, `drawn`, home.md 3.3.1; 26 square in the icon's place) and its name as the screens write it (`home::Source::name`) |
| Focus | On the launcher of the game pressed, wherever it is in the list |

**A** goes on with the launcher chosen exactly as Install on that launcher's copy alone would
(`tile_menu::install_from_store`), looked at again as it is chosen: where its store has more than one folder and can
be told which, "Where do you want to install X?" takes this question's place (storage.md 5); else the store is
asked at once (`Effect::Install` of that copy) and the game joins the Queue; the licence agreement, the Toasts and
Manage Queue follow as for any install (manage-queue.md 10). A copy that has been installed or queued meanwhile asks
nothing. **B** closes the question and installs nothing; what asked has gone already. Up and Down move along the rows
and stop at the ends; nothing else does anything.

**Sounds**: as the folder question's (storage.md 5.5): Install is a menu row that goes on, `GuideSelect`, and the
question is in its place with no sound of its own; then `GuideMove` on a move, `PromptClose` on A or B.

**The focus log**: `InstallFrom: Launcher(0) Steam, the one pressed of Quiet Orbit`, `InstallFrom: Launcher(1) Epic
Games of Quiet Orbit`.

Pictures: `crates/screens/tests/reference/install-from.png` (Steam, Epic Games and GOG, focus on Steam, the one
pressed; the EA app's copy left out, as it can't install), `install-from-other-launcher.png` (focus on Epic Games).

## 5. Tests

- `crates/model/src/names.rs`: the rule, both columns of the table above.
- `crates/app/src/storage.rs`: the twins found in one pass, and only those a store can install made installable.
- `crates/screens/src/install_from`: which launchers are offered, in which order, and which are not (installed,
  queued, switched off, not installable, a second copy in one launcher, a game pressed that can't be installed).
- `crates/screens/tests/install_from.rs`: the question from the Tile menu and the Game hub, focus and sounds, A on
  another launcher (its install), on the one pressed (as before), into the folder question of a store that asks one,
  B; one launcher only, or the rest installed: no question. The two pictures.
- `e2e/payloads/install-from`: the fake Steam and a fake Epic owning the same game (`standin make-epic --owning`,
  `Fixture::owning`), Install on Steam's tile, the question by its focus line, Epic Games chosen, and Epic's own
  install asked for (`store: asked Epic Games to install Quiet Orbit: ...`, the stand-in launcher's `epic-url: asked
  to install`), Steam asked nothing.

## 6. Not done, and hooks

- **One tile for a game owned twice** is not this: tiles are left as they are.
- **A copy installed elsewhere** is not said in the question (section 1).
- **A helper added or removed** (Settings > Helpers) does not read `installable` again by itself; the next Storage
  read does. Neither changes it today: Epic and GOG can be asked their own way without one.
