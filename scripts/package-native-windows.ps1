param(
    [switch]$Offline,
    [string]$ReleaseRepository = 'lkuliuying/githubsp',
    [ValidateNotNullOrEmpty()][string]$OutputDirectory = 'artifacts/slint-retirement/package'
)
$ErrorActionPreference = 'Stop'
$packageRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$packageMetadata = & cargo metadata --manifest-path (Join-Path $packageRoot 'Cargo.toml') --no-deps --locked --offline --format-version 1
if ($LASTEXITCODE -ne 0) { throw '无法读取 workspace 版本。' }
$packageVersion = ((($packageMetadata -join "`n") | ConvertFrom-Json).packages | Where-Object { $_.name -eq 'githubsp-native' }).version
if ($packageVersion -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') { throw '原生版本号不适用于分发文件名。' }
& (Join-Path $PSScriptRoot 'native-notices.ps1')
$packageCargo = @('build', '--release', '--locked')
if ($Offline) { $packageCargo += '--offline' }
if ($ReleaseRepository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') { throw '更新源必须是公开 GitHub owner/repo 标识。' }
$packagePreviousRepository = [Environment]::GetEnvironmentVariable('GITHUBSP_RELEASE_REPOSITORY', 'Process')
try {
    [Environment]::SetEnvironmentVariable('GITHUBSP_RELEASE_REPOSITORY', $ReleaseRepository, 'Process')
    & (Join-Path $PSScriptRoot 'native-cargo.ps1') -CargoArguments $packageCargo
} finally {
    if ($null -eq $packagePreviousRepository) { Remove-Item Env:GITHUBSP_RELEASE_REPOSITORY -ErrorAction SilentlyContinue }
    else { [Environment]::SetEnvironmentVariable('GITHUBSP_RELEASE_REPOSITORY', $packagePreviousRepository, 'Process') }
}
$packageTree = & cargo tree --manifest-path (Join-Path $packageRoot 'Cargo.toml') -p githubsp-native --target x86_64-pc-windows-msvc --locked --offline -e normal,no-proc-macro --prefix none --format '{p}'
if ($LASTEXITCODE -ne 0) { throw '无法核对运行依赖树。' }
if ($packageTree | Where-Object { $_ -match '^(tauri(?:-|\s)|wry\s|webview2)' }) { throw '原生产物仍包含 Tauri 或 WebView 运行依赖。' }
$packageVswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path -LiteralPath $packageVswhere)) { throw '未找到 Visual Studio Build Tools 的 vswhere，无法核对 PE 导入。' }
$packageVs = & $packageVswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$packageDumpbin = @(Get-ChildItem -LiteralPath (Join-Path $packageVs 'VC/Tools/MSVC') -Directory | Sort-Object Name -Descending | ForEach-Object { Join-Path $_.FullName 'bin/Hostx64/x64/dumpbin.exe' } | Where-Object { Test-Path -LiteralPath $_ }) | Select-Object -First 1
if (-not $packageDumpbin) { throw '未找到 dumpbin，停止打包。' }
$packageBinary = Join-Path $packageRoot 'native-desktop/target/x86_64-pc-windows-msvc/release/githubsp-native.exe'
$packageImports = & $packageDumpbin /dependents $packageBinary
if ($LASTEXITCODE -ne 0) { throw '读取 PE 导入表失败。' }
$packageLibraries = @($packageImports | ForEach-Object { if ($_ -match '^\s+(\S+\.dll)\s*$') { $Matches[1] } })
if ($packageLibraries.Count -eq 0) { throw '导入表为空，不能判断独立运行条件。' }
if ($packageLibraries | Where-Object { $_ -match '(?i)webview|vcruntime|msvcp\d|node\.dll|python\d|libssl|libcrypto' }) { throw '检测到需核对的非系统运行库，停止打包。' }
$packageOutput = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
    [IO.Path]::GetFullPath($OutputDirectory)
} else {
    [IO.Path]::GetFullPath((Join-Path $packageRoot $OutputDirectory))
}
New-Item -ItemType Directory -Path $packageOutput -Force | Out-Null
$packageFileName = "GitHubSP-v$packageVersion-windows-x64.exe"
$packageFile = Join-Path $packageOutput $packageFileName
Copy-Item -LiteralPath $packageBinary -Destination $packageFile -Force
$packageHash = (Get-FileHash -LiteralPath $packageFile -Algorithm SHA256).Hash
[ordered]@{
    File=$packageFileName;Sha256=$packageHash;Bytes=(Get-Item -LiteralPath $packageFile).Length
    Stage='Slint 独立桌面构建；未发布';DefaultDataDirectory='%LOCALAPPDATA%\com.githubsp.desktop';DatabaseVersion=2
    StorageRevision=1;CompatibleLegacyVersions=@('0.2.0','0.2.1','0.2.2','0.2.3');ExcludedUnpatchedVersions=@('0.2.4')
    Renderer='Slint 1.18.1 / Winit / FemtoVG-WGPU / DX12';StaticCrtRequested=$true
    ReleaseRepository=$ReleaseRepository
    DirectImports=$packageLibraries
    Pending=@('中文输入法与多档 DPI 实机验收','多显示器及提醒不抢焦点验收','同场景资源基准与启动耗时','干净 Windows 离线启动','完整真实网络下载回归')
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $packageOutput 'acceptance.json') -Encoding utf8
"$packageHash  $packageFileName" | Set-Content -LiteralPath (Join-Path $packageOutput 'SHA256SUMS.txt') -Encoding utf8
$packageTree | Set-Content -LiteralPath (Join-Path $packageOutput 'runtime-dependencies.txt') -Encoding utf8
Write-Output "已生成免安装程序：$packageFile"
