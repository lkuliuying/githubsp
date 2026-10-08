param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$startupRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$startupExe = (Resolve-Path -LiteralPath $Executable).Path
if (-not $startupExe.StartsWith($startupRoot + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw '仅验收当前工作区内的原生产物。' }
$startupOutput = Join-Path $startupRoot ('artifacts/slint-retirement/startup-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$startupData = Join-Path $startupOutput '中文 隔离数据'
New-Item -ItemType Directory -Path $startupData | Out-Null
$startupTimer = [Diagnostics.Stopwatch]::StartNew()
$startupProcess = Start-Process -FilePath $startupExe -ArgumentList @('--data-dir', ('"' + $startupData + '"')) -WindowStyle Hidden -PassThru
$startupSecond = $null
try {
    do {
        if ($startupProcess.HasExited) { throw "原生程序提前退出：$($startupProcess.ExitCode)" }
        $startupProcess.Refresh()
        if ($startupProcess.MainWindowHandle -ne 0 -and (Test-Path -LiteralPath (Join-Path $startupData 'tasks.sqlite3'))) { break }
        if ($startupTimer.Elapsed.TotalSeconds -gt 30) { throw '30 秒内未发现原生主窗口和隔离数据库。' }
        Start-Sleep -Milliseconds 100
    } while ($true)
    $startupWindowMilliseconds = $startupTimer.Elapsed.TotalMilliseconds
    $startupModules = @($startupProcess.Modules | ForEach-Object { [ordered]@{Name=$_.ModuleName;Path=$_.FileName} })
    $startupSuspects = @($startupModules | Where-Object { $_.Name -match '(?i)webview|vcruntime|msvcp\d|node\.dll|python\d|libssl|libcrypto' })
    $startupSecond = Start-Process -FilePath $startupExe -ArgumentList @('--data-dir', ('"' + $startupData + '"')) -WindowStyle Hidden -PassThru
    if (-not $startupSecond.WaitForExit(30000)) { throw '第二实例没有退出。' }
    if ($startupSecond.ExitCode -ne 0) { throw '第二实例启动失败。' }
    $startupProcess.Refresh()
    if ($startupProcess.HasExited) { throw '第二实例错误地结束了原实例。' }
    $startupPrivate = $startupProcess.PrivateMemorySize64
    if (-not $startupProcess.CloseMainWindow()) { throw '无法请求原生窗口正常关闭。' }
    if (-not $startupProcess.WaitForExit(30000)) { throw '原生程序未在 30 秒内正常保存退出。' }
    if ($startupProcess.ExitCode -ne 0) { throw "原生程序异常退出：$($startupProcess.ExitCode)" }
    [ordered]@{
        Executable=$startupExe;Sha256=(Get-FileHash -LiteralPath $startupExe -Algorithm SHA256).Hash
        DataDirectory=$startupData;WindowAndDatabaseMilliseconds=$startupWindowMilliseconds
        PrivateBytes=$startupPrivate;SingleInstance=$true;GracefulExit=$true
        Modules=$startupModules;SuspectModules=$startupSuspects
        Qualification='当前开发机隔离启动和模块快照；未模拟干净系统、断网、首帧、输入法、托盘或 DPI。'
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $startupOutput 'result.json') -Encoding utf8
    if ($startupSuspects.Count -gt 0) { throw "需人工核对加载模块，报告已保存：$startupOutput/result.json" }
    Write-Output "原生窗口、中文隔离目录、第二实例退出和正常保存退出通过：$startupOutput/result.json"
} finally {
    # 只回收本脚本创建并持有的验收进程，不按名称停止用户程序。
    foreach ($startupOwned in @($startupSecond,$startupProcess)) {
        if ($null -ne $startupOwned) {
            if (-not $startupOwned.HasExited) { $startupOwned.Kill(); $startupOwned.WaitForExit() }
            $startupOwned.Dispose()
        }
    }
}
