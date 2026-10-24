# Instrukcja — 4B-9 EXP-0 AUDIT FIX

Bazą jest dokładny `AI_HOME_4B9_EXP0_SOURCE.zip`.

1. Nałóż `AI_HOME_4B9_EXP0_AUDIT_FIX_CHANGED_FILES.zip` na tę bazę.
2. Nie dodawaj EXP-1 ani własnych zmian runtime przed wynikiem CI i ponownym audytem EXP-0.
3. Uruchom pełne GitHub Actions, w szczególności Cargo all-targets locked, Vitest, TypeScript/Vite i wszystkie verify scripts.
4. Sprawdź regresje lifecycle: inactive Experimental tree musi zostać usunięte razem z profilem, a non-inactive instance musi blokować permanent delete przez `POLICY_DENIED`.
5. Sprawdź kontrakt ActionRecord: `accepted` bez receipt ma być niepoprawny; `reconciled` wymaga receipt + jawnego outcome.
6. SQLite pozostaje `PRAGMA user_version = 13`; frozen P2 pozostaje bez zmian.
7. Po zielonym CI przekaż pełny SOURCE do ponownego audytu. Dopiero po jego akceptacji zamrażamy EXP-0 i przechodzimy do EXP-1.

AUDIT FIX nie powinien uruchamiać żadnych spontanicznych model calls ani Experimental schedulera.
