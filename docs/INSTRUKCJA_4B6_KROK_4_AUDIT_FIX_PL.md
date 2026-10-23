# Instrukcja — AI Home 4B-6 krok 4 AUDIT FIX

Bazą jest dokładnie `AI_HOME_4B6_KROK_4_SOURCE.zip` o SHA-256 `651fb1b579ac42d4caef1a4fd914dc7df0a45d9dd238d57a4ab379d779090465`.

1. Nałóż `AI_HOME_4B6_KROK_4_AUDIT_FIX_CHANGED_FILES.zip` na tę bazę.
2. Nie usuwaj istniejących plików poza zmianami wynikającymi z overlay; patch nie ma deletions.
3. Uruchom pełne GitHub Actions, w szczególności `cargo test --all-targets --locked`, Vitest, TypeScript typecheck i Vite build.
4. Sprawdź natywne regresje w `src-tauri/tests/lifecycle.rs` dotyczące D1/D2.
5. Potwierdź, że `grep` nie znajduje `instr(response` ani tekstowego kasowania receipts.
6. Potwierdź 193/193 P2, 29/29 boundaries, 51/51 command parity oraz `verify-rust-source-text.py`.
7. Po zielonym CI przekazuj pełny SOURCE do ponownego audytu kroku 4. Nie rozpoczynaj 4B-7 przed jego odbiorem.
