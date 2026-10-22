# AI Home — instrukcja 4B-4 krok 4 AUDIT FIX

Baza: dokładny `AI_HOME_4B4_KROK_4_SOURCE.zip` o SHA-256 `72fbdb8c85f4ff4aa2148687dcab3d5ade7f3e4d3188e97f659f675aacaa809c`.

1. Rozpakuj `AI_HOME_4B4_KROK_4_AUDIT_FIX_CHANGED_FILES.zip` w katalogu głównym repozytorium i nadpisz pliki. Niczego nie usuwaj.
2. Uruchom pełne CI: Rust all-targets/locked, Vitest, TypeScript/Vite, boundary verifier i P2 verifier.
3. Szczególnie sprawdź: toggle guidance/knowledge po preflight; memory candidate do ACCEPT/MODIFY/REJECT/STOP; profil z >100 rekordami; profilowy import rozpoczęty w rozmowie A, przełączenie do B, odzyskanie statusu i STOP.
4. Po zielonym CI przekaż audytorowi pełny SOURCE korekty, overlay, raport, SHA256 i reconstruction log.
5. 4B-4 zamknąć dopiero po PASS audytora. Dopiero potem kontynuować 4B-5.
