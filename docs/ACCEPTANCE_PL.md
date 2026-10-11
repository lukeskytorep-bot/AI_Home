# Odbiór 4B-0 — otwarte bramki

Stan: kandydat źródłowy; frontend sprawdzony, natywna podstawa nieodebrana.

- [x] Instalacja npm z lockfile, TypeScript i produkcyjny build.
- [x] Korekta po audycie: „Ponów” po konflikcie nie kasuje szkicu, spóźnione zdarzenie zapisu nie kasuje nowych zmian; test nowszych zdarzeń przy szkicu zmienionym i czystym.
- [x] Testy logiki i React w jsdom, kontrola zamrożonych P2 i granic modułów.
- [ ] Rust: uruchomić dostarczone testy (m.in. restart SQLite, CAS, idempotencja, rollback po błędzie receipts, równoległy konflikt, uszkodzony zapis).
- [ ] Windows/Linux: kompilacja Tauri z ograniczoną komendą, zatwierdzenie wygenerowanych capability.
- [ ] Zachować Cargo.lock i wyniki CI; nowe CI już używa --locked, a zachowanie lockfile pozostaje do wykonania.
- [ ] W rzeczywistym WebView: EN na czystym starcie, zapis PL po restarcie, oba wyglądy light/dark/system, klawiatura, 360/700/1280 px, czytelność błędów.
- [ ] Zapis natywny po restarcie: stan, receipts i odtwarzanie journal; sprawdzić, że nie ma fallbacku do preview.
- [ ] Potwierdzić natywny startup i zamknięcie; błąd otwarcia bazy nie uruchamia pozornie działającego kernela.
- [ ] Audyt uzupełnienia AppEvent/KernelStatus i pełnego pakietu hashy; decyzja o zamknięciu 4B-0.

Uruchomienie z jsdom nie potwierdza renderowania, WebView, keyringu ani API. Keyring i rzeczywiste AI API są poza tym etapem. Następny etap funkcjonalny: 4B-1 — adaptacja providerów Harness po usunięciu zależności RV i testy kontraktowe.
