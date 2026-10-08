param(
    [Parameter(Mandatory = $true)][int]$RootProcessId,
    [Parameter(Mandatory = $true)][string]$ExpectedExecutable,
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9-]+$')][string]$Label,
    [ValidateRange(5, 120)][int]$Samples = 15
)
$ErrorActionPreference = 'Stop'
$measureRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$measureExpected = (Resolve-Path -LiteralPath $ExpectedExecutable).Path
$measureProcess = Get-Process -Id $RootProcessId
if ($measureProcess.Path -ne $measureExpected) { throw '进程路径与预期程序不符，停止采样。' }
$measureStarted = $measureProcess.StartTime
$measurePrevious = @{}
$measureRecords = [Collections.Generic.List[object]]::new()
$measureTimer = [Diagnostics.Stopwatch]::StartNew()
$measureLast = 0.0
for ($measureIndex = 0; $measureIndex -lt $Samples; $measureIndex++) {
    $measureProcess = Get-Process -Id $RootProcessId
    if ($measureProcess.StartTime -ne $measureStarted) { throw '根进程已被替换，结果无效。' }
    $measureTree = @(Get-CimInstance Win32_Process)
    $measureIds = [Collections.Generic.HashSet[int]]::new()
    [void]$measureIds.Add($RootProcessId)
    do {
        $measureCount = $measureIds.Count
        foreach ($measureNode in $measureTree) {
            if ($measureIds.Contains([int]$measureNode.ParentProcessId)) { [void]$measureIds.Add([int]$measureNode.ProcessId) }
        }
    } while ($measureCount -ne $measureIds.Count)
    $measureChildren = @(Get-Process -Id @($measureIds) -ErrorAction SilentlyContinue)
    $measurePrivate = 0L
    $measureCpu = 0.0
    foreach ($measureChild in $measureChildren) {
        if ($null -eq $measureChild.CPU) { throw '没有读取进程 CPU 的权限，不能记录为零。' }
        $measurePrivate += $measureChild.PrivateMemorySize64
        $measureKey = "$($measureChild.Id):$($measureChild.StartTime.Ticks)"
        if ($measurePrevious.ContainsKey($measureKey)) { $measureCpu += [Math]::Max(0, $measureChild.CPU - $measurePrevious[$measureKey]) }
        elseif ($measureIndex -gt 0) { $measureCpu += $measureChild.CPU }
        $measurePrevious[$measureKey] = $measureChild.CPU
    }
    $measureNow = $measureTimer.Elapsed.TotalSeconds
    $measureCpuPercent = if ($measureIndex -eq 0) { $null } else { 100 * $measureCpu / ($measureNow - $measureLast) }
    $measureRecords.Add([pscustomobject]@{Seconds=$measureNow;Processes=$measureChildren.Count;PrivateBytes=$measurePrivate;CpuPercentOneCore=$measureCpuPercent})
    $measureLast = $measureNow
    if ($measureIndex -lt $Samples - 1) { Start-Sleep -Milliseconds 1000 }
}
$measureResult = [ordered]@{
    Label=$Label;Executable=$measureExpected;Sha256=(Get-FileHash -LiteralPath $measureExpected -Algorithm SHA256).Hash
    ProcessStarted=$measureStarted.ToUniversalTime().ToString('o');LogicalProcessors=[Environment]::ProcessorCount
    Qualification='仅进程树采样；界面内容、数据、DPI、下载状态和启动首帧需另外核实。'
    MeanPrivateMiB=($measureRecords | Measure-Object PrivateBytes -Average).Average / 1MB
    MeanCpuPercentOneCore=($measureRecords | Where-Object { $null -ne $_.CpuPercentOneCore } | Measure-Object CpuPercentOneCore -Average).Average
    GpuUsage=$null;StartupTime=$null;Samples=$measureRecords
}
$measureDestination = Join-Path $measureRoot 'artifacts/native-migration/measurements'
New-Item -ItemType Directory -Path $measureDestination -Force | Out-Null
$measureOutput = Join-Path $measureDestination "$Label-$([DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')).json"
$measureResult | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $measureOutput -Encoding utf8
[pscustomobject]@{Report=$measureOutput;MeanPrivateMiB=$measureResult.MeanPrivateMiB;MeanCpuPercentOneCore=$measureResult.MeanCpuPercentOneCore}
