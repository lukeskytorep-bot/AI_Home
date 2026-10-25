# AI Home — 4B-9 EXP-1 AUDIT FIX 2 — raport

Data: 25.10.2026

## Zakres

Poprawka obejmuje wyłącznie F1 z reaudytu EXP-1. Nie rozszerza zakresu do EXP-2, nie dodaje live IPC, desktopowego schedulera, prawdziwego LLM, MCP, forum, Messenger, A2A ani ConsciousCore.

## F1 — granica po skutku zewnętrznym

`run_due()` sprawdza teraz trwały Action Ledger przed claimem. Każdy `ActionRecord` w stanie `sent` lub `uncertain` blokuje nową decyzję i zwraca `ReconciliationRequired`. Ten ledger jest autorytatywną barierą również wtedy, gdy zapis pomocniczego statusu instancji `recovery_required` sam się nie powiedzie.

Po wejściu w `ActionExecutor` błędy nie są już zamieniane na zwykły retry decyzji. Jeżeli po możliwym skutku zewnętrznym wystąpi błąd utrwalenia receipt, UPDATE ActionRecord, wymiany WakePlan, `finish_cycle` albo commit, pozostawiony trwały `sent`/`uncertain` blokuje przyszły claim. Runtime zachowuje oryginalny kod błędu zamiast przepisywać go na `INVALID_DECISION`.

`ActionOutcome::Uncertain` oraz `Accepted` z pustym receipt zapisują `uncertain` i przełączają instancję do `recovery_required` w tej samej transakcji. Nie powstaje nowy idempotency key i nie ma blind retry.

Dodano `RunResult::ReconciliationRequired`. `claim()` nie przejmuje instancji `recovery_required`.

## Regresje

Dodano dwa natywne fault-injection testy SQLite:

- poprawny skutek executora + wymuszona awaria UPDATE wyniku akcji; następne `run_due()` nie wykonuje akcji ponownie, a ledger pozostaje `sent`;
- poprawny skutek executora + wymuszona awaria końcowego `finish_cycle`; po restarcie akcja nadal nie jest wykonywana drugi raz przed reconciliation.

Oba testy sprawdzają również, że licznik execute pozostaje równy 1 i DecisionProvider nie otrzymuje nowej okazji.

## Kontrole lokalne

- P2 reference checks: 193/193 PASS
- boundaries: 31/31 PASS
- command parity: 54/54 PASS
- Standard closure: 8/8 PASS
- Standard release: 8/8 PASS
- Experimental foundation: 12/12 PASS
- Experimental runtime: 24/24 PASS
- Rust source-text: PASS
- SQLite target: v13
- Frozen P2: unchanged
- Cargo: NOT RUN locally
- Full Vitest/typecheck/Vite: require CI

## Następny krok

Po zielonym CI i reaudytcie można zamknąć EXP-1 i rozpocząć EXP-2 Work Continuity / context rollover.
