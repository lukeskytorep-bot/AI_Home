# INSTRUKCJA — AI HOME 4B-5 REBASE AUDIT FIX 3

Baza: `AI_HOME_4B5_REBASE_AUDIT_FIX_2_SOURCE.zip`.

1. Rozpakuj `AI_HOME_4B5_REBASE_AUDIT_FIX_3_CHANGED_FILES.zip` w katalogu głównym repozytorium, gdzie znajdują się `package.json` i `src-tauri`, z nadpisaniem plików.
2. Nie nakładaj ponownie wcześniejszych overlayów.
3. Uruchom pełne CI, w szczególności `npm run build`, Vitest oraz Rust/Cargo.
4. Oczekiwany efekt compile-fix: brak TS2304 dla `Operation` oraz brak TS2554/TS2322 w `tests/imports-ui.test.tsx`.
5. Po zielonym CI przekaż pełny SOURCE z tego pakietu do ponownego audytu 4B-5.
