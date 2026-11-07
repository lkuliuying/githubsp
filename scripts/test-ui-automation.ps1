#Requires -Version 7.0
. "$PSScriptRoot/automation-common.ps1"
Add-Type -AssemblyName System.Drawing.Common
$output = Resolve-AutomationPath ('artifacts/automation/self-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path "$output/baseline", "$output/actual" -Force | Out-Null
@{ channelTolerance = 8; maxChangedRatio = 0.001; images = @('sample.png') } | ConvertTo-Json |
    Set-Content -LiteralPath "$output/baseline/manifest.json" -Encoding utf8
function Save-TestImage([string]$File, [int]$Size, [bool]$Change) {
    $bitmap = [Drawing.Bitmap]::new($Size, $Size)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([Drawing.Color]::White)
        if ($Change) { $graphics.FillRectangle([Drawing.Brushes]::Black, 0, 0, $Size / 2, $Size / 2) }
        $bitmap.Save($File, [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
function Assert-Compare([bool]$Expected, [string]$Name) {
    $arguments = @('-NoProfile', '-File', "$PSScriptRoot/compare-ui-snapshots.ps1", '-BaselineDirectory', "$output/baseline", '-ActualDirectory', "$output/actual", '-OutputDirectory', "$output/$Name")
    $code = Invoke-AutomationProcess (Get-Command pwsh).Source $arguments "$output/$Name/command" 30
    if (($code -eq 0) -ne $Expected) { throw "视觉比较失败路径不符合预期：$Name，退出码 $code" }
}
Save-TestImage "$output/baseline/sample.png" 20 $false
Save-TestImage "$output/actual/sample.png" 20 $false
Assert-Compare $true 'same'
Save-TestImage "$output/actual/sample.png" 20 $true
Assert-Compare $false 'changed'
Save-TestImage "$output/actual/sample.png" 24 $false
Assert-Compare $false 'dimensions'
Move-Item -LiteralPath "$output/actual/sample.png" -Destination "$output/actual/other.png"
Assert-Compare $false 'missing'
Move-Item -LiteralPath "$output/actual/other.png" -Destination "$output/actual/sample.png"
Move-Item -LiteralPath "$output/baseline/sample.png" -Destination "$output/baseline/removed.png"
Assert-Compare $false 'missing-baseline'
$blocked = $false
try { [void](Resolve-AutomationPath '../outside.png') } catch { $blocked = $true }
if (-not $blocked) { throw '仓库外路径没有被拒绝。' }
$protected = $false
try { Assert-AutomationScope @('native-desktop/src/ui_tests.rs') } catch { $protected = $true }
if (-not $protected) { throw '任务白名单允许了受保护的测试文件。' }
$protected = $false
try { Assert-AutomationScope @('native-desktop/src/completion/tests.rs') } catch { $protected = $true }
if (-not $protected) { throw '任务白名单允许了 tests.rs 文件。' }
@{ objective = '字段缺失的任务' } | ConvertTo-Json | Set-Content -LiteralPath "$output/invalid-task.json" -Encoding utf8
$invalidArgs = @('-NoProfile', '-File', "$PSScriptRoot/invoke-ui-iteration.ps1", '-TaskFile', "$output/invalid-task.json")
$code = Invoke-AutomationProcess (Get-Command pwsh).Source $invalidArgs "$output/invalid-task" 10
if ($code -eq 0 -or (Get-Content -LiteralPath "$output/invalid-task.stderr.log" -Raw) -notlike '*任务必须提供*') { throw '不完整任务未在启动模型前被拒绝。' }
$timeout = $false
try { [void](Invoke-AutomationProcess (Get-Command pwsh).Source @('-NoProfile', '-Command', 'Start-Sleep -Seconds 20') "$output/timeout" 1) }
catch { $timeout = $_.Exception.Message -like '*超时*' }
if (-not $timeout) { throw '子进程超时没有使流水线失败。' }
$code = Invoke-AutomationProcess (Get-Command pwsh).Source @('-NoProfile', '-Command', 'exit 7') "$output/exit-code" 10
if ($code -ne 7) { throw '子进程失败码未被保留。' }
@{ Passed = $true; Cases = @('same', 'changed', 'dimensions', 'missing', 'missing-baseline', 'path-boundary', 'protected-test', 'protected-tests-module', 'invalid-task', 'timeout', 'exit-code') } |
    ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$output/result.json" -Encoding utf8
Write-Host "自动化失败路径通过：11 项；证据：$output"
