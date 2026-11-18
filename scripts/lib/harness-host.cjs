// 由已安装 Harness 的 Electron/Node 执行，使用真实 Cordis 与 ToolRuntime。
const path = require('node:path');
const { createRequire } = require('node:module');
const { pathToFileURL } = require('node:url');
const { createInterface } = require('node:readline');
const assert = require('node:assert/strict');

async function main() {
  const hostRoot = path.join(path.dirname(process.execPath), 'resources/app.asar/dsh');
  const host = createRequire(path.join(hostRoot, 'package.json'));
  const version = host('@deepseek-ai/dsh-tools/package.json').version;
  assert.equal(version, process.env.GITHUBSP_TEST_DSH_VERSION || require('../../integrations/agent-tools/package.json').engines.dsh);
  const { Context } = host('@deepseek-ai/cordis');
  const { SystemPrompt, renderPrompt } = host('@deepseek-ai/dsh-system-prompt');
  const { ToolRuntime } = host('@deepseek-ai/dsh-tools');
  const plugin = await import(pathToFileURL(process.argv[3] || path.resolve(__dirname, '../../integrations/agent-tools/harness.mjs')).href);
  const ctx = new Context();
  const prompt = new SystemPrompt(ctx, {});
  const tools = new ToolRuntime(ctx, version === '0.2.1-rc.2' ? { mode: 'native' } : {});
  if (version === '0.2.2-rc.1') await ctx.plugin(host('@deepseek-ai/dsh-tool-executor-native'), {});
  const config = { dataDirectory: process.argv[4] === 'env' ? '' : process.argv[2] };
  let runtime = await ctx.plugin(plugin, config);
  const names = async () => (await prompt.assemble({})).tools.filter(tool => tool.name.startsWith('githubsp_')).map(tool => tool.name);
  const toolCount = (await names()).length;
  const guidanceMarker = '需要获取 GitHub Release 软件或整理关注更新时';
  const hasGuidance = renderPrompt(await prompt.assemble({})).includes(guidanceMarker);
  const emit = message => process.stdout.write(JSON.stringify(message) + '\n');
  emit({ type: 'ready', version, tools: await names(), processId: process.pid, modelCalls: 0, workflowGuidance: hasGuidance });
  const lines = createInterface({ input: process.stdin });
  for await (const line of lines) {
    const req = JSON.parse(line);
    try {
      if (req.method === 'reload') {
        await runtime.dispose();
        assert.equal((await names()).length, 0, '卸载必须移除全部工具');
        assert.equal(renderPrompt(await prompt.assemble({})).includes(guidanceMarker), false, '卸载必须释放工作流提示');
        runtime = await ctx.plugin(plugin, config);
        const discovered = await names();
        assert.equal(discovered.length, toolCount); assert.equal(new Set(discovered).size, toolCount);
        if (hasGuidance) assert.equal(renderPrompt(await prompt.assemble({})).split(guidanceMarker).length - 1, 1);
        emit({ id: req.id, ok: true, value: { disposed: true, reloaded: true, tools: discovered } });
      } else if (req.method === 'dispose') {
        await runtime.dispose(); assert.equal((await names()).length, 0);
        emit({ id: req.id, ok: true, value: { remainingTools: 0 } });
      } else {
        const signal = new AbortController().signal;
        const result = version === '0.2.2-rc.1'
          ? (await tools.execute({ calls: [{ id: req.id, name: req.tool, arguments: JSON.stringify(req.args ?? {}) }], signal })).results[0].result
          : await tools.execute({ name: req.tool, arguments: req.args, callId: req.id, signal, agent: { session: { id: `isolated-native-${process.pid}` } } });
        const content = result.content.filter(item => item.type === 'text').map(item => item.text).join('\n');
        if (result.isError) throw Error(content);
        emit({ id: req.id, ok: true, value: JSON.parse(content) });
      }
    } catch (error) { emit({ id: req.id, ok: false, error: error.message }); }
  }
  await runtime.dispose();
}
main().catch(error => { console.error(error.stack); process.exitCode = 1; });
