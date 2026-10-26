#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
root=Path(__file__).resolve().parents[1]
fail=[]
def ok(cond,msg):
    if not cond: fail.append(msg)
exp=json.loads((root/'contracts/experimental.schema.json').read_text())
defs=exp.get('$defs',{})
required={'ExperimentalSourceRef','ExperimentalInstance','WakePlan','DecisionRecord','RefusalRecord','WorkEpisode','TechnicalWorkCheckpoint','SemanticWorkCheckpoint','ActionRecord','ExperimentalEventCursor'}
ok(required <= set(defs), 'experimental.schema.json missing required EXP-0 DTOs')
# No raw content escape hatches in technical/action contracts.
props=set(defs.get('TechnicalWorkCheckpoint',{}).get('properties',{}))
ok(not ({'content','rawContent','rawPayload','rawConversation','messages'} & props), 'TechnicalWorkCheckpoint must not store raw content directly')
action_variants=defs.get('ActionRecord',{}).get('oneOf',[])
ok(len(action_variants)==3,'ActionRecord must freeze base, accepted and reconciled state variants')
for variant in action_variants:
    props=set(variant.get('properties',{}))
    ok(not ({'content','rawContent','rawPayload','rawConversation','messages'} & props), 'ActionRecord must not store raw content directly')
    ok({'payloadHash','payloadRef','visibility','senderIdentity','destination','receiptRef'} <= props, 'ActionRecord variants must freeze payload reference, evidence, audience and identity')
accepted=[v for v in action_variants if v.get('properties',{}).get('state',{}).get('const')=='accepted']
reconciled=[v for v in action_variants if v.get('properties',{}).get('state',{}).get('const')=='reconciled']
ok(len(accepted)==1 and accepted[0]['properties']['receiptRef'].get('minLength')==1,'accepted ActionRecord must require receipt evidence')
ok(len(reconciled)==1 and reconciled[0]['properties']['receiptRef'].get('minLength')==1 and 'reconciliationOutcome' in reconciled[0].get('required',[]),'reconciled ActionRecord must require evidence and explicit outcome')
ok(defs['ExperimentalInstance']['properties']['direction'].get('const')=='experimental','ExperimentalInstance must be direction-bound')
ok('permanently_deleted' in defs['ExperimentalSourceRef']['properties']['availability'].get('enum',[]),'source refs must represent permanent deletion without hidden copies')
# Storage remains backward compatible; 4B-10 raises the current version to 14 for internal-state persistence.
p=(root/'src-tauri/src/persistence.rs').read_text()
ok('if version>14' in p and 'PRAGMA user_version=14' in p,'storage target must be v14')
for file_name,needle in [('src-tauri/src/backup.rs','CURRENT_SCHEMA:i64=14'),('src-tauri/src/export.rs','\"schemaVersion\":14'),('src-tauri/src/diagnostics.rs','\"storageSchemaVersion\":14')]:
    ok(needle in (root/file_name).read_text(),f'stale storage schema constant in {file_name}')
for table in ['experimental_instances','experimental_wake_plans','experimental_decisions','experimental_refusals','experimental_work_episodes','experimental_technical_checkpoints','experimental_semantic_checkpoints','experimental_actions','experimental_event_cursors','experimental_internal_state_blobs','experimental_internal_state_snapshots','experimental_internal_state_events','experimental_wake_candidates']:
    ok(f'CREATE TABLE IF NOT EXISTS {table}' in p,f'missing storage table {table}')
# EXP-0 must not activate scheduler/provider behavior.
all_rs='\n'.join(x.read_text(errors='ignore') for x in (root/'src-tauri/src').glob('*.rs'))
ok('ExperimentalDecisionService' not in all_rs,'EXP-0 must not add live Experimental decision service yet')
ok('experimental.start' not in all_rs and 'experimental.impulse' not in all_rs,'EXP-0 must not add start/impulse IPC yet')
# Public validators must include the new schema without editing frozen domain.schema.
ts=(root/'src/domain/schema.ts').read_text(); rs=(root/'src-tauri/src/domain/schema.rs').read_text()
ok("experimental.schema.json" in ts and '...experimental.$defs' in ts,'TypeScript validator must load supplemental Experimental schema')
ok('experimental.schema.json' in rs,'Rust validator must load supplemental Experimental schema')
# Permanent current-version expectations in live source/tests must not remain at 12.
for base in [root/'src-tauri/src',root/'src-tauri/tests']:
    for f in base.rglob('*.rs'):
        t=f.read_text(errors='ignore')
        if re.search(r'PRAGMA user_version[\s\S]{0,180}(?:assert_eq!|unwrap\(\),)\s*12',t): fail.append(f'stale current schema expectation 12: {f.relative_to(root)}')
# Required regression files.
ok((root/'src-tauri/tests/experimental_contracts.rs').exists(),'missing native Experimental contract/migration regressions')
ok((root/'tests/experimental-contracts.test.ts').exists(),'missing TypeScript Experimental schema regressions')
connections=(root/'src-tauri/src/connections.rs').read_text()
ok('experimental_statuses' in connections and 'status!="inactive"' in connections,'profile permanent delete must reject non-inactive Experimental instance')
for table in ['experimental_wake_candidates','experimental_internal_state_events','experimental_internal_state_snapshots','experimental_internal_state_blobs','experimental_semantic_checkpoints','experimental_technical_checkpoints','experimental_event_cursors','experimental_actions','experimental_refusals','experimental_decisions','experimental_wake_plans','experimental_work_episodes','experimental_instances']:
    ok(table in connections,f'profile permanent delete must cover {table}')
if fail:
    print('FAIL: Experimental foundation')
    for x in fail: print(' - '+x)
    sys.exit(1)
print('PASS: 12/12 Experimental foundation invariants.')
