# Now Playing (docs/spec/now-playing.md), over Home, with the stand-in's media session (`standin media`):
#
#   1. The stand-in starts a session of its own, playing a track with a title no one else's has. The Dashboard's
#      `media:` lines say Windows lists it, and by which app's name.
#   2. Settings > Guide > Now Playing source: its list opened, walked by the log to the stand-in's app, and chosen;
#      the Dashboard says it saved it. From here on the card follows the stand-in's session whatever else plays.
#   3. The Guide's Audio Flyout: `now playing:` says the stand-in's track, playing, in its app; focus walked down to
#      the card, and down again to Play/Pause.
#   4. A on Play/Pause: the stand-in says it was pressed Pause, and the card says paused once its session says so.
#   5. Right to Next, A: the stand-in says it was pressed Next, and the card shows its next track, " (2)".
#   6. Out of the Guide; the stand-in's own process ended.
#
# This runs on the TV PC too, where Spotify or a browser may be playing: no control is pressed until the source is
# the stand-in's app and the last `now playing:` line names it, so nothing the test presses reaches the user's media.
# Judged by the Dashboard's `guide:`, `focus:`, `media:` and `now playing:` lines (--log-focus) and the stand-in's
# `media: pressed ...` lines in standin.log.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
# A title only this run's stand-in has.
$title = "E2E Stand-in Song $(Get-Random -Minimum 1000 -Maximum 9999)"
$artist = 'The Stand-ins'
$standin = $null

function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

# The last line with this prefix the Dashboard logged, without the prefix.
function Get-LastLine([string] $Prefix) {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match "^${Prefix}: " })
    if ($lines.Count) { $lines[-1] -replace "^${Prefix}: ", '' } else { '' }
}
function Get-GuideLine { $line = Get-LastLine 'guide'; if ($line) { $line } else { 'closed' } }

# What the stand-in has said in standin.log, and a wait for a line of it after the first $After.
function Get-StandInLog {
    $file = Join-Path $env:E2E_OUT 'standin.log'
    if (-not (Test-Path $file)) { return @() }
    $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $stream = [IO.File]::Open($file, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
    try { $text = (New-Object IO.StreamReader $stream, ([Text.Encoding]::UTF8)).ReadToEnd() } finally { $stream.Dispose() }
    @($text -split "\r?\n" | Where-Object { $_ -ne '' })
}
function Wait-StandInLog([string] $Pattern, [double] $Seconds, [int] $After = 0) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    do {
        $hit = Get-StandInLog | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 200
    } while ($watch.Elapsed.TotalSeconds -lt $Seconds)
    $null
}

# One press, then the line it must bring (a regular expression), judged by the log.
function Step([string] $Button, [string] $Line, [string] $Name = '', [double] $Seconds = 8) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $hit = Wait-DashboardLog $Line $Seconds $mark
    Say "$Button$(if ($Name) { " -> $Name" }): $(if ($hit) { $hit } else { "NO line '$Line' (the Guide is at: $(Get-GuideLine); focus: $(Get-LastLine 'focus'))" })"
    if (-not $hit) { $script:failures += "$Button did not bring '$Line'" }
    if ($Name) {
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    [bool] $hit
}

# A press while walking somewhere: a press that brings no line is said, and the walk goes by the log again.
function Walk([string] $Button, [string] $Line) {
    $mark = Get-DashboardLogLength
    Push-Button $Button
    $hit = Wait-DashboardLog $Line 5 $mark
    Say "$Button`: $(if ($hit) { $hit } else { "note: no line '$Line' of this press; the walk goes on" })"
}

function Move-ToUtilities {
    for ($i = 0; $i -lt 12; $i++) {
        if ((Get-GuideLine) -match '^Landing Utility\(0\)') { return }
        if ((Get-GuideLine) -notmatch '^Landing Entry\((\d+)\)') { throw "The Guide is not on its list: $(Get-GuideLine)" }
        Walk Down '^guide: Landing (Entry|Utility)\(\d+\)'
    }
    throw "Focus never reached the Utilities: $(Get-GuideLine)"
}

function Move-ToItem([int] $Item) {
    for ($i = 0; $i -lt 8; $i++) {
        if ((Get-GuideLine) -notmatch 'flyout Audio Item\((\d+)\)') { throw "The Flyout's focus is not on an entry: $(Get-GuideLine)" }
        $at = [int] $Matches[1]
        if ($at -eq $Item) { return }
        $next = if ($at -lt $Item) { $at + 1 } else { $at - 1 }
        Walk $(if ($at -lt $Item) { 'Down' } else { 'Up' }) "^guide: Landing Utility\(1\), flyout Audio Item\($next\)"
    }
    throw "Focus never reached entry $Item of the Flyout: $(Get-GuideLine)"
}

# Whether the card now shows the stand-in's session, by the last `now playing:` line: the only time a control is
# pressed.
function Test-ShowsStandIn([string] $App) {
    $line = Get-LastLine 'now playing'
    $line -match "^`"$([regex]::Escape($title))[^`"]*`" by `"$([regex]::Escape($artist))`" in $([regex]::Escape($App)), "
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # ---- 1. The stand-in's session, and the Dashboard following Windows' sessions.
    # A data folder of the test's own: the source it chooses is saved there, and nowhere the user's Dashboard reads.
    $data = Get-TestFolder 'now-playing-data'
    Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
    New-Item $data -ItemType Directory -Force | Out-Null
    $dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    $exe = Join-Path $PSScriptRoot 'standin.exe'
    $standin = Start-Process $exe -ArgumentList @('media', '--title', "`"$title`"", '--artist', "`"$artist`"", '--seconds', '300') `
        -WorkingDirectory $PSScriptRoot -PassThru
    Say "started the stand-in's media session, pid $($standin.Id): `"$title`""
    if (-not (Wait-StandInLog '^\[pid \d+\] media: session up' 20)) {
        throw "The stand-in's session never came up (standin.log: $((Get-StandInLog | Select-Object -Last 3) -join ' | '))"
    }
    $listed = Wait-DashboardLog "^media: .*`"$([regex]::Escape($title))`" playing" 20
    if (-not $listed) { throw "Windows never listed the stand-in's session to the Dashboard (media: $(Get-LastLine 'media'))" }
    # `media: 2 sessions, current "...": <App> (<app id>) "<title>" playing; ...`: the stand-in's app as named.
    if ($listed -notmatch "(?:: |; )([^;:`"]+?) \(([^)]*)\) `"$([regex]::Escape($title))`"") { throw "No app name for the stand-in in: $listed" }
    $app, $appId = $Matches[1], $Matches[2]
    Say "Windows lists the stand-in as $appId, which the Dashboard calls '$app'"

    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 2
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }

    # ---- 2. Settings > Guide > Now Playing source, to the stand-in's app.
    for ($i = 0; $i -lt 6 -and (Get-LastLine 'focus') -notmatch 'TopBar\(Settings\)$'; $i++) { Walk Right '^focus: ' }
    if ((Get-LastLine 'focus') -notmatch 'TopBar\(Settings\)$') { throw "Focus never reached the top bar's Settings (focus: $(Get-LastLine 'focus'))" }
    Step A '^focus: Settings: Rail: Theme settings$' | Out-Null
    Step Right '^focus: Settings: Theme settings: Personalisation$' | Out-Null
    Step Down '^focus: Settings: Theme settings: Guide$' | Out-Null
    Step A '^focus: Settings: Guide: Use the Guide$' | Out-Null
    Step Down '^focus: Settings: Guide: Now Playing source$' | Out-Null
    Step A '^focus: Settings: Guide: Now Playing source, list on "' '01-source-list' | Out-Null
    # Down the list, a press at a time, to the stand-in's app: Any, Spotify, then the apps seen this run.
    $wanted = "Settings: Guide: Now Playing source, list on `"$app`""
    # Two presses that move nothing are the list's end; one may be a press the Dashboard never saw.
    $still = 0
    for ($i = 0; $i -lt 30 -and (Get-LastLine 'focus') -ne $wanted -and $still -lt 2; $i++) {
        $before = Get-LastLine 'focus'
        Walk Down '^focus: Settings: Guide: Now Playing source, list on '
        $still = if ((Get-LastLine 'focus') -eq $before) { $still + 1 } else { 0 }
    }
    if ((Get-LastLine 'focus') -ne $wanted) { throw "The source's list has no '$app' (focus: $(Get-LastLine 'focus'))" }
    $mark = Get-DashboardLogLength
    Step A '^focus: Settings: Guide: Now Playing source$' '02-source-chosen' | Out-Null
    if (-not (Wait-DashboardLog "^settings: guide\.now_playing_source = $([regex]::Escape($app))$" 8 $mark)) { throw "The source '$app' was not saved" }

    # ---- 3. The Audio Flyout's card.
    if (-not (Step Guide '^guide: Landing Entry\(0\)$')) { throw "The Guide did not open (it is at: $(Get-GuideLine))" }
    Move-ToUtilities
    Step Right '^guide: Landing Utility\(1\)$' | Out-Null
    $mark = Get-DashboardLogLength
    Step A '^guide: Landing Utility\(1\), flyout Audio Item\(0\)' | Out-Null
    $line = Wait-DashboardLog "^now playing: `"$([regex]::Escape($title))`" by `"$([regex]::Escape($artist))`" in $([regex]::Escape($app)), playing, " 10 $mark
    Say "the card: $(if ($line) { $line } else { "NOT the stand-in's track (now playing: $(Get-LastLine 'now playing'))" })"
    if (-not $line) { throw "The card does not show the stand-in's track" }
    Move-ToItem 4
    Start-Sleep -Milliseconds 3000
    Save-Screen '03-card'
    Step Down '^guide: Landing Utility\(1\), flyout Audio Control\(PlayPause\)' '04-play-pause' | Out-Null

    # ---- 4. Play/Pause.
    if (-not (Test-ShowsStandIn $app)) { throw "Not pressing: the card is not the stand-in's (now playing: $(Get-LastLine 'now playing'))" }
    $mark, $standinMark = (Get-DashboardLogLength), @(Get-StandInLog).Count
    Push-Button A
    $pressed = Wait-StandInLog '^\[pid \d+\] media: pressed Pause$' 10 $standinMark
    Say "the stand-in: $(if ($pressed) { $pressed } else { "NOT pressed Pause ($((Get-StandInLog | Select-Object -Last 2) -join ' | '))" })"
    if (-not $pressed) { Fail 'A on Play/Pause did not reach the stand-in as Pause' }
    $paused = Wait-DashboardLog "^now playing: `"$([regex]::Escape($title))`" .* in $([regex]::Escape($app)), paused, " 10 $mark
    Say "the card: $(if ($paused) { $paused } else { "NOT paused (now playing: $(Get-LastLine 'now playing'))" })"
    if (-not $paused) { Fail "the card did not show the stand-in's session paused" }
    Start-Sleep -Milliseconds 3000
    Save-Screen '05-paused'

    # ---- 5. Next.
    Step Right '^guide: Landing Utility\(1\), flyout Audio Control\(Next\)' | Out-Null
    if (-not (Test-ShowsStandIn $app)) { throw "Not pressing: the card is not the stand-in's (now playing: $(Get-LastLine 'now playing'))" }
    $mark, $standinMark = (Get-DashboardLogLength), @(Get-StandInLog).Count
    Push-Button A
    $pressed = Wait-StandInLog '^\[pid \d+\] media: pressed Next$' 10 $standinMark
    Say "the stand-in: $(if ($pressed) { $pressed } else { "NOT pressed Next ($((Get-StandInLog | Select-Object -Last 2) -join ' | '))" })"
    if (-not $pressed) { Fail 'A on Next did not reach the stand-in as Next' }
    $next = Wait-DashboardLog "^now playing: `"$([regex]::Escape($title)) \(2\)`" .* in $([regex]::Escape($app)), " 10 $mark
    Say "the card: $(if ($next) { $next } else { "NOT the next track (now playing: $(Get-LastLine 'now playing'))" })"
    if (-not $next) { Fail "the card did not show the stand-in's next track" }
    Start-Sleep -Milliseconds 3000
    Save-Screen '06-next'

    # ---- 6. Out.
    Step B '^guide: Landing Utility\(1\)$' | Out-Null
    Step B '^guide: closed$' | Out-Null
    if ($dashboard.HasExited) { Fail "the Dashboard exited with $($dashboard.ExitCode)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: the Guide at: $(Get-GuideLine); focus: $(Get-LastLine 'focus'); now playing: $(Get-LastLine 'now playing')"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    # Only the stand-in this test started, from its own folder.
    Get-OwnProcess standin | Stop-Process -Force -ErrorAction SilentlyContinue
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
