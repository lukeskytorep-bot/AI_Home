#!/usr/bin/env python3
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[1]
fail=[];checks=0
def ok(cond,msg):
 global checks; checks+=1
 if not cond: fail.append(msg)
rt=(root/'src-tauri/src/experimental_runtime.rs').read_text()
p=(root/'src-tauri/src/persistence.rs').read_text()
t=(root/'src-tauri/tests/experimental_runtime.rs').read_text()
ok('pub trait Clock' in rt and 'pub struct SystemClock' in rt,'EXP-1 must isolate canonical clock behind Clock')
ok('pub trait DecisionProvider' in rt and 'DisabledDecisionProvider' in rt,'decision provider boundary missing')
ok('pub trait ActionExecutor' in rt and 'ActionOutcome' in rt,'action executor boundary missing')
ok('run_due' in rt and 'due_reason' in rt,'durable due-opportunity path missing')
ok('runtimeGeneration' in rt and 'STALE_EXECUTOR' in rt,'stale executor generation fence missing')
ok("'$.status','interrupted'" in p and "'$.state','uncertain'" in p,'restart recovery must distinguish interrupted cycle and uncertain send')
ok('DecisionChoice::NoAction' in rt and 'DecisionChoice::Rest' in rt and 'DecisionChoice::Sleep' in rt and 'DecisionChoice::WaitEvent' in rt and 'DecisionChoice::Refusal' in rt,'required decision outcomes missing')
ok('active_refusals' in rt and 'RefusalRecord' in rt,'active refusal must survive into later opportunity packet')
ok('rawPayload' not in rt and 'payloadHash' in rt and 'payloadRef' in rt,'runtime action path must not create raw shadow payload storage')
ok('interval_is_an_opportunity_not_a_periodic_model_call' in t and 'ai_selected_sleep_is_not_overridden_by_unselected_human_event' in t and 'restart_recovers_running_cycle_and_sent_action_without_fabricating_outcome' in t,'required EXP-1 regressions missing')
ok('experimental.start' not in rt and 'experimental.impulse' not in rt,'EXP-1 service must not expose live production IPC yet')
ok('ConsciousCore' not in rt and 'mcp' not in rt.lower() and 'community.read' not in rt,'EXP-1 must not pull later capabilities into runtime')
ok('committedThrough' in rt and 'sequence>committed' in rt,'R1: wake eligibility must ignore already committed events')
ok('release_if_running_with_retry' in rt and 'ACTION_OUTCOME_UNCERTAIN' in rt,'R2: every failed claimed cycle must finalize without fabricating external success')
ok('retryNotBefore' in rt and 'retryAttempt' in rt and 'RESOURCE_UNAVAILABLE' in rt,'R3: resource unavailability needs a persisted technical retry gate')
ok('existing_standard_profile_cannot_be_converted_to_experimental' in t and 'create_instance_atomically_creates_direction_bound_profile' in t,'R4: direction-bound profile creation regressions missing')
ok('committed_event_cannot_wake_wait_event_twice_and_new_sequence_can' in t and 'accepted_without_receipt_becomes_uncertain_and_does_not_leave_running' in t,'R1/R2 native regressions missing')
ok('resource_unavailable_uses_technical_backoff_not_ai_wake_plan' in t,'R3 retry regression missing')
conn=(root/'src-tauri/src/connections.rs').read_text()
ok('command != "profile.delete.permanent"' in conn and 'experimental_instances WHERE profile_id=?' in conn and 'POLICY_DENIED' in conn,'R4: Standard profile-scoped dispatcher must reject Experimental-bound profiles')
ok('NOT EXISTS(SELECT 1 FROM experimental_instances' in conn,'R4: Standard profile lists must exclude Experimental-bound profiles')
ok("json_extract(body,'$.state') IN ('sent','uncertain')" in rt and 'ReconciliationRequired' in rt,'F1: unresolved external action ledger must fence new decisions')
ok('status=="running"||status=="inactive"||status=="recovery_required"' in rt,'F1: recovery_required instances must not be reclaimed')
ok('post_effect_action_update_failure_blocks_new_decision_until_reconciliation' in t and 'finalization_failure_after_executor_is_fenced_across_restart' in t,'F1: post-effect storage fault regressions missing')
ok('let _=self.mark_recovery_required_if_possible' in rt and 'release_if_running_with_retry(profile_id,instance_id,generation,self.clock.now(),&code)' in rt,'F1: recovery marker must be best-effort and original error preserved')
if fail:
 print('FAIL: Experimental runtime')
 for x in fail: print(' - '+x)
 sys.exit(1)
print(f'PASS: {checks}/{checks} Experimental runtime invariants.')
