# 界面设计与验收

## 2026-10-09：v0.2.6 发布验收与本地目录规范

用户已验收下节方案 1 的试用 EXE，并授权发布最新源码和 Windows x64 免安装程序。本版统一 workspace、两个本地包及主窗口、设置、更新弹窗、关于窗口的当前版本为 `0.2.6`。保留直链与确认页的紧凑双栏、状态汇总、窄窗堆叠和固定操作，不改公共接口、数据库结构或下载规则。兼容构建脚本的固定提交、旧版源码标识和第三方依赖版本保留。

### 本地验证结果

本轮证据根目录为 `artifacts/releases/v0.2.6/audit/`。开工前保存完整 Git 状态、补丁、146 个源码文件的摘要与字节副本；原有界面、兼容脚本和文档改动均保留。

| 实际命令或检查 | 观察结果 |
| --- | --- |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','--workspace','--locked','--offline')` | 默认并行、默认输出捕获：108 项核心测试、16 项桌面测试通过，无失败、过滤或忽略；文档测试 0 项。证据 `workspace-tests-initial.log`。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings')` | 通过；`clippy.log`。 |
| `cargo fmt --all -- --check`、`git diff --check` | 通过；保留现有格式约定，无仓库范围重新格式化。 |
| `.\scripts\package-native-windows.ps1 -Offline -OutputDirectory 'artifacts/releases/v0.2.6'` | Release 构建、280 个运行依赖节点的声明生成、运行依赖树和 PE 导入检查通过；`package.log`。 |
| `.\scripts\test-native-startup.ps1 -Executable '.\artifacts\releases\v0.2.6\GitHubSP-v0.2.6-windows-x64.exe'` | 主窗口、中文隔离目录、第二实例退出、原实例保留、正常保存退出通过；`startup.log`、`startup-result.json`。 |
| PE 文件头、`Get-AuthenticodeSignature`、`Get-FileHash -Algorithm SHA256` | PE32+ / Windows x64 GUI，`NotSigned`；EXE、构建输出、打包清单与启动报告摘要一致，见 `artifact-verification.json`。 |
| PowerShell 语法解析和 `cargo metadata --no-deps --locked --offline --format-version 1` | 修改脚本无语法错误；两个本地包均为 0.2.6，仅两个自身锁文件条目更新。 |

最终 EXE 为 `artifacts/releases/v0.2.6/GitHubSP-v0.2.6-windows-x64.exe`，51,068,416 字节，SHA-256：`83BF722DF9814CF8BB15A1D85457E0D0FE087672B3EBC881359AE6113ABD0F92`。脚本原始启动报告位于 `artifacts/verification/startup-20261009-044348/result.json`。

软件测试后端复核 1448×1086、1040×740、720×520，覆盖直链、混合预览、未知大小、全无效、百项列表、返回草稿、忙碌态和底部操作可达性；截图在 `audit/ui-renders/`。版本显示的四个位置均核对为 v0.2.6，更新弹窗中的可选目标版本属于测试样本，不替换为当前版本。

最终 EXE 另以 `artifacts/verification/v026-manual-ui/中文 隔离数据/` 启动，用 Computer Use 检查真实原生窗口。默认和最小客户端分别为 1040×740、720×520；另检查最大化状态，含标题栏截图为 2048×1104。公开 v0.2.5 LICENSE 的真实目录元数据与无效域名样本形成“有效 1 项／无效 1 项”，正确显示 1.1 KB、目标目录及可用空间。窄窗可滚动到线路选择，返回与创建按钮可达；返回后批量草稿与直链草稿保留。主窗口、设置、关于显示 v0.2.6。未点击创建下载；该检查不代表完整外网下载回归。原生证据为 `audit/native-ui/` 与 `manual-ui-result.json`，交互结束后正常请求关闭并确认验收进程退出；该次外部进程句柄未提供退出码，退出码验证以启动脚本为准。

此前两次 `0xc0000409` 测试进程退出在本轮默认运行中没有复现。本轮没有跳过测试、改为串行或保留临时诊断来取得通过，也不据单次通过宣称旧异常的根因已查明或修复。完整真实外网下载、原版旧 EXE 的独立 Windows 账户验收、中文输入法、多档 DPI、多显示器、干净 Windows 离线启动和同场景性能基准仍未完成。

### 发布与目录入口

本次发布流程为功能分组提交后提交版本文档，正常推送 `main` 和注释标签 `v0.2.6`；标签已存在或远端出现冲突时停止覆盖。源码 ZIP 使用 `git -c core.autocrlf=false archive` 从该标签导出，逐文件比较 Git blob。五个附件先上传草稿，回下载校验一致后再公开并设为 Latest。上传、公开下载及标签一致性的最终机器记录保存在本地发布审计目录，不将这些本地产物收入源码包。

后续普通构建默认输出到 `artifacts/build/portable/`；兼容构建默认输出到 `artifacts/build/v024-compat/<UTC 时间戳>/`；两种构建均保留自定义输出参数。测试截图、原生 fixture、启动及测量记录默认写入 `artifacts/verification/`。源码仍按 `core/`、`native-desktop/`、`scripts/` 维护。

仅在公开发布和回下载验证后清理已批准的三处编译缓存：`native-desktop/target/`、`artifacts/v024-compat/20261009-compat1/source/native-desktop/target/`、`artifacts/db-compatibility-verification/target/`。每次删除前核对绝对路径、重解析点及占用，不结束用户进程。其余 31 个历史批次整体归档，归档前已记录 1,627 个保留文件的 SHA-256，并另行保护 253 个历史发布文件；执行后核对文件数量、大小和摘要。

正式发布及审计保留在 `artifacts/releases/`；独立试用包归入 `artifacts/trials/`；其余历史验证、兼容、旧缓存资料及散落旧文件依次归入 `artifacts/archive/verification/`、`compatibility/`、`legacy-cache/`、`legacy-files/`。旧记录原文中的历史路径不改写，按本地 `artifacts/INDEX.md` 和 `artifacts/path-map.json` 定位；原始报告、源码快照和测试数据库保持字节一致。实际释放字节数及归档后校验结果记录在 `audit/cleanup-result.json`。

### 项目记忆同步

读取并更新 README 与本文，同步当前版本、当前界面、构建默认目录、验证入口、发布约定和历史路径索引；没有独立项目记忆文件，也未新建记忆体系。当前 `core/src/store/migration.rs` 的 `VERSION=2`、`REVISION=1` 与 README 和打包清单一致；外部旧记录的 SQLite v3“当前版本”说法不适用于现行源码。历史验收章节保留发生时的事实，不将早期限制改写为已完成。

---

## 2026-10-09：方案 1 的免安装 EXE 试用包

用户要求打包后自行试用和反馈。本轮沿用现有脚本，将当前 v0.2.5 工作区（包含下节直链与确认页的紧凑双栏调整）构建为 Windows x64 Release 免安装程序，输出到新的 `artifacts/download-confirm-trial-20261009/`。没有改动业务代码、版本号、依赖、打包规则或旧包，也没有提交、推送或发布。

- EXE：`artifacts/download-confirm-trial-20261009/GitHubSP-v0.2.5-windows-x64.exe`，51,068,416 字节，PE32+ / Windows x64 GUI，签名状态 `NotSigned`。
- SHA-256：`B7BDF8C51D6965BE503E50020B878D174D620CB3F35DBAFF2C33640EE3E2AA5D`。最终 EXE、Release 构建输出、`SHA256SUMS.txt`、`acceptance.json` 和启动验收记录一致。
- 包内附有 `试用说明.txt`，说明退出旧版托盘实例、普通启动与 `--data-dir` 隔离试用方式，以及直链页、批量确认页和小窗口三个反馈重点。双击启动仍沿用现有默认数据目录，移动 EXE 不会移动任务数据。

| 实际命令或检查 | 结果 |
| --- | --- |
| `.\scripts\package-native-windows.ps1 -Offline -OutputDirectory 'artifacts/download-confirm-trial-20261009'` | 通过；Release 编译、280 个运行依赖节点的声明生成、运行依赖树与 PE 导入检查通过；`audit/package.log`。 |
| `.\scripts\test-native-startup.ps1 -Executable '.\artifacts\download-confirm-trial-20261009\GitHubSP-v0.2.5-windows-x64.exe'` | 通过；当前开发机的主窗口、中文隔离目录、第二实例退出、原实例存活和正常保存退出；`audit/startup.log`。 |
| `Get-FileHash -Algorithm SHA256`、`Get-AuthenticodeSignature` 和 PE 文件头读取 | 摘要一致，Windows x64 GUI 格式正确，未签名；`audit/artifact-verification.json`。 |
| `git diff --check` 与 146 个文件摘要复核 | 通过；除本文追加打包记录，其余 145 个文件字节未变，包含重新生成的第三方声明、既有 README 改动和兼容打包脚本；`audit/final-review.json`。 |

启动原始报告为 `artifacts/slint-retirement/startup-20261009-041615/result.json`，副本为试用包中的 `audit/startup-result.json`。该验收只使用新建的隔离数据，不读取正式用户数据库。此轮没有代码改动，因此不重复运行下节已通过的 16 项界面/原生测试；下节记录的两次测试进程偶发异常仍未确认触发原因，不能由本次打包和启动通过推断其已修复。完整真实网络下载、中文输入法、多档 DPI、多显示器及干净 Windows 仍待实机试用，PE 与依赖检查不代替这些验收。

项目记忆检查读取 README、本文和外部历史打包记录。当前源码 `core/src/store/migration.rs` 明确为 `VERSION=2`、`REVISION=1`，与现行 README 和本包清单一致；外部历史记录中 SQLite v3 的“当前”表述已经过时，本次以核实后的源码为准。没有独立项目记忆文件，也未新建记忆体系或更改外部记忆。本轮只在本文同步可复用的试用入口、摘要、验证结果及限制，已有历史章节保留。

---

## 2026-10-09：直链与批量确认采用方案 1「紧凑双栏」

本轮根据用户选定的第一张设计板，仅调整直链输入与共享任务确认视图。保留 Slint + Rust、现有配色与图标、默认 1040×740 和最小 720×520 窗口。下载核心、目录校验、队列执行、数据库、依赖和打包方式不变；没有提交、推送或发布。开工前已有的 README 改动及 `scripts/package-v024-compat.ps1` 保留。

### 参考、实现与交互

- 原始选图为 `C:/Users/likecandy/.codex/generated_images/01a11ea4-de06-7e03-97bb-809b260d1fbe/exec-5b544010-9361-4749-af99-5b6163e7b7e8.png`，按字节保存于 `artifacts/download-confirm-layout-20261009/references/selected-option-1.png`。这是一张 1024×1536 的双页面设计板，包含板外标题；对照取其中对应页面的内容比例，不把整板尺寸当成单页视口，也不声称像素完全相同。
- `DirectDownloadContent` 左侧是链接与目录表单，右侧是浅蓝下载说明；`BatchConfirmation` 左侧是附件列表，右侧集中显示预览目录、已知大小、未知大小数量、可用空间、空间提示和线路选择。正文宽度不足 660px 时两栏上下排列。
- 预览状态由 `sources.rs` 将已有 `BatchPreview` 数据分项映射到 `Sources`，文件行补充实际大小；有效、重复、无效分别使用绿色、琥珀色和红色，并保留文字状态。创建按钮显示有效数量，全无效或处理中禁止提交。重复条目保留“继续原任务”；返回修改保留原草稿和线路。
- 附件列表独立滚动，外层正文右侧留出滚动区域，短窗口仍能到达下载设置。标题、状态统计、返回和创建操作固定。仓库附件与批量链接共用同一确认组件；最多 100 项、仅创建有效任务及部分失败保留输入的语义不变。
- 截图来自真实 Slint 组件的软件测试后端，窗口为 1448×1086、1040×740、720×520，像素密度为 1。三种尺寸各有 `compact-direct-*`、`compact-confirmation-*` 和 `compact-confirmation-settings-*`；未知大小、全无效、长文本、100 项列表和末项另有截图，共 14 张本轮专项截图。文件位于上述证据目录的 `ui-renders/`。
- 实际预览映射通过临时数据库与回环目录服务验证；用于与设计板对照的目录、可用空间和个别显示文本在断言之后固定为图例。未知大小、无效原因与已有任务标识另行校验。100 项边界使用模型注入检验渲染与滚动，不代表 100 次真实下载或外网验收。

### 视觉复核与修正

将所选原图与 1040×740 直链、确认截图在同轮图像输入中对照，并检查三种窗口尺寸、窄窗滚动后的设置和列表末项。

| 核对面 | 结果 |
| --- | --- |
| 字体与层级 | 沿用 Segoe UI 与现有中文字体回退、20/24px 页面标题和 11–15px 内容层级；辅助说明不抢占主要操作。 |
| 布局与间距 | 直链表单约占双栏可用宽度的 66%；确认设置约占 34%、最大 280px；短窗口正文滚动，底部操作可达。 |
| 色彩与状态 | 沿用浅蓝画布、白色主面板、绿色主按钮；有效/重复/无效配合图标和文字，标签固定为紧凑宽度。 |
| 图标与资源 | 复用现有 Phosphor SVG，包括文件、链接、广播、列表与状态图标；没有新增生成插画、在线素材或公共主题修改。 |
| 内容与行为 | 目录来自预览的规范化路径，未知大小明确标注，空间告警保留。长错误与长目录换行，长文件名遵循现有省略规则。 |

修正了初次双栏实现未显式设置起点造成的重叠，并用区域分离断言复核。新增重复场景暴露测试种子目录未按实际创建路径规范化，现只修正 fixture，使其与真实预检一致；目录与去重的生产逻辑不改。嵌套列表导致测试工具默认中心滚动落入内层列表，现测试通过右侧正文留白发送实际滚轮事件，验证外层设置可达；没有直接改写内部滚动偏移或放松可见性断言。

已接受差异：原生窗口保留既有应用外壳和字体；参考图中的装饰性底部勾选不新增为功能开关。重复任务的“继续原任务”保留现有按钮尺寸并另起一行；大小与未知值遵循真实数据，不照抄设计板数字。窄窗无对应生成参考，按现有最小尺寸与滚动规则验证。未发现剩余 P0/P1/P2 视觉问题。

### 验证与证据范围

截图输出环境变量为 `$env:GITHUBSP_UI_RENDER_DIRECTORY = 'F:\Program\githubsp\artifacts\download-confirm-layout-20261009\ui-renders'`，仅影响测试证据输出。

| 实际命令 | 观察结果 |
| --- | --- |
| `.\scripts\native-cargo.ps1 -CargoArguments @('check','-p','githubsp-native','--locked','--offline')` | 通过。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','-p','githubsp-native','--locked','--offline','--','ui_tests::module_pages_bind_to_core_and_remain_usable_at_supported_sizes','--exact','--nocapture')` | 界面专项 1 项通过，包含新布局与既有交互；`ui-test-diagnostic.log`。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','-p','githubsp-native','--locked','--offline','--','--nocapture')` | 16 项通过；`native-tests-diagnostic.log`。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','-p','githubsp-native','--locked','--offline','--','--test-threads=1')` | 16 项通过，无过滤或忽略；`native-tests-serial.log`。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','-p','githubsp-native','--locked','--offline')` | 移除临时诊断后，最终默认模式 16 项通过；`native-tests-final-verified.log`。此前出现过异常退出，见下文。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('clippy','-p','githubsp-native','--all-targets','--locked','--offline','--','-D','warnings')` | 通过；`native-clippy.log`。 |
| `cargo fmt --all -- --check` | 通过。 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('build','-p','githubsp-native','--locked','--offline')` | 原生调试构建通过；`native-build.log`。 |
| `.\scripts\test-native-startup.ps1 -Executable '.\native-desktop\target\x86_64-pc-windows-msvc\debug\githubsp-native.exe'` | 主窗口、中文隔离目录、第二实例退出、原实例存活和正常保存退出通过；`native-startup.log`。 |
| `git diff --check` | 通过；LF/CRLF 提示不属于空白错误。 |

初次沙箱测试中的目录规范化与回环访问被环境权限阻止，在正常权限下执行相同离线测试后这些失败不再出现。布局重叠、fixture 路径与嵌套滚动断言的失败日志均保留，按上述原因修正，没有删除或跳过测试。默认并行且捕获输出的测试进程曾两次以 `0xc0000409` 退出，Windows 事件偏移经本地 PDB 映射到 `abort + 0x35`，不能仅据退出码声称发生了已证实的栈缓冲区溢出。串行与不捕获输出的完整运行通过；临时 panic 诊断的一次完整运行也通过且没有异常记录。移除临时诊断后，最终默认模式同样 16 项通过。具体触发原因尚未证实，不将该问题归为既有问题，也不以最后一次通过宣称偶发异常已修复。

原生启动记录为 `artifacts/slint-retirement/startup-20261009-035349/result.json`，对应本轮调试 exe 的 SHA-256 `14942CE1DD6A093CF2FAF1FDDC5F5C1AC84D86C47E3448261A8ED3EEDACF658F`。所有数据写入专用中文隔离目录，没有使用正式数据。此检查只证明当前开发机上的启动、单实例及退出；软件截图不代替真实外网、完整下载、输入法、DPI、多显示器或干净系统验收。没有制作新安装包或发布包。

### 范围与项目记忆

读取了 README、本文、资源说明和外部历史记忆；以 Cargo、Slint 与当前回调核实实际原生架构。没有独立项目记忆文件，也未新建记忆体系。本轮 README 仅在原有未提交内容上增补中英文各一条布局说明，本文追加选图、交互、证据与验证边界；历史章节保留其发生时的事实，不以本轮状态改写旧验收。没有发现需改正的当前架构矛盾。

开工保存了 145 个跟踪文件与 1 个既有未跟踪脚本的摘要，以及 6 个计划文件的字节基线。本轮只修改 `downloads.slint`、`state.slint`、`sources.rs`、`ui_tests.rs`、README 和本文。`downloads.slint` 从 `RouteSummary` 至文件末尾与基线一致；排队、线路、全局主题、核心和依赖保持原状。截图、日志、参考和隔离数据均位于忽略目录，范围复核结果记录在本轮证据目录。

final result: passed

本结论仅针对上述范围内的视觉与交互验收；测试进程异常的限制另行记录，不能据此声称所有运行方式稳定或真实环境完整验收通过。

---

## 2026-10-08：v0.2.4 发布基线

用户确认新建下载选项 1、线路检测选项 2 的成品没有问题，并授权分批提交、推送、发布 v0.2.4 及发布后的目录清理。本次将既有 Slint 迁移、原生工作区和界面验收成果纳入 Git；下面各节中的 v0.2.3 试用路径、摘要和“未发布”标记仍是当时的历史状态。

- 根 workspace、`githubsp-core`、`githubsp-native` 和主窗口／设置／更新／关于中的版本统一为 0.2.4。锁文件只改变两个本项目包的版本，第三方依赖版本不变。
- 初次暂存检查发现上游声明包含两处行尾空格；随后明确规范汇总文件的 LF 换行及行尾空白，修正暂存时混合换行导致的空白检查问题。`scripts/native-notices.ps1` 只对生成文本做空白规范化，许可正文经逐字比较一致，上游许可文件不改动。
- `.\scripts\native-cargo.ps1 -CargoArguments @('test','--workspace','--locked','--offline')`：95 项核心、16 项原生测试通过，文档测试 0 项。
- `.\scripts\native-cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings')` 和 `cargo fmt --all -- --check`：通过。
- 本次截图目录为 `artifacts/releases/v0.2.4/audit/ui-renders`，保留旧截图。测试仍使用隔离数据库、回环服务及软件后端；不能据此声称真实外网、DPI、多显示器、输入法或干净系统通过。
- 发布入口为 `.\scripts\package-native-windows.ps1 -Offline -OutputDirectory 'artifacts/releases/v0.2.4'`。正式附件为 exe、从发布标签导出的源码 ZIP、`LICENSE`、`THIRD_PARTY_NOTICES.txt`、`SHA256SUMS.txt`；源码归档不包含本地证据、构建缓存或数据。
- 发布构建、隔离启动、远端附件回下载和发布后的清理结果记录在本地 `artifacts/releases/v0.2.4/audit/DELIVERY.md`；本节仅列出提交前已经执行的验证，不预先将后续步骤记为通过。

README 已同步 v0.2.4 下载入口、原生构建、目录职责和保留范围。独立项目记忆文件仍不存在，外部 Vue/Tauri 记忆只作历史参考。清理限于已核实的生成文件、依赖／编译缓存和空旧目录；正式数据、历史源码归档、试用包、许可证原件及截图证据保留。

---

## 2026-10-08：新建下载选 1、线路检测选 2

本轮在下面已完成的 A／B／C 版本上，调整 `NewDownloadPanel`、`SourceBrowser`、版本与附件列表、`RoutePanel` 及必要展示字段。实际 Windows 验收同时发现公共错误提示区的既有零高度问题，补充了宽高绑定这一处最小修正。队列、设置、历史、收藏和窗口的内容布局，以及下载核心、数据库和依赖保持原状。所有既有未提交迁移成果保留，版本仍为 0.2.3。

### 视觉目标与证据

- 新建下载：选项 1「紧凑双栏」设计板的上半部分，原图按字节保存在 `artifacts/download-layout-review/references/new-download-option-1.png`。
- 线路检测：选项 2「聚焦流程」设计板的下半部分，原图按字节保存在 `artifacts/download-layout-review/references/routes-option-2.png`。
- 两张原始设计板都是 1024×1536，包含两个页面及板外标题；对照时分别取所选页面的内容区域，按窗口和内容区比例核对，不将整张设计板误作单页视口。不对生成图的系统标题栏进行重绘，也不声称逐像素相同。
- 实现来自真实 Slint 软件测试后端，客户端为 1448×1086、1040×740、720×520；像素尺寸与测试窗口尺寸一致，密度为 1，没有浏览器 CSS 视口。目录响应使用回环 HTTP fixture，线路结果明确注入 `Snapshot`，不代表实时测速。
- 对照入口：`artifacts/download-layout-review/index.html`。本轮共 100 张软件截图；其中 `selected-new-download-*` 和 `selected-routes-*` 为选择方向的主要证据，`selected-new-directory-*`、`selected-routes-info-*` 为滚动后证据，`diagnostic-input-error-*` 验证错误信息可见。上一轮截图没有覆盖。
- 将原始参考和 1040×740 实现截图在同轮图像输入中比较，并检查三种尺寸。重点区域包括版本分页、附件选择与底栏、速度与错误列、最快摘要。主要控件与文字在原尺寸清晰可读，无需另造放大截图。

### 布局及保留的行为

1. 新建下载按“模式 → 仓库地址 → 版本与附件 → 保存目录 → 固定预览操作”排列。正文不足 660px 时版本和附件上下堆叠；短窗口隐藏辅助副标题。直链和批量入口保留独立草稿，共用目录。
2. 版本分页移到列表顶部，版本说明放在列表底部；附件选中行使用浅绿色背景。底栏按现有选择状态汇总数量和大小，跨版本所选附件也计入，未知或无法合计时不伪造大小。继续使用现有十进制单位与显示精度。
3. 线路检测在宽内容区采用结果列表与右侧摘要，内容宽度不足 720px 时摘要移到列表之后。地址、检测按钮和标题固定，列表及长错误文本可以滚动。
4. 最快名称和速度直接从 `Snapshot` 映射，检测中清空旧摘要。缓存缺少检测来源时，最快摘要标记为历史结果。空白地址、下载期间检测及不自动切换线路的行为不变。
5. 复用已有云下载、广播、链接、文件等 Phosphor SVG。参考图中的 GitHub 猫图标、示例域名、橙色低速判断和数值精度不作为新增产品语义；保留实际品牌和单位规则。

### 视觉复核与修正记录

| 项目 | 结果 |
| --- | --- |
| 字体与排版 | 沿用 Segoe UI 与已有中文回退；标题 20/24px，内容使用既有 11–13px 层级，速度单独强调；长错误换行，文件名按原规则省略。 |
| 间距与布局 | 新建页保持约 36%／64% 版本附件分栏；检测侧栏约占 30%，上限 280px。默认窗口内目录和预览按钮完整可见，窄窗口正文滚动。 |
| 颜色与状态 | 沿用浅蓝背景、白色内容区、绿色操作；可用/失败继续配合文字和图标，不只用颜色传达结果。 |
| 图像与图标 | 复用嵌入的既有 SVG，无新增在线素材或生成插画；未修改公共品牌、导航或其他页面资源。 |
| 文案与内容 | 保留真实接口状态、公开附件范围、目录校验和预览流程；检测说明集中，示例数据仅存在于测试和预览图。 |

首次软件对照发现 [P2] 默认尺寸下第三个版本部分隐藏、附件选择框与文件图标垂直不齐。版本行改为 44px、底部说明 54px，附件行 50px，并让选择框使用完整行高度。重新生成的 `selected-new-download-1040x740.png` 显示三个版本、三个附件和目录均完整可见；720×520 下滚动后目录和固定按钮仍可操作。该项已解决，没有剩余 P0/P1/P2 视觉问题。

随后 Windows 实机输入无效检测地址时发现 [P2] 公共错误提示仅显示关闭按钮，错误文字不可见。该区域原先读取 `Message.preferred-height`，但组件仅声明内容计算得到的 `min-height`，造成滚动区高度为零；`main.slint` 与本轮基线比对确认问题预先存在。现改为按内容最小高度设置滚动区、明确内容宽高，超过 76px 时仍在提示区内滚动。三种尺寸新增文字可访问性、提示宽高及关闭／检测按钮可达断言，软件截图均能读到错误原因。

已接受的差异：参考是生成设计板，当前使用真实原生窗口、原有字体和功能文案；速度 `3.24 MB/s` 显示为现有规则的 `3.2 MB/s`，`128.36 KB/s` 显示为 `128 KB/s`。窄窗口没有对应生成图，按既定最小尺寸与滚动规则验收。

### 自动化验证

本轮运行测试前设置 `$env:GITHUBSP_UI_RENDER_DIRECTORY = 'F:\Program\githubsp\artifacts\download-layout-review\ui-renders'`，仅影响测试截图输出。

| 实际命令 | 结果 |
| --- | --- |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','-p','githubsp-native','--locked','--offline')` | 最终 16 项通过，包含错误提示区修正；`native-tests-final.log` |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','--workspace','--locked','--offline')` | 核心 95 项、原生 16 项通过；文档测试 0 项；`workspace-tests.log` |
| `.\scripts\native-cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings')` | 最终通过；`clippy-final.log` |
| `cargo fmt --all -- --check` | 最终通过 |

新增验证覆盖：跨版本选择数量和大小、切页保留选择、目录可达、分页失败保留原页和附件、最快结果字段、历史摘要、检测中清空旧结果、全部失败及长错误、公共输入错误实际可见；既有草稿、部分创建失败、迟到响应、设置和弹窗回归继续执行。工作区全套测试在最后提示区修正前通过，修正后重跑完整原生测试及工作区 Clippy；未修改核心源码。

初期失败已保留日志并修正：Slint 列宽引用滚动内容造成循环绑定，改用独立外层宽度；新增测试辅助函数错误选择旧队列滚动区，改为识别新建页；大小断言原先未遵循 100 MB 以上的既有取整规则，核对 `presentation::size` 后修正期望；新增场景结束时未恢复原设置页，导致后续更新弹窗测试失败，现已恢复原页面。没有删除或放宽既有业务断言。

### 打包、原生验收与范围复核

| 实际命令 | 结果 |
| --- | --- |
| `.\scripts\package-native-windows.ps1 -Offline -OutputDirectory 'artifacts/download-layout-review/package'` | 最终通过，包含错误提示区修正；`package-final.log`。运行依赖与 PE 导入检查通过。 |
| `.\scripts\test-native-startup.ps1 -Executable '.\artifacts\download-layout-review\package\GitHubSP-v0.2.3-windows-x64.exe'` | 最终通过；`startup-final.log`、`startup-result.json`。中文隔离目录、主窗口、单实例和正常保存退出均通过，未发现待核对的运行库模块。 |
| `git diff --check` | 通过；另对本轮 8 个文件执行与字节基线的 `git -c core.autocrlf=false diff --no-index --check`，均通过。 |

最终产物：`artifacts/download-layout-review/package/GitHubSP-v0.2.3-windows-x64.exe`，50,605,568 字节，版本 0.2.3。SHA-256：`702247727E24AC78FC4F14B59DBDC2B363631E1C49D6917CF0424B7FE0C115DC`，同时写在包内 `SHA256SUMS.txt`。旧 A／B／C 与 Slint 迁移试用 exe 的摘要保持原样。

最终 exe 使用专用 `artifacts/download-layout-review/native-final-data` 在当前 Windows 开发机实际运行，核对新建页的版本／附件双栏、仓库输入后启用查找、切换模块后保留仓库草稿、线路页结果与摘要、无效检测地址错误文字可见及关闭提示。没有向示例仓库发起请求，也没有进行外网测速。4 张最终窗口截图为 `native-captures/new-download-final.jpg`、`new-download-draft-final.jpg`、`routes-empty-final.jpg`、`routes-invalid-final.jpg`；客户端为默认 1040×740，截图含系统标题栏。`routes-invalid-before.jpg` 保留修正前仅显示关闭按钮的证据，其余无 `final` 后缀的截图也来自首次构建，不作为最终 exe 的证据。最终验收窗口关闭后确认对应进程已退出。

软件渲染与本机 Windows 验收分别记录。完整真实网络、DPI、多显示器、输入法和干净系统验收未执行，不能从以上结果推断通过。

读取 README、本文、资源说明和外部历史记忆；当前架构以 Cargo 与 Slint 源码核实，旧 Vue/Tauri 记录仍标为历史。未发现独立项目记忆文件，也未建立新记忆体系。本轮 README 同步两个选图方向、尺寸规则、展示字段、测试截图目录参数和新试用位置。

本轮开始保存了 138 个相关文件摘要和 7 个拟修改文件的字节基线，公共提示区修正前补存 `main.slint`，共 8 个文件基线。`downloads.slint` 中从 `ProgressCell` 到文件末尾的队列与公共导航区已与基线核对一致。证据、日志和试用包统一位于被忽略的 `artifacts/download-layout-review/`，正式数据不用于测试。

最终摘要复核仅以上 8 个文件改变，其余 130 个基线文件保持原样；Git 状态条目与开工基线一致，既有未提交改动未被覆盖。对照入口引用的 36 个静态及尺寸组合路径均存在，两张参考图与所选原始设计图字节一致。范围、旧包保留和最终启动匹配检查保存在 `final-review.json`。

final result: passed

---

## 2026-10-08：Slint A／B／C 界面改造

本节对应当前 Slint + Rust 工作区，基于用户已存在、尚未提交的原生迁移成果实施。下载采用 A 紧凑工作台，设置采用 B 宽松卡片，更新、关于和完成提醒采用 C 分区布局。版本仍为 0.2.3；不提交、推送、创建安装包或公开发布。下面的 Vue/Tauri 记录仅为历史阶段证据。

### 实现范围与边界

- 下载队列、新建下载、线路检测三个模块；设置中的下载设置、窗口与提醒、收藏检查、数据管理、软件更新与关于五个模块；更新模态层、独立关于窗口、独立完成提醒小窗。
- 保留顶部下载／历史／收藏／设置、既有云下载标识、系统窗口装饰、默认 1040×740、最小 720×520。历史和收藏只继承公共控件外观。
- 队列总数直接使用全库 `Snapshot.total_tasks`，执行中按 `TaskStatus::running()` 计算，排队与等待恢复单独统计。结束记录仍只显示最近 50 条。详情使用来源、保存位置、下载量和累计执行耗时的独立展示字段。
- 内容宽度达到 1000px 的队列显示完整表格；窄屏显示两行任务，次要字段进入展开详情。新建、设置和更新弹窗正文分别滚动，标题及底部操作固定。
- 设置中的托盘、提醒和收藏更新预览均标注为功能示意；不新增图片中的地区线路、私有仓库、搜索、安装器或虚构技术栈。图形仅使用既有 SVG 与 Slint 组件，不加载在线素材。
- 下载核心、数据库、依赖及锁文件不变。限速输入、滑块与不限速状态在现有展示适配层联动；恢复默认仍只修改草稿，保存失败保留输入。更新提交阶段拦截键盘，禁用但仍保留焦点的公共按钮也不能响应回车。
- `package-native-windows.ps1` 新增可选 `-OutputDirectory`，默认仍为 `artifacts/slint-retirement/package`。本轮使用 `artifacts/style-abc-review/package`，独立保留旧试用程序和正式数据。

### 对照与截图

对照入口：`artifacts/style-abc-review/index.html`。用户的 10 张原图按字节复制到 `references/`，没有编辑原图。81 张软件截图位于同目录下的 `ui-renders/`，来自真实 Slint 组件、软件测试后端、临时 SQLite 数据库与回环 HTTP fixture；样例不代表真实网络下载已经通过。另有 6 张最终 exe 的本机 Windows 截图位于 `native-captures/`，与软件渲染截图分别列出。

| 页面 | 对照截图前缀 | 覆盖尺寸 |
| --- | --- | --- |
| 下载队列 | `downloads`、`queue-statuses`、`queue-empty`、`queue-recovery`、`queue-details` | 1448×1086、1040×740、720×520 |
| 新建下载 | `new-download-0/1/2`、`repository-populated`、`batch-preview` | 同上 |
| 线路检测 | `diagnostics` | 同上 |
| 设置五页 | `settings-0/1/2/3/4`，另有长说明与导出操作 | 同上 |
| 更新弹窗 | `update-dialog`；另有 `update-duplicate-1040x740` | 同上 |
| 关于窗口 | `about` | 三种尺寸及独立窗口的 660×520、最小 480×360 |
| 完成提醒 | `completion-380x170` | 固定 380×170 |
| 历史与收藏 | `history`、`history-pagination`、`favorites`、`favorites-cards` | 三种主窗口尺寸 |

软件截图不包含系统标题栏，不代表 DPI、输入法、多显示器和干净 Windows 验收。窄窗口的长表单按设计在正文区域滚动，截图首屏不展示全部设置；保存、恢复默认、预览与确认等固定操作均有可达性断言。

### 发现与修正

1. 手工分栏区域缺少显式起点，Slint 自动居中造成版本列表、设置示意或详情互相覆盖。补齐定位，增加宽屏和窄屏双栏分离断言；测试后端只枚举可见元素，窄屏检查先滚动获取两块区域的句柄。
2. 线路检测地址输入框分配到过多高度。固定为 42px，让空间留给结果列表。
3. 更新提交期间，已禁用且仍保留焦点的按钮可能响应回车。公共按钮再次检查禁用/等待状态，提交中的模态层吞掉键盘输入；Tab、Shift+Tab、Esc 与提交保护测试通过。
4. 原始不限速显示只比较字符串 `"0"`，会与有效输入 `"0.0"` 或空白包围的零不一致。显示状态改为从同一数值输入派生。实际 Windows 窗口还发现单向绑定在控件操作后断开，已改为与展示状态双向绑定，并补充“操作开关／滑块后再输入 0”的控件级断言；滑块填充条固定从左端开始。`settings-limit-custom` 与 `settings-limit-reset` 记录两种状态。
5. 沙箱下规范化临时目录出现 `拒绝访问。 (os error 5)`，受影响的是已有服务/路径测试与 Rust 工具；在正常权限下重跑同一离线命令通过，没有放宽路径检查或测试断言。
6. 最后新增的控件断言有一处 Rust 换行格式不符，按检查结果调整后 `cargo fmt --all -- --check` 通过。

### 自动化验收

| 实际命令 | 结果 |
| --- | --- |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','-p','githubsp-native','--locked','--offline')` | 16 项通过 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('test','--workspace','--locked','--offline')` | 核心 95 项、原生 16 项通过；文档测试 0 项 |
| `.\scripts\native-cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings')` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `.\scripts\package-native-windows.ps1 -Offline -OutputDirectory 'artifacts/style-abc-review/package'` | 通过；独立免安装产物，运行依赖及 PE 导入检查通过 |
| `.\scripts\test-native-startup.ps1 -Executable '.\artifacts\style-abc-review\package\GitHubSP-v0.2.3-windows-x64.exe'` | 通过；中文隔离目录、第二实例退出、原实例存活、正常保存退出 |
| `git diff --check` | 通过；另按修改前字节基线检查未跟踪原生文件的差异，未发现空白错误 |

UI 验证包括 64 条全库记录与 51 条列表工作集、全部任务状态统计、空列表、未知大小、长文件名、失败/恢复/换线提示、独立草稿、附件选择、预览和创建结果、过期响应、非法限速定位、保存失败保留、恢复默认需保存、固定操作与滚动可达性。关于窗口逐页拼接真实内嵌的 314 页声明并与完整文件比较，验证分页边界；完成窗口保留停留事件，8 秒、后台过滤和连续合并继续由核心通知测试覆盖。

### 打包和原生烟雾检查

- 产物：`artifacts/style-abc-review/package/GitHubSP-v0.2.3-windows-x64.exe`，50,114,048 字节，版本 0.2.3。
- SHA-256：`F2D378A62F3CAF6EB7067D86215F38DE06C794526E8932AC0B3ACE33D94D2A4D`。摘要及构建边界保存在同目录的 `SHA256SUMS.txt`、`acceptance.json`。
- 原生烟雾报告：`artifacts/style-abc-review/startup-result.json`，由既有脚本的 `artifacts/slint-retirement/startup-20261008-132712/result.json` 按字节复制。被检查程序摘要与最终 exe 一致；未检测到脚本列出的外部运行库模块。该报告只证明当前开发机的隔离启动条件。
- 最终 exe 的 Computer Use 核对使用独立 `native-visual-data/`：下载空队列、限速开关／滑块／数值交互后回到 0、设置模块切换与正文滚动、固定保存区、关于窗口打开、许可证第 1 页到第 2 页、关闭关于和主窗口。主窗口截图含系统边框为 1042×771，关于窗口为 662×551；客户端尺寸分别为 1040×740 与 660×520。试用进程已关闭，未操作正式数据库或发起真实下载。
- 旧试用 exe 摘要仍为 `D3401FFCFC7392D32F53FCA78F4B56A5308C20469BE20107405151178B066A36`。重新生成的第三方声明与修改前字节一致，核心、依赖及锁文件无本轮修改。

尚未执行的真实网络、干净系统、中文输入法、多档 DPI、多显示器、托盘／提醒焦点实机回归和资源收益均不记为通过。完成提醒、更新弹窗的当前证据为测试后端与核心测试；没有据此声称通过上述 Windows 场景。

### 项目记忆与范围复核

读取 README、本文历史记录、资源说明和外部历史记忆，当前 Cargo、源码及构建入口确认已是 Slint；旧 Vue/Tauri 记录保留历史标识。没有独立项目记忆文件，未新增记忆体系。本轮在 README 同步 A／B／C 布局、统计与展示字段、数值限速、窗口边界和可选打包目录，在本文记录当前验证证据。外部历史记忆不作当前架构依据，也未改写。

本轮修改前对 67 个相关文件保存字节基线，既有 `.gitignore` 改动、Vue/Tauri 删除项和未提交原生迁移均保留。基线、对照页、截图、日志与试用包位于被忽略的 `artifacts/style-abc-review/`，不混入源码。正式数据没有用于本轮测试。

---

## 历史阶段：2026-10-06 至 2026-10-07 Vue/Tauri

本文记录 2026-10-06 至 2026-10-07 首次公开发布前的界面重构验收。下文的 Git 状态、未配置发布源及未构建桌面程序等描述仅对应当时阶段；当前构建与发布方式以 README 为准。引用的 `artifacts/` 和 `.cache/` 文件属于维护者本地历史证据，不随源码或 Release 源码包分发。

## 范围与证据类别

依据用户提供的下载、历史、收藏、设置四张参考图重构现有 Vue 前端。保留 Tauri 原生窗口装饰、Rust 服务、真实数据与 IPC 契约。

视觉证据通过 Codex 内置浏览器和 CUA 获取。普通入口为 `http://127.0.0.1:1420/`；隔离验收入口为 `http://127.0.0.1:1420/.cache/reference-review.html`，后者挂载相同的生产组件，但仅在该文档内替换服务为内存样例。页面明确标有“界面验收样例 · 不执行真实下载”。样例未访问用户历史、真实文件、下载目标或外部服务，不参与生产入口构建。

这些证据确认前端布局和交互，不代表 Windows 原生 IPC、托盘、系统目录选择或真实网络下载已通过验收。

## 参考、尺寸和截图

证据目录：`artifacts/frontend-reference-review/`。原图均为 1448 × 1086 PNG，保留原文件内容。

| 页面 | 用户参考 | 最终实现 |
| --- | --- | --- |
| 下载 | `reference-downloads.png` | `downloads-desktop.jpg` |
| 历史 | `reference-history.png` | `history-desktop.jpg` |
| 收藏 | `reference-favorites.png` | `favorites-desktop.jpg` |
| 设置 | `reference-settings.png` | `settings-desktop.jpg` |

四张最终桌面截图使用相同的 1448 × 1086 CSS 视口；内置浏览器报告的 DPR 约为 1.03，返回的截图为 1441 × 1081 JPEG。对照按 CSS 视口及内容区域归一化，未声称原生像素完全一致。参考图包含系统窗口边框；浏览器证据不包含 Tauri 的原生标题栏。每页的参考图与实现图已在同一轮工具输出中成对显示并逐项检查。

- 1040 × 740 CSS 视口：四页 `*-1040.jpg`，截图为 1033 × 735。
- 720 × 520 CSS 视口：四页 `*-720.jpg`，截图为 713 × 516。
- 局部复查：`settings-detail.jpg` 确认不限速开关与数值一致；`favorites-detail.jpg` 展示成功、失败和待检查卡片。
- 长内容：`long-task-720.jpg`、`long-favorites-720.jpg`，覆盖长中文文件名、长仓库名及详情展开。
- DOM 尺寸：`viewport-checks.json`、`desktop-checks.json`。宽表格自身允许横向滚动，页面整体不横向溢出。
- 第一轮布局：`downloads-initial.png` 是浏览器返回的 JPEG 数据，历史扩展名保留；不作为最终截图。

下载截图包含五种任务状态、真实组件渲染的三条内置线路、已选附件和版本列表。历史使用 28 条样例数据、固定每页 20 条。收藏使用六个项目，包含检查失败与尚未检查。设置保留真实默认值和未检查的软件更新状态。截图中的数值仅是隔离验收数据。

## 五项视觉检查

| 检查面 | 结果 |
| --- | --- |
| 字体与层级 | 使用本地 Segoe UI / 微软雅黑字体栈；品牌、页标题、分区标题、指标数值和辅助说明有明确层级。表格采用较紧凑的字号；与参考图具体字体渲染仍有轻微差异。 |
| 布局与间距 | 顶部图标导航、提示横条、白色面板、下载双栏、底部队列、历史筛选与表格、收藏三列卡片、设置左右分区均对照实现。窄窗口变为堆叠布局。 |
| 颜色与效果 | 浅蓝背景、深蓝文字、绿色主按钮/进度/选中导航、轻边框与阴影统一到共享样式；错误、警告、成功有对应文字与颜色。 |
| 图标与素材 | 使用项目已有 Phosphor 图标。云下载品牌、导航、圆形分区图标、彩色仓库图标及操作图标与参考含义对应，无额外图片或字体依赖。 |
| 文案与状态 | 不显示未经支持的“已是最新”、假延迟、地区线路、今日更新计数或多任务并行承诺。保留空状态、加载、失败、文件不可访问及未通过官方摘要验证提示。 |

## 发现、修正与复查

| 级别 | 发现 | 修正与最终证据 |
| --- | --- | --- |
| P2，已解决 | 下载附件区过高，队列明显低于参考位置。 | 仓库搜索放入版本列，多行添加放入附件列，收紧队列行距。最终 `downloads-desktop.jpg` 展示完整五行队列。 |
| P2，已解决 | 不限速按钮语义为开启，样式却显示关闭。 | 共享开关样式覆盖 `[aria-checked='true']`；`settings-detail.jpg` 显示绿色开关和 0 KiB/s，交互测试确认关闭后恢复上次限速。 |
| P2，已解决 | 收藏状态标签与项目图标、标题的关系不清楚。 | 标签移到标题区域右上方，调整名称和操作文字；桌面及长名称截图无覆盖。 |
| P2，已解决 | 720 像素窗口中，历史表格的隐藏辅助文本撑大整页。 | 固定辅助文本的绝对定位，并将其约束到表格滚动容器。最终四页页面宽度均为 713，不再超过 720 CSS 视口。 |
| P3，已接受 | 系统字体、阴影和参考图存在轻微差异；实际状态说明会增加少量纵向空间。 | 保留可读的实际状态，使用正常纵向滚动；页尾和操作通过滚动、Tab 可达。该差异不妨碍操作。 |

最终下载截图已将附件列表滚回顶部，避免勾选附件引起的自动滚动影响对照。没有遗留的 P0、P1、P2 视觉阻塞项。

## 行为与工程验证

- `npm run typecheck`：通过。
- `npm run test -- --run`：3 个测试文件、24 项测试通过，包含原有 12 项和新增 12 项回归。
- `npm run build`：通过；Vite 处理 1559 个模块，生成生产静态资源。没有打包或覆盖桌面可执行文件。
- 初次受限环境中的测试/构建遇到 `spawn EPERM`，在允许的执行环境重跑后通过；未通过关闭检查或修改依赖规避问题。
- 浏览器交互：版本查找和附件预览/确认、批量暂停、任务详情与取消确认、历史搜索/重置/翻页、收藏筛选/列表切换/检查/进入附件、设置修改/保存/恢复默认、未配置软件更新源的提示均已检查。
- 边界回归：批量失败及卸载后中止、历史末页删除回退、后端稍后就绪、无效页码、跨版本附件选择、非法限速值、活动下载阻止手动检测，均由新增测试覆盖。
- 响应式：1448 × 1086、1040 × 740、720 × 520 CSS 视口；窄窗口四页均无整页横向溢出，表格之外的控件没有越出窗口。长名称、详情、键盘 Tab 和页尾操作可用。
- 浏览器控制台：验收期间未捕获 warning/error；记录见 `browser-console.json`。
- 范围核对：对 56 个基线文件校验 SHA-256；既有改动只涉及前端组件、样式与 README。Rust、类型与服务契约、原有测试、依赖清单和锁文件保持一致。该轮验收时目录没有 Git 元数据，因此以预先保存的文件基线和 `git diff --no-index` 核对范围。

## 有意保留的产品差异与限制

- 线路仍是 GitHub 直连、GH-Proxy、ghproxy.net；现有接口没有单独延迟指标。
- 历史使用已有统一关键词查询和固定 20 条分页，其余统计明确标注“本页”。
- 收藏状态是检查成功、失败、待检查；不推断用户已安装的版本。项目描述使用通用说明，不虚构仓库元数据。
- 限速仍以 KiB/s 保存，0 表示不限速；快捷滑块到 100 MiB/s，更高值可通过数值框输入。自动检查间隔仍为 6 小时。
- 软件版本仍为 0.2.0；未配置发布源时不声明已是最新；原生窗口按钮不在网页中重复绘制。
- 所有真实桌面能力继续使用原有服务，但本次未进行 Tauri 原生启动、真实下载、真实托盘与安装包验收。

## 最终结论

本次前端重构的视觉与隔离交互验收通过，范围和原生能力限制如上。

final result: passed
