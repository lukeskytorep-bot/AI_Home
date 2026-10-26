#!/usr/bin/env python3
"""Static scope guard; behavioral proof lives in native integration tests."""
from pathlib import Path
import json
r=Path(__file__).resolve().parents[1]
m=(r/'src-tauri/src/experimental_reflection.rs').read_text()
s=(r/'src-tauri/src/persistence.rs').read_text()
c=(r/'src-tauri/src/connections.rs').read_text()
d=(r/'src-tauri/src/deletion.rs').read_text()
e=(r/'src-tauri/src/export.rs').read_text()
t=(r/'src-tauri/tests/experimental_reflection.rs').read_text()
a=(r/'src-tauri/src/experimental_runtime.rs').read_text()
defs=json.loads((r/'contracts/experimental.schema.json').read_text())['$defs']
checks=[
 ('supplemental DTOs',all(n in defs for n in ['ReflectionOutput','ReflectionRun','PrivateReflection','PrivateSelfReview','PrivateSelfRecord'])),
 ('storage target', 'PRAGMA user_version=15' in s),
 ('new records participate in permanent deletion',all(n in s and n in c for n in ['experimental_reflection_runs','experimental_private_reflections','experimental_self_reviews','experimental_self_records'])),
 ('source tombstones','experimental_private_reflections' in d and 'experimental_self_records' in d),
 ('generator is replaceable and disabled by default','pub trait ReflectionProvider' in m and 'DisabledReflectionProvider' in a),
 ('no network or executor in private module', all(x not in m for x in ['reqwest::','ActionExecutor','Vault','std::process','tokio::process'])),
 ('explicit speculation labels','synthetic_hypothesis' in m and 'self_authored_not_verified_fact' in m),
 ('whole context and call budget','context_current' in m and 'checked_add' in m and 'tokio::time::timeout' in a),
 ('portable export explicitly excludes private records','private reflection/dream records, self notes, values and review receipts' in e),
 ('migration and crash recovery coverage','actual_v14_migration_preserves_preferences' in t and 'restart_interrupts_reserved_run' in t),
 ('backup/privacy/deletion coverage','full_backup_restore_keeps_private_records' in t and 'permanent_profile_deletion_removes_real_reflections' in t),
 ('stale output and content corruption coverage','changed_private_context_during_generation' in t and 'private_corruption_is_detected' in t),
]
checks += [
 ('full-entry reviewability guard', 'fn ensure_reviewable' in m and 'ensure_reviewable("records"' in m and 'candidate_entry(&body' in m),
 ('original checkpoint and private-record provenance', 'fn inherit' in m and 'r.semantic_checkpoint.iter()' in m and 'inherit(&mut refs, r)' in m),
 ('bounded provenance without silent truncation', 'refs.len() > 32' in m and '> 8192' in m and 'source_refs(packet)?; // complete provenance' in m),
 ('audit regressions', 'audit_f1_unlimited_modified_note_5000' in t and 'audit_f2_provenance_overflow' in t and (r/'src-tauri/src/experimental_reflection_audit_tests.rs').is_file()),
]
for label,ok in checks:
 if not ok: raise SystemExit('FAIL: '+label)
print(f'PASS: {len(checks)}/{len(checks)} Experimental reflection static scope checks (not runtime proof).')
