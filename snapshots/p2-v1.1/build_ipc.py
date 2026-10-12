"""Generate a typed IPC catalog; no command handlers are implemented here."""
import json
from pathlib import Path
from build_contracts import defs,ref,obj,arr,enum,ident,positive,natural,nonempty,string,const,entity,utc
out=Path(__file__).parent/'contracts'
defs['RelationshipMetadata']=entity({'memoryRecordId':ident,'peerRef':ident,'lastContactAt':utc,'contactCount':natural})
defs['InternalState']=entity({'kind':enum('null','conscious_core'),'engineVersion':nonempty,'stateBlobId':ident},('engineVersion','stateBlobId'))
for name in ['MemoryProposal','MemoryVersion']:
    defs[name]['properties']['claimKind']=enum('fact','preference','inference','rule','mixed')
    defs[name]['required'].append('claimKind')
defs['ProfileDraft']=obj({'displayName':nonempty})
defs['ProfilePatch']=obj({'displayName':nonempty,'memorySettings':ref('MemorySettings'),'contextBudget':ref('ContextBudgetPolicy'),'scribeMode':enum('automatic','disabled'),'languagePolicy':enum('preserve_source','explicit_translation'),'memoryLanguage':nonempty},('displayName','memorySettings','contextBudget','scribeMode','languagePolicy','memoryLanguage'))
defs['ProfilePatch']['minProperties']=1
defs['RouteDraft']=obj({'role':enum('primary','scribe'),'connectionId':ident,'modelId':nonempty,'outputLimitTokens':positive,'temperature':{'type':'number','minimum':0,'maximum':2},'reasoningValue':nonempty},('temperature','reasoningValue'))
defs['PageRequest']=obj({'cursor':nonempty,'limit':{'type':'integer','minimum':1,'maximum':100}},('cursor',))
defs['ImportRequest']=obj({'selectionToken':ident,'purpose':enum('conversation','knowledge','guidance','memory_import'),'conversationId':ident,'author':ref('Author')},('conversationId',))
defs['Accepted']=obj({'operation':ref('Operation')})
defs['Ack']=obj({'revision':positive})
defs['StateSnapshot']=obj({'profile':ref('Profile'),'connections':arr(ref('ProviderConnection')),'routes':arr(ref('ModelRoute')),'memoryRecords':arr(ref('MemoryRecord')),'lastEventSequence':natural})
empty=obj({}); commands={}
def add(name,scope,payload,result,stage,mutation=False,revision=False,async_=False):
    commands[name]={'scope':scope,'payload':payload,'result':result,'firstImplementationStage':stage,'mutates':mutation,'requiresExpectedRevision':revision,'async':async_}
add('app.preferences.get','app',empty,ref('AppPreferences'),'4B-0')
add('app.preferences.set','app',obj({'locale':nonempty,'themeId':ident,'themeVariant':enum('light','dark','system')}),ref('AppPreferences'),'4B-0',True,True)
add('app.pack.validate','app',{'oneOf':[obj({'kind':const('locale'),'pack':ref('LocalePack')}),obj({'kind':const('theme'),'pack':ref('ThemePack')})]},obj({'valid':const(True),'warnings':arr(nonempty)}),'4B-0')
add('profile.create','app',ref('ProfileDraft'),ref('Profile'),'4B-2',True)
add('profile.list','app',ref('PageRequest'),obj({'items':arr(ref('Profile')),'nextCursor':string}),'4B-2')
add('profile.snapshot','profile',empty,ref('StateSnapshot'),'4B-2')
add('profile.update','profile',ref('ProfilePatch'),ref('Profile'),'4B-2',True,True)
add('profile.archive','profile',obj({'archived':{'type':'boolean'}}),ref('Profile'),'4B-6',True,True)
add('connection.create','profile',obj({'connection':ref('ConnectionDraft'),'credential':ref('CredentialInput')}),ref('ProviderConnection'),'4B-1',True)
add('connection.update','profile',obj({'connectionId':ident,'connection':ref('ConnectionDraft')}),ref('ProviderConnection'),'4B-1',True,True)
add('connection.credential.set','profile',obj({'connectionId':ident,'credential':ref('CredentialInput')}),ref('ProviderConnection'),'4B-1',True,True)
add('connection.test','profile',obj({'connectionId':ident}),ref('Accepted'),'4B-1',True,True,True)
add('route.save','profile',obj({'routeId':ident,'route':ref('RouteDraft')},('routeId',)),ref('ModelRoute'),'4B-1',True,True)
add('route.assign','profile',obj({'role':enum('primary','scribe'),'routeId':ident}),ref('Profile'),'4B-1',True,True)
add('conversation.create','profile',obj({'title':string,'language':nonempty}),ref('Conversation'),'4B-2',True)
add('conversation.draft.set','profile',obj({'conversationId':ident,'text':string}),ref('Conversation'),'4B-2',True,True)
add('conversation.messages.list','profile',obj({'conversationId':ident,'page':ref('PageRequest')}),obj({'items':arr(ref('Message')),'nextCursor':string}),'4B-2')
add('chat.send','profile',obj({'conversationId':ident,'content':nonempty,'language':nonempty,'attachmentIds':arr(ident)}),ref('Accepted'),'4B-2',True,True,True)
add('operation.get','profile',obj({'operationId':ident}),ref('Operation'),'4B-1')
add('operation.cancel','profile',obj({'operationId':ident}),ref('Operation'),'4B-1',True)
add('material.import','profile',ref('ImportRequest'),ref('Accepted'),'4B-2A',True,False,True)
add('material.get','profile',obj({'materialId':ident}),ref('ImportedMaterial'),'4B-2A')
add('guidance.enabled.set','profile',obj({'guidanceId':ident,'enabled':{'type':'boolean'}}),ref('PersistentGuidance'),'4B-4',True,True)
add('memory.update.request','profile',obj({'recordId':ident,'sourceRefs':arr(ref('SourceRef'))}),ref('Accepted'),'4B-3',True,True,True)
add('memory.versions.list','profile',obj({'recordId':ident,'page':ref('PageRequest')}),obj({'items':arr(ref('MemoryVersion')),'nextCursor':string}),'4B-4')
add('memory.outdated.set','profile',obj({'recordId':ident,'outdated':{'type':'boolean'}}),ref('MemoryRecord'),'4B-4',True,True)
add('memory.restore','profile',obj({'recordId':ident,'versionId':ident}),ref('Accepted'),'4B-4',True,True,True)
add('activity.list','profile',obj({'afterSequence':natural,'limit':{'type':'integer','minimum':1,'maximum':100}}),obj({'items':arr(ref('Activity')),'lastSequence':natural}),'4B-2')
add('context.preview','profile',obj({'conversationId':ident,'routeId':ident}),ref('ContextManifest'),'4B-5')
add('connector.capabilities.get','profile',obj({'bindingId':ident}),obj({'capabilities':arr(nonempty),'protocolVersion':string}),'4B-12')
# Internal port contracts are explicit and never registered as UI commands.
ports={
 'ProviderGateway':{'input':['profileId','routeId','routeRevision','contextManifestId','operationId','outputSchemaId'],'output':['normalizedResponse','usage','continuationEnvelope'],'effects':'provider network only; no storage mutation'},
 'CommunityConnector':{'operations':['capabilities','registerOrBind','readThread','readFeed','createThread','reply','recoverIdentity'],'input':['profileId','bindingId','operationId','policyGrantId','idempotencyKey'],'output':['remoteResult','cursor','epoch'],'effects':'write requires Executive/Policy grant; wire fixtures frozen at 4B-12'},
 'ConsciousAdapter':{'input':['profileId','engineState','observation'],'output':['WakeCandidate','stateSnapshot'],'effects':'no provider calls, tools, memory writes or external sends'},
 'MemoryStore':{'input':['profileId','recordId','expectedRevision','expectedActiveVersion','validatedDecisionReceipt','policySnapshot'],'output':['newImmutableVersion','newRecordRevision'],'effects':'single transactional commit'},
 'MemoryRetriever':{'input':['profileId','query','budget','excludedStates'],'output':['sourceFragments','measurements','omissions'],'effects':'read only; no policy changes'},
 'ToolGateway':{'input':['profileId','operationId','toolId','policyGrantId','validatedArguments'],'output':['toolResult','activity'],'effects':'all external effects go through Executive/Policy'}
}
events={'operation.changed':ref('Operation'),'activity.appended':ref('Activity'),'message.created':ref('Message'),'memory.changed':ref('MemoryRecord'),'profile.changed':ref('Profile'),'connection.changed':ref('ProviderConnection'),'guidance.changed':ref('PersistentGuidance'),'snapshot.required':obj({'lastAvailableSequence':natural})}
errors=['VALIDATION_ERROR','UNSUPPORTED_CONTRACT_VERSION','UNKNOWN_COMMAND','NOT_FOUND','PROFILE_SCOPE_MISMATCH','REVISION_CONFLICT','IDEMPOTENCY_CONFLICT','CONNECTION_UNVERIFIED','CREDENTIAL_UNAVAILABLE','STALE_CONNECTION_TEST','MODEL_CAPABILITY_UNKNOWN','VISION_UNSUPPORTED','IMPORT_BATCH_TOO_LARGE','IMPORT_TOO_LARGE','IMPORT_UNREADABLE','MEMORY_CAPACITY_EXCEEDED','MEMORY_PRIMARY_REJECTED','MEMORY_FORMAT_INVALID','CONTEXT_BUDGET_EXCEEDED','GUIDANCE_BUDGET_EXCEEDED','OUTPUT_BUDGET_INSUFFICIENT','COST_ESTIMATE_UNKNOWN','OPERATION_BUDGET_EXCEEDED','OPERATION_CANCELLED','OPERATION_INTERRUPTED','PROVIDER_ERROR','POLICY_DENIED','MODE_UNAVAILABLE','EVENT_GAP','PACK_INVALID','MIGRATION_FAILED','STORAGE_ERROR','CONNECTION_INVALID','PROVIDER_OUTPUT_INCOMPLETE','PROVIDER_AUTHENTICATION_FAILED','PROVIDER_TIMEOUT']
defs['Error']['properties']['code']=enum(*errors)
schema=json.loads((out/'domain.schema.json').read_text());schema['$defs']=defs
(out/'domain.schema.json').write_text(json.dumps(schema,ensure_ascii=False,indent=2)+'\n')
(out/'ipc.catalog.json').write_text(json.dumps({'contractVersion':1,'commands':commands,'events':events,'internalPorts':ports,'errors':errors},ensure_ascii=False,indent=2)+'\n')
print(f'Generated {len(defs)} DTO definitions, {len(commands)} commands, {len(events)} event payloads')
