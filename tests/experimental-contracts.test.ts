import {describe,it,expect} from 'vitest';
import {validDto} from '../src/domain/schema';
const sha='a'.repeat(64);
const source={sourceKind:'conversation',sourceId:'conversation-a',sourceRevision:1,sha256:sha,availability:'available'};
describe('Experimental EXP-0 contracts',()=>{
 it('validates the direction-bound instance and durable work records',()=>{
  expect(validDto('ExperimentalInstance',{schemaVersion:1,id:'exp-a',profileId:'p',direction:'experimental',revision:1,runtimeGeneration:1,status:'inactive',eventCursor:0,createdAt:'2026-10-24T12:00:00Z',updatedAt:'2026-10-24T12:00:00Z'})).toBe(true);
  expect(validDto('TechnicalWorkCheckpoint',{schemaVersion:1,id:'t',instanceId:'exp-a',episodeId:'e',revision:1,parentCheckpointId:'',sourceEventSequence:2,status:'active',decisionRefs:[],actionRefs:[],sourceRefs:[source],createdAt:'2026-10-24T12:00:00Z'})).toBe(true);
  expect(validDto('SemanticWorkCheckpoint',{schemaVersion:1,id:'s',instanceId:'exp-a',episodeId:'e',revision:1,parentCheckpointId:'',technicalCheckpointId:'t',proposer:'primary',primaryDisposition:'accepted',sourceEventSequence:2,sourceRefs:[source],content:{focus:'topic',currentIntention:'continue',findings:[],openQuestions:[],nextSuggestedStep:''},createdAt:'2026-10-24T12:00:00Z'})).toBe(true);
 });
 it('does not let technical continuity become a raw-content shadow store',()=>{
  const technical={schemaVersion:1,id:'t',instanceId:'exp-a',episodeId:'e',revision:1,parentCheckpointId:'',sourceEventSequence:2,status:'active',decisionRefs:[],actionRefs:[],sourceRefs:[source],createdAt:'2026-10-24T12:00:00Z',rawConversation:'secret'};
  expect(validDto('TechnicalWorkCheckpoint',technical)).toBe(false);
  const action={schemaVersion:1,id:'a',instanceId:'exp-a',revision:1,decisionId:'d',episodeId:'e',connector:'community',actionKind:'reply',senderIdentity:'agent:x',destination:'thread:1',visibility:'public',payloadHash:sha,payloadRef:'payload:a:v1',idempotencyKey:'i',state:'prepared',receiptRef:'',createdAt:'2026-10-24T12:00:00Z',updatedAt:'2026-10-24T12:00:00Z',content:'secret'};
  expect(validDto('ActionRecord',action)).toBe(false);
 });
 it('preserves explicit deletion provenance instead of copying deleted source content',()=>{
  expect(validDto('ExperimentalSourceRef',{...source,availability:'permanently_deleted'})).toBe(true);
 });
});

it('requires evidence for accepted/reconciled external action states',()=>{
  const base={schemaVersion:1,id:'a2',instanceId:'exp-a',revision:1,decisionId:'d',episodeId:'',connector:'community',actionKind:'reply',senderIdentity:'agent:x',destination:'thread:1',visibility:'public',payloadHash:'d'.repeat(64),payloadRef:'payload:a2:v1',idempotencyKey:'idem',createdAt:'2026-10-24T12:00:00Z',updatedAt:'2026-10-24T12:00:00Z'};
  expect(validDto('ActionRecord',{...base,state:'accepted',receiptRef:''})).toBe(false);
  expect(validDto('ActionRecord',{...base,state:'accepted',receiptRef:'receipt:community:42'})).toBe(true);
  expect(validDto('ActionRecord',{...base,state:'reconciled',receiptRef:'reconcile:42'})).toBe(false);
  expect(validDto('ActionRecord',{...base,state:'reconciled',receiptRef:'reconcile:42',reconciliationOutcome:'confirmed_accepted'})).toBe(true);
  expect(validDto('ActionRecord',{...base,state:'reconciled',receiptRef:'reconcile:43',reconciliationOutcome:'confirmed_not_executed'})).toBe(true);
  expect(validDto('ActionRecord',{...base,state:'reconciled',receiptRef:'reconcile:44',reconciliationOutcome:'still_unknown'})).toBe(true);
});
