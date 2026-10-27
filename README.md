# AI Home — 4B-12 step 2A: Community identity preparation

Native P-256 dual-key custody, exact 4A registration packets and validated signing. This prepares registration; it does not perform HTTP registration or accept a forum token. Dedicated OS-vault namespace, explicit reserve/provision/load/remove and no provider-secret fallback.

Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Step 2 and all of 4B-12 remain open. Edward confirmed the green step 1 audit fix baseline. Next: 2B durable registry, real registration/recovery HTTP, token custody and application lifecycle wiring.

Report (PL): `docs/AI_HOME_4B12_STEP_2A_IDENTITY_PL.md`. Developer guide (EN): `docs/COMMUNITY_IDENTITY_DEVELOPER.md`. Canonical plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` (content 1.75). Evidence: `docs/evidence/4b12-step-2a/`.

SQLite stays v15; frozen P2, provider vault, model adapters, permissions and workflows are unchanged. Pinned RustCrypto dependencies and Cargo.lock additions are documented. Desktop/real OS vault/live forum acceptance remains separate. Source handoff only, no release or installers.
