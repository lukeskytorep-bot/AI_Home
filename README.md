# AI Home — 4B-12 / 2B-5.2: two-store offline backup and restore

OfflineCommunityBackup captures the application Store and sanitized Community registry as one verified bundle. Restore uses a durable safety/journal flow, preserves newer Community generations and deletion tombstones, prunes locally deleted profiles from the staged app DB, and never exports vault secrets or generates identity keys. Imported metadata requires explicit reconciliation.

Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Edward confirmed the green 2B-5.1 CI fix 1 baseline. Store stays **16**, Community registry stays **4**; dependencies, lockfiles, workflow, providers, frontend, frozen P2 and permissions are unchanged.

This is an **offline native API**. All Store-derived pools must be closed/dropped; shared OS ownership prevents maintenance against a running Store. Interrupted restore blocks Store startup and normal Community operations until explicit recovery. A committed receipt remains readable without the input bundle, and finish_cleanup can complete housekeeping without reapplying data.

Production desktop registration and backup/recovery routing are **not enabled**. Existing app commands still refuse a Store-only backup for a Community-bound host. Next is **2B-5.3**: maintenance/recovery bootstrap, direction/capability checks and registration IPC before desktop enablement. The whole 2B-5 and 4B-12 remain open.

Local checks: **585 Rust tests** (22 new tests including two subprocess helpers), **219 frontend tests**, TypeScript/Vite build, 193 P2 references, 34 boundary checks and 54 command-parity checks pass. These are Linux headless/fake-vault results; Windows/Tauri/WebView, real keyring, live forum and power-loss acceptance remain separate. No release or installers are needed.

Report (PL): `docs/AI_HOME_4B12_STEP_2B5_2_OFFLINE_BACKUP_PL.md`. Developer guide (EN): `docs/COMMUNITY_IDENTITY_DEVELOPER.md`. Canonical plan content **1.82**: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md`. Evidence: `docs/evidence/4b12-step-2b5-2/`.
