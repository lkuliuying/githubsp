param(
    [switch]$Offline,
    [ValidateNotNullOrEmpty()][string]$OutputDirectory = ('artifacts/build/v024-compat/' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
)
$ErrorActionPreference = 'Stop'
$compatRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$compatCommit = '34a80f7e083edbdc2e2a082e9c94249d759c398d'
$compatOutput = if ([IO.Path]::IsPathRooted($OutputDirectory)) { [IO.Path]::GetFullPath($OutputDirectory) } else { [IO.Path]::GetFullPath((Join-Path $compatRoot $OutputDirectory)) }
if (Test-Path -LiteralPath $compatOutput) {
    $compatExisting = Get-Item -LiteralPath $compatOutput -Force
    if (-not $compatExisting.PSIsContainer -or ($compatExisting.Attributes -band [IO.FileAttributes]::ReparsePoint) -or @(Get-ChildItem -LiteralPath $compatOutput -Force).Count -ne 0) {
        throw '输出目录必须不存在或为空的普通目录，不覆盖现有内容。'
    }
}
$compatResolved = & git -C $compatRoot rev-parse --verify ($compatCommit + '^{commit}')
if ($LASTEXITCODE -ne 0 -or $compatResolved -ne $compatCommit) { throw '缺少固定的 v0.2.4 兼容修复提交，停止构建。' }
$compatPaths = @(& git -C $compatRoot -c core.quotepath=false ls-tree -r --name-only $compatCommit)
if ($LASTEXITCODE -ne 0 -or $compatPaths.Count -eq 0) { throw '无法读取固定源码清单。' }
if (-not (Test-Path -LiteralPath $compatOutput)) { New-Item -ItemType Directory -Path $compatOutput | Out-Null }
$compatAudit = Join-Path $compatOutput 'audit'
# 不使用 Force；并发构建或残留目录必须报错，不能复用另一轮的产物。
New-Item -ItemType Directory -Path $compatAudit | Out-Null
$compatSource = Join-Path $compatOutput 'source'
$compatArchive = Join-Path $compatAudit 'base-source.zip'
& git -C $compatRoot -c core.autocrlf=false archive --format=zip "--output=$compatArchive" $compatCommit
if ($LASTEXITCODE -ne 0) { throw '导出固定源码失败。' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::ExtractToDirectory($compatArchive, $compatSource)
$compatOriginalHashes = @{}
foreach ($compatPath in $compatPaths) { $compatOriginalHashes[$compatPath] = (Get-FileHash -LiteralPath (Join-Path $compatSource $compatPath) -Algorithm SHA256).Hash }

function Set-CompatText([string]$RelativePath, [string]$Before, [string]$After) {
    $compatEditPath = Join-Path $compatSource $RelativePath
    $compatText = [IO.File]::ReadAllText($compatEditPath)
    if ([regex]::Matches($compatText, [regex]::Escape($Before)).Count -ne 1) { throw "固定源码内容不匹配：$RelativePath" }
    [IO.File]::WriteAllText($compatEditPath, $compatText.Replace($Before, $After), [Text.UTF8Encoding]::new($false))
}
Set-CompatText 'native-desktop/ui/windows.slint' '让优秀的开源项目触手可及。' '数据库兼容修复版 compat.1'
# 仅调整隔离源码的分发标识，使源码包自身也能生成不会混淆的修复版文件名。
Set-CompatText 'scripts/package-native-windows.ps1' '$packageFileName = "GitHubSP-v$packageVersion-windows-x64.exe"' '$packageFileName = "GitHubSP-v$packageVersion-compat.1-windows-x64.exe"'
Set-CompatText 'scripts/package-native-windows.ps1' "StorageRevision=1;CompatibleLegacyVersions=" "StorageRevision=1;CompatibilityPatch='compat.1';CompatibleFixedVersions=@('0.2.4-compat.1','0.2.5');CompatibleLegacyVersions="
$compatReadme = [IO.File]::ReadAllText((Join-Path $compatRoot 'README.md'))
$compatHeader = @'
# GitHubSP v0.2.4 数据库兼容修复版 compat.1

本源码基于 v0.2.4 及完整存储修复提交 34a80f7e083edbdc2e2a082e9c94249d759c398d。Cargo 版本仍为 0.2.4，关于窗口带有 compat.1 标识；主线正式版为 v0.2.5。下文 README 来自维护主线，发布链接指向主线版本，不改变本源码版本。

从本目录运行 `.\scripts\package-native-windows.ps1 -Offline`，默认在 `artifacts/slint-retirement/package/` 生成 `GitHubSP-v0.2.4-compat.1-windows-x64.exe`。不需要原仓库 Git 历史；首次获取依赖时省略 `-Offline`。运行只需 exe，不需要源码或构建工具。

先从托盘退出其他版本，再启动修复版；默认保留现有任务、历史、收藏、设置和续传信息。兼容 v0.2.0～v0.2.3 原版及 v0.2.5，原版 v0.2.4 仍须替换。不要删除数据库、手动修改版本号或恢复旧快照来切换版本。首次转换旧结构前自动生成完整备份；已是兼容结构时无需再次迁移。验收范围以随包验收记录为准。

This source builds v0.2.4 compat.1, not the unpatched v0.2.4 binary. The complete storage fix is pinned to the commit above; Cargo remains at 0.2.4 and update checks can discover v0.2.5. Run the packaging command above from this directory to reproduce the labeled executable without repository history. Exit the active app before switching. Original v0.2.0–v0.2.3 and v0.2.5 share the compatible database; replace unpatched v0.2.4. The project README below describes the maintained mainline and its published release.

---

'@
[IO.File]::WriteAllText((Join-Path $compatSource 'README.md'), $compatHeader + "`n" + $compatReadme, [Text.UTF8Encoding]::new($false))
$compatMetadata = & cargo metadata --manifest-path (Join-Path $compatSource 'Cargo.toml') --no-deps --locked --offline --format-version 1
if ($LASTEXITCODE -ne 0) { throw '无法核对隔离 workspace。' }
$compatPackages = @(($compatMetadata -join "`n" | ConvertFrom-Json).packages)
if ($compatPackages.Count -ne 2 -or @($compatPackages | Where-Object version -ne '0.2.4').Count -ne 0) { throw '隔离源码版本不是 0.2.4。' }
$compatProvenance = [ordered]@{
    SourceCommit=$compatCommit;BaseTag='v0.2.4';Version='0.2.4';CompatibilityPatch='compat.1'
    DatabaseVersion=2;StorageRevision=1;DocumentationSha256=(Get-FileHash -LiteralPath (Join-Path $compatRoot 'README.md') -Algorithm SHA256).Hash
    ModifiedSourceFiles=@('README.md','native-desktop/ui/windows.slint','scripts/package-native-windows.ps1')
    Qualification='本地兼容修复构建，不是原版 v0.2.4；构建成功不代表桌面和真实下载验收已完成。'
}
$compatProvenance | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $compatSource 'COMPAT-BUILD.json') -Encoding utf8
Push-Location -LiteralPath $compatSource
try {
    & (Join-Path $compatSource 'scripts/package-native-windows.ps1') -Offline:$Offline -OutputDirectory $compatOutput *>&1 | Tee-Object -FilePath (Join-Path $compatAudit 'package.log')
} finally {
    Pop-Location
}
$compatExecutable = Join-Path $compatOutput 'GitHubSP-v0.2.4-compat.1-windows-x64.exe'
if (-not (Test-Path -LiteralPath $compatExecutable -PathType Leaf)) { throw '兼容修复版 exe 未生成。' }
$compatManifest = foreach ($compatPath in $compatPaths) {
    $compatHash = (Get-FileHash -LiteralPath (Join-Path $compatSource $compatPath) -Algorithm SHA256).Hash
    if ($compatHash -ne $compatOriginalHashes[$compatPath] -and $compatPath -notin $compatProvenance.ModifiedSourceFiles) { throw "非预期源码变化：$compatPath" }
    [ordered]@{Path=$compatPath;BaseSha256=$compatOriginalHashes[$compatPath];Sha256=$compatHash}
}
$compatManifest | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $compatAudit 'source-files.json') -Encoding utf8
$compatSourceName = 'GitHubSP-v0.2.4-compat.1-source.zip'
$compatZip = [IO.Compression.ZipFile]::Open((Join-Path $compatOutput $compatSourceName), [IO.Compression.ZipArchiveMode]::Create)
try {
    # 只打包固定源码清单与来源记录，绝不递归收入 target、测试库、日志或用户文件。
    foreach ($compatPath in @($compatPaths) + @('COMPAT-BUILD.json')) {
        [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($compatZip, (Join-Path $compatSource $compatPath), ('GitHubSP-v0.2.4-compat.1/' + $compatPath), [IO.Compression.CompressionLevel]::Optimal)
    }
} finally {
    $compatZip.Dispose()
}
Copy-Item -LiteralPath (Join-Path $compatSource 'LICENSE') -Destination (Join-Path $compatOutput 'LICENSE')
Copy-Item -LiteralPath (Join-Path $compatSource 'native-desktop/resources/THIRD_PARTY_NOTICES.txt') -Destination (Join-Path $compatOutput 'THIRD_PARTY_NOTICES.txt')
$compatAssets = foreach ($compatName in @('GitHubSP-v0.2.4-compat.1-windows-x64.exe', $compatSourceName, 'LICENSE', 'THIRD_PARTY_NOTICES.txt')) {
    $compatAsset = Join-Path $compatOutput $compatName
    [ordered]@{File=$compatName;Sha256=(Get-FileHash -LiteralPath $compatAsset -Algorithm SHA256).Hash;Bytes=(Get-Item -LiteralPath $compatAsset).Length}
}
$compatAssets | ForEach-Object { "$($_.Sha256)  $($_.File)" } | Set-Content -LiteralPath (Join-Path $compatOutput 'SHA256SUMS.txt') -Encoding utf8
[ordered]@{
    Source=$compatProvenance;SourceFileCount=$compatPaths.Count + 1;BuiltAtUtc=[DateTime]::UtcNow.ToString('o')
    Assets=$compatAssets;Stage='本地构建完成，测试及桌面验收另行记录；未发布'
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $compatOutput 'build-record.json') -Encoding utf8
Write-Output "兼容修复版和源码已生成：$compatOutput"
