import {describe,it,expect} from 'vitest';
import fixtures from '../contracts/fixtures.json';import {validDto} from '../src/domain/schema';import {locales,themes,validatePack,t} from '../src/domain/packs';
describe('P2 compatibility',()=>{
 for(const fixture of fixtures){it(fixture.name,()=>expect(validDto(fixture.dto,fixture.value)).toBe(true));}
 it('rejects extra DTO fields',()=>{for(const f of fixtures){expect(validDto(f.dto,{...f.value,unexpected:true})).toBe(false);}});
 it('rejects empty route identity',()=>{const f=fixtures.find(f=>f.dto==='TokenMeasurement')!;expect(validDto(f.dto,{...f.value,modelRouteId:''})).toBe(false);});
 it('does not require fake image OCR',()=>{const f=fixtures.find(f=>f.name==='ready-vision-image')??fixtures.find(f=>f.dto==='ImportedMaterial'&&'imageMetadata'in f.value)!;expect(validDto(f.dto,f.value)).toBe(true);expect('extractedBlobId'in f.value).toBe(false);});
});
describe('declarative packs',()=>{
 it('accepts all shipped packs',()=>{for(const l of locales)expect(validatePack('locale',l)).toEqual([]);for(const p of themes)expect(validatePack('theme',p)).toEqual([]);});
 it('ships complete EN/PL with English fallback',()=>{expect(Object.keys(locales[0].strings).sort()).toEqual(Object.keys(locales[1].strings).sort());expect(t('unknown','action.save')).toBe(locales[0].strings['action.save']);});
 it('reports missing optional translations and refuses invented keys',()=>{const l=structuredClone(locales[0]) as {strings:Record<string,string>};delete l.strings['action.save'];expect(validatePack('locale',l)).toContain('fallback.en:action.save');l.strings['secret.script']='execute';expect(()=>validatePack('locale',l)).toThrow();});
 it('rejects executable CSS in a theme',()=>{const p=structuredClone(themes[0]);p.variants.light.accent='url(https://example.com)';expect(()=>validatePack('theme',p)).toThrow();});
});

function luminance(hex:string){const values=[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)/255).map(v=>v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4);return values[0]*0.2126+values[1]*0.7152+values[2]*0.0722;}
function contrast(a:string,b:string){const x=luminance(a),y=luminance(b);return (Math.max(x,y)+0.05)/(Math.min(x,y)+0.05);}
it('keeps text and controls readable in every shipped theme variant',()=>{for(const theme of themes){for(const [variant,colors] of Object.entries(theme.variants)){for(const key of ['text','muted','error','success'] as const){expect(contrast(colors[key],colors.surface),theme.id+variant+key).toBeGreaterThanOrEqual(4.5);}expect(contrast(colors.accent,variant==='dark'?colors.background:colors.surface)).toBeGreaterThanOrEqual(4.5);}}});
