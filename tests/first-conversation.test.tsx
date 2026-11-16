import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import type {Request, Response, Transport} from '../src/domain/types';
import {App} from '../src/ui/App';
import {setupStorageKey} from '../src/ui/firstConversation';

function desktop({existing = false, legacyDraft = false, starter = true, loseProfileReceipt = false, loseConversationReceipt = false}: {
  existing?: boolean; legacyDraft?: boolean; starter?: boolean; loseProfileReceipt?: boolean; loseConversationReceipt?: boolean;
} = {}) {
  const requests: Request[] = [];
  const profile = {id: 'p', displayName: existing || legacyDraft ? 'Existing agent' : 'Ada', revision: 1,
    status: existing ? 'ready' : 'draft', mode: 'standard', primaryRouteId: existing ? 'r' : undefined as string|undefined};
  const connection = {id: 'c', revision: 1, status: 'unverified', providerKind: 'openrouter', label: 'OpenRouter', normalizedEndpoint: 'https://openrouter.ai/api/v1'};
  const route = {id: 'r', role: 'primary', connectionId: 'c', modelId: 'openai/gpt-4o-mini', revision: 1};
  const conversation = {id: 't', profileId: 'p', revision: 1, language: 'en', title: 'First conversation',
    draftText: '', status: 'open'};
  let hasProfile = existing || legacyDraft;
  let hasConnection = existing;
  let hasRoute = existing;
  let hasConversation = existing;
  let createdProfileRequest: string | undefined;
  let createdConversationRequest: string | undefined;
  let contextTokens = existing ? 16384 : 0;
  let memoryContext = 0;
  let mode = 'automatic';
  let validKey = true;
  let reply = false;
  const working = () => ({record: {id:'w', revision: 1, state:'empty'}, primary: {contextTokens: memoryContext, verified:true}, attempts: [],
    proposals: [], decisions: [], profileRevision: profile.revision, capacityPolicy: {kind:'bounded',tokens:300},
    scopeId: 'a'.repeat(64), scribeMode: mode, formatRepairCount: 0});
  const routeSettings = () => ({routeId: route.id, routeRevision: route.revision, contextTokens,
    modelId: route.modelId, outputTokens: 1024, inputReserveTokens: 512});
  const transport: Transport = {
    mode: 'desktop',
    subscribe: async () => () => {},
    async dispatch<T>(r: Request): Promise<Response<T>> {
      requests.push(r);
      let result: unknown;
      switch (r.command) {
        case 'kernel.status.get': result = {lastEventSequence: 0}; break;
        case 'app.preferences.get': result = {revision: 1, locale: 'en', themeId: 'orbital', themeVariant: 'light'}; break;
        case 'profile.list': result = {items: hasProfile ? [profile] : [], nextCursor:''}; break;
        case 'profile.archived.list':
        case 'backup.list': result = {items: [], nextCursor:''}; break;
        case 'backup.restore.status': result = {pending:null}; break;
        case 'profile.create':
          if (hasProfile) expect(r.requestId).toBe(createdProfileRequest);
          hasProfile = true;
          createdProfileRequest = r.requestId;
          if (loseProfileReceipt) { loseProfileReceipt = false; throw Error('response lost'); }
          result = profile; break;
        case 'profile.snapshot':
          result = {profile, connections: hasConnection ? [connection] : [], routes: hasRoute ? [route] : []}; break;
        case 'connection.create':
          expect(hasConnection).toBe(false);
          hasConnection = true;
          const newConnection = r.payload.connection as {providerKind:string; label:string; endpoint:string};
          connection.providerKind = newConnection.providerKind;
          connection.label = newConnection.label;
          if (newConnection.endpoint) connection.normalizedEndpoint = newConnection.endpoint;
          validKey = (r.payload.credential as {secret:string}).secret === 'test-key';
          result = connection; break;
        case 'connection.update':
          expect(r.expectedRevision).toBe(connection.revision);
          expect(r.payload.connectionId).toBe(connection.id);
          connection.normalizedEndpoint = (r.payload.connection as {endpoint:string}).endpoint;
          validKey = false; // The previous credential belongs to the old address.
          connection.status = 'unverified';
          connection.revision++;
          result = connection; break;
        case 'connection.credential.set':
          expect(r.payload.connectionId).toBe(connection.id);
          expect(r.expectedRevision).toBe(connection.revision);
          validKey = (r.payload.credential as {secret:string}).secret === 'test-key';
          connection.revision++;
          result = connection; break;
        case 'connection.test.latest': result = {found:false}; break;
        case 'connection.test':
          expect(r.expectedRevision).toBe(connection.revision);
          result = {operation:{id:'test',status:'queued'}}; break;
        case 'operation.get':
          if (r.payload.operationId === 'chat') {
            reply = true;
            result = {id:'chat',status:'succeeded',revision:2,physicalCallCount:1};
          } else {
            const goodEndpoint = !connection.normalizedEndpoint.includes(':1234');
            if (validKey && goodEndpoint) connection.status = 'verified';
            result = {id:'test',status:validKey && goodEndpoint ? 'succeeded' : 'failed',
              error:!goodEndpoint ? {messageKey:'error.provider_error'} :
                validKey ? undefined : {messageKey:'error.provider_authentication_failed'}};
          }
          break;
        case 'connection.models.list':
          expect(connection.status).toBe('verified');
          result = {items: starter ? [{id:route.modelId,label:'Starter'}] : [{id:'other',label:'Another model'}]}; break;
        case 'route.save':
          expect(hasRoute).toBe(false);
          expect(r.expectedRevision).toBe(profile.revision);
          hasRoute = true;
          route.modelId = (r.payload.route as {modelId:string}).modelId;
          profile.revision++;
          result = route; break;
        case 'route.assign':
          expect(r.expectedRevision).toBe(profile.revision);
          profile.primaryRouteId = route.id;
          profile.revision++;
          result = profile; break;
        case 'chat.settings.get':
          expect(profile.primaryRouteId).toBe(route.id);
          result = routeSettings(); break;
        case 'chat.settings.set':
          expect(r.expectedRevision).toBe(route.revision);
          expect(r.payload.routeId).toBe(route.id);
          contextTokens = Number(r.payload.contextTokens);
          route.revision++;
          result = routeSettings(); break;
        case 'memory.working.get': result = working(); break;
        case 'memory.settings.set':
          expect(contextTokens).toBeGreaterThan(0);
          expect(r.expectedRevision).toBe(profile.revision);
          expect(r.payload.scribeMode).toBe('disabled');
          memoryContext = Number(r.payload.primaryContextTokens);
          mode = 'disabled';
          profile.revision++;
          profile.status = 'ready';
          result = working(); break;
        case 'conversation.create':
          expect(profile.status).toBe('ready');
          expect(contextTokens).toBeGreaterThan(0);
          if (hasConversation) expect(r.requestId).toBe(createdConversationRequest);
          hasConversation = true;
          createdConversationRequest = r.requestId;
          if (loseConversationReceipt) { loseConversationReceipt = false; throw Error('response lost'); }
          result = conversation; break;
        case 'conversation.list': result = {items:hasConversation ? [conversation] : [], nextCursor:''}; break;
        case 'conversation.get': result = conversation; break;
        case 'conversation.chat.latest': result = {found:false}; break;
        case 'conversation.messages.list':
          result = {items:reply ? [{id:'msg',role:'assistant',content:'Hello from the provider'}] : [], nextCursor:''}; break;
        case 'chat.vision.get': result = {support:'unknown'}; break;
        case 'chat.send':
          expect(r.profileId).toBe(profile.id);
          expect(r.payload.conversationId).toBe(conversation.id);
          expect(contextTokens).toBeGreaterThan(0);
          result = {operation:{id:'chat',revision:1,status:'queued',physicalCallCount:0}}; break;
        case 'material.list':
        case 'conversation.materials.list': result = {items:[], nextCursor:''}; break;
        default: throw Error('Unexpected command ' + r.command);
      }
      return {contractVersion:1,requestId:r.requestId,ok:true,result:structuredClone(result) as T};
    },
  };
  return {transport, requests};
}

let host: HTMLDivElement;
let root: Root;
beforeEach(() => {
  localStorage.clear();
  vi.useFakeTimers();
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  vi.stubGlobal('matchMedia', () => ({matches:false,addEventListener:vi.fn(),removeEventListener:vi.fn()}));
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
async function mount(transport: Transport) { await act(async () => root.render(<App transport={transport}/>)); }
async function restart(transport: Transport) {
  await act(async () => root.unmount());
  root = createRoot(host);
  await mount(transport);
}
async function fill(selector: string, value: string) {
  const el = host.querySelector<HTMLInputElement|HTMLTextAreaElement>(selector)!;
  const prototype = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  await act(async () => {
    Object.getOwnPropertyDescriptor(prototype,'value')!.set!.call(el,value);
    el.dispatchEvent(new Event('input',{bubbles:true}));
  });
}
function button(name: string) {
  const target = [...host.querySelectorAll('button')].find(btn => btn.textContent===name);
  if (!target) throw Error('Missing button: ' + name);
  return target;
}
async function click(name: string) {await act(async () => button(name).click());}
async function finishPoll() {await act(async () => {await vi.advanceTimersByTimeAsync(600);});}

it('opens a send-ready first conversation from a clean install and keeps advanced panels in Settings', async () => {
  const {transport, requests}=desktop();
  await mount(transport);
  expect(host.querySelector('.first-conversation')).not.toBeNull();
  expect(host.querySelector('.connection-setup')).toBeNull();
  expect(requests.map(r=>r.command)).not.toContain('connection.test');
  await fill('.first-conversation input:not([type=password])','Ada');
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  expect(localStorage.getItem(setupStorageKey)).not.toContain('test-key');
  await finishPoll();
  expect(host.querySelector('.first-conversation')).toBeNull();
  expect(localStorage.getItem(setupStorageKey)).toBeNull();
  expect(requests.map(r=>r.command)).toContain('chat.settings.set');
  expect(requests.filter(r=>r.command==='route.assign').map(r=>r.payload.role)).toEqual(['primary']);
  await fill('.conversations textarea','Hello');
  expect(button('Send').disabled).toBe(false);
  await click('Send');
  await finishPoll();
  expect(host.textContent).toContain('Hello from the provider');
  await click('Settings');
  expect(host.querySelector('.connection-setup')).not.toBeNull();
  expect(host.querySelector('.memory-settings')).not.toBeNull();
  expect(host.querySelector('.center[hidden] .chat-panel')).not.toBeNull();
});

it('reuses the profile and connection after a bad key and a restart, then asks for a model', async () => {
  const {transport, requests}=desktop({starter:false});
  await mount(transport);
  await fill('.first-conversation input:not([type=password])','Ada');
  await fill('input[type=password]','bad-key');
  await click('Connect and start');
  await finishPoll();
  expect(host.querySelector('[role=alert]')?.textContent).toContain('Checking the provider connection');
  expect(localStorage.getItem(setupStorageKey)).not.toContain('bad-key');
  await restart(transport);
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  await finishPoll();
  expect(host.textContent).toContain('Choose a model');
  expect(requests.some(r=>r.command==='route.save')).toBe(false);
  await act(async()=>{
    const model=[...host.querySelectorAll<HTMLSelectElement>('form select')].at(-1)!;
    model.value='other';
    model.dispatchEvent(new Event('change',{bubbles:true}));
  });
  await click('Connect and start');
  for (const command of ['profile.create','connection.create','connection.credential.set','route.save','conversation.create']) {
    expect(requests.filter(r=>r.command===command)).toHaveLength(1);
  }
  expect(requests.find(r=>r.command==='route.save')?.payload.route).toMatchObject({modelId:'other'});
  expect(host.querySelector('.conversations textarea')).not.toBeNull();
});

it('replays a lost profile or conversation response with the original request identity after reopening', async () => {
  const {transport, requests}=desktop({loseProfileReceipt:true,loseConversationReceipt:true});
  await mount(transport);
  await fill('.first-conversation input:not([type=password])','Ada');
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  await restart(transport);
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  await finishPoll();
  await restart(transport);
  await click('Connect and start');
  for (const command of ['profile.create','conversation.create']) {
    const attempts=requests.filter(r=>r.command===command);
    expect(attempts).toHaveLength(2);
    expect(attempts[0].requestId).toBe(attempts[1].requestId);
  }
  expect(requests.filter(r=>r.command==='connection.create')).toHaveLength(1);
  expect(requests.filter(r=>r.command==='memory.settings.set')).toHaveLength(1);
  expect(host.querySelector('.conversations textarea')).not.toBeNull();
});

it('opens an existing ready Standard workspace without a wizard or configuration writes', async () => {
  const {transport, requests}=desktop({existing:true});
  await mount(transport);
  expect(host.querySelector('.first-conversation')).toBeNull();
  expect(host.querySelector('.conversations textarea')).not.toBeNull();
  expect(host.textContent).toContain('Existing agent');
  expect(requests.map(r=>r.command).filter(c=>/\.(create|save|assign|set|test)$/.test(c))).toEqual([]);
});

async function choose(selector: string, value: string) {
  await act(async () => {
    const el = host.querySelector<HTMLSelectElement>(selector)!;
    el.value = value;
    el.dispatchEvent(new Event('change',{bubbles:true}));
  });
}

it('corrects a failed custom address after restart and retests the same connection with a rebound key', async () => {
  const {transport, requests} = desktop({starter:false});
  await mount(transport);
  await fill('form input:not([type=password])','Ada');
  await choose('form select','custom_openai');
  await fill('input[type=url]','http://localhost:1234/v1');
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  await finishPoll();
  expect(host.querySelector('[role=alert]')?.textContent).toContain('Checking the provider connection');
  await restart(transport);
  const endpoint = host.querySelector<HTMLInputElement>('input[type=url]')!;
  expect(endpoint.readOnly).toBe(false);
  await fill('input[type=url]','http://localhost:4321/v1/');
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  await finishPoll();
  await choose('form label:last-of-type select','other');
  await click('Connect and start');
  expect(requests.find(r=>r.command==='connection.update')).toMatchObject({profileId:'p',expectedRevision:1,
    payload:{connectionId:'c',connection:{endpoint:'http://localhost:4321/v1',providerKind:'custom_openai'}}});
  expect(requests.find(r=>r.command==='connection.credential.set')?.expectedRevision).toBe(2);
  expect(requests.filter(r=>r.command==='connection.test').map(r=>r.expectedRevision)).toEqual([1,3]);
  for (const command of ['profile.create','connection.create','conversation.create']) {
    expect(requests.filter(r=>r.command===command)).toHaveLength(1);
  }
  expect(host.querySelector('.conversations textarea')).not.toBeNull();
});

it('offers explicit Finish setup for a legacy draft and completes it without creating a replacement profile', async () => {
  const {transport,requests}=desktop({legacyDraft:true});
  await mount(transport);
  expect(host.querySelector('.first-conversation')).toBeNull();
  expect(requests.some(r=>r.command==='connection.create')).toBe(false);
  await click('Finish setup');
  const name=host.querySelector<HTMLInputElement>('.first-conversation input:not([type=password])')!;
  expect(name.value).toBe('Existing agent');
  expect(name.readOnly).toBe(true);
  await fill('input[type=password]','test-key');
  await click('Connect and start');
  await finishPoll();
  expect(requests.some(r=>r.command==='profile.create')).toBe(false);
  expect(requests.find(r=>r.command==='connection.create')?.profileId).toBe('p');
  expect(requests.filter(r=>r.command==='conversation.create')).toHaveLength(1);
  expect(host.querySelector('.conversations textarea')).not.toBeNull();
});
