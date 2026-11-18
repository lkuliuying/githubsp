// 使用隔离数据和真实 WebView2/IPC 验收；下载的公开测试文件不会执行。
const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const { createHash } = require('node:crypto');
const net = require('node:net');

const root = path.resolve(__dirname, '..');
const executable = path.resolve(process.argv[2] || path.join(root, 'target/debug/githubsp-desktop.exe'));
const output = path.resolve(root, process.argv[3] || 'artifacts/verification/desktop-native');
assert(output.toLowerCase().startsWith((root + path.sep).toLowerCase()), '验收目录必须位于项目内');
const data = path.join(output, 'data');
const downloads = path.join(output, 'downloads');
const checks = [];
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
let child, socket, rpc, boot, passed = false;
let observedWebView = false, observedIpc = false, observedNetwork = false;
const check = (name, value) => { assert.ok(value, name); checks.push(name); console.log(`通过：${name}`); };
async function until(read, predicate = Boolean, timeout = 30000) {
  const deadline = Date.now() + timeout;
  let value;
  while (Date.now() < deadline) {
    value = await read();
    if (predicate(value)) return value;
    await delay(100);
  }
  throw Error(`等待超时：${JSON.stringify(value)}`);
}
async function evaluate(expression) {
  const result = await rpc('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}
const invoke = request => evaluate(`window.__TAURI_INTERNALS__.invoke('dispatch',{request:${JSON.stringify(request)}})`);
const click = selector => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
const clickText = text => evaluate(`(()=>{const e=[...document.querySelectorAll('button')].find(e=>e.getClientRects().length&&e.textContent.trim()===${JSON.stringify(text)});if(!e)throw Error('按钮未找到');e.click();})()`);
const fill = (selector, value) => evaluate(`(()=>{const e=document.querySelector(${JSON.stringify(selector)});Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e),'value').set.call(e,${JSON.stringify(value)});e.dispatchEvent(new Event('input',{bubbles:true}));})()`);
const choose = (selector, value) => evaluate(`(()=>{const e=document.querySelector(${JSON.stringify(selector)});e.value=${JSON.stringify(value)};e.dispatchEvent(new Event('change',{bubbles:true}));})()`);
async function nav(view) {
  await click(`[data-view="${view}"]`);
  await until(() => evaluate(`!!document.querySelector('.page-frame[data-page="${view}"]:not([hidden])')`));
}
async function launch() {
  const probe = net.createServer();
  await new Promise(resolve => probe.listen(0, '127.0.0.1', resolve));
  const port = probe.address().port;
  await new Promise(resolve => probe.close(resolve));
  let launchError;
  child = spawn(executable, ['--data-dir', data], {
    cwd: root, windowsHide: true,
    env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: path.join(output, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.once('error', error => { launchError = error; });
  for (const [stream, name] of [[child.stdout, 'stdout'], [child.stderr, 'stderr']])
    stream.on('data', chunk => { fs.appendFile(path.join(output, `${name}.log`), chunk).catch(error => console.error(error.message)); });
  const target = await until(async () => {
    if (launchError) throw launchError;
    if (child.exitCode !== null) throw Error(`桌面启动失败：${child.exitCode}`);
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      return targets.find(target => target.type === 'page' && !target.url.includes('notice'));
    } catch { return null; }
  });
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    const item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id); clearTimeout(item.timer);
    message.error ? item.reject(Error(JSON.stringify(message.error))) : item.resolve(message.result);
  };
  socket.onclose = () => {
    for (const item of pending.values()) { clearTimeout(item.timer); item.reject(Error('WebView2 调试连接关闭')); }
    pending.clear();
  };
  rpc = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(`${method} 超时`)); }, 60000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  await rpc('Runtime.enable'); await rpc('Page.enable');
  await until(() => evaluate('!!window.__TAURI_INTERNALS__'));
  check('连接到真实主窗口', await evaluate('window.__TAURI_INTERNALS__.metadata.currentWindow.label==="main"'));
  observedWebView = true;
  await until(() => evaluate('({url:location.href,ready:document.querySelectorAll("[data-view]").length>=4})'), value => value.ready);
  boot = await invoke({ type: 'bootstrap' });
  observedIpc = true;
}
async function stop() {
  if (socket?.readyState === WebSocket.OPEN) {
    await invoke({ type: 'exit' });
    await until(() => Promise.resolve(child.exitCode), code => code !== null, 10000);
    socket.close();
  }
}
async function screenshot(name, width, height, dark = false) {
  await rpc('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  await evaluate(`document.documentElement.classList.toggle('dark',${dark})`);
  await delay(350);
  check(`${name} 无页面横向溢出`, await evaluate('document.documentElement.scrollWidth<=innerWidth+1'));
  const picture = await rpc('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(output, `${name}.png`), Buffer.from(picture.data, 'base64'));
}
(async () => {
  try { await fs.access(path.join(output, 'result.json')); throw Error('验收结果已存在，请使用新目录'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  await fs.mkdir(downloads, { recursive: true });
  let failure;
  try {
    await launch();
    await invoke({ type: 'saveSettings', settings: { ...boot.snapshot.settings, limitKib: 1000, autoCheck: false, reduceMotion: true, closeToTray: false } });
    // 先确认状态推送已抵达，再首次挂载设置草稿，避免把传输时序当作显示错误。
    await nav('queue');
    await until(() => evaluate('document.querySelector(".header-stats")?.textContent.includes("1.024 MB/s")'));
    await nav('settings'); await click('#settings-tab-downloads');
    await until(() => evaluate('document.querySelector("#speed-limit")?.value==="1.024"'));
    check('旧 KiB 限速按十进制 MB/s 显示', true);
    await click('#settings-tab-network');
    await choose('#proxy-mode', 'custom');
    await until(() => evaluate('!!document.querySelector("#proxy-url")'));
    await fill('#proxy-url', 'http://127.0.0.1:7890');
    await screenshot('network-1280-light', 1280, 820);
    // 改回原值后网络设置没有改动，不需要保存；其余设置改完即自动保存。
    await choose('#proxy-mode', 'direct');
    await nav('queue');
    await until(() => evaluate('document.querySelector(".header-stats")?.textContent.includes("1.024 MB/s")'));
    check('队列与设置的限速数值一致', true);
    await nav('new');
    await fill('#source-url', 'https://github.com/lkuliuying/githubsp/releases/tag/v0.3.1');
    await clickText('读取版本');
    await until(() => evaluate('document.querySelectorAll(".asset-row input:checked").length===1'));
    check('真实 Release 只勾选一个安装附件', await evaluate('document.querySelector(".asset-row input:checked").parentElement.textContent.includes("Setup.exe")'));
    await choose('.asset-preference-panel [aria-label="分发类型"]', 'any');
    await until(() => evaluate('document.querySelector(".asset-preference-actions")?.textContent.includes("有多个附件")'));
    check('歧义时不自动选择', await evaluate('document.querySelectorAll(".asset-row input:checked").length===0'));
    await choose('.asset-preference-panel [aria-label="分发类型"]', 'portable');
    await until(() => evaluate('document.querySelector(".asset-row input:checked")?.parentElement.textContent.includes("Portable.zip")'));
    await clickText('收藏并保存偏好');
    await until(() => invoke({ type: 'snapshot' }), state => state.watchState.repositories['lkuliuying/githubsp']?.rule.preference?.package === 'portable');
    check('原生 IPC 保存项目便携版偏好', true);
    await screenshot('preferences-1280-light', 1280, 820);
    await screenshot('preferences-760-dark', 760, 560, true);
    await rpc('Emulation.clearDeviceMetricsOverride');
    const state = await invoke({ type: 'snapshot' });
    await invoke({ type: 'saveSettings', settings: { ...state.settings, limitKib: 64 } });
    const checksumUrl = 'https://github.com/lkuliuying/githubsp/releases/download/v0.3.1/SHA256SUMS.txt';
    const summary = await invoke({ type: 'create', urls: [checksumUrl], directory: downloads, route: null });
    const checksumId = summary.items[0].taskId;
    const checksum = await until(() => invoke({ type: 'snapshot' }), state => state.tasks.some(task => task.id === checksumId && ['completed','failed'].includes(task.status)), 90000);
    const checksumTask = checksum.tasks.find(task => task.id === checksumId);
    check('真实附件下载并通过官方摘要校验', checksumTask.status === 'completed' && checksumTask.verification === 'verified');
    observedNetwork = true;
    const sums = await fs.readFile(checksumTask.finalPath, 'utf8');
    const installer = (await invoke({ type: 'browse', input: 'https://github.com/lkuliuying/githubsp/releases/tag/v0.3.1', page: 1, includePrerelease: false })).releases[0].assets.find(asset => asset.name.endsWith('Setup.exe'));
    const created = await invoke({ type: 'create', urls: [installer.url], directory: downloads, route: null });
    const taskId = created.items[0].taskId;
    await until(() => invoke({ type: 'snapshot' }), state => state.tasks.some(task => task.id === taskId && task.status === 'downloading' && task.downloaded > 32768), 90000);
    await invoke({ type: 'action', id: taskId, action: 'pause' });
    const paused = await until(() => invoke({ type: 'snapshot' }), state => state.tasks.some(task => task.id === taskId && task.status === 'paused'));
    const pausedBytes = paused.tasks.find(task => task.id === taskId).downloaded;
    check('真实下载暂停并保存进度', pausedBytes > 0);
    await stop();
    await launch();
    const restored = await invoke({ type: 'snapshot' });
    check('重启保留暂停状态及进度', restored.tasks.some(task => task.id === taskId && task.status === 'paused' && task.downloaded >= pausedBytes));
    check('重启保留项目偏好', restored.watchState.repositories['lkuliuying/githubsp'].rule.preference.package === 'portable');
    await invoke({ type: 'saveSettings', settings: { ...restored.settings, limitKib: 0 } });
    await invoke({ type: 'action', id: taskId, action: 'resume' });
    const completed = await until(() => invoke({ type: 'snapshot' }), state => state.tasks.some(task => task.id === taskId && ['completed','failed'].includes(task.status)), 60000);
    const task = completed.tasks.find(task => task.id === taskId);
    check('重启后续传完成', task.status === 'completed');
    const actual = createHash('sha256').update(await fs.readFile(task.finalPath)).digest('hex');
    const expected = sums.split(/\r?\n/).find(line => line.endsWith(installer.name))?.split(/\s+/)[0]?.toLowerCase();
    check('续传成品与官方校验文件一致', actual === expected && task.verification === 'verified');
    await fs.writeFile(path.join(output, 'download-receipt.json'), JSON.stringify({ taskId, filename: task.filename, bytes: task.downloaded, pausedBytes, sha256: actual, verification: task.verification }, null, 2));
    passed = true;
  } catch (error) { failure = error.stack; throw error; }
  finally {
    try { await stop(); } catch (error) { console.error(`退出检查：${error.message}`); }
    if (child && child.exitCode === null) child.kill();
    await fs.writeFile(path.join(output, 'result.json'), JSON.stringify({ passed, version: boot?.version, checks, failure, realWebView: observedWebView, realIpc: observedIpc, realNetwork: observedNetwork, cleanSystemInstall: false }, null, 2));
  }
})().catch(error => { console.error(error.stack); process.exitCode = 1; });
