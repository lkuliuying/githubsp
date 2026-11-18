const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const fs = require('node:fs/promises');
const path = require('node:path');

// 只使用实际客户端的初始化/状态控制接口，不发送模型请求。
module.exports = function discover({ root, output, dataDirectory, pluginDirectory }) {
  return new Promise((resolve, reject) => {
    const executable = process.env.CLAUDE_EXE || 'C:/ProgramSoftware/nodejs/node_global/node_modules/@anthropic-ai/claude-code/bin/claude.exe';
    const child = spawn(executable, ['--settings','{"disableAllHooks":true,"autoMemoryEnabled":false}','--setting-sources','','--print','--input-format','stream-json','--output-format','stream-json','--verbose','--no-session-persistence','--plugin-dir',pluginDirectory,'--tools','Skill','--permission-mode','dontAsk'], {
      cwd:root, windowsHide:true, env:{ ...process.env, CLAUDE_CODE_SIMPLE:'', CLAUDE_CONFIG_DIR:path.join(output,'claude-discovery-home'), GITHUBSP_DATA_DIR:dataDirectory }, stdio:['pipe','pipe','pipe'],
    });
    let commands = [], server, version, failure, finished = false, poll;
    const send = (id, subtype) => child.stdin.write(JSON.stringify({ type:'control_request',request_id:id,request:{ subtype } }) + '\n');
    const timer = setTimeout(() => { failure = Error('Claude 插件发现超过 25 秒'); child.kill(); },25_000);
    child.on('error', error => { clearTimeout(timer); clearTimeout(poll); reject(error); });
    child.stdin.on('error', () => {});
    child.stderr.on('data', () => {});
    createInterface({ input:child.stdout }).on('line', line => {
      let event; try { event = JSON.parse(line); } catch { return; }
      if (event.type !== 'control_response') return;
      const envelope = event.response || {}, value = envelope.response || {};
      if (envelope.request_id === 'init') {
        commands = (value.commands || []).filter(command => command.name?.startsWith('githubsp:')).map(command => command.name);
        version = value.claude_code_version;
        send('status','mcp_status');
      } else if (envelope.request_id === 'status' && !finished) {
        const own = (value.mcpServers || []).find(item => item.name === 'plugin:githubsp:githubsp');
        if (own?.status === 'connected') {
          // 不保存账号信息或其他服务的配置。
          server = { name:own.name,status:own.status,version:own.serverInfo?.version,tools:(own.tools || []).map(tool => tool.name) };
          finished = true; child.stdin.end();
        } else poll = setTimeout(() => send('status','mcp_status'),250);
      }
    });
    child.on('spawn', () => send('init','initialize'));
    child.on('exit', async code => {
      clearTimeout(timer); clearTimeout(poll);
      const receipt = { code,version,commands,server,failure:failure?.message };
      try {
        await fs.writeFile(path.join(output,'claude-plugin-discovery.json'),JSON.stringify(receipt,null,2));
        if (failure || code !== 0 || !server) reject(failure || Error('Claude 未连接 GitHubSP 插件 MCP')); else resolve(receipt);
      } catch (error) { reject(error); }
    });
  });
};
