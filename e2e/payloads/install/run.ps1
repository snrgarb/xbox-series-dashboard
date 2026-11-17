# The installer (#155; docs/release.md): installs silently into a folder of the test's own, starts the installed
# Dashboard offscreen and reads the version it reports, uninstalls, and looks at what is left; then once more,
# uninstalling with the data folder. Safe on the TV PC: nothing goes to %LOCALAPPDATA%\Programs, where the user's own
# copy is, and shell mode is never turned on or off (the uninstaller's --shell-off is a dry run here, /DRYSHELL).
#
# Stage the installer beside the dashboard.exe given to the harness: CI's Package artifact holds
# XboxSeriesDashboard-<version>-setup.exe, its Windows artifact dashboard.exe and standin.exe; unpack both into one
# folder and run `e2e/htpc/run.sh install --exe <folder>/dashboard.exe --standin <folder>/standin.exe`. The
# staged dashboard.exe itself is not run: the installed one is.
#
# The one write this test lets through outside its own folder is the installer's own: the per-user uninstall entry,
# HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\XboxSeriesDashboard, and the Start menu shortcut
# "Xbox Series Dashboard.lnk", which the uninstaller removes and this test checks are gone. So it refuses to run
# where either is there already: that is a real install's, which it would replace and then remove.
#
# Passes when:
#   - `setup.exe /S /D=<test folder>` ends 0 having written dashboard.exe, README.txt, THIRD-PARTY-LICENSES.txt and
#     uninstall.exe there and nothing else (no font, sound, video or picture), the uninstall entry (DisplayVersion
#     the installer's version, InstallLocation the folder, UninstallString its uninstall.exe, NoModify, NoRepair),
#     and the Start menu shortcut to the installed dashboard.exe;
#   - the installed `dashboard.exe --version` prints `dashboard <version> (<commit>)` and ends 0, the version the
#     installer's own;
#   - the installed Dashboard, offscreen (--screenshot) on Settings > About, logs `dashboard <version> ...` and
#     `focus: Settings: About: Version <version> (<commit>); ...`, the same commit, writes its picture and ends 0;
#   - `uninstall.exe /S /DRYSHELL /DATADIR=<test data>` runs `dashboard.exe --shell-off --dry-shell` first (its lines
#     kept in <test data>\uninstall-shell-off.log: a dry run, exit 0), then removes the folder, the shortcut and the
#     uninstall entry, leaves no RunOnce entry, and keeps the data folder;
#   - installed again and uninstalled with /REMOVEDATA, the data folder goes too;
#   - every place shell mode writes, and RunOnce, are as they were before.
#
# Bounded: the whole payload ends within $Budget seconds whatever happens.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$Budget = 300
$clock = [Diagnostics.Stopwatch]::StartNew()
$started = @()

$UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\XboxSeriesDashboard'
$Shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Xbox Series Dashboard.lnk'
$RealInstall = Join-Path $env:LOCALAPPDATA 'Programs\XboxSeriesDashboard'

# Seconds left of $Want, within what is left of the budget (a few kept for the end).
function Get-Limit([double] $Want) { [Math]::Max(1, [Math]::Min($Want, $Budget - 10 - $clock.Elapsed.TotalSeconds)) }

function Expect([bool] $Ok, [string] $What) {
    Say "$(if ($Ok) { 'ok' } else { 'NOT' }): $What"
    if (-not $Ok) { $script:failures += $What }
}

# Runs $Exe with $Arguments, its output in <name>.log and <name>.out.log, and waits at most $Seconds; its exit code
# (or 'timeout') and its lines.
function Invoke-Bounded([string] $Exe, [string] $Name, [string[]] $Arguments, [double] $Seconds = 60) {
    $p = Start-Process -FilePath $Exe -ArgumentList $Arguments -WorkingDirectory $Payload -NoNewWindow -PassThru `
        -RedirectStandardError (Join-Path $Out "$Name.log") -RedirectStandardOutput (Join-Path $Out "$Name.out.log")
    $p.Handle | Out-Null
    $script:started += $p.Id
    $limit = Get-Limit $Seconds
    $code = if ($p.WaitForExit([int] ($limit * 1000))) { $p.ExitCode } else {
        Say "$Name didn't end within $([int] $limit) s"
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        'timeout'
    }
    $lines = @(Get-Content (Join-Path $Out "$Name.log") -ErrorAction SilentlyContinue) +
             @(Get-Content (Join-Path $Out "$Name.out.log") -ErrorAction SilentlyContinue)
    Say "$Name ($($Arguments -join ' ')): exit $code, $($lines.Count) lines"
    $lines | ForEach-Object { Say "  $_" }
    [pscustomobject]@{ Code = $code; Lines = $lines }
}

# One HKCU value as text, or 'none'.
function Get-UserValue([string] $Key, [string] $Name) {
    $k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Key)
    $value = if ($k) { try { $k.GetValue($Name) } finally { $k.Dispose() } } else { $null }
    if ($null -ne $value) { "$value" } else { 'none' }
}

# Every place shell mode writes, and RunOnce's, as text: what must be the same after as before.
function Get-Places {
    @(
        @('Software\Microsoft\Windows NT\CurrentVersion\Winlogon', 'Shell'),
        @('Software\Microsoft\Windows\CurrentVersion\Run', 'XboxSeriesDashboardSentinel'),
        @('Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement', 'ScoobeSystemSettingEnabled'),
        @('Software\Microsoft\Windows\CurrentVersion\RunOnce', 'XboxSeriesDashboardCleanup')
    ) | ForEach-Object { "HKCU\$($_[0])\$($_[1]) = $(Get-UserValue $_[0] $_[1])" }
    $machine = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey('Software\Microsoft\Windows NT\CurrentVersion\Winlogon')
    "HKLM\...\Winlogon\Shell = $(if ($machine) { try { $machine.GetValue('Shell') } finally { $machine.Dispose() } })"
}

# Whether the installer's uninstall entry is there.
function Test-UninstallEntry {
    $k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($UninstallKey)
    if ($k) { $k.Dispose(); $true } else { $false }
}

# Installs into the test's folder; the setup's exit code.
function Install([string] $Name) {
    # NSIS's /D= is the last argument, unquoted, spaces and all.
    (Invoke-Bounded $setup.FullName $Name @('/S', "/D=$installed") 120).Code
}

# Uninstalls with $Arguments, and waits for it: the uninstaller runs from a copy of itself in %TEMP% and returns at
# once, so it is done when its entry and its own file are gone.
function Uninstall([string] $Name, [string[]] $Arguments) {
    $run = Invoke-Bounded (Join-Path $installed 'uninstall.exe') $Name $Arguments 30
    $done = Wait-Until { -not (Test-UninstallEntry) -and -not (Test-Path (Join-Path $installed 'uninstall.exe')) } (Get-Limit 90)
    Say "${Name}: $(if ($done) { 'done' } else { 'not done in time' }) after $([int] $clock.Elapsed.TotalSeconds) s"
    # The copy in %TEMP% may still be closing: a moment for it to delete the folder it emptied.
    Wait-Until { -not (Test-Path $installed) } 5 | Out-Null
    $run
}

try {
    $setup = @(Get-ChildItem $Payload -Filter 'XboxSeriesDashboard-*-setup.exe')
    if ($setup.Count -ne 1) { throw "stage exactly one XboxSeriesDashboard-<version>-setup.exe beside dashboard.exe (found $($setup.Count))" }
    $setup = $setup[0]
    if ($setup.Name -notmatch '^XboxSeriesDashboard-(\d+\.\d+\.\d+)-setup\.exe$') { throw "no version in $($setup.Name)" }
    $version = $Matches[1]
    Say "installer $($setup.Name), version $version"

    $installed = Get-TestFolder 'installed'
    $data = Get-TestFolder 'install-data'
    foreach ($folder in $installed, $data) { if (Test-Path $folder) { Remove-Item $folder -Recurse -Force } }
    if ([IO.Path]::GetFullPath($installed).TrimEnd('\') -ieq [IO.Path]::GetFullPath($RealInstall).TrimEnd('\')) {
        throw "the test's install folder is the real one: $installed"
    }
    if (Test-UninstallEntry) { throw "HKCU\$UninstallKey is there: a real install's, which this test would replace and remove. Not run." }
    if (Test-Path $Shortcut) { throw "$Shortcut is there: a real install's, which this test would replace and remove. Not run." }
    New-Item $data -ItemType Directory -Force | Out-Null
    $before = Get-Places
    $before | Set-Content (Join-Path $Out 'places-before.txt') -Encoding UTF8

    # --- Install.
    $code = Install 'setup'
    Expect ($code -eq 0) "setup /S /D=<test folder> ends 0 (got $code)"
    $files = @(Get-ChildItem $installed -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName.Substring($installed.Length + 1) } | Sort-Object)
    Say "installed: $($files -join ', ')"
    Expect (($files -join ',') -eq 'dashboard.exe,README.txt,THIRD-PARTY-LICENSES.txt,uninstall.exe') 'the folder holds dashboard.exe, README.txt, THIRD-PARTY-LICENSES.txt and uninstall.exe, and nothing else'
    Expect (Test-UninstallEntry) 'the uninstall entry is written'
    $entry = @{}
    foreach ($name in 'DisplayName', 'DisplayVersion', 'InstallLocation', 'UninstallString', 'NoModify', 'NoRepair') {
        $entry[$name] = Get-UserValue $UninstallKey $name
        Say "  $name = $($entry[$name])"
    }
    Expect ($entry.DisplayVersion -eq $version) "its DisplayVersion is $version"
    Expect ($entry.InstallLocation -ieq $installed) 'its InstallLocation is the test folder'
    Expect ($entry.UninstallString -ieq "`"$(Join-Path $installed 'uninstall.exe')`"") 'its UninstallString is the folder''s uninstall.exe'
    Expect ($entry.NoModify -eq '1' -and $entry.NoRepair -eq '1') 'NoModify and NoRepair are 1'
    $target = if (Test-Path $Shortcut) { (New-Object -ComObject WScript.Shell).CreateShortcut($Shortcut).TargetPath } else { 'none' }
    Say "shortcut $Shortcut -> $target"
    Expect ($target -ne 'none' -and [E2EDesk]::LongPath($target) -ieq [E2EDesk]::LongPath((Join-Path $installed 'dashboard.exe'))) 'the Start menu shortcut starts the installed dashboard.exe'

    # --- The installed Dashboard: its version, and Settings > About offscreen.
    $exe = Join-Path $installed 'dashboard.exe'
    $said = Invoke-Bounded $exe 'version' @('--version') 30
    $line = @($said.Lines | Where-Object { $_ -match '^dashboard ' }) | Select-Object -First 1
    Expect ($said.Code -eq 0) "--version ends 0 (got $($said.Code))"
    Expect ($line -match "^dashboard $([regex]::Escape($version))( \(([0-9a-f]{7,})\))?$") "--version says dashboard $version (got '$line')"
    $commit = $Matches[2]
    $about = "Version $version$(if ($commit) { " ($commit)" })"
    $arguments = @('--screenshot', "`"$(Join-Path $Out 'about.png')`"", '--page', 'settings',
        '--press', 'right,down,down,down,down,right,a', '--log-focus', '--data-dir', "`"$data`"",
        '--store-art-service', 'http://127.0.0.1:9', '--art-service', 'http://127.0.0.1:9')
    if (Test-SafeMode) { $arguments += @(Get-SafeArguments $arguments) }
    $shot = Invoke-Bounded $exe 'about' $arguments 90
    Expect ($shot.Code -eq 0) "the offscreen Dashboard ends 0 (got $($shot.Code))"
    Expect ([bool] ($shot.Lines -match "^dashboard $([regex]::Escape($version))")) 'it says its version as it starts'
    $aboutLine = '^focus: Settings: About: ' + [regex]::Escape($about) + ';'
    Expect ([bool] ($shot.Lines -match $aboutLine)) "Settings > About says $about"
    Expect (Test-Path (Join-Path $Out 'about.png')) 'it wrote its picture'

    # --- Uninstall, keeping the data folder.
    'kept' | Set-Content (Join-Path $data 'kept.txt')
    Uninstall 'uninstall' @("/DATADIR=$data", '/S', '/DRYSHELL') | Out-Null
    $off = @(Get-Content (Join-Path $data 'uninstall-shell-off.log') -ErrorAction SilentlyContinue)
    Say "uninstall-shell-off.log: $($off -join ' | ')"
    Expect ($off.Count -gt 0 -and $off[0] -eq 'exit 0') 'the uninstaller ran dashboard.exe --shell-off first, and it ended 0'
    Expect ([bool] ($off -match '^shell: a dry run \(--dry-shell\): nothing is written')) '--shell-off was a dry run'
    Expect (-not (Test-Path $installed)) 'the install folder is gone'
    Expect (-not (Test-UninstallEntry)) 'the uninstall entry is gone'
    Expect (-not (Test-Path $Shortcut)) 'the Start menu shortcut is gone'
    Expect (Test-Path (Join-Path $data 'kept.txt')) 'the data folder is kept'

    # --- Again, and uninstall with the data folder.
    $code = Install 'setup-again'
    Expect ($code -eq 0 -and (Test-Path $exe)) "installed again (exit $code)"
    Uninstall 'uninstall-with-data' @("/DATADIR=$data", '/S', '/DRYSHELL', '/REMOVEDATA') | Out-Null
    Expect (-not (Test-Path $installed)) 'the install folder is gone again'
    Expect (-not (Test-UninstallEntry)) 'the uninstall entry is gone again'
    Expect (-not (Test-Path $data)) 'with /REMOVEDATA the data folder is gone'

    $after = Get-Places
    $after | Set-Content (Join-Path $Out 'places-after.txt') -Encoding UTF8
    $changed = @(Compare-Object $before $after)
    Expect ($changed.Count -eq 0) 'every place shell mode writes, and RunOnce, is as it was'
    $changed | ForEach-Object { Say "  changed: $($_.SideIndicator) $($_.InputObject)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
} finally {
    foreach ($id in $started) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    # Whatever happened, nothing of the installer's is left: its entry and shortcut only if this test made them
    # (it refuses to start where they were), then its folder.
    if ($installed -and (Test-Path (Join-Path $installed 'uninstall.exe'))) {
        Say 'cleaning up: uninstalling what is left'
        try { Uninstall 'cleanup' @("/DATADIR=$data", '/S', '/DRYSHELL') | Out-Null } catch { Say "cleanup: $_" }
    }
    if ($installed -and (Test-Path $installed)) { Remove-Item $installed -Recurse -Force -ErrorAction SilentlyContinue }
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result ($([int] $clock.Elapsed.TotalSeconds) s)"
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
