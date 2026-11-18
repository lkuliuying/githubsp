# 本机 Agent 接入

## 仓库与版本边界

DSH 插件已拆分至 [lkuliuying/dsh-githubsp](https://github.com/lkuliuying/dsh-githubsp)，后续插件源码、测试、设置、MCP、Skill 和打包在该仓库维护，当前版本为 v0.7.2。本仓库继续维护桌面 v0.7.0 的 Tauri 界面、下载核心和本机 Agent 服务；两端版本号独立迭代。

本文件保留桌面接口约定和拆分前联调记录，`integrations/agent-tools` 及旧插件打包脚本仅保留拆分快照。插件开发与当前兼容限制以[插件仓库 README](https://github.com/lkuliuying/dsh-githubsp) 为准。本次仅推送插件仓库，桌面 v0.7.0 仍未推送或发布。

## 原生插件 v0.7.2 的设置入口

v0.7.2 使用与 DSH-better-sidebar 相同的 `settings.section` 插槽，直接在 Harness「设置 → GitHubSP」显示数据目录，使用宿主输入框和按钮；旧 `plugins.bundle.config` 设置入口已移除。7 项相关测试通过，覆盖原生设置分类登记和卸载清理。本机已从线上提交 `56d3d09579fe425977c9c6f5b39fb1ef9318bad9` 的官方源码归档升级至此版本，保留数据目录与其他插件。窗口点击与捕获状态不一致，真实设置页点击验收尚未完成；重启 Harness 后可加载新客户端模块。

### v0.7.1 的历史接入记录

当前安装的 Harness 为 0.2.2-rc.1；插件 v0.7.1 明确声明并在此版本验证，早期 v0.7.0/0.2.1-rc.2 的证据仍按下文的阶段记录解释。桌面程序维持 v0.7.0。

`integrations/agent-tools/package.json` 增加浏览器模块、图标、中文/英文显示元数据及可访问的包清单导出。`harness-client.js` 通过宿主的 `plugins.bundle.config` 提供包详情中的数据目录设置，用 ConfigForms 保存；写入被拒绝、无修改权限、读取不可用和并行修改均有相应状态，不将 false 回执当作成功。`harness.mjs` 用官方 Schemastery 3.18.5 声明 volatile 数据目录；每次工具调用读取当前引用，空值继续读取环境变量。未配置的包仍可加载设置页，调用工具时才报告缺少配置。

此前 `desktop` profile 从 [dsh-githubsp 仓库提交 `1b87e48`](https://github.com/lkuliuying/dsh-githubsp/commit/1b87e485661d2a7d634c053e4d11ce2fd90336ef) 的 GitHub 官方源码归档安装 v0.7.1，替换旧的本地目录链接，并正式登记 `dsh.profile.bundles`；现已升级至上方 v0.7.2。唯一 `githubsp` 设置条目继续使用原实际数据目录。首次替换前备份到 `.dsh/plugin-backups/githubsp-github-20261108-200839/`；当时的 13 个依赖、数据目录设置和插件管理器的 9 个本地文件均核对保留。旧 `.dsh/plugins/dsh-githubsp/0.7.1` 目录保留，不作为安装来源。

`artifacts/verification/roadmap-20261108/plugin-settings/` 保留本轮后台证据。6 项相关测试通过，覆盖设置校验、未配置仍可加载、实时目录引用切换及原有传输/MCP 契约；已通过实际 0.2.2 原生执行器与 GitHubSP 的只读连接及包配置合并检查。当前执行器已改为独立插件和批量调用，`scripts/lib/harness-host.cjs` 按明确宿主版本适配，仍执行严格版本校验。

用户正在使用 Harness 并明确要求暂不操作界面，因此没有重启当前宿主，也没有完成真实列表、开关和设置保存验收。这些 UI 项仍是待验收状态；之后在用户方便时重新加载 Harness，再打开「插件 → GitHubSP」检查。既有 dsh-codex-chain 0.1.2 的宿主版本不匹配提示属于其他插件，未改动或授予版本豁免。

## 共用架构与阶段记录

GitHubSP 桌面进程是队列与 SQLite 的唯一执行者。DeepSeek Harness 原生 Cordis 插件先接入其受限命名管道，后续 MCP 适配器复用同一接口；不把下载算法放进插件，不依赖模型处理状态。

接入默认关闭。在「设置 → Agent 接入」选择已有绝对下载目录并启用，重启后创建管道。关闭设置立即拒绝新请求。管道名称由规范化数据目录的 SHA-256 确定，Windows DACL 仅授予当前用户访问，并拒绝远程连接。每连接一次 JSON 行请求，最多 1 MiB、16 个并行连接；响应上限 2 MiB。解析缓存最多 128 项，十分钟失效，服务重启后应重新解析。

文件创建及暂停/继续/取消均检查规范化后的允许目录。插件不启动隐藏服务、不执行附件、不删除成品文件。关注自动下载优先使用项目专用目录，否则使用桌面最近保存目录；可能自动入队的关注操作会先检查有效目录是否允许。项目显式“仅提醒”可覆盖全局自动下载，旧规则缺少覆盖字段时继续沿用原有逻辑。

`CreateOptions.request_key` 和任务指纹由 Manager 串行处理，请求键映射与任务记录在同一 SQLite 事务写入。相同键与相同固定下载参数复用任务；不同参数拒绝；删除任务后保留键映射，避免重试意外重新下载。任务详情保留本地 SHA-256 和官方摘要，旧数据缺少字段时默认未知。普通应用备份包含这些信息；不把普通备份当作可公开的关注清单。

工具连接中断、调用取消或插件卸载不取消已接受下载。超时后按原请求键查询，不能假定失败并换键重试。桌面退出沿用保存/暂停流程，重启后按任务 ID 继续。

原生插件安装与工具契约见 [插件说明](../integrations/agent-tools/README.md)。当前已验证宿主是安装版 DeepSeek Harness `0.2.1-rc.2`，通过其真实 Cordis 与 ToolRuntime 加载文件插件、装配工具、执行请求并卸载。验收脚本为 `scripts/verify-harness.cjs`，使用隔离桌面数据、独立宿主上下文和公开测试附件，不修改正式 Harness profile。

`artifacts/verification/roadmap-20261108/m2a-native-03/result.json` 记录 20 项通过，包括两个宿主并发创建、桌面创建任务互见、越界目录拒绝、暂停、重启恢复、关注偏好复用、官方摘要及本地哈希一致、插件重载无重复注册、卸载后继续下载、关闭接入立即拒绝。附件为项目公开 v0.3.1 安装包，仅下载未执行。验收构建暂用 v0.4.0 元数据，随后候选源码升级为 v0.5.0-alpha.1。

`m2a-native-01` 暴露短仓库名未转换为 URL，已修复；`02` 暴露测试脚本把正常退出的 IPC 断开判作失败，已改为确认进程实际退出。失败记录保留。当时未验收包安装，M4 后已用真实 `dsh plugin add` 在独立 `DSH_HOME` 安装 TGZ 并加载。尚未调用 Harness 真实模型，不把运行时工具验收等同于自然语言工作流质量、正式用户 profile 的界面操作或全部版本兼容性验证。

## MCP 阶段

`integrations/agent-tools/mcp.mjs` 使用官方 SDK 1.32.2 的 stdio 传输与 JSON Schema 校验，复用原生插件的参数定义和本机客户端。stdout 只输出协议消息。业务错误返回 `isError`，成功同时返回文本与 `structuredContent`。MCP 关闭时不关闭桌面服务。

`m2a-checks/result.json` 记录候选 v0.5.0-alpha.1 的阶段回归通过：133 项核心、9 项宿主、5 项前端及 2 项客户端测试，前端构建、Clippy、格式检查通过。加入 MCP 后另有官方 SDK stdio 工具发现、参数拒绝和业务错误测试。

M2B 实际宿主证据：

- `m2b-codex-01/result.json`：实际 Codex CLI 0.163.0 的 app-server 在隔离 `CODEX_HOME` 和临时会话中完成工具发现、目录边界、幂等、真实下载、暂停/桌面重启/续传、关注规则持久化及官方哈希核对。调用其原生 MCP 控制接口，没有模型推理。
- `m2b-claude-03/claude-host.json` 和 `result.json`：实际 Claude Code 2.1.319 的模型回合通过显式临时 MCP 配置调用工具，完成同键复用、暂停、查询、继续、等待、关注规则保存及下载官方验证；Codex 同时连接并查到 Claude 创建的任务。禁用 Shell/文件工具，未保存 Claude 会话，没有修改正式 MCP 配置。
- `m2b-hosts-02` 的一次额外运行遇到 GitHub API HTTP 504；失败保留，没有把它计入通过。

这些检查证明具体版本和本机配置下的工具路径，不代表所有模型或自然语言任务都能稳定选择工具。原生插件与 MCP 的长期使用对比尚无统计样本，暂不宣称比 Shell/CLI 更快。当前可直接核对的收益是共享任务 ID、请求键、关注规则和重启进度；Shell/CLI 下载仍可用于一次性获取文件。

## 更新、软件库与迁移

M3 的 `core/src/update_center.rs` 维护持久化发布事件和带 epoch 的游标，读列表不确认已读。仓库与标签去重，并将 Atom/API 的身份合并；元数据后续补全不生成重复事件。完整说明按详情读取，已看到、跳过版本和下载关联分别记录。新增关注建立当前基线，不回灌全部历史版本。

M4 的 `core/src/software.rs` 从完成任务关联本地文件，仅检查存在性及大小，不每次重新计算哈希。下载时的验证状态与当前文件状态分开返回，不宣称已安装；最近发现标签不同也不等于可比较的版本升级。

`core/src/watchlist.rs` 的 schema 1 清单最多 200 个项目、1 MiB，不含本机目录、凭据、历史或任务。预览要求新的本机根目录，展示新增/重复/冲突及 keep/replace 行为；指纹绑定输入、根目录、策略和当前规则。确认时再次核对，规则已变化则要求重新预览。整批规则与收藏在一个 SQLite 事务写入；根目录绑定只保存在本机，不进入下次导出。

新增能力先通过 `m3-native-01`、`m4-native-01` 的实际 Harness/桌面检查，再进入共享 MCP 工具定义。M3 的 `m3-mcp-02` 包含三端共享队列、显式确认和重启；M4 的最终分发检查、安装与模型边界见 [v0.7.0](milestones/local-v0.7.0.md)。插件安装、升级、目录配置及 13 个工具见[包内说明](../integrations/agent-tools/README.md)。
