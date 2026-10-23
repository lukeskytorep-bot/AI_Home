# Instrukcja — AI Home 4B-6 krok 4 AUDIT FIX 2

1. Bazą jest dokładnie `AI_HOME_4B6_KROK_4_AUDIT_FIX_SOURCE.zip` SHA-256 `41ffe190f77685b61b80d64ced458210c7904a10e70e716d88bbd8be769c3b59`.
2. Nałóż `AI_HOME_4B6_KROK_4_AUDIT_FIX_2_CHANGED_FILES.zip` na tę bazę, zachowując strukturę katalogów.
3. Uruchom pełne GitHub Actions / CI projektu, w szczególności `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`, Vitest, TypeScript typecheck i Vite build.
4. Szczególnie potwierdź `src-tauri/tests/lifecycle.rs` oraz `tests/deletion-error.test.ts`.
5. Po zielonym CI przekaż finalny SOURCE do reaudytu. Nie rozpoczynaj 4B-7 przed odbiorem permanent deletion.
