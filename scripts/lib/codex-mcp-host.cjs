const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const fs = require('node:fs/promises');
const path = require('node:path');
const { stopChild } = require('./desktop-driver.cjs');

module.exports = async function codexHost({ root, output, dataDirectory }) {
  const codexHome = path.join(output, 'codex-home');
  await fs.mkdir(codexHome, { recursive: true });
  await fs.cp(path.join(root, 'integrations/agent-tools/skills/githubsp-release-workflows'), path.join(codexHome, 'skills/githubsp-release-workflows'), { recursive: true });
  const child = spawn(process.env.CODEX_EXE || 'codex.exe', ['app-server', '--stdio'], {
    cwd: root, env: { ...process.env, CODEX_HOME: codexHome }, windowsHide: true, stdio: ['pipe','pipe','pipe'],
  });
  let sequence = 0;
  const pending = new Map();
  const fail = error => { for (const item of pending.values()) { clearTimeout(item.timer); item.reject(error); } pending.clear(); };
  child.on('error', fail); child.on('exit', code => fail(Error(`Codex app-server 退出：${code}`)));
  child.stderr.on('data', chunk => fs.appendFile(path.join(output, 'codex-stderr.log'), chunk).catch(console.error));
  createInterface({ input: child.stdout }).on('line', line => {
    let message;
    try { message = JSON.parse(line); } catch { return; }
    const item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id); clearTimeout(item.timer);
    message.error ? item.reject(Error(JSON.stringify(message.error))) : item.resolve(message.result);
  });
  const rpc = (method, params) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(`Codex ${method} 超时`)); }, 100_000);
    pending.set(id, { resolve, reject, timer }); child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
  });
  try {
    const init = await rpc('initialize', { clientInfo: { name: 'githubsp_acceptance', version: '1.0.0' }, capabilities: { experimentalApi: true } });
    child.stdin.write(JSON.stringify({ method: 'initialized', params: {} }) + '\n');
    const started = await rpc('thread/start', { cwd: root, ephemeral: true, approvalPolicy: 'never', sandbox: 'read-only', config: { mcp_servers: { githubsp: { command: process.execPath, args: [path.join(root, 'integrations/agent-tools/mcp.mjs'), '--data-dir', dataDirectory] } } } });
    const threadId = started.thread.id;
    return {
      info: init,
      list: () => rpc('mcpServerStatus/list', { threadId, serverName: 'githubsp' }),
      skills: () => rpc('skills/list', { cwds: [root], forceReload: true }),
      async call(tool, args = {}) {
        const value = await rpc('mcpServer/tool/call', { threadId, server: 'githubsp', tool, arguments: args });
        if (value.isError) throw Error(value.content.map(item => item.text || '').join('\n'));
        return value.structuredContent ?? JSON.parse(value.content[0].text);
      },
      close: () => stopChild(child),
    };
  } catch (error) { child.kill(); throw error; }
};
