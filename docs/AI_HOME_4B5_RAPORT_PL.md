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

## AUDIT FIX 4 — poprawki po pełnym CI

Pełne CI ujawniło dwa pozostałe problemy niezależne od F1/F2:

1. Rust/Cargo: `profile_imports.rs` wywoływał niedostępny prywatny helper `validate()`. Odczyt `profile.import.latest` waliduje teraz zapisany obiekt przez publicznie używany w projekcie `schema::dto("Operation", &op)` oraz sprawdza zgodność `profileId`.
2. Vitest: dwa testy kandydatur pamięci używały nieaktualnych angielskich `aria-label` (`Choose memory record`, `Reason for saving or correction`). Aktualne locale to `Memory record for candidate review` oraz `Reason for memory review/correction`; fixture testowe zostały zaktualizowane bez zmiany UI/runtime.

Dodatkowo znormalizowano `docs/evidence/4b5/status.json` do `4B-5 REBASE AUDIT FIX 4`, planu 1.37 i jawnie zachowano wcześniejszy log FIX 2/FIX 3 jako historię evidence.

Zakres F1/F2 nie został zmieniony. 4B-5 nadal wymaga nowego pełnego zielonego CI przed `PASS CLOSED`.

## AUDIT FIX 5 — korekta dwóch testów Cargo po FIX 4

Pełny `cargo test --all-targets --locked` po FIX 4 skompilował projekt, a następnie ujawnił dwa błędy testowe w `src-tauri/tests/chat.rs`:

1. `actual_v4_database_migrates_without_changing_profile_or_preferences` nadal oczekiwał `PRAGMA user_version == 11`. 4B-5 wprowadza migrację v11→v12, więc aktualnym prawidłowym wynikiem po otwarciu starej bazy jest 12. Oczekiwanie testu zostało zaktualizowane bez zmiany migracji.
2. `checkpoint_excerpt_prioritizes_newest_turns_over_older_very_long_turn` tworzył bardzo długą pierwszą turę i dwie bardzo krótkie kolejne. Przy `contextTokens=16384` pierwszy segment pozostawał poniżej progu po pierwszej turze, ale dwie krótkie tury również nie gwarantowały przekroczenia progu. Test szukał więc newest markerów w sealed segmencie, którego mógł nie być. Obie najnowsze tury mają teraz kontrolowane dopełnienie 250 znaków: po pierwszej nadal pozostaje poniżej progu, a po drugiej ten sam segment przekracza próg i zostaje zapieczętowany.

Nie zmieniono runtime F1/F2, algorytmu budowania summary, kontraktów, SQLite ani UI. FIX 5 wymaga ponownego pełnego CI.
