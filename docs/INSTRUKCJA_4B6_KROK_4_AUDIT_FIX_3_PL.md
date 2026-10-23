# Instrukcja — AI Home 4B-6 krok 4 AUDIT FIX 3

Baza: `AI_HOME_4B6_KROK_4_AUDIT_FIX_2_SOURCE.zip`.

1. Nałóż `AI_HOME_4B6_KROK_4_AUDIT_FIX_3_CHANGED_FILES.zip` na dokładną bazę powyżej.
2. Uruchom pełne GitHub Actions, w szczególności `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.
3. Zweryfikuj, że test `permanent_conversation_delete_redacts_memory_run_sources_and_cancels_active_memory_operation` przechodzi.
4. Nie rozpoczynaj 4B-7 przed zielonym CI i re-audytem kroku 4.

Poprawka nie zmienia produkcyjnej semantyki permanent deletion. Naprawia wyłącznie wadliwy fixture testowy i dodaje jego samowalidację kontraktową.
