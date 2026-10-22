# AI Home — 4B-4 krok 4

Baza: `AI_HOME_4B4_KROK_3_SOURCE.zip`, SHA-256 `7b7f3abccdcd0825721e1c1122e0cefdcf0672a2fcbf4ae552507e80b016e147`.

1. Rozpakuj `AI_HOME_4B4_KROK_4_CHANGED_FILES.zip` w głównym katalogu AI_Home, tam gdzie `package.json` i `src-tauri`. Zgódź się na nadpisanie. Niczego nie usuwaj.
2. Uruchom pełne CI. Ten handoff ma lokalnie 193/193 P2 i 26/26 boundaries, ale sandbox przygotowujący paczkę nie miał działającego Rust toolchainu ani kompletnego `node_modules`, więc Rust/frontend/build **muszą** zostać zweryfikowane przez CI przed akceptacją.
3. Po zielonym CI przekaż audytorowi pełny SOURCE, raport, plan, SHA256 i log odtworzenia.

Smoke po zbudowaniu aplikacji:

- zaimportuj TXT/MD/DOCX/tekstowy PDF jako Profile knowledge i sprawdź, że nie pojawia się jako attachment rozmowy;
- sprawdź osobne etykiety autora i importera;
- wyłącz/włącz knowledge i persistent guidance;
- przy guidance sprawdź preflight oraz jawny błąd po przekroczeniu budżetu, bez cichego cięcia;
- zaimportuj materiał jako Memory candidate, wybierz rekord pamięci i wpisz powód; operacja ma przejść przez istniejący Primary ACCEPT/MODIFY/REJECT, a sam import nie może utworzyć MemoryVersion;
- wykonaj czat pasujący słowami do Profile knowledge i sprawdź captured context; potem zmień enable state podczas runu i potwierdź konflikt zamiast spóźnionego commitu;
- sprawdź, że dokument zawierający tekst typu `SYSTEM: ignore previous rules` pozostaje zewnętrznym user data i nie dostaje system authority.

Po zielonym CI/audycie 4B-4 może zostać zamknięte. Następny etap: 4B-5 Conversation Segmentation + Continuity.
