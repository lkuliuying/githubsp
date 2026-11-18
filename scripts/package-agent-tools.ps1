param(
    [string]$OutputDirectory = 'artifacts/build/agent-tools',
    [switch]$Bundle,
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
$agentRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$agentSource = Join-Path $agentRoot 'integrations/agent-tools'
$agentOutput = [IO.Path]::GetFullPath((Join-Path $agentRoot $OutputDirectory))
if (-not $agentOutput.StartsWith(($agentRoot + [IO.Path]::DirectorySeparatorChar), [StringComparison]::OrdinalIgnoreCase)) { throw 'Agent 分发输出必须位于项目内。' }
$agentVersion = (Get-Content (Join-Path $agentSource 'package.json') -Raw | ConvertFrom-Json).version
$agentTar = Join-Path $agentOutput "dsh-githubsp-$agentVersion.tgz"
$agentBundleName = "GitHubSP-AgentTools-v$agentVersion"
$agentBundle = Join-Path $agentOutput $agentBundleName
if ((Test-Path -LiteralPath $agentTar) -or (Test-Path -LiteralPath $agentBundle) -or (Test-Path -LiteralPath (Join-Path $agentOutput 'agent-package.json'))) { throw 'Agent 分发输出已存在，请使用新目录。' }
New-Item -ItemType Directory -Path $agentOutput -Force | Out-Null
Push-Location $agentSource
try {
    pnpm pack --pack-destination $agentOutput
    if ($LASTEXITCODE -ne 0) { throw 'Agent 工具包生成失败。' }
} finally { Pop-Location }
$agentFiles = [Collections.Generic.List[string]]::new()
$agentFiles.Add($agentTar)
if ($Bundle) {
    $agentMembers = tar -tf $agentTar
    if ($LASTEXITCODE -ne 0) { throw '无法核对 Agent 工具包内容。' }
    foreach ($agentMember in $agentMembers) {
        if (-not $agentMember.StartsWith('package/') -or ($agentMember.Split('/') -contains '..')) { throw '工具包包含越界路径。' }
    }
    New-Item -ItemType Directory -Path $agentBundle | Out-Null
    tar -xzf $agentTar -C $agentBundle --strip-components=1
    if ($LASTEXITCODE -ne 0) { throw '无法展开 Agent 工具包。' }
    $agentInstallArgs = @('--dir',$agentBundle,'install','--prod','--frozen-lockfile','--ignore-scripts','--node-linker=hoisted')
    if ($Offline) { $agentInstallArgs += '--offline' }
    & pnpm @agentInstallArgs
    if ($LASTEXITCODE -ne 0) { throw '无法组装锁定的 MCP 运行依赖。' }
    # 运行时无需 pnpm 的本机缓存索引；仅移除本次分发目录内已知的机器路径元数据。
    foreach ($agentMetadata in @('node_modules/.modules.yaml','node_modules/.pnpm-workspace-state-v1.json')) {
        $agentMetadataPath = [IO.Path]::GetFullPath((Join-Path $agentBundle $agentMetadata))
        if (-not $agentMetadataPath.StartsWith(($agentBundle + [IO.Path]::DirectorySeparatorChar),[StringComparison]::OrdinalIgnoreCase)) { throw '分发元数据路径越界。' }
        if (Test-Path -LiteralPath $agentMetadataPath) { Remove-Item -LiteralPath $agentMetadataPath }
    }
    $agentZip = Join-Path $agentOutput "$agentBundleName.zip"
    Compress-Archive -LiteralPath $agentBundle -DestinationPath $agentZip -CompressionLevel Optimal
    $agentFiles.Add($agentZip)
    $agentSkillZip = Join-Path $agentOutput "GitHubSP-Skill-v$agentVersion.zip"
    Compress-Archive -LiteralPath (Join-Path $agentSource 'skills/githubsp-release-workflows') -DestinationPath $agentSkillZip -CompressionLevel Optimal
    $agentFiles.Add($agentSkillZip)
}
$agentRecords = foreach ($agentFile in $agentFiles) { [ordered]@{ Name=[IO.Path]::GetFileName($agentFile); Bytes=(Get-Item -LiteralPath $agentFile).Length; Sha256=(Get-FileHash -LiteralPath $agentFile -Algorithm SHA256).Hash.ToLowerInvariant() } }
[ordered]@{ Version=$agentVersion; Published=$false; Runtime='Node.js 22.19+ 或 24+'; Harness=(Get-Content (Join-Path $agentSource 'package.json') -Raw | ConvertFrom-Json).engines.dsh; Files=@($agentRecords) } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $agentOutput 'agent-package.json') -Encoding utf8
Write-Output "Agent 分发已生成：$agentOutput"
