const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const fs = require('node:fs/promises');
const path = require('node:path');

module.exports = function workflow({ root, output, dataDirectory, pluginDirectory }) {
  const executable = process.env.CLAUDE_EXE || 'C:/ProgramSoftware/nodejs/node_global/node_modules/@anthropic-ai/claude-code/bin/claude.exe';
  const prompt = '这是 GitHubSP 插件工作流验收。先用 Skill 工具加载 githubsp:githubsp-release-workflows。查找 lkuliuying/githubsp 的 v0.3.1 已下载文件 SHA256SUMS.txt，只复用本地存在的文件，不创建下载或安装。调用 githubsp_status 和 githubsp_software 后报告可用路径；失败如实报告。';
  return new Promise((resolve, reject) => {
    const child = spawn(executable, ['--settings','{"disableAllHooks":true,"autoMemoryEnabled":false}','--setting-sources','','--print','--verbose','--output-format','stream-json','--no-session-persistence',...(process.env.GITHUBSP_TEST_CLAUDE_MODEL ? ['--model',process.env.GITHUBSP_TEST_CLAUDE_MODEL] : []),'--plugin-dir',pluginDirectory,'--tools','Skill','--allowedTools','Skill,mcp__plugin_githubsp_githubsp__githubsp_status,mcp__plugin_githubsp_githubsp__githubsp_software','--permission-mode','dontAsk','--',prompt], { cwd:root, windowsHide:true, env:{ ...process.env, CLAUDE_CODE_SIMPLE:'', CLAUDE_CONFIG_DIR:path.join(output,'claude-workflow-home'), GITHUBSP_DATA_DIR:dataDirectory, ANTHROPIC_CUSTOM_HEADERS:[process.env.ANTHROPIC_CUSTOM_HEADERS,process.env.GITHUBSP_TEST_CLAUDE_EXTRA_HEADERS].filter(Boolean).join('\n') }, stdio:['ignore','pipe','pipe'] });
    const calls = [], results = [];
    let final, failure, stderr = '';
    const timer = setTimeout(() => { failure = Error('Claude 插件工作流超过 150 秒'); child.kill(); }, 150_000);
    child.on('error', error => { clearTimeout(timer); reject(error); });
    child.stderr.on('data', chunk => { stderr = (stderr + chunk.toString().replace(/Bearer\s+\S+|sk-[A-Za-z0-9_-]+/g, '[redacted]')).slice(-2000); });
    createInterface({ input:child.stdout }).on('line', line => {
      let event; try { event = JSON.parse(line); } catch { return; }
      if (event.type === 'system' && event.subtype === 'init') console.log(`Claude 插件初始化：${JSON.stringify((event.mcp_servers || []).filter(server => server.name?.includes('githubsp')))}`);
      if (event.type === 'assistant') for (const item of event.message?.content || []) if (item.type === 'tool_use' && (item.name === 'Skill' || item.name.includes('githubsp'))) { calls.push({ id:item.id, name:item.name, input:item.input }); console.log(`Claude 插件调用：${item.name}`); }
      if (event.type === 'user') for (const item of event.message?.content || []) if (item.type === 'tool_result' && calls.some(call => call.id === item.tool_use_id)) results.push({ id:item.tool_use_id, error:!!item.is_error, content:item.content });
      if (event.type === 'result') final = { subtype:event.subtype, isError:event.is_error, text:event.result, totalCostUsd:event.total_cost_usd, errors:event.errors };
    });
    child.on('exit', async code => {
      clearTimeout(timer);
      const receipt = { code, calls, results, final, stderr, failure:failure?.message };
      await fs.writeFile(path.join(output,'claude-plugin-workflow.json'),JSON.stringify(receipt,null,2));
      if (failure || code !== 0 || final?.isError) reject(failure || Error(`Claude 插件工作流失败：${JSON.stringify(final || stderr)}`)); else resolve(receipt);
    });
  });
};
