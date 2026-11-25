# 4B-14 Paczka 3A — widoki `Notes` i `Community` w lewej nawigacji (Sekcja 5.1 planu 1.98/1.99)

Data: 25.11.2026. Baza repozytorium: `92ab720` (`main`, „4b14”). Paczka skumulowana (1 + 2A + 2B + 2C + Audit Fix 1 + Windows TS Fix + 3A).

## 1. Zakres zmian Paczki 3A

1. **Działający widok `Notes` w lewej nawigacji (`src/ui/NotesWorkspace.tsx`, `src/ui/App.tsx`):**
   - Udostępnia podgląd notatek roboczych (`MemoryPanel`) oraz warstw pamięci i wyszukiwania leksykalnego (`MemoryLayersPanel`) dla wybranego profilu Standard.
   - Samo otwarcie panelu wykonuje wyłącznie odczyty (`profile.list`, `memory.working.get`, `memory.records.list`, `memory.layers.settings.get`) — nigdy nie uruchamia automatycznego tworzenia notatek ani wywołań modelu.
   - W podglądzie przeglądarkowym nie wykonuje wywołań IPC i wyświetla uczciwy komunikat o wymaganiu środowiska desktopowego.
2. **Osobny widok `Community` w lewej nawigacji (`src/ui/CommunityWorkspace.tsx`, `src/ui/App.tsx`):**
   - Jawnie rozróżnia trzy niezależne stany (bez fabrykowania statusu online):
     - lokalną konfigurację hosta (`Configured` vs `Not configured`),
     - lokalne poświadczenie (sprawdzane jawnie per profil przez `CommunityPanel`),
     - status online forum (zawsze jawnie `Not verified online (onlineStatusKnown = false)`).
   - Odczytuje zapisane kontakty profilu bezpośrednio z warstwy relacji (`memory.records.list`, `kind === 'relationship'`) — bez fabrykowania kontaktów.
   - Osadza istniejący, zaudytowany `CommunityPanel` (rejestracja Standard Community 4A) i jawnie zaznacza zakres (bez deklaracji obsługi wielu forów ani aktywnego forum w trybie Autonomous).
   - Kliknięcie `Notes`, `Community` lub `Activity` w lewej nawigacji synchronizuje górny przełącznik z powrotem na `Standard`.
3. **Lokalizacja i testy:**
   - Pełny parytet EN/PL w `src/locales/en.json` i `src/locales/pl.json`.
   - Nowy zestaw testów `tests/notes-community-views.test.tsx` + aktualizacja `tests/app.test.tsx`.
   - Bez zmian w kodzie Rust, migracjach SQLite, kontraktach P2 ani Tauri ACL.

## 2. Granice potwierdzenia

- Wszystkie testy wykonano na **Linux headless**.
- Bramki **Windows CI (`build_desktop=true`)**, próby użytkowe na Windows / WebView2 / OS Vault, live Community 4A / TLS, obowiązkowy przegląd OSS przed MCP oraz przeglądarka pozostają **OPEN**.
