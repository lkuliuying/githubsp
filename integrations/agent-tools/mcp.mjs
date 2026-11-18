#!/usr/bin/env node
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { AjvJsonSchemaValidator } from '@modelcontextprotocol/sdk/validation/ajv';
import { readFileSync } from 'node:fs';
import { pipeName, request } from './client.mjs';
import { tools, operation } from './tools.mjs';

async function main() {
  const args = process.argv.slice(2);
  if (args.length && !(args.length === 2 && args[0] === '--data-dir')) throw Error('用法：githubsp-mcp --data-dir <GitHubSP 实际数据目录>');
  const dataDirectory = args[1] ?? process.env.GITHUBSP_DATA_DIR;
  pipeName(dataDirectory);
  const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'));
  const server = new Server({ name: 'githubsp', version: pkg.version }, { capabilities: { tools: {} } });
  const validator = new AjvJsonSchemaValidator();
  const inputs = new Map(tools.map(tool => [tool.name, validator.getValidator(tool.parameters)]));
  server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: tools.map(tool => ({
    name: tool.name, description: tool.description, inputSchema: tool.parameters,
    outputSchema: { type: 'object', additionalProperties: true },
    annotations: { readOnlyHint: ['status', 'resolve', 'tasks', 'task', 'wait', 'updates', 'software'].includes(tool.op), destructiveHint: tool.op === 'control', openWorldHint: ['resolve', 'create', 'watch'].includes(tool.op), idempotentHint: ['status', 'resolve', 'tasks', 'task', 'wait', 'create', 'updates', 'update', 'projectLabels', 'software'].includes(tool.op) },
  })) }));
  server.setRequestHandler(CallToolRequestSchema, async (call, extra) => {
    const tool = tools.find(tool => tool.name === call.params.name);
    if (!tool) return { isError: true, content: [{ type: 'text', text: '未知 GitHubSP 工具' }] };
    try {
      const input = inputs.get(tool.name)(call.params.arguments ?? {});
      if (!input.valid) throw Error(`工具参数无效：${input.errorMessage}`);
      const result = await request(dataDirectory, operation(tool, input.data), { signal: extra.signal });
      return { structuredContent: result, content: [{ type: 'text', text: JSON.stringify(result) }] };
    } catch (error) {
      return { isError: true, content: [{ type: 'text', text: error instanceof Error ? error.message : String(error) }] };
    }
  });
  await server.connect(new StdioServerTransport());
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
