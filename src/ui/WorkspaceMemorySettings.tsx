import {useEffect, useRef, useState} from 'react';
import type {Client} from '../ipc/client';
import {t} from '../domain/packs';
import {MemoryPanel, type MemoryAttempt} from './MemoryPanel';
import {listStandardProfiles, type SetupProfile} from './firstConversation';

export function WorkspaceMemorySettings({client, locale, desktop, initialProfileId}: {
  client: Client;
  locale: string;
  desktop: boolean;
  initialProfileId?: string;
}) {
  const [profiles, setProfiles] = useState<SetupProfile[]>([]);
  const [profileId, setProfileId] = useState(initialProfileId ?? '');
  const [error, setError] = useState(false);
  const attempts = useRef(new Map<string, MemoryAttempt>());
  useEffect(() => {
    if (!desktop) return;
    let active = true;
    void listStandardProfiles(client).then(items => {
      if (active) setProfiles(items);
    }).catch(() => { if (active) setError(true); });
    return () => { active = false; };
  }, [client, desktop]);
  if (!desktop) return null;
  const text = (key: string) => t(locale, key);
  return <section className="setup memory-settings">
    <h2>{text('settings.memory')}</h2>
    <label>{text('talk.profile')}
      <select value={profileId} onChange={e => setProfileId(e.target.value)}>
        <option value="">{text('config.choose')}</option>
        {profiles.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}
      </select>
    </label>
    {profiles.some(p => p.id === profileId) && <MemoryPanel key={profileId} client={client} locale={locale}
      profileId={profileId} conversationId="" attempts={attempts.current} settingsOnly/>}
    {error && <p role="alert" className="error">{text('error.transport')}</p>}
  </section>;
}
