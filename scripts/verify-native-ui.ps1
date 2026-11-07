#Requires -Version 7.0
param([string]$OutputDirectory = 'artifacts/automation/verify', [switch]$Offline, [switch]$Package, [string]$ReuseVerification)
. "$PSScriptRoot/automation-common.ps1"
$output = Resolve-AutomationPath $OutputDirectory
New-Item -ItemType Directory -Path $output -Force | Out-Null
$report = [ordered]@{ Passed = $false; Commit = (& git -C $script:AutomationRoot rev-parse HEAD); Source = Get-AutomationSource; Steps = @(); Error = $null }
try {
    if ($ReuseVerification) {
        if (-not $Package) { throw '-ReuseVerification 仅用于复用相同源码的验收后生成候选包。' }
        $previousReport = Get-Content -LiteralPath (Join-Path (Resolve-AutomationPath $ReuseVerification) 'verification.json') -Raw | ConvertFrom-Json
        if ($previousReport.Passed -ne $true -or $previousReport.Source -ne $report.Source -or $previousReport.Commit -ne $report.Commit) {
            throw '已有验收不属于当前源码，不能复用。'
        }
        foreach ($required in @('format', 'workspace-tests', 'screenshots', 'visual-baseline', 'clippy')) {
            if ($required -notin $previousReport.Steps) { throw "已有验收缺少步骤：$required" }
        }
        $report.Steps = @($previousReport.Steps)
        $report.ReusedFrom = Resolve-AutomationPath $ReuseVerification
    } else {
        $format = Invoke-AutomationProcess -File (Get-Command cargo).Source -Arguments @('fmt', '--all', '--', '--check') -Log (Join-Path $output 'format')
        if ($format -ne 0) { throw 'Rust 格式检查失败，见 format.stdout.log。' }
        $report.Steps += 'format'
        $extra = if ($Offline) { @('--offline') } else { @() }
        Invoke-AutomationCargo (@('test', '-p', 'githubsp-core', '-p', 'githubsp-native', '--locked') + $extra) (Join-Path $output 'tests')
        $report.Steps += 'workspace-tests'
        $previous = $env:GITHUBSP_UI_RENDER_DIRECTORY
        # 每次使用新目录，防止零匹配测试或中断后遗留的旧截图被当成本轮结果。
        $screenshots = Join-Path $output ('screenshots/' + [Guid]::NewGuid().ToString('N'))
        $report.ScreenshotDirectory = $screenshots
        try {
            $env:GITHUBSP_UI_RENDER_DIRECTORY = $screenshots
            Invoke-AutomationCargo (@('test', '-p', 'githubsp-native', '--locked') + $extra + @('ui_tests::automation_ui_tests::automation_visual_scenarios', '--', '--ignored', '--exact')) (Join-Path $output 'screenshots')
        } finally {
            if ($null -eq $previous) { Remove-Item Env:GITHUBSP_UI_RENDER_DIRECTORY -ErrorAction SilentlyContinue }
            else { $env:GITHUBSP_UI_RENDER_DIRECTORY = $previous }
        }
        $report.Steps += 'screenshots'
        & "$PSScriptRoot/compare-ui-snapshots.ps1" -ActualDirectory $screenshots -OutputDirectory (Join-Path $output 'visual')
        $report.Steps += 'visual-baseline'
        Invoke-AutomationCargo (@('clippy', '-p', 'githubsp-core', '-p', 'githubsp-native', '--all-targets', '--locked') + $extra + @('--', '-D', 'warnings')) (Join-Path $output 'clippy')
        $report.Steps += 'clippy'
    }
    if ($Package) {
        & "$PSScriptRoot/package-native-windows.ps1" -Offline:$Offline -OutputDirectory (Join-Path $output 'package')
        $acceptance = Get-Content -LiteralPath (Join-Path $output 'package/acceptance.json') -Raw | ConvertFrom-Json
        $startup = Join-Path $output ('startup/' + [Guid]::NewGuid().ToString('N'))
        & "$PSScriptRoot/test-native-startup.ps1" -Executable (Join-Path $output "package/$($acceptance.File)") -OutputDirectory $startup
        $report.StartupDirectory = $startup
        $report.Steps += 'package-and-native-startup'
        $report.BinaryFile = $acceptance.File
        $report.BinarySha256 = $acceptance.Sha256
    }
    if ((Get-AutomationSource) -ne $report.Source) { throw '验收过程中源码发生变化，本次证据失效。' }
    $report.Passed = $true
} catch { $report.Error = $_.Exception.Message; throw }
finally { $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'verification.json') -Encoding utf8 }
