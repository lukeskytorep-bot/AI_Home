import catalog from '../../contracts/ipc.catalog.json';import foundation from '../../contracts/foundation.ipc.json';import defaults from '../../contracts/defaults.json';
import type {AppEvent,Transport,Preferences,Request,Response} from '../domain/types';import {valid,validDto,type JsonSchema} from '../domain/schema';import {locales,themes,validatePack} from '../domain/packs';
const KEY='ai-home.foundation.preview.v1';
interface State {version:1;preferences:Preferences;sequence:number;events:AppEvent[];receipts:Record<string,{fingerprint:string;response:Response}>}
export function browserTransport(storage:Storage=localStorage):Transport {
 const listeners=new Set<(e:AppEvent)=>void>();
 const fail=(r:Request,code:string):Response=>({contractVersion:1,requestId:r.requestId||'invalid-request',ok:false,error:{code,messageKey:'error.'+code.toLowerCase(),retryable:false}});
 function load():State{
  const raw=storage.getItem(KEY);if(raw===null)return {version:1,preferences:structuredClone(defaults.appPreferences) as Preferences,sequence:0,events:[],receipts:{}};
  const s=JSON.parse(raw) as State;
  if(s.version!==1||!validDto('AppPreferences',s.preferences)||!Number.isSafeInteger(s.sequence)||s.sequence<0||!Array.isArray(s.events)||!s.receipts||typeof s.receipts!=='object'||Array.isArray(s.receipts)||s.events.some(e=>!validDto('AppEvent',e)))throw Error('STORAGE_ERROR');
  if(s.events.length!==Math.min(s.sequence,100)||s.events.some((e,i)=>e.sequence!==s.sequence-s.events.length+i+1)||Object.entries(s.receipts).some(([id,r])=>typeof r.fingerprint!=='string'||!r.response||r.response.requestId!==id||!r.response.ok||!validDto('AppPreferences',r.response.result)))throw Error('STORAGE_ERROR');
  if(!locales.some(p=>p.locale===s.preferences.locale)||!themes.some(p=>p.id===s.preferences.themeId))throw Error('STORAGE_ERROR');
  return s;
 }
 return {mode:'browser',subscribe:async handler=>{listeners.add(handler);return ()=>{listeners.delete(handler);};},async dispatch<T>(r:Request):Promise<Response<T>>{
  if(r.contractVersion!==1)return fail(r,'UNSUPPORTED_CONTRACT_VERSION') as Response<T>;
  if(!validDto('CommandEnvelope',r)||r.profileId!==undefined)return fail(r,'VALIDATION_ERROR') as Response<T>;
  const supported=['app.preferences.get','app.preferences.set','app.pack.validate','kernel.status.get','app.events.list'];
  if(!supported.includes(r.command))return fail(r,'UNKNOWN_COMMAND') as Response<T>;
  const c={...catalog.commands,...foundation.commands}[r.command as keyof typeof catalog.commands] as {payload:JsonSchema};
  if(!valid(r.payload,c.payload))return fail(r,'VALIDATION_ERROR') as Response<T>;
  try{
   const s=load();let result:unknown;
   if(r.command==='app.preferences.get')result=s.preferences;
   else if(r.command==='kernel.status.get')result={schemaVersion:1,lifecycle:'ready',mode:'standard',stateProvider:'null',stage:'4B-0',services:['preferences','packs','events'],lastEventSequence:s.sequence};
   else if(r.command==='app.events.list'){
    const after=r.payload.afterSequence as number;
    if(s.events.length&&after<s.events[0].sequence-1)return fail(r,'EVENT_GAP') as Response<T>;
    const items=s.events.filter(e=>e.sequence>after).slice(0,r.payload.limit as number);result={items,lastSequence:s.sequence,hasMore:items.length>0&&items.at(-1)!.sequence<s.sequence};
   }else if(r.command==='app.pack.validate'){
    const kind=r.payload.kind as 'theme'|'locale';try{result={valid:true,warnings:validatePack(kind,r.payload.pack)};}catch{return fail(r,'PACK_INVALID') as Response<T>;}
   }else{
    if(r.expectedRevision===undefined)return fail(r,'VALIDATION_ERROR') as Response<T>;
    const fingerprint=JSON.stringify([r.command,r.expectedRevision,r.payload.locale,r.payload.themeId,r.payload.themeVariant]);
    const receipt=Object.hasOwn(s.receipts,r.requestId)?s.receipts[r.requestId]:undefined;if(receipt)return (receipt.fingerprint===fingerprint?receipt.response:fail(r,'IDEMPOTENCY_CONFLICT')) as Response<T>;
    if(!locales.some(p=>p.locale===r.payload.locale)||!themes.some(p=>p.id===r.payload.themeId))return fail(r,'VALIDATION_ERROR') as Response<T>;
    if(s.preferences.revision!==r.expectedRevision)return fail(r,'REVISION_CONFLICT') as Response<T>;
    const preferences={schemaVersion:1,revision:s.preferences.revision+1,...r.payload} as Preferences;
    const event:AppEvent={contractVersion:1,eventId:crypto.randomUUID(),sequence:s.sequence+1,eventType:'app.preferences.changed',occurredAt:new Date().toISOString(),payload:preferences};
    const response:Response={contractVersion:1,requestId:r.requestId,ok:true,result:preferences};
    s.preferences=preferences;s.sequence=event.sequence;s.events=[...s.events,event].slice(-100);Object.defineProperty(s.receipts,r.requestId,{value:{fingerprint,response},enumerable:true,writable:true,configurable:true});
    storage.setItem(KEY,JSON.stringify(s));for(const listener of listeners){try{listener(structuredClone(event));}catch{/* Storage committed; a failed subscriber must resync. */}}
    return response as Response<T>;
   }
   return {contractVersion:1,requestId:r.requestId,ok:true,result:structuredClone(result) as T};
  }catch{return fail(r,'STORAGE_ERROR') as Response<T>;}
 }};
}
