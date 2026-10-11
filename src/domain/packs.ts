import en from '../locales/en.json';import pl from '../locales/pl.json';import orbital from '../themes/orbital.json';import aurora from '../themes/aurora.json';import {validDto} from './schema';
export const locales=[en,pl];export const themes=[orbital,aurora];
export function validatePack(kind:'locale'|'theme',pack:unknown):string[]{
 if(!validDto(kind==='locale'?'LocalePack':'ThemePack',pack))throw Error('PACK_INVALID');
 if(kind==='theme')return [];
 const strings=(pack as typeof en).strings;
 if(Object.keys(strings).some(k=>!Object.hasOwn(en.strings,k)))throw Error('PACK_INVALID');
 return Object.keys(en.strings).filter(k=>!Object.hasOwn(strings,k)).map(k=>'fallback.en:'+k);
}
export function t(locale:string,key:string):string{const p=locales.find(x=>x.locale===locale)??en;return (p.strings as Record<string,string>)[key]??(en.strings as Record<string,string>)[key]??key;}
export function applyTheme(id:string,variant:'light'|'dark'|'system'):void{
 const p=themes.find(x=>x.id===id);if(!p)throw Error('PACK_INVALID');
 const actual=variant==='system'?(window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'):variant;
 for(const [key,value] of Object.entries(p.variants[actual]))document.documentElement.style.setProperty('--'+key,value);
 document.documentElement.style.colorScheme=actual;
}
