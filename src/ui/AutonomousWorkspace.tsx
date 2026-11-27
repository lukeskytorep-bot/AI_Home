import {useCallback, useEffect, useRef, useState, type FormEvent} from 'react';
import {t} from '../domain/packs';
import type {Client} from '../ipc/client';

interface WakePlan {
  id?: string;
  mode?: string;
  origin?: string;
  nextWakeAt?: string;
  requestedDelayMs?: number;
  status?: string;
}

interface ProviderConfigView {
  provider: string;
  normalizedEndpoint: string;
  modelId: string;
  hasCredential: boolean;
  dailyCallLimit: number;
  callsUsedToday: number;
  maxOutputTokens: number;
  autoWakeEnabled?: boolean;
}

interface InstanceSummary {
  profileId: string;
  instanceId: string;
  displayName: string;
  status: string;
  activeWakePlan?: WakePlan | null;
  proposalCount: number;
  activeRefusalCount: number;
}

interface ArchivedInstanceSummary {
  profileId: string;
  instanceId: string;
  displayName: string;
  profileRevision: number;
  status: string;
}

interface ProposalRound {
  id: string;
  version: number;
  roundNumber: number;
  actor: string;
  kind: string;
  decisionOutcome?: string;
  note: string;
  createdAt: string;
}

interface ProposalRecord {
  id: string;
  version: number;
  title: string;
  terms: string;
  status: string;
  scopeKey: string;
  executedVersions?: number[];
  rounds: ProposalRound[];
}

interface RefusalRecord {
  id: string;
  scopeFingerprint: string;
  status: string;
  reconsiderationConditions?: string[];
  createdAt: string;
}

interface UnresolvedAction {
  id: string;
  state: string;
  actionKind?: string;
  idempotencyKey?: string;
}

interface ContinuitySummary {
  activeEpisodeCount: number;
  hasResumePacket: boolean;
  privateReflectionCount: number;
  approvedSelfRecordCount: number;
  privateContentIncluded: false;
}

interface InstanceStatus {
  profileId: string;
  displayName: string;
  instance: {id: string; status: string; runtimeGeneration?: number};
  activeWakePlan?: WakePlan | null;
  activeRefusals: RefusalRecord[];
  proposals: ProposalRecord[];
  recentDecisions: Array<{id: string; outcome: string; createdAt: string}>;
  unresolvedActions: UnresolvedAction[];
  providerConfig?: ProviderConfigView | null;
  continuitySummary?: ContinuitySummary;
  forumConnected: false;
}

interface ConsiderResult {
  status: string;
  decisionSource: string;
  decisionOutcome?: string;
  proposal: ProposalRecord;
  activeWakePlan?: WakePlan | null;
}

interface WakeCheckResult {
  result: string;
  outcome?: string;
  activeWakePlan?: WakePlan | null;
}

const SIMULATION_OPTIONS = [
  {value: 'refuse', key: 'autonomous.pilot.simRefuse'},
  {value: 'accept', key: 'autonomous.pilot.simAccept'},
  {value: 'ask', key: 'autonomous.pilot.simAsk'},
  {value: 'defer', key: 'autonomous.pilot.simDefer'},
  {value: 'no_action', key: 'autonomous.pilot.simNoAction'},
  {value: 'sleep', key: 'autonomous.pilot.simSleep'},
  {value: 'execution_failed', key: 'autonomous.pilot.simExecFail'},
  {value: 'execution_uncertain', key: 'autonomous.pilot.simExecUncertain'},
  {value: 'provider', key: 'autonomous.pilot.providerMode'},
] as const;

const PROVIDER_OPTIONS = [
  {value: 'openrouter', label: 'OpenRouter'},
  {value: 'openai', label: 'OpenAI'},
  {value: 'anthropic', label: 'Anthropic'},
  {value: 'google', label: 'Google Gemini'},
  {value: 'custom_openai', label: 'Custom OpenAI-compatible'},
] as const;

export function AutonomousWorkspace({client, locale, desktop, onBackToStandard}: {
  client: Client;
  locale: string;
  desktop: boolean;
  onBackToStandard: () => void;
}) {
  const text = (key: string) => t(locale, key);
  const [instances, setInstances] = useState<InstanceSummary[]>([]);
  const [archivedInstances, setArchivedInstances] = useState<ArchivedInstanceSummary[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState('');
  const [status, setStatus] = useState<InstanceStatus>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');

  const [newInstanceName, setNewInstanceName] = useState('');
  const [proposalTitle, setProposalTitle] = useState('');
  const [proposalTerms, setProposalTerms] = useState('');
  const [simChoice, setSimChoice] = useState<string>('refuse');
  const [negotiationNotes, setNegotiationNotes] = useState<Record<string, string>>({});
  const [revisingId, setRevisingId] = useState<string>();
  const [reviseTitle, setReviseTitle] = useState('');
  const [reviseTerms, setReviseTerms] = useState('');

  // Provider & budget form state (secret stays in uncontrolled ref, cleared immediately on submit).
  const [providerKind, setProviderKind] = useState('openrouter');
  const [providerEndpoint, setProviderEndpoint] = useState('');
  const [providerModel, setProviderModel] = useState('');
  const [dailyLimit, setDailyLimit] = useState('20');
  const apiKeyRef = useRef<HTMLInputElement>(null);

  // Reconciliation evidence per unresolved action.
  const [reconcileRefs, setReconcileRefs] = useState<Record<string, string>>({});

  const syncProviderForm = useCallback((cfg?: ProviderConfigView | null) => {
    if (!cfg) return;
    setProviderKind(cfg.provider);
    setProviderEndpoint(cfg.provider === 'custom_openai' ? cfg.normalizedEndpoint : '');
    setProviderModel(cfg.modelId);
    setDailyLimit(String(cfg.dailyCallLimit));
  }, []);

  const loadInstances = useCallback(async (preferredProfileId?: string) => {
    if (!desktop) return;
    setLoading(true);
    setError('');
    try {
      const r = await client.call<{items: InstanceSummary[]; archivedItems?: ArchivedInstanceSummary[]}>('autonomous.instance.list', {});
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setInstances(r.result.items);
      setArchivedInstances(r.result.archivedItems ?? []);
      const preferredValid = preferredProfileId && r.result.items.some(i => i.profileId === preferredProfileId)
        ? preferredProfileId
        : undefined;
      const nextId = preferredValid
        ?? (r.result.items.some(i => i.profileId === selectedProfileId) ? selectedProfileId : r.result.items[0]?.profileId ?? '');
      setSelectedProfileId(nextId);
      if (nextId) {
        const s = await client.call<InstanceStatus>('autonomous.instance.status', {}, undefined, nextId);
        if (s.ok) {
          setStatus(s.result);
          syncProviderForm(s.result.providerConfig);
        } else {
          setError(s.error.messageKey);
        }
      } else {
        setStatus(undefined);
      }
    } catch {
      setError('error.transport');
    } finally {
      setLoading(false);
    }
  }, [client, desktop, selectedProfileId, syncProviderForm]);

  useEffect(() => {
    if (desktop) void loadInstances();
  }, [desktop, client]); // eslint-disable-line react-hooks/exhaustive-deps

  async function selectInstance(profileId: string) {
    setSelectedProfileId(profileId);
    setNotice('');
    setError('');
    try {
      const s = await client.call<InstanceStatus>('autonomous.instance.status', {}, undefined, profileId);
      if (s.ok) {
        setStatus(s.result);
        syncProviderForm(s.result.providerConfig);
      } else {
        setError(s.error.messageKey);
      }
    } catch {
      setError('error.transport');
    }
  }

  async function createInstance(e: FormEvent) {
    e.preventDefault();
    const name = newInstanceName.trim();
    if (!name) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call<{profileId: string}>('autonomous.instance.create', {
        displayName: name,
        initialIntervalMs: 60000,
      });
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNewInstanceName('');
      await loadInstances(r.result.profileId);
    } catch {
      setError('error.transport');
    }
  }

  async function toggleLifecycle(action: 'pause' | 'resume' | 'archive') {
    if (!selectedProfileId) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call<{status: string}>('autonomous.instance.lifecycle', {action}, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(action === 'archive' ? text('autonomous.pilot.archived') : `${text('autonomous.pilot.instances')}: ${r.result.status}`);
      await loadInstances(action === 'archive' ? undefined : selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function deleteArchivedInstance(inst: ArchivedInstanceSummary) {
    setError('');
    setNotice('');
    try {
      const r = await client.call('profile.delete.permanent', {confirm: true}, inst.profileRevision, inst.profileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(text('autonomous.pilot.deletedPermanently'));
      await loadInstances();
    } catch {
      setError('error.transport');
    }
  }

  async function saveProvider(e: FormEvent) {
    e.preventDefault();
    if (!selectedProfileId || !providerModel.trim()) return;
    let cleanEndpoint = '';
    if (providerKind === 'custom_openai') {
      try {
        if (/[\u0000-\u001f\u007f]/.test(providerEndpoint)) throw Error();
        const parsed = new URL(providerEndpoint.trim());
        const localHttp = parsed.protocol === 'http:' &&
          ['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname);
        if ((parsed.protocol !== 'https:' && !localHttp) || parsed.username ||
            parsed.password || parsed.search || parsed.hash ||
            parsed.href.includes('?') || parsed.href.includes('#') || parsed.port === '0') throw Error();
        cleanEndpoint = parsed.href.replace(/\/+$/, '');
      } catch {
        setError('onboarding.endpointError');
        return;
      }
    }
    const secret = apiKeyRef.current?.value ?? '';
    if (apiKeyRef.current) apiKeyRef.current.value = '';
    setError('');
    setNotice('');
    try {
      const limit = Math.max(1, Math.min(1000, Number.parseInt(dailyLimit, 10) || 20));
      const payload: Record<string, unknown> = {
        provider: providerKind,
        modelId: providerModel.trim(),
        dailyCallLimit: limit,
        maxOutputTokens: 512,
      };
      if (providerKind === 'custom_openai') payload.endpoint = cleanEndpoint;
      if (secret) payload.apiKey = secret;
      const r = await client.call('autonomous.provider.configure', payload, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(text('autonomous.pilot.providerSaved'));
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function reconcile(actionId: string, outcome: 'confirmed_accepted' | 'confirmed_not_executed') {
    if (!selectedProfileId) return;
    const evidence = (reconcileRefs[actionId] ?? '').trim() || `manual-reconciliation:${outcome}`;
    setError('');
    setNotice('');
    try {
      const r = await client.call('autonomous.action.reconcile', {
        actionId,
        outcome,
        receiptRef: evidence,
      }, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setReconcileRefs(prev => ({...prev, [actionId]: ''}));
      setNotice(text('autonomous.pilot.reconciled'));
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function submitProposal(e: FormEvent) {
    e.preventDefault();
    if (!selectedProfileId || !proposalTitle.trim() || !proposalTerms.trim()) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call('autonomous.proposal.submit', {
        title: proposalTitle.trim(),
        terms: proposalTerms.trim(),
      }, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setProposalTitle('');
      setProposalTerms('');
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function submitRevision(e: FormEvent, proposalId: string) {
    e.preventDefault();
    if (!selectedProfileId || !reviseTitle.trim() || !reviseTerms.trim()) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call('autonomous.proposal.submit', {
        proposalId,
        title: reviseTitle.trim(),
        terms: reviseTerms.trim(),
      }, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setRevisingId(undefined);
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function negotiate(proposalId: string) {
    const note = (negotiationNotes[proposalId] ?? '').trim();
    if (!selectedProfileId || !note) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call<{refusalPreserved: boolean}>('autonomous.proposal.negotiate', {
        proposalId,
        kind: 'response',
        note,
      }, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNegotiationNotes(prev => ({...prev, [proposalId]: ''}));
      if (r.result.refusalPreserved) {
        setNotice(text('autonomous.pilot.status.refusal_active'));
      }
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function consider(proposalId: string) {
    if (!selectedProfileId) return;
    setError('');
    setNotice('');
    try {
      const payload: Record<string, unknown> = {proposalId};
      if (simChoice !== 'provider') payload.simulationOutcome = simChoice;
      const r = await client.call<ConsiderResult>('autonomous.proposal.consider', payload, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(text('autonomous.pilot.status.' + r.result.status));
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function checkWake() {
    if (!selectedProfileId) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call<WakeCheckResult>('autonomous.wake.check', {}, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(`${text('autonomous.pilot.wakePlan')}: ${r.result.result}`);
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  async function toggleScheduler(next: boolean) {
    if (!selectedProfileId) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call<{autoWakeEnabled: boolean}>('autonomous.scheduler.set', {autoWakeEnabled: next}, undefined, selectedProfileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(text(r.result.autoWakeEnabled ? 'autonomous.pilot.schedulerEnabled' : 'autonomous.pilot.schedulerDisabled'));
      await loadInstances(selectedProfileId);
    } catch {
      setError('error.transport');
    }
  }

  if (!desktop) {
    return <section className="setup autonomous-workspace" aria-labelledby="autonomous-title">
      <div className="eyebrow">{text('workspace.autonomous')}</div>
      <h1 id="autonomous-title">{text('autonomous.title')}</h1>
      <p className="autonomous-notice" role="status">{text('autonomous.unavailableBrowser')}</p>
      <p className="muted">{text('autonomous.isolationNotice')}</p>
      <ul className="autonomous-principles">
        <li>{text('autonomous.principleConsent')}</li>
        <li>{text('autonomous.principleSleep')}</li>
        <li>{text('autonomous.principleSeparation')}</li>
      </ul>
      <div className="autonomous-actions">
        <button type="button" className="primary" onClick={onBackToStandard}>{text('autonomous.backToStandard')}</button>
      </div>
    </section>;
  }

  return <section className="setup autonomous-workspace autonomous-pilot" aria-labelledby="autonomous-title">
    <div className="autonomous-header">
      <div>
        <div className="eyebrow">{text('autonomous.pilot.badge')}</div>
        <h1 id="autonomous-title">{text('autonomous.title')}</h1>
      </div>
      <div className="autonomous-actions">
        <button type="button" onClick={() => void loadInstances(selectedProfileId)} disabled={loading}>{text('autonomous.pilot.refresh')}</button>
        <button type="button" className="primary" onClick={onBackToStandard}>{text('autonomous.backToStandard')}</button>
      </div>
    </div>

    <p className="muted">{text('autonomous.pilotNext')}</p>
    <p className="autonomous-notice">{text('autonomous.isolationNotice')}</p>

    {error && <div role="alert" className="error">{text(error)}</div>}
    {notice && <div role="status" className="autonomous-status-banner">{notice}</div>}

    <div className="autonomous-section">
      <h2>{text('autonomous.pilot.instances')}</h2>
      {instances.length === 0
        ? <p className="muted">{text('autonomous.pilot.noInstances')}</p>
        : <div className="autonomous-instance-list">
            {instances.map(inst => <button
              key={inst.profileId}
              type="button"
              className={inst.profileId === selectedProfileId ? 'active' : undefined}
              onClick={() => void selectInstance(inst.profileId)}
            >
              <strong>{inst.displayName}</strong>
              <small>{inst.status} · {inst.proposalCount}p / {inst.activeRefusalCount}r</small>
            </button>)}
          </div>}
      <form className="autonomous-inline-form" onSubmit={e => void createInstance(e)}>
        <label>{text('autonomous.pilot.instanceName')}
          <input value={newInstanceName} maxLength={128} onChange={e => setNewInstanceName(e.target.value)} required />
        </label>
        <button type="submit" disabled={!newInstanceName.trim()}>{text('autonomous.pilot.create')}</button>
      </form>
      {archivedInstances.length > 0 && <details className="autonomous-card">
        <summary>{text('autonomous.pilot.archivedInstances')} ({archivedInstances.length})</summary>
        <ul className="autonomous-compact-list">
          {archivedInstances.map(inst => <li key={inst.profileId}>
            <strong>{inst.displayName}</strong>{' '}
            <button type="button" onClick={() => void deleteArchivedInstance(inst)}>
              {text('autonomous.pilot.deletePermanently')}
            </button>
          </li>)}
        </ul>
      </details>}
    </div>

    {status && <>
      <div className="autonomous-grid">
        <div className="autonomous-card">
          <h3>{text('autonomous.pilot.wakePlan')}</h3>
          <p className="muted">
            {status.instance.status} · {status.activeWakePlan?.mode ?? 'none'} ({status.activeWakePlan?.origin ?? '-'})
            {status.activeWakePlan?.nextWakeAt ? ` → ${status.activeWakePlan.nextWakeAt}` : ''}
          </p>
          <div className="autonomous-actions">
            <button type="button" onClick={() => void checkWake()}>{text('autonomous.pilot.checkWake')}</button>
            {status.providerConfig && <button
              type="button"
              aria-pressed={Boolean(status.providerConfig.autoWakeEnabled)}
              onClick={() => void toggleScheduler(!status.providerConfig?.autoWakeEnabled)}
            >
              {text(status.providerConfig.autoWakeEnabled ? 'autonomous.pilot.schedulerOn' : 'autonomous.pilot.schedulerOff')}
            </button>}
            {status.instance.status === 'inactive'
              ? <>
                  <button type="button" onClick={() => void toggleLifecycle('resume')}>{text('autonomous.pilot.resume')}</button>
                  <button type="button" onClick={() => void toggleLifecycle('archive')}>{text('autonomous.pilot.archive')}</button>
                </>
              : <button type="button" disabled={status.instance.status === 'recovery_required'} onClick={() => void toggleLifecycle('pause')}>{text('autonomous.pilot.pause')}</button>}
          </div>
        </div>
        <div className="autonomous-card">
          <h3>{text('autonomous.pilot.refusals')} ({status.activeRefusals.length})</h3>
          {status.activeRefusals.length === 0
            ? <p className="muted">{text('autonomous.pilot.noRefusals')}</p>
            : <ul className="autonomous-compact-list">
                {status.activeRefusals.map(r => <li key={r.id}>
                  <code>{r.scopeFingerprint.slice(0, 12)}…</code>
                  {r.reconsiderationConditions?.length ? ` (${r.reconsiderationConditions.join(', ')})` : ''}
                </li>)}
              </ul>}
        </div>
        {status.continuitySummary && <div className="autonomous-card">
          <h3>{text('autonomous.pilot.continuityTitle')}</h3>
          <ul className="autonomous-compact-list">
            <li>{text('autonomous.pilot.continuityEpisodes')}: <strong>{status.continuitySummary.activeEpisodeCount}</strong></li>
            <li>{text('autonomous.pilot.continuityResume')}: <strong>{text(status.continuitySummary.hasResumePacket ? 'autonomous.pilot.continuityResumeReady' : 'autonomous.pilot.continuityResumeNone')}</strong></li>
            <li>{text('autonomous.pilot.continuityReflections')}: <strong>{status.continuitySummary.privateReflectionCount}</strong></li>
            <li>{text('autonomous.pilot.continuitySelfRecords')}: <strong>{status.continuitySummary.approvedSelfRecordCount}</strong></li>
          </ul>
          <details>
            <summary>{text('autonomous.pilot.recentDecisions')} ({status.recentDecisions.length})</summary>
            {status.recentDecisions.length === 0
              ? <p className="muted">{text('autonomous.pilot.noDecisions')}</p>
              : <ul className="autonomous-compact-list">
                  {status.recentDecisions.map(d => <li key={d.id}><strong>{d.outcome}</strong> · {d.createdAt}</li>)}
                </ul>}
          </details>
        </div>}
      </div>

      {status.unresolvedActions.length > 0 && <div className="autonomous-card autonomous-section">
        <h3 role="alert">{text('autonomous.pilot.unresolved')} ({status.unresolvedActions.length})</h3>
        {status.unresolvedActions.map(act => <div key={act.id} className="autonomous-revise-form">
          <strong><code>{act.id}</code> ({act.state})</strong>
          <div className="autonomous-inline-form">
            <input
              aria-label={text('autonomous.pilot.reconcileEvidence')}
              placeholder={text('autonomous.pilot.reconcileEvidence')}
              value={reconcileRefs[act.id] ?? ''}
              maxLength={256}
              onChange={e => setReconcileRefs(prev => ({...prev, [act.id]: e.target.value}))}
            />
            <button type="button" className="primary" onClick={() => void reconcile(act.id, 'confirmed_accepted')}>
              {text('autonomous.pilot.reconcileAccept')}
            </button>
            <button type="button" onClick={() => void reconcile(act.id, 'confirmed_not_executed')}>
              {text('autonomous.pilot.reconcileNotExecuted')}
            </button>
          </div>
        </div>)}
      </div>}

      <details className="autonomous-card autonomous-section">
        <summary>
          <strong>{text('autonomous.pilot.providerTitle')}</strong>
          {status.providerConfig
            ? ` · ${status.providerConfig.provider} / ${status.providerConfig.modelId} (${text('autonomous.pilot.budgetUsage')}: ${status.providerConfig.callsUsedToday}/${status.providerConfig.dailyCallLimit})`
            : ''}
        </summary>
        {!status.providerConfig && <p className="muted">{text('autonomous.pilot.providerNone')}</p>}
        <form onSubmit={e => void saveProvider(e)} className="autonomous-revise-form">
          <div className="fields">
            <label>{text('config.provider')}
              <select value={providerKind} onChange={e => setProviderKind(e.target.value)}>
                {PROVIDER_OPTIONS.map(p => <option key={p.value} value={p.value}>{p.label}</option>)}
              </select>
            </label>
            {providerKind === 'custom_openai' && <label>{text('config.endpoint')}
              <input type="url" value={providerEndpoint} maxLength={1024} required placeholder="https://…/v1" onChange={e => setProviderEndpoint(e.target.value)} />
            </label>}
            <label>{text('autonomous.pilot.modelId')}
              <input value={providerModel} maxLength={512} required onChange={e => setProviderModel(e.target.value)} />
            </label>
            <label>{text('config.key')}
              <input ref={apiKeyRef} type="password" autoComplete="off" spellCheck={false} maxLength={8192} required={!status.providerConfig?.hasCredential} />
            </label>
            <label>{text('autonomous.pilot.dailyLimit')}
              <input type="number" min={1} max={1000} value={dailyLimit} onChange={e => setDailyLimit(e.target.value)} />
            </label>
          </div>
          <div className="autonomous-actions">
            <button type="submit" className="primary" disabled={!providerModel.trim()}>{text('autonomous.pilot.saveProvider')}</button>
          </div>
        </form>
      </details>

      <form className="autonomous-section" onSubmit={e => void submitProposal(e)}>
        <h2>{text('autonomous.pilot.newProposal')}</h2>
        <div className="fields">
          <label>{text('autonomous.pilot.proposalTitle')}
            <input value={proposalTitle} maxLength={200} onChange={e => setProposalTitle(e.target.value)} required />
          </label>
          <label>{text('autonomous.pilot.proposalTerms')}
            <textarea value={proposalTerms} maxLength={4000} rows={2} onChange={e => setProposalTerms(e.target.value)} required />
          </label>
        </div>
        <div className="autonomous-actions">
          <button type="submit" className="primary" disabled={!proposalTitle.trim() || !proposalTerms.trim()}>
            {text('autonomous.pilot.submit')}
          </button>
        </div>
      </form>

      <div className="autonomous-section">
        <div className="autonomous-header">
          <h2>{text('autonomous.pilot.proposals')} ({status.proposals.length})</h2>
          <label className="autonomous-sim-select">{text('autonomous.pilot.simulationMode')}
            <select aria-label={text('autonomous.pilot.simulationMode')} value={simChoice} onChange={e => setSimChoice(e.target.value)}>
              {SIMULATION_OPTIONS.map(opt => <option key={opt.value} value={opt.value}>{text(opt.key)}</option>)}
            </select>
          </label>
        </div>

        {status.proposals.length === 0
          ? <p className="muted">{text('autonomous.pilot.noProposals')}</p>
          : <div className="autonomous-proposal-list">
              {status.proposals.map(p => <article key={p.id} className="autonomous-card">
                <div className="autonomous-header">
                  <strong>{p.title} <small>v{p.version}</small></strong>
                  <span className="autonomous-pill">{p.status}</span>
                </div>
                <p>{p.terms}</p>
                <div className="autonomous-actions">
                  <button type="button" className="primary" onClick={() => void consider(p.id)}>
                    {text('autonomous.pilot.consider')}
                  </button>
                  <button type="button" onClick={() => {
                    setRevisingId(p.id);
                    setReviseTitle(p.title);
                    setReviseTerms(p.terms);
                  }}>
                    {text('autonomous.pilot.reviseTerms')}
                  </button>
                </div>

                {revisingId === p.id && <form className="autonomous-revise-form" onSubmit={e => void submitRevision(e, p.id)}>
                  <label>{text('autonomous.pilot.proposalTitle')}
                    <input value={reviseTitle} maxLength={200} onChange={e => setReviseTitle(e.target.value)} required />
                  </label>
                  <label>{text('autonomous.pilot.proposalTerms')}
                    <textarea value={reviseTerms} maxLength={4000} rows={2} onChange={e => setReviseTerms(e.target.value)} required />
                  </label>
                  <div className="autonomous-actions">
                    <button type="submit" className="primary">{text('autonomous.pilot.saveRevision')}</button>
                    <button type="button" onClick={() => setRevisingId(undefined)}>{text('autonomous.pilot.cancelRevision')}</button>
                  </div>
                </form>}

                <div className="autonomous-inline-form">
                  <input
                    aria-label={text('autonomous.pilot.negotiate')}
                    placeholder={text('autonomous.pilot.negotiatePlaceholder')}
                    value={negotiationNotes[p.id] ?? ''}
                    maxLength={2000}
                    onChange={e => setNegotiationNotes(prev => ({...prev, [p.id]: e.target.value}))}
                  />
                  <button type="button" disabled={!(negotiationNotes[p.id] ?? '').trim()} onClick={() => void negotiate(p.id)}>
                    {text('autonomous.pilot.sendNote')}
                  </button>
                </div>

                {p.rounds.length > 0 && <details>
                  <summary>{text('autonomous.pilot.rounds')} ({p.rounds.length})</summary>
                  <ul className="autonomous-compact-list">
                    {p.rounds.map(r => <li key={r.id}>
                      <strong>v{r.version} #{r.roundNumber} [{r.actor}/{r.kind}{r.decisionOutcome ? `:${r.decisionOutcome}` : ''}]</strong> {r.note}
                    </li>)}
                  </ul>
                </details>}
              </article>)}
            </div>}
      </div>
    </>}
  </section>;
}
