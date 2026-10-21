import {useEffect,useRef,useState} from 'react';
import {Client} from '../ipc/client';
import {t} from '../domain/packs';
interface Capacity{kind:'bounded'|'unlimited';tokens?:number}
interface Route{id:string;revision:number;modelId:string;contextTokens:number;verified:boolean}
interface RecordDto{id:string;revision:number;state:string;activeVersionId?:string}
interface Version{id:string;content:string;language:string;contentSha256:string;measurement:{value:number}}
interface Operation{id:string;status:string;physicalCallCount:number;capacityAttemptCount:number;result?:{outcome:string};error?:{messageKey:string}}
interface Attempt{id:string;attemptNumber:number;result:string;capacityPolicySnapshot:Capacity;candidateMeasurement:{value:number}}
interface Decision{id:string;decision:string;reason:string}
interface Proposal{id:string;content:string;language:string;proposerRole:string;sources:{entityId:string;contentSha256:string}[]}
interface View{record:RecordDto;version?:Version;profileRevision:number;capacityPolicy:Capacity;scribeMode:string;scopeId:string;primary?:Route;scribe?:Route;latestOperation?:Operation;attempts:Attempt[];decisions:Decision[];proposals:Proposal[];formatRepairCount:number}
export interface MemoryAttempt{id:string;payload:{recordId:string;sourceRefs:unknown[]};expected:number}
const running=(op?:Operation)=>Boolean(op&&['queued','running'].includes(op.status));
export function MemoryPanel({client,locale,profileId,conversationId,attempts,refreshToken=0,onAttemptsChange,onContextChange}:{client:Client;locale:string;profileId:string;conversationId:string;attempts:Map<string,MemoryAttempt>;refreshToken?:number;onAttemptsChange?:()=>void;onContextChange?:(key:string)=>void}){
 const text=(k:string)=>t(locale,k);const [view,setView]=useState<View>();const [busy,setBusy]=useState(false);const [error,setError]=useState('');
 const [capacity,setCapacity]=useState('300');const [unlimited,setUnlimited]=useState(false);const [mode,setMode]=useState('automatic');const [primary,setPrimary]=useState('');const [scribe,setScribe]=useState('');
 const active=useRef(false);const dirty=useRef(false);const settingsBase=useRef<View|undefined>(undefined);const refreshGeneration=useRef(0);
 function apply(v:View,reset=false){if(!active.current)return;setView(v);onContextChange?.(JSON.stringify([v.record.id,v.record.revision,v.record.state,v.version?.id]));if(reset||!dirty.current){settingsBase.current=v;setCapacity(String(v.capacityPolicy.tokens??300));setUnlimited(v.capacityPolicy.kind==='unlimited');setMode(v.scribeMode);setPrimary(v.primary?.contextTokens?String(v.primary.contextTokens):'');setScribe(v.scribe?.contextTokens?String(v.scribe.contextTokens):'');dirty.current=false;}}
 function safe(e:unknown){return e instanceof Error&&e.message.startsWith('error.')?e.message:'error.transport';}
 async function load(reset=false){const generation=++refreshGeneration.current;const r=await client.call<View>('memory.working.get',{},undefined,profileId);if(!r.ok)throw Error(r.error.messageKey);if(generation===refreshGeneration.current)apply(r.result,reset);return r.result;}
 useEffect(()=>{active.current=true;setView(undefined);void load().catch(e=>{if(active.current)setError(safe(e));});return()=>{active.current=false;++refreshGeneration.current;};},[client,profileId,refreshToken]);
 useEffect(()=>{if(!running(view?.latestOperation))return;let live=true;let timer:ReturnType<typeof setTimeout>;
  async function poll(){try{const v=await load();if(live&&active.current&&running(v.latestOperation))timer=setTimeout(()=>void poll(),500);}catch(e){if(live&&active.current){setError(safe(e));timer=setTimeout(()=>void poll(),1000);}}}
  timer=setTimeout(()=>void poll(),500);return()=>{live=false;clearTimeout(timer);};
 },[client,profileId,view?.latestOperation?.id,view?.latestOperation?.status]);
 async function action(fn:()=>Promise<void>){if(busy)return;setBusy(true);setError('');try{await fn();}catch(e){if(active.current)setError(safe(e));}finally{if(active.current)setBusy(false);}}
 async function save(){const base=settingsBase.current;if(!base)return;const r=await client.call<View>('memory.settings.set',{scopeId:base.scopeId,capacityPolicy:unlimited?{kind:'unlimited'}:{kind:'bounded',tokens:Number(capacity)},scribeMode:mode,primaryContextTokens:Number(primary),...(mode==='automatic'?{scribeContextTokens:Number(scribe)}:{})},base.profileRevision,profileId);
  if(!r.ok){await load();throw Error(r.error.messageKey);}apply(r.result,true);
 }
 async function update(){let pending=attempts.get(profileId);if(!pending){const r=await client.call<{recordId:string;recordRevision:number;sourceRefs:unknown[]}>('memory.working.sources',{conversationId},undefined,profileId);if(!r.ok)throw Error(r.error.messageKey);pending={id:crypto.randomUUID(),payload:{recordId:r.result.recordId,sourceRefs:r.result.sourceRefs},expected:r.result.recordRevision};attempts.set(profileId,pending);onAttemptsChange?.();}
  const r=await client.call<{operation:Operation}>('memory.update.request',pending.payload,pending.expected,profileId,pending.id);
  if(!r.ok){attempts.delete(profileId);onAttemptsChange?.();await load();throw Error(r.error.messageKey);}if(attempts.get(profileId)===pending){attempts.delete(profileId);onAttemptsChange?.();}if(active.current)await load();
 }
 const validWindow=(s:string)=>Number.isSafeInteger(Number(s))&&Number(s)>=1024&&Number(s)<=2000000;
 const unresolved=attempts.has(profileId);const live=running(view?.latestOperation);
 const configured=Boolean(view?.primary?.verified&&view.primary.contextTokens>=1024&&(view.scribeMode==='disabled'||(view.scribe?.verified&&view.scribe.contextTokens>=1024)));
 return <section className="memory-panel" aria-label={text('memory.title')}><h3>{text('memory.title')}</h3><p className="muted">{text('memory.notice')}</p>
 <fieldset disabled={busy||live||unresolved}><legend>{text('memory.title')}</legend><p className="muted">{text('memory.models')}</p>
 <label>{text('memory.capacity')}<input aria-label={text('memory.capacity')} type="number" min={1} list="memory-capacities" value={capacity} disabled={unlimited} onChange={e=>{dirty.current=true;setCapacity(e.target.value);}}/><datalist id="memory-capacities">{[300,500,800,1000].map(n=><option key={n} value={n}/>)}</datalist></label>
 <label><input type="checkbox" checked={unlimited} onChange={e=>{dirty.current=true;setUnlimited(e.target.checked);}}/>{text('memory.unlimited')}</label>{unlimited&&<p role="note">{text('memory.warning')}</p>}<p className="muted">{text('memory.measurement')}</p>
 <label>{text('memory.scribe')}<select aria-label={text('memory.scribe')} value={mode} onChange={e=>{dirty.current=true;setMode(e.target.value);}}><option value="automatic">{text('memory.automatic')}</option><option value="disabled">{text('memory.disabled')}</option></select></label>
 <label>{text('memory.primaryContext')} {view?.primary?.modelId}<input aria-label={text('memory.primaryContext')} type="number" min={1024} max={2000000} value={primary} onChange={e=>{dirty.current=true;setPrimary(e.target.value);}}/></label>
 {mode==='automatic'&&<label>{text('memory.scribeContext')} {view?.scribe?.modelId}<input aria-label={text('memory.scribeContext')} type="number" min={1024} max={2000000} value={scribe} onChange={e=>{dirty.current=true;setScribe(e.target.value);}}/></label>}
 <button disabled={!view||!view.primary?.verified||(mode==='automatic'&&!view.scribe?.verified)||!validWindow(primary)||(mode==='automatic'&&!validWindow(scribe))||(!unlimited&&(!Number.isSafeInteger(Number(capacity))||Number(capacity)<1))} onClick={()=>void action(save)}>{text('memory.save')}</button>
 <button onClick={()=>void action(async()=>{await load(true);})}>{text('memory.reload')}</button></fieldset>
 {unresolved&&<p role="status">{text('memory.unresolved')}</p>}
 <button disabled={busy||!view||(!unresolved&&(live||dirty.current||!configured))} onClick={()=>void action(update)}>{text(unresolved?'memory.resolve':'memory.update')}</button>
 {live&&<button className="stop" disabled={busy} onClick={()=>void action(async()=>{const r=await client.call<Operation>('operation.cancel',{operationId:view!.latestOperation!.id},undefined,profileId);if(!r.ok)throw Error(r.error.messageKey);await load();})}>{text('chat.stop')}</button>}
 <button disabled={busy} onClick={()=>void action(async()=>{await load();})}>{text('memory.refresh')}</button>
 {view?.latestOperation&&<div role="status">{text('chat.operation')}: {text('config.'+view.latestOperation.status)} · {text('config.calls')}: {view.latestOperation.physicalCallCount} · {text('memory.formatRepairs')}: {view.formatRepairCount}{view.latestOperation.result&&<p>{text('memory.'+view.latestOperation.result.outcome)}</p>}{view.latestOperation.error&&<p>{text(view.latestOperation.error.messageKey)}</p>}</div>}
 <ul>{view?.attempts.map(a=><li key={a.id}>{text('memory.attempt')} {a.attemptNumber}/2: {text('memory.'+a.result)} · {a.candidateMeasurement.value} / {a.capacityPolicySnapshot.kind==='unlimited'?text('memory.unlimited'):a.capacityPolicySnapshot.tokens}</li>)}</ul>
 {!!view?.decisions.length&&<div><h4>{text('memory.decisions')}</h4>{view?.decisions.map(d=><p key={d.id}>{text('memory.'+d.decision)} — {d.reason}</p>)}</div>}
 {view?.version?<article><h4>{text('memory.saved')} · {view.version.language} · {text('memory.state.'+view.record.state)}</h4>{view.record.state!=='active'&&<p role="status">{text('memory.excludedNotice')}</p>}<div className="message-text">{view.version.content}</div><small>SHA-256: {view.version.contentSha256} · {view.version.measurement.value}</small></article>:<p>{text('memory.empty')}</p>}
 {!!view?.proposals.length&&<details><summary>{text('memory.proposals')}</summary>{view.proposals.map(p=><article key={p.id}><strong>{p.proposerRole} · {p.language}</strong><div className="message-text">{p.content}</div><ul>{p.sources.map(s=><li key={s.entityId}>{s.entityId} · SHA-256: {s.contentSha256}</li>)}</ul></article>)}</details>}
 {error&&<p role="alert" className="error">{text(error)}</p>}</section>;
}
