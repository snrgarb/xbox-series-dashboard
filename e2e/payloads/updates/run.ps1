# Updates (#156; docs/spec/updates.md): a copy of the Dashboard in the test's own folder, taking itself for 0.0.1
# (--pretend-version), finds the build's own version at the stand-in update address (`standin serve-updates`, which
# serves a release of it around the staged dashboard.exe), downloads and checks it, and installs it: the copy's
# dashboard.exe is replaced by renaming, the new version starts with --handover and shows its first frame, and the
# old one exits 0, kept as dashboard.0.0.1.old. Then, from a fresh copy, a release whose manifest gives another
# SHA-256 is refused and nothing in the folder changes. Driven by the virtual pad in Settings > About, and judged by
# the Dashboard's log (`update: ...`, `focus: Settings: About: ...`).
#
# Safe on the TV PC: the copies, their data folders and their logs are under the test's folder; the update address
# is the stand-in's on 127.0.0.1, never GitHub; the user's installed copy in %LOCALAPPDATA%\Programs is never touched
# (the test refuses to run if its folder were that one). Needs only the harness's dashboard.exe and standin.exe:
#   e2e/htpc/run.sh updates
#
# Passes when:
#   - the copy starts, logs `taking itself for 0.0.1`, and Settings > About's focus line says Version 0.0.1;
#   - A on Check for updates: `update: checking http://127.0.0.1:<port>/releases/latest`, then `downloading <v>`,
#     the zip matching its manifest, `ready <v>`, and the focus line `ready <v>`; the stand-in was asked with the
#     User-Agent dashboard-updates and no Authorization;
#   - A on Install: `update: ... has shown its first frame: this one exits`, the old process exits 0, a process of the
#     copy's dashboard.exe runs with `--handover <old pid>`, the log says `update: taking over from pid <old pid>` and
#     `dashboard <v>` after it, the folder holds dashboard.exe (the staged build's bytes) and dashboard.0.0.1.old (the
#     copy's), and no dashboard.exe.new;
#   - from a fresh copy and a stand-in with --bad-hash: `update: refused <v>`, the focus line `bad-download <v>`, and
#     the folder holds dashboard.exe alone, unchanged.
#
# Bounded: the whole payload ends within $Budget seconds whatever happens.
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'lib\E2E.psm1') -Force
$result = 'FAIL'
$failures = @()
$Budget = 300
$clock = [Diagnostics.Stopwatch]::StartNew()
$started = @()
$RealInstall = Join-Path $env:LOCALAPPDATA 'Programs\XboxSeriesDashboard'
$standinLog = Join-Path $Out 'standin.log'
$Pretend = '0.0.1'

function Get-Limit([double] $Want) { [Math]::Max(1, [Math]::Min($Want, $Budget - 15 - $clock.Elapsed.TotalSeconds)) }

function Expect([bool] $Ok, [string] $What) {
    Say "$(if ($Ok) { 'ok' } else { 'NOT' }): $What"
    if (-not $Ok) { $script:failures += $What }
}

# Lines of a log file, read while it is written.
function Read-Log([string] $Path) {
    if (-not (Test-Path $Path)) { return @() }
    $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $file = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
    try { $text = (New-Object IO.StreamReader $file, ([Text.Encoding]::UTF8)).ReadToEnd() } finally { $file.Dispose() }
    @($text -split "\r?\n" | Where-Object { $_ -ne '' })
}

# The first line of $Path matching $Pattern among those after the first $After, within $Seconds; $null if none.
function Wait-Line([string] $Path, [string] $Pattern, [double] $Seconds, [int] $After = 0) {
    Wait-Until { Read-Log $Path | Select-Object -Skip $After | Where-Object { $_ -match $Pattern } | Select-Object -First 1 } (Get-Limit $Seconds)
}

# Expects a line, and says it.
function Expect-Line([string] $Path, [string] $Pattern, [double] $Seconds, [string] $What, [int] $After = 0) {
    $line = Wait-Line $Path $Pattern $Seconds $After
    Expect ([bool] $line) "$What$(if ($line) { ": $line" } else { " (no line matching '$Pattern')" })"
    $line
}

# A fresh copy of the staged dashboard.exe in a folder of the test's own named $Name: its path.
function New-Copy([string] $Name) {
    $folder = Get-TestFolder $Name
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    if ([IO.Path]::GetFullPath($folder).TrimEnd('\') -ieq [IO.Path]::GetFullPath($RealInstall).TrimEnd('\')) {
        throw "the test's folder is the user's install folder: $folder"
    }
    New-Item $folder -ItemType Directory -Force | Out-Null
    Copy-Item (Join-Path $Payload 'dashboard.exe') (Join-Path $folder 'dashboard.exe')
    Join-Path $folder 'dashboard.exe'
}

# The stand-in update address on a free port, serving $Version around the staged dashboard.exe; its address.
function Start-Releases([string] $Version, [string[]] $Extra = @()) {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop()
    $arguments = @('serve-updates', '--exe', "`"$(Join-Path $Payload 'dashboard.exe')`"", '--version', $Version, '--port', "$port") + $Extra
    $server = Start-Process (Join-Path $Payload 'standin.exe') -ArgumentList $arguments -PassThru -WindowStyle Hidden
    $script:started += $server.Id
    $script:releases = $server.Id
    $line = Wait-Line $standinLog "serve-updates: listening on (http://127\.0\.0\.1:$port/releases/latest)" 20
    if (-not $line) { throw "the stand-in update address on port $port never said it was listening" }
    $line -match '(http://\S+)' | Out-Null
    $Matches[1]
}

# Starts the copy at $Exe as 0.0.1, asking $Source, its log in $Log, on Settings; returns its process.
function Start-Copy([string] $Exe, [string] $Source, [string] $Log, [string] $DataName) {
    $data = Get-TestFolder $DataName
    New-Item $data -ItemType Directory -Force | Out-Null
    $arguments = @('--no-startup', '--log-focus', '--page', 'settings', '--pretend-version', $Pretend,
        '--update-source', $Source, '--log-file', "`"$Log`"", '--data-dir', "`"$data`"",
        '--store-art-service', 'http://127.0.0.1:9', '--art-service', 'http://127.0.0.1:9')
    if (Test-SafeMode) { $arguments += @(Get-SafeArguments $arguments) }
    $p = Start-Process -FilePath $Exe -ArgumentList $arguments -WorkingDirectory (Split-Path $Exe) -PassThru
    $p.Handle | Out-Null  # held, so ExitCode can be read once it has exited
    $script:started += $p.Id
    Say "started $Exe (pid $($p.Id)): $($arguments -join ' ')"
    $p
}

# From Settings' rail to About, by the pad, as the screenshot tests walk it; true once the focus line says so.
function Open-About([string] $Log) {
    if (-not (Wait-Line $Log 'first frame presented' 60)) { throw "the copy never presented a frame" }
    Start-Sleep -Seconds 2
    foreach ($button in 'Right', 'Down', 'Down', 'Down', 'Down', 'Right') { Push-Button $button; Start-Sleep -Milliseconds 400 }
    if (-not (Wait-Line $Log '^focus: Settings: Theme settings: About$' 10)) { throw 'focus never came to About in Theme settings' }
    Push-Button 'A'
    Expect-Line $Log "^focus: Settings: About: Version $([regex]::Escape($Pretend))\b.*; Check for updates; unchecked$" 10 'About opens as 0.0.1, focus on Check for updates'
}

# The files in $Folder, by name.
function Get-Names([string] $Folder) { @(Get-ChildItem $Folder -File | ForEach-Object Name | Sort-Object) }

try {
    $staged = Join-Path $Payload 'dashboard.exe'
    $said = & cmd.exe /c "`"$staged`" --version 2>&1"
    $line = @($said | Where-Object { "$_" -match '^dashboard ' }) | Select-Object -First 1
    if ("$line" -notmatch '^dashboard (\d+\.\d+\.\d+)') { throw "no version from dashboard.exe --version: $said" }
    $version = $Matches[1]
    Say "the staged build is $version; the copy takes itself for $Pretend"
    if ([version] $version -le [version] $Pretend) { throw "the build's version $version is not newer than $Pretend" }
    $stagedHash = (Get-FileHash $staged -Algorithm SHA256).Hash
    Close-WindowsPrompts
    Connect-Pad

    # --- Found, downloaded, checked and installed.
    $exe = New-Copy 'update-app'
    $folder = Split-Path $exe
    $source = Start-Releases $version
    $log = Join-Path $Out 'update-app.log'
    $old = Start-Copy $exe $source $log 'update-data'
    Expect-Line $log "^dashboard: taking itself for $([regex]::Escape($Pretend))" 20 'the copy takes itself for 0.0.1' | Out-Null
    Open-About $log | Out-Null
    $mark = (Read-Log $log).Count
    Push-Button 'A'
    Expect-Line $log "^update: checking $([regex]::Escape($source)) \(Check for updates\); this is $([regex]::Escape($Pretend))" 10 'Check for updates asks the stand-in' $mark | Out-Null
    Expect-Line $log "^update: downloading $([regex]::Escape($version))$" 30 'a newer version is found and downloaded' $mark | Out-Null
    Expect-Line $log "^update: $([regex]::Escape($version))'s zip matches its manifest" 60 'its zip matches its manifest' $mark | Out-Null
    Expect-Line $log "^update: ready $([regex]::Escape($version))$" 10 'it is ready' $mark | Out-Null
    Expect (Test-Path (Join-Path $folder 'dashboard.exe.new')) 'the new build is staged as dashboard.exe.new'
    # The stand-in says each request a moment after serving it (it looks every 200 ms), so the zip's line can come
    # after the Dashboard is already ready: wait for all three of this server's lines rather than reading once.
    $server = $script:releases
    $asked = @(Wait-Until {
            $lines = @(Read-Log $standinLog | Where-Object { $_ -match "^\[pid $server\] serve-updates: /" })
            if ($lines.Count -ge 3) { $lines }
        } (Get-Limit 10))
    $asked | ForEach-Object { Say "  stand-in: $_" }
    Expect ($asked.Count -ge 3 -and -not ($asked | Where-Object { $_ -notmatch '\(User-Agent dashboard-updates, no Authorization\)$' })) 'every request carried the plain User-Agent and no Authorization'

    Push-Button 'Right'
    Expect-Line $log "^focus: Settings: About: .*; Install; ready $([regex]::Escape($version))$" 10 'focus on Install, ready' $mark | Out-Null
    $mark = (Read-Log $log).Count
    Push-Button 'A'
    Expect-Line $log "^update: started $([regex]::Escape($version)) \(pid \d+\)" 20 'Install starts the new version' $mark | Out-Null
    Expect-Line $log '^update: taking over from pid \d+' 30 'the new version takes over' $mark | Out-Null
    $handed = Expect-Line $log "^update: $([regex]::Escape($version)) \(pid (\d+)\) has shown its first frame: this one exits \((\d+) ms after its start\)" 40 'the new version shows its first frame, and the old one exits' $mark
    # A real update hands over in about a second: the new one's first frame comes as soon as it would on any start.
    $took = if ($handed -match '\((\d+) ms after its start\)$') { [int] $Matches[1] } else { -1 }
    Expect ($took -ge 0 -and $took -lt 5000) "the new version's first frame came within 5 s of its start ($took ms)"
    $exited = $old.WaitForExit([int] ((Get-Limit 15) * 1000))
    Expect ($exited -and $old.ExitCode -eq 0) "the old version exited 0 (exited: $exited, code $(if ($exited) { $old.ExitCode }))"
    $new = @(Get-CimInstance Win32_Process -Filter "Name = 'dashboard.exe'" | Where-Object {
        $_.ExecutablePath -and $_.ExecutablePath -ieq $exe -and $_.CommandLine -match "--handover $($old.Id)\b" })
    $new | ForEach-Object { $script:started += $_.ProcessId; Say "  new: pid $($_.ProcessId): $($_.CommandLine)" }
    Expect ($new.Count -eq 1) "the copy's dashboard.exe runs with --handover $($old.Id)"
    Expect ([bool] (Read-Log $log | Select-Object -Skip $mark | Where-Object { $_ -match "^dashboard $([regex]::Escape($version))\b" })) "the new process says dashboard $version"
    Expect ([bool] (Read-Log $log | Where-Object { $_ -match "^update: first frame at \d+ ms; told pid $($old.Id): true$" })) 'the new process told the old one of its first frame'
    $names = Get-Names $folder
    Say "the copy's folder: $($names -join ', ')"
    Expect (($names -join ',') -eq "dashboard.$Pretend.old,dashboard.exe") "the folder holds dashboard.exe and dashboard.$Pretend.old, and no dashboard.exe.new"
    Expect ((Get-FileHash $exe -Algorithm SHA256).Hash -eq $stagedHash) "dashboard.exe is the release's build"
    $new | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }

    # --- A manifest whose SHA-256 is not the zip's: refused, nothing changed.
    $exe = New-Copy 'update-bad'
    $folder = Split-Path $exe
    $before = (Get-FileHash $exe -Algorithm SHA256).Hash
    $source = Start-Releases $version @('--bad-hash')
    $log = Join-Path $Out 'update-bad.log'
    $bad = Start-Copy $exe $source $log 'update-bad-data'
    Open-About $log | Out-Null
    $mark = (Read-Log $log).Count
    Push-Button 'A'
    Expect-Line $log "^update: refused $([regex]::Escape($version)): its zip's SHA-256 is [0-9a-f]{64}, not the manifest's [0-9a-f]{64}; nothing was changed$" 60 'the download is refused' $mark | Out-Null
    Expect-Line $log "^update: bad-download $([regex]::Escape($version))$" 10 'About says so' $mark | Out-Null
    Expect-Line $log "^focus: Settings: About: .*; Check for updates; bad-download $([regex]::Escape($version))$" 10 'the focus line says bad-download' $mark | Out-Null
    Expect (-not $bad.HasExited) 'the copy runs on'
    $names = Get-Names $folder
    Expect (($names -join ',') -eq 'dashboard.exe') "the folder holds dashboard.exe alone (holds $($names -join ', '))"
    Expect ((Get-FileHash $exe -Algorithm SHA256).Hash -eq $before) 'dashboard.exe is unchanged'

    if ($failures.Count -eq 0) { $result = 'PASS' }
} catch {
    $failures += "error: $_"
} finally {
    foreach ($id in $started) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    # Any Dashboard still running from the test's copies.
    Get-OwnProcess dashboard | Stop-Process -Force -ErrorAction SilentlyContinue
    Disconnect-Pad
    $failures | ForEach-Object { Say "failed: $_" }
    Say "result: $result ($([int] $clock.Elapsed.TotalSeconds) s)"
}
exit $(if ($result -eq 'PASS') { 0 } else { 1 })
