# 原生界面资源

`NotoSansSC-Regular.otf` 取自 [Noto CJK 官方简体中文子集](https://github.com/notofonts/noto-cjk/tree/main/Sans/SubsetOTF/SC)，文件大小 8,331,336 字节，SHA-256 为 `FAA6C9DF652116DDE789D351359F3D7E5D2285A2B2A1F04A2D7244DF706D5EA9`。使用上游静态 Regular 字重，未修改字形或安装系统字体；字形范围之外的字符由 Windows 字体回退。`NotoSansSC-OFL.txt` 保留上游 OFL 许可，完整中文输入法和罕见字符显示仍需实机验证。

`THIRD_PARTY_NOTICES.txt` 由 `scripts/native-notices.ps1` 从 Windows x64 运行依赖提取，包含项目、字体、Slint 及其余组件声明，并嵌入“关于”窗口。Slint 采用 `LicenseRef-Slint-Royalty-free-2.0`，界面提供 `AboutSlint`。`licenses/` 补齐 Cargo 包中未附带的上游许可文件；固定提交来源及文件摘要保存在 `license-sources.json`。更新依赖后应重新生成并核对声明。该清单排除仅用于构建的过程宏，不是运行时动态加载模块的审计结果。

`icons/` 保存原页面使用的 Phosphor 静态 SVG，来源为 `@phosphor-icons/vue` 2.2.1。旧导出脚本和 npm 依赖已退役；构建直接嵌入这些资源，不要求 Node.js 或在线图标服务。`Phosphor-LICENSE.txt` 保存上游 MIT 许可并合并进内嵌声明。`tray.png` 为窗口与托盘图标，`icon.ico` 和 `icon.svg` 保留原项目图标资源。

主页面沿用 Segoe UI 优先的系统字体选择；嵌入的 Noto Sans SC 作为可用中文字体保留。不同 Windows 字体回退和 DPI 下的字形、粗细仍需实机对比。
