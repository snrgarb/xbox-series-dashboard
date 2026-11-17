# Settings' Groups, walked with the virtual pad on the fake Steam library (docs/spec/filters.md, section 6): a Group
# hidden and seen gone from Home, two Groups swapped and Home's order seen to follow, one added by name through text
# entry, and everything put back as it was.
#
# Judged by the Dashboard's log, a press at a time. --log-focus says where focus is after every press that moved it
# or changed what the Groups page says of its Row (`focus: Settings: Groups: Most played; shown`), and Home's Groups
# in order with the first frame and whenever they change (`groups: Couch co-op, Short sessions, Most played,
# Recently added`). A change carried out on the library database is `library: filter <id> ...`. Which Groups there
# are and in what order is the fixture's and the database's to say: the test reads them from the first `groups:` line
# and walks by name, never by place.
#
# The pictures are to look at beside the Bundle's of Settings (docs/spec/pictures.md, 30 and 32: Personalisation and
# a page of Rows) and of the Tile menu (23 and 26: the menu and its question), which the Groups page is built from;
# the Bundle has no Groups page of its own.
#
# The data folder is the test's own, empty as it starts, and is removed at the end.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-groups-test'
$added = 'Zed Group'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Groups-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^groups: ' }) }

# Home's Groups as the log last said them, in order.
function Groups-Now {
    $line = Groups-Lines | Select-Object -Last 1
    if (-not $line) { return @() }
    $names = $line -replace '^groups: ', ''
    if (-not $names) { return @() }
    @($names -split ', ')
}

# Waits for a `groups:` line after the first $After lines of the log whose Groups are $Expect, in that order.
function Expect-Groups([string[]] $Expect, [int] $After, [string] $Why) {
    $want = "groups: $($Expect -join ', ')"
    $got = Wait-DashboardLog "^$([regex]::Escape($want))`$" 10 $After
    if ($got) {
        Say "$Why -> $got"
    } else {
        Say "$Why -> NOT '$want': $(Groups-Lines | Select-Object -Last 1)"
        $script:failures += "$Why should have given '$want', and the log's last is '$(Groups-Lines | Select-Object -Last 1)'"
    }
}

# One press that must be followed by a new focus line matching $Expect, a regular expression for what comes after
# "focus: ". Then a rest for the picture, if one is asked for: the software renderer takes most of a second a frame.
function Press([string] $Button, [string] $Expect, [string] $Name = '') {
    $before = @(Focus-Lines).Count
    Push-Button $Button
    $pattern = "^focus: $Expect`$"
    $got = Wait-Until { $lines = @(Focus-Lines); if ($lines.Count -gt $before -and $lines[-1] -match $pattern) { $lines[-1] } } 8
    if ($Name) {
        Start-Sleep -Milliseconds 3000
        Save-Screen $Name
    }
    if ($got) {
        Say "$Button -> $got"
    } else {
        Say "$Button -> NOT '$Expect': $(Focus-Now)"
        $script:failures += "$Button should have given 'focus: $Expect', and the log's last is '$(Focus-Now)'"
    }
}

# Presses a direction until focus is where $Expect says, $Most times at most: a press held 120 ms can count twice
# where the VM's clock jumps, and one can be lost, so a walk is judged by where it ends, not by how many presses.
function Walk([string] $Button, [string] $Expect, [int] $Most = 12) {
    $pattern = "^focus: $Expect`$"
    foreach ($press in 1..$Most) {
        if ((Focus-Now) -match $pattern) { break }
        Push-Button $Button
        Start-Sleep -Milliseconds 800
    }
    if ((Focus-Now) -match $pattern) {
        Say "$Button until $(Focus-Now)"
    } else {
        Say "$Button never reached '$Expect': $(Focus-Now)"
        $script:failures += "$Button never reached 'focus: $Expect': '$(Focus-Now)'"
    }
}

# Presses a direction until it moves focus no more: the top or the foot of the Rows, which do not wrap.
function Walk-ToEdge([string] $Button, [int] $Most = 14) {
    foreach ($press in 1..$Most) {
        $was = Focus-Now
        Push-Button $Button
        Start-Sleep -Milliseconds 1500
        if ((Focus-Now) -eq $was) { break }
    }
    Say "$Button to the end: $(Focus-Now)"
}

# Focus on the Row of the Group named $Group: up to the top, then down until it is found.
function Go-To([string] $Group) {
    Walk-ToEdge Up
    Walk Down "Settings: Groups: $([regex]::Escape($Group)); (not )?shown" 14
}

# A on a Group's Row turns its tick over: the Row says so, and the database is told.
function Turn-Over([string] $Group, [string] $Now, [string] $Name = '') {
    $mark = Get-DashboardLogLength
    Press A "Settings: Groups: $([regex]::Escape($Group)); $Now" $Name
    if (-not (Wait-DashboardLog "^library: filter \d+ is $Now`$" 8 $mark)) {
        $script:failures += "A on '$Group' should have logged 'library: filter <id> is $Now'"
    }
}

# Menu on the Row focus is on, then down the menu to its row $Row, and A. The menu has the pad until it closes.
function Menu-Row([string] $Group, [string] $Row, [string] $Name = '') {
    $g = [regex]::Escape($Group)
    Press Start "Settings: Groups: Menu\(.+\) of $g"
    Walk-ToEdge Up 4
    Walk Down "Settings: Groups: Menu\($Row\) of $g" 4
    if ($Name) {
        Start-Sleep -Milliseconds 2500
        Save-Screen $Name
    }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
    # Steam is read as the window comes up: the Groups once it has been.
    Wait-DashboardLog '^groups: ' 30 | Out-Null
    Start-Sleep -Seconds 2
    $initial = @(Groups-Now)
    Say "Home's Groups: $($initial -join ', ')"
    if ($initial.Count -lt 3) { throw "the fixture should give Home three Groups at least, and it has: $($initial -join ', ')" }

    # Settings by the top bar's button, the rail down to Games & apps, Groups the Library section's third box.
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: General'
    Press Down 'Settings: Rail: Account'
    Press Down 'Settings: Rail: Games & apps'
    Press Right 'Settings: Games & apps: Launchers'
    Press Down 'Settings: Games & apps: Games & apps'
    Press Down 'Settings: Games & apps: Groups' '01-games-and-apps-groups'
    Press A 'Settings: Groups: .+; (not )?shown' '02-groups-page'

    # --- Hide Home's first Group: it leaves Home at once.
    $hidden = $initial[0]
    Go-To $hidden
    $mark = Get-DashboardLogLength
    Turn-Over $hidden 'not shown' '03-group-hidden'
    $shown = @($initial | Select-Object -Skip 1)
    Expect-Groups $shown $mark "hiding $hidden"

    # --- Swap the two Groups now first on Home: the second moved up, and Home's order follows.
    $one = $shown[0]
    $two = $shown[1]
    Go-To $two
    Menu-Row $two 'Move up' '04-group-menu'
    $mark = Get-DashboardLogLength
    # Every filter counts, shown or not: Move up until Home has it before the other. How many moves that took is
    # counted, to move it back as many.
    $swapped = @($two, $one) + @($shown | Select-Object -Skip 2)
    foreach ($again in 1..8) {
        if (((Groups-Now) -join ', ') -eq ($swapped -join ', ')) { break }
        if ((Focus-Now) -notmatch '^focus: Settings: Groups: Menu\(Move up\) of ') { break }
        Push-Button A
        Start-Sleep -Milliseconds 1500
    }
    $ups = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^library: filter \d+ moved by -1$' }).Count
    if ($ups -lt 1) { $failures += "Move up on '$two' should have logged 'library: filter <id> moved by -1'" }
    Say "moved $two up $ups times"
    Expect-Groups $swapped $mark "moving $two up"
    Press B "Settings: Groups: $([regex]::Escape($two)); shown"

    # --- New Group, named through text entry: last on Home.
    Walk-ToEdge Down
    if ((Focus-Now) -ne 'focus: Settings: Groups: New Group') { $failures += "the last Row should be New Group: '$(Focus-Now)'" }
    $mark = Get-DashboardLogLength
    Press A 'Settings: Group: Rename; "New Group", \d+ games' '05-group-page'
    if (-not (Wait-DashboardLog '^library: filter \d+ made: "New Group"$' 8 $mark)) { $failures += 'New Group should have logged its making' }
    Press A 'TextEntry: .+'
    [System.Windows.Forms.SendKeys]::SendWait('{BS}' * 'New Group'.Length)
    Start-Sleep -Milliseconds 1500
    [System.Windows.Forms.SendKeys]::SendWait($added)
    Start-Sleep -Milliseconds 2000
    Say "typed: $(Focus-Now)"
    Save-Screen '06-group-name'
    $mark = Get-DashboardLogLength
    Press Start "Settings: Group: Rename; `"$added`", \d+ games"
    if (-not (Wait-DashboardLog "^library: filter \d+ set: `"$added`"$" 8 $mark)) { $failures += "the name should have been saved as '$added'" }
    Expect-Groups ($swapped + @($added)) $mark "adding $added"
    Press B "Settings: Groups: $([regex]::Escape($added)); shown" '07-group-added'

    # --- Everything back: the new Group deleted after its question, the two in their order, the first shown again.
    Menu-Row $added 'Delete'
    $mark = Get-DashboardLogLength
    Press A "Settings: Groups: Prompt\(Delete\) of $([regex]::Escape($added))" '08-delete-prompt'
    Press A 'Settings: Groups: New Group'
    if (-not (Wait-DashboardLog '^library: filter \d+ deleted$' 8 $mark)) { $failures += "Delete should have deleted '$added'" }
    Expect-Groups $swapped $mark "deleting $added"

    Go-To $two
    Menu-Row $two 'Move down'
    $mark = Get-DashboardLogLength
    foreach ($again in 1..12) {
        $downs = @(Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^library: filter \d+ moved by 1$' }).Count
        if ($downs -ge $ups) { break }
        if ((Focus-Now) -notmatch '^focus: Settings: Groups: Menu\(Move down\) of ') { break }
        Push-Button A
        Start-Sleep -Milliseconds 1500
    }
    Expect-Groups $shown $mark "moving $two back down"
    Press B "Settings: Groups: $([regex]::Escape($two)); shown"

    Go-To $hidden
    $mark = Get-DashboardLogLength
    Turn-Over $hidden 'shown'
    Expect-Groups $initial $mark "showing $hidden again"

    # B, a level at a time, to Home, whose Groups are as they were at the start.
    Press B 'Settings: Games & apps: Groups'
    Press B 'Settings: Rail: Games & apps'
    Press B 'TopBar\(Settings\)' '09-home-after'
    if (((Groups-Now) -join ', ') -ne ($initial -join ', ')) { $failures += "Home's Groups should be as they were: '$((Groups-Now) -join ', ')'" }

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
    if (Test-Path $data) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue }
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
