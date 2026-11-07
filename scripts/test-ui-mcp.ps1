#Requires -Version 7.0
param([string]$OutputDirectory = 'artifacts/automation/mcp', [switch]$Offline)
. "$PSScriptRoot/automation-common.ps1"
$output = Resolve-AutomationPath $OutputDirectory
$session = $null
$report = [ordered]@{ Passed = $false; Qualification = '隔离软件渲染测试窗口；非 DX12 实机验收'; Error = $null }
try {
    $session = Start-AutomationPreview $output -Offline:$Offline
    $tools = Invoke-SlintMcp $session.Endpoint 'tools/list'
    foreach ($name in @('list_windows', 'take_screenshot', 'get_element_tree')) {
        if ($name -notin $tools.tools.name) { throw "Slint MCP 缺少工具：$name" }
    }
    $windows = Invoke-SlintMcp $session.Endpoint 'tools/call' @{ name = 'list_windows'; arguments = @{} }
    $windows | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $output 'windows.json') -Encoding utf8
    $list = $windows.content[0].text | ConvertFrom-Json
    if ($null -eq $list.windowHandles -or @($list.windowHandles).Count -ne 1) { throw '隔离 MCP 会话应提供一个主窗口。' }
    $handle = $list.windowHandles[0]
    $properties = Invoke-SlintMcp $session.Endpoint 'tools/call' @{ name = 'get_window_properties'; arguments = @{ windowHandle = $handle } }
    $window = $properties.content[0].text | ConvertFrom-Json
    if (-not $window.rootElementHandle) { throw 'MCP 没有返回根元素。' }
    $treeReply = Invoke-SlintMcp $session.Endpoint 'tools/call' @{ name = 'get_element_tree'; arguments = @{ elementHandle = $window.rootElementHandle; maxElements = 1000 } }
    $tree = $treeReply.content[0].text | ConvertFrom-Json
    if (-not $tree.elements -or $tree.elements.Count -lt 2) { throw 'MCP 没有返回有效元素树。' }
    $tree | ConvertTo-Json -Depth 25 | Set-Content -LiteralPath (Join-Path $output 'tree.json') -Encoding utf8
    $capture = Invoke-SlintMcp $session.Endpoint 'tools/call' @{ name = 'take_screenshot'; arguments = @{ windowHandle = $handle } }
    $png = @($capture.content | Where-Object type -eq 'image')
    if ($png.Count -ne 1 -or $png[0].mimeType -ne 'image/png') { throw 'MCP 没有返回 PNG 截图。' }
    [IO.File]::WriteAllBytes((Join-Path $output 'settings-mcp.png'), [Convert]::FromBase64String($png[0].data))
    $tab = @($tree.elements | Where-Object { $_.accessibleRole -eq 'Button' -and $_.accessibleLabel -eq '窗口与提醒' })
    if ($tab.Count -ne 1) { throw '无法唯一定位窗口与提醒分区。' }
    [void](Invoke-SlintMcp $session.Endpoint 'tools/call' @{ name = 'click_element'; arguments = @{ elementHandle = $tab[0].handle } })
    $after = Invoke-SlintMcp $session.Endpoint 'tools/call' @{ name = 'take_screenshot'; arguments = @{ windowHandle = $handle } }
    $afterPng = @($after.content | Where-Object type -eq 'image')
    if ($afterPng.Count -ne 1 -or $afterPng[0].data -eq $png[0].data) { throw '点击分区后没有得到变化的界面截图。' }
    [IO.File]::WriteAllBytes((Join-Path $output 'settings-window-mcp.png'), [Convert]::FromBase64String($afterPng[0].data))
    $report.Passed = $true
    $report.Tools = @($tools.tools.name)
    Write-Host 'Slint MCP 连接、窗口枚举、元素树、点击与截图通过。'
} catch { $report.Error = $_.Exception.Message; throw }
finally {
    Stop-AutomationPreview $session
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    $report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'mcp.json') -Encoding utf8
}
