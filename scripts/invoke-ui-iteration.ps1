#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$TaskFile,
    [ValidateRange(1, 5)][int]$MaxRounds = 3,
    [ValidateRange(5, 180)][int]$TimeBudgetMinutes = 45,
    [switch]$Offline,
    [switch]$Publish
)
. "$PSScriptRoot/automation-common.ps1"
$taskPath = Resolve-AutomationPath $TaskFile
$task = Get-Content -LiteralPath $taskPath -Raw | ConvertFrom-Json
if ($task.objective -isnot [string] -or [string]::IsNullOrWhiteSpace($task.objective) -or
    $task.acceptance -isnot [array] -or $task.allowedPaths -isnot [array] -or
    $task.acceptance.Count -eq 0 -or $task.allowedPaths.Count -eq 0) {
    throw '任务必须提供 objective、allowedPaths 和非空 acceptance。'
}
foreach ($criterion in $task.acceptance) {
    if ($criterion -isnot [string] -or [string]::IsNullOrWhiteSpace($criterion)) { throw '每条验收标准必须是非空文本。' }
}
# 自动修改只在干净检出上运行，避免代理覆盖其他会话的未提交工作。
if (@(& git -C $script:AutomationRoot status --porcelain).Count -gt 0) { throw 'AI 迭代要求干净工作区。请先自行保存现有改动；只做验收可运行 verify-native-ui.ps1。' }
Assert-AutomationScope $task.allowedPaths
foreach ($path in $task.allowedPaths) {
    & git -C $script:AutomationRoot ls-files --error-unmatch -- $path *> $null
    if ($LASTEXITCODE -ne 0) { throw "首版迭代只修改已跟踪的文件：$path" }
}
$codex = (Get-Command codex -ErrorAction Stop).Source
$commit = & git -C $script:AutomationRoot rev-parse HEAD
$branch = & git -C $script:AutomationRoot branch --show-current
if ($Publish -and $branch -ne 'main') { throw '-Publish 只支持 main；此入口不会创建或切换分支。' }
if ($Publish) { $gh = (Get-Command gh -ErrorAction Stop).Source }
$run = Resolve-AutomationPath ('artifacts/automation/iteration-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))
New-Item -ItemType Directory -Path $run -Force | Out-Null
$deadline = [DateTime]::UtcNow.AddMinutes($TimeBudgetMinutes)
function Remaining-Seconds {
    $remaining = [int]($deadline - [DateTime]::UtcNow).TotalSeconds
    if ($remaining -le 0) { throw '本次迭代已达到总时间预算。' }
    return $remaining
}
function Assert-IterationSource {
    if ((& git -C $script:AutomationRoot rev-parse HEAD) -ne $commit -or (& git -C $script:AutomationRoot branch --show-current) -ne $branch) { throw '代理改变了 Git 提交或分支，停止执行。' }
    Assert-AutomationScope $task.allowedPaths
}
$result = [ordered]@{ Passed = $false; Commit = $commit; Rounds = 0; Pushed = $false; Published = $false; Error = $null }
$feedback = '第一轮：依据任务与界面证据实施。'
try {
    for ($round = 1; $round -le $MaxRounds; $round++) {
        $result.Rounds = $round
        $directory = Join-Path $run "round-$round"
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        Write-Host "AI 迭代 $round / $MaxRounds"
        $preview = $null
        try {
            $preview = Start-AutomationPreview $directory -Offline:$Offline -TimeoutSeconds (Remaining-Seconds)
            $prompt = (Get-Content -LiteralPath (Join-Path $script:AutomationRoot 'automation/prompts/develop.md') -Raw) +
                "`n任务：`n" + ($task | ConvertTo-Json -Depth 8) + "`n上一轮反馈：`n$feedback"
            $prompt | Set-Content -LiteralPath (Join-Path $directory 'develop.prompt.md') -Encoding utf8
            $agentArgs = @('exec', '--cd', $script:AutomationRoot, '--sandbox', 'workspace-write', '--json',
                '-c', 'approval_policy="never"', '-c', "mcp_servers.githubsp.url=`"$($preview.Endpoint)`"",
                '-c', 'mcp_servers.githubsp.required=true', '--output-last-message', (Join-Path $directory 'develop.md'), '-')
            $exitCode = Invoke-AutomationProcess $codex $agentArgs (Join-Path $directory 'develop') (Remaining-Seconds) $prompt @{ GH_TOKEN = $null; GITHUB_TOKEN = $null }
            Assert-IterationSource
            if ($exitCode -ne 0) { throw "开发代理退出码：$exitCode" }
        } finally { Stop-AutomationPreview $preview }
        $verify = Join-Path $directory 'verification'
        $verifyArgs = @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'verify-native-ui.ps1'), '-OutputDirectory', $verify)
        if ($Offline) { $verifyArgs += '-Offline' }
        $code = Invoke-AutomationProcess (Get-Command pwsh).Source $verifyArgs (Join-Path $directory 'verify') (Remaining-Seconds)
        Assert-IterationSource
        if ($code -ne 0) {
            $feedback = "独立验收失败。请读取 $verify/verification.json、当前轮 verify.stderr.log 及对应检查日志、visual/ 差异图，修复根因。"
            continue
        }
        $preview = $null
        try {
            $preview = Start-AutomationPreview (Join-Path $directory 'review-preview') -Offline:$Offline -TimeoutSeconds (Remaining-Seconds)
            $source = Get-AutomationSource
            $reviewPrompt = (Get-Content -LiteralPath (Join-Path $script:AutomationRoot 'automation/prompts/review.md') -Raw) +
                "`n任务：`n" + ($task | ConvertTo-Json -Depth 8) + "`n独立验证：$verify/verification.json`n请读取 git diff 并评审。"
            $reviewFile = Join-Path $directory 'review.json'
            $reviewArgs = @('exec', '--cd', $script:AutomationRoot, '--sandbox', 'read-only', '--json',
                '-c', 'approval_policy="never"', '-c', "mcp_servers.githubsp.url=`"$($preview.Endpoint)`"",
                '-c', 'mcp_servers.githubsp.required=true', '--output-schema', (Join-Path $script:AutomationRoot 'automation/review.schema.json'), '--output-last-message', $reviewFile)
            $manifest = Get-Content -LiteralPath (Join-Path $script:AutomationRoot 'automation/ui-baseline/manifest.json') -Raw | ConvertFrom-Json
            $verified = Get-Content -LiteralPath (Join-Path $verify 'verification.json') -Raw | ConvertFrom-Json
            foreach ($image in $manifest.images) {
                $reviewArgs += @('--image', (Join-Path $script:AutomationRoot "automation/ui-baseline/$image"), '--image', (Join-Path $verified.ScreenshotDirectory $image))
            }
            $reviewArgs += '-'
            $code = Invoke-AutomationProcess $codex $reviewArgs (Join-Path $directory 'review') (Remaining-Seconds) $reviewPrompt @{ GH_TOKEN = $null; GITHUB_TOKEN = $null }
            Assert-IterationSource
            if ((Get-AutomationSource) -ne $source) { throw '只读评审期间源码发生变化。' }
            if ($code -ne 0) { throw "审查代理退出码：$code" }
            $review = Get-Content -LiteralPath $reviewFile -Raw | ConvertFrom-Json
            if ($review.passed -isnot [bool] -or $null -eq $review.issues -or $null -eq $review.summary) { throw '审查结果格式无效。' }
            if (-not $review.passed -or @($review.issues).Count -ne 0) {
                $feedback = "独立审查未通过，请读取 $reviewFile 并处理其中的有据问题。"
                continue
            }
        } finally { Stop-AutomationPreview $preview }
        # 已有验收的源码指纹必须一致，避免只读评审后重复运行无变化的测试。
        $candidate = Join-Path $run 'candidate'
        $packageArgs = @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'verify-native-ui.ps1'), '-OutputDirectory', $candidate, '-Package', '-ReuseVerification', $verify)
        if ($Offline) { $packageArgs += '-Offline' }
        $code = Invoke-AutomationProcess (Get-Command pwsh).Source $packageArgs (Join-Path $directory 'candidate') (Remaining-Seconds)
        Assert-IterationSource
        if ($code -ne 0) { throw "候选 EXE 验收失败，见 $candidate/verification.json" }
        $result.Candidate = $candidate
        $result.Passed = $true
        if ($Publish -and @(& git -C $script:AutomationRoot diff --name-only HEAD).Count -gt 0) {
            $git = (Get-Command git -ErrorAction Stop).Source
            $gitEnvironment = @{ GIT_TERMINAL_PROMPT = '0' }
            $code = Invoke-AutomationProcess $git (@('add', '--all', '--') + @($task.allowedPaths)) (Join-Path $run 'git-add') (Remaining-Seconds) -Environment $gitEnvironment
            if ($code -ne 0) { throw '暂存任务改动失败。' }
            $code = Invoke-AutomationProcess $git @('commit', '-m', 'fix(ui): apply verified UI iteration') (Join-Path $run 'git-commit') (Remaining-Seconds) -Environment $gitEnvironment
            if ($code -ne 0) { throw '提交失败，没有推送或发布。' }
            $code = Invoke-AutomationProcess $git @('push', 'origin', 'HEAD:main') (Join-Path $run 'git-push') (Remaining-Seconds) -Environment $gitEnvironment
            if ($code -ne 0) { throw '推送失败，没有确认触发远程发布。' }
            $result.Pushed = $true
            $pushedCommit = & git -C $script:AutomationRoot rev-parse HEAD
            # 普通推送只验收；只有显式 -Publish 才另行触发带发布参数的工作流。
            $code = Invoke-AutomationProcess $gh @('workflow', 'run', 'native-ui.yml', '--ref', 'main', '-f', 'publish=true') (Join-Path $run 'ci-dispatch') (Remaining-Seconds)
            if ($code -ne 0) { throw '已推送，但无法显式触发测试版发布，发布尚未确认。' }
            $runDeadline = [DateTime]::UtcNow.AddSeconds(60)
            do {
                $code = Invoke-AutomationProcess $gh @('run', 'list', '--workflow', 'native-ui.yml', '--commit', $pushedCommit, '--event', 'workflow_dispatch', '--limit', '1', '--json', 'databaseId') (Join-Path $run 'ci-list') (Remaining-Seconds)
                if ($code -ne 0) { throw '已推送，但无法读取 CI 状态；发布状态未知。' }
                $runs = @(Get-Content -LiteralPath (Join-Path $run 'ci-list.stdout.log') -Raw | ConvertFrom-Json)
                if ($runs.Count -gt 0) { break }
                Start-Sleep -Seconds 2
            } while ([DateTime]::UtcNow -lt $runDeadline)
            if ($runs.Count -eq 0) { throw '已推送，但 60 秒内未发现本提交的 CI；发布状态未知。' }
            $result.RunId = $runs[0].databaseId
            $code = Invoke-AutomationProcess $gh @('run', 'watch', "$($result.RunId)", '--exit-status') (Join-Path $run 'ci-watch') (Remaining-Seconds)
            if ($code -ne 0) { throw '远程 CI 或测试版发布失败，见 ci-watch 日志。' }
            $result.Published = $true
        }
        break
    }
    if (-not $result.Passed) { throw "达到 $MaxRounds 轮上限，未通过验收。最后反馈：$feedback" }
} catch { $result.Error = $_.Exception.Message; $result.Passed = $false; throw }
finally {
    $patchFile = Join-Path $run 'changes.patch'
    & git -C $script:AutomationRoot diff --binary $commit "--output=$patchFile"
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $run 'result.json') -Encoding utf8
    Write-Host "迭代证据：$run"
}
