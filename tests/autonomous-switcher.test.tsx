import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import type {Request, Response, Transport} from '../src/domain/types';
import {browserTransport} from '../src/ipc/browser';
import {App} from '../src/ui/App';

function readyStandardDesktop(locale: 'en' | 'pl' = 'en') {
  const requests: Request[] = [];
  const profile = {id: 'p1', displayName: 'Ada', revision: 1, status: 'ready', mode: 'standard', primaryRouteId: 'r1'};
  const conversation = {id: 'c1', profileId: 'p1', revision: 1, language: locale, title: 'Main chat', draftText: '', status: 'open'};
  const autonomousInstances: Array<{profileId: string; instanceId: string; displayName: string; status: string; activeWakePlan: Record<string, unknown>; proposalCount: number; activeRefusalCount: number}> = [];
  const proposals: Array<{id: string; version: number; title: string; terms: string; status: string; scopeKey: string; rounds: Array<Record<string, unknown>>}> = [];
  const refusals: Array<{id: string; scopeFingerprint: string; status: string; reconsiderationConditions: string[]; createdAt: string}> = [];

  const transport: Transport = {
    mode: 'desktop',
    subscribe: async () => () => {},
    async dispatch<T>(r: Request): Promise<Response<T>> {
      requests.push(r);
      let result: unknown;
      switch (r.command) {
        case 'kernel.status.get': result = {lastEventSequence: 0}; break;
        case 'app.preferences.get': result = {revision: 1, locale, themeId: 'orbital', themeVariant: 'light'}; break;
        case 'profile.list': result = {items: [profile], nextCursor: ''}; break;
        case 'profile.archived.list':
        case 'material.list':
        case 'conversation.materials.list':
        case 'conversation.messages.list':
        case 'activity.list': result = {items: [], nextCursor: ''}; break;
        case 'conversation.list': result = {items: [conversation], nextCursor: ''}; break;
        case 'conversation.get': result = conversation; break;
        case 'conversation.chat.latest': result = {found: false}; break;
        case 'chat.settings.get': result = {routeId: 'r1', routeRevision: 1, contextTokens: 8192, modelId: 'm', outputTokens: 1024, inputReserveTokens: 512}; break;
        case 'chat.vision.get': result = {support: 'unknown'}; break;
        case 'memory.working.get': result = {record: {id: 'w1', revision: 1, state: 'empty'}, primary: {contextTokens: 8192, verified: true}, attempts: [], proposals: [], decisions: [], profileRevision: 1, capacityPolicy: {kind: 'bounded', tokens: 300}, scopeId: 'a'.repeat(64), scribeMode: 'disabled', formatRepairCount: 0}; break;
        case 'autonomous.instance.list':
          result = {items: autonomousInstances};
          break;
        case 'autonomous.instance.create': {
          const created = {
            profileId: 'auto-p1',
            instanceId: 'auto-i1',
            displayName: String(r.payload.displayName),
            status: 'ready',
            activeWakePlan: {mode: 'interval', origin: 'initial_configuration', requestedDelayMs: 1},
            proposalCount: 0,
            activeRefusalCount: 0,
          };
          autonomousInstances.push(created);
          result = {profileId: created.profileId, instanceId: created.instanceId, displayName: created.displayName, status: created.status, activeWakePlan: created.activeWakePlan};
          break;
        }
        case 'autonomous.instance.status': {
          const inst = autonomousInstances.find(i => i.profileId === r.profileId) ?? autonomousInstances[0];
          result = {
            profileId: inst.profileId,
            displayName: inst.displayName,
            instance: {id: inst.instanceId, status: inst.status},
            activeWakePlan: inst.activeWakePlan,
            activeRefusals: refusals,
            proposals,
            recentDecisions: [],
            unresolvedActions: [],
            forumConnected: false,
          };
          break;
        }
        case 'autonomous.proposal.submit': {
          const pid = r.payload.proposalId ? String(r.payload.proposalId) : 'prop-1';
          const existing = proposals.find(p => p.id === pid);
          if (existing) {
            existing.version += 1;
            existing.title = String(r.payload.title);
            existing.terms = String(r.payload.terms);
            existing.status = 'pending';
            existing.rounds.push({id: 'r-' + (existing.rounds.length + 1), version: existing.version, roundNumber: existing.rounds.length + 1, actor: 'human', kind: 'revision', note: existing.terms, createdAt: '2026-11-25T12:00:00Z'});
            result = {proposal: existing, version: existing.version, created: false};
          } else {
            const prop = {
              id: pid,
              version: 1,
              title: String(r.payload.title),
              terms: String(r.payload.terms),
              status: 'pending',
              scopeKey: `proposal:${pid}:v1`,
              rounds: [{id: 'r-1', version: 1, roundNumber: 1, actor: 'human', kind: 'proposal', note: String(r.payload.terms), createdAt: '2026-11-25T12:00:00Z'}],
            };
            proposals.push(prop);
            result = {proposal: prop, version: 1, created: true};
          }
          break;
        }
        case 'autonomous.proposal.consider': {
          const prop = proposals.find(p => p.id === r.payload.proposalId)!;
          if (prop.status === 'refused') {
            result = {status: 'refusal_active', decisionSource: 'policy_guard', proposal: prop};
          } else if (r.payload.simulationOutcome === 'refuse') {
            prop.status = 'refused';
            refusals.push({id: 'ref-1', scopeFingerprint: 'f'.repeat(64), status: 'active', reconsiderationConditions: ['revised_terms'], createdAt: '2026-11-25T12:01:00Z'});
            result = {status: 'decided', decisionSource: 'pilot_simulation', decisionOutcome: 'refusal', proposal: prop};
          } else {
            prop.status = 'accepted';
            result = {status: 'decided', decisionSource: 'pilot_simulation', decisionOutcome: 'action', proposal: prop};
          }
          break;
        }
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

async function fill(el: HTMLInputElement | HTMLTextAreaElement, value: string) {
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  await act(async () => {
    Object.getOwnPropertyDescriptor(proto, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', {bubbles: true}));
  });
}

it('switches between Standard and Autonomous without Standard writes, preserves draft, and syncs workspace on Activity (P3-3)', async () => {
  const {transport, requests} = readyStandardDesktop('en');
  await act(async () => root.render(<App transport={transport}/>));

  const textarea = host.querySelector<HTMLTextAreaElement>('.conversations textarea')!;
  await fill(textarea, 'Unsaved note');

  await act(async () => button('Autonomous').click());
  expect(host.querySelector('.autonomous-workspace')).not.toBeNull();
  expect(host.querySelector('.autonomous-workspace')?.textContent).toContain('Local pilot');
  expect(button('Autonomous').getAttribute('aria-pressed')).toBe('true');
  expect(button('Standard').getAttribute('aria-pressed')).toBe('false');
  expect(host.querySelector('.center[hidden] .conversations')).not.toBeNull();
  expect(requests.filter(r => r.command.startsWith('autonomous.')).map(r => r.command)).toEqual(['autonomous.instance.list']);

  // Clicking Standard Activity from Autonomous switches top switcher back to Standard (P3-3).
  await act(async () => button('Activity').click());
  expect(button('Standard').getAttribute('aria-pressed')).toBe('true');
  expect(button('Autonomous').getAttribute('aria-pressed')).toBe('false');

  await act(async () => button('Home').click());
  expect(host.querySelector<HTMLTextAreaElement>('.conversations textarea')?.value).toBe('Unsaved note');
});

it('runs local Autonomous pilot flow: create instance, submit v1, refuse, preserve refusal, revise to v2 and accept', async () => {
  const {transport} = readyStandardDesktop('en');
  await act(async () => root.render(<App transport={transport}/>));
  await act(async () => button('Autonomous').click());

  const nameInput = host.querySelector<HTMLInputElement>('.autonomous-inline-form input')!;
  await fill(nameInput, 'Pilot One');
  await act(async () => button('Create instance').click());
  expect(host.textContent).toContain('Pilot One');

  const propInputs = host.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('.autonomous-section .fields input, .autonomous-section .fields textarea');
  await fill(propInputs[0], 'Local task');
  await fill(propInputs[1], 'Initial terms');
  await act(async () => button('Submit proposal').click());
  expect(host.textContent).toContain('Local task v1');

  // Default simulation choice is refuse.
  await act(async () => button('Consider').click());
  expect(host.textContent).toContain('refused');

  // Re-considering same version preserves refusal.
  await act(async () => button('Consider').click());
  expect(host.textContent).toContain('Active refusal preserved');

  // Revise terms to v2 and accept.
  await act(async () => button('Revise terms (new version)').click());
  const revTerms = host.querySelector<HTMLTextAreaElement>('.autonomous-revise-form textarea')!;
  await fill(revTerms, 'Narrower terms');
  await act(async () => button('Submit new version').click());
  expect(host.textContent).toContain('Local task v2');

  const simSelect = host.querySelector<HTMLSelectElement>('.autonomous-sim-select select')!;
  await act(async () => {
    simSelect.value = 'accept';
    simSelect.dispatchEvent(new Event('change', {bubbles: true}));
  });
  await act(async () => button('Consider').click());
  expect(host.textContent).toContain('accepted');
});

it('displays Polish Autonomiczny labels and honest browser-preview unavailability without autonomous IPC', async () => {
  const {transport} = readyStandardDesktop('pl');
  await act(async () => root.render(<App transport={transport}/>));
  await act(async () => button('Autonomiczny').click());
  expect(host.querySelector('.autonomous-workspace h1')?.textContent).toBe('Przestrzeń Autonomiczna');
  expect(host.textContent).toContain('Lokalny pilotaż');

  await act(async () => root.unmount());
  root = createRoot(host);
  await act(async () => root.render(<App transport={browserTransport()}/>));
  await act(async () => button('Autonomous').click());
  expect(host.querySelector('.autonomous-workspace')?.textContent).toContain('browser preview');
});
