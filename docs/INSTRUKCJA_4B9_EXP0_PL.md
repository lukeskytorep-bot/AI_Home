# Instrukcja — 4B-9 EXP-0

Bazą jest dokładny zielony `AI_HOME_4B8_AUDIT_FIX_2_SOURCE.zip`.

1. Nałóż `AI_HOME_4B9_EXP0_CHANGED_FILES.zip` na tę bazę.
2. Nie mieszaj EXP-1 ani własnych zmian runtime przed wynikiem CI/audytu EXP-0.
3. Uruchom pełne GitHub Actions, w szczególności Cargo all-targets locked, Vitest, typecheck/build i wszystkie verify scripts.
4. Sprawdź, że storage po migracji ma `PRAGMA user_version = 13`, frozen P2 nadal 193/193, a Standard tests pozostają zielone.
5. Po zielonym CI przekaż pełny SOURCE do audytu. Dopiero po akceptacji zaczynamy EXP-1.

EXP-0 nie powinien wykonywać żadnych spontanicznych model calls ani aktywować Experimental scheduler.
