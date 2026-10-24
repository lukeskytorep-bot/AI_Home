# AI Home — 4B-9 EXP-0 — raport implementacyjny

Data: 24.10.2026.

## Zakres

Pierwszy krok Experimental jest celowo kontraktowy. Zamrożono minimalny model danych i trwałość potrzebne później do scheduler/decision runtime, bez uruchamiania modelu i bez konektorów społecznych.

## Wykonano

- supplemental `experimental.schema.json`, bez zmiany frozen P2;
- DTO: ExperimentalInstance, WakePlan, DecisionRecord, RefusalRecord, WorkEpisode, TechnicalWorkCheckpoint, SemanticWorkCheckpoint, ActionRecord, ExperimentalEventCursor, ExperimentalSourceRef;
- SQLite schema v13 i 9 tabel `experimental_*`;
- v12 -> v13 migration regression zachowujący istniejący Standard profile;
- TS/Rust schema validators rozszerzone o supplemental definitions;
- testy contracts/privacy shape w TypeScript i Rust;
- stała bramka `verify:experimental-foundation` 10/10;
- repozytoryjny kanon `docs/AI_HOME_EXPERIMENTAL_KANON_v1.0_PL.md`;
- sweep docelowych oczekiwań `PRAGMA user_version` do v13.

## Granice

Nie dodano live schedulera, Primary calls, ExperimentalDecisionService, start/impulse IPC, ConsciousCore, MCP, Community, Messenger ani A2A. Nie ma jeszcze UI Experimental runtime. To zakres następnych podkroków po akceptacji EXP-0.

## Lokalne bramki wykonane

- P2 reference checks: 193/193 PASS.
- boundaries: 31/31 PASS.
- command parity: 54/54 PASS.
- Standard closure: 8/8 PASS.
- Standard release: 8/8 PASS.
- Rust source-text: PASS.
- Experimental foundation: 10/10 PASS.

Cargo/Vitest/typecheck/Vite nie były możliwe lokalnie w tym środowisku: brak Rust toolchain i brak `node_modules`. Wymagane pełne GitHub Actions.

## Finalny handoff lokalny

- verify-source: 589 manifest hashes / 590 SHA256SUMS entries PASS.
- SOURCE tree: 591 plików.
- overlay względem dokładnego zielonego `AI_HOME_4B8_AUDIT_FIX_2_SOURCE.zip`: 39 zmienionych/dodanych, 0 usuniętych.
- rekonstrukcja BASE + overlay = SOURCE: byte-for-byte PASS.
- ZIP CRC: PASS.
