#!/usr/bin/env python3
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[1]
fail=[];checks=0
def ok(cond,msg):
 global checks;checks+=1
 if not cond:fail.append(msg)
mod=(root/'src-tauri/src/experimental_continuity.rs').read_text()
t=(root/'src-tauri/tests/experimental_continuity.rs').read_text()
lib=(root/'src-tauri/src/lib.rs').read_text()
ok('pub mod experimental_continuity;' in lib,'EXP-2 continuity module must be registered')
ok('pub struct WorkContinuityService' in mod and 'create_episode' in mod,'WorkEpisode service missing')
ok('write_technical_checkpoint' in mod and 'write_semantic_checkpoint' in mod,'two checkpoint paths missing')
ok('parentCheckpointId' in mod and 'REVISION_CONFLICT' in mod,'checkpoint CAS/revision fence missing')
ok('sourceEventSequence' in mod and 'committedThrough' in mod,'technical checkpoint must be bounded by committed event cursor')
ok('source_event_sequence<*parent_sequence' in mod,'technical checkpoint source sequence must be monotonic relative to parent')
ok('pub struct ResumePacket' in mod and 'build_resume_packet' in mod,'Resume Packet builder missing')
ok("json_extract(body,'$.state') IN ('sent','uncertain')" in mod,'Resume Packet must surface unresolved external actions')
ok('caller_estimated_tokens' in mod and 'CheckpointRequired' in mod and 'Impossible' in mod,'context pressure preflight semantics missing')
ok('checkpoint_reserve_tokens' in mod and 'reserved_output_tokens' in mod and 'input_reserve_tokens' in mod,'preflight must reserve output/input/checkpoint room')
ok('route_change_requires_fresh_preflight_against_new_context_window' in t,'route/model context re-preflight regression missing')
ok('restart_rebuilds_resume_packet_from_durable_work_state' in t,'restart Resume Packet regression missing')
ok('empty_newer_episode_does_not_hide_older_checkpointed_work_after_restart' in t and 'EXISTS (SELECT 1 FROM experimental_technical_checkpoints' in mod,'latest Resume Packet must skip empty newer episodes')
ok('technical_checkpoint_source_sequence_is_monotonic_and_cannot_hide_semantic_state' in t,'monotonic source event sequence regression missing')
ok('decision_opportunity_receives_resume_packet_when_active_work_has_checkpoint' in t,'runtime decision packet must receive active work Resume Packet')
ok('stale_semantic_checkpoint_cannot_overwrite_newer_technical_work_state' in t,'stale semantic checkpoint regression missing')
ok('technical_checkpoint_parent_is_compare_and_swap_fence' in t,'technical checkpoint CAS regression missing')
ok('technical_and_semantic_checkpoints_build_resume_packet_without_hidden_reasoning' in t,'rollover/Resume Packet regression missing')
ok('resume_packet_exposes_unresolved_external_action_instead_of_hiding_it' in t,'uncertain action Resume Packet regression missing')
ok('rawPayload' not in mod and 'payload_bytes' not in mod,'Work Continuity must not become raw action/tool payload shadow storage')
ok('tool_result' in t and 'sourceRefs' in mod,'large tool result must be represented by source reference')
delmod=(root/'src-tauri/src/deletion.rs').read_text();conv=(root/'src-tauri/src/conversations.rs').read_text()
ok('redact_experimental_source_refs' in delmod and 'permanently_deleted' in delmod and 'redact_experimental_source_refs' in conv,'checkpoint source refs must participate in permanent-deletion lifecycle')
ok('experimental_checkpoint_source_refs_are_tombstoned_on_source_deletion' in delmod,'permanent-deletion native regression for checkpoint refs missing')
if fail:
 print('FAIL: Experimental continuity')
 for x in fail: print(' - '+x)
 sys.exit(1)
print(f'PASS: {checks}/{checks} Experimental continuity invariants.')
