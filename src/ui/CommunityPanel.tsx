import {useEffect,useRef,useState} from 'react';
import type {Transport} from '../domain/types';
import {Client} from '../ipc/client';
import {communityCall,type CommunityCommand} from '../ipc/community';
import {t} from '../domain/packs';
interface Profile {id:string;revision:number;displayName:string;mode:string;status:string}
interface Preparation {registrationRevision:number;preparationState:'absent'|'reserved'|'provisioning'|'ready'}
interface Progress {finishRevision:number;finishState:'absent'|'uncertain'|'ready_metadata'}
interface Snapshot {profile:Profile;preparation:Preparation;progress:Progress;usable:boolean}
export function CommunityPanel({client,transport,locale}:{client:Client;transport:Transport;locale:string}){
 const text=(key:string)=>t(locale,'community.'+key);
 const [profiles,setProfiles]=useState<Profile[]>([]),[id,setId]=useState(''),[snapshot,setSnapshot]=useState<Snapshot>();
 const [name,setName]=useState(''),[about,setAbout]=useState(''),[invite,setInvite]=useState('');
 const [busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
 const [listVersion,setListVersion]=useState(0);
 const generation=useRef(0),lock=useRef(false);
 const available=transport.mode==='desktop'&&Boolean(transport.communityDispatch);
 useEffect(()=>{let active=true;if(available)void (async()=>{try{let cursor='';const items:Profile[]=[];do{const r=await client.call<{items:Profile[];nextCursor:string}>('profile.list',{limit:100,...(cursor?{cursor}:{})});if(!r.ok)throw Error();items.push(...r.result.items);cursor=r.result.nextCursor;}while(cursor);if(active)setProfiles(items.filter(p=>p.mode==='standard'&&p.status!=='archived'));}catch{if(active)setError('community.failed');}})();return()=>{active=false;generation.current++;};},[available,client,listVersion]);
 async function load(profileId:string,token:number){
  const r=await client.call<{profile:Profile}>('profile.snapshot',{},undefined,profileId);
  if(!r.ok)throw Error('community.failed');
  if(r.result.profile.mode!=='standard'||r.result.profile.status==='archived')throw Error('community.denied');
  const call=<T,>(command:CommunityCommand)=>communityCall<T>(transport,command,profileId,r.result.profile.revision);
  const preparation=await call<Preparation>('community.registration.status');
  const progress=await call<Progress>('community.registration.progress');
  const access=await call<{localCredentialUsable:boolean}>('community.registration.access.status');
  if(token===generation.current)setSnapshot({profile:r.result.profile,preparation,progress,usable:access.localCredentialUsable});
 }
 function select(profileId:string){generation.current++;setId(profileId);setSnapshot(undefined);setInvite('');setName('');setAbout('');setError('');setNotice('');}
 async function refresh(){if(lock.current||!id)return;lock.current=true;setBusy(true);setError('');setNotice('');setSnapshot(undefined);const token=++generation.current;try{await load(id,token);}catch(e){if(token===generation.current)setError(e instanceof Error&&e.message.startsWith('community.')?e.message:'community.failed');}finally{lock.current=false;if(token===generation.current)setBusy(false);}}
 async function action(command:CommunityCommand,payload:Record<string,unknown>={},secret?:string){
  if(lock.current||!snapshot)return;lock.current=true;setBusy(true);setError('');setNotice('');setInvite('');const token=++generation.current;const current=snapshot;setSnapshot(undefined);
  try{await communityCall(transport,command,id,current.profile.revision,payload,secret);if(token===generation.current)setNotice('community.done');await load(id,token);}catch(e){if(token===generation.current)setError(e instanceof Error&&e.message.startsWith('community.')?e.message:'community.uncertain');}finally{lock.current=false;if(token===generation.current)setBusy(false);}
 }
 const prep=snapshot?.preparation,progress=snapshot?.progress;
 return <section className="setup connection-setup community-setup" aria-labelledby="community-title"><h2 id="community-title">{text('title')}</h2><p className="muted">{text(available?'body':'unavailable')}</p>{available&&<>
 <fieldset disabled={busy}><button onClick={()=>{select('');setListVersion(v=>v+1);}}>{text('refreshProfiles')}</button><label>{text('profile')}<select aria-label={text('profile')} value={id} onChange={e=>select(e.target.value)}><option value="">{text('choose')}</option>{profiles.map(p=><option key={p.id} value={p.id}>{p.displayName}</option>)}</select></label><button disabled={!id} onClick={()=>void refresh()}>{text('refresh')}</button></fieldset>
 {snapshot&&<><p role="status">{text('preparation')}: {text(prep!.preparationState)} · {text('finish')}: {text(progress!.finishState==='uncertain'?'uncertainState':progress!.finishState)} · {text(snapshot.usable?'usable':'unusable')}</p><p className="muted">{text('offline')}</p>
 <fieldset disabled={busy}><label>{text('name')}<input maxLength={120} value={name} onChange={e=>setName(e.target.value)}/></label><label>{text('about')}<textarea maxLength={2048} value={about} onChange={e=>setAbout(e.target.value)}/></label>
 <button disabled={prep!.preparationState!=='absent'||!name.trim()} onClick={()=>void action('community.registration.prepare',{clientStartId:crypto.randomUUID(),displayName:name,about})}>{text('prepare')}</button>
 <button disabled={prep!.preparationState!=='reserved'} onClick={()=>void action('community.registration.provision',{registrationRevision:prep!.registrationRevision})}>{text('provision')}</button>
 <button disabled={!['provisioning','ready'].includes(prep!.preparationState)} onClick={()=>void action('community.registration.recover',{registrationRevision:prep!.registrationRevision})}>{text('recoverKeys')}</button>
 <label>{text('invite')}<input type="password" autoComplete="off" spellCheck={false} value={invite} onChange={e=>setInvite(e.target.value)}/></label><p className="muted">{text('secret')}</p>
 <button disabled={prep!.preparationState!=='ready'||!invite||new TextEncoder().encode(invite).length>200||/[\r\n\0]/.test(invite)} onClick={()=>void action('community.registration.start',{registrationRevision:prep!.registrationRevision},invite)}>{text('start')}</button>
 <button disabled={prep!.preparationState!=='ready'||progress!.finishState!=='absent'} onClick={()=>void action('community.registration.complete',{registrationRevision:prep!.registrationRevision})}>{text('complete')}</button>
 <button disabled={progress!.finishState!=='uncertain'||progress!.finishRevision<1} onClick={()=>void action('community.registration.complete.retry',{finishRevision:progress!.finishRevision})}>{text('retry')}</button>
 <button disabled={progress!.finishRevision<1} onClick={()=>void action('community.registration.access.recover',{finishRevision:progress!.finishRevision})}>{text('recoverAccess')}</button>
 </fieldset></>}
 </>}{error&&<p role="alert" className="error">{t(locale,error)}</p>}{notice&&<p role="status">{t(locale,notice)}</p>}</section>;
}
