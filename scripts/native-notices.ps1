$ErrorActionPreference = 'Stop'
$noticeRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$noticeJson = & cargo metadata --manifest-path (Join-Path $noticeRoot 'Cargo.toml') --filter-platform x86_64-pc-windows-msvc --locked --offline --format-version 1
if ($LASTEXITCODE -ne 0) { throw '无法读取原生依赖元数据。' }
$noticeData = ($noticeJson -join "`n") | ConvertFrom-Json
$noticeRuntime = & cargo tree --manifest-path (Join-Path $noticeRoot 'Cargo.toml') -p githubsp-native --target x86_64-pc-windows-msvc --locked --offline -e normal,no-proc-macro --prefix none --format '{p}'
if ($LASTEXITCODE -ne 0) { throw '无法读取运行依赖树。' }
$noticeRuntimeSet = [Collections.Generic.HashSet[string]]::new()
foreach ($noticeEntry in $noticeRuntime) { if ($noticeEntry -match '^(\S+) v(\S+)') { [void]$noticeRuntimeSet.Add("$($Matches[1]) $($Matches[2])") } }
$noticeSources = Get-Content -LiteralPath (Join-Path $noticeRoot 'native-desktop/resources/license-sources.json') -Raw | ConvertFrom-Json
$noticePackages = @{}
$noticeNodes = @{}
foreach ($noticePackage in $noticeData.packages) { $noticePackages[$noticePackage.id] = $noticePackage }
foreach ($noticeNode in $noticeData.resolve.nodes) { $noticeNodes[$noticeNode.id] = $noticeNode }
$noticeQueue = [Collections.Generic.Queue[string]]::new()
$noticeVisited = [Collections.Generic.HashSet[string]]::new()
$noticeNative = @($noticeData.packages | Where-Object { $_.name -eq 'githubsp-native' -and $null -eq $_.source })
if ($noticeNative.Count -ne 1) { throw '无法确定原生 workspace 包。' }
$noticeQueue.Enqueue($noticeNative[0].id)
while ($noticeQueue.Count -gt 0) {
    $noticeId = $noticeQueue.Dequeue()
    if (-not $noticeVisited.Add($noticeId)) { continue }
    foreach ($noticeDep in $noticeNodes[$noticeId].deps) {
        if (@($noticeDep.dep_kinds | Where-Object { $null -eq $_.kind }).Count -gt 0) { $noticeQueue.Enqueue($noticeDep.pkg) }
    }
}
$noticeText = [Text.StringBuilder]::new()
[void]$noticeText.AppendLine('GitHubSP 原生桌面第三方声明')
[void]$noticeText.AppendLine('本程序中的 Slint 选择 LicenseRef-Slint-Royalty-free-2.0；其他组件分别遵循所列许可证。')
[void]$noticeText.AppendLine([IO.File]::ReadAllText((Join-Path $noticeRoot 'LICENSE')))
[void]$noticeText.AppendLine('Noto Sans SC Regular：官方简体中文静态字体，保持原字形和 OFL 许可。')
[void]$noticeText.AppendLine('来源：https://github.com/notofonts/noto-cjk/tree/main/Sans/SubsetOTF/SC')
[void]$noticeText.AppendLine('Copyright 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name Source.')
[void]$noticeText.AppendLine([IO.File]::ReadAllText((Join-Path $noticeRoot 'native-desktop/resources/NotoSansSC-OFL.txt')))
[void]$noticeText.AppendLine('Phosphor Icons：沿用原页面图标，静态 SVG 源于 @phosphor-icons/vue 2.2.1，构建无需 npm 包。')
[void]$noticeText.AppendLine([IO.File]::ReadAllText((Join-Path $noticeRoot 'native-desktop/resources/Phosphor-LICENSE.txt')))
$noticeMissing = [Collections.Generic.List[string]]::new()
foreach ($noticePackage in @($noticeVisited | ForEach-Object { $noticePackages[$_] } | Sort-Object name,version)) {
    if (-not $noticePackage.source -or -not $noticeRuntimeSet.Contains("$($noticePackage.name) $($noticePackage.version)")) { continue }
    $noticeDirectory = Split-Path -Parent $noticePackage.manifest_path
    $noticeFiles = @(Get-ChildItem -LiteralPath $noticeDirectory -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([.\-_]|$)' })
    $noticeLicenseDirectory = Join-Path $noticeDirectory 'LICENSES'
    if (Test-Path -LiteralPath $noticeLicenseDirectory -PathType Container) { $noticeFiles += @(Get-ChildItem -LiteralPath $noticeLicenseDirectory -File) }
    if ($noticePackage.license_file) {
        $noticeExplicit = Join-Path $noticeDirectory $noticePackage.license_file
        if (Test-Path -LiteralPath $noticeExplicit -PathType Leaf) { $noticeFiles += Get-Item -LiteralPath $noticeExplicit }
    }
    if ($noticePackage.license -match 'LicenseRef-Slint-Royalty-free') {
        $noticeFiles = @($noticeFiles | Where-Object { $_.Name -match 'Royalty-free-2.0|COPYRIGHT|NOTICE' })
    }
    foreach ($noticeSource in $noticeSources) {
        if ($noticePackage.name -in $noticeSource.packages) {
            foreach ($noticeSupplement in $noticeSource.files) {
                $noticeFiles += Get-Item -LiteralPath (Join-Path $noticeRoot "native-desktop/resources/licenses/$($noticeSupplement.file)")
            }
        }
    }
    [void]$noticeText.AppendLine("`n--- $($noticePackage.name) $($noticePackage.version) ---")
    [void]$noticeText.AppendLine("许可证：$($noticePackage.license)")
    [void]$noticeText.AppendLine("来源：$($noticePackage.repository)")
    if ($noticeFiles.Count -eq 0) { $noticeMissing.Add("$($noticePackage.name) $($noticePackage.version)"); continue }
    foreach ($noticeFile in @($noticeFiles | Sort-Object FullName -Unique)) {
        [void]$noticeText.AppendLine("[$($noticeFile.Name)]")
        [void]$noticeText.AppendLine([IO.File]::ReadAllText($noticeFile.FullName))
    }
}
# SQLite 随 rusqlite 的 bundled 特性静态编译，另外保留上游版权说明。
[void]$noticeText.AppendLine("`nSQLite：Public Domain，https://www.sqlite.org/copyright.html")
if ($noticeMissing.Count -gt 0) { throw "以下依赖缺少可提取的许可文件，请先补齐：$($noticeMissing -join ', ')" }
# 只规范汇总文件的换行和行尾空白，不改动许可正文或上游原文件。
$noticeOutput = [regex]::Replace($noticeText.ToString().Replace("`r`n", "`n"), '[\t ]+$', '', [Text.RegularExpressions.RegexOptions]::Multiline)
[IO.File]::WriteAllText((Join-Path $noticeRoot 'native-desktop/resources/THIRD_PARTY_NOTICES.txt'), $noticeOutput, [Text.UTF8Encoding]::new($false))
Write-Output "已生成 $($noticeRuntimeSet.Count) 个运行依赖节点的声明。"
