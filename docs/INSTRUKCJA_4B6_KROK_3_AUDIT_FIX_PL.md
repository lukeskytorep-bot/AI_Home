# Instrukcja wdrożenia — AI Home 4B-6 krok 3 AUDIT FIX

1. Bazą musi być dokładnie `AI_HOME_4B6_KROK_3_SOURCE.zip` SHA-256 `7055dd774695979cc6b5aa186cacf268dd06f2fa039555703eafa62a63b8f964`.
2. Rozpakuj `AI_HOME_4B6_KROK_3_AUDIT_FIX_CHANGED_FILES.zip` w katalogu głównym `AI_Home`, nadpisując pliki. Nie usuwaj innych plików.
3. Uruchom:
   - `python3 scripts/verify-source.py .`
   - `python3 check_contracts.py`
   - `node scripts/verify-boundaries.mjs`
   - `python3 scripts/verify-rust-source-text.py`
   - `python3 scripts/verify-command-parity.py`
   - `npm ci`
   - `npm run test`
   - `npm run typecheck`
   - `npm run build`
   - `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`
4. W WebView sprawdź utworzenie oraz Download ZIP dla dwóch profili, w tym duży plik. Potwierdź, że po przełączeniu profilu nie pojawia się lista poprzedniego profilu.
5. Otwórz ZIP i sprawdź `connections.portable.json`, `routes.json`, `memory-proposals.json`, `memory-decisions.json`, manifest oraz źródła. Nie może być `credentialRef`, wartości vault ani provider replay.
6. Nie rozpoczynaj permanent deletion ani 4B-7 przed zielonym CI i re-audytem tego SOURCE.
