# Helpers

Some stores install through a helper program the user adds: legendary for Epic Games, gogdl for GOG
(`docs/research/installs.md`, 3, 4 and 10.1). This page is how the Dashboard fetches, checks, keeps, runs and removes
them (#174). What the stores do with them is #175 (Epic) and #176 (GOG); nothing here knows a store's commands. The
Bundle had nothing of the kind: the words are the port's own.

The code: `crates/helpers` (the table, the fetch and check, the removal; none of it needs Windows),
`crates/shell/src/helper.rs` (the runner), `crates/app/src/helper_programs.rs` (the thread, the states, the log),
Settings > Helpers (`crates/screens/src/settings/helpers.rs`), the stand-in (`helpers::fake`, `standin
serve-helpers`).

## 1. The table

**One table, in the Dashboard's own source**: `helpers::builtin()`. Each helper has one fixed release: its version,
the Windows x64 program's address on the project's own GitHub release page, and that file's SHA-256. Every update of
a helper is a reviewed change of this table. Looked up on 2026-11-17:

| Helper | Store | Version | Licence | Program | SHA-256 |
| --- | --- | --- | --- | --- | --- |
| legendary | Epic Games | [0.21.1](https://github.com/legendary-gl/legendary/releases/tag/0.21.1) | GPL-3.0 | `legendary_windows_x64.exe` | `6be77857dc0a6dd33ea7442ce8b5291944ec6da53757f81ca95b57ce794baf5b` |
| gogdl | GOG | [v1.4.0](https://github.com/Heroic-Games-Launcher/heroic-gogdl/releases/tag/v1.4.0) | GPL-3.0 | `gogdl_windows_x86_64.exe` | `291b653e6dbf7a394a3342ed0ebcea2cbc83e5f66d030777d65ccdd60d18ab02` |

**Where the SHA-256s come from.** Neither project publishes a checksum file of its own: no `SHA256SUMS` among either
release's files and none in the notes. The hashes above are the ones GitHub itself shows beside each file on the
release page (the releases API's `digest`, which GitHub computes from the file as it was uploaded). Nothing was
downloaded and hashed here. A table entry with no SHA-256 is never fetched ("It can't be checked yet").

A test gives a table of its own with `dashboard --helper-table <file>` (the same fields, as JSON; `standin
serve-helpers` writes one). Nothing else names an address.

## 2. Settings > Helpers

Games & apps > **Helpers** (U+E896), in the Installs section under Storage ([settings.md](settings.md) 6.1). A group a helper from y 190 (the line about what
helpers are went in #261, settings.md 16.11; the licence and the source stay, here and in Add's question), as About's: its name and store ("legendary for Epic Games"), "Version 0.21.1. GPL-3.0 licence. Source:
github.com/legendary-gl/legendary", how it stands, and one button.

| State | Line | Button |
| --- | --- | --- |
| Not added | "Not added. Epic Games games can't be installed from here without it." | Add |
| Fetching | "Downloading and checking it..." | none: a spinner |
| Added | "Added: version 0.21.1, in the Dashboard's data folder." | Remove |
| Failed | why, in words for the user (section 3) | Add |

Up and Down move between the helpers' buttons; B or Left goes back. **Add asks first**, a prompt over the dimmed page:
"Add legendary?", the licence, the source and the version in its message ("legendary is free software under the
GPL-3.0 licence, from github.com/legendary-gl/legendary. The Dashboard downloads version 0.21.1 from there, checks it,
and keeps it in its data folder. It isn't part of the Dashboard, and you can remove it here."), Yes and No, Yes
focused. Only Yes fetches (`Effect::Helper(HelperAsk::Add)`). **Remove asks too** ("Remove legendary?", "Epic Games
games can't be installed from here until it's added again.", Remove and Cancel).

Focus line: `Settings: Helpers: <key>; <Add|Remove|busy>; <state>`, the state `not added`, `fetching`,
`added <version>` or `failed`, and `; asking <question>` while one is up.

## 3. Fetching, checking, keeping

On Yes, on a thread of its own:

1. A plain GET of the program's address, redirects followed, User-Agent `dashboard-helpers`, nothing else: no token,
   no cookie, nothing of the user's, the machine's or the Dashboard's version. At most 64 MB.
2. Its SHA-256 compared with the table's. Not the same: **nothing is written**, and the page says "The download wasn't
   the file expected, so it wasn't kept." (`helpers: <key>: refused: its SHA-256 is <got>, not the <want> the
   Dashboard expects`).
3. Kept at `<data folder>\helpers\<key>\<key>.exe` (written as `.part`, then renamed), with `helper.json` beside it
   (its version and SHA-256).
4. Run once through the shell's runner with `--version`, every line it says logged (`helpers: <key>: says ...`),
   30 s at most. Ended with 0: Added. Otherwise it is removed again and the page says "It was downloaded but didn't
   run, so it wasn't kept."

At start the Dashboard reads what is kept: a helper whose `helper.json` gives the table's version is Added; another
version is Not added (`helpers: <key> <old> is kept, not <new>: add it again`). One line says them all:
`helpers: legendary 0.21.1 not added, gogdl 1.4.0 added`.

**Remove** deletes `<data folder>\helpers\<key>` and everything in it (`helpers: <key>: removed <folder>`).

The helpers are **never bundled or linked into the Dashboard**, and nothing of theirs is in the repository or the
release.

## 4. The runner

`shell::helper::Job::start(program, args, wake)` (`source_kit::helper`, so the stores' crates run their helper with it
too; `Job::start_with` adds environment variables) starts a helper in its own folder, with no window on Windows
(`CREATE_NO_WINDOW`), its standard output and error piped. A thread for each reads it a line at a time, a carriage
return ending a line too (a progress line rewrites itself with one), and hands each over through a channel
(`Said::Out`, `Said::Err`), calling `wake` so a sleeping UI thread looks; once the process has ended and both are read to their ends (or a
second more, where a process it started still holds them open), `Said::Ended(code)`. Nothing is read on the UI thread. `Job::stop()` kills the process (both helpers resume from what
they kept, so pause is a stop); its end is said as any end is, with no code. A job dropped is stopped.

`source_kit::downloads::Downloads` follows a store's installs through its helper on that runner: one run a game,
each line of its output read by the store's own parser (`Line::Total`, `Done`, `Speed`, `Error`) into the Queue's
`Install` (bytes, and the helper's own rate as bits a second), every line it says logged. Pause stops the run and
keeps the bytes; Resume runs the same command again, the helper resuming from its own files, the bytes it says are
left read against the whole; Cancel stops it and deletes what it fetched (the store says what). A run that ends 0 is
checked against the helper's own record before it counts as done (legendary ends 0 on some failures).

## 5. The stores through their helpers (#175, #176)

The decisions recorded on #166: Pause, Resume and Cancel are in the Queue; a game a helper installed is the
Dashboard's to look after; Windows' permission prompts are answered with the pad where they can be; the stores' terms
risk is accepted. Wherever a helper is not added or not signed in, the store's own route is used as before (its
launcher's install dialog), and the log says why (`epic: legendary: <AppName>: legendary is not signed in ...: asking
the Epic Games Launcher instead`).

**Epic Games through legendary 0.21.1** (`source_epic::legendary`; every fact from legendary's source at that tag,
named beside it in the module's comments):

- *Its records* stay where legendary keeps them (`LEGENDARY_CONFIG_PATH`, else `%USERPROFILE%\.config\legendary`), so a
  sign-in made with legendary itself is the Dashboard's. `user.json` is the sign-in; `installed.json` what it
  installed; `metadata\<AppName>.json` the account's library as it last listed it. Epic's source reads these beside
  the launchers' files: owned games it lists (not DLC, not Unreal Engine content, only those with a Windows build) and
  the games it installed (`epic: 4 games from ... (..., legendary 1 installed, 1 owned) in 6 ms`).
- *The library* is listed again at start, and whenever a helper is added, with `legendary status --json` (which also
  checks legendary's own site for a newer legendary, as it always does), off the window's thread; Epic is then read
  again (`Installer::refresh`).
- *Install* is `legendary -y install <AppName> --base-path <folder> --skip-sdl --skip-dlcs`: into `<folder>\<the game's
  FolderName>`, the folder the user's `Games` (one folder, so no question; naming folders with the pad is #174's).
  `-y` answers legendary's questions; prerequisites (`prereq_info`, a game's redistributables) are not run, and DLC is
  not installed. Progress is legendary's own lines on its standard error: `[cli] INFO: Download size: <MiB> MiB`, then
  `[DLManager] INFO:  - Downloaded: <MiB> MiB, Written: ...` and ` + Download\t- <MiB> MiB/s (raw) / ...` once a
  second. Once it ends, `-y egl-sync --export-only --one-shot --egl-manifest-path <...\Data\Manifests>` writes the old
  launcher's record where that folder is there. The current Epic Games Launcher does not read those records
  (installs.md P1), so a game legendary installed is not in Epic's launcher; Uninstall removes the record again.
- *Pause* stops legendary (it keeps `tmp\<AppName>.resume`), *Resume* runs the install again into the same folder,
  *Cancel* stops it and deletes the game's folder and the resume file. A pause lasts as long as the Dashboard runs:
  after a restart the next Install resumes from legendary's resume file.
- *Uninstall* is `legendary -y uninstall <AppName>`, no question but the Dashboard's own.
- *Play* for a game legendary installed is `legendary launch <AppName>`, which signs the game in with Epic; any other
  game starts through the launcher's link as before.
- *Signing in* is not the Dashboard's: Epic's sign-in is a web page, and the Dashboard shows none, asks for no password
  and keeps none. Settings > Helpers says how legendary stands (`Signed in to Epic Games as <name>.`, or `Not signed
  in to Epic Games: ...` with how to do it): today, `legendary auth` run once on the PC, which shows Epic's page in a
  window of its own (keyboard and mouse). A sign-in page driven by the pad (a web view, or a code shown as a QR code
  for a phone) is the user's decision, not made here; the log says `helpers: legendary: signed in as <name>` or `not
  signed in` at start.
- *What it cannot do* is refused through `Installer::can`, not faked: no folder to choose, nothing while not signed
  in.

**GOG through gogdl 1.4.0** (`source_gog::gogdl`; gogdl's source at that tag, and for the set-up and uninstall steps
Heroic's, which drives gogdl the same way):

- *Its folder* is the Dashboard's, `<data>\gogdl`: gogdl keeps no record of what it installed. `auth.json` is the
  sign-in (`--auth-config-path`), `heroic_gogdl\manifests\<id>` the build's manifest (every run is given
  `GOGDL_CONFIG_PATH`, so nothing goes to Heroic's own `%APPDATA%\heroic_gogdl`), `support\<id>` and `redist` the
  support files and the script interpreter, and `installed.json` the Dashboard's record of the games it installed,
  read by GOG's source beside the registry's (a gogdl install writes no `GOG.com\Games` key itself).
- *The library*: gogdl lists none, so the games owned and not installed are Galaxy's database's alone. Without Galaxy
  signed in once on the PC, there is nothing to install from here (listing it from GOG's own API with gogdl's token is
  a decision for later: a source reads files, never the network).
- *Install* is `gogdl --auth-config-path <auth.json> download <id> --platform windows --path <Games> --support
  <support\id> --skip-dlcs --lang en-US`, into `<Games>\<installDirectory>`; progress is gogdl's own `[PROGRESS] INFO: =
  Progress: <percent> <written>/<total>, ...` and ` + Download\t- <MiB> MiB/s (raw) ...` lines. Then the game's
  *set-up step*, as Heroic runs it: where the manifest says `scriptInterpreter`, `scriptinterpreter.exe` (fetched with
  `redist --ids ISI --path <redist>`) with `/VERYSILENT /DIR=<folder> /Language=English /LANG=English
  /ProductId=<id> /galaxyclient /buildId=<build> /versionName=<version> /lang-code=en-US /supportDir=<support>
  /nodesktopshortcut` (the build and version from `gogdl info`); else each product's `temp_executable` from its
  support folder. Redistributables are not run.
- *Windows' permission prompt*: Heroic always starts the set-up step elevated. The Dashboard starts it as it is, and
  only where Windows answers that it needs an administrator (error 740) asks again through `powershell Start-Process
  -Verb RunAs`, which puts the prompt up. That prompt is on Windows' secure desktop, where no program's input reaches,
  the virtual pad's included: the pad cannot answer it. Ways out, for the user to decide: run the Dashboard elevated
  (no prompt, a bigger blast radius), turn UAC's prompt for administrators down to "without the secure desktop" (then a
  pad mapper can press Yes), or skip the set-up step for games whose build asks for it (most run without it).
- *Pause* stops gogdl (it keeps `.gogdl-resume` in the game's folder), *Resume* runs the download again, *Cancel*
  deletes the folders the install made (those not there before it began) and the manifest and support files.
- *Uninstall* is the game's own `unins000.exe /VERYSILENT /dir=<folder>` (Heroic's, elevated only where Windows asks),
  then what is left of the folder, the manifest, the support files and the record deleted. A GOG game the Dashboard
  did not install through gogdl is uninstalled as before, its uninstaller asking the user.
- *Play* is the game's own program, as for every GOG game (`.info`'s primary play task).
- *Signing in*: GOG's sign-in is a web page (`login.gog.com`, Galaxy's client), which ends on an address carrying a
  code; `gogdl auth --code <code>` turns that into `auth.json`. The Dashboard shows no page, asks for no password and
  keeps none: Settings > Helpers says `Signed in to GOG.` or how to sign in, and the log `helpers: gogdl: signed in` or
  `not signed in`. A pad-driven page (a web view catching the code, or a phone) is the user's decision.

## 6. Tests

- `helpers`: the table's shape; a wrong SHA-256 refused with nothing kept; a right one kept and removed whole; no
  SHA-256, nothing asked; only the plain request sent (`helpers::fake`).
- `shell::helper`: lines from both outputs one by one, carriage returns splitting them, the exit code; a job stopped.
- `app::helper_programs`: a refused fetch, a kept helper read as Added, Remove.
- `screens/tests/settings.rs` `settings_helpers`: focus, the questions, the effects, and three pictures
  (`settings-helpers`, `-question`, `-refused`).
- The payload `e2e/payloads/helpers`: Add legendary from `standin serve-helpers` (standin.exe, which runs as the
  stand-in helper when named `legendary.exe`), its `--version` run and logged; gogdl's address serving other bytes,
  refused; legendary removed.
- `source_kit::downloads`: a run's lines into bytes and rate; pause, resume from what is left, cancel's cleanup.
- `source_epic::legendary`: legendary's lines (its format strings, made-up numbers), its records read.
- `crates/standin/tests/helpers.rs`: Epic's installer driving the stand-in legendary (standin.exe copied as
  `legendary`): sign-in, library, install, pause, resume, cancel, install to the end, egl-sync, start, uninstall.
- The payload `e2e/payloads/epic-legendary`: the same with the pad, against the stand-in legendary.
- `source_gog::gogdl`: gogdl's lines (its format strings, made-up numbers), its sign-in and the Dashboard's record.
- `crates/standin/tests/helpers.rs`: GOG's installer driving the stand-in gogdl: sign-in, download, pause, resume,
  cancel (what was there before kept), to the end with the script interpreter's set-up step, start, uninstall.
- The payload `e2e/payloads/gog-gogdl`: the same with the pad, against the stand-in gogdl.
