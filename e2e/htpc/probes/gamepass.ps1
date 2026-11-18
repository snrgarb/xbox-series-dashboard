# A read-only look at PC Game Pass and the Store's install queue on this PC, for #173 and the subscription check
# (docs/research/game-pass.md 7 and 8; docs/research/installs.md 8, X1 to X4). PowerShell 5.1. It installs,
# uninstalls, buys, claims and changes nothing: it lists packages, reads the registry and folder names (never a
# file's contents), and asks Windows' Store APIs only what reads. Account names, user ids, security ids and tokens are
# never printed: they are written <masked>.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File gamepass.ps1 [-ProductId 9PD5BM2Z8C4L]
#
# Every line is `key: value`, the keys as docs/research/game-pass.md 8 lists them. Run it once as the TV's user,
# unelevated, with the Xbox app signed in; once more while the Xbox app installs a game, so the queue's lines show
# an install under way (X2).
param([string] $ProductId = '9PD5BM2Z8C4L')
$ErrorActionPreference = 'Continue'

function Say([string] $Key, $Value) { Write-Output ("{0}: {1}" -f $Key, $Value) }

# A message with anything that looks like an account, an id or a token taken out.
function Clean([string] $Text) {
    if (-not $Text) { return '' }
    $t = $Text -replace '[\w.+-]+@[\w-]+\.[\w.]+', '<masked>'
    $t = $t -replace 'S-1-5-[\d-]+', '<masked>'
    $t = $t -replace '\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b', '<masked>'
    $t = $t -replace '[A-Za-z0-9_\-]{40,}', '<masked>'
    ($t -replace '\s+', ' ').Trim()
}

function Hresult($Exception) {
    $e = $Exception
    while ($e.InnerException) { $e = $e.InnerException }
    '0x{0:X8} {1}' -f $e.HResult, (Clean $e.Message)
}

# --- WinRT from PowerShell 5.1: the types loaded by name, and an IAsyncOperation waited on.
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
} | Select-Object -First 1
$asTaskAction = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncAction'
} | Select-Object -First 1
function Await($Operation, [Type] $Result) {
    $task = $asTask.MakeGenericMethod($Result).Invoke($null, @($Operation))
    if (-not $task.Wait(30000)) { throw 'no answer in 30 s' }
    $task.Result
}
function Load([string] $Type, [string[]] $Contracts) {
    foreach ($contract in $Contracts) {
        try { $t = [Type]::GetType("$Type, $contract, ContentType=WindowsRuntime"); if ($t) { return $t } } catch {}
    }
    $null
}

# --- Windows, the Store, the Xbox app and Gaming Services.
$os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
Say 'windows.build' $(if ($os) { "$($os.Version) $($os.Caption)" } else { 'unknown' })
Say 'powershell.version' $PSVersionTable.PSVersion
$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Say 'process.elevated' $elevated
foreach ($p in @(
        @{ Key = 'store'; Name = 'Microsoft.WindowsStore' },
        @{ Key = 'xboxapp'; Name = 'Microsoft.GamingApp' },
        @{ Key = 'gamingservices'; Name = 'Microsoft.GamingServices' },
        @{ Key = 'xboxidentity'; Name = 'Microsoft.XboxIdentityProvider' })) {
    $pkg = Get-AppxPackage -Name $p.Name -ErrorAction SilentlyContinue | Select-Object -First 1
    Say "$($p.Key).installed" $(if ($pkg) { "yes $($pkg.Version) $($pkg.PackageFamilyName)" } else { 'no' })
}
$services = Get-Service -Name GamingServices, GamingServicesNet, InstallService, ClipSVC -ErrorAction SilentlyContinue
foreach ($s in $services) { Say "service.$($s.Name)" "$($s.Status) $($s.StartType)" }

# The Xbox app's own folders: names and sizes only, never contents.
$app = Join-Path $env:LOCALAPPDATA 'Packages\Microsoft.GamingApp_8wekyb3d8bbwe'
foreach ($sub in 'LocalState', 'LocalCache', 'Settings', 'AC') {
    $dir = Join-Path $app $sub
    if (Test-Path $dir) {
        $files = @(Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue)
        Say "xboxapp.$($sub.ToLower()).files" $files.Count
        $named = $files | Where-Object { $_.Name -match 'pass|subscr|entitle|licen|ownership|catalog' } | Select-Object -First 15
        foreach ($f in $named) { Say "xboxapp.$($sub.ToLower()).named" ("{0} {1} bytes" -f $f.FullName.Substring($app.Length), $f.Length) }
    } else { Say "xboxapp.$($sub.ToLower()).files" 'none' }
}

# Gaming Services' registry: key names and value names only.
foreach ($key in 'HKLM:\SOFTWARE\Microsoft\GamingServices', 'HKCU:\SOFTWARE\Microsoft\GamingServices', 'HKCU:\SOFTWARE\Microsoft\XboxLive') {
    if (Test-Path $key) {
        $subs = @(Get-ChildItem $key -ErrorAction SilentlyContinue | ForEach-Object { $_.PSChildName })
        Say "registry.$($key -replace '[:\\]+', '.')" ("{0} keys: {1}" -f $subs.Count, (($subs | Select-Object -First 12) -join ', '))
        $values = (Get-Item $key -ErrorAction SilentlyContinue).Property
        if ($values) { Say "registry.$($key -replace '[:\\]+', '.').values" (($values | Select-Object -First 12) -join ', ') }
    } else { Say "registry.$($key -replace '[:\\]+', '.')" 'none' }
}
$licences = 'C:\ProgramData\Microsoft\Windows\ClipSVC\tokens.dat'
Say 'clipsvc.tokens' $(if (Test-Path $licences) { "there, $((Get-Item $licences).Length) bytes, not read (encrypted)" } else { 'not readable or not there' })

# Installed Xbox games (a MicrosoftGame.config in the package): how many, not which.
$games = @(Get-AppxPackage -ErrorAction SilentlyContinue | Where-Object {
        -not $_.IsFramework -and $_.InstallLocation -and (Test-Path (Join-Path $_.InstallLocation 'MicrosoftGame.config'))
    })
Say 'xbox.games.installed' $games.Count
$signed = @($games | Where-Object { $_.SignatureKind -eq 'Store' }).Count
Say 'xbox.games.storesigned' $signed

# --- StoreContext (route A1): the subscriptions' products, asked as an unpackaged process.
$storeContext = Load 'Windows.Services.Store.StoreContext' @('Windows.Services.Store', 'Windows.Services')
if (-not $storeContext) { Say 'storecontext.type' 'not loadable' } else {
    try {
        $context = $storeContext::GetDefault()
        Say 'storecontext.create' 'ok'
        try {
            $user = $context.User
            Say 'storecontext.user' $(if ($user) { "a user (<masked>), type $($user.Type)" } else { 'none' })
        } catch { Say 'storecontext.user' (Hresult $_.Exception) }
        $resultType = Load 'Windows.Services.Store.StoreProductQueryResult' @('Windows.Services.Store', 'Windows.Services')
        $kinds = [System.Collections.Generic.List[string]]::new(); 'Durable', 'Consumable', 'UnmanagedConsumable', 'Application', 'Game' | ForEach-Object { $kinds.Add($_) }
        $ids = [System.Collections.Generic.List[string]]::new(); 'CFQ7TTC0KGQ8', 'CFQ7TTC0KHS0', 'CFQ7TTC0K6L8', $ProductId | ForEach-Object { $ids.Add($_) }
        try {
            $answer = Await ($context.GetStoreProductsAsync($kinds, $ids)) $resultType
            Say 'storecontext.products.error' $(if ($answer.ExtendedError) { '0x{0:X8}' -f $answer.ExtendedError.HResult } else { 'none' })
            Say 'storecontext.products.count' $answer.Products.Count
            foreach ($kv in $answer.Products) {
                $p = $kv.Value
                Say "storecontext.product.$($kv.Key)" ("kind {0}; in the user's collection {1}; title {2}" -f $p.ProductKind, $p.IsInUserCollection, $p.Title)
            }
        } catch { Say 'storecontext.products' (Hresult $_.Exception) }
        try {
            $collection = Await ($context.GetUserCollectionAsync($kinds)) $resultType
            Say 'storecontext.collection.count' $(if ($collection.ExtendedError) { "error 0x{0:X8}" -f $collection.ExtendedError.HResult } else { $collection.Products.Count })
        } catch { Say 'storecontext.collection' (Hresult $_.Exception) }
        try {
            $licenceType = Load 'Windows.Services.Store.StoreAppLicense' @('Windows.Services.Store', 'Windows.Services')
            $licence = Await ($context.GetAppLicenseAsync()) $licenceType
            Say 'storecontext.applicense' ("active {0}; add-on licences {1}" -f $licence.IsActive, $licence.AddOnLicenses.Count)
        } catch { Say 'storecontext.applicense' (Hresult $_.Exception) }
    } catch { Say 'storecontext.create' (Hresult $_.Exception) }
}

# --- AppInstallManager (#173): created, asked, and its queue read. Nothing is started, paused or cancelled.
$contracts = @('Windows.ApplicationModel.Store.Preview.InstallControl', 'Windows.ApplicationModel.Store.Preview', 'Windows.ApplicationModel')
$managerType = Load 'Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager' $contracts
if (-not $managerType) { Say 'appinstallmanager.type' 'not loadable' } else {
    try {
        $manager = [Activator]::CreateInstance($managerType)
        Say 'appinstallmanager.create' 'ok'
        try { Say 'appinstallmanager.autoupdate' $manager.AutoUpdateSetting } catch { Say 'appinstallmanager.autoupdate' (Hresult $_.Exception) }
        try { Say 'appinstallmanager.acquisitionidentity' $(if ($manager.AcquisitionIdentity) { '<masked>' } else { 'none' }) } catch { Say 'appinstallmanager.acquisitionidentity' (Hresult $_.Exception) }
        try {
            $allowed = Await ($manager.GetIsAppAllowedToInstallAsync($ProductId)) ([bool])
            Say "appinstallmanager.allowed.$ProductId" $allowed
        } catch { Say "appinstallmanager.allowed.$ProductId" (Hresult $_.Exception) }
        try {
            $items = @($manager.AppInstallItems)
            Say 'appinstallmanager.items' $items.Count
            foreach ($item in $items) {
                $s = $item.GetCurrentStatus()
                Say "appinstallmanager.item.$($item.ProductId)" ("family {0}; type {1}; user-initiated {2}; state {3}; {4:N1}%; {5} of {6} bytes; error 0x{7:X8}" -f
                    $item.PackageFamilyName, $item.InstallType, $item.IsUserInitiated, $s.InstallState, $s.PercentComplete,
                    $s.BytesDownloaded, $s.DownloadSizeInBytes, $(if ($s.ErrorCode) { $s.ErrorCode.HResult } else { 0 }))
            }
        } catch { Say 'appinstallmanager.items' (Hresult $_.Exception) }
    } catch { Say 'appinstallmanager.create' (Hresult $_.Exception) }
}

# --- PackageManager (#173 uninstall): created, and the volumes an install may target.
try {
    $pmType = Load 'Windows.Management.Deployment.PackageManager' @('Windows.Management.Deployment', 'Windows.Management')
    $pm = [Activator]::CreateInstance($pmType)
    Say 'packagemanager.create' 'ok'
    try {
        foreach ($v in @($pm.FindPackageVolumes())) {
            Say 'packagemanager.volume' ("{0}; offline {1}; system {2}; path {3}" -f $v.MountPoint, $v.IsOffline, $v.IsSystemVolume, $v.PackageStorePath)
        }
    } catch { Say 'packagemanager.volumes' (Hresult $_.Exception) }
} catch { Say 'packagemanager.create' (Hresult $_.Exception) }

# The drives' .GamingRoot folders, where the Xbox app installs.
foreach ($drive in Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue) {
    $root = Join-Path $drive.Root '.GamingRoot'
    if (Test-Path $root) { Say "gamingroot.$($drive.Name)" ("there; {0:N1} GB free" -f ($drive.Free / 1GB)) }
}
Say 'done' 'nothing was changed'
