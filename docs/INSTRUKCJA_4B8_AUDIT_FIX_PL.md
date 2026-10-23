# Instrukcja — 4B-8 AUDIT FIX

1. Bazą jest dokładnie `AI_HOME_4B8_SOURCE.zip`.
2. Nałóż `AI_HOME_4B8_AUDIT_FIX_CHANGED_FILES.zip` bez usuwania innych plików.
3. Uruchom pełne GitHub Actions, w szczególności `npm test`, `npm run typecheck`, `npm run build` oraz `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.
4. Po zielonym CI wykonaj desktop smoke: Refresh diagnostyki, realny download JSON w WebView, EN/PL, Orbital/Aurora, light/dark.
5. Nie rozpoczynaj 4B-9 przed re-audytem i formalnym PASS CLOSED 4B-8.
