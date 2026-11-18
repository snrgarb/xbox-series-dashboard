# The Store's install queue, asked from Windows PowerShell 5.1 (#173; docs/research/installs.md 8). The Dashboard
# runs it with `-EncodedCommand`, the lines setting $Command, $Product, $Family, $Volume and $Parent put first
# (`source_xbox::store_queue::Windows`): every call to AppInstallManager and PackageManager is here and nowhere else,
# and each answers one line the Dashboard reads.
#
#   start   $Product [$Volume]  StartProductInstallAsync, for this user  -> `started <state>` | `refused 0x... <why>`
#   pause | resume | cancel $Product                                     -> `ok` | `refused 0x... <why>`
#   watch   [$Parent]   the queue's items as one JSON line a second, until the Dashboard ($Parent) is gone, or the
#                       queue has been empty for 20 s
#   remove  $Family     RemovePackageAsync, for this user, every package of the family  -> `removed <n>` | `refused`
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$methods = [System.WindowsRuntimeSystemExtensions].GetMethods()
$asTask = $methods | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' } | Select-Object -First 1
$asTaskProgress = $methods | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperationWithProgress`2' } | Select-Object -First 1

function Load([string] $Type, [string[]] $Contracts) {
    foreach ($contract in $Contracts) {
        try { $t = [Type]::GetType("$Type, $contract, ContentType=WindowsRuntime"); if ($t) { return $t } } catch {}
    }
    throw "the type $Type is not there"
}
function Await($Operation, [Type] $Result, [int] $Seconds = 60) {
    $task = $asTask.MakeGenericMethod($Result).Invoke($null, @($Operation))
    if (-not $task.Wait($Seconds * 1000)) { throw "no answer in $Seconds s" }
    $task.Result
}
function Refused($Exception) {
    $e = $Exception
    while ($e.InnerException) { $e = $e.InnerException }
    'refused 0x{0:X8} {1}' -f $e.HResult, (($e.Message -replace '\s+', ' ').Trim())
}

$install = @('Windows.ApplicationModel.Store.Preview.InstallControl', 'Windows.ApplicationModel.Store.Preview', 'Windows.ApplicationModel')
$deployment = @('Windows.Management.Deployment', 'Windows.Management')

function Items($Manager) {
    $all = @()
    foreach ($item in @($Manager.AppInstallItems)) {
        $s = $item.GetCurrentStatus()
        $all += [ordered]@{
            product = [string]$item.ProductId
            family  = [string]$item.PackageFamilyName
            state   = [string]$s.InstallState
            percent = [double]$s.PercentComplete
            done    = [uint64]$s.BytesDownloaded
            total   = [uint64]$s.DownloadSizeInBytes
            error   = $(if ($s.ErrorCode) { [int]$s.ErrorCode.HResult } else { 0 })
        }
    }
    ConvertTo-Json -InputObject @($all) -Compress -Depth 3
}

try {
    switch ($Command) {
        'start' {
            $manager = [Activator]::CreateInstance((Load 'Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager' $install))
            $options = [Activator]::CreateInstance((Load 'Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallOptions' $install))
            $options.InstallForAllUsers = $false
            $options.AllowForcedAppRestart = $false
            $options.LaunchAfterInstall = $false
            if ($Volume) {
                $pm = [Activator]::CreateInstance((Load 'Windows.Management.Deployment.PackageManager' $deployment))
                $target = @($pm.FindPackageVolumes()) | Where-Object { $_.MountPoint -and $Volume.StartsWith($_.MountPoint.TrimEnd('\'), 'OrdinalIgnoreCase') } | Select-Object -First 1
                if ($target) { $options.TargetVolume = $target }
            }
            $itemType = Load 'Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallItem' $install
            $listType = [System.Collections.Generic.IReadOnlyList``1].MakeGenericType($itemType)
            $items = Await ($manager.StartProductInstallAsync($Product, '', 'xbox-series-dashboard', '', $options)) $listType
            $first = @($items) | Select-Object -First 1
            if ($first) { 'started {0}' -f $first.GetCurrentStatus().InstallState } else { 'refused 0x00000000 the Store started nothing' }
        }
        { $_ -in 'pause', 'resume', 'cancel' } {
            $manager = [Activator]::CreateInstance((Load 'Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager' $install))
            switch ($Command) { 'pause' { $manager.Pause($Product) } 'resume' { $manager.Restart($Product) } 'cancel' { $manager.Cancel($Product) } }
            'ok'
        }
        'watch' {
            $manager = [Activator]::CreateInstance((Load 'Windows.ApplicationModel.Store.Preview.InstallControl.AppInstallManager' $install))
            $empty = 0
            while ($true) {
                if ($Parent -and -not (Get-Process -Id $Parent -ErrorAction SilentlyContinue)) { break }
                $line = Items $manager
                [Console]::Out.WriteLine($line)
                [Console]::Out.Flush()
                if ($line -eq '[]') { $empty++ } else { $empty = 0 }
                if ($empty -ge 20) { break }
                Start-Sleep -Seconds 1
            }
        }
        'remove' {
            $pm = [Activator]::CreateInstance((Load 'Windows.Management.Deployment.PackageManager' $deployment))
            $resultType = Load 'Windows.Management.Deployment.DeploymentResult' $deployment
            $progressType = Load 'Windows.Management.Deployment.DeploymentProgress' $deployment
            $removed = 0
            foreach ($package in @($pm.FindPackagesForUser('', $Family))) {
                $task = $asTaskProgress.MakeGenericMethod($resultType, $progressType).Invoke($null, @($pm.RemovePackageAsync($package.Id.FullName)))
                if (-not $task.Wait(600000)) { throw 'no answer in 600 s' }
                if ($task.Result.ExtendedErrorCode) { throw $task.Result.ExtendedErrorCode }
                $removed++
            }
            "removed $removed"
        }
        default { "refused 0x80070057 no command $Command" }
    }
} catch {
    Refused $_.Exception
}
