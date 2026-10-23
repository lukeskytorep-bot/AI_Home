# Instrukcja — AI Home 4B-6 krok 4

1. Bazą jest dokładnie zielony `AI_HOME_4B6_KROK_3_AUDIT_FIX_SOURCE.zip`.
2. Nałóż `AI_HOME_4B6_KROK_4_CHANGED_FILES.zip` albo użyj pełnego SOURCE kroku 4.
3. Uruchom pełne GitHub Actions, w tym Cargo all-targets, Vitest, typecheck i Vite build.
4. Szczególnie sprawdź testy `lifecycle.rs` i `deletion-ui.test.tsx`.
5. Permanent delete ma działać tylko dla Archive. Active profile/conversation musi dostać `POLICY_DENIED`; stale revision `REVISION_CONFLICT`.
6. Zweryfikuj replay dokładnie tego samego requestId po skutecznym delete. Nie może wykonywać drugiego usunięcia.
7. Zweryfikuj `PRAGMA foreign_key_check` po operacjach i brak osieroconych materiałów/blobów/operations.
8. Sprawdź, że portable exports i backupy nie są automatycznie kasowane. UI ma to jasno komunikować.
9. Sprawdź, że warningi `unused variable export` i `unnecessary parentheses around type` nie wracają.
10. Nie rozpoczynaj 4B-7 przed zielonym CI i audytem kroku 4.
