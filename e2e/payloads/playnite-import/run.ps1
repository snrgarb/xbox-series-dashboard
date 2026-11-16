# Import from Playnite, once (docs/import.md): a made-up Playnite library (`standin make-playnite`) beside the fake
# Steam, whose games it shares, offered at the first start in a Toast, previewed and imported from Settings with the
# virtual pad, and imported again to see that nothing doubles. Playnite's files are only read: the test checks they
# are byte for byte as they were made.
#
# Judged by the Dashboard's log. --log-focus says where focus is after every press that moved it, and how the import
# stands on its page (`focus: Settings: Import from Playnite: Import; ready`, then `; done`). What the import found
# and did is `import: ...`: `import: found Playnite's library at ...`, `import: would bring ...`, one line a change
# (`import: Dune Cartographer (steam 9100080): play time 754 min, was 0 min`, `import: Fathom Nine (steam 9100100): a
# favourite`, `import: Group "Couch picks" made: 2 games as Playnite picked them`), and `import: done: ...`. Home's
# Groups are the `groups:` line. Dune Cartographer, never played in the fake Steam and played last in the made-up
# Playnite, is then the first of Home's recent games, and its hub opens from there (`focus: GameHub: 9100080 ...`).
#
# The data folder is the test's own, empty as it starts, and is removed at the end. Safe mode passes `--playnite` for
# every test (an empty folder for those that make no fake Playnite), so the user's own Playnite is never read.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$data = Get-TestFolder 'dashboard-playnite-import-test'

function Focus-Lines { @(Get-DashboardLog | Where-Object { $_ -match '^focus: ' }) }
function Focus-Now { Focus-Lines | Select-Object -Last 1 }
function Groups-Now { Get-DashboardLog | Where-Object { $_ -match '^groups: ' } | Select-Object -Last 1 }

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

# Presses a direction until focus is where $Expect says, $Most times at most: a walk is judged by where it ends.
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

# A line of the log after the first $After matching $Pattern, or a failure saying $Why.
function Expect-Line([string] $Pattern, [int] $After, [string] $Why, [double] $Seconds = 10) {
    $got = Wait-DashboardLog $Pattern $Seconds $After
    if ($got) { Say "$Why -> $got" } else { $script:failures += "$Why should have logged '$Pattern'" }
    $got
}

# Every file under $Root with its hash: Playnite's files must be as they were made.
function Get-Hashes([string] $Root) {
    Get-ChildItem $Root -Recurse -File | Sort-Object FullName | ForEach-Object { "$($_.FullName) $((Get-FileHash $_.FullName).Hash)" }
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null
    $playnite = Get-FakeRoot 'playnite'
    if (Test-Path $playnite) { Remove-Item $playnite -Recurse -Force }
    $plan = Initialize-FakePlaynite $playnite
    if (@($plan.groups) -notcontains 'Couch picks') { throw "the made-up Playnite should have the Group Couch picks: $(@($plan.groups) -join ', ')" }
    $made = @(Get-Hashes $playnite)
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }

    $script:dashboard = Start-Dashboard @('--log-focus', '--data-dir', "`"$data`"", '--playnite', "`"$playnite`"")
    $first = Wait-DashboardLog 'first frame presented' 60
    if (-not $first) { throw 'The Dashboard never presented a frame' }
    Say "started: $first"
    Start-Sleep -Seconds 4
    if ((Get-Foreground).Title -ne 'Xbox Series Dashboard') { Say "note: put in front: $(Set-Foreground 'Xbox Series Dashboard')" }
    Wait-DashboardLog '^groups: ' 30 | Out-Null
    $before = Groups-Now
    Say "Home's Groups before: $before"

    # --- Offered once, in a Toast, with the stores read: a library found that was never imported.
    Expect-Line '^import: offered: ' 0 'the first start' 30 | Out-Null
    Expect-Line '^toasts: shown .*Import from Playnite' 0 'the offer' 15 | Out-Null
    Save-Screen '01-offer-toast'
    Wait-DashboardLog '^toasts: gone' 20 | Out-Null

    # --- Settings by the top bar's button, Theme settings, Import from Playnite under Games & apps.
    Walk Right 'TopBar\(Settings\)' 5
    Press A 'Settings: Rail: Theme settings'
    Press Right 'Settings: Theme settings: Personalisation'
    Walk Down 'Settings: Theme settings: Import from Playnite' 4
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Line '^focus: Settings: Import from Playnite: Import; ready$' $mark 'the preview' | Out-Null
    Expect-Line "^import: found Playnite's library at .+: 8 games, 5 of them here, 2 not here, 1 of other stores$" $mark 'what was found' | Out-Null
    Expect-Line '^import: would bring play time for 1 game, last played for 1 game, 1 favourite, 1 game hidden, Groups: Couch picks, Installed on Steam$' $mark 'what an import would bring' | Out-Null
    Expect-Line '^import: Group "Released in 2019" cannot come over: it filters by release year$' $mark 'the Group that cannot' | Out-Null
    Start-Sleep -Milliseconds 3000
    Save-Screen '02-preview'

    # --- Import: play time, a favourite, a hidden game and two Groups arrive; Home's Groups follow.
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Line '^focus: Settings: Import from Playnite: Import; done$' $mark 'Import' | Out-Null
    Expect-Line '^import: Dune Cartographer \(steam 9100080\): play time 754 min, was 0 min$' $mark 'play time' | Out-Null
    Expect-Line '^import: Dune Cartographer \(steam 9100080\): last played \d+, was 0$' $mark 'last played' | Out-Null
    Expect-Line '^import: Fathom Nine \(steam 9100100\): a favourite$' $mark 'a favourite' | Out-Null
    Expect-Line '^import: Hollow Pines Motel \(steam 9100120\): hidden$' $mark 'a hidden game' | Out-Null
    Expect-Line '^import: Group "Couch picks" made: 2 games as Playnite picked them$' $mark 'a Group of games' | Out-Null
    Expect-Line '^import: Group "Installed on Steam" made: filter \d+, by its rule$' $mark 'a Group by its rule' | Out-Null
    Expect-Line '^import: done: ' $mark 'the import' | Out-Null
    $groups = Expect-Line '^groups: .*Couch picks, Installed on Steam' $mark "Home's Groups"
    Start-Sleep -Milliseconds 3000
    Save-Screen '03-done'

    # --- Again: nothing new, nothing doubled.
    Press B 'Settings: Theme settings: Import from Playnite'
    $mark = Get-DashboardLogLength
    Push-Button A
    Expect-Line '^focus: Settings: Import from Playnite: Import; nothing new$' $mark 'opened again' | Out-Null
    Expect-Line '^import: would bring nothing new$' $mark 'a second preview' | Out-Null
    Push-Button A
    Start-Sleep -Milliseconds 2000
    if (Get-DashboardLog | Select-Object -Skip $mark | Where-Object { $_ -match '^import: (done|.+\(steam )' }) {
        $failures += 'A with nothing new should import nothing'
    }

    # --- Home: Dune Cartographer, played last in Playnite, is the first recent game; its hub opens from there.
    Press B 'Settings: Theme settings: Import from Playnite'
    Press B 'Settings: Rail: Theme settings'
    Press B 'TopBar\(Settings\)'
    Walk Down 'Recent\(\d+\)' 3
    Walk Left 'Recent\(0\)' 8
    Save-Screen '04-home-after'
    Press A 'GameHub: 9100080 .+' '05-hub'
    Press B 'Recent\(0\)'

    # Playnite's files are as they were made.
    $after = @(Get-Hashes $playnite)
    if (($after -join "`n") -ne ($made -join "`n")) { $failures += "Playnite's files changed: $($after -join '; ')" }

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
