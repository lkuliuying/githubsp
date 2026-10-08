param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$historyRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$historyArgs = @('build', '-p', 'githubsp-core', '--example', 'history_scaling', '--release', '--locked')
if ($Offline) { $historyArgs += '--offline' }
& (Join-Path $PSScriptRoot 'native-cargo.ps1') -CargoArguments $historyArgs
$historyExe = Join-Path $historyRoot 'native-desktop/target/x86_64-pc-windows-msvc/release/examples/history_scaling.exe'
$historyOutput = Join-Path $historyRoot ('artifacts/slint-retirement/history-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $historyOutput | Out-Null
$historyResults = @()
foreach ($historyCount in @(1000,10000,100000)) {
    $historyDatabase = Join-Path $historyOutput "$historyCount.sqlite3"
    & $historyExe seed $historyDatabase $historyCount
    if ($LASTEXITCODE -ne 0) { throw '创建隔离历史样本失败。' }
    $historyStart = [Diagnostics.ProcessStartInfo]::new($historyExe)
    $historyStart.UseShellExecute = $false
    $historyStart.CreateNoWindow = $true
    $historyStart.RedirectStandardInput = $true
    $historyStart.RedirectStandardOutput = $true
    $historyStart.RedirectStandardError = $true
    foreach ($historyArgument in @('measure',$historyDatabase,"$historyCount")) { [void]$historyStart.ArgumentList.Add($historyArgument) }
    $historyProcess = [Diagnostics.Process]::Start($historyStart)
    try {
        $historyErrors = $historyProcess.StandardError.ReadToEndAsync()
        $historyLine = $historyProcess.StandardOutput.ReadLineAsync()
        if (-not $historyLine.Wait(60000)) { throw '历史性能测试超时。' }
        if (-not $historyLine.Result) { throw '历史性能测试未返回结果。' }
        $historyResult = $historyLine.Result | ConvertFrom-Json
        $historyProcess.Refresh()
        $historyResult | Add-Member -NotePropertyName PrivateBytes -NotePropertyValue $historyProcess.PrivateMemorySize64
        $historyResult | Add-Member -NotePropertyName Qualification -NotePropertyValue '独立核心进程；未包含 Slint、GPU、窗口首帧或旧 WebView 对比。'
        $historyResults += $historyResult
        $historyProcess.StandardInput.WriteLine('complete')
        if (-not $historyProcess.WaitForExit(10000)) { throw '历史性能测试未正常退出。' }
        if ($historyProcess.ExitCode -ne 0) { throw "历史性能测试失败：$($historyErrors.Result)" }
    } finally {
        # 只回收本脚本持有的验收子进程，样本及测量报告保留在隔离目录。
        if (-not $historyProcess.HasExited) { $historyProcess.Kill(); $historyProcess.WaitForExit() }
        $historyProcess.Dispose()
    }
}
$historyResults | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $historyOutput 'results.json') -Encoding utf8
$historyResults | Select-Object completed,residentTasks,startupMilliseconds,PrivateBytes
Write-Output "测量报告：$historyOutput/results.json"
