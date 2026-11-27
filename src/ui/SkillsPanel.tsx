import {useCallback, useEffect, useState} from 'react';
import type {Client} from '../ipc/client';
import {t} from '../domain/packs';
import {listStandardProfiles, type SetupProfile} from './firstConversation';

const ALL_CAPABILITIES = [
  'read_context',
  'draft_only',
  'mcp_read_only',
  'community_read_only',
  'mcp_mutating',
  'community_publish',
] as const;

interface SkillRecord {
  id: string;
  revision: number;
  name: string;
  description: string;
  instructions: string;
  instructionsSha256: string;
  source: 'user' | 'imported';
  requestedCapabilities: string[];
  grantedCapabilities: string[];
  enabled: boolean;
}

export function SkillsPanel({client, locale, desktop, initialProfileId}: {
  client: Client;
  locale: string;
  desktop: boolean;
  initialProfileId?: string;
}) {
  const text = (key: string) => t(locale, key);
  const [profiles, setProfiles] = useState<SetupProfile[]>([]);
  const [profileId, setProfileId] = useState(initialProfileId ?? '');
  const [skills, setSkills] = useState<SkillRecord[]>([]);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [instructions, setInstructions] = useState('');
  const [source, setSource] = useState<'user' | 'imported'>('user');
  const [granted, setGranted] = useState<string[]>([]);
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');

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
        if (active) setError('error.transport');
      }
    })();
    return () => { active = false; };
  }, [client, desktop, initialProfileId]);

  const loadSkills = useCallback(async (pid: string) => {
    if (!desktop || !pid) {
      setSkills([]);
      return;
    }
    try {
      const res = await client.call<{items: SkillRecord[]}>('skills.list', {}, undefined, pid);
      if (res.ok) {
        setSkills(res.result.items);
        setError('');
      } else if (res.error.code !== 'UNKNOWN_COMMAND') {
        setError(res.error.messageKey);
      }
    } catch {
      setError('error.transport');
    }
  }, [client, desktop]);

  useEffect(() => {
    void loadSkills(profileId);
  }, [loadSkills, profileId]);

  function toggleCap(cap: string) {
    setGranted(prev => prev.includes(cap) ? prev.filter(c => c !== cap) : [...prev, cap]);
  }

  async function saveSkill(e: React.FormEvent) {
    e.preventDefault();
    if (!profileId || !name.trim() || !instructions.trim()) return;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const payload: Record<string, unknown> = {
        name: name.trim(),
        description: description.trim(),
        instructions: instructions.trim(),
        source,
      };
      if (source === 'imported') {
        payload.requestedCapabilities = granted;
        payload.grantedCapabilities = [];
        payload.enabled = false;
      } else {
        payload.grantedCapabilities = granted;
        payload.enabled = enabled;
      }
      const res = await client.call<{skill: SkillRecord}>('skills.save', payload, undefined, profileId);
      if (!res.ok) {
        setError(res.error.messageKey);
      } else {
        setName('');
        setDescription('');
        setInstructions('');
        setGranted([]);
        setEnabled(false);
        setNotice(text('skills.saved'));
        await loadSkills(profileId);
      }
    } catch {
      setError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function toggleSkillEnabled(skill: SkillRecord) {
    if (!profileId) return;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const res = await client.call(
        'skills.save',
        {
          skillId: skill.id,
          name: skill.name,
          description: skill.description,
          instructions: skill.instructions,
          source: 'user',
          grantedCapabilities: skill.grantedCapabilities,
          enabled: !skill.enabled,
        },
        undefined,
        profileId,
      );
      if (!res.ok) {
        setError(res.error.messageKey);
      } else {
        setNotice(text('skills.saved'));
        await loadSkills(profileId);
      }
    } catch {
      setError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  async function deleteSkill(skillId: string) {
    if (!profileId) return;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const res = await client.call('skills.delete', {skillId}, undefined, profileId);
      if (!res.ok) {
        setError(res.error.messageKey);
      } else {
        setNotice(text('skills.deleted'));
        await loadSkills(profileId);
      }
    } catch {
      setError('error.transport');
    } finally {
      setBusy(false);
    }
  }

  return <section className="setup skills-panel" aria-labelledby="skills-panel-title">
    <h2 id="skills-panel-title">{text('skills.title')}</h2>
    <p className="muted">{text('skills.intro')}</p>
    {!desktop
      ? <p className="muted">{text('skills.unavailable')}</p>
      : <>
          {profiles.length > 0 && <label>{text('skills.profile')}
            <select aria-label={text('skills.profile')} value={profileId} onChange={e => setProfileId(e.target.value)}>
              {profiles.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}
            </select>
          </label>}

          {error && <p role="alert" className="error">{error.startsWith('error.') ? text(error) : error}</p>}
          {notice && <p role="status" className="muted">{notice}</p>}

          <form className="setup" onSubmit={saveSkill}>
            <label>{text('skills.name')}
              <input value={name} onChange={e => setName(e.target.value)} placeholder="Forum Digest Helper" />
            </label>
            <label>{text('skills.description')}
              <input value={description} onChange={e => setDescription(e.target.value)} />
            </label>
            <label>{text('skills.instructions')}
              <textarea rows={3} value={instructions} onChange={e => setInstructions(e.target.value)} />
            </label>
            <label>
              <select aria-label="Skill source" value={source} onChange={e => {
                const next = e.target.value as 'user' | 'imported';
                setSource(next);
                if (next === 'imported') setEnabled(false);
              }}>
                <option value="user">{text('skills.sourceUser')}</option>
                <option value="imported">{text('skills.sourceImported')}</option>
              </select>
            </label>
            <fieldset>
              <legend>{text('skills.capabilities')}</legend>
              {ALL_CAPABILITIES.map(cap => <label key={cap} className="checkbox-row">
                <input type="checkbox" checked={granted.includes(cap)} onChange={() => toggleCap(cap)} />
                <span><code>{cap}</code></span>
              </label>)}
            </fieldset>
            {source === 'user' && <label className="checkbox-row">
              <input type="checkbox" checked={enabled} onChange={e => setEnabled(e.target.checked)} />
              <span>{text('skills.enableOnSave')}</span>
            </label>}
            <button type="submit" disabled={busy || !profileId || !name.trim() || !instructions.trim()}>
              {text('skills.save')}
            </button>
          </form>

          {skills.length === 0
            ? <p className="muted">{text('skills.empty')}</p>
            : <ul className="autonomous-compact-list">
                {skills.map(sk => <li key={sk.id} className="autonomous-card">
                  <div>
                    <strong>{sk.name}</strong>
                    {' · '}<span>{text(sk.enabled ? 'skills.enabled' : 'skills.disabled')}</span>
                    {' · '}<span><code>{sk.source}</code></span>
                  </div>
                  {sk.description ? <p className="muted">{sk.description}</p> : null}
                  {sk.requestedCapabilities.length > 0 && <p className="muted">
                    {text('skills.requestedLabel')}: {sk.requestedCapabilities.join(', ')}
                  </p>}
                  <p className="muted">
                    {text('skills.grantedLabel')}: {sk.grantedCapabilities.length > 0 ? sk.grantedCapabilities.join(', ') : 'none'}
                  </p>
                  <div className="actions-row">
                    <button type="button" disabled={busy} onClick={() => void toggleSkillEnabled(sk)}>
                      {text(sk.enabled ? 'skills.disable' : 'skills.enable')}
                    </button>
                    <button type="button" disabled={busy} onClick={() => void deleteSkill(sk.id)}>
                      {text('skills.remove')}
                    </button>
                  </div>
                </li>)}
              </ul>}
        </>}
  </section>;
}
