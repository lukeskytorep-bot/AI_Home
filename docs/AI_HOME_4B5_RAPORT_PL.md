# AI Home — 4B-5 REBASE AUDIT FIX 3 — raport

**Data:** 22 października 2026  
**Baza:** `AI_HOME_4B5_REBASE_AUDIT_FIX_2_SOURCE.zip`  
**Zakres:** wyłącznie korekta trzech błędów TypeScript ujawnionych przez `npm run build`.

## Poprawione błędy

1. `src/ui/AttachmentsPanel.tsx`: `Operation` nie istniał w tym module. Wynik `material.import` używa teraz istniejącego `ImportOperation`.
2. `tests/imports-ui.test.tsx`: `useRef<CandidatePending>()` zastąpiono przez `useRef<CandidatePending | undefined>(undefined)`, zgodnie z aktualną sygnaturą React/TypeScript.
3. Ta sama zmiana usuwa błąd przypisania `CandidatePending | undefined` do zbyt wąskiego refa.

Nie zmieniono runtime segmentacji, F1/F2, kontraktów IPC, SQLite, providerów ani zachowania importów.

## Weryfikacja lokalna

- `python3 check_contracts.py`: do ponownego uruchomienia po spakowaniu.
- `node scripts/verify-boundaries.mjs`: do ponownego uruchomienia po spakowaniu.
- `python3 scripts/verify-source.py .`: do ponownego uruchomienia po regeneracji manifestu.
- `npm run build`: **NOT RUN lokalnie**, brak lokalnego `node_modules/.bin/tsc`; pełne potwierdzenie należy do CI.

## Status

`IMPLEMENTED / REQUIRES CI`. 4B-5 nie jest `PASS CLOSED` do czasu zielonego pełnego CI i ponownego audytu.
