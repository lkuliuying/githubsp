import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { mkdtemp, rmdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { tools } from '../tools.mjs';

test('官方 SDK 通过 stdio 发现共用工具，业务错误不伪造成功', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'githubsp-mcp-'));
  const client = new Client({ name: 'githubsp-regression', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [fileURLToPath(new URL('../mcp.mjs', import.meta.url)), '--data-dir', directory] });
  try {
    await client.connect(transport);
    const listed = await client.listTools();
    assert.deepEqual(listed.tools.map(tool => tool.name), tools.map(tool => tool.name));
    assert.equal(listed.tools.find(tool => tool.name === 'githubsp_create_download').annotations.readOnlyHint, false);
    const result = await client.callTool({ name: 'githubsp_status', arguments: {} });
    assert.equal(result.isError, true); assert.match(result.content[0].text, /请启动桌面应用/);
    const unknown = await client.callTool({ name: 'githubsp_unknown', arguments: {} });
    assert.equal(unknown.isError, true);
    const invalid = await client.callTool({ name: 'githubsp_create_download', arguments: { op: 'status' } });
    assert.equal(invalid.isError, true); assert.match(invalid.content[0].text, /工具参数无效/);
  } finally { await client.close(); await rmdir(directory); }
});
