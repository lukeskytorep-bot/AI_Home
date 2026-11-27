import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {act} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import {InnateSensesBar, readInnateSenses} from '../src/ui/InnateSensesBar';
import {App} from '../src/ui/App';
import {browserTransport} from '../src/ipc/browser';

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

it('computes local time and day part accurately in readInnateSenses', () => {
  expect(readInnateSenses(new Date(2026, 10, 27, 8, 5)).dayPart).toBe('morning');
  expect(readInnateSenses(new Date(2026, 10, 27, 14, 30)).dayPart).toBe('day');
  expect(readInnateSenses(new Date(2026, 10, 27, 20, 0)).dayPart).toBe('evening');
  expect(readInnateSenses(new Date(2026, 10, 27, 2, 15)).dayPart).toBe('night');
});

it('renders InnateSensesBar in Polish and English and exposes tabbed Settings without Standard/Autonomous switcher', async () => {
  await act(async () => root.render(<InnateSensesBar locale="pl"/>));
  expect(host.querySelector('.senses-bar')).not.toBeNull();
  expect(host.textContent).toContain('Online');

  await act(async () => root.render(<App transport={browserTransport()}/>));
  expect(host.querySelector('.workspace-switch')).toBeNull();
  expect(host.querySelector('header .senses-bar')).not.toBeNull();

  const settingsBtn = host.querySelector<HTMLButtonElement>('header button[data-nav="settings"]')!;
  await act(async () => settingsBtn.click());
  const tabs = [...host.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
  expect(tabs.map(t => t.textContent)).toEqual([
    'Appearance & language',
    'Residents & models',
    'Senses & MCP',
    'Skills',
    'Backup & diagnostics',
  ]);

  await act(async () => tabs[2].click());
  expect(host.textContent).toContain('Innate senses');
});
