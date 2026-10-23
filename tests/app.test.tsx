import {it,expect,beforeEach,afterEach,vi} from 'vitest';import {act} from 'react';import {createRoot,type Root} from 'react-dom/client';import {App} from '../src/ui/App';import {browserTransport} from '../src/ipc/browser';
let root:Root;let host:HTMLDivElement;
beforeEach(()=>{localStorage.clear();Object.assign(globalThis,{IS_REACT_ACT_ENVIRONMENT:true});vi.stubGlobal('matchMedia',()=>({matches:false,addEventListener:vi.fn(),removeEventListener:vi.fn()}));host=document.createElement('div');document.body.append(host);root=createRoot(host);});
afterEach(async()=>{await act(async()=>root.unmount());host.remove();vi.unstubAllGlobals();});
async function mount(){await act(async()=>{root.render(<App transport={browserTransport()}/>);});}
async function select(index:number,value:string){await act(async()=>{const el=host.querySelectorAll('select')[index];el.value=value;el.dispatchEvent(new Event('change',{bubbles:true}));});}
it('boots EN and displays explicit preview and future stages',async()=>{await mount();expect(document.documentElement.lang).toBe('en');expect(host.textContent).toContain('Browser preview');expect(host.querySelectorAll('nav button:disabled')).toHaveLength(3);expect(host.querySelector('button.primary')?.hasAttribute('disabled')).toBe(false);});
it('changes PL, skin and variant without losing the draft, then saves',async()=>{await mount();await select(0,'pl');await select(1,'aurora');await select(2,'dark');expect(document.documentElement.lang).toBe('pl');expect([...host.querySelectorAll('select')].map(s=>s.value)).toEqual(['pl','aurora','dark']);expect(document.documentElement.style.colorScheme).toBe('dark');await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});expect(host.querySelector('[role=status]')?.textContent).not.toBe('');await act(async()=>root.unmount());root=createRoot(host);await mount();expect([...host.querySelectorAll('select')].map(s=>s.value)).toEqual(['pl','aurora','dark']);});
it('shows a readable error instead of accepting broken storage',async()=>{localStorage.setItem('ai-home.foundation.preview.v1','invalid');await mount();expect(host.querySelector('[role=alert]')).not.toBeNull();expect(host.querySelector('button.primary')?.hasAttribute('disabled')).toBe(true);});
it('offers labeled controls and a current navigation item',async()=>{await mount();for(const s of host.querySelectorAll('select'))expect(s.getAttribute('aria-label')).toBeTruthy();expect(host.querySelector('[aria-current=page]')).not.toBeNull();(host.querySelector('select') as HTMLSelectElement).focus();expect(document.activeElement).toBe(host.querySelector('select'));});

it('keeps the user draft after a revision conflict and allows explicit retry',async()=>{await mount();await select(0,'pl');const other=browserTransport();await other.dispatch({contractVersion:1,requestId:'external-update',command:'app.preferences.set',expectedRevision:1,payload:{locale:'en',themeId:'aurora',themeVariant:'light'}});await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});expect(host.querySelector('[role=alert]')).not.toBeNull();expect(host.querySelector('select')?.value).toBe('pl');await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});expect(host.querySelector('[role=alert]')).toBeNull();expect(host.querySelector('[role=status]')?.textContent).not.toBe('');});

it('preserves PL when the visible Retry button resynchronizes after a conflict',async()=>{
 await mount();await select(0,'pl');
 await browserTransport().dispatch({contractVersion:1,requestId:'conflicting-save',command:'app.preferences.set',expectedRevision:1,payload:{locale:'en',themeId:'orbital',themeVariant:'light'}});
 await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});
 expect(host.querySelector('[role=alert]')).not.toBeNull();
 await act(async()=>{(host.querySelector('[role=alert] button') as HTMLButtonElement).click();});
 expect(host.querySelector('select')?.value).toBe('pl');
 await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});
 const persisted=await browserTransport().dispatch<import('../src/domain/types').Preferences>({contractVersion:1,requestId:'inspect-retry',command:'app.preferences.get',payload:{}});
 expect(persisted.ok&&persisted.result.locale).toBe('pl');
});

it('does not erase new edits when the saved revision event arrives late',async()=>{
 const inner=browserTransport();const queued:import('../src/domain/types').AppEvent[]=[];
 let deliver:((event:import('../src/domain/types').AppEvent)=>void)|undefined;
 await inner.subscribe(e=>queued.push(e));
 const transport:import('../src/domain/types').Transport={mode:'browser',dispatch:r=>inner.dispatch(r),subscribe:async handler=>{deliver=handler;return()=>{deliver=undefined;};}};
 await act(async()=>{root.render(<App transport={transport}/>);});
 await select(0,'pl');await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});
 await select(1,'aurora');await select(2,'dark');
 expect(queued).toHaveLength(1);
 await act(async()=>{deliver!(queued[0]);});
 expect([...host.querySelectorAll('select')].map(s=>s.value)).toEqual(['pl','aurora','dark']);
 await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});
 const persisted=await inner.dispatch<import('../src/domain/types').Preferences>({contractVersion:1,requestId:'inspect-late-event',command:'app.preferences.get',payload:{}});
 expect(persisted.ok&&[persisted.result.locale,persisted.result.themeId,persisted.result.themeVariant]).toEqual(['pl','aurora','dark']);
});

it('updates committed revisions while dirty and resumes clean synchronization after saving',async()=>{
 const transport=browserTransport();await act(async()=>{root.render(<App transport={transport}/>);});
 await select(0,'pl');
 await act(async()=>{await transport.dispatch({contractVersion:1,requestId:'newer-while-dirty',command:'app.preferences.set',expectedRevision:1,payload:{locale:'en',themeId:'aurora',themeVariant:'dark'}});});
 expect([...host.querySelectorAll('select')].map(s=>s.value)).toEqual(['pl','orbital','light']);
 await act(async()=>{(host.querySelector('button.primary') as HTMLButtonElement).click();});
 expect(host.querySelector('[role=alert]')).toBeNull();
 await act(async()=>{await transport.dispatch({contractVersion:1,requestId:'newer-while-clean',command:'app.preferences.set',expectedRevision:3,payload:{locale:'en',themeId:'aurora',themeVariant:'dark'}});});
 expect([...host.querySelectorAll('select')].map(s=>s.value)).toEqual(['en','aurora','dark']);
});
