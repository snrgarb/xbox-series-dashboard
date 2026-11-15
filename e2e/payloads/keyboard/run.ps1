# Text entry on the Console keyboard, with the virtual pad and the VM's own keyboard: the keys walked, the pad's
# shortcuts, the three layers, a name kept and a name dropped, a secret, and a physical keyboard's typing, which only
# a real window can show. Text entry is reached through the stand-in page `--page keyboard` starts on, until Full
# library's name box and Settings open it themselves (#53, #57): then this walks to it from Home instead.
#
# Judged by what --log-focus says after each press, a press at a time: `focus: TextEntry: A; "Ha|" lower letters` is
# the page, the focused key, then the text with a bar at the caret, Shift and the layer; under it the stand-in page
# says `focus: Keyboard: Name; name "Ha" group "Couch co-op" secret 0, kept`. Passes when every press is followed by
# the line expected of it and the Dashboard stays up. The screenshots are to look at beside the Bundle's pictures 13,
# 15 and 17: here the fonts and the keys' glyphs are Windows' own, which no test on Linux draws.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }

# Waits for a new focus line matching $Expect, a regular expression for what comes after "focus: ", among those
# after the first $Before. Then a rest for the picture, if one is asked for: the software renderer takes most of a
# second a frame.
function Expect([string] $Did, [int] $Before, [string] $Expect, [string] $Name = '') {
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $Before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        Start-Sleep -Milliseconds 2500
        Save-Screen $Name
    }
    if ($got) {
        Say "$Did -> $got"
    } else {
        Say "$Did -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Did should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

# One press of the pad that must be followed by a new focus line matching $Expect.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    Expect $Button $before $Expect $Name
}

# Keys of the VM's keyboard, as SendKeys writes them ('h', '{BS}', '{ESC}'), that must be followed by a new focus
# line matching $Expect. They go to the window in front, which must be the Dashboard's.
function Type-Keys([string] $Keys, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Say "typed $Keys"
    Expect "typing $Keys" $before $Expect $Name
}

# Presses a direction until focus is on the key $Key, $Most times at most: a walk along the keys is judged by where
# it ends, not by how many presses it took, since a press can count twice where the VM's clock jumps.
function Walk([string] $Button, [string] $Key, [int] $Most = 14) {
    $pattern = "^focus: TextEntry: $Key;"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -match $pattern) {
        Say "$Button until $(Focus-Now)"
    } else {
        Say "$Button never reached the key $Key`: $(Focus-Now)"
        $script:failures += "$Button never reached the key $Key`: '$(Focus-Now)'"
    }
}

# Presses a direction until it moves focus no more: an edge of the board, which does not wrap. The key there must
# be $Key.
function Walk-ToEdge([string] $Button, [string] $Key, [int] $Most = 14) {
    $reached = $false
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1200
        if ((Focus-Now) -eq $was) { $reached = $true; break }
    }
    if ($reached -and (Focus-Now) -match "^focus: TextEntry: $Key;") {
        Say "$Button to the board's edge: $(Focus-Now)"
    } else {
        Say "$Button did not end on the key $Key`: $(Focus-Now)"
        $script:failures += "$Button should have ended on the key $Key, and focus is '$(Focus-Now)'"
    }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $script:dashboard = Start-Dashboard @('--log-focus', '--page', 'keyboard')
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    # The greeting's Toast comes and goes: it is not in the pictures, and the Guide button is the Guide's again.
    Wait-Greeting | Out-Null
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
    Save-Screen '00-the-stand-in-page'

    # A on the Name box: text entry over the page, "Filter by name", empty, focus on Q, Shift on (its key lit).
    Press A 'TextEntry: Q; "\|" upper letters' '01-text-entry-shift-on'

    # "Ha", as the Bundle's picture 13 has it: down to A's row, along to H, A; back to A, A. The first character is
    # a capital and Shift goes off with it.
    Walk Down 'A'
    Walk Right 'H'
    Press A 'TextEntry: H; "H\|" lower letters'
    Walk Left 'A'
    Press A 'TextEntry: A; "Ha\|" lower letters' '02-ha-typed'

    # The pad's shortcuts: X backspace, Y space, LB and RB the caret, the left stick's click Shift.
    Press X 'TextEntry: A; "H\|" lower letters'
    Press Y 'TextEntry: A; "H \|" lower letters'
    Press LeftShoulder 'TextEntry: A; "H\| " lower letters'
    Press RightShoulder 'TextEntry: A; "H \|" lower letters'
    Press LeftThumb 'TextEntry: A; "H \|" upper letters'
    Press A 'TextEntry: A; "H A\|" lower letters'

    # The symbols key, by walking to it: up the left of the board. The layer stays through what is typed on it.
    Walk Up '1'
    Walk Left 'Symbols'
    Press A 'TextEntry: Symbols; "H A\|" lower symbols' '03-symbols'
    Walk Right '1'
    Walk Down 'Q'
    Press A 'TextEntry: Q; "H A!\|" lower symbols'
    Walk Up '1'
    Walk Left 'Symbols'
    Press A 'TextEntry: Symbols; "H A!\|" lower letters'

    # The board's edges: nothing wraps. Left of the symbols key and above it; down its side to Shift; along the
    # foot to Enter; up the right side to backspace.
    Walk-ToEdge Left 'Symbols'
    Walk-ToEdge Up 'Symbols'
    Walk-ToEdge Down 'Shift'
    Walk-ToEdge Right 'Enter'
    Walk-ToEdge Up 'Backspace'
    Walk-ToEdge Right 'Backspace'

    # A on backspace three times, then Menu keeps what is left: the page under it has the name.
    Press A 'TextEntry: Backspace; "H A\|" lower letters'
    Press A 'TextEntry: Backspace; "H \|" lower letters'
    Press A 'TextEntry: Backspace; "H\|" lower letters'
    Press Start 'Keyboard: Name; name "H" group "Couch co-op" secret 0, kept' '04-kept'

    # Opened again over the name kept, changed, and dropped with B: the page has the name it had.
    Press A 'TextEntry: Q; "H\|" lower letters'
    Press A 'TextEntry: Q; "Hq\|" lower letters'
    Press B 'Keyboard: Name; name "H" group "Couch co-op" secret 0, dropped'

    # The VM's keyboard: letters as typed, Space, Backspace; the arrows move along the keys and Enter presses the
    # focused one; Escape drops, and the Dashboard stays up.
    Press A 'TextEntry: Q; "H\|" lower letters'
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
    Type-Keys 'a' 'TextEntry: Q; "Ha\|" lower letters'
    Type-Keys 'L' 'TextEntry: Q; "HaL\|" lower letters'
    Type-Keys ' ' 'TextEntry: Q; "HaL \|" lower letters'
    Type-Keys '{BS}' 'TextEntry: Q; "HaL\|" lower letters'
    Type-Keys '{RIGHT}' 'TextEntry: W; "HaL\|" lower letters'
    Type-Keys '{ENTER}' 'TextEntry: W; "HaLw\|" lower letters' '05-typed-on-the-keyboard'
    Type-Keys '{ESC}' 'Keyboard: Name; name "H" group "Couch co-op" secret 0, dropped'
    Start-Sleep -Seconds 1
    if ($dashboard.HasExited) { throw 'Escape in text entry closed the Dashboard' }

    # A secret: from empty, a star for each character in the log and a dot in the field; kept, the page has its
    # length and nothing more.
    Press Down 'Keyboard: Group; .*'
    Press Down 'Keyboard: Secret; .*'
    Press A 'TextEntry: Q; "\|" upper letters'
    Press A 'TextEntry: Q; "\*\|" lower letters'
    Press A 'TextEntry: Q; "\*\*\|" lower letters' '06-a-secret'
    Press Start 'Keyboard: Secret; name "H" group "Couch co-op" secret 2, kept'

    if ($dashboard.HasExited) { throw "The Dashboard exited with $($dashboard.ExitCode)" }
    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
    try { Save-Screen 'error' } catch {}
} finally {
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result"
    Stop-Dashboard
    Disconnect-Pad
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
