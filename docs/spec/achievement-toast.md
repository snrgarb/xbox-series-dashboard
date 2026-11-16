# The Achievement unlocked Toast (#142)

The Bundle's sixth Toast (`~/projects/xbox-series-theme/CONTEXT.md`, **Toast**, **Rare**, **Score**), built from the Bundle at HEAD: `extension/XboxSeries.Extension/AchievementToast.xaml` and `.xaml.cs` (the pill and its timeline), `AchievementPages.cs` (its pages), `XboxSeries.Core/ExtensionCore.Achievements.cs` (what is told of, and how long) and `ExtensionCore.cs` (the queue's waits), `XboxSeries.Core.Tests/AchievementToastXamlTests.cs`. The data is #140's ([../achievements.md](../achievements.md)). There is no picture of the current Bundle's achievement Toast (the `achievements-20261110-*` sets are Bundle 0.2.0, the banner of that time): every value here is from the XAML and the code.

In the port: `screens::toasts::pill` (the drawing and the timeline), `screens::toasts::achievement` (what a Toast says, the notification, the second's gathering), `Dashboard::unlocked` (the way in), `crates/app/src/window.rs` (hands it `achievements_update().unlocked`).

## 1. What is told of

- **What**: only what the achievements' thread sends as new: an unlock found while watching the running game, or the one that just exited, since it started (docs/achievements.md, "What is told of as new"). What the files had as the Dashboard started, and an older session's unlock, never are.
- **Gathered for a second** (the Bundle's `UnlocksGatherFor`): the first unlock waits 1 s; every one found by then joins it (`toasts::achievement::Gather`, timed on the Dashboard's clock and in `Dashboard::timer`). The thread already sends what one read finds as one update.
- **One unlock**: "Achievement unlocked", its icon, its Score and name, then its description.
- **One Rare unlock told of alone** (rarity known and under 10%): the rare form, "Rare Achievement unlocked - 7.4%" (`{0:0.#}`: whole numbers bare, else to a tenth, halves away from 0), blue, the spinning diamond, its own sound.
- **Several together**: one ordinary Toast, "<n> achievements unlocked", the trophy in the circle and no Score line, a Rare one among them or not.
- It waits its turn as a notification's does, behind the greeting during Start-up too.

## 2. The pill

All design units of the 1920 x 1080 canvas. The pill is 778 x 128 at x 571, y 888: in the middle across, its foot 64 above the screen's, where the banner sits.

| Part | Value |
|---|---|
| Pill | two round ends 128 across and the span between them (x 64..714), the pill's colour |
| Circle | 126 across, a unit inside the left end (x 1..127), riding the left end |
| Ordinary | pill `#107C10` (the Toast's green), circle `#0FC60F`, going: dark disc `#107C10`, halo `#0B550B` |
| Rare | pill `#0F4E7E`, circle `#0D7FC9`, dark disc `#0F5180`, halo `#0B3456` |
| Icon | the achievement's unlocked icon filling the circle, cut round; under it the trophy (Segoe UI Symbol U+1F3C6, 60, white), which shows while the icon is not fetched, and for several |
| Diamond | 80 x 79, five slices of crown over five of pavilion, each turning to the next's place every 0.66 s (the Bundle's `View`, `Glint`, facet colours `#CBD9E6 #9FBAD3 #7FAEDB`, `#E2EAF2 #B4C8DA #4F8AC2`, shade `#123F66`, edges `#B3FFFFFF` 0.7) |
| Text column | x 153..716, y 4..124, clipped |
| Type | Segoe UI 28.5, white, lines 45 apart |
| Page 1 | the line (ellipsis), and under it the Score badge (a disc 33.5 `#FCFFFF`, "G" Segoe UI Semibold 22 in the pill's colour, 11 to the Score), "`<Score>` -", 9, the name (ellipsis); the line alone, centred, without a name |
| Description | the whole of it, wrapped to the column, two lines a page, never cut short |
| Prompt | "Press", the Guide button's mark 34 across (16 before, 22 after), "to view", centred; none with the Guide off, the last page staying |
| Shine | a band of white, 20% at its brightest, leaning "/", its middle from 185 to 590 across the pill, once on the first page and once on the description's first |

## 3. The timeline

From the Toast's first frame (seconds), as the Bundle's storyboards (`AchievementToast.xaml.cs`):

| | |
|---|---|
| 0.117 to 0.217 | the circle pops in at the pill's centre, 0.4 to 1 of its size (cubic out), dark then bright; the Xbox mark fades in from 0.167 |
| 1.052 to 1.085 | the mark gives way: to the icon (or trophy) zooming from 3x to its place by 1.485 (quintic out); a Rare one's to the pale inside of the gem, the diamond flying out of it from 3x (quintic out) through a round window that opens to 1.5x, the inside fading 1.25 to 1.485, and spinning from 1.052 on |
| 1.485 to 2.385 | the pill unfurls from behind the circle (quintic out), the circle riding its left end out |
| 1.786 to 2.386 | the text fades in, drifting 4 left into place |
| 2.954 | the first shine (1.47 s) |
| 4.122 | the first page leaves up 24 over 0.18 s (cubic in); the description comes from 58 below at 4.30 over 0.38 s (cubic out); the second shine at 5.222 |
| 7.091, 7.158 to 7.341 | the last page is cut, the prompt fades in |
| 8.5 | it goes (8.45 a Rare one; 3 s more an ordinary one for each further page of a long description, about 80 characters a page). More pages than one are held for less where they must be, so the prompt is in before it goes (`plan`, the Bundle's `AchievementPages.Plan`) |
| going, 0 to 0.5 | the pill collapses into the circle (quintic in), the text fading from 0.1 over 0.27, the icon or gem from 0.4 |
| going, 0.5 to 1.12 | the dark circle and the mark, which swells (to 1.11 at 0.813) as a bright ring closes in, then shrinks to a dot inside a dark halo, gone at 1.12 |

The next Toast waits for the going and then the usual half second. Reduce motion changes nothing of it (CONTEXT.md: "an achievement's Toast move as ever"). A frame at rest (a screenshot) shows it at 2.7 s: the first page in, before the shine. Between its moves it asks for no frames, but a Rare one's diamond spins for as long as it shows.

## 4. Sound

`Notification.wav` as an ordinary one shows, `RareAchievement.wav` (`Sound::RareAchievement`) as a Rare one does, from the media folder when it is there; silent without it. `standin make-tones` writes a tone under the new name too.

## 5. The Guide button, and the Notifications Flyout

- **A tap while it shows** opens the Guide on Profile & system, Rare or not, as the Bundle's (CONTEXT.md, "pressing the Guide button while one is showing"). The pill goes over the Guide.
- **Listed in the Notifications Flyout** (port only; below): "Achievement unlocked in Ashfall Courier: Night Route", "Rare Achievement unlocked in Ashfall Courier: No Detours", "2 achievements unlocked in Ashfall Courier", the trophy in the bell's place. It is listed, and its Toast shows, whether or not the Flyout is open.
- **A on that row** closes the Guide onto the game's Achievements page (docs/spec/achievements-page.md) over Home, in front of a running game as the Guide's other pages are; B is Home. A game with no achievements now opens its Game hub; one the library has lost, nothing. A on any other notification does nothing, as before. The sound is the Guide's select.

## 6. Log

- `toasts: shown "Achievement unlocked" (Achievement) "Night Route", score 20, 0 waiting at <ms> ms`
- `toasts: shown "Rare Achievement unlocked" (RareAchievement) "No Detours", score 30, 7.4%, 0 waiting at <ms> ms`
- `toasts: shown "2 achievements unlocked" (Achievement), 0 waiting at <ms> ms`
- then `toasts: going` and `toasts: gone`, as every Toast's. The Flyout's focus is the `guide:` line's `flyout Notifications Item(<n>)`; A on the row is `focus: Achievements: <appid> <key>; <u> of <n> unlocked` once the list is read.

## 7. Differs from the Bundle

- **The Xbox mark** in the circle and in the prompt is the plain white disc every placeholder mark is (the repository holds no trademark).
- **Listed in the Notifications Flyout, and A on it.** The Bundle's Flyout lists Playnite's own notifications and never an unlock; the ticket asks for the row, and A opens the game's Achievements page, which the Bundle never built.
- **Clipping.** The port clips to rectangles: the shine is kept to the span between the pill's ends (the Bundle's reaches into the right end's curve, at 10% or less), and the trophy, while it is zoomed past the circle, to a square inside it; the icon and the diamond are cut round, as in the Bundle.
- **The diamond's spin** is 24 painted pictures a turn (the Bundle interpolates 16 key frames).
- **The inside of the gem** is a radial gradient from its middle (`#F4F8FB` to `#A9BBCB`); the Bundle's starts up and left of it with a stop between.

## 8. Tests

`crates/screens/src/toasts/pill.rs` and `achievement.rs` (timeline, pages, durations, gathering, wording), `crates/screens/tests/achievement_toast.rs` (the queue, sounds, the Guide button, the Flyout's A, and pictures: `toast-achievement`, `-description`, `-prompt`, `-going`, `-coming`, `-icon-zooming`, `-rare`, `-rare-flying`, `toast-achievements-several`, `guide-notifications-achievement`), and the VM payload `e2e/payloads/achievement-toast`.
