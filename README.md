> CI update (2B-5.1 fix 1): native build-script linking failed on GitHub (missing writeable/base64 rlib). Target artifact caching has been removed; downloaded dependency caching is retained under a new key. A fresh commit and cold native CI are required. The cache diagnosis is provisional, and this change does not claim a successful remote build. See `docs/AI_HOME_4B12_STEP_2B5_1_CI_FIX_1_PL.md`; canonical plan content is now 1.81.

# AI Home — 4B-12 step 2B-5.1: coordinated profile deletion

The opt-in native CommunityLifecycle service connects the existing profile.delete.permanent dispatcher to both databases and Community key/token cleanup. A durable app intent precedes cleanup; success receipts are committed only with the final app-data deletion. Vault/final-transaction failures remain explicitly resumable without generating new keys or making HTTP calls. Experimental still requires inactive instances; there is no owner override.

Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Edward confirmed the green 2B-4 baseline. App Store migrates 15 -> 16; Community registry stays 4. The production desktop is **not connected** to the new seam; registration UI/IPC remain disabled. Coordinated backup/restore is next in **2B-5.2**, followed by direction/capability checks and registration IPC in **2B-5.3**. The whole 2B-5 and 4B-12 remain open.

The native host is pinned to the original protected registry location. A wrong registry or detached host cannot bypass deletion. Until coordinated backup exists, an attached/pinned Community host refuses Store-only backup/restore. Default, unbound Standard backups remain functional. Do not enable this native seam in production before the next integration substeps.

Local verification: **563 Rust tests** (including 13 new integration cases), **219 frontend tests**, TypeScript/build, 193 P2 references, 34 boundary checks and 54 command-parity checks pass. These are Linux headless/fake-vault results; Windows/Tauri/WebView, real keyring and live-forum acceptance remain separate. No release/installers are needed for this handoff.

Report (PL): `docs/AI_HOME_4B12_STEP_2B5_1_PROFILE_DELETION_PL.md`. Developer guide (EN): `docs/COMMUNITY_IDENTITY_DEVELOPER.md`. Canonical plan content 1.80: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md`. Evidence: `docs/evidence/4b12-step-2b5-1/`.

Dependencies, Cargo.lock, frozen P2, providers, frontend, permissions and workflows are unchanged from the exact 2B-4 baseline. Apply the source overlay at the repository root and run the normal CI workflow.
