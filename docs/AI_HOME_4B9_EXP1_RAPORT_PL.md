# AI Home — 4B-9 EXP-1 — raport wykonawczy

Data: 24.10.2026

## Zakres

EXP-1 uruchamia pierwszy działający fundament Experimental na `NullState`, ale wyłącznie z deterministycznym Clock i wstrzykiwanym fake DecisionProvider/ActionExecutor. Nie dodaje background schedulera desktopowego ani prawdziwego model/provider call.

## Wdrożone

- `src-tauri/src/experimental_runtime.rs` z granicami `Clock`, `DecisionProvider`, `ActionExecutor`.
- trwałe tworzenie Experimental instance + początkowego WakePlan i EventCursor.
- due-opportunity dla interval/deadline/event_only.
- decyzje: ACTION, NO ACTION, REST, SLEEP, WAIT_EVENT, REFUSAL oraz podstawowe DEFER/ABANDON/CONTINUE/REVISE.
- AI-selected sleep i event-only nie są nadpisywane przypadkowym zdarzeniem użytkownika.
- aktywne odmowy są przekazywane do następnego OpportunityPacket.
- fake local peer zapisuje ActionRecord z payload hash/ref i receipt, bez raw payload w storage.
- runtimeGeneration + CAS chronią przed stale executor.
- restart recovery: `running -> interrupted`, `sent -> uncertain`.
- missed wake/interval jest scalany do jednej okazji.
- resource unavailable nie jest zapisywane jako refusal/decision AI.
- `verify:experimental-runtime` jako stała bramka 12/12.

## Świadome granice

- brak live IPC Experimental;
- brak host background timer;
- brak realnego provider/model call;
- brak ConsciousCore;
- brak MCP/Community/Messenger/A2A;
- Work Continuity context rollover i Context Compiler pozostają na EXP-2.

## Weryfikacja lokalna

- P2 reference: 193/193 PASS
- boundaries: 31/31 PASS
- command parity: 54/54 PASS
- Standard closure: 8/8 PASS
- Standard release: 8/8 PASS
- Experimental foundation: 12/12 PASS
- Experimental runtime: 12/12 PASS
- Rust source-text: PASS
- SQLite: v13
- Cargo: niewykonane lokalnie — brak toolchainu
- Vitest/typecheck/Vite: niewykonane lokalnie — brak node_modules

Pełne Cargo i frontend CI są wymagane przed odbiorem EXP-1.
