# GitHubSP

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![Vue 3](https://img.shields.io/badge/Vue-3-4FC08D?logo=vuedotjs&logoColor=white)](https://vuejs.org/)

[简体中文](#中文) · [English](#english) · [下载 / Downloads](https://github.com/lkuliuying/githubsp/releases/latest) · [问题反馈 / Issues](https://github.com/lkuliuying/githubsp/issues)

<a id="中文"></a>

## 简体中文

面向 Windows 的 GitHub Release 附件下载工具。基于 Tauri 2 + Vue 3 + Rust + SQLite，提供项目附件选择、批量下载、托盘、限速、历史检索、收藏和版本检查。v0.2.0 首次公开发布包含当前四页界面重构，v0.2.1 支持确认后创建缺失的保存目录，v0.2.2 修复版本分页时的加载闪烁和布局跳动；当前正式版 v0.2.3 增加线路自动恢复与低速换线建议、后台完成提醒和耗时统计，并支持 Markdown 更新说明。应用界面为简体中文，默认串行下载；本文档提供中英双语说明。

顶部三个徽章分别说明 MIT 许可证及 Tauri 2、Vue 3 技术栈，使用无需申请的 [Shields.io 徽章](https://shields.io/badges/static-badge)，不代表安全认证或 Windows 代码签名。

## 使用

从 [GitHub Releases](https://github.com/lkuliuying/githubsp/releases/latest) 获取新版。v0.2.3 附件：

| 文件 | 用途 |
| --- | --- |
| [GitHubSP-v0.2.3-windows-x64.exe](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/GitHubSP-v0.2.3-windows-x64.exe) | Windows x64 免安装程序 |
| [GitHubSP-v0.2.3-source.zip](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/GitHubSP-v0.2.3-source.zip) | 对应发布标签的源码，包含 npm/Cargo 锁文件及 LICENSE |
| [SHA256SUMS.txt](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/SHA256SUMS.txt) | exe、源码包和许可证文件的 SHA-256 校验值 |
| [LICENSE](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/LICENSE) | 随分发提供的 MIT 许可证 |
| [THIRD_PARTY_NOTICES.txt](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/THIRD_PARTY_NOTICES.txt) | Markdown 渲染依赖的第三方许可声明 |

1. 下载 exe 和 `SHA256SUMS.txt`，用下方命令计算摘要，并与校验文件中对应文件名的一行比较；不一致时不要运行。
2. 若旧版正在运行，先从托盘菜单选择“保存进度并退出”，再双击新版，避免单实例机制激活旧进程。
3. 粘贴公开 GitHub 附件、仓库或版本链接，输入或选择保存目录并确认下载。手动输入的目录不存在时，提交后会询问是否新建，确认后继续原操作。完成后可打开所在目录；程序不会运行下载的文件。

```powershell
Get-FileHash .\GitHubSP-v0.2.3-windows-x64.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

新版沿用原应用标识和数据目录。旧版及 UI 试用产物保留在维护者本地 `artifacts/`；该目录不提交到源码仓库。本次公开发布文件单独保存在 `artifacts/releases/v0.2.3/`。

支持 Windows 10/11 x64，需要系统已安装 [WebView2 Runtime](https://v2.tauri.app/reference/webview-versions/)。便携程序不需要安装 GitHubSP，但运行数据仍保存在用户本地应用数据目录。当前构建未进行代码签名。

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
- **手动线路检测**：无需先创建任务。输入框留空或只有空白时，使用本项目公开发布的 `GitHubSP-v0.2.2-windows-x64.exe`；填写附件直链则检测指定附件，仓库或版本页链接会提示改用附件直链或清空输入。每轮按顺序检测内置线路，每条最多读取 524,288 字节（约 524 KB）、等待 10 秒，不保存或执行文件。一次只允许一轮手动检测；下载、队列接续、暂停和取消可以同时进行，检测不自动切换正在下载的线路。检测流量不受下载限速控制，可能短暂影响实时速度，结果仅供参考；默认文件不可用时可填写其他附件直链重试。面板显示来源和真实文件名，逐轮替换结果；手动检测前及期间启动的下载自动检测不会覆盖本轮展示，之后新启动的下载可更新结果。旧缓存只标为历史检测结果，不推断来源。异常或超时后可再次检测，退出会取消未完成的检测。
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

四个页面沿用本地 `view` 状态切换，采用浅蓝背景、白色面板、绿色主操作和 Phosphor 图标，不新增路由或 UI 依赖。

- **下载工作台**：顶部新建下载，宽窗口中间为版本/附件选择与线路检测双栏，底部为任务表格。任务的更多按钮展开来源、目录、换线及队列排序。全部继续只处理暂停/失败任务，全部暂停处理活动、排队和等待网络恢复任务；批量操作遇到失败即停止并显示错误。清空已完成需要确认，仅移除记录。
- **版本分页**：加载提示固定在附件面板标题栏右侧，空闲时保留占位。请求期间保留旧版本、附件和页码，成功后更新，失败保留原页并提示错误。临时等待的控件保持原有明暗且禁止重复操作；页码边界、无附件选择等原本不可用的控件仍灰显。加载提示出现或消失不推动面板和下载队列，不同页实际内容高度仍按现有布局变化。
- **历史**：统一关键词搜索和状态筛选、统计卡、独立滚动的记录表格与页码跳转。匹配记录数来自服务端，其余统计只代表当前页；每页仍为 20 条。删除末页最后一条记录后回到有效页。
- **收藏**：项目卡片和列表两种布局，可按检查结果筛选，按检查时间或项目名称排序。统计与标签使用真实的检查成功、失败、待检查状态，不推断“今日更新”或已安装版本。
- **设置**：限速、窗口行为、收藏检查和软件更新分区。限速输入为 0 或 0.001024–10240 MB/s；滑块用于 0–100 MB/s 的快捷调整，步长 0.25 MB/s，更高值通过输入框设置。现有 IPC 和存储仍使用整数 `limitKib`，未编辑限速时原值保存；新输入换算到最近的整数 KiB/s，保存成功回显实际值。恢复默认只修改表单，点击保存后才生效；收藏自动检查间隔仍固定为 6 小时。
- **适配与状态**：窄窗口将主要分区堆叠，宽表格在面板内滚动；保留后端未就绪、加载、空记录、文件不可访问和错误状态。列表、历史及附件选择不显示常态校验标记；历史统计按任务状态显示本页下载失败数，收尾阶段显示“正在完成下载”。浏览器入口仍禁用真实下载和系统目录操作。

参考图中的示例版本、地区线路、延迟、并发设置及自动生成源码包不代表现有产品能力。软件版本为 0.2.3，窗口按钮由 Tauri 原生标题栏提供。历史视觉核对见 [design-qa.md](design-qa.md)，其中截图和日志属于维护者本地验收资料，不随源码分发；隔离浏览器验收只提供前端证据，不等同于 Windows 原生或真实网络下载验收。

## GitHubSP 自身更新源

本仓库发布的 v0.2.3 便携程序在构建时配置官方发布源 `lkuliuying/githubsp`。源码自行构建时，若未设置 `GITHUBSP_RELEASE_REPOSITORY`，点击检查仍会显示“尚未配置官方发布源”，不会发送更新请求或声称已是最新版。该变量是公开仓库标识而非密钥，只在编译时读取；`build.rs` 跟踪其变化。构建官方发布配置：

```powershell
$previousReleaseRepository = $env:GITHUBSP_RELEASE_REPOSITORY
try {
    $env:GITHUBSP_RELEASE_REPOSITORY = 'lkuliuying/githubsp'
    npm run tauri -- build --no-bundle --ci -- --locked
    if ($LASTEXITCODE -ne 0) { throw 'GitHubSP build failed' }
} finally {
    $env:GITHUBSP_RELEASE_REPOSITORY = $previousReleaseRepository
}
```

只读取最新正式 Release，按语义化版本比较并接受 `v` 前缀；更新说明通过 `markdown-it` 渲染标题、列表、强调、代码、引用和表格，保留原有面板和滚动区域。原始 HTML 作为文本显示，图片只展示替代文字，不自动请求远程资源；不提供代码高亮或额外 Markdown 插件。说明中的有效 HTTPS GitHub 链接（包括 `compare` 页面）由用户点击后通过独立的 `open_update_link` 接口在系统浏览器打开；前后端拒绝站外地址、非 HTTPS、凭据、非标准端口及非法地址，其他链接保留不可点击文字。打开失败保留说明正文并允许重试。官方发布页按钮继续使用原有 `open_release` 校验规则。不下载覆盖本程序、不执行安装。测试用可控服务验证未配置、比较及限流逻辑；公开 Release 的可访问性与桌面界面实际点击验收是不同的验证范围。

## 下载规则

- 默认串行处理任务。新任务自动排队；退出应用会停止写入并保存进度，重启后恢复为暂停状态，由用户点击继续。
- 候选线路为 GitHub 直连、[GH-Proxy](https://gh-proxy.com/docs/github-accelerator)（`gh-proxy.org`）和 [ghproxy.net](https://ghproxy.net/)。线路集中定义在 `src-tauri/src/network.rs`；公共加速服务会收到公开附件 URL。当前网络客户端不读取系统代理环境变量。
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

- 桌面任务数据库：`%LOCALAPPDATA%\com.githubsp.desktop\tasks.sqlite3`，保存任务、恢复信息和上次目录，使用 SQLite WAL 和事务。
- 数据库结构版本为 2，新增收藏及可兼容的任务字段、设置、队列位置。升级 v0.1.0 前通过 SQLite 备份 API 保存同目录 `tasks-v1-backup-{UUID}.sqlite3`，随后在事务中迁移；迁移失败回滚。备份包含已提交的 WAL 内容。不要让旧程序打开已升级数据库；需要回退时先退出新版，在另存升级数据库后由用户恢复 v1 备份。
- 每个任务的临时目录：目标下载目录下 `.githubsp-{任务 UUID}`。任务专用标记限制清理范围，临时目录和分片拒绝符号链接。运行过程中不要移动或修改临时文件。
- 合并时同时保留分片和待发布文件，因此目标磁盘应预留约两倍附件大小的空间。
- 应用采用单实例。异常退出恢复后，用户手动继续；只有能确认资源一致时复用已保存进度。

本项目没有独立项目记忆文件，长期可复用的结构、命令、发布配置和行为以本 README 为准。源码维护在 [main 分支](https://github.com/lkuliuying/githubsp/tree/main)，发布产物通过 GitHub Releases 分发。

## 开发与构建

需要 Node.js 22.12+（本次使用 24.14）、Rust 1.96+ 的 MSVC 工具链、Visual Studio C++ Build Tools 和 WebView2。源码交付目录包含 npm 和 Cargo 锁文件；安装会读取这些锁文件，不需要配置密钥。

```powershell
npm ci
npm run tauri -- dev
```

`npm run dev` 仅提供浏览器界面预览，下载和目录选择按钮会明确禁用；真实下载通过桌面程序调用 Rust，不依赖本地 HTTP 后端。

```powershell
npm run tauri -- build --no-bundle --ci -- --locked
```

构建产物为 `src-tauri/target/release/githubsp.exe`，本次分发文件名为 `GitHubSP-v0.2.3-windows-x64.exe`。`--no-bundle` 不生成安装程序。若开发环境强制 npm 离线模式，可对安装命令显式增加 `--offline=false --registry=https://registry.npmjs.org`，无需修改全局配置。

发布构建重新构建前端并锁定 Cargo 依赖，随后核对 PE 架构、版本和 SHA-256；依赖已缓存时可在 Cargo 参数末尾追加 `--offline`。发布源码包从对应 Git 标签导出，不包含依赖目录、缓存、个人任务数据库或测试产物。打包验证不执行安装，也不读取用户任务库。

## 模块边界

| 位置 | 职责 |
| --- | --- |
| `src/App.vue`、`src/components/TaskRow.vue` | 四页导航、下载输入、表格队列、批量操作和任务详情 |
| `src/composables/useDownloads.ts` | 订阅快照、状态更新、过期事件处理和资源释放 |
| `src/services/downloads.ts`、`src/types.ts` | 类型化 Tauri 命令与事件边界 |
| `src-tauri/src/app.rs` | 桌面生命周期、目录操作、单实例及 IPC |
| `src-tauri/src/manager.rs` | 权威任务状态、串行队列、取消与关闭协调 |
| `src-tauri/src/manager/route_ux.rs`、`src-tauri/src/route_policy.rs`、`src/components/RouteFeedback.vue` | Rust 恢复预算、低速与换线策略；Vue 展示反馈及确认操作 |
| `src-tauri/src/store.rs`、`model.rs` | SQLite 持久化和任务/恢复数据模型 |
| `src-tauri/src/source.rs`、`network.rs` | 链接校验、官方元数据、线路、响应校验 |
| `src-tauri/src/engine.rs`、`files.rs` | 分段传输、恢复、文件边界和最终校验 |
| `src/components/SourcePicker.vue`、`RouteControls.vue` | 附件、多行链接、批量确认、线路操作 |
| `src/components/RouteDiagnostics.vue` | 真实内置线路检测结果、未检测/失败状态及手动检测入口 |
| `src/components/HistoryView.vue`、`FavoritesView.vue`、`SettingsView.vue`、`UpdateView.vue` | 历史、收藏、设置、自身更新入口 |
| `src/components/UpdateNotes.vue`、`src/updateNotes.ts` | 更新说明 Markdown 渲染与链接交互；系统打开前由 Rust 再次校验 |
| `src-tauri/src/catalog.rs`、`intake.rs`、`preflight.rs` | 官方资源、缓存/退避、批量边界与目录空间检查 |
| `src-tauri/src/limiter.rs`、`history.rs`、`desktop.rs` | 共享限速、历史页文件检查、托盘与桌面生命周期 |
| `src-tauri/src/library.rs`、`updates.rs` | 收藏检查调度及独立的软件版本比较 |
| `src-tauri/src/completion.rs`、`src/components/DownloadNotice.vue`、`src/services/notices.ts` | 完成提醒去重、倒计时、独立窗口、交互和定向事件；Windows 显示适配复用现有 `windows-sys` |

命令为 `list_tasks`、`create_task`、`task_action`（暂停、继续、取消、移除记录）、`open_directory`。`downloads-changed` 发送包含递增版本号的任务快照；快照携带 ID、状态、字节数、速度、线路与校验结果。Vue 不直接执行网络或文件操作。

保留以上命令，新增 `browse_releases`、`preview_batch`、`create_batch`、`change_route`、`diagnose_routes`、`save_settings`、`reorder_queue`、`query_history`、`acknowledge_notices`、`hide_to_tray`、`add_favorite`、`remove_favorite`、`check_favorites`、`check_app_update`、`open_release`、`open_update_link`。Rust actor 持有权威任务、收藏、设置状态；队列有独立版本号，前端拒绝过期快照。IPC、目录选择和剪贴板写入都经过类型化服务。

`diagnose_routes(url)` 保持原接口，默认检测地址由 Rust 管理。actor 持有手动检测工作任务和回复通道，统一释放忙碌状态并在退出时取消；内部检测序号隔离手动与自动结果，不影响下载引擎的选线。快照兼容增加可选的 `diagnosticContext`，包含 `source`（`default`／`input`／`download`）和 `filename`。检测结果继续保存在原有 `diagnostics` JSON 中，不修改表结构；上下文仅存在本次会话，重启恢复的缓存显示为历史结果。

线路体验快照增加可选的 `retryInfo`、`recoveryInfo`、`routeSuggestion` 和默认空的 `routeFailures`，旧任务使用默认值，不修改数据库表结构。`waiting_network` 单独计数，不属于正在传输；会话计时和建议在重启时清空。新增 `apply_route_suggestion(id, suggestionId)` 和 `dismiss_route_suggestion(id, suggestionId)`，后端拒绝过期及重复确认；立即重试复用 `task_action` 的继续操作。

目录操作另有 `inspect_directory(directory)` 和 `create_directory(directory)`：前者只读，返回规范化的绝对路径及 `existing`／`missing` 状态；后者仅由用户确认触发，递归创建并检查可写性后返回目录路径。权限、文件占位和不可访问磁盘等错误不作为可创建的缺失目录返回。原有任务创建、批量和传输接口仍保留目录预检查。

任务 JSON 兼容增加 `elapsedMs`（可空）和 `elapsedIsPartial`（默认 `false`）；设置增加 `backgroundCompletionNotice`（默认 `true`），数据库结构版本仍为 2。通知携带 `kind` 和 `taskId`。`downloads-changed` 仅发往主窗口；独立提醒窗使用 `download-notice-changed` 及 `read_download_notice`、`present_download_notice`、`dismiss_download_notice`、`hover_download_notice`、`open_download_notice` 命令。前端先订阅再读取，渲染确认后计时；提醒窗口仅获事件订阅权限，Rust 命令入口另按来源窗口限制，不能从提醒窗创建任务、改目录或保存设置。

## 自动化验证

```powershell
npm run typecheck
npm run test -- --run
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Rust 测试使用本地可控 HTTP 服务覆盖 URL/路径、200/206/416、错误范围、不支持 Range、断流、超时、限流、不允许的重定向、HTML 页面、摘要不匹配、分段、队列、暂停/继续、活动取消、重启恢复、换线、文件锁和异常退出后的发布恢复。Vue 测试覆盖提交去重、目录错误、过期事件、重试及完整性失败提示；`src/redesign.test.ts` 另覆盖批量操作失败/卸载中止、任务详情、版本切换、版本分页等待/去重/失败重试/空页/末页/过期返回、历史分页与统计、收藏筛选和设置草稿。分页加载的布局稳定性需另用浏览器检查，jsdom 不提供真实布局尺寸。测试使用隔离临时目录，不运行下载文件。

提醒和耗时测试另外覆盖前台消费、后台去重、合并、开关、过期回调、悬停与键盘焦点倒计时、订阅/读取竞态、IPC 隔离、耗时累计与冻结、无进度保存、正常退出和异常恢复、旧记录兼容。展示测试覆盖校验标签隐藏及错误保留、平均速度的有效性边界、十进制单位、限速换算和旧值保留。多显示器、任务栏位置、实际不抢焦点以及 100%／150%／200% 缩放仍需 Windows 原生验收，状态测试及浏览器渲染不能替代。

线路检测测试覆盖空白输入、指定附件、无效链接、并发传输与队列接续、暂停/继续/取消、重复请求、调用方中止、超时与部分/全部失败、工作任务异常、退出取消、过期结果及整轮替换、旧缓存来源兼容。前端另验证按钮状态、错误重试、实际文件名和 MB/s 展示；浏览器布局与 Windows 桌面按钮操作仍需分别验收。

线路体验测试使用可注入策略和单调时间参数，覆盖恢复成功/耗尽、冷却、让位与预算冻结、多等待任务、暂停/取消/退出/重启、速度窗口、低速排除条件、收益门槛、候选失效、重复确认及保持自动模式；前端覆盖倒计时、进度损失确认、卸载清理和错误反馈。测试加速恢复时钟，不实际等待 5 分钟。

真实网络验收可按需执行以下命令；完整下载会保存指定附件，测速每条线路取同一个 8,388,608 字节（约 8.4 MB）区间，三轮轮换顺序，每次最多等待 30 秒。

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --example verify_download -- "https://github.com/rakanki911/DLSS5-Swapper/releases/download/v2.2.9/DLSS5-Swapper-Setup-2.2.9.exe" "artifacts/acceptance/下载验证"
cargo run --manifest-path src-tauri/Cargo.toml --example benchmark_routes -- "https://github.com/rakanki911/DLSS5-Swapper/releases/download/v2.2.9/DLSS5-Swapper-Setup-2.2.9.exe"
```

`verify_download` 复用桌面下载核心，在验收目录创建独立的 `verification.sqlite3`，不会改动桌面任务库。历史结果保留在维护者本地 `artifacts/acceptance-v0.2.0/` 和 `artifacts/acceptance/`，不属于仓库内容。此外，已有 v1 测试数据库时，`verify_v2` 可读取其一致副本，验证迁移和从官方目录选择两个小附件进行批量下载；请将最后一个参数替换为实际的 v1 测试数据库路径：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --example verify_v2 -- "https://github.com/cli/cli" "artifacts/acceptance-v0.2.0/批量与迁移" "artifacts/acceptance/下载验证/verification.sqlite3"
```

验收目录必须全新；原数据库只读，迁移写入仅发生在副本。测试资产、浏览器截图和构建缓存不属于源码。浏览器布局验证、Rust 下载验证与 Windows 原生点击验证分别记录，不能相互替代。Release 更新说明面向用户，描述功能变化、下载与使用方法，以及影响使用的限制和兼容性提醒。测试数量、构建检查、验收过程及未执行测试等开发验证结果单独保留在维护者验收记录中，不写入更新说明。

## 已知限制

公共线路的可用性和速度随网络与服务策略变化，短区间测速不能保证整个文件的持续速度，直连也可能最快。没有官方摘要时无法证明附件真实性；下载完成不会自动启动任何文件。免费官方 API 可能限流，目录不可用时标准直链仍可尝试下载。

当前发布程序没有 Windows Authenticode 代码签名，Windows 可能显示发布者未知。MIT、Tauri 2 和 Vue 3 徽章不改变这一状态。未承诺覆盖全部 Windows 10/11 系统版本、缩放及 WebView2 环境；本次构建检查不代表原生界面、托盘、系统目录选择或完整真实网络下载已经重新验收。

## 许可证

项目代码按 [MIT License](LICENSE) 提供。第三方依赖继续受各自许可证约束。

---

<a id="english"></a>

## English

GitHubSP is a Windows desktop downloader for public GitHub Release assets, built with Tauri 2, Vue 3, Rust, and SQLite. It provides release browsing, asset selection, batch downloads, a system tray, bandwidth limits, searchable history, repository favorites, and update checks. The first public release, v0.2.0, introduced the current four-page interface redesign; v0.2.1 added confirmation before creating missing destination directories. v0.2.2 fixed flicker and layout shifts while loading release pages. The current stable release, v0.2.3, adds automatic route recovery, low-speed route suggestions, background completion notices, elapsed-time statistics, and Markdown release notes. The application UI is in Simplified Chinese; this README is bilingual. Downloads run sequentially by default.

The three badges identify the MIT license and the Tauri 2 / Vue 3 technology stack. They use [Shields.io static badges](https://shields.io/badges/static-badge), which require no application or approval. They do not certify security or provide Windows code signing.

## Quick start

Download the current version from [GitHub Releases](https://github.com/lkuliuying/githubsp/releases/latest). Assets for v0.2.3:

| File | Purpose |
| --- | --- |
| [GitHubSP-v0.2.3-windows-x64.exe](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/GitHubSP-v0.2.3-windows-x64.exe) | Portable Windows x64 application |
| [GitHubSP-v0.2.3-source.zip](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/GitHubSP-v0.2.3-source.zip) | Source from the release tag, including npm/Cargo lockfiles and LICENSE |
| [SHA256SUMS.txt](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/SHA256SUMS.txt) | SHA-256 checksums for the executable, source archive, and license files |
| [LICENSE](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/LICENSE) | MIT license supplied with the distribution |
| [THIRD_PARTY_NOTICES.txt](https://github.com/lkuliuying/githubsp/releases/download/v0.2.3/THIRD_PARTY_NOTICES.txt) | Third-party license notices for Markdown rendering dependencies |

1. Download the executable and `SHA256SUMS.txt`. Run the commands below and compare the hash with the line for that filename. Do not run the executable if they differ.
2. If an older instance is running, select “保存进度并退出” (save progress and exit) from its tray menu before opening the new executable. Otherwise, single-instance handling will activate the older process.
3. Paste a public GitHub asset, repository, or release URL, enter or choose a destination directory, and confirm the download. If a manually entered directory is missing, submitting asks whether to create it before continuing. You can open the destination directory when it finishes; GitHubSP does not execute downloaded files.

```powershell
Get-FileHash .\GitHubSP-v0.2.3-windows-x64.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

The release keeps the existing application identifier and data directory. Older builds and UI trial packages remain in the maintainer's local `artifacts/` directory, which is excluded from Git. Files for this public release are stored separately under `artifacts/releases/v0.2.3/`.

Requires Windows 10/11 x64 and an installed [WebView2 Runtime](https://v2.tauri.app/reference/webview-versions/). GitHubSP itself needs no installation, but runtime data is stored in the user's local application data directory. The executable is not code-signed.

Supported URL formats:

```text
https://github.com/{owner}/{repo}/releases/download/{tag}/{filename}
https://github.com/{owner}/{repo}/releases/latest/download/{filename}
https://github.com/{owner}/{repo}
https://github.com/{owner}/{repo}/releases
https://github.com/{owner}/{repo}/releases/latest
https://github.com/{owner}/{repo}/releases/tag/{tag}
```

URL-encoded Chinese characters and spaces in filenames are supported. Other domains, non-HTTPS URLs, embedded credentials, query strings, fragments, path traversal, and invalid Windows filenames are rejected. Repository browsing defaults to the latest stable release, supports pagination, and can explicitly include prereleases. Platform and installer hints are inferred from filenames; users must confirm compatibility.

Fixed asset URLs can be added directly. Repository and release URLs open the asset picker. Multiple URLs or selected assets are previewed before confirmation, with 1–100 items per batch; invalid or failed items remain available for correction. Latest-asset links are resolved to fixed release URLs. An unfinished task with the same canonical URL and normalized directory must be resumed; completed files are preserved and a new filename is chosen. Favorite repositories are deduplicated case-insensitively.

Private repositories, GitHub-generated source archives, Git clone, custom proxies, and automatic update installation are not supported. Source archives uploaded as regular public Release assets can be downloaded normally.

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

The four pages use local `view` state without an additional router or UI dependency. Shared styles use a light-blue background, white panels, green primary actions, and Phosphor icons.

- **Download workspace:** creation controls at the top, release/asset selection and diagnostics side by side on wide windows, then the task table. Task details expose source, directory, route, and queue controls. Resume-all handles paused/failed tasks; pause-all handles active/waiting tasks. Bulk actions stop and report the first error. Clearing completed records requires confirmation and preserves files.
- **Release pagination:** loading feedback occupies a reserved slot on the right of the asset-panel heading, including while idle. Requests retain the previous releases, assets, and page number until success; failures keep that page and show an error. Temporarily locked controls retain their normal appearance while preventing duplicate actions; controls already unavailable because of page boundaries or empty selections remain subdued. Showing or hiding the loading indicator does not move the panel or task queue. Different page contents can still change height through the existing layout.
- **History:** shared search and status filters, summary cards, an independently scrollable table, and page navigation. The matched count comes from the backend; other statistics cover only the current page. Pages contain 20 records. Removing the last record on the last page returns to a valid page.
- **Favorites:** card/list layouts, result filters, and sorting by last check or repository name. Labels show actual successful, failed, or pending checks without inferring updates “today” or installed versions.
- **Settings:** bandwidth, window behavior, favorite checks, and software updates. Limit input accepts 0 or 0.001024–10240 MB/s. The slider covers 0–100 MB/s in steps of 0.25 MB/s; higher limits use numeric entry. IPC and storage retain integer `limitKib`: unedited limits are preserved exactly, while new input is rounded to the nearest KiB/s and the saved value is displayed. Restoring defaults changes the draft only until saved. Automatic favorite checks retain the fixed six-hour interval.
- **Responsive and error states:** narrow windows stack sections and wide tables scroll inside their panels. Backend-not-ready, loading, empty, inaccessible-file, and error states remain visible. Download lists, history, and asset selection hide routine verification badges. History counts failed downloads by task status, and finalization displays “正在完成下载”. Browser preview disables real downloads and system-directory operations.

Reference images are not promises of extra versions, regional routes, latency metrics, parallel task settings, or support for generated source archives. The app version is 0.2.3, and window controls are provided by Tauri's native title bar. [design-qa.md](design-qa.md) contains the historical visual review in Chinese. Its screenshots and logs are local maintainer evidence, not distributed source. Isolated browser checks establish frontend behavior, not native Windows or real-network acceptance.

## Application update source

The v0.2.3 portable executable published by this repository is built with `GITHUBSP_RELEASE_REPOSITORY=lkuliuying/githubsp`. For a source build without that variable, checking updates reports that no official source is configured, sends no update request, and does not claim the app is current. The value is a public repository identifier, not a secret. It is read at compile time, and `build.rs` tracks changes. To build with the official release source:

```powershell
$previousReleaseRepository = $env:GITHUBSP_RELEASE_REPOSITORY
try {
    $env:GITHUBSP_RELEASE_REPOSITORY = 'lkuliuying/githubsp'
    npm run tauri -- build --no-bundle --ci -- --locked
    if ($LASTEXITCODE -ne 0) { throw 'GitHubSP build failed' }
} finally {
    $env:GITHUBSP_RELEASE_REPOSITORY = $previousReleaseRepository
}
```

Only the latest stable Release is checked. Versions follow semantic versioning and may have a `v` prefix. Release notes use `markdown-it` to render headings, lists, emphasis, code, quotations, and tables within the existing panel and scroll area. Raw HTML is shown as text; images display alternative text without requesting remote resources. No syntax highlighter or extra Markdown plugins are included. Clicking a valid HTTPS GitHub link, including a `compare` page, opens the system browser through the separate `open_update_link` command. Frontend and backend reject external domains, non-HTTPS URLs, credentials, nonstandard ports, and malformed addresses; other links remain non-clickable text. Opening failures preserve the notes and allow retry. The official release-page button retains the existing `open_release` validation. The app does not replace its executable or install updates. Controlled-server tests cover missing configuration, comparison, and rate limiting. Public Release availability and actually clicking the native update UI are separate verification scopes.

## Download rules

- Tasks are sequential by default, and new tasks queue automatically. Exiting stops writes and saves progress. On restart, tasks recover paused and require explicit continuation.
- Built-in routes are direct GitHub, [GH-Proxy](https://gh-proxy.com/docs/github-accelerator) through `gh-proxy.org`, and [ghproxy.net](https://ghproxy.net/). Definitions live in `src-tauri/src/network.rs`. Public acceleration services receive the public asset URL. The network client does not read system proxy environment variables.
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

- Desktop database: `%LOCALAPPDATA%\com.githubsp.desktop\tasks.sqlite3`, containing tasks, recovery metadata, and the last directory. SQLite WAL and transactions are used.
- Database schema version 2 adds favorites, compatible task fields, settings, and queue positions. Before migrating v0.1.0 data, SQLite's backup API creates `tasks-v1-backup-{UUID}.sqlite3` beside the database, including committed WAL data. Migration is transactional and rolls back on failure. Do not open an upgraded database with the older app. To roll back, exit the new app, preserve a separate copy of the upgraded database, and restore the v1 backup manually.
- Task temporary directories live under the destination as `.githubsp-{task UUID}`. Ownership markers limit cleanup; symbolic links are rejected for temporary directories and parts. Do not move or edit these files during operation.
- Merging retains both parts and the pending final file; allow roughly twice the asset size on the destination disk.
- The app permits one instance. Recovery after an unexpected exit requires manual continuation and reuses progress only when resource identity can be established.

There is no separate project-memory file. This README records durable architecture, commands, release configuration, and behavior. Source is maintained on [main](https://github.com/lkuliuying/githubsp/tree/main); binaries are distributed through GitHub Releases.

## Development and builds

Requires Node.js 22.12+ (the current build uses 24.14), Rust 1.96+ with the MSVC toolchain, Visual Studio C++ Build Tools, and WebView2. npm and Cargo lockfiles are included. Installing project dependencies does not require credentials.

```powershell
npm ci
npm run tauri -- dev
```

`npm run dev` provides only a browser preview. Real downloads and directory selection are disabled there. Desktop operations call Rust through Tauri and do not require a local HTTP backend.

```powershell
npm run tauri -- build --no-bundle --ci -- --locked
```

The output is `src-tauri/target/release/githubsp.exe`, distributed for this version as `GitHubSP-v0.2.3-windows-x64.exe`. `--no-bundle` does not create an installer. If your environment forces npm offline, add `--offline=false --registry=https://registry.npmjs.org` to the dependency-install command without changing global configuration.

Release builds rebuild the frontend and retain locked Cargo dependencies, then check PE architecture, version, and SHA-256. When dependencies are cached, append `--offline` to the Cargo arguments. Release source archives are exported from the matching Git tag, excluding dependencies, caches, personal databases, and test artifacts. Packaging checks neither install the app nor access the user's task database.

## Module boundaries

| Location | Responsibility |
| --- | --- |
| `src/App.vue`, `src/components/TaskRow.vue` | Navigation, download input, queue table, bulk actions, and task details |
| `src/composables/useDownloads.ts` | Snapshot subscriptions, state updates, stale-event rejection, and cleanup |
| `src/services/downloads.ts`, `src/types.ts` | Typed Tauri commands and events |
| `src-tauri/src/app.rs` | Desktop lifecycle, directory operations, single instance, and IPC |
| `src-tauri/src/manager.rs` | Authoritative task state, serial queue, cancellation, and shutdown coordination |
| `src-tauri/src/manager/route_ux.rs`, `src-tauri/src/route_policy.rs`, `src/components/RouteFeedback.vue` | Rust recovery budgets and route policies; Vue feedback and confirmation |
| `src-tauri/src/store.rs`, `src-tauri/src/model.rs` | SQLite persistence and task/recovery models |
| `src-tauri/src/source.rs`, `src-tauri/src/network.rs` | URL validation, official metadata, routes, and response checks |
| `src-tauri/src/engine.rs`, `src-tauri/src/files.rs` | Segmented transfer, resume, filesystem boundaries, and final verification |
| `src/components/SourcePicker.vue`, `src/components/RouteControls.vue` | Asset selection, multiline URLs, batch confirmation, and route controls |
| `src/components/RouteDiagnostics.vue` | Built-in route results, pending/failure states, and manual checks |
| `src/components/HistoryView.vue`, `src/components/FavoritesView.vue`, `src/components/SettingsView.vue`, `src/components/UpdateView.vue` | History, favorites, settings, and application-update UI |
| `src/components/UpdateNotes.vue`, `src/updateNotes.ts` | Release-note Markdown rendering and link interactions; Rust revalidates links before opening the browser |
| `src-tauri/src/catalog.rs`, `src-tauri/src/intake.rs`, `src-tauri/src/preflight.rs` | Official catalog, caching/backoff, batch boundaries, and directory/space checks |
| `src-tauri/src/limiter.rs`, `src-tauri/src/history.rs`, `src-tauri/src/desktop.rs` | Shared bandwidth limit, history file checks, tray, and desktop lifecycle |
| `src-tauri/src/library.rs`, `src-tauri/src/updates.rs` | Favorite-check scheduling and application version comparison |
| `src-tauri/src/completion.rs`, `src/components/DownloadNotice.vue`, `src/services/notices.ts` | Completion-notice deduplication, countdown, window lifecycle, interaction, and targeted events; Windows activation control reuses the existing `windows-sys` dependency |

The original commands are `list_tasks`, `create_task`, `task_action` (pause, resume, cancel, remove record), and `open_directory`. The `downloads-changed` event carries snapshots with increasing revisions, task IDs, status, byte counts, speed, routes, and verification results. Vue does not perform network or filesystem work directly.

Additional commands are `browse_releases`, `preview_batch`, `create_batch`, `change_route`, `diagnose_routes`, `save_settings`, `reorder_queue`, `query_history`, `acknowledge_notices`, `hide_to_tray`, `add_favorite`, `remove_favorite`, `check_favorites`, `check_app_update`, `open_release`, and `open_update_link`. A Rust actor owns authoritative tasks, favorites, and settings. Queue revisions are separate, and the frontend rejects stale snapshots. IPC, directory selection, and clipboard writes use the typed service boundary.

`diagnose_routes(url)` retains its interface, with Rust owning the default test URL. The actor owns the manual worker and reply channel, releases busy state on completion or failure, and cancels the worker on exit. Internal generations isolate manual and automatic display results without changing engine route selection. Snapshots add optional `diagnosticContext` with `source` (`default`, `input`, or `download`) and `filename`. Reports retain the existing `diagnostics` JSON storage without a schema change. Context is session-only, so restored reports are labeled historical.

Route UX snapshots add optional `retryInfo`, `recoveryInfo`, and `routeSuggestion`, with an empty default for `routeFailures`. Legacy records use defaults; no table migration is needed. `waiting_network` is counted separately from active transfers. Session clocks and suggestions reset on restart. `apply_route_suggestion(id, suggestionId)` and `dismiss_route_suggestion(id, suggestionId)` reject stale or duplicate confirmations. Retry now reuses the resume task action.

Directory operations also expose `inspect_directory(directory)` and `create_directory(directory)`. Inspection is read-only and returns the normalized absolute path with an `existing` or `missing` state. Creation runs only after user confirmation, recursively creates missing directories, checks write access, and returns the directory path. Permission errors, file conflicts, and inaccessible drives are not reported as creatable missing directories. Existing task, batch, and transfer interfaces retain their directory preflight checks.

Task JSON adds nullable `elapsedMs` and `elapsedIsPartial` (default `false`); settings add `backgroundCompletionNotice` (default `true`). SQLite schema version remains 2. Notices include `kind` and `taskId`. `downloads-changed` targets only the main window. The notice window uses `download-notice-changed` and the `read_download_notice`, `present_download_notice`, `dismiss_download_notice`, `hover_download_notice`, and `open_download_notice` commands. It subscribes before reading and acknowledges rendering before the countdown starts. Its capability grants only event subscription; Rust also restricts application commands by originating window so notices cannot create tasks, change directories, or save settings.

## Automated verification

```powershell
npm run typecheck
npm run test -- --run
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

Rust tests use controlled local HTTP servers to cover URLs/paths, 200/206/416 responses, bad ranges, missing Range support, interrupted streams, timeouts, rate limits, rejected redirects, HTML, digest mismatch, segmentation, queues, pause/resume, active cancellation, restart recovery, route switching, file locks, and crash recovery during publication. Vue tests cover duplicate submission, directory errors, stale events, retries, and integrity-failure messages. `src/redesign.test.ts` also covers bulk failure/unmount interruption, task details, version switching, release-pagination loading/deduplication/retries/empty and final pages/stale responses, history pagination/statistics, favorite filters, and settings drafts. Pagination layout stability requires separate browser checks because jsdom does not provide real layout measurements. Tests use isolated temporary directories and never execute downloaded files.

Notice and timing tests also cover foreground consumption, background deduplication, merging, settings, stale callbacks, hover and keyboard-focus countdowns, subscription/read races, IPC isolation, accumulated/frozen durations, saving without progress, clean shutdown and interrupted recovery, and legacy records. Display tests cover hidden verification badges with retained errors, average-speed validity, decimal units, limit conversion, and preservation of existing limits. Multiple monitors, taskbar placement, actual focus preservation, and 100%/150%/200% scaling still require native Windows acceptance; state tests and browser rendering do not replace it.

Route-diagnostic tests cover blank input, explicit assets, invalid links, concurrent transfers and queue continuation, pause/resume/cancel, duplicate requests, abandoned callers, timeouts, partial/all failures, worker faults, shutdown cancellation, stale results, whole-batch replacement, and legacy report context. Frontend tests also cover button states, error recovery, real filenames, and MB/s display. Browser layout and native Windows button interactions require separate acceptance checks.

Route UX tests use injectable policies and monotonic-time inputs to cover recovery success/exhaustion, cooldown, queue handoff and frozen allowances, multiple waiting tasks, pause/cancel/exit/restart, speed windows, low-speed exclusions, savings thresholds, unavailable candidates, duplicate confirmation, and retained automatic mode. Frontend tests cover countdowns, restart-loss confirmation, unmount cleanup, and errors. Accelerated tests do not wait five real minutes.

Optional live-network checks follow. Full downloads save the specified asset. The benchmark samples the same 8,388,608-byte range (about 8.4 MB) on each route, rotates order over three rounds, and waits up to 30 seconds per request.

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --example verify_download -- "https://github.com/rakanki911/DLSS5-Swapper/releases/download/v2.2.9/DLSS5-Swapper-Setup-2.2.9.exe" "artifacts/acceptance/下载验证"
cargo run --manifest-path src-tauri/Cargo.toml --example benchmark_routes -- "https://github.com/rakanki911/DLSS5-Swapper/releases/download/v2.2.9/DLSS5-Swapper-Setup-2.2.9.exe"
```

`verify_download` reuses the desktop download core and creates its own `verification.sqlite3` under the acceptance directory without touching desktop data. Historical results in `artifacts/acceptance-v0.2.0/` and `artifacts/acceptance/` are local maintainer files, not repository contents. With an existing v1 test database, `verify_v2` checks migration on a consistent copy and downloads two small assets from the official catalog. Replace the last argument with your actual v1 test database path:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --example verify_v2 -- "https://github.com/cli/cli" "artifacts/acceptance-v0.2.0/批量与迁移" "artifacts/acceptance/下载验证/verification.sqlite3"
```

Use a fresh acceptance directory. The source database is opened read-only, and migration writes only to the copy. Test assets, browser screenshots, and build caches are excluded from source. Browser layout checks, Rust download checks, and native Windows interaction checks are recorded separately. Release notes describe user-facing changes, download and usage instructions, and limitations or compatibility information that affect users. Developer verification results, including test counts, build checks, acceptance procedures, and checks not performed, are kept separately in maintainer acceptance records and omitted from release notes.

## Known limitations

Public routes vary with network conditions and service policies. Short probes cannot guarantee sustained whole-file throughput, and direct GitHub may be fastest. Without an official digest, authenticity is not established. Downloaded files are never started automatically. The unauthenticated official API may be rate-limited; standard direct asset URLs may still work when catalog browsing is unavailable.

This executable has no Windows Authenticode signature, so Windows may report an unknown publisher. The MIT, Tauri 2, and Vue 3 badges do not change that. Coverage of every Windows 10/11 version, display scale, and WebView2 environment is not claimed. Packaging checks do not mean native UI, tray operations, system-directory selection, or full live-network downloads were reaccepted for this release.

## License

Project code is provided under the [MIT License](LICENSE). Third-party dependencies retain their respective licenses.
