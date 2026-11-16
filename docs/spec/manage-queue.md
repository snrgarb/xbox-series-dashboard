# Manage Queue specification (as the Bundle draws it)

Manage Queue lists the Queue (CONTEXT.md:38-39): what is installing, the current download first, and the games
installed lately. This document gives its layout and its buttons as the Bundle has them, then what the port decided
(#79). Manage, the page with the Queue box that opens it, is [my-games.md](my-games.md) 9.1; the nav's Queue line and
the current download at the nav's foot are my-games.md 2.5 and 2.6, and section 6 here.

Sources, as in my-games.md: E = the extension (`MyGamesControl.xaml`, `.xaml.cs`), C = the core (`ExtensionCore*.cs`,
`MyGamesState.cs`). **No picture of the Bundle's shows Manage Queue or Manage with something queued**:
everything here is from the XAML, and the port's screenshot tests (`crates/screens/tests/manage_queue.rs`) are drawn
from it.

## 1. Slot

A page of its own over the whole screen (E/MyGamesControl.xaml:819-950): the page fill `#FF1F1F1F` across all
1920 x 1080; the nav, its foot and the divider hidden (E/MyGamesControl.xaml.cs:375-380). The Manage nav item counts
as showing while it is up (:371), which nothing shows with the nav hidden.

| Part | Value |
|---|---|
| Title | "Manage Queue", Bahnschrift SemiLight 42, white, at x 96, y 72 |
| Scroller | Under the title's line (y 122.4) to the screen's foot, x 72..1848; its content 24 in, x 96..1824. Vertical only, no bar |
| Section header | Bahnschrift 30 white: "Recently installed" with 14 above and 14 below; "Installing" with 30 above and 30 below |
| End | 80 under the last row or the empty box |

## 2. Recently installed

Only when there is a game to show. A row of **Home's tiles at 200 x 200** (home.md section 3: cover, source disc,
status band, title band on focus), 16 apart, in a plain horizontal scroller with 24 round them for the ring
(E/MyGamesControl.xaml:838-851). It holds **up to 8** games: those seen to finish installing since start, the latest
first, then the rest of the installed games by `InstalledAt ?? Added` (InstalledAt as near as the install folder
tells), the latest first (C/ExtensionCore.Queue.cs:515-543).

## 3. Installing

One row per Queue entry, the current download first and then the rest in the Queue's order
(E/MyGamesControl.xaml:871-930; C/ExtensionCore.Queue.cs:554-600; C/MyGamesState.cs:325).

| Part | Value |
|---|---|
| Row | 250 tall, 16 to the next, x 96..1824, fill `#FF2D2D2D` |
| Cover | The row's first 250 across, the game's cover cropped to fill it, on `#FF1A1A1A` |
| Text column | 36 right of the cover, 40 short of the row's end, from 38 down |
| Title | Segoe UI 26 **bold**, white, one line, ellipsis, 160 short of the column's end |
| Speed | Right-aligned on the title's line: "106 Mbps", "323 Kbps"; decimal units, whole (C/ExtensionCore.Queue.cs `SpeedOf`) |
| State | Under the title: "Installing..." while it downloads, "Queued" while it waits its turn or is paused |
| Amount line | 34 under the state: "Network: {amount}" at the left, only when there is an amount; the share right-aligned, "1.9%" |
| Amount | "67.80 MB of 3.55 GB": two decimals, a GB being 2^30 bytes, MB below 1 GB, TB from 1024 GB, the unit chosen by the value as rounded (C/MyGamesState.cs:300) |
| Bar | 12 under the amount line, 5 tall, across the column: track `#FF4A4A4A`, the share filled white |
| Focus ring | `Margin="-8" BorderThickness="4" CornerRadius="8"`, `#FF2CA243`, with the focus glow |

A game that is not downloading reads "Queued", "0.0%" and an empty bar, whatever of it is down already, and has no
speed and no amount. A download whose store has said no bytes yet (or no whole) reads "Installing...", "0.0%", no
amount and no speed: the Bundle's `Network:` line and speed are bound to values that are null until then.

**Speed** (C/ExtensionCore.Queue.cs:587, `SpeedOf`): null until the bytes first move, then worked out from successive byte counts; 0 once
they have stood still 10 s.

## 4. Nothing queued

In place of Installing: a box 40 under what is above it (the title, or Recently installed), 330 tall, x 96..1824,
outlined 2 `#FF5C5C5C`, not filled. Centred in it: the Download glyph U+E896 at 64, then 40 under it "When games and
apps are installing, you'll see them here", Segoe UI 26 white (E/MyGamesControl.xaml:932-948). Recently installed
stays above it when it has games.

## 5. Buttons

(E/MyGamesControl.xaml.cs:1681, `OnQueuePageKeyDown`; C/ExtensionCore.cs:844-849.)

| Focus | Button | Does |
|---|---|---|
| A cover | Left, Right | Along the covers; nothing at their ends |
| A cover | Down | The first row |
| A row | Up, Down | Along the rows; Up from the first goes to the cover last focused |
| A cover or row | A | As on a Home tile: the game's hub, a second A within 300 ms plays it (home.md 6) |
| A cover or row | Menu, View | The game's Tile menu, its Customise row "Customise games & apps" |
| Anywhere | B | Back to where the page was opened from |

On open, focus is on the first cover, else the first row, else nothing. When the lists change under it, focus follows
its game: to its row, else to its cover (a game that finishes installing goes from Installing to Recently installed,
and focus with it); a game that has left both gives focus to the first cover, else the first row. Scrolling: a cover
puts the list at its top; a row scrolls the least that shows it whole.

Hint bar: "More options" (the Menu glyph) with focus on a cover or row; none with nothing focused.

Ways in (my-games.md 3.1): Manage's Queue box, B back to the box; Home's Downloads tile, B back to the tile; a Game
hub's Queue action, B back to the hub on Queue; the Guide button during a Queued Toast.

## 6. Manage while something is queued

- **The nav's Manage item** has a second line, "Queue" at the label's x with the Queue's count at the count's right
  edge, both `#FF8F8F8F`, and is 108 tall in place of 68 while the Queue holds something (E/MyGamesControl.xaml:515-528).
- **The nav's foot** has the current download over the storage ring (my-games.md 2.6): left x 75, 301 wide, 34 above
  the ring; the game's name; its state with the speed right-aligned; "`{amount}, {percent}`". Only while the Queue's
  first entry is downloading.
- **Manage's box** counts the Queue (my-games.md 9.1).

## 7. Decided in #79

1. **One page for every way in.** Manage Queue is a page over what opened it (`Open::ManageQueue`), never My games &
   apps with its nav hidden: B is a plain close, and what is under it is as it was, focus included. From Home's
   Downloads tile it is the first page over Home.
2. **The speed** is the library's: the Queue's follower samples each download's bytes once a second, works out
   bits per second from the last two samples and smooths it (half the new, half the old). It is none until the bytes
   first move, and 0 once they have stood still 10 s, as the Bundle's. It travels with the Queue the follower sends;
   the database keeps none, so a library read in between keeps the speeds it had.
3. **Recently installed** orders the rest of the installed games by when they came into the library, there being no
   install date kept: a game installed outside the Dashboard and long in the library comes after one added lately.
   Games finished while the Dashboard was up come first, as the Bundle's. A hidden game is left out.
4. **What the focus log says.** `ManageQueue: <where>; <n> installing, <m> recently installed[; <state> <share>]`,
   where is `Cover(i) of <name>`, `Row(i) of <name>` or `Nothing`, and a row adds what it reads: `Installing...
   25.0%`, `Queued 0.0%`. The speed is left out: it changes every second. A change of the Queue that moves focus or
   changes the row's share logs a new line without a press.
5. **A on a game** is Home's tile's, the hub after the double tap's 300 ms; a row's game, not installed, has its hub
   on Installing or Queued.
6. **Not drawn yet:** the storage ring under the foot's block (no drive space is read), the Queued Toast, and the
   Guide button's way in during it (Toasts for the Queue are not built).
7. **Home redraws** for a Queue change only when what it shows changed (order, state, finished games), not for bytes
   alone; with a page up every change is drawn.
