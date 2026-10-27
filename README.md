# AI Home — 4B-12 step 2B-3: token-safe Community completion and recovery

Explicit native registration/complete, durable signed packets, separate immutable OS-vault token candidates and proof-status/token-reissue recovery. Lost responses preserve the original identity. Operation revisions and server generation CAS prevent late candidates from replacing newer active credentials.

Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Edward confirmed the green step 2B-2 baseline. Registry schema 2 upgrades to 3; app Store remains v15. Token readiness is checked through the vault, never inferred from a complete HTTP receipt alone. No registration is automatically invoked and no UI/IPC is enabled.

Report (PL): `docs/AI_HOME_4B12_STEP_2B3_COMPLETE_RECOVERY_PL.md`. Developer guide (EN): `docs/COMMUNITY_IDENTITY_DEVELOPER.md`. Canonical plan content 1.78: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md`. Evidence: `docs/evidence/4b12-step-2b3/`.

Cargo dependencies/lockfile, frozen P2, provider vault/adapters, permissions and workflows are unchanged. Next: application profile lifecycle/deletion, registry/vault backup-restore reconciliation, host direction/capability checks and IPC before end-user enablement. Signed forum content writes/feed remain later in 4B-12. Real OS vault, PHP/live forum/TLS and desktop acceptance remain separate. Source handoff only, no release or installers.
