import {Client} from '../ipc/client';

export interface SetupProfile {
  id: string;
  displayName: string;
  mode?: string;
  status: string;
  revision: number;
  primaryRouteId?: string;
}
interface Connection {
  id: string;
  revision: number;
  status: string;
  providerKind: string;
  label: string;
  normalizedEndpoint: string;
}
interface Route {
  id: string;
  role: string;
  connectionId: string;
  modelId: string;
}
interface Snapshot {
  profile: SetupProfile;
  connections: Connection[];
  routes: Route[];
}
export interface Model {id: string; label: string}
interface TestOperation {id: string; status: string; expectedRecordRevision?: number; error?: {messageKey: string}}
interface MemorySettings {
  scopeId: string;
  profileRevision: number;
  scribeMode: string;
  capacityPolicy: unknown;
  primary?: {contextTokens: number};
}
export interface SetupDraft {
  id: string;
  name: string;
  provider: string;
  endpoint: string;
  profileId?: string;
  existingConnectionId?: string;
  language: 'en' | 'pl';
}
export interface WorkspaceSelection {profileId: string; conversationId?: string}

export const setupStorageKey = 'ai-home.first-conversation.v1';
export const providers: Record<string, {label: string; starter?: string}> = {
  openrouter: {label: 'OpenRouter', starter: 'openai/gpt-4o-mini'},
  openai: {label: 'OpenAI', starter: 'gpt-4o-mini'},
  google: {label: 'Google', starter: 'gemini-2.5-flash'},
  anthropic: {label: 'Anthropic', starter: 'claude-haiku-4-5-20251001'},
  custom_openai: {label: 'OpenAI-compatible'},
};

export function readSetupDraft(): SetupDraft | undefined {
  try {
    const raw = localStorage.getItem(setupStorageKey);
    if (!raw) return;
    const d = JSON.parse(raw) as SetupDraft;
    if (/^[a-f0-9-]{36}$/.test(d.id) && typeof d.name === 'string' &&
        Object.hasOwn(providers, d.provider) && typeof d.endpoint === 'string' && (d.language === 'en' || d.language === 'pl') &&
        (d.profileId === undefined || typeof d.profileId === 'string') &&
        (d.existingConnectionId === undefined || typeof d.existingConnectionId === 'string')) return d;
  } catch {
    // Invalid or unavailable local UI state does not grant access to a profile.
  }
}

export async function listStandardProfiles(client: Client): Promise<SetupProfile[]> {
  const profiles: SetupProfile[] = [];
  const cursors = new Set<string>();
  let cursor: string | undefined;
  do {
    const r: {items: SetupProfile[]; nextCursor: string} = await call(client, 'profile.list', {
      limit: 100, ...(cursor ? {cursor} : {}),
    });
    profiles.push(...r.items.filter(p => p.mode === 'standard' && p.status !== 'archived'));
    cursor = r.nextCursor;
    if (cursor && cursors.has(cursor)) throw Error('error.transport');
    if (cursor) cursors.add(cursor);
  } while (cursor);
  return profiles;
}

async function call<T>(client: Client, command: string, payload: Record<string, unknown>,
    scope?: string, revision?: number, requestId?: string): Promise<T> {
  const response = await client.call<T>(command, payload, revision, scope, requestId);
  if (!response.ok) throw Error(response.error.messageKey);
  return response.result;
}

export async function prepareExistingSetup(client: Client, profileId: string, locale: string): Promise<SetupDraft> {
  const {profile, connections, routes} = await call<Snapshot>(client, 'profile.snapshot', {}, profileId);
  const connection = connections[0];
  if (profile.mode !== 'standard' || profile.status !== 'draft' || profile.primaryRouteId ||
      connections.length > 1 || (connection && !Object.hasOwn(providers, connection.providerKind)) ||
      routes.some(r => r.role !== 'primary' || r.connectionId !== connection?.id) || routes.length > 1) {
    throw Error('onboarding.existingConfig');
  }
  return {
    id: crypto.randomUUID(), name: profile.displayName, profileId,
    provider: connection?.providerKind ?? 'openrouter',
    endpoint: connection?.providerKind === 'custom_openai' ? connection.normalizedEndpoint : '',
    existingConnectionId: connection?.id,
    language: locale === 'pl' ? 'pl' : 'en',
  };
}

export type SetupStep = 'profile' | 'credential' | 'verify' | 'model' | 'route' | 'memory' | 'conversation';
export async function firstConversation(
  client: Client, draft: SetupDraft, secret: string, selectedModel: string,
  onStep: (step: SetupStep) => void, saveDraft: (draft: SetupDraft) => void,
  signal: AbortSignal,
): Promise<{selection: WorkspaceSelection} | {models: Model[]}> {
  const active = () => signal.throwIfAborted();
  const runCommand = <T>(command: string, payload: Record<string, unknown>, scope?: string,
      revision?: number, requestId?: string) => {
    active();
    return call<T>(client, command, payload, scope, revision, requestId);
  };

  onStep('profile');
  const profileId = draft.profileId ?? (await runCommand<SetupProfile>(
    'profile.create', {displayName: draft.name}, undefined, undefined, draft.id,
  )).id;
  active();
  if (!draft.profileId) {
    draft = {...draft, profileId};
    saveDraft(draft);
  }
  const load = () => runCommand<Snapshot>('profile.snapshot', {}, profileId);
  let snapshot = await load();
  if (snapshot.profile.mode !== 'standard' || snapshot.profile.status === 'archived') throw Error('error.policy_denied');
  const label = providers[draft.provider].label;
  let connection = snapshot.connections.find(c => c.providerKind === draft.provider &&
    (draft.existingConnectionId ? c.id === draft.existingConnectionId : c.label === label));
  if (snapshot.connections.length > 1 || (!connection && (snapshot.connections.length || draft.existingConnectionId))) {
    throw Error('onboarding.existingConfig');
  }

  onStep('credential');
  if (!connection) {
    if (!secret) throw Error('onboarding.keyRequired');
    connection = await runCommand<Connection>('connection.create', {
      connection: {providerKind: draft.provider, endpoint: draft.endpoint, label},
      credential: {secret},
    }, profileId, undefined, draft.id + ':connection');
  } else {
    if (draft.provider === 'custom_openai' && connection.normalizedEndpoint !== draft.endpoint) {
      if (draft.existingConnectionId || snapshot.routes.length) throw Error('onboarding.existingConfig');
      // Credentials are scoped to the normalized endpoint. Require a fresh
      // key before changing it, and bind the key to the updated revision.
      if (!secret) throw Error('onboarding.keyRequired');
      connection = await runCommand<Connection>('connection.update', {
        connectionId: connection.id,
        connection: {providerKind: draft.provider, endpoint: draft.endpoint, label: connection.label},
      }, profileId, connection.revision);
    }
    if (connection.status !== 'verified' && secret) {
      connection = await runCommand<Connection>('connection.credential.set', {
        connectionId: connection.id, credential: {secret},
      }, profileId, connection.revision);
    }
  }
  secret = '';

  onStep('verify');
  const connectionId = connection.id;
  if (connection.status !== 'verified') {
    const latest = await runCommand<{operation?: TestOperation}>('connection.test.latest', {connectionId}, profileId);
    let operation = latest.operation;
    if (!operation || !['queued','running'].includes(operation.status) ||
        operation.expectedRecordRevision !== connection.revision) {
      operation = (await runCommand<{operation: TestOperation}>(
        'connection.test', {connectionId}, profileId, connection.revision,
      )).operation;
    }
    for (let polls = 0; ['queued','running'].includes(operation.status); polls++) {
      if (polls >= 420) throw Error('error.provider_timeout');
      await new Promise(resolve => setTimeout(resolve, 500));
      operation = await runCommand<TestOperation>('operation.get', {operationId: operation.id}, profileId);
    }
    if (operation.status !== 'succeeded') throw Error(operation.error?.messageKey ?? 'error.provider_error');
  }

  onStep('model');
  snapshot = await load();
  const assigned = snapshot.routes.find(r => r.id === snapshot.profile.primaryRouteId);
  const primaryRoutes = snapshot.routes.filter(r => r.role === 'primary');
  if ((assigned && assigned.connectionId !== connectionId) ||
      primaryRoutes.some(r => r.connectionId !== connectionId) || primaryRoutes.length > 1 ||
      snapshot.routes.some(r => r.role === 'scribe')) {
    throw Error('onboarding.existingConfig');
  }
  if (!assigned) {
    const {items} = await runCommand<{items: Model[]}>('connection.models.list', {connectionId}, profileId);
    if (!items.length) throw Error('onboarding.noModels');
    const candidate = primaryRoutes[0]?.modelId ?? (selectedModel || providers[draft.provider].starter);
    const model = items.find(m => m.id === candidate);
    if (!model) return {models: items};

    onStep('route');
    // Discovery currently returns IDs and labels. Keep the starter budget small
    // and leave provider-specific tuning to Settings.
    const route = primaryRoutes[0] ?? await runCommand<Route>('route.save', {
      route: {role: 'primary', connectionId, modelId: model.id, outputLimitTokens: 1024},
    }, profileId, snapshot.profile.revision);
    snapshot = await load();
    if (!snapshot.profile.primaryRouteId) {
      await runCommand('route.assign', {role: 'primary', routeId: route.id}, profileId, snapshot.profile.revision);
    } else if (snapshot.profile.primaryRouteId !== route.id) {
      throw Error('onboarding.existingConfig');
    }
  }

  onStep('route');
  const chat = await runCommand<{routeId: string; routeRevision: number; contextTokens: number}>(
    'chat.settings.get', {}, profileId,
  );
  // Connection discovery gives no model context limits. The starter uses a
  // conservative editable budget and keeps any previously declared budget.
  const contextTokens = chat.contextTokens || 8192;
  if (!chat.contextTokens) {
    await runCommand('chat.settings.set', {routeId: chat.routeId, contextTokens}, profileId, chat.routeRevision);
  }

  onStep('memory');
  const memory = await runCommand<MemorySettings>('memory.working.get', {}, profileId);
  if (memory.scribeMode !== 'disabled' || !memory.primary?.contextTokens) {
    await runCommand('memory.settings.set', {
      scopeId: memory.scopeId,
      capacityPolicy: memory.capacityPolicy,
      scribeMode: 'disabled',
      primaryContextTokens: memory.primary?.contextTokens || contextTokens,
    }, profileId, memory.profileRevision);
  }

  if ((await load()).profile.status !== 'ready') throw Error('error.connection_unverified');
  onStep('conversation');
  const conversation = await runCommand<{id: string}>('conversation.create', {
    title: draft.language === 'pl' ? 'Pierwsza rozmowa' : 'First conversation',
    language: draft.language,
  }, profileId, undefined, draft.id + ':conversation');
  active();
  return {selection: {profileId, conversationId: conversation.id}};
}
