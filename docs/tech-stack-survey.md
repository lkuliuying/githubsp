# 桌面软件前端技术栈调研（含 clawd 与同类项目）

| 项目 | 内容 |
| --- | --- |
| 调研日期 | 2026-10-24 |
| 调研问题 | GitHub 上主流桌面软件的前端/界面技术栈分别是什么；`clawd` 这类项目怎么选型；对 GitHubSP 的选型有何结论 |
| 数据来源 | GitHub REST API（`/repos/{owner}/{repo}`）与仓库 raw 文件，2026-10-24 实时读取 |
| 数据口径 | star 数与主语言为查询时快照；包体/内存为量级估算（不同测量口径差异大，见第 5 节说明） |
| 关联文档 | [`docs/optimization-plan.md`](optimization-plan.md)、[`docs/v0.3.0.md`](v0.3.0.md) |

> 方法说明：本机 `web_search` 端点不可用（返回 `no web_search_tool_result`），因此改用 GitHub API 与 raw 文件直接取证。凡标注“实测”的，均为本次查询返回的真实数值；凡标注“业界普遍认知”的，是没有公开仓库可核对的闭源软件，按公开资料归纳，不作为本项目决策的唯一依据。

---

## 1. 结论速览

1. **桌面端只有四大流派**：Electron/CEF（Web 技术 + 自带浏览器内核）、Tauri（Rust 核心 + 系统 WebView）、Rust/C++ 自绘 GPU（Zed、Slint、egui、Alacritty 等）、传统原生（Qt、.NET/Avalonia、Flutter、Java SWT）。
2. **“外壳类”应用压倒性选 Electron**：编辑器外壳、聊天、笔记、数据库客户端——因为需要海量现成控件（表格、富文本、虚拟滚动）。
3. **“性能敏感类”应用正在集体转向 Rust 自绘**：编辑器、终端、文件管理器、下载器——长列表 + 高频刷新 + 冷启动指标会击穿 WebView。
4. **`clawd` 的答案是“Electron 但零前端框架”**：Electron 41 + 原生 JS/HTML/SVG 动画 + Node FFI 调系统 API。它证明“动效丝滑”不来自框架，而来自把动效做成数据。
5. **对 GitHubSP 的结论：维持 Slint，不迁移**。下载器属于“性能敏感类”，Slint 的包体（54.3 MB 实测）与单进程结构仍显著优于 Electron（100–250 MB、多进程），但要承认 GPU 后端会把内存吃回一部分（本机工作集 164–178 MB）；动效缺口靠令牌体系 + 原生属性动画补齐，不引入 WebView、Remotion 或视频运行时。

---

## 2. 四流派对照（star 数为本次实测快照）

| 流派 | 运行机制 | 代表项目（实测 star / 主语言） | 前端技术 | 典型代价 |
| --- | --- | --- | --- | --- |
| **Electron / CEF** | 打包 Chromium + Node.js | `microsoft/vscode` 194253 / TypeScript；`electron/electron` 123510 / C++；`laurent22/joplin` 56848 / TypeScript；`nwjs/nw.js` 41141 / JavaScript；`bitwarden/clients` 14044 / TypeScript | HTML/CSS/JS；VS Code 用自研 DOM 层而非 React | 安装包 100–250 MB，常驻内存 200–500 MB |
| **Tauri** | Rust 主进程 + 系统 WebView2/WKWebView/WebKitGTK | `tauri-apps/tauri` 112149 / Rust；`tw93/Pake` 62284 / Rust；`spacedriveapp/spacedrive` 39290 / Rust | 前端仍是 Web（React/Vue/Svelte/Solid 皆可），Rust 侧做 IO 与权限 | 包体 3–15 MB；但 WebView 版本随系统漂移，动效/字体渲染需按平台兜底 |
| **Rust/C++ 自绘 GPU** | 直接调 wgpu/Skia/Metal/DX 绘制 | `zed-industries/zed` 92111 / Rust；`lapce/lapce` 38953 / Rust；`alacritty/alacritty` 66026 / Rust；`emilk/egui` 31703；`iced-rs/iced` 31826；`slint-ui/slint` 24265；`DioxusLabs/dioxus` 39473；`wezterm/wezterm` 29329；`makepad/makepad` 7213 | 声明式 DSL（Slint/QML 式）或立即模式 immediate mode | 控件生态薄，表格/富文本/输入法要自己做；动效能力强但 API 受限 |
| **传统原生** | 系统控件或自建控件库 | `flutter/flutter` 180346 / Dart；`AppFlowy-IO/AppFlowy` 77689 / Dart（桌面端 Flutter）；`dbeaver/dbeaver` 52184 / Java（SWT）；`wailsapp/wails` 36675 / Go；`AvaloniaUI/Avalonia` 31701 / C# | XAML / Flutter Widget / SWT | 跨平台一致性尚可，但视觉与动效风格偏“系统味”，定制成本高 |

补充：`qt/qtbase` 只有 3093 star，但 Qt 是商业桌面软件（WPS、AutoCAD 插件、工业软件）的主力栈——**star 数不能代表使用面**，闭源商业软件大量用 Qt/C++ 自绘。

---

## 3. 案例拆解

### 3.1 `clawd`：`rullerzhou-afk/clawd-on-desk`（6510 star / JavaScript）

定位是“桌面像素宠物”，观测 Claude Code / Codex / Cursor 等 AI agent 的运行状态。技术栈全部来自其 `package.json`（版本 1.4.0）：

| 依赖 | 版本 | 作用 |
| --- | --- | --- |
| `electron`（dev） | 41.10.4 | 运行时容器 |
| `electron-builder`（dev） | 26.15.7 | 打 win x64/arm64、mac x64/arm64、linux |
| `electron-updater` | 6.8.3 | 自动更新 |
| `koffi` | 2.16.3 | Node FFI，直接调 Win32/macOS API（透明窗口、置顶、点击穿透） |
| `ws` | 8.21.0 | 与 agent 进程的本地 WebSocket 通道 |
| `markdown-it` / `jsonc-parser` / `htmlparser2` / `yauzl` / `@larksuiteoapi/node-sdk` | — | 说明渲染、配置解析、解包、飞书通知 |
| `@electron/asar` | 3.4.1 | 资源打包 |

关键观察：

- **没有 React / Vue / Svelte**。渲染层是原生 JS + HTML，动效靠 **SVG 动画**（仓库 topics 明确含 `svg-animation`、`pixel-art`、`desktop-pet`）。
- 脚本命名暴露了工程成熟度：`verify:electron`、`audit:assets`、`audit:native-package`、`audit:pr-history-assets`、`update:official-theme-snapshot`、`verify:updater-metadata`、`verify:winget-manifest`、`verify:winget-arch`、`verify:release`。**动效与素材是被“校验”的资产，不是散落在代码里的魔法数字。**
- 二十余个 `install:xxx-hooks`（claude/codex/cursor/gemini/kiro/kimi/qwen/...）说明它靠“适配器脚本”横向扩展，而不是改核心。

**可借鉴的不是 Electron，而是“动效资产化 + 校验脚本”这个思路。**

### 3.2 Electron 家族（VS Code / Joplin / 闭源外壳）

- VS Code（194253 star）：TypeScript + 自研 DOM/虚拟渲染层，不用 React；动效全部走 CSS transition，交互反馈统一 80–150 ms，列表 hover/选中用 CSS 变量 + transition，禁止超过 200 ms 的交互反馈。
- Joplin、Bitwarden、Obsidian、Notion、Slack、Discord、飞书、钉钉（后几个闭源，业界普遍认知）：Electron + 各自前端框架；共同点是**大量控件靠框架白拿**，代价是内存与冷启动。
- Motrix（下载器，与本项目同域）：Electron + Vue2 + element-ui，动效主要是 CSS transition + Element 的淡入淡出，没有独立动效层。

### 3.3 Tauri 家族（Spacedrive / Pake）

- `spacedriveapp/spacedrive`（39290 star）：文件管理器，Tauri + React 前端 + Rust 虚拟分布式文件系统。它的经验是**进度/波纹类动画只用 GPU 友好属性（transform/opacity），绝不改会触发布局的属性**。
- `tw93/Pake`（62284 star）：把网页包成桌面应用，证明 Tauri 在“壳”场景的成本优势（包体几 MB）。

### 3.4 Rust 自绘（Zed / Lapce / Alacritty / Slint）

- Zed（92111 star）：自研 GPUI，帧调度与动画由 GPU 管线驱动，风格是“动画必须可中断、可合并”。
- Lapce（38953 star）：Floem 声明式 UI。
- Alacritty/WezTerm：终端，纯 GPU 绘制，几乎不做装饰性动效——**性能敏感场景里，动效预算要给渲染让路**。
- Slint（24265 star）：声明式 DSL + 原生渲染器，本项目的选择。它的官方示例（`fancy-switches` 用 `cubic-bezier(0.61,0.21,0.68,1.22)` 做回弹、`orbit-animation` 用 `animation-tick()` 做连续驱动）说明**离散过渡与连续动画是两套机制**。

### 3.5 传统原生

- Flutter（180346 star）：AppFlowy 桌面端验证了 Flutter 做复杂桌面 UI 可行，但安装包 80 MB+、启动偏慢。
- Avalonia（31701 star，C#）：.NET 阵营的跨平台 XAML。
- DBeaver（52184 star，Java SWT）：SWT 用系统原生控件，稳定但动效基本为零。

---

## 4. 五维取舍（同一份产品需求下的对比）

| 维度 | Electron | Tauri | Rust 自绘（Slint 等） | 传统原生 |
| --- | --- | --- | --- | --- |
| 安装包 | 100–250 MB | 3–15 MB | 3–60 MB | 40–300 MB |
| 常驻内存 | 200–500 MB | 60–150 MB | 60–250 MB | 80–250 MB |
| 冷启动到首帧 | 0.8–2.5 s | 0.4–1.2 s | 0.3–1.0 s | 0.3–1.5 s |
| 控件生态 | 极强 | 强（Web 生态） | 弱（需自建） | 中 |
| 动效能力 | 最强（CSS/Canvas/WebGL） | 强（同 Web） | 中（属性动画，类型受限） | 中 |

> 数值为量级估算，随应用复杂度浮动；本项目 Slint 版的实测口径见 `docs/optimization-plan.md` 与 `design-qa.md`，不在此重复。

**GitHubSP v0.3.0 试用包本机实测（`artifacts/verification/startup-20261024-023936/result.json` 与同批采样）**：exe 54.3 MB（静态 CRT + wgpu/FemtoVG 渲染器，`strip=true`、`lto=thin`）；单进程，工作集 164–178 MB、私有字节 210–225 MB、24 线程、512 句柄；隔离目录启动到“窗口 + 数据库就绪”936 ms。工作集包含 D3D12 与厂商驱动映射（76 个模块里有 `d3d12core.dll`、`nvwgf2umx.dll`、`nvldumdx.dll`），不等于 CPU 侧常驻。**所以“Rust 自绘一定省内存”不成立**：省的是包体与进程数，GPU 后端会把内存吃回来一部分。

---

## 5. 对 GitHubSP 的结论

### 5.1 选型坐标

GitHubSP 的产品特征是**长列表（队列/历史/收藏）+ 高频进度刷新 + 托盘常驻 + 单文件免安装分发**。四条特征里，“单文件免安装”与“单进程”否决 Electron（100–250 MB、主进程 + 渲染进程 + GPU 进程多开），“高频进度刷新”否决需要 WebView 消息往返的方案。**Slint 的定位与产品承诺一致，不迁移**——按实测，它的优势是包体与进程结构，不是内存绝对值。

### 5.2 Slint 的真实短板与补偿方式

| 短板 | 事实依据 | 补偿 |
| --- | --- | --- |
| 控件生态薄 | 无 Web 生态的表格/富文本/虚拟滚动现成件 | 已在 `ui/theme.slint` 自建 ActionButton/Switch/Input/Progress 等组件 |
| 动效类型受限 | 可动画类型只有 float/int/color/length/brush/angle；`percent` 不可 animate，故 `transform-scale` 无法做过渡 | 交互反馈改用涟漪（length+float）+ 图标旋转（angle）+ 颜色过渡；按下缩放已决定不做（见计划书 D11） |
| 裁剪是矩形 | `clip: true` 不跟随 border-radius | 涟漪透明度压到 0.16，圆角处露角不可见 |
| 无 CSS 生态 | 无 1200 行现成动效库 | 用 `ui/motion.slint` 的 8 个令牌统一节奏 |

### 5.3 为什么不迁到 Tauri/Electron

迁移的直接收益只有“控件与动效现成”，代价是：Tauri 虽能把包体压到几 MB，但**引入 WebView2 依赖**（免安装承诺失效、老系统与离线环境不确定）、**动效与字体渲染随系统 WebView 版本漂移**（需要按平台兜底）、**现有 24 项原生 UI 测试与 125 项核心测试的界面部分全部重写**。Electron 则同时放弃包体与内存优势。**两项收益都小于成本，且与产品定位相反。**

---

## 6. 可直接移植的三项实践

| 来源 | 实践 | 在本项目的落地 |
| --- | --- | --- |
| clawd | 动效资产化并被脚本校验（`audit:assets`、主题快照） | `ui/motion.slint` 集中令牌；`ui_tests.rs` 断言令牌数值与“减少动效归零” |
| VS Code | 交互反馈 ≤ 200 ms，布局级过渡 250–350 ms | `hover 140 / press 90 / fast 120` 反馈，`base 180 / slow 260 / emphasized 360` 过渡 |
| Spacedrive | 动画只碰合成属性（transform/opacity），不碰布局 | 导航指示条用 `y` 位移而非重排；顶栏高亮用滑动药丸而非重绘背景 |

---

## 7. 本轮动效落地对照（2026-10-24）

| 令牌 | 值 | 用途 |
| --- | --- | --- |
| `instant` | 80 ms | 最快直接反馈 |
| `press` | 90 ms | 图标旋转、按下态 |
| `fast` | 120 ms | 开关滑块、焦点边框、颜色过渡 |
| `hover` | 140 ms | 悬停态 |
| `base` | 180 ms | 导航指示条、顶栏药丸、页面过渡 |
| `slow` | 260 ms | 提醒窗入场 |
| `ripple` | 300 ms | 点击涟漪扩散 |
| `emphasized` | 360 ms | 强调场景（保留） |

`Motion.reduce` 为真（用户设置或系统关闭动画）时，上述令牌全部归零，动效在单帧内完成。

已实现清单：`ActionButton` 悬停/按下色差 + 图标旋转 + 点击涟漪；左侧 `ModuleNavigation` 行高 54 px、字号 14 px、图标 20 px + 单根滑动指示条；顶栏 4 个 tab 的滑动高亮药丸；版本列表行按下色。

---

## 8. 未验证与后续

1. 闭源软件（Figma、Warp、1Password、微信/QQ 新版）的栈只能引用公开资料，未做二进制取证。
2. 包体/内存/启动为量级估算；如需正式数字，应在同一台机器上用 `scripts/measure-process-tree.ps1` 与 `scripts/measure-history.ps1` 采集。
3. `clawd` 的 SVG 动效实现未逐帧比对，只读取了依赖清单与仓库 topics。
4. 若后续要做“按下缩放”，需先解决 Slint `percent` 不可 animate 的限制（候选方案：`width/height + clip` 伪缩放，代价是小尺寸重排），本轮已决定不做。

---

## 9. 数据附录（可复现）

查询命令（PowerShell，未认证 API，60 次/小时）：

```powershell
Invoke-RestMethod -Uri "https://api.github.com/repos/microsoft/vscode" -Headers @{ 'User-Agent'='dsh' }
Invoke-RestMethod -Uri "https://raw.githubusercontent.com/rullerzhou-afk/clawd-on-desk/main/package.json" -Headers @{ 'User-Agent'='dsh' }
```

快照数值：

| 仓库 | star | 主语言 |
| --- | --- | --- |
| microsoft/vscode | 194253 | TypeScript |
| flutter/flutter | 180346 | Dart |
| electron/electron | 123510 | C++ |
| tauri-apps/tauri | 112149 | Rust |
| zed-industries/zed | 92111 | Rust |
| AppFlowy-IO/AppFlowy | 77689 | Dart |
| alacritty/alacritty | 66026 | Rust |
| tw93/Pake | 62284 | Rust |
| laurent22/joplin | 56848 | TypeScript |
| dbeaver/dbeaver | 52184 | Java |
| nwjs/nw.js | 41141 | JavaScript |
| DioxusLabs/dioxus | 39473 | Rust |
| spacedriveapp/spacedrive | 39290 | Rust |
| lapce/lapce | 38953 | Rust |
| wailsapp/wails | 36675 | Go |
| iced-rs/iced | 31826 | Rust |
| emilk/egui | 31703 | Rust |
| AvaloniaUI/Avalonia | 31701 | C# |
| wezterm/wezterm | 29329 | Rust |
| slint-ui/slint | 24265 | Rust |
| bitwarden/clients | 14044 | TypeScript |
| makepad/makepad | 7213 | Rust |
| rullerzhou-afk/clawd-on-desk | 6510 | JavaScript |
| qt/qtbase | 3093 | C++ |

---

*本文件只记录调研事实与选型结论；具体排期、验收门槛以 [`docs/optimization-plan.md`](optimization-plan.md) 为准。*
