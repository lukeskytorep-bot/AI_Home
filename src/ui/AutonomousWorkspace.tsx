import {useCallback, useEffect, useState, type FormEvent} from 'react';
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

interface InstanceSummary {
  profileId: string;
  instanceId: string;
  displayName: string;
  status: string;
  activeWakePlan?: WakePlan | null;
  proposalCount: number;
  activeRefusalCount: number;
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

interface InstanceStatus {
  profileId: string;
  displayName: string;
  instance: {id: string; status: string; runtimeGeneration?: number};
  activeWakePlan?: WakePlan | null;
  activeRefusals: RefusalRecord[];
  proposals: ProposalRecord[];
  recentDecisions: Array<{id: string; outcome: string; createdAt: string}>;
  unresolvedActions: Array<{id: string; state: string; actionKind?: string}>;
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

export function AutonomousWorkspace({client, locale, desktop, onBackToStandard}: {
  client: Client;
  locale: string;
  desktop: boolean;
  onBackToStandard: () => void;
}) {
  const text = (key: string) => t(locale, key);
  const [instances, setInstances] = useState<InstanceSummary[]>([]);
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

  const loadInstances = useCallback(async (preferredProfileId?: string) => {
    if (!desktop) return;
    setLoading(true);
    setError('');
    try {
      const r = await client.call<{items: InstanceSummary[]}>('autonomous.instance.list', {});
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setInstances(r.result.items);
      const nextId = preferredProfileId
        ?? (r.result.items.some(i => i.profileId === selectedProfileId) ? selectedProfileId : r.result.items[0]?.profileId ?? '');
      setSelectedProfileId(nextId);
      if (nextId) {
        const s = await client.call<InstanceStatus>('autonomous.instance.status', {}, undefined, nextId);
        if (s.ok) setStatus(s.result);
        else setError(s.error.messageKey);
      } else {
        setStatus(undefined);
      }
    } catch {
      setError('error.transport');
    } finally {
      setLoading(false);
    }
  }, [client, desktop, selectedProfileId]);

  useEffect(() => {
    if (desktop) void loadInstances();
  }, [desktop, client]); // eslint-disable-line react-hooks/exhaustive-deps

  async function selectInstance(profileId: string) {
    setSelectedProfileId(profileId);
    setNotice('');
    setError('');
    try {
      const s = await client.call<InstanceStatus>('autonomous.instance.status', {}, undefined, profileId);
      if (s.ok) setStatus(s.result);
      else setError(s.error.messageKey);
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
        initialIntervalMs: 1,
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
    </div>

    {status && <>
      <div className="autonomous-grid">
        <div className="autonomous-card">
          <h3>{text('autonomous.pilot.wakePlan')}</h3>
          <p className="muted">
            {status.instance.status} · {status.activeWakePlan?.mode ?? 'none'} ({status.activeWakePlan?.origin ?? '-'})
            {status.activeWakePlan?.nextWakeAt ? ` → ${status.activeWakePlan.nextWakeAt}` : ''}
          </p>
          <button type="button" onClick={() => void checkWake()}>{text('autonomous.pilot.checkWake')}</button>
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
          {status.unresolvedActions.length > 0 && <p role="alert" className="error">
            {text('autonomous.pilot.unresolved')}: {status.unresolvedActions.length}
          </p>}
        </div>
      </div>

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
