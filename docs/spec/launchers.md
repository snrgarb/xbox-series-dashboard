# Launchers (Settings > Games & apps > Launchers; #163)

The page that says which launchers the Dashboard reads games from, switches one off or on, and brings the library up to date while the Dashboard runs.

**The layout is the port's own.** The Bundle has no such page: in Playnite, library plugins and "Update game library" did this job, and Playnite's windows go away in the port (settings.md, section 14). The page is made from the Settings pages' own parts (settings.md, section 4: the description, the Row, the checkbox glyph, the pane button) at their measurements, on the 892-wide column the Settings pages use from (520, 210). Nothing here is the Bundle's, and nothing comes from Playnite: there is no import of Playnite's library or settings anywhere.

## 1. Its box

**Launchers**, icon U+E8F1, the first box of Games & apps' Library section (520, 262, 437, 131), Games & apps, Groups and Art under it ([settings.md](settings.md) 6.1). Right from it is Storage, the Installs section's first.

Picture: `crates/screens/tests/reference/settings-theme-settings-launchers-box.png`.

## 2. The page

| Element | y | Notes |
|---|---|---|
| Header | as every page | "Launchers" |
| What it does | 210 | `controls::description`, wrapped at 892: "Your games come in from the launchers switched on here, with their play time and collections. Switch one off to hide its games until you switch it on again." |
| The count | 18 under it | A label: "14 games from 2 launchers", "1 game from 1 launcher", "No games from your launchers". The games shown, from the launchers switched on |
| The Rows | 26 under the count | One a launcher, 892 x 66 every 74 (settings.md, 4.3), in the order Steam, Epic Games, GOG, EA app, Ubisoft Connect, Battle.net, Xbox |
| Update now | 40 under the last Row | A pane button (settings.md, 4.4), and beside it, 30 to its right, one line (section 4) |

A Row: the launcher's mark (the drawing the tiles carry, home.md 3.3.1, `home::source_mark`, 28 x 28, centred 45 in), its name where a Row's text goes, what it says (right-aligned, from 340 in to just before the checkbox), and its checkbox (28 px, 18 from the right): ticked when the launcher is switched on. A launcher that cannot be switched (section 3) is drawn at 40 % and has no checkbox. The page scrolls as the other Settings pages do (`controls::scrolled`), so focus is always whole on screen.

What a Row says:

| State | Text |
|---|---|
| On and read | "14 games, read 5 minutes ago" ("read just now", "read 1 hour ago", "read 3 days ago") |
| On, last read failed | "14 games, couldn't be read" |
| On, never read | "Not read yet" |
| Off | "Off: 14 games kept" |
| Not on this PC | "Not installed" |
| Not on this PC, games kept from before | "Not installed: 14 games kept" |

The game count is the launcher's games at its last read (not gone), whether or not it is switched on.

Pictures: `settings-launchers.png` (at rest, focus on Steam), `settings-launchers-updating.png` (Epic off, Update now working), `settings-launchers-updated.png` (its summary), `settings-launchers-refused.png` (Steam refused while a game plays), all under `crates/screens/tests/reference/`.

## 3. Switching a launcher

- **All on by default.** A launcher's switch is the settings store's `launcher.<key>` (`launcher.steam`, `launcher.epic`, `launcher.gog`, `launcher.ea`, `launcher.ubisoft`, `launcher.battlenet`, `launcher.xbox`): no row until it is switched, `false` when off (settings.md, 16.4). Shared by every Profile.
- **Off hides, never deletes.** Off, the launcher's games leave every list at once: Home's rows and Groups, My games & apps (its tab on Full library too, [filters.md](filters.md) 4.2), Search, Play history, the Queue, the Idle cards, the achievement totals and the Achievements page. Its rows stay in the database as they are, and the launcher is not read while it is off (at start-up or by Update now), so nothing of its is marked gone. Switched on, its games are back as they were, with their play time, Play sessions, favourite, hidden and Groups, and it is read again at once (section 4) to catch up.
- **Who can be switched.** A launcher that is on this PC, or one that is not but has games kept from before (so they can be hidden), or one switched off (so it can be switched on again). A launcher that is not installed, has no games and is on is dimmed, says "Not installed" and has no switch: focus passes over it.
- **A game playing.** A launcher whose game is running or launching **cannot be switched off**: the page says "Quit <game> before switching <launcher> off." beside Update now, nothing is saved, and the log says `launchers: steam stays on: Halyard Point is playing`. The executable refuses it too, whatever asks. Reason: the running game would leave the snapshot under the Guide and the Launch screen, which hold it by its place in the list.
- **A game installing.** A launcher with an install under way **can** be switched off. The game leaves the Queue at once with the launcher's other games; the install itself is the launcher's and goes on (nothing is cancelled). The Queue's record of it is kept, and is neither followed nor asked about while the launcher is off. Switched on, the launcher is read again and the Queue looks at its installs, so an install still under way is back in the Queue where it is, and one that finished in the meantime is an installed game.

## 4. Update now

Reads every launcher switched on again, off the Dashboard's thread, and brings into the library what is missing or out of date:

- what `Database::sync_all` merges from a read (sources.md, "How the library merges them"): games added, games gone, installed state, play time and last played, size, the store's collections as Groups;
- then, once the read is in: the art lookup starts over for the games with none (if art online is on), every game's achievements are read again (`achievements::Ask::ReadAll`: each game's schema and unlocks, and rarity where it is more than seven days old), and the Queue looks at the launchers' installs again.

**The summary line** counts what the read changed: "Updated: 1 new game, 2 games gone, 1 installed, 1 uninstalled, play time for 3, Groups changed." or "Your library is up to date.", with ", <launcher> couldn't be read" for each launcher whose read failed. Art and achievements come in afterwards, in the background as they always do, and are not in the line.

- **Never blocks.** The read is a thread of its own with its own database connection; the frame and the pad go on. While it works, the line beside the button is a spinner and "Updating your library...".
- **Pressed while it works**, Update now does nothing. A launcher switched on while an update works is read when it is done.
- **Fails**: "Your library couldn't be updated. Try again later." The library is as it was.
- **Each Row's "read ... ago"** is that launcher's last read, Update now's or the start-up's.

## 5. What each launcher can give

From [../sources.md](../sources.md), "The stores". What a launcher does not keep on the PC, Update now cannot bring in.

| Launcher | Games | Installed | Play time, last played | Collections as Groups | Art | Achievements |
|---|---|---|---|---|---|---|
| Steam | Owned and installed | Yes | Yes | Yes (its collections, favourites, hidden) | Yes, its cache; the rest online | Yes (`appcache/stats`), with rarity online |
| Epic Games | Owned (catalogue cache, once signed in) and installed | Yes | No (on Epic's servers) | No | Online only, its catalogue's addresses | No |
| GOG | Installed; owned too with Galaxy | Yes | With Galaxy | No | With Galaxy, its cache | No |
| EA app | Installed only | Yes | No | No | No (online lookup only) | No |
| Ubisoft Connect | Owned (its cache) and installed | Yes | No | No | Its cache | No |
| Battle.net | Installed only | Yes | No | No | No (online lookup only) | No |
| Xbox | Installed only, no sign-in | Yes | No | No | The package's own logos | No |

## 6. Focus and the log

Focus lines (`--log-focus`), with the page's state after the `;`:

```
focus: Settings: Games & apps: Launchers
focus: Settings: Launchers: Steam; on, 5 games; 14 games from 2 launchers; ready
focus: Settings: Launchers: Epic Games; off, 9 games; 5 games from 1 launcher; ready
focus: Settings: Launchers: GOG; off, 3 games, not installed; ...
focus: Settings: Launchers: Steam; on, 5 games, unreadable; ...
focus: Settings: Launchers: Update now; 14 games from 2 launchers; updating
focus: Settings: Launchers: Update now; 15 games from 2 launchers; updated: 1 new game, play time for 1
focus: Settings: Launchers: Update now; ...; update failed
focus: Settings: Launchers: Steam; ...; refused: Quit Halyard Point before switching Steam off.
```

The line changes as the state does, an update finishing included, without a press.

`--log` lines:

```
launchers: epic off: 9 games hidden
launchers: epic on: 9 games back
launchers: steam stays on: Halyard Point is playing
launchers: update: reading every launcher switched on
launchers: update: reading epic
launchers: update: steam, gog read in 41 ms: 1 new game, play time for 1
launchers: update: nothing read in 2 ms: nothing new
launchers: update: already under way
launchers: update: no library database to update
```

## 7. Tests

- `crates/library/src/launchers/tests.rs`: off hides from every query and the Queue, on gives back the same snapshot, an off launcher is not read, the read state is kept, the summary's wording.
- `crates/screens/tests/settings.rs`: the walk, a launcher switched off and on through a real (in-memory) database, the refusal while a game plays, Update now once while it works, and the pictures.
- `crates/source-steam/src/fixture.rs`: `fixture::change`, which the end-to-end test drives with `standin steam-change <dir> add-game | played <appid> <minutes>`.
- `e2e/payloads/launchers/run.ps1` (the TV PC, safe mode): the page opened with the pad; Epic switched off and on, its games leaving Home and Search and coming back with their play time; a game added to the fake Steam and another's play time changed; Update now bringing both in with its summary line.
