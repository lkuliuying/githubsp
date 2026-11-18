const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { DesktopDriver, until, delay } = require('./lib/desktop-driver.cjs');
const harnessHost = require('./lib/harness-driver.cjs');
const codexHost = require('./lib/codex-mcp-host.cjs');
const claudeWorkflow = require('./lib/claude-plugin-workflow.cjs');
const root = path.resolve(__dirname, '..');
const output = path.resolve(root, process.argv[2] || 'artifacts/verification/software-library');
assert(output.toLowerCase().startsWith((root + path.sep).toLowerCase()));
const app = new DesktopDriver(root, path.resolve(root, process.argv[3] || 'target/debug/githubsp-desktop.exe'), output);
const repository = 'lkuliuying/githubsp';
const checks = [];
const check = (name, value = true) => { assert.ok(value, name); checks.push(name); console.log(`通过：${name}`); };
const clickText = value => app.evaluate(`(()=>{const button=[...document.querySelectorAll('button')].find(button=>button.getClientRects().length&&button.textContent.trim()===${JSON.stringify(value)});if(!button)throw Error('按钮未找到');button.click()})()`);
const fill = (selector, value, textarea = false) => app.evaluate(`(()=>{const element=document.querySelector(${JSON.stringify(selector)});Object.getOwnPropertyDescriptor(${textarea ? 'HTMLTextAreaElement' : 'HTMLInputElement'}.prototype,'value').set.call(element,${JSON.stringify(value)});element.dispatchEvent(new Event('input',{bubbles:true}))})()`);
async function screenshot(name, width, dark) {
  await app.rpc('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false });
  await app.evaluate(`document.documentElement.classList.toggle('dark',${dark})`); await delay(250);
  check(`${name} 无横向溢出`, await app.evaluate('document.documentElement.scrollWidth<=innerWidth+1'));
  const picture = await app.rpc('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(output, `${name}.png`), Buffer.from(picture.data, 'base64'));
}
(async () => {
  try { await fs.access(path.join(output, 'result.json')); throw Error('请使用新的验收目录'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const previousRoot = path.join(output, 'previous-downloads'), importedRoot = path.join(output, 'Imported downloads');
  await fs.mkdir(previousRoot, { recursive: true }); await fs.mkdir(importedRoot, { recursive: true });
  let native, codex, passed = false, failure, receipt, version;
  try {
    const boot = await app.launch(); version = boot.version;
    await app.invoke({ type: 'saveSettings', settings: { ...boot.snapshot.settings, agent: { enabled: true, allowedDirectories: [importedRoot] }, autoCheck: false } });
    await app.invoke({ type: 'favoriteAdd', input: `https://github.com/${repository}` });
    const created = await app.invoke({ type: 'create', urls: [`https://github.com/${repository}/releases/download/v0.3.1/SHA256SUMS.txt`], directory: previousRoot, route: null });
    const previousId = created.items[0].taskId;
    await until(() => app.invoke({ type: 'snapshot' }), state => state.tasks.some(task => task.id === previousId && task.status === 'completed'), 90_000);
    await app.stop(); await app.launch();
    native = await harnessHost({ root, output, dataDirectory: app.data });
    check('实际 Harness 注册软件库与迁移工具和工作流提示', native.info.tools.includes('githubsp_software') && native.info.tools.includes('githubsp_watchlist') && native.info.workflowGuidance);
    const software = await native.call('githubsp_software', { query: repository, page: 1 });
    check('原生工具找到桌面已下载文件及版本', software.items.length === 1 && software.items[0].fileState === 'present' && software.items[0].tag === 'v0.3.1' && software.items[0].latestTag === 'v0.3.1');
    const portable = await native.call('githubsp_watchlist', { action: 'export' });
    const serialized = JSON.stringify(portable);
    check('导出关注清单不含机器目录和应用设置', !serialized.includes(previousRoot) && !serialized.includes(importedRoot) && !serialized.includes('downloadDirectory') && !serialized.includes('allowedDirectories') && !serialized.includes('network'));
    portable.projects[0].group = '原生工具迁移'; portable.projects[0].tags = ['便携'];
    const text = JSON.stringify(portable);
    await assert.rejects(native.call('githubsp_watchlist', { action: 'preview', manifest: text, policy: 'replace' }), /重新选择/);
    await assert.rejects(native.call('githubsp_watchlist', { action: 'preview', manifest: text, directory: previousRoot, policy: 'replace' }), /允许范围/);
    check('清单导入必须重新指定本机允许目录');
    const firstPreview = await native.call('githubsp_watchlist', { action: 'preview', manifest: text, directory: importedRoot, policy: 'replace' });
    check('导入预览明确冲突和覆盖', firstPreview.items[0].state === 'conflict' && firstPreview.items[0].action === 'replace');
    await app.invoke({ type: 'projectLabels', repository, group: '预览后变化', tags: [] });
    await assert.rejects(native.call('githubsp_watchlist', { action: 'import', manifest: text, directory: importedRoot, policy: 'replace', fingerprint: firstPreview.fingerprint }), /重新预览/);
    check('预览后项目变更拒绝旧指纹');
    const preview = await native.call('githubsp_watchlist', { action: 'preview', manifest: text, directory: importedRoot, policy: 'replace' });
    await native.call('githubsp_watchlist', { action: 'import', manifest: text, directory: importedRoot, policy: 'replace', fingerprint: preview.fingerprint });
    let watched = await native.call('githubsp_watch', { action: 'list' });
    check('导入规则绑定新根目录而不复用旧机器目录', watched.watchState.repositories[repository].group === '原生工具迁移' && watched.watchState.repositories[repository].rule.downloadDirectory === await fs.realpath(importedRoot));
    const current = await app.invoke({ type: 'snapshot' });
    await app.invoke({ type: 'saveSettings', settings: { ...current.settings, watchAutoDownload: true } });
    const rule = { autoDownload: false, includePrerelease: false, assetPattern: null, preference: null, downloadDirectory: importedRoot };
    await native.call('githubsp_watch', { action: 'rule', repository, rule });
    watched = await native.call('githubsp_watch', { action: 'list' });
    check('单项目仅提醒能覆盖全局自动下载', watched.watchState.repositories[repository].rule.autoDownload === false);
    await native.call('githubsp_watch', { action: 'rule', repository, rule: { ...rule, autoDownload: true } });
    check('项目专用允许目录不受最近旧目录干扰', (await native.call('githubsp_watch', { action: 'list' })).watchState.repositories[repository].rule.autoDownload);
    await native.call('githubsp_watch', { action: 'rule', repository, rule });
    await app.evaluate('document.querySelector("[data-view=software]").click()');
    await clickText('关注清单');
    await until(() => app.evaluate('document.querySelector("#watchlist-json")?.value.startsWith("{")'));
    await clickText('导入');
    const uiManifest = await native.call('githubsp_watchlist', { action: 'export' }); uiManifest.projects[0].group = '界面导入';
    await fill('#watchlist-json', JSON.stringify(uiManifest, null, 2), true); await fill('#watchlist-directory', importedRoot);
    await app.evaluate('(()=>{const select=document.querySelector("#watchlist-policy");select.value="replace";select.dispatchEvent(new Event("change",{bubbles:true}))})()');
    await clickText('预览合并');
    await until(() => app.evaluate('!!document.querySelector(".watchlist-preview")'));
    check('真实桌面清单预览显示冲突和本机根目录', await app.evaluate('document.querySelector(".watchlist-preview").textContent.includes("冲突")'));
    await screenshot('watchlist-preview-1280-light', 1280, false);
    await clickText('确认导入（1 项）').catch(() => clickText('确认导入（1项）'));
    await until(() => app.evaluate('!document.querySelector("[role=dialog]")'));
    check('真实桌面导入与原生工具读取一致', (await native.call('githubsp_watch', { action: 'list' })).watchState.repositories[repository].group === '界面导入');
    const original = software.items[0].path;
    const canonicalPrevious = await fs.realpath(previousRoot);
    assert(path.dirname(original).toLowerCase() === canonicalPrevious.toLowerCase());
    const moved = path.join(canonicalPrevious, 'moved-SHA256SUMS.txt'); assert(path.dirname(moved) === canonicalPrevious);
    await fs.rename(original, moved);
    check('文件被移走后不再显示存在', (await native.call('githubsp_software', { query: repository, page: 1 })).items[0].fileState === 'missing');
    // 我的软件在页面重新显示时刷新文件状态。
    await app.evaluate('document.querySelector("[data-view=queue]").click()');
    await app.evaluate('document.querySelector("[data-view=software]").click()');
    await until(() => app.evaluate('!!document.querySelector("[data-file-state=missing]")'));
    await screenshot('software-missing-1280-light', 1280, false); await screenshot('software-missing-760-dark', 760, true);
    const resolution = await native.call('githubsp_resolve_release', { input: `https://github.com/${repository}/releases/tag/v0.3.1` });
    const asset = resolution.release.assets.find(asset => asset.name === 'SHA256SUMS.txt');
    const downloaded = await native.call('githubsp_create_download', { resolutionId: resolution.resolutionId, assetId: asset.id, directory: importedRoot, requestKey: 'm4.public-file' });
    receipt = await until(() => native.call('githubsp_get_task', { id: downloaded.id }), task => ['completed','failed'].includes(task.status), 90_000);
    check('新根目录中的真实下载可被软件库重新发现', receipt.status === 'completed' && receipt.verification === 'official_match' && (await native.call('githubsp_software', { query: repository, page: 1 })).items.some(file => file.path === receipt.path && file.fileState === 'present'));
    await app.stop(); await app.launch();
    check('重启保留迁移目录、标签和本地文件关联', (await native.call('githubsp_watch', { action: 'list' })).watchState.repositories[repository].rule.downloadDirectory === await fs.realpath(importedRoot) && (await native.call('githubsp_software', { query: repository, page: 1 })).items.some(file => file.taskId === receipt.id && file.fileState === 'present'));
    if (process.argv.includes('--mcp')) {
      codex = await codexHost({ root, output, dataDirectory: app.data });
      const skills = await codex.skills();
      await fs.writeFile(path.join(output, 'codex-skill-discovery.json'), JSON.stringify(skills, null, 2));
      check('Codex 新隔离会话发现打包工作流 Skill', skills.data.some(entry => entry.skills.some(skill => skill.name === 'githubsp-release-workflows')));
      const shared = await codex.call('githubsp_software', { query: repository, page: 1 });
      check('MCP 软件库与 Harness 返回同一文件状态', shared.items.some(file => file.taskId === receipt.id && file.fileState === 'present') && shared.items.some(file => file.taskId === previousId && file.fileState === 'missing'));
      const exported = await codex.call('githubsp_watchlist', { action: 'export' });
      check('MCP 与原生插件导出相同可迁移清单', JSON.stringify(exported) === JSON.stringify(await native.call('githubsp_watchlist', { action: 'export' })));
      if (process.argv.includes('--workflow')) {
        const before = (await app.invoke({ type: 'snapshot' })).totalTasks;
        const workflow = await claudeWorkflow({ root, output, dataDirectory:app.data, pluginDirectory:path.join(root,'integrations/agent-tools') });
        check('Claude 新会话加载插件工作流 Skill', workflow.calls.some(call => call.name === 'Skill' && JSON.stringify(call.input).includes('githubsp-release-workflows')));
        check('Claude 通过插件提供的 MCP 查询本地软件库', workflow.calls.some(call => call.name.startsWith('mcp__plugin_githubsp_githubsp__') && call.name.endsWith('githubsp_software')));
        check('插件工作流复用现有文件且未创建新下载', (await app.invoke({ type:'snapshot' })).totalTasks === before && workflow.results.every(result => !result.error));
      }
    }
    const lifecycle = await native.lifecycle('reload'); check('软件工具与工作流提示重载无泄漏', lifecycle.reloaded);
    passed = true;
  } catch (error) { failure = error.stack; throw error; }
  finally {
    await Promise.allSettled([native?.close(), codex?.close()]); try { await app.stop(); } finally { app.kill(); }
    await fs.writeFile(path.join(output, 'result.json'), JSON.stringify({ passed, version, checks, receipt, failure, mcpChecked: process.argv.includes('--mcp'), realHarness: !!native }, null, 2));
  }
})().catch(error => { console.error(error.stack); process.exitCode = 1; });
