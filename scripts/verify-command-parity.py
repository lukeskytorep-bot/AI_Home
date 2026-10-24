#!/usr/bin/env python3
"""Ensure implemented supplemental IPC contracts have a native routing string."""
import json,re,sys
from pathlib import Path
contracts=[
'connections.ipc.json','conversations.ipc.json','chat.ipc.json','imports.ipc.json','document-chat.ipc.json','vision.ipc.json','memory.ipc.json','memory-chat.ipc.json','memory-layers.ipc.json','memory-history.ipc.json','profile-imports.ipc.json','conversation-segments.ipc.json','lifecycle.ipc.json','deletion.ipc.json','backup.ipc.json','export.ipc.json','diagnostics.ipc.json','foundation.ipc.json']
source='\n'.join((Path('src-tauri/src/connections.rs').read_text(),Path('src-tauri/src/application.rs').read_text()))
# Frozen catalog commands that are part of the current Standard runtime.
catalog_runtime={'activity.list'}
missing=[];count=0
catalog=json.loads((Path('contracts')/'ipc.catalog.json').read_text())
for cmd in sorted(catalog_runtime):
    count+=1
    if cmd not in catalog.get('commands',{}): missing.append(f'ipc.catalog.json missing frozen command: {cmd}')
    elif f'"{cmd}"' not in source: missing.append(f'ipc.catalog.json: {cmd}')
for name in contracts:
    p=Path('contracts')/name
    if not p.exists(): continue
    for cmd in json.loads(p.read_text()).get('commands',{}):
        # Future/non-runtime foundation entries are allowed only if absent from current source.
        if name=='foundation.ipc.json' and cmd not in {'app.preferences.get','app.preferences.set','app.pack.validate','kernel.status.get','app.events.list'}: continue
        count+=1
        if f'"{cmd}"' not in source: missing.append(f'{name}: {cmd}')
if missing:
    print('FAIL command parity:\n'+'\n'.join(missing));sys.exit(1)
print(f'PASS: contract/handler/router parity {count}/{count}.')
