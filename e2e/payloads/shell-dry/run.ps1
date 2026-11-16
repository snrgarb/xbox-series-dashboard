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
#   - the four places shell mode writes, and the record, are exactly as they were before (read from the registry, and
#     with schtasks.exe, which this test only reads with);
#   - no line of either says it wrote anything (`shell: set `, `shell: register`, `shell: saved`, `shell: renamed`);
#   - `--from-shell --dry-shell`, the window's path as the shell starts it with the shell's events left alone, logs to
#     <data>\shell\logs\dashboard-<stamp>.log, says it is a dry run, and presents its first frame (`first frame
#     presented`) with no `window: ERROR` and no panic: checked here, on any machine, before shell-mode restarts one.
#
# Bounded: the whole payload ends within $Budget seconds whatever happens. Every process it starts has its stdout and
# stderr in files of its own (none holds this script's output, which the harness reads to its end), is waited on with
# a limit, and is ended with everything it started, by pid, on every path out.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$Budget = 170
$clock = [Diagnostics.Stopwatch]::StartNew()
$Since = (Get-Date).AddSeconds(-2)
$started = @()

# Seconds left of $Want, within what is left of the budget (a few kept for the end).
function Get-Limit([double] $Want) { [Math]::Max(1, [Math]::Min($Want, $Budget - 10 - $clock.Elapsed.TotalSeconds)) }

# Ends $Id and every process under it, children whose parent has already gone included (they keep its pid). Only
# processes made since this test began count, so a pid Windows has given to something else since is never touched.
function Stop-Tree([int] $Id) {
    $all = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object { $_.CreationDate -ge $script:Since })
    $tree = @($all | Where-Object ProcessId -eq $Id | ForEach-Object { [int] $_.ProcessId })
    if (-not $tree.Count) { $tree = @($Id) }  # gone already: its children may not be
    for ($i = 0; $i -lt $tree.Count; $i++) {
        $tree += @($all | Where-Object { $_.ParentProcessId -eq $tree[$i] -and $_.ProcessId -ne $tree[$i] -and $tree -notcontains $_.ProcessId } |
            ForEach-Object { [int] $_.ProcessId })
    }
    if (-not ($all | Where-Object ProcessId -eq $Id)) { $tree = @($tree | Select-Object -Skip 1) }
    [array]::Reverse($tree)
    foreach ($p in $tree) {
        $name = ($all | Where-Object ProcessId -eq $p | Select-Object -First 1).Name
        if (Get-Process -Id $p -ErrorAction SilentlyContinue) {
            Stop-Process -Id $p -Force -ErrorAction SilentlyContinue
            Say "stopped pid $p ($name)"
        }
    }
}

# Starts $Exe with its output in <name>.log and <name>.out.log; the process, held, its tree ended at the end.
function Start-Logged([string] $Exe, [string] $Name, [string[]] $Arguments) {
    $p = Start-Process -FilePath $Exe -ArgumentList $Arguments -WorkingDirectory $Payload -NoNewWindow -PassThru `
        -RedirectStandardError (Join-Path $Out "$Name.log") -RedirectStandardOutput (Join-Path $Out "$Name.out.log")
    $p.Handle | Out-Null
    $script:started += $p.Id
    $p
}

# Runs $Exe and waits at most $Seconds (within the budget); its exit code (or 'timeout') and its lines.
function Invoke-Bounded([string] $Exe, [string] $Name, [string[]] $Arguments, [double] $Seconds = 30) {
    $p = Start-Logged $Exe $Name $Arguments
    $limit = Get-Limit $Seconds
    $code = if ($p.WaitForExit([int] ($limit * 1000))) { $p.ExitCode } else {
        Say "$Name didn't end within $([int] $limit) s"
        Stop-Tree $p.Id
        'timeout'
    }
    $lines = @(Get-Content (Join-Path $Out "$Name.log") -ErrorAction SilentlyContinue) +
             @(Get-Content (Join-Path $Out "$Name.out.log") -ErrorAction SilentlyContinue)
    [pscustomobject]@{ Code = $code; Lines = $lines }
}

# What shell mode would touch, as text: each value, or "none"; the task's XML, or "none"; the record.
function Get-ShellPlaces([string] $Data, [string] $When) {
    $values = @(
        @('CurrentUser', 'Software\Microsoft\Windows NT\CurrentVersion\Winlogon', 'Shell'),
        @('CurrentUser', 'Software\Microsoft\Windows\CurrentVersion\Run', 'XboxSeriesDashboardSentinel'),
        @('CurrentUser', 'Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement', 'ScoobeSystemSettingEnabled'),
        @('LocalMachine', 'Software\Microsoft\Windows NT\CurrentVersion\Winlogon', 'Shell')
    )
    $lines = foreach ($v in $values) {
        $key = [Microsoft.Win32.Registry]::$($v[0]).OpenSubKey($v[1])
        $value = if ($key) { try { $key.GetValue($v[2]) } finally { $key.Dispose() } } else { $null }
        "$($v[0])\$($v[1])\$($v[2]) = $(if ($null -ne $value) { "$value" } else { 'none' })"
    }
    $schtasks = Join-Path $env:SystemRoot 'System32\schtasks.exe'
    $task = Invoke-Bounded $schtasks "schtasks-$When" @('/Query', '/TN', 'XboxSeriesDashboard.SafetyNet', '/XML') 20
    $lines += "Task XboxSeriesDashboard.SafetyNet = $(if ($task.Code -eq 0) { ($task.Lines -join ' ').Trim() } elseif ($task.Code -eq 'timeout') { 'timeout' } else { 'none' })"
    $record = Join-Path $Data 'shell\record.json'
    $lines += "record = $(if (Test-Path $record) { (Get-Content $record -Raw).Trim() } else { 'none' })"
    $lines
}

# Runs dashboard.exe with $Arguments; its exit code and lines, said.
function Invoke-Dashboard([string] $Name, [string[]] $Arguments) {
    $run = Invoke-Bounded (Join-Path $Payload 'dashboard.exe') $Name $Arguments 30
    Say "$Name ($($Arguments -join ' ')): exit $($run.Code), $($run.Lines.Count) lines"
    $run.Lines | ForEach-Object { Say "  $_" }
    $run
}

function Expect([bool] $Ok, [string] $What) {
    Say "$(if ($Ok) { 'ok' } else { 'NOT' }): $What"
    if (-not $Ok) { $script:failures += $What }
}

# The lines of a file another process may still be writing.
function Read-Shared([string] $Path) {
    $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $file = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
    try { @((New-Object IO.StreamReader $file).ReadToEnd() -split "\r?\n" | Where-Object { $_ }) } finally { $file.Dispose() }
}

try {
    $data = Get-DashboardDataDir
    New-Item $data -ItemType Directory -Force | Out-Null
    $exe = Join-Path $Payload 'dashboard.exe'
    $before = Get-ShellPlaces $data 'before'
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
    # lines go to <data>\shell\logs\dashboard-<stamp>.log, as under the shell; what comes before that, and stdout, to
    # from-shell.early.log and from-shell.out.log. It is ended, with anything it started, as soon as it has presented.
    $logs = Join-Path $data 'shell\logs'
    $had = @(Get-ChildItem $logs -Filter 'dashboard-*.log' -ErrorAction SilentlyContinue | ForEach-Object FullName)
    $arguments = @('--from-shell', '--dry-shell', '--data-dir', "`"$data`"", '--store-art-service', 'http://127.0.0.1:9')
    if (Test-SafeMode) { $arguments += @(Get-SafeArguments $arguments) }
    $dashboard = Start-Logged $exe 'from-shell.early' $arguments
    Say "started the Dashboard, pid $($dashboard.Id): $($arguments -join ' ')"
    $fromShell = $null
    $lines = @()
    $limit = Get-Limit 60
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($watch.Elapsed.TotalSeconds -lt $limit) {
        if (-not $fromShell) {
            $fromShell = Get-ChildItem $logs -Filter 'dashboard-*.log' -ErrorAction SilentlyContinue |
                Where-Object { $had -notcontains $_.FullName } | Sort-Object LastWriteTime | Select-Object -Last 1
        }
        if ($fromShell) {
            $lines = @(Read-Shared $fromShell.FullName)
            if ($lines -match '^first frame presented') { break }
        }
        if ($dashboard.HasExited) { break }
        Start-Sleep -Milliseconds 250
    }
    $exited = $dashboard.HasExited
    Stop-Tree $dashboard.Id
    $dashboard.WaitForExit(5000) | Out-Null
    Say "from-shell (--from-shell --dry-shell): $(if ($fromShell) { $fromShell.Name } else { 'no log' }), $(if ($exited) { "exited by itself with $($dashboard.ExitCode)" } else { "ended by this test after $([int] $watch.Elapsed.TotalSeconds) s" })"
    @(Get-Content (Join-Path $Out 'from-shell.early.log') -ErrorAction SilentlyContinue) + $lines | ForEach-Object { Say "  $_" }
    if ($fromShell) { $lines | Set-Content (Join-Path $Out 'from-shell.log') -Encoding UTF8 }
    Expect ([bool] $fromShell) 'the --from-shell Dashboard logs to shell\logs'
    Expect ([bool] ($lines -match '^shell: started by the shell; --dry-shell: the events are left alone')) '--from-shell --dry-shell says the events are left alone'
    Expect ([bool] ($lines -match '^first frame presented')) "the --from-shell window presents its first frame (within $([int] $limit) s)"
    Expect (-not ($lines -match '^window: ERROR')) 'its swap chain was made the first time'
    Expect (-not ($lines -match 'panicked')) 'it did not panic'

    $after = Get-ShellPlaces $data 'after'
    $after | Set-Content (Join-Path $Out 'places-after.txt') -Encoding UTF8
    $changed = @(Compare-Object $before $after)
    Expect ($changed.Count -eq 0) 'every place shell mode writes is as it was'
    $changed | ForEach-Object { Say "  changed: $($_.SideIndicator) $($_.InputObject)" }
    Expect (-not (Test-Path (Join-Path $data 'shell\logs\setup.log'))) 'no setup.log was written'

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
} finally {
    # Whatever happened: nothing this test started outlives it.
    foreach ($id in $started) { try { Stop-Tree $id } catch { Say "couldn't stop pid ${id}: $_" } }
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result ($([int] $clock.Elapsed.TotalSeconds) s)"
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
