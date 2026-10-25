# AI Home — 4B-9 EXP-1 AUDIT FIX — raport

Data: 24.10.2026

## Zakres

Poprawka obejmuje wyłącznie R1–R4 z audytu głównego EXP-1. Nie uruchamia live IPC, desktopowego schedulera, prawdziwego LLM, MCP, forum, Messenger, A2A ani ConsciousCore.

## R1 — committed event cursor

`claim()` odczytuje trwały `ExperimentalEventCursor` i do wake qualification oraz OpportunityPacket przepuszcza tylko eventy z `sequence > committedThrough`. Powtórne dostarczenie już commitowanego sequence nie tworzy kolejnego decision cycle. `receivedThrough` może rosnąć niezależnie; commit następuje dopiero po durable finish cycle.

Dodane regresje obejmują WAIT_EVENT, nowy sequence oraz restart podczas AI-selected sleep.

## R2 — finalizacja błędów po claimie

Błędne dane od DecisionProvider nie pozostawiają instancji jako `running`. Runtime ma chronioną generacją finalizację techniczną i retry gate dla błędów przed skutkiem zewnętrznym. Po możliwym skutku zewnętrznym nie wykonuje blind retry: `Accepted` bez receipt jest zapisywane jako `uncertain`, cykl jest domykany i zwracany jest `ACTION_OUTCOME_UNCERTAIN`.

Regresje: Sleep(0), WaitEvent([]), brak WorkEpisode, niepoprawny ActionIntent oraz pusty receipt po execute.

## R3 — resource retry gate

`RESOURCE_UNAVAILABLE` zapisuje na ExperimentalInstance techniczne pola `retryNotBefore`, `retryAttempt`, `lastTechnicalError`. Backoff jest ograniczony i nie zmienia WakePlan AI. Dziesięć kolejnych `run_due` w tej samej chwili nie generuje dziesięciu provider calls.

## R4 — direction-bound profile

`create_instance` nie przyjmuje istniejącego profilu. Atomowo tworzy nowy profil i ExperimentalInstance/WakePlan/EventCursor. Existing Standard profile daje `POLICY_DENIED`. Standard profile listing filtruje profile powiązane z Experimental, a standardowe profile-scoped workflow odrzuca je przed mutacją. Frozen Profile DTO nie został zmieniony; `mode=conscious` pozostaje wyłącznie legacy frozen discriminator, a kierunek produktu jest autorytatywnie przechowywany jako `ExperimentalInstance.direction=experimental`.

## Kontrole lokalne

- P2 reference checks: 193/193 PASS
- boundaries: 31/31 PASS
- command parity: 54/54 PASS
- Standard closure: 8/8 PASS
- Standard release: 8/8 PASS
- Experimental foundation: 12/12 PASS
- Experimental runtime: 20/20 PASS
- Rust source-text: PASS
- SQLite target: v13
- Frozen P2: unchanged
- verify-source: 635 manifest hashes / 636 SHA256SUMS entries PASS
- Overlay: 24 changed/added, 0 deleted; BASE + overlay = SOURCE byte-for-byte
- Cargo: NOT RUN locally
- Full Vitest/typecheck/Vite: do CI

## Następny krok

Po zielonym CI i reaudytcie EXP-1 można rozpocząć EXP-2 Work Continuity / context rollover.
