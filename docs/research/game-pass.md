# The PC Game Pass catalogue, without a sign-in (#277)

What Microsoft's public, no-sign-in catalogue gives of PC Game Pass for a region, how it was checked, and what is
still a guess. Built in `library::game_pass` (fetch, keep) and `source_xbox` (list); the spec is docs/sources.md,
"Xbox", and docs/spec/launchers.md, 8.

Every request here was made by hand on 2026-11-18 with a generic user agent and no cookie, for the markets US and GB.
**[V]** fetched and checked; **[3P]** seen in other people's code; **[U]** not verified.

## 1. The lists: `catalog.gamepass.com/sigls/v2`

`GET https://catalog.gamepass.com/sigls/v2?id=<list id>&language=<xx-YY>&market=<CC>` **[V]**:

- A JSON array. Element 0 is the list's header, `{siglId, title, description, requiresShuffling: "False", imageUrl}`;
  every other element is `{"id": "<product id>"}` (the Store's twelve-character "big id").
- An unknown list id is 404; a market without Game Pass (CN, or a made-up `XX`) is 404 with a `problem+json` body.
- Headers: `cache-control: max-age=~595`, `expires`, `ms-cv`. **No `ETag` and no `Last-Modified`**: no request can be
  conditional today. The code keeps an `ETag` and sends it back (`If-None-Match`) should one ever be given; until
  then politeness is the once-a-day rule alone.

The lists (counts without the header) **[V]**:

| List | id | US | GB | Header title |
|---|---|---|---|---|
| **All PC games, with EA Play** (used) | `609d944c-d395-4c0a-9ea4-e9f39b52c1ad` | 603 | 605 | All PC Games |
| All PC games, without EA Play | `fdd9e2a7-0fee-49f6-ad69-4354098401ff` | 514 | 515 | All PC Games |
| EA Play on PC | `1d33fbb9-b895-4732-a8ca-a55c8b99fa2c` | 89 | 90 | EA Play Games |
| PC, recently added | `3fdd7f57-7092-4b65-bd40-5a9dac1b2b84` | 38 | 39 | Recently added |
| PC, leaving soon | `cc7fc951-d00f-410e-9e02-5e4628e04163` | 8 | 8 | Leaving soon |
| PC, coming soon | `4165f752-d702-49c8-886b-fb57936f6bae` | 9 | 9 | Coming to Game Pass |
| PC, day one | `4b59700c-801f-494a-a34c-842b8c98f154` | 60 | 60 | Day one releases |
| PC, most popular | `a884932a-f02b-40c8-a903-a008c23b1df1` | 20 | 20 | Most popular |

`609d944c` is exactly `fdd9e2a7` and the EA Play list together, in both markets. **Why `609d944c`**: it is the list
xbox.com's own catalogue page names `allgamespc` (its script,
`https://www.xbox.com/en-us/xbox-game-pass/games/js/xgpcatPopulate-2025.js`), and EA Play is part of PC Game Pass.
Other tools use `fdd9e2a7`: Playnite's Game Pass Catalog Browser
(`darklinkpower/PlayniteExtensionsCollection`, `GamePassCatalogBrowserService.cs`) and `pazguille/xbox-games-api`
(`utils/fetch-gamepass-list.js`) **[3P]**; `itimurcom/gamepass` lists the ids (`xbox_sigl_ids.md`) **[3P]**.

**xbox.com now asks `sigls/v3`** with `&platformContext=pc&subscriptionContext=<tier>` (`cfq7ttc0kgq8` PC Game Pass,
`cfq7ttc0khs0` Ultimate, `cfq7ttc0p85b` Premium, `cfq7ttc0k5dj` Essential): same shape, filtered by tier
(`609d944c` gave 557 ids for the PC tier against v2's 603). v2 is what the ticket names and still answers; which 46
games v3 leaves out for the PC tier is **[U]**, so the Dashboard may list a few that only Ultimate has. Moving to v3
with the PC tier is one line (`library::game_pass::fetch`) if that proves wrong.

## 2. The products: `displaycatalog.mp.microsoft.com/v7.0/products`

`GET https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds=<ids, comma-separated>&market=<CC>&languages=<xx-YY>`
**[V]**, the same service the art lookup asks (docs/research/art-sources.md):

- `Products[]`: `ProductId`, `ProductKind` (`Game`), `LocalizedProperties[0]` (`ProductTitle`, `DeveloperName`,
  `PublisherName`, `Images[]`), `MarketProperties[0].OriginalReleaseDate`, `Properties.PackageFamilyName`,
  `DisplaySkuAvailabilities[].Sku.Properties` (`Packages[].PackageFamilyName` with `PlatformDependencies`,
  `IsBundle`, `BundledSkus[].BigId`).
- Images as the art lookup takes them: `BoxArt` (square, 1080), `Poster` (720x1080), `SuperHeroArt` and
  `TitledHeroArt` (1920x1080), each `Uri` starting `//store-images.s-microsoft.com/...`, which takes `?w=`.
- **The package family name** (what an installed Xbox game is known by), over the first 100 PC Game Pass ids: 81 in
  `Properties.PackageFamilyName`, 67 under the SKUs' packages, 14 only in `Properties`, **19 in neither**: bundles
  (`IsBundle: true`, no packages), whose family name is their bundled products' (Age of Empires II DE `9N42SSSX2MTG`,
  Avowed `9MSVPJCHHRPR`). So: `Properties` first, then the SKUs' packages, then a bundle's `BundledSkus` asked for
  too.
- **Batches**: 20 to 300 ids in one request all came back whole; xbox.com's own page asks 20 at a time
  (`chunk = 20`); the Dashboard asks 100, a quarter of a second apart: at 20 a fetch of the 603 took 26 s, at 100 12 to 14 s, which
  a first start without a library waits for.
- `MS-CV` is not needed (Microsoft's page and Playnite send a fixed one; requests without it answered the same). An
  unknown id is left out of the answer. `cache-control: no-store, no-cache`.

## 3. Install: the Store's page

`ms-windows-store://pdp/?ProductId=<product id>` is Microsoft's documented, recommended form (the package family name
and app id forms are deprecated; `&mode=mini` falls back to the full page from February 2026):
https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-store-app. `msxbox://game/?productId=<id>` opens
the Xbox app's page; it is in PlayFab's deep-link table (`microsoft-xbox-pc`,
https://learn.microsoft.com/xbox/playfab/economy-monetization/economy-v2/catalog/deep-links) and Microsoft's GDK
samples' readmes, but on no reference page of its own. The Dashboard keeps the Store's, the route Xbox games
already take (docs/sources.md), until #173's queue route is proven.

## 4. Where PC Game Pass is sold

Xbox's regions page, https://www.xbox.com/en-US/regions, its table loaded from
`https://www.xbox.com/en-US/regions/JS/regionsTable.json` (column "PC Game Pass", keyed by locale; the country is the
locale's suffix) with names from `.../JS/globalContent.json` **[V]**: 85 countries marked available. Russia is
marked with a footnote (new sales suspended) and left out; China is not available. The list is `model::game_pass::
MARKETS`, by the page's names. The lists answer 200 for every one of them (and for RU), 404 for CN.

## 5. Windows' region and language

- `GetUserDefaultGeoName` (Windows 10 1709 on): the user's home location, an ISO 3166-1 code, **or a UN M.49 number
  as text** where the place has no two-letter code: then the regional format's country is taken.
  https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getuserdefaultgeoname
- `GetUserDefaultLocaleName`: the regional format (`en-GB`).
  https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getuserdefaultlocalename
- `GetUserPreferredUILanguages` with `MUI_LANGUAGE_NAME`: the display languages by name, the first taken as the
  catalogue's language. https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getuserpreferreduilanguages

## 7. Whether the account has PC Game Pass (the subscription check)

Asked: can an unpackaged program, with no sign-in or token of its own, tell whether the Microsoft account on this PC
has PC Game Pass or Game Pass Ultimate? **No route found tells "subscribed now" without a sign-in; none open to it
even tells "a Game Pass licence was seen on this PC".** Ranked, with what each would prove:

| # | Route | Proves | Unpackaged, no sign-in? | Verdict |
|---|---|---|---|---|
| 1 | Microsoft's subscriptions service, `POST https://beige.xboxservices.com/PCGAFD/entitlements/subscriptions` with the plan ids (`CFQ7TTC0KGQ8` PC, `CFQ7TTC0KHS0` Ultimate, `CFQ7TTC0K6L8` console; `status: "Active"`) **[3P]** (`uxandai/launchme`, `helpers/xbox/src/library.rs`; `playportdev/playport`, `XboxEntitlements.swift`) | subscribed now | **No**: its `authorization` is a Microsoft account ticket for `www.microsoft.com`, which launchme gets by posing as Microsoft's own licensing client (`tokens.rs`): impersonation, out of bounds | the sign-in that would be needed |
| 2 | Xbox Live OAuth with a registered app of our own (Playnite XboxLibrary: `login.live.com/oauth20_authorize.srf`, scope `Xboxlive.signin Xboxlive.offline_access`, then `user.auth` and `xsts.auth.xboxlive.com`) **[3P]** (`JosefNemec/PlayniteExtensions`, `XboxAccountClient.cs`) | the title history; Playnite reads no subscription at all | No: a sign-in in a browser window, and an app registration | see docs/research/owned-games.md (#276) |
| 3 | `Windows.Services.Store.StoreContext` (`GetStoreProductsAsync`, `GetUserCollectionAsync`, `GetAppLicenseAsync`) for the plan ids | would be "this account has it" | **No**: it answers for "the current app" and its add-ons only; an unpackaged process has none **[docs]** (learn.microsoft.com, `StoreContext.GetUserCollectionAsync`: "the add-ons of the current app") | the probe prints what it answers |
| 4 | `AppInstallManager` checks (`GetIsAppAllowedToInstallAsync`, `GetIsApplicableAsync`) on a Game Pass game | policy and device, not licence **[docs]** | Yes | says nothing of a subscription; its free-entitlement calls *claim*, a change, never used |
| 5 | Gaming Services' registry, the Xbox app's `LocalState`/`LocalCache`, ClipSVC's `tokens.dat` | at most "a licence was seen" | Registry yes (same values for bought and Game Pass games **[3P]**); the rest protected or encrypted | nothing to read |

**Built**: `source_xbox::subscription::judged` answers **can't tell**, with why, in the log at start
(`gamepass: subscription: can't tell (no route tells without a sign-in: ...)`), and Settings > Launchers shows
**Game Pass: can't tell** beside Show Game Pass games. The three answers exist (`model::game_pass::Subscription`:
`Active` "a Game Pass licence was seen on this PC", never "subscribed now"; `NotFound`; `Unknown`) and
`DASHBOARD_GAMEPASS_SUBSCRIPTION` (`active`, `none`, `unknown`) stands in for them. **Nothing depends on the answer**:
the catalogue is listed whatever it is, and Install goes to the Store's queue, which refuses a game the account has no
licence for (`0x803F8001`); the Dashboard then opens the Store's page, where the user subscribes or buys. Decided:
the hub's Install stays **Install**, whatever the answer; the Store says the rest.

**What would change this**: the probe (`e2e/htpc/probes/gamepass.ps1`, section 8) printing a `storecontext.product.
CFQ7TTC0KGQ8` line with `in the user's collection True` from an unpackaged PowerShell: that would make route 3 work,
and `judged` would ask it the same way the queue is asked.

## 8. Installing through the Store's queue (#173)

**What an unpackaged caller may call** **[docs]** (learn.microsoft.com, `Windows.ApplicationModel.Store.Preview.
InstallControl.AppInstallManager`): the class's requirements list `runFullTrust`, and its remarks say "Access to this
API is protected by a private capability that is restricted to apps that are developed by Microsoft". That restriction
is enforced through a packaged app's manifest; an unpackaged process has no manifest, and is not refused for it
**[3P]**: Chromium's browser process calls `StartProductInstallAsync` unpackaged to install the Windows App Runtime
(`chrome/browser/webnn/win_app_runtime_installer.cc`), WSL's `wsl.exe` installs distributions with it
(`src/windows/common/Distribution.cpp`), and Fleet and Atlas script it from PowerShell (elevated). So it is a support
risk (Microsoft may close it), not a technical block. `StoreContext.RequestDownloadAndInstallStorePackagesAsync` is
the current app's own packages only **[docs]**: not a route. `PackageManager.RemovePackageAsync(fullName)` works for
the user's own packages from a normal, unelevated process **[docs]**.

**Built** (`source_xbox::store_queue`, the one place every call is): `store_queue.ps1`, run by Windows PowerShell 5.1
(`-EncodedCommand`, no window), loads the WinRT types and answers one line a call:

- Install: `AppInstallManager.StartProductInstallAsync(productId, "", "xbox-series-dashboard", "", options)`, the
  options for this user only (`InstallForAllUsers = false`), no forced restart, no launch, `TargetVolume` the
  `PackageVolume` of the folder's drive (the Xbox app's folder there, from `.GamingRoot`).
- Progress: `AppInstallItems`, each `GetCurrentStatus()`: `InstallState`, `PercentComplete`, `BytesDownloaded`,
  `DownloadSizeInBytes`, `ErrorCode`; one `watch` process writes them as JSON once a second while the Dashboard runs
  and the queue has anything. States: Pending, Starting, AcquiringLicense, ReadyToDownload are Queued; Downloading,
  RestoringData, Installing are Downloading; Paused* are Paused; Completed, Canceled, Error end it.
- Pause, resume, cancel: `AppInstallManager.Pause`, `Restart`, `Cancel` by product id.
- Uninstall: `PackageManager.FindPackagesForUser("", family)` and `RemovePackageAsync(fullName)` for each.
- Any refusal (`refused 0x... <why>`) falls back: install to the Store's page (`ms-windows-store://pdp/?ProductId=`),
  uninstall to Settings' installed apps; the log says which and why.

Log: `xbox: 9PD5BM2Z8C4L: asking the Store's queue to install Vampire Survivors onto D:\XboxGames`,
`xbox: store queue: start 9PD5BM2Z8C4L D:\XboxGames -> started Pending (2310 ms)`, `xbox: 9PD5BM2Z8C4L: the Store's
queue took Vampire Survivors (Pending)`, `xbox: store queue: watching the queue`, `xbox: 9PD5BM2Z8C4L: Vampire
Survivors is Installing... (Downloading, 4%)`, `... installed: the Store says complete`; a refusal `xbox:
9PD5BM2Z8C4L: the Store's queue did not take Vampire Survivors (0x803F8001 ...): opening ms-windows-store://pdp/
?ProductId=9PD5BM2Z8C4L instead`.

**Still guesses** (installs.md 12, X1 to X4): whether the queue takes a Game Pass GDK game for an unpackaged caller
(the licence step, `AcquiringLicense`); whether `TargetVolume` puts it in the Xbox app's `XboxGames` folder or the
drive's `WindowsApps`; whether installs the Xbox app starts are listed in `AppInstallItems` (only the Dashboard's own
are followed); whether `RemovePackageAsync` removes `XboxGames\<game>` and leaves the Xbox app saying not installed;
whether PowerShell's WinRT loading finds every type by the names `store_queue.ps1` tries.

**The probe** (`e2e/htpc/probes/gamepass.ps1`, read-only) prints `key: value` lines: `process.elevated`,
`store.installed`, `xboxapp.installed`, `gamingservices.installed`, `service.<name>`, `xboxapp.<folder>.files` and
`.named` (names only), `registry.<key>` (key and value names only), `clipsvc.tokens`, `xbox.games.installed`,
`storecontext.create`, `storecontext.user`, `storecontext.products.error`, `storecontext.product.<id>`,
`storecontext.collection.count`, `storecontext.applicense`, `appinstallmanager.create`,
`appinstallmanager.autoupdate`, `appinstallmanager.allowed.<id>`, `appinstallmanager.items` and
`appinstallmanager.item.<id>`, `packagemanager.create`, `packagemanager.volume`, `gamingroot.<drive>`. Answers that
would change the code: `appinstallmanager.create` not `ok` (the queue route is dead: fall back always);
`appinstallmanager.items` failing while `create` is ok (progress from the package manager instead);
`storecontext.product.CFQ7TTC0KGQ8 ... in the user's collection True` (the subscription check gains a route);
`packagemanager.volume` without the drive the Xbox app installs to (`TargetVolume` left to the Store).

## 6. What is not known

- **Whether the user subscribes.** Nothing here says; the Dashboard lists the catalogue as what can be installed
  with a subscription, and the Store's page says the rest.
- Which games v3's PC tier leaves out (section 1).
- Whether an EA Play game's Store page installs it or sends the user to the EA app: the Store decides.
- Counts drift daily: the live test (`cargo test -p library --lib game_pass -- --ignored --nocapture`) prints them.
