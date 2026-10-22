# Instrukcja — AI Home 4B-6 krok 1 AUDIT FIX

1. Bazą jest dokładny AI_HOME_4B6_KROK_1_SOURCE.zip.
2. Rozpakuj AI_HOME_4B6_KROK_1_AUDIT_FIX_CHANGED_FILES.zip w katalogu AI_Home z nadpisaniem plików. Nie usuwaj innych plików.
3. Uruchom pełne CI: cargo test --all-targets --locked, npm test, npm run typecheck, npm run build oraz istniejące verify-source/contracts/boundaries.
4. Szczególnie sprawdź race: chat POST w toku → archive conversation → restore → odpowiedź wraca. Stara operacja musi pozostać cancelled i nie wolno zapisać assistant message.
5. Sprawdź niejednoznaczny lifecycle write: retry musi użyć tego samego requestId.
6. Sprawdź Accepted restore + failure kolejnego readback: rodzic ma już dostać przywrócony rekord.
7. Nie rozpoczynaj 4B-6 krok 2 przed PASS tego audit fix.
