param(
    [switch]$Offline,
    [string]$ReleaseRepository = 'lkuliuying/githubsp',
    [string]$WebView2RuntimeDirectory,
    [string]$OutputDirectory = 'artifacts/build/tauri'
)
$ErrorActionPreference = 'Stop'
$desktopPackageRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$desktopPackageFrontend = Join-Path $desktopPackageRoot 'desktop'
if ($ReleaseRepository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') { throw '发布源必须为 owner/repo。' }
$desktopPackageOutput = [IO.Path]::GetFullPath((Join-Path $desktopPackageRoot $OutputDirectory))
if (-not $desktopPackageOutput.StartsWith(($desktopPackageRoot + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase)) { throw '产物目录必须位于当前项目中。' }
if ($WebView2RuntimeDirectory) {
    $WebView2RuntimeDirectory = (Resolve-Path -LiteralPath $WebView2RuntimeDirectory).Path
    if (-not (Test-Path -LiteralPath (Join-Path $WebView2RuntimeDirectory 'msedgewebview2.exe') -PathType Leaf)) { throw '固定版本运行时目录缺少 msedgewebview2.exe。' }
}
$desktopPackagePreviousFlags = [Environment]::GetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', 'Process')
$desktopPackagePreviousSource = [Environment]::GetEnvironmentVariable('GITHUBSP_RELEASE_REPOSITORY', 'Process')
if ($null -ne $desktopPackagePreviousFlags -or (Test-Path Env:RUSTFLAGS)) { throw '请在没有自定义 Rust 编译参数的进程中打包。' }
Push-Location $desktopPackageFrontend
try {
    [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', ('-C' + [char]31 + 'target-feature=+crt-static'), 'Process')
    [Environment]::SetEnvironmentVariable('GITHUBSP_RELEASE_REPOSITORY', $ReleaseRepository, 'Process')
    & node scripts/notices.mjs --rust
    if ($LASTEXITCODE -ne 0) { throw '运行依赖声明生成失败。' }
    $desktopPackageArgs = @('exec', 'tauri', 'build', '--ci', '--no-sign', '--no-binary-patching', '--target', 'x86_64-pc-windows-msvc', '--bundles', 'nsis', '--', '--locked')
    if ($Offline) { $desktopPackageArgs += '--offline' }
    & pnpm @desktopPackageArgs
    if ($LASTEXITCODE -ne 0) { throw 'Tauri 构建或安装器打包失败。' }
} finally {
    if ($null -eq $desktopPackagePreviousFlags) { Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue } else { [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', $desktopPackagePreviousFlags, 'Process') }
    if ($null -eq $desktopPackagePreviousSource) { Remove-Item Env:GITHUBSP_RELEASE_REPOSITORY -ErrorAction SilentlyContinue } else { [Environment]::SetEnvironmentVariable('GITHUBSP_RELEASE_REPOSITORY', $desktopPackagePreviousSource, 'Process') }
    Pop-Location
}
$desktopPackageVersion = (Get-Content -LiteralPath (Join-Path $desktopPackageFrontend 'package.json') -Raw | ConvertFrom-Json).version
$desktopPackageBinary = Join-Path $desktopPackageRoot 'target/x86_64-pc-windows-msvc/release/githubsp-desktop.exe'
$desktopPackageInstallers = @(Get-ChildItem -LiteralPath (Join-Path $desktopPackageRoot 'target/x86_64-pc-windows-msvc/release/bundle/nsis') -File -Filter "GitHubSP_${desktopPackageVersion}_x64-setup.exe")
if ($desktopPackageInstallers.Count -ne 1) { throw '安装器数量不唯一，停止交付。' }
$desktopPackageStem = "GitHubSP-v$desktopPackageVersion-windows-x64"
New-Item -ItemType Directory -Path $desktopPackageOutput -Force | Out-Null
$desktopPackagePortable = Join-Path $desktopPackageOutput "$desktopPackageStem-Portable"
if (Test-Path -LiteralPath $desktopPackagePortable) { throw "免安装暂存目录已存在，请使用新的输出目录：$desktopPackagePortable" }
New-Item -ItemType Directory -Path $desktopPackagePortable | Out-Null
Copy-Item -LiteralPath $desktopPackageBinary -Destination (Join-Path $desktopPackagePortable 'GitHubSP.exe')
Copy-Item -LiteralPath (Join-Path $desktopPackageRoot 'LICENSE') -Destination (Join-Path $desktopPackagePortable 'LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $desktopPackageFrontend 'THIRD_PARTY_NOTICES.md') -Destination $desktopPackagePortable
Copy-Item -LiteralPath (Join-Path $desktopPackageRoot 'artifacts/build/tauri-notices/FRONTEND-LICENSES.txt') -Destination $desktopPackagePortable
Copy-Item -LiteralPath (Join-Path $desktopPackageRoot 'artifacts/build/tauri-notices/RUST-LICENSES.txt') -Destination $desktopPackagePortable
$desktopPackageReadme = @"
GitHubSP v$desktopPackageVersion · Windows x64 免安装版

解压完整目录后运行 GitHubSP.exe，无需安装 GitHubSP。
本轻量包使用系统已有的 Microsoft Edge WebView2 Runtime。
运行时缺失时，请使用同版本 Setup 安装版补齐，或安装 Microsoft 官方 WebView2 Evergreen Runtime。
https://developer.microsoft.com/microsoft-edge/webview2/

默认沿用正式数据目录及已保存的数据位置。安装版与免安装版不能同时打开同一数据目录。
需要数据随程序移动时，在 exe 同目录创建空文件 portable-mode.flag；之后使用同目录 data 文件夹。
随身数据模式不会自动导入正式数据，也不能迁移正式数据位置。
调试隔离运行：GitHubSP.exe --data-dir <绝对路径>

本程序未进行代码签名。下载的文件不会自动运行。
"@
$desktopPackageReadme | Set-Content -LiteralPath (Join-Path $desktopPackagePortable 'README.txt') -Encoding utf8
Compress-Archive -LiteralPath $desktopPackagePortable -DestinationPath (Join-Path $desktopPackageOutput "$desktopPackageStem-Portable.zip") -Force
Copy-Item -LiteralPath $desktopPackageInstallers[0].FullName -Destination (Join-Path $desktopPackageOutput "$desktopPackageStem-Setup.exe") -Force
foreach ($desktopPackageNotice in @('FRONTEND-LICENSES.txt','RUST-LICENSES.txt','THIRD_PARTY_NOTICES.md')) { Copy-Item -LiteralPath (Join-Path $desktopPackagePortable $desktopPackageNotice) -Destination $desktopPackageOutput -Force }
if ($WebView2RuntimeDirectory) {
    Copy-Item -LiteralPath $WebView2RuntimeDirectory -Destination (Join-Path $desktopPackagePortable 'webview2') -Recurse
    Compress-Archive -LiteralPath $desktopPackagePortable -DestinationPath (Join-Path $desktopPackageOutput "$desktopPackageStem-Portable-Offline.zip") -Force
}
$desktopPackageArtifacts = @(Get-ChildItem -LiteralPath $desktopPackageOutput -File | Where-Object { $_.Name -match '\.(exe|zip)$' })
$desktopPackageArtifacts | ForEach-Object { "$( (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash )  $($_.Name)" } | Set-Content -LiteralPath (Join-Path $desktopPackageOutput 'SHA256SUMS.txt') -Encoding utf8
[ordered]@{
    Version=$desktopPackageVersion; Stack='Tauri 2 + React + Rust'; Published=$false; Signed=$false
    ReleaseRepository=$ReleaseRepository; InstallerWebView2='缺失时下载并安装 Evergreen Runtime'
    PortableWebView2='轻量包使用系统运行时；可选 webview2 目录提供固定版本运行时'
    Files=@($desktopPackageArtifacts | ForEach-Object { [ordered]@{ Name=$_.Name; Bytes=$_.Length; Sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash } })
    Pending=@('干净 Windows 的安装与卸载','缺少 WebView2 的系统','多 DPI 与多显示器','真实网络完整下载回归')
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $desktopPackageOutput 'package.json') -Encoding utf8
Write-Output "安装版与免安装版已生成：$desktopPackageOutput"
