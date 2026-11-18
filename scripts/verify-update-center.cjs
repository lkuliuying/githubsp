const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { DatabaseSync } = require('node:sqlite');
const { DesktopDriver, until, delay } = require('./lib/desktop-driver.cjs');
const harnessHost = require('./lib/harness-driver.cjs');
const codexHost = require('./lib/codex-mcp-host.cjs');
const root = path.resolve(__dirname, '..');
const output = path.resolve(root, process.argv[2] || 'artifacts/verification/update-center');
assert(output.toLowerCase().startsWith((root + path.sep).toLowerCase()));
const app = new DesktopDriver(root, path.resolve(root, process.argv[3] || 'target/debug/githubsp-desktop.exe'), output);
const repository = 'lkuliuying/githubsp';
const checks = [];
const check = (name, value = true) => { assert.ok(value, name); checks.push(name); console.log(`通过：${name}`); };

function seedEarlierBaseline() {
  // 仅在脚本自建且已退出的隔离库内设置前一版本；不发布远程 Release。
  assert(app.data.startsWith(output + path.sep));
  const db = new DatabaseSync(path.join(app.data, 'tasks.sqlite3'));
  try {
    db.exec('BEGIN IMMEDIATE');
    const favorite = JSON.parse(db.prepare('SELECT payload FROM favorites WHERE repository=?').get(repository).payload);
    favorite.latest = { ...favorite.latest, id: 1, tag: 'fixture-earlier', url: `https://github.com/${repository}/releases/tag/fixture-earlier` };
    favorite.seen = [1];
    const state = JSON.parse(db.prepare("SELECT value FROM settings WHERE key='watch_state'").get().value);
    Object.assign(state.repositories[repository], { ackedReleaseId: 1, notifiedReleaseId: 1, skippedReleaseIds: [] });
    db.prepare('UPDATE favorites SET payload=? WHERE repository=?').run(JSON.stringify(favorite), repository);
    db.prepare("UPDATE settings SET value=? WHERE key='watch_state'").run(JSON.stringify(state));
    db.exec('COMMIT');
  } finally { db.close(); }
}

async function screenshot(name, width, dark) {
  await app.rpc('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false });
  await app.evaluate(`document.documentElement.classList.toggle('dark',${dark})`);
  await delay(250);
  check(`${name} 无横向溢出`, await app.evaluate('document.documentElement.scrollWidth<=innerWidth+1'));
  const picture = await app.rpc('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(output, `${name}.png`), Buffer.from(picture.data, 'base64'));
}

(async () => {
  try { await fs.access(path.join(output, 'result.json')); throw Error('请使用新的验收目录'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const downloads = path.join(output, 'downloads'); await fs.mkdir(downloads, { recursive: true });
  let native, codex, passed = false, failure, eventId, version;
  try {
    const boot = await app.launch(); version = boot.version;
    await app.invoke({ type: 'saveSettings', settings: { ...boot.snapshot.settings, agent: { enabled: true, allowedDirectories: [downloads] }, autoCheck: false } });
    await app.invoke({ type: 'favoriteAdd', input: `https://github.com/${repository}` });
    const baseline = await app.invoke({ type: 'updates', cursor: null, limit: 50, repository: null });
    check('首次实际关注只建立基线', baseline.events.length === 0);
    await app.stop(); seedEarlierBaseline(); await app.launch();
    native = await harnessHost({ root, output, dataDirectory: app.data });
    check('实际 Harness 发现新增更新工具', native.info.tools.includes('githubsp_updates') && native.info.tools.includes('githubsp_update') && native.info.tools.includes('githubsp_project_labels'));
    await native.call('githubsp_watch', { action: 'check', repository });
    const first = await native.call('githubsp_updates', { cursor: baseline.nextCursor, limit: 50 });
    const retry = await native.call('githubsp_updates', { cursor: baseline.nextCursor, limit: 50 });
    check('真实检查后只生成一条可重读事件', first.events.length === 1 && retry.events.length === 1 && first.events[0].id === retry.events[0].id);
    eventId = first.events[0].id;
    check('读取游标不会标记已读，列表省略正文', first.events[0].seenAt === null && first.events[0].notes === null && first.notesOmitted);
    const detail = await native.call('githubsp_update', { id: eventId, action: 'detail' });
    const source = await app.invoke({ type: 'browse', input: detail.release.url, page: 1, includePrerelease: false });
    check('详情保存完整真实 Release 说明并标识 API 元数据', detail.notes === source.releases[0].notes && detail.metadataComplete && detail.seenAt === null);
    await native.call('githubsp_project_labels', { repository, group: '开发工具', tags: ['Rust', '桌面'] });
    await app.evaluate('document.querySelector("[data-view=software]").click()');
    await until(() => app.evaluate('!!document.querySelector(".sw-tabs")'));
    await app.evaluate('[...document.querySelectorAll(".sw-tabs [role=tab]")].find(tab=>tab.textContent.includes("更新动态")).click()');
    await until(() => app.evaluate('document.querySelectorAll(".update-row").length===1'));
    check('真实桌面更新中心显示同一记录', await app.evaluate('document.querySelector(".update-row").textContent.includes("lkuliuying/githubsp")'));
    await screenshot('updates-1280-light', 1280, false); await screenshot('updates-760-dark', 760, true);
    await app.evaluate('document.querySelector(".update-open").click()');
    await until(() => app.evaluate('!!document.querySelector("[role=dialog] .markdown")'));
    await screenshot('update-detail-1280-light', 1280, false);
    check('打开原生详情不算已看到', (await native.call('githubsp_update', { id: eventId, action: 'detail' })).seenAt === null);
    await app.evaluate('document.querySelector("[aria-label=关闭对话框]").click()');
    await native.call('githubsp_update', { id: eventId, action: 'skip' });
    check('原生插件显式忽略同步到桌面记录', (await app.invoke({ type: 'updateDetail', id: eventId })).skipped);
    await app.stop(); await app.launch();
    const after = await native.call('githubsp_updates', { cursor: baseline.nextCursor, limit: 50 });
    check('重启保留游标、忽略和分组标签', after.events[0].id === eventId && after.events[0].skipped && (await native.call('githubsp_watch', { action: 'list' })).watchState.repositories[repository].group === '开发工具');
    await native.call('githubsp_watch', { action: 'check', repository });
    check('重复检查相同版本不重复记事件', (await native.call('githubsp_updates', { limit: 50 })).events.length === 1);
    codex = await codexHost({ root, output, dataDirectory: app.data });
    const resolved = await native.call('githubsp_resolve_release', { input: detail.release.url });
    const asset = resolved.release.assets.find(asset => asset.name === 'SHA256SUMS.txt');
    const created = await codex.call('githubsp_create_download', { resolutionId: resolved.resolutionId, assetId: asset.id, directory: downloads, requestKey: 'm3.three-clients' });
    const completed = await until(() => native.call('githubsp_get_task', { id: created.id }), task => ['completed','failed'].includes(task.status), 90_000);
    check('Harness 与 Codex MCP 及桌面同时共享下载队列', completed.status === 'completed' && completed.verification === 'official_match' && (await app.invoke({ type: 'snapshot' })).tasks.some(task => task.id === created.id));
    const downloaded = await native.call('githubsp_update', { id: eventId, action: 'detail' });
    check('已下载附件与已看到和忽略分别保留', downloaded.downloadedTasks.includes(created.id) && downloaded.skipped && downloaded.seenAt !== null);
    if (process.argv.includes('--mcp')) {
      const shared = await codex.call('githubsp_updates', { cursor: baseline.nextCursor, limit: 50 });
      check('MCP 同步新增能力后读取同一事件及处理结果', shared.events.length === 1 && shared.events[0].id === eventId && shared.events[0].skipped && shared.events[0].downloadedTasks.includes(created.id));
      await codex.call('githubsp_update', { id: eventId, action: 'unskip' });
      check('MCP 显式处理可由 Harness 重新读取', !(await native.call('githubsp_update', { id: eventId, action: 'detail' })).skipped);
    }
    const lifecycle = await native.lifecycle('reload'); check('新增工具重载无重复注册', lifecycle.reloaded);
    passed = true;
  } catch (error) { failure = error.stack; throw error; }
  finally {
    await Promise.allSettled([native?.close(), codex?.close()]); try { await app.stop(); } finally { app.kill(); }
    await fs.writeFile(path.join(output, 'result.json'), JSON.stringify({ passed, version, checks, eventId, failure, seededEarlierBaseline: true, realNetwork: !!eventId, realHarness: !!native, mcpUpdatesChecked: process.argv.includes('--mcp') }, null, 2));
  }
})().catch(error => { console.error(error.stack); process.exitCode = 1; });
