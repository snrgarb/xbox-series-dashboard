# The pad's battery (#191; docs/spec/home.md 2.4, docs/research/pad-battery.md), faked in Windows' place with
# `--fake-pad-battery`, since the VM's pad is a virtual one that says nothing of a battery:
#
#   1. Unfaked first: the virtual pad is "wired" or "not known" by Windows' own word, and shows the controller glyph.
#   2. Then a Dashboard each for a percentage (85), a level only (low) and nothing known (none): the `gamepad:
#      battery` line says what it was given, and Home's top bar and the Guide's Status area are pictured.
#
# Judged by the Dashboard's `gamepad:` lines. The pictures are for a person to look at: the cell with "85%" beside it;
# for low and for none the controller glyph (a level only is logged and not shown, #191).
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$dashboardTitle = 'Xbox Series Dashboard'

function Fail([string] $What) { $script:failures += $What; Say "WRONG: $What" }

# Starts a Dashboard with these arguments, waits for the pad and its battery line ($Battery, a regular expression),
# pictures Home and the Guide as $Name, and stops it.
function Show-Battery([string[]] $Arguments, [string] $Battery, [string] $Name) {
    $script:dashboard = Start-Dashboard (@('--log-focus') + $Arguments)
    if (-not (Wait-DashboardLog 'first frame presented' 60)) { throw 'The Dashboard never presented a frame' }
    if (-not (Wait-DashboardLog '^gamepad: pad \d connected' 15)) { Fail "${Name}: the virtual pad was not seen" }
    $line = Wait-DashboardLog "^gamepad: battery $Battery$" 15
    Say "${Name}: $(if ($line) { $line } else { "NO line 'gamepad: battery $Battery'" })"
    if (-not $line) { Fail "${Name}: the battery was not read as '$Battery'" }
    Wait-Greeting | Out-Null
    if ((Get-Foreground).Title -ne $dashboardTitle) { Say "note: put in front: $(Set-Foreground $dashboardTitle)" }
    Start-Sleep -Seconds 2
    Save-Screen "$Name-home"
    $mark = Get-DashboardLogLength
    Push-Button Guide
    if (-not (Wait-DashboardLog '^guide: Landing Entry\(0\)$' 8 $mark)) { Fail "${Name}: the Guide did not open" }
    Start-Sleep -Seconds 3
    Save-Screen "$Name-guide"
    if ($script:dashboard.HasExited) { Fail "${Name}: the Dashboard exited with $($script:dashboard.ExitCode)" }
    Stop-Dashboard
    # Each Dashboard logs to the same file: kept as this run's, and the next starts it afresh.
    Copy-Item (Join-Path $env:E2E_OUT 'dashboard.log') (Join-Path $env:E2E_OUT "$Name-dashboard.log") -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $env:E2E_OUT 'dashboard.log') -ErrorAction SilentlyContinue
}

# The Guide's battery line in a saved picture: each pixel's brightness, row by row.
function Get-StatusLine([string] $Name) {
    $bmp = New-Object System.Drawing.Bitmap (Join-Path $env:E2E_OUT "$Name.png")
    try {
        $sx = $bmp.Width / 1920.0; $sy = $bmp.Height / 1080.0
        foreach ($y in 84..114) {
            foreach ($x in 1560..1775) {
                $c = $bmp.GetPixel([int] ($x * $sx), [int] ($y * $sy))
                [int] (($c.R + $c.G + $c.B) / 3)
            }
        }
    } finally { $bmp.Dispose() }
}

# How many pixels of two battery lines differ by more than a little.
function Compare-StatusLine($A, $B) {
    $n = 0
    for ($i = 0; $i -lt [Math]::Min($A.Count, $B.Count); $i++) { if ([Math]::Abs($A[$i] - $B[$i]) -gt 48) { $n++ } }
    $n
}

try {
    Close-WindowsPrompts
    Connect-Pad
    Initialize-FakeSteam | Out-Null

    # ---- 1. Windows' own word on the virtual pad: a cable, or nothing.
    Show-Battery @() '(wired|not known)' '01-unfaked'
    # ---- 2. Each of the three cases, faked.
    Show-Battery @('--fake-pad-battery', '85') '85%' '02-percent'
    Show-Battery @('--fake-pad-battery', 'low') 'level 1 of 3' '03-level'
    Show-Battery @('--fake-pad-battery', 'none') 'not known' '04-none'

    # A level only is not shown (#191): the Guide's battery line is the same with `low` as with `none`, the glyph,
    # and not the same with 85%, the cell and its figure. The line is x 1560..1775, y 84..114 of the 1920 picture.
    $none = Get-StatusLine '04-none-guide'
    $level = Compare-StatusLine $none (Get-StatusLine '03-level-guide')
    $percent = Compare-StatusLine $none (Get-StatusLine '02-percent-guide')
    Say "the Guide's battery line against none's: low differs in $level pixels, 85% in $percent"
    if ($level -gt 40) { Fail "a level only was drawn: $level pixels of the battery line differ from none's" }
    if ($percent -le 40) { Fail "85% looks like none: only $percent pixels of the battery line differ" }

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
