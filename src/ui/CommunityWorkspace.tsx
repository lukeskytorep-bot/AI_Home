import {useEffect, useState} from 'react';
import type {Transport} from '../domain/types';
import type {Client} from '../ipc/client';
import {t} from '../domain/packs';
import {listStandardProfiles, type SetupProfile} from './firstConversation';
import {CommunityPanel} from './CommunityPanel';

interface ContactEntry {
  record: {id: string; kind: string; state: string; subjectRef?: string; createdAt: string};
  metadata: {title: string};
  version?: {content: string};
}

export function CommunityWorkspace({client, transport, locale, initialProfileId}: {
  client: Client;
  transport: Transport;
  locale: string;
  initialProfileId?: string;
}) {
  const text = (key: string) => t(locale, key);
  const desktop = transport.mode === 'desktop';
  const communityConfigured = desktop && Boolean(transport.communityDispatch);
  const [profiles, setProfiles] = useState<SetupProfile[]>([]);
  const [profileId, setProfileId] = useState(initialProfileId ?? '');
  const [contacts, setContacts] = useState<ContactEntry[]>([]);
  const [contactError, setContactError] = useState('');

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    void listStandardProfiles(client).then(items => {
      if (!active) return;
      setProfiles(items);
      setProfileId(old => {
        if (old && items.some(p => p.id === old)) return old;
        if (initialProfileId && items.some(p => p.id === initialProfileId)) return initialProfileId;
        return items[0]?.id ?? '';
      });
    }).catch(() => {
      if (active) setContactError('error.transport');
    });
    return () => { active = false; };
  }, [client, desktop, initialProfileId]);

  useEffect(() => {
    if (!desktop || !profileId) {
      setContacts([]);
      return;
    }
    let active = true;
    setContactError('');
    void (async () => {
      try {
        let cursor = '';
        let pages = 0;
        const found: ContactEntry[] = [];
        do {
          const r = await client.call<{items: ContactEntry[]; nextCursor: string}>(
            'memory.records.list',
            {page: {limit: 50, ...(cursor ? {cursor} : {})}},
            undefined,
            profileId,
          );
          if (!active) return;
          if (!r.ok) {
            setContactError(r.error.messageKey);
            setContacts([]);
            return;
          }
          found.push(...r.result.items.filter(item => item.record.kind === 'relationship' && item.record.state !== 'deleted'));
          cursor = r.result.nextCursor;
          pages += 1;
        } while (cursor && pages < 20);
        if (active) setContacts(found);
      } catch {
        if (active) {
          setContactError('error.transport');
          setContacts([]);
        }
      }
    })();
    return () => { active = false; };
  }, [client, desktop, profileId]);

  return <div className="community-workspace">
    <section className="setup" aria-labelledby="community-workspace-title">
      <h1 id="community-workspace-title">{text('communityView.title')}</h1>
      <p className="muted">{text('communityView.subtitle')}</p>
      <div className="autonomous-card">
        <h2>{text('communityView.statusSummary')}</h2>
        <ul className="autonomous-compact-list">
          <li><strong>{text('communityView.configLabel')}:</strong> {text(communityConfigured ? 'communityView.configPresent' : 'communityView.configAbsent')}</li>
          <li><strong>{text('communityView.credentialLabel')}:</strong> {text('communityView.credentialCheckHint')}</li>
          <li><strong>{text('communityView.onlineLabel')}:</strong> {text('communityView.onlineUnknown')}</li>
        </ul>
      </div>
      <p className="autonomous-notice">{text('communityView.scopeNotice')}</p>
    </section>

    <section className="setup" aria-labelledby="community-contacts-title">
      <h2 id="community-contacts-title">{text('communityView.contactsTitle')}</h2>
      <p className="muted">{text('communityView.contactsSubtitle')}</p>
      {!desktop
        ? <p className="muted">{text('communityView.contactsBrowser')}</p>
        : <>
            {profiles.length > 0 && <label>{text('talk.profile')}
              <select aria-label={text('talk.profile')} value={profileId} onChange={e => setProfileId(e.target.value)}>
                {profiles.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}
              </select>
            </label>}
            {contactError && <p role="alert" className="error">{text(contactError)}</p>}
            {contacts.length === 0
              ? <p className="muted">{text('communityView.contactsEmpty')}</p>
              : <ul className="autonomous-compact-list">
                  {contacts.map(c => <li key={c.record.id}>
                    <strong>{c.metadata.title}</strong>
                    {c.record.subjectRef ? ` (${c.record.subjectRef})` : ''}
                    {' · '}<span>{c.record.state}</span>
                    {c.version?.content ? <p className="muted">{c.version.content}</p> : null}
                  </li>)}
                </ul>}
          </>}
    </section>

    <CommunityPanel client={client} transport={transport} locale={locale} />
  </div>;
}
