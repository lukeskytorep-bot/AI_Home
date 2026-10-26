import fs from 'node:fs';import crypto from 'node:crypto';import path from 'node:path';
let checks=0;const check=(v,message)=>{checks++;if(!v)throw Error(message);};
const hashes=JSON.parse(fs.readFileSync('contracts/p2-frozen.sha256.json'));
for(const [name,hash]of Object.entries(hashes)){check(crypto.createHash('sha256').update(fs.readFileSync('contracts/'+name)).digest('hex')===hash,'P2 contract changed: '+name);}
const kernel=fs.readFileSync('src-tauri/src/kernel.rs','utf8');check(!/sqlx|reqwest|tauri|sqlite|ProviderConnection/.test(kernel),'kernel contains application infrastructure');
function walk(dir){return fs.readdirSync(dir,{withFileTypes:true}).flatMap(e=>e.isDirectory()?walk(path.join(dir,e.name)):[path.join(dir,e.name)]);}
for(const f of walk('src').filter(f=>/\.(ts|tsx)$/.test(f)&&!f.startsWith('src/ipc/'))){check(!/@tauri-apps|\binvoke\(|\bfetch\(/.test(fs.readFileSync(f,'utf8')),'UI/domain bypasses IPC: '+f);}
const cap=JSON.parse(fs.readFileSync('src-tauri/capabilities/main.json'));check(JSON.stringify(cap.windows)==='["main"]','unscoped window permission');check(JSON.stringify(cap.permissions)==='["allow-home-dispatch","core:event:allow-listen","core:event:allow-unlisten"]','unexpected capability');check(!cap.remote,'remote capability');
const en=JSON.parse(fs.readFileSync('src/locales/en.json'));const pl=JSON.parse(fs.readFileSync('src/locales/pl.json'));check(JSON.stringify(Object.keys(en.strings).sort())===JSON.stringify(Object.keys(pl.strings).sort()),'missing translations');
check(JSON.parse(fs.readFileSync('contracts/defaults.json')).appPreferences.locale==='en','default language');


// 4B-3 step 1 stays a pure protocol; runtime integration belongs to the coordinator.
const memoryProtocol=fs.readFileSync("src-tauri/src/memory/mod.rs","utf8");
check(!/\b(?:sqlx|reqwest|tauri|tokio)\b/.test(memoryProtocol),"memory protocol contains runtime infrastructure");

const retrievalProtocol=fs.readFileSync("src-tauri/src/retrieval.rs","utf8");
check(!/\b(?:sqlx|reqwest|tauri|tokio)\b/.test(retrievalProtocol),"retriever contains runtime infrastructure");

// Migration fixtures keep their historical PRAGMA assignments. Assertions after
// Store::open must check today's target, including both common Rust layouts.
const storageSource=fs.readFileSync('src-tauri/src/persistence.rs','utf8');
const storageTarget=storageSource.match(/PRAGMA user_version=(\d+)/);
check(Boolean(storageTarget),'missing current SQLite storage target');
function storageAssertions(source){return [...source.matchAll(/assert_eq!\s*\(\s*sqlx::query_scalar(?:::\s*<[^>]+>)?\s*\(\s*"PRAGMA user_version"\s*\)\s*\.fetch_one\([^)]*\)\s*\.await\s*\.unwrap\(\)\s*,\s*(\d+)\s*,?\s*\)/g)];}
const storageSamples='sqlx::query("PRAGMA user_version=13");\nassert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(&pool).await.unwrap(),13);\nassert_eq!(\n sqlx::query_scalar::<_, i64>("PRAGMA user_version")\n .fetch_one(&pool).await.unwrap(),\n 14\n);';
check(JSON.stringify(storageAssertions(storageSamples).map(m=>m[1]))==='["13","14"]','schema assertion scanner must handle both layouts and ignore fixtures');
const staleStorage=[];let storageAssertionCount=0;
for(const file of walk('src-tauri/tests').filter(f=>f.endsWith('.rs'))){
 const source=fs.readFileSync(file,'utf8');
 for(const match of storageAssertions(source)){
  storageAssertionCount++;
  if(match[1]!==storageTarget[1])staleStorage.push(`${file}:${source.slice(0,match.index).split('\n').length}: expects ${match[1]}, storage target ${storageTarget[1]}`);
 }
}
check(storageAssertionCount>0&&staleStorage.length===0,'stale SQLite target expectations:\n'+staleStorage.join('\n'));
console.log(`SQLite target v${storageTarget[1]}: ${storageAssertionCount} migration assertions checked`);

console.log(`${checks} boundary checks PASS`);
