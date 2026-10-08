# GitHubSP

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Slint](https://img.shields.io/badge/Slint-1.18.1-2379F4)](https://slint.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.96-orange?logo=rust)](https://www.rust-lang.org/)

[简体中文](#中文) · [English](#english) · [下载 / Downloads](https://github.com/lkuliuying/githubsp/releases/latest) · [问题反馈 / Issues](https://github.com/lkuliuying/githubsp/issues)

<a id="中文"></a>

## 简体中文

面向 Windows 的 GitHub Release 附件下载工具。v0.2.4 基于 Slint + Rust + SQLite，完成原生桌面迁移和下载、设置、弹窗的界面改造，提供项目附件选择、批量下载、托盘、限速、历史检索、收藏和版本检查。保留 v0.2.3 的线路自动恢复与低速换线建议、后台完成提醒、耗时统计和 Markdown 更新说明。应用界面为简体中文，默认串行下载；本文档提供中英双语说明。

顶部三个徽章分别说明项目 MIT 许可证及 Slint、Rust 技术栈，使用无需申请的 [Shields.io 徽章](https://shields.io/badges/static-badge)，不代表安全认证或 Windows 代码签名。

## 使用

当前源码已统一为 Slint，保留下载、历史、收藏、设置四个顶层入口。v0.2.4 发布产物位于 `artifacts/releases/v0.2.4/`，只需 `GitHubSP-v0.2.4-windows-x64.exe` 即可运行，不需要安装 GitHubSP、WebView2、Node.js 或 Python。需 Windows 10/11 x64 与兼容 DX12 的图形驱动；源码、摘要和验收记录不参与程序运行。

2026-10-08 界面采用“下载 A：紧凑工作台／设置 B：宽松卡片／弹窗 C：分区与固定操作”，随后按选图只细化新建下载（选项 1：紧凑双栏）和线路检测（选项 2：结果列表与摘要）。旧试用包 `artifacts/slint-retirement/package/` 和 `artifacts/style-abc-review/package/` 独立保留。本轮截图对照入口为 `artifacts/download-layout-review/index.html`，上一轮完整对照仍为 `artifacts/style-abc-review/index.html`；具体检查与限制见 [设计验收记录](design-qa.md)。如需与正式数据隔离，使用绝对目录参数 `--data-dir F:\Program\githubsp\artifacts\download-layout-review\trial-data` 启动；默认启动仍使用原正式数据目录。

从 [v0.2.4 Release](https://github.com/lkuliuying/githubsp/releases/tag/v0.2.4) 下载以下文件。v0.2.3 及更早的公开附件采用旧 Vue/Tauri 架构，与本版原生源码不同：

| 文件 | 用途 |
| --- | --- |
| [GitHubSP-v0.2.4-windows-x64.exe](https://github.com/lkuliuying/githubsp/releases/download/v0.2.4/GitHubSP-v0.2.4-windows-x64.exe) | Windows x64 免安装程序 |
| [GitHubSP-v0.2.4-source.zip](https://github.com/lkuliuying/githubsp/releases/download/v0.2.4/GitHubSP-v0.2.4-source.zip) | 从发布标签导出的 Slint/Rust 源码，含 Cargo.lock 和资源许可证 |
| [SHA256SUMS.txt](https://github.com/lkuliuying/githubsp/releases/download/v0.2.4/SHA256SUMS.txt) | exe、源码包和许可证文件的 SHA-256 校验值 |
| [LICENSE](https://github.com/lkuliuying/githubsp/releases/download/v0.2.4/LICENSE) | 随分发提供的 MIT 许可证 |
| [THIRD_PARTY_NOTICES.txt](https://github.com/lkuliuying/githubsp/releases/download/v0.2.4/THIRD_PARTY_NOTICES.txt) | 原生运行依赖、字体和图标的第三方声明 |

1. 下载 exe 和 `SHA256SUMS.txt`，用下方命令计算摘要，并与校验文件中对应文件名的一行比较；不一致时不要运行。
2. 若旧版正在运行，先从托盘菜单选择“保存进度并退出”，再双击新版，避免单实例机制激活旧进程。
3. 粘贴公开 GitHub 附件、仓库或版本链接，输入或选择保存目录并确认下载。手动输入的目录不存在时，提交后会询问是否新建，确认后继续原操作。完成后可打开所在目录；程序不会运行下载的文件。

```powershell
Get-FileHash .\GitHubSP-v0.2.4-windows-x64.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

原生版沿用原应用标识与正式数据目录；旧试用数据不合并。旧版与迁移前源码归档保存在维护者本地 `artifacts/`，不提交。历史公开发布文件仍位于 `artifacts/releases/v0.2.3/`。

原生版面向 Windows 10/11 x64，使用 Windows 系统组件与 DX12 图形驱动。字体、UI、SQLite、图标和许可嵌入 exe，构建启用静态 CRT；无需另装 WebView2、Node.js、Python 或 VC++ 运行库。干净系统离线启动仍需独立验收。免安装不表示数据跟随 exe 移动；数据位于用户本地应用目录。当前构建未进行代码签名。

支持的链接格式：

```text
https://github.com/{owner}/{repo}/releases/download/{tag}/{filename}
https://github.com/{owner}/{repo}/releases/latest/download/{filename}
https://github.com/{owner}/{repo}
https://github.com/{owner}/{repo}/releases
https://github.com/{owner}/{repo}/releases/latest
https://github.com/{owner}/{repo}/releases/tag/{tag}
```

支持 URL 编码的中文和空格文件名。拒绝其他域名、非 HTTPS、凭据、查询参数、片段、目录穿越及 Windows 非法文件名。仓库入口默认显示最新正式版，可以分页浏览版本并显式显示预发布。附件的 Windows、x64、安装包等提示来自文件名，用户仍需确认实际平台。

标准附件直链可直接“添加并下载”。仓库和版本链接进入附件选择器；多行附件链接或多选附件先预览，再确认创建，每批 1–100 项，失败和无效链接保留以便修改。最新附件链接会转换成固定版本地址。重复标准地址和规范化目录对应的未完成任务需要继续原任务，已完成文件仍另存。同名仓库忽略大小写进行收藏去重。

不支持私有仓库、GitHub 自动生成的源码包、Git clone、自建代理、自动安装更新；公开 Release 中作为附件上传的源码压缩包可按普通附件处理。

## 下载、历史、收藏、设置

- **下载**：快捷直链、官方版本与附件列表、批量预览、队列，以及线路最近检测结果。可在任务暂停后指定内置线路；指定线路失败不会自动换线。确实换线且存在分片时需要确认重新下载。恢复为自动模式可先复用符合恢复条件的旧线路。
- **手动线路检测**：无需先创建任务。线路检测模块的检测地址留空或只有空白时，使用本项目公开发布的 `GitHubSP-v0.2.2-windows-x64.exe`；填写附件直链则检测指定附件，仓库或版本页链接会提示改用附件直链或清空输入。每轮按顺序检测内置线路，每条最多读取 524,288 字节（约 524 KB）、等待 10 秒，不保存或执行文件。一次只允许一轮手动检测；下载、队列接续、暂停和取消可以同时进行，检测不自动切换正在下载的线路。检测流量不受下载限速控制，可能短暂影响实时速度，结果仅供参考；默认文件不可用时可填写其他附件直链重试。面板显示来源和真实文件名，逐轮替换结果；手动检测前及期间启动的下载自动检测不会覆盖本轮展示，之后新启动的下载可更新结果。旧缓存只标为历史检测结果，不推断来源。异常或超时后可再次检测，退出会取消未完成的检测。
- **目录检查**：创建和开始传输前检查目录、可写性及当前用户可用空间。预留剩余分片加完整合并文件的空间；大小未知时明确提示无法完整估算。检查不能预防后续其他程序占用空间，实际写入错误仍会停止任务。另选目录重新下载创建新任务，旧任务及分片不移动。
- **输入或选择目录**：手动输入路径后，添加任务或批量预览时检查目录；缺失时展示完整路径，用户确认后递归创建目录及缺失父目录，再继续原操作。取消保留输入，不创建目录或任务。批量最终提交再次检查；浏览仓库版本不创建目录。“浏览”只接受实际存在的目录，保留 Windows 系统选择框的新建文件夹按钮；选择的目录随后消失时要求重新选择。路径是文件、父路径不是目录、权限不足或磁盘不可访问时直接提示错误。创建后若后续检查或下载失败，保留已创建目录，不自动删除；后台传输和任务恢复不会自动补建缺失目录。
- **托盘与完成提醒**：工具栏可收起到托盘，菜单支持显示窗口、暂停下载与自动恢复、保存进度并退出。关闭窗口默认仍退出；可选择关闭时收起到托盘。完成或最终失败生成应用内提醒和托盘未读提示，同次状态变化只提醒一次。设置中的“后台下载完成提醒”默认开启：下载完成且主窗口失焦、最小化或隐藏时，显示独立的静音提醒窗，包含文件名和累计耗时；8 秒后收起，鼠标停留或键盘操作期间暂停倒计时，连续完成合并计数。查看下载会激活主窗口并切回下载页；关闭提醒只隐藏提醒。前台完成、已处理的通知及历史记录不补弹、不在重启后重放。关闭设置或主窗口重新获得焦点会收起提醒。提醒失败不改变下载结果。此功能不是 Windows 系统通知，不进入系统通知中心，也不自动继承系统勿扰设置。
- **下载耗时**：详情展示已耗时，完成后展示总耗时。Rust 使用单调时钟累计检测、传输、自动重试、合并与校验时间，不计排队、等待网络恢复、手动暂停和应用退出时间；恢复探测和实际传输照常计入。同一任务继续、重试及换线保留累计值，新建重新下载任务从零计时。运行期间每秒刷新，随状态和检查点保存，长时间没有进度时每 5 秒补存；正常停止结算最终值。旧任务缺少可靠数据时显示“未知”；异常退出后只恢复已保存数值，并持续标记“记录不完整”，不以创建或完成时间推算。
- **大小与速度**：界面按 1000 换算 B、KB、MB、GB、TB，速度使用对应的每秒单位。传输期间按最近 5 秒数据量显示速度，无新增数据时降至 0 并隐藏失效的剩余时间；完成后显示“平均速度”，以最终文件字节数除以累计执行耗时计算，包含检测、重试和收尾。缺少有效耗时或耗时记录不完整时显示“—”并说明原因；其他未传输状态不显示过期速度。
- **限速与队列**：设置使用十进制 MB/s，支持小数，0 为不限速，保存后立即生效，两路分片共享额度；限速等待可被取消、暂停和退出打断，且不算网络无数据超时。线路检测流量不受传输限速影响。等待任务可上移、下移、置顶，不抢占活动任务，也不自动启动暂停任务。顺序发生竞争时刷新后重试。
- **历史**：按文件名、仓库、版本搜索，状态筛选，每页 20 条。打开或刷新时只检查当前页成品是否存在；缺失和不可访问分别显示。可以打开目录、复制链接、重新下载，完成/取消任务可移除记录而保留成品。旧任务无可信完成时间时显示“未知”。
- **收藏**：从仓库链接或任务来源添加，最多 200 项，收藏之间独立于下载任务。初次成功检查建立正式版基线，以 Release ID 记录已提醒版本；预发布不自动提醒。删除收藏不影响文件和任务。
- **检查调度**：默认手动；可启用应用运行期间每 6 小时检查，退出即停止，无后台服务或自启。官方目录、收藏、自身更新共享串行请求、60 秒缓存和条件请求，重复操作合并到缓存结果。检查失败保留上次成功版本，尊重 `Retry-After` 和 API 限流重置时间。未认证请求仍受 GitHub 配额限制，条件请求不保证免配额。

## 前端页面与交互

Slint 保留下载、历史、收藏、设置四个顶层入口，以进程内页面状态切换。下载的三个模块采用 A 紧凑工作台，设置的五个模块采用 B 宽松卡片，更新、关于和完成提醒采用 C 分区布局；共用浅蓝背景、白色内容区、绿色主操作与既有 Phosphor 图标。历史、收藏只继承公共控件外观，不重新设计内容布局。

- **统计与详情**：队列四项统计为任务总数、执行中、排队中、等待线路恢复。总数来自全库 `Snapshot.total_tasks`，其余直接按任务状态计算；结束记录仍只显示最近 50 条。展开详情分别绑定来源、路径、下载量与累计执行耗时，宽屏信息与操作双栏，窄屏堆叠。
- **设置卡片与窗口**：窗口行为与完成提醒各用一张卡片；收藏检查的旁栏明确标注功能示意，仅展示应用内收藏的实际范围。数据管理分为实际目录、SQLite 备份、当前运行导出反馈。不限速开关由当前输入的数值派生，0、0.0 和带空白的 0 保持一致；输入、滑块和保存校验共用现有 Rust 适配层。更新仍在主窗口内显示模态层，关于和完成提醒仍是独立窗口；许可证完整保留分页与 `AboutSlint`。

- **下载工作台**：保留顶部四页导航，下载页左侧分为“下载队列／新建下载／线路检测”，默认显示队列。队列填满右侧剩余高度，列表独立滚动，标题和批量操作固定。内容区达到 1000px 时使用完整表格，文件名列伸展、操作列靠右；较窄时使用两行任务条目，线路、剩余时间和完整文件名可在详情查看。任务的更多按钮展开来源、目录、换线及队列排序。全部继续只处理暂停/失败任务，全部暂停处理活动、排队和等待网络恢复任务；批量操作遇到失败即停止并显示错误。清空已完成需要确认，仅移除记录。
- **新建与预览**：保存目录由“直链下载／仓库附件／批量链接”共享，三种输入分别保留；线路检测使用独立的检测地址。仓库附件在宽内容区双栏显示，窄内容区上下排列。预览在新建模块内进入确认步骤，返回修改保留选择；全部创建成功切回队列，部分失败保留未成功的链接或附件选择。切换模块、页面或输入后，迟到的附件与目录选择结果不再覆盖当前内容。收藏浏览直接进入仓库附件，新版下载和完成提醒的“查看下载”进入队列。模块选择和草稿只保留在当前运行期间，不新增数据库配置。
- **新建下载布局**：先输入仓库地址，再从左侧版本列表和右侧附件表选择，保存目录位于资源选择之后；正文宽度不足 660px 时上下排列。固定底栏汇总当前选择的数量和大小，跨版本选择按所选附件合计，继续沿用现有大小显示精度。版本分页位于版本列表顶部，当前版本说明位于列表底部。短窗口收起辅助副标题，正文滚动，预览和确认操作固定可达。
- **线路检测布局**：地址与检测入口固定在左侧结果区顶部，列表展示真实速度、状态和错误；内容宽度达到 720px 时，右侧独立显示最快结果和检测说明，窄窗口将摘要放在结果列表下方。最快名称与速度直接映射 `Snapshot`，重新检测时清空旧摘要；没有检测来源的最快摘要明确提示为历史结果。使用已有云下载、广播与链接图标，不引入示例域名、延迟或未经判定的“低速”标签。
- **版本分页**：加载提示固定在新建模块标题栏右侧，空闲时保留占位。请求期间保留旧版本、附件和页码，成功后更新，失败保留原页并提示错误。临时等待的控件保持原有明暗且禁止重复操作；页码边界、无附件选择等原本不可用的控件仍灰显。加载提示出现或消失不改变模块和底部操作的位置，较长内容在模块内部滚动。
- **历史**：统一关键词搜索和状态筛选、统计卡、独立滚动的记录表格与页码跳转。匹配记录数来自数据库，其余统计只代表当前页；每页仍为 20 条。删除末页最后一条记录后回到有效页。
- **收藏**：项目卡片和列表两种布局，可按检查结果筛选，按检查时间或项目名称排序。统计与标签使用真实的检查成功、失败、待检查状态，不推断“今日更新”或已安装版本。
- **设置**：左侧分为“下载设置／窗口与提醒／收藏检查／数据管理／软件更新与关于”，右侧一次显示一个模块。底部“保存全部设置／恢复全部默认”固定可见并作用于全部设置；跨模块和页面保留草稿，非法限速会定位到下载设置，保存失败保留输入。限速输入为 0 或 0.001024–10240 MB/s；滑块用于 0–100 MB/s 的快捷调整，步长 0.25 MB/s，更高值通过输入框设置。Rust 接口和存储仍使用整数 `limitKib`，未编辑限速时原值保存；新输入换算到最近的整数 KiB/s，保存成功回显实际值。恢复默认只修改表单，点击保存后才生效；收藏自动检查间隔仍固定为 6 小时。
- **适配与状态**：下载、设置页采用固定左侧导航与独立内容滚动，窄窗口仍保留文字导航；历史与收藏保留原有滚动方式。保留后端未就绪、加载、空记录、文件不可访问和错误状态。列表、历史及附件选择不显示常态校验标记；历史统计按任务状态显示本页下载失败数，收尾阶段显示“正在完成下载”。旧浏览器入口已删除。设置新增数据位置、打开数据目录和导出备份。

参考图中的示例版本、地区线路、延迟、并发设置及自动生成源码包不代表现有产品能力。软件版本为 0.2.4，窗口按钮由原生 Windows 标题栏提供。历史视觉核对见 [design-qa.md](design-qa.md)，其中截图和日志属于维护者本地验收资料，不随源码分发；隔离浏览器验收只提供前端证据，不等同于 Windows 原生或真实网络下载验收。

## GitHubSP 自身更新源

本仓库发布的 v0.2.4 便携程序在构建时配置官方发布源 `lkuliuying/githubsp`。源码自行构建时，若未设置 `GITHUBSP_RELEASE_REPOSITORY`，点击检查仍会显示“尚未配置官方发布源”，不会发送更新请求或声称已是最新版。该变量是公开仓库标识而非密钥，只在编译时读取；`build.rs` 跟踪其变化。构建官方发布配置：

```powershell
$previousReleaseRepository = $env:GITHUBSP_RELEASE_REPOSITORY
try {
    $env:GITHUBSP_RELEASE_REPOSITORY = 'lkuliuying/githubsp'
    .\scripts\native-cargo.ps1 -CargoArguments @('build','--release','--locked')
    if ($LASTEXITCODE -ne 0) { throw 'GitHubSP build failed' }
} finally {
    $env:GITHUBSP_RELEASE_REPOSITORY = $previousReleaseRepository
}
```

检查默认读取 GitHub 官方 Latest 正式 Release，按语义化版本比较并接受 `v` 前缀。发现新版时弹出下载选择，默认选中最新正式版；“选择其他版本”按需分页读取高于当前版本的正式版，排除草稿、预发布和不可比较的版本号。分页追加去重并保留选择，过滤后为空仍可继续加载；关闭后可通过“下载新版”重新打开。

每个版本只匹配官方附件中的 `GitHubSP-v{版本}-windows-x64.exe`，展示文件名、大小与更新说明，使用接口返回的真实附件地址；缺失、不可用或匹配不唯一时禁用下载，并保留官方发布页入口。点击“选择保存位置并下载”后，系统目录选择器默认定位上次成功使用的目录，并重新校验所选目录。取消不创建任务；确认后以自动线路加入现有串行队列，成功时切换到下载页。目录错误或创建失败保留版本选择，已有相同未完成任务时提示继续或重试原任务。请求期间防止重复提交；关闭弹窗、卸载页面或后端失去就绪状态后，迟到的目录选择结果不再创建任务。已经提交的创建请求不能通过关闭弹窗取消，后续暂停或取消在下载队列操作。下载完成后由用户自行启动新版，不覆盖正在运行的程序、不自动执行或安装。

更新说明通过 `pulldown-cmark` 解析并由原生 Slint 组件呈现标题、列表、强调、代码、引用和表格，保留原有面板和滚动区域。原始 HTML 作为文本显示，图片只展示替代文字，不自动请求远程资源；不提供代码高亮或额外 Markdown 插件。说明中的有效 HTTPS GitHub 链接（包括 `compare` 页面）由用户点击后经过 Rust `updates::validate_update_link` 校验后在系统浏览器打开；Rust 拒绝站外地址、非 HTTPS、凭据、非标准端口及非法地址，其他链接保留不可点击文字。打开失败保留说明正文并允许重试。官方发布页按钮继续使用原有 `open_release` 校验规则。测试用可控服务验证未配置、版本过滤、分页、限流及更新附件入队；公开 Release 的可访问性与桌面界面实际点击验收是不同的验证范围。

## 下载规则

- 默认串行处理任务。新任务自动排队；退出应用会停止写入并保存进度，重启后恢复为暂停状态，由用户点击继续。
- 候选线路为 GitHub 直连、[GH-Proxy](https://gh-proxy.com/docs/github-accelerator)（`gh-proxy.org`）和 [ghproxy.net](https://ghproxy.net/)。线路集中定义在 `core/src/network.rs`；公共加速服务会收到公开附件 URL。当前网络客户端不读取系统代理环境变量。
- 每条线路对实际附件检测最多 524,288 字节（约 524 KB）、最多 10 秒，按有效数据吞吐选择。排除错误页面、异常响应及不合法范围；HTTPS 重定向仅限登记域名，最多 5 次。
- 已知大小、不小于 16,777,216 字节（约 16.8 MB）且实际返回有效 `206` 的附件使用同一线路的两路分段下载；其他附件使用单连接流式下载。每次响应核对范围、长度和实际写入量。
- 连接超时 10 秒，无数据超时 30 秒。每条线路首次失败后最多重试两次；按服务端 `Retry-After` 等待，超过 30 秒则停止本轮尝试该线路并考虑其他线路，后续恢复检测仍遵守该线路的冷却。任务行显示重试原因、次数、倒计时和换线重下提示，各线路失败详情折叠在任务详情中。磁盘或数据库错误直接停止，不持续重试写入。
- 续传要求同一线路、相同已知长度、有效 Range，以及相同官方摘要或同线路强 ETag。本地分片也必须通过已保存的 SHA-256 检查；条件不足或资源变化时重新下载。切换线路从头下载，不拼接不同线路数据。
- 先同步分片落盘，再事务性保存恢复信息。合并校验后才生成最终文件，同名文件自动添加序号。生成最终文件前记录发布意图，恢复时可识别“文件已生成、状态尚未保存”的异常退出，避免重复生成。
- 暂停保留临时数据；取消等待所有写入结束后清理该任务的数据；移除历史记录保留成品文件。遇到临时目录内无法识别的文件会报告错误，避免误删。

- 候选耗尽且仍存在连接失败、超时、HTTP 408/429/5xx 等临时故障时进入 `waiting_network`。每轮重新检测同一附件，先前不可用的线路可重新加入；仅附件不存在、协议失败或本地磁盘/权限问题不会自动等待，完整性错误不进入恢复。手动固定线路只重试该线路。
- 每次恢复过程最多消耗 5 分钟，计入退避和恢复探测；其他任务占用下载位置期间暂停恢复计时。退避为 10、20、40、60 秒，增加最多 20% 的随机延迟且不超过 60 秒，同时遵守更长的服务端 `Retry-After`。收到有效文件数据后结束本次恢复过程。普通排队任务优先，多个等待任务按恢复到期先后、原队列位置接续，保持串行传输。
- 等待恢复可立即重试、暂停或取消。立即重试只跳过退避，不重置额度、不绕过服务端冷却；暂停停止自动恢复，取消沿用分片清理规则。退出停止恢复，重启统一暂停，需手动继续。恢复期间不发送失败提醒，仅额度耗尽或不可恢复错误最终失败时提醒一次。
- 自动模式下，连续 30 秒平均速度低于 256 KB/s 或所选线路本轮探测速率的 25%，才检测备选线路，每 120 秒最多一轮。设置限速、大小未知或预计剩余不足 60 秒时不检测；手动检测优先，取消重叠的低速检测。继续当前线路按剩余字节/当前速度估算，换线按完整大小/新探测速率再加 10 秒估算，至少节省 30% 且 30 秒才显示建议。
- 低速建议出现时继续下载，并展示预计耗时和需要重下的数据量。用户确认后重新检测候选并复核收益；失效时保留原线路。换线由 Rust 等待旧写入完全停止、清理分片再开始，保持自动模式。选择继续当前线路后 10 分钟内不再提示。阈值为首版默认值，短时检测不承诺实际提速。

## 完整性保护

程序直接查询 [GitHub 官方附件元数据](https://docs.github.com/en/rest/releases/assets)，不信任代理提供的摘要。存在官方 SHA-256 时，必须匹配才能生成成品；摘要不匹配会失败。界面不展示校验结果标签，完整性失败仍以文件内容异常提示原因及换线重试方式；原始错误和校验字段保留在内部记录中。

没有官方摘要或官方接口暂时不可用时，仍按长度、范围和本地分片一致性检查完成下载，这不等于已验证文件真实性。服务端返回 HTML 错误页面、截断内容或不正确的长度会导致失败。隐藏展示不改变后台检查或完成判定。

## 数据位置与恢复

默认数据库为 `%LOCALAPPDATA%\com.githubsp.desktop\tasks.sqlite3`。设置显示实际位置，可打开目录或选择新文件导出备份。移动或替换 exe 不移动数据；普通用户权限即可运行。试用版的 `com.githubsp.native-prototype` 目录继续独立保留，不自动合并。开发验收使用显式绝对路径：

```powershell
.\artifacts\releases\v0.2.4\GitHubSP-v0.2.4-windows-x64.exe --data-dir "F:\GitHubSP-验收"
```

- **数据库 v3**：保留任务 JSON、收藏、设置和恢复信息；增加状态、规范化检索文本、队列位置及查询索引。投影与 JSON 同事务写入。使用 `rusqlite = 0.39.0` bundled/backup，内嵌 SQLite 3.51.3；继续 WAL 与 `synchronous=FULL`，不放宽分片先落盘再确认检查点的顺序。[SQLite WAL 说明](https://sqlite.org/wal.html)
- **升级**：新库直接建立 v3；v1/v2 先用 SQLite Backup API 将一致性副本写入 `backups/tasks-v{旧版本}-before-v3-{UUID}.sqlite3`，校验后逐条回填并事务升级。失败回滚，原库保留；未知结构、损坏记录或更高版本明确报错，不以空库替换。已升级的库不在每次启动时解析全历史，读取到损坏记录时报告错误。
- **按需加载**：下载页只保留全部未完成任务（含暂停、失败、等待恢复）与最近 50 条已完成/取消记录；“最近”按添加时间倒序，同时间按 ID 倒序。历史全部保留，SQL 筛选、计数和排序，每页 20 条；Unicode 小写规范化后进行字面子串搜索，`%` 和 `_` 不作为通配符。仅检查当前页文件；三项状态统计只代表本页。清理已完成作用于全部完成记录，确认显示总数并保留成品。
- **备份**：手动导出同样使用 [SQLite Backup API](https://sqlite.org/backup.html)，含已提交的 WAL 内容，校验、关闭并同步临时副本后发布；目标已存在时拒绝覆盖。失败不修改原库，不报告部分文件成功。备份只包含应用记录，下载文件和续传分片需要另行保留。
- **运行边界**：单一 actor 写入，历史/按 ID 读取/备份共用一个后台读取许可，调用取消后仍等实际连接关闭才释放许可；退出停止新读取并等现有读取完成。正式目录继续兼容旧版单实例互斥，新旧程序不得同时操作此库。
- **下载文件**：成品和下载目录内的 `.githubsp-{任务 UUID}` 分片保持原位置。程序不因迁移自动移动、清理或归档这些文件。合并期间仍需额外空间；异常退出后任务恢复暂停，手动继续。

**离线恢复步骤（会更换正在使用的数据库，请先保存当前目录）：**

1. 从托盘退出所有 GitHubSP 版本，确认进程停止。
2. 将整个当前数据目录移到另一个保留位置，连同其中的 WAL/SHM 一起保存；不要直接覆盖原数据库。
3. 新建原数据目录，仅将选定的一致性备份复制为 `tasks.sqlite3`。不得把旧 `tasks.sqlite3-wal` 或 `tasks.sqlite3-shm` 混入新目录。
4. 启动支持该数据库版本的程序并核对历史、收藏和设置。成品与续传分片需位于记录中的原路径；恢复操作不会替你移动它们。

v3 不能直接交给旧版程序。回退旧程序需使用升级前的 v1/v2 备份，升级后新增的记录不会自动合并回去。首次升级需要备份和索引的额外空间与时间；备份不自动轮换或删除。

本项目没有独立项目记忆文件，长期结构、命令、行为和限制以本 README 为准。源码维护在 [main 分支](https://github.com/lkuliuying/githubsp/tree/main)，发布需另行授权。

## 项目目录与本地产物

维护入口为根目录 `Cargo.toml`、`Cargo.lock`、`core/`、`native-desktop/` 和 `scripts/`。当前发布文件放在 `artifacts/releases/v0.2.4/`，其下 `audit/` 保存本地测试、上传校验及目录清理记录，不进入源码包。

退役的 `node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/`、`.cache/npm/`、`.cache/icons/` 以及无文件的旧 `src/` 目录不再参与构建，可清理。`native-desktop/target/` 是可重新生成的 Rust 构建缓存，清理后下一次构建需要重新编译。`artifacts/` 中的历史源码归档、发布包、截图及数据备份，以及 `.cache/` 中的历史交付记录仍保留；不能将整个目录当作无用缓存删除。正式应用数据位于项目目录之外，不属于项目清理范围。

## 原生桌面架构

Slint 1.18.1 + Winit + FemtoVG-WGPU 显式使用 DX12。界面与窗口留在主线程；下载、校验、数据读取在后台运行。状态事件合并到最高每秒 4 次，完成和错误立即更新；隐藏到托盘后停止普通界面刷新，恢复时读取最新快照。长列表使用 Slint ListView，历史数据库分页。

页面和已保存 SVG 直接编译嵌入 exe。Noto Sans SC 与第三方声明随程序嵌入，“关于”保留 `AboutSlint`。软件渲染器仅用于确定性组件测试，不是正式默认后端。静态依赖/PE 检查不代表已经通过干净系统、输入法、DPI、驱动或资源收益验收。旧 Vue/Tauri 代码和 npm/Vite 构建链已退役；迁移前未提交源码有本地归档。

## 开发与构建

需要 Rust 1.96+ MSVC 工具链和 Visual Studio C++ Build Tools。开发和打包均无需 Node.js。根目录 Cargo workspace 统一版本和锁文件，默认成员为 Slint。脚本固定 Windows x64、静态 CRT 和 `native-desktop/target` 输出目录，避免静态 CRT 影响编译期过程宏。初次构建需联网获取 Cargo 依赖；缓存完整后可追加 `--offline`。

```powershell
.\scripts\native-cargo.ps1 -CargoArguments @('run','--locked')
.\scripts\package-native-windows.ps1
```

打包脚本从 Cargo 读取版本，更新内嵌许可证，编译并检查依赖树和 PE 导入。默认仍输出到 `artifacts/slint-retirement/package/`；可用 `-OutputDirectory` 指定独立目录，相对路径以仓库根目录解析。v0.2.4 发布构建使用 `.\scripts\package-native-windows.ps1 -Offline -OutputDirectory 'artifacts/releases/v0.2.4'`，输出 exe、SHA-256 和验收限制。打包脚本不生成安装包或自动发布；用户运行只需 exe。打包默认编译公开更新源 `lkuliuying/githubsp`，可用 `-ReleaseRepository owner/repo` 更换。普通源码构建未配置 `GITHUBSP_RELEASE_REPOSITORY` 时，检查更新明确提示未配置，不发起请求。

## 模块边界

| 路径 | 职责 |
| --- | --- |
| `Cargo.toml`、`Cargo.lock` | 唯一 workspace 版本、锁定依赖和发布 profile |
| `core/` | `githubsp-core` 包；继续导出 `githubsp_lib`，不依赖 UI |
| `core/src/manager.rs`、`manager/route_ux.rs`、`route_policy.rs` | 权威任务 actor、串行队列、恢复预算、换线和退出协调 |
| `core/src/store.rs`、`store/migration.rs`、`store/backup.rs`、`manager/reads.rs` | v3 持久化、升级、SQL 分页、一致性备份和读取生命周期 |
| `core/src/model.rs`、`history.rs`、`notice.rs` | 数据契约、检索规范化、当前页文件检查、完成提醒纯状态 |
| `core/src/engine.rs`、`files.rs`、`source.rs`、`network.rs` | 下载、续传、文件边界、URL 与响应校验 |
| `core/src/catalog.rs`、`intake.rs`、`preflight.rs`、`library.rs`、`updates.rs` | 官方目录、批量、目录确认、收藏及软件更新 |
| `native-desktop/ui/` | Slint 四页、附件与更新选择器、提醒和关于窗口 |
| `native-desktop/src/` | 进程内界面适配、过期请求保护、托盘、单实例、目录选择和原生窗口 |
| `native-desktop/resources/` | 字体、SVG、窗口/托盘图标、第三方声明 |
| `scripts/`、`core/examples/` | Windows 构建打包、许可生成、隔离下载和资源验证 |

`Snapshot.tasks` 仅表示下载页工作集；`total_tasks` 和 `completed_tasks` 是数据库总数。`history_revision` 随历史字段变化而递增，普通下载进度不触发整页重查。快照版本、队列版本、页面 epoch 和历史修订号共同拒绝过期结果。历史操作先按 ID 获取记录，完成提醒携带文件名和耗时摘要，均不依赖记录仍在最近 50 条中。

`Manager::history` 查询 SQL 历史页，`task` 支持缓存外读取，`remove_completed` 校验确认时的历史修订号，`export_backup` 生成一致性副本。所有写操作仍由 actor 串行处理。暂停、取消、退出、同名保护、分片校验以及五分钟有界恢复保持原有规则。UI 通过 Rust 类型调用这些接口，不再存在 Tauri IPC 或 WebView 提醒窗口。

## 自动化验证

```powershell
.\scripts\native-cargo.ps1 -CargoArguments @('test','--workspace','--locked','--offline')
.\scripts\native-cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings')
cargo fmt --all -- --check
.\scripts\package-native-windows.ps1 -Offline
.\scripts\measure-history.ps1 -Offline
.\scripts\test-native-startup.ps1 -Executable .\artifacts\slint-retirement\package\GitHubSP-v0.2.4-windows-x64.exe
```

核心测试覆盖真实本地 HTTP fixture 的暂停续传、取消、校验、同名文件、限速、恢复等待、换线确认及退出保存；迁移测试覆盖 v1/v2、WAL、一致性备份、损坏/高版本、容量不足和事务回滚。历史测试覆盖中文/大小写/字面子串搜索、全部状态、同时间排序、末页删除、按 ID 操作及超出 50 条的清理和提醒。

Slint 测试使用临时数据库、回环目录服务和软件测试后端，覆盖四页、64 条历史、模块与输入草稿、批量预览、创建结果呈现、过期查询、设置保存与失败保留、更新弹窗与安全 Markdown；以 1448×1086、1040×740 和 720×520 检查队列、双栏不重叠、模块、详情和固定操作可达性，图片位于 `artifacts/style-abc-review/ui-renders/`。此外覆盖全库统计与列表上限的差异、限速联动、弹窗 Tab／Shift+Tab／Esc 和提交保护、完整许可分页与完成小窗停留事件。目录确认、重复操作、更新边界的核心断言迁入或保留在 Rust 测试中，旧框架专属测试已删除。

测试专用环境变量 `GITHUBSP_UI_RENDER_DIRECTORY` 可指定截图目录，未设置时保持上述默认路径。本轮设为绝对路径 `F:\Program\githubsp\artifacts\download-layout-review\ui-renders`，保留上一轮截图；新增跨版本大小汇总、分页失败保留、最快摘要重置、历史检测标识及长错误文本的布局验收。该变量不参与正式程序行为。

`measure-history.ps1` 分别用新进程测量 1 千、1 万、10 万条完成记录和固定 3 条未完成任务，断言常驻 53 条、页内至多 20 条，记录启动、分页、搜索时间和 Windows 私有字节；它只测核心，不包含 Slint/GPU 或旧 WebView 对比。完整资源验收仍需同机器、同数据、同 DPI、同窗口和相同下载状态，对完整进程树测空闲、传输、托盘与提醒；30% 内存下降仍是验收目标，不是已经取得的结果。`scripts/measure-process-tree.ps1` 保留用于这种进程树采样。

`test-native-startup.ps1` 新建中文隔离数据目录，启动指定 exe，检查主窗口、实际加载模块、第二实例退出和正常保存退出。只回收脚本创建的进程，不触及正式数据；这是当前开发机的烟雾测试，不模拟断网或未安装额外运行库的干净系统。

真实网络验收使用独立目录（以下命令会下载公开附件）：

```powershell
.\scripts\native-cargo.ps1 -CargoArguments @('run','-p','githubsp-core','--example','verify_download','--locked','--','https://github.com/rakanki911/DLSS5-Swapper/releases/download/v2.2.9/DLSS5-Swapper-Setup-2.2.9.exe','artifacts/acceptance/下载验证')
```

`verify_v2` 保留历史命名，用 Backup API 只读复制 v1 验收库后升级至当前 v3，再执行隔离批量下载；不得对正式库直接试验。截图、测试数据库、源码归档、资源结果与构建缓存都放在忽略目录。测试、静态导入审计、真实下载和干净 Windows 桌面验收是不同证据；未执行项不记为通过。

## 已知限制

公共线路的可用性和速度随网络与服务策略变化，短区间测速不能保证整个文件的持续速度，直连也可能最快。没有官方摘要时无法证明附件真实性；下载完成不会自动启动任何文件。免费官方 API 可能限流，目录不可用时标准直链仍可尝试下载。

当前发布程序没有 Windows Authenticode 代码签名，Windows 可能显示发布者未知。技术与许可证徽章不改变这一状态。干净 Windows 离线启动、中文输入法、100%/150%/200% DPI、多显示器、DX12 驱动、托盘与提醒不抢焦点、真实网络和完整资源门槛须单独验收。

## 许可证

项目代码按 [MIT License](LICENSE) 提供。第三方依赖继续受各自许可证约束。

---

<a id="english"></a>

## English

GitHubSP v0.2.4 is a Windows desktop downloader for public GitHub Release assets, built with Slint, Rust, and SQLite. This release completes the native desktop migration and redesigns downloads, settings, and dialogs. It provides release browsing, asset selection, batch previews, a system tray, bandwidth limits, searchable history, favorites, and update checks. Route recovery, confirmation-based route suggestions, background completion notices, elapsed-time statistics, and Markdown notes from v0.2.3 are retained. The application UI is in Simplified Chinese; this README is bilingual. Downloads run sequentially by default.

The three badges identify the project MIT license and the Slint / Rust technology stack. They use [Shields.io static badges](https://shields.io/badges/static-badge), which require no application or approval. They do not certify security or provide Windows code signing.

## Quick start

Download the portable executable from [v0.2.4 Release](https://github.com/lkuliuying/githubsp/releases/tag/v0.2.4). Maintainer artifacts are under `artifacts/releases/v0.2.4/`; only `GitHubSP-v0.2.4-windows-x64.exe` is needed to run the app. The source ZIP comes from the release tag and includes Cargo.lock, fonts, icons, and licenses. Historical v0.2.3 and older assets use Vue/Tauri and are different from this native build.

The 2026-10-08 design uses compact download workspaces (A), spacious settings cards (B) and structured dialogs (C). The subsequent revision changes only New Download (selected option 1: compact version/asset columns) and Route Diagnostics (selected option 2: results with a summary sidebar). Previous packages remain under `artifacts/slint-retirement/package/` and `artifacts/style-abc-review/package/`. See `artifacts/download-layout-review/index.html` for the latest comparisons and `design-qa.md` for verification boundaries. Launch with an absolute `--data-dir` path to isolate trial data; ordinary startup still uses the existing application data directory.

The native build targets Windows 10/11 x64 with system components and a compatible DX12 driver. UI, fonts, icons, SQLite and notices are embedded. Static CRT is requested and the package script audits direct imports: users should not need a separate WebView2, Node.js, Python or VC++ runtime installation. Clean-system offline startup remains a separate acceptance check. This build is unsigned; badges indicate technology and licensing, not security certification.

1. Compare the executable SHA-256 with `SHA256SUMS.txt`.
2. Save progress and exit all older instances from their tray menu.
3. Start the executable, paste a public release-asset, repository or release URL, and select a destination. Missing manually entered directories require confirmation; picked directories must already exist. Downloaded files are never executed automatically.

Public GitHub HTTPS URLs are supported; credentials, queries, fragments, unsafe filenames and path traversal are rejected. Standard asset links download directly; repository/release links open asset selection. Batch preview accepts 1–100 assets. Duplicate unfinished tasks use the existing task; completed filenames are protected by collision handling. Private repositories, Git clone, generated source archives, custom proxies and automatic update installation are unsupported.

## Downloads, history, favorites, and settings

- **Downloads:** direct links, official release/asset lists, batch previews, a queue, and the most recent route diagnostics. A built-in route can be selected after pausing a task. A pinned route does not automatically fail over. Switching routes with existing parts requires confirmation to restart; returning to automatic mode can reuse the old route if its resume conditions hold.
- **Manual route diagnostics:** no task is required. Empty or whitespace-only input uses this project's public `GitHubSP-v0.2.2-windows-x64.exe` asset; a direct asset URL checks that asset. Repository or release-page URLs prompt the user to supply a direct asset URL or clear the input. Routes are checked sequentially, reading at most 524,288 bytes (about 524 KB) and waiting at most ten seconds per route, without saving or executing files. Only one manual round runs at a time. Transfers, queue continuation, pause, and cancel remain available, and checking never switches the active download's route. Probe traffic bypasses download bandwidth limits and may briefly affect live speed; results are advisory. If the default asset is unavailable, another direct asset URL can be used. The panel shows the source and actual filename, replacing results as a complete batch. Automatic checks from downloads started before or during a manual round cannot overwrite it; downloads started afterward can update the panel. Restored reports are labeled historical without inferring their source. Errors and timeouts allow another attempt, and application exit cancels unfinished checks.
- **Directory checks:** validate the directory, write access, and space available to the current user before task creation and transfer. Reserve space for remaining parts plus the complete merged file. Unknown sizes are reported explicitly. Another process may consume space after the check; write failures still stop the task. Downloading to another directory creates a new task without moving old parts.
- **Enter or choose a directory:** adding a task or previewing a batch checks a manually entered path. If missing, a confirmation displays the full path and offers to create it, including missing parents, then continue. Cancelling preserves input and creates neither directories nor tasks. Batch submission checks again; browsing releases creates no directories. The system picker accepts only existing directories and retains Windows' own new-folder button. If a selected directory disappears, select it again. File paths, non-directory parents, permission errors, and inaccessible drives are reported as errors. Directories already created are retained if later checks or downloads fail. Transfers and resumed tasks do not automatically recreate missing directories.
- **Tray and completion notices:** hide from the toolbar; the tray menu can show the window, pause downloads and automatic recovery, or save progress and exit. Closing the main window exits by default, with an option to hide instead. Completion or final failure creates an in-app notice and tray unread indicator once per state change. Background completion notices are enabled by default: a completed download shows a separate silent window when the main window is unfocused, minimized, or hidden. It shows the filename and elapsed execution time, disappears after eight seconds, pauses its countdown during hover or keyboard interaction, and combines consecutive completions. Viewing downloads restores the main window and selects the downloads page; closing the notice only hides it. Foreground completions, handled notices, and historical records are never replayed. Disabling the setting or focusing the main window dismisses the notice. Notification failures do not change download results. This is not a Windows system notification; it does not enter the notification center or automatically follow Do Not Disturb.
- **Elapsed download time:** task details show elapsed execution time and freeze the total after completion. Rust uses a monotonic clock to accumulate probing, transfer, automatic retries, merging, and verification, excluding queueing, network-recovery waits, manual pauses, and time after application exit. Recovery probes and actual transfers still count. Resuming, retrying, and changing routes retain the same task's accumulated time; a newly created download starts at zero. Live values refresh every second and are saved with status/checkpoint updates, with an additional save every five seconds during long waits. Normal stops save the final value. Older tasks without timing data show unknown; recovery after an unexpected exit retains only the saved value and marks it incomplete, without inferring duration from creation/completion timestamps.
- **Size and speed:** the UI uses decimal B, KB, MB, GB, and TB with a factor of 1000; rates use the corresponding per-second units. Active transfers show live speed. Completed tasks show the average of final file bytes over accumulated execution time, including probing, retries, and finalization. Missing, invalid, or incomplete timing displays a dash with an explanation. Other non-transferring states do not display stale speeds.
- **Bandwidth and queue:** limits use decimal MB/s with fractional input; 0 means unlimited. Saved changes take effect immediately, and both segments share the allowance. Waiting for bandwidth can be interrupted by pause, cancel, or exit and does not count as a network idle timeout. Probe traffic is outside the transfer limit. Waiting tasks can move up, down, or to the front without preempting an active task or starting paused tasks. Refresh and retry if queue order changes concurrently.
- **History:** search by filename, repository, or release; filter by status; paginate in groups of 20. Opening or refreshing checks final-file availability only for the current page, distinguishing missing from inaccessible files. Open the directory, copy the URL, or download again. Removing completed/cancelled records preserves final files. Legacy tasks without a trusted completion time display “未知” (unknown).
- **Favorites:** add from repository URLs or task sources, up to 200 repositories. Favorites are independent of tasks. The first successful check establishes a stable-release baseline; Release IDs track already-notified versions. Prereleases do not trigger automatic notices. Removing a favorite does not affect files or tasks.
- **Check scheduling:** manual by default; optional checks every six hours while the app runs. Exiting stops them; there is no background service or automatic startup. Catalog, favorite, and application-update requests share serialized access, a 60-second cache, and conditional requests. Repeated operations reuse cached results. Failures preserve the last successful version, and `Retry-After` / rate-limit reset times are respected. Unauthenticated GitHub API quotas still apply; conditional requests do not guarantee quota exemption.

## Interface and interaction

The four Slint pages use in-process page state. Shared styles use a light-blue background, white panels, green primary actions, and Phosphor icons.

- **Download workspace:** the top-level navigation is retained. A left sidebar selects Queue, New download, or Route diagnostics, with Queue as the default. The queue fills the available height and scrolls independently beneath fixed actions. Content areas at least 1000px wide show the complete table with a stretching filename column and right-aligned actions; narrower areas use two-line task rows, with route, remaining time, and full filenames in details. Task details expose source, directory, route, and queue controls. Resume-all handles paused/failed tasks; pause-all handles active/waiting tasks. Bulk actions stop and report the first error. Clearing completed records requires confirmation and preserves files.
- **Creation and preview:** Direct link, Repository assets, and Batch links share the destination directory while retaining separate inputs. Diagnostics has its own address field. Repository versions and assets use columns on wide content areas and stack on narrow ones. Preview replaces the editing step within the module; returning preserves selection. Complete success opens the queue, while partial failures retain unsuccessful links or asset selections. Navigation or input changes invalidate late directory and asset results. Favorites open repository assets; app-update downloads and completion notices open the queue. Navigation and drafts remain session-only and add no stored settings.
- **Release pagination:** loading feedback occupies a reserved slot on the right of the creation-module heading, including while idle. Requests retain the previous releases, assets, and page number until success; failures keep that page and show an error. Temporarily locked controls retain their normal appearance while preventing duplicate actions; controls already unavailable because of page boundaries or empty selections remain subdued. Loading feedback does not shift the module or its fixed footer; longer content scrolls within the module.
- **History:** shared search and status filters, summary cards, an independently scrollable table, and page navigation. The matched count comes from the backend; other statistics cover only the current page. Pages contain 20 records. Removing the last record on the last page returns to a valid page.
- **Favorites:** card/list layouts, result filters, and sorting by last check or repository name. Labels show actual successful, failed, or pending checks without inferring updates “today” or installed versions.
- **Settings:** the sidebar selects Download settings, Window and reminders, Favorite checks, Data management, or Updates and About. One module is shown at a time. Save all settings and Restore all defaults remain visible at the bottom and apply across modules. Drafts survive page and module switches; an invalid limit opens Download settings, and a failed save retains input. Limit input accepts 0 or 0.001024–10240 MB/s. The slider covers 0–100 MB/s in steps of 0.25 MB/s; higher limits use numeric entry. Rust interfaces and storage retain integer `limitKib`: unedited limits are preserved exactly, while new input is rounded to the nearest KiB/s and the saved value is displayed. Restoring defaults changes the draft only until saved. Automatic favorite checks retain the fixed six-hour interval.
- **Responsive and error states:** Downloads and Settings use fixed sidebars with visible text labels and independent content scrolling, including narrow windows. History and Favorites retain their existing scrolling. Backend-not-ready, loading, empty, inaccessible-file, and error states remain visible. Download lists, history, and asset selection hide routine verification badges. History counts failed downloads by task status, and finalization displays “正在完成下载”. The browser entry has been removed. Settings expose the data directory and consistent backup export.

Reference images are not promises of extra versions, regional routes, latency metrics, parallel task settings, or support for generated source archives. The app version is 0.2.4, and window controls are provided by the native Windows title bar. [design-qa.md](design-qa.md) contains the historical visual review in Chinese. Its screenshots and logs are local maintainer evidence, not distributed source. Isolated browser checks establish frontend behavior, not native Windows or real-network acceptance.

## Application update source

The v0.2.4 portable executable published by this repository is built with `GITHUBSP_RELEASE_REPOSITORY=lkuliuying/githubsp`. For a source build without that variable, checking updates reports that no official source is configured, sends no update request, and does not claim the app is current. The value is a public repository identifier, not a secret. It is read at compile time, and `build.rs` tracks changes. To build with the official release source:

```powershell
$previousReleaseRepository = $env:GITHUBSP_RELEASE_REPOSITORY
try {
    $env:GITHUBSP_RELEASE_REPOSITORY = 'lkuliuying/githubsp'
    .\scripts\native-cargo.ps1 -CargoArguments @('build','--release','--locked')
    if ($LASTEXITCODE -ne 0) { throw 'GitHubSP build failed' }
} finally {
    $env:GITHUBSP_RELEASE_REPOSITORY = $previousReleaseRepository
}
```

Checks start with GitHub's official Latest stable Release. Versions follow semantic versioning and may have a `v` prefix. When an update is available, a download dialog opens with that release selected. Other stable versions newer than the running version load on demand through pagination, excluding drafts, prereleases, and incomparable tags. Appending pages deduplicates releases and preserves the selection; an empty filtered page does not prevent loading later pages. Dismissing the dialog leaves a download button to reopen it.

Each release must have exactly one available official asset named `GitHubSP-v{version}-windows-x64.exe`. The dialog shows its name, size, and release notes and uses the actual asset URL from the API. Missing, unavailable, or ambiguous assets disable downloading while keeping the official release-page action. The download action opens the system directory picker at the last successfully used location and revalidates the chosen directory. Cancelling creates no task. Confirmation adds a task to the existing serial queue with automatic route selection and switches to the downloads view on success. Directory and creation errors preserve the selected version; duplicate unfinished tasks prompt the user to continue or retry the original. Repeated submissions are blocked. Closing the dialog, unmounting the view, or losing backend readiness invalidates pending directory selections. Once creation has been submitted, closing the dialog cannot cancel it; subsequent pause or cancellation uses the download queue. Users launch the downloaded portable program themselves; the app does not replace, run, or install it automatically.

Release notes use `pulldown-cmark` and native Slint components to display headings, lists, emphasis, code, quotations, and tables within the existing panel and scroll area. Raw HTML is shown as text; images display alternative text without requesting remote resources. No syntax highlighter or extra Markdown plugins are included. Clicking a valid HTTPS GitHub link, including a `compare` page, opens the system browser after Rust `updates::validate_update_link` validation. Rust rejects external domains, non-HTTPS URLs, credentials, nonstandard ports, and malformed addresses; other links remain non-clickable text. Opening failures preserve the notes and allow retry. The official release-page button retains the existing `open_release` validation. Controlled-server tests cover missing configuration, version filtering, pagination, rate limiting, and adding update assets to the queue. Public Release availability and actually clicking the native update UI are separate verification scopes.

## Download rules

- Tasks are sequential by default, and new tasks queue automatically. Exiting stops writes and saves progress. On restart, tasks recover paused and require explicit continuation.
- Built-in routes are direct GitHub, [GH-Proxy](https://gh-proxy.com/docs/github-accelerator) through `gh-proxy.org`, and [ghproxy.net](https://ghproxy.net/). Definitions live in `core/src/network.rs`. Public acceleration services receive the public asset URL. The network client does not read system proxy environment variables.
- Each route probes at most 524,288 bytes (about 524 KB) of the actual asset for up to 10 seconds. Selection uses valid data throughput and rejects error pages, malformed responses, and invalid ranges. HTTPS redirects are restricted to registered hosts, with at most five hops.
- Assets with a known size of at least 16,777,216 bytes (about 16.8 MB) and valid `206` support use two segments on the same route. Other assets stream through one connection. Every response is checked for range, length, and actual bytes written.
- Connection timeout is 10 seconds; network idle timeout is 30 seconds. Each route allows up to two retries after the first failure. `Retry-After` is respected; a delay exceeding 30 seconds stops attempts on that route and may lead to another route. Disk or database errors stop the task rather than repeatedly retrying writes.
- Resume requires the same route, the same known size, valid Range support, and either a matching official digest or a strong ETag from that route. Local parts must match their stored SHA-256 values. Missing conditions or changed resources cause a restart. Data from different routes is never combined.
- Parts are synced to disk before recovery metadata is committed in a transaction. A final file is published only after merge and verification; existing filenames receive a numeric suffix. Publication intent is recorded first, allowing recovery from a crash between file creation and state persistence without creating a second copy.
- Pausing retains temporary data. Cancelling waits for writers to stop before cleaning only the task's data. Removing history preserves final files. Unrecognized files in a task directory produce an error instead of being deleted.

- Exhausted candidates enter `waiting_network` only when transient connection failures, timeouts, or HTTP 408/429/5xx remain. Every recovery round probes the same asset again so unavailable routes can rejoin. Permanent-only missing assets and protocol failures, integrity failures, and local disk/permission errors do not enter recovery; pinned mode retries only its chosen route.
- Each recovery episode has a five-minute allowance covering backoff and probes, frozen while another task owns the transfer slot. Backoff is 10, 20, 40, then 60 seconds, with up to 20% extra jitter capped at 60 seconds; longer server `Retry-After` values remain binding. Valid file data ends the episode. Ordinary queued tasks go first, then due recovery tasks by due order and original queue position, preserving serial transfers.
- Retry now skips backoff without resetting the allowance or bypassing server cooldown. Pause stops recovery; cancel retains existing part-cleanup rules. Exit stops recovery, and restart restores paused tasks requiring manual continuation. Recovery emits no failure notice until the allowance is exhausted or an unrecoverable failure ends the task.
- Automatic mode checks alternatives after a sustained 30-second average below 256 KB/s or 25% of the selected route's initial probe throughput, at most once per 120 seconds. It skips capped downloads, unknown sizes, and estimated remaining times below 60 seconds. Manual diagnostics cancel overlapping automatic probes. Continuing costs remaining bytes/current speed; restarting costs full size/candidate speed plus ten seconds. Suggestions require both 30% and 30 seconds of savings.
- Suggestions show estimated time and bytes to redownload while the current transfer continues. Confirmation rechecks the candidate and savings before stopping the old writer and clearing parts. Invalid suggestions preserve the current transfer; successful switches retain automatic mode. Dismissing suppresses suggestions for ten minutes. These initial thresholds and short probes do not guarantee faster completion. Displayed transfer speed uses a five-second window, decays to zero without new data, and hides stale ETA. Retry phases/countdowns appear in the row, with route failures in expandable details.

## Integrity protection

GitHubSP queries [official GitHub asset metadata](https://docs.github.com/en/rest/releases/assets) directly and does not trust proxy-provided digests. When an official SHA-256 digest is available, a match is required before publishing the file. A mismatch fails the task. The UI hides verification badges; integrity failures still explain that the file content is invalid and suggest switching routes and downloading again. Original errors and verification fields remain in internal records.

If no official digest exists or the API is temporarily unavailable, downloads can complete after length, range, and local-part consistency checks; this does not establish authenticity. HTML error pages, truncation, and incorrect lengths fail the download. Hiding the display does not change backend checks or completion criteria.

## Data storage and recovery

The default remains `%LOCALAPPDATA%\com.githubsp.desktop\tasks.sqlite3`. Settings show the actual directory and provide open-directory and backup-export actions. Moving the exe does not move data. Prototype data in `com.githubsp.native-prototype` is retained separately without merging. Use `--data-dir <absolute-directory>` for isolated acceptance.

- Schema v3 preserves task JSON, settings, favorites and resume metadata, with status, normalized search text, queue position and query indexes updated atomically. `rusqlite 0.39.0` bundles SQLite 3.51.3; WAL and `synchronous=FULL` remain enabled.
- v1/v2 migrations first create a verified SQLite Backup API copy at `backups/tasks-v{old}-before-v3-{UUID}.sqlite3`, including committed WAL data. Rows are validated and backfilled transactionally. Failure rolls back; damaged, unknown and newer databases are not replaced by empty ones. Existing v3 history is validated when read rather than fully deserialized at every startup.
- The download working set contains all unfinished tasks plus the most recent 50 completed/cancelled records, ordered by creation time and ID. Full history remains in SQLite. SQL filtering/counting/sorting returns at most 20 rows; only that page receives filesystem checks. Search is a Unicode-lowercased literal substring; percent and underscore are not wildcards.
- Clear-completed affects all completed records, shows the total in confirmation and preserves files. History operations resolve records by ID. Completion notices include their own summary and survive cache eviction.
- Manual backup uses the same consistent Backup API and integrity validation. It publishes a synced temporary copy without overwriting existing files. Backups include app records only, not downloaded files or resume parts. No automatic backup retention/deletion policy is applied.
- One actor owns writes; one background read permit bounds history, ID lookup and backup connections. Cancellation cannot release a permit before its actual worker closes. Shutdown drains reads. Default-directory single-instance locking remains compatible with the former shell.
- Downloads and `.githubsp-{task UUID}` parts stay in their original destinations. Migration does not move or clean them. Resume still validates parts; merging needs additional destination space.

**Offline restore changes the active database; preserve the entire current directory first:**

1. Exit every app version, including tray instances.
2. Move the entire current data directory to a separate retained location, including any WAL/SHM files.
3. Recreate the original directory. Copy only the selected consistent backup there as `tasks.sqlite3`; never mix old WAL/SHM files into it.
4. Start a compatible app and verify records/settings/favorites. Downloads and parts must still be at the recorded paths.

Older apps cannot open v3. Rollback requires the pre-upgrade v1/v2 backup; newer records are not merged back automatically. Initial migration needs extra disk space and time.

This README records durable architecture, commands, and limitations; there is no separate memory system. Version v0.2.4 is the first release of the current native workspace. Older design and trial records remain historical evidence.

The maintained source lives in the root Cargo workspace, `core/`, `native-desktop/`, and `scripts/`. Release assets are under `artifacts/releases/v0.2.4/`; local verification records live in its `audit/` directory and are excluded from source archives. Retired frontend dependencies/output, old Tauri build caches, and current `native-desktop/target/` can be regenerated or are no longer used. Historical archives, releases, screenshots, backups, and delivery records under `artifacts/` and `.cache/` are retained. Never remove either whole directory as a cache; production application data is outside this cleanup scope.

## Development and builds

Requires Rust 1.96+ MSVC and Visual Studio C++ Build Tools. No Node.js build chain remains. The root Cargo workspace owns versions and the single lockfile; Slint is the default member. The wrapper targets Windows x64, enables static CRT only for target code, and uses `native-desktop/target`.

```powershell
.\scripts\native-cargo.ps1 -CargoArguments @('run','--locked')
.\scripts\package-native-windows.ps1
```

Initial dependency retrieval requires network access; cached builds support `-Offline` for packaging or `--offline` in Cargo arguments. Packaging derives `GitHubSP-v<version>-windows-x64.exe` from Cargo, regenerates embedded notices, audits runtime dependencies and PE imports, and writes the exe, SHA-256 and acceptance limits under `artifacts/slint-retirement/package`. Use `-OutputDirectory` to select another directory (relative paths resolve from the repository root); the default is unchanged. It creates no installer or release. The package script compiles the public update repository `lkuliuying/githubsp`; override with `-ReleaseRepository owner/repo`. Ordinary source builds without the compile-time repository report that updates are unconfigured.

## Module boundaries

| Path | Responsibility |
| --- | --- |
| `core/` | UI-independent `githubsp-core` package, retaining `githubsp_lib` exports |
| `core/src/manager.rs`, `manager/route_ux.rs`, `route_policy.rs` | Authoritative actor, serial queue, route/recovery policies and shutdown |
| `core/src/store.rs`, `store/`, `manager/reads.rs` | Schema v3, migration, SQL pagination, backups and bounded read lifecycle |
| `core/src/model.rs`, `history.rs`, `notice.rs` | Contracts, search/file checks and completion-notice state |
| `core/src/engine.rs`, `files.rs`, `source.rs`, `network.rs` | Download/resume, filesystem and transport integrity |
| `core/src/catalog.rs`, `intake.rs`, `preflight.rs`, `library.rs`, `updates.rs` | Releases, batch intake, directory validation, favorites and app updates |
| `native-desktop/` | Slint pages, native dialogs/windows/tray/single-instance behavior and typed UI adapters |
| `scripts/`, `core/examples/` | Builds, packaging, notices and isolated verification |

Slint 1.18.1 + Winit + FemtoVG-WGPU uses DX12. UI runs on the main thread; downloads and reads run in the background. Progress refreshes coalesce to four per second, completion/errors update immediately, and hidden windows stop ordinary refreshes. Software rendering is only a test backend.

`Snapshot.tasks` is the download working set; `total_tasks`/`completed_tasks` cover the database. `history_revision` changes for history fields, not ordinary progress. Snapshot/queue revisions and page epochs reject stale work. `Manager::task`, `history`, `remove_completed`, and `export_backup` support records beyond the cache, with confirmation revisions for bulk deletion. Notifications carry file/time summaries. Tauri IPC and WebView notice windows have been removed.

Resources and third-party notices are embedded, including Noto Sans SC and the required Slint `AboutSlint` UI. Saved Phosphor SVGs require no npm export step. The pre-retirement source archive includes uncommitted work; historical visual evidence in `design-qa.md` is explicitly from the former UI.

## Automated verification

```powershell
.\scripts\native-cargo.ps1 -CargoArguments @('test','--workspace','--locked','--offline')
.\scripts\native-cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings')
cargo fmt --all -- --check
.\scripts\package-native-windows.ps1 -Offline
.\scripts\measure-history.ps1 -Offline
.\scripts\test-native-startup.ps1 -Executable .\artifacts\slint-retirement\package\GitHubSP-v0.2.4-windows-x64.exe
```

Core fixtures exercise pause/resume, cancellation, recovery, switching, hashes, filename protection and saved shutdown. Storage fixtures cover v1/v2, WAL backups, corruption/newer versions, capacity exhaustion, rollback, Unicode/literal search, stable ties, pagination, cold-record operations and cleanup. Native tests retain directory, duplicate-action, stale-result, update pagination and safe Markdown coverage. Slint tests use isolated data, a loopback catalog fixture, and the software backend to cover module navigation, drafts, previews, submission-result rendering, stale responses, and failed-save preservation. Snapshots cover 1448×1086, 1040×740, and 720×520, written under `artifacts/style-abc-review/ui-renders/`, with additional full-count, split-layout, numeric-limit, modal keyboard, license-pagination and completion-window checks.

The test-only `GITHUBSP_UI_RENDER_DIRECTORY` variable overrides the screenshot directory. This revision writes to `artifacts/download-layout-review/ui-renders/` and checks selection totals across versions, failed pagination, cleared diagnostic summaries while probing, historical results and long errors. Production behavior does not use this variable.

History scaling uses separate processes with 1k/10k/100k completed records and three unfinished tasks, asserting 53 resident tasks and at most 20 rows/page. Reports include core startup/query timing and Windows private bytes; they exclude Slint/GPU and a legacy WebView comparison. Full-process-tree idle/download/tray/notice measurements, the proposed 30% memory reduction, clean Windows offline startup, IME, DPI and multi-monitor checks remain separate acceptance work.

`test-native-startup.ps1` uses a new Unicode data path to inspect the native window, loaded modules, second-instance exit and graceful shutdown. It only owns its created processes and does not access production data. This is a developer-machine smoke test, not a clean-system or offline check.

The retained `verify_download`, `benchmark_routes` and `verify_v2` examples run through `native-cargo.ps1` with `-p githubsp-core --example <name>`. They perform real network activity only when explicitly invoked and require isolated output paths. `verify_v2` retains its historical name but copies a v1 fixture via the Backup API and migrates that copy to v3. Never run migration experiments against production data.

## Known limitations

Public routes vary with network conditions and service policies. Short probes cannot guarantee sustained whole-file throughput, and direct GitHub may be fastest. Without an official digest, authenticity is not established. Downloaded files are never started automatically. The unauthenticated official API may be rate-limited; standard direct asset URLs may still work when catalog browsing is unavailable.

This executable has no Windows Authenticode signature, so Windows may report an unknown publisher. Technology and license badges do not change that. Clean Windows offline startup, IME, 100%/150%/200% DPI, multiple monitors, DX12 drivers, tray/notice focus behavior, live downloads and full process-tree resource targets require separate acceptance.

## License

Project code is provided under the [MIT License](LICENSE). Third-party dependencies retain their respective licenses.
