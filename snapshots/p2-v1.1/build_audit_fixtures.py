"""Rebuild the post-audit examples without duplicating original P2 examples."""
import copy,json
from pathlib import Path
p=Path(__file__).parent/'contracts/fixtures.json'
fixtures=[f for f in json.loads(p.read_text()) if not f['name'].startswith('audit/')]
def add(name,dto,value):fixtures.append({'name':'audit/'+name,'dto':dto,'value':value})
base={'schemaVersion':1,'id':'operation-1','profileId':'profile-1','revision':2,'createdAt':'2026-10-11T12:00:00Z'}
finished='2026-10-11T12:01:00Z'
def operation(kind,status,**extra):return {**base,'kind':kind,'status':status,'capacityAttemptCount':0,'physicalCallCount':0,'limits':{'maxPhysicalCalls':4,'maxElapsedMs':180000},'recordRefs':[],**extra}
results={
 'chat':{'kind':'chat','conversationId':'conversation-1','userMessageId':'message-1','assistantMessageId':'message-2'},
 'memory_update':{'kind':'memory_update','outcome':'committed','recordId':'working-1','versionId':'version-2','recordRevision':2},
 'import':{'kind':'import','materialIds':['material-1'],'attachmentIds':[],'knowledgeSourceIds':['knowledge-1'],'guidanceIds':[],'proposalIds':[]},
 'restore':{'kind':'restore','recordId':'working-1','versionId':'version-3','restoredFromVersionId':'version-1','recordRevision':3},
 'connection_test':{'kind':'connection_test','connectionId':'connection-1','connectionRevision':1,'credentialRevision':1,'testedAt':finished,'outcome':'verified'},
 'connector_action':{'kind':'connector_action','bindingId':'binding-1','action':'read_feed','remoteEntityIds':['remote-thread-1'],'feedCursor':'opaque-cursor-1','feedEpoch':'epoch-1'}
}
for kind,result in results.items():add('success-'+kind,'Operation',operation(kind,'succeeded',result=result,finishedAt=finished))
add('memory-no-change','Operation',operation('memory_update','succeeded',result={'kind':'memory_update','outcome':'no_change','recordId':'working-1','activeVersionId':'version-1'},finishedAt=finished))
for status,code in [('failed','PROVIDER_AUTHENTICATION_FAILED'),('cancelled','OPERATION_CANCELLED'),('interrupted','OPERATION_INTERRUPTED')]:
 op=operation('connection_test',status,finishedAt=finished,error={'code':code,'messageKey':'error.'+code.lower(),'retryable':False,'operationId':base['id']})
 op['recordRefs']=[{'profileId':'profile-1','recordKind':'connection','recordId':'connection-1'}]
 add('connection-'+status,'Operation',op)
material=copy.deepcopy(next(f['value'] for f in fixtures if f['dto']=='ImportedMaterial'))
for key in ['extractedBlobId','extractedSha256','importMethod']:material.pop(key)
material.update(id='image-1',mediaKind='image',fileName='photo.png',mimeType='image/png',purpose='conversation',conversationId='conversation-1',imageMetadata={'format':'png','width':640,'height':480})
add('vision-ready-no-ocr','ImportedMaterial',material)
pending=copy.deepcopy(material);pending.pop('imageMetadata');pending['parseState']='pending';add('image-pending','ImportedMaterial',pending)
failed=dict(pending,parseState='failed',errorCode='IMPORT_UNREADABLE');add('image-failed','ImportedMaterial',failed)
measure={'value':287,'method':'estimated','estimatorId':'demo-estimator/1','modelRouteId':'primary-route-1','modelRouteRevision':1}
add('primary-measurement','TokenMeasurement',measure)
add('scribe-measurement-same-revision','TokenMeasurement',dict(measure,modelRouteId='scribe-route-1',value=301))
add('guidance-measurement','PersistentGuidance',{**base,'id':'guidance-1','materialId':'material-1','enabled':True,'order':0,'language':'pl','contentSha256':'c'*64,'tokenMeasurement':measure})
add('version-measurement','MemoryVersion',{**base,'id':'version-2','recordId':'working-1','versionNumber':2,'claimKind':'mixed','content':'Ustalony cel i otwarte zadania.','language':'pl','contentSha256':'d'*64,'measurement':measure,'sources':[],'origin':'agent_decision','decisionReceiptId':'receipt-1'})
add('receipt-measurement','MemoryDecisionReceipt',{**base,'id':'receipt-1','operationId':'operation-1','proposalId':'proposal-1','primaryRouteId':'primary-route-1','routeRevision':1,'decision':'accept','candidateSha256':'d'*64,'candidateMeasurement':measure,'reason':'Supported by the source.'})
add('attempt-measurement','MemoryWriteAttempt',{**base,'id':'attempt-1','operationId':'operation-1','proposalId':'proposal-1','attemptNumber':1,'capacityPolicySnapshot':{'kind':'bounded','tokens':300},'candidateSha256':'d'*64,'candidateMeasurement':measure,'result':'accepted'})
p.write_text(json.dumps(fixtures,ensure_ascii=False,indent=2)+'\n')
print('Fixtures:',len(fixtures))
