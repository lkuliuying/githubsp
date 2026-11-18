import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { mkdtemp, rmdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pipeName, request } from '../client.mjs';

test('连接失败明确提示启动，不创建服务', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'githubsp-absent-'));
  try { await assert.rejects(request(dir, { op: 'status' }), /请启动桌面应用/); }
  finally { await rmdir(dir); }
});

test('校验响应身份、传输超时与取消；不重放请求', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'githubsp-client-'));
  let mode = 'ok', count = 0;
  const sockets = new Set();
  const server = createServer(socket => {
    sockets.add(socket); socket.on('close', () => sockets.delete(socket));
    socket.on('data', input => {
      count++;
      const req = JSON.parse(input);
      if (mode === 'wait') return;
      socket.end(JSON.stringify({ version: 1, id: mode === 'bad' ? 'wrong' : req.id, ok: true, result: { running: true } }) + '\n');
    });
  });
  await new Promise(resolve => server.listen(pipeName(dir), resolve));
  try {
    assert.deepEqual(await request(dir, { op: 'status' }), { running: true });
    mode = 'bad'; await assert.rejects(request(dir, { op: 'status' }), /协议不匹配/);
    mode = 'wait'; await assert.rejects(request(dir, { op: 'status' }, { timeoutMs: 100 }), /原 requestKey 查询/);
    const controller = new AbortController();
    const pending = request(dir, { op: 'status' }, { signal: controller.signal });
    setTimeout(() => controller.abort(), 50);
    await assert.rejects(pending, /aborted/i);
    assert.equal(count, 4);
  } finally {
    for (const socket of sockets) socket.destroy();
    await new Promise(resolve => server.close(resolve)); await rmdir(dir);
  }
});
