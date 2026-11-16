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
#   - no line of either says it wrote anything (`shell: set `, `shell: register`, `shell: saved`, `shell: renamed`).
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
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
