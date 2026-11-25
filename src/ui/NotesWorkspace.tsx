import {useEffect, useRef, useState} from 'react';
import type {Client} from '../ipc/client';
import {t} from '../domain/packs';
import {listStandardProfiles, type SetupProfile} from './firstConversation';
import {MemoryPanel, type MemoryAttempt} from './MemoryPanel';
import {MemoryLayersPanel, type LibraryWrite} from './MemoryLayersPanel';

export function NotesWorkspace({client, locale, desktop, initialProfileId}: {
  client: Client;
  locale: string;
  desktop: boolean;
  initialProfileId?: string;
}) {
  const text = (key: string) => t(locale, key);
  const [profiles, setProfiles] = useState<SetupProfile[]>([]);
  const [profileId, setProfileId] = useState(initialProfileId ?? '');
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
