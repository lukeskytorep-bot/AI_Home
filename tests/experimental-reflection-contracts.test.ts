import {it,expect} from 'vitest';
import {validDto} from '../src/domain/schema';
it('accepts an explicit empty conclusion and rejects hidden reasoning or publication commands',()=>{
 expect(validDto('ReflectionOutput',{conclusion:'none'})).toBe(true);
 expect(validDto('ReflectionOutput',{conclusion:'none',rawReasoning:'secret'})).toBe(false);
 expect(validDto('ReflectionOutput',{conclusion:'draft',summary:'A hypothesis',candidates:[{kind:'value',content:'Listen carefully'}]})).toBe(true);
 expect(validDto('ReflectionOutput',{conclusion:'draft',summary:'A hypothesis',candidates:[],publish:true})).toBe(false);
});
it('requires a durable run or review receipt for the new decision outcomes',()=>{
 const d={schemaVersion:1,id:'d',instanceId:'i',revision:1,opportunityId:'o',episodeId:'',contextRevision:0,createdAt:'2026-10-26T12:00:00Z'};
 expect(validDto('DecisionRecord',{...d,outcome:'dream',reflectionRunId:'r'})).toBe(true);
 expect(validDto('DecisionRecord',{...d,outcome:'dream'})).toBe(false);
 expect(validDto('DecisionRecord',{...d,outcome:'self_review',reviewId:'r'})).toBe(true);
 expect(validDto('DecisionRecord',{...d,outcome:'self_withdrawal',selfRecordId:'n',reason:'Reconsidered'})).toBe(true);
});
it('does not confuse a synthetic hypothesis with a verified fact',()=>{
 const r={schemaVersion:1,id:'r',instanceId:'i',revision:1,createdAt:'2026-10-26T12:00:00Z',runId:'run',mode:'dream',language:'en',epistemicStatus:'synthetic_hypothesis',summary:'Maybe',candidates:[],contentSha256:'a'.repeat(64),sourceRefs:[]};
 expect(validDto('PrivateReflection',r)).toBe(true);
 expect(validDto('PrivateReflection',{...r,epistemicStatus:'verified_fact'})).toBe(false);
});
