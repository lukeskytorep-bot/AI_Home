# Instrukcja wdrożenia — AI Home 4B-6 krok 2 AUDIT FIX

1. Bazą jest dokładnie `AI_HOME_4B6_KROK_2_SOURCE.zip`.
2. Nałóż `AI_HOME_4B6_KROK_2_AUDIT_FIX_CHANGED_FILES.zip` w katalogu `AI_Home/`, z nadpisaniem plików. Nie usuwaj innych plików.
3. Uruchom pełne CI: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`, `npm ci`, `npm run test`, `npm run typecheck`, `npm run build`.
4. Wymagane regresje restore: write-freeze po Accepted, idempotency po restarcie, requestId conflict, brak current, crash po current->previous, corrupt source fail-closed.
5. W desktop smoke sprawdź awaryjny restart z pending restore oraz Windows/Tauri/WebView.
6. Nie rozpoczynaj 4B-6 kroku 3 przed zielonym CI i re-audytem tej poprawki.
