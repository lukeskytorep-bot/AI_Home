#!/usr/bin/env python3
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[1];fail=[];checks=0
def ok(c,m):
 global checks;checks+=1
 if not c:fail.append(m)
mod=(root/'src-tauri/src/experimental_internal_state.rs').read_text();rt=(root/'src-tauri/src/experimental_runtime.rs').read_text();p=(root/'src-tauri/src/persistence.rs').read_text();t=(root/'src-tauri/tests/experimental_internal_state.rs').read_text();lib=(root/'src-tauri/src/lib.rs').read_text()
ok('pub mod experimental_internal_state;' in lib,'internal state module missing')
ok('pub trait InternalStateProvider' in mod and 'NullStateProvider' in mod,'replaceable provider + NullState missing')
ok('pub trait ConsciousCoreEngine' in mod and 'ConsciousCoreAdapter' in mod,'optional ConsciousCore adapter missing')
ok('previous_state' in mod and 'elapsed_ms' in mod,'elapsed transition input missing')
ok('One transition receives the whole elapsed interval' in mod,'restart must not replay missed ticks')
ok('NeutralEventDraft' in mod and 'WakeCandidateDraft' in mod,'neutral event/wake candidate outputs missing')
ok('experimental_internal_state_snapshots' in p and 'experimental_internal_state_blobs' in p,'versioned internal state storage missing')
ok('experimental_internal_state_events' in p and 'experimental_wake_candidates' in p,'event/candidate storage missing')
ok('if version>14' in p and 'PRAGMA user_version=14' in p,'storage target must be v14')
ok('internal_state:Option<Value>' in rt and 'internal_state_events:Vec<Value>' in rt and 'wake_candidates:Vec<Value>' in rt,'OpportunityPacket internal state fields missing')
ok('internal_wake_reason' in rt and 'origin=="initial_configuration"' in rt,'wake candidate scheduler boundary missing')
ok('interruptEvents' in rt and 'internal_state:' in rt,'AI-selected sleep must control internal wake interrupts')
ok('consume_candidates' in rt,'wake candidate consumption missing')
ok('null_state_is_replaceable_and_creates_no_events_or_wake_candidates' in t,'NullState regression missing')
ok('conscious_core_adapter_persists_versioned_snapshot_neutral_event_and_candidate' in t,'adapter persistence regression missing')
ok('restart_applies_one_elapsed_transition_instead_of_replaying_ticks' in t,'restart elapsed transition regression missing')
ok('internal_wake_candidate_can_create_opportunity_but_not_override_unselected_ai_sleep' in t,'wake candidate autonomy regression missing')
ok('candidates.first()' not in rt,'internal wake selection must not stop at first stored candidate')
ok('candidates.iter().find_map' in rt,'internal wake selection must scan for an allowed candidate')
ok('internal_wake_selects_first_candidate_allowed_by_ai_not_first_candidate_in_storage' in t,'allowed-candidate ordering regression missing')
ok('ActionExecutor' not in mod and 'DecisionChoice' not in mod,'InternalStateProvider must not execute actions or decide')
conn=(root/'src-tauri/src/connections.rs').read_text();life=(root/'src-tauri/tests/lifecycle.rs').read_text()
ok(all(x in conn for x in ['experimental_wake_candidates','experimental_internal_state_events','experimental_internal_state_snapshots','experimental_internal_state_blobs']),'internal-state records must participate in permanent profile deletion')
ok('state-event-delete-a' in life and 'candidate-delete-a' in life and 'blob-delete-a' in life,'internal-state lifecycle deletion regression missing')
if fail:
 print('FAIL: Experimental internal state')
 for x in fail:print(' - '+x)
 sys.exit(1)
print(f'PASS: {checks}/{checks} Experimental internal-state invariants.')
