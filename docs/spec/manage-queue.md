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
| State | Under the title: "Installing..." while it downloads, "Queued" while it waits its turn or is paused (the port reads a paused one "Paused", section 8, and a game installed whose Workshop items are coming in "Downloading Workshop content", section 9) |
| Amount line | 34 under the state: "Network: {amount}" at the left, only when there is an amount; the share right-aligned, "1.9%" |
| Amount | "67.80 MB of 3.55 GB": two decimals, a GB being 2^30 bytes, MB below 1 GB, TB from 1024 GB, the unit chosen by the value as rounded (C/MyGamesState.cs:300) |
| Bar | 12 under the amount line, 5 tall, across the column: track `#FF4A4A4A`, the share filled white |
| Focus ring | `Margin="-8" BorderThickness="4" CornerRadius="8"`, `#FF2CA243`, with the focus glow |

A game that is not downloading reads "Queued", "0.0%" and an empty bar, whatever of it is down already, and has no
speed and no amount. A download whose store has said no bytes yet (or no whole) reads "Installing..." alone: no share, no
amount, no speed, and the bar's empty track, as the Bundle's row does for a store that gives no bytes (its progress
null: `Network:` line, speed and share are bound to values that are null until then). A share that stood at "0.0%"
read as a stalled download on the real machine (#132). Steam's running download is counted live (docs/sources.md),
so it has a share once Steam's client has written anything.

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
| A cover or row | A | As on a Home tile: the game's hub, at once; A again is its Play (home.md 11) |
| A cover or row | Menu, View | The game's Tile menu, its Customise row "Customise games & apps"; on a row, the queued game's menu (section 8) |
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
  the ring; the game's name; its state with the speed right-aligned; "`{amount}, {percent}`" (no line with no whole known). Only while the Queue's
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
   25.0%`, `Queued 0.0%`; `Installing...` alone with no whole known. The speed is left out: it changes every second. A change of the Queue that moves focus or
   changes the row's share logs a new line without a press.
5. **A on a game** is Home's tile's, the hub at once (no double tap, the user's decision of 2026-11-17, departing from the Bundle); a row's game, not installed, has its hub
   on Installing or Queued.
6. **Not drawn yet:** the Queued Toast, and the Guide button's way in during it (Toasts for the Queue are not
   built). The storage ring under the foot's block is drawn since #179 (storage.md, section 6).
7. **Home redraws** for a Queue change only when what it shows changed (order, state, finished games), not for bytes
   alone; with a page up every change is drawn.

## 8. Pause, Resume and Cancel (decided in #170)

The Bundle's Manage Queue has none of them. The user decided (2026-11-17, on #166) that they go in the Queue, as the
console has them: in a queued game's menu. No picture of the console was at hand: what follows is the console's
manner as remembered, the wording guessed where marked.

1. **Where.** Menu or View on a row of Installing opens the game's Tile menu as before (`tile_menu::Opener::Queue`),
   with, **in place of its first row** (the Bundle's Install, which for a game already installing asks nothing and
   reads wrong), **Pause installation** for a game downloading or waiting its turn, or **Resume installation** for
   one paused, then **Cancel installation**. The same for the game's menu from anywhere while it is in the Queue: a
   Home tile, My games, the library, the Game hub's Manage (#171, the lead's decision on the TV PC, 2026-11-17: the
   console offers the queue's actions on the game wherever it is). The Game hub's own main action stays Queued or
   Installing..., and its Queue action opens this page. Focus starts on the first row, as always.
   Labels: "Pause installation" and "Resume installation" as the console's queue words them (from memory, not a
   picture); "Cancel installation" guessed to match.
2. **Only where the store can.** Each row shows only where the game's store can be asked it: its installer's
   `Can` (docs/sources.md "Installs"), which the Queue's follower gives each entry (`Queued::can`). Pause and Resume
   go together. A store that cannot (Steam with its CEF debugger off, every other store until its ticket) has no row
   in the first row's place: the menu starts at the Favourites row. A game installed whose extras are still coming in (#188) has none of them.
3. **What they do.** Pause and Resume close the menu and ask the store (`Effect::Steer`); the row follows what the
   store then reports, a second or so later. Cancel asks first, in the menu's place as Uninstall does: "Cancel
   installing <game>?", "What has downloaded so far is deleted.", **Cancel installation** (focused) or **Keep
   installing**, which goes back to the menu on Cancel installation; B is Keep installing. Yes closes the menu and
   asks the store, which drops the install and deletes what it fetched: the game leaves the Queue (`queue: <game>
   dropped`). Each is looked at again as it is chosen (tile-menu.md 3.3): one that no longer applies does nothing,
   and the menu closes all the same.
4. **A paused row** reads **Paused**, the console's word, with its amount and share as far as it got and the bar
   filled to that; no speed. (The Bundle reads it Queued, 0.0%, which said nothing true once the user can pause from
   here.) Its tiles still read Queued, as the Bundle's. The focus log: `Row(1) of <game>; ...; Paused 25.0%`.
5. **The log.** Steam is asked through its own client (#171): `steam client: pause <appid>: asking Steam's client
   over its CEF debugger ...`, a line a call, then `steam client: pause <appid>: Steam's client took it` or `not
   taken: <why>` (docs/sources.md, "Steam's own calls"); `store: Steam can't pause <game>'s install from here` where
   it cannot; then the Queue's own `queue: <game> paused, 50% of 100 bytes`.
6. **Which folder.** Where the game's store has more than one folder and can be told which, Install asks "Where do
   you want to install X?", focus on the default Settings > Storage gives (storage.md, section 5; #179); else the
   install goes to that default. Steam's own default is the library Steam itself marks default, asked of its client
   at the install.

## 9. Workshop content (decided in #188)

Neither the Bundle nor the console has this: a Steam game whose own files are in while Steam is still fetching the
Workshop items the account subscribes to (Garry's Mod on the TV PC, #188: 3.9 GB of its own, then 160 items).

1. **The game is installed.** Play is offered as soon as its own files are in, and the Ready to play Toast fires then
   (the user's decision on #188); its tiles read it installed. Steam itself lets it be played.
2. **Its row stays** in Installing until Steam has every item, and then leaves with nothing more said (no second
   Toast). It is ordered with the downloads, first.
3. **What the row reads.** The state line: **Downloading Workshop content**. The amount line, in the place of
   "Network: ...": the items in of those subscribed to, `3 of 12 items`, then the bytes where Steam's record gives a
   size for every item, `3 of 12 items, 82.00 MB of 1.20 GB`. The share at its right and the bar: by the bytes where
   known, else by the items. **No speed**, ever: Steam's speed is not known to be the items', and "0 Kbps" said
   nothing true of them (#188). With no item count known: the state line alone, the bar's empty track.
4. **Its menu** is an installed game's: no Pause, Resume or Cancel.
5. **The focus log**: `Row(0) of <game>; ...; Downloading Workshop content 3 of 12 items 25.0%`. The Queue's own:
   `queue: <game> installed; its store is still fetching more for it` as it becomes playable, then `queue: <game>
   downloading Workshop content, 3 of 12 items` (and `, <done> of <total> bytes` where known) at each whole percent,
   and `queue: <game> has all its store was fetching for it` as it leaves.
6. **The nav's foot** (section 6) shows only a download, not this.
