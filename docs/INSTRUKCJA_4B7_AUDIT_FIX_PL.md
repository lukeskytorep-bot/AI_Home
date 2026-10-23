# Instrukcja — AI Home 4B-7 AUDIT FIX

1. Bazą jest dokładny `AI_HOME_4B7_SOURCE.zip`.
2. Nałóż `AI_HOME_4B7_AUDIT_FIX_CHANGED_FILES.zip`.
3. Uruchom pełne GitHub Actions: frontend Vitest, TypeScript/Vite build i `cargo test --all-targets --locked`.
4. Szczególnie potwierdź nowe regresje `standard_closure.rs` i `activity-ui.test.tsx`.
5. Nie rozpoczynaj 4B-8 przed zielonym CI i reaudytorem.
