# Home, walked with the virtual pad on a fake Steam library: where it starts, the Recent row, the wide tiles, the
# Groups and See all, B back to the Recent row, the top bar. A screenshot at each rest, to set beside the Bundle's.
# Then what Home costs standing still, and a second start, which must come up from the library database.
#
# Passes when the Dashboard stays up, each press is followed by its move sound (the proof focus moved: the tones
# `standin make-tones` writes, --media and --log-sounds), and the second start says it read the database.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Moves { @(Get-DashboardLog | Where-Object { $_ -match 'audio: played navigation\.wav' }).Count }

# One press that must move focus, then a rest for the slide and the growth to finish.
# The software renderer takes most of a second a frame and an animation a few frames: a picture waits for them.
function Step([string] $Button, [string] $Name, [int] $RestMs = 3000) {
    $before = Moves
    Push-Button $Button
    $moved = Wait-Until { (Moves) -gt $before } 4
    Start-Sleep -Milliseconds $RestMs
    if ($Name) { Save-Screen $Name }
    Say "$Button$(if ($Name) { " -> $Name" }): $(if ($moved) { 'moved' } else { 'NO move sound' })"
    if (-not $moved) { $script:failures += "$Button before $Name did not move focus" }
}

# Processor time and memory over $Seconds with nothing pressed.
function Measure-Idle([string] $What, [int] $Seconds = 10, [switch] $FromRest) {
    if ($FromRest) {
        # Measured from rest: once --log-frames has said that five seconds went by with nothing drawn.
        $before = @(Get-Content $DashboardLog | Select-String '^frames:').Count
        $rested = Wait-Until {
            $said = @(Get-Content $DashboardLog | Select-String '^frames:')
            $said.Count -gt $before -and $said[-1].Line -match 'home 0, overlay 0'
        } 90
        if (-not $rested) { $script:failures += "Home never came to rest $What" }
    }
    $p = Get-Process -Id $script:dashboard.Id
    $cpu = $p.TotalProcessorTime.TotalMilliseconds
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds $Seconds
    $p.Refresh()
    $used = $p.TotalProcessorTime.TotalMilliseconds - $cpu
    $percent = 100 * $used / $watch.ElapsedMilliseconds
    Say ("idle, {0}: {1:N0} ms of processor time in {2:N1} s ({3:N1}% of one core), working set {4:N0} MB, private {5:N0} MB" -f `
        $What, $used, ($watch.ElapsedMilliseconds / 1000), $percent, ($p.WorkingSet64 / 1MB), ($p.PrivateMemorySize64 / 1MB))
    # What it drew meanwhile, as --log-frames said it.
    Get-Content $DashboardLog | Select-String '^frames:' | Select-Object -Last 2 | ForEach-Object { Say "  $($_.Line)" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $media = Join-Path $Out 'media'
    & (Join-Path $PSScriptRoot 'standin.exe') make-tones $media | Out-Null
    $arguments = @('--media', "`"$media`"", '--log-sounds', '--log-frames')

    $script:dashboard = Start-Dashboard $arguments
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "first start: $first"
    Say "  $(Wait-DashboardLog '^start-up:' 2)"
    Say "  $(Wait-DashboardLog '^steam:' 5)"
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
    Save-Screen '01-start'
    Measure-Idle 'on the profile, the swirl moving'

    Step Down '02-recent-first' 3500     # the background waits 1.5 s before it follows
    Measure-Idle 'on a Recent tile, nothing moving' -FromRest
    Step Right ''
    Step Right '03-recent-third' 3500
    Step Down '04-wide-tiles'
    Step Right '05-wide-second'
    Step Down '06-group-1'
    Step Down '07-group-2'
    Step Right ''
    Step Right '08-group-2-third'
    Step Up '09-up-group-1'
    Step Up '10-up-wide-tiles'
    Step Down ''
    # Down from a wide tile lands on the Group's first tile; See all is its seventh.
    1..5 | ForEach-Object { Step Right '' 500 }
    Step Right '11-see-all' 4000
    Push-Button B
    Start-Sleep -Milliseconds 2500
    Save-Screen '12-b-recent'
    Say 'B from the Group'
    Step Up '13-top-bar'
    Step Right '14-top-bar-next'
    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }

    # A second start: from the database.
    Stop-Dashboard
    Copy-Item $DashboardLog (Join-Path $Out 'dashboard-first.log')
    $script:dashboard = Start-Dashboard $arguments
    $again = Wait-DashboardLog 'first frame presented' 60
    $fromDatabase = Wait-DashboardLog '^library: \d+ games from the database' 5
    Say "second start: $again"
    Say "  $(Wait-DashboardLog '^start-up:' 2)"
    Say "  $(if ($fromDatabase) { $fromDatabase } else { 'no line saying the library came from the database' })"
    if (-not $again) { $failures += 'the second start never presented a frame' }
    if ($fromDatabase -notmatch "^library: $(@($library.games).Count) games from the database") {
        $failures += "the second start did not read the $(@($library.games).Count) games from the database: $fromDatabase"
    }
    Start-Sleep -Seconds 3
    Save-Screen '15-second-start'

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
