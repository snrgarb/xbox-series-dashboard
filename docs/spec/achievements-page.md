# Achievements page specification (designed here: the Bundle never built it)

`~/projects/xbox-series-theme/CONTEXT.md` names the **Achievements page**: "the Game hub's view of one game's achievements, opened from the hub and shown in its place". At the Bundle's HEAD it is not built (game-hub.md 6.6): no action, binding or card opens it, and `E/AchievementPages.cs` pages the achievement Toast's text, not a page. So this page is the port's own, laid out in the hub's own parts (game-hub.md) so that it reads as the hub's: nothing in it is measured from a picture of the Bundle. Built in #141, on the data of #140 ([../achievements.md](../achievements.md)).

All numbers are design units on a 1920x1080 canvas, 1:1 as the hub's.

## 1. Way in, way out

- **In:** A on the hub's **Your game stats** card of a game that has achievements (game-hub.md Q12: the card where the Achievements row is). The hub's A on that card did nothing until now. A game with none has no row and A on the card still does nothing; there is no "Needs PlayniteAchievements" state, nothing being needed.
- **Shown in the hub's place:** the page is a page of its own over the hub (`pages::Open::Achievements`), drawn on the hub's band with the hub's picture of the screen behind it (the same `Page::behind` number as the hub's), so the strips above and below the band do not change.
- **Out:** B, back to the hub with focus on Your game stats, as it was. Menu opens the game's Tile menu as on the hub (`Opener::GameHub`). A, X, Y, View, the bumpers and the triggers do nothing. The hint bar says "More options".

## 2. Where the list comes from

No snapshot of the library carries a game's whole list. The page says which game it wants (`Dashboard::achievements_wanted`, the game's id in the library database), and the shell gives it (`Dashboard::set_achievements`): read from `library::Database::achievements(game)` as the page opens, and again when the achievement thread's update names the game in `icons` or brings a new snapshot. On its first opening for a game the shell asks for the game's icons (`Ask::Icons`), which come only with Settings' Download art online on. Until the list is given the page shows its progress card and no list.

Log: `achievements: the page lists <n> (<u> unlocked, <i> with icons)` each time the list is given.

## 3. Layout

| Part | Value |
|---|---|
| Backdrop | the hub's: its art in the band, darkened, the picture of the screen behind in the strips (game-hub.md 2) |
| Header | the hub's (game-hub.md 3): the game's name, and "Achievements" on the facts line |
| Progress card | at the hub's first card's place, x 80, y 270.4, 416 x 500 (the hub's card height), titled "Your progress" as the hub's cards are titled. Its rows: the hub's Achievements row ("u / n", the share, the bar); then label-and-value pairs as Your game stats has them: "Score", "`{unlocked score:N0} of {all:N0}`"; "Rare", "`{k} of {n} unlocked`" (only when any is Rare); "Last unlocked", the date (only when a time is known) |
| List | from x 512 (16 past the card) to 1840; two to a row, 16 apart: each box 656 x 120, rows 136 apart from y 270.4; five rows show |
| A box | fill `#4D2A2A2A`, radius 4. Icon 88 square, 16 in, radius 4. Text from 120 in to 20 short of the right |
| Name line | the name (Segoe UI 26, white, one line, cut); at the right the Score: a G disc 22 across (black G) and the number (26, white), 6 apart |
| Description | 50 down: Segoe UI 20, dim, one line, cut |
| Foot | 82 down: "Unlocked 9/20/2026" in white (or "Unlocked" with no time known), or "Locked" dim; at the right "`{r}% of players`" dim, with a bold white "Rare" 10 before it for a Rare one; nothing at the right while the rarity is not known |
| Focus | the hub's card ring (`focus_ring_cornered`, radius 14) round the box |

**Icons.** An unlocked achievement shows its icon, a locked one its grey icon (Steam's `icon_gray`), each once in the cache. One not fetched, and every secret one still locked, is the grey `#4A4A4A` square with the trophy, as the Guide's recent achievements stand in.

**Secret ones** still locked say "Secret achievement" and "Keep playing to unlock this secret achievement.", as the console does, and show the stand-in icon. Their Score and rarity show: they tell nothing of what the achievement is.

## 4. Order and focus

- **Order:** unlocked first, newest first (those whose time is not known after those whose time is), then the locked in the store's order, the secret ones last.
- **Focus** starts on the first, the newest unlock. Left and Right go along a row and stop at its ends; Up and Down go along a column; Down from above the empty place of a shorter last row goes to its last achievement. Nothing wraps.
- **Scrolling:** by the least that shows focus's row; no animation.
- **When the list comes again** focus stays on its achievement (by key) wherever it is now; if it has gone, on the same place, clamped.
- `--log-focus`: `focus: Achievements: <appid> <key>; <u> of <n> unlocked`, for example `focus: Achievements: 9100010 ACH_ASH_RUNNER; 2 of 6 unlocked`; `<appid> none` with no state until the list is given.

## 5. Decisions (no Bundle source)

- The way in is A on Your game stats (game-hub.md Q12), not a fifth action: the hub's actions are the Bundle's, and the card is where the row is.
- The page is its own page over the hub, not a third page of the hub's toggle: B goes back to the hub with focus where it was, as "shown in its place" reads.
- The list's box, type and colours are the hub's cards' and facts', so nothing new is introduced to the look; the progress card reuses Your game stats' rows.
- No sorting or filtering, no "Rare" filter, no per-achievement page: nothing in the Bundle asks for them.

## 6. Tests

`crates/screens/src/achievements/` (order, navigation, focus kept, scrolling, B and Menu, no page without achievements), `crates/screens/tests/shots.rs` (`achievements-page`, `achievements-page-scrolled`, `achievements-hub`), and `e2e/payloads/achievements` (the walk hub > page > hub on the fake Steam's Ashfall Courier).
