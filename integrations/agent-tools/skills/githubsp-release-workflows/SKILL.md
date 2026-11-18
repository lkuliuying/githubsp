---
name: githubsp-release-workflows
description: 使用已经连接的本机 GitHubSP 工具获取公开 GitHub Release 软件、复用已下载文件、检查本地关注更新或迁移关注清单。用于下载与关注任务，不用于代码仓库编辑、安装程序执行或远程 GitHub Star/Watch 管理。
---

使用宿主提供的 `githubsp_*` 工具，MCP 宿主可能增加服务器或插件前缀。GitHubSP 桌面或托盘进程需运行；连接失败时按工具提示告知启动条件。

获取软件时先检查 `githubsp_status` 和 `githubsp_software`。只有本地文件存在、版本满足请求时才复用；`missing`、`inaccessible` 和 `changed_size` 不能称为可用。`verificationAtDownload` 是下载时的校验记录，不代表再次验证了当前内容。标签不同只表示不同版本，不自行按任意标签推断版本新旧。

需要下载时，用 `githubsp_resolve_release` 固定真实版本和附件身份，优先沿用项目偏好。无匹配或存在歧义时让用户选择。确认允许目录后以稳定 `requestKey` 创建任务；同一创建请求重试保持键不变。超时或断连后先按原键查询，服务重启后仍可查询原任务，但解析结果需重新获取。用有界 `githubsp_wait_task` 等待，等待超时不等于取消下载。完成后提供实际路径、版本和校验状态，不把下载完成称为已安装，也不执行附件。

检查关注更新时使用本地 `githubsp_watch`，与远程 Star/Watch 分开。更新简报读取 `githubsp_updates`，保留来源链接和元数据是否完整的说明。读取或生成简报不标记已读；用户明确要求处理时才用 `githubsp_update` 的 `seen` 或 `skip`。处理成功后保存 `nextCursor`；重试使用原游标，避免漏掉记录。读取完整正文使用 `detail`。

迁移关注时使用 `githubsp_watchlist` 导出便携 JSON。导入先 `preview`，向用户展示新增、重复、冲突、保留/覆盖策略及本机根目录。得到对应授权后用相同正文、目录、策略和预览的 `fingerprint` 执行 `import`。预览过期时重新预览。清单不携带凭据或机器路径；原应用备份用于保存本机完整数据。
