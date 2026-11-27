import {useEffect, useRef, useState} from 'react';
import type {Client} from '../ipc/client';
import {t} from '../domain/packs';
import {listStandardProfiles, type SetupProfile} from './firstConversation';
import {MemoryPanel, type MemoryAttempt} from './MemoryPanel';
import {MemoryLayersPanel, type LibraryWrite} from './MemoryLayersPanel';

interface ContinuityCounters {
  displayName: string;
  activeEpisodeCount: number;
  approvedSelfRecordCount: number;
  privateReflectionCount: number;
}

export function NotesWorkspace({client, locale, desktop, initialProfileId}: {
  client: Client;
  locale: string;
  desktop: boolean;
  initialProfileId?: string;
}) {
  const text = (key: string) => t(locale, key);
  const [profiles, setProfiles] = useState<SetupProfile[]>([]);
  const [profileId, setProfileId] = useState(initialProfileId ?? '');
  const [identityBook, setIdentityBook] = useState<ContinuityCounters | null>(null);
  const [error, setError] = useState('');
  const attempts = useRef(new Map<string, MemoryAttempt>());
  const writes = useRef(new Map<string, LibraryWrite>());

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    setError('');
    void listStandardProfiles(client).then(items => {
      if (!active) return;
      setProfiles(items);
      setProfileId(old => {
        if (old && items.some(p => p.id === old)) return old;
        if (initialProfileId && items.some(p => p.id === initialProfileId)) return initialProfileId;
        return items[0]?.id ?? '';
      });
    }).catch(() => {
      if (active) setError('error.transport');
    });
    void client.call<{items: Array<{profileId: string; displayName: string}>}>('autonomous.instance.list', {})
      .then(async res => {
        if (!active || !res.ok || !res.result.items.length) {
          if (active) setIdentityBook(null);
          return;
        }
        const first = res.result.items[0];
        const statusRes = await client.call<{
          displayName: string;
          continuitySummary?: {activeEpisodeCount: number; approvedSelfRecordCount: number; privateReflectionCount: number};
        }>('autonomous.instance.status', {}, undefined, first.profileId);
        if (!active || !statusRes.ok || !statusRes.result.continuitySummary) return;
        setIdentityBook({
          displayName: statusRes.result.displayName,
          activeEpisodeCount: statusRes.result.continuitySummary.activeEpisodeCount,
          approvedSelfRecordCount: statusRes.result.continuitySummary.approvedSelfRecordCount,
          privateReflectionCount: statusRes.result.continuitySummary.privateReflectionCount,
        });
      })
      .catch(() => {
        // Optional read-only summary; keep Notes fully functional if autonomous summary is unavailable.
      });
    return () => { active = false; };
  }, [client, desktop, initialProfileId]);

  if (!desktop) {
    return <section className="setup notes-workspace" aria-labelledby="notes-title">
      <h1 id="notes-title">{text('notes.title')}</h1>
      <p className="autonomous-notice" role="status">{text('notes.unavailableBrowser')}</p>
      <p className="muted">{text('notes.readOnlyNotice')}</p>
    </section>;
  }

  return <section className="setup notes-workspace" aria-labelledby="notes-title">
    <h1 id="notes-title">{text('notes.title')}</h1>
    <p className="muted">{text('notes.subtitle')}</p>
    <p className="autonomous-notice" role="note">{text('notes.readOnlyNotice')}</p>
    {identityBook && <div className="autonomous-card identity-book">
      <h2>{text('ux2.identityTitle')} · {identityBook.displayName}</h2>
      <p className="muted">{text('ux2.identitySubtitle')}</p>
      <div className="identity-metrics">
        <div className="identity-metric"><span>{text('ux2.identityMissions')}</span><strong>{identityBook.activeEpisodeCount}</strong></div>
        <div className="identity-metric"><span>{text('ux2.identityValues')}</span><strong>{identityBook.approvedSelfRecordCount}</strong></div>
        <div className="identity-metric"><span>{text('ux2.identityPrivate')}</span><strong>{identityBook.privateReflectionCount}</strong></div>
      </div>
      <p className="muted field-hint">{text('ux2.identityPrivateNote')}</p>
    </div>}
    {error && <p role="alert" className="error">{text(error)}</p>}
    {profiles.length === 0
      ? <p className="muted">{text('notes.noProfiles')}</p>
      : <>
          <label>{text('talk.profile')}
            <select aria-label={text('talk.profile')} value={profileId} onChange={e => setProfileId(e.target.value)}>
              {profiles.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}
            </select>
          </label>
          {profiles.some(p => p.id === profileId) && <>
            <MemoryPanel
              key={'working-' + profileId}
              client={client}
              locale={locale}
              profileId={profileId}
              conversationId=""
              attempts={attempts.current}
              hideSettings
            />
            <MemoryLayersPanel
              key={'layers-' + profileId}
              client={client}
              locale={locale}
              profileId={profileId}
              conversationId=""
              attempts={attempts.current}
              writes={writes.current}
              onContextChange={() => {}}
            />
          </>}
        </>}
  </section>;
}
