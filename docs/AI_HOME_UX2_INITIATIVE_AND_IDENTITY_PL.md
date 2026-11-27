# AI Home — Paczka UX-2: Czysty Czat, Obecność i Dziennik Inicjatywy AI, Księga Tożsamości + Zamknięcie P3-1/P3-2/P3-3

**Data:** 2026-11-27  
**Zakres:** Krok 2 z planu przebudowy (`v3`) oraz zamknięcie wszystkich 3 obserwacji P3 z audytu Paczki UX-1.

## Zrealizowane zmiany

1. **Zamknięcie uwag P3 z Audytu UX-1:**
   - **`P3-1`:** W [`src/ui/FirstConversation.tsx`](../src/ui/FirstConversation.tsx) wpisanie filtra w `modelFilter` zeruje `model`, jeśli aktualnie wybrany model został odfiltrowany — stan React i `<select>` są zawsze spójne.
   - **`P3-2`:** W [`src/ui/App.tsx`](../src/ui/App.tsx) skrót `#diagnostics` używa `e.preventDefault()`, przełącza zakładkę Ustawień na `system` i przewija do `#diagnostics` w kolejnej klatce (`requestAnimationFrame`).
   - **`P3-3`:** Usunięto martwe klasy `.workspace-switch` z [`src/ui/style.css`](../src/ui/style.css), usunięto nieużywany klucz `onboarding.checkAndLoad` z `en.json`/`pl.json` oraz zsynchronizowano [`docs/AI_HOME_4B12_WINDOWS_VERIFICATION_CHECKLIST_4E_PL.md`](AI_HOME_4B12_WINDOWS_VERIFICATION_CHECKLIST_4E_PL.md) z nową nawigacją i zakładkami.
2. **Przebudowa ekranu `Życie i inicjatywa` ([`src/ui/AutonomousWorkspace.tsx`](../src/ui/AutonomousWorkspace.tsx)):**
   - Usunięto deweloperski selektor symulacji decyzji z głównego nagłówka propozycji i przeniesiono go do zwiniętego na samym dole panelu diagnostycznego (`details.dev-sim-panel`).
   - Po skonfigurowaniu providera API instancja automatycznie przełącza się na prawdziwe decyzje modelu (`simChoice = 'provider'`).
   - Dodano czytelny pasek obecności (`.presence-banner`: *Czuwa i gotowy*, *Odpoczywa do…*, *Wstrzymany*, *Wymaga Twojej uwagi*) oraz widoczną kartę **Dziennik dnia Rezydenta** (`ux2.diaryTitle`).
3. **Uporządkowany ekran Rozmowy ([`src/ui/Conversations.tsx`](../src/ui/Conversations.tsx)):**
   - W trybie aplikacji (`workspaceMode`) kompaktowy pasek wyboru profilu i wątku (`.conversation-toolbar`), zwijany formularz `+ Nowa rozmowa`, zwijane narzędzia szkicu/rewizji (`.chat-advanced-tools`) i archiwizacji — przy 100% zachowaniu kompatybilności testów jednostkowych.
4. **Księga Tożsamości w `Notatkach` ([`src/ui/NotesWorkspace.tsx`](../src/ui/NotesWorkspace.tsx)):**
   - Wyłącznie odczytowa karta (`.identity-book`) pokazująca liczniki Aktywnych Misji, Zatwierdzonych Wartości i Chronionej Strefy Prywatnej (bez ujawniania prywatnych treści).
