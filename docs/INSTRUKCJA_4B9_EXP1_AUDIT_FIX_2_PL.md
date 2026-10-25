# Instrukcja — 4B-9 EXP-1 AUDIT FIX 2

Baza: `AI_HOME_4B9_EXP1_AUDIT_FIX_SOURCE.zip`.

1. Nałóż `AI_HOME_4B9_EXP1_AUDIT_FIX_2_CHANGED_FILES.zip` na dokładną bazę.
2. Commit/push bez dodatkowego cleanup.
3. Uruchom pełne GitHub Actions, w szczególności Cargo/native tests.
4. Wymagane są nowe fault-injection regresje F1.
5. Nie rozpoczynaj EXP-2 przed zielonym CI i reaudytcie.

Zakres pozostaje EXP-1: fake clock/provider/executor, bez live IPC i bez produkcyjnych konektorów.
