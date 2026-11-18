# A read-only look at the real Battle.net on this PC, for #172 (docs/research/installs.md 7 and 12, questions B1 to
# B4; docs/sources.md "Battle.net through its Agent"). PowerShell 5.1. It installs, uninstalls, starts and changes
# nothing: it reads files and the registry, lists processes and ports, and asks the Agent only what reads (GET).
# It never prints a token, a session, a user id or an account name: those are written <masked>.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File battlenet.ps1 [-Uid libra]
#
# Run it once with the Battle.net client open and signed in, and once more during the real install of Tony Hawk's
# Pro Skater 3 + 4 (-Uid libra), so the Agent's progress shows its fields while they move (B4).
param([string] $Uid = 'libra')
$ErrorActionPreference = 'Continue'

function Say([string] $Text) { Write-Output $Text }

# A value with its secrets taken out: every field whose name says it is one, at any depth.
$secret = '^(authorization|session|user_id|user_name|account.*|battle_tag|email|token)$'
function Mask($Value) {
    if ($null -eq $Value) { return $null }
    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        $copy = [ordered]@{}
        foreach ($p in $Value.PSObject.Properties) {
            if ($p.Name -match $secret) { $copy[$p.Name] = '<masked>' } else { $copy[$p.Name] = Mask $p.Value }
        }
        return [pscustomobject]$copy
    }
    if ($Value -is [System.Array]) { return @($Value | ForEach-Object { Mask $_ }) }
    $Value
}

# GET on the Agent: the status and the answer, masked, as one line of JSON. $Token is sent as the Authorization header,
# $ProcessId as a `pid` header (redact7's bnetinstaller sends the running Agent's own).
function Ask-Agent([int] $Port, [string] $Path, [string] $Token = '', [int] $ProcessId = 0) {
    $headers = @{}
    if ($Token) { $headers['Authorization'] = $Token }
    if ($ProcessId) { $headers['pid'] = [string]$ProcessId }
    try {
        $r = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:${Port}$Path" -UserAgent 'phoenix-agent/1.0' -Headers $headers -TimeoutSec 5
        $status = [int]$r.StatusCode
        $text = $r.Content
    } catch [System.Net.WebException] {
        if ($_.Exception.Response) {
            $status = [int]$_.Exception.Response.StatusCode
            $text = (New-Object IO.StreamReader($_.Exception.Response.GetResponseStream())).ReadToEnd()
        } else {
            return @{ Status = 0; Json = $null; Line = "no answer: $($_.Exception.Message)" }
        }
    } catch {
        return @{ Status = 0; Json = $null; Line = "no answer: $($_.Exception.Message)" }
    }
    $json = $null
    try { $json = $text | ConvertFrom-Json } catch {}
    $shown = if ($json) { (Mask $json) | ConvertTo-Json -Depth 8 -Compress } else { "(not JSON, $($text.Length) chars)" }
    @{ Status = $status; Json = $json; Line = "$status $shown" }
}

function Size([string] $Path) {
    if (Test-Path $Path -PathType Leaf) { return (Get-Item $Path).Length }
    if (Test-Path $Path) { return (Get-ChildItem $Path -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum }
    $null
}

Say "== Battle.net on $env:COMPUTERNAME, $(Get-Date -Format s)"

# --- B1. Where Agent.exe is: the stub in %ProgramData%\Battle.net\Agent and the real one in Agent.<build>, the newest
# build being the one started (what source_battlenet::agent::Place::program takes).
$agentDir = Join-Path $env:ProgramData 'Battle.net\Agent'
Say "-- B1: the Agent's folder $agentDir exists: $(Test-Path $agentDir)"
$builds = @(Get-ChildItem $agentDir -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^Agent\.(\d+)$' } |
    Sort-Object { [int]($_.Name -replace '^Agent\.', '') })
foreach ($b in $builds) { Say "   build folder $($b.Name): Agent.exe $(Test-Path (Join-Path $b.FullName 'Agent.exe'))" }
$stub = Join-Path $agentDir 'Agent.exe'
if (Test-Path $stub) { Say "   stub Agent.exe: version $((Get-Item $stub).VersionInfo.FileVersion), private part $((Get-Item $stub).VersionInfo.FilePrivatePart)" }

# --- B1. The port: what Agent.dat says (DroidDeck's way), what an Agent.exe listens on (barncastle's), the wiki's 1120.
$dat = Join-Path $agentDir 'Agent.dat'
$datPort = $null
if (Test-Path $dat) {
    $raw = (Get-Content $dat -Raw -ErrorAction SilentlyContinue)
    if ($raw -match '^\s*(\d+)\s*$') { $datPort = [int]$Matches[1] }
    Say "-- B1: Agent.dat: $((Get-Item $dat).Length) bytes, reads as port $(if ($datPort) { $datPort } else { '(not one number)' })"
} else { Say '-- B1: no Agent.dat' }

# --- B1. Whether the Agent and the client run, and on which ports the Agent listens.
$agents = @(Get-Process Agent -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($agentDir, [StringComparison]::OrdinalIgnoreCase) })
$clients = @(Get-Process 'Battle.net' -ErrorAction SilentlyContinue)
Say "-- B1: Battle.net.exe running: $($clients.Count -gt 0) ($(@($clients | ForEach-Object { $_.Id }) -join ', '))"
Say "-- B1: Agent.exe running: $($agents.Count -gt 0)"
$ports = @()
foreach ($a in $agents) {
    $listening = @(Get-NetTCPConnection -State Listen -OwningProcess $a.Id -ErrorAction SilentlyContinue)
    Say "   pid $($a.Id) $($a.Path), started $($a.StartTime): listening on $(@($listening | ForEach-Object { "$($_.LocalAddress):$($_.LocalPort)" }) -join ', ')"
    $ports += @($listening | ForEach-Object { [int]$_.LocalPort })
    # The command line the client started it with: which flags it gets (--internalclienttools? --port? --session?).
    $line = (Get-CimInstance Win32_Process -Filter "ProcessId=$($a.Id)" -ErrorAction SilentlyContinue).CommandLine
    if ($line) { Say "   its command line: $($line -replace '(--session=)\S+', '$1<masked>')" }
}
$candidates = @(@($datPort) + $ports + @(1120) | Where-Object { $_ } | Select-Object -Unique)
Say "   ports to try, in the Dashboard's order: $($candidates -join ', ')"
if ($agents.Count -eq 0) { Say '   (the Agent is not running: this probe does not start it; the Dashboard would, with --internalclienttools)' }

# --- B1. A token from GET /agent, with phoenix-agent/1.0 (and, for comparison, without it).
$token = $null
$port = $null
foreach ($p in $candidates) {
    $answer = Ask-Agent $p '/agent'
    Say "-- B1: GET /agent on ${p}: $($answer.Line)"
    if ($answer.Json -and $answer.Json.authorization) {
        $token = [string]$answer.Json.authorization
        $port = $p
        Say "   the answer's fields: $(@($answer.Json.PSObject.Properties | ForEach-Object { "$($_.Name):$($_.Value.GetType().Name)" }) -join ', ')"
        Say "   token: $($token.Length) characters, digits only: $($token -match '^\d+$')"
        break
    }
}
# --- B1. The client's own Agent with its process id in a `pid` header, as redact7's tool asks it (#172: the Dashboard
# asks so once before giving up on an Agent it did not start). Read-only: GET /agent alone.
if (-not $token) {
    foreach ($a in $agents) {
        foreach ($l in @(Get-NetTCPConnection -State Listen -OwningProcess $a.Id -ErrorAction SilentlyContinue)) {
            $answer = Ask-Agent ([int]$l.LocalPort) '/agent' '' $a.Id
            Say "-- B1: GET /agent on $($l.LocalPort) with pid $($a.Id): $($answer.Line)"
            if (-not $token -and $answer.Json -and $answer.Json.authorization) {
                $token = [string]$answer.Json.authorization
                $port = [int]$l.LocalPort
                Say '   a token with the pid header: the Dashboard would use the client''s own Agent and start none'
            }
        }
    }
}
if (-not $token) { Say '-- B1: no port gave a token. With Battle.net or its Agent running the Dashboard starts no Agent and opens Battle.net on the game; with neither, it starts its own.' }
if ($port) {
    try {
        $plain = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:${port}/agent" -TimeoutSec 5
        Say "   GET /agent without the user agent: $([int]$plain.StatusCode)"
    } catch { Say "   GET /agent without the user agent: $($_.Exception.Message)" }
}

# --- B2. What the Agent lists: GET /game (each product's uid), then GET /game/<uid> (its product code, state, folder).
if ($token) {
    $games = Ask-Agent $port '/game' $token
    Say "-- B2: GET /game: $($games.Status)"
    $uids = @()
    if ($games.Json) { $uids = @($games.Json.PSObject.Properties | ForEach-Object { $_.Name }) }
    foreach ($u in $uids) {
        $g = Ask-Agent $port "/game/$u" $token
        $j = $g.Json
        if ($j) {
            Say ("   {0}: product {1}, installed {2}, playable {3}, install_dir {4}, selected_locale {5}" -f $u, $j.product, $j.installed, $j.playable, $j.install_dir, $j.selected_locale)
        } else { Say "   ${u}: $($g.Line)" }
    }
    # The game asked about: whether the Agent knows it before it is installed, and what /install says of it (404 when
    # nothing is under way; during the real install, its progress fields: B4).
    foreach ($path in "/game/$Uid", "/install/$Uid", "/update/$Uid") {
        Say "-- B2/B4: GET ${path}: $((Ask-Agent $port $path $token).Line)"
    }
    # What the Agent says of its downloads and of itself, read only.
    foreach ($path in '/version', '/priorities', '/agent/download', '/hardware') {
        Say "-- B3: GET ${path}: $((Ask-Agent $port $path $token).Line)"
    }
    Say '-- B3: pause, resume and cancel are not tried here (they change things): the Dashboard tries its guesses only with DASHBOARD_BATTLENET_STEER=1.'
}

# --- B2. Owned games (and what the client lists once the Agent installs): the local files that might name them,
# with their sizes. If one of them lists the account's games, the built-in catalogue (source_battlenet::catalogue)
# could give way to it.
Say '-- B2: local files (sizes in bytes):'
$files = @(
    (Join-Path $env:APPDATA 'Battle.net\Battle.net.config'),
    (Join-Path $env:LOCALAPPDATA 'Battle.net\CachedData.db'),
    (Join-Path $env:LOCALAPPDATA 'Battle.net\Cache'),
    (Join-Path $env:LOCALAPPDATA 'Battle.net\BrowserCaches'),
    (Join-Path $env:LOCALAPPDATA 'Battle.net\Account'),
    (Join-Path $env:ProgramData 'Battle.net\Agent\product.db'),
    (Join-Path $env:ProgramData 'Battle.net\Agent\aggregate.json'),
    (Join-Path $env:ProgramData 'Battle.net\Agent\data\cache'),
    (Join-Path $env:ProgramData 'Battle.net\Setup')
)
foreach ($f in $files) { Say "   $f : $(if (Test-Path $f) { Size $f } else { 'not there' })" }
foreach ($f in @(Get-ChildItem (Join-Path $env:APPDATA 'Battle.net') -Filter '*.config' -ErrorAction SilentlyContinue)) {
    Say "   $($f.FullName) : $($f.Length)"
}
# Battle.net.config: its Games section's keys (uids: installed or played ones only, it is said) and the folder the
# client offers for installs (source_battlenet reads Client.Install.DefaultInstallPath).
$config = Join-Path $env:APPDATA 'Battle.net\Battle.net.config'
if (Test-Path $config) {
    try {
        $c = Get-Content $config -Raw | ConvertFrom-Json
        if ($c.Games) { Say "   Battle.net.config Games: $(@($c.Games.PSObject.Properties | ForEach-Object { $_.Name }) -join ', ')" }
        Say "   Battle.net.config Client.Install.DefaultInstallPath: $($c.Client.Install.DefaultInstallPath)"
        Say "   Battle.net.config top-level keys: $(@($c.PSObject.Properties | ForEach-Object { $_.Name }) -join ', ')"
    } catch { Say "   Battle.net.config: not read as JSON: $($_.Exception.Message)" }
}
# aggregate.json: the products the client lists as installed or installing.
$aggregate = Join-Path $env:ProgramData 'Battle.net\Agent\aggregate.json'
if (Test-Path $aggregate) {
    try {
        $a = Get-Content $aggregate -Raw | ConvertFrom-Json
        Say "   aggregate.json installed: $(@($a.installed | ForEach-Object { "$($_.product_id) ($($_.name))" }) -join ', ')"
    } catch { Say "   aggregate.json: not read: $($_.Exception.Message)" }
}
# The cached catalogue fragments: how many files name the game asked about (its uid or launch code), and which.
$cache = Join-Path $env:LOCALAPPDATA 'Battle.net\Cache'
if (Test-Path $cache) {
    $naming = @(Get-ChildItem $cache -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Length -lt 4MB } |
        Where-Object { Select-String -Path $_.FullName -Pattern "`"$Uid`"|LBRA" -Quiet -ErrorAction SilentlyContinue })
    Say "   Cache files naming $Uid or LBRA: $($naming.Count)$(if ($naming.Count) { ': ' + (@($naming | Select-Object -First 5 | ForEach-Object { "$($_.FullName) ($($_.Length))" }) -join ', ') })"
}
# The Agent's own logs, by name and size: where its calls and errors are said.
foreach ($b in $builds) {
    foreach ($l in @(Get-ChildItem (Join-Path $b.FullName 'Logs') -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 5)) {
        Say "   log $($l.FullName) : $($l.Length), $($l.LastWriteTime)"
    }
}

# --- B-O1 (#276, docs/research/owned-games.md 13). The account's licences and the client's catalogue in its cache,
# as source_battlenet::cache reads them: COUNTS ONLY, never a licence, an id or a name. The licences are the JSON of
# CachedData.db's features_cached_data_points, read from the file's bytes (no SQLite here); the catalogue's root is
# the cached JSON with a "fragments" list, each fragment a file of Cache named by its MD5.
$cachedData = Join-Path $env:LOCALAPPDATA 'Battle.net\CachedData.db'
if (Test-Path $cachedData) {
    $bytes = [IO.File]::ReadAllBytes($cachedData)
    $text = [Text.Encoding]::UTF8.GetString($bytes)
    $licences = @()
    if ($text -match '"licenses":\[([\d,\s]*)\]') { $licences = @($Matches[1] -split ',' | Where-Object { $_.Trim() } | ForEach-Object { [int64]$_.Trim() }) }
    Say "-- B-O1: CachedData.db: $($bytes.Length) bytes; licences in features_cached_data_points: $($licences.Count); catalog_cache named: $($text -match 'catalog_cache')"
    if (Test-Path $cache) {
        $roots = @(Get-ChildItem $cache -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Length -lt 4MB } |
            Where-Object { Select-String -Path $_.FullName -Pattern '"fragments":\[' -Quiet -ErrorAction SilentlyContinue } |
            Sort-Object LastWriteTime)
        Say "   catalogue roots cached: $($roots.Count)"
        $root = $roots | Select-Object -Last 1
        if ($root) {
            $fragments = @((Get-Content $root.FullName -Raw | ConvertFrom-Json).fragments)
            $products = 0; $retail = 0; $named = @{}; $cachedCount = 0
            foreach ($f in $fragments) {
                $h = [string]$f.hash
                $path = Join-Path $cache (Join-Path $h.Substring(0, 2) (Join-Path $h.Substring(2, 2) $h))
                if (-not (Test-Path $path)) { continue }
                $cachedCount++
                $raw = Get-Content $path -Raw
                $json = $null
                try { $json = $raw | ConvertFrom-Json } catch { continue }
                foreach ($p in @($json.products)) {
                    if ($p.base.program_id) { $products++; if ($p.base.types.retail) { $retail++ } }
                }
                foreach ($m in [regex]::Matches($raw, '"license_id":\s*\[?([\d,\s]+)')) {
                    foreach ($n in ($m.Groups[1].Value -split ',')) { if ($n.Trim()) { $named[[int64]$n.Trim()] = $true } }
                }
            }
            $known = @($licences | Where-Object { $named.ContainsKey($_) }).Count
            Say "   newest root: $($fragments.Count) fragments, $cachedCount cached; products with a program: $products, with a retail type: $retail"
            Say "   licences named by some rule of the catalogue: $known of $($licences.Count)"
        }
    }
    Say '   (the Dashboard says what it made of them: battlenet: owned: <n> of <m> catalogue games (from CachedData.db, <when>))'
} else { Say '-- B-O1: no CachedData.db' }

# --- B2. Installed games as the registry has them: the Uninstall keys with --uid= (the Dashboard's fallback
# uninstall; after an Agent install, whether the key is written as the client's own install writes it).
Say '-- B2: Uninstall keys naming a Battle.net uid:'
foreach ($root in 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall') {
    foreach ($k in @(Get-ChildItem $root -ErrorAction SilentlyContinue)) {
        $v = Get-ItemProperty $k.PSPath -ErrorAction SilentlyContinue
        if ($v.UninstallString -match '--uid=(\S+)') { Say "   $($Matches[1]): $($v.DisplayName), $($v.InstallLocation)" }
    }
}
Say '== done: nothing was changed'
