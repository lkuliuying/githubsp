#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$OutputEncoding = [Text.UTF8Encoding]::new($false)
$script:AutomationRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))

function Resolve-AutomationPath([string]$Path) {
    $resolved = [IO.Path]::GetFullPath($(if ([IO.Path]::IsPathRooted($Path)) { $Path } else { Join-Path $script:AutomationRoot $Path }))
    if (-not $resolved.StartsWith($script:AutomationRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "路径必须位于当前仓库内：$Path"
    }
    return $resolved
}

function Invoke-AutomationProcess {
    param([string]$File, [string[]]$Arguments, [string]$Log, [int]$TimeoutSeconds = 600,
        [string]$InputText, [hashtable]$Environment = @{})
    New-Item -ItemType Directory -Path (Split-Path -Parent $Log) -Force | Out-Null
    $info = [Diagnostics.ProcessStartInfo]::new($File)
    $info.WorkingDirectory = $script:AutomationRoot
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.RedirectStandardInput = $true
    $info.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $info.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    foreach ($key in $Environment.Keys) {
        if ($null -eq $Environment[$key]) { [void]$info.Environment.Remove($key) }
        else { $info.Environment[$key] = $Environment[$key] }
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $started = $false
    try {
        [void]$process.Start()
        $started = $true
        $outFile = [IO.FileStream]::new("$Log.stdout.log", [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
        $errFile = [IO.FileStream]::new("$Log.stderr.log", [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
        $stdout = $process.StandardOutput.BaseStream.CopyToAsync($outFile)
        $stderr = $process.StandardError.BaseStream.CopyToAsync($errFile)
        if ($InputText) { $process.StandardInput.Write($InputText) }
        $process.StandardInput.Close()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw "执行超时（$TimeoutSeconds 秒），已停止本次进程树：$File"
        }
        return $process.ExitCode
    } finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        if ($stdout) { [void]$stdout.GetAwaiter().GetResult(); $outFile.Dispose() }
        if ($stderr) { [void]$stderr.GetAwaiter().GetResult(); $errFile.Dispose() }
        $process.Dispose()
    }
}

function Invoke-AutomationCargo([string[]]$CargoArguments, [string]$Log, [int]$TimeoutSeconds = 1200) {
    $quoted = @($CargoArguments | ForEach-Object { "'" + $_.Replace("'", "''") + "'" }) -join ','
    $entry = (Join-Path $script:AutomationRoot 'scripts/native-cargo.ps1').Replace("'", "''")
    $command = "[Console]::OutputEncoding=[Text.UTF8Encoding]::new(`$false); & '$entry' -CargoArguments @($quoted); if (`$LASTEXITCODE) { exit `$LASTEXITCODE }"
    $code = Invoke-AutomationProcess -File (Get-Command pwsh).Source -Arguments @('-NoProfile', '-Command', $command) -Log $Log -TimeoutSeconds $TimeoutSeconds
    if ($code -ne 0) { throw "Cargo 检查失败，退出码 $code，日志：$Log.stderr.log" }
}

function Get-AutomationSource {
    $paths = @(& git -C $script:AutomationRoot ls-files --cached --others --exclude-standard)
    if ($LASTEXITCODE -ne 0) { throw '无法读取 Git 文件清单。' }
    $entries = foreach ($path in ($paths | Sort-Object -Unique)) {
        $full = Join-Path $script:AutomationRoot $path
        $hash = if (Test-Path -LiteralPath $full -PathType Leaf) { (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash } else { 'DELETED' }
        "$path`t$hash"
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes(($entries -join "`n"))
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes))
}

function Assert-AutomationScope([string[]]$AllowedPaths) {
    foreach ($path in $AllowedPaths) {
        if ($path -notmatch '^native-desktop/(ui/[^*?]+\.slint|src/[^*?]+\.rs)$' -or
            $path -match '(^|/)\.\.?(/|$)|(^|/)[^/]*tests\.rs$' -or $path.Contains('\')) {
            throw "只允许显式列出的原生界面或 Rust 业务文件，测试与流水线受保护：$path"
        }
        [void](Resolve-AutomationPath $path)
    }
    $changed = @(& git -C $script:AutomationRoot -c core.quotepath=false diff --name-only --no-renames HEAD)
    if ($LASTEXITCODE -ne 0) { throw '无法核对工作区差异。' }
    $changed += @(& git -C $script:AutomationRoot -c core.quotepath=false ls-files --others --exclude-standard)
    foreach ($path in $changed) {
        if ($path -notin $AllowedPaths) { throw "代理修改了任务范围外文件，停止验收及发布：$path" }
    }
}

function Invoke-SlintMcp([string]$Endpoint, [string]$Method, [hashtable]$Params = @{}) {
    $body = @{ jsonrpc = '2.0'; id = 1; method = $Method; params = $Params } | ConvertTo-Json -Depth 25 -Compress
    $reply = Invoke-RestMethod -Uri $Endpoint -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 5 -NoProxy
    if ($reply.error -or $reply.result.isError) { throw "Slint MCP 调用失败：$Method" }
    return $reply.result
}

function Start-AutomationPreview([string]$OutputDirectory, [switch]$Offline, [int]$TimeoutSeconds = 1200) {
    $previewTimer = [Diagnostics.Stopwatch]::StartNew()
    $build = @('test', '-p', 'githubsp-native', '--features', 'automation-mcp', '--no-run', '--locked', '--message-format=json')
    if ($Offline) { $build += '--offline' }
    $buildLog = Join-Path $OutputDirectory 'mcp-build'
    Invoke-AutomationCargo $build $buildLog $TimeoutSeconds
    $binaries = @(Get-Content -LiteralPath "$buildLog.stdout.log" | ForEach-Object {
        $line = $_ | ConvertFrom-Json -ErrorAction SilentlyContinue
        if ($line.reason -eq 'compiler-artifact' -and $line.executable -and $line.profile.test) { $line.executable }
    })
    if ($binaries.Count -ne 1) { throw '无法确定唯一的 Slint MCP 测试程序。' }
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    $info = [Diagnostics.ProcessStartInfo]::new($binaries[0])
    $info.WorkingDirectory = $script:AutomationRoot
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in @('ui_tests::automation_ui_tests::automation_mcp_session', '--ignored', '--exact', '--nocapture')) { $info.ArgumentList.Add($arg) }
    $info.Environment['SLINT_MCP_PORT'] = "$port"
    $info.Environment['GITHUBSP_MCP_SESSION_SECONDS'] = '3600'
    $stopFile = Join-Path $OutputDirectory ('stop-' + [Guid]::NewGuid().ToString('N'))
    $info.Environment['GITHUBSP_MCP_STOP_FILE'] = $stopFile
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    [void]$process.Start()
    $session = [pscustomobject]@{
        Process = $process; Endpoint = "http://127.0.0.1:$port/mcp"; Directory = $OutputDirectory; StopFile = $stopFile
        Stdout = $process.StandardOutput.ReadToEndAsync(); Stderr = $process.StandardError.ReadToEndAsync()
    }
    $lastConnectionError = '未收到有效 initialize 响应'
    try {
        $readySeconds = [Math]::Min(30, $TimeoutSeconds - [int]$previewTimer.Elapsed.TotalSeconds)
        if ($readySeconds -le 0) { throw '构建 MCP 预览后已无剩余时间。' }
        $deadline = [DateTime]::UtcNow.AddSeconds($readySeconds)
        do {
            if ($process.HasExited) { throw 'Slint MCP 测试程序提前退出。' }
            try {
                $init = Invoke-SlintMcp $session.Endpoint 'initialize' @{ protocolVersion = '2024-11-05'; capabilities = @{}; clientInfo = @{ name = 'githubsp-automation'; version = '1' } }
                if ($init.serverInfo) { return $session }
            } catch { $lastConnectionError = $_.Exception.Message; if ([DateTime]::UtcNow -ge $deadline) { throw } }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "Slint MCP 未在 30 秒内就绪：$lastConnectionError"
    } catch { Stop-AutomationPreview $session; throw }
}

function Stop-AutomationPreview($Session) {
    if (-not $Session) { return }
    try {
        if (-not $Session.Process.HasExited) {
            # 先请求正常退出，让 Rust 关闭服务并释放临时数据库目录。
            try { [IO.File]::WriteAllText($Session.StopFile, 'stop') }
            finally {
                if (-not $Session.Process.WaitForExit(5000)) { $Session.Process.Kill($true); $Session.Process.WaitForExit() }
            }
        }
        [IO.File]::WriteAllText((Join-Path $Session.Directory 'mcp.stdout.log'), $Session.Stdout.GetAwaiter().GetResult())
        [IO.File]::WriteAllText((Join-Path $Session.Directory 'mcp.stderr.log'), $Session.Stderr.GetAwaiter().GetResult())
    } finally { $Session.Process.Dispose() }
}
