# Shell mode for real, across restarts (docs/spec/shell-mode.md; docs/research/shell.md, section 9): the VM only. It
# refuses safe mode: the TV PC never has shell mode turned on by a test (its test is shell-dry).
#
#   e2e/run.sh shell-mode --timeout 3600
#
# The payload runs in phases, one per sign-in. Each phase registers the next as a logon task for this user, keeps what
# it needs in C:\e2e\shell-mode\state.json, and restarts the VM; it never returns before the restart, so the harness's
# boot.ps1 sends nothing back. The last phase sends C:\e2e\out itself, to $env:E2E_UPLOAD. Every line of every phase is
# in steps.txt; shell mode's own logs are copied to shell-logs\ at the end.
#
#   1 (explorer)   P2 refusals; P1 dry run, then on for real, read back; restart
#   2 (--shell)    P3 the sign-in; P4 Exit to desktop, Guide, View+Menu; P5 crash three times; P6b kill --shell after
#                  explorer; P7 delete Shell; restart
#   3 (explorer)   P7 the sentinel writes Shell back and starts the Dashboard; restart
#   4 (--shell)    P6a kill --shell as soon as it is seen: the safety net and the sentinel; P8 rename the running exe
#                  aside and put a copy in its place; restart
#   5 (--shell)    P8 the copy runs as the shell, the old one deleted; P9 off for real, read back; restart
#   6 (explorer)   P9 explorer is the shell from the start and no Dashboard runs; the end
#
# Judged by log lines (shell-*.log, dashboard-*.log, sentinel.log, safety-net.log, setup lines), never by sleeps alone.
$ErrorActionPreference = 'Stop'
$root = 'C:\e2e\shell-mode'
$stateFile = Join-Path $root 'state.json'
New-Item $root -ItemType Directory -Force | Out-Null
$state = if (Test-Path $stateFile) { Get-Content $stateFile -Raw | ConvertFrom-Json } else {
    [pscustomobject]@{ phase = 1; out = $env:E2E_OUT; upload = $env:E2E_UPLOAD; failures = @(); before = $null }
}
# A later phase runs from a logon task, with none of the harness's environment.
$env:E2E_OUT = $state.out
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
Assert-VmOnly 'it turns shell mode on and restarts the machine'
$failures = @($state.failures | Where-Object { $_ })

$user = "$env:USERDOMAIN\$env:USERNAME"
$data = Join-Path $env:LOCALAPPDATA 'XboxSeriesDashboard'
$logs = Join-Path $data 'shell\logs'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\XboxSeriesDashboard'
$installed = Join-Path $installDir 'dashboard.exe'
$shellKey = 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Winlogon'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$scoobeKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement'
$machineKey = 'HKLM:\Software\Microsoft\Windows NT\CurrentVersion\Winlogon'
$policyKey = 'HKLM:\SOFTWARE\Microsoft\PolicyManager\current\device\WindowsLogon'
# This sign-in's logs are those written since the machine started.
$booted = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime
# sentinel.log runs on across sign-ins: this phase reads only what came after the last phase's lines.
$sentinelMark = if ($state.PSObject.Properties['sentinelMark']) { [int] $state.sentinelMark } else { 0 }

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class E2EShell {
    [DllImport("user32.dll")] public static extern IntPtr GetShellWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    public static uint ShellPid() { IntPtr h = GetShellWindow(); if (h == IntPtr.Zero) return 0; uint p; GetWindowThreadProcessId(h, out p); return p; }
}
'@

function Expect([bool] $Ok, [string] $What) {
    Say "$(if ($Ok) { 'ok' } else { 'NOT' }): $What"
    if (-not $Ok) { $script:failures += "phase $($state.phase): $What" }
}

function Save-State { $state.failures = @($script:failures); $state | ConvertTo-Json -Depth 6 | Set-Content $stateFile -Encoding UTF8 }

# A value as text, "none" for none: what is compared before and after.
function Get-Value([string] $Key, [string] $Name) {
    $item = Get-ItemProperty $Key -Name $Name -ErrorAction SilentlyContinue
    if ($null -eq $item) { return 'none' }
    "$($item.$Name)"
}

function Test-Task([string] $Name) {
    & cmd.exe /c "schtasks /Query /TN $Name >nul 2>nul"
    $LASTEXITCODE -eq 0
}

function Get-Places {
    [ordered]@{
        shell = Get-Value $shellKey 'Shell'
        sentinel = Get-Value $runKey 'XboxSeriesDashboardSentinel'
        scoobe = Get-Value $scoobeKey 'ScoobeSystemSettingEnabled'
        task = Test-Task 'XboxSeriesDashboard.SafetyNet'
        machineShell = Get-Value $machineKey 'Shell'
        policy = Get-Value $policyKey 'OverrideShellProgram'
    }
}

function Format-Places($Places) { ($Places.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join '; ' }

# dashboard.exe $Arguments, its lines in <name>.log; returns its exit code and lines.
function Invoke-Exe([string] $Exe, [string] $Name, [string[]] $Arguments) {
    $log = Join-Path $Out "$Name.log"
    $p = Start-Process -FilePath $Exe -ArgumentList $Arguments -NoNewWindow -PassThru -RedirectStandardError $log `
        -RedirectStandardOutput (Join-Path $Out "$Name.out.log")
    $p.Handle | Out-Null
    if (-not $p.WaitForExit(90000)) { Stop-Process -Id $p.Id -Force; throw "$Name didn't end within 90 s" }
    $lines = @(Get-Content $log -ErrorAction SilentlyContinue)
    Say "$Name ($($Arguments -join ' ')): exit $($p.ExitCode)"
    $lines | ForEach-Object { Say "  $_" }
    [pscustomobject]@{ Code = $p.ExitCode; Lines = $lines }
}

# The newest log of $Kind (shell, dashboard) begun in this phase.
function Get-NewLog([string] $Kind) {
    Get-ChildItem $logs -Filter "$Kind-*.log" -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -ge $booted } |
        Sort-Object Name | Select-Object -Last 1
}

function Read-Shared([string] $Path) {
    if (-not $Path -or -not (Test-Path $Path)) { return @() }
    $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $file = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
    try { $text = (New-Object IO.StreamReader $file).ReadToEnd() } finally { $file.Dispose() }
    @($text -split "\r?\n" | Where-Object { $_ })
}

# Waits for a line matching $Pattern in the file $Get gives (a script block, as the file may come later), after the
# first $After lines; the line or $null.
function Wait-Line([scriptblock] $Get, [string] $Pattern, [double] $Seconds = 60, [int] $After = 0) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    do {
        $hit = Read-Shared (& $Get) | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 250
    } while ($watch.Elapsed.TotalSeconds -lt $Seconds)
    $null
}

function Expect-Line([scriptblock] $Get, [string] $Pattern, [string] $What, [double] $Seconds = 60, [int] $After = 0,
                     [switch] $Require) {
    $line = Wait-Line $Get $Pattern $Seconds $After
    $said = "${What}: $(if ($line) { $line } else { "no line '$Pattern' within $Seconds s" })"
    if ($Require) { Require ([bool] $line) $said } else { Expect ([bool] $line) $said }
    $line
}

# A precondition: what follows depends on it, so a failure stops the phase here (the catch below goes to Finish, which
# turns shell mode off and sends what there is) rather than running on against a Dashboard that isn't there.
function Require([bool] $Ok, [string] $What) {
    Expect $Ok $What
    if (-not $Ok) { throw "stopped at a failed precondition: $What" }
}

$shellLog = { (Get-NewLog 'shell').FullName }
$dashboardLog = { (Get-NewLog 'dashboard').FullName }
$sentinelLog = { Join-Path $logs 'sentinel.log' }
function Get-Length([scriptblock] $Get) { @(Read-Shared (& $Get)).Count }

function Get-Dashboards { @(Get-Process dashboard -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq (Get-Process -Id $PID).SessionId }) }

# The --shell process: the dashboard.exe whose command line is --shell.
function Get-ShellProcess {
    Get-CimInstance Win32_Process -Filter "Name = 'dashboard.exe'" | Where-Object { $_.CommandLine -match '--shell\s*$' } | Select-Object -First 1
}

function Hold-ViewMenu([int] $Ms = 6000) {
    $back = [Nefarius.ViGEm.Client.Targets.Xbox360.Xbox360Button]::Back
    $start = [Nefarius.ViGEm.Client.Targets.Xbox360.Xbox360Button]::Start
    $script:Pad.SetButtonState($back, $true); $script:Pad.SetButtonState($start, $true)
    Start-Sleep -Milliseconds $Ms
    $script:Pad.SetButtonState($back, $false); $script:Pad.SetButtonState($start, $false)
    Say "held View+Menu for $Ms ms"
}

# Who's playing today? on a fresh data folder: A until Home's first frame.
function Pass-Startup {
    for ($i = 0; $i -lt 4; $i++) {
        if (Wait-Line $dashboardLog "startup: Home's first frame" 15) { return $true }
        Push-Button A
    }
    [bool] (Wait-Line $dashboardLog "startup: Home's first frame" 30)
}

function Next-Phase([int] $Phase) {
    $state.phase = $Phase
    $state | Add-Member -NotePropertyName sentinelMark -NotePropertyValue (Get-Length $sentinelLog) -Force
    Save-State
    $action = New-ScheduledTaskAction -Execute conhost.exe -Argument "--headless powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
    $principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Highest
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Hours 1) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
    Register-ScheduledTask -TaskName xbox-e2e-shell-next -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null
    Say "phase $Phase registered for the next sign-in; restarting"
    Save-Screen "phase-$($Phase - 1)-end"
    & shutdown.exe /r /t 0 /f
    # Never back to boot.ps1, which would send the output now.
    Start-Sleep -Seconds 600
    exit 0
}

# The end, from any phase: shell mode off, everything collected, and the output sent.
function Finish {
    try {
        if (Test-Path (Join-Path $data 'shell\record.json')) { Invoke-Exe $installed 'shell-off-at-end' @('--shell-off') | Out-Null }
    } catch { Say "couldn't turn it off at the end: $_" }
    Unregister-ScheduledTask -TaskName xbox-e2e-shell-next -Confirm:$false -ErrorAction SilentlyContinue
    if (Test-Path $logs) { Copy-Item $logs (Join-Path $Out 'shell-logs') -Recurse -Force }
    $failures | ForEach-Object { Say "failed: $_" }
    $result = if ($failures.Count -eq 0) { 'PASS' } else { 'FAIL' }
    Say "result: $result"
    Set-Content (Join-Path $Out 'exitcode.txt') $(if ($result -eq 'PASS') { 0 } else { 1 })
    Disconnect-Pad
    if ($state.phase -gt 1 -and $state.upload) {
        $zip = 'C:\e2e\out.zip'
        Compress-Archive (Join-Path $Out '*') $zip -Force
        Invoke-WebRequest -UseBasicParsing -Method Put -InFile $zip -ContentType application/zip $state.upload | Out-Null
    }
    exit $(if ($result -eq 'PASS') { 0 } else { 1 })
}

try {
    Say "phase $($state.phase), signed in as $user; the shell window's pid $([E2EShell]::ShellPid())"
    Unregister-ScheduledTask -TaskName xbox-e2e-shell-next -Confirm:$false -ErrorAction SilentlyContinue
    switch ($state.phase) {
        1 {
            Close-WindowsPrompts
            New-Item $installDir -ItemType Directory -Force | Out-Null
            Copy-Item (Join-Path $Payload 'dashboard.exe') $installed -Force
            $before = Get-Places
            $state.before = $before
            Say "before: $(Format-Places $before)"

            # P2: each refusal, with nothing written.
            $refusals = @(
                @{ key = 'bundle-boot-shell'; set = { Set-ItemProperty $runKey XboxSeriesBootShellSentinel '"C:\BootShell\XboxSeries.BootShell.exe" --sentinel' }
                   unset = { Remove-ItemProperty $runKey XboxSeriesBootShellSentinel } },
                @{ key = 'other-shell'; set = { Set-ItemProperty $shellKey Shell 'C:\Windows\System32\notepad.exe' }
                   unset = { if ($before.shell -eq 'none') { Remove-ItemProperty $shellKey Shell } else { Set-ItemProperty $shellKey Shell $before.shell } } },
                @{ key = 'machine-shell'; set = { Set-ItemProperty $machineKey Shell 'cmd.exe' }
                   unset = { Set-ItemProperty $machineKey Shell $before.machineShell } },
                @{ key = 'shell-overridden'; set = { New-Item $policyKey -Force | Out-Null; Set-ItemProperty $policyKey OverrideShellProgram 1 -Type DWord }
                   unset = { Remove-ItemProperty $policyKey OverrideShellProgram } }
            )
            foreach ($refusal in $refusals) {
                & $refusal.set
                $set = Get-Places
                $run = Invoke-Exe $installed "refused-$($refusal.key)" @('--shell-on')
                $after = Get-Places
                & $refusal.unset
                Expect ($run.Code -eq 3) "$($refusal.key): --shell-on exits 3 (got $($run.Code))"
                Expect ([bool] ($run.Lines -match "^shell: refused: $($refusal.key): ")) "$($refusal.key): said"
                Expect ((Format-Places $set) -eq (Format-Places $after)) "$($refusal.key): nothing written"
            }
            $portable = Invoke-Exe (Join-Path $Payload 'dashboard.exe') 'refused-not-installed' @('--shell-on')
            Expect ($portable.Code -eq 3 -and [bool] ($portable.Lines -match '^shell: refused: not-installed: ')) 'not-installed: the payload''s own copy is refused'
            Expect ((Format-Places (Get-Places)) -eq (Format-Places $before)) 'after the refusals every place is as before'

            # P1: the dry run says the four writes and writes nothing.
            $dry = Invoke-Exe $installed 'shell-on-dry' @('--shell-on', '--dry-shell')
            $want = "shell: would set HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell = `"`"$installed`" --shell`""
            Expect ($dry.Code -eq 0) "dry on exits 0 (got $($dry.Code))"
            Expect ([bool] ($dry.Lines | Where-Object { $_ -like "$want*" })) 'dry on says Shell exactly'
            Expect ([bool] ($dry.Lines -match '^shell: would register Task XboxSeriesDashboard\.SafetyNet')) 'dry on says the task'
            Expect ((Format-Places (Get-Places)) -eq (Format-Places $before)) 'the dry run wrote nothing'
            Expect (-not (Test-Path (Join-Path $data 'shell\record.json'))) 'the dry run wrote no record'

            # P1: on for real, then read back.
            $on = Invoke-Exe $installed 'shell-on' @('--shell-on')
            $now = Get-Places
            Say "on: $(Format-Places $now)"
            Expect ($on.Code -eq 0) "on exits 0 (got $($on.Code))"
            Expect ($now.shell -eq "`"$installed`" --shell") "Shell is `"$installed`" --shell"
            Expect ($now.sentinel -eq "`"$installed`" --sentinel") 'the sentinel names the exe'
            Expect ($now.scoobe -eq '0') 'SCOOBE is 0'
            Expect ($now.task) 'the safety net task is registered'
            Expect ($now.machineShell -eq $before.machineShell -and $now.policy -eq $before.policy) 'no HKLM value changed'
            $record = Join-Path $data 'shell\record.json'
            Expect (Test-Path $record) 'the record is saved'
            if (Test-Path $record) { Copy-Item $record (Join-Path $Out 'record-on.json') }
            Expect ((Invoke-Exe $installed 'shell-on-again' @('--shell-on')).Lines -contains 'shell: on already') 'on twice: on already'
            Next-Phase 2
        }
        2 {
            # P3: the sign-in.
            Expect-Line $shellLog '--shell started' 'the --shell process started' -Require | Out-Null
            foreach ($p in 'black up', 'ShellDesktopSwitchEvent set', 'events made', 'started pid \d+: .* --from-shell') {
                Expect-Line $shellLog $p "shell log: $p" 120 | Out-Null
            }
            # Everything after needs a Dashboard on the screen: one that never presents stops the phase here.
            Expect-Line $dashboardLog '^first frame presented' 'the Dashboard under --from-shell presented its first frame' 120 -Require | Out-Null
            foreach ($p in 'signal: FirstFrame', 'black down', 'signal: BootVideoEnded', 'explorer started, pid (\d+)') {
                Expect-Line $shellLog $p "shell log: $p" 120 | Out-Null
            }
            $explorerLine = Wait-Line $shellLog 'explorer started, pid (\d+)' 5
            if ($explorerLine -match 'pid (\d+)') {
                $started = [int] $Matches[1]
                Expect (Wait-Until { [E2EShell]::ShellPid() -eq $started } 30) "the shell window is that explorer's (pid $started; now $([E2EShell]::ShellPid()))"
            }
            Expect ([E2EShell]::FindWindow('CabinetWClass', [NullString]::Value) -eq [IntPtr]::Zero) 'no File Explorer window'
            Expect-Line $dashboardLog 'shell: FirstFrame set' 'the Dashboard set FirstFrame' | Out-Null
            Expect-Line $sentinelLog 'sentinel: the --shell process runs; nothing to do' 'the sentinel saw --shell and did nothing' 120 | Out-Null
            Save-Screen 'p3-signed-in'

            # P4: Exit to desktop from Power, the Guide button, View+Menu.
            Connect-Pad
            Require (Pass-Startup) 'Who''s playing today? passed to Home'
            Start-Sleep -Seconds 8
            $mark = Get-Length $shellLog
            Push-Button Guide 1000
            Start-Sleep -Seconds 3
            Push-Button Up; Start-Sleep -Seconds 2
            Push-Button A
            Expect-Line $shellLog 'exited with code 0x0: Exit to desktop' 'Exit to desktop: exit 0, a quit' 30 $mark -Require | Out-Null
            Expect-Line $shellLog 'the Dashboard stays down' 'it stays down' 10 $mark | Out-Null
            Start-Sleep -Seconds 3
            $shell = Get-ShellProcess
            Require ([bool] $shell) 'the --shell process runs'
            Expect ((Get-Dashboards | Where-Object { $_.Id -ne $shell.ProcessId }).Count -eq 0) 'no Dashboard runs'
            Expect ([E2EShell]::ShellPid() -ne 0) 'the desktop is up'
            Save-Screen 'p4-desktop'
            $mark = Get-Length $shellLog
            Push-Button Guide
            Expect-Line $shellLog 'Guide button with no Dashboard' 'Guide brings it back' 15 $mark | Out-Null
            Expect-Line $shellLog 'started pid \d+' 'started again' 15 $mark -Require | Out-Null
            Require (Pass-Startup) 'and it reaches Home'
            Start-Sleep -Seconds 5
            $mark = Get-Length $shellLog
            Hold-ViewMenu 6500
            Expect-Line $shellLog 'View\+Menu hold: closing 1 Dashboard' 'View+Menu closes it' 15 $mark | Out-Null
            Expect-Line $shellLog 'closed by the View\+Menu hold' 'it closed, not a crash' 20 $mark | Out-Null

            # P5: three crashes in 5 minutes.
            Set-Content (Join-Path $data 'shell\test-crash-after-first-frame') '3' -NoNewline
            $mark = Get-Length $shellLog
            Push-Button Guide
            Expect-Line $shellLog '3 crashes in the last 5 min' 'three crashes counted' 240 $mark | Out-Null
            Expect-Line $shellLog 'line up: The Dashboard closed 3 times in 5 minutes\. Press the Guide button to try again\.' 'the fallback line shows' 10 $mark | Out-Null
            Save-Screen 'p5-fallback-line'
            Expect-Line $shellLog 'fallback line down; starting the desktop' 'then the desktop' 15 $mark | Out-Null
            $relaunches = @(Read-Shared (& $shellLog) | Select-Object -Skip $mark | Where-Object { $_ -match 'started pid \d+' }).Count
            Expect ($relaunches -eq 3) "three starts, no more (got $relaunches)"
            Start-Sleep -Seconds 10
            Expect (@(Read-Shared (& $shellLog) | Select-Object -Skip $mark | Where-Object { $_ -match 'started pid \d+' }).Count -eq 3) 'nothing more started on its own'
            $mark = Get-Length $shellLog
            Push-Button Guide
            Expect-Line $shellLog 'Guide button with no Dashboard \(fallen back\)' 'Guide brings it back after the fallback' 15 $mark | Out-Null
            Require (Pass-Startup) 'and it stays up'

            # P6b: --shell killed after explorer: nothing changes.
            $shell = Get-ShellProcess
            Require ([bool] $shell) 'the --shell process runs before P6b'
            $explorer = [E2EShell]::ShellPid()
            Stop-Process -Id $shell.ProcessId -Force
            Say "killed --shell, pid $($shell.ProcessId)"
            Start-Sleep -Seconds 15
            Expect ([E2EShell]::ShellPid() -eq $explorer) 'explorer is still the shell'
            Expect ((Get-Dashboards).Count -ge 1) 'the Dashboard runs on'

            # P7: Shell lost, as a Windows upgrade loses it.
            Remove-ItemProperty $shellKey Shell
            Say 'deleted HKCU Shell'
            Next-Phase 3
        }
        3 {
            # P7: explorer was the shell from the start; the sentinel puts Shell back and starts the Dashboard.
            $mark = 0
            Expect-Line $sentinelLog 'sentinel: no --shell process in this session' 'the sentinel saw no --shell' 120 $sentinelMark | Out-Null
            Expect-Line $sentinelLog 'sentinel: Shell: none -> ' 'the sentinel wrote Shell back' 30 $sentinelMark | Out-Null
            Expect-Line $sentinelLog 'sentinel: started the Dashboard' 'and started the Dashboard' 30 $sentinelMark | Out-Null
            Expect ((Get-Value $shellKey 'Shell') -eq "`"$installed`" --shell") 'Shell is back'
            Expect ($null -eq (Get-NewLog 'shell')) 'no --shell process this sign-in'
            Next-Phase 4
        }
        4 {
            # P6a: --shell killed before explorer, as early as this task can: the safety net, then the sentinel.
            $shell = Get-ShellProcess
            $explorerUp = [E2EShell]::ShellPid() -ne 0
            if ($shell) { Stop-Process -Id $shell.ProcessId -Force; Say "killed --shell, pid $($shell.ProcessId); a shell window already: $explorerUp" }
            if ($explorerUp) {
                Say 'note: explorer was up before this task could kill --shell: P6a not reached this run (the logon task came late)'
            } else {
                Expect-Line { Join-Path $logs 'safety-net.log' } 'no explorer 90 s after sign-in: started it' 'the safety net started explorer' 150 | Out-Null
                # The Dashboard --shell had started may still run: the sentinel then says so and starts none.
                Expect-Line $sentinelLog 'sentinel: (started the Dashboard|a Dashboard runs already)' 'the sentinel saw to a Dashboard' 60 $sentinelMark | Out-Null
            }
            # P8: the running exe renamed aside, a copy in its place.
            $running = @(Get-Dashboards).Count
            Rename-Item $installed 'dashboard.0.old.exe'
            Copy-Item (Join-Path $Payload 'dashboard.exe') $installed
            Expect ((Test-Path (Join-Path $installDir 'dashboard.0.old.exe')) -and (Test-Path $installed)) "renamed aside while $running Dashboard process(es) ran from it"
            Next-Phase 5
        }
        5 {
            # P8: the copy is the shell now.
            Expect-Line $shellLog '--shell started' 'the copy runs as --shell' -Require | Out-Null
            Expect-Line $shellLog 'signal: FirstFrame' 'and its Dashboard came up' 120 | Out-Null
            $old = Join-Path $installDir 'dashboard.0.old.exe'
            Remove-Item $old -ErrorAction SilentlyContinue
            Expect (-not (Test-Path $old)) 'the old one could be deleted'
            # P9: off for real.
            $off = Invoke-Exe $installed 'shell-off' @('--shell-off')
            Expect ($off.Code -eq 0) "off exits 0 (got $($off.Code))"
            $now = Get-Places
            Say "off: $(Format-Places $now)"
            foreach ($name in 'shell', 'sentinel', 'scoobe', 'task', 'machineShell', 'policy') {
                Expect ("$($now.$name)" -eq "$($state.before.$name)") "$name is as before ($($state.before.$name))"
            }
            Expect (-not (Test-Path (Join-Path $data 'shell\record.json'))) 'the record is renamed'
            Expect ((Invoke-Exe $installed 'shell-off-again' @('--shell-off')).Lines -match '^shell: off already') 'off twice: off already'
            Next-Phase 6
        }
        6 {
            # P9: the desktop from the start.
            Start-Sleep -Seconds 20
            Expect ($null -eq (Get-NewLog 'shell')) 'no --shell process this sign-in'
            Expect ((Get-Process explorer -ErrorAction SilentlyContinue | Where-Object Id -eq ([E2EShell]::ShellPid())).Count -eq 1) 'explorer is the shell'
            Expect ((Get-Dashboards).Count -eq 0) 'no Dashboard runs'
            Save-Screen 'p9-desktop'
            Finish
        }
    }
} catch {
    $failures += "phase $($state.phase): error: $_"
    try { Save-Screen "error-phase-$($state.phase)" } catch {}
    # Phase 1's output goes back by boot.ps1, a later phase's from Finish.
    Finish
}
