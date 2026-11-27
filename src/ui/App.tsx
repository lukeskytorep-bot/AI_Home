import {AutonomousWorkspace} from './AutonomousWorkspace';
import {NotesWorkspace} from './NotesWorkspace';
import {CommunityWorkspace} from './CommunityWorkspace';
import {WorkspaceMemorySettings} from './WorkspaceMemorySettings';
import {FirstConversation} from './FirstConversation.tsx';
import {listStandardProfiles,prepareExistingSetup,readSetupDraft,type SetupProfile,type WorkspaceSelection} from './firstConversation';
import {CommunityPanel} from './CommunityPanel';
import {Conversations} from './Conversations';
import {BackupPanel} from './BackupPanel';
import {ExportPanel} from './ExportPanel';
import {ConnectionsSetup} from './ConnectionsSetup';
import {ActivityPanel} from './ActivityPanel';
import {DiagnosticsPanel} from './DiagnosticsPanel';
import {McpPanel} from './McpPanel';
import {useEffect,useState,useMemo,useRef} from 'react';import type {Preferences,Transport,KernelStatus,AppEvent} from '../domain/types';import {Client} from '../ipc/client';import {locales,themes,t,applyTheme} from '../domain/packs';
export function App({transport}:{transport:Transport}){
 const client=useMemo(()=>new Client(transport),[transport]);const [prefs,setPrefs]=useState<Preferences>();const [draft,setDraft]=useState({locale:'en',themeId:'orbital',themeVariant:'light' as Preferences['themeVariant']});const [error,setError]=useState('');const [busy,setBusy]=useState(false);const [saved,setSaved]=useState(false);const [ready,setReady]=useState(false);const [retry,setRetry]=useState(0);const revision=useRef(0);const sequence=useRef(0);// Draft ownership is independent of the last committed revision.
 const dirty=useRef(false);const edits=useRef(0);
 const [page,setPage]=useState<'home'|'notes'|'community'|'settings'|'activity'>('home');
 const [workspace,setWorkspace]=useState<'standard'|'autonomous'>('standard');
 const [workspaceRefresh,refreshWorkspace]=useState(0);
 const [initial,setInitial]=useState(()=>transport.mode==='desktop'?readSetupDraft():undefined);
 const [profiles,setProfiles]=useState<SetupProfile[]>();
 const [profileError,setProfileError]=useState('');
 const [selection,setSelection]=useState<WorkspaceSelection>();
 const [showSetup,setShowSetup]=useState(Boolean(initial));
 const desktop=transport.mode==='desktop';
 useEffect(()=>{
  if(!desktop||!ready)return;
  let active=true;
  setProfileError('');
  void listStandardProfiles(client).then(items=>{
    if(!active)return;
    setProfiles(items);
    if(!initial&&items.length===0)setShowSetup(true);
    const existing=items.find(p=>p.status==='ready')??items[0];
    if(!initial&&existing)setSelection(old=>old??{profileId:existing.id});
  }).catch(()=>{if(active)setProfileError('error.transport');});
  return()=>{active=false;};
 },[client,desktop,ready,retry,initial]);
 const locale=draft.locale;const text=(key:string)=>t(locale,key);
 useEffect(()=>{document.documentElement.lang=locale;applyTheme(draft.themeId,draft.themeVariant);const media=window.matchMedia('(prefers-color-scheme: dark)');const change=()=>applyTheme(draft.themeId,draft.themeVariant);media.addEventListener('change',change);return()=>media.removeEventListener('change',change);},[locale,draft.themeId,draft.themeVariant]);
 useEffect(()=>{let active=true;let unlisten:(()=>void)|undefined;
  // Equal-revision notifications may arrive after a save and newer local edits.
  const accept=(p:Preferences)=>{if(!active||p.revision<=revision.current)return;revision.current=p.revision;setPrefs(p);if(!dirty.current)setDraft({locale:p.locale,themeId:p.themeId,themeVariant:p.themeVariant});};
  const event=(e:AppEvent)=>{if(!active||e.sequence<=sequence.current)return;if(e.sequence>sequence.current+1){void sync();return;}sequence.current=e.sequence;accept(e.payload);};
  async function sync(){try{const k=await client.call<KernelStatus>('kernel.status.get');const p=await client.call<Preferences>('app.preferences.get');if(!active)return;if(!p.ok){setError(p.error.messageKey);return;}if(!k.ok){setError(k.error.messageKey);return;}accept(p.result);sequence.current=Math.max(sequence.current,k.result.lastEventSequence);setReady(true);}catch{if(active)setError('error.transport');}}
  void transport.subscribe(event).then(stop=>{if(!active){stop();return;}unlisten=stop;void sync();}).catch(()=>{if(active)setError('error.transport');});
  return()=>{active=false;unlisten?.();};
 },[client,transport,retry]);
 async function save(){if(!prefs||busy)return;const savingEdit=edits.current;setBusy(true);setError('');setSaved(false);try{const r=await client.call<Preferences>('app.preferences.set',draft,prefs.revision);if(r.ok){if(r.result.revision>=revision.current){revision.current=r.result.revision;setPrefs(r.result);if(edits.current===savingEdit){dirty.current=false;setDraft({locale:r.result.locale,themeId:r.result.themeId,themeVariant:r.result.themeVariant});setSaved(true);}}}else{setError(r.error.messageKey);if(r.error.code==='REVISION_CONFLICT'){const latest=await client.call<Preferences>('app.preferences.get');if(latest.ok&&latest.result.revision>revision.current){revision.current=latest.result.revision;setPrefs(latest.result);}}}}catch{setError('error.transport');}finally{setBusy(false);}}

 function navigate(next: typeof page){
  if(page==='settings'&&next!=='settings')refreshWorkspace(value=>value+1);
  if(next==='activity'||next==='notes'||next==='community')setWorkspace('standard');
  setPage(next);
 }
 function switchWorkspace(next: typeof workspace){
  if(page==='settings')refreshWorkspace(value=>value+1);
  setWorkspace(next);
  setPage('home');
 }
 const retryBoot=()=>{setError('');setRetry(x=>x+1);};
 return <div className="app">
  <aside className="navigation">
    <div className="brand"><span className="orb" aria-hidden="true">✦</span><div>AI Home<small>{text('brand.tagline')}</small></div></div>
    <nav aria-label="AI Home">
      <button aria-current={page==='home'?'page':undefined} className={'nav'+(page==='home'?' active':'')} onClick={()=>navigate('home')}>{text('nav.home')}</button>
      <button aria-current={page==='notes'?'page':undefined} className={'nav'+(page==='notes'?' active':'')} onClick={()=>navigate('notes')}>{text('nav.notes')}</button>
      <button aria-current={page==='community'?'page':undefined} className={'nav'+(page==='community'?' active':'')} onClick={()=>navigate('community')}>{text('nav.community')}</button>
      {['knowledge','rules'].map(key=><button key={key} className="nav" disabled title={text('status.pending')}>{text('nav.'+key)}</button>)}
      <button aria-current={page==='activity'?'page':undefined} className={'nav'+(page==='activity'?' active':'')} onClick={()=>navigate('activity')}>{text('nav.activity')}</button>
    </nav>
  </aside>
  <main>
    <header><span className="status"><i/>{text(desktop?'status.desktop':'status.browser')}</span>
      <div className="header-actions">
        <div className="workspace-switch" role="group" aria-label={text('workspace.switcher')}>
          <button type="button" aria-pressed={workspace==='standard'} className={workspace==='standard'?'active':undefined} onClick={()=>switchWorkspace('standard')}>{text('workspace.standard')}</button>
          <button type="button" aria-pressed={workspace==='autonomous'} className={workspace==='autonomous'?'active':undefined} onClick={()=>switchWorkspace('autonomous')}>{text('workspace.autonomous')}</button>
        </div>
        <button data-nav="settings" aria-current={page==='settings'?'page':undefined} onClick={()=>navigate(page==='settings'?'home':'settings')}>{text('nav.settings')}</button>
      </div>
    </header>
    <div className="workspace">
      <div className="center" hidden={page!=='home'||workspace!=='standard'}>
        {!desktop&&<section className="hero"><div className="planet" aria-hidden="true"><span>✦</span></div><h1>{text('hero.title')}</h1><p>{text('talk.preview')}</p></section>}
        {desktop&&!profiles&&!profileError&&!error&&<p role="status">{text('status.loading')}</p>}
        {(profileError||(!ready&&error))&&<div role="alert" className="error"><p>{text(profileError||error)}</p><button onClick={retryBoot}>{text('action.retry')}</button></div>}
        {desktop&&ready&&profiles&&showSetup&&<FirstConversation key={initial?.id??'new'} client={client} locale={locale} initial={initial}
          onCancel={profiles.length>0&&Boolean(initial?.profileId)?()=>{setInitial(undefined);setShowSetup(false);}:undefined}
          onOpen={next=>{setSelection(next);setInitial(undefined);setShowSetup(false);setPage('home');}}/>}
        {desktop&&ready&&profiles&&!showSetup&&<Conversations client={client} locale={locale} desktop
          initialSelection={selection} workspaceMode refreshToken={workspaceRefresh}
          onFinishSetup={async profileId=>{
            const existing=await prepareExistingSetup(client,profileId,locale);
            setInitial(existing);
            setShowSetup(true);
          }} onProfileChange={profileId=>setSelection(old=>old?.profileId===profileId?old:profileId?{profileId}:undefined)}/>}
      </div>
      {page==='home'&&workspace==='autonomous'&&<div className="center">
        <AutonomousWorkspace client={client} locale={locale} desktop={desktop} onBackToStandard={()=>switchWorkspace('standard')}/>
      </div>}
      {page==='notes'&&<div className="center">
        <NotesWorkspace client={client} locale={locale} desktop={desktop} initialProfileId={selection?.profileId}/>
      </div>}
      {page==='community'&&<div className="center">
        <CommunityWorkspace client={client} transport={transport} locale={locale} initialProfileId={selection?.profileId}/>
      </div>}
      {page==='settings'&&<div className="center settings-page">
        <h1>{text('nav.settings')}</h1>
        <a className="settings-jump" href="#diagnostics">{text('nav.diagnostics')}</a>
        <section className="setup" aria-labelledby="setup-title"><h2 id="setup-title">{text('settings.appearance')}</h2><div className="fields"><label>{text('settings.language')}<select aria-label={text('settings.language')} value={draft.locale} disabled={busy} onChange={e=>{dirty.current=true;edits.current++;setSaved(false);setDraft({...draft,locale:e.target.value});}}>{locales.map(p=><option key={p.locale} value={p.locale}>{p.displayName}</option>)}</select></label><label>{text('settings.theme')}<select aria-label={text('settings.theme')} value={draft.themeId} disabled={busy} onChange={e=>{dirty.current=true;edits.current++;setSaved(false);setDraft({...draft,themeId:e.target.value});}}>{themes.map(p=><option key={p.id} value={p.id}>{p.displayName}</option>)}</select></label><label>{text('settings.variant')}<select aria-label={text('settings.variant')} value={draft.themeVariant} disabled={busy} onChange={e=>{dirty.current=true;edits.current++;setSaved(false);setDraft({...draft,themeVariant:e.target.value as Preferences['themeVariant']});}}>{['light','dark','system'].map(v=><option key={v} value={v}>{text('variant.'+v)}</option>)}</select></label></div>{error&&<div role="alert" className="error"><strong>{text('error.title')}</strong><p>{text(error)}</p><button onClick={()=>{setError('');setRetry(x=>x+1);}}>{text('action.retry')}</button></div>}<div className="save"><button className="primary" disabled={!ready||busy} onClick={()=>void save()}>{text(ready?'action.save':'status.loading')}</button><span role="status">{saved?text('status.saved'):''}</span></div></section>
        <ConnectionsSetup client={client} locale={locale} desktop={desktop} initialProfileId={selection?.profileId}/>
        <WorkspaceMemorySettings client={client} locale={locale} desktop={desktop} initialProfileId={selection?.profileId}/>
        <McpPanel client={client} locale={locale} desktop={desktop} initialProfileId={selection?.profileId}/>
        <BackupPanel client={client} locale={locale} desktop={desktop}/>
        <ExportPanel client={client} locale={locale} desktop={desktop}/>
        <DiagnosticsPanel client={client} locale={locale} desktop={desktop}/>
        <details className="settings-community"><summary>{text('settings.community')}</summary>
          <CommunityPanel client={client} transport={transport} locale={locale}/>
        </details>
      </div>}
      {page==='activity'&&<ActivityPanel client={client} locale={locale} desktop={desktop}/>}
    </div>
  </main>
 </div>;
}
