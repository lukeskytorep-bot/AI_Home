# AI Home — Paczka UX-1: Jeden Dom (bez podziału Standard/Autonomous), Kreator 30s, Zmysły Wrodzone i Redesign CSS

**Data:** 2026-11-27  
**Zakres:** Krok 1 z planu przebudowy (`v3`) oparty bezpośrednio na wnioskach z prób na realnym Windows.

## Zrealizowane zmiany

1. **Likwidacja podziału `Standard | Autonomous` w UI ([`src/ui/App.tsx`](../src/ui/App.tsx)):**
   - Usunięto górny przełącznik dwóch trybów (`.workspace-switch`).
   - Zastąpiono go jedną, spójną lewą nawigacją Domu (`Start`, `Życie i inicjatywa`, `Notatki`, `Społeczność`, `Aktywność`) oraz przyciskiem `+ Nowy rezydent` w nagłówku.
2. **Naprawa blokera klucza API i nowy 3-krokowy kreator ([`src/ui/FirstConversation.tsx`](../src/ui/FirstConversation.tsx)):**
   - Pole klucza API **nie jest już czyszczone przy błędzie weryfikacji** — czyści się dopiero po sukcesie (lub przy zmianie dostawcy / zamknięciu), dzięki czemu użytkownik widzi jasny błąd i może natychmiast poprawić klucz.
   - Dodano 3-krokowy indykator (`.wizard-steps`), filtr wyszukiwania modeli (dla list > 5 modeli) oraz przycisk `Usuń niedokończony szkic` (`onboarding.resetDraft`), odblokowujący stare profile `draft`.
3. **Zmysły Wrodzone bez kluczy API ([`src/ui/InnateSensesBar.tsx`](../src/ui/InnateSensesBar.tsx)):**
   - Zdarzeniowy (bez `setInterval`) odczyt lokalnego czasu, pory doby, strefy czasowej, stanu sieci (`Online`/`Offline`) oraz opcjonalnie zasilania/baterii urządzenia.
4. **Gruntowny redesign CSS i zakładki w Ustawieniach ([`src/ui/style.css`](../src/ui/style.css), [`src/ui/App.tsx`](../src/ui/App.tsx)):**
   - Globalne, równe style dla wszystkich `input`, `textarea`, `select`, `label`, `.checkbox-row` i `.actions-row` — eliminacja nachodzących na siebie i zbitych pól w `CommunityWorkspace`, `McpPanel` i `SkillsPanel`.
   - Podział `Settings` na 5 czytelnych zakładek: *Wygląd i język*, *Rezydenci i modele*, *Zmysły i MCP*, *Umiejętności (Skills)*, *Kopie i diagnostyka*.
5. **Lokalizacja i testy:**
   - Pełny parytet kluczy w [`src/locales/en.json`](../src/locales/en.json) i [`src/locales/pl.json`](../src/locales/pl.json).
   - Nowe testy [`tests/ux1-unified-home.test.tsx`](../tests/ux1-unified-home.test.tsx) + zaktualizowane testy nawigacji, kreatora i zakładek.
