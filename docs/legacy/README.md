# Slint 历史入口

当前默认桌面入口是 `desktop/` 的 Tauri + React 实现。`native-desktop/` 保留旧版源码、已完成的界面改造和验收工具，用于历史版本维护及兼容性比对，不作为当前界面的验收结果。

历史 v0.3.0 发布采用 Slint + Rust，运行条件和单文件分发方式与新 Tauri 包不同。发布范围见 [v0.3.0 记录](v0.3.0.md)；原版 v0.2.4 的兼容问题与数据恢复规则见 [数据与存储](../data-storage.md)。

## 本地命令

以下脚本固定面向 `githubsp-native`，不会因为默认 Cargo 入口改变而构建 Tauri：

```powershell
./scripts/native-cargo.ps1 -CargoArguments @('test','-p','githubsp-core','-p','githubsp-native','--locked','--offline','--','--test-threads=1')
./scripts/verify-native-ui.ps1 -Offline
./scripts/test-ui-mcp.ps1 -Offline
./scripts/package-native-windows.ps1 -Offline
```

AI 迭代、固定视觉基线及手动测试版发布的约束见 [旧版自动化](ui-automation.md)。普通源码推送只验证，不创建 GitHub Release。旧代码、三个 PNG 基线和脚本是保留的维护能力，不属于编译缓存。

旧选型调查、阶段优化计划和逐次设计验收日记已移除，避免与现行架构混淆；原文可从 Git 历史查看。历史正式发布、数据与兼容性证据仍保留在本地 `artifacts/releases/`、`artifacts/archive/` 和 `artifacts/trials/`，未随目录清理删除。
