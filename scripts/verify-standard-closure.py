#!/usr/bin/env python3
"""Static 4B-7 Standard closure guard. Runtime idle behavior remains covered by Vitest/Cargo acceptance tests."""
import json,re,sys
from pathlib import Path
checks=[]
def check(name,ok):
    checks.append((name,ok))
    print(('PASS ' if ok else 'FAIL ')+name)
connections=Path('src-tauri/src/connections.rs').read_text()
app=Path('src/ui/App.tsx').read_text()
kernel=Path('src-tauri/src/kernel.rs').read_text()
ui='\n'.join(p.read_text() for p in Path('src/ui').glob('*.tsx'))
rust='\n'.join(p.read_text() for p in Path('src-tauri/src').rglob('*.rs'))
check('activity.list native route','"activity.list"' in connections and 'activity::list' in connections)
activity_ui=Path('src/ui/ActivityPanel.tsx').read_text()
activity_native=Path('src-tauri/src/activity.rs').read_text()
check('ActivityPanel visible in Standard UI','<ActivityPanel' in app and 'activity-log' in activity_ui)
check('Activity skips permanent-deletion tombstones','permanent_deletion' in activity_native and 'tombstone' in activity_native)
check('Activity refresh reloads profiles','async function refresh()' in activity_ui and 'await loadProfiles()' in activity_ui)
check('Standard uses NullState','"mode":"standard"' in kernel and 'stateProvider' in kernel and 'NullState' in kernel)
check('no periodic frontend interval', 'setInterval(' not in ui)
check('no periodic native interval', 'tokio::time::interval' not in rust and 'Interval::' not in rust)
package=json.loads(Path('package.json').read_text())
deps=set(package.get('dependencies',{}))|set(package.get('devDependencies',{}))
check('no forum/community runtime package dependency',not any(re.search(r'forum|community|eqis',d,re.I) for d in deps))
if not all(ok for _,ok in checks):sys.exit(1)
print(f'RESULT: {len(checks)}/{len(checks)} Standard closure static checks PASS.')
