const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { createRequire } = require('node:module');
const { DesktopDriver, until } = require('./lib/desktop-driver.cjs');
const codexHost = require('./lib/codex-mcp-host.cjs');
const claudeHost = require('./lib/claude-mcp-host.cjs');
const root = path.resolve(__dirname, '..');
const pkgRequire = createRequire(path.join(root, 'integrations/agent-tools/package.json'));
const { Client } = pkgRequire('@modelcontextprotocol/sdk/client/index.js');
const { StdioClientTransport } = pkgRequire('@modelcontextprotocol/sdk/client/stdio.js');
const output = path.resolve(root, process.argv[2] || 'artifacts/verification/mcp-hosts');
assert(output.toLowerCase().startsWith((root + path.sep).toLowerCase()));
const app = new DesktopDriver(root, path.resolve(root, process.argv[3] || 'target/debug/githubsp-desktop.exe'), output);
const checks = [];
const check = (name, value = true) => { assert.ok(value, name); checks.push(name); console.log(`通过：${name}`); };

(async () => {
  try { await fs.access(path.join(output, 'result.json')); throw Error('请使用新的验收目录'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const downloads = path.join(output, 'downloads');
  await fs.mkdir(downloads, { recursive: true });
  let codex, client, passed = false, failure, receipt, claude;
  const connect = async () => {
    client = new Client({ name: 'githubsp-lifecycle-check', version: '1.0.0' });
    await client.connect(new StdioClientTransport({ command: process.execPath, args: [path.join(root, 'integrations/agent-tools/mcp.mjs'), '--data-dir', app.data] }));
    return client;
  };
  try {
    const boot = await app.launch();
    await app.invoke({ type: 'saveSettings', settings: { ...boot.snapshot.settings, agent: { enabled: true, allowedDirectories: [downloads] }, limitKib: 64, autoCheck: false } });
    await app.stop(); await app.launch();
    codex = await codexHost({ root, output, dataDirectory: app.data });
    const listed = await codex.list();
    await fs.writeFile(path.join(output, 'codex-discovery.json'), JSON.stringify(listed, null, 2));
    check('实际 Codex app-server 发现 MCP 工具', JSON.stringify(listed).includes('githubsp_create_download'));
    const status = await codex.call('githubsp_status');
    check('实际 Codex MCP 调用到桌面共享服务', status.running);
    if (!process.argv.includes('--claude-only')) {
    const release = await codex.call('githubsp_resolve_release', { input: 'https://github.com/lkuliuying/githubsp/releases/tag/v0.3.1', preference: { platform: 'windows', architecture: 'x64', package: 'installer' } });
    const asset = release.release.assets.find(asset => asset.url === release.recommendation.selectedUrls[0]);
    const args = { resolutionId: release.resolutionId, assetId: asset.id, directory: downloads, requestKey: 'mcp.get-tool.1' };
    await assert.rejects(codex.call('githubsp_create_download', { ...args, directory: root }), /允许范围/);
    check('Codex MCP 写入受到相同允许目录限制');
    const task = await codex.call('githubsp_create_download', args);
    await connect();
    const duplicate = await client.callTool({ name: 'githubsp_create_download', arguments: args });
    check('另一个 MCP 连接重试复用同一任务', duplicate.structuredContent.id === task.id);
    await until(() => codex.call('githubsp_get_task', { id: task.id }), item => item.downloaded > 32768 && item.status === 'downloading', 90_000);
    await codex.call('githubsp_control_task', { id: task.id, action: 'pause' });
    const paused = await until(() => codex.call('githubsp_get_task', { id: task.id }), item => item.status === 'paused');
    await client.close(); await connect();
    check('MCP 断连重连按原键查询，不重建下载', (await client.callTool({ name: 'githubsp_get_task', arguments: { requestKey: args.requestKey } })).structuredContent.id === task.id);
    await codex.call('githubsp_watch', { action: 'add', repository: 'lkuliuying/githubsp' });
    await codex.call('githubsp_watch', { action: 'rule', repository: 'lkuliuying/githubsp', rule: { autoDownload: false, includePrerelease: false, preference: { platform: 'windows', architecture: 'x64', package: 'portable' } } });
    await app.stop(); await app.launch();
    const restored = await codex.call('githubsp_get_task', { requestKey: args.requestKey });
    check('Codex 在桌面重启后恢复原任务状态及进度', restored.id === task.id && restored.status === 'paused' && restored.downloaded >= paused.downloaded);
    const watch = await codex.call('githubsp_watch', { action: 'list' });
    check('Codex MCP 复用持久关注规则', watch.watchState.repositories['lkuliuying/githubsp'].rule.preference.package === 'portable');
    const snapshot = await app.invoke({ type: 'snapshot' });
    await app.invoke({ type: 'saveSettings', settings: { ...snapshot.settings, limitKib: 0 } });
    await codex.call('githubsp_control_task', { id: task.id, action: 'resume' });
    receipt = await until(() => codex.call('githubsp_get_task', { id: task.id }), item => ['completed','failed'].includes(item.status), 90_000);
    check('Codex MCP 真实下载续传完成并匹配官方摘要', receipt.status === 'completed' && receipt.verification === 'official_match');
    check('MCP 与原生插件采用相同回执哈希语义', createHash('sha256').update(await fs.readFile(receipt.path)).digest('hex') === receipt.sha256 && receipt.sha256 === receipt.officialSha256);
    }
    await fs.writeFile(path.join(output, 'claude-mcp.json'), JSON.stringify({ mcpServers: { githubsp: { command: process.execPath, args: [path.join(root, 'integrations/agent-tools/mcp.mjs'), '--data-dir', app.data] } } }, null, 2));
    const beforeClaude = await app.invoke({ type: 'snapshot' });
    await app.invoke({ type: 'saveSettings', settings: { ...beforeClaude.settings, limitKib: 64 } });
    claude = await claudeHost({ root, output, config: path.join(output, 'claude-mcp.json'), downloads, onResume: async () => {
      const state = await app.invoke({ type: 'snapshot' });
      await app.invoke({ type: 'saveSettings', settings: { ...state.settings, limitKib: 0 } });
    } });
    for (const name of ['githubsp_status', 'githubsp_resolve_release', 'githubsp_create_download', 'githubsp_get_task', 'githubsp_control_task', 'githubsp_watch']) check(`实际 Claude Code 调用 ${name}`, claude.calls.some(call => call.name.endsWith(name)));
    const fromClaude = await codex.call('githubsp_get_task', { requestKey: 'claude.get-tool.1' });
    check('Claude 创建的公开附件任务在 Codex 中完成且官方校验通过', fromClaude.status === 'completed' && fromClaude.verification === 'official_match');
    check('Claude 工具调用没有未解决错误', claude.results.every(result => !result.error));
    passed = true;
  } catch (error) { failure = error.stack; throw error; }
  finally {
    await client?.close(); await codex?.close();
    try { await app.stop(); } finally { app.kill(); }
    await fs.writeFile(path.join(output, 'result.json'), JSON.stringify({ passed, checks, receipt, failure, codexVersion: codex?.info, claudeCode: claude ? 'actual-model' : 'pending', modelCalls: claude ? 'Claude Code 实际回合，详见 claude-host.json' : 0 }, null, 2));
  }
})().catch(error => { console.error(error.stack); process.exitCode = 1; });
