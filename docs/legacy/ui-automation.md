# Slint 历史界面自动迭代

本流程只验证保留的 `native-desktop/` Slint 实现，不作为当前 Tauri + React 界面的验收入口。

工作流由 `invoke-ui-iteration.ps1`、`verify-native-ui.ps1` 和 `.github/workflows/native-ui.yml` 组成。自动开发与独立只读评审通过 Codex CLI 执行；测试、视觉比较、发布构建和发布条件由脚本执行。默认不提交或推送。现有开发工作区有未提交改动时，AI 入口拒绝修改；普通验收不要求工作区干净。

## 前置条件

- Windows、PowerShell 7、Rust 1.96.1（含 rustfmt、clippy）、Visual Studio C++ Build Tools。
- 使用 AI 入口时，`codex` 在 PATH 中且已完成认证；脚本不复制认证文件，也不要求把凭据写入仓库。CI 普通验收和测试版发布不调用模型、不需要模型 API key。
- `automation-mcp` 是专用测试 feature，通过 Slint 1.18.1 的 MCP 调试服务操作临时数据库和软件渲染窗口。发布命令不启用该 feature。初次需联网获取锁文件中的可选依赖，之后可使用 `-Offline`。

## 验收与基线

```powershell
./scripts/test-ui-automation.ps1
./scripts/verify-native-ui.ps1 -Offline
./scripts/test-ui-mcp.ps1 -Offline
./scripts/verify-native-ui.ps1 -Offline -Package
```

验收依次运行格式检查、workspace 测试、三档固定设置页截图、像素比较和 Clippy。`-Package` 继续运行已有打包和 EXE 隔离启动检查。失败会返回非零退出码，完整证据写入被忽略的 `artifacts/automation/`；验收前后源码指纹必须一致。

打包时许可证汇总仅在内容变化时写入，避免未变化的 `include_str!` 输入触发昂贵的重复优化构建。

初始 PNG 来自接入时的设置页，防止后续无意变化，并不表示现有设计已经得到最终认可。当前自动视觉覆盖是下载设置页的 720×520、1040×740、1448×1086，其他模块仍由现有布局和交互测试覆盖。比较阈值为单通道差值 8、变化像素比例 0.1%；差异标为红色。固定内嵌字体、软件渲染和关闭动效减少环境差异。基线缺失、截图缺失或尺寸变化均失败。

有意更改设计时，先明确目标、审阅新截图，再单独执行以下命令并审阅 PNG 差异。AI 修复任务不允许修改基线或测试，不会自行接受新结果。

```powershell
$verification = Get-Content artifacts/automation/verify/verification.json -Raw | ConvertFrom-Json
./scripts/compare-ui-snapshots.ps1 -ActualDirectory $verification.ScreenshotDirectory -UpdateBaseline
```

## 自动迭代

复制并编辑 `automation/settings.task.example.json`，明确目标、逐个文件的 `allowedPaths` 和验收标准。将任务及当前源码保存到干净检出后运行：

```powershell
./scripts/invoke-ui-iteration.ps1 `
  -TaskFile automation/settings.task.example.json -MaxRounds 3 -TimeBudgetMinutes 45
```

每轮先构建隔离 MCP 预览供开发代理检查，代理退出后关闭进程，独立运行固定验收。失败的报告和差异图反馈到下一轮；通过后重新构建预览，启动新的只读评审会话并附上基线图与实际图。审查必须通过且无遗留问题，才复用相同源码指纹的验证结果进行 EXE 构建和启动检查。每个子进程受剩余总时长限制，超时回收本次进程树。达到轮数/时长上限、模型调用失败、格式无效、范围外修改或 Git 提交变化时停止，保留报告与补丁，不回退工作。首版仅修改逐个列出的已跟踪文件，不创建新页面文件。

截图基线固定时，此入口适合在既定设计下修复缺陷。有意重设计应先完成参考图/新基线审阅，不能让修复代理同时决定验收标准。独立评审是额外证据，不能覆盖失败的确定性测试。

仅在希望本次运行**自动提交任务范围内的源码改动、推送 main，并显式触发测试版发布**时加 `-Publish`。默认不会执行这些 Git 操作；入口不会创建或切换分支。没有源码变化时不创建空提交。推送后另行调用 `workflow_dispatch` 并传入 `publish=true`，在总时间预算内等待该提交的 CI 完成，只有远程验证和发布作业成功才记录 Published=true；超时并不会取消已开始的远程作业。该参数表示已允许这次提交和推送，不能用于包含其他未保存工作的检出。

模型调用使用现有 Codex 配置，默认最多三轮且有时间上限；没有硬性的金额限额。需要金额限制时应在所用服务账户设置预算。代理子进程不接收 `GH_TOKEN` / `GITHUB_TOKEN`。不要向任务和模型输入放入密钥、私人数据或不可信指令。

## CI 与测试版发布

PR 和普通 main 推送只验证，不发布。仅在 main 手动运行 Native UI (legacy Slint) 并勾选 publish，或使用明确的 `-Publish` 入口，才会在验收、打包、MCP 截图和源码干净检查全部成功后发布 GitHub prerelease。普通手动运行只验收。

发布作业与构建作业分离，只有发布作业拥有 `contents: write`。它不执行产物中的代码，而是核对验证报告的提交 SHA 与 EXE 哈希，上传验收时的原始产物，使用唯一 `ui-<run-id>-<attempt>` 标签，不覆盖旧版本、不标记为 latest。发布后核对附件列表。正式版发布不在本流程内。

Windows Runner 必须能够创建窗口并运行现有 DX12 程序；如 GitHub 托管 Runner 无法满足，EXE 验收将失败，需将 verify job 指向有交互桌面及适用图形驱动的专用 Windows Runner，不得用 MCP 的软件截图冒充通过。输入法、多档 DPI、多显示器、托盘、干净系统和真实网络仍需专项验收；发布说明与现有 acceptance.json 保留这些边界。

参考：[Slint AI Coding Assistants](https://docs.slint.dev/latest/docs/slint/guide/tooling/ai-coding-assistants/)、[Slint MCP 架构（1.18.1）](https://github.com/slint-ui/slint/blob/v1.18.1/docs/development/mcp-server.md)、[Codex 非交互执行](https://learn.chatgpt.com/docs/non-interactive-mode)、[GitHub CLI release](https://cli.github.com/manual/gh_release_create)。
