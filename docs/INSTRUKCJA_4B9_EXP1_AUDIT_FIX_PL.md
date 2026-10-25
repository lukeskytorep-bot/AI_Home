# Instrukcja — 4B-9 EXP-1 AUDIT FIX

Baza: `AI_HOME_4B9_EXP1_SOURCE.zip`.

Nałóż `AI_HOME_4B9_EXP1_AUDIT_FIX_CHANGED_FILES.zip` na dokładny SOURCE EXP-1 albo użyj pełnego `AI_HOME_4B9_EXP1_AUDIT_FIX_SOURCE.zip`.

Uruchom pełne GitHub Actions, w szczególności:
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`
- `npm test`
- `npm run build`
- wszystkie verify:* gates.

Szczególnie wymagane są natywne regresje R1–R4 w `src-tauri/tests/experimental_runtime.rs`.

Nie rozpoczynać EXP-2 przed zielonym CI i reaudytem.
