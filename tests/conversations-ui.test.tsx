import catalog from '../contracts/ipc.catalog.json';
import extra from '../contracts/conversations.ipc.json';
import {valid,validDto,type JsonSchema} from '../src/domain/schema';
import {it,expect,beforeEach,afterEach} from 'vitest';
import {act} from 'react';import {createRoot,type Root} from 'react-dom/client';
import {Conversations} from '../src/ui/Conversations';import {Client} from '../src/ipc/client';import type {Request,Response,Transport} from '../src/domain/types';
let root:Root,host:HTMLDivElement,calls:Request[],conflict:boolean,ambiguous:boolean,paginated:boolean,records:Record<string,any>,deferred:Promise<void>|undefined,release:(()=>void)|undefined;
const fake:Transport={mode:'desktop',subscribe:async()=>()=>{},dispatch:async<T,>(r:Request):Promise<Response<T>>=>{
 calls.push(r);
 const contract=({...catalog.commands,...extra.commands} as Record<string,{payload:JsonSchema;scope:string;requiresExpectedRevision:boolean}>)[r.command];
 expect(contract,'known command '+r.command).toBeDefined();
 expect(validDto('CommandEnvelope',r),'command envelope '+r.command).toBe(true);
 expect(valid(r.payload,contract.payload),'contract payload '+r.command).toBe(true);
 expect(Boolean(r.profileId),'profile scope '+r.command).toBe(contract.scope==='profile');
 if(contract.requiresExpectedRevision)expect(r.expectedRevision).toBeGreaterThan(0);
 let result:any;switch(r.command){case 'profile.list':result=paginated?(r.payload.cursor?{items:[{id:'p2',displayName:'Two'}],nextCursor:''}:{items:[{id:'p1',displayName:'One'}],nextCursor:'page-2'}):{items:[{id:'p1',displayName:'One'},{id:'p2',displayName:'Two'}],nextCursor:''};break;
 case 'conversation.list':result={items:Object.values(records).filter(c=>c.profileId===r.profileId),nextCursor:''};break;
 case 'conversation.get':result=records[r.payload.conversationId as string];break;
 case 'conversation.create':result={id:'new',profileId:r.profileId,revision:1,title:r.payload.title,language:r.payload.language,draftText:'',status:'open'};records.new=result;break;
 case 'conversation.draft.set':if(deferred)await deferred;if(ambiguous){ambiguous=false;throw Error('PRIVATE RAW');}if(conflict){conflict=false;records.c1={...records.c1,revision:2,draftText:'Remote'};return {contractVersion:1,requestId:r.requestId,ok:false,error:{code:'REVISION_CONFLICT',messageKey:'error.revision_conflict',retryable:false}};}result={...records[r.payload.conversationId as string],revision:r.expectedRevision!+1,draftText:r.payload.text};records[result.id]=result;break;
 default:throw Error('Unexpected '+r.command);
 }return {contractVersion:1,requestId:r.requestId,ok:true,result:result as T};}};
const client=new Client(fake);
beforeEach(()=>{(globalThis as any).IS_REACT_ACT_ENVIRONMENT=true;calls=[];paginated=false;deferred=undefined;release=undefined;conflict=false;ambiguous=false;records={c1:{id:'c1',profileId:'p1',revision:1,title:'First',language:'en',draftText:'Stored',status:'open'},c2:{id:'c2',profileId:'p1',revision:1,title:'Second',language:'pl',draftText:'Other',status:'open'}};host=document.createElement('div');document.body.append(host);root=createRoot(host);});
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
it('creates a conversation without a model or API key',async()=>{await mount();await change('Conversation profile','p1');await change('Conversation title','New topic');await change('Conversation language','pl');await click('Create conversation');expect(calls.find(r=>r.command==='conversation.create')).toMatchObject({profileId:'p1',payload:{title:'New topic',language:'pl'}});expect(control('Message draft').value).toBe('');expect(calls.some(r=>r.command.includes('chat'))).toBe(false);});

it('a delayed successful save preserves newer edits and marks them unsaved',async()=>{await open();await change('Message draft','First edit');deferred=new Promise<void>(resolve=>{release=resolve;});await click('Save draft');await change('Message draft','Newer edit');await act(async()=>{release!();await deferred;});expect(records.c1.draftText).toBe('First edit');expect(control('Message draft').value).toBe('Newer edit');expect(host.textContent).toContain('newer edits still need saving');});

it('profile pages use the frozen P2 payload and load both pages',async()=>{paginated=true;await mount();const pages=calls.filter(r=>r.command==='profile.list');expect(pages.map(r=>r.payload)).toEqual([{limit:100},{limit:100,cursor:'page-2'}]);for(const r of pages)expect(valid(r.payload,catalog.commands['profile.list'].payload as JsonSchema)).toBe(true);expect([...((control('Conversation profile')as HTMLSelectElement).options)].map(o=>o.value)).toEqual(['','p1','p2']);});
