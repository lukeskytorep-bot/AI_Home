#!/usr/bin/env python3
"""Static guard for 4B-12/13 MCP client isolation and safety invariants (not live server proof)."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
mcp = (root / 'src-tauri/src/mcp_client.rs').read_text(encoding='utf-8')
kernel = (root / 'src-tauri/src/kernel.rs').read_text(encoding='utf-8')
refl = (root / 'src-tauri/src/experimental_reflection.rs').read_text(encoding='utf-8')
tests = (root / 'src-tauri/tests/mcp_client.rs').read_text(encoding='utf-8')

checks = [
    ('internal port trait isolation', 'pub trait McpClientPort' in mcp and 'mcp_client' not in kernel),
    ('no MCP dependency in reflection module', 'mcp_client' not in refl),
    ('redirects explicitly disabled', 'RedirectPolicy::none()' in mcp and 'RedirectBlocked' in mcp),
    ('per-profile per-service Vault scoping', 'fn vault_scope' in mcp and 'mcp-service:' in mcp),
    ('no self-grant metadata allowed', 'contains_self_grant_keys' in mcp and 'untrusted_external_metadata: true' in mcp),
    ('private reflection sources blocked', 'experimental_private_reflections' in mcp and 'experimental_self_records' in mcp),
    ('uncertain mutating call maps to ActionOutcome::Uncertain', 'ActionOutcome::Uncertain' in mcp),
    ('no shell/process spawning in prototype', 'std::process' not in mcp and 'tokio::process' not in mcp),
    ('acceptance test coverage (all 6 scenarios)', all(name in tests for name in [
        'mcp_streamable_http_initialize_list_and_call_tool_succeeds_against_local_fake_server',
        'mcp_isolates_vault_credentials_between_two_profiles_and_two_services',
        'mcp_rejects_self_grant_from_tool_metadata_and_enforces_allowlist_and_read_only',
        'mcp_blocks_http_redirect_so_secret_never_leaks_to_foreign_host',
        'mcp_mutating_disconnect_records_uncertain_and_blocks_restart_until_reconciled',
        'mcp_respects_autonomous_refusal_and_sleep_and_blocks_private_reflection_publication',
    ])),
]

failed = [label for label, ok in checks if not ok]
if failed:
    for label in failed:
        print(f'FAIL: {label}')
    sys.exit(1)
print(f'PASS: {len(checks)}/{len(checks)} MCP client static isolation guards (prototype/fake only).')
