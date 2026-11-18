const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const fs = require('node:fs/promises');
const path = require('node:path');

module.exports = function claudeHost({ root, output, config, downloads, onResume }) {
  const prompt = `这是 GitHubSP 插件接入验收，只使用提供的 GitHubSP MCP 工具，不运行文件或命令。按顺序执行：1. 调用 githubsp_status。2. 解析 https://github.com/lkuliuying/githubsp/releases/tag/v0.3.1，偏好 windows/x64/installer。3. 用返回的固定 resolutionId 和推荐附件 assetId 创建下载，directory=${JSON.stringify(downloads)}，requestKey=claude.get-tool.1。4. 重复同一创建请求核对 ID 相同。5. 暂停该任务，查询到 paused；然后 resume，使用 wait_task（单次最多 30000 毫秒）等待 completed 或 failed 并查询回执。6. 使用 watch add 添加 lkuliuying/githubsp，rule 保存 includePrerelease=false、autoDownload=false、preference={platform:windows,architecture:x64,package:portable}，再 list 确认。工具失败应如实停止并报告。最后仅给出任务 ID、状态、验证状态和规则是否保存。`;
  const executable = process.env.CLAUDE_EXE || 'C:/ProgramSoftware/nodejs/node_global/node_modules/@anthropic-ai/claude-code/bin/claude.exe';
  return new Promise((resolve, reject) => {
    const child = spawn(executable, ['--bare', '--print', '--verbose', '--output-format', 'stream-json', '--no-session-persistence', '--strict-mcp-config', '--mcp-config', config, '--tools', '', '--allowedTools', 'mcp__githubsp__*', '--permission-mode', 'dontAsk', '--', prompt], { cwd: root, windowsHide: true, stdio: ['ignore','pipe','pipe'] });
    const calls = [], results = [];
    let final, stderr = '', failure;
    const timer = setTimeout(() => { failure = Error('Claude Code 验收超过 240 秒'); child.kill(); }, 240_000);
    child.on('error', error => { clearTimeout(timer); reject(error); });
    child.stderr.on('data', chunk => { stderr = (stderr + chunk.toString()).slice(-4000); });
    createInterface({ input: child.stdout }).on('line', line => {
      let event;
      try { event = JSON.parse(line); } catch { return; }
      if (event.type === 'assistant') for (const item of event.message?.content || []) {
        if (item.type === 'tool_use' && item.name.startsWith('mcp__githubsp__')) {
          calls.push({ id: item.id, name: item.name, input: item.input });
          if (item.name.endsWith('githubsp_control_task') && item.input.action === 'resume') onResume().catch(error => { failure = error; child.kill(); });
        }
      }
      if (event.type === 'user') for (const item of event.message?.content || []) if (item.type === 'tool_result') results.push({ id: item.tool_use_id, error: !!item.is_error, content: item.content });
      if (event.type === 'result') final = { subtype: event.subtype, isError: event.is_error, text: event.result, totalCostUsd: event.total_cost_usd, errors: event.errors };
    });
    child.on('exit', async code => {
      clearTimeout(timer);
      const receipt = { code, calls, results, final, stderr, failure: failure?.message };
      await fs.writeFile(path.join(output, 'claude-host.json'), JSON.stringify(receipt, null, 2));
      if (failure || code !== 0 || final?.isError || !calls.length) reject(failure || Error(`Claude Code 未完成真实 MCP 调用，退出码 ${code}：${JSON.stringify(final || stderr)}`));
      else resolve(receipt);
    });
  });
};
