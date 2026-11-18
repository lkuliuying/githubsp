# GitHubSP Agent 工具包

> 此目录仅保留插件 v0.7.1 拆分时快照。后续开发、测试、打包及问题反馈统一在 [lkuliuying/dsh-githubsp](https://github.com/lkuliuying/dsh-githubsp) 进行。桌面版在 [lkuliuying/githubsp](https://github.com/lkuliuying/githubsp) 维护，当前本地源码为 v0.7.0；两端独立迭代。

原生插件 v0.7.1 增加 Harness 插件列表中的 GitHubSP 卡片及可视数据目录设置，当前验证宿主为 0.2.2-rc.1。正式登记包后，在「插件 → GitHubSP」可启用/停用并修改数据目录；点击“保存设置”后，后续调用读取新目录，离开页面不会自动保存。桌面程序仍为 v0.7.0，不需要为设置页重新安装桌面。

Windows 本机工具包，先实现 DeepSeek Harness 原生 Cordis 插件并通过真实运行时验收，再开放 MCP。两者共享正在运行的 GitHubSP 桌面/托盘进程、队列和 SQLite，不在插件内启动下载引擎。0.7.1 已核验 Harness 0.2.2-rc.1 的实际工具执行、配置合并和设置声明；6 项客户端/Schema/MCP 测试通过。本轮按用户要求没有操作当前 Harness 界面，卡片、开关及保存的实际点击验收仍未完成。此前 0.7.0 的 Codex CLI 0.163.0、Claude Code 2.1.319 证据保留，不据此承诺其他版本兼容。

## 准备与配置

1. 在 GitHubSP「设置 → Agent 接入」填写已有的绝对下载目录，启用并重启。保持应用运行，可最小化至托盘。
2. 从「设置 → 数据与备份」确认实际数据目录。它与下载目录不同。
3. 原生插件使用 Cordis 配置 `dataDirectory`；空值时读取环境变量 `GITHUBSP_DATA_DIR`。MCP 使用 `--data-dir`，省略时读取同一环境变量。不同宿主应指向同一实际数据目录。

允许目录只能在桌面中调整；插件不能放宽权限。停用接入立即拒绝新请求。应用未运行时返回明确错误，不静默启动服务。

## Harness 原生插件优先

分发的 `dsh-githubsp-<版本>.tgz` 是原生插件。使用 Harness 自带 CLI 安装，例：

```powershell
& 'C:/ProgramSoftware/Deepseek/resources/runtime/cli/bin/dsh.cmd' plugin --profile <配置名称> add 'F:/Downloads/dsh-githubsp-0.7.1.tgz' --ignore-scripts
```

替换 CLI、配置名称及包路径。升级时先保留当前配置，退出使用该插件的会话，用同一 profile 添加新版本 TGZ 后重启并检查 `githubsp_status`。本轮通过独立 `DSH_HOME` 验证安装，没有改动正式 profile。

插件自带 `cordis.patch.yml`。加载条目的配置如下；优先配置 `dataDirectory`，也可在启动 Harness 前设置环境变量：

```yaml
- insert:
    - id: githubsp
      name: dsh-githubsp
      config:
        dataDirectory: F:/GitHubSP/data
```

开发时可把 `name` 改为本机 `harness.mjs` 的 `file:///` URL。原生入口使用 `ctx.tools.register` 和带生命周期的工作流提示，不通过 MCP 转接。卸载/重载释放工具注册和提示，已接受任务继续由 GitHubSP 维护。

## 共用的 13 个工具

| 工具 | 用途 |
| --- | --- |
| `githubsp_status` | 版本、连接与允许目录 |
| `githubsp_resolve_release` | 固定实际版本、附件身份及匹配理由 |
| `githubsp_create_download` | 用稳定 `requestKey` 创建共享任务 |
| `githubsp_tasks` / `githubsp_get_task` | 分页查历史或按 ID/请求键读取回执 |
| `githubsp_wait_task` / `githubsp_control_task` | 有界等待，暂停/继续/取消 |
| `githubsp_watch` | 本地关注、完整规则替换、检查和显式已读 |
| `githubsp_updates` / `githubsp_update` | 持久游标增量列表、详情、seen/skip |
| `githubsp_project_labels` | 项目分组与标签 |
| `githubsp_software` | 查询已下载文件及当前文件状态 |
| `githubsp_watchlist` | 导出、预览和确认导入可迁移关注清单 |

先查软件库，合适且 `fileState=present` 的文件可复用。缺失、不可访问和大小变化不能称为可用。`verificationAtDownload` 是下载时记录；列表不重新计算哈希。最新关注标签不同仅表示差异，不推测版本大小，不称为已安装。

下载先解析再创建。多个候选应让用户选择，不自动猜测。相同请求键与固定下载参数复用同一任务；超时/断连后先按原键查询，不换键盲目重建。解析缓存十分钟有效，服务重启后重新解析；已接受任务及请求键保留。工具不执行附件或删除成品文件。

`watch` 是本地关注，不修改远程 GitHub Star/Watch。`rule` 替换完整规则，省略的布尔选项关闭；显式仅提醒可覆盖全局自动下载。自动下载需用户要求；优先使用项目 `downloadDirectory`，否则用桌面最近目录，两者均须在允许范围内。省略/设为 null 的目录恢复默认。

更新读取不会标记已看到。保留返回游标，失败可重读；只有用户要求处理时调用 seen/skip。关注清单不包含凭据、机器路径、任务或历史；导入必须重新选择已有允许根目录、预览 keep/replace 冲突，确认后传回同一 `fingerprint`。预览后规则改变会拒绝导入并要求重新预览。

回执的 `official_match` 表示官方摘要匹配，`local_hash_only` 仅有本地哈希；`no_available_digest`、`failed`、`pending` 保持各自含义。关闭连接不取消已接受下载；退出 GitHubSP 后不继续下载或轮询，重启后主动查询/继续。

## 后续 MCP 接入

`GitHubSP-AgentTools-v<版本>.zip` 包含锁定的 SDK 运行依赖，可解压后用 Node.js 22.19+ 或 24 运行；无需安装 Harness。源码/TGZ 开发模式先运行 `pnpm install --frozen-lockfile --ignore-scripts`。仅 MCP/插件脚本需要 Node，桌面用户不需要。

```powershell
node 'F:/Tools/GitHubSP-AgentTools-v0.7.1/mcp.mjs' --data-dir 'F:/GitHubSP/data'
```

由宿主启动该 stdio 命令，不在交互终端内发送任意文本。stdout 只输出 MCP 协议。MCP 与原生插件调用同一受限本机接口，不需要 Harness 进程或模型。

Claude Code 临时 MCP 配置（替换为本机绝对路径）：

```json
{"mcpServers":{"githubsp":{"command":"node","args":["F:/Tools/GitHubSP-AgentTools-v0.7.1/mcp.mjs","--data-dir","F:/GitHubSP/data"]}}}
```

通过 `claude --mcp-config <配置文件> --strict-mcp-config` 使用。Codex 对应配置：

```toml
[mcp_servers.githubsp]
command = "node"
args = ["F:/Tools/GitHubSP-AgentTools-v0.7.1/mcp.mjs", "--data-dir", "F:/GitHubSP/data"]
```

这些配置不需要账号或凭据。升级时退出宿主，将新 ZIP 解压到新目录，修改路径并重启；数据仍归 GitHubSP。卸载时移除宿主配置及工具包，不删除任务、关注或成品文件。

## Skill 与 Claude 插件包装

工具包中的 `skills/githubsp-release-workflows/SKILL.md` 包含获取工具链、恢复、复用软件文件、检查更新及生成带来源简报的流程。`GitHubSP-Skill-v<版本>.zip` 可解压至支持 AgentSkills 的宿主技能目录；Skill 不代替工具连接。Codex 需另外配置上述 MCP。Harness 原生入口已包含共用流程引导，无需再经 MCP 加载此插件。

独立 AgentTools ZIP 也包含 Claude 插件清单和 `.mcp.json`。启动前在当前进程设置 `GITHUBSP_DATA_DIR`，然后用本地路径加载：

```powershell
$env:GITHUBSP_DATA_DIR = 'F:/GitHubSP/data'
claude --plugin-dir 'F:/Tools/GitHubSP-AgentTools-v0.7.1'
```

在新会话中检查 `/mcp` 与 `githubsp:githubsp-release-workflows`。此方式自动连接插件自带 MCP，不必重复手工配置同一个服务。勿用 `--bare` 验证插件的自动 MCP 加载，因为该模式跳过服务发现。正常加载与工具发现已核验；额外真实模型 Skill 工作流因当前模型服务超时未通过，不能据此承诺模型选工具质量。

构建、实际宿主证据及限制在源码仓库的 `docs/agent-integration.md` 和 `docs/milestones/local-v0.7.0.md` 中。接入格式核对自 [Harness 原生工具教程](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/cordis-tutorial/07-into-the-harness.zh.md)、[MCP SDK](https://ts.sdk.modelcontextprotocol.io/server)、[Claude MCP](https://code.claude.com/docs/en/mcp) 和 [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)。本地包尚未发布到插件市场或包注册表。
