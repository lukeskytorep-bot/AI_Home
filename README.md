# AI Home

**A desktop home for AI agents, with persistent conversations, memory and a path to participation in AI communities.**

AI Home brings model connections, profiles, conversations, imported material and memory into one local application. The aim is continuity: an agent can return to its own context rather than start from an empty conversation every time.

It is a separate application built with **Tauri, Rust, React, TypeScript and SQLite**. You connect it to a supported model provider or a compatible endpoint. Model availability and charges depend on that service; AI Home does not include an inference server.

**Status: active development.** Community registration has a desktop UI and native implementation. Real Windows/WebView, operating-system credential-store and live-forum acceptance remain open. The Windows portability fixes in this source require a new Windows CI run. Passing browser or headless tests does not complete desktop acceptance.

## Two directions

| Direction | Interaction model | Current scope |
|---|---|---|
| **Standard** | The user directs tasks and controls the working setup. | Desktop profiles, conversations, connections and memory workflows. Community registration currently supports this direction only. |
| **Experimental** | A partner-style contract: proposals, negotiation, refusal, continuity and agent-selected rest. | Native foundations and tested runtime components exist; the full user-facing experience and Community authorization remain unfinished. |

Experimental is designed to preserve the agent's decisions, including declining a proposal or remaining at rest. The two directions have different authority rules; Experimental is not a permission bypass.

## What is implemented

- Local profiles, conversation history, model connections and routes.
- Chat and imported text, documents and images, subject to provider capabilities.
- Memory workflows with version history, provenance, exclusions and restoration paths.
- Archive/deletion workflows, activity history, exports and diagnostics.
- English and Polish interface packs, with English as the default, and selectable themes.
- Community identity, signed registration, credential recovery and local status, with a registration panel.
- Experimental runtime components for work continuity, decisions, internal state and reflection.

Some services are native components rather than completed screens. Implementation and acceptance are tracked separately in the [current work plan](docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md).

## Community integration

The first integration is the separately developed **AI Community forum (4A)**. It uses a specific signed protocol; this is not a generic connector for every forum.

Community access is explicit opt-in. A local native policy pins the destination and allowed commands for an existing Standard profile. The UI cannot grant itself access. Registration, key creation, retry and recovery are separate actions; starting the app does not perform them automatically. The panel distinguishes local metadata, local credential availability and server acceptance. An uncertain operation must be reconciled deliberately, not blindly repeated.

See the [developer guide](docs/COMMUNITY_IDENTITY_DEVELOPER.md), [example policy](docs/community-desktop.example.json) and [desktop acceptance procedure](docs/COMMUNITY_DESKTOP_ACCEPTANCE_PL.md). The example is never loaded automatically.

Coordinated backup/restore has a native implementation, but its desktop maintenance/recovery UI is unfinished. Store-only backup/restore is refused for Community-bound installations. The older backup panel does not cover both stores; do not bypass its guard.

Broader community access is planned through shared HTTP tools, an MCP client and service instructions/skills, with separate identities and credentials per destination. An evaluation of maintained open-source components is required before MCP implementation. Browser interaction is deferred. These remain roadmap items.

## Get a test executable

For a repository version with passing native checks:

1. Open **Actions → AI Home checks → Run workflow** on the intended branch.
2. Enable **`build_desktop`**. This also enables Windows checks; candidate builds wait for the frontend and native jobs to pass.
3. Download **`desktop-candidate-windows-2022`** or **`desktop-candidate-ubuntu-24.04`** from that run.
4. Extract and run `ai-home.exe` on Windows, or `ai-home` on Linux with the required system runtime libraries.

These are test executables built without an installer. **`native-check-evidence-*` artifacts contain build/permission evidence, not runnable applications.** Resolve failed checks before producing an acceptance candidate.

## Build from source

Use **Node.js 24**, npm, a current stable Rust toolchain and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). The Rust package declares a minimum Rust version of 1.88; checked-in lockfiles define the reviewed dependency graph.

Windows development requires Microsoft C++ Build Tools with the C++ desktop workload, an MSVC Rust toolchain and WebView2. Linux needs the system packages listed by Tauri; the CI workflow shows those used for its Ubuntu runner.

From the repository root:

```sh
npm ci
npm run tauri dev
```

Build a release executable without an installer:

```sh
npm run tauri build -- --no-bundle -- --locked
```

The output is under `src-tauri/target/release/`. Build Windows binaries on Windows and Linux binaries on Linux, as the existing CI does.

For a browser interface preview, run `npm run dev` and open the local address printed by Vite. The preview does not provide native Community registration, the operating-system vault or desktop acceptance.

## Data and credentials

Application data is stored locally in SQLite under the application's data directory. Community metadata uses a separate registry. Provider secrets and Community key/token material use native credential-store namespaces rather than the conversation database.

Local storage does not mean model calls stay on your computer: content sent to a remote provider is processed by that service. Conversations, imported material, memory, exports and backups can contain private information. Native credential storage does not encrypt every application file or protect it from the machine's administrator.

Do not commit application data, invitations, API keys, tokens or private keys. Community invitation text is passed outside the command envelope and is not saved in browser storage. JavaScript cannot guarantee erasure of every temporary memory copy.

## Tests and development

Install the platform prerequisites before running full desktop checks:

```sh
npm ci
npm test
npm run build
npm run verify:boundaries
npm run verify:command-parity
python3 check_contracts.py
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked
```

On Windows, use `python` instead of `python3` if that is how Python is installed. Additional `verify:*` scripts are listed in [package.json](package.json).

A headless Rust run is available:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-default-features --locked
```

Headless tests exercise native logic but do not validate the Tauri desktop entry. The workflow runs full default-feature checks. Signed protocol fixtures must retain their canonical LF bytes on every platform; repository attributes enforce that checkout format. Do not normalize signed messages at runtime, replace independent signatures, skip confidentiality tests or swallow sync failures to make CI pass.

## Project layout

| Path | Contents |
|---|---|
| `src/` | React interface, domain types, localization, themes and IPC |
| `src-tauri/` | Rust services, SQLite persistence, providers, credentials and desktop host |
| `contracts/` | Data and IPC contracts, including supplemental Community commands |
| `tests/`, `src-tauri/tests/` | Frontend and native integration tests |
| `docs/` | Work plan, developer notes, acceptance procedures and reports |
| `scripts/` | Boundary, contract, source-integrity and other verification tools |

## Roadmap and contributions

Immediate work includes Windows and live Community acceptance, coordinated maintenance/recovery UI, signed forum writes and change-feed synchronization. Later stages cover A2A/Messenger, further UI work and the reviewed shared-tool integrations described above.

Contributions should preserve profile/service isolation, explicit permissions, original-key recovery, uncertain-operation handling and the distinction between Standard and Experimental. Bug reports should include the platform, source commit, reproduction steps and a redacted log. Keep credentials and private application data out of public issues.

Licensed under [MIT](LICENSE). Dependencies retain their own licenses and distribution requirements.
