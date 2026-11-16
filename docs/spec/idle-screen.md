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

- The count **waits during Start-up and while a Launch screen is up**. It starts again from zero once they are gone: a game launched counts from its Launch screen going. A launch while dimmed wakes the screen. Start-up is the Boot video's (#126) for now, and Who's playing today?'s when that comes (#127). While it shows it has the pad to itself, so nothing of it reaches the Idle screen.
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
  - A tip has its pad in the art's place: 442×312 at y 506. The buttons it is about are filled.
- **Y prompt:** a 72 ring with Y in it, 62 up from the foot, then its words, 46 bold, 26 after it. A tip's prompt is at 60%.
- **Cards:** each shows for **20 s**, then crossfades to the next over **800 ms**. They come in a shuffled turn, reshuffled when the pool runs out, and never the same card twice running.

### 4.1 The cards

| Kind | Header | Title | Label | Description | Prompt | Y |
|---|---|---|---|---|---|---|
| Recently played (the last 5 played) | "Played yesterday" | the game's name | "Recently played" | "12 h played" or "45 min played" | See game | wakes it and opens the game's Game hub, unless a game is running |
| Tip | "Tip" | the tip | - | - | Got it | wakes it |

When-words, by calendar day: today, yesterday, N days ago (2 to 6), a week ago, N weeks ago (2 to 4), a month ago, N months ago, a year ago, N years ago. Months are calendar months.

The tips are the Bundle's that apply to the port. The ones about the Guide are left out while the Guide is off.

- Press {Guide} anywhere, even in a game, to open the Guide.
- Press {Start} on a game tile to play it, favourite it or manage it.
- Press {Y} on Home to search your games and apps.
- Customise Home from a tile's menu, or Settings › Theme settings › Personalisation.
- Choose the Groups Home shows in Settings › Theme settings › Personalisation › Groups. (This is the Bundle's tip about Playnite's filter presets, as the port has them.)
- On the keyboard, {X} is backspace, {Y} is space, {Start} is Enter.
- See what's downloading in My games & apps › Manage › Queue.
- See what's playing in the Guide's Audio, under Now Playing.

## 5. Waking

**Any input wakes it, and the press that wakes it does nothing else.** It does not move focus, reach a page, open the Guide or type a letter.

- A press of the pad, or a key that stands for one, while Dimmed or Showing things: woken. Its repeats and its release are swallowed too, and the next press of it is a press.
- The Guide button: `PadEvent::GuideDown` wakes the screen, and its tap or hold that follows is swallowed.
- Typing, on a page that takes text: woken, and nothing typed.
- Input that only the clock saw (the mouse, a stick moved inside what makes a press, a key in another window) waits **500 ms** for a press to come. If one comes, it is the swallowed wake press. If none does, the screen wakes anyway, "by input". The pad's thread sends its events before it marks the pad used, and the window reads the clock before it takes the events. So a press that made the clock move is always in the same turn or a later one.
- **Y on a game card** while no game runs wakes the screen and opens that game's Game hub. The Guide closes first, and any pages close.

## 6. Settings

Settings › Theme settings › **Idle** (`docs/spec/settings.md`, 6.7) has two dropdowns of the same steps: 1, 2, 5, 10, 15 and 30 min, 1, 2, 3 and 4 h, Never.

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

- **No achievement cards.** The Bundle's nearly-finished games and last unlocks come from PlayniteAchievements, which the port has not got. Only Recently played and tips are in the pool.
- **Tips:** the ones about Discord, the Score and the Boot video are left out until those come. The button tokens are written as words ("Y", "Menu", "the Guide button") rather than drawn as glyphs in the line.
- **The tip's pad** is plain shapes (rings and rounded rectangles), not the Bundle's traced outline. The Guide button is a ring.
- **Dim after on Never** keeps the screen awake. The Bundle turns the display off; the port does not touch the display (an open question for the plan).
