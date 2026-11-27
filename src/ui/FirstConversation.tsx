import {useEffect, useRef, useState, type FormEvent} from 'react';
import {t} from '../domain/packs';
import type {Client} from '../ipc/client';
import {firstConversation, readSetupDraft, providers, setupStorageKey,
  type Model, type SetupDraft, type SetupStep, type WorkspaceSelection} from './firstConversation';

export function FirstConversation({client, locale, initial, onOpen, onCancel, onResetDraft}: {
  client: Client;
  locale: string;
  initial?: SetupDraft;
  onOpen: (selection: WorkspaceSelection) => void;
  onCancel?: () => void;
  onResetDraft?: () => void;
}) {
  const text = (key: string) => t(locale, key);
  const saved = useRef(initial);
  const task = useRef<AbortController | undefined>(undefined);
  const [name, setName] = useState(initial?.name ?? '');
  const [provider, setProvider] = useState(initial?.provider ?? 'openrouter');
  const [endpoint, setEndpoint] = useState(initial?.endpoint ?? '');
  const [models, setModels] = useState<Model[]>();
  const [modelFilter, setModelFilter] = useState('');
  const [model, setModel] = useState('');
  const [step, setStep] = useState<SetupStep>();
  const [error, setError] = useState('');
  const [failedStep, setFailedStep] = useState<SetupStep>();
  const [started, setStarted] = useState(() => Boolean(initial && readSetupDraft()?.id === initial.id));
  const [checkedEndpoint, setCheckedEndpoint] = useState(initial?.endpoint ?? '');
  const password = useRef<HTMLInputElement>(null);

  useEffect(() => () => {
    task.current?.abort();
    task.current = undefined;
    if (password.current) password.current.value = '';
  }, [client]);

  const filteredModels = models
    ? models.filter(m => {
        const q = modelFilter.trim().toLowerCase();
        return !q || m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q);
      })
    : undefined;

  async function discardDraft() {
    if (task.current) return;
    setError('');
    const targetId = saved.current?.profileId ?? initial?.profileId;
    if (targetId) {
      try {
        const snap = await client.call<{profile: {revision: number; status: string}}>('profile.snapshot', {}, undefined, targetId);
        if (snap.ok && snap.result.profile.status === 'draft') {
          await client.call('profile.archive', {}, snap.result.profile.revision, targetId);
        }
      } catch {
        // If archiving fails, still clear local UI draft so the user is never trapped.
      }
    }
    localStorage.removeItem(setupStorageKey);
    if (password.current) password.current.value = '';
    saved.current = undefined;
    setStarted(false);
    setName('');
    setProvider('openrouter');
    setEndpoint('');
    setModels(undefined);
    setModel('');
    setModelFilter('');
    setFailedStep(undefined);
    onResetDraft?.();
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (task.current || !name.trim() || (models && !model)) return;
    setError('');
    setFailedStep(undefined);
    if (new TextEncoder().encode(name.trim()).length > 256) {
      setError('onboarding.nameLength');
      return;
    }
    let currentStep: SetupStep | undefined;
    let url = '';
    if (provider === 'custom_openai') {
      try {
        if (/[\u0000-\u001f\u007f]/.test(endpoint)) throw Error();
        const raw = endpoint.trim();
        const parsed = new URL(raw);
        const localHttp = parsed.protocol === 'http:' &&
          ['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname);
        if ((parsed.protocol !== 'https:' && !localHttp) || parsed.username ||
            parsed.password || parsed.search || parsed.hash ||
            parsed.href.includes('?') || parsed.href.includes('#') || parsed.port === '0') throw Error();
        url = parsed.href.replace(/\/+$/, '');
      } catch {
        setError('onboarding.endpointError');
        return;
      }
    }
    const secret = password.current?.value ?? '';
    if ((!started && !initial?.existingConnectionId) && !secret) {
      setError('onboarding.keyRequired');
      return;
    }
    const controller = new AbortController();
    task.current = controller;
    const saveDraft = (draft: SetupDraft) => {
      localStorage.setItem(setupStorageKey, JSON.stringify(draft));
      saved.current = draft;
      setStarted(true);
    };
    try {
      const draft = saved.current ? {...saved.current, provider, endpoint: url} : {
        id: crypto.randomUUID(), name: name.trim(), provider, endpoint: url, language: locale === 'pl' ? 'pl' as const : 'en' as const,
      };
      saveDraft(draft);
      const result = await firstConversation(client, draft, secret, model, next => {
        currentStep = next;
        setStep(next);
      }, saveDraft, controller.signal);
      if (password.current) password.current.value = '';
      setCheckedEndpoint(url);
      if ('models' in result) {
        setModels(result.models);
        setModelFilter('');
        setModel('');
      } else {
        localStorage.removeItem(setupStorageKey);
        onOpen(result.selection);
      }
    } catch (e) {
      if (!controller.signal.aborted) {
        const message = e instanceof Error ? e.message : '';
        setFailedStep(currentStep);
        setError(/^(error|onboarding)\.[a-zA-Z_]+$/.test(message) ? message : 'error.transport');
      }
    } finally {
      if (task.current === controller) {
        task.current = undefined;
        if (!controller.signal.aborted) setStep(undefined);
      }
    }
  }

  const activeStep = models ? 2 : 1;

  return <section className="setup first-conversation" aria-labelledby="first-conversation-title">
    <ol className="wizard-steps" aria-label="Setup steps">
      <li className={activeStep === 1 ? 'active' : 'done'}>{text('onboarding.step1')}</li>
      <li className={activeStep === 2 ? 'active' : undefined}>{text('onboarding.step2')}</li>
      <li>{text('onboarding.step3')}</li>
    </ol>
    <h1 id="first-conversation-title">{text('onboarding.title')}</h1>
    <p className="muted">{text('onboarding.body')}</p>
    <form onSubmit={event => void submit(event)}>
      <fieldset disabled={Boolean(step)}>
        <label>{text('onboarding.name')}
          <input value={name} maxLength={256} required readOnly={started||Boolean(initial?.profileId)}
            onChange={e => setName(e.target.value)} />
        </label>
        <label>{text('config.provider')}
          <select value={provider} disabled={started||Boolean(initial?.existingConnectionId)} onChange={e => {
            setProvider(e.target.value);
            if (password.current) password.current.value = '';
          }}>
            {Object.entries(providers).map(([id, p]) => <option key={id} value={id}>{p.label}</option>)}
          </select>
        </label>
        {provider === 'custom_openai' && <label>{text('config.endpoint')}
          <input type="url" value={endpoint} maxLength={1024} required readOnly={Boolean(initial?.existingConnectionId)}
            placeholder="https://…/v1" onChange={e => {
              setEndpoint(e.target.value);
              setModels(undefined);
              setModel('');
            }} />
        </label>}
        {!models && <>
          <label>{text('config.key')}
            <input ref={password} type="password" autoComplete="off" spellCheck={false} maxLength={8192} required={(!started&&!initial?.existingConnectionId)||(Boolean(checkedEndpoint)&&endpoint.replace(/\/+$/,'')!==checkedEndpoint)} />
          </label>
          <p className="muted field-hint">{text('onboarding.keyHint')}</p>
        </>}
        {models && <>
          <p role="status" className="autonomous-status-banner">{text('onboarding.modelsLoaded')}</p>
          {models.length > 5 && <label>{text('onboarding.filterModels')}
            <input type="search" value={modelFilter} onChange={e => setModelFilter(e.target.value)} placeholder="gpt-4o, claude, gemini…" />
          </label>}
          <label>{text('onboarding.chooseModel')}
            <select value={model} onChange={e => setModel(e.target.value)} required>
              <option value="">{text('config.choose')}</option>
              {(filteredModels ?? models).map(m => <option key={m.id} value={m.id}>{m.label} ({m.id})</option>)}
            </select>
          </label>
        </>}
        <div className="first-conversation-actions">
          <button className="primary" disabled={!name.trim() || Boolean(models && !model)}>{text('onboarding.start')}</button>
          {(started || Boolean(initial?.profileId)) && <button type="button" onClick={() => void discardDraft()}>{text('onboarding.resetDraft')}</button>}
          {onCancel && <button type="button" onClick={() => { localStorage.removeItem(setupStorageKey); onCancel(); }}>{text('onboarding.cancel')}</button>}
        </div>
      </fieldset>
    </form>
    {step && !error && <p role="status">{text('onboarding.' + step)}</p>}
    {error && <p role="alert" className="error">{failedStep ? text('onboarding.' + failedStep) + ' · ' : ''}{text(error)}</p>}
  </section>;
}
