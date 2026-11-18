param(
    [switch]$Offline,
    [switch]$Package,
    [string]$OutputDirectory = 'artifacts/verification/desktop'
)
$ErrorActionPreference = 'Stop'
$verificationRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$verificationOutput = [IO.Path]::GetFullPath((Join-Path $verificationRoot $OutputDirectory))
if (-not $verificationOutput.StartsWith(($verificationRoot + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase)) { throw '验证输出必须位于当前项目中。' }
if (Test-Path -LiteralPath (Join-Path $verificationOutput 'result.json')) { throw '验证结果已存在，请使用新的输出目录。' }
New-Item -ItemType Directory -Path $verificationOutput -Force | Out-Null
$verificationSteps = [Collections.Generic.List[object]]::new()
$verificationPassed = $false
function Invoke-DesktopCheck([string]$Name, [scriptblock]$Run) {
    Write-Output "正在检查：$Name"
    $verificationStarted = Get-Date
    & $Run 2>&1 | Tee-Object -FilePath (Join-Path $verificationOutput "$Name.log")
    $verificationExit = $LASTEXITCODE
    $verificationSteps.Add([ordered]@{ Name=$Name; ExitCode=$verificationExit; Seconds=((Get-Date)-$verificationStarted).TotalSeconds })
    if ($verificationExit -ne 0) { throw "$Name 失败，退出码 $verificationExit。" }
}
Push-Location $verificationRoot
try {
    $verificationCargoArgs = @('--locked')
    if ($Offline) { $verificationCargoArgs += '--offline' }
    Invoke-DesktopCheck 'frontend-test' { pnpm --dir desktop test }
    Invoke-DesktopCheck 'frontend-build' { pnpm --dir desktop build }
    Invoke-DesktopCheck 'agent-plugin-test' { node --test integrations/agent-tools/tests/*.test.mjs }
    Invoke-DesktopCheck 'rust-test' { cargo test -p githubsp-core -p githubsp-desktop @verificationCargoArgs }
    Invoke-DesktopCheck 'rust-clippy' { cargo clippy -p githubsp-desktop --all-targets @verificationCargoArgs -- -D warnings }
    Invoke-DesktopCheck 'rust-format' { cargo fmt --all -- --check }
    if ($Package) {
        Invoke-DesktopCheck 'package' { & (Join-Path $PSScriptRoot 'package-desktop-windows.ps1') -Offline:$Offline -OutputDirectory (Join-Path $OutputDirectory 'package') }
    }
    $verificationPassed = $true
} finally {
    [ordered]@{
        Passed=$verificationPassed
        Version=(Get-Content -LiteralPath (Join-Path $verificationRoot 'desktop/package.json') -Raw | ConvertFrom-Json).version
        Steps=@($verificationSteps)
        RealNetwork=$false
        CleanSystemInstall=$false
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $verificationOutput 'result.json') -Encoding utf8
    Pop-Location
}
Write-Output "桌面验证通过，证据：$verificationOutput"
