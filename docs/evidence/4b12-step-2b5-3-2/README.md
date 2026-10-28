# 4B-12 / 2B-5.3.2 evidence

This directory records the local Linux/headless verification for the supplemental native registration adapter.

- `rust-all.log`: all Rust targets with no default features, locked and offline.
- `local-tests.log`: the 18 adapter regressions.
- `frontend.log` and `build.log`: unchanged frontend regression and production build.
- `contracts.log`: frozen P2 reference checks.
- `static-checks.log`: boundary, migration, parity and existing scope guards.
- `tested-source-hashes.json`: hashes of the code and contract exercised by the run.
- `verification-summary.json`: machine-readable scope and exclusions.
- `base-and-protection.json`: exact input archive and protected-surface statement.

The tests use real SQLite files and a deterministic secret-store test backend. They do not constitute acceptance of the production desktop, OS keyring, Windows/WebView or a live forum/TLS deployment.
