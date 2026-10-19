# Korekta audytu 4B-2 krok 2 — 19.10.2026

- [x] Accepted rozlicza szkic również po zmianie rozmowy i przy błędzie historii; nowszy tekst jest zachowany.
- [x] Terminalny polling uzgadnia rewizję mimo opóźnionego IPC.
- [x] Odrzucenie załączników używa istniejącego kodu P2 i podlega walidacji Error/ResultEnvelope.
- [x] Cztery nowe regresje UI; logi korekty osobno od historycznych wyników.
- [ ] Nowe CI i ponowny audyt korekty.
- [ ] Live API, rzeczywisty vault, Windows/Tauri/WebView.

4B-2 pozostaje otwarte. 4B-2A nie rozpoczęto. Poniżej historyczny odbiór przed korektą.

# Aktualny odbiór: 4B-2 krok 2

- [x] chat.send, trwały człowiek/operacja przed HTTP, atomowa odpowiedź i końcowy wynik.
- [x] History ordering, izolacja, idempotencja, CAS, STOP i ochrona przed late response po zmianie konfiguracji.
- [x] Jawna deklaracja kontekstu i metoda szacowana; brak cichego skracania historii; limit odpowiedzi/rezerwa widoczne.
- [x] Prywatny replay związany ze ScopeStamp, bez hidden data w IPC.
- [x] Migracja v4 → v5 i interrupted po restarcie bez wznowienia HTTP.
- [x] 125/125 native headless, 89/89 frontend, build, 20/20 granice, 193/193 P2 PASS.
- [ ] Nowe CI/audyt kroku 2.
- [ ] Rzeczywisty OS vault/live API i Windows/Tauri/WebView — nadal osobny odbiór.
- [ ] Odbiór wcześniejszej korekty kroku 1 nie został osobno potwierdzony w ostatniej wiadomości Edwarda.

Edward potwierdził zielone CI 4B-1 i upoważnił kontynuację. Poniższe wpisy zachowują historyczne stany. 4B-2 nie jest PASS CLOSED.

# Aktualny odbiór: 4B-2 krok 1

- [x] Profile-scoped create/list/get i draft.set z CAS/idempotencją.
- [x] UI EN/PL zachowuje dirty draft przy wyborze rozmowy/profilu, zmianie języka, konflikcie i spóźnionym save.
- [x] Migration v3 → v4, restart, FK i rollback sprawdzone.
- [x] 107/107 native headless z audytu (kod niezmieniony), 81/81 frontend, build, 19/19 granice, 193/193 P2 PASS.
- [x] Korekta payloadu profile.list na wszystkich stronach; mock UI waliduje każde żądanie według kontraktu.
- [ ] Nowe CI/ponowny audyt korekty tej części.
- [ ] Wysyłanie do modelu i wyświetlanie trwałej historii wiadomości — 4B-2 krok 2.
- [ ] Rzeczywisty OS vault/live API, Windows/Tauri/WebView — wcześniejsze odbiory nadal otwarte.

Edward potwierdził zielone CI wcześniejszego kroku 4 po synchronizacji i poprawce deadline, bez identyfikacji platform/przebiegu. Poniższy materiał kroku 4 jest historyczny. 4B-2 ani całe 4B-1 nie otrzymują PASS CLOSED.

# Historyczne bramki wcześniejszego przekazania

Stan 12.10.2026: kandydat 4B-1 krok 4 po korekcie audytu do nowego CI/audytu. Poprzedni krok ma potwierdzenie zielonego CI Edwarda, bez numeru/platform. Całe 4B-1 pozostaje otwarte.

## Krok 4 — wykonane i lokalnie sprawdzone

- [x] Jeden retry transportowy na etap i jedna próba recovery odczytu/JSON/pustego wyniku; wspólny limit 4 wywołań/180 s, bez resetowania przez paginację.
- [x] Anulowanie backoff, Retry-After ograniczone do 30 s i pozostałego deadline.
- [x] Commit każdego licznika przed HTTP; trwały wynik connection.test i operation.get po dwóch próbach.
- [x] Brak retry po sukcesie, odmowie, limicie wyjścia, błędzie bindingu, walidacji lub zapisu.
- [x] Korekta audytu: Google functionCall razem z tekstem daje OutputIncomplete; malformed tool_calls daje InvalidResponse przed klasyfikacją pustego tekstu. Oba bez ponowienia.
- [x] HMAC nowych credential receipts, klucz instalacji tylko w vault, świeży readback i brak regeneracji brakującego klucza.
- [x] Migracja v1/v2 → v3 i zgodny, tylko odczytowy replay starych receipts sprawdzone w testach.
- [x] 97/97 Rust Linux headless, 71/71 frontend, build, 18/18 granice i P2 193/193 PASS.
- [x] P2, zależności, Cargo.lock, workflow i uprawnienia bez zmian.
- [ ] Nowe CI oraz audyt dokładnej nakładki kroku 4.
- [ ] Rzeczywisty OS vault na Windows/Linux i uzgodniony smoke dostawców.
- [ ] Odbiór Windows i WebView zgodnie z otwartymi bramkami 4B-0.

Raport aktualny: `AI_HOME_4B1_KROK_4_RAPORT_PL.md`. Poniższe wyniki kroku 3 są historyczne. Krok 4 nie dodaje streamingu, rozmów, automatycznej kontynuacji, narzędzi ani naprawy pamięci; te funkcje mają dalsze etapy.

## Historyczny zakres kroku 3 — zachowany

- [x] Profile, własne połączenia/endpointy, trasy Primary/Scribe i scoped IPC.
- [x] Systemowy vault zaimplementowany z własnym bindingiem i świeżym odczytem kontrolnym; brak secret-only fallbacku.
- [x] Trwałe queued/running/result/error, operation.get/cancel i ostatni test po restarcie.
- [x] Commit rezerwacji przed HTTP; spóźniony wynik nie zatwierdza zmienionego połączenia.
- [x] Klucz nie trafia do SQLite, receipts, localStorage ani odpowiedzi IPC; zmiana klucza zachowuje tożsamość profilu.
- [x] Migracja faktycznej bazy v1 → v2: preferencje zachowane, integrity/FK PASS.
- [x] 70/70 Rust Linux headless, 71/71 frontend, TypeScript/build, 18/18 granice i P2 193/193 PASS.
- [x] Cargo.lock wygenerowany przez Cargo, wcześniejsze wersje pakietów zachowane; workflow/uprawnienia bez zmian.
- [x] Inwentarz katalogu/ZIP-a i odtworzenie nakładki na poprzedniej bazie sprawdzone.
- [x] Zielone CI kroku 3 potwierdzone przez Edwarda; numer przebiegu i platformy nie zostały przekazane.
- [ ] Audyt kroku 3.
- [ ] Rzeczywisty vault na Windows/Linux: zapis/readback/restart, zablokowany magazyn, zmiana klucza i endpointu.
- [ ] Natywny WebView: oba wyglądy, EN/PL, klawiatura, wąskie okno, szkice, restart i błędy. Testy UI mają zakres jsdom; Chromium nie uruchomiono.
- [ ] Uzgodniony live smoke połączeń; pobranie listy modeli nie dowodzi działania czatu.
- [x] Retry/recovery zaimplementowane i lokalnie sprawdzone w kroku 4; wymagany nowy wynik CI/audyt.

Szczegóły: `AI_HOME_4B1_KROK_3_RAPORT_PL.md`. Raporty kroków 1/2 oraz wcześniejsze logi są historycznymi stanami przekazań, nie deklaracją obecnego NOT RUN dla testów już wykonanych w kroku 3. Nie rozpoczęto rozmów 4B-2; wcześniejsze create/list/snapshot profilu są zależnością konfiguratora.

## Pozostałe bramki fundamentu 4B-0

- [x] Ochrona szkicu ustawień: Ponów i opóźnione zdarzenia nie kasują własnych zmian.
- [x] Cargo.lock i poprawiony async Result; lokalne testy fundamentu/SQLite przeszły w aktualnym zestawie.
- [x] P2 1.1 zamrożone; dawne 119/145/193 rozdzielone, duplikaty zostały wcześniej uporządkowane.
- [ ] Zachować link/commit/logi zielonego CI i potwierdzić zakres obu platform.
- [ ] Windows: istniejące `check_windows=true`, jeśli brak dowodu jego zaliczenia.
- [ ] Kontrola wygenerowanych uprawnień głównego okna.
- [ ] Rzeczywisty WebView ze startem, restartem, preferencjami i obsługą błędów.
- [ ] Decyzja odbiorowa o zamknięciu 4B-0.

Przed 4B-12 ustalić kanoniczny status integracji 4A. Odbiór potwierdzony przez Edwarda obowiązuje; starszy indeks z otwartą bramką produkcyjną wymaga uzgodnienia.
