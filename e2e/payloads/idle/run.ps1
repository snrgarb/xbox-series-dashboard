# The Idle screen (docs/spec/idle-screen.md), with its times cut to 3 s and 6 s (`--idle-after 3,6`):
#
#   1. Over Home, focus on the Recent row's first tile: `idle: dimmed`, then `idle: showing things`, a picture of each.
#      Right wakes it (`idle: woken by Right`) and does nothing more: no `focus:` line. Right again moves focus.
#   2. Over the stand-in game, started from its hub: Dimmed, then Showing things, on the Overlay, with the game seen
#      dimmed under it; A wakes it and does nothing more: the game stays in front, and nothing is launched or opened.
#
# Judged by the Dashboard's `idle:` and `focus:` lines (--log-focus), the window in front and pixels over the game.
#
# The idle time counts from the last input anywhere, the user's own mouse and keyboard on the TV PC among them. The
# times are a few seconds so that nobody has to keep off the machine for long; and a stage that someone else's input
# woke early (`idle: woken by input`, which no press of the test's makes) is said in the steps and waited for again,
# at most three times.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'
$gameBlue = @(16, 32, 80)
# The stand-in game under the dim, 70% black: what is left of it, at a point the panel never covers when dimmed.
$dimmedGame = @(5, 10, 24)

function Front { $f = Get-Foreground; "$($f.Process) '$($f.Title)'" }
function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

function Get-LastLine([string] $Prefix) {
    $lines = @(Get-DashboardLog | Where-Object { $_ -match "^${Prefix}: " })
    if ($lines.Count) { $lines[-1] -replace "^${Prefix}: ", '' } else { '' }
}

function Expect-Pixel([double] $X, [double] $Y, $Colour, [string] $What) {
    $ok = Wait-Until { Test-Colour (Get-DesignPixel $X $Y) $Colour 6 } 8
    $seen = Get-DesignPixel $X $Y
    Say "at ($X, $Y), ${What}: seen $(Format-Colour $seen), expected $(Format-Colour $Colour)"
    if (-not $ok) { $script:failures += "at ($X, $Y) expected $What $(Format-Colour $Colour), seen $(Format-Colour $seen)" }
}

function Expect-Front([string] $Title, [string] $When) {
    $ok = Wait-Until { (Get-Foreground).Title -eq $Title } 8
    Say "${When}: foreground $(Front)"
    if (-not $ok) { $script:failures += "${When}: '$Title' is not in front: $(Front)" }
}

# Waits for the Idle screen to dim and then show things, from no input now: a picture of each, named with $Prefix.
# Woken early by input the test did not make, it says so and waits again from there. True once it is showing things.
function Wait-Idle([string] $Prefix, [scriptblock] $Dimmed = {}, [int] $From = -1) {
    for ($try = 1; $try -le 3; $try++) {
        $mark = if ($try -eq 1 -and $From -ge 0) { $From } else { Get-DashboardLogLength }
        $dim = Wait-DashboardLog '^idle: (dimmed|woken by input) at ' 15 $mark
        if ($dim -match 'woken by input') { Say "note: someone's input woke it before it dimmed ($dim); waiting again"; continue }
        if (-not $dim) { Fail "$Prefix`: never dimmed (idle: $(Get-LastLine 'idle'))"; return $false }
        Say "${Prefix}: $dim"
        # The dim fades in over 800 ms; a frame or two of the software renderer besides.
        Start-Sleep -Milliseconds 1500
        Save-Screen "$Prefix-dimmed"
        & $Dimmed
        $showing = Wait-DashboardLog '^idle: (showing things|woken by input) at ' 15 $mark
        if ($showing -match 'woken by input') { Say "note: someone's input woke it while it was dimmed ($showing); waiting again"; continue }
        if (-not $showing) { Fail "$Prefix`: never showed things (idle: $(Get-LastLine 'idle'))"; return $false }
        Say "${Prefix}: $showing"
        Start-Sleep -Milliseconds 2000
        if ((Get-LastLine 'idle') -match '^woken by input') { Say "note: someone's input woke it while it showed things; waiting again"; continue }
        Save-Screen "$Prefix-showing-things"
        return $true
    }
    Fail "${Prefix}: woken by someone's input three times over; nothing judged"
    $false
}

# A press that must only wake the screen: `idle: woken by <Button>`, and no `focus:` line after it.
function Wake([string] $Button) {
    $mark = Get-DashboardLogLength
    $focus = Get-LastLine 'focus'
    Push-Button $Button
    $woken = Wait-DashboardLog "^idle: woken by " 8 $mark
    Say "$Button wakes it: $(if ($woken) { $woken } else { "NO 'idle: woken by' line (idle: $(Get-LastLine 'idle'))" })"
    if (-not $woken) { Fail "$Button did not wake the Idle screen"; return }
    if ($woken -notmatch "^idle: woken by $Button at ") { Say "note: woken by something else first: $woken" }
    Start-Sleep -Milliseconds 1500
    $moved = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^(focus|guide|launch): ' })
    if ($moved.Count) { Fail "the press that woke it did more: $($moved -join ' | ')" } else { Say "nothing moved: focus is still $focus" }
    Save-Screen "woken-by-$($Button.ToLower())"
}

try {
    Close-WindowsPrompts
    Connect-Pad
    $library = Initialize-FakeSteam
    $game = @($library.games | Where-Object { $_.installed -and $_.last_played -gt 0 })[0]
    if (-not $game) { throw 'The fake Steam has no installed game that was played' }
    $env:DASHBOARD_NEEDS_USER_MS = '120000'

    # A data folder of the test's own: nothing of the user's Dashboard's Settings is read or written.
    $data = Get-TestFolder 'idle-data'
    Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
    New-Item $data -ItemType Directory -Force | Out-Null
    $dashboard = Start-Dashboard @('--log-focus', '--idle-after', '3,6', '--data-dir', "`"$data`"")
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    $times = Wait-DashboardLog '^idle: dim after ' 5
    Say "the times: $times"
    if ($times -notmatch '^idle: dim after 3 s, show things after 6 s \(--idle-after\)$') { Fail "the times are not --idle-after's: $times" }
    # The greeting's Toast neither wakes it nor counts; it is waited out so that it is not in the pictures.
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    if (-not (Wait-DashboardLog '^overlay: configured' 15)) { throw 'The Overlay was not made' }

    # ---- 1. Over Home.
    Push-Button Down
    if (-not (Wait-DashboardLog '^focus: Recent\(0\)' 10)) { throw "Down did not put focus on the Recent row's first tile" }
    if (Wait-Idle 'home') {
        Wake 'Right'
        $mark = Get-DashboardLogLength
        Push-Button Right
        $moved = Wait-DashboardLog '^focus: Recent\(1\)' 5 $mark
        Say "Right again: $(if ($moved) { $moved } else { "NO move (focus: $(Get-LastLine 'focus'))" })"
        if (-not $moved) { Fail 'the press after the wake did not move focus' }
    }

    # ---- 2. Over the stand-in game.
    Push-Button Left
    Wait-DashboardLog '^focus: Recent\(0\)' 5 | Out-Null
    $mark = Get-DashboardLogLength
    Push-Button A
    if (-not (Wait-DashboardLog '^focus: GameHub: \d+ Hub Play$' 10 $mark)) { throw "A did not open the game's hub (focus: $(Get-LastLine 'focus'))" }
    Push-Button A
    if (-not (Wait-DashboardLog "^launch: $([regex]::Escape($game.name)) \($($game.source_id)\) asked of Steam" 15 $mark)) { throw "$($game.name) was not asked of Steam" }
    if (-not (Wait-DashboardLog 'the game has booted' 60 $mark)) { throw "$($game.name) never counted as booted" }
    if (-not (Wait-DashboardLog 'Launch screen gone' 15 $mark)) { Fail 'the Launch screen did not go' }
    Expect-Front $game.name "$($game.name) started"
    # Counted from the Launch screen going, which may be before this line is reached.
    if (Wait-Idle 'game' { Expect-Pixel 1230 700 $dimmedGame 'the game under the dim' } $mark) {
        Expect-Front $game.name 'showing things over the game'
        Wake 'A'
        Expect-Front $game.name 'after the wake'
        Expect-Pixel 1230 700 $gameBlue 'the game, the dim gone'
        if (@(Get-StandInGame).Count -ne 1) { Fail "$(@(Get-StandInGame).Count) stand-in games are running, not one" }
    }
    if ($dashboard.HasExited) { Fail "the Dashboard exited with $($dashboard.ExitCode)" }

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
    Say "at the error: idle: $(Get-LastLine 'idle'); focus: $(Get-LastLine 'focus'); front: $(Front)"
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-StandInGame
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
