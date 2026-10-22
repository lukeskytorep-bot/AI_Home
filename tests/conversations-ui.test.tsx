import imports from '../contracts/imports.ipc.json';import vision from '../contracts/vision.ipc.json';
import chat from '../contracts/chat.ipc.json';
import catalog from '../contracts/ipc.catalog.json';
import extra from '../contracts/conversations.ipc.json';import lifecycle from '../contracts/lifecycle.ipc.json';
import {valid,validDto,type JsonSchema} from '../src/domain/schema';
import {it,expect,beforeEach,afterEach} from 'vitest';
import {act} from 'react';import {createRoot,type Root} from 'react-dom/client';
import {Conversations} from '../src/ui/Conversations';import {Client} from '../src/ipc/client';import type {Request,Response,Transport} from '../src/domain/types';
let auditArchiveGate:Promise<void>|undefined,auditArchiveRelease:(()=>void)|undefined,auditListGate:Promise<void>|undefined,auditListRelease:(()=>void)|undefined;
let chatEnabled:boolean,historyFails:boolean,sendGate:Promise<void>|undefined,releaseSend:(()=>void)|undefined,lifecycleAmbiguous:boolean,lifecycleReadbackFails:boolean;
let root:Root,host:HTMLDivElement,calls:Request[],conflict:boolean,ambiguous:boolean,paginated:boolean,profileRecords:Record<string,any>,records:Record<string,any>,deferred:Promise<void>|undefined,release:(()=>void)|undefined;
const fake:Transport={mode:'desktop',subscribe:async()=>()=>{},dispatch:async<T,>(r:Request):Promise<Response<T>>=>{
 calls.push(r);
 const contract=({...catalog.commands,...extra.commands,...chat.commands,...imports.commands,...vision.commands,...lifecycle.commands} as Record<string,{payload:JsonSchema;scope:string;requiresExpectedRevision:boolean}>)[r.command];
 expect(contract,'known command '+r.command).toBeDefined();
 expect(validDto('CommandEnvelope',r),'command envelope '+r.command).toBe(true);
 expect(valid(r.payload,contract.payload),'contract payload '+r.command).toBe(true);
 expect(Boolean(r.profileId),'profile scope '+r.command).toBe(contract.scope==='profile');
 if(contract.requiresExpectedRevision)expect(r.expectedRevision).toBeGreaterThan(0);
 let result:any;switch(r.command){case 'profile.list':{const active=Object.values(profileRecords).filter((p:any)=>p.status!=='archived');result=paginated?(r.payload.cursor?{items:[active[1]],nextCursor:''}:{items:[active[0]],nextCursor:'page-2'}):{items:active,nextCursor:''};break;}
 case 'chat.vision.get':result={routeId:'r1',routeRevision:1,modelId:'mock-model',policyRevision:0,scopeId:'a'.repeat(64),support:'unknown',source:'unconfigured'};break;case 'chat.settings.get':result={routeId:'r1',routeRevision:1,modelId:'model',contextTokens:chatEnabled?32768:0,source:'user_declared',measurement:'utf8_bytes_estimate',outputTokens:1024,inputReserveTokens:512};break;
 case 'conversation.chat.latest':case 'conversation.import.latest':case 'profile.import.latest':result={found:false};break;
 case 'profile.archived.list':if(lifecycleReadbackFails){lifecycleReadbackFails=false;throw Error('READBACK_FAIL');}result={items:Object.values(profileRecords).filter((p:any)=>p.status==='archived'),nextCursor:''};break;
 case 'conversation.archived.list':if(auditListGate&&r.profileId==='p1'){const captured=Object.values(records).filter((c:any)=>c.profileId===r.profileId&&c.status==='archived');await auditListGate;result={items:captured,nextCursor:''};break;}if(lifecycleReadbackFails){lifecycleReadbackFails=false;throw Error('READBACK_FAIL');}result={items:Object.values(records).filter((c:any)=>c.profileId===r.profileId&&c.status==='archived'),nextCursor:''};break;
 case 'profile.archive':{const p=profileRecords[r.profileId!];result={...p,revision:r.expectedRevision!+1,status:r.payload.archived?'archived':'draft'};profileRecords[r.profileId!]=result;if(lifecycleAmbiguous){lifecycleAmbiguous=false;throw Error('AMBIGUOUS');}break;}
 case 'conversation.archive':{if(auditArchiveGate)await auditArchiveGate;const c=records[r.payload.conversationId as string];result={...c,revision:r.expectedRevision!+1,status:r.payload.archived?'archived':'open'};records[result.id]=result;if(lifecycleAmbiguous){lifecycleAmbiguous=false;throw Error('AMBIGUOUS');}break;}
 case 'conversation.materials.list':result={items:[],nextAfterId:''};break;
 case 'conversation.messages.list':if(historyFails)throw Error('History unavailable');result={items:[],nextCursor:''};break;
 case 'chat.send':if(sendGate)await sendGate;records[r.payload.conversationId as string]={...records[r.payload.conversationId as string],revision:r.expectedRevision!+1,draftText:''};result={operation:{id:'op1',revision:1,status:'queued',physicalCallCount:0}};break;
 case 'conversation.list':result={items:Object.values(records).filter(c=>c.profileId===r.profileId&&c.status!=='archived'),nextCursor:''};break;
 case 'conversation.get':result=records[r.payload.conversationId as string];break;
 case 'conversation.create':result={id:'new',profileId:r.profileId,revision:1,title:r.payload.title,language:r.payload.language,draftText:'',status:'open'};records.new=result;break;
 case 'conversation.draft.set':if(deferred)await deferred;if(ambiguous){ambiguous=false;throw Error('PRIVATE RAW');}if(conflict){conflict=false;records.c1={...records.c1,revision:2,draftText:'Remote'};return {contractVersion:1,requestId:r.requestId,ok:false,error:{code:'REVISION_CONFLICT',messageKey:'error.revision_conflict',retryable:false}};}result={...records[r.payload.conversationId as string],revision:r.expectedRevision!+1,draftText:r.payload.text};records[result.id]=result;break;
 default:throw Error('Unexpected '+r.command);
 }return {contractVersion:1,requestId:r.requestId,ok:true,result:result as T};}};
const client=new Client(fake);
beforeEach(()=>{(globalThis as any).IS_REACT_ACT_ENVIRONMENT=true;calls=[];auditArchiveGate=undefined;auditArchiveRelease=undefined;auditListGate=undefined;auditListRelease=undefined;chatEnabled=false;historyFails=false;lifecycleAmbiguous=false;lifecycleReadbackFails=false;sendGate=undefined;releaseSend=undefined;paginated=false;deferred=undefined;release=undefined;conflict=false;ambiguous=false;profileRecords={p1:{id:'p1',revision:1,displayName:'One',status:'draft'},p2:{id:'p2',revision:1,displayName:'Two',status:'draft'}};records={c1:{id:'c1',profileId:'p1',revision:1,title:'First',language:'en',draftText:'Stored',status:'open'},c2:{id:'c2',profileId:'p1',revision:1,title:'Second',language:'pl',draftText:'Other',status:'open'}};host=document.createElement('div');document.body.append(host);root=createRoot(host);});
afterEach(async()=>{await act(async()=>root.unmount());host.remove();});
async function mount(locale='en',desktop=true){await act(async()=>root.render(<Conversations client={client} locale={locale} desktop={desktop}/>));}
function control(name:string){return host.querySelector(`[aria-label="${name}"]`) as HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement;}
async function change(name:string,value:string){const el=control(name);const proto=el.tagName==='SELECT'?HTMLSelectElement.prototype:el.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value')!.set!.call(el,value);await act(async()=>el.dispatchEvent(new Event(el.tagName==='SELECT'?'change':'input',{bubbles:true})));}
async function click(name:string){const b=[...host.querySelectorAll('button')].find(b=>b.textContent===name);expect(b).toBeDefined();await act(async()=>b!.click());}
async function open(){await mount();await change('Conversation profile','p1');await change('Conversation','c1');}
it('browser preview does not call native storage',async()=>{await mount('en',false);expect(calls).toEqual([]);expect(host.textContent).toContain('requires the desktop app');});
it('keeps unsaved drafts separate when changing conversations',async()=>{await open();await change('Message draft','Local one');await change('Conversation','c2');expect(control('Message draft').value).toBe('Other');await change('Message draft','Local two');await change('Conversation','c1');expect(control('Message draft').value).toBe('Local one');});
it('language change and refresh preserve dirty text',async()=>{await open();await change('Message draft','Keep me');await mount('pl');expect(control('Szkic wiadomości').value).toBe('Keep me');await click('Odśwież rozmowę');expect(control('Szkic wiadomości').value).toBe('Keep me');});
it('save uses conversation scope and revision; send remains unavailable',async()=>{await open();await change('Message draft','Żółć 🪐');await click('Save draft');expect(calls.find(r=>r.command==='conversation.draft.set')).toMatchObject({profileId:'p1',expectedRevision:1,payload:{conversationId:'c1',text:'Żółć 🪐'}});expect(records.c1.draftText).toBe('Żółć 🪐');expect([...host.querySelectorAll('button')].find(b=>b.textContent==='Send')!.disabled).toBe(true);});
it('conflict updates revision and keeps local text for retry',async()=>{await open();await change('Message draft','Local');conflict=true;await click('Save draft');expect(control('Message draft').value).toBe('Local');await click('Save draft');expect(calls.filter(r=>r.command==='conversation.draft.set').map(r=>r.expectedRevision)).toEqual([1,2]);expect(records.c1.draftText).toBe('Local');});
it('ambiguous failure reuses request ID and never reveals raw error',async()=>{await open();ambiguous=true;await click('Save draft');expect(host.textContent).not.toContain('PRIVATE RAW');await click('Save draft');const a=calls.filter(r=>r.command==='conversation.draft.set');expect(a).toHaveLength(2);expect(a[0].requestId).toBe(a[1].requestId);});
it('profile switching hides foreign conversations and restores owned draft',async()=>{await open();await change('Message draft','Private');await change('Conversation profile','p2');expect(host.textContent).not.toContain('First');expect(host.querySelector('textarea')).toBeNull();await change('Conversation profile','p1');await change('Conversation','c1');expect(control('Message draft').value).toBe('Private');});
it('creates a conversation without a model or API key',async()=>{await mount();await change('Conversation profile','p1');await change('Conversation title','New topic');await change('Conversation language','pl');await click('Create conversation');expect(calls.find(r=>r.command==='conversation.create')).toMatchObject({profileId:'p1',payload:{title:'New topic',language:'pl'}});expect(control('Message draft').value).toBe('');expect(calls.some(r=>r.command==='chat.send')).toBe(false);});

it('a delayed successful save preserves newer edits and marks them unsaved',async()=>{await open();await change('Message draft','First edit');deferred=new Promise<void>(resolve=>{release=resolve;});await click('Save draft');await change('Message draft','Newer edit');await act(async()=>{release!();await deferred;});expect(records.c1.draftText).toBe('First edit');expect(control('Message draft').value).toBe('Newer edit');expect(host.textContent).toContain('newer edits still need saving');});

it('profile pages use the frozen P2 payload and load both pages',async()=>{paginated=true;await mount();const pages=calls.filter(r=>r.command==='profile.list');expect(pages.map(r=>r.payload)).toEqual([{limit:100},{limit:100,cursor:'page-2'}]);for(const r of pages)expect(valid(r.payload,catalog.commands['profile.list'].payload as JsonSchema)).toBe(true);expect([...((control('Conversation profile')as HTMLSelectElement).options)].map(o=>o.value)).toEqual(['','p1','p2']);});

it('acknowledges Accepted after switching conversations without reviving the sent draft',async()=>{chatEnabled=true;await open();await change('Message draft','Sent once');sendGate=new Promise<void>(resolve=>{releaseSend=resolve;});await click('Send');await change('Conversation','c2');await act(async()=>{releaseSend!();await sendGate;});expect(control('Message draft').value).toBe('Other');await change('Conversation','c1');expect(control('Message draft').value).toBe('');expect(calls.filter(r=>r.command==='chat.send')).toHaveLength(1);});
it('clears an accepted draft even if history fails, including after manual refresh',async()=>{chatEnabled=true;await open();await change('Message draft','Sent once');historyFails=true;await click('Send');expect(control('Message draft').value).toBe('');historyFails=false;await click('Refresh chat');expect(control('Message draft').value).toBe('');await click('Send');expect(calls.filter(r=>r.command==='chat.send')).toHaveLength(1);});
it('Accepted keeps newer text typed while sending across a conversation switch',async()=>{chatEnabled=true;await open();await change('Message draft','Sent once');sendGate=new Promise<void>(resolve=>{releaseSend=resolve;});await click('Send');await change('Message draft','New text');await change('Conversation','c2');await act(async()=>{releaseSend!();await sendGate;});await change('Conversation','c1');expect(control('Message draft').value).toBe('New text');});

it('archives and restores a conversation without deleting its draft',async()=>{await open();await click('Archive conversation');expect(records.c1.status).toBe('archived');expect(records.c1.draftText).toBe('Stored');expect(calls.some(r=>r.command==='conversation.archive'&&r.payload.archived===true)).toBe(true);expect(host.textContent).toContain('First');await click('Restore conversation');expect(records.c1.status).toBe('open');await change('Conversation','c1');expect(control('Message draft').value).toBe('Stored');});
it('archives a profile out of the active selector and restores it from Archive',async()=>{await open();await click('Archive profile');expect(profileRecords.p1.status).toBe('archived');expect((control('Conversation profile') as HTMLSelectElement).value).toBe('');expect([...((control('Conversation profile')as HTMLSelectElement).options)].map(o=>o.value)).not.toContain('p1');await click('Restore profile');expect(profileRecords.p1.status).toBe('draft');expect(calls.some(r=>r.command==='profile.archive'&&r.profileId==='p1'&&r.payload.archived===false)).toBe(true);});

it('lifecycle retry reuses requestId after an ambiguous transport result',async()=>{await open();lifecycleAmbiguous=true;await click('Archive conversation');await click('Archive conversation');const a=calls.filter(r=>r.command==='conversation.archive'&&r.payload.archived===true);expect(a).toHaveLength(2);expect(a[0].requestId).toBe(a[1].requestId);});
it('accepted restore updates parent before archive-list readback failure',async()=>{records.c1={...records.c1,status:'archived',revision:2};await mount();await change('Conversation profile','p1');lifecycleReadbackFails=true;await click('Restore conversation');expect(records.c1.status).toBe('open');const options=[...((control('Conversation') as HTMLSelectElement).options)].map(o=>o.textContent);expect(options).toContain('First');const lists=calls.filter(r=>r.command==='conversation.list');expect(lists.length).toBeGreaterThan(0);});


it('stale archived list does not leak a previous profile after switching',async()=>{
 records.old={id:'old',profileId:'p1',revision:1,title:'PRIVATE_ARCHIVE_P1',language:'en',draftText:'',status:'archived'};
 auditListGate=new Promise<void>(resolve=>auditListRelease=resolve);
 await open();await change('Conversation profile','p2');
 await act(async()=>{auditListRelease!();await auditListGate;});auditListGate=undefined;
 expect((control('Conversation profile') as HTMLSelectElement).value).toBe('p2');
 expect(host.textContent).not.toContain('PRIVATE_ARCHIVE_P1');
});

it('late archive does not replace the active conversation of another profile',async()=>{
 records.c3={id:'c3',profileId:'p2',revision:1,title:'P2_CURRENT',language:'en',draftText:'p2 draft',status:'open'};
 await open();auditArchiveGate=new Promise<void>(resolve=>auditArchiveRelease=resolve);
 await click('Archive conversation');await change('Conversation profile','p2');await change('Conversation','c3');
 await act(async()=>{auditArchiveRelease!();await auditArchiveGate;});auditArchiveGate=undefined;
 expect((control('Conversation profile') as HTMLSelectElement).value).toBe('p2');
 expect((control('Conversation') as HTMLSelectElement).value).toBe('c3');
 expect(host.textContent).not.toContain('Second');
});
