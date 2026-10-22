# Instrukcja — AI Home 4B-5 REBASE AUDIT FIX 2 (F1/F2 + frontend CI)

1. Bazą jest dokładny `AI_HOME_4B5_REBASE_AUDIT_FIX_SOURCE.zip` o SHA-256 `f5ae15fee8c94ac789bdcdd84fe954d7524b57e5931211ed3dd50565c9c8b2ab`.
2. Rozpakuj `AI_HOME_4B5_REBASE_AUDIT_FIX_2_CHANGED_FILES.zip` do katalogu głównego `AI_Home`, z nadpisaniem. Nie usuwaj innych plików.
3. Uruchom pełne CI. Wymagane: Cargo/Rust all-targets locked, Vitest, TypeScript, Vite, P2 193/193 i boundaries 26/26.
4. W Rust zweryfikuj przede wszystkim:
   - `legacy_preflight_is_read_only_and_send_persists_same_bounded_continuity`;
   - `final_summary_preserves_newest_turns_after_all_budgets`;
   - wcześniejsze regresje S1/S2 i wielosegmentowej ciągłości.
5. W frontendzie oczekiwane jest usunięcie wcześniejszych 8 FAIL: pięć `chat-ui` i trzy `imports-ui`.
6. Po zielonym CI wykonaj ponowny audyt dokładnego SOURCE. Nie rozpoczynaj 4B-6 przed `PASS CLOSED` 4B-5.
