# GitHubSP 新桌面界面来源

本项目以本地 Rust 下载核心为业务实现。以下前端源码已获取至本地忽略目录 `artifacts/references/`，没有执行上游应用或构建脚本。

- cc-switch：https://github.com/farion1231/cc-switch，commit `506f3cf836da730fba59af25b9f87873a92a6c48`。`src/components/ui/button.tsx`、`input.tsx`、`dialog.tsx` 改编为本项目基础控件；样式与业务绑定已替换。
- Motrix：https://github.com/agalwood/Motrix，commit `a45a729e985446a936167106af6b4173f06116aa`。`src/renderer/routes/downloads/task-progress.tsx` 改编为本项目进度组件，下载工作台的列表与详情分区作为布局参考；没有引入其 Electron 或下载核心。

- magpie：https://github.com/yetone/magpie，commit `75d40ba48bcc4b215de044c78ce226ed47f4f510`。`internal/gui/assets/index.html`、`app.css` 的侧栏、标题栏、控件表面、页面淡入和宽屏限宽适配为 React 页面，设置页的分类标签、跨分类搜索、行式偏好列表和分段开关适配为 `Settings.tsx`；`app.js` 的 `navTopGo` / `navTopStep` 弹簧逻辑适配为 `Navigation.tsx`。只复用界面与动画实现，没有引入 Wails、Go 或网关业务。

## cc-switch

MIT License

Copyright (c) 2025 Jason Young

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Motrix

The MIT License

Copyright 2018-present Dr_rOot

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

未复制上游品牌图、截图、图标包或其他单独授权的素材。界面图标由 lucide-react 提供，应用图标为本轮原创的几何包裹资源。完整运行依赖许可证随打包脚本归集。

## magpie

MIT License

Copyright (c) 2026 yetone

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
