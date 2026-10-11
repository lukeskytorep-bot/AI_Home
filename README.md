# AI Home P2 handoff

Read AI_HOME_P2_KONTRAKTY_IPC_I_MODULY_PL.md and the updated plan in snapshots/.

Verify the package without extracting untrusted paths:

    python3 verify_handoff.py /absolute/path/AI_HOME_P2_DOWODY.zip /absolute/path/AI_HOME_P2_DOWODY_SHA256.txt

After extracting into a private directory, run reference checks:

    python3 check_contracts.py

Generators rebuild schemas/catalog/defaults: python3 build_ipc.py. fixtures.json is a committed reference input, not generated production data.

119 reference checks passed. These are subset schema, static contract and reference SQLite checks, not application tests. No Tauri/Rust/keyring/WebView/live API tests were performed. P2 is ready for review; 4B-0 follows.

SHA256SUMS identifies every included file except itself. The complete ZIP hash is deliberately external. snapshots/ contains frozen handoff bytes, not a claim that the external P1 auditor examined these exact hashes. SOURCE_IDENTITIES.json describes provenance. The archive contains documents and reference code, not a runnable app or copied Harness runtime.
