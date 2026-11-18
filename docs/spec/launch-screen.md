# The Launch screen, and the backdrop behind a game

From pressing Play until Home is back, the Windows desktop is never seen (#164, asked for by the user: "make sure the windows desktop is hidden completely with the game splash screen in the background then fade back to the home screen on exit"). The Bundle's glossary entry is `CONTEXT.md`, "Launch screen". Its sources are `C/ExtensionCore.LaunchScreen.cs`, `C/LaunchScreenState.cs`, `E/LaunchScreenWindow.xaml(.cs)`, `T/Views/GameStatus.xaml` (game-hub.md 11) and `C/ExtensionCore.cs` `OnGameStopped` / `ReturnHomeFrom`. The port's code is `crates/screens/src/launch.rs` (the pictures), `crates/shell/src/launch.rs` (following the game, z-order), `crates/app/src/window.rs` (drawing Home's window) and `crates/standin` (the stand-in game).

## 1. What happened before #164

Read from the code at `235150a`, before any change. Each moment, which of the Dashboard's windows is up, and what is on screen.

| Moment | Home's window | Overlay | What is seen |
|---|---|---|---|
| Play pressed (`Effect::Launch`, `Launch::start`) | Shown, in front, drawn until the Launch screen has faded in over it (`home_covered`), then not drawn | Shown `Passive` over the monitor of the window in front, topmost, never activated; the Launch screen fades in over 300 ms (whole at once on a slow renderer) | Home, then the Launch screen |
| The store's and the launcher's windows | Under them, as it was | Raised above everything at each look (every 200 ms, `overlay.raise()`) | The Launch screen. A gap between a launcher closing and the game opening is covered too: the Overlay is still up |
| The game's first frame (a window of a program in the game's folder covering 60% of the screen) | As it was | As it was | The Launch screen; the game's window is given the foreground |
| Booted (that window up 1.5 s, or another window has held the foreground 5 s, or two minutes with no window) | **Hidden** (`ShowWindow(SW_HIDE)`, `Launch::reveal`) | The Launch screen fades away over 300 ms, then hidden | The game. **Round a game that does not fill the screen (windowed, borderless at another size): the desktop and the taskbar**, since nothing of the Dashboard's is behind it |
| The game running | Hidden; not drawn | Hidden, but for the Guide, the Idle screen and Toasts (its own strip) | The game, the desktop round it |
| The game's window goes: it exits, crashes, is quit or force quit | Still hidden. The launch's look sees nothing of the game; it waits until neither the desktop nor the store has shown anything of it for 2 s (`GONE`; for Steam the store's word is `HKCU\Software\Valve\Steam\Apps\<appid>\Running`, which Steam clears in its own time) | Hidden (or the Guide, for Quit) | **The desktop: the wallpaper, the taskbar and whatever window Windows activates next**, for at least 2 s and as long as Steam takes to clear `Running` |
| Ended | Shown and given the foreground (`restore`: `SetWindowPos`, the Alt tap, `SetForegroundWindow`), drawn afresh: Home, focus on the game's tile | | Home appears at once, with no fade |
| Quit or Force quit from the Guide | Hidden until ended, then shown behind the Guide without the foreground | The Guide stays; its dim is over the desktop once the game has gone; it closes 500 ms after the game has ended | The Guide over the desktop, then over Home |
| A game that hands off to another process (a launcher starting the real game, a game restarting itself) | Hidden while the game runs | Hidden | The desktop between the first window going and the second coming. While the game starts the Overlay covers the hand-off |
| Home or a page chosen in the Guide over a game | Shown in front of the game (`show_home`), drawn | | Home; back to the game hides it again (`show_game`) |
| Exit to desktop | The Dashboard exits after the leaving sound; the game runs on | | The desktop, as meant |

**The reason for the user's report:** Home's window is hidden from the moment the game boots (`reveal`) until the launch has ended (`GONE` after the last of the game, `restore`). Nothing of the Dashboard's is on screen while the game runs, so whatever Windows has behind the game, the desktop, shows round a game that does not fill the screen and through the whole of the time between the game's window going and the launch ending. The Launch screen lives on the Overlay, which is hidden once the game has booted and is not shown again on exit.

**The Bundle.** Playnite's Fullscreen window is never hidden while a game runs: it stays behind the game showing Playnite's game status view, which the Bundle draws to match the Launch screen ("so the Launch screen's fade-out reveals the same picture", `T/Views/GameStatus.xaml` 1-12: the game's art as Home's hero chooses it, the scrim, the shade, the cover and "{Game} is running…"). When the game stops, `ReturnHomeFrom` brings Playnite to the front on Home (`playnite.BringToFront(DashboardPage.Home)`), with no animation of its own. The only fade the Bundle has between the game's picture and what is under it is the Launch screen's own: **300 ms, WPF's `CubicEase` `EaseOut`** (`E/LaunchScreenWindow.xaml.cs`, `FadeDuration`).

## 2. The backdrop

**Home's window is the backdrop.** While a game is up, Home's window stays shown, full screen on the Dashboard's display, and draws the game's **backdrop** in Home's place: the Launch screen's picture (the art as Home's hero chooses it, the scrim, the shade, the game's name) without the Spinner, opaque to the corners. It is never hidden behind a game any more.

Why Home's window and not a window of its own: it is already there, full screen, opaque and on the right display, so nothing more is made or kept (another window would be another swap chain, 25 to 100 MB); it is the window that takes the foreground back when the game's window goes (section 3); and the fade to Home happens inside one window, so no frame of anything else can come between two windows. The Bundle does the same with Playnite's window and its game status view (section 1).

| | Value |
|---|---|
| Picture | The Launch screen's (`screens::launch`), without the Spinner: nothing in it moves. The name stays, as the Bundle's game status view keeps its text |
| Drawn | Once, when it begins to cover Home's window (the Launch screen all the way in over Home, or the game in front of Home again), and again when what was drawn may be wrong or gone: the picture was drawn without its art (the picture says so itself; the art cache's count of art still decoding missed art that came between the draw and the count), art comes in, Home's window changes size or scale, is shown again or is activated (`app::backdrop`). Each draw is logged with its size and whether its art was in. Never otherwise while it sits behind the game: it costs the game nothing but a place in the z-order |
| Fade to Home | **300 ms, `CubicOut`**, from the backdrop whole to Home: the Launch screen's own fade (section 1), taken because the Bundle has none of its own here. Whole at once on a slow renderer, as the Launch screen is there. Not changed by Reduce motion, as the Launch screen is not |
| Cut to Home | Home chosen over the running game (the Guide's Home, or Alt+Tab to the Dashboard): no fade, the game runs on behind |

## 3. Windows and z-order, moment by moment

Top to bottom of the z-order on the Dashboard's display. Topmost windows are above every other.

| Moment | Z-order | On screen |
|---|---|---|
| Play pressed | Toast strip (topmost, while a Toast shows), Overlay (topmost, passive: the Launch screen fading in), Home's window | Home, then the Launch screen |
| Launch screen in | Overlay, [the store's and the launcher's windows], Home's window, which draws the backdrop once the Launch screen covers it (`launch: backdrop up`) | The Launch screen |
| A launcher's window, a gap, the game's first frame | Overlay, [game, launcher], Home's window | The Launch screen. The game's window is given the foreground when it comes |
| Booted (`the game has booted, Launch screen fading`) | Overlay fading, game, **Home's window directly behind the game's** (`SetWindowPos(home, game, SWP_NOACTIVATE)`: `launch: Home's window behind <window> as the game's backdrop; Home's window <state>`). The game's window is marked full screen for the shell (`launch: the game's window marked full screen`), so the taskbar is at the bottom while it is active | The game; round a game that does not fill the screen, the backdrop |
| Running | [Guide, Idle screen, Toasts on the Overlay and the strip], game, Home's window, everything else, the taskbar | The game, the backdrop round it |
| The game's window goes (exit, crash, Quit, Force quit, a hand-off) | [Overlay], Home's window, everything else. Home's window takes the foreground at once (`launch: Home's window takes the foreground: <bool>, <window> has it`), whatever Windows would have activated | **The backdrop** (`launch: game gone, backdrop showing`) |
| Ended: no window and no process of the game's, and its store no longer says it runs | Home's window brought to the front with the foreground (`the game has ended, Home back`), or behind the open Guide; the windows marked full screen unmarked | The backdrop fading to Home (`launch: fading to Home`), then Home, focus on the game's tile (`launch: Home back`) |
| A hand-off | A process of the game's is still running between its windows (`no window of the game's, its processes still running: <names>`), so the launch holds. The second process's window comes on top; Home's window is put directly behind it and, having the foreground, gives it to the new window (`the game's new window given the foreground`) | The backdrop through the gap, then the game |
| Home chosen in the Guide over the game | Home's window in front of the game | Home (cut) |
| Back to the game from the Guide | The game in front, Home's window directly behind it again | The game, the backdrop round it |
| Alt+Tab to the Dashboard while the game is played | Home's window comes to the front: taken as Home chosen over the game (`launch: Home's window chosen over the game`) | Home, the game running behind |
| Alt+Tab to another program | That program's window, the game, Home's window | That window, over the game and the backdrop |
| Exit to desktop | The Dashboard exits, and its windows with it; the game runs on | The desktop, as meant |

**Not activated while the game is up.** Home's window goes behind the game without being activated (`SWP_NOACTIVATE`), so the game keeps the foreground and the pad. It takes the foreground itself as the game's last window goes: Windows hands the foreground on to whatever it chooses (on the TV PC a console that had been active came up over the backdrop), and only Home's window having it keeps the backdrop in front. The user makes it active with Alt+Tab.

**When the game has ended.** The launch looks at the desktop every 0.2 s. Once no window of the game's is up it asks which processes of programs in the game's folder still run: while one does, the game is between windows or handing off to another of its programs, and the backdrop holds. Once none does, it asks the store: Steam's `Running` still set while another window has the foreground may be a launcher outside the game's folder starting it again, and the backdrop holds until Steam clears it (the log says how long it took). With Home's window in front (it took the foreground as the game's window went) the store is not waited for: Steam took 2.2 s to clear `Running` after Ricochet was killed on the TV PC, and that only matters for offering Play again, which is a question of its own. Then the launch ends and the fade begins, with no wait of its own: from the game's window going to Home being whole is the next look (0.2 s at most), the game's process exiting, and the 300 ms fade. Before #164's second round there was a fixed 2 s wait here (`GONE`), which showed as two seconds of a still picture on every exit. `GONE` is left only for a launch whose game has shown nothing at all (the Launch screen stepped aside for a window that wanted the user).

**A topmost game.** A window placed after a topmost window in the z-order is made topmost itself, and Home's window must never be. Behind a game whose window is topmost, Home's window goes to the top of the ordinary windows instead (`HWND_TOP`), which is directly under the topmost ones.

**The taskbar.** The taskbar is Windows' own topmost window. Windows takes it out of the way of an active window that fills its monitor (a full-screen or borderless game, Home's window), and puts it in front of an active window that does not, which on the TV PC showed the taskbar over the backdrop's foot round a windowed game. The desktop must not show at all, so the game's window is **marked full screen for the shell** (`ITaskbarList2::MarkFullscreenWindow(game, TRUE)`): its documentation says the shell then treats it as full screen and moves the taskbar to the bottom of the z-order while it is active, so the taskbar goes under Home's window too. The game's own window is marked, not Home's, because the game's is the active one; Home's fills its monitor and needs no marking. Every window Home's window is put behind is marked (a launcher's that wants the user included), and each still there is unmarked when the launch ends (`FALSE` hands it back to the shell's own judging). The Overlay, the Idle screen and the Toast strip are topmost windows of the Dashboard's and are not moved by this: they stay above the game. The taskbar is not hidden or moved by the Dashboard: a Dashboard that crashed would leave nothing changed but a mark on a window that has gone. Not used: making Home's window topmost under a topmost game, which would make the game's window topmost too and fight the taskbar's own raising.

**Xbox mode.** Windows' Xbox mode cloaks the home app's window while another app is in front (seen on the TV PC: `Home's window shown, cloaked by the shell, 3840x2160` behind Ricochet), so there the backdrop is not on screen and round a game that does not fill the screen Xbox mode shows its own flat grey. Xbox mode is not the Dashboard's route (shell mode is, where Home's window is not cloaked and the backdrop shows), so nothing is done about it.

**Xbox mode: the game's own windows cloaked.** Xbox mode cloaks the game's windows in turn while Home is in front, as it cloaks Home's behind the game. The launch's look at the desktop once passed over every cloaked window, so with Home in front it found no window of the game's: going back to the game did nothing (`the game has no window to go back to`), and `game gone, Home in front` was said of a game still running (seen on the TV PC with "over the hill", a Unity game). A window cloaked by the shell (`DWM_CLOAKED_SHELL`, 2) now counts as the game's; one its own app cloaked (1) or cloaked with its owner (4) does not, and other programs' windows are kept out by their path as before (`may_be_the_games`, with a unit test). Each time the game's largest window or its cloak changes the log says so (`the game's window <window>, cloaked by the shell (2)`). Going back to the game activates its window (`SetForegroundWindow`); no documented call switches Xbox mode's app, and activating a window is how an app on another virtual desktop is brought forward, so the log says the window's state after it (`the game's window given the foreground: <bool>; it is <state>`) for a run on the TV PC to show whether the shell uncloaked it. The other places that look at windows (`overlay.rs`, `toast_strip.rs`, `steam.rs`, the watch for a window that needs the user) test `IsWindowVisible` alone, which a cloaked window still passes: none passes over a cloaked window.

**Other displays.** The backdrop covers the Dashboard's display only, the one Home's window is on (the primary). Any other display shows what it did: the desktop, or other programs. A game on another display leaves the backdrop on the primary with nothing in front of it until the game ends.

**Exclusive full screen.** A game in true exclusive full screen owns the display; Home's window behind it is not composed and costs nothing. When such a game leaves full screen (Alt+Tab, its exit) Home's window is what is under it.

**Launchers.** A launcher's own window (Steam's update dialog, an Epic launcher) may come above the backdrop: it is the launcher's, and shows as it is; round it is the backdrop, never the desktop.

## 4. The log

What a test judges each moment by: `launch:` lines, each ending `at <ms> ms` since the Dashboard started.

| Line | When |
|---|---|
| `launch: backdrop up at <ms> ms` | Home's window has drawn the game's backdrop; once a launch |
| `launch: <store> getting ready to start the game: the Launch screen stays up at <ms> ms` | The store said it is still getting ready (Battle.net's client starting, #172): a window of its own in front does not step the Launch screen aside; `<store> ready, or no longer getting ready` when that ends |
| `launch: no window of the game's yet, its processes running: <names> at <ms> ms` | After the Launch screen stepped aside with nothing of the game's shown, a process of a program in its folder runs (Battle.net's `Base.exe`, which starts the game): the game is followed as one that has started |
| `launch: game window seen at <ms> ms` | A window of a program in the game's folder has come after none, its launcher's included |
| `launch: game gone, Launch screen showing at <ms> ms` | The last such window went while the Launch screen was up |
| `launch: game gone, backdrop showing at <ms> ms` | The last such window went after the Launch screen had gone: the backdrop is what shows |
| `launch: game gone, Home in front at <ms> ms` | The same, with Home chosen in front of the game |
| `launch: Home's window behind <window> as the game's backdrop; Home's window <state> at <ms> ms` | Home's window put directly behind that window (or, for a topmost one, behind the topmost windows). `<state>`: shown or hidden, minimised, cloaked (by the Dashboard, the shell or with its owner), its size and place |
| `launch: the game's window marked full screen: the taskbar goes under the backdrop at <ms> ms` | The window Home's window went behind marked for the shell; or `could not be marked full screen: <why>` |
| `launch: backdrop drawn, <w>x<h>, its art in at <ms> ms` | Each frame of the backdrop while it covers Home's window; `its art still to come` when drawn without it |
| `launch: backdrop to be drawn again: <why> at <ms> ms` | Home's window resized, rescaled, shown again or activated while the backdrop covers it |
| `launch: Home's window takes the foreground: <bool>, <window> has it; Home's window <state> at <ms> ms` | As the game's last window goes |
| `launch: no window of the game's, its processes still running: <names> at <ms> ms` | The game between windows, or handing off; said again when the list changes |
| `launch: the game's processes have gone, <store> still says it runs at <ms> ms` | The store has still to say it has stopped |
| `launch: the game's windows and processes have gone[, and <store> said so <n> ms later] at <ms> ms` | The game has ended; then `the game has ended, Home back` |
| `launch: the game's windows and processes have gone, Home's window has the foreground: ended without waiting for <store> to say so at <ms> ms` | The same, the store still saying it runs |
| `launch: the game's window <window>, cloaked by the shell (2) at <ms> ms` | The game's largest window, or how it is cloaked, changed: `not cloaked (0)` or `cloaked by the shell (2)` |
| `launch: the game's window given the foreground: <bool>; it is <state> at <ms> ms` | Back to the game from Home or the Guide |
| `launch: the game's new window given the foreground: <bool> at <ms> ms` | After a hand-off |
| `launch: fading to Home at <ms> ms` | The game ended; the backdrop fades to Home |
| `launch: Home back at <ms> ms` | Home is all Home's window shows |
| `launch: Home's window chosen over the game at <ms> ms` | Alt+Tab to the Dashboard while the game is played |

The lines that were there stay as they were (`the game's window is up`, `the game has booted, Launch screen fading`, `the game has ended, Home back`, `Home in front of the game`), but for `back to the game, Home hidden`, now `back to the game, Home behind it`.

## 5. Decided

- **Home's window is the backdrop**, not a window of its own (section 2).
- **No Spinner on the backdrop**: nothing is loading behind a running game, and a Spinner would need frames for as long as the game runs. The name stays.
- **The fade to Home is the Launch screen's**, 300 ms `CubicOut`: the Bundle brings Playnite to Home with no fade of its own, and the user asked for one ("fade back to the home screen on exit"). Whole on a slow renderer.
- **The fade starts as soon as the game is known gone** (section 3): no window, no process of the game's folder, the store no longer saying it runs. The backdrop holds longer only while a process of the game's runs on (a hand-off, a game between windows) or its store still says it runs.
- **The game's window is marked full screen** so the taskbar goes under the backdrop (section 3).
- **Home's window takes the foreground as the game's last window goes**, rather than waiting for Windows to give it.
- **A window of the game's that the shell has cloaked is still its window** (section 3): "game gone" is never said of a game whose processes run with such a window.
- **The store's word is not waited for once Home's window has the foreground**: offering Play again while Steam still says the game runs is left to its own ticket.
- **Only the Dashboard's display is covered.**
- **Alt+Tab to the Dashboard is Home chosen over the game**, as the Guide's Home is: Home's window is in Alt+Tab's list now that it is never hidden.
- The screenshot tests' `draw_all` still draws Home where a game would be: it is the Dashboard's windows with no game, and the backdrop has tests of its own (`crates/screens/tests/shots.rs`).

## 6. Not verified without Windows

Built and unit-tested on Linux; the Windows code compiles and passes clippy for `x86_64-pc-windows-gnullvm`, and the end-to-end test (`e2e/payloads/launch-backdrop`) is written and has not run. To be seen on Windows:

- That Home's window gets the foreground as the game's window goes, with Steam still up, and that putting Home's window behind the game never activates it.
- That a window put after a topmost one is made topmost (the reason for `HWND_TOP`): Windows' behaviour as known, not documented plainly.
- That marking another process's window full screen moves the taskbar under the backdrop while it is active (the documentation names a window, not whose).
- Real Steam games: Steam's own windows and how long it takes to clear `Running`; exclusive full-screen games leaving full screen onto the backdrop; a game on a second display.
- **Seen on the TV PC (Ricochet through Steam, a 640x480 window on a 4K display in Xbox mode):** everything round the game was a flat dark grey (21, 21, 21) and not the backdrop. The log showed why: `Home's window shown, cloaked by the shell`. Xbox mode is not the Dashboard's route, so it is left so (section 3); in shell mode the backdrop shows.
- One cause in the code was found and mended on the way: a backdrop drawn before its art was decoded was never drawn again if the art came in between the draw and the art cache's count.
