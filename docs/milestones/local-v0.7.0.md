# v0.7.0 本地交付与验收

2026-11-08。按本轮授权完成 M0–M4 开发、阶段测试和本地版本迭代；先验证 DeepSeek Harness 原生插件，再封装 MCP，每轮新增 Agent 能力继续沿用此顺序。本轮没有提交、建分支、标签、推送、远程发布或修改正式宿主配置。GitHub 上的下载链接仍指向已发布 v0.3.1。

## 交付行为

| 阶段 | 实现与结果 |
| --- | --- |
| M0 | 兼容 KiB/s 存储，设置/队列统一十进制 MB/s；Tauri 验证脚本与独立 CI；公开附件实际下载及哈希对照。本地 v0.3.2 安装版/便携包。 |
| M1 | 桌面与关注共用平台/架构/安装版偏好与匹配理由，歧义不自动选；直连/系统/无凭据 HTTP(S) 代理，重启生效。本地 v0.4.0 安装版/便携包。 |
| M2A | Harness 原生 Cordis 工具、受限同用户管道、共享单写队列、原子幂等请求键、结构化回执及生命周期。实际运行时通过后生成 v0.5.0-alpha.1 桌面包与原生 TGZ。 |
| M2B | 官方 SDK 1.32.2 stdio MCP，复用已验证操作、参数和允许目录；实际 Codex 控制接口和 Claude 模型回合完成下载/恢复/关注。 |
| M3 | 更新中心、完整来源说明、分组标签、已看到/忽略版本/下载关联、去重与持久增量游标。v0.6.0-alpha.1 阶段源码验证通过。 |
| M4 | 已下载软件文件库、存在/缺失/不可访问/大小变化、下载标签与最近关注标签差异；无机器路径关注清单、根目录重绑、冲突预览指纹及事务合并；原生工作流提示、Skill、Claude 插件与本地分发。最终统一版本 v0.7.0。 |

原生插件与 MCP 共用 13 个工具。GitHubSP 必须保持运行；Agent 接入默认关闭，下载及控制只允许已配置目录，不执行附件、不删除成品、不修改远程 Star/Watch。文件库不检测系统安装状态，验证字段是下载时的记录，列表不重新计算哈希。

项目显式“仅提醒”可覆盖全局自动下载；旧规则缺少覆盖字段时保持旧语义。自动下载优先使用项目目录，否则使用桌面最近目录。可迁移清单不包含这些机器目录，新增/覆盖项目须重新确认根目录，keep 项保留原目录。

## 本地产物与使用

最终交付目录为 `artifacts/build/roadmap-v0.7.0/`，桌面构建原始证据位于 `artifacts/verification/roadmap-20261108/final-v0.7.0/package/`。

| 文件 | 用途 |
| --- | --- |
| `GitHubSP-v0.7.0-windows-x64-Setup.exe` | Windows x64 当前用户安装版；未执行安装器。 |
| `GitHubSP-v0.7.0-windows-x64-Portable.zip` | 解压完整目录后运行 GitHubSP.exe，需已有 WebView2。 |
| `dsh-githubsp-0.7.0.tgz` | DeepSeek Harness 原生插件，真实 CLI 隔离安装验收。 |
| `GitHubSP-AgentTools-v0.7.0.zip` | 含锁定 MCP 运行依赖，也可作为本地 Claude 插件加载；需 Node.js。 |
| `GitHubSP-Skill-v0.7.0.zip` | AgentSkills 工作流说明；另外配置工具连接。 |
| `package.json` / `agent-package.json` | 文件大小与 SHA-256、运行环境及本地分发边界。 |

接入、安装与升级的完整步骤见 [Agent 包说明](../../integrations/agent-tools/README.md)，数据约束见[存储说明](../data-storage.md)。不要将试用程序同时连接正式数据目录：可用 `--data-dir <已有或新建的隔离绝对路径>`，退出旧版本后再试用正式数据。安装包没有签名或远程发布。

## 验证证据

以下路径均相对于本地忽略目录 `artifacts/verification/roadmap-20261108/`；源码保留可重复执行的验收脚本，原始证据不提交。

| 记录 | 范围 |
| --- | --- |
| `m1-native-04/result.json` | 实际 WebView2/IPC、设置和偏好持久化、真实公开安装包暂停/桌面重启/续传、官方 SHA-256。 |
| `m2a-native-03/result.json` | 实际 Harness 0.2.1-rc.2 Cordis/ToolRuntime 的 20 项检查；多上下文/桌面共享任务、同键复用、允许目录、恢复、关注及生命周期。无模型调用。 |
| `m2b-codex-01/result.json` | 实际 Codex CLI 0.163.0 app-server MCP 控制接口，隔离新会话；公开下载、重启恢复、关注与哈希。无模型调用。 |
| `m2b-claude-03/result.json` / `claude-host.json` | 实际 Claude Code 2.1.319 模型工具回合；同键复用、暂停/继续/等待/关注、官方验证；Codex 读取同一任务。只启用临时 MCP，禁用 Shell/文件工具。 |
| `m3-native-01/result.json` → `m3-mcp-02/result.json` | 先原生 16 项再 MCP 18 项；增量重读、说明详情、显式处理、重启以及三端共享队列。 |
| `m4-native-01/result.json` | 先原生 18 项；真实文件移走后显示缺失、清单无目录、预览冲突/过期指纹、重新绑定含空格根目录、规则覆盖、真实桌面导入和工作流重载。 |
| `final-v0.7.0/result.json` | 137 核心、9 宿主、5 前端、3 插件测试；前端构建、Clippy、格式检查及 Windows 分发构建。只代表本机检查，不代表远程 CI。 |
| `final-harness-install/install.log` | 真实 dsh CLI 将 v0.7.0 TGZ 安装到项目内独立 DSH_HOME/profile，不改动正式配置。 |
| `final-software-mcp/result.json` | 21 项通过；最终 v0.7.0 便携程序、真实公开文件与桌面交互、Harness/MCP 软件库及迁移结果一致、Codex 新会话发现 Skill。 |
| `final-agent-package/result.json` / `claude-plugin-discovery.json` | 9 项通过；已安装原生包、独立分发 MCP 共享任务/清单与生命周期；实际 Claude 加载器在隔离新配置中发现 Skill 并连接全部 13 个工具。无模型调用。 |
| `final-delivery-audit.json` | 5 个交付文件的大小和 SHA-256 符合清单；便携 ZIP 的 6 个文件、AgentTools ZIP 的 3657 个文件与已测试目录逐字节一致，包含点开头的插件配置及独立 SDK。 |

最终回归命令：

```powershell
./scripts/verify-desktop.ps1 -Offline -Package -OutputDirectory artifacts/verification/roadmap-20261108/final-v0.7.0
./scripts/package-agent-tools.ps1 -Bundle -Offline -OutputDirectory artifacts/build/roadmap-v0.7.0
node scripts/verify-software.cjs artifacts/verification/roadmap-20261108/final-software-mcp artifacts/build/roadmap-v0.7.0/GitHubSP-v0.7.0-windows-x64-Portable/GitHubSP.exe --mcp
```

验收输出目录需未使用；公开下载依赖 GitHub 可用性。额外分发验收使用 `scripts/verify-agent-package.cjs`，分别传入输出目录、真实安装的原生包目录、独立 AgentTools 目录、已有隔离测试数据目录及最终桌面 EXE；不使用正式用户数据。

## 未验收项与失败记录

- 没有干净 Windows/缺失 WebView2 的可用环境，未执行安装器/卸载器；多 DPI、多显示器、系统缓存外观及固定运行时离线包也未验收。便携版在已有 WebView2 的开发机运行通过，不能替代这些环境测试。
- Harness 验证使用实际已安装运行时和工具执行流水线，没有调用其真实模型，也没有操作正式 profile 的插件界面。未承诺自然语言选工具质量或所有宿主版本兼容。
- M4 的额外 Claude Skill 模型工作流未通过，历史超时记录在 `m4-mcp-workflow-02`、`m4-package-03`。独立的不加载插件最小模型请求也发生超时；随后修复了插件 `.mcp.json` 缺少 `mcpServers` 外层及验收脚本 `--bare` 跳过服务的问题。实际加载器连接与工具发现已独立验证，未把它算成模型工作流通过。
- `m3-checks` 首次格式检查的一处新断言失败已修正，独立 `format-recheck.json` 及最终完整检查证明修复。早期驱动退出资源问题、沙箱文件权限错误及 GitHub HTTP 504 均保留原失败记录，后续使用新目录复验。
- 没有替代产品性能实测或长期用户样本，未宣称比 Shell/CLI 更快。已验证的差异是跨会话共享任务、请求键、允许目录、关注/处理记录和重启进度。

私有仓库、凭据管理、独立无界面服务、自动安装、跨平台及远程 MCP 仍是有条件候选，没有随本轮扩展。长期约定同步到 README、架构、数据与接入文档；项目没有独立记忆文件，不新增记忆系统。
