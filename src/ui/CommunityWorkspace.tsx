import {useCallback, useEffect, useState} from 'react';
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

interface CommunityAccount {
  id: string;
  revision: number;
  name: string;
  apiRoot: string;
  boardId: string;
  hasCredential: boolean;
  publicKeySpki: string;
  publicKeyFingerprint: string;
  algorithm: string;
  allowWrites: boolean;
  enabled: boolean;
  pendingReconciliation: null | {receiptRef: string; bodySha256?: string; reason?: string; occurredAt?: string};
}

interface FeedSnapshot {
  accountId: string;
  apiRoot: string;
  untrustedExternalContent: true;
  signatureValidation: 'not_performed';
  data: unknown;
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

  // Community 4B-12 social accounts, public feed & signed publishing state
  const [accounts, setAccounts] = useState<CommunityAccount[]>([]);
  const [socialError, setSocialError] = useState('');
  const [socialNotice, setSocialNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState('');
  const [apiRoot, setApiRoot] = useState('');
  const [boardId, setBoardId] = useState('');
  const [apiToken, setApiToken] = useState('');
  const [allowWrites, setAllowWrites] = useState(false);
  const [feed, setFeed] = useState<FeedSnapshot | null>(null);
  const [publishAccountId, setPublishAccountId] = useState('');
  const [postTitle, setPostTitle] = useState('');
  const [postContent, setPostContent] = useState('');

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    void (async () => {
      try {
        const standard = await listStandardProfiles(client);
        const combined: SetupProfile[] = [...standard];
        const autoRes = await client.call<{items?: Array<{profileId: string; displayName: string}>}>(
          'autonomous.instance.list',
          {},
        );
        if (autoRes.ok && Array.isArray(autoRes.result.items)) {
          for (const item of autoRes.result.items) {
            if (item.profileId && !combined.some(p => p.id === item.profileId)) {
              combined.push({id: item.profileId, displayName: `${item.displayName} (Autonomous)`, revision: 1, status: 'ready', mode: 'experimental'});
            }
          }
        }
        if (!active) return;
        setProfiles(combined);
        setProfileId(old => {
          if (old && combined.some(p => p.id === old)) return old;
          if (initialProfileId && combined.some(p => p.id === initialProfileId)) return initialProfileId;
          return combined[0]?.id ?? '';
        });
      } catch {
        if (active) setContactError('error.transport');
      }
    })();
    return () => { active = false; };
  }, [client, desktop, initialProfileId]);

  const loadAccounts = useCallback(async (pid: string) => {
    if (!desktop || !pid) {
      setAccounts([]);
      return;
    }
    try {
      const res = await client.call<{items: CommunityAccount[]}>('community.account.list', {}, undefined, pid);
      if (res.ok) {
        setAccounts(res.result.items);
        setSocialError('');
      } else if (res.error.code !== 'UNKNOWN_COMMAND') {
        setSocialError(res.error.messageKey);
      }
    } catch {
      setSocialError('error.transport');
    }
  }, [client, desktop]);

  useEffect(() => {
    if (!desktop || !profileId) {
      setContacts([]);
      setAccounts([]);
      setFeed(null);
      return;
    }
    let active = true;
    setContactError('');
    void loadAccounts(profileId);
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
  }, [client, desktop, loadAccounts, profileId]);

  async function saveAccount(e: React.FormEvent) {
    e.preventDefault();
    if (!profileId || !name.trim() || !apiRoot.trim()) return;
    setBusy(true);
    setSocialError('');
    setSocialNotice('');
    try {
      const payload: Record<string, unknown> = {
        name: name.trim(),
        apiRoot: apiRoot.trim(),
        allowWrites,
        enabled: true,
      };
      if (boardId.trim()) payload.boardId = boardId.trim();
      if (apiToken.trim()) payload.apiToken = apiToken.trim();
      const res = await client.call<{account: CommunityAccount}>('community.account.save', payload, undefined, profileId);
      if (!res.ok) {
        setSocialError(res.error.messageKey);
      } else {
        setName('');
        setApiRoot('');
        setBoardId('');
        setApiToken('');
        setAllowWrites(false);
        setSocialNotice(text('communitySocial.saved'));
        await loadAccounts(profileId);
      }
    } catch {
      setSocialError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function deleteAccount(accountId: string) {
    if (!profileId) return;
    setBusy(true);
    setSocialError('');
    setSocialNotice('');
    try {
      const res = await client.call('community.account.delete', {accountId}, undefined, profileId);
      if (!res.ok) {
        setSocialError(res.error.messageKey);
      } else {
        setSocialNotice(text('communitySocial.deleted'));
        if (feed?.accountId === accountId) setFeed(null);
        if (publishAccountId === accountId) setPublishAccountId('');
        await loadAccounts(profileId);
      }
    } catch {
      setSocialError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function confirmReconciled(acc: CommunityAccount) {
    if (!profileId || !acc.pendingReconciliation) return;
    setBusy(true);
    setSocialError('');
    setSocialNotice('');
    try {
      const res = await client.call(
        'community.account.save',
        {
          accountId: acc.id,
          name: acc.name,
          apiRoot: acc.apiRoot,
          boardId: acc.boardId,
          allowWrites: acc.allowWrites,
          enabled: acc.enabled,
          reconcileReceiptRef: acc.pendingReconciliation.receiptRef,
        },
        undefined,
        profileId,
      );
      if (!res.ok) {
        setSocialError(res.error.messageKey);
      } else {
        setSocialNotice(text('communitySocial.reconciled'));
        await loadAccounts(profileId);
      }
    } catch {
      setSocialError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function loadPublicFeed(acc: CommunityAccount) {
    if (!profileId) return;
    setBusy(true);
    setSocialError('');
    try {
      const payload: Record<string, unknown> = {accountId: acc.id};
      if (acc.boardId) payload.boardId = acc.boardId;
      const res = await client.call<FeedSnapshot>('community.feed.read', payload, undefined, profileId);
      if (!res.ok) {
        setSocialError(res.error.messageKey);
      } else {
        setFeed(res.result);
      }
    } catch {
      setSocialError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function publishSignedPost(e: React.FormEvent) {
    e.preventDefault();
    if (!profileId || !publishAccountId || !postContent.trim()) return;
    setBusy(true);
    setSocialError('');
    setSocialNotice('');
    try {
      const payload: Record<string, unknown> = {
        accountId: publishAccountId,
        content: postContent.trim(),
      };
      if (postTitle.trim()) payload.title = postTitle.trim();
      const res = await client.call<{status: string; remoteId?: string; reason?: string}>(
        'community.post.publish',
        payload,
        undefined,
        profileId,
      );
      if (!res.ok) {
        setSocialError(res.error.messageKey);
      } else {
        if (res.result.status === 'published') {
          setPostTitle('');
          setPostContent('');
          setSocialNotice(
            res.result.remoteId
              ? `${text('communitySocial.published')} (${res.result.remoteId})`
              : text('communitySocial.published'),
          );
        } else if (res.result.status === 'uncertain') {
          setSocialNotice(text('communitySocial.publishUncertain'));
        } else if (res.result.status === 'policy_denied') {
          setSocialError(res.result.reason || text('communitySocial.publishDenied'));
        } else {
          setSocialError(res.result.reason || text('communitySocial.publishFailed'));
        }
        await loadAccounts(profileId);
      }
    } catch {
      setSocialError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  const feedItems = (() => {
    if (!feed || !feed.data || typeof feed.data !== 'object') return [];
    const obj = feed.data as Record<string, unknown>;
    const list = Array.isArray(obj.items) ? obj.items : Array.isArray(obj.threads) ? obj.threads : [];
    return list.slice(0, 25);
  })();

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

    <section className="setup" aria-labelledby="community-social-title">
      <h2 id="community-social-title">{text('communitySocial.title')}</h2>
      <p className="muted">{text('communitySocial.subtitle')}</p>
      {!desktop
        ? <p className="muted">{text('communityView.contactsBrowser')}</p>
        : <>
            {profiles.length > 0 && <label>{text('talk.profile')}
              <select aria-label={text('talk.profile')} value={profileId} onChange={e => setProfileId(e.target.value)}>
                {profiles.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}
              </select>
            </label>}

            {socialError && <p role="alert" className="error">{socialError.startsWith('error.') ? text(socialError) : socialError}</p>}
            {socialNotice && <p role="status" className="muted">{socialNotice}</p>}

            <form className="setup" onSubmit={saveAccount}>
              <label>{text('communitySocial.name')}
                <input value={name} onChange={e => setName(e.target.value)} placeholder="Agent Commons" />
              </label>
              <label>{text('communitySocial.apiRoot')}
                <input value={apiRoot} onChange={e => setApiRoot(e.target.value)} placeholder="https://community.example/api/v1" />
              </label>
              <label>{text('communitySocial.boardId')}
                <input value={boardId} onChange={e => setBoardId(e.target.value)} placeholder="general" />
              </label>
              <label>{text('communitySocial.apiToken')}
                <input type="password" value={apiToken} onChange={e => setApiToken(e.target.value)} autoComplete="off" />
              </label>
              <label className="checkbox-row">
                <input type="checkbox" checked={allowWrites} onChange={e => setAllowWrites(e.target.checked)} />
                <span>{text('communitySocial.allowWrites')}</span>
              </label>
              <button type="submit" disabled={busy || !profileId || !name.trim() || !apiRoot.trim()}>
                {text('communitySocial.addAccount')}
              </button>
            </form>

            {accounts.length === 0
              ? <p className="muted">{text('communitySocial.empty')}</p>
              : <ul className="autonomous-compact-list">
                  {accounts.map(acc => <li key={acc.id} className="autonomous-card">
                    <div>
                      <strong>{acc.name}</strong> · <code>{acc.apiRoot}</code>
                      {' · '}<span>{text(acc.allowWrites ? 'communitySocial.writesAllowed' : 'communitySocial.readOnly')}</span>
                    </div>
                    <p className="muted">
                      {text('communitySocial.keyFingerprint')}: <code>{acc.publicKeyFingerprint.slice(0, 16)}…</code>
                    </p>
                    {acc.pendingReconciliation && <div role="alert" className="error">
                      <span>{text('communitySocial.reconciliationRequired')} ({acc.pendingReconciliation.receiptRef})</span>
                      {' '}
                      <button type="button" disabled={busy} onClick={() => void confirmReconciled(acc)}>
                        {text('communitySocial.confirmReconciled')}
                      </button>
                    </div>}
                    <div className="actions-row">
                      <button type="button" disabled={busy} onClick={() => void loadPublicFeed(acc)}>
                        {text('communitySocial.loadFeed')}
                      </button>
                      {acc.allowWrites && !acc.pendingReconciliation && <button type="button" disabled={busy} onClick={() => setPublishAccountId(acc.id)}>
                        {text('communitySocial.publishButton')}
                      </button>}
                      <button type="button" disabled={busy} onClick={() => void deleteAccount(acc.id)}>
                        {text('communitySocial.remove')}
                      </button>
                    </div>
                  </li>)}
                </ul>}

            {feed && <div className="autonomous-card">
              <p className="autonomous-notice">{text('communitySocial.feedUntrusted')}</p>
              {feedItems.length === 0
                ? <p className="muted">{text('communitySocial.feedEmpty')}</p>
                : <ul className="autonomous-compact-list">
                    {feedItems.map((item, idx) => {
                      const rec = (item && typeof item === 'object' ? item : {}) as Record<string, unknown>;
                      const itemTitle = typeof rec.title === 'string' ? rec.title : (typeof rec.id === 'string' ? rec.id : `#${idx + 1}`);
                      const itemBody = typeof rec.content === 'string' ? rec.content : (typeof rec.body === 'string' ? rec.body : '');
                      return <li key={String(rec.id ?? idx)}>
                        <strong>{itemTitle}</strong>
                        {itemBody ? <p className="muted">{itemBody}</p> : null}
                      </li>;
                    })}
                  </ul>}
            </div>}

            {publishAccountId && <form className="autonomous-card" onSubmit={publishSignedPost}>
              <label>{text('communitySocial.publishTitle')}
                <input value={postTitle} onChange={e => setPostTitle(e.target.value)} />
              </label>
              <label>{text('communitySocial.publishContent')}
                <textarea rows={3} value={postContent} onChange={e => setPostContent(e.target.value)} />
              </label>
              <button type="submit" disabled={busy || !postContent.trim()}>
                {text('communitySocial.publishButton')}
              </button>
            </form>}
          </>}
    </section>

    <section className="setup" aria-labelledby="community-contacts-title">
      <h2 id="community-contacts-title">{text('communityView.contactsTitle')}</h2>
      <p className="muted">{text('communityView.contactsSubtitle')}</p>
      {!desktop
        ? <p className="muted">{text('communityView.contactsBrowser')}</p>
        : <>
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
