const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { DesktopDriver } = require('./lib/desktop-driver.cjs');
const harnessHost = require('./lib/harness-driver.cjs');
const workflowHost = require('./lib/claude-plugin-workflow.cjs');
const discoverPlugin = require('./lib/claude-plugin-discovery.cjs');
const root = path.resolve(__dirname, '..');
const output = path.resolve(root, process.argv[2]);
const nativePackage = path.resolve(root, process.argv[3]);
const bundledPackage = path.resolve(root, process.argv[4]);
const dataDirectory = path.resolve(root, process.argv[5]);
for (const directory of [output,nativePackage,bundledPackage,dataDirectory]) assert(directory.toLowerCase().startsWith((root + path.sep).toLowerCase()));
const app = new DesktopDriver(root,path.resolve(root,process.argv[6] || 'target/debug/githubsp-desktop.exe'),output);
app.data = dataDirectory;
const pkgRequire = createRequire(path.join(root,'integrations/agent-tools/package.json'));
const { Client } = pkgRequire('@modelcontextprotocol/sdk/client/index.js');
const { StdioClientTransport } = pkgRequire('@modelcontextprotocol/sdk/client/stdio.js');
const checks = [];
const check = (name,value = true) => { assert.ok(value,name); checks.push(name); console.log(`通过：${name}`); };
(async () => {
  try { await fs.access(path.join(output,'result.json')); throw Error('请使用新的验收目录'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  await fs.mkdir(output,{recursive:true});
  let native,client,passed=false,failure;
  try {
    const boot = await app.launch();
    native = await harnessHost({ root,output,dataDirectory,pluginEntry:path.join(nativePackage,'harness.mjs'),environmentConfig:true });
    check('已安装 TGZ 原生插件通过环境配置发现全部工具', native.info.tools.length === 13 && native.info.workflowGuidance);
    check('已安装原生插件连接实际桌面版本', (await native.call('githubsp_status')).version === boot.version);
    client = new Client({name:'githubsp-package-check',version:'1.0.0'});
    await client.connect(new StdioClientTransport({command:process.execPath,args:[path.join(bundledPackage,'mcp.mjs'),'--data-dir',dataDirectory]}));
    const listed = await client.listTools();
    check('独立依赖分发中的 MCP 与原生插件发现相同工具', JSON.stringify(listed.tools.map(tool=>tool.name).sort()) === JSON.stringify([...native.info.tools].sort()));
    const call = async (name,args={}) => { const result=await client.callTool({name,arguments:args}); if(result.isError) throw Error(result.content[0].text); return result.structuredContent; };
    const shelf = await call('githubsp_software',{query:'lkuliuying/githubsp',page:1});
    check('分发包可查询存在和移走的真实文件', shelf.items.some(file=>file.fileState==='present') && shelf.items.some(file=>file.fileState==='missing'));
    const manifest = await call('githubsp_watchlist',{action:'export'});
    check('原生安装包与 MCP 分发导出相同关注清单', JSON.stringify(manifest) === JSON.stringify(await native.call('githubsp_watchlist',{action:'export'})));
    const status = await native.call('githubsp_status');
    const preview = await call('githubsp_watchlist',{action:'preview',manifest:JSON.stringify(manifest),directory:status.allowedDirectories[0],policy:'keep'});
    const imported = await call('githubsp_watchlist',{action:'import',manifest:JSON.stringify(manifest),directory:status.allowedDirectories[0],policy:'keep',fingerprint:preview.fingerprint});
    check('MCP 分发能够预览并合并重复清单', imported.imported && preview.items.every(item=>item.action==='keep'));
    check('TGZ 原生插件卸载重载清理工作流与工具', (await native.lifecycle('reload')).reloaded);
    const plugin = await discoverPlugin({root,output,dataDirectory,pluginDirectory:bundledPackage});
    check('Claude 实际插件加载器发现分发 Skill', plugin.commands.includes('githubsp:githubsp-release-workflows'));
    check('Claude 实际插件加载器连接分发 MCP 并发现相同工具', JSON.stringify([...plugin.server.tools].sort()) === JSON.stringify([...native.info.tools].sort()));
    if(process.argv.includes('--workflow')) {
      const before = (await app.invoke({type:'snapshot'})).totalTasks;
      const result = await workflowHost({root,output,dataDirectory,pluginDirectory:bundledPackage});
      check('Claude 插件新会话加载打包 Skill', result.calls.some(call=>call.name==='Skill' && JSON.stringify(call.input).includes('githubsp-release-workflows')));
      check('Claude 插件实际调用软件库且无失败结果', result.calls.some(call=>call.name.endsWith('githubsp_software')) && result.results.every(result=>!result.error));
      check('工作流复用已下载文件而不重复建任务', (await app.invoke({type:'snapshot'})).totalTasks === before);
    }
    passed = true;
  } catch(error) { failure=error.stack; throw error; }
  finally {
    await client?.close(); await native?.close(); try { await app.stop(); } finally { app.kill(); }
    await fs.writeFile(path.join(output,'result.json'),JSON.stringify({passed,checks,failure,nativePackage,bundledPackage,workflow:process.argv.includes('--workflow')},null,2));
  }
})().catch(error=>{console.error(error.stack);process.exitCode=1});
