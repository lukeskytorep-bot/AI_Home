# AI Home — 4B-0 foundation

**Source candidate for audit. The React frontend builds and its tests pass. Native Rust/Tauri compilation and WebView verification are pending. This is not a desktop release.**

AI Home is independent of the forum and AI RV Harness. English is the default; Polish is available. This foundation implements application preferences, a kernel in Standard/NullState mode, typed IPC, locale/theme validation, and an application event journal. It does not call AI providers. Profiles, chat, file imports, memories and Conscious are later stages.

## Run the browser preview

Use Node.js 24 and npm. From this directory:

```sh
npm ci
npm run dev
```

Open http://127.0.0.1:1420. The preview is explicitly labeled and uses its own localStorage namespace. It does not test SQLite, desktop permissions or the WebView. Language and theme changes are previews until **Save settings** is clicked.

```sh
npm test
npm run verify:boundaries
npm run build
```

## Run the desktop candidate

Install Rust stable, the platform Tauri 2 prerequisites and Node.js 24. See https://v2.tauri.app/start/prerequisites/ . On Windows, Rust/MSVC and WebView2 are required. On Linux, WebKitGTK and related build libraries are required.

```sh
npm ci
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
npm run tauri dev
npm run tauri build -- --no-bundle
```

The first Cargo run resolves transitive dependencies and generates `src-tauri/Cargo.lock`. Direct versions come from the audited Harness snapshot; **the Rust lockfile could not be resolved in the preparation environment**. Preserve and commit the generated lockfile after a successful native run, then switch CI to `--locked`. The npm lockfile is included and was used in the verified build.

The native candidate stores `foundation.sqlite3` in Tauri's application data directory. A SQLite transaction saves preferences, idempotency receipt and journal event together. Desktop errors do not cause fallback to browser storage. No API credentials are accepted in this stage.

## Check before closing 4B-0

The provided GitHub Actions workflow tests the frontend, runs headless Rust tests, and compiles the desktop candidate on Windows and Linux. It has not been run yet. This directory can become a GitHub repository now; no repository was remotely created or published during preparation.

Complete the checklist in `docs/ACCEPTANCE_PL.md`, retain the CI logs and Rust lockfile, and audit the resulting code. Native compilation alone does not confirm rendering, keyboard behavior or persistence in the actual WebView.

Polski: szczegóły modułów, poleceń i ograniczeń znajdują się w `docs/ARCHITECTURE_PL.md`. Raport przekazania i aktualny plan są w `docs/`. Pakiety językowe i skórek są zwykłym JSON-em, bez wykonywalnych dodatków. Walidacja pakietów jest gotowa; instalator pakietów użytkownika należy do późniejszego etapu.
