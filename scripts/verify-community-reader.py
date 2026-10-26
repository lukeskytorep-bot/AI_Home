#!/usr/bin/env python3
"""Static read-adapter scope/source guards. Native tests provide behavioral proof."""
from pathlib import Path
import json,hashlib
r=Path(__file__).resolve().parents[1]
m=(r/'src-tauri/src/community_connector/mod.rs').read_text()
t=(r/'src-tauri/src/community_connector/tests.rs').read_text()
c=json.loads((r/'contracts/community.read.v1.json').read_text())
e=r/'docs/evidence/4b12-step-1/forum-reference'
identity=json.loads((e/'source-identity.json').read_text())
checks=[
 ('known accepted 4A source',identity['sha256']=='afe25c58590ec146f7be01ef83fc4854e717813bb99c86cd7d2c89251e4ebc8a'),
 ('source snapshot byte identity',all(hashlib.sha256((e/n).read_bytes()).hexdigest()==h for n,h in identity['files'].items())),
 ('only GET routes',all(x['method']=='GET' for x in c['routes']) and '.post(' not in m and '.put(' not in m),
 ('independent native module','pub mod community_connector;' in (r/'src-tauri/src/lib.rs').read_text()),
 ('no model, SQL, credentials or actions',all(x not in m for x in ['sqlx::','Vault','ActionExecutor','DecisionChoice','bearer_auth(','Authorization'])),
 ('no implicit retry/redirect/proxy','retry(reqwest::retry::never())' in m and 'Policy::none()' in m and '.no_proxy()' in m),
 ('bounded body and whole-call deadline','saturating_sub(bytes.len())' in m and 'tokio::time::timeout' in m),
 ('untrusted data and honest signature status','UntrustedExternal' in m and 'SignatureValidation::NotPerformed' in m),
 ('no active polling or IPC',not c['automaticPolling'] and not c['uiEnabled'] and not c['writesEnabled'] and not c['feedEnabled']),
 ('actual 4A page/limit convention','append_pair("board_id", board)' in m and c['page']['max']==10000 and c['limit']['max']==50),
 ('scope/tombstone regressions','thread_and_replies_bind_exact_requested_resource' in t and 'tombstones_never_reintroduce_payload' in t),
 ('native HTTP negative regressions','timeout_includes_wait_for_headers' in t and 'size_limit_handles_declared_and_streamed_body' in t and 'redirect_is_rejected' in t),
]
checks += [
 ('locked public-read status',c['normalPayloadStatuses']==['active','locked'] and '["active", "locked", "deleted"]' in m and '["active", "locked"].contains' in m),
 ('locked native HTTP regressions','audit_locked_thread_detail' in t and 'audit_mixed_active_and_locked_page' in t and 'audit_hidden_and_unknown_status' in t and 'audit_deleted_tombstone' in t),
]
for label,ok in checks:
 if not ok: raise SystemExit('FAIL: '+label)
print(f'PASS: {len(checks)}/{len(checks)} Community reader static guards (not live-server acceptance).')
