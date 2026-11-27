import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import type {Request, Response, Transport} from '../src/domain/types';
import {Client} from '../src/ipc/client';
import {McpPanel} from '../src/ui/McpPanel';

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});

function button(label: string) {
  const found = [...host.querySelectorAll('button')].find(b => b.textContent === label);
  if (!found) throw Error('Missing button: ' + label);
  return found;
}

async function fill(el: HTMLInputElement, value: string) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', {bubbles: true}));
  });
}

it('manages per-profile MCP servers, clears token input, discovers tools, toggles allowlist, and removes server', async () => {
  const requests: Request[] = [];
  const servers: Array<Record<string, unknown>> = [];

  const transport: Transport = {
    mode: 'desktop',
    subscribe: async () => () => {},
    async dispatch<T>(r: Request): Promise<Response<T>> {
      requests.push(r);
      let result: unknown;
      switch (r.command) {
        case 'profile.list':
          result = {items: [{id: 'p1', displayName: 'Ada'}], nextCursor: ''};
          break;
        case 'autonomous.instance.list':
          result = {items: [{profileId: 'auto-1', displayName: 'Pilot'}], archivedItems: []};
          break;
        case 'mcp.server.list':
          result = {profileId: r.profileId, items: servers};
          break;
        case 'mcp.server.save': {
          const existing = servers.find(s => s.id === r.payload.serverId);
          if (existing) {
            existing.allowedTools = r.payload.allowedTools ?? [];
            result = {profileId: r.profileId, server: existing};
          } else {
            const created = {
              id: 'mcp-1',
              revision: 1,
              name: r.payload.name,
              normalizedEndpoint: r.payload.endpoint,
              hasCredential: Boolean(r.payload.apiKey),
              allowMutating: Boolean(r.payload.allowMutating),
              allowedTools: r.payload.allowedTools ?? [],
              enabled: true,
              discoveredTools: [],
            };
            servers.push(created);
            result = {profileId: r.profileId, server: created};
          }
          break;
        }
        case 'mcp.server.discover': {
          const srv = servers[0];
          const tools = [{name: 'kb.search', description: 'Search KB', declaredMutating: false, untrustedExternalMetadata: true}];
          srv.discoveredTools = tools;
          result = {profileId: r.profileId, server: srv, capabilities: {protocolVersion: '2025-11-25', serverName: 'FakeMCP', supportsTools: true}, tools};
          break;
        }
        case 'mcp.server.delete': {
          servers.splice(0, servers.length);
          result = {profileId: r.profileId, serverId: r.payload.serverId, deleted: true};
          break;
        }
        default:
          throw Error('Unexpected command: ' + r.command);
      }
      return {contractVersion: 1, requestId: r.requestId, ok: true, result: structuredClone(result) as T};
    },
  };

  await act(async () => root.render(<McpPanel client={new Client(transport)} locale="en" desktop initialProfileId="p1"/>));
  expect(host.textContent).toContain('No MCP servers configured');

  const nameInput = host.querySelector<HTMLInputElement>('form input:not([type])')!;
  const urlInput = host.querySelector<HTMLInputElement>('form input[type="url"]')!;
  const keyInput = host.querySelector<HTMLInputElement>('form input[type="password"]')!;
  await fill(nameInput, 'Local KB');
  await fill(urlInput, 'https://example.com/mcp');
  await fill(keyInput, 'sk-mcp-secret');
  await act(async () => button('Add MCP server').click());

  expect(keyInput.value).toBe('');
  expect(host.textContent).toContain('Local KB');
  expect(host.textContent).toContain('Read-only');
  expect(host.textContent).toContain('Token in Vault');

  await act(async () => button('Discover tools').click());
  expect(host.textContent).toContain('kb.search');
  expect(button('Allow tool')).toBeTruthy();

  await act(async () => button('Allow tool').click());
  expect(button('Revoke tool')).toBeTruthy();

  await act(async () => button('Remove').click());
  expect(host.textContent).toContain('No MCP servers configured');
  expect(requests.map(r => r.command)).toContain('mcp.server.discover');
  expect(requests.map(r => r.command)).toContain('mcp.server.delete');
});
