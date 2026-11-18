const fs = require('node:fs/promises');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const { until, stopChild } = require('./desktop-driver.cjs');

module.exports = async function harnessHost({ root, output, dataDirectory, pluginEntry, environmentConfig = false }) {
  const executable = process.env.DSH_DESKTOP_EXE || 'C:/ProgramSoftware/Deepseek/DeepSeek Harness.exe';
  const child = spawn(executable, ['--expose-internals', path.join(__dirname, 'harness-host.cjs'), dataDirectory, pluginEntry || '', environmentConfig ? 'env' : 'config'], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1', GITHUBSP_DATA_DIR: dataDirectory }, cwd: root, windowsHide: true, stdio: ['pipe','pipe','pipe'],
  });
  let ready, failure, sequence = 0;
  const pending = new Map();
  const fail = error => { failure = error; for (const item of pending.values()) { clearTimeout(item.timer); item.reject(error); } pending.clear(); };
  child.on('error', fail); child.on('exit', code => fail(Error(`Harness 上下文退出：${code}`)));
  child.stderr.on('data', chunk => fs.appendFile(path.join(output, `harness-${child.pid}.log`), chunk).catch(console.error));
  createInterface({ input: child.stdout }).on('line', line => {
    let msg; try { msg = JSON.parse(line); } catch { return; }
    if (msg.type === 'ready') { ready = msg; return; }
    const item = pending.get(msg.id);
    if (item) { pending.delete(msg.id); clearTimeout(item.timer); msg.ok ? item.resolve(msg.value) : item.reject(Error(msg.error)); }
  });
  try {
    const info = await until(async () => { if (failure) throw failure; return ready; });
    const send = (method, tool, args) => new Promise((resolve, reject) => {
      const id = `${child.pid}-${++sequence}`;
      const timer = setTimeout(() => { pending.delete(id); reject(Error(`Harness ${tool || method} 超时`)); }, 100_000);
      pending.set(id, { resolve, reject, timer }); child.stdin.write(JSON.stringify({ id, method, tool, args }) + '\n');
    });
    return { info, call: (tool, args = {}) => send('call', tool, args), lifecycle: method => send(method), close: () => stopChild(child) };
  } catch (error) { child.kill(); throw error; }
};
