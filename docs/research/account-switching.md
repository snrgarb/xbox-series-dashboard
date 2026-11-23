# Profiles bound to store accounts, switched with the Profile

Research for [#265](https://github.com/snrgarb/xbox-series-dashboard/issues/265), milestone **M10 Account switching**. The user's request, 2026-11-18: "I'd like to have the profile system be bound to specific steam, epic, battle.net and xbox accounts (i know xbox is harder because of credentials, maybe there could be a middleman executable that injects the credentials at runtime? take a look at Xodeus Nexus)".

The question, then: when a Profile is chosen at Who's playing today? or at Switch profile, can each launcher on the TV PC be moved to that Profile's account, on the user's own PC, with accounts that have each been signed in there before, and without the Dashboard ever holding a password?

Read on 18 November 2026, on Linux. **No store client was run, nothing was signed in to, and nothing was read from `data/` or the TV PC.** What follows is other programs' source code, Microsoft's documentation, the stores' own help pages and terms. It builds on [stores.md](stores.md) (where each store keeps its library), [installs.md](installs.md) (how the Dashboard drives each client) and [../sources.md](../sources.md) (what the Dashboard does today), and does not repeat them. Tags, as there:

- **[code]** read in the source of a program that does it, linked at the commit read.
- **[MS]** Microsoft Learn or Microsoft's own pages.
- **[docs]** a store's own help or terms, or a project's README about itself. "(excerpt)" where only a search engine's excerpt of the page could be read, because the page refused automated reading.
- **[reported]** one forum post, one project's notes, one article: not confirmed by a second.
- **[inferred]** my reasoning. A guess until the TV PC tests it.

Whether something breaks a store's terms is **my reading, not legal advice**: every such line is [inferred].

## Decisions, 18 November 2026

The user answered the open questions the same day (recorded on [#265](https://github.com/snrgarb/xbox-series-dashboard/issues/265)); the follow-up research for them was read on 18 November 2026 too, on the same terms as above (nothing run, nothing signed in to).

1. **Xbox: no Windows user per person.** The user's answer was "Nah". **Reading to confirm with the user**: the Dashboard only names the Profile's Xbox account and, when the Xbox app is signed in as another, opens the Xbox app's own account page; it switches nothing (5.3). Section 3.4 stays as the record of why not.
2. **No credential helper** (the "middleman executable", 3.5). Agreed.
3. **A kept copy of the Epic launcher's sign-in per account: undecided.** The user asked for the security risks first; they are in [section 7](#7-keeping-a-copy-of-a-launchers-sign-in-the-risks), with the alternative that keeps no copy. GOG, EA and Ubisoft raise the same question (7.6).
4. **Play time and achievements follow the Profile, not the store account** (5.5).
5. **GOG, EA and Ubisoft switch too** (1.5 to 1.7, tickets in 5.6).
6. **Binding a Discord account is out of scope.**
7. **A download under way at switch time**: wait until it is done, or offer to cancel it and switch (5.4). Pausing it is no longer offered.
8. **Both routes scoped**: the Dashboard doing the file work itself, or calling TcNo Account Switcher's command line, side by side in [section 8](#8-two-routes-the-dashboards-own-file-work-or-tcnos-command-line). Recommended: the Dashboard's own (8.4). Not yet chosen by the user.

## Summary

| Client | Switch between accounts it already remembers, with no password? | How | Client restart | Confidence |
|---|---|---|---|---|
| **Steam** | **Yes** | `config\loginusers.vdf`: the target's `MostRecent`, `AutoLogin`, `RememberPassword` to 1 and every other row's to 0; `HKCU\Software\Valve\Steam` `AutoLoginUser` = its account name, `RememberPassword` = 1; Steam closed with `steam.exe -shutdown` first and started after. Steam keeps each remembered account's own session token, so nothing is copied | Yes | [code: TcNo]; Steam's own Change Account does the same with a click [reported] |
| **Epic Games Launcher** (current, `com.epicgames.eglv2`) | **Yes, by swapping a snapshot** | The launcher remembers **one** account. Its session is a Windows Credential Manager entry `credentials.com.epicgames.eglv2.prod` plus the folder `%LocalAppData%\com.epicgames.eglv2\auth` (DPAPI files). A snapshot of both is taken while each account is signed in, and put back to switch | Yes, killed | [code: TcNo]; single source |
| Epic, older launcher | Yes, by swap | `GameUserSettings.ini` `[RememberMe] Data` and `HKCU\Software\Epic Games\Unreal Engine\Identifiers` `AccountId` | Yes | [code: TcNo, legendary] |
| **Battle.net** | **Probably yes** | `%AppData%\Battle.net\Battle.net.config` `Client.SavedAccountNames`: the target's email put first; each account must have been signed in with the launcher's "remember" ticked | Yes, killed | [code: TcNo]; where the token itself lives is not known |
| **Xbox app / Microsoft Store / Gaming Services** | **Not by files. Only through Microsoft's own sign-in UI** | Two separate accounts: the **Xbox profile** (gamertag, achievements, friends) and the **Microsoft Store account** (what is owned and Game Pass). Both are held by Windows' Web Account Manager, not in files a program can swap. The Xbox app can switch between them through its own UI; whether a second account added to Windows switches without a password or PIN is untested | The Xbox app, perhaps; the games yes | [MS] for the two-account model; the rest [reported] / untested |
| GOG Galaxy | Yes, by swapping a snapshot | Remembers one account. `HKCU\Software\GOG.com\Galaxy` `refreshToken`, `settings\userId`, `settings\username`, `%LocalAppData%\GOG.com\Galaxy\Configuration\config.json`, and much of `%ProgramData%\GOG.com\Galaxy` (its `galaxy-2.0.db` is the library) | Yes, killed | [code: TcNo]; single source |
| EA app | Yes, by swapping a snapshot, heavy | Remembers one account. The whole of `%LocalAppData%\Electronic Arts\EA Desktop\` and `%ProgramData%\EA Desktop\` | Yes, killed | [code: TcNo]; single source |
| Ubisoft Connect | Yes, by swapping a snapshot | Remembers one account. `%LocalAppData%\Ubisoft Game Launcher\` `ConnectSecureStorage.dat`, `user.dat`, `user.vdat`, `settings.yaml` | Yes, killed | [code: TcNo]; single source |

**Xodus Nexus** (the user's "Xodeus Nexus") is **Nexus Launcher**, formerly Xodus Nexus: an unofficial, MIT-licensed launcher that runs Microsoft Store and PC Game Pass games **on Linux**, on top of the GPL-3.0 **Xodus** project, which reimplements Microsoft account sign-in by **impersonating Windows' own sign-in host** and a Windows device. It does keep several Microsoft accounts and switch between them without a password, but only because it *is* its own Microsoft Store client holding its own refresh tokens in the Linux keyring. **It does not inject credentials into the Windows Xbox app, and nothing in it can.** It is worth reading to learn how Microsoft's sign-in works; it is not a model for the Dashboard (section 2.3).

**The user's middleman idea** (a helper that supplies Xbox credentials at run time): **do not build it.** Microsoft gives no API that signs an account into Windows or the Xbox app without its own UI; a password typed by a program is defeated by the second factor, by passkeys, and by accounts with no password at all (new Microsoft accounts are passwordless by default since May 2025); the only way around that is what Xodus does, impersonating Windows to Microsoft's servers, which is fragile, against the Microsoft Services Agreement's spirit if not letter, and puts every Profile's Microsoft account at risk (section 3.5).

**Recommended**, in five lines (updated with the decisions above):

1. A Profile is bound, per launcher, to **one account the client already remembers**, picked from a list the Dashboard reads from the client's own files (Steam's `loginusers.vdf`, Battle.net's `SavedAccountNames`, a snapshot taken of a one-account launcher's session: Epic, GOG, EA, Ubisoft, if decision 3 allows it); never typed into the Dashboard.
2. At Signing you in and at Switch profile, the Dashboard switches only the launchers whose account differs: closes each cleanly, writes the client's own remembered-account files, starts it again silent, one at a time, Steam first, with "Signing you in" held until each says it is signed in (or 60 s). The Dashboard does this file work itself (section 8), using TcNo's definitions as its reference.
3. A game running stops the switch for that launcher; a download under way is waited for, or cancelled if the user chooses; the Profile still changes in the Dashboard.
4. **Play time and achievements follow the Profile**: the Dashboard records them per Profile; what a store reports for an account is shown for the Profile bound to it (5.5). Owned games follow the account, since the licence is the account's.
5. **Xbox is not switched by the Dashboard.** It is told which Microsoft account the Profile uses and, when the Xbox app shows another, offers to open the Xbox app's own account page. No Windows user per person (decision 1).

## 1. Each client

What the Dashboard already does with each client, and so what a switch can break, is in [../sources.md](../sources.md) and [installs.md](installs.md). The parts that matter here:

- **Steam**: the library, play time and achievements are read from the **most recently written** `userdata\<account id>\config\localconfig.vdf` (`source_steam::signed_in_account`), and achievements from `appcache\stats\UserGameStats_<account id>_<appid>.bin` [code: this repository, `crates/source-steam/src/lib.rs`, `achievements.rs`]. Installs go through Steam's CEF debugger (port 8080 or 8081) where the user has turned it on, which Steam reopens at each start while `.cef-enable-remote-debugging` is in its folder [code: `crates/source-steam/src/cef.rs`].
- **Epic**: the library comes from the **most recently written** `cache\prod\account\<account>\library-cache.json` and from `catcache.bin` [code: `crates/source-epic/src/modern.rs`]. legendary, where the user added it, has its **own** sign-in in its own `user.json` [code: `crates/source-epic/src/legendary/mod.rs`].
- **Battle.net**: installs from the Agent's local HTTP API; installed games from `product.db`, which is the machine's, not an account's [installs.md 7, stores.md].
- **Xbox**: installed packages only, from the machine; nothing of an account's is read [sources.md].
- **The database**: `games.playtime_minutes`, `games.last_played`, `achievements.unlocked` and `play_sessions` are one per game, with **no account or Profile column** [code: `crates/library/src/lib.rs`, `achievements.rs`].

### 1.1 Steam

**Where the accounts are.**

- `<Steam>\config\loginusers.vdf`: one block per account Steam remembers, keyed by SteamID64, with `AccountName`, `PersonaName`, `Timestamp`, `WantsOfflineMode`, `MostRecent`, `AutoLogin`, `RememberPassword`, `SkipOfflineModeWarning`. TcNo reads `AutoLogin` as "current Steam's" marker of the active session and `MostRecent` as the older one [code: TcNo [`loginusers.go`][tc-lu] `ActiveSessionSteamID64`, [`vdf_write.go`][tc-vw]]. The ticket names the key `AllowAutoLogin`; TcNo's code reads and writes `AutoLogin`. Which one today's Steam writes is a TV PC look (S1).
- `HKCU\Software\Valve\Steam`: `AutoLoginUser` (the account **name**, not the id) and `RememberPassword` (DWORD) [code: TcNo [`autologin_windows.go`][tc-al]]. TcNo's comment: the account name "is the one thing Steam cannot sign in without, because AutoLoginUser is it" [code: [`known_accounts.go`][tc-ka]].
- **The session token itself** is not in either. Steam stores it encrypted, and "the way to decrypt it hasn't been published by anyone" (Dr. McKay, author of the node `steam-user` library, 2023) [reported: [dev.doctormckay.com][mckay]]. Where (a `ConnectCache` in a `config` or `local.vdf` file is the usual guess) and whether it is bound to the machine or the Windows user is **not known** [UNVERIFIED]. It does not matter for the design: nothing copies it; Steam keeps one per remembered account itself.

**Switching without a password: yes.** TcNo's Steam switch, in order [code: TcNo [`switcher.go`][tc-sw] `SwapToAccount`, `writeLoginUsersAndAutoLogin`]:

1. If the target already holds the session markers, do nothing.
2. Close Steam: `steam.exe -shutdown` ("Steam treats WM_CLOSE on its main window as minimise-to-tray ... The shutdown argument drains steam.exe and its steamwebhelper children") [code: [`native_quit_windows.go`][tc-nq]], then kill what is left: `steam.exe`, `steamwebhelper.exe`, `GameOverlayUI.exe` and the `Steam Client Service`.
3. In `loginusers.vdf`, for every row `MostRecent`, `AutoLogin`, `RememberPassword` = 0; for the target, all three = 1 (and `WantsOfflineMode`, `SkipOfflineModeWarning` to the offline choice). The old file is copied to `loginusers.vdf_last` first.
4. `AutoLoginUser` = the target's `AccountName`; `RememberPassword` = 1.
5. Optionally `AlwaysShowUserChooser` in `config\config.vdf`, and the persona state in the target's `localconfig.vdf`.
6. Start `steam.exe` with the user's arguments.

TcNo's own FAQ: "Account switching swaps local login state and asks for no password at all", and "Do not use the launcher's sign-out button" [docs: [TcNo FAQ][tc-faq]]: signing out from Steam drops that account's remembered session, and it then needs its password and Steam Guard again [reported: [Steam Community, 2023][steam-forum]; [TcNo wiki][tc-faq]].

Steam's own **Change Account** does the same from inside the client: "you don't have to provide anything when changing accounts; you just click the target account's avatar" [reported: Steam Community, 2023, as quoted by search], and a Steam Deck beta's change account dialog "shows a warning that proceeding will close active games" [reported: [SteamDeckHQ][deck-beta]].

**What it costs.** A full close and start of Steam. No measurement was found; my guess is 10 to 30 s on the TV PC to a signed-in, windowless Steam [inferred; T1 measures it]. What breaks:

- **Downloads** under way stop with Steam and resume when it next runs, for whichever account owns the game [inferred]. Another account's game installed on the PC stays installed and updates under any account that owns it [inferred].
- **Cloud saves**: a game closed by Steam's shutdown syncs on exit like any close; a game killed does not [inferred]. Hence: never switch while a game runs (section 5).
- **The Dashboard's Steam reads**: `signed_in_account` already follows the newest `localconfig.vdf`, which Steam writes at sign-in, so it follows a switch after Steam has signed in, not before [inferred]. Better to read the bound account's folder by id (section 5.5).
- **The CEF debugger**: dropped with Steam, reopened at Steam's next start while the file is there [code: this repository]. The Dashboard must reconnect, and fall back to `steam.exe` routes until it can.
- **Family sharing**: a game lent through Steam Families plays under the borrower's own account; switching accounts is not needed for it [inferred].

**Bound to the machine?** The remembered session is used only by the Steam on this PC; nothing is moved. TcNo backs up `config` and `userdata` but never copies a token between machines [code]. Whether a copied `loginusers.vdf` works on another PC was not looked at and does not matter here.

### 1.2 Epic Games Launcher

**The current launcher** (`com.epicgames.eglv2`, the one on the TV PC per stores.md) remembers **one** account at a time. TcNo's definition for it [code: TcNo [`Platforms.json`][tc-pj] "Epic Games", [`epic_new_launcher.go`][tc-epic]]:

- The session is **two things together**: a Windows Credential Manager generic credential with target `LegacyGeneric:target=credentials.com.epicgames.eglv2.prod`, and every file in `%LocalAppData%\com.epicgames.eglv2\auth` (it requires `auth.json`; the account id is read from `access-token-prod.dpapi`, which TcNo decrypts with DPAPI: `UniqueIdMethod: DPAPI_JSON_SELECT`). The display name is `auth.json`'s `prod.account_info.display_name`.
- **To save an account**: read the credential (`CredRead`) and the `auth` files, and write them as one snapshot, itself DPAPI-protected (`writeProtectedSession` → `DPAPIProtect`), keeping 3 recovery copies.
- **To switch**: kill `EpicGamesLauncher.exe` and `EpicWebHelper.exe` (`ClosingMethod: TaskKill`, `ExitBeforeInteract`), write the credential back (`CredWrite`) and the `auth` files, and clear `%LocalAppData%\com.epicgames.eglv2\EBWebView` (the launcher's WebView2 profile).
- TcNo validates the credential's size (at most 2560 bytes) and persistence (1 to 3) before writing it, and refuses links in the path [code].

The files and the credential are DPAPI-protected for this Windows user on this machine ([MS: `CryptProtectData`][dpapi]: "only a user with the same logon credential as the user who encrypted the data can decrypt the data. In addition, the encryption and decryption usually must be done on the same computer"). So a snapshot works only where it was taken, which is what is wanted.

**The older launcher** keeps its session in `%LocalAppData%\EpicGamesLauncher\Saved\Config\WindowsEditor\GameUserSettings.ini` `[RememberMe] Data` and the account id in `HKCU\Software\Epic Games\Unreal Engine\Identifiers` `AccountId` [code: TcNo "Epic Games (Legacy)"]. `Data` is base64 of a JSON array whose first item has a `Token` (a refresh token), AES-encrypted with keys legendary knows [code: [legendary `core.py` `auth_import`][lg-auth]]. That is why `legendary auth --import` works only with the old launcher (installs.md P3 asks whether it works with the new one: by this, it cannot).

**What it costs**: the launcher killed and started again; no figure found [inferred: 10 to 20 s]. What breaks:

- **Downloads** in the launcher stop; it resumes them [inferred].
- **Epic Online Services** in a running game: the game is signed in through the launcher's session; killing the launcher mid-game may sign the game out of its online features [inferred]. Never switch while an Epic game runs.
- **The Dashboard's Epic reads**: `library-cache.json` is per account folder already; the newest is read. After a switch, the new account's folder becomes newest only once the launcher has written it [inferred]. `catcache.bin` is one file for whoever signed in last [stores.md].
- **legendary is separate.** Its own sign-in (`user.json` under `LEGENDARY_CONFIG_PATH`) is not touched by the launcher's switch. Per Profile, the Dashboard can give legendary **its own folder per account** (`LEGENDARY_CONFIG_PATH=<data>\legendary\<account id>`), each signed in once through legendary's own page; then switching legendary costs nothing [inferred from [legendary `lgndry.py`][lg-cfg]]. Its `installed.json` would then also be per account, which is wrong for a machine's installs: keep one installs folder and point only the sign-in at the account, or move `installed.json` between them [inferred; a ticket of its own].

### 1.3 Battle.net

TcNo's definition [code: TcNo [`Platforms.json`][tc-pj] "BattleNet"]:

- The remembered accounts are `%AppData%\Battle.net\Battle.net.config`'s `Client.SavedAccountNames`, a comma-separated list of emails. The **first** is the one the client signs in as (`JSON_SELECT_FIRST,`).
- **To switch**: kill `Battle.net` (`ClosingMethod: TaskKill`), put the target's email first, start the client. When adding a new account, TcNo empties `SavedAccountNames` and deletes `%LocalAppData%\Battle.net\BrowserCaches\common\Network\Cookies*`.
- The BattleTag for the list is read from `%LocalAppData%\Battle.net\CachedData.db` (SQLite: `SELECT battle_tag FROM login_cache WHERE account_id_lo = ...`), the account id from the newest `Logs\battle.net-*.log`.

TcNo's FAQ: Battle.net "can detect saved accounts, but each account still needs the launcher's remember-password option enabled" [docs: [TcNo FAQ][tc-faq]]. So the client keeps a token per remembered email somewhere TcNo does not touch [inferred]; where (a file under `%LocalAppData%\Battle.net`, or Credential Manager) is **unknown** [UNVERIFIED; B1]. Blizzard's own forums say an update once stopped remembering emails [reported: [Blizzard forums][bn-regress]], which is the kind of change that would break this silently.

**What it costs**: the client killed and started again [inferred: 10 to 20 s]. **The Agent** (`Agent.exe`, the downloader the Dashboard talks to) is a separate process. TcNo does not kill it; whether a download through the Agent survives the client's restart, and whether the Agent follows the new account, is **unknown** [B2]. Installed games are the machine's (`product.db`); what an account may launch is checked by the client at launch [inferred].

### 1.4 Xbox app, Microsoft Store and Gaming Services

Section 3 has this in depth. In short: two Microsoft accounts are in play, the Store account (purchases and Game Pass) and the Xbox profile (identity) [MS: [mismatched store accounts][ms-mismatch]]. Both are Web Account Manager accounts, held by Windows' token broker for the Windows user, not in files. **There is no file to swap.**

### 1.5 GOG Galaxy

Decision 5 brings GOG, EA and Ubisoft in. All three remember **one** account, as Epic does, so each is switched by a snapshot of its session, and each raises decision 3's question (7.6). The only public account of what to swap is TcNo's, read again at [`6f1ad81`][tc] [code: TcNo [`Platforms.json`][tc-pj] "GOG Galaxy", "EA Desktop", "Ubisoft"]; no store documents any of it. What the Dashboard reads today is in [../sources.md](../sources.md) "The stores".

**Where the account is.** TcNo's login files for GOG Galaxy:

- Registry: `HKCU\Software\GOG.com\Galaxy` value `refreshToken` (**the session**: an OAuth refresh token), `...\Galaxy\settings` values `userId` (TcNo's unique id: `UniqueIdMethod: REGKEY`) and `username`.
- `%LocalAppData%\GOG.com\Galaxy\Configuration\config.json`.
- Under `%ProgramData%\GOG.com\Galaxy`: `config.json`, `storage\galaxy-2.0.db*` and `storage\etags.db`, and the client's web view state under `webcache\common`: `Cookies`, `Local Storage\leveldb`, `Session Storage\*.ldb` and `*.log`, and cache indexes.
- Closed first (`ExitBeforeInteract`): `GalaxyClient.exe`, `GalaxyClientService.exe`, `GalaxyCommunication.exe`, `GOG Galaxy Notifications Renderer.exe`. `PathListToClear: SAME_AS_LOGIN_FILES`: to add a new account TcNo deletes all of the above, which signs Galaxy out.

**How it is kept.** TcNo dumps registry values into a `reg.json` in its snapshot folder as they are, strings kept as strings [code: [`internal/basic/flow_registry.go`][tc-reg]]: the `refreshToken` is copied in the clear unless the user turned on TcNo's own app password (8.2). Whether Galaxy itself encrypts the registry value was not looked at; read as plain text it would be a bearer token for the account [UNVERIFIED; G1].

**What a switch breaks.**

- **The library**: `galaxy-2.0.db` is where `source-gog` reads owned-not-installed games, play time and last played ([../sources.md](../sources.md)). Swapping it swaps all of that with the account, which is what decision 4 wants for owned games and **not** for play time (5.5): the Dashboard must keep its own per-Profile record and not overwrite it from Galaxy's database after each switch.
- **`%ProgramData%`** is the machine's, shared by every Windows user [inferred]. With one Windows user (decision 1) that does not matter; it does mean a snapshot writes outside the user's own folders, which needs write access there (Galaxy's installer grants it, or the write fails) [UNVERIFIED; G1].
- **Downloads**: Galaxy's own downloads stop with it and resume at its next start [inferred]. GOG games start as their own programs with no client ([../sources.md](../sources.md)), so a GOG game running does not stop a switch the way a Steam game does; Galaxy's overlay and cloud saves sync at the game's exit through Galaxy, so still never switch while a GOG game runs [inferred].
- **gogdl**, where the user added it, has its own sign-in file per `--auth-config-path` [installs.md]; like legendary (A11), it can be given a folder per bound account.

**What it costs**: four processes killed and Galaxy started again [inferred: 15 to 30 s; G1 times it]. Galaxy is known to sign itself back in on start with no prompt once remembered [reported (excerpt): [GOG forum][gog-forum]].

**How the user revokes a copy**: GOG added "one-click log out of all active account sessions across all devices" with two-step login in 2016 [reported: [TweakTown, 2016][gog-2fa]]; where it is on gog.com today was not found [UNVERIFIED]. Changing the password is the fallback [inferred].

### 1.6 EA app

**Where the account is.** TcNo's login files are **two whole folders**: `%LocalAppData%\Electronic Arts\EA Desktop\` and `%ProgramData%\EA Desktop\` (log files left out: `BackupFileTypesIgnore: .log`), with `cookie.ini` required (`RequiredLoginFiles`). The unique id is read from `telemetry.ini` (`=([^\r\n]*)`), the user id from `user_*.ini`, the display name and picture out of the client's CEF web cache (`CEF\*\EADesktop\Default\Cache\Cache_Data\data_*`). Closed first, and before saving too (`ExitBeforeSave`): `EADesktop.exe`, `EABackgroundService.exe`. TcNo also has a separate **Origin** definition (whole `%AppData%\Origin` caches and `%ProgramData%\Origin` files); Origin is retired and not a target here.

**What is in it.** The folder holds settings, caches and the session together, and which file is the session beyond `cookie.ini` is not known [UNVERIFIED]. So a snapshot is the whole folder, and putting one back also puts back the settings and caches of the time it was taken: settings changed since are lost, which is why this is "heavy and brittle" [inferred].

**EA's own behaviour.** EA's help says to switch by "Sign out" from the menu, after which the app asks for another account [docs (excerpt): [help.ea.com, How to use the EA app][ea-help]]; no remembered-accounts list. Players report the EA app signing them out after closing [reported (excerpt): [EA forums][ea-forum-keep]], and that an EA account signs in on one PC at a time ("EA is the only one that doesn't" let them stay signed in on two) [reported (excerpt): [EA forums][ea-forum-two]]. A game can also bring its own linked account back: The Sims 4 "immediately logs you into the different EA account you use for that Sims 4 game" after a switch [reported (excerpt): [EA forums][ea-forum-sims]]. Each of those would show as "EA needs you to sign in" after a switch (5.2 step 3).

**What a switch breaks.** `source-ea` reads installed games from the registry and each game's `__Installer` folder ([../sources.md](../sources.md)), not from these folders, so the library is unaffected [code: this repository]. EA's downloads stop and are its to resume [inferred]. EA games start their own program, which brings the EA app up itself when it needs it ([../sources.md](../sources.md)); a game started that way during a switch would start the app as whoever's snapshot is there at that moment, hence one launcher at a time and never during a game [inferred].

**What it costs**: two processes killed, two folders written (size not known: an EA folder's caches can be hundreds of MB [UNVERIFIED; EA1]), the app started again.

### 1.7 Ubisoft Connect

**Where the account is.** TcNo's login files: `%LocalAppData%\Ubisoft Game Launcher\` `ConnectSecureStorage.dat`, `user.dat`, `user.vdat`, `settings.yaml`, and the client's `logs\launcher_log.txt`, from which the account's id (a GUID after `User:`) is read. Display name from the client's HTTP cache (`nameOnPlatform` matched by `userId`); picture from `avatars.ubisoft.com/<id>/default_256_256.png`. Closed first: `upc.exe`, `UplayService.exe`, `UplayWebCore.exe`. Before a switch TcNo sets `settings.yaml` `user.closebehavior` to `CloseBehavior_Close`, so closing the window quits rather than going to the tray.

**What is in it.** `ConnectSecureStorage.dat` reads as the session by its name; whether it is DPAPI-bound was not looked at [UNVERIFIED; U1]. Ubisoft offers no remembered-accounts list; players report Connect asking for the password again despite "Remember me" until the PC is made a **trusted device**, which needs two-step verification [reported (excerpt): [Steam Community][ubi-remember]]. So an account with 2FA may need the PC trusted before a snapshot of it survives a restart [inferred; U1].

**What a switch breaks.** `source-ubisoft` reads installed games from the registry and owned games from `cache\configuration\configurations`, which is in the same folder but not in TcNo's list [code: this repository; TcNo]: after a switch the cache is rewritten by the client for the new account, so the owned list follows the account once the client has run [inferred]. Ubisoft games bought on Steam start Connect themselves and sign in with whatever it holds [inferred].

**What it costs**: three processes killed, five files written, the client started again [inferred: 10 to 20 s].

## 2. How existing tools do it

### 2.1 TcNo Account Switcher

The best witness: open source (**GPL-3.0**), in active development, a definition per platform in one JSON file. Read at [`6f1ad81`][tc] (master, 16 Nov 2026; `Platforms.json` version 4.0.9, now in Go with Wails). The newest published release is **v4.0.9**, published 16 Nov 2026 half an hour after that commit [code: GitHub API, read 18 Nov 2026] (the first version of this document said `2025-11-20_03`: wrong, an older listing), so what users run is what was read. ("Platforms.json version 4.0.9" above and the release are the same version.)

- **Model**: for every platform but Steam, a "basic" flow driven by `Platforms.json`: `ExesToEnd`, `LoginFiles` (files, registry values `REG:...`, or a JSON path `JSON_SELECT...`), `PathListToClear`, `UniqueIdFile`/`UniqueIdMethod`, `ClosingMethod`. **Save current** copies the login files into TcNo's own folder for that account; **switch** closes the client, writes them back, starts the client. Steam has its own code (section 1.1), as does Epic's credential session (1.2).
- **No passwords** are kept for switching. Its optional Steam Guard feature does take the Steam password, which "is encrypted with a key fetched from Steam, sent to Steam, and never written to disk", and keeps Steam Guard secrets in a vault behind a password of the user's own (Argon2id) [docs: [TcNo FAQ][tc-faq]; [`docs/steam-guard-passwords-and-factors.md`][tc-sg]].
- **No Xbox or Microsoft Store** platform at all; the nearest is "Spotify (Windows Store)", a file swap in a Store app's `LocalState` [code].
- **A CLI**: `+s:<steamId64>` swaps Steam, `+<platformShort>:<uniqueId>` any other (`+b:`, `+eg:`...), `--list-accounts --json` lists them [code: [`internal/cli/help.go`][tc-cli]]. So the Dashboard **could** drive TcNo as a helper the user adds, as it drives legendary, instead of doing the file work itself. Against: TcNo is a whole desktop app with its own window, update checker and Discord presence; its saved accounts live in its own folder under its own lock [inferred]. Section 8 scopes this route in full.
- **Trust**: the code is careful (atomic writes, `.vdf_last` backups, recovery copies, link checks, logs that never record arguments "they can contain passwords or tokens"). It is a fair reference for *what* to swap [inferred]. Its licence means **read it, do not copy it** into the Dashboard (whose own licence is not decided, plan.md open question 5); section 8.2 has what the licence allows for running or shipping it.

### 2.2 Others

- **accshift** (SourceForge): "captures each launcher's own remember-me session and restores the right one on switch" [reported: search excerpt only].
- **steam-acolyte** (PyPI): Steam only; "only stores login tokens, but never any passwords" [reported].
- Steam's own **Change Account**, Battle.net's **Allow multiple instances** (a second client window signed in to another account [reported: Blizzard forums]); Epic and the Xbox app have nothing like it beyond signing out and in.

None of these does Xbox.

### 2.3 Xodus Nexus

Searched as "Xodeus Nexus" (no results), then "Xodus Nexus", "Xodus" and "Nexus Launcher". **It is Nexus Launcher, called Xodus Nexus until 0.5.0** (9 Nov 2026) [docs: [GitHub releases][nx-rel]; [xodusnexus.com][nx-site], a fan guide that hosts no files].

- **What**: "Your Microsoft Store PC games, on Linux." An Electron launcher (TypeScript) with an Xbox-like look, **MIT**, by one developer (`domsvoboda/nexus-launcher`, 1.3k stars, created 11 Oct 2026, read at [`c239c3f`][nx], 17 Nov 2026). **Linux only**; Steam Deck experimental [docs: [README][nx]].
- **How**: three parts. A **backend**, "our patched build of Xodus, handling Microsoft account sign-in, genuine license checks, package downloads and launch preparation"; a **runtime** (forks of Proton, Wine and an open `xgameruntime.dll`); and the launcher [docs: README]. It says itself: "an independent, unofficial project built on Xodus. It is not an official project or release from the upstream Xodus developers."
- **Xodus** (`xodus-gaming/xodus`, **GPL-3.0**, Rust, read at [`49453a4`][xo], 8 Nov 2026): "The project can now login, download packages and obtain licenses for games." Its sign-in [docs: [`docs/xodus/login.md`][xo-login], [`device.md`][xo-dev]]:
  - It opens Microsoft's `InlineLogin.srf`, "the same hosted login page Windows loads inside `CloudExperienceHost` (the native shell that embeds sign-in webviews for TokenBroker)", in its own web view, **presenting as `CloudExperienceHost`**: it sends the `cxh-*` headers a real one would, and "re-implements just enough of it ... for the page to behave as if it were signing in through Windows".
  - It **registers a fake Windows device** with `deviceaddcredential.srf`, sending device components (MAC address, disk serials, board and SMBIOS details) and obfuscated "CLEP" blobs, and recovers the device's RSA key from the server's reply, then trades tokens at `RST2.srf` for Xbox Live tokens.
  - Tokens are kept in the OS keyring (`keyring_core`) [code: [`tokens/backend/keychain.rs`][xo-kc]].
- **Accounts**: since 0.5.0, optional **saved accounts**: "Makes a saved account the active Store account, no sign-in and no device reset" (`saved-accounts --use <name>`), and a separate **player** account from the **licence** account [code: [`src/main/saved-accounts.ts`][nx-sa]; docs: [xodusnexus.com][nx-site]: "manage several Store accounts and a separate Xbox player account"]. "Your credentials are stored in your system keyring" (KWallet or GNOME Keyring) [docs: site]. "Xodus owns sign-in credentials; the launcher should not copy them" [docs: [`SECURITY.md`][nx]].
- **Xbox on Windows**: nothing. It never touches Windows' Xbox app, Store or Web Account Manager; it replaces them.
- **Trustworthy to learn from?** As documentation of Microsoft's sign-in protocol: yes, it is the most detailed public account, and it says plainly which parts are reverse-engineered [inferred]. As something to build on: **no**. It impersonates a Windows component and a Windows device to Microsoft's servers, which Microsoft can detect and stop at any time; its README warns of the Microsoft Store's 10-device limit ("licensing code 501"), which a fake device counts against; xodusnexus.com warns "Microsoft does not support or endorse third-party launchers"; and on Windows the real Xbox app, Gaming Services and the games' own `XUser` calls would not use its tokens anyway [inferred]. The idea worth taking from it is only the split it makes visible: **the player and the licence holder are two accounts**, which Windows has too (section 3.1).

## 3. Xbox in depth

### 3.1 Who decides the signed-in account

- **Two accounts.** "The `XStoreContext` on PC titles in production is linked to the Microsoft Account (MSA) that is currently signed into the Microsoft Store app ... This account can sign out or switch with another account at any time without notice. Additionally, the Microsoft Store account can be switched from the Xbox app based on user settings." And: "the Xbox services account can be switched with the sign-in flow in-game to create an account mis-match between the Xbox and Microsoft Store apps. The mismatched account status can be intentional for family or shared PC scenarios." Microsoft's own figure: "10-15% of Xbox services users have a different MSA signed into the Microsoft Store app than Xbox services in a Windows session" [MS: [Handling mismatched store account scenarios on PC][ms-mismatch]].
- **What each does**: the Store account holds what is owned and the Game Pass membership; the Xbox profile is the gamertag, achievements, friends and cloud saves [docs (excerpt): [support.xbox.com, "How your PC gaming account relates to your Microsoft Store account"][xb-store]]. In the Xbox app: gamerpic > Settings > Account > "Change my Microsoft Store account" [docs (excerpt): same]. "You can only sign in to the Xbox app for Windows PC with one Microsoft account at a time, but you can switch between accounts" [docs (excerpt): [support.xbox.com, "Switch between Microsoft accounts for gaming on your Windows device"][xb-switch]]. Both pages are rendered by script and could not be read whole.
- **Installed games and licences**: if the PC plays games bought by another account, "their account must be set as the Store account. Otherwise, games already installed using that account won't launch because they weren't purchased using your Microsoft account" [docs (excerpt): [xb-store]]. So **switching the Store account changes which installed games start**, and a Game Pass game starts only while a Store account with Game Pass is signed in [inferred from the same].
- **In a game**: a GDK game asks Windows for its player with `XUserAddAsync`; `AddDefaultUserSilently` returns "the user who launched the game", and with more than one user signed in and no default, it fails and the game must show Microsoft's account picker [MS: [`XUserAddAsync`][ms-xuser], [`XUserAddOptions`][ms-xuseropt]].
- **Underneath**: Windows' **Web Account Manager** (WAM) holds the Microsoft accounts added to the Windows user ("Accounts used by other apps" in Settings > Accounts > Email & accounts); an app finds them with `WebAuthenticationCoreManager.FindAllAccountsAsync` and asks for a token silently, showing `AccountsSettingsPane` only when it must [MS: [Web Account Manager][ms-wam], [`FindAllAccountsAsync`][ms-findall]]. The **Xbox Live Auth Manager** service (`XblAuthManager`) "provides authentication and authorization services for interacting with Xbox Live" [MS: [services list][ms-svc]]; its COM interface `IXblIdpAuthManager` is documented only as "Reserved for Microsoft use" [MS: [xblidpauthmanager.h][ms-xblidp]].

### 3.2 Can two Microsoft accounts be remembered and switched without a password?

- **Remembered: yes.** WAM keeps several accounts per Windows user [MS: [ms-wam]], and Microsoft's own GDK page treats a different Store and Xbox account on one Windows session as normal [MS: [ms-mismatch]].
- **Switched without the password or PIN: unknown.** No Microsoft page says what the Xbox app's account switch asks for when the target is already in WAM. Community answers only describe signing out, "Use a different account", and removing and re-adding the account in Email & accounts [reported: Microsoft Q&A threads]. One 2021 Steam forum post says changing "allow apps to sign me in" under that list made a game ask for email and password instead [reported, single]. My guess: an account already added to Windows for apps switches with a click, as WAM's silent token request is meant to allow; one that is not asks for its full sign-in [inferred]. **X1 settles it.**
- **What a switch does**: the Xbox profile change signs the old gamertag out of the Xbox app and Game Bar; a running game keeps its `XUser` until it asks again [inferred]. A Store account change changes which installed games launch, at once (3.1).

### 3.3 The Xbox app's own switcher, driven by UI Automation

The Xbox app is a packaged app whose controls UI Automation can see [inferred]. The Dashboard could open the Xbox app, walk to gamerpic > Switch account (or Settings > Change my Microsoft Store account), and pick the Profile's account by name. **Fragile**: the app's layout changes with its updates (monthly or faster), names are localised, and any prompt (password, PIN, consent, "keep me signed in") stops it [inferred]. It also shows the Xbox app's window over the Dashboard, which shell mode exists to avoid. **Do not build it** as the main route; at most, "Open the Xbox app's account page" as a button, leaving the clicks to the user (section 5).

### 3.4 One Windows user per Profile

The only route that gives each Profile its own Xbox app, Store, WAM and every launcher's session with nothing swapped: each family member has their own Windows user, and choosing a Profile is a Windows **fast user switch** [inferred].

- **Shell mode is per user**: the Dashboard's shell is `HKCU\...\Winlogon\Shell` [docs: this repository, [../spec/shell-mode.md](../spec/shell-mode.md)], so each Windows user needs it installed and set separately, and each has its own Dashboard, database, Profiles and logs.
- **A switch** goes through Windows' lock screen and sign-in (password, PIN, Windows Hello) for the target user, and the first user's session, Dashboard, launchers and any game keep running behind, holding memory and the GPU's attention [inferred]. Signing in automatically without Windows' prompt needs the target's Windows password stored for auto-logon (`AutoAdminLogon`), the very thing to avoid.
- **Launchers**: Steam, Epic, Battle.net and the rest are installed once for the machine but run and sign in per user; whether two users' Steams can run at once (one per session) is **not known** [UNVERIFIED]. Store packages are installed per user, so each Windows user installs (registers) an Xbox game again, reportedly without a second download [reported, not checked].
- **For a TV**: a Windows lock screen between "Who's playing today?" and Home is not a console. **Not recommended** as the default [inferred]. **Decided against** on 18 November 2026 (decision 1, as read): not offered, not mentioned in Settings.

### 3.5 The middleman executable that supplies credentials

The user's idea, assessed squarely.

**What it would have to store.** Either the Microsoft account **password** (and something for the second factor), or a **refresh token**. There is no third thing.

**Where.** At best Windows Credential Manager or a DPAPI blob, both protected by the Windows user's own key: "only a user with the same logon credential ... usually ... on the same computer" [MS: [dpapi]]. That protects it from other users and other PCs, **not from any program running as this user**, malware included, which can call `CredRead` or `CryptUnprotectData` as TcNo does [code: TcNo]. Since the Dashboard runs as the one Windows user every family member uses, every Profile's stored secret is readable by every other Profile's games and programs [inferred].

**How it would hand them over.** This is where it fails:

- **No API signs a Microsoft account into Windows, the Xbox app or the Store from a program.** WAM's account add goes through `AccountsSettingsPane` or `RequestTokenAsync` with UI; Microsoft's guidance is that users "should not type in credentials in windows that pop up within the operating system with no attached context" [MS: [MSAL.NET with WAM][ms-msal-wam]]. `IXblIdpAuthManager` is "Reserved for Microsoft use" [MS: [ms-xblidp]]. So the helper would have to **type into Microsoft's sign-in window** (UI Automation or `SendInput`) [inferred].
- **Second factors** stop that: an account with two-step verification asks for an Authenticator approval, a code or a security key after the password [inferred].
- **Passwordless accounts have no password to type.** "Brand new Microsoft accounts will now be "passwordless by default."" and "Existing users can visit their account settings to delete their password." [MS: [Microsoft Security blog, 1 May 2025][ms-passkey]]. A passkey or Windows Hello cannot be replayed by a program by design [inferred].
- **Storing a refresh token instead** means getting one, which means being a Microsoft sign-in client: Xodus's route of impersonating `CloudExperienceHost` and registering a fake device (2.3). Those tokens are then the helper's, not Windows'; the Xbox app, the Store and the games' `XUserAddAsync` would never see them [inferred].
- **Terms**: "You cannot transfer your Microsoft account credentials to another user or entity" and "To protect your account, keep your account details and password confidential" [docs: [Microsoft Services Agreement][msa], section 4.a.i, published 30 July 2026]. A program holding family members' passwords to type them is at least against the second [inferred].

**Verdict.** What is workable: nothing that injects credentials. What is fragile: driving the Xbox app's own switcher by UI Automation (3.3). What I should not build: **a credential store, a password typer, or a token client of our own for Microsoft.** The workable Xbox story is: the Profile knows which Microsoft account is theirs; the Dashboard says when the Xbox app's account is another and opens the Xbox app's own account page for them; and Windows' own "Accounts used by other apps" is where the second account lives, remembered by Windows, not by the Dashboard (X1 says how smooth that is).

## 4. What the Dashboard must never keep or do

- **Never ask for, keep or log a password, a Steam Guard code, a token or a cookie** of any store. Not in the database, not in settings, not in logs, not in a crash report. Log lines name the launcher, the account's public name and id, and what was done.
- **Never copy a session to another PC or another Windows user.** Only the clients' own remembered-session files that already live on this PC under this Windows user are touched. Most are DPAPI-bound anyway (1.2).
- **At most, swap the clients' own files back and forth.** Where a client remembers several accounts itself (Steam, Battle.net), only its own "which one" markers are changed. Where it remembers one (Epic, GOG, EA, Ubisoft), a snapshot of its own session files is kept **only if the user allows it (decision 3, open)**, under the limits of section 7.4: DPAPI-protected for this Windows user, in the Dashboard's data folder, as Discord's tokens already are (docs/discord.md), never in the database or a log, deleted on unbind, forgettable from Settings.
- **Back up before writing**, write atomically, and restore the backup if the client does not come up signed in (TcNo's `.vdf_last` and recovery copies) [code: TcNo].
- **Never sign anyone out** of a client; that drops the remembered session (1.1).
- **Never switch under a running game or a running download**: wait for it, or cancel the download if the user chooses (5.4).
- **Never drive a Microsoft sign-in**, type into any store's sign-in window, or impersonate a client to a store's servers.
- **Never use one account for another person.** The stores' terms, quoted:
  - **Steam**: "You may not reveal, share or otherwise allow others to use your password or Account except as otherwise specifically authorized by Valve" (section 1.C); and "You may not use Cheats, mods, hacks, or any other unauthorized third-party software ... in interacting with or controlling the processes or user interface of Steam Content and Services" (4.B) [docs: [Steam Subscriber Agreement][ssa], revision of 10 September 2026, read through a summarising fetch].
  - **Epic**: "You may not share your Epic Games Account credentials with others. You can only use your own Epic Games Account that is registered to you. You can't sell it, give it away, trade it, or use someone else's Epic Games Account, even if they let you" [docs: [Epic Games Terms of Service][epic-tos], last updated 20 October 2025, effective 28 November 2025].
  - **Blizzard**: "you cannot share the Account or the Login Information with anyone, unless the terms of this Agreement allow it" (1.A.iii.2); prohibited is "any code and/or software, not expressly authorized by Blizzard, that can be used in connection with the Platform" (1.C.ii.4) and "any connection using third-party programs or tools not expressly authorized by Blizzard" (1.C.ix) [docs: [Blizzard EULA][bn-eula], last revised 21 March 2024].
  - **Microsoft**: section 3.5 above.

  My reading [inferred]: each Profile using **its own person's** account on a shared PC is what these terms expect; the switcher only changes which remembered account a client starts as, which the client's own Change Account does too. Blizzard's 1.C.ii.4 is broad enough to cover any outside program that edits its files; the Dashboard already reads Battle.net's files and drives its Agent (installs.md), so this adds no new kind of risk, but it is one. Steam's 4.B is about controlling Steam's processes or UI; writing `loginusers.vdf` while Steam is closed does neither, but closing Steam arguably does. No report was found of an account banned for using TcNo's file swaps [inferred from the absence of reports in what was searched; weak].

## 5. Recommended design

### 5.1 Binding a Profile to accounts

- A Profile's page in Settings gains **Accounts**: a row per launcher installed (Steam, Epic, Battle.net, GOG, EA, Ubisoft, Xbox), each showing the bound account or "Any" (the default: nothing switched).
- **Choosing an account** lists those the client already remembers, read from its own files: Steam's `loginusers.vdf` rows (persona name and avatar from `userdata`), Battle.net's `SavedAccountNames` (BattleTag from `CachedData.db`), Epic's **saved snapshots** plus "the account signed in now". Nothing is typed. An account not in the list: "Sign in to it in <launcher> once, with Remember me, then come back."
- **Epic, GOG, EA, Ubisoft** (one remembered account each): if decision 3 allows copies, "Save this sign-in for <Profile>" takes the snapshot of the launcher's session as it is now (1.2, 1.5 to 1.7). To add a second account the user signs in to it in the launcher and saves again. If it does not, the row binds an account by name only and the switch is the launcher's own sign-in window (7.5).
- **Xbox**: the row records which Microsoft account the Profile uses, read from what the Xbox app is signed in as when the user picks "This one", by its gamertag; nothing is switched (5.3).
- Stored in the database: `profile_accounts (profile, source, account_id, display_name)`; for Epic, the snapshot file's name. No secret is in the database.

### 5.2 At "Who's playing today?" and at Switch profile

1. The Profile is signed in at once, as today (start-up.md 6). **Signing you in** stays up, its spinner turning, past its 3 s **while any launcher is being switched**, with one line under it: "Switching Steam to <persona>..." then "Switching Epic Games...".
2. For each launcher whose bound account is not "Any" and differs from the one signed in now, **one at a time**, Steam first (the Dashboard's main source), then Epic, Battle.net, the rest:
   - running and busy (a game of its own running, a download in the Queue): **skip it for now** as 5.4 says, and say so later in a Toast ("Steam stays on <old> while <game> runs");
   - close it cleanly (`steam.exe -shutdown`; the others by their window close, then killed after 10 s), wait for its processes to be gone;
   - back up and write its files (1.1 to 1.5);
   - start it silent (`steam.exe -silent`; the others minimised to tray where they can be);
   - wait for "signed in": Steam's `loginusers.vdf` target row with the new `Timestamp`, or the CEF debugger answering with that account; Epic's `auth` folder rewritten; Battle.net's newest log naming the account. **Limit 60 s** per launcher.
3. Home then shows. A launcher that did not come up signed in gets a Toast: "<launcher> needs you to sign in" and stays on its own sign-in window, as today when a session has expired; the backup is put back if the client failed to start at all.
4. **Same account already, or only one account remembered, or "Any"**: nothing is done; no delay.
5. **Waking from sleep** does nothing (start-up.md 8: it asks nothing and signs nobody in again; the user, 2026-11-23).

**How long**: 0 s when nothing differs; about 15 to 40 s per launcher that switches [inferred; T1 to T3 measure it]. The order and the "skip it" rule keep the worst case from blocking Home: after **30 s in total**, Home shows and the rest finish behind it, each with its Toast.

### 5.3 Xbox

Decision 1, **as read by the lead and to be confirmed with the user**: no Windows user per person; what follows is all the Dashboard does for Xbox.

- At Signing you in, if the Profile has an Xbox account and the Xbox app's signed-in gamertag (read how? see X2; else not read at all) is another, a Toast: "The Xbox app is signed in as <other>" with A = **Open the Xbox app's account page** (`ms-xbox://` or the Xbox app's Settings, X3), leaving the switch to the user and to Microsoft's UI.
- No Microsoft account is switched by the Dashboard, no credential is kept.
- Settings says once, plainly: "The Xbox app's account is switched in the Xbox app." The earlier line suggesting a Windows user per person is dropped with decision 1.

### 5.4 A game running, or a download under way

- **A game of that launcher running**: that launcher is not switched; the Profile changes in the Dashboard; the Toast says why; the switch is offered again when the game ends ("Switch Steam to <persona> now?").
- **A download in the Queue for that launcher** (decision 7): asked once, A = **Wait until it is done** (the default, focused), the other = **Cancel it and switch**: "Steam is downloading <game>. Switch to <persona> when it is done, or cancel the download and switch now?" Waiting: the Profile changes in the Dashboard at once, the launcher switches when its part of the Queue is empty, with a Toast then. Cancelling: the Dashboard cancels the download through the route that started it (the Queue's Cancel, built in M8), which may leave a partly downloaded game for the launcher to clean or resume later [inferred], then switches. A download the launcher started itself, not through the Queue, is not seen by the Dashboard; the launcher stops it at close and resumes it at its next start [inferred; T4 checks Steam's].
- **The Steam CEF connection** is dropped and remade after the restart; installs fall back to `steam.exe` routes until it answers (sources.md already does this when the debugger is off).

### 5.5 Owned games follow the account; play time and achievements follow the Profile

Decision 4: **play time and achievements follow the Profile, not the store account.** Owned games cannot: a licence is the store account's, so what a Profile can install and start is its bound account's.

**What the database keeps.** Today `games.playtime_minutes`, `games.last_played`, `achievements.unlocked` and `play_sessions` are one per game with no Profile column (section 1). They become:

- `play_sessions` gain the **Profile** that played (and the source account, for the record). The Dashboard already writes a session per launch it follows; that becomes the Profile's own history.
- **Per-Profile figures**: a `profile_games (profile, game, playtime_minutes, last_played)` table in place of the two columns on `games`, and `profile_achievements (profile, game, achievement, unlocked_at)` in place of `achievements.unlocked`. `games` keeps what is the game's (name, art, installed or not, the store's achievement list).
- **Owned-not-installed rows** gain the **account** they are owned by, so My games shows the signed-in Profile's bound accounts' games plus every installed game (installed games stay the machine's).

**Where the figures come from.** The stores report per account, the Dashboard per Profile:

| Store | Per-account figure the Dashboard reads today | With decision 4 |
|---|---|---|
| Steam | play time and last played in `userdata\<id>\config\localconfig.vdf`; achievements in `UserGameStats_<id>_<appid>.bin` | read for the bound account only |
| GOG | play time and last played in `galaxy-2.0.db` (swapped with the account, 1.5) | read for the bound account only, from the database Galaxy holds while that account is in |
| Epic, EA, Ubisoft, Battle.net, Xbox | none of play time or achievements on disk ([../sources.md](../sources.md)) | the Dashboard's own sessions only |

The rule [inferred; **a reading to confirm with the user**]:

1. **Play time of a Profile on a game** = the Dashboard's own sessions by that Profile, **plus**, once, at the moment a Profile is first bound to an account, the store's figure for that account then, as "played before the Dashboard". After that the store's figure is not read back into the Profile, because it would also count other Profiles' play on the same account, or "Any" sessions.
2. **Last played** = the latest of the Profile's own sessions and that seed.
3. **Achievements**: an achievement is unlocked **for the Profile** when a store reports it newly unlocked on the Profile's bound account while that Profile is signed in (the Dashboard compares the account's list before and after each session), plus, at first binding, every achievement the account already has. Two Profiles sharing an account, or a Profile on "Any", each keep only what was unlocked in their own sessions.
4. **"Any"** (nothing bound): the Profile still gets its own sessions; nothing is seeded, since no account is its.
5. **Unbinding** keeps the Profile's figures; **binding another account** seeds that account's figures once more for the games not yet seeded.

What this means for the stores that report per account: Steam's and GOG's own figures stop being the shown truth after the seed; a game played on Steam outside the Dashboard (Big Picture, a Steam shortcut) adds to the account's figure, not to any Profile's, unless the Dashboard is running and follows it [inferred]. That gap is the price of decision 4; a later ticket could attribute play seen on the account while a Profile was signed in to that Profile.

**After a switch** the sources re-read (as at start), and Home and My games show the Profile's own play time and its bound accounts' owned games; the greeting Toast waits for that, so "Hi Sam!" and Sam's Home come together.

- **Sources read the bound account, not "the newest"**: `source-steam` takes the account id to read (`userdata\<id>`, `UserGameStats_<id>_*`) from the signed-in Profile's binding, falling back to today's newest-`localconfig` rule for "Any"; `source-epic` reads `cache\prod\account\<bound>\library-cache.json`; GOG's database is whatever Galaxy holds (it is swapped with the account).

### 5.6 Tickets for M10, smallest first

Reshaped on 18 November 2026 for the decisions: GOG, EA and Ubisoft in (5), play time and achievements per Profile (4), the download rule (7), Discord out (6), Xbox naming only (1). The list is **route (a)**, the Dashboard's own file work, as recommended in section 8; section 8.3 says which tickets route (b), TcNo's command line, would replace.

M10 depends on nothing in **M9 Full Discord**. It depends on what is built: Profiles and Signing you in (M5: #125, #127), the Queue and installs (M8, for "a download under way"), Steam's CEF route (#132/#171), legendary (#175), gogdl (#176).

| # | Ticket | Done when | Stand-in or real |
|---|---|---|---|
| A1 | **Profile accounts in the database**: `profile_accounts (profile, source, account_id, display_name)`, the `ProfileChange` to bind and unbind, the log lines | A Profile can be bound to "Any" or an account per source; the binding survives a restart; tests on the library alone | Stand-in |
| A2 | **Play time and achievements per Profile** (decision 4): `play_sessions` with the Profile, `profile_games`, `profile_achievements`, the seed at first binding, the before-and-after compare of an account's achievements around a session (5.5) | Two Profiles playing the same made-up game show two play times; a seed is taken once; tests on the library alone | Stand-in |
| A3 | **Steam: list the remembered accounts** from `loginusers.vdf` (`AccountName`, `PersonaName`, `MostRecent`, `AutoLogin`, `RememberPassword`, `Timestamp`) and which is active | `source_steam::accounts(root)` returns them; tested on made-up files, including a broken one | Stand-in |
| A4 | **Settings > Profile > Accounts** page: a row per installed launcher, the list from A3 (others "Any" only until their ticket), `--log-focus` lines | Binding Steam from the pad works on the stand-in Steam; screenshot test | Stand-in |
| A5 | **Steam reads by account**: `signed_in_account` replaced by the bound id where there is one; owned games per account; achievements of the bound account fed to A2 | Two made-up `userdata` accounts in the fixture show two libraries by Profile | Stand-in |
| A6 | **The switch runner**: one launcher at a time, Steam first; Signing you in held with its line; the 30 s cap and Toasts; skip while a game of that launcher runs and offer again at its end; the **download question** (wait until done / cancel it and switch, 5.4); back up before writing and restore on failure; a `Switcher` trait per launcher | With a fake switcher that succeeds, fails and hangs, each case shows what 5.2 says; the download dialog's two answers on the stand-in Queue; screenshot tests | Stand-in |
| A7 | **Steam switch**: close with `-shutdown`, write `loginusers.vdf` and `AutoLoginUser`, start silent, wait for signed in | The stand-in Steam (`standin`'s `steam.exe` taught `-shutdown` and to read the files) comes up as the right account; **then S1, T1 to T4 on the TV PC** | Stand-in, then **real Steam** |
| A8 | **Battle.net**: list `SavedAccountNames` (+ BattleTag from `CachedData.db`), switch by putting one first; leave the Agent running | Made-up config files; **then B1, B2 on the TV PC** | Stand-in, then **real client** |
| A9 | **The sign-in store** (only if decision 3 allows copies): a snapshot per (launcher, account) of files, registry values and a Credential Manager entry, each wrapped with DPAPI for the Windows user in the data folder (7.4); never in the database or a log; deleted on unbind; "Forget this sign-in" in Settings; listed in Settings with the date each was saved | DPAPI and Credential Manager behind a trait, a fake on Linux; unbind and Forget leave no file behind; a log scan test finds no token | Stand-in |
| A9' | **No copies** (if decision 3 refuses them): a one-account launcher whose account differs is closed, its local sign-in cleared, and started on its own sign-in window, with a Toast "Sign in to <launcher> as <account>" (7.5) | Stand-in launcher shows its sign-in window; the Toast names the account | Stand-in, then **real Epic** |
| A10 | **Epic switch** by snapshot (A9): Credential Manager entry + `auth` folder, `EBWebView` cleared | Stand-in for the logic; **E1, E2, E4 on the TV PC** | Stand-in, then **real launcher** |
| A11 | **legendary and gogdl per account**: each one's sign-in file per bound account, their installs shared | Two made-up `user.json` folders (legendary) and two auth configs (gogdl); the right one used per Profile | Stand-in |
| A12 | **GOG Galaxy switch** by snapshot (A9): the registry values, `Configuration\config.json`, the `%ProgramData%` files of 1.5; the per-Profile play time of A2 not overwritten from `galaxy-2.0.db` | `standin make-gog` given two accounts' made-up files; **G1 on the TV PC** | Stand-in, then **real Galaxy** |
| A13 | **Ubisoft Connect switch** by snapshot (A9): the five files of 1.7, `user.closebehavior` set to close | Made-up files; **U1 on the TV PC** | Stand-in, then **real client** |
| A14 | **EA app switch** by snapshot (A9): the two folders of 1.6 without logs; the snapshot's size shown in Settings | Made-up folders; **EA1 on the TV PC** | Stand-in, then **real app** |
| A15 | **Xbox: name the account and say when it differs**, with the button to the Xbox app's account page | Needs X2 (how to read the Xbox app's gamertag) first; Toast tested on the stand-in | **Real Xbox app** for X1 to X3 |
| A16 | **Settings > Accounts help**: what is kept and where (7), how to forget it, "the Xbox app's account is switched in the Xbox app" | Text in, screenshot | Stand-in |

**Size**: 16 tickets (A9 or A9', not both). A1 to A6 change nothing on the TV PC and can be built now; A7 is the first that writes a real client's files. A9, A10 and A12 to A14 wait for decision 3; if it refuses copies, A9' replaces A9, and A10, A12, A13 and A14 shrink to their launcher's "clear and show the sign-in window" (a line each in A9'), leaving 12.

## 6. What needs trying on the TV PC

Each by the lead, by hand, with the user's own accounts, **after a backup** of the files named (copy them aside first; never into the repository or a ticket). Two accounts per launcher are needed; a family member's own, signed in on the PC once with Remember me.

**Steam**

- **S1. Which keys.** With Steam signed in to account A and B remembered, open `<Steam>\config\loginusers.vdf` and note **the key names only** of A's block (is it `AutoLogin` or `AllowAutoLogin`? is `MostRecent` there?), and `reg query HKCU\Software\Valve\Steam /v AutoLoginUser` (note only that it is A's account name). Look for: the key names TcNo uses.
- **T1. A switch by hand, timed.** `steam.exe -shutdown`; wait until `steam.exe` and `steamwebhelper.exe` are gone (time it); edit as 1.1 steps 3 and 4 for B; `steam.exe -silent`; time until B is signed in (the tray menu, or `userdata\<B>\config\localconfig.vdf` rewritten). Look for: no password asked, no Steam Guard, the time from shutdown to signed in, whether a window showed.
- **T2. Steam's own Change Account.** Steam > Change Account, pick A. Look for: whether it asks for anything; whether `loginusers.vdf` changes the same way as T1 (key names only).
- **T3. The Dashboard after a switch.** With the Dashboard running and the CEF debugger on, do T1. Read the log: does the CEF connection come back, and does the Steam source (at its next read) show B's play time? Look for: the `store:` lines and the library count.
- **T4. A download.** Start a small download as A, switch to B as T1. Look for: does it resume under B, is it paused, is it gone.

**Epic**

- **E1. What the session is.** Signed in as A: `cmdkey /list | findstr epicgames` (note the target name only), and list the file names in `%LocalAppData%\com.epicgames.eglv2\auth`. Look for: the target `credentials.com.epicgames.eglv2.prod` and `auth.json`, `access-token-prod.dpapi`.
- **E2. A swap by hand.** Close the launcher (tray > Exit; then confirm `EpicGamesLauncher.exe` and `EpicWebHelper.exe` are gone); copy `auth` aside as A; sign in to B in the launcher with Remember me; close it; copy `auth` aside as B. Credential Manager's entry cannot be copied with built-in tools, so this test tries **files only** first: put A's `auth` back, start the launcher. Look for: signed in as A, as B, or signed out. If signed out, the credential is needed too (A10 must snapshot it, as TcNo does); that is expected.
- **E3. legendary.** `legendary status` before and after E2. Look for: legendary stays on its own account (it should).
- **E4. How long a snapshot lives** (for decision 3). After E2, leave B's snapshot unused for a week (the launcher runs as A meanwhile), then put it back. Look for: B signed in, or signed out. Then sign B out from Epic's site ("Sign out everywhere", 7.3) and put B's snapshot back again. Look for: signed out (the copy is dead), which is what makes revoking work.

**Battle.net**

- **B1. Remembered accounts.** Signed in as A with "Keep me logged in", sign out to the account picker, sign in as B the same way. Then: the client closed, `%AppData%\Battle.net\Battle.net.config`'s `SavedAccountNames` (note the order only), put A first, start the client. Look for: signed in as A with no password. Also `cmdkey /list | findstr -i battle` (names only) to see whether its token is in Credential Manager.
- **B2. The Agent.** Start a small install through the Dashboard (the Agent route), then do B1. Look for: does `Agent.exe` keep running, does the install continue, does `GET /install/<uid>` still answer.

**GOG Galaxy, Ubisoft Connect, EA app** (decision 5; each after a backup of the files 1.5 to 1.7 name)

- **G1. GOG.** With Galaxy signed in as A: `reg query HKCU\Software\GOG.com\Galaxy /v refreshToken` and note only whether the value **reads as text** (a long printable string) or as binary; do not copy it anywhere. Then a swap by hand as E2, with the registry values and files of 1.5, A to B and back, timed. Look for: signed in with no password; whether writing under `%ProgramData%\GOG.com\Galaxy` needed an administrator; whether the Dashboard's GOG play time (from `galaxy-2.0.db`) changed with the swap.
- **U1. Ubisoft.** The swap by hand as E2 with the five files of 1.7, for an account with two-step verification on and the PC not trusted, then trusted. Look for: signed in, or asked for the password or a code; whether `ConnectSecureStorage.dat` from A works after B has signed in and out.
- **EA1. EA.** The size of `%LocalAppData%\Electronic Arts\EA Desktop\` and `%ProgramData%\EA Desktop\` (sizes only), then the swap by hand of both folders, logs left out. Look for: signed in as A with no password; whether the EA app's settings went back to what they were when A was saved; whether starting an EA game after the swap brings up A.

**Xbox**

- **X1. Two accounts in the Xbox app.** Settings > Accounts > Email & accounts > "Accounts used by other apps": is the second Microsoft account there? If not, add it there (its owner signs in). Then in the Xbox app: gamerpic > Switch account (or Sign out > sign in) to B. Look for: does it ask for B's password, PIN or an approval, or switch with a click? Then the same for Settings > Account > Change my Microsoft Store account. Then start a Game Pass game under each. Look for: which starts, which says it is not owned.
- **X2. Reading the Xbox app's gamertag.** Is there a file or setting naming the signed-in gamertag without a sign-in of our own (look in `%LocalAppData%\Packages\Microsoft.GamingApp_8wekyb3d8bbwe\LocalState` and `\Settings` for a gamertag or XUID, names only)? Look for: anything stable. If none, A15 shows the account it was told, and cannot say when it differs.
- **X3. Opening the account page.** `start ms-xbox:` and `start ms-windows-store://settings`. Look for: which link opens the Xbox app's or Store's account settings directly.

## 7. Keeping a copy of a launcher's sign-in: the risks

Decision 3 is open: the user asked "Explain the security risks". This is that, for Epic first, as plainly as the sources allow. Nothing here was tried; Epic documents none of its launcher's tokens.

### 7.1 What the copy contains, and what it allows

- **What is copied**: the Epic launcher's own remembered sign-in, two things together (1.2): the Credential Manager entry `credentials.com.epicgames.eglv2.prod` (at most 2560 bytes, copied whole, contents not documented) and the files in `%LocalAppData%\com.epicgames.eglv2\auth` (`auth.json` with the account's id and display name; `access-token-prod.dpapi` with the access token) [code: TcNo [`epic_new_launcher.go`][tc-epic]]. Since the launcher signs in after a restart with no password, one of the two must hold a **refresh token**, the long-lived key that gets new access tokens [inferred].
- **What it allows**: everything the launcher can do as that person, because it *is* the launcher's sign-in: see and download the library, start games and sign them in to Epic Online Services, friends and presence, and the launcher's store [inferred from what the launcher does with it]. **Purchases**: the store in the launcher uses the same session; whether a purchase with a saved card asks again for a password or a code is not documented [UNVERIFIED]. Epic's **parental controls PIN**, when set, is required to "authorize real-money purchases made with Epic payment" [docs (excerpt): [Epic, parental controls][epic-pin]], which is the brake that exists. **Changing the password, the email or two-step verification** is done on Epic's website, not in the launcher; whether a launcher token alone is enough there is not known [UNVERIFIED; my guess: no, those ask for the password or a code].
- So, in plain terms: **a copy is the account, for playing and probably for buying, but probably not for taking the account over.**

### 7.2 How it is protected

- The Dashboard would wrap each copy with **DPAPI for the Windows user** (as `discord-secrets.dat` is, docs/discord.md). DPAPI: "only a user with the same logon credential as the user who encrypted the data can decrypt the data. In addition, the encryption and decryption usually must be done on the same computer" [MS: [dpapi]]. So the copy is useless on another PC, to another Windows user, or to someone holding only the disk.
- It does **not** protect against **any program running as that Windows user**: such a program calls `CryptUnprotectData` or `CredRead` and gets it, as TcNo does [code: TcNo]. That is **exactly the protection the launcher's own sign-in has today**: the same DPAPI, the same Credential Manager, the same user.
- With decision 1 (one Windows user for the household), every person's programs, games and mods run as that one user, so each can reach every copy [inferred].

### 7.3 What is new compared with today

The launcher already keeps one such sign-in on the PC. What copies add:

1. **N sign-ins at rest instead of one.** Malware, a bad mod or a cheat tool running on the PC today gets the one Epic account signed in; with copies it gets every bound account's [inferred].
2. **A sign-in that lives on while its owner thinks it is gone.** When the launcher shows Sam, Alex may believe Alex's account is not on the PC; it is, in the Dashboard's folder. A family member who leaves, or a guest bound once, stays signed in on the PC until the copy is forgotten [inferred].
3. **Copies in a place a backup may take.** The Dashboard's data folder is `%LOCALAPPDATA%\XboxSeriesDashboard` (docs/HANDOVER.md). OneDrive's folder backup takes "Desktop, Documents, Pictures, Screenshots, and Camera Roll" [MS: [Known Folder Move][ms-kfm]], not `AppData`, so it would not; a **whole-disk or whole-profile image** would. DPAPI's keys go with such an image, unlocked by the Windows password, so an image plus that password opens the copies [inferred]. Today's one launcher sign-in has that same exposure.
4. **Lifetime.** Epic does not document its launcher's token lifetimes. A community-documented token reply for another Epic client shows `"expires_in": 7200` (2 h, the access token) and `"refresh_expires": 28800` (8 h, the refresh token) [reported: [FortniteEndpointsDocumentation][epic-token]]; the launcher's remembered sign-in evidently lasts longer, since it signs in after days unused [inferred]. Refresh tokens are usually **replaced** at each use, so a copy is only good until the launcher next refreshes that account; TcNo handles it by re-saving the outgoing account's session at every switch away, after closing the launcher and before restoring the other [code: TcNo [`internal/basic/flow.go`][tc-flow] `SwapTo`, "needsFreshSnapshot"]. A copy left unused ages out on its own at Epic's limit [inferred]; **E4** measures how long.
5. **How the owner revokes a copy from Epic's side**: account settings, **Password and security**, "Sign out everywhere", confirmed by a code emailed to the account [docs (excerpt): [Epic, "How do I sign out of all devices and all signed in sessions"][epic-signout]]. That ends every session, the copy included (E4 checks that a copy is dead after it). Changing the password does the same, by Epic's usual practice [inferred].

### 7.4 What the Dashboard would do to limit it

- Keep copies **only wrapped with DPAPI** for the Windows user, in the data folder (`accounts\<launcher>\<account id>.dat`), never in the database, settings, a log, a crash report or a screenshot; the log names the launcher and the account's public name only (section 4).
- **One copy per account**, refreshed at each switch away (as TcNo), so no stale extras pile up; no recovery copies beyond the one backup a switch needs, deleted when the switch succeeds.
- **Delete on unbind**, and when a Profile is deleted.
- **Settings > Profile > Accounts > Forget this sign-in**, per account, and one "Forget all saved sign-ins"; the page lists every copy with the date it was saved, so nothing is kept unseen.
- **Opt-in per account**: "Save this sign-in for <Profile>" is pressed for each account by its owner; nothing is saved by default.
- Settings says what a saved sign-in is, in two lines: "A saved sign-in lets the Dashboard switch Epic to you without a password. Anyone who can run programs on this PC as this Windows user could use it; sign out everywhere on Epic's site to end it."

### 7.5 The alternative: no copy

At each switch to a one-account launcher whose account differs, the Dashboard closes it, clears its local sign-in (the `auth` folder and the credential), and starts it on **its own sign-in window**, with a Toast "Sign in to Epic as <account>". The person signs in there each time (password, or Epic's other sign-in options, and two-step verification if on), with the on-screen keyboard and a pad [inferred]. At most one sign-in is at rest, as today, and none is ever the Dashboard's. Cost: a sign-in by hand at every switch of that launcher, which on a TV is slow, and Epic's sign-in page is not made for a pad [inferred]. Clearing the old sign-in locally leaves it valid at Epic until it expires, but no longer on the PC [inferred]. This is ticket A9'.

### 7.6 The same question for the other launchers

| Launcher | Is a copy kept? | What it is | Notes |
|---|---|---|---|
| **Steam** | **No** | Steam keeps each remembered account's sign-in itself; the Dashboard only changes which one starts (1.1) | No new exposure |
| **Battle.net** | **No** | The client keeps a sign-in per remembered email; the Dashboard reorders `SavedAccountNames` (1.3) | Where the client keeps them is unknown (B1); no new exposure |
| **Epic** | **Yes** | Credential Manager entry + `auth` folder (7.1) | This section |
| **GOG Galaxy** | **Yes** | The registry's `refreshToken` and the files of 1.5 | Possibly plain text in the registry (G1), so the DPAPI wrap would be the copy's only protection; revoke: GOG's sign-out of all sessions [reported, 2016] or a password change |
| **EA app** | **Yes** | Two whole folders, `cookie.ini` among them (1.6) | The largest copy, with settings and caches in it; how to revoke EA sessions was not found [UNVERIFIED] |
| **Ubisoft Connect** | **Yes** | `ConnectSecureStorage.dat` and four files (1.7) | Trusted-device rules may decide whether it works at all (U1); how to revoke was not found [UNVERIFIED] |
| **legendary, gogdl** | Already, by their design | Each keeps its own sign-in file; A11 would keep **one per account** | The same "N at rest" point, in files the user chose to add; listed and forgettable on the same page |
| **Xbox** | **No** | Not switched (5.3) | - |

So decision 3 is really "may the Dashboard keep saved sign-ins for one-account launchers (Epic, GOG, EA, Ubisoft)?", which can be answered per launcher.

## 8. Two routes: the Dashboard's own file work, or TcNo's command line

Decision 8: both scoped side by side. Read in TcNo at [`6f1ad81`][tc] (= release v4.0.9).

### 8.1 Route (a): the Dashboard does the file and registry work itself

- **What has to be built**: the switch runner (A6); per launcher, a lister and a switcher following 1.1 to 1.7 (A3, A7, A8, A10, A12, A13, A14); the sign-in store or its no-copy alternative (A9 or A9'). Everything else in 5.6 (A1, A2, A4, A5, A11, A15, A16) is needed by either route.
- **Coverage**: Steam, Battle.net, Epic (current launcher), GOG Galaxy, EA app, Ubisoft Connect. Not Epic's old launcher (not on the TV PC) or Xbox (not switched).
- **Effort**: 16 tickets for M10, of which **9 are the file work** (A3, A6's write-and-restore part, A7, A8, A9, A10, A12, A13, A14). Steam and Battle.net are small (markers only); each snapshot launcher is small once A9 exists.
- **The user installs**: nothing.
- **Licence**: nothing to comply with. TcNo is read as a reference for *where* each launcher keeps its sign-in, which is a fact about the launchers; no TcNo code is copied [inferred; not legal advice].
- **Failure modes**: a launcher update moves its files and the Dashboard's switch for it breaks until a fix ships (TcNo's maintainers face the same, and their `Platforms.json` changes are a free early warning to watch); a bug of ours could leave a launcher signed out (the backup and restore of A6 are the guard); writing under `%ProgramData%` (GOG, EA) may need an administrator, which the shell is not (TcNo turns "access is denied" into a "needs admin" error and offers to restart elevated [code: [`internal/basic/flow_helpers.go`][tc-helpers] `wrapNeedsAdminIfPermission`]); G1 and EA1 find out.
- **Testability**: all of it but the real launchers' acceptance runs in CI: listers on made-up files, switchers against `standin`'s fake launchers (it already makes a fake Steam, Epic, GOG, EA, Ubisoft and Battle.net), DPAPI and Credential Manager behind a trait with a fake on Linux. The real launchers are tested on the TV PC (section 6).

### 8.2 Route (b): calling TcNo Account Switcher's command line

- **What TcNo's CLI offers** [code: [`internal/cli/help.go`][tc-cli], [`main.go`][tc-main], [`internal/app/dispatch.go`][tc-disp]]:
  - `--list-accounts=<platform> --json`: each saved account's `uniqueId`, `displayName`, `lastLoggedIn`; printed and exited before TcNo's window or single-instance lock.
  - `+s:<steamId64>` and `+<code>:<uniqueId>` (codes from `Platforms.json`: `eg` Epic, `b` Battle.net, `g` GOG, `ea` EA, `u` Ubisoft): a **headless swap** (`RunHeadless`), exit code 0, or 1 with the error on stderr. It closes the launcher, re-saves the outgoing account, restores the target and starts the launcher.
  - **No "save current account" on the command line**: accounts are saved only in TcNo's own window.
- **What has to be built**: a TcNo driver (find it, check its version, list, swap, read the exit code, log), a Settings page that lists what TcNo has saved and sends the user to TcNo to save more, and everything route (a) shares (A1, A2, A4, A5, A6 without the write part, A11, A15, A16). The runner must still check "signed in" itself: TcNo starts the launcher and returns.
- **Coverage**: everything TcNo covers: 26 platforms, including all six here. No Xbox.
- **Effort**: about **10 tickets** (the 8 shared, the driver, the TcNo set-up page).
- **The user installs**: TcNo (installer or portable build from its [releases][tc-rel]), keeps it updated, and **saves every account of every launcher in TcNo's window**, outside the Dashboard; and must not set TcNo's app password (below).
- **Licence**: TcNo is **GPL-3.0** [code: [LICENSE][tc-lic]].
  - **Invoking it** is allowed and puts no condition on the Dashboard: "pipes, sockets and command-line arguments are communication mechanisms normally used between two separate programs" [docs: [GNU GPL FAQ, MereAggregation][gpl-faq-agg]].
  - **Shipping it** inside the Dashboard's installer is allowed as an aggregate, "even when the licenses of the other software are nonfree or GPL-incompatible" [docs: same], provided TcNo goes with its licence and its source (or a written offer for it) [docs: [GPL-3.0][gpl3] sections 5 and 6], and the Dashboard's own licence (undecided; plan.md question 5) does not restrict TcNo's. Simplest: **do not ship it**; the user installs it from TcNo's own page. Not legal advice.
  - Copying TcNo's code or its `Platforms.json` into the Dashboard would make that part GPL: not done on either route.
- **Failure modes** [code unless marked]:
  - **TcNo's window already open**: a second TcNo forwards its arguments to the first over a named pipe and **exits 0 at once**, so the Dashboard cannot tell whether the swap happened ([`main.go`][tc-main]: `ipc.ForwardArgs`). The Dashboard must check the result itself, and should make sure TcNo is not running first.
  - **TcNo's app password**: a headless swap first calls `security.RequireUnlocked()` and refuses while TcNo is locked; with saved-account encryption on, the saved copies cannot be read while locked [code: [`internal/security/vault.go`][tc-vault]]. Whether a headless run can ever be unlocked was not followed [UNVERIFIED].
  - **Saved copies outside the Dashboard's control**: TcNo keeps them in its own `LoginCache` folder; without its app password they are **plain copies** (GOG's `refreshToken` dumped into a `reg.json` as text [code: [`flow_registry.go`][tc-reg]]); only Epic's credential session is DPAPI-wrapped. So route (b) answers decision 3 with "yes, copies, in a weaker form, kept by another program", and the app password that would protect them breaks the headless swap.
  - **Elevation**: "needs admin" errors for `%ProgramData%` writes, and an "always run as admin" setting that would put a UAC prompt over the shell [code: [`flow_helpers.go`][tc-helpers]; `main.go` `ShouldElevateAtStartup`].
  - **A moving interface**: TcNo's CLI is described only by its `--help` text, with no promise of stability [inferred]. Its updater can change behaviour under the Dashboard between two of our releases.
  - **One more program trusted with every account's sign-in**, with its own updater, crash reporter and Discord presence [code; inferred].
- **Testability**: in CI, only the Dashboard's side, against a fake `TcNo-Acc-Switcher.exe` (a `standin` subcommand answering `--list-accounts --json` and `+x:<id>`); TcNo itself is a Windows desktop program and its file work is tested only on the TV PC [inferred].

### 8.3 Side by side

| | (a) The Dashboard's own | (b) TcNo's CLI |
|---|---|---|
| Tickets for M10 | **16** (9 of them file work) | **about 10** |
| Launchers | Steam, Battle.net, Epic, GOG, EA, Ubisoft | The same, and 20 more not needed |
| User installs | Nothing | TcNo, kept updated |
| Saving an account | In Settings, with the pad | In TcNo's own window |
| Saved sign-ins (decision 3) | Ours: DPAPI-wrapped, opt-in, listed, forgettable; or none (A9') | TcNo's: plain copies unless its app password, which blocks headless swaps |
| Licence | Nothing | GPL-3.0: invoking fine; shipping needs its source and licence along |
| When a launcher changes | We fix it | TcNo fixes it, usually first |
| Result of a swap | Known (our own code) | Exit code, but none when TcNo's window is open |
| Tested in CI | All but the real launchers | The driver only, against a fake TcNo |

Route (b) drops A3, A7's and A8's writes, A9 or A9', A10, A12, A13 and A14, and adds the driver and the TcNo set-up page.

### 8.4 Recommendation

**Route (a), the Dashboard's own file work**, with TcNo's `Platforms.json` and Steam code kept as the reference to check against at each TcNo release. The six extra tickets are small, and the gains are the ones that matter on a TV: accounts are saved from the Dashboard with the pad, the saved sign-ins are the Dashboard's to protect, list and forget (decision 3's whole question), nothing else has to be installed or kept unlocked, and a swap's result is never unknown. Build it in two steps: Steam and Battle.net first (no copies, so decision 3 does not block them: A1 to A8), then the snapshot launchers once decision 3 is taken. Route (b) is the fallback if the snapshot launchers turn out to change their files often (watch for it in G1, EA1, U1 and TcNo's history).

## Open questions for the user

Answered on 18 November 2026: see "Decisions" at the top. Still open:

1. **Decision 3, saved sign-ins**, now for Epic, GOG, EA and Ubisoft, with the risks in section 7: keep them (opt-in per account, 7.4), or no copies and the launcher's own sign-in window at each switch (7.5)? It can be answered per launcher.
2. **Xbox, the reading of "Nah"**: no Windows user per person, and the Dashboard only names the account and opens the Xbox app's account page. Right?
3. **The route**: (a) the Dashboard's own file work (recommended) or (b) TcNo's command line (section 8)?
4. **Play time seed** (5.5): when a Profile is first bound to an account, take that account's existing play time and achievements into the Profile once, or start the Profile from zero?

[tc]: https://github.com/TCNOco/TcNo-Acc-Switcher/tree/6f1ad81e80703da600c8f6d3263a288a23bf698f
[tc-pj]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/Platforms.json
[tc-sw]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/switcher.go
[tc-lu]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/loginusers.go
[tc-vw]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/vdf_write.go
[tc-al]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/autologin_windows.go
[tc-ka]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/known_accounts.go
[tc-nq]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/steam/native_quit_windows.go
[tc-epic]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/basic/epic_new_launcher.go
[tc-cli]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/cli/help.go
[tc-sg]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/docs/steam-guard-passwords-and-factors.md
[tc-faq]: https://github.com/TCNOco/TcNo-Acc-Switcher/wiki/FAQ---More-Info
[mckay]: https://dev.doctormckay.com/topic/4380-is-there-way-to-extract-refreshtoken-from-steam-client/
[steam-forum]: https://steamcommunity.com/discussions/forum/1/3875966763780751664
[deck-beta]: https://steamdeckhq.com/news/steam-deck-beta-client-update-account-switching/
[lg-auth]: https://github.com/legendary-gl/legendary/blob/170964e/legendary/core.py
[lg-cfg]: https://github.com/legendary-gl/legendary/blob/170964e/legendary/lfs/lgndry.py
[bn-regress]: https://us.forums.blizzard.com/en/blizzard/t/regression-launcher-cannot-remember-usernameemail-anymore/56298
[nx]: https://github.com/domsvoboda/nexus-launcher/tree/c239c3fd4c38582d66fb148ccb13317581b40af4
[nx-sa]: https://github.com/domsvoboda/nexus-launcher/blob/c239c3fd4c38582d66fb148ccb13317581b40af4/src/main/saved-accounts.ts
[nx-rel]: https://github.com/domsvoboda/nexus-launcher/releases
[nx-site]: https://xodusnexus.com/
[xo]: https://github.com/xodus-gaming/xodus/tree/49453a4a458f8d9d0e8c93de6319d60b4291fa59
[xo-login]: https://github.com/xodus-gaming/xodus/blob/49453a4a458f8d9d0e8c93de6319d60b4291fa59/docs/xodus/login.md
[xo-dev]: https://github.com/xodus-gaming/xodus/blob/49453a4a458f8d9d0e8c93de6319d60b4291fa59/docs/xodus/device.md
[xo-kc]: https://github.com/xodus-gaming/xodus/blob/49453a4a458f8d9d0e8c93de6319d60b4291fa59/crates/xodus/src/tokens/backend/keychain.rs
[ms-mismatch]: https://learn.microsoft.com/en-us/gaming/gdk/docs/store/commerce/pc-specific-considerations/xstore-handling-mismatched-store-accounts
[ms-xuser]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/xuser/functions/xuseraddasync
[ms-xuseropt]: https://learn.microsoft.com/gaming/gdk/docs/reference/system/xuser/enums/xuseraddoptions
[ms-wam]: https://learn.microsoft.com/windows/uwp/security/web-account-manager
[ms-findall]: https://learn.microsoft.com/uwp/api/windows.security.authentication.web.core.webauthenticationcoremanager.findallaccountsasync
[ms-msal-wam]: https://learn.microsoft.com/entra/msal/dotnet/acquiring-tokens/desktop-mobile/wam
[ms-svc]: https://learn.microsoft.com/windows-server/security/windows-services/security-guidelines-for-disabling-system-services-in-windows-server
[ms-xblidp]: https://learn.microsoft.com/windows/win32/api/_xblidp/
[ms-passkey]: https://www.microsoft.com/en-us/security/blog/2025/05/01/pushing-passkeys-forward-microsofts-latest-updates-for-simpler-safer-sign-ins/
[dpapi]: https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
[xb-store]: https://support.xbox.com/en-US/help/account-profile/manage-account/gaming-and-store-accounts
[xb-switch]: https://support.xbox.com/en-US/help/account-profile/manage-account/switch-accounts-xbox-app-windows
[msa]: https://www.microsoft.com/en-us/servicesagreement
[ssa]: https://store.steampowered.com/subscriber_agreement/
[epic-tos]: https://cdn1.epicgames.com/eulatracking-download/epicgames_tos/en/v2/r1/45d4dd9e67fab2855ba409cd77569499.pdf
[bn-eula]: https://www.blizzard.com/legal/2c72a35c-bf1b-4ae6-99ec-80624e1b429c
[tc-reg]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/basic/flow_registry.go
[tc-flow]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/basic/flow.go
[tc-helpers]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/basic/flow_helpers.go
[tc-main]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/main.go
[tc-disp]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/app/dispatch.go
[tc-vault]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/internal/security/vault.go
[tc-lic]: https://github.com/TCNOco/TcNo-Acc-Switcher/blob/6f1ad81e80703da600c8f6d3263a288a23bf698f/LICENSE
[tc-rel]: https://github.com/TCNOco/TcNo-Acc-Switcher/releases
[gpl-faq-agg]: https://www.gnu.org/licenses/gpl-faq.html#MereAggregation
[gpl3]: https://www.gnu.org/licenses/gpl-3.0.html
[epic-token]: https://github.com/LeleDerGrasshalmi/FortniteEndpointsDocumentation/blob/main/EpicGames/AccountService/Authentication/Token.md
[epic-signout]: https://www.epicgames.com/help/en-US/c-202300000001645/c-202300000001755/how-do-i-sign-out-of-all-devices-and-all-signed-in-sessions-for-my-epic-games-account-a202300000014282
[epic-pin]: https://safety.epicgames.com/parental-controls
[ms-kfm]: https://learn.microsoft.com/sharepoint/redirect-known-folders
[gog-forum]: https://www.gog.com/forum/general/my_account_has_been_hacked/post7
[gog-2fa]: https://www.tweaktown.com/news/50911/gog-adds-optional-two-step-login-https-encryption/index.html
[ea-help]: https://help.ea.com/no/articles/platforms/how-to-use-ea-app/
[ea-forum-keep]: https://forums.ea.com/discussions/ea-app-technical-issues-en/re-ea-app-fails-to-remember-keep-me-signed-in-option/7657954
[ea-forum-two]: https://forums.ea.com/discussions/ea-app-feedback-en/is-two-too-many/12367299
[ea-forum-sims]: https://forums.ea.com/discussions/the-sims-4-technical-issues-pc-en/how-do-you-signout-of-your-sims4-account-to-login-into-a-different-account/11817195
[ubi-remember]: https://steamcommunity.com/app/2208920/discussions/1/3713811178932018440
