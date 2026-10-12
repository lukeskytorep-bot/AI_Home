"""Generate the P2 contract reference files. Not an application runtime."""
import json
from pathlib import Path
out=Path(__file__).parent/'contracts';out.mkdir(parents=True,exist_ok=True)
defs={}
def ref(n):return {'$ref':'#/$defs/'+n}
def enum(*v):return {'type':'string','enum':list(v)}
def obj(p,optional=()):return {'type':'object','properties':p,'required':[k for k in p if k not in optional],'additionalProperties':False}
def arr(x):return {'type':'array','items':x}
def const(v):return {'const':v}
string={'type':'string'};nonempty={'type':'string','minLength':1};ident={'type':'string','minLength':1,'maxLength':128};positive={'type':'integer','minimum':1};natural={'type':'integer','minimum':0};utc={'type':'string','format':'date-time','pattern':r'^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$'};sha={'type':'string','pattern':'^[a-f0-9]{64}$'};locale={'type':'string','pattern':r'^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$'}
def entity(p,optional=()):return obj({'schemaVersion':const(1),'id':ident,'profileId':ident,'revision':positive,'createdAt':utc,**p},optional)
defs['MemoryCapacityPolicy']={'oneOf':[obj({'kind':const('bounded'),'tokens':positive}),obj({'kind':const('unlimited')})]}
defs['TokenMeasurement']=obj({'value':natural,'method':enum('exact','estimated'),'estimatorId':nonempty,'modelRouteId':ident,'modelRouteRevision':positive})
defs['SourceRef']=obj({'profileId':ident,'kind':enum('message','import','memory_version','work_rule_version','external_snapshot'),'entityId':ident,'contentSha256':sha,'snapshotBlobId':ident},('snapshotBlobId',))
defs['Author']=obj({'kind':enum('human','ai','collaboration','unknown'),'label':string,'assertedBy':enum('importer','source_metadata','application'),'verified':{'type':'boolean'}})
defs['MemorySettings']=obj({k:ref('MemoryCapacityPolicy') for k in ['working','episodic','relationship','longTerm','intent','workRule']})
defs['ContextBudgetPolicy']=obj({'retrievalTargetTokens':positive,'guidanceTargetTokens':positive,'inputReserveTokens':positive,'chatOutputTokens':positive,'memoryOutputCeilingTokens':positive,'maxPhysicalCalls':positive,'maxElapsedMs':positive,'costLimit':{'oneOf':[obj({'kind':const('bounded'),'usd':{'type':'number','exclusiveMinimum':0}}),obj({'kind':const('unconfigured')})]}})
defs['Profile']=obj({'schemaVersion':const(1),'id':ident,'revision':positive,'createdAt':utc,'displayName':nonempty,'status':enum('draft','ready','archived'),'mode':enum('standard','conscious'),'scribeMode':enum('automatic','disabled'),'primaryRouteId':ident,'scribeRouteId':ident,'memorySettings':ref('MemorySettings'),'contextBudget':ref('ContextBudgetPolicy'),'languagePolicy':enum('preserve_source','explicit_translation'),'memoryLanguage':locale},('primaryRouteId','scribeRouteId','memoryLanguage'))
defs['ProviderConnection']=entity({'providerKind':enum('openrouter','openai','google','anthropic','custom_openai'),'normalizedEndpoint':{'type':'string','format':'uri'},'credentialRef':ident,'credentialRevision':positive,'label':nonempty,'status':enum('unverified','verified','error'),'verifiedRevision':positive,'verifiedCredentialRevision':positive,'testedAt':utc},('verifiedRevision','verifiedCredentialRevision','testedAt'))
defs['ConnectionDraft']=obj({'providerKind':enum('openrouter','openai','google','anthropic','custom_openai'),'endpoint':string,'label':nonempty})
defs['CredentialInput']=obj({'secret':nonempty})
defs['ModelRoute']=entity({'role':enum('primary','scribe'),'connectionId':ident,'modelId':nonempty,'outputLimitTokens':positive,'temperature':{'type':'number','minimum':0,'maximum':2},'reasoningValue':nonempty,'capabilitySnapshotId':ident},('temperature','reasoningValue','capabilitySnapshotId'))
defs['Conversation']=entity({'title':string,'language':locale,'status':enum('open','archived'),'activeSegmentId':ident,'draftText':string},('activeSegmentId',))
defs['Message']=entity({'conversationId':ident,'sequence':positive,'role':enum('human','assistant','tool'),'content':string,'language':locale,'status':enum('complete','interrupted','failed'),'sourceRefs':arr(ref('SourceRef')),'operationId':ident},('operationId',))
defs['ContextSegment']=entity({'conversationId':ident,'firstMessageSequence':positive,'lastMessageSequence':positive,'summary':string,'summaryLanguage':locale,'messageDigest':sha,'summarySources':arr(ref('SourceRef')),'state':enum('open','sealed')})
material_base={'fileName':nonempty,'mimeType':nonempty,'originalBlobId':ident,'originalSha256':sha,'originalSizeBytes':positive,'purpose':enum('conversation','knowledge','guidance','memory_import'),'conversationId':ident,'author':ref('Author'),'importer':obj({'kind':const('human'),'localActorId':ident}),'importedAt':utc}
material_options=[]
for media_kind in ['document','image']:
    common={**material_base,'mediaKind':const(media_kind)}
    ready=({'extractedBlobId':ident,'extractedSha256':sha,'importMethod':nonempty} if media_kind=='document' else {'imageMetadata':obj({'format':enum('png','jpeg','webp','gif'),'width':positive,'height':positive})})
    material_options.extend([
        entity({**common,'parseState':const('ready'),**ready},('conversationId',)),
        entity({**common,'parseState':const('pending')},('conversationId',)),
        entity({**common,'parseState':enum('unreadable','failed'),'errorCode':nonempty},('conversationId',))
    ])
defs['ImportedMaterial']={'oneOf':material_options}
defs['Attachment']=entity({'conversationId':ident,'messageId':ident,'materialId':ident,'kind':enum('document','image'),'state':enum('staged','bound','failed')},('messageId',))
defs['KnowledgeSource']=entity({'materialId':ident,'enabled':{'type':'boolean'},'indexRevision':positive,'sourceLanguage':locale},('indexRevision',))
defs['PersistentGuidance']=entity({'materialId':ident,'enabled':{'type':'boolean'},'order':natural,'language':locale,'contentSha256':sha,'tokenMeasurement':ref('TokenMeasurement')})
defs['MemoryKind']=enum('working','episodic','relationship','longTerm','intent','workRule')
defs['MemoryProposal']=entity({'operationId':ident,'memoryRecordId':ident,'memoryKind':ref('MemoryKind'),'baseVersionId':ident,'proposerRole':enum('scribe','primary'),'proposerRouteId':ident,'content':nonempty,'language':locale,'sources':arr(ref('SourceRef')),'state':enum('pending','decided','failed'),'capacityPolicySnapshot':ref('MemoryCapacityPolicy'),'attemptNumber':{'type':'integer','enum':[1,2]}},('baseVersionId',))
defs['PrimaryDecisionOutput']={'oneOf':[obj({'decision':const('accept'),'reason':string}),obj({'decision':const('modify'),'modifiedContent':nonempty,'reason':string}),obj({'decision':const('reject'),'reason':string})]}
defs['MemoryDecisionReceipt']=entity({'operationId':ident,'proposalId':ident,'primaryRouteId':ident,'routeRevision':positive,'decision':enum('accept','modify','reject'),'candidateSha256':sha,'candidateMeasurement':ref('TokenMeasurement'),'reason':string})
defs['MemoryRecord']=entity({'kind':ref('MemoryKind'),'state':enum('empty','active','outdated','archived'),'activeVersionId':ident,'subjectRef':ident},('activeVersionId','subjectRef'))
defs['MemoryVersion']=entity({'recordId':ident,'versionNumber':positive,'content':nonempty,'language':locale,'contentSha256':sha,'measurement':ref('TokenMeasurement'),'sources':arr(ref('SourceRef')),'origin':enum('agent_decision','restore'),'decisionReceiptId':ident,'previousVersionId':ident,'restoredFromVersionId':ident},('previousVersionId','restoredFromVersionId'))
defs['MemoryWriteAttempt']=entity({'operationId':ident,'proposalId':ident,'attemptNumber':{'type':'integer','enum':[1,2]},'capacityPolicySnapshot':ref('MemoryCapacityPolicy'),'candidateSha256':sha,'candidateMeasurement':ref('TokenMeasurement'),'result':enum('capacity_rejected','schema_rejected','primary_rejected','accepted','failed_provider','no_change'),'failureCode':nonempty},('failureCode',))
defs['WorkRule']=entity({'memoryRecordId':ident,'enabled':{'type':'boolean'},'sourceCorrectionRefs':arr(ref('SourceRef'))})
defs['Intent']=entity({'memoryRecordId':ident,'status':enum('open','completed','expired','abandoned'),'trigger':obj({'kind':enum('manual','time','event'),'value':string}),'importance':{'type':'integer','minimum':0,'maximum':100},'expiresAt':utc},('expiresAt',))
defs['Activity']=entity({'operationId':ident,'sequence':positive,'code':nonempty,'status':enum('running','success','error','cancelled','no_action'),'metadata':obj({'attemptNumber':{'type':'integer','enum':[1,2]},'measuredTokens':natural,'capacityTokens':positive,'errorCode':nonempty,'recordId':ident},('attemptNumber','measuredTokens','capacityTokens','errorCode','recordId'))})
defs['ScopedRecordRef']=obj({'profileId':ident,'recordKind':enum('conversation','message','material','attachment','knowledge_source','guidance','memory_record','memory_version','connection','connector_binding'),'recordId':ident})
unique_ids={**arr(ident),'uniqueItems':True}
defs['ChatOperationResult']=obj({'kind':const('chat'),'conversationId':ident,'userMessageId':ident,'assistantMessageId':ident})
defs['MemoryUpdateOperationResult']={'oneOf':[
    obj({'kind':const('memory_update'),'outcome':const('committed'),'recordId':ident,'versionId':ident,'recordRevision':positive}),
    obj({'kind':const('memory_update'),'outcome':const('no_change'),'recordId':ident,'activeVersionId':ident},('activeVersionId',))
]}
defs['ImportOperationResult']=obj({'kind':const('import'),'materialIds':{**unique_ids,'minItems':1},'attachmentIds':unique_ids,'knowledgeSourceIds':unique_ids,'guidanceIds':unique_ids,'proposalIds':unique_ids})
defs['RestoreOperationResult']=obj({'kind':const('restore'),'recordId':ident,'versionId':ident,'restoredFromVersionId':ident,'recordRevision':positive})
defs['ConnectionTestOperationResult']=obj({'kind':const('connection_test'),'connectionId':ident,'connectionRevision':positive,'credentialRevision':positive,'testedAt':utc,'outcome':const('verified')})
defs['ConnectorActionOperationResult']=obj({'kind':const('connector_action'),'bindingId':ident,'action':enum('register_or_bind','read_thread','read_feed','create_thread','reply','recover_identity'),'remoteEntityIds':unique_ids,'feedCursor':string,'feedEpoch':string},('feedCursor','feedEpoch'))
operation_results={'chat':'ChatOperationResult','memory_update':'MemoryUpdateOperationResult','import':'ImportOperationResult','restore':'RestoreOperationResult','connection_test':'ConnectionTestOperationResult','connector_action':'ConnectorActionOperationResult'}
defs['OperationResult']={'oneOf':[ref(n) for n in operation_results.values()]}
operation_options=[]
for kind,result_name in operation_results.items():
    common={'kind':const(kind),'capacityAttemptCount':{'type':'integer','minimum':0,'maximum':2},'physicalCallCount':natural,'expectedRecordRevision':positive,'limits':obj({'maxPhysicalCalls':positive,'maxElapsedMs':positive}),'recordRefs':arr(ref('ScopedRecordRef'))}
    operation_options.extend([
        entity({**common,'status':enum('queued','running')},('expectedRecordRevision',)),
        entity({**common,'status':const('succeeded'),'finishedAt':utc,'result':ref(result_name)},('expectedRecordRevision',)),
        entity({**common,'status':enum('failed','cancelled','interrupted'),'finishedAt':utc,'error':ref('Error')},('expectedRecordRevision',))
    ])
defs['Operation']={'oneOf':operation_options}
defs['ContextManifest']=entity({'operationId':ident,'routeId':ident,'routeRevision':positive,'connectionRevision':positive,'modelContextTokens':positive,'inputTokens':natural,'outputReservationTokens':positive,'safetyReservationTokens':positive,'measurementMethod':enum('exact','estimated'),'estimatorId':nonempty,'includedSources':arr(ref('SourceRef')),'excludedSources':arr(obj({'source':ref('SourceRef'),'reason':enum('disabled','outdated','irrelevant','retrieval_budget','superseded')})),'activeGuidanceIds':arr(ident)})
defs['ConnectorBinding']=entity({'kind':enum('community_4a','a2a','messenger'),'baseUrl':{'type':'string','format':'uri'},'remoteAgentId':ident,'protocolVersion':nonempty,'credentialRef':ident,'operationalKeyRef':ident,'recoveryKeyRef':ident,'capabilities':arr(nonempty),'state':enum('disabled','disconnected','ready','resync_required'),'feedCursor':string,'feedEpoch':string},('remoteAgentId','protocolVersion','credentialRef','operationalKeyRef','recoveryKeyRef','feedCursor','feedEpoch'))
defs['ToolPolicy']=entity({'toolId':nonempty,'effectLevel':{'type':'integer','minimum':0,'maximum':3},'rule':enum('deny','allow','ask'),'maxCalls':natural})
defs['LocalePack']=obj({'schemaVersion':const(1),'packVersion':nonempty,'locale':locale,'displayName':nonempty,'strings':{'type':'object','additionalProperties':{'type':'string'}}})
defs['ThemePack']=obj({'schemaVersion':const(1),'packVersion':nonempty,'id':ident,'displayName':nonempty,'variants':obj({v:obj({k:{'type':'string','pattern':r'^#[a-fA-F0-9]{6}$'} for k in ['background','surface','text','muted','accent','border','error','success']}) for v in ['light','dark']})})
defs['AppPreferences']=obj({'schemaVersion':const(1),'revision':positive,'locale':locale,'themeId':ident,'themeVariant':enum('light','dark','system')})
defs['Error']=obj({'code':nonempty,'messageKey':nonempty,'retryable':{'type':'boolean'},'operationId':ident,'field':string},('operationId','field'))
defs['CommandEnvelope']=obj({'contractVersion':const(1),'requestId':ident,'command':nonempty,'profileId':ident,'expectedRevision':positive,'payload':{'type':'object'}},('profileId','expectedRevision'))
defs['ResultEnvelope']={'oneOf':[obj({'contractVersion':const(1),'requestId':ident,'ok':const(True),'result':{'type':'object'}}),obj({'contractVersion':const(1),'requestId':ident,'ok':const(False),'error':ref('Error')})]}
defs['EventEnvelope']=obj({'contractVersion':const(1),'eventId':ident,'profileId':ident,'operationId':ident,'sequence':positive,'eventType':nonempty,'occurredAt':utc,'payload':{'type':'object'}},('operationId',))
schema={'$schema':'https://json-schema.org/draft/2020-12/schema','$id':'urn:ai-home:p2:contracts:1','title':'AI Home P2 DTO reference','description':'Canonical DTO definitions; relational, lifecycle, authorization and command-specific rules are in the P2 specification and IPC catalog. CredentialInput is transient and never portable.','$defs':defs}
(out/'domain.schema.json').write_text(json.dumps(schema,ensure_ascii=False,indent=2)+'\n')
bounded=lambda n:{'kind':'bounded','tokens':n}
defaults={'contractVersion':1,'appPreferences':{'schemaVersion':1,'revision':1,'locale':'en','themeId':'orbital','themeVariant':'light'},'memorySettings':{k:bounded(n) for k,n in [('working',300),('episodic',300),('relationship',300),('longTerm',500),('intent',300),('workRule',300)]},'contextBudget':{'retrievalTargetTokens':1200,'guidanceTargetTokens':800,'inputReserveTokens':512,'chatOutputTokens':1024,'memoryOutputCeilingTokens':4096,'maxPhysicalCalls':4,'maxElapsedMs':180000,'costLimit':{'kind':'unconfigured'}},'memoryFormatRepairLimit':1,'transportRetriesPerPhysicalStage':1,'capacityProposalLimit':2,'otherLimitPresets':[300,500,800,1000],'importLimits':{'maxBatchFiles':12,'maxDocumentBytes':15728640,'maxExtractedTextBytes':2097152,'maxImageBytes':10485760},'snapshotHistoryRetention':'until_explicit_purge','diagnosticsContentIncludedByDefault':False}
(out/'defaults.json').write_text(json.dumps(defaults,ensure_ascii=False,indent=2)+'\n')
print('Generated',len(defs),'DTO definitions and defaults')
