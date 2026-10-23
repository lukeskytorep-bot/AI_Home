# Instrukcja — AI Home 4B-7 Domknięcie Standard

Baza: `AI_HOME_4B6_KROK_4_AUDIT_FIX_4_SOURCE.zip`.

Nałóż `AI_HOME_4B7_CHANGED_FILES.zip` na dokładną bazę albo użyj pełnego `AI_HOME_4B7_SOURCE.zip`. Nie uruchamiaj jeszcze 4B-8.

Po commit/push uruchom pełne GitHub Actions, w szczególności:

- `npm test`
- `npm run typecheck`
- `npm run build`
- `npm run verify:boundaries`
- `npm run verify:command-parity`
- `npm run verify:rust-source-text`
- `npm run verify:standard`
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`

Kluczowe regresje do obserwacji: `tests/activity-ui.test.tsx`, `tests/standard-closure.test.tsx`, `src-tauri/tests/standard_closure.rs`.

Warunek przejścia do 4B-8: pełne zielone CI i brak blokującego audytu 4B-7.
