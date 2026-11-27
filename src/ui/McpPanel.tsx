import {useCallback, useEffect, useRef, useState, type FormEvent} from 'react';
import {t} from '../domain/packs';
import type {Client} from '../ipc/client';

interface ProfileSummary {
  id: string;
  displayName: string;
}

interface DiscoveredTool {
  name: string;
  description: string;
  declaredMutating: boolean;
  untrustedExternalMetadata: boolean;
}

interface McpServerView {
  id: string;
  revision: number;
  name: string;
  normalizedEndpoint: string;
  hasCredential: boolean;
  allowMutating: boolean;
  allowedTools: string[];
  enabled: boolean;
  pendingReconciliation?: {toolName: string; idempotencyKey: string; reason: string; recordedAt: string} | null;
  lastTestedAt?: string | null;
  lastServerName?: string | null;
  discoveredTools?: DiscoveredTool[];
}

export function McpPanel({client, locale, desktop, initialProfileId}: {
  client: Client;
  locale: string;
  desktop: boolean;
  initialProfileId?: string;
}) {
  const text = (key: string) => t(locale, key);
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [profileId, setProfileId] = useState(initialProfileId ?? '');
  const [servers, setServers] = useState<McpServerView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');

  const [name, setName] = useState('');
  const [endpoint, setEndpoint] = useState('');
  const [allowMutating, setAllowMutating] = useState(false);
  const apiKeyRef = useRef<HTMLInputElement>(null);

  const loadServers = useCallback(async (pid: string) => {
    if (!desktop || !pid) {
      setServers([]);
      return;
    }
    setBusy(true);
    setError('');
    try {
      const r = await client.call<{items: McpServerView[]}>('mcp.server.list', {}, undefined, pid);
      if (r.ok) {
        setServers(r.result.items);
      } else {
        setError(r.error.messageKey);
      }
    } catch {
      setError('error.transport');
    } finally {
      setBusy(false);
    }
  }, [client, desktop]);

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    (async () => {
      try {
        const [std, auto] = await Promise.all([
          client.call<{items: ProfileSummary[]}>('profile.list', {limit: 50}),
          client.call<{items: Array<{profileId: string; displayName: string}>}>('autonomous.instance.list', {}),
        ]);
        if (!active) return;
        const merged: ProfileSummary[] = [];
        if (std.ok) merged.push(...std.result.items);
        if (auto.ok) {
          for (const a of auto.result.items) {
            if (!merged.some(p => p.id === a.profileId)) {
              merged.push({id: a.profileId, displayName: `${a.displayName} (${text('workspace.autonomous')})`});
            }
          }
        }
        setProfiles(merged);
        const chosen = (initialProfileId && merged.some(p => p.id === initialProfileId))
          ? initialProfileId
          : (merged[0]?.id ?? '');
        setProfileId(chosen);
        if (chosen) await loadServers(chosen);
      } catch {
        if (active) setError('error.transport');
      }
    })();
    return () => { active = false; };
  }, [client, desktop, initialProfileId]); // eslint-disable-line react-hooks/exhaustive-deps

  async function saveServer(e: FormEvent) {
    e.preventDefault();
    if (!profileId || !name.trim() || !endpoint.trim()) return;
    setError('');
    setNotice('');
    const rawKey = apiKeyRef.current?.value.trim() ?? '';
    if (apiKeyRef.current) apiKeyRef.current.value = '';
    try {
      const payload: Record<string, unknown> = {
        name: name.trim(),
        endpoint: endpoint.trim(),
        allowMutating,
        allowedTools: [],
      };
      if (rawKey) payload.apiKey = rawKey;
      const r = await client.call<{server: McpServerView}>('mcp.server.save', payload, undefined, profileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setName('');
      setEndpoint('');
      setAllowMutating(false);
      setNotice(text('mcp.saved'));
      await loadServers(profileId);
    } catch {
      setError('error.transport');
    }
  }

  async function discover(serverId: string) {
    if (!profileId) return;
    setError('');
    setNotice('');
    setBusy(true);
    try {
      const r = await client.call<{capabilities: {serverName: string}; tools: DiscoveredTool[]}>(
        'mcp.server.discover',
        {serverId},
        undefined,
        profileId,
      );
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(`${r.result.capabilities.serverName}: ${r.result.tools.length}`);
      await loadServers(profileId);
    } catch {
      setError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function toggleAllowedTool(server: McpServerView, toolName: string) {
    if (!profileId) return;
    setError('');
    setNotice('');
    const nextAllowed = server.allowedTools.includes(toolName)
      ? server.allowedTools.filter(x => x !== toolName)
      : [...server.allowedTools, toolName];
    try {
      const r = await client.call('mcp.server.save', {
        serverId: server.id,
        name: server.name,
        endpoint: server.normalizedEndpoint,
        allowMutating: server.allowMutating,
        allowedTools: nextAllowed,
      }, undefined, profileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      await loadServers(profileId);
    } catch {
      setError('error.transport');
    }
  }

  async function removeServer(serverId: string) {
    if (!profileId) return;
    setError('');
    setNotice('');
    try {
      const r = await client.call('mcp.server.delete', {serverId}, undefined, profileId);
      if (!r.ok) {
        setError(r.error.messageKey);
        return;
      }
      setNotice(text('mcp.deleted'));
      await loadServers(profileId);
    } catch {
      setError('error.transport');
    }
  }

  return <section className="setup mcp-panel" aria-labelledby="mcp-title">
    <h2 id="mcp-title">{text('mcp.title')}</h2>
    <p className="muted">{text('mcp.intro')}</p>
    {!desktop && <p className="muted">{text('mcp.unavailable')}</p>}
    {desktop && <>
      {profiles.length > 0 && <div className="fields">
        <label>{text('mcp.profile')}
          <select value={profileId} onChange={e => { setProfileId(e.target.value); void loadServers(e.target.value); }}>
            {profiles.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}
          </select>
        </label>
      </div>}

      {error && <div role="alert" className="error">{text(error)}</div>}
      {notice && <div role="status" className="autonomous-status-banner">{notice}</div>}

      <form className="fields" onSubmit={e => void saveServer(e)}>
        <label>{text('mcp.serverName')}
          <input value={name} maxLength={128} onChange={e => setName(e.target.value)} required />
        </label>
        <label>{text('mcp.endpoint')}
          <input type="url" value={endpoint} maxLength={1024} placeholder="https://example.com/mcp" onChange={e => setEndpoint(e.target.value)} required />
        </label>
        <label>{text('mcp.apiKey')}
          <input ref={apiKeyRef} type="password" autoComplete="off" maxLength={8192} />
        </label>
        <label>
          <input type="checkbox" checked={allowMutating} onChange={e => setAllowMutating(e.target.checked)} />
          {' '}{text('mcp.allowMutating')}
        </label>
        <button type="submit" disabled={busy || !profileId || !name.trim() || !endpoint.trim()}>{text('mcp.addServer')}</button>
      </form>

      {servers.length === 0
        ? <p className="muted">{text('mcp.noServers')}</p>
        : <ul className="autonomous-compact-list">
            {servers.map(srv => <li key={srv.id} className="autonomous-card">
              <div>
                <strong>{srv.name}</strong> · <code>{srv.normalizedEndpoint}</code>
                {' · '}{text(srv.allowMutating ? 'mcp.mutatingAllowed' : 'mcp.readOnly')}
                {' · '}{text(srv.hasCredential ? 'mcp.credentialStored' : 'mcp.noCredential')}
              </div>
              <div className="autonomous-actions">
                <button type="button" disabled={busy} onClick={() => void discover(srv.id)}>{text('mcp.discover')}</button>
                {srv.pendingReconciliation && <button type="button" disabled={busy} onClick={async () => {
                  const r = await client.call('mcp.server.save', {
                    serverId: srv.id,
                    name: srv.name,
                    endpoint: srv.normalizedEndpoint,
                    allowMutating: srv.allowMutating,
                    allowedTools: srv.allowedTools,
                    reconcileReceiptRef: `ui-reconciled:${srv.pendingReconciliation?.idempotencyKey}`,
                  }, undefined, profileId);
                  if (r.ok) { setNotice(text('mcp.reconciled')); await loadServers(profileId); }
                  else setError(r.error.messageKey);
                }}>{text('mcp.confirmReconciled')}</button>}
                <button type="button" disabled={busy} onClick={() => void removeServer(srv.id)}>{text('mcp.remove')}</button>
              </div>
              {srv.pendingReconciliation && <p role="alert" className="error">
                {text('mcp.reconciliationRequired')}: <code>{srv.pendingReconciliation.toolName}</code>
              </p>}
              {(srv.discoveredTools?.length ?? 0) > 0 && <ul className="autonomous-compact-list">
                {srv.discoveredTools!.map(tool => {
                  const allowed = srv.allowedTools.includes(tool.name);
                  return <li key={tool.name}>
                    <code>{tool.name}</code>{' '}
                    <small>({text(tool.declaredMutating ? 'mcp.toolMutating' : 'mcp.toolReadOnly')})</small>
                    {tool.description ? ` — ${tool.description}` : ''}{' '}
                    <button type="button" onClick={() => void toggleAllowedTool(srv, tool.name)}>
                      {text(allowed ? 'mcp.revokeTool' : 'mcp.allowTool')}
                    </button>
                  </li>;
                })}
              </ul>}
            </li>)}
          </ul>}
    </>}
  </section>;
}
