# GitHubSP

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)](https://v2.tauri.app/)
[![React](https://img.shields.io/badge/React-18-149ECA)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.96-orange?logo=rust)](https://www.rust-lang.org/)

面向 Windows 的 GitHub Release 附件下载工具，使用 **Tauri 2 + React + TypeScript + Rust**，提供安装版和免安装版。

[English](#english) · [发布页](https://github.com/lkuliuying/githubsp/releases) · [问题反馈](https://github.com/lkuliuying/githubsp/issues)

> 当前源码已迁移至 Tauri。新的安装版和免安装版已在本地构建，尚未发布到 GitHub Releases；历史 v0.3.0 发布附件使用 Slint，与当前界面及 WebView2 运行条件不同。

![下载队列，使用模拟数据展示](docs/images/queue.png)

## 功能

| 模块 | 功能 |
| --- | --- |
| 下载队列 | 串行下载、暂停恢复、任务详情、下载进度、失败原因与换线确认 |
| 新建下载 | 仓库或 Release 页面读取、版本与附件选择、直链批量预览、目录检查、重复项识别与下载前确认 |
| 下载历史 | 分页、搜索、状态筛选、文件位置与重新下载 |
| 项目收藏 | 版本跟踪、未读提醒、自动检查、独立关注规则与匹配附件入队 |
| 线路检测 | 比较 GitHub 直连和内置公共加速线路的可用性与当前速度 |
| 设置 | 通用、下载、收藏更新、数据与备份、关于；支持跨分类搜索 |

界面以灰白和黑灰为主，支持浅色、深色与减少动态。切换模块保留本次运行中的输入、筛选、选择、滚动位置及未保存设置。[查看设置页预览](docs/images/settings.png)。预览使用模拟数据。

支持公开 GitHub 仓库、Release 页面和标准附件链接：

```text
https://github.com/{owner}/{repo}
https://github.com/{owner}/{repo}/releases/tag/{tag}
https://github.com/{owner}/{repo}/releases/download/{tag}/{filename}
https://github.com/{owner}/{repo}/releases/latest/download/{filename}
```

附件直链可批量预览，每批最多 100 项。平台和架构提示来自文件名，下载前仍需确认。暂不支持私有仓库、Git clone、自建代理和 GitHub 自动生成的源码包；作为 Release 附件上传的源码压缩包可以下载。程序不会自动执行下载的文件或安装应用更新。

## Windows 使用与分发

要求 Windows 10/11 x64。安装版和轻量免安装版均使用 Microsoft Edge WebView2 Runtime，用户无需安装 Node.js、pnpm 或 Rust。

| 文件 | 使用方式 |
| --- | --- |
| `GitHubSP-v<版本>-windows-x64-Setup.exe` | 当前用户安装；缺失 WebView2 时下载并安装 Evergreen Runtime。 |
| `GitHubSP-v<版本>-windows-x64-Portable.zip` | 解压完整目录后运行 `GitHubSP.exe`；需要系统已有 WebView2。 |
| `SHA256SUMS.txt` | 对照文件名校验安装包和 ZIP 的 SHA-256。 |

试用前退出旧版。普通免安装模式默认仍使用 `%LOCALAPPDATA%\com.githubsp.desktop`，并遵循已保存的数据位置；免安装不等于数据随程序移动。需要随身数据时，可在 exe 旁创建空文件 `portable-mode.flag`；独立测试可使用 `--data-dir <绝对路径>`。具体规则、备份及恢复见 [数据与存储](docs/data-storage.md)。

```powershell
Get-FileHash .\GitHubSP-v0.3.0-windows-x64-Portable.zip -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
.\GitHubSP.exe --data-dir "F:\GitHubSP-试用"
```

## 开发

要求 Rust 1.96+、Node.js、pnpm 10、PowerShell 7、Visual Studio C++ Build Tools 和 WebView2 Runtime。

```powershell
pnpm --dir desktop install --frozen-lockfile --ignore-scripts
cargo fetch --locked --target x86_64-pc-windows-msvc
node desktop/scripts/notices.mjs --rust
pnpm --dir desktop desktop
```

许可证生成会读取 Cargo 缓存，首次开发先执行 `cargo fetch`。源代码、应用标识、IPC、数据库与下载核心的边界见 [桌面架构与构建](docs/tauri-desktop.md)。

```powershell
pnpm --dir desktop test
pnpm --dir desktop build
cargo test -p githubsp-core -p githubsp-desktop --locked --offline
cargo clippy -p githubsp-desktop --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
```

生成安装版、免安装版、依赖许可证和校验和：

```powershell
./scripts/package-desktop-windows.ps1 -Offline -OutputDirectory artifacts/build/tauri-candidate
```

再次打包请使用新的输出目录。`-Offline` 仅限制 Cargo 依赖访问，不代表缺失 WebView2 的机器可以完全离线安装。打包脚本不发布、不签名；固定版本 WebView2 的可选打包方式见 [分发说明](docs/tauri-desktop.md#两种-windows-分发)。

## 项目结构

```text
core/                 下载、网络、SQLite、收藏、备份与迁移
desktop/             React 前端与 Tauri 宿主
native-desktop/       保留的 Slint 历史实现与测试
scripts/              当前打包脚本及明确命名的历史验收工具
docs/                架构、数据、历史版本说明与界面预览
automation/          Slint 历史验收配置与基线
artifacts/           本地交付包、参考源码和验证记录（不提交）
```

默认 Cargo 入口为 `githubsp-desktop`。Slint 源码保留用于旧版维护和兼容性比对，其脚本与截图不作为 Tauri 验收结果；使用方法见 [历史入口](docs/legacy/README.md)。旧技术栈调查、阶段计划和逐次验收日记已由当前文档取代，原文保留在 Git 历史。

## 验证与限制

当前本机已通过前端测试与生产构建、125 项核心测试、7 项 Tauri 宿主测试；界面另有模拟 IPC 的三种尺寸、深浅主题、状态保留和异步确认检查。模拟接口不代表真实网络、安装或完整桌面验收。

- 公共加速服务会收到公开附件 URL；短时探测不保证整文件持续速度。
- GitHub 未认证 API 可能限流；没有官方校验摘要时，不能据此确认文件来源的真实性。
- 下载与数据库记录沿用现有兼容协议；备份应用记录不会移动下载成品或续传分片。
- 原有限速保存和队列显示使用不同单位换算，数值显示存在差异，尚未统一。
- 干净系统安装/卸载、缺失 WebView2、多 DPI/多显示器及真实网络完整下载仍需专项验收；本轮设置页原生交互核查因焦点捕获限制未完成。

普通源码推送只触发已有 Slint 历史验证，不自动发布 Release；测试版发布须显式手动触发。当前没有独立项目记忆文件，长期约定以本 README 及 `docs/` 中的现行说明为准。

## 许可与来源

GitHubSP 自有代码使用 [MIT 许可](LICENSE)。页面骨架与动效改编自固定提交的 magpie，基础控件参考 cc-switch，进度组件参考 Motrix；各自版权、完整 MIT 文本和改编范围见 [第三方声明](desktop/THIRD_PARTY_NOTICES.md)。安装版与免安装版均随附运行依赖许可。

## English

GitHubSP is a Windows GitHub Release asset downloader built with **Tauri 2, React, TypeScript and Rust**. It supports a serial download queue, pause/resume, release browsing, batch previews, history, favorites, route diagnostics, and searchable settings.

The current Tauri source produces an NSIS installer and a portable ZIP. Both use WebView2; the installer can download the runtime when missing, while the lightweight ZIP needs an existing runtime. The new UI has been built locally and has not been published to GitHub Releases. Historical v0.3.0 assets use Slint.

Exit older versions before running a build. Portable normally shares the configured application data directory; `portable-mode.flag` beside the executable selects a local `data/` directory, and `--data-dir <absolute path>` selects isolated test data. Private repositories, Git clone, auto-generated source archives and automatic installation are not supported.

Development and build commands are shown above. `core/` owns downloads and storage, `desktop/` is the current application, and `native-desktop/` retains the legacy implementation. Local tests and simulated UI checks do not establish clean-system installation or real-network acceptance. Project and third-party license notices are included with both distribution formats.
