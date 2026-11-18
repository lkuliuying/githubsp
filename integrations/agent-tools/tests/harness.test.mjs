import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { mkdtemp, rmdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { apply, Config } from '../harness.mjs';
import { pipeName } from '../client.mjs';

function context() {
  const tools = new Map();
  return { tools, ctx: { tools: { register: tool => tools.set(tool.name, tool) }, systemPrompt: { section() {} }, inject() {} } };
}
test('可视配置拒绝相对目录，并保留空值的环境变量语义', async () => {
  assert.equal((await Config['~standard'].validate({ dataDirectory: 'C:/Data' })).issues, undefined);
  assert.equal((await Config['~standard'].validate({ dataDirectory: '' })).issues, undefined);
  assert.ok((await Config['~standard'].validate({ dataDirectory: 'relative' })).issues.length);
});
test('未配置时仍可加载插件和设置页面，工具明确报告配置缺失', () => {
  const { ctx, tools } = context();
  apply(ctx, { dataDirectory: '' });
  if (!process.env.GITHUBSP_DATA_DIR) assert.throws(() => tools.get('githubsp_status').execute({}, {}), /请配置 GitHubSP/);
});
test('目录引用更新后，后续工具调用连接新目录', async () => {
  const directories = await Promise.all([mkdtemp(join(tmpdir(), 'githubsp-setting-a-')), mkdtemp(join(tmpdir(), 'githubsp-setting-b-'))]);
  const servers = directories.map((directory, index) => createServer(socket => socket.once('data', bytes => {
    const input = JSON.parse(bytes.toString());
    socket.end(JSON.stringify({ version: 1, id: input.id, ok: true, result: { origin: index } }) + '\n');
  })));
  let current = directories[0];
  try {
    await Promise.all(servers.map((server,index) => new Promise(resolve => server.listen(pipeName(directories[index]),resolve))));
    const { ctx, tools } = context();
    apply(ctx, { dataDirectory: { get: () => current } });
    assert.deepEqual(await tools.get('githubsp_status').execute({}, {}), { origin: 0 });
    current = directories[1];
    assert.deepEqual(await tools.get('githubsp_status').execute({}, {}), { origin: 1 });
  } finally {
    await Promise.all(servers.map(server => new Promise(resolve => server.close(resolve))));
    await Promise.all(directories.map(directory => rmdir(directory)));
  }
});
