param([Parameter(Mandatory = $true)][string[]]$CargoArguments)
$ErrorActionPreference = 'Stop'
$nativeRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ($CargoArguments.Count -eq 0 -or $CargoArguments[0] -notin @('build', 'check', 'test', 'clippy', 'run', 'tree')) {
    throw '支持的命令为 build/check/test/clippy/run/tree。'
}
$nativeSeparator = [Array]::IndexOf($CargoArguments, '--')
$nativeCount = if ($nativeSeparator -lt 0) { $CargoArguments.Count } else { $nativeSeparator }
$nativePrefix = @($CargoArguments | Select-Object -First $nativeCount)
$nativeSuffix = if ($nativeSeparator -lt 0) { @() } else { @($CargoArguments | Select-Object -Skip $nativeSeparator) }
if ($nativePrefix | Where-Object { $_ -match '^--(manifest-path|target|target-dir)(=|$)' }) {
    throw '此入口固定原生项目、Windows x64 和输出目录。'
}
$nativeOriginal = [Environment]::GetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', 'Process')
$nativeHadEncoded = Test-Path Env:CARGO_ENCODED_RUSTFLAGS
$nativeRaw = [Environment]::GetEnvironmentVariable('RUSTFLAGS', 'Process')
if ($null -ne $nativeOriginal -or $null -ne $nativeRaw) {
    throw '检测到自定义 Rust 编译参数，请在无自定义参数的进程中运行，以免覆盖静态 CRT 设置。'
}
$nativeArgs = $nativePrefix + @('--manifest-path', (Join-Path $nativeRoot 'Cargo.toml'), '--target', 'x86_64-pc-windows-msvc')
if ($CargoArguments[0] -ne 'tree') { $nativeArgs += @('--target-dir', (Join-Path $nativeRoot 'native-desktop\target')) }
$nativeArgs += $nativeSuffix
try {
    # 显式 target 将静态 CRT 参数限制在目标产物，避免影响编译期宏和构建脚本。
    [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', ('-C' + [char]31 + 'target-feature=+crt-static'), 'Process')
    & cargo @nativeArgs
    if ($LASTEXITCODE -ne 0) { throw "原生 Cargo 命令失败：$LASTEXITCODE" }
} finally {
    # 某些 .NET 版本将 null 参数转为空值；原本不存在的变量应删除进程项。
    if ($nativeHadEncoded) {
        [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', $nativeOriginal, 'Process')
    } else {
        Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
    }
}
