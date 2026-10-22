# INSTRUKCJA — AI HOME 4B-5 REBASE AUDIT FIX 4

Baza: `AI_HOME_4B5_REBASE_AUDIT_FIX_3_SOURCE.zip`.

1. Rozpakuj `AI_HOME_4B5_REBASE_AUDIT_FIX_4_CHANGED_FILES.zip` w katalogu głównym repozytorium (`package.json`, `src-tauri`) z nadpisaniem.
2. Nie nakładaj ponownie FIX 1–3.
3. Uruchom pełne CI:
   - `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`
   - `npm ci`
   - `npm run test`
   - `npm run typecheck`
   - `npm run build`
4. Oczekiwany efekt Rust: brak E0425 `cannot find function validate` w `src/profile_imports.rs`.
5. Oczekiwany efekt Vitest: testy `polls an imported memory candidate...` i `keeps memory-candidate monitoring...` znajdują aktualne kontrolki i przechodzą.
6. Po zielonym CI przekaż `AI_HOME_4B5_REBASE_AUDIT_FIX_4_SOURCE.zip` do końcowego audytu 4B-5.

Nie rozpoczynaj 4B-6 przed formalnym `PASS CLOSED` 4B-5.
