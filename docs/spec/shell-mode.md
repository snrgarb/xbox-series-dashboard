# Shell mode

The Dashboard in place of the Windows desktop at sign-in, as a console starts: what the Bundle's Boot shell did for
Playnite (`~/projects/xbox-series-theme`, `bootshell/`), folded into `dashboard.exe` (#154). The research behind every
choice is `docs/research/shell.md`; this page says what was built. The code is `crates/shell-mode`.

## 1. One exe, several jobs

`dashboard.exe` is the Dashboard. Run with one of these as its first argument it does one of shell mode's jobs instead,
before anything of the Dashboard's is set up (`shell_mode::run_job`):

| Argument | Who runs it | What it does |
|---|---|---|
| `--shell` | Winlogon, from `HKCU\…\Winlogon\Shell` | The Boot shell's job for the whole session (section 5). |
| `--sentinel` | Explorer's startup run, `HKCU\…\Run\XboxSeriesDashboardSentinel` | Writes `Shell` back if Windows dropped it, and starts the Dashboard (section 7). |
| `--shell-on`, `--shell-off` | Settings > System, or a terminal | Turns shell mode on or off from the next sign-in (section 3). |
| `--shell-probe` | `--shell-on` | Prints `dashboard: --shell-probe ok` and exits `0x58440002`: the exe starts. |

`dashboard.exe` is a GUI-subsystem program: however it is started (Winlogon, the Run key, Explorer, a shortcut, another
program with a plain `CreateProcess`) no console window opens for it. Run from a terminal, it attaches to that
terminal's console for whichever of its output and errors aren't already going to a file or a pipe, so `--shell-on`'s
lines and the logs print there; `--shell` and `--sentinel` never attach. cmd and PowerShell don't wait for a GUI
program: `start /wait dashboard.exe --shell-on`, or `Start-Process -Wait -NoNewWindow`, to wait and read its exit code
(`echo %errorlevel%`, `$p.ExitCode`).

Every start leaves a log. Where the Dashboard's stderr goes nowhere (a start by Explorer, a shortcut, another program
that gives it no handles), its lines go to `<data>\logs\dashboard-<stamp>.log`, the newest 30 kept; `--log-file
<path>` appends them to that file instead, however it is started (and under `--from-shell` too). Under `--from-shell`
without it they go to `<data>\shell\logs\dashboard-<stamp>.log` (section 6). A terminal, a file or a pipe it was given
keeps them.

Exit codes: `--shell-on` and `--shell-off` end 0 (done, or so already), 3 (refused), 4 (failed) or 2 (bad arguments).
`--sentinel`, a second `--shell` and a second Dashboard end `0x58440001`, which the watchdog never takes for a
Dashboard's exit.

## 2. Settings > System

The System category's first box is **System** (icon U+E770), the port's own, About beside it ([settings.md](settings.md) 6.1). Its page, laid out as Discord's
(settings.md, 6.9): a label, its description, the control, the next group 40 under.

- **Start with Windows** — "Starts the Dashboard in place of the Windows desktop when you sign in, as a console starts.
  The desktop still starts behind it once the Boot video ends. A change takes effect the next time you sign in."
- The checkbox row **Start with Windows as the shell**, ticked while it is on, and under it a line saying where it stands:

  | Stands | Line |
  |---|---|
  | Off | "Off. Windows starts its desktop when you sign in." |
  | On | "On. The Dashboard starts in place of the desktop when you sign in." |
  | Refused | the refusal's own words (section 4) |
  | Failed | "Couldn't change it, and nothing was left half done. The Dashboard's log says why." |
  | Unknown | "Shell mode is only on Windows." (and until the first look has answered) |

- **Getting to the desktop** — "Choose Exit to desktop in the Guide's Power, or hold View and Menu for 5 seconds. Press
  the Guide button to come back to the Dashboard."
- **If the Dashboard won't start** — "The desktop starts by itself within two minutes. Or press Ctrl+Shift+Esc, choose
  Run new task and enter explorer.exe. Then turn this off here, or run dashboard.exe --shell-off."

A on the row while it is off asks first, over the dimmed page as Art's Clear asks: **Start with Windows as the shell?**
"From your next sign-in, the Dashboard starts in place of the Windows desktop. To get to the desktop, choose Exit to
desktop or hold View and Menu for 5 seconds." with **Turn on** and **Cancel**. A while it is on turns it off at once:
off is the way back, and is never asked. A while it is refused or unknown does nothing; the line says why.

The page asks with `Effect::ShellMode(on)`; the executable does it on a thread of its own (`setup::turn_on` is slow:
schtasks and the probe) and answers in `Options::shell`, logging `shell: Settings > System shows on`. Where it stands is
looked up the same way as the Dashboard starts. The focus line is `System: Start with Windows as the shell`, or with
the question up `System: Prompt(Turn on) of Start with Windows as the shell`.

With `--dry-shell` the Dashboard's Settings does a dry run (section 3.3), and a dry run's Done shows as done.

## 3. Turning on and off

Paths: the installed exe `%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe`; the data folder
`%LOCALAPPDATA%\XboxSeriesDashboard` (`--data-dir` in its place); shell mode's folder `<data>\shell`, its record
`<data>\shell\record.json`, its logs `<data>\shell\logs`.

### 3.1 On (`--shell-on`)

1. Refusals (section 4). Any one: nothing is written, exit 3.
2. The installed exe is there and answers `--shell-probe` within 20 s. Else nothing is written, exit 4.
3. The four steps, in this order. Before a step's first write its value as it was is saved to the record, so a
   failure at any point can put it back:
   1. Task `XboxSeriesDashboard.SafetyNet`: a logon trigger for this user, 90 s delay, interactive token, least
      privilege; `%SystemRoot%\System32\conhost.exe --headless …\powershell.exe -NoProfile -NonInteractive -Command
      "<if no explorer in this session: start explorer.exe and add a line to <logs>\safety-net.log>"`. Never this exe,
      so it covers a missing or broken one.
   2. `HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement\ScoobeSystemSettingEnabled` = DWORD 0.
   3. `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\XboxSeriesDashboardSentinel` = `"<install>\dashboard.exe" --sentinel`.
   4. Last: `HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell` = `"<install>\dashboard.exe" --shell`,
      then read back.
4. A step that fails, or a `Shell` that reads back otherwise: every step written is put back as the record had it,
   in the order sentinel, `Shell`, SCOOBE, task (what was absent is deleted), the record renamed
   `record.failed-<stamp>.json`, exit 4. One that can't be put back is said with how to do it by hand, and the record
   is kept for `--shell-off`.

A step already holding what it would write is left alone. On twice: `shell: on already`, nothing written.

### 3.2 Off (`--shell-off`)

From the record, in the order sentinel, `Shell`, SCOOBE, task: each back to its value before (deleted if there was
none). A value that holds neither shell mode's nor the one before was changed by someone else, and is left alone and
said. Then the record is renamed `record.<stamp>.json`.

- The sentinel can't be taken off: nothing else is touched, shell mode stays on whole.
- `Shell` can't be put back: the sentinel is put back too, so it stays on whole; the line says how by hand.
- SCOOBE or the task can't: `Shell` is off, the record is kept, so `--shell-off` can try again (exit 4).
- No record (lost): only what names this exe is deleted, the sentinel first, then `Shell`, then the task. SCOOBE is
  left: its 0 may have been the user's.

The running `--shell` process runs on until sign-out.

### 3.3 The lines

Every line starts `shell: `. A real change goes to stderr and is appended to `<logs>\setup.log`. With `--dry-shell`,
everything is read for real and nothing is written anywhere, the record included; each write is said as it would be:

```
shell: a dry run (--dry-shell): nothing is written
shell: turning on for C:\Users\<user>\AppData\Local\Programs\XboxSeriesDashboard\dashboard.exe
shell: C:\…\dashboard.exe --shell-probe answered
shell: would save the record C:\…\XboxSeriesDashboard\shell\record.json with Task XboxSeriesDashboard.SafetyNet as none
shell: would register Task XboxSeriesDashboard.SafetyNet: at sign-in + 90 s: C:\Windows\System32\conhost.exe --headless …
shell: would save the record … with HKCU\…\UserProfileEngagement\ScoobeSystemSettingEnabled as none
shell: would set HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement\ScoobeSystemSettingEnabled = 0 (DWORD) (now none)
shell: would set HKCU\Software\Microsoft\Windows\CurrentVersion\Run\XboxSeriesDashboardSentinel = ""C:\…\dashboard.exe" --sentinel" (now none)
shell: would set HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell = ""C:\…\dashboard.exe" --shell" (now none)
shell: would be on from the next sign-in: Windows starts "C:\…\dashboard.exe" --shell
```

Without `--dry-shell` the same lines read `set`, `register`, `saved`, `on from the next sign-in`. A refusal is
`shell: refused: <key>: <words>` then `shell: nothing was written`. `--as-installed`, only with `--dry-shell`, takes
the running exe for the installed one, so a PC with nothing installed (the TV's test folder) can be dry-run.

## 4. Refusals

Shell mode isn't turned on, nothing is written, and the page says why, when:

| Key | When | Says |
|---|---|---|
| `bundle-boot-shell` | `Shell` names `XboxSeries.BootShell.exe`, or `Run\XboxSeriesBootShellSentinel` or the task `XboxSeries.BootShell.SafetyNet` exists (research F11) | "The Xbox Series Theme's Boot shell is set up on this PC, and would take it back at the next sign-in. Remove it first with XboxSeries.BootShell.exe --uninstall." |
| `other-shell` | HKCU `Shell` names another program, not explorer | "Windows already starts another program in place of the desktop for your account. Turn that off first." |
| `machine-shell` | HKLM `Winlogon\Shell` isn't explorer (F8) | "This PC's own shell isn't the Windows desktop, so there would be no desktop to come back to. An administrator has to set it back to explorer.exe first." |
| `shell-overridden` | HKLM `PolicyManager\…\WindowsLogon\OverrideShellProgram` is set (F9) | "Your organisation sets what this PC starts at sign-in, so it can't be changed here." |
| `not-installed` | This copy isn't the installed one: a portable copy would leave `Shell` naming a folder the user may move | "This copy of the Dashboard isn't the installed one. Install it, then turn this on from there." |

The words are `Refusal::says` in `crates/model/src/shell_mode.rs`. HKLM is only ever read. Settings shows the first
found; `--shell-on` says each.

## 5. The `--shell` process

What Winlogon starts. It never touches the GPU, the library or the stores (`crates/shell-mode/src/boot.rs`).

1. Holds `Local\XboxSeriesDashboard.Shell`; a second `--shell` in the session exits.
2. A black, topmost, click-through window over every screen, on a thread of its own with its own 60 s limit: if
   the main loop stops renewing it, it takes itself down, so a hung `--shell` never covers the desktop the safety net
   brings up. Then `ShellDesktopSwitchEvent`, so Welcome gives way to the black.
3. Makes the events `Local\XboxSeriesDashboard.v1.FirstFrame` and `…v1.BootVideoEnded`, unset.
4. Waits for Windows' default audio device to settle: started once the same device has been there 1.5 s, or at 6 s.
5. Starts `dashboard.exe --from-shell` from its own folder (the file beside it, so an update's renamed-aside build
   still starts the new one), then opens its log `<data>\shell\logs\shell-<stamp>.log` (newest 30 kept) and logs
   Windows' build, HKLM `Shell`, `OverrideShellProgram` and HKCU `Shell`.
6. Then every 100 ms until Windows ends the session:
   - **The events.** FirstFrame: the black comes down. BootVideoEnded: explorer starts. Explorer also starts 20 s
     after FirstFrame, or 60 s after sign-in, whichever comes first, and never while a shell window exists.
   - **The watchdog**, over every `dashboard.exe` at that path in this session, by image path:
     - exit 0 with none left: Exit to desktop. It stays down; explorer starts if there is no shell window.
     - exit 0 with another left: a handover; the other is watched.
     - any other code, or an exit before that start's FirstFrame: a crash. Relaunched under the black. The third in
       5 minutes shows "The Dashboard closed 3 times in 5 minutes. Press the Guide button to try again." for 5 s,
       then the desktop, and nothing more is relaunched on its own.
     - `0x58440001` (a sentinel, a second Dashboard) counts for nothing, and so does any exit of one it didn't start
       that never had the Dashboard's window (titled "Xbox Series Dashboard"): `--shell-off` from a terminal, the
       probe. One it didn't start is a Dashboard from its window on (the Start menu's, an update's handover).
   - **The pad** (XInput, every 30 ms, its own thread). The Guide button while no Dashboard runs starts one and
     resets the crash count. View+Menu held 5 s: `WM_CLOSE` to the Dashboard's windows, ended after 10 s, and it
     stays down (Exit to desktop).
   - **The shell window.** None for 10 s after explorer had one: explorer is started again.
   - **The black's 60 s limit** in the main loop: no FirstFrame 60 s after a launch, the black comes down.
7. `WM_QUERYENDSESSION` (or `SM_SHUTTINGDOWN`): watching stops, so a Turn off or Restart is never taken for a crash.
8. A panic: the black comes down and explorer starts.

## 6. The Dashboard's side

- **One at a time.** `Local\XboxSeriesDashboard.Running` (with `--data-dir`, a name of its own:
  `…Running.<FNV-1a of the folder>`, so a test's Dashboard never gives way to the user's). A second start brings the
  first's window forward and exits `0x58440001`.
- **Under `--from-shell`:**
  - its stderr goes to `<data>\shell\logs\dashboard-<stamp>.log` from its first line;
  - FirstFrame is set as its first frame is presented (the Boot video's first frame, or Start-up's or Home's with
    none), BootVideoEnded as the video ends, is skipped or fails, or with the first frame when none plays
    (`--no-startup` included);
  - the window is topmost while Start-up shows, so explorer and the startup apps coming up behind it never draw over
    Who's playing today?, and a window among others from Home's first frame. The window is made plain, as without
    `--from-shell`, and is put topmost only once its swap chain is made: nothing of shell mode's is on it while DXGI
    makes the swap chain;
  - the test's crash on purpose: `<data>\shell\test-crash-after-first-frame` holding a count. Each start under
    `--from-shell` that finds it counts it down and exits `0x58440DEA` right after its first frame. Nothing else writes
    it; without `--from-shell` it is never read.
  - `--from-shell --dry-shell` is the window's path without being the shell, for a test: it logs as above and is put
    topmost for Start-up, but leaves the events alone (`shell: started by the shell; --dry-shell: the events are left
    alone ...`) and never reads the crash marker.
- **A swap chain refused** (any start): never a panic. `window: ERROR: the swap chain couldn't be made: <wgpu's error>;
  window <hwnd>: style, extended style, client size, visible, minimised, owner, parent, desktop`, then `window: trying
  once more on a new plain window; dropped: <what>` and, if that works, `window: the plain window's swap chain was
  made`. A second refusal logs `window: ERROR: again: ...` and exits 2, which the watchdog counts as a crash.
- **Packaged games** (`shell:AppsFolder\<AUMID>`, the Xbox source's and Store apps) are activated with
  `IApplicationActivationManager::ActivateApplication`, which works with no shell window; `ShellExecuteW` if it fails.
- Exit to desktop is unchanged: exit 0.

## 7. Sentinel and safety net

- **Sentinel** (`--sentinel`, explorer's startup run each sign-in): with the `--shell` mutex held, nothing. With none,
  and a record, explorer was the shell from the start (a Windows upgrade dropped `Shell`, or `--shell` crashed early):
  `Shell` is written back if it differs, and the Dashboard started with `--from-shell` unless one runs. Without a
  record it does nothing, so a sentinel left behind can't turn shell mode on. Its lines go to `<logs>\sentinel.log`.
- **Safety net**: the task, 90 s after sign-in, starts explorer if none runs in the session.

## 8. The way back

As research section 6. In short: Exit to desktop or View+Menu 5 s; with a keyboard Ctrl+Shift+Esc → Run new task →
`explorer.exe`, then `"%LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe" --shell-off`, or by hand
`reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v XboxSeriesDashboardSentinel /f` then
`reg delete "HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon" /v Shell /f`.

## 9. Not here yet

- The update handover (`--handover`, `.old` removal) is updates.md's (#156), section 5.1 there for shell mode.
- No hang detection beyond View+Menu, as in the Bundle.

## 10. Tests

- `cargo test -p shell-mode`: every write through `system::Fake`; nothing touches a real registry or task.
- `e2e/payloads/shell-dry` (safe, TV and VM): `--shell-on --dry-shell --as-installed` and `--shell-off --dry-shell`,
  judged by their lines, and that nothing changed; then `--from-shell --dry-shell`, which must log `first frame
  presented` to `<data>\shell\logs\dashboard-<stamp>.log` with no `window: ERROR`: the window as the shell starts it,
  checked on any machine before shell-mode restarts one.
- `e2e/payloads/shell-mode` (VM only; refuses in safe mode): P1–P9 across restarts (e2e/README.md).
