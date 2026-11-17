# Storage (Settings > Games & apps > Storage; #179)

Where installs may go: this PC's drives with their free and total space, each store's install folders with the one its installs go to ticked, the question an Install asks where a store has more than one, and the storage ring on My games & apps (my-games.md 2.6). From docs/research/installs.md 10.2 and 13, item 11.

**The page is the port's own.** The Bundle has none (Playnite left storage to the stores); the console has one, but no picture of it was at hand. It is made from the Settings pages' own parts (settings.md, section 4: the description, a label, the Row and its tick) at their measurements, on the 892-wide column from (520, 210), as Launchers is (launchers.md). The question is the Guide's and the Tile menu's prompt (`menu_panel`), in the console's manner as remembered: a small panel, the default focused. **Wording guessed** where marked, with nothing of the console's to copy.

## 1. Where it reads from

- **Drives**: Windows' fixed drives (`GetLogicalDriveStringsW`, `GetDriveTypeW` = fixed), each with its label (`GetVolumeInformationW`) and the free and total bytes this user may use (`GetDiskFreeSpaceExW`), read by `shell::drives::SystemDrives`. A folder's drive is its volume (`GetVolumePathNameW`), so a folder on a mounted volume is on that volume. Behind `model::Drives`: a test makes its own, and anywhere but Windows nothing is read.
- **Folders**: each store's `Installer::folders()` (docs/sources.md, "Installs"). Only Steam has any today: its libraries from `libraryfolders.vdf`, the Steam folder's own as its default. A store whose installer names none is not on the page.
- **Which the user may choose**: the installer's `Can::choose_folder`. Steam's is true with its CEF debugger on and more than one library: Steam's client is then asked for the library chosen (`InstallFolder.GetInstallFolders`, `client::Call::Install`). With the debugger off Steam installs to its own default and the Dashboard cannot tell it otherwise.
- **The user's choice** is the settings row `storage.default.<store>` (`storage.default.steam`), the folder's path. It is the store's default while it is still one of the store's folders and the store can be told; else the store's own default is.
- **The ring's drive**: the one holding most installed games that are not hidden, by each game's install folder; ties to the first by its root; none where no game's drive can be read (the Bundle's rule, my-games.md 2.6).

All of it is `model::Storage`, put together by `model::storage::read` with no I/O and read by the executable on a thread of its own (`crate::storage`), which carries it in every snapshot (`Library::storage`). It is read after the first frame, once the sources have been read, whenever an install finishes or a game is uninstalled, and each time Settings > Storage opens (`Effect::LookAtStorage`). The log says what it found each time: `storage: 2 drives; Steam: 2 folders, installs into C:\Program Files (x86)\Steam unless asked; ring on C:\`.

## 2. Its box

**Storage**, icon U+EDA2 (Segoe Fluent's hard drive), the first box of Games & apps' Installs section (975, 262, 437, 131), beside Launchers, Helpers under it ([settings.md](settings.md) 6.1). Left from it is Launchers; Down from it is Helpers.

Picture: `crates/screens/tests/reference/settings-theme-settings-launchers-box.png`.

## 3. The page

| Element | y | Notes |
|---|---|---|
| Header | as every page | "Storage" |
| What it does | 210 | `controls::description`, wrapped at 892: "Where your games are installed. Choose the folder each store installs into; where a store has more than one, Install asks which, starting on the one ticked here." |
| Drives | 30 under it | A label, "Drives"; 12 under it each drive: its name left ("Local Disk (C:)" as Explorer names a drive with no label, "Games (D:)"), "354.6 GB free of 802.0 GB" right, dim; 8 under them a bar 892 x 6, `#FF262626`, filled with the accent (`#FF107C10`) as far as the drive is used; 22 to the next. With none: "This PC's drives couldn't be read.", dim |
| A store | 18 under the drives, 26 between stores | Its name as a label ("Steam"); under it, dim, "Installs go to the folder ticked." or, for a store that chooses for itself, "Installs go to the folder ticked: this launcher chooses it for itself."; 14 under that a Row a folder, 892 x 66 every 74 (settings.md, 4.3): the tick on the store's default, the folder's path, its drive's free space right-aligned 24 in, dim |
| No store | in the stores' place | "No launcher here says where it installs games.", dim |

Sizes are written as the Bundle's ring writes them (C/MyGamesState.cs:283-309): one decimal, GB, MB under 1 GB, TB from 1024 GB, 1024 to a step.

The page scrolls as the other Settings pages do (`controls::scrolled`), so the focused Row is always whole on screen. Pictures: `settings-storage.png`, `settings-storage-other-folder.png`, `settings-storage-nothing.png`.

## 4. Focus and what A does

- Focus is only ever on a folder's Row, top to bottom across the stores: Up and Down move a Row, and stop at the ends. It opens on the first store's ticked folder. With no store, focus is nowhere.
- **A** on a folder of a store that can be told makes it the store's default, at once: ticked here, saved (`Effect::DefaultFolder`, `settings: storage.default.steam = D:\SteamLibrary`), and the store's default in every snapshot from then. On the ticked one, or a store that chooses for itself, A does nothing.
- **B** or **Left** goes back to Games & apps' boxes, on Storage.
- A new snapshot keeps focus on the same folder of the same store where it still is, else on the first store's ticked one.
- **The focus log**: `Settings: Storage: Steam D:\SteamLibrary; the default; 2 drives`, `...; not the default; ...`; `Settings: Storage: Nothing; 1 drive` with no store.

## 5. Which folder: the question an Install asks

1. **When.** Install on the Tile menu or the Game hub (`tile_menu::install`) asks it when the game's store has more than one folder and can be told which (`StoreFolders::asks`). Otherwise there is no question, as on the console with one drive: the install goes to the store's default as Settings > Storage has it.
2. **What.** A prompt (`MenuPanel::prompt`) in the middle of the screen over the dimmed page, in the place of the Tile menu or the Game hub that asked (`PageAction::Replace`), flying up as the Tile menu's question does. Title: **"Where do you want to install {game}?"** (guessed). A row a folder, the Install glyph and the folder's own name with its drive's letter, short enough for the panel: "SteamLibrary (D:)", "Steam (C:)", a name of more than 22 characters cut to 19 and "...". Under the title, where the rows' message goes, the focused folder's free space: "1.2 TB free" (nothing where its drive could not be read).
3. **Focus** starts on the store's default (Settings > Storage's choice), not on the first row.
4. **A** installs into the focused folder (`Effect::InstallTo { game, folder }`): the question closes and the store is asked as Install asks it, into that folder; the game joins the Queue. Looked at again as it is chosen: a game installed or queued meanwhile asks nothing. **B** installs nothing and closes the question; what asked has gone already.
5. **Sounds**: Install is a menu row that goes on, `GuideSelect`, and the question is in the menu's place with no sound of its own, as Manage game's hub is; then a prompt's: `GuideMove` on a move, `PromptClose` on A or B.
6. **The focus log**: `InstallWhere: Folder(0) D:\SteamLibrary, the default of Barlin's Gate 3`, `InstallWhere: Folder(1) C:\Program Files (x86)\Steam of Barlin's Gate 3`.
7. **The store asked.** `shell::store::ask` gives the installer the folder of its `folders()` that is the one chosen (by path, either slash, any case); one that has gone since is said in the log and the store's default taken. Steam: `steam client: install 9100040 into D:\SteamLibrary: asking Steam's client over its CEF debugger ...`.

Pictures: `install-where.png` (focus on the default), `install-where-other-folder.png`.

## 6. The storage ring

As my-games.md 2.6 says, from `Library::storage`'s ring: "All storage", the drive's free space under it, and the ring of how much is used with the share inside. Hidden with no drive. With the current download's block (manage-queue.md 6) the block stands 34 over the ring. Picture: `my_games_storage_ring.png`. "All storage" is the Bundle's word though the ring shows one drive, as the Bundle's does.

## 7. Not built, and hooks

- **A folder of the user's own** for a store with no folders of its own (Battle.net, Epic and GOG through their helpers: installs.md 10.2, "a folder per drive the user names once"): needs a way to name a folder with the pad, and a store that installs to it. The page lists only what installers name; `Installer::folders` is where such a store's folders will come from (#174, #175, #176).
- **Adding or removing a Steam library** is Steam's own (its Storage Manager); the page only shows them.
- **Moving a game between drives** (the console's Move) is not built.
- The ring is read again on a library change and as Settings > Storage opens, not each time My games & apps opens.
