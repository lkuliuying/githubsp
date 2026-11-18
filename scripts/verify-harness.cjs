const fs = require('node:fs/promises');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const { createHash } = require('node:crypto');
const assert = require('node:assert/strict');
const { DesktopDriver, until } = require('./lib/desktop-driver.cjs');

const root = path.resolve(__dirname, '..');
const output = path.resolve(root, process.argv[2] || 'artifacts/verification/harness-native');
const executable = path.resolve(root, process.argv[3] || 'target/debug/githubsp-desktop.exe');
const harness = process.env.DSH_DESKTOP_EXE || 'C:/ProgramSoftware/Deepseek/DeepSeek Harness.exe';
assert(output.toLowerCase().startsWith((root + path.sep).toLowerCase()));
const app = new DesktopDriver(root, executable, output);
const checks = [], hosts = [];
const check = (label, value = true) => { assert.ok(value, label); checks.push(label); console.log(`通过：${label}`); };
async function host() {
  const child = spawn(harness, ['--expose-internals', path.join(__dirname, 'lib/harness-host.cjs'), app.data], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
  });
  let ready, failure, sequence = 0;
  const pending = new Map();
  child.once('error', error => { failure = error; });
  child.stderr.on('data', chunk => fs.appendFile(path.join(output, `harness-${child.pid}.log`), chunk).catch(console.error));
  createInterface({ input: child.stdout }).on('line', line => {
    let msg;
    try { msg = JSON.parse(line); } catch { return; }
    if (msg.type === 'ready') { ready = msg; return; }
    const item = pending.get(msg.id);
    if (item) { pending.delete(msg.id); clearTimeout(item.timer); msg.ok ? item.resolve(msg.value) : item.reject(Error(msg.error)); }
  });
  const info = await until(async () => { if (failure) throw failure; if (child.exitCode !== null) throw Error(`Harness 退出 ${child.exitCode}`); return ready; });
  const send = (method, tool, args) => new Promise((resolve, reject) => {
    const id = `${child.pid}-${++sequence}`;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(`Harness ${tool || method} 超时`)); }, 100_000);
    pending.set(id, { resolve, reject, timer }); child.stdin.write(JSON.stringify({ id, method, tool, args }) + '\n');
  });
  const handle = { child, info, call: (tool, args = {}) => send('call', tool, args), lifecycle: method => send(method), close: () => child.stdin.end() };
  hosts.push(handle); return handle;
}

(async () => {
  try { await fs.access(path.join(output, 'result.json')); throw Error('请使用新的验收目录'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const downloads = path.join(output, 'downloads');
  await fs.mkdir(downloads, { recursive: true });
  let passed = false, failure, version, receipt;
  try {
    const boot = await app.launch(); version = boot.version;
    await app.invoke({ type: 'saveSettings', settings: { ...boot.snapshot.settings, agent: { enabled: true, allowedDirectories: [downloads] }, limitKib: 64, autoCheck: false } });
    await app.stop(); await app.launch();
    const first = await host(), second = await host();
    check('实际 Harness 0.2.1-rc.2 发现基础原生工具', first.info.tools.length >= 8 && second.info.tools.length >= 8);
    const status = await first.call('githubsp_status');
    check('两个独立宿主上下文连接同一桌面队列', status.running && (await second.call('githubsp_status')).queueRevision === status.queueRevision);
    const release = await first.call('githubsp_resolve_release', { input: 'https://github.com/lkuliuying/githubsp/releases/tag/v0.3.1', preference: { platform: 'windows', architecture: 'x64', package: 'installer' } });
    check('真实 Release 固定版本且唯一选中安装包', release.release.tag === 'v0.3.1' && release.recommendation.state === 'selected' && release.recommendation.selectedUrls.length === 1);
    const asset = release.release.assets.find(asset => asset.url === release.recommendation.selectedUrls[0]);
    const create = { resolutionId: release.resolutionId, assetId: asset.id, directory: downloads, requestKey: 'native.get-tool.1' };
    await assert.rejects(first.call('githubsp_create_download', { ...create, directory: root, requestKey: 'outside' }), /允许范围/);
    check('真实服务拒绝范围外目录');
    const [one, two] = await Promise.all([first.call('githubsp_create_download', create), second.call('githubsp_create_download', create)]);
    check('多宿主并发重试仅创建一个任务', one.id === two.id);
    const id = one.id;
    await until(() => second.call('githubsp_get_task', { id }), task => task.status === 'downloading' && task.downloaded > 32768, 90_000);
    await first.call('githubsp_control_task', { id, action: 'pause' });
    const paused = await until(() => second.call('githubsp_get_task', { id }), task => task.status === 'paused');
    check('原生工具暂停真实下载并保留进度', paused.downloaded > 0);
    const desktopCreate = await app.invoke({ type: 'create', urls: ['https://github.com/lkuliuying/githubsp/releases/download/v0.3.1/SHA256SUMS.txt'], directory: downloads, route: null });
    const desktopId = desktopCreate.items[0].taskId;
    const desktopFile = await until(() => second.call('githubsp_get_task', { id: desktopId }), task => ['completed','failed'].includes(task.status), 90_000);
    check('桌面创建的任务在原生插件可查询且共用调度', desktopFile.status === 'completed' && desktopFile.verification === 'official_match');
    const reload = await first.lifecycle('reload');
    check('真实 Cordis 卸载和重载无工具泄漏', reload.disposed && reload.reloaded);
    const third = await host();
    check('新宿主上下文按请求键找到原任务', (await third.call('githubsp_get_task', { requestKey: create.requestKey })).id === id);
    await first.call('githubsp_watch', { action: 'add', repository: 'lkuliuying/githubsp' });
    const rule = { includePrerelease: false, autoDownload: false, preference: { platform: 'windows', architecture: 'x64', package: 'portable' } };
    await first.call('githubsp_watch', { action: 'rule', repository: 'lkuliuying/githubsp', rule });
    check('另一个宿主读取保存的本地关注偏好', (await second.call('githubsp_watch', { action: 'list' })).watchState.repositories['lkuliuying/githubsp'].rule.preference.package === 'portable');
    await app.stop();
    await assert.rejects(third.call('githubsp_status'), /请启动桌面应用/);
    check('GitHubSP 未运行时返回明确启动提示');
    await app.launch();
    const restored = await third.call('githubsp_get_task', { requestKey: create.requestKey });
    check('桌面重启后恢复任务 ID、暂停状态与字节', restored.id === id && restored.status === 'paused' && restored.downloaded >= paused.downloaded);
    const reused = await third.call('githubsp_resolve_release', { input: 'https://github.com/lkuliuying/githubsp/releases/tag/v0.3.1' });
    check('重启后解析复用已保存关注偏好', reused.recommendation.selectedUrls.length === 1 && reused.recommendation.selectedUrls[0].endsWith('Portable.zip'));
    await third.call('githubsp_watch', { action: 'check', repository: 'lkuliuying/githubsp' });
    check('重启后可再次检查已有关注');
    const state = await app.invoke({ type: 'snapshot' });
    await app.invoke({ type: 'saveSettings', settings: { ...state.settings, limitKib: 0 } });
    await second.call('githubsp_control_task', { id, action: 'resume' });
    await first.lifecycle('dispose');
    check('卸载原生插件不删除共享队列任务', (await third.call('githubsp_get_task', { id })).id === id);
    receipt = await until(async () => {
      const value = await third.call('githubsp_get_task', { id });
      return third.call('githubsp_wait_task', { id, afterRevision: value.revision, timeoutMs: 1000 });
    }, task => ['completed', 'failed'].includes(task.status), 90_000);
    check('插件卸载和桌面重启后真实续传完成且官方摘要匹配', receipt.status === 'completed' && receipt.verification === 'official_match');
    const digest = createHash('sha256').update(await fs.readFile(receipt.path)).digest('hex');
    check('结构化回执的实际路径、字节和本地哈希均可核对', digest === receipt.sha256 && digest === receipt.officialSha256 && (await fs.stat(receipt.path)).size === receipt.downloaded);
    const repeated = await third.call('githubsp_create_download', { ...create, resolutionId: reused.resolutionId });
    check('完成后同键再次调用仍复用原任务', repeated.id === id && repeated.status === 'completed');
    const desktop = await app.invoke({ type: 'snapshot' });
    check('桌面与所有宿主共享相同的两个任务', desktop.tasks.length === 2 && desktop.tasks.some(task => task.id === id) && desktop.tasks.some(task => task.id === desktopId));
    const disable = await app.invoke({ type: 'saveSettings', settings: { ...desktop.settings, agent: { ...desktop.settings.agent, enabled: false } } });
    await assert.rejects(second.call('githubsp_status'), /已关闭/);
    check('桌面关闭接入立即拒绝新请求', !!disable);
    passed = true;
  } catch (error) { failure = error.stack; throw error; }
  finally {
    for (const host of hosts) { host.close(); if (host.child.exitCode === null) host.child.kill(); }
    try { await app.stop(); } finally { app.kill(); }
    await fs.writeFile(path.join(output, 'result.json'), JSON.stringify({ passed, version, checks, receipt, failure, harnessVersion: hosts[0]?.info.version, realHostRuntime: !!hosts.length, modelCalls: 0, profileWrites: 0 }, null, 2));
  }
})().catch(error => { console.error(error.stack); process.exitCode = 1; });
