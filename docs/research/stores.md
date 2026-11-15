# Where each store keeps its library, and how each launches and installs

Research for [#72](https://github.com/snrgarb/xbox-series-dashboard/issues/72). It feeds #73 to #80: the `Source` interface, the six `source-*` crates, installs and the Queue, and art. No code came out of it.

Sources were read on 15 November 2026, on Linux. **No store client was run and no signed-in machine was looked at**, so everything here is what other programs' source code and Microsoft's documentation say. Each claim carries a tag:

- **[code]** read in the source of a program that does this today, linked at the commit read.
- **[MS]** read in Microsoft Learn.
- **[reported]** one project's notes or a single reader, not confirmed by a second.
- **[inferred]** my reasoning from the above, or something recalled and not re-read. Treat as a guess until tested.

Every sample file below is made up. None is from a real machine, and nothing was read from `data/`.

The witnesses, and why each is trusted:

| Short name | What it is | Read at |
| --- | --- | --- |
| Playnite | Playnite's own library plugins, and its common code | [PlayniteExtensions `886468f`][pe] (21 Sep 2026), [Playnite `e2e6efc`][pn] (8 Nov 2026) |
| Playnite (EA) | The EA plugin, removed from master on 15 Oct 2025 ([`f61dfdc`][pe-ea-removed]); read at its last commit | [PlayniteExtensions `091083c`][pe-ea] |
| GameFinder | A .NET library that only finds installed games; it has tests against fake registries and file systems | [`efa23bd`][gf] (20 Jul 2025) |
| DLSS Swapper | A Windows tool that lists installed games of every store, with cover art | [`9011345`][ds] (8 Nov 2026) |
| rai-pal | A Rust tool; reads Epic's owned list from local files, including the new launcher's | [`50c638f`][rp] (15 Nov 2026) |
| Steam ROM Manager | Parsers for EA, GOG, Ubisoft, Epic | [`a026089`][srm] (11 Nov 2026) |
| Lutris, BoilR | EA launch links | [Lutris `f9ef1bc`][lu], [BoilR `3742dfe`][br] |
| TACTLib | The schema of Battle.net's `product.db` | [`f185550`][tl] (9 Nov 2026) |
| unifideck notes | One project's hands-on notes on Battle.net and Game Pass. Under Wine on a Steam Deck, not Windows | [`2e9981d`][ud] (9 Nov 2026) |

## Summary

| Store | Installed games come from | Owned, not installed, without sign-in | Launch | Install from outside | Uninstall from outside | A fake on the VM |
| --- | --- | --- | --- | --- | --- | --- |
| Epic | `Manifests\*.item` (JSON) | **Yes**, if the launcher was signed in once: `catcache.bin` | `com.epicgames.launcher://apps/…?action=launch&silent=true` | Opens the launcher (an install link exists but shows the old dialog) | No: opens the launcher's library | Files only. Easy |
| GOG | Registry `GOG.com\Games\<id>` and `goggame-<id>.info` | **Yes**, if Galaxy was signed in once: `galaxy-2.0.db` (SQLite) | The game's own exe, or `GalaxyClient.exe /command=runGame` | `GalaxyClient.exe /command=installGame` (Galaxy must be running) | `unins000.exe` in the game folder | Registry and files. Easy |
| EA app | Uninstall keys whose uninstaller is EA's, and `__Installer\installerdata.xml` | **No** | `origin2://game/launch?offerIds=<id>` | No: opens the app's library | The key's own `UninstallString` | Registry and files. Easy; the launch link needs a stand-in handler |
| Ubisoft Connect | Registry `Ubisoft\Launcher\Installs\<id>` | **Yes**, if the client was signed in once: the `configurations` cache | `uplay://launch/<id>` | `uplay://install/<id>` | `uplay://uninstall/<id>` | Registry and one small protobuf file. Easy; stand-in handler |
| Battle.net | `product.db` (protobuf) and `aggregate.json`, plus Uninstall keys | **No** | `Battle.net.exe --exec="launch <code>"` (client must be up) | Opens the game's page: `Battle.net.exe --game=<uid>` | The key's own `UninstallString` | Files and registry. Fair; a stand-in `Battle.net.exe` |
| Xbox | Packages of the user that carry a `MicrosoftGame.config` | **No** for what the user owns. The Game Pass catalogue itself is public | App user model ID, through the shell | Opens the Store or the Xbox app on the product | Not confirmed; Playnite opens Settings | A loose-file package the payload registers. Needs developer mode; untested |

Three things hold for every store:

- **Running and ended** are told the way Playnite tells them: a game is running while any process of this session has its image under the game's install folder, and has ended when none does ([`ProcessMonitor.cs`][pn-procmon] [code]). That is what the port's Launch screen already keys on (HANDOVER: "a window of a program in the game's install folder"). Only Xbox needs something else, because the install folder may not be readable; see its section.
- **"Owned without sign-in" always means "the client was signed in on this PC at some point and left a cache behind".** No store lists an account's games to a machine that has never signed in. A stale cache also keeps listing games after the account loses them.
- **A missing client is not an error.** Every reader below starts from a key or a folder that simply does not exist when the client was never installed.

## Epic Games Store

### Installed games

**`%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests\*.item`**, one JSON file per installed item. [code: [Playnite `EpicLauncher.cs`][pe-epic-launcher], [GameFinder `EGSHandler.cs`][gf-egs], [rai-pal `legacy.rs`][rp-legacy]]

The folder can be elsewhere. GameFinder reads `HKCU\Software\Epic Games\EOS`, value `ModSdkMetadataDir`, first and falls back to the path above [code]. rai-pal reads `HKLM\SOFTWARE\Epic Games\EpicGamesLauncher`, value `AppDataPath` (the `Data` folder) [code]; whether that key sits under `WOW6432Node` was not checked.

```json
{
  "bIsApplication": true,
  "bIsExecutable": true,
  "LaunchExecutable": "Lanternfall/Binaries/Win64/Lanternfall.exe",
  "LaunchCommand": "",
  "DisplayName": "Lanternfall",
  "AppName": "9c1f0e7b2a6d4c53b8e0a1d2f3c4b5a6",
  "CatalogNamespace": "4b7d2e9a1c3f4e5d8a6b0c1d2e3f4a5b",
  "CatalogItemId": "e2a4c6f8b0d14e3fa5c7d9e1b3f5a7c9",
  "AppCategories": ["public", "games", "applications"],
  "InstallLocation": "D:\\Epic\\Lanternfall",
  "InstallationGuid": "5F0A3C7E41B94D2E9A8B6C1D0E2F3A4B",
  "MainGameAppName": "9c1f0e7b2a6d4c53b8e0a1d2f3c4b5a6",
  "MainWindowProcessName": "",
  "ProcessNames": [],
  "TechnicalType": ""
}
```

The fields that matter (all of them are in Playnite's model, [`LauncherInstalled.cs`][pe-epic-model]):

- `AppName` is the game's id in every launcher link, and Playnite's `GameId`.
- `CatalogNamespace` and `CatalogItemId` are the other two parts of the launch link, and `CatalogItemId` is the key into the catalogue cache below.
- `DisplayName`, `InstallLocation`, `LaunchExecutable` (relative to `InstallLocation`).
- `AppCategories`: skip an item with `addons` and without `addons/launchable` (DLC), and one with `plugins` or `plugins/engine`; also skip when `CompatibleApps` has an entry starting `UE_` or `TechnicalType` contains `plugins/engine` (Unreal Engine content). [code: [`EpicLibrary.cs`][pe-epic-lib]]
- Skip an item whose `InstallLocation` does not exist. Two manifests for one `AppName` happen; Playnite keeps the one the install list agrees with. A manifest can be empty or invalid JSON (people hand-edit them when moving games): skip that one file. [code]

The file's name is the `InstallationGuid` with `.item` [inferred: not read anywhere; readers only glob `*.item`].

**`%PROGRAMDATA%\Epic\UnrealEngineLauncher\LauncherInstalled.dat`**, JSON, a second list. Playnite uses it only to correct a manifest's `InstallLocation`, and notes it "can be sometimes empty or have stale data". [code]

```json
{
  "InstallationList": [
    {
      "InstallLocation": "D:\\Epic\\Lanternfall",
      "AppName": "9c1f0e7b2a6d4c53b8e0a1d2f3c4b5a6",
      "AppVersion": "1.4.2"
    }
  ]
}
```

**The client itself** is found by the Uninstall entry named "Epic Games Launcher", else `C:\Program Files (x86)\Epic Games\` or `C:\Program Files\Epic Games\`, where `Launcher\Portal\Binaries\Win32\EpicGamesLauncher.exe` (preferred) or `Win64` exists. Playnite's comment: "These registry keys sometimes go missing". [code]

### Owned but not installed

**Yes, from a cache, without our own sign-in.** `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Catalog\catcache.bin` is base64 text; decoded, it is a JSON array of catalogue items for the account's library. [code: [rai-pal `legacy.rs`][rp-legacy] and [`mod.rs`][rp-mod], [DLSS Swapper `EpicGamesStoreLibrary.cs`][ds-epic]]

Decoded, made up:

```json
[
  {
    "id": "e2a4c6f8b0d14e3fa5c7d9e1b3f5a7c9",
    "namespace": "4b7d2e9a1c3f4e5d8a6b0c1d2e3f4a5b",
    "title": "Lanternfall",
    "categories": [{ "path": "games" }, { "path": "applications" }],
    "releaseInfo": [{ "appId": "9c1f0e7b2a6d4c53b8e0a1d2f3c4b5a6", "dateAdded": "2025-03-02T10:00:00.000Z" }],
    "keyImages": [
      { "type": "DieselGameBoxTall", "url": "https://cdn.example.invalid/lanternfall-tall.jpg", "width": 1200, "height": 1600 },
      { "type": "DieselGameBox", "url": "https://cdn.example.invalid/lanternfall-wide.jpg", "width": 2560, "height": 1440 }
    ]
  }
]
```

- A game is an item with a `categories` entry whose `path` is `games` [code: rai-pal]. Playnite's signed-in path filters harder (must have `applications`, must not be `digitalextras`, DLC has a `mainGameItem`) [code]; whether `catcache.bin` carries `mainGameItem` was not checked.
- `id` matches a manifest's `CatalogItemId`; `releaseInfo[0].appId` is the `AppName`. An item with no matching manifest is owned and not installed.
- Play time and last played are not in it. Playnite gets them from Epic's service with a signed-in token. [code]

**A new launcher keeps different files** [reported: only rai-pal reads them, in [`modern.rs`][rp-modern]; nothing else confirms it and I could not look at one]. Under `%LOCALAPPDATA%\com.epicgames.eglv2`:

- `cache\prod\account\<account>\library-cache.json`: `{"items":[{"appId":{"namespaceId","itemId","appName"},"title","owned"}]}`.
- `cache\prod\catalog-cache.json`: `{"items":{"<itemId>":{"item":{…the same catalogue item as above…}}}}`.
- `installations.jsonl`: one JSON object a line, `installationId`, `complete`, `dir`, `itemId`, `launchExecutable`; the last line for an `installationId` wins, and only `complete: true` counts.

rai-pal tries these first and falls back to the files above when they hold no games. `source-epic` should do the same, and #74 should confirm on a real machine which launcher the TV PC has.

### Art

- **No image files are known on disk.** The catalogue cache holds URLs (`keyImages[].url`), which point at Epic's CDN. DLSS Swapper takes `DieselGameBoxTall` (portrait), else `DieselGameBox` (landscape) [code]. Other `type` values exist; which one is a hero or a logo was not checked.
- An installed item's manifest has `VaultThumbnailUrl` in Playnite's model [code]; what it holds for a game was not checked.
- The exe's own icon is the fallback: `InstallLocation\LaunchExecutable`. [code: [`EpicMetadataProvider.cs`][pe-epic-meta]]

### Launch, install, uninstall

- **Launch:** `com.epicgames.launcher://apps/<CatalogNamespace>%3A<CatalogItemId>%3A<AppName>?action=launch&silent=true`, opened as a URL (ShellExecute). [code: Playnite's `GameLaunchUrlMask`, rai-pal's `app_uri`] The older short form `com.epicgames.launcher://apps/<AppName>?action=launch&silent=true` is what Steam ROM Manager still writes [code: [`epic.parser.ts`][srm-epic]]. The launcher starts if it is not running.
- **Install:** `com.epicgames.launcher://apps/<AppName>?action=install` exists, but Playnite stopped using it: "it shows the old installation dialog … it doesn't show install progress correctly … until somebody figures out how to invoke install via new install dialog, we just open library page". It opens `com.epicgames.launcher://store/library` and polls the manifests every 10 s. [code: [`EpicGameController.cs`][pe-epic-ctl]] rai-pal offers the three-part form with `?action=install` [code]. Neither is silent: the user confirms in the launcher.
- **Uninstall:** nothing from outside. Playnite opens the library page and polls until the manifest is gone. [code]
- **Progress of an install** is not in any file read here. The Queue (#78) can show an Epic game as installing only between "we opened the launcher" and "its manifest appeared" [inferred].

### Running and ended

Processes under `InstallLocation` (Playnite: `TrackingMode.Directory`) [code]. The manifest's `MainWindowProcessName` and `ProcessNames` are in Playnite's model but it does not use them.

### A fake on the VM

Files only, no registry, no client:

1. `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\<guid>.item` per fake game, as the sample, with `InstallLocation` pointing at a folder that holds the stand-in game as `LaunchExecutable`.
2. `C:\ProgramData\Epic\EpicGamesLauncher\Data\Catalog\catcache.bin`: the base64 of a JSON array holding the installed games and one or two more that have no manifest (owned, not installed). `keyImages` URLs must not be fetched in a test: point them at the payload's local server or leave the list empty.
3. To launch: register a stand-in for the link, `HKCU\Software\Classes\com.epicgames.launcher` with `URL Protocol` (empty string) and `shell\open\command` = `"<stand-in-launcher.exe>" "%1"`. The stand-in parses the link, finds the manifest by `AppName` and starts its `LaunchExecutable`. That is ours to write for the payload; it also proves the link's shape.
4. Without step 3, the source should fall back to nothing: an Epic game cannot be started properly without its launcher (ownership check) [inferred].

### Unknown

- Whether the TV PC's launcher writes the old files, the new ones, or both.
- Everything about `catcache.bin` beyond the fields above: when it is rewritten, whether it survives sign-out, whether it lists items the account no longer owns.
- Whether the three-part install link opens the new install dialog today.
- No way found to read an install's progress.

## GOG

### Installed games

Two records, written by the game's installer whether or not Galaxy is installed.

**Registry: `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<product id>`** (the 32-bit view). [code: [GameFinder `GOGHandler.cs`][gf-gog], [DLSS Swapper `GOGLibrary.cs`][ds-gog], [Steam ROM Manager `gog-galaxy.parser.ts`][srm-gog]]

```
[HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\GOG.com\Games\1900000042]
"gameID"="1900000042"
"gameName"="Saltmarsh Chronicles"
"path"="D:\\GOG\\Saltmarsh Chronicles"
"exe"="D:\\GOG\\Saltmarsh Chronicles\\saltmarsh.exe"
"launchCommand"="D:\\GOG\\Saltmarsh Chronicles\\saltmarsh.exe"
"launchParam"=""
"workingDir"="D:\\GOG\\Saltmarsh Chronicles"
"buildId"="57000000000000001"
```

All values are strings. `gameID`, `gameName`, `path` and `buildId` are what GameFinder requires; a `dependsOn` value naming another id marks DLC [code]. `launchCommand` and `launchParam` are read by Steam ROM Manager [code]; `exe` and `workingDir` are from memory [inferred].

**Playnite uses the Uninstall keys instead:** an entry whose key name matches `^(\d+)_is1`, with `Publisher` = `GOG.com`, not starting `GOGPACK`, and whose `InstallLocation` exists. The digits are the product id, `DisplayName` the name. [code: [`GogLibrary.cs`][pe-gog-lib]]

**File: `<install dir>\goggame-<product id>.info`**, JSON. It tells a game from DLC and says what to run. [code: [`GogLibrary.cs`][pe-gog-lib], [`GogGameTaskInfo.cs`][pe-gog-task]]

```json
{
  "gameId": "1900000042",
  "rootGameId": "1900000042",
  "name": "Saltmarsh Chronicles",
  "language": "English",
  "playTasks": [
    { "isPrimary": true, "type": "FileTask", "path": "saltmarsh.exe", "workingDir": "", "arguments": "", "name": "Saltmarsh Chronicles" },
    { "isPrimary": false, "type": "URLTask", "link": "https://example.invalid/manual", "name": "Manual" }
  ]
}
```

- No primary `playTasks` entry means DLC; so does a `rootGameId` that differs from the id. [code]
- A `FileTask`'s `path` is relative to the install folder; if `workingDir` is set and `path` starts with it, Playnite strips it (a fix for badly made files). [code]

**The client:** `HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths`, value `client` (a folder; the exe is `GalaxyClient.exe`). Galaxy counts as *open* only when the process `GOG Galaxy Notifications Renderer` exists, "because other Galaxy related process can be running in the background". [code: [`Gog.cs`][pe-gog]]

### Owned but not installed

**Yes, from Galaxy's database, without our own sign-in:** `%PROGRAMDATA%\GOG.com\Galaxy\storage\galaxy-2.0.db`, SQLite. [code: [GOG-Galaxy-Export-Script][gx], [DLSS Swapper `GOGLibrary.cs`][ds-gog], [Steam ROM Manager][srm-gog]]

- `ProductPurchaseDates.gameReleaseKey` lists what was bought or added. A release key is `<platform>_<id>`; GOG's own are `gog_<product id>`. Galaxy also lists other stores' games here when the user connected them, so filter on the `gog_` prefix. [code: the export script]
- `GamePieces(releaseKey, gamePieceTypeId, value)` holds JSON per piece; `GamePieceTypes(id, type)` names them. Piece types read by the export script: `title` (`{"title": "…"}`), `originalImages` (`{"background": url, "squareIcon": url, "verticalCover": url}`), `meta`, `originalMeta`, `summary`, `sortingTitle`, `dlcs`, `osCompatibility`. [code]
- `InstalledBaseProducts(productId, …)` and `LimitedDetails` are also there. [code: DLSS Swapper]
- Play time and last played: `GameTimes.minutesInGame` and `LastPlayedDates.lastPlayedDate`, both keyed by release key [code: the export script].

Made-up rows:

```
GamePieceTypes:        (12, 'title'), (17, 'originalImages')
ProductPurchaseDates:  ('gog_1900000042', '2024-06-01 10:00:00'), ('gog_1900000077', '2025-01-12 18:30:00')
GamePieces:            ('gog_1900000077', 12, '{"title":"Brasswork Gardens"}')
                       ('gog_1900000077', 17, '{"background":"https://images.example.invalid/bg.webp","squareIcon":"https://images.example.invalid/sq.webp","verticalCover":"https://images.example.invalid/cover.webp"}')
```

The schema is Galaxy's private business: the type ids differ per database, so look them up by name every time. Open the file read-only (Galaxy has it open; SQLite's WAL may hold newer rows) [inferred].

Without Galaxy, or before its first sign-in, there is no owned list: Playnite's own path needs a signed-in web view. [code]

### Art

- **Local, when Galaxy has shown the game:** `%PROGRAMDATA%\GOG.com\Galaxy\webcache\<user id>\gog\<product id>\<file name>`, where the file name comes from the database: `WebCache(id, releaseKey, userId)` joined to `WebCacheResources(webCacheId, webCacheResourceTypeId, filename)` and `WebCacheResourceTypes(id, type)`, type `verticalCover`. [code: DLSS Swapper] The files are WebP [inferred from the URLs' extension].
- **Local, without Galaxy:** some installs carry `<install dir>\webcache.zip` holding the same art. [code: DLSS Swapper]
- **Public, no sign-in:** `https://api.gog.com/products/<product id>?expand=description&locale=<locale>` returns `images.logo2x`, `images.icon`, `images.background` (protocol-relative URLs). [code: [`GogApiClient.cs`][pe-gog-api], [`GOGMetadataProvider.cs`][pe-gog-meta]]

### Launch, install, uninstall

- **Launch without the client:** GOG games have no launcher check, so run the primary `FileTask` directly, with its `arguments` and `workingDir`. Playnite does this unless its "start games using Galaxy" setting is on. [code] It is the simplest thing for the port and needs no fake client.
- **Launch through Galaxy** (cloud saves, play time, overlay): `GalaxyClient.exe /launchViaAutostart /gameId=<id> /command=runGame /path="<install dir>"`. [code: [`GogLibrary.cs`][pe-gog-lib]] Steam ROM Manager uses `/command=runGame /gameId=<id>` alone. [code]
- **Install:** `GalaxyClient.exe /gameId=<id> /command=installGame`, which "only works when Galaxy core components are initialized". Playnite's sequence: if Galaxy is open, send it; else start `GalaxyClient.exe`, watch `%PROGRAMDATA%\GOG.com\Galaxy\lock-files` for `GOG Galaxy Notifications Renderer.exe.lock`, then send it; 2.5 s later open `goggalaxy://openGameView/<id>` ("can't handle two instructions in quick succession"). If that cannot work, only `goggalaxy://openGameView/<id>`. Then poll the installed list every 10 s. [code: [`GogGameController.cs`][pe-gog-ctl]] Whether `installGame` starts the download or stops at Galaxy's dialog was not checked; Playnite's setting is called "automatic game installs".
- **Uninstall:** run `<install dir>\unins000.exe` and poll until the registry entry is gone. [code] It is an Inno Setup uninstaller and shows its own prompt; `/VERYSILENT` would skip it [inferred, not tested].
- **Progress of an install** is not in any file read here.

### Running and ended

Processes under the install folder. Playnite tracks the directory when launching through Galaxy and the started process otherwise. [code]

### A fake on the VM

Registry and files, no client:

1. `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>` with the values of the sample (needs admin; the payload has it). If the source reads Uninstall keys instead, also `…\Uninstall\<id>_is1` with `DisplayName`, `Publisher` = `GOG.com`, `InstallLocation`.
2. The install folder with the stand-in game and `goggame-<id>.info`.
3. For owned-not-installed: a `galaxy-2.0.db` at the real path, built by the payload with only the tables and columns named above. The source's tests build the same database in a temp folder.
4. Launch needs nothing more when the source runs the play task directly. To test install and uninstall hooks: a stand-in `GalaxyClient.exe` at the `paths\client` folder that logs its arguments, and a stand-in `unins000.exe` that deletes the registry key.

### Unknown

- Whether `galaxy-2.0.db` can be read while Galaxy is writing it without a copy (DLSS Swapper does read it in place).
- Whether `/command=installGame` is silent.
- GOG games installed by the offline installer with Galaxy absent: the registry key is expected the same [inferred].

## EA app

Playnite dropped its EA plugin in October 2025 ([`f61dfdc`][pe-ea-removed]); the commit gives no reason. Its last version could not list even installed games without a signed-in account: it walked the account's entitlements, fetched a manifest per offer from EA's servers, and tested the registry path that manifest named ([`OriginLibrary.cs`][pe-ea-lib] [code]). So the local record comes from other readers.

### Installed games

**Uninstall keys whose uninstaller is EA's.** In `HKLM` and `HKCU`, both registry views, `SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*`: keep an entry whose `UninstallString` contains both `EAInstaller` and `Cleanup.exe`. `DisplayName` is the name, `InstallLocation` the folder, `DisplayIcon` an icon. [code: [DLSS Swapper `EAAppLibrary.cs`][ds-ea]]

```
[HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{7A1C2E3F-4B5D-4E6F-8A9B-0C1D2E3F4A5B}]
"DisplayName"="Cinderpeak Rally"
"Publisher"="Electronic Arts"
"InstallLocation"="D:\\EA Games\\Cinderpeak Rally\\"
"DisplayIcon"="\"D:\\EA Games\\Cinderpeak Rally\\CinderpeakRally.exe\""
"UninstallString"="\"C:\\Program Files\\Common Files\\EAInstaller\\Cinderpeak Rally\\Cleanup.exe\" uninstall_game -autologging"
```

The arguments after `Cleanup.exe` are from memory [inferred]; the test is only the two substrings.

**`<install dir>\__Installer\installerdata.xml`** gives the id and the exe. It may be UTF-8 or UTF-16LE, and comes in two shapes. [code: [Steam ROM Manager `ea-desktop.parser.ts`][srm-ea], [Lutris `ea_app.py`][lu-ea], [Playnite's `GameInstallerData.cs`][pe-ea-inst]]

```xml
<?xml version="1.0" encoding="utf-8"?>
<DiPManifest version="4.0">
  <contentIDs>
    <contentID>1099001</contentID>
  </contentIDs>
  <gameTitles>
    <gameTitle locale="en_US">Cinderpeak Rally</gameTitle>
  </gameTitles>
  <runtime>
    <launcher>
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\Made Up Studio\Cinderpeak Rally\Install Dir]CinderpeakRally.exe</filePath>
      <parameters></parameters>
      <executeElevated>0</executeElevated>
      <requires64BitOS>1</requires64BitOS>
      <trial>0</trial>
    </launcher>
  </runtime>
</DiPManifest>
```

- The first `contentID` is the id used in the launch link. [code: Steam ROM Manager, DLSS Swapper]
- The other shape has a `<game>` root with `metadata/localeInfo/title` in place of `gameTitles`. [code: Steam ROM Manager]
- `filePath` is `[<registry key>\<value name>]<relative path>`: the bracket names a registry *value* holding the install folder. Try the 64-bit view, then the 32-bit. Pick the launcher that is not `trial`, and on 64-bit Windows the one with `requires64BitOS`. [code: Playnite's `GetPathFromPlatformPath` and `GetGamePlayTask`]
- That per-game key, `HKLM\SOFTWARE\<publisher>\<game>` with `Install Dir`, is a third record of the install. [code: same]

**The encrypted list.** `%PROGRAMDATA%\EA Desktop\530c11479fe252fc5aabc24935b9776d4900eb3ba58fdc271e0d6229413ad40e\IS` holds every install as JSON (`installInfos[]` with `softwareId`, `baseSlug`, `baseInstallPath`, `installCheck`; `schema.version`, 21 when read), AES-256-CBC after a 64-byte header. The key is SHA3-256 of `allUsersGenericId` + `IS` + the SHA-1 of a string built from the PC's hardware (baseboard, BIOS, volume serial, video controller, processor, through WMI). [code: [GameFinder `EADesktopHandler.cs`][gf-ea], [`Decryption.cs`][gf-ea-dec], [`HardwareInformation.cs`][gf-ea-hw]] GameFinder calls it "by far, the most complicated Store Handler" and warns the key changes with hardware. **Do not build on it:** it needs WMI, breaks on EA's next schema, and a fake would have to be encrypted for the VM's own hardware. The Uninstall keys give the same list.

**Scanning a default folder** for `*\__Installer\installerdata.xml` is what Steam ROM Manager (`C:\Program Files (x86)\EA Games`) and Lutris (`Program Files\EA Games`) do [code]; it misses games on another drive, so it is only a fallback.

**The client:** `HKLM\SOFTWARE\Electronic Arts\EA Desktop`, value `InstallLocation` [code: DLSS Swapper]. Playnite's plugin, still named for Origin, read `HKLM\SOFTWARE\WOW6432Node\Origin`, value `OriginPath` or `ClientPath` [code: [`Origin.cs`][pe-ea-origin]].

### Owned but not installed

**No.** No reader found gets the account's library from a local file. Playnite used `https://api1.origin.com/ecommerce2/consolidatedentitlements/<user id>` with a signed-in token [code: [`OriginAccountClient.cs`][pe-ea-acct]]; Lutris uses EA's GraphQL service, also signed in [code]. The EA app keeps a local cache of its own, but nobody reads it and it was not looked at.

### Art

- **Nothing local is known.** DLSS Swapper ships its own table of EA titles and art for this reason [code].
- The exe's icon (`DisplayIcon`, or the launcher's `filePath`) is the only local picture.
- Public, no sign-in, as Playnite used them when the plugin was dropped: `https://api2.origin.com/ecommerce2/public/supercat/<offer id>/en_IE?country=IE` and `https://data3.origin.com/ocd/<franchise>/<game>.en-us.irl.ocd` [code: [`OriginApiClient.cs`][pe-ea-api]]. They are Origin-era endpoints keyed by *offer* id, not content id; whether they still answer was not tested. Plan on #80's fallback (SteamGridDB with the user's key, else the placeholder) for EA.

### Launch, install, uninstall

- **Launch:** `origin2://game/launch?offerIds=<content id>&autoDownload=1`, opened as a URL; the EA app handles the old scheme. [code: Lutris `get_launch_arguments`, Steam ROM Manager (`origin2://game/launch/?offerIds=<id>`)] BoilR writes `link2ea://launchgame/<id>` for the EA app and keeps `origin2://` for Origin [code: [`origin_game.rs`][br-origin]]; that form has one witness.
- **Launch by exe:** Playnite ran the launcher's `filePath` directly, and for a game with EasyAntiCheat the exe named in `<install dir>\EasyAntiCheat\Launcher\Settings.json` [code: [`OriginGameController.cs`][pe-ea-ctl]]. The game then starts the EA app itself if it needs it. This works with no stand-in client, which makes it the easier default; the link is the fallback when no launcher is found.
- **Install:** nothing. Playnite opened `origin2://library/open`. Lutris: "download used to be a valid action but it doesn't seem like it's implemented in EA App". [code]
- **Uninstall:** the entry's own `UninstallString`, which runs EA's `Cleanup.exe` with its own window [inferred from the key; Playnite only opened the library].

### Running and ended

Processes under the install folder. Playnite waited **40 s** before it started watching a game that needs the client or uses EasyAntiCheat, to ride out "game process being started/shutdown multiple times during startup". [code] The port's rule (a window up for 1.5 s) should cover the same case; keep the restart in mind when deciding a game has ended early in a launch.

### A fake on the VM

1. An Uninstall key as the sample, with any `UninstallString` containing `EAInstaller` and `Cleanup.exe` (it can point at a stand-in that deletes the key).
2. The install folder with the stand-in game and `__Installer\installerdata.xml`.
3. The registry value the `filePath` bracket names, holding the install folder.
4. For the link: `HKCU\Software\Classes\origin2` with `URL Protocol` and `shell\open\command` pointing at a stand-in that reads `offerIds`, finds the folder whose `installerdata.xml` has that `contentID`, and starts its launcher.

### Unknown

- Whether the Uninstall-key test catches every EA game today, and games bought on Steam or Epic that install through the EA app.
- Whether `origin2://game/launch` still works on the current EA app, and `link2ea://` at all. A signed-in machine has to say.
- The content id against the offer id: the link's parameter is called `offerIds` and readers pass the content id. Both worked for those readers; why is not known.
- Any local cache of the owned library.

## Ubisoft Connect

### Installed games

**Registry: `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>`**, value `InstallDir`. Playnite tries the 32-bit view, then the 64-bit. The path uses forward slashes; convert them. Skip an entry whose folder does not exist. [code: [`UplayLibrary.cs`][pe-ubi-lib], [DLSS Swapper `UbisoftConnectLibrary.cs`][ds-ubi], [Steam ROM Manager `uplay.parser.ts`][srm-ubi]]

```
[HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\90210]
"InstallDir"="D:/Ubisoft/Harborlight/"
```

The key has no name for the game: Playnite uses the folder's name until the cache below supplies one. [code]

**The client:** the Uninstall entry named "Ubisoft Connect" (`InstallLocation`, exe `UbisoftConnect.exe`) [code: [`Uplay.cs`][pe-ubi]], or `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher`, value `InstallDir` [code: DLSS Swapper].

### Owned but not installed

**Yes, from the client's cache, without our own sign-in:** `%LOCALAPPDATA%\Ubisoft Game Launcher\cache\configuration\configurations`. DLSS Swapper also looks in `<client folder>\cache\configuration\configurations`. Playnite's error when it is missing: "please start the client at least once to generate user library data". [code]

It is a protobuf message [code: [`LocalCache.cs`][pe-ubi-cache]]:

```proto
message Cache  { repeated Game games = 1; }
message Game   { uint32 uplay_id = 1; uint32 install_id = 2; string game_info = 3; }
```

`game_info` is YAML [code: [`ProductSchema.cs`][pe-ubi-schema]]. A made-up entry, as text:

```
uplay_id: 90210
install_id: 90210
game_info: |
  version: 2.0
  root:
    name: l1
    thumb_image: t1
    background_image: b1
    logo_image: g1
    icon_image: i1
    is_ulc: false
    start_game:
      online:
        executables:
          - path:
              relative: Harborlight.exe
            working_directory:
              register: HKEY_LOCAL_MACHINE\SOFTWARE\Ubisoft\Launcher\Installs\90210\InstallDir
  localizations:
    default:
      l1: Harborlight
      t1: 3f9a1c7e5b2d4f60a8c1e3b5d7f90213.jpg
      b1: 8c2e4a6f0b1d3e5f7a9c1b3d5e7f9a0c.jpg
```

How Playnite turns entries into games [code]:

- `root.name` and the image fields are either the text itself or a key into `localizations.default`: look the value up there first.
- Skip an entry with `root.third_party_platform` (a Steam or Epic copy), with `root.is_ulc` (DLC), with no `root.start_game`, or whose id is in another entry's `root.addons[].id`.
- The game's id is `uplay_id`. An entry with no matching `Installs` key is owned and not installed.

**Which id the registry key is, is not settled.** Playnite matches the key name to `uplay_id`; DLSS Swapper matches it to `install_id`; Steam ROM Manager decodes a "launcher id" from the raw bytes itself. They are usually the same number [inferred]. `source-ubisoft` should match on either and the tests should cover an entry where they differ.

Play time is not in the cache.

### Art

- **Public, no sign-in:** `https://ubistatic3-a.akamaihd.net/orbit/uplay_launcher_3_0/assets/<file name>`, the file name being the resolved `thumb_image` (cover), `background_image`, `logo_image`, `icon_image` or `dialog_image`. [code: Playnite's `AssertUrlBase`, DLSS Swapper]
- **Local:** the client keeps the same files in a cache folder beside the configurations (`…\Ubisoft Game Launcher\cache\assets\<file name>`) [inferred, from memory; no reader uses it]. Check on a real machine before relying on it; the CDN works either way.

### Launch, install, uninstall

All three are links, opened as URLs. [code: [`UplayGameController.cs`][pe-ubi-ctl], `Uplay.GetLaunchString`]

- **Launch:** `uplay://launch/<id>`. Steam ROM Manager writes the same. A trailing `/0` is common in shortcuts [inferred, from memory].
- **Install:** `uplay://install/<id>`; Playnite then polls the `Installs` key every 10 s. The client shows its install dialog.
- **Uninstall:** `uplay://uninstall/<id>`; polls every 2 s. The client asks first.
- **Progress of an install** is not in any file read here.

### Running and ended

Processes under `InstallDir`, with a wrinkle. If the game's folder has a file matching `uplay_*_loader*` or `upc_*_loader*`, it needs the client: Playnite first waits for a process named `UbisoftGameLauncher`, then starts watching the folder with a delay (2 s; 10 s for one game), because the game's process is "started/shutdown multiple times during startup". [code] The client's processes are `upc` (main) and `UplayWebCore` [code: [`UplayClient.cs`][pe-ubi-client]].

### A fake on the VM

1. `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>` with `InstallDir`, and the folder with the stand-in game.
2. `%LOCALAPPDATA%\Ubisoft Game Launcher\cache\configuration\configurations`, written by the payload: the protobuf is three fields deep and can be written by hand (the fixture crate can encode it, as `source_steam::fixture` does `appinfo.vdf`). Include the installed games, one owned and not installed, one DLC and one third-party entry to prove the filters.
3. `HKCU\Software\Classes\uplay` with `URL Protocol` and a stand-in that handles `launch` (starts the first `start_game` executable under the game's `InstallDir`), and logs `install` and `uninstall`.
4. Art: point nothing at the CDN in a test.

### Unknown

- `uplay_id` against `install_id` against the registry key name (above).
- Whether a local `cache\assets` folder exists and what it holds.
- Whether the cache lists Ubisoft+ subscription games, and whether it empties on sign-out.
- Whether `uplay://install` can start without a click.

## Battle.net

### Installed games

Three records. None alone is enough.

**`%PROGRAMDATA%\Battle.net\Agent\product.db`**, protobuf. [code: [`BattleNetLibrary.cs`][pe-bn-lib] and [`BattleNetGames.cs`][pe-bn-games], [TACTLib `ProtoDatabase.cs`][tl-proto], [DLSS Swapper `BattleNetLibrary.cs`][ds-bn]] The schema, from TACTLib's generated code (field numbers read; message nesting is my reading of their order [inferred]):

```proto
message Database {
  repeated ProductInstall  product_install  = 1;
  repeated InstallHandshake active_installs = 2;
  repeated ActiveProcess   active_processes = 3;   // process_name = 1, pid = 2, uri = 3
  repeated ProductConfig   product_configs  = 4;
}
message ProductInstall {
  string uid = 1;                 // "prometheus"
  string product_code = 2;        // "pro"
  UserSettings settings = 3;      // install_path = 1, play_region = 2, …, game_subfolder = 13
  CachedProductState cached_product_state = 4;
  string product_family = 6;
  bool hidden = 7;
}
message CachedProductState {
  BaseProductState base_product_state = 1;   // installed = 1, playable = 2, update_complete = 3, current_version_str = 7
  UpdateProgress   update_progress    = 4;   // progress = 2, total_to_download = 4, download_remaining = 5
}
```

A made-up entry, as text:

```
product_install {
  uid: "emberwatch"
  product_code: "emb"
  settings { install_path: "D:/Battle.net/Emberwatch" play_region: "eu" }
  cached_product_state { base_product_state { installed: true playable: true update_complete: true current_version_str: "2.1.0.4711" } }
  product_family: "EMBR"
}
```

- It also lists the client's own parts. DLSS Swapper skips a `uid` matching `^(agent|agent_beta|beta|battle\.net|wow.*)$` [code] (the `wow.*` there is that tool's own choice, not a rule). Playnite instead keeps only uids in its table of known games. [code]
- "Uninstalled games sometimes remain": keep only `base_product_state.installed` [code: DLSS Swapper]. The unifideck notes go further: installed means `installed`, `playable` and `update_complete` together [reported].
- Older files are a bare `ProductInstall`, not a `Database`: TACTLib falls back to that when parsing fails. [code]

**`%PROGRAMDATA%\Battle.net\Agent\aggregate.json`**, plain JSON. [code: DLSS Swapper reads `installed[].product_id` and the logo art; the full shape is [reported] by the [unifideck notes][ud-bn]]

```json
{
  "installed": [
    {
      "product_id": "emb",
      "name": "Emberwatch",
      "icon_path": "D:/Battle.net/Emberwatch/Emberwatch Launcher.exe",
      "launch_uri": "battlenet://game/emb",
      "box_art_uri": "https://assets.example.invalid/emb/box-enUS.webp",
      "logo_art_uri": "https://assets.example.invalid/emb/logo-enUS.webp",
      "last_played_timestamp": 0,
      "icon_index": 0
    }
  ]
}
```

`product_id` here is `product.db`'s `product_code` [code: DLSS Swapper joins them]. It gives the name and art that `product.db` lacks. **It is written early:** an entry appears while the game is still downloading, so it does not mean installed [reported].

**Uninstall keys:** an entry whose `UninstallString` matches `Battle\.net.*--uid=(.*?)\s`; the group is the `uid`, `InstallLocation` the folder. Playnite reads these first and uses `product.db` as "fallback" because "Battle.net client seems to not create registry keys for some games". Skip a `DisplayName` ending `Test` or `Beta`. Old games (Diablo II, Warcraft III) are plain entries with `Publisher` = `Blizzard Entertainment` and a known `DisplayName`. [code]

**Names.** Playnite carries a hand-kept table of 37 products ([`BattleNetGames.cs`][pe-bn-games]): for each, a `ProductId` (the launch code: `WoW`, `D3`, `Pro`, `Fen`, `WTCG`…), an `InternalId` (the `uid`: `wow`, `diablo3`, `prometheus`…), a name and art URLs. A game not in the table is ignored. `aggregate.json` carries names itself, so the port needs no such table for *listing*; the launch code is another matter, below.

**The client:** the Uninstall entry whose `UninstallString` contains `-uid=battle.net` [code: [`BattleNet.cs`][pe-bn]], or the key named `Battle.net` under Uninstall [code: DLSS Swapper]; `InstallLocation\Battle.net.exe`.

### Owned but not installed

**No.** Playnite gets the account's games from `https://account.battle.net/api/games-and-subs` and `/api/classic-games` in a signed-in web view. [code: [`BattleNetAccountClient.cs`][pe-bn-acct]] No local file with the owned list was found.

### Art

- `aggregate.json`'s `box_art_uri` and `logo_art_uri` are Blizzard CDN URLs, for installed games only. [reported; DLSS Swapper uses the logo one, code]
- No local image cache is known. The exe's icon is `icon_path`.

### Launch, install, uninstall

- **Launch:** `Battle.net.exe --exec="launch <code>"`, the code being Playnite's `ProductId` (`Pro`, `Fen`…). **The client must already be running**: Playnite starts it, waits until it has 5 or more processes, then 5 s more, then sends the command. [code: [`BattleNetGameController.cs`][pe-bn-ctl]]
  - The unifideck notes saw the same command only *navigate the client to the game's page*, and saw a wrong code do nothing at all with no error [reported, under Wine]. Whether it starts the game on Windows today is the main thing to confirm on a real machine.
  - The code is not in `product.db` by that name. `product_family` may be it (the notes call the launch code the "FAMILY") [inferred]; if not, a table like Playnite's is needed.
  - `battlenet://game/<product_code>` from `aggregate.json` is a second form nobody has tested [reported].
- **Install:** `Battle.net.exe --game=<uid>` opens the game's page; the user clicks Install. Playnite then polls the Uninstall keys every 10 s. [code] `--exec="install <code>"` did nothing in the notes [reported].
- **Uninstall:** run the entry's `UninstallString` (`cmd /C "<string>"`), which opens Blizzard's uninstaller, and poll until the key is gone. [code]
- **Progress of an install:** `update_progress` has `total_to_download` and `download_remaining`, but the notes found the progress fields stayed at zero through a whole download [reported]. Plan on none.

### Running and ended

Processes under the install folder [code]. `product.db`'s `active_processes` (process name, pid, uri) looks made for this, but no reader uses it and it was not seen filled [inferred].

### A fake on the VM

1. `C:\ProgramData\Battle.net\Agent\product.db` with a few `ProductInstall` entries (one not `installed`, one of the client's own), and `aggregate.json` beside it. The fixture crate writes the protobuf.
2. Install folders with the stand-in game.
3. An Uninstall key for the client (`UninstallString` containing `-uid=battle.net`, `InstallLocation`) and a stand-in `Battle.net.exe` there that stays running and, on `--exec="launch <code>"`, starts the matching game. Because the real client must be running first, the stand-in should be started by the source the same way, so the test covers the wait.
4. Uninstall keys per game if the source reads them.

### Unknown

- Whether `--exec="launch …"` starts a game or only shows its page, on Windows, today.
- Where the launch code comes from without a hand-kept table.
- Whether `battlenet://game/<code>` works.
- Games from Activision that Battle.net carries: no reader was checked for them.

## Xbox: packaged games and Game Pass

How a packaged game is activated, and what that needs from the shell, is the Bundle's [`game-pass-without-explorer.md`](https://github.com/snrgarb/xbox-series-theme/blob/main/docs/research/game-pass-without-explorer.md). This section adds how to find the games and tell them apart.

### Installed games

A Game Pass or Store PC game is a package registered for the user. Two ways to list them.

**1. The package manager** (WinRT `Windows.Management.Deployment.PackageManager`, `FindPackagesForUser` with the current user's SID). This is what Playnite and DLSS Swapper do. For each package Playnite: skips frameworks and resource packages and anything whose `SignatureKind` is not `Store`; opens `<InstalledLocation>\AppxManifest.xml`; takes the first `Application`'s `Id`; takes the name from `Properties/DisplayName` (resolving `ms-resource:` names through the package's resources); takes the logo from `VisualElements` (`Square150x150Logo`, else `Square70x70Logo`, `Square44x44Logo`, `Logo`), picking the largest `.scale-NNN.png` when the plain file is absent. Its id for the game is the **package family name**. [code: [`Programs2.cs` `GetUWPApps`][pn-programs2]]

That lists every Store app. **Playnite cannot tell a game from an app locally**: it asks Xbox's title service, signed in, whether each package is of type `Game`. [code: [`XboxLibrary.cs`][pe-xbox-lib]] The port's rule, from #76, is local: **a package is a game when its root has a `MicrosoftGame.config`**. Every GDK game has one: Microsoft's packaging steps start with "Create a MicrosoftGame.config, and put it in the root folder alongside your game executable" [MS: [What is the GDK][ms-gdk-intro]]. Older UWP games from the Store have none and would be missed; they can be added by hand as Apps (#77).

**2. The folders, with no WinRT.** Each drive that holds Xbox games has a `.GamingRoot` file at its root: the bytes `RGBX`, a 32-bit count, then that many folder names, UTF-16LE, each ended by a zero. Each names a folder relative to the drive (by default `XboxGames`). Under it, one folder per game with `Content\appxmanifest.xml` and `Content\MicrosoftGame.config`. Also `<drive>\Program Files\ModifiableWindowsApps\<game>\appxmanifest.xml`. [code: [GameFinder `XboxHandler.cs`][gf-xbox], [DLSS Swapper `XboxLibrary.cs`][ds-xbox]] Microsoft: with flat-file install "all paths will now point to C:\XboxGames unless otherwise specified" [MS: [Flat File Install][ms-ffi]]. Games still under `C:\Program Files\WindowsApps` are not found this way, and that folder cannot be listed by a normal user [inferred; the Bundle's research calls it locked down].

Use both: folders first (fast, no COM), the package manager to catch the rest and to confirm the package is registered for this user.

**`MicrosoftGame.config`** [MS: [schema summary][ms-mgc]; the elements DLSS Swapper reads, code]:

```xml
<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
  <Identity Name="MadeUpStudio.TidewaterTrials" Publisher="CN=00000000-1111-2222-3333-444444444444" Version="1.2.0.0" />
  <ExecutableList>
    <Executable Name="TidewaterTrials.exe" Id="Game" TargetDeviceFamily="PC" />
  </ExecutableList>
  <StoreId>9ZZZMADEUP01</StoreId>
  <TitleId>7F00ABCD</TitleId>
  <ShellVisuals DefaultDisplayName="Tidewater Trials"
                PublisherDisplayName="Made Up Studio"
                Square480x480Logo="Assets\Logo480.png"
                Square150x150Logo="Assets\Logo150.png"
                Square44x44Logo="Assets\Logo44.png"
                StoreLogo="Assets\StoreLogo.png"
                SplashScreenImage="Assets\Splash.png" />
</Game>
```

- `Identity` `Name` and `Publisher` are required; `ShellVisuals` "defines the title's Shell presence. For example, Images and Names"; `ExecutableList` needs at least one `Executable`, whose attributes include `Name` and its application `Id`. [MS]
- `StoreId` is optional, 12 alphanumeric characters; `TitleId` is 8 hex digits. [MS: [StoreId][ms-mgc-storeid], [TitleId][ms-mgc-titleid]]
- `configVersion` is `0` or `1`; DLSS Swapper skips anything else. [code]
- `DefaultDisplayName` can be an `ms-resource:` name, as in the manifest [inferred].

**`AppxManifest.xml`**, the part that matters:

```xml
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10">
  <Identity Name="MadeUpStudio.TidewaterTrials" Publisher="CN=00000000-1111-2222-3333-444444444444" Version="1.2.0.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Tidewater Trials</DisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Applications>
    <Application Id="Game" Executable="GameLaunchHelper.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Tidewater Trials" Square150x150Logo="Assets\Logo150.png" Square44x44Logo="Assets\Logo44.png" />
    </Application>
  </Applications>
</Package>
```

**The app user model ID** is `<package family name>!<Application Id>`. The family name is `<Identity Name>_<publisher id>`, the publisher id being a 13-character hash of `Publisher`. Do not compute the hash by hand: `PackageFamilyNameFromId` gives the family name from a name and publisher, and `FormatApplicationUserModelId` joins it to the application id [MS: [appmodel.h][ms-appmodel]]; the package manager's `Package.Id.FamilyName` is the same thing [code: Playnite].

### Owned but not installed

**Not without sign-in.** What the user owns or has through Game Pass comes from Xbox's services: Playnite reads `titlehub.xboxlive.com/users/xuid(<id>)/titles/titlehistory/…` with a signed-in token, and that only lists games the account has *played* ("a game that was not started at least once … won't appear in user API data"). [code: [`XboxAccountClient.cs`][pe-xbox-acct], `XboxLibrary.cs`] No local file of the Xbox app with the library was found; none was looked for on a real machine.

**The Game Pass catalogue itself is public** [reported: [unifideck `game_pass.py`][ud-gp], which says it was measured against a real account on 4 October 2026]: `https://catalog.gamepass.com/sigls/v2?id=<list id>&language=<lang>&market=<market>` returns product ids for a list ("all PC games" is one of five list ids in that file), with no sign-in. Product details and art for an id come from Microsoft's display catalogue; that file uses it, but the exact request was not read. This is "what Game Pass has", not "what this user may play": without sign-in the port cannot know whether the user subscribes. It is a possible "Browse Game Pass" page, not a library. Listed under Needs a human in #14: a signed-in look at what the Xbox app caches locally.

### Art

- **In the package:** the `ShellVisuals` images, relative to the config's folder. DLSS Swapper tries `Square480x480Logo`, then `Square150x150Logo`, `StoreLogo`, `Square44x44Logo` [code]. `SplashScreenImage` is the only wide one. They are square logos, small for a Home tile and nothing like a hero.
- When the plain file name is absent, look for `<name>.scale-NNN.png` and take the largest. [code: Playnite `GetUWPGameIcon`]
- **Public:** the display catalogue by `StoreId` [reported, request not read]. Otherwise #80's fallback.
- Nothing of Microsoft's goes in the repository: fixtures use generated plain-colour images.

### Launch, install, uninstall

- **Launch:** activate the app user model ID. Playnite runs `explorer.exe shell:AppsFolder\<family name>!<app id>` [code]; the documented API is `IApplicationActivationManager::ActivateApplication` [MS, via the Bundle's research]. Either needs the shell's activation manager, so neither works with no explorer shell: the Bundle's research has the whole of that, and its tests T1 to T3 are still to run on the TV PC. Never run the game's exe directly: it skips "app-launch" and the game loses its identity and cloud services [MS, same].
  - `ActivateApplication` returns the new process id, which the explorer route does not give [inferred from the API's signature; not re-read here]. For a GDK game that process is `GameLaunchHelper.exe`, not the game.
- **Install:** `ms-windows-store://pdp/?ProductId=<StoreId>` opens the Store on the product; "the recommended way". The `?PFN=<family name>` form, which Playnite uses, "is deprecated and may not be supported in the future". [MS: [ms-windows-store URIs][ms-store-uri]; code: [`XboxGameController.cs`][pe-xbox-ctl]] Playnite's other choice is to open the Xbox app (`Microsoft.GamingApp_8wekyb3d8bbwe`) with no product. [code] A link straight to a game in the Xbox app (`msxbox://…`) is used by other tools but was not read in any source here [inferred]. Either way the user presses Install there; Playnite polls the package list every 10 s.
- **Uninstall:** Playnite opens `ms-settings:appsfeatures` and polls. [code] The package manager has a remove call that needs no admin for the user's own packages [inferred, from memory; not read]. Whether it cleanly removes a GDK game with its `XboxGames` folder is unknown: confirm on a real machine before the Game hub's Uninstall uses it.
- **Progress of an install** was not found in any file. The Xbox app and the Store show it; nothing local was read.

### Running and ended

The install folder under `WindowsApps` may not be readable, and a process's image path there may not be either. Ask the process instead: `GetPackageFamilyName(hProcess)` and `GetApplicationUserModelId(hProcess)` take a handle opened with `PROCESS_QUERY_LIMITED_INFORMATION` and return the family name or the app user model ID, or `APPMODEL_ERROR_NO_PACKAGE` for an ordinary program [MS: [GetPackageFamilyName][ms-gpfn], [GetApplicationUserModelId][ms-gaumid]]. A game is running while any process of this session reports its family name, and ended when none does. This also covers the hand-over from `GameLaunchHelper.exe` to the game, which are both in the package [inferred].

Playnite falls back to the same folder rule as elsewhere when it can read the folder, with a note that proper tracking waits on a 64-bit build. [code]

The PC Bootstrapper (`GamingServicesUI.exe`) shows a splash, may update the game and syncs saves before the game's window appears [MS, via the Bundle's research]: the Launch screen will be up longer than for other stores.

### A fake on the VM

The plan in #76 is a self-signed package built by the payload. Two cheaper routes exist; **none was tried**.

1. **Listing only, files alone:** a `.GamingRoot` at `C:\` naming `XboxGames`, and `C:\XboxGames\<Fake>\Content\` with a made-up `MicrosoftGame.config`, `appxmanifest.xml` and generated logos. This tests the folder reader and the art, on Linux too. The game will not launch (no package is registered).
2. **A registered loose-file package, no certificate:** `Add-AppxPackage -Register <path>\AppxManifest.xml` registers a folder of loose files on the local device, "during development" [MS: [loose file registration][ms-loose], [Add-AppxPackage][ms-addappx]]. It needs developer mode turned on first [inferred: the pages read do not say so in as many words]. The folder holds the manifest, the stand-in game as the `Application`'s `Executable`, the logos, and a `MicrosoftGame.config` so the source counts it as a game. It then launches by its app user model ID like a real one, and `GetPackageFamilyName` reports it. A full-trust exe in the manifest needs the `runFullTrust` capability, which a sideloaded package may declare [inferred].
3. **An unsigned `.msix`:** Windows 11 installs one with `Add-AppxPackage -AllowUnsigned` when the `Identity`'s `Publisher` carries the special OID (`…, OID.2.25.311729368913984317654407730594956997722=1`); with executable content it needs an administrator. [MS: [unsigned package][ms-unsigned]] Building the `.msix` needs `makeappx`, so route 2 is less work.

Traps:

- **Do not filter on `SignatureKind == Store`** as Playnite does: the fake is not Store-signed, and neither is a game a developer sideloads. Filter on `MicrosoftGame.config` instead.
- The fake has no Gaming Services launch flow, so it starts faster than a real game and shows no bootstrapper.
- `Remove-AppxPackage` at the end of the payload, so the VM template stays clean.
- If registration fails on the VM (developer mode cannot be turned on, or policy forbids sideloading), the payload says why and the check moves to the TV PC list, as #76 already allows.

### Unknown

- Whether any local file of the Xbox app lists the account's library or Game Pass entitlement.
- Whether a GDK game still under `WindowsApps` (not flat-file) lets a normal user read its `MicrosoftGame.config` through `InstalledLocation`.
- Uninstalling from outside.
- A link that opens the Xbox app on one game.
- Whether launching works under the Boot shell at all: the Bundle's T1 to T3.

## What this means for #73 to #80

- **#73, the `Source` interface.** Every store can list installed games from local files alone; four of six can also give a name and art without the network. "Owned" needs a third state beside installed and not installed: **unknown** (EA, Battle.net, Xbox never say; the other three say only when a cache exists). "Is it running" is one shared function over an install folder for five stores, and a family-name check for Xbox. Install and uninstall are, for most, "open the client here and poll the installed list": the interface should let a source answer "opened the client" as a result, not only "started".
- **#74, Epic and GOG.** Both are file readers plus one link. GOG can launch with no client at all. Epic's owned list needs the base64 decode; read the new launcher's files first when they exist.
- **#75, EA, Ubisoft, Battle.net.** Ubisoft is the only one with an owned list. EA and Battle.net launches are the two that most need a real machine; both go under Needs a human.
- **#76, Xbox.** Local rule for "is a game": `MicrosoftGame.config`. Loose-file registration may spare the self-signed package.
- **#78, the Queue.** No store but Steam exposes download progress in a file. For the others a tile can read *Installing…* without a percentage, from "the client was asked" until the game appears in the installed list, with a timeout that drops it.
- **#80, art.** Local or public art exists for Epic (URLs in the cache), GOG (files and a public API), Ubisoft (public CDN), Battle.net (URLs for installed games), Xbox (square logos). EA has none.

### For "Needs a human" in #14

One signed-in Windows PC with each client, to answer:

1. Epic: which launcher files exist; does the three-part install link open the new dialog.
2. GOG: is `/command=installGame` silent; read `galaxy-2.0.db` while Galaxy runs.
3. EA: do `origin2://game/launch?offerIds=` and `link2ea://launchgame/` work; does the Uninstall-key test list every game.
4. Ubisoft: `uplay_id`, `install_id` and the registry key name for a few games; is there a `cache\assets` folder.
5. Battle.net: does `--exec="launch <code>"` start the game; is `product_family` the code; does `battlenet://game/<code>` work.
6. Xbox: what the Xbox app caches locally; removing a GDK game from outside; the Bundle's T1 to T3 under the Boot shell.

[pe]: https://github.com/JosefNemec/PlayniteExtensions/tree/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries
[pn]: https://github.com/JosefNemec/Playnite/tree/e2e6efcda311fea38fcd92f61c8ca90811d97f28/source/Playnite/Common
[pn-procmon]: https://github.com/JosefNemec/Playnite/blob/e2e6efcda311fea38fcd92f61c8ca90811d97f28/source/Playnite/Common/ProcessMonitor.cs
[pn-programs2]: https://github.com/JosefNemec/Playnite/blob/e2e6efcda311fea38fcd92f61c8ca90811d97f28/source/Playnite/Common/Programs2.cs
[pe-epic-launcher]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicLauncher.cs
[pe-epic-model]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/Models/LauncherInstalled.cs
[pe-epic-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicLibrary.cs
[pe-epic-ctl]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicGameController.cs
[pe-epic-meta]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicMetadataProvider.cs
[pe-gog]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/Gog.cs
[pe-gog-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/GogLibrary.cs
[pe-gog-task]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/Models/GogGameTaskInfo.cs
[pe-gog-ctl]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/GogGameController.cs
[pe-gog-api]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/Services/GogApiClient.cs
[pe-gog-meta]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/GOGMetadataProvider.cs
[pe-ubi]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/Uplay.cs
[pe-ubi-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/UplayLibrary.cs
[pe-ubi-cache]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/Models/LocalCache.cs
[pe-ubi-schema]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/Models/ProductSchema.cs
[pe-ubi-ctl]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/UplayGameController.cs
[pe-ubi-client]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/UplayClient.cs
[pe-bn]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNet.cs
[pe-bn-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNetLibrary.cs
[pe-bn-games]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNetGames.cs
[pe-bn-ctl]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNetGameController.cs
[pe-bn-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/Services/BattleNetAccountClient.cs
[pe-xbox-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/XboxLibrary/XboxLibrary.cs
[pe-xbox-ctl]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/XboxLibrary/XboxGameController.cs
[pe-xbox-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/XboxLibrary/Services/XboxAccountClient.cs
[pe-ea-removed]: https://github.com/JosefNemec/PlayniteExtensions/commit/f61dfdcfadeb05b26c1490688ceb6646d4e77b64
[pe-ea]: https://github.com/JosefNemec/PlayniteExtensions/tree/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary
[pe-ea-lib]: https://github.com/JosefNemec/PlayniteExtensions/blob/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary/OriginLibrary.cs
[pe-ea-origin]: https://github.com/JosefNemec/PlayniteExtensions/blob/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary/Origin.cs
[pe-ea-ctl]: https://github.com/JosefNemec/PlayniteExtensions/blob/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary/OriginGameController.cs
[pe-ea-inst]: https://github.com/JosefNemec/PlayniteExtensions/blob/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary/Models/GameInstallerData.cs
[pe-ea-acct]: https://github.com/JosefNemec/PlayniteExtensions/blob/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary/Services/OriginAccountClient.cs
[pe-ea-api]: https://github.com/JosefNemec/PlayniteExtensions/blob/091083c5e07d425c60c9918f4aa796a9bc8ec63b/source/Libraries/OriginLibrary/Services/OriginApiClient.cs
[gf]: https://github.com/erri120/GameFinder/tree/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f
[gf-egs]: https://github.com/erri120/GameFinder/blob/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f/src/GameFinder.StoreHandlers.EGS/EGSHandler.cs
[gf-gog]: https://github.com/erri120/GameFinder/blob/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f/src/GameFinder.StoreHandlers.GOG/GOGHandler.cs
[gf-ea]: https://github.com/erri120/GameFinder/blob/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f/src/GameFinder.StoreHandlers.EADesktop/EADesktopHandler.cs
[gf-ea-dec]: https://github.com/erri120/GameFinder/blob/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f/src/GameFinder.StoreHandlers.EADesktop/Crypto/Decryption.cs
[gf-ea-hw]: https://github.com/erri120/GameFinder/blob/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f/src/GameFinder.StoreHandlers.EADesktop/Crypto/HardwareInformation.cs
[gf-xbox]: https://github.com/erri120/GameFinder/blob/efa23bd6d02ec07aa9fb1abe1025efafe3190d9f/src/GameFinder.StoreHandlers.Xbox/XboxHandler.cs
[ds]: https://github.com/beeradmoore/dlss-swapper/tree/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data
[ds-epic]: https://github.com/beeradmoore/dlss-swapper/blob/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data/EpicGamesStore/EpicGamesStoreLibrary.cs
[ds-gog]: https://github.com/beeradmoore/dlss-swapper/blob/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data/GOG/GOGLibrary.cs
[ds-ea]: https://github.com/beeradmoore/dlss-swapper/blob/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data/EAApp/EAAppLibrary.cs
[ds-ubi]: https://github.com/beeradmoore/dlss-swapper/blob/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data/UbisoftConnect/UbisoftConnectLibrary.cs
[ds-bn]: https://github.com/beeradmoore/dlss-swapper/blob/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data/BattleNet/BattleNetLibrary.cs
[ds-xbox]: https://github.com/beeradmoore/dlss-swapper/blob/901134553baefd6581eca3595dc4cf15a869d2a8/src/Data/Xbox/XboxLibrary.cs
[rp]: https://github.com/Raicuparta/rai-pal/tree/50c638f01adeb5f1156f07f7de76ecd4cf6eca0f/crates/backend/src/game_providers/epic
[rp-legacy]: https://github.com/Raicuparta/rai-pal/blob/50c638f01adeb5f1156f07f7de76ecd4cf6eca0f/crates/backend/src/game_providers/epic/legacy.rs
[rp-mod]: https://github.com/Raicuparta/rai-pal/blob/50c638f01adeb5f1156f07f7de76ecd4cf6eca0f/crates/backend/src/game_providers/epic/mod.rs
[rp-modern]: https://github.com/Raicuparta/rai-pal/blob/50c638f01adeb5f1156f07f7de76ecd4cf6eca0f/crates/backend/src/game_providers/epic/modern.rs
[srm]: https://github.com/SteamGridDB/steam-rom-manager/tree/a02608984941eea9361afbc4a7b1fe2a162845e8/src/lib/parsers
[srm-epic]: https://github.com/SteamGridDB/steam-rom-manager/blob/a02608984941eea9361afbc4a7b1fe2a162845e8/src/lib/parsers/epic.parser.ts
[srm-gog]: https://github.com/SteamGridDB/steam-rom-manager/blob/a02608984941eea9361afbc4a7b1fe2a162845e8/src/lib/parsers/gog-galaxy.parser.ts
[srm-ea]: https://github.com/SteamGridDB/steam-rom-manager/blob/a02608984941eea9361afbc4a7b1fe2a162845e8/src/lib/parsers/ea-desktop.parser.ts
[srm-ubi]: https://github.com/SteamGridDB/steam-rom-manager/blob/a02608984941eea9361afbc4a7b1fe2a162845e8/src/lib/parsers/uplay.parser.ts
[lu]: https://github.com/lutris/lutris/tree/f9ef1bc4b5b6a3bb52e26ef973dc5b7d66965634
[lu-ea]: https://github.com/lutris/lutris/blob/f9ef1bc4b5b6a3bb52e26ef973dc5b7d66965634/lutris/services/ea_app.py
[br]: https://github.com/PhilipK/BoilR/tree/3742dfeb8b4eed79dd238b44c66abba48cc0b6c3
[br-origin]: https://github.com/PhilipK/BoilR/blob/3742dfeb8b4eed79dd238b44c66abba48cc0b6c3/src/platforms/origin/origin_game.rs
[tl]: https://github.com/overtools/TACTLib/tree/f1855500441a7072dea26d75a2b120e43bb2b0d8/TACTLib/Agent
[tl-proto]: https://github.com/overtools/TACTLib/blob/f1855500441a7072dea26d75a2b120e43bb2b0d8/TACTLib/Agent/Protobuf/ProtoDatabase.cs
[ud]: https://github.com/mubaraknumann/unifideck/tree/2e9981d2de73db8aba7864c5f351fe630895e2d7
[ud-bn]: https://github.com/mubaraknumann/unifideck/blob/2e9981d2de73db8aba7864c5f351fe630895e2d7/docs/feasibility/battlenet.md
[ud-gp]: https://github.com/mubaraknumann/unifideck/blob/2e9981d2de73db8aba7864c5f351fe630895e2d7/py_modules/unifideck/stores/microsoft/game_pass.py
[gx]: https://github.com/AB1908/GOG-Galaxy-Export-Script/blob/9d13805bbfb03cf9634f5581d11bdc93b1fa7cf8/galaxy_library_export.py
[ms-gdk-intro]: https://learn.microsoft.com/gaming/gdk/docs/gdk-dev/intro/introduction
[ms-ffi]: https://learn.microsoft.com/gaming/gdk/docs/features/common/packaging/packaging-flatfileinstall
[ms-mgc]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/microsoftgameconfig/microsoftgameconfig-schema
[ms-mgc-storeid]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/microsoftgameconfig/elements/microsoftgameconfig-element-storeid
[ms-mgc-titleid]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/microsoftgameconfig/elements/microsoftgameconfig-element-titleid
[ms-appmodel]: https://learn.microsoft.com/windows/win32/api/appmodel/
[ms-gpfn]: https://learn.microsoft.com/windows/win32/api/appmodel/nf-appmodel-getpackagefamilyname
[ms-gaumid]: https://learn.microsoft.com/windows/win32/api/appmodel/nf-appmodel-getapplicationusermodelid
[ms-store-uri]: https://learn.microsoft.com/windows/apps/develop/launch/launch-store-app
[ms-loose]: https://learn.microsoft.com/windows/apps/develop/testing/loose-file-registration
[ms-addappx]: https://learn.microsoft.com/powershell/module/appx/add-appxpackage
[ms-unsigned]: https://learn.microsoft.com/windows/msix/package/unsigned-package
