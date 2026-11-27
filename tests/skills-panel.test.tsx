import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import type {Request, Response, Transport} from '../src/domain/types';
import {App} from '../src/ui/App';

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  localStorage.clear();
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  vi.stubGlobal('matchMedia', () => ({matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn()}));
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

it('renders SkillsPanel in Settings, displays untrusted requested vs granted capabilities, and enables/removes a skill', async () => {
  const requests: Request[] = [];
  let skills = [
    {
      id: 'skill_1',
      revision: 1,
      name: 'Forum Digest Helper',
      description: 'Summarizes public threads',
      instructions: 'Summarize public threads.',
      instructionsSha256: 'a'.repeat(64),
      source: 'imported' as const,
      requestedCapabilities: ['community_publish'],
      grantedCapabilities: ['community_read_only'],
      enabled: false,
    },
  ];

  const transport: Transport = {
    mode: 'desktop',
    subscribe: async () => () => {},
    async dispatch<T>(r: Request): Promise<Response<T>> {
      requests.push(r);
      let result: unknown;
      switch (r.command) {
        case 'kernel.status.get': result = {lastEventSequence: 0}; break;
        case 'app.preferences.get': result = {revision: 1, locale: 'en', themeId: 'orbital', themeVariant: 'light'}; break;
        case 'profile.list': result = {items: [{id: 'p1', displayName: 'Ada', revision: 1, status: 'ready', mode: 'standard', primaryRouteId: 'r1'}], nextCursor: ''}; break;
        case 'profile.archived.list':
        case 'material.list':
        case 'conversation.materials.list':
        case 'conversation.messages.list':
        case 'autonomous.instance.list':
        case 'backup.list':
        case 'connection.list':
        case 'route.list': result = {items: [], nextCursor: ''}; break;
        case 'conversation.list': result = {items: [{id: 'c1', profileId: 'p1', revision: 1, language: 'en', title: 'Main', draftText: '', status: 'open'}], nextCursor: ''}; break;
        case 'conversation.get': result = {id: 'c1', profileId: 'p1', revision: 1, language: 'en', title: 'Main', draftText: '', status: 'open'}; break;
        case 'conversation.chat.latest': result = {found: false}; break;
        case 'chat.settings.get': result = {routeId: 'r1', routeRevision: 1, contextTokens: 8192, modelId: 'm', outputTokens: 1024, inputReserveTokens: 512}; break;
        case 'chat.vision.get': result = {support: 'unknown'}; break;
        case 'backup.restore.status': result = {pending: false}; break;
        case 'diagnostics.snapshot.get': result = {schemaVersion: 20, profilesCount: 1}; break;
        case 'mcp.server.list': result = {profileId: 'p1', items: [], onlineStatusKnown: false}; break;
        case 'skills.list': result = {profileId: 'p1', items: skills}; break;
        case 'skills.save': {
          const enabled = Boolean(r.payload.enabled);
          skills = skills.map(s => s.id === r.payload.skillId ? {...s, enabled, source: 'imported'} : s);
          result = {profileId: 'p1', skill: skills[0]};
          break;
        }
        case 'skills.delete': {
          skills = skills.filter(s => s.id !== r.payload.skillId);
          result = {profileId: 'p1', skillId: r.payload.skillId, deleted: true};
          break;
        }
        default: throw Error('Unexpected command ' + r.command);
      }
      return {contractVersion: 1, requestId: r.requestId, ok: true, result: structuredClone(result) as T};
    },
  };

  await act(async () => root.render(<App transport={transport}/>));
  await act(async () => button('Settings').click());

  expect(host.textContent).toContain('Profile skills (no self-grant)');
  expect(host.textContent).toContain('Forum Digest Helper');
  expect(host.textContent).toContain('Requested (untrusted): community_publish');
  expect(host.textContent).toContain('Granted: community_read_only');

  // Enable skill explicitly -> sends source: 'user' and enabled: true
  await act(async () => button('Enable').click());
  const enableReq = requests.find(r => r.command === 'skills.save');
  expect(enableReq?.payload.source).toBe('user');
  expect(enableReq?.payload.enabled).toBe(true);
  expect(host.textContent).toContain('Enabled');

  // Remove skill -> sends skills.delete
  await act(async () => button('Remove').click());
  expect(requests.some(r => r.command === 'skills.delete' && r.payload.skillId === 'skill_1')).toBe(true);
  expect(host.textContent).toContain('No skills configured for this profile.');
});
