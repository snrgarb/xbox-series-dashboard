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

Games & apps > **Helpers** (U+E896), in the Installs section under Storage ([settings.md](settings.md) 6.1). One line about what helpers are, then a group a
helper, as About's: its name and store ("legendary for Epic Games"), "Version 0.21.1. GPL-3.0 licence. Source:
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

`shell::helper::Job::start(program, args, wake)` starts a helper in its own folder, with no window on Windows
(`CREATE_NO_WINDOW`), its standard output and error piped. A thread for each reads it a line at a time, a carriage
return ending a line too (a progress line rewrites itself with one), and hands each over through a channel
(`Said::Out`, `Said::Err`), calling `wake` so a sleeping UI thread looks; once the process has ended and both are read to their ends (or a
second more, where a process it started still holds them open), `Said::Ended(code)`. Nothing is read on the UI thread. `Job::stop()` kills the process (both helpers resume from what
they kept, so pause is a stop); its end is said as any end is, with no code. A job dropped is stopped.

## 5. Tests

- `helpers`: the table's shape; a wrong SHA-256 refused with nothing kept; a right one kept and removed whole; no
  SHA-256, nothing asked; only the plain request sent (`helpers::fake`).
- `shell::helper`: lines from both outputs one by one, carriage returns splitting them, the exit code; a job stopped.
- `app::helper_programs`: a refused fetch, a kept helper read as Added, Remove.
- `screens/tests/settings.rs` `settings_helpers`: focus, the questions, the effects, and three pictures
  (`settings-helpers`, `-question`, `-refused`).
- The payload `e2e/payloads/helpers`: Add legendary from `standin serve-helpers` (standin.exe, which runs as the
  stand-in helper when named `legendary.exe`), its `--version` run and logged; gogdl's address serving other bytes,
  refused; legendary removed.
