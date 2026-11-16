# Shell mode's dry run (docs/spec/shell-mode.md, 3.3), safe on any machine, the TV PC included: `dashboard.exe
# --shell-on --dry-shell --as-installed` and `--shell-off --dry-shell` read this PC for real and write nothing. On or
# off, nothing here can change what Windows starts at sign-in.
#
# Passes when:
#   - on's dry run says it is a dry run, says each of the four writes as `would`, in the order task, SCOOBE, sentinel,
#     Shell, the Shell line naming the exe and `--shell` exactly; or, on a PC where shell mode can't be turned on (the
#     TV PC with the Bundle's Boot shell set up), says each refusal and that nothing was written. Either way its exit
#     code goes with what it said (0 or 3);
#   - off's dry run says off already, or what it would put back, and exits 0;
#   - the four places shell mode writes, and the record, are exactly as they were before (read with reg.exe and
#     schtasks.exe, which this test only reads with);
#   - no line of either says it wrote anything (`shell: set `, `shell: register`, `shell: saved`, `shell: renamed`);
#   - `--from-shell --dry-shell`, the window's path as the shell starts it with the shell's events left alone, logs to
#     <data>\shell\logs\dashboard-<stamp>.log, says it is a dry run, and presents its first frame (`first frame
#     presented`) with no `window: ERROR` and no panic: checked here, on any machine, before shell-mode restarts one.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

# What shell mode would touch, as text: each value with its type, or "none"; the task's XML, or "none"; the record.
function Get-ShellPlaces([string] $Data) {
    $values = @(
        @('HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon', 'Shell'),
        @('HKCU\Software\Microsoft\Windows\CurrentVersion\Run', 'XboxSeriesDashboardSentinel'),
        @('HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement', 'ScoobeSystemSettingEnabled'),
        @('HKLM\Software\Microsoft\Windows NT\CurrentVersion\Winlogon', 'Shell')
    )
    $lines = foreach ($v in $values) {
        $said = & cmd.exe /c "reg query `"$($v[0])`" /v $($v[1]) 2>nul"
        $line = $said | Where-Object { $_ -match "^\s+$($v[1])\s" } | Select-Object -First 1
        "$($v[0])\$($v[1]) = $(if ($line) { $line.Trim() } else { 'none' })"
    }
    $task = & cmd.exe /c 'schtasks /Query /TN XboxSeriesDashboard.SafetyNet /XML 2>nul'
    $lines += "Task XboxSeriesDashboard.SafetyNet = $(if ($LASTEXITCODE -eq 0) { ($task -join ' ').Trim() } else { 'none' })"
    $record = Join-Path $Data 'shell\record.json'
    $lines += "record = $(if (Test-Path $record) { (Get-Content $record -Raw).Trim() } else { 'none' })"
    $lines
}

# Runs dashboard.exe with $Arguments, its lines (stderr and stdout) in <name>.log; returns the exit code and the lines.
function Invoke-Dashboard([string] $Name, [string[]] $Arguments) {
    $exe = Join-Path $Payload 'dashboard.exe'
    $log = Join-Path $Out "$Name.log"
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -WorkingDirectory $Payload -NoNewWindow -PassThru `
        -RedirectStandardError $log -RedirectStandardOutput (Join-Path $Out "$Name.out.log")
    $p.Handle | Out-Null
    if (-not $p.WaitForExit(60000)) { Stop-Process -Id $p.Id -Force; throw "$Name didn't end within 60 s" }
    $lines = @(Get-Content $log -ErrorAction SilentlyContinue)
    Say "$Name ($($Arguments -join ' ')): exit $($p.ExitCode), $($lines.Count) lines"
    $lines | ForEach-Object { Say "  $_" }
    [pscustomobject]@{ Code = $p.ExitCode; Lines = $lines }
}

function Expect([bool] $Ok, [string] $What) {
    Say "$(if ($Ok) { 'ok' } else { 'NOT' }): $What"
    if (-not $Ok) { $script:failures += $What }
}

try {
    $data = Get-DashboardDataDir
    New-Item $data -ItemType Directory -Force | Out-Null
    $exe = Join-Path $Payload 'dashboard.exe'
    $before = Get-ShellPlaces $data
    $before | Set-Content (Join-Path $Out 'places-before.txt') -Encoding UTF8

    $on = Invoke-Dashboard 'shell-on-dry' @('--shell-on', '--dry-shell', '--as-installed', '--data-dir', "`"$data`"")
    Expect ($on.Lines -contains 'shell: a dry run (--dry-shell): nothing is written') 'on says it is a dry run'
    $refused = @($on.Lines | Where-Object { $_ -match '^shell: refused: ' })
    if ($refused.Count) {
        Say "this PC refuses shell mode: $($refused -join '; ')"
        Expect ($on.Code -eq 3) "refused on exits 3 (got $($on.Code))"
        Expect ($on.Lines -contains 'shell: nothing was written') 'a refusal says nothing was written'
    } else {
        Expect ($on.Code -eq 0) "on exits 0 (got $($on.Code))"
        $want = "shell: would set HKCU\Software\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell = `"`"$exe`" --shell`""
        $order = @(
            '^shell: (would register|Task XboxSeriesDashboard\.SafetyNet holds)',
            '^shell: (would set |)HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\UserProfileEngagement\\ScoobeSystemSettingEnabled',
            '^shell: (would set |)HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\\XboxSeriesDashboardSentinel',
            '^shell: (would set |)HKCU\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\\Shell'
        )
        $at = -1
        foreach ($pattern in $order) {
            $i = 0; $found = -1
            foreach ($line in $on.Lines) { if ($i -gt $at -and $line -match $pattern) { $found = $i; break }; $i++ }
            Expect ($found -gt $at) "on says '$pattern' after the step before"
            if ($found -gt $at) { $at = $found }
        }
        $shellLine = $on.Lines | Where-Object { $_ -like "$want*" } | Select-Object -First 1
        Expect ([bool] $shellLine -or ($on.Lines -match 'Winlogon\\Shell holds')) "on's Shell line names `"$exe`" --shell exactly"
        Expect ([bool] ($on.Lines -match '^shell: would be on from the next sign-in')) 'on ends with would be on'
    }

    $off = Invoke-Dashboard 'shell-off-dry' @('--shell-off', '--dry-shell', '--data-dir', "`"$data`"")
    Expect ($off.Code -eq 0) "off exits 0 (got $($off.Code))"
    Expect ($off.Lines -contains 'shell: a dry run (--dry-shell): nothing is written') 'off says it is a dry run'

    foreach ($line in @($on.Lines) + @($off.Lines)) {
        if ($line -match '^shell: (set |register|saved|renamed|put back)') { $failures += "a line says it wrote: $line" }
    }

    # The window as the shell starts it, without being the shell: --from-shell --dry-shell makes the window plain,
    # configures its swap chain, puts it topmost for Start-up, and presents, leaving the shell's events alone. Its
    # lines go to <data>\shell\logs\dashboard-<stamp>.log, as under the shell.
    $logs = Join-Path $data 'shell\logs'
    $had = @(Get-ChildItem $logs -Filter 'dashboard-*.log' -ErrorAction SilentlyContinue | ForEach-Object FullName)
    $dashboard = Start-Dashboard @('--from-shell', '--dry-shell', '--data-dir', "`"$data`"") -Startup
    $fromShell = $null
    $lines = @()
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($watch.Elapsed.TotalSeconds -lt 60) {
        if (-not $fromShell) {
            $fromShell = Get-ChildItem $logs -Filter 'dashboard-*.log' -ErrorAction SilentlyContinue |
                Where-Object { $had -notcontains $_.FullName } | Sort-Object LastWriteTime | Select-Object -Last 1
        }
        if ($fromShell) {
            $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
            $file = [IO.File]::Open($fromShell.FullName, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
            try { $lines = @((New-Object IO.StreamReader $file).ReadToEnd() -split "\r?\n" | Where-Object { $_ }) }
            finally { $file.Dispose() }
            if ($lines -match '^first frame presented') { break }
        }
        if ($dashboard.HasExited) { break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $dashboard.HasExited) { Stop-Process -Id $dashboard.Id -Force; $dashboard.WaitForExit(10000) | Out-Null }
    Say "from-shell (--from-shell --dry-shell): $(if ($fromShell) { $fromShell.Name } else { 'no log' }), exit $($dashboard.ExitCode)"
    @(Get-DashboardLog) + $lines | ForEach-Object { Say "  $_" }
    if ($fromShell) { Copy-Item $fromShell.FullName (Join-Path $Out 'from-shell.log') }
    Expect ([bool] $fromShell) 'the --from-shell Dashboard logs to shell\logs'
    Expect ([bool] ($lines -match '^shell: started by the shell; --dry-shell: the events are left alone')) '--from-shell --dry-shell says the events are left alone'
    Expect ([bool] ($lines -match '^first frame presented')) 'the --from-shell window presents its first frame'
    Expect (-not ($lines -match '^window: ERROR')) 'its swap chain was made the first time'
    Expect (-not ($lines -match 'panicked')) 'it did not panic'

    $after = Get-ShellPlaces $data
    $after | Set-Content (Join-Path $Out 'places-after.txt') -Encoding UTF8
    $changed = @(Compare-Object $before $after)
    Expect ($changed.Count -eq 0) 'every place shell mode writes is as it was'
    $changed | ForEach-Object { Say "  changed: $($_.SideIndicator) $($_.InputObject)" }
    Expect (-not (Test-Path (Join-Path $data 'shell\logs\setup.log'))) 'no setup.log was written'

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
} finally {
    if ($dashboard -and -not $dashboard.HasExited) { Stop-Process -Id $dashboard.Id -Force -ErrorAction SilentlyContinue }
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
