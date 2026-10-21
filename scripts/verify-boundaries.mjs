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

console.log(`${checks} boundary checks PASS`);
