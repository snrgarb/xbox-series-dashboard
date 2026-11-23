# The live Dashboard as the TV PC's shell, for the launcher and the cleanup (dot-sourced by both): finding it, stopping
# it for a test, and starting it again afterwards.
#
# The live shell is the installed copy, %LOCALAPPDATA%\Programs\XboxSeriesDashboard\dashboard.exe, and nothing else:
# its watchdog (`dashboard.exe --shell`, which Winlogon started) and the Dashboard it started (`--from-shell`). A
# process counts only by its program's path, never by its name, so a payload's own dashboard.exe (in the test's folder)
# is never taken for it. $Note, which each function logs through, must write nothing to the pipeline. While the launcher has it stopped, the file shell-stopped.txt in the test's folder says so,
# so the cleanup, run over ssh after a timeout or a killed run, can start it again when the launcher could not.

$LiveShellExe = Join-Path $env:LOCALAPPDATA 'Programs\XboxSeriesDashboard\dashboard.exe'
$LiveShellTask = 'XsdE2EShell'

# Every dashboard.exe whose program is the installed copy, watchdogs first, each with .Watchdog set.
function Get-LiveShell {
    @(Get-CimInstance Win32_Process -Filter "Name = 'dashboard.exe'" -ErrorAction SilentlyContinue | Where-Object {
        $_.ExecutablePath -and $_.ExecutablePath -ieq $LiveShellExe
    } | ForEach-Object {
        $_ | Add-Member -NotePropertyName Watchdog -NotePropertyValue ([bool]($_.CommandLine -match '\s--shell(\s|$)')) -PassThru
    } | Sort-Object { -not $_.Watchdog })
}

# Stops the live shell: the watchdogs first, so none of them starts the Dashboard again, then the rest. True when none
# is left. $Note is how to log.
function Stop-LiveShell([scriptblock] $Note) {
    foreach ($process in Get-LiveShell) {
        $what = if ($process.Watchdog) { 'the watchdog' } else { 'the Dashboard' }
        & $Note "stopping the live shell's $what (pid $($process.ProcessId)): $($process.CommandLine)"
        Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
        # The watchdog gone before the Dashboard goes, so it cannot see the Dashboard's end and start it again.
        if ($process.Watchdog) { Wait-Process -Id $process.ProcessId -Timeout 10 -ErrorAction SilentlyContinue }
    }
    for ($i = 0; $i -lt 20; $i++) {
        $left = @(Get-LiveShell)
        if (-not $left.Count) { & $Note 'the live shell is stopped'; return $true }
        Start-Sleep -Milliseconds 250
    }
    & $Note "the live shell would not stop: $(($left | ForEach-Object { "pid $($_.ProcessId)" }) -join ', ')"
    $false
}

# Starts `dashboard.exe --shell` again in the signed-in user's session, whoever calls it (the launcher on the desktop,
# or the cleanup over ssh, which is not in that session): through a one-off interactive task, whose action only
# starts it and ends, so the Dashboard belongs to no task and no test that is ended later. True when the watchdog is
# seen running within 20 s; the task is deleted either way.
function Start-LiveShell([scriptblock] $Note) {
    if (@(Get-LiveShell | Where-Object Watchdog).Count) { & $Note 'the live shell is running already'; return $true }
    if (-not (Test-Path $LiveShellExe)) { & $Note "cannot start the live shell: no $LiveShellExe"; return $false }
    $system = Join-Path $env:SystemRoot 'System32'
    # conhost --headless: no console window flashes on the TV (as shell mode's own safety net). start: cmd ends at once.
    $action = New-ScheduledTaskAction -Execute (Join-Path $system 'conhost.exe') `
        -Argument "--headless `"$(Join-Path $system 'cmd.exe')`" /c start `"`" `"$LiveShellExe`" --shell"
    # Over ssh USERDOMAIN may be unset: the account is then the machine's own, by its name.
    $domain = if ($env:USERDOMAIN) { $env:USERDOMAIN } else { $env:COMPUTERNAME }
    $principal = New-ScheduledTaskPrincipal -UserId "$domain\$env:USERNAME" -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 1)
    try {
        Register-ScheduledTask -TaskName $LiveShellTask -Action $action -Principal $principal -Settings $settings -Force -ErrorAction Stop | Out-Null
        Start-ScheduledTask -TaskName $LiveShellTask -ErrorAction Stop
        & $Note "starting the live shell: `"$LiveShellExe`" --shell, through the task $LiveShellTask"
        for ($i = 0; $i -lt 40; $i++) {
            $watchdog = Get-LiveShell | Where-Object Watchdog | Select-Object -First 1
            if ($watchdog) { & $Note "the live shell is running again (pid $($watchdog.ProcessId))"; return $true }
            Start-Sleep -Milliseconds 500
        }
        & $Note 'the live shell did not start within 20 s'
        $false
    } catch {
        & $Note "cannot start the live shell: $_"
        $false
    } finally {
        Unregister-ScheduledTask -TaskName $LiveShellTask -Confirm:$false -ErrorAction SilentlyContinue
    }
}
