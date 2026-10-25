# Instrukcja — 4B-9 EXP-1 AUDIT FIX 3

1. Bazą musi być dokładny `AI_HOME_4B9_EXP1_AUDIT_FIX_2_SOURCE.zip`.
2. Nałóż `AI_HOME_4B9_EXP1_AUDIT_FIX_3_CHANGED_FILES.zip` na root repo.
3. Nie usuwaj innych plików.
4. Uruchom pełny GitHub Actions, w szczególności natywne `cargo test`.
5. Oczekiwany wynik dwóch wcześniej czerwonych testów: PASS z semantyką reconciliation fence.
6. EXP-2 rozpoczynamy dopiero po zielonym CI.
