import type {WorkspaceSelection} from './firstConversation';
import {MemoryLayersPanel,type LibraryWrite} from './MemoryLayersPanel';
import {MemoryPanel,type MemoryAttempt} from './MemoryPanel';
import {AttachmentsPanel,type CandidatePending,type ImportOperation} from './AttachmentsPanel';
import {ChatPanel,type ChatAttempt} from './ChatPanel';
import {LifecyclePanel,type LifecycleWrite} from './LifecyclePanel';
import {useEffect,useRef,useState} from 'react';
import {Client} from '../ipc/client';
import {t} from '../domain/packs';
interface Conversation {id:string;profileId:string;revision:number;title:string;language:string;draftText:string;status:string}
interface Profile {id:string;revision:number;displayName:string;status:string;mode?:string}
interface Page<T>{items:T[];nextCursor:string}
interface Draft {text:string;dirty:boolean;edit:number}
export function Conversations({client,locale,desktop,initialSelection,workspaceMode=false,onProfileChange,onFinishSetup,refreshToken=0}:{client:Client;locale:string;desktop:boolean;initialSelection?:WorkspaceSelection;workspaceMode?:boolean;refreshToken?:number;onProfileChange?:(id:string)=>void;onFinishSetup?:(id:string)=>Promise<void>}){
 const opening=useRef(initialSelection);
 const [showLayers,setShowLayers]=useState(false);
 const [memoryContextKey,setMemoryContextKey]=useState('');const [profileContextRevision,setProfileContextRevision]=useState(0);
 const text=(k:string)=>t(locale,k);const [profiles,setProfiles]=useState<Profile[]>([]);const [profile,setProfile]=useState(initialSelection?.profileId??'');const [list,setList]=useState<Conversation[]>([]);const [cursor,setCursor]=useState('');const [id,setId]=useState('');const [record,setRecord]=useState<Conversation>();const [draft,setDraft]=useState('');const [title,setTitle]=useState('');const [language,setLanguage]=useState('en');const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');
 const documentSelections=useRef(new Map<string,string[]>());const [sourcesVersion,setSourcesVersion]=useState(0);
 const candidateOperations=useRef(new Map<string,ImportOperation>());const candidatePendings=useRef(new Map<string,CandidatePending>());const [,setCandidateTrackerVersion]=useState(0);
 const [memoryAttemptVersion,setMemoryAttemptVersion]=useState(0);
 const libraryWrites=useRef(new Map<string,LibraryWrite>());
 const memoryAttempts=useRef(new Map<string,MemoryAttempt>());
 const chatAttempts=useRef(new Map<string,ChatAttempt>());
 const lifecycleWrites=useRef(new Map<string,LifecycleWrite>());
 const profileGeneration=useRef(0);const drafts=useRef(new Map<string,Draft>());const generation=useRef(0);const alive=useRef(true);const retry=useRef<{key:string;id:string}|undefined>(undefined);
 const selected=useRef({profile,id});selected.current={profile,id};const selectedProfile=profiles.find(p=>p.id===profile);
 const key=(p:string,c:string)=>JSON.stringify([p,c]);
 async function read<T>(command:string,payload:Record<string,unknown>,scope?:string){const r=await client.call<T>(command,payload,undefined,scope);if(!r.ok)throw Error(r.error.messageKey);return r.result;}
 async function mutate<T>(command:string,payload:Record<string,unknown>,scope:string,revision?:number){const k=JSON.stringify([command,payload,scope,revision]);if(retry.current?.key!==k)retry.current={key:k,id:crypto.randomUUID()};const r=await client.call<T>(command,payload,revision,scope,retry.current.id);if(!r.ok){retry.current=undefined;throw Error(r.error.messageKey);}retry.current=undefined;return r.result;}
 function accept(c:Conversation){setRecord(c);const saved=drafts.current.get(key(c.profileId,c.id));if(!saved?.dirty){drafts.current.set(key(c.profileId,c.id),{text:c.draftText,dirty:false,edit:0});setDraft(c.draftText);}else setDraft(saved.text);}
 async function loadProfiles(){const g=++profileGeneration.current;let next:string|undefined;const all:Profile[]=[];const seen=new Set<string>();do{const p=await read<Page<Profile>>('profile.list',{limit:100,...(next?{cursor:next}:{})});all.push(...p.items);next=p.nextCursor;if(next&&seen.has(next))throw Error('error.transport');seen.add(next);}while(next);if(alive.current&&g===profileGeneration.current)setProfiles(workspaceMode?all.filter(p=>p.mode==='standard'):all);}
 useEffect(()=>{alive.current=true;if(desktop)void loadProfiles().catch(e=>setError(safe(e)));return()=>{alive.current=false;++generation.current;++profileGeneration.current;};},[client,desktop]);
 useEffect(()=>{
  const g=++generation.current;
  setList([]);setCursor('');setId('');setRecord(undefined);setDraft('');setError('');setNotice('');
  if(!desktop||!profile)return;
  const initial=opening.current?.profileId===profile?opening.current.conversationId:undefined;
  async function open(){
   const p=await read<Page<Conversation>>('conversation.list',{page:{limit:50}},profile);
   if(g!==generation.current)return;
   setList(p.items);setCursor(p.nextCursor);
   const target=initial??(workspaceMode?p.items[0]?.id:undefined);
   if(!target)return;
   const c=await read<Conversation>('conversation.get',{conversationId:target},profile);
   if(g!==generation.current)return;
   opening.current=undefined;
   if(!p.items.some(item=>item.id===c.id))setList(items=>[c,...items]);
   setId(c.id);accept(c);
  }
  void open().catch(e=>{if(g===generation.current)setError(safe(e));});
 },[client,profile,desktop,workspaceMode]);

 useEffect(()=>{
  if(desktop&&refreshToken>0)void loadProfiles().catch(e=>{if(alive.current)setError(safe(e));});
 },[client,desktop,refreshToken]);
 async function reloadConversations(scope=selected.current.profile){if(!scope){if(alive.current){setList([]);setCursor('');}return;}const g=++generation.current;const p=await read<Page<Conversation>>('conversation.list',{page:{limit:50}},scope);if(alive.current&&g===generation.current&&selected.current.profile===scope){setList(p.items);setCursor(p.nextCursor);}}
 function safe(e:unknown){return e instanceof Error&&e.message.startsWith('error.')?e.message:'error.transport';}
 async function action(run:()=>Promise<void>){if(busy)return;setBusy(true);setError('');setNotice('');try{await run();}catch(e){if(alive.current)setError(safe(e));}finally{if(alive.current)setBusy(false);}}
 async function choose(value:string){const g=++generation.current;setId(value);setRecord(undefined);setDraft('');setError('');setNotice('');if(!value)return;try{const c=await read<Conversation>('conversation.get',{conversationId:value},profile);if(g===generation.current)accept(c);}catch(e){if(g===generation.current)setError(safe(e));}}
 async function save(){if(!record)return;const k=key(profile,id);const current=drafts.current.get(k);const savingEdit=current?.edit??0;const textToSave=draft;try{const c=await mutate<Conversation>('conversation.draft.set',{conversationId:id,text:textToSave},profile,record.revision);const latest=drafts.current.get(k);if(latest?.edit===savingEdit)drafts.current.set(k,{text:c.draftText,dirty:false,edit:savingEdit});accept(c);setNotice(latest?.edit===savingEdit?'talk.saved':'talk.newEdits');}catch(e){if(e instanceof Error&&e.message==='error.revision_conflict'){const latest=await read<Conversation>('conversation.get',{conversationId:id},profile);accept(latest);}throw e;}}
 const contextTools=record&&<>
 <AttachmentsPanel key={'files'+key(profile,id)} client={client} locale={locale} profileId={profile} conversationId={id} onProfileContextChange={()=>setProfileContextRevision(v=>v+1)} candidateTracker={{operation:candidateOperations.current.get(profile),setOperation:op=>{if(op)candidateOperations.current.set(profile,op);else candidateOperations.current.delete(profile);setCandidateTrackerVersion(v=>v+1);},getPending:()=>candidatePendings.current.get(profile),setPending:p=>{if(p)candidatePendings.current.set(profile,p);else candidatePendings.current.delete(profile);setCandidateTrackerVersion(v=>v+1);}}} selected={documentSelections.current.get(key(profile,id))??[]} refreshToken={sourcesVersion} onSelectionChange={ids=>{documentSelections.current.set(key(profile,id),ids);setSourcesVersion(v=>v+1);}}/><button onClick={()=>setShowLayers(v=>!v)}>{text('layers.toggle')}</button>{showLayers&&<MemoryLayersPanel key={'layers'+profile} client={client} locale={locale} profileId={profile} conversationId={id} attempts={memoryAttempts.current} writes={libraryWrites.current} onContextChange={setMemoryContextKey}/>}<MemoryPanel hideSettings={workspaceMode} onContextChange={setMemoryContextKey} key={'memory'+key(profile,id)} client={client} locale={locale} profileId={profile} conversationId={id} attempts={memoryAttempts.current} refreshToken={memoryAttemptVersion+refreshToken} onAttemptsChange={()=>{if(alive.current)setMemoryAttemptVersion(v=>v+1);}}/>
 </>;
 return <section className="setup conversations" aria-labelledby="conversations-title"><h2 id="conversations-title">{text('talk.title')}</h2><p className="muted">{text(desktop?'talk.body':'talk.preview')}</p>{desktop&&<>
 <label>{text('talk.profile')}<select aria-label={text('talk.profile')} value={profile} disabled={busy} onChange={e=>{opening.current=undefined;setProfile(e.target.value);onProfileChange?.(e.target.value);}}><option value="">{text('config.choose')}</option>{profiles.map(p=><option key={p.id} value={p.id}>{p.displayName}</option>)}</select></label><button disabled={busy} onClick={()=>void action(loadProfiles)}>{text('talk.reloadProfiles')}</button>
 {selectedProfile?.status==='draft'&&onFinishSetup&&
   <button className="primary" disabled={busy} onClick={()=>void action(()=>onFinishSetup(selectedProfile.id))}>{text('onboarding.finish')}</button>}
 {profile&&<><fieldset disabled={busy}><div className="fields"><label>{text('talk.newTitle')}<input aria-label={text('talk.newTitle')} value={title} maxLength={512} onChange={e=>setTitle(e.target.value)}/></label><label>{text('talk.language')}<select aria-label={text('talk.language')} value={language} onChange={e=>setLanguage(e.target.value)}><option value="en">English</option><option value="pl">Polski</option></select></label></div><button disabled={!title.trim()} onClick={()=>void action(async()=>{const c=await mutate<Conversation>('conversation.create',{title,language},profile);setList(old=>[c,...old]);setTitle('');++generation.current;setId(c.id);accept(c);})}>{text('talk.create')}</button></fieldset>
 <label>{text('talk.choose')}<select aria-label={text('talk.choose')} value={id} disabled={busy} onChange={e=>void choose(e.target.value)}><option value="">{text('config.choose')}</option>{list.map(c=><option key={c.id} value={c.id}>{c.title||text('talk.untitled')}</option>)}</select></label>{cursor&&<button disabled={busy} onClick={()=>void action(async()=>{const p=await read<Page<Conversation>>('conversation.list',{page:{limit:50,cursor}},profile);setList(old=>{const byId=new Map(old.map(c=>[c.id,c]));p.items.forEach(c=>byId.set(c.id,c));return [...byId.values()];});setCursor(p.nextCursor);})}>{text('talk.more')}</button>}
 {record&&<><p className="muted">{record.language} · {text('talk.revision')} {record.revision}</p><label>{text('talk.draft')}<textarea aria-label={text('talk.draft')} value={draft} rows={7} onChange={e=>{const k=key(profile,id);const old=drafts.current.get(k);drafts.current.set(k,{text:e.target.value,dirty:true,edit:(old?.edit??0)+1});setDraft(e.target.value);setNotice('');}}/></label><p className="muted">{text('talk.saveNotice')}</p><button disabled={busy} onClick={()=>void action(save)}>{text('talk.save')}</button><button disabled={busy} onClick={()=>void action(async()=>accept(await read<Conversation>('conversation.get',{conversationId:id},profile)))}>{text('talk.refresh')}</button><ChatPanel compact={workspaceMode} refreshToken={refreshToken} memoryContextKey={memoryContextKey+'|profile:'+profileContextRevision} key={key(profile,id)} client={client} locale={locale} record={record} draft={draft} attempts={chatAttempts.current} attachmentIds={documentSelections.current.get(key(profile,id))??[]} onSourcesAccepted={(p,c,ids)=>{const k=key(p,c);documentSelections.current.set(k,(documentSelections.current.get(k)??[]).filter(id=>!ids.includes(id)));setSourcesVersion(v=>v+1);}} onRecord={c=>{if(alive.current&&selected.current.profile===c.profileId&&selected.current.id===c.id)accept(c);}} onAccepted={(p,c,sent)=>{const k=key(p,c);const current=drafts.current.get(k);if(current?.text===sent){drafts.current.set(k,{text:'',dirty:false,edit:current.edit});if(alive.current&&selected.current.profile===p&&selected.current.id===c)setDraft('');}}}/>{workspaceMode?<details className="conversation-context"><summary>{text('talk.context')}</summary>{contextTools}</details>:contextTools}</>}
 </>}
 <LifecyclePanel client={client} locale={locale} writes={lifecycleWrites.current} profile={selectedProfile} conversation={record} onProfileArchived={p=>{if(selected.current.profile===p.id){setProfile('');onProfileChange?.('');setId('');setRecord(undefined);setList([]);}void loadProfiles();}} onProfileRestored={()=>{void loadProfiles();}} onConversationArchived={c=>{if(selected.current.profile===c.profileId&&selected.current.id===c.id){setId('');setRecord(undefined);setDraft('');}void reloadConversations(c.profileId);}} onConversationRestored={c=>{void reloadConversations(c.profileId);}}/>
 </>}{error&&<p role="alert" className="error">{text(error)}</p>}{notice&&<p role="status">{text(notice)}</p>}</section>;
}
