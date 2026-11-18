// 仅操作由本脚本启动、使用隔离数据目录的真实桌面实例。
const fs = require('node:fs/promises');
const path = require('node:path');
const net = require('node:net');
const { spawn } = require('node:child_process');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(read, predicate = Boolean, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let value;
  while (Date.now() < deadline) {
    value = await read();
    if (predicate(value)) return value;
    await delay(100);
  }
  throw Error(`等待超时：${JSON.stringify(value)}`);
}
async function stopChild(child) {
  if (!child) return;
  if (child.exitCode === null && child.signalCode === null) {
    await new Promise(resolve => {
      const timer = setTimeout(() => { child.kill(); resolve(); }, 5000);
      child.once('exit', () => { clearTimeout(timer); resolve(); });
      child.stdin?.end();
    });
  }
  // 原生子进程可能把日志句柄交给自己的工作进程；不保留测试父进程的管道引用。
  child.stdin?.destroy(); child.stdout?.destroy(); child.stderr?.destroy(); child.unref();
}

class DesktopDriver {
  constructor(root, executable, output) { Object.assign(this, { root, executable, output, data: path.join(output, 'data') }); }
  async launch() {
    const probe = net.createServer();
    await new Promise(resolve => probe.listen(0, '127.0.0.1', resolve));
    const port = probe.address().port;
    this.port = port;
    await new Promise(resolve => probe.close(resolve));
    let launchError;
    this.child = spawn(this.executable, ['--data-dir', this.data], {
      cwd: this.root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: path.join(this.output, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` },
    });
    this.child.once('error', error => { launchError = error; });
    for (const [stream, name] of [[this.child.stdout, 'stdout'], [this.child.stderr, 'stderr']])
      stream.on('data', chunk => fs.appendFile(path.join(this.output, `${name}.log`), chunk).catch(error => console.error(error.message)));
    const target = await until(async () => {
      if (launchError) throw launchError;
      if (this.child.exitCode !== null) throw Error(`桌面进程退出：${this.child.exitCode}`);
      try { return (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find(item => item.type === 'page' && !item.url.includes('notice')); }
      catch { return null; }
    });
    this.socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => { this.socket.onopen = resolve; this.socket.onerror = reject; });
    let sequence = 0;
    const pending = new Map();
    this.socket.onmessage = event => {
      const message = JSON.parse(event.data), item = pending.get(message.id);
      if (!item) return;
      pending.delete(message.id); clearTimeout(item.timer);
      message.error ? item.reject(Error(JSON.stringify(message.error))) : item.resolve(message.result);
    };
    this.socket.onclose = () => {
      for (const item of pending.values()) { clearTimeout(item.timer); item.reject(Error('WebView2 调试连接关闭')); }
      pending.clear();
    };
    this.rpc = (method, params = {}) => new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { pending.delete(id); reject(Error(`${method} 超时`)); }, 95_000);
      pending.set(id, { resolve, reject, timer }); this.socket.send(JSON.stringify({ id, method, params }));
    });
    await this.rpc('Runtime.enable'); await this.rpc('Page.enable');
    await until(() => this.evaluate('!!window.__TAURI_INTERNALS__&&document.querySelectorAll("[data-view]").length>=4'));
    this.boot = await this.invoke({ type: 'bootstrap' });
    return this.boot;
  }
  async evaluate(expression) {
    const value = await this.rpc('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (value.exceptionDetails) throw Error(JSON.stringify(value.exceptionDetails));
    return value.result.value;
  }
  invoke(request) { return this.evaluate(`window.__TAURI_INTERNALS__.invoke('dispatch',{request:${JSON.stringify(request)}})`); }
  async stop() {
    if (this.socket?.readyState === WebSocket.OPEN) {
      // 原生窗口可能先于异步 IPC 回执关闭；仍需确认本进程实际退出。
      try { await this.invoke({ type: 'exit' }); }
      catch (error) { if (!/调试连接关闭/.test(error.message)) throw error; }
      await until(async () => this.child.exitCode, code => code !== null, 10_000);
      this.socket.close();
      this.child.stdout?.destroy(); this.child.stderr?.destroy();
    }
  }
  kill() { if (this.child && this.child.exitCode === null) this.child.kill(); this.child?.stdout?.destroy(); this.child?.stderr?.destroy(); this.socket?.close(); }
}
module.exports = { DesktopDriver, until, delay, stopChild };
