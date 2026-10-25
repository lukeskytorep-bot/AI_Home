# AI Home — 4B-9 EXP-1 AUDIT FIX 3 — raport PL

Data: 25.10.2026

## Powód
GitHub Actions po AUDIT FIX 2 uruchomił natywne testy Rust i wykazał dwa FAIL-e. Oba były starymi oczekiwaniami testów sprzed poprawki F1, nie regresją produkcyjnego runtime.

## Zmiana
- `accepted_without_receipt...`: oczekiwany status zmieniono z `ready` na `recovery_required`; dodano sprawdzenie `RunResult::ReconciliationRequired` i że executor nadal ma dokładnie jedno wywołanie.
- `restart_recovers_running_cycle_and_sent_action_without_fabricating_outcome`: po restarcie `sent -> uncertain`; kolejny `run_due()` ma zwrócić `ReconciliationRequired`, nie `Completed`; `DecisionProvider` musi mieć 0 nowych wywołań, a instance przejść do `recovery_required`.
- Nazwę regresji pustego receipt doprecyzowano do `accepted_without_receipt_becomes_uncertain_and_requires_reconciliation`; static verifier zsynchronizowano.

## Bez zmian
Nie zmieniono `experimental_runtime.rs`, kontraktów, SQLite schema, frozen P2, Standard, live IPC, schedulera desktopowego, ConsciousCore, MCP ani konektorów.

## Lokalne kontrole
- P2 193/193 PASS
- boundaries 31/31 PASS
- command parity 54/54 PASS
- Standard closure 8/8 PASS
- Standard release 8/8 PASS
- Experimental foundation 12/12 PASS
- Experimental runtime 24/24 PASS
- Rust source-text PASS
- Cargo niedostępne lokalnie; wymagany pełny GitHub Actions.
