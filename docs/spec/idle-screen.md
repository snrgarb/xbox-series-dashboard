# Idle screen

With no input for a while the screen dims, over everything but Toasts. Later it shows things over the dim: the console's idle panel. The first input after that only wakes it. The Bundle's glossary entry is `CONTEXT.md`, "Idle screen". Its sources are `C/IdleState.cs`, `C/ExtensionCore.Idle.cs`, `E/DimWindow.xaml(.cs)` and `E/IdleCardControl.xaml(.cs)`. The port's code is `crates/screens/src/idle/`, `crates/shell/src/idle.rs` and `crates/input` (the pads' activity).

## 1. Stages

| Stage | After | What shows |
|---|---|---|
| Awake | - | nothing |
| Dimmed | **Dim after** (default 2 min) with no input | 70% black (`#B3000000`) over the whole screen |
| Showing things | **Show things after** (default 5 min) with no input | the dim, the panel on the right half and the time at the bottom left |

- Both times count from the last input, not from each other. With the two equal, the screen dims and shows things at once.
- Show things after on Never: it only dims. Dim after on Never: the screen stays awake (Show things after is then Never too, section 6).
- The dim fades in over **800 ms** (CubicOut) and out over **250 ms** (CubicOut).

## 2. What counts as input

The idle time counts from the later of two clocks. Both are behind a seam, so the core is told a moment and reads no clock itself:

- **Windows' last input** (`GetLastInputInfo`): the keyboard and the mouse, anywhere in the session, whichever window is in front. The value is read against `GetTickCount`, as how long ago, so the tick's 49.7-day wrap does no harm (`shell::idle::SystemInput`). Elsewhere than Windows it is a stand-in that is told (`FakeInput`), and on Linux only the Dashboard's own presses count.
- **The pads** (`input::Pad::last_used`), which Windows does not count. A pad counts as used while it is off its rest (any button, a trigger past 30, a stick past its deadzone of 7849 left or 8689 right, measured radially). It also counts when it moved since the last reading by a trigger step of 16 or a stick step of 2048. A pad lying still, even with a stick drifting inside its deadzone, does not count.
- Every press, key and typed text that reaches the Dashboard counts as well.

Toasts neither wake the screen nor count as input. A game stopping changes nothing.

## 3. Holding off

- The count **waits during Start-up and while a Launch screen is up**. It starts again from zero once they are gone: a game launched counts from its Launch screen going. A launch while dimmed wakes the screen. Start-up is the Boot video, Who's playing today? and Signing you in (start-up.md), however long Who's playing today? waits for someone; the count starts from zero as Home shows after. While it shows it has the pad to itself, so nothing of it reaches the Idle screen. Waking from sleep does nothing to it (start-up.md 8): it asks nothing, so it has no reason to take the Idle screen down.
- **There is no media or video hold-off**, as the Bundle has none. A film playing in another app is dimmed after Dim after just the same. (Windows' own display-off timers respect `ES_DISPLAY_REQUIRED`; the Bundle's Idle screen does not ask, and nor does the port.)

## 4. The panel (Showing things)

`E/IdleCardControl.xaml`. Measured at 1920×1080:

- **Panel:** the right half of the screen and two more, `round(w/2)+2` = 962 wide, the full height. It flies in from the right over **500 ms** (CubicOut; under Reduce motion as ever, as the Bundle's). The time fades in with it. On wake the panel and the time fade with the dim.
- **Panel colour:** a game's card is `#303030`, a tip's `#647184`.
- **Time:** 64 bold, `#CCCCCC`, 80 from the left and 57 from the bottom.
- **Text:** 72 in from the panel's left, 78 from its right, from 75 down.
  - The header: 46, weight 350, `#E6FFFFFF`, in a 46-tall top line.
  - The title, 29 under the header: 64 bold, at most 3 lines, 37 short of the column's right. A tip's title is 46 bold.
  - The label, 26 under the title: 46 bold.
  - The description, 30 under the label: 46.
- **Art:** the panel's bottom 540. It is a game's hero art (UniformToFill, centred, top-aligned) rising out of the panel's colour: none of it a tenth of the way down, half at 35%, all of it at 60%. A shade goes over its lower half (`#00000000` to `#59000000`).
  - A game with no hero art has its cover there instead, blurred and darkened (`#99000000`). The cover itself sits on top, whole, 320 tall and 600 down, centred.
  - A tip has its pad in the art's place: 442×312 at y 506, centred across the panel 4 short of its right, in white line 3.2 wide, round-joined. The buttons it is about are filled white over their line: A, B, X and Y with their letter (22 bold) in the panel's colour, View and Menu with their mark cut out, the D-pad's four arms as one, the bumpers, the sticks and the Guide button. The triggers are not drawn.
- **Y prompt:** a 72 ring with Y in it, 62 up from the foot, then its words, 46 bold, 26 after it. A tip's prompt is at 60%.
- **Cards:** each shows for **20 s**, then crossfades to the next over **800 ms**. They come in a shuffled turn, reshuffled when the pool runs out, and never the same card twice running.

### 4.1 The cards

| Kind | Header | Title | Label | Description | Prompt | Y |
|---|---|---|---|---|---|---|
| Close achievement (5 games: the most achievements unlocked without all, then the nearer finished, then by name) | - (the Score row: the Rare diamond, the G disc 46 with the G knocked out in the panel's colour, the Score bold 46 13 after it; the game's share at the right; a 10-tall bar under it, `#008080` on `#838383`, 29 down; the title 40 under the bar) | "Ashfall Courier - Night Route" (the game's next achievement), or the game alone with none to name | "Close achievement" | the achievement's | See game | as Recently played |
| Recent unlock (the last 5 unlocked) | "Unlocked yesterday" ("Unlocked" with no time known), at the right, the Score row at the left | "Ashfall Courier - Ash Runner" | "Achievement unlocked" | the achievement's | See game | as Recently played; its icon 320 square at y 600 over the game's art, which is darkened `#80000000` under it |
| Recently played (the last 5 played) | "Played yesterday" | the game's name | "Recently played" | "12 h played" or "45 min played" | See game | wakes it and opens the game's Game hub, unless a game is running |
| Tip | "Tip" | the tip | - | - | Got it | wakes it |

When-words, by calendar day: today, yesterday, N days ago (2 to 6), a week ago, N weeks ago (2 to 4), a month ago, N months ago, a year ago, N years ago. Months are calendar months.

The tips are the Bundle's that apply to the port. The ones about the Guide are left out while the Guide is off.

- Press {Guide} anywhere, even in a game, to open the Guide.
- Press {Start} on a game tile to play it, favourite it or manage it.
- Press {Y} on Home to search your games and apps.
- Customise Home from a tile's menu, or Settings › Games & apps › Groups.
- Choose the Groups Home shows in Settings › Games & apps › Groups. (This is the Bundle's tip about Playnite's filter presets, as the port has them.)
- On the keyboard, {X} is backspace, {Y} is space, {Start} is Enter.
- See what's downloading in My games & apps › Manage › Queue.
- See what's playing in the Guide's Audio, under Now Playing.
- Your Score adds up achievements across every game. (Only while there is a Score.)

## 5. Waking

**Any input wakes it, and the press that wakes it does nothing else.** It does not move focus, reach a page, open the Guide or type a letter.

- A press of the pad, or a key that stands for one, while Dimmed or Showing things: woken. Its repeats and its release are swallowed too, and the next press of it is a press.
- The Guide button: `PadEvent::GuideDown` wakes the screen, and its tap or hold that follows is swallowed.
- Typing, on a page that takes text: woken, and nothing typed.
- Input that only the clock saw (the mouse, a stick moved inside what makes a press, a key in another window) waits **500 ms** for a press to come. If one comes, it is the swallowed wake press. If none does, the screen wakes anyway, "by input". The pad's thread sends its events before it marks the pad used, and the window reads the clock before it takes the events. So a press that made the clock move is always in the same turn or a later one.
- **Y on a game card** while no game runs wakes the screen and opens that game's Game hub. The Guide closes first, and any pages close.

## 6. Settings

Settings › General › **Idle** (`docs/spec/settings.md`, 6.7) has two dropdowns of the same steps: 1, 2, 5, 10, 15 and 30 min, 1, 2, 3 and 4 h, Never.

- Show things after is never shorter than Dim after. Its list leaves out the steps below Dim after, and raising Dim after past it raises it too.
- Both are saved at once, as the rows `idle.dim_after` and `idle.show_things_after`, in seconds or `never`. They count from the moment they are set.

## 7. Where it is drawn

Last on the **Overlay** (`docs/spec/guide.md`), over the Launch screen, the Guide, its dialogs, the Tile menu, the keyboard and a running game. While it shows, the Overlay is Passive: it takes no input and no foreground. Toasts are a window above the Overlay, so they show over the dim. Where there is no Overlay, the Idle screen dims Home's window instead.

## 8. For tests

- `--idle-after <dim s>,<show s>`: the two times in seconds, either of them `never`, in place of Settings'. They are not saved, and they stay in force if Settings changes.
- The log lines, whatever the flags:
  - `idle: dim after 3 s, show things after 6 s (--idle-after)` at the start
  - `idle: dimmed at <ms> ms`
  - `idle: showing things at <ms> ms`
  - `idle: woken by <what> at <ms> ms`, where `<what>` is a button's name (`A`, `Right`, `Y`), `Guide`, `typing` or `input`
- `--log-focus`'s `focus:` line is not said for the wake press, because focus does not move.

## 9. Differences from the Bundle

- **Achievement cards come from Steam's files** ([../achievements.md](../achievements.md), #141), not PlayniteAchievements: only Steam games have them. They are in the pool only while there are totals, and only for games in the library, never hidden ones.
- **The nearly-finished card's achievement** is the game's locked one most players have (`model::achievement::nearest_locked`: the highest known rarity, secret ones and unnamed ones never), where PlayniteAchievements names it.
- **The Score tip** says "across every game" where the Bundle's says "across every platform": the port reads Steam's alone. It is in the pool only while there is a Score.
- **Tips:** the ones about Discord and the Boot video are left out until those come. The button tokens are written as words ("Y", "Menu", "the Guide button") rather than drawn as glyphs in the line.
- **The tip's pad's Guide button** is the Bundle's (#275): the Xbox sphere from Kenney's Input Prompts (`xbox_guide.svg`, CC0 1.0), the XAML's path data (`path_data::marks::XBOX_GUIDE_PIECES`, `XBOX_GUIDE_TOP`), its three lower pieces outlined 1.8 wide and its top filled, scaled by 0.725 about the button's middle; lit, the whole sphere scaled by 0.56 cut out of a white disc of radius 19.5 (`idle::pad::guide_mark`). Until #275 a plain ring stood in for the sphere; NOTICE.md credits the mark. The rest of the pad is the Bundle's: its traced body, bumpers and D-pad are the XAML's path data, carried over by the user's request (#251), and its buttons the XAML's circles, all 3.2 wide.
- **Dim after on Never** keeps the screen awake. The Bundle turns the display off; the port does not touch the display (an open question for the plan).
