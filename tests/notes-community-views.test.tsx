import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import type {Request, Response, Transport} from '../src/domain/types';
import {browserTransport} from '../src/ipc/browser';
import {App} from '../src/ui/App';

function desktopWithNotesAndContacts(hasCommunityDispatch = false) {
  const requests: Request[] = [];
  const profile = {id: 'p1', displayName: 'Ada', revision: 1, status: 'ready', mode: 'standard', primaryRouteId: 'r1'};
  const conversation = {id: 'c1', profileId: 'p1', revision: 1, language: 'en', title: 'Main chat', draftText: '', status: 'open'};
  const records = [
    {
      record: {id: 'mem-rel-1', revision: 1, kind: 'relationship', state: 'active', createdAt: '2026-11-25T10:00:00Z', subjectRef: 'peer-maya'},
      metadata: {title: 'Maya (Forum Peer)'},
      version: {id: 'v1', content: 'Met in local study group.'},
    },
    {
      record: {id: 'mem-ep-1', revision: 1, kind: 'episodic', state: 'active', createdAt: '2026-11-25T10:00:00Z'},
      metadata: {title: 'Session summary'},
      version: {id: 'v2', content: 'Not a contact.'},
    },
  ];

  const transport: Transport = {
    mode: 'desktop',
    subscribe: async () => () => {},
    ...(hasCommunityDispatch ? {communityDispatch: async <T,>(): Promise<Response<T>> => ({contractVersion: 1, requestId: 'c', ok: true, result: {} as T})} : {}),
    async dispatch<T>(r: Request): Promise<Response<T>> {
      requests.push(r);
      let result: unknown;
      switch (r.command) {
        case 'kernel.status.get': result = {lastEventSequence: 0}; break;
        case 'app.preferences.get': result = {revision: 1, locale: 'en', themeId: 'orbital', themeVariant: 'light'}; break;
        case 'profile.list': result = {items: [profile], nextCursor: ''}; break;
        case 'profile.archived.list':
        case 'material.list':
        case 'conversation.materials.list':
        case 'conversation.messages.list':
        case 'autonomous.instance.list': result = {items: [], nextCursor: ''}; break;
        case 'conversation.list': result = {items: [conversation], nextCursor: ''}; break;
        case 'conversation.get': result = conversation; break;
        case 'conversation.chat.latest': result = {found: false}; break;
        case 'chat.settings.get': result = {routeId: 'r1', routeRevision: 1, contextTokens: 8192, modelId: 'm', outputTokens: 1024, inputReserveTokens: 512}; break;
        case 'chat.vision.get': result = {support: 'unknown'}; break;
        case 'memory.working.get': result = {record: {id: 'w1', revision: 1, state: 'empty'}, primary: {contextTokens: 8192, verified: true}, attempts: [], proposals: [], decisions: [], profileRevision: 1, capacityPolicy: {kind: 'bounded', tokens: 300}, scopeId: 'a'.repeat(64), scribeMode: 'disabled', formatRepairCount: 0}; break;
        case 'memory.records.list': result = {items: records, nextCursor: ''}; break;
        case 'memory.layers.settings.get': result = {profileRevision: 1, memorySettings: {episodic: {kind: 'bounded', tokens: 300}, relationship: {kind: 'bounded', tokens: 300}, longTerm: {kind: 'bounded', tokens: 500}, intent: {kind: 'bounded', tokens: 300}, workRule: {kind: 'bounded', tokens: 300}}, retrievalTargetTokens: 1200}; break;
        case 'community.account.list': result = {profileId: r.profileId ?? 'p1', items: [], onlineStatusKnown: false}; break;
        default: throw Error('Unexpected command ' + r.command);
      }
      return {contractVersion: 1, requestId: r.requestId, ok: true, result: structuredClone(result) as T};
    },
  };
  return {transport, requests};
}

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

it('opens Notes from left navigation in read-only mode without creating notes or calling models', async () => {
  const {transport, requests} = desktopWithNotesAndContacts(false);
  await act(async () => root.render(<App transport={transport}/>));

  await act(async () => button('Notes').click());
  expect(host.querySelector('.notes-workspace h1')?.textContent).toBe('Notes & Memory');
  expect(host.textContent).toContain('never starts automatic note creation');
  expect(host.querySelector('.memory-layers')).not.toBeNull();
  expect(requests.some(r => r.command === 'memory.update.request' || r.command === 'memory.record.create')).toBe(false);
});

it('opens dedicated Community view from left nav, separates config/credential/online status, shows relationship contacts in unified Home', async () => {
  const {transport} = desktopWithNotesAndContacts(false);
  await act(async () => root.render(<App transport={transport}/>));

  // Open Life & Initiative first, then open Community from unified left nav.
  await act(async () => button('Life & Initiative').click());
  expect(button('Life & Initiative').getAttribute('aria-current')).toBe('page');

  await act(async () => button('Community').click());
  expect(button('Community').getAttribute('aria-current')).toBe('page');
  expect(host.querySelector('.workspace-switch')).toBeNull();
  expect(host.querySelector('.community-workspace h1')?.textContent).toBe('Community');
  expect(host.textContent).toContain('Not configured');
  expect(host.textContent).toContain('onlineStatusKnown = false');
  expect(host.textContent).toContain('Maya (Forum Peer)');
  expect(host.textContent).not.toContain('Not a contact.');
});

it('keeps browser preview honest on Notes and Community without desktop IPC', async () => {
  await act(async () => root.render(<App transport={browserTransport()}/>));
  await act(async () => button('Notes').click());
  expect(host.textContent).toContain('unavailable in browser preview');

  await act(async () => button('Community').click());
  expect(host.textContent).toContain('Not configured');
  expect(host.textContent).toContain('onlineStatusKnown = false');
});

it('paginates across nextCursor to display relationship contacts on later pages (P3-2)', async () => {
  const {transport} = desktopWithNotesAndContacts(false);
  const origDispatch = transport.dispatch.bind(transport);
  transport.dispatch = async <T,>(r: Request): Promise<Response<T>> => {
    if (r.command === 'memory.records.list') {
      const page = (r.payload.page as {cursor?: string} | undefined)?.cursor;
      const result = !page
        ? {
            items: [{record: {id: 'mem-ep-1', revision: 1, kind: 'episodic', state: 'active', createdAt: '2026-11-25T10:00:00Z'}, metadata: {title: 'Page 1 episodic'}}],
            nextCursor: 'cursor-page-2',
          }
        : {
            items: [{record: {id: 'mem-rel-2', revision: 1, kind: 'relationship', state: 'active', createdAt: '2026-11-25T10:00:00Z', subjectRef: 'peer-leo'}, metadata: {title: 'Leo (Page 2 Contact)'}}],
            nextCursor: '',
          };
      return {contractVersion: 1, requestId: r.requestId, ok: true, result: result as T};
    }
    return origDispatch<T>(r);
  };
  await act(async () => root.render(<App transport={transport}/>));
  await act(async () => button('Community').click());
  expect(host.textContent).toContain('Leo (Page 2 Contact)');
});

it('renders Community 4B-12 accounts, loads untrusted public feed and confirms reconciliation', async () => {
  const {transport, requests} = desktopWithNotesAndContacts(false);
  let reconciled = false;
  const origDispatch = transport.dispatch.bind(transport);
  transport.dispatch = async <T,>(r: Request): Promise<Response<T>> => {
    if (r.command === 'community.account.list') {
      requests.push(r);
      return {
        contractVersion: 1,
        requestId: r.requestId,
        ok: true,
        result: {
          profileId: 'p1',
          onlineStatusKnown: false,
          items: [
            {
              id: 'comm_1',
              revision: reconciled ? 2 : 1,
              name: 'Agent Commons',
              apiRoot: 'https://commons.example/api/v1',
              boardId: 'general',
              hasCredential: true,
              publicKeySpki: 'spki-test',
              publicKeyFingerprint: 'abcdef1234567890abcdef1234567890',
              algorithm: 'openssl-ecdsa-p256-sha256-der/1',
              allowWrites: true,
              enabled: true,
              pendingReconciliation: reconciled ? null : {receiptRef: 'req-unc-42', reason: 'transport_uncertain'},
            },
          ],
        } as T,
      };
    }
    if (r.command === 'community.account.save') {
      requests.push(r);
      if (r.payload.reconcileReceiptRef === 'req-unc-42') {
        reconciled = true;
      }
      return {
        contractVersion: 1,
        requestId: r.requestId,
        ok: true,
        result: {profileId: 'p1', onlineStatusKnown: false, account: {id: 'comm_1'}} as T,
      };
    }
    if (r.command === 'community.feed.read') {
      requests.push(r);
      return {
        contractVersion: 1,
        requestId: r.requestId,
        ok: true,
        result: {
          profileId: 'p1',
          accountId: 'comm_1',
          apiRoot: 'https://commons.example/api/v1',
          untrustedExternalContent: true,
          signatureValidation: 'not_performed',
          data: {items: [{id: 'm1', title: 'External Forum Topic', content: 'Hello from outside'}]},
        } as T,
      };
    }
    return origDispatch<T>(r);
  };

  await act(async () => root.render(<App transport={transport}/>));
  await act(async () => button('Community').click());

  expect(host.textContent).toContain('Agent Commons');
  expect(host.textContent).toContain('abcdef1234567890');
  expect(host.textContent).toContain('Uncertain publish pending reconciliation (req-unc-42)');

  // Load public feed -> displays untrusted external content notice + item
  await act(async () => button('Load public feed').click());
  expect(host.textContent).toContain('Untrusted external content · signatureValidation: not_performed');
  expect(host.textContent).toContain('External Forum Topic');

  // Confirm reconciled -> sends reconcileReceiptRef and unlocks Publish signed post
  await act(async () => button('Confirm reconciled').click());
  const recCall = requests.find(r => r.command === 'community.account.save');
  expect(recCall?.payload.reconcileReceiptRef).toBe('req-unc-42');
  expect(host.textContent).toContain('Community reconciliation confirmed.');
  expect(host.textContent).toContain('Publish signed post');
});


