# AI Home 4B-5 REBASE AUDIT FIX 5 — instrukcja

Baza: `AI_HOME_4B5_REBASE_AUDIT_FIX_4_SOURCE.zip`.

Nałóż `AI_HOME_4B5_REBASE_AUDIT_FIX_5_CHANGED_FILES.zip` w katalogu głównym AI_Home, bez usuwania innych plików.

Zakres tej poprawki jest celowo mały:
- aktualizacja oczekiwania migracji v4 do bieżącej wersji schematu 12;
- poprawienie scenariusza regresyjnego checkpointu tak, aby deterministycznie wymuszał seal segmentu zawierającego newest markers;
- aktualizacja evidence do FIX 5 / plan 1.38.

Po nałożeniu uruchom pełne CI, w szczególności:
`cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`
`npm ci`
`npm run test`
`npm run typecheck`
`npm run build`

Nie rozpoczynaj 4B-6 przed PASS CLOSED 4B-5.
