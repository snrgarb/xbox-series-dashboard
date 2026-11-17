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

Why Home's window and not a window of its own: it is already there, full screen, opaque and on the right display, so nothing more is made or kept (another window would be another swap chain, 25 to 100 MB); Windows activates the next window in z-order when the game's window goes, and with Home's window directly behind the game that is Home's, so the Dashboard has the foreground back on exit without asking for it; and the fade to Home happens inside one window, so no frame of anything else can come between two windows. The Bundle does the same with Playnite's window and its game status view (section 1).

| | Value |
|---|---|
| Picture | The Launch screen's (`screens::launch`), without the Spinner: nothing in it moves. The name stays, as the Bundle's game status view keeps its text |
| Drawn | Once, when it begins to cover Home's window (the Launch screen all the way in over Home, or the game in front of Home again), and again only when art it waited for comes in or the window changes size (`app::backdrop`). Never while it sits behind the game: it costs the game nothing but a place in the z-order |
| Fade to Home | **300 ms, `CubicOut`**, from the backdrop whole to Home: the Launch screen's own fade (section 1), taken because the Bundle has none of its own here. Whole at once on a slow renderer, as the Launch screen is there. Not changed by Reduce motion, as the Launch screen is not |
| Cut to Home | Home chosen over the running game (the Guide's Home, or Alt+Tab to the Dashboard): no fade, the game runs on behind |

## 3. Windows and z-order, moment by moment

Top to bottom of the z-order on the Dashboard's display. Topmost windows are above every other.

| Moment | Z-order | On screen |
|---|---|---|
| Play pressed | Toast strip (topmost, while a Toast shows), Overlay (topmost, passive: the Launch screen fading in), Home's window | Home, then the Launch screen |
| Launch screen in | Overlay, [the store's and the launcher's windows], Home's window, which draws the backdrop once the Launch screen covers it (`launch: backdrop up`) | The Launch screen |
| A launcher's window, a gap, the game's first frame | Overlay, [game, launcher], Home's window | The Launch screen. The game's window is given the foreground when it comes |
| Booted (`the game has booted, Launch screen fading`) | Overlay fading, game, **Home's window directly behind the game's** (`SetWindowPos(home, game, SWP_NOACTIVATE)`: `launch: Home's window behind <window> as the game's backdrop`) | The game; round a game that does not fill the screen, the backdrop |
| Running | [Guide, Idle screen, Toasts on the Overlay and the strip], game, Home's window, everything else | The game, the backdrop round it |
| The game's window goes (exit, crash, Quit, Force quit, a hand-off) | [Overlay], Home's window, everything else. Windows activates Home's window, the next in z-order | **The backdrop** (`launch: game gone, backdrop showing`) |
| Ended: nothing of the game's for 2 s, and its store says it has stopped | Home's window brought to the front and given the foreground, as before (`the game has ended, Home back`), or behind the open Guide | The backdrop fading to Home (`launch: fading to Home`), then Home, focus on the game's tile (`launch: Home back`) |
| A hand-off | The second process's window comes on top; Home's window is put directly behind it and, if Windows gave Home's window the foreground meanwhile, the new window is given it (`the game's new window given the foreground`) | The backdrop through the gap, then the game. The launch does not end if the new window comes within the 2 s |
| Home chosen in the Guide over the game | Home's window in front of the game | Home (cut) |
| Back to the game from the Guide | The game in front, Home's window directly behind it again | The game, the backdrop round it |
| Alt+Tab to the Dashboard while the game is played | Home's window comes to the front: taken as Home chosen over the game (`launch: Home's window chosen over the game`) | Home, the game running behind |
| Alt+Tab to another program | That program's window, the game, Home's window | That window, over the game and the backdrop |
| Exit to desktop | The Dashboard exits, and its windows with it; the game runs on | The desktop, as meant |

**Never activated.** Home's window goes behind the game without being activated (`SWP_NOACTIVATE`), so the game keeps the foreground and the pad. Only Windows, as the game's window goes, or the user, with Alt+Tab, makes it active.

**A topmost game.** A window placed after a topmost window in the z-order is made topmost itself, and Home's window must never be. Behind a game whose window is topmost, Home's window goes to the top of the ordinary windows instead (`HWND_TOP`), which is directly under the topmost ones.

**The taskbar.** The taskbar is Windows' own topmost window. Windows takes it out of the way of an active window that fills its monitor (a full-screen or borderless game, Home's window), and puts it in front of an active window that does not. The Dashboard leaves it alone: hiding another program's window is not its to do, and a Dashboard that crashed would leave it hidden. So **while a windowed game has the foreground the taskbar shows along the foot of the screen, over the backdrop**; everywhere else round the game is the backdrop. As the game's window goes, Home's window, full screen, is made active and the taskbar goes behind it.

**Other displays.** The backdrop covers the Dashboard's display only, the one Home's window is on (the primary). Any other display shows what it did: the desktop, or other programs. A game on another display leaves the backdrop on the primary with nothing in front of it until the game ends.

**Exclusive full screen.** A game in true exclusive full screen owns the display; Home's window behind it is not composed and costs nothing. When such a game leaves full screen (Alt+Tab, its exit) Home's window is what is under it.

**Launchers.** A launcher's own window (Steam's update dialog, an Epic launcher) may come above the backdrop: it is the launcher's, and shows as it is; round it is the backdrop, never the desktop.

## 4. The log

What a test judges each moment by: `launch:` lines, each ending `at <ms> ms` since the Dashboard started.

| Line | When |
|---|---|
| `launch: backdrop up at <ms> ms` | Home's window has drawn the game's backdrop; once a launch |
| `launch: game window seen at <ms> ms` | A window of a program in the game's folder has come after none, its launcher's included |
| `launch: game gone, Launch screen showing at <ms> ms` | The last such window went while the Launch screen was up |
| `launch: game gone, backdrop showing at <ms> ms` | The last such window went after the Launch screen had gone: the backdrop is what shows |
| `launch: game gone, Home in front at <ms> ms` | The same, with Home chosen in front of the game |
| `launch: Home's window behind <window> as the game's backdrop at <ms> ms` | Home's window put directly behind that window (or, for a topmost one, behind the topmost windows) |
| `launch: the game's new window given the foreground: <bool> at <ms> ms` | After a hand-off |
| `launch: fading to Home at <ms> ms` | The game ended; the backdrop fades to Home |
| `launch: Home back at <ms> ms` | Home is all Home's window shows |
| `launch: Home's window chosen over the game at <ms> ms` | Alt+Tab to the Dashboard while the game is played |

The lines that were there stay as they were (`the game's window is up`, `the game has booted, Launch screen fading`, `the game has ended, Home back`, `Home in front of the game`), but for `back to the game, Home hidden`, now `back to the game, Home behind it`.

## 5. Decided

- **Home's window is the backdrop**, not a window of its own (section 2).
- **No Spinner on the backdrop**: nothing is loading behind a running game, and a Spinner would need frames for as long as the game runs. The name stays.
- **The fade to Home is the Launch screen's**, 300 ms `CubicOut`: the Bundle brings Playnite to Home with no fade of its own, and the user asked for one ("fade back to the home screen on exit"). Whole on a slow renderer.
- **The backdrop holds through the 2 s the launch waits** (`GONE`) before it counts the game as ended, and as long after as the store takes to say it has stopped: that wait is what lets a hand-off through, and the user asked for the splash to remain.
- **The taskbar is left alone** (section 3): it shows over the backdrop's foot while a windowed game is in front.
- **Only the Dashboard's display is covered.**
- **Alt+Tab to the Dashboard is Home chosen over the game**, as the Guide's Home is: Home's window is in Alt+Tab's list now that it is never hidden.
- The screenshot tests' `draw_all` still draws Home where a game would be: it is the Dashboard's windows with no game, and the backdrop has tests of its own (`crates/screens/tests/shots.rs`).

## 6. Not verified without Windows

Built and unit-tested on Linux; the Windows code compiles and passes clippy for `x86_64-pc-windows-gnullvm`, and the end-to-end test (`e2e/payloads/launch-backdrop`) is written and has not run. To be seen on Windows:

- That Windows activates Home's window when the game's window goes, rather than another program's, and that putting Home's window behind the game never activates it.
- That a window put after a topmost one is made topmost (the reason for `HWND_TOP`): Windows' behaviour as known, not documented plainly.
- The taskbar over a windowed game, and going behind Home's window as the game exits.
- Real Steam games: Steam's own windows and how long it takes to clear `Running`; exclusive full-screen games leaving full screen onto the backdrop; a game on a second display.
