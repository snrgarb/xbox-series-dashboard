# Apps, and games added by hand (decided in #77)

What the port has in the place of Playnite's Software tools and its games added by hand. The Bundle's own Apps are in [my-games.md](my-games.md), section 8 (the pane, the tile), and the glossary's **Apps**; this file is what the port decided where the Bundle leaned on Playnite.

## 1. What they are

- **A game added by hand** is a game like any store's: in Games, Full library, the Groups, Search's Games row, Play history and, once played, the Recent row. It is a source of its own, **`manual`** (`model::by_hand::MANUAL`, docs/sources.md): a row of the library database's `games` with `source = 'manual'` and `source_id` the program's full path as typed, so one program is never two games. Its install folder is the program's folder (what the shell follows its windows by), it is always installed, and the art given for it is its `cover`. Its source (`library::by_hand::Manual`) reads nothing, so no sync touches it, and starts it as `Start::Program { path, args: [], dir: None }`: the program, in its own folder, with no arguments. It launches as any game does: the Launch screen, a Play session, and first in the Recent row after. Its tiles carry no store mark; the Tile menu has no Uninstall for it (it is removed in Settings, section 4).
- **An App** is not a game: it is in none of the snapshot's lists of games (`Library::apps`, apart), so never in the Recent row, a Group, Full library, Play history or Search's Games row, and never has a Play session. It is in the Guide's list once started from the Dashboard (#345, guide.md section on the landing tab): when it was last started is kept in `apps.last_opened` (migration 13; log `library: <name> opened`). A on its tile starts it at once, whatever is running, with no Launch screen and no quit prompt, as the Bundle starts a Software tool (`games::start_app`; log `apps: started <name>: <what was opened>`). It comes in front of the Dashboard: Windows does not count the pad's presses as the user's input, so the App is given the right to take the foreground (`AllowSetForegroundWindow`, and `CoAllowSetForegroundWindow` on the activation manager for a Store app), and for up to 20 s after its start the shell looks for its window (a Store app's by its package family, its frame being ApplicationFrameHost's; a program's by its path) and puts it in front if it is not (`shell::app`; log `apps: <name>'s window up at <ms> ms, not in front (<window> is): put in front: <true|false>`, then `apps: <name> in front: <window>`). A on the tile of an App that runs brings it in front again the same way. The Guide over an App works as over a game: B gives the App the foreground back, and Home or a page chosen in it puts the Dashboard in front (`apps: Home's window in front of <window>` where the Guide was in Home's window), the App running on behind. Apps are kept in the library database's own table, `apps` (migration 4), in the order they were added, which is the order of the pane. An App is one of two kinds:
  - **a program** added by hand (`kind = 'program'`, `target` its full path), with the icon given for it, if any;
  - **a Microsoft Store app** ticked in the list of those installed (`kind = 'store'`, `target` its Application User Model ID), its picture the package's tile art. It is started through the shell's Apps folder, `shell:AppsFolder\<AUMID>`, as the Bundle's Playnite starts one (`explorer.exe shell:AppsFolder\...`) and as the Start menu does.
- Nothing is an App or a game added by hand until the user adds it. There is no scanning for emulators or programs.

## 2. The Apps pane

As my-games.md section 8 has it: the grid, five to a row, tiles 234 square at a pitch of 249 from (520, 190), in the order added. Left from the first column goes to the nav, every other edge stops; Down onto a shorter last row lands on its last tile. Scrolling is the least that has the focused tile wholly on screen with the 24 over it that the first row has (so y 166, the list's top, is the highest those 24 go, and a first-row tile puts the list at its top), at once, as WPF brings an item into view. No Tile menu, no hints. With no Apps the pane's line is "Apps you add in Settings show up here." (the Bundle's names Playnite's Software tools), at (548, 190), Segoe UI 26 `#8F8F8F`, and focus stays on the nav.

**The tile** (`crate::app_tile`, which Search's Apps row will share): white, the icon inset 18 and as large as fits (`Stretch="Uniform"`), over a shadow of the icon's own shape, 30% black, blur 18, 3 down and right (the Bundle's `ImageShadow ShadowDepth="3"`, WPF's default direction of 315 degrees); a Store app's art edge to edge over the accent with no shadow; with no icon or one that can't be read, the glyph U+ECAA as large as the inset allows, `#333333` on white, white on the accent. Cut to corners of 6. Focused: the ring 4 wide and 8 out with outer corners 14 and the glow, and the `#E6000000` band at the foot with the name in Segoe UI 26, 8 in, on two lines at most, which fades in over 150 ms after 50 and out over 100, Reduce motion or not (my-games.md, 12.2).

**The icon** of a program App is the picture file the user gave: an `.exe`'s own icon is not read (Windows-only extraction, left for later; the glyph stands in). Pictures are decoded at most 480 wide, as the Bundle's.

**Focus log.** `focus: MyGames: Apps: Tile(<n>); "<name>", <count> apps`: the tile's place, then the App's name and how many there are; on the nav `MyGames: Apps: Nav(Apps)` with the same state (the App last focused, which Right goes back to), and nothing after it while there are none.

**Not seen.** There is no picture of the Bundle's Apps pane with Apps in it (port-reference 05 shows it empty): the pane is built from the XAML, and a picture of it with a program and a Store app in it, focused and not, is to be taken with `port-reference`.

## 3. Search

The Bundle's Search has an Apps row under Games (search.md, section 3.3). Here the row is **left as a hook**: Search's rows hold places in `Library::games`, and an App is in no list of games, so it is never in Search's Games row; showing an Apps row needs rows of two kinds of thing, which is more than this ticket. `search::find::apps` already finds Apps by Search's rule (the name holds the text; starting with it first, then a word starting with it, then the rest, by name), tested, for the row to use.

## 4. Settings: Games & apps

A box on the Games & apps category, **Games & apps** (glyph U+E71D), in its Library section under Launchers ([settings.md](settings.md) 6.1), opens its page in the boxes' place. Everything on it is the port's: the Bundle adds both in Playnite's own windows.

### 4.1 The page

Header "Games & apps"; under it the Rows (settings.md 4.3; 892 x 66, 74 apart) from y 210, no line over them (#261, settings.md 16.11): **Add a game** and **Add an app** (the plus glyph), then each game added by hand by name (hidden ones too: this is where they are removed; glyph U+E7FC), then each App in the order added (glyph U+E71D).

- A on Add a game or Add an app opens its page in this one's place; B there comes back here, focus where it was.
- A or Menu on a game's or an App's Row opens its menu, the Tile menu's panel over the dimmed page, as Groups' (filters.md, 6.2): **Rename**, which opens text entry ("Name", at most 60) on its name, saved as it is kept unless it is empty or unchanged; and **Remove**, which asks "Remove <name>?" (Remove / Cancel) first. A game removed goes with its Play sessions; its program is left on disk.
- The hint bar shows More options on a game's or an App's Row, nothing on the two that add.
- Focus stays on its Row through every change, and on the Row in its place when it has gone.
- **Focus log**: `Settings: GamesAndApps: Add a game`, `... Add an app`, `... Game <name>`, `... App <name>`, each followed by `; <n> games, <m> apps` (`1 game`, `1 app` for one); with the menu up `Settings: GamesAndApps: Menu(Rename) of <name>`, with its question `Settings: GamesAndApps: Prompt(Remove) of <name>`.

### 4.2 Add a game, Add an app

A page of fields, laid out as a Group's page (filters.md, 6.3: a name over each control, 40 between): **Program** (the full path), **Name**, **Art (optional)** for a game or **Icon (optional)** for an App, each a text field 892 wide; then the pane button **Add**.

- A on a field opens text entry on it (captions "The game's program", "The app's program", "Name", "Art for its tiles", "Its icon"; 260 characters at most for a path, 60 for a name), typed on the Console keyboard or a keyboard. A path kept loses the spaces and the quotes round it (Explorer's "Copy as path" puts them there), and with no name yet offers its file's name without the extension.
- **A text field** is the port's own control: a dropdown box's fill and edge (settings.md, 4.5) holding the text on one line, cut short with an ellipsis; with nothing typed a dim hint in its place.
- **Add** is dimmed, and does nothing, until there is a path and a name. Pressed, it asks the shell to add, and the page goes back to Games & apps, where the new Row is once the database has it. **No file picker** (the issue's decision: a pad has no use for one).
- The shell refuses a path with no file there, and says so in a notification ("There's no program at <path>", which shows as a Toast and is in the Guide's notifications); a picture with no file there is left out and logged. The database refuses a program added twice (as a game, or as an App), logged.
- **Focus log**: `Settings: AddGame: Path` (`Name`, `Picture`, `Add`), `Settings: AddApp: ...`, `Settings: AddApp: Store(<n>)` on the n-th Store app; each followed by `; path "<path>", name "<name>", picture "<picture>", ready` (or `not ready` while Add would do nothing).

### 4.3 The Microsoft Store apps

Add an app goes on under Add with the heading "Microsoft Store apps" and, 26 under it, a Row for each Store app installed, by name, ticked while it is an App. A ticks one, which adds it as an App at once, or unticks it, which removes it; focus stays.

- **Where the list comes from** (`crates/app/src/store_apps.rs`): Windows PowerShell's `Get-StartApps`, the apps the Start menu starts, of which those with an Application User Model ID (`<package family>!<app id>`) are packaged apps; for each, `Get-AppxPackage` gives the package's folder and `Get-AppxPackageManifest` its `Square150x150Logo` (else the package's own logo), whose largest scale on disk (`.scale-200` and so on; never a high-contrast one) is its art. It is listed once, on a thread of its own after the first frame (`apps: <n> Microsoft Store apps listed in <ms> ms`), and handed to the screens with the library's snapshots (`Library::store_apps`); it is not kept in the database.
- What PowerShell prints is read as JSON and the art is found on disk in Rust, both tested on made-up data. **The PowerShell itself has not run on a real machine**: if the list is empty on the VM, `store_apps::SCRIPT` is the one place to look.
- Until it is listed, and anywhere but Windows, a line 4 under the heading: "The Microsoft Store apps on this PC show up here once Windows has listed them."; listed and empty: "There are no Microsoft Store apps on this PC."; with apps, no line ("Tick the ones to start from My games & apps." until #261).

## 5. What is left for later

- Search's Apps row (section 3).
- A program's own icon from its `.exe` (section 2).
- Arguments and a working folder for a game added by hand: the database could keep them (`Start::Program` has both); nothing asks for them yet.
- A picture of the Apps pane with Apps in it from the Bundle, to compare with (section 2).
