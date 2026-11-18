import Schema from '@deepseek-ai/schemastery';
import { request } from './client.mjs';
import { tools, operation } from './tools.mjs';

export const name = 'dsh-githubsp';
export const inject = ['tools', 'systemPrompt'];
export const Config = Schema.object({
  dataDirectory: Schema.string().pattern(/^(?:$|[a-z]:[\\/]|\\\\)/i).default('').volatile()
    .description('GitHubSP 实际数据目录；在 GitHubSP 的数据设置中查看，留空时读取 GITHUBSP_DATA_DIR。'),
});

export function apply(ctx, config) {
  // 可视设置通过 volatile 引用更新；每次调用读取当前值，避免沿用旧连接目录。
  const directory = () => {
    const value = config?.dataDirectory;
    return (typeof value?.get === 'function' ? value.get() : value) || process.env.GITHUBSP_DATA_DIR;
  };
  ctx.inject(['settings'], ctx => ctx.settings.configure({ auto: false }, ctx.fiber));
  ctx.systemPrompt.section({ name: 'githubsp-workflows', order: 10160, interpolate: false, text: '需要获取 GitHub Release 软件或整理关注更新时，先检查 githubsp_status 和 githubsp_software，复用合适且存在的本地文件。下载先解析固定版本；存在歧义时请用户选择附件。创建使用稳定 requestKey；超时或断连先按原键查任务，不换键盲目重建。结果给出路径、版本和校验状态，不称为已安装。更新简报用 githubsp_updates 游标重读并附 Release 来源，读取不算已读；用户要求处理时才调用 seen/skip。关注清单导入先预览冲突和本机根目录，用户确认后使用同一 fingerprint 导入。' });
  for (const tool of tools) {
    // 注册归属 Cordis 插件作用域；卸载由宿主撤销，下载仍由 GitHubSP 管理。
    ctx.tools.register({
      name: tool.name, description: tool.description, parameters: tool.parameters,
      output: { schema: { type: 'object', additionalProperties: true }, render: (_args, value) => [{ type: 'text', text: JSON.stringify(value) }] },
      execute: (args, exec) => request(directory(), operation(tool, args), { signal: exec.signal }),
      isConcurrencySafe: () => true,
    });
  }
}
