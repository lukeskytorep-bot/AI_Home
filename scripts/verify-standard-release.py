#!/usr/bin/env python3
from pathlib import Path
import json,sys
checks=[]
def ok(name,cond): checks.append((name,bool(cond)))
root=Path('.')
en=json.loads((root/'src/locales/en.json').read_text())['strings'];pl=json.loads((root/'src/locales/pl.json').read_text())['strings']
orb=json.loads((root/'src/themes/orbital.json').read_text());aur=json.loads((root/'src/themes/aurora.json').read_text())
diag=(root/'src-tauri/src/diagnostics.rs').read_text();app=(root/'src/ui/App.tsx').read_text();contract=json.loads((root/'contracts/diagnostics.ipc.json').read_text())
ok('locale key parity',set(en)==set(pl))
ok('diagnostics localized',all(k in en and k in pl for k in ['diagnostics.title','privacy.title','about.title','diagnostics.safe']))
ok('both themes have light/dark',all(set(x['variants'])=={'light','dark'} for x in [orb,aur]))
ok('diagnostics contract has snapshot/export',set(contract['commands'])=={'diagnostics.snapshot.get','diagnostics.export.create'})
ok('diagnostics excludes content by construction','SELECT body FROM messages' not in diag and 'memory_versions' not in diag and 'credential_ref' not in diag.lower())
ok('diagnostics navigation visible',"id=\"diagnostics\"" not in app and 'DiagnosticsPanel' in app and 'nav.diagnostics' in app)
ok('privacy default remains content-free',json.loads((root/'contracts/defaults.json').read_text())['diagnosticsContentIncludedByDefault'] is False)
ok('stage label updated',en.get('status.stage')=='Standard Mode · 4B-8' and pl.get('status.stage')=='Tryb Standard · 4B-8')
fail=[n for n,v in checks if not v]
for n,v in checks: print(('PASS' if v else 'FAIL')+': '+n)
if fail: sys.exit(1)
print(f'PASS: Standard release checks {len(checks)}/{len(checks)}.')
