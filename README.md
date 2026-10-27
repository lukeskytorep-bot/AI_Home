# AI Home — 4B-12 step 2B-1: durable Community preparation

Native SQLite registration registry with stable scope/start/key references, one-shot provisioning ownership, compare-and-set revisions and load-only recovery after interruption. Two P-256 keys remain in the separate Community OS-vault namespace. No registration HTTP, token acceptance, IPC or automatic activity is enabled by this step.

Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Edward confirmed the green step 2A baseline. Step 2B and all of 4B-12 remain open. Next: bounded registration HTTP, durable progress and proof-status/token recovery, followed by application lifecycle and IPC integration before end-user enablement.

Report (PL): `docs/AI_HOME_4B12_STEP_2B1_REGISTRY_PL.md`. Developer guide (EN): `docs/COMMUNITY_IDENTITY_DEVELOPER.md`. Canonical plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` (content 1.76). Evidence: `docs/evidence/4b12-step-2b1/`.

The registry explicitly opens a dedicated SQLite file (its own schema v1/application ID); no production host opens it yet. It refuses application/unrelated databases. The application's existing SQLite stays v15; frozen P2, Cargo manifests/lockfile, provider vault, adapters, permissions, tests and workflows are unchanged. Existing CI includes the new native tests automatically. Desktop/real OS vault/live forum acceptance remains separate. Source handoff only, no release or installers.
