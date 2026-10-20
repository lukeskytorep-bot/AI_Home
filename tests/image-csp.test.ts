import {it,expect} from 'vitest';
import config from '../src-tauri/tauri.conf.json';
it('production and development CSP allow local image Blob previews without granting Blob access to other directives',()=>{
 for(const field of ['csp','devCsp'] as const){
  const directives=config.app.security[field].split(';').map(s=>s.trim().split(/\s+/)).filter(a=>a[0]);
  expect(directives.filter(a=>a[0]==='img-src')).toHaveLength(1);
  const images=directives.find(a=>a[0]==='img-src')!;
  expect(images.slice(1)).toEqual(["'self'",'data:','blob:']);
  expect(directives.filter(a=>a[0]!=='img-src').flat()).not.toContain('blob:');
  expect(directives.flat()).not.toContain('*');
 }
});
