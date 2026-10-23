import {afterEach,expect,it} from 'vitest';
import {act} from 'react';
import {createRoot,type Root} from 'react-dom/client';
import {LifecyclePanel,type LifecycleWrite} from '../src/ui/LifecyclePanel';
import {Client} from '../src/ipc/client';
import type {Request,Response,Transport} from '../src/domain/types';
let root:Root|undefined,host:HTMLDivElement|undefined,calls:Request[]=[];
afterEach(async()=>{if(root)await act(async()=>root!.unmount());host?.remove();root=undefined;host=undefined;calls=[];});
const archivedProfile={id:'p-old',revision:3,displayName:'Old',status:'archived'};
const activeProfile={id:'p1',revision:1,displayName:'One',status:'draft'};
const archivedConversation={id:'c-old',profileId:'p1',revision:4,title:'Old chat',language:'en',draftText:'',status:'archived'};
function transport():Transport{return{mode:'desktop',subscribe:async()=>()=>{},dispatch:async<T,>(r:Request):Promise<Response<T>>=>{calls.push(r);let result:any;if(r.command==='profile.archived.list')result={items:[archivedProfile],nextCursor:''};else if(r.command==='conversation.archived.list')result={items:[archivedConversation],nextCursor:''};else if(r.command==='profile.delete.permanent')result={deleted:true,profileId:r.profileId};else if(r.command==='conversation.delete.permanent')result={deleted:true,profileId:r.profileId,conversationId:r.payload.conversationId};else throw Error(r.command);return{contractVersion:1,requestId:r.requestId,ok:true,result};}};}
async function mount(){(globalThis as any).IS_REACT_ACT_ENVIRONMENT=true;host=document.createElement('div');document.body.append(host);root=createRoot(host);await act(async()=>root!.render(<LifecyclePanel client={new Client(transport())} locale="en" profile={activeProfile} writes={new Map<string,LifecycleWrite>()} onProfileArchived={()=>{}} onProfileRestored={()=>{}} onConversationArchived={()=>{}} onConversationRestored={()=>{}}/>));}
async function click(label:string){const b=[...host!.querySelectorAll('button')].find(x=>x.textContent===label);expect(b).toBeDefined();await act(async()=>b!.click());}
it('requires a second explicit confirmation before permanent profile deletion',async()=>{await mount();await click('Delete permanently');expect(calls.some(r=>r.command==='profile.delete.permanent')).toBe(false);await click('Confirm permanent delete');const d=calls.find(r=>r.command==='profile.delete.permanent');expect(d).toMatchObject({profileId:'p-old',expectedRevision:3,payload:{confirm:true}});});
it('permanently deletes only an archived conversation in the current profile',async()=>{await mount();const buttons=[...host!.querySelectorAll('button')].filter(x=>x.textContent==='Delete permanently');expect(buttons.length).toBe(2);await act(async()=>buttons[1].click());await click('Confirm permanent delete');const d=calls.find(r=>r.command==='conversation.delete.permanent');expect(d).toMatchObject({profileId:'p1',expectedRevision:4,payload:{conversationId:'c-old',confirm:true}});});
