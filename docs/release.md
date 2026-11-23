# Releases

How a version of the Dashboard is cut, what CI makes of it, and what ships (#155). **CI publishes nothing.** It makes
a portable zip and an installer as workflow artifacts; making a GitHub release of them, or the repository public, is
the user's step and is never done by a workflow or an agent.

## The version

One number, the workspace's: `version` under `[workspace.package]` in `Cargo.toml`, which every crate takes
(`version.workspace = true`). It is `x.y.z`, nothing after (the installer's version resource takes numbers only).

The executable says it, with the commit it was built from (`crates/app/build.rs`: `DASHBOARD_COMMIT` if CI sets it,
else `git rev-parse --short=7 HEAD`):

- Settings > System > About: "Version 0.1.0 (0123abc)" (docs/spec/settings.md, 16.10);
- `dashboard.exe --version`: `dashboard 0.1.0 (0123abc)`;
- the first lines of every start's log.

## Cutting a release

1. On `main`, with CI green: set the new version in `Cargo.toml`'s `[workspace.package]`, then `cargo metadata
   --format-version 1 >/dev/null` (or any build) to bring `Cargo.lock` along. Commit as `chore(release): 0.2.0`
   through a pull request like any change.
2. Tag the merged commit and push the tag:

   ```sh
   git tag -a v0.2.0 -m "Xbox Series Dashboard 0.2.0" <merged commit>
   git push origin v0.2.0
   ```

3. CI (`.github/workflows/ci.yml`) runs on the tag: Test, Windows (the release build), then **Package**, which fails
   unless the tag is `v` and the workspace's version. Its artifact, `XboxSeriesDashboard-<version>`, holds:
   - `XboxSeriesDashboard-<version>-setup.exe`, the per-user installer;
   - `XboxSeriesDashboard-<version>-portable.zip`, a folder of the same four files;
   - `SHA256SUMS.txt`.
4. Try it: on the TV PC, `e2e/payloads/install` installs, starts and uninstalls it in a folder of its own (below).
5. **Publishing is the user's.** A published release is what the Dashboard's updates find (docs/spec/updates.md): its
   tag, its portable zip and `SHA256SUMS.txt`, under those names, are what an update reads, so a release needs all
   three as CI makes them. To publish, they download the artifact and make the release by hand (`gh release
   create v0.2.0 <files> --draft --latest`, then look and publish). Nothing in this repository does it.

   **Alphas are published as the latest release, not as pre-releases.** The Dashboard's updates ask GitHub for
   `/releases/latest` (`crates/update/src/lib.rs`, `SOURCE`), which never returns a draft or anything marked
   pre-release, so an alpha marked as a pre-release would be invisible to every install. The version is how an alpha
   says what it is: `0.y.z`, still numbers only (the tag parser and the installer take nothing after `x.y.z`).
   GitHub marks the newest published release latest by default; `--latest` makes it explicit, and `--prerelease`
   is never used. Check the published release's page says "Latest" before telling anyone to update.

A package of any commit, without a tag: run CI by hand, `gh workflow run ci.yml --ref <branch>`, or from the
Actions tab. Package then runs after Test and Windows on that branch, and its artifact is named by the workspace's
version as it stands.

Locally (Linux, as CI does it): build for Windows, then

```sh
tools/package.sh target/x86_64-pc-windows-gnullvm/release/dashboard.exe target/package
```

with `makensis` (NSIS 3; Debian and Ubuntu's `nsis` package, or `apt-get download nsis nsis-common` unpacked with
`dpkg-deb -x` and `NSISDIR` set, as CI does without root) and `cargo-about` 0.9 on the path, or named by `$MAKENSIS`
and `$CARGO_ABOUT`.

## What ships, and what does not

Exactly four files; `tools/package.sh` fails if the folder holds anything else.

| File | What |
|---|---|
| `dashboard.exe` | The Dashboard: one exe, which is also shell mode's (`--shell`, `--sentinel`, `--shell-on`, `--shell-off`) |
| `WebView2Loader.dll` | Microsoft's redistributable WebView2 loader, which Discord (#197, docs/discord-web.md) loads at run time from beside `dashboard.exe`. Taken from beside the exe given to `tools/package.sh`, where the Windows build puts it from webview2-com-sys's build; never committed. Without it the Dashboard runs and Discord says there is no runtime |
| `README.txt` | `packaging/README.txt`: installed or portable, where data lives, and how to fill the media folder from a Bundle the user already has |
| `THIRD-PARTY-LICENSES.txt` | Every crate `dashboard.exe` is built from for Windows and its licence, gathered by cargo-about (`packaging/about.toml`, `packaging/about.hbs`). A dependency under a licence not in `about.toml`'s `accepted` list fails the package |

The installed copy has `uninstall.exe` beside them. Never in the package:

- **Fonts.** Segoe UI, Bahnschrift and Segoe Fluent Icons are loaded from Windows at run time.
- **Microsoft's sounds and boot videos** (#10). They are read at run time from the media folder,
  `%LOCALAPPDATA%\XboxSeriesDashboard\media\sounds` and `...\videos`, or from the Bundle where Playnite has it
  (`crates/app/src/media_folder.rs`). The README says which of the Bundle's folders to copy there.
- **Store and Discord marks.** Their places are plain shapes in the code.

The repository's own licence is PolyForm Noncommercial 1.0.0 (`LICENSE.md`). The package carries only the four
files above, so it does not include `LICENSE.md`; `THIRD-PARTY-LICENSES.txt` is for the dependencies.

## The installer

NSIS, `packaging/installer.nsi`, built by `makensis` on the same Linux runner that cross-builds the exe. Chosen over
Inno Setup because NSIS builds on Linux (`makensis` is in Debian and Ubuntu, and unpacks without root), so the
package job runs on the repository's own Linux runner beside the build, taking its artifact, with no Windows runner
(which a private repository pays for by the minute) and no Wine. The script is in the repository and the tools are
pinned (cargo-about by version and checksum; NSIS by the runner's distribution). Built twice from the same files on
the same machine, both the zip and the installer came out byte for byte the same; across NSIS versions they need not.

What it does (docs/research/shell.md, section 8, is the reasoning):

- **Per user, no admin.** `RequestExecutionLevel user`; into `%LOCALAPPDATA%\Programs\XboxSeriesDashboard`, a path
  that never changes between versions, because shell mode names it in `Winlogon\Shell` and the sentinel's Run entry.
  No folder page: `/D=<folder>` on the command line is the only other place (for a test).
- **Writes**: the four files and `uninstall.exe`; the Start menu shortcut `Xbox Series Dashboard.lnk` (the user's
  own Programs folder); `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\XboxSeriesDashboard`
  (`DisplayName`, `DisplayVersion`, `Publisher`, `InstallLocation`, `DisplayIcon`, `UninstallString`,
  `QuietUninstallString`, `NoModify`, `NoRepair`, `EstimatedSize`). Nothing of Winlogon, Run or Task Scheduler: those
  are shell mode's, turned on from Settings > System.
- **Over a running copy**: `dashboard.exe` that can't be deleted is renamed `dashboard.<old version>.old`, and the new
  one written beside it; the running one runs on. Shell mode deletes `dashboard.*.old` at the next sign-in, and the
  uninstaller does too.
- **Uninstall** (Settings > Apps, or `uninstall.exe`): first `dashboard.exe --shell-off`, and if that fails nothing
  else is done (a `Shell` naming a missing exe would leave the user with no desktop). Then the files, the shortcut and
  the uninstall key. A `dashboard.exe` still in use (the `--shell` process of this session) is left to
  `HKCU\...\RunOnce\XboxSeriesDashboardCleanup`, which removes the folder at the next sign-in. The data folder,
  `%LOCALAPPDATA%\XboxSeriesDashboard`, is kept unless **Your settings and library** is ticked on the uninstaller's
  page, or `/REMOVEDATA` given.
- **Command lines**: `setup.exe [/S] [/D=<folder>]` (`/D=` last, unquoted); `uninstall.exe [/S] [/REMOVEDATA]
  [/DATADIR=<folder>] [/DRYSHELL]`. The last two are for the test: the data folder named in place of the user's,
  and `--shell-off --dry-shell`, which writes nothing, its lines kept in `<data>\uninstall-shell-off.log`.

The portable zip is the same four files in a folder `XboxSeriesDashboard-<version>`. It runs from anywhere, and
keeps its data in the same data folder; it can't be the shell (shell mode refuses a copy that is not at the installed
path).

## Signing

Unsigned for now: Windows SmartScreen may ask before an unsigned download first runs ("More info", "Run anyway"), and
whether an unsigned exe named in `Shell` draws antivirus attention at sign-in is unverified.

Where signing plugs in, ready for a certificate: the repository secrets `WINDOWS_SIGNING_CERT` (the PFX, base64)
and `WINDOWS_SIGNING_PASSWORD`. With them set, Package's **Signing** step installs `osslsigncode` and gives
`tools/package.sh` a command (`$SIGN_COMMAND`) that signs a file in place (Authenticode, SHA-256, timestamped); the
script runs it on `dashboard.exe` before packing, and makensis on the installer and on the uninstaller it carries
(`!finalize`, `!uninstfinalize`). Without the secrets, as now, the step is skipped and everything is unsigned. A
certificate is the user's to get.

## Testing a package

`e2e/payloads/install` (on the TV PC; the VM is no longer used). Put the Package artifact's setup.exe beside the
Windows artifact's `dashboard.exe` and `standin.exe` in one folder, then:

```sh
gh run download <run id> -n windows -D target/release-test
gh run download <run id> -n XboxSeriesDashboard-<version> -D target/release-test
e2e/htpc/run.sh install --exe target/release-test/dashboard.exe --standin target/release-test/standin.exe
```

It installs with `/S /D=<a folder under the test's own>`, so the user's copy in `%LOCALAPPDATA%\Programs` is never
touched; checks the files, the uninstall entry and the shortcut; runs the installed `--version` and an offscreen
frame of Settings > About; uninstalls with `/DRYSHELL` (shell mode is never turned on or off) and checks what is
left; then installs again and uninstalls with `/REMOVEDATA`. It refuses to run where an uninstall entry or the Start
menu shortcut is already there (a real install's, which it would replace), and checks that every place shell mode
writes, and RunOnce, is as it was.
