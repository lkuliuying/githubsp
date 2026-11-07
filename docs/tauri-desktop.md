# Tauri + React 桌面入口

2026-11-07 确定使用 Tauri 2 + React + TypeScript + Rust。默认 Cargo 包为 `githubsp-desktop`；`native-desktop/` 的 Slint 实现作为历史入口保留，历史原生脚本仍明确针对 `githubsp-native`，不能把其截图或测试结果当作新界面的验收。

## 结构与接口

- `desktop/src/`：React 工作台，含下载队列、新建下载、线路检测、历史、收藏、设置，以及独立提醒窗口。
- `desktop/src-tauri/`：Tauri 窗口、托盘、受控命令、退出保存、数据位置和资源打包。
- `core/`：沿用现有下载引擎、SQLite、历史分页、收藏检查、限流、数据迁移与更新源逻辑；本轮未重写下载或存储算法。
- `desktop/src/lib/api.ts`：唯一的前端 IPC 入口。`dispatch` 使用带 `type` 的请求，Rust 保持输入校验与文件操作；浏览器预览不访问本地数据。
- 状态通过 `workspace-state` 推送，常规更新合并为约 250 ms；前端按 `revision` 丢弃旧快照。初始请求先注册监听，窗口卸载时释放全部订阅。
- 退出保存和迁移使用写锁等待已接受的命令结束。目标目录检查及确认发生在停止服务之前；停止后失败会明确要求重启，不继续假装服务可用。

页面外壳、控件和导航动效改编自 magpie 固定提交 `75d40ba48bcc4b215de044c78ce226ed47f4f510`，另保留 cc-switch 与 Motrix 的组件改编。具体文件、改编范围与许可见 [`desktop/THIRD_PARTY_NOTICES.md`](../desktop/THIRD_PARTY_NOTICES.md)。参考源码保存在本地忽略目录 `artifacts/references/`；不参与产品构建。

界面采用 magpie 的灰白内容区、紧凑侧栏、黑灰主按钮和白色导航滑块；蓝色用于链接和下载进度，绿、橙、红表示成功、等待和失败。深浅主题均使用中性灰，不使用紫色；没有已保存偏好时默认浅色。侧栏常规宽度 212px，窄窗口为 184px，内容最大宽度 1200px。顶部仅保留模块标题、相关操作和窗口按钮；下载限速归入队列，版本信息归入设置的“关于”。配色集中在 `desktop/src/styles.css` 的语义变量中。

新建下载页先展示地址输入，读取后展示版本下拉与附件列表；保存目录、线路和预览确认固定在底部，读取失败在输入附近提供重试。队列使用约 88px 的任务行，点击任务才显示详情；1120px 及以下窗口中详情在列表区域内展开，收起后焦点返回任务。目录检查、重复项识别、下载前确认及 Rust 命令契约沿用原实现。布局与状态连续性沿用前一批参考 [Stageshow](https://github.com/benjitaylor/skills/tree/main/stageshow) 的固定操作区，并适配 magpie 的导航体验；没有新增动效依赖。

页面首次访问时挂载，之后隐藏并设置 `inert`，保留本次运行中的输入、筛选、附件选择、设置草稿和内部滚动位置。历史页仅在可见时刷新。页面可见状态用于抑制迟到的确认框和自动跳页；已确认的后台操作继续执行。打开模态框时暂缓模块跳转，关闭后处理最后一次外部跳转。导航弹簧沿用 magpie 的 response 0.42、damping 0.84，快速切换从当前位置重新定向；内容区使用 240ms 淡入，系统或应用开启减少动态时立即切换。

设置页适配 magpie 的分类标签、跨分类搜索、行式偏好列表和“关／开”分段控件，分为通用、下载、收藏更新、数据与备份、关于。分类与搜索只影响前端展示，草稿仍统一保存或放弃，不新增存储字段。切换分类或修改搜索后，迟到的更新检查和迁移目录检查不再弹出对话框；已确认的迁移命令沿用原流程。

原创图标源文件为 `desktop/src/assets/app-icon.svg` 与 `tray-icon.svg`；Tauri 使用 `desktop/src-tauri/icons/` 的 ICO/PNG，安装器和卸载器显式使用同一 ICO。旧 Slint 图标资源保留。图标为石墨色底、白色几何包裹和蓝色下载切口。

## 开发与验证

要求 Windows x64、Rust 1.96+、Node.js 以及 pnpm 10；Windows 需要可用的 WebView2 Runtime。

```powershell
pnpm --dir desktop install --frozen-lockfile --ignore-scripts
cargo fetch --locked --target x86_64-pc-windows-msvc
node desktop/scripts/notices.mjs --rust
pnpm --dir desktop desktop
```

首次运行前按实际需要用隔离目录试用：

```powershell
pnpm --dir desktop tauri dev -- -- --data-dir F:\Program\githubsp\artifacts\verification\tauri-manual-data
```

```powershell
pnpm --dir desktop test
pnpm --dir desktop build
cargo test -p githubsp-desktop -p githubsp-core --locked --offline
cargo fmt -p githubsp-desktop --check
```

核心测试包含临时目录、Windows 文件锁和本地回环服务，受限沙箱可能出现 `拒绝访问 (os error 5)`；应区分环境限制与断言失败。不能删除或跳过相关测试来换取通过。

## 两种 Windows 分发

```powershell
./scripts/package-desktop-windows.ps1 -Offline
```

`-Offline` 传递给 Cargo；它不表示安装版在缺失 WebView2 的机器上无需联网。脚本要求已有前端依赖与本机构建工具。输出目录默认 `artifacts/build/tauri/`，再次打包请使用新的 `-OutputDirectory`，避免覆盖已有免安装暂存目录。

| 文件 | 行为 |
| --- | --- |
| `GitHubSP-v<版本>-windows-x64-Setup.exe` | NSIS 当前用户安装，支持开始菜单与卸载；缺少 WebView2 时下载并安装 Evergreen Runtime。 |
| `GitHubSP-v<版本>-windows-x64-Portable.zip` | 解压完整目录后运行 `GitHubSP.exe`，无需安装 GitHubSP；使用系统已有 WebView2。 |
| `GitHubSP-v<版本>-windows-x64-Portable-Offline.zip` | 可选；通过下述参数传入 Microsoft 固定版本运行时目录后生成。 |

```powershell
./scripts/package-desktop-windows.ps1 -Offline `
  -OutputDirectory artifacts/build/tauri-with-runtime `
  -WebView2RuntimeDirectory D:\WebView2-FixedRuntime-x64
```

目录必须包含 `msedgewebview2.exe`；程序会优先使用 exe 同目录的 `webview2/`。运行时的来源、架构、更新和分发条款应按 Microsoft 官方说明核对。未提供固定运行时时，不生成或宣称已验证离线完整包。

安装版和免安装版共用应用标识及默认数据位置 `%LOCALAPPDATA%\com.githubsp.desktop`，并遵循已保存的数据位置配置。单实例锁与旧版本兼容，同一数据目录不能同时打开。免安装不自动代表数据随程序移动：需要该模式时，由用户在 exe 旁创建空文件 `portable-mode.flag`；程序随后使用同目录 `data/`，不自动复制正式数据。显式 `--data-dir <绝对路径>` 优先，用于独立试用；这两种模式都禁用修改正式数据位置。

打包脚本生成两种分发文件、SHA-256、运行依赖声明及 `package.json` 清单，不执行安装器、不签名、不发布。前端编译工具只在开发和构建时需要，用户不需要 Node.js、pnpm 或 Rust。

## 验收边界

当前开发机已通过以下检查，完整记录保存在本地忽略目录 `artifacts/verification/`，不随源码提交：

| 范围 | 结果与边界 |
| --- | --- |
| 下载核心 | 125 项现有回归通过，包含本地 HTTP、存储兼容、备份、迁移与恢复夹具。 |
| Tauri 宿主 | 7 项测试通过，覆盖隔离路径、单实例、命令契约、设置持久化和退出保存；Clippy 与格式检查通过。 |
| React 前端 | 4 项现有契约测试与生产构建通过。 |
| 六页布局与切换 | 77 项模拟 IPC 检查，包含 760×560、1280×820、1920×1080、深浅主题、状态保留、焦点隔离、快速切换、迟到回包和减少动态；保留切换录像。 |
| 设置页后续迭代 | 37 项设置检查、7 项迁移流程检查，均使用模拟 IPC；不代表真实数据迁移。 |
| 分发包 | 安装版与免安装版构建成功，校验和与 ZIP 内程序一致，保留完整第三方许可；EXE 和安装器内嵌图标与新 ICO 一致。 |

前一批免安装包在独立数据目录实测了启动、设置读写、切页与退出；后续设置页的原生交互核查因窗口焦点和捕获不可靠而未完成。干净系统安装/卸载、缺失 WebView2、多 DPI/多显示器、任务栏/托盘的系统缓存外观、固定运行时离线包和真实网络完整下载仍待专项验收。

`--data-dir` 只隔离业务数据，不隔离 WebView2 配置。旧版并行运行时本机曾出现 WebView2 `0x8007139F`；测试进程另设 `WEBVIEW2_USER_DATA_FOLDER` 后成功。普通试用应先退出旧版，不修改或删除已有 WebView2 数据。

原有限速按 KiB 保存，设置和队列采用不同单位显示，存在数值显示差异，本轮未改变存储契约。数据目录、兼容性与恢复步骤见 [数据与存储](data-storage.md)。当前 GitHub Actions 的 Native UI 工作流只验收历史 Slint 入口；普通推送不自动发布，详见 [历史自动化说明](legacy/ui-automation.md)。
