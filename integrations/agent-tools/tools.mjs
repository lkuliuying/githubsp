import { updateTools } from './update-tools.mjs';
import { softwareTools } from './software-tools.mjs';
const text = description => ({ type: 'string', description });
const integer = (description, maximum) => ({ type: 'integer', minimum: 0, ...(maximum === undefined ? {} : { maximum }), description });
const enumeration = values => ({ type: 'string', enum: values });
const object = (properties, required = []) => ({ type: 'object', additionalProperties: false, properties, required });
const preference = object({ platform: enumeration(['windows', 'macos', 'linux', 'any']), architecture: enumeration(['x64', 'arm64', 'x86', 'any']), package: enumeration(['installer', 'portable', 'any']) }, ['platform', 'architecture', 'package']);
const rule = object({ autoDownload: { type: 'boolean' }, includePrerelease: { type: 'boolean' }, assetPattern: { anyOf: [text('可选通配符，支持 {version}'), { type: 'null' }] }, preference: { anyOf: [preference, { type: 'null' }] }, downloadDirectory: { type: ['string','null'], description: '项目专用的允许下载目录绝对路径' } });

const baseTools = [
  { name: 'githubsp_status', op: 'status', description: '检查 GitHubSP 是否运行、版本与用户允许的下载目录。需要桌面/托盘进程；本工具不启动应用。', parameters: object({}) },
  { name: 'githubsp_resolve_release', op: 'resolve', description: '解析公开 GitHub 仓库或固定 Release 地址，返回固定版本、附件身份与匹配理由。不确定时让用户选附件；读取本地已保存偏好。返回的 resolutionId 十分钟有效，重启失效。', parameters: object({ input: text('owner/repo 或公开 GitHub Release URL'), preference, pattern: text('附件名通配符'), includePrerelease: { type: 'boolean' } }, ['input']) },
  { name: 'githubsp_create_download', op: 'create', description: '将已解析的附件加入 GitHubSP 共享队列，只下载文件。提供稳定且唯一 requestKey；重试使用相同键。连接中断后先用原键查任务。目录必须已存在且在允许范围内；禁止执行下载文件。', parameters: object({ resolutionId: text('解析返回的 resolutionId'), assetId: integer('固定附件 ID'), directory: text('用户确认的允许目录绝对路径'), requestKey: text('1–128 位字母数字或 _-.:；同一任务重试保持不变') }, ['resolutionId', 'assetId', 'directory', 'requestKey']) },
  { name: 'githubsp_tasks', op: 'tasks', description: '分页查询共享下载历史，找到之前会话的任务。返回文件路径、进度和校验状态，不返回文件字节。', parameters: object({ page: { type: 'integer', minimum: 1 }, query: text('按仓库、版本或文件名搜索') }) },
  { name: 'githubsp_get_task', op: 'task', description: '按任务 ID 或原请求键查询已接受任务及完成回执。只提供 id、requestKey 之一。official_match 为官方摘要匹配；local_hash_only 仅是本地哈希。', parameters: object({ id: text('任务 ID'), requestKey: text('原幂等请求键') }) },
  { name: 'githubsp_wait_task', op: 'wait', description: '最多等待 30 秒，返回状态变化或当前状态。超时与断连不会取消下载；不要密集轮询。', parameters: object({ id: text('任务 ID'), afterRevision: integer('上次任务 revision'), timeoutMs: integer('0–30000 毫秒', 30_000) }, ['id', 'afterRevision', 'timeoutMs']) },
  { name: 'githubsp_control_task', op: 'control', description: '暂停、继续或取消允许目录中的任务。不删除已下载文件；取消是用户意图，等待超时不要自动取消。', parameters: object({ id: text('任务 ID'), action: enumeration(['pause', 'resume', 'cancel']) }, ['id', 'action']) },
  { name: 'githubsp_watch', op: 'watch', description: '管理 GitHubSP 本地关注：list 读取；add 收藏并建立当前基线；rule 调整既有项目规则；check 主动检查；dismiss 显式标记已读。读取不会标记已读。与远程 GitHub Star/Watch 无关。自动下载仅按用户明确要求启用。', parameters: object({ action: enumeration(['list', 'add', 'rule', 'check', 'dismiss']), repository: text('owner/repo；add/rule 必填'), rule }, ['action']) },
];

export const tools = [...baseTools, ...updateTools, ...softwareTools];
export function operation(tool, args) { return { ...args, op: tool.op }; }
