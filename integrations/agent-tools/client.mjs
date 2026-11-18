import { createHash, randomUUID } from 'node:crypto';
import { realpathSync } from 'node:fs';
import { isAbsolute } from 'node:path';
import { createConnection } from 'node:net';

export function pipeName(dataDirectory) {
  if (process.platform !== 'win32') throw new Error('GitHubSP 本机接入目前支持 Windows');
  if (typeof dataDirectory !== 'string' || !isAbsolute(dataDirectory)) throw new Error('请配置 GitHubSP 实际数据目录的绝对路径 dataDirectory');
  const normalized = realpathSync(dataDirectory).replace(/^\\\\\?\\/, '').replaceAll('/', '\\').replace(/[A-Z]/g, c => c.toLowerCase());
  return `\\\\.\\pipe\\githubsp-${createHash('sha256').update(normalized).digest('hex')}`;
}

export function request(dataDirectory, operation, { signal, timeoutMs = 95_000 } = {}) {
  signal?.throwIfAborted();
  const path = pipeName(dataDirectory);
  const id = randomUUID();
  const input = JSON.stringify({ version: 1, id, request: operation }) + '\n';
  if (Buffer.byteLength(input) > 1_048_576) throw new Error('GitHubSP 请求超过 1 MiB');
  return new Promise((resolve, reject) => {
    const socket = createConnection({ path });
    let chunks = [], size = 0, done = false;
    const finish = (error, value) => {
      if (done) return;
      done = true;
      clearTimeout(timer); signal?.removeEventListener('abort', aborted); socket.destroy();
      error ? reject(error) : resolve(value);
    };
    const aborted = () => finish(signal.reason ?? new Error('调用已取消；已接受的下载继续运行'));
    const timer = setTimeout(() => finish(new Error('GitHubSP 调用超时；请用任务 ID 或原 requestKey 查询，不要盲目重建')), timeoutMs);
    signal?.addEventListener('abort', aborted, { once: true });
    if (signal?.aborted) aborted();
    socket.on('connect', () => socket.write(input));
    socket.on('error', error => finish(new Error(['ENOENT', 'ECONNREFUSED'].includes(error.code)
      ? '无法连接 GitHubSP：请启动桌面应用，在 Agent 接入设置中启用并重启，核对 dataDirectory'
      : `GitHubSP 连接失败：${error.code ?? error.message}`)));
    socket.on('end', () => { if (!done) finish(new Error('GitHubSP 连接已断开；请重新查询原任务')); });
    socket.on('data', chunk => {
      size += chunk.length;
      if (size > 2 * 1024 * 1024) return finish(new Error('GitHubSP 响应超过 2 MiB'));
      chunks.push(chunk);
      if (!chunk.includes(10)) return;
      try {
        const response = JSON.parse(Buffer.concat(chunks).toString('utf8').trim());
        if (response.version !== 1 || response.id !== id || typeof response.ok !== 'boolean') throw new Error('GitHubSP 响应协议不匹配');
        response.ok ? finish(null, response.result) : finish(new Error(String(response.error)));
      } catch (error) { finish(error); }
    });
  });
}
