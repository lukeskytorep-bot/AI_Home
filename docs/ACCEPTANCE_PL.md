# 4B-3 krok 3 — Working Context w czacie, 21.10.2026

Korekta kroku 2 ma potwierdzenie zielonego wyniku Edwarda (bez identyfikatora/platform i nowego tekstu audytu). Krok 3 wykonano lokalnie.

- [x] Tylko aktywna zatwierdzona wersja własnego profilu; hash/receipt/commit zweryfikowane.
- [x] JSON user data z pochodzeniem, jedna stała instrukcja systemowa, pełna historia/źródła/replay.
- [x] Wspólny preflight/send liczy pełny blok i modelowe okno; bez cichego skracania i wyjątku dla unlimited.
- [x] Fresh TokenMeasurement dla bieżącego Primary ID/revision, bez mutowania starej wersji.
- [x] Atomowy snapshot w chat_runs; kontrola przed każdym fizycznym POST i commit.
- [x] Zmiana rekordu/wersji/empty→active blokuje spóźniony wynik; retry nie zawyża licznika po odrzuceniu przed dispatch.
- [x] STOP, idempotencja, błędy receipt, izolacja i restart.
- [x] Durable chat.context.get, EN/PL, historyczna wersja zamiast dzisiejszej, legacy unknown, ostrzeżenie o wyłączeniu.
- [x] Zmiana odczytanej pamięci unieważnia preflight i jego spóźniony wynik bez kasowania szkiców.
- [x] 250/250 Rust, 137/137 frontend, build, 23/23 granice, 193/193 P2.
- [x] Brak usunięć i zmian frozen P2/foundation, SQLite/migracji, dependencies/lockfiles, provider/vault i CI/capabilities.
- [ ] Nowe CI oraz audyt kroku 3; brak PASS CLOSED całego 4B-3.
- [ ] Osobne wcześniejsze odbiory live API/systemowego vault/Windows/Tauri/WebView.

---

## Historyczne kryteria wcześniejszych przekazań

# 4B-3 krok 2 — korekta audytu budżetu, 21.10.2026

Odebrany werdykt: **DO KOREKTY**. Poprawka ma lokalne wyniki PASS; brak nowego CI i werdyktu ponownego audytu.

- [x] Regresja przed poprawką: Accepted, 1 POST do Scribe, potem błąd kontekstu Primary.
- [x] Preflight obu koniecznych etapów przed Accepted, zapisem i siecią.
- [x] Minimalna decyzja Primary korzysta z tego samego build() i rzeczywistych znanych instrukcji/danych/wyjścia/rezerwy.
- [x] Primary 1024 oraz 2048 /Scribe 32768: 0 POST i brak nowych operations/receipts/events/proposals/versions.
- [x] Niewystarczający Scribe oraz pula jednego fizycznego wywołania odrzucone bez POST.
- [x] Ponowna walidacja rzeczywistej propozycji nadal działa; preflight minimalny nie jest gwarancją całej operacji.
- [x] 231/231 Rust, 130/130 frontend, build, 23/23 granice, 193/193 P2.
- [x] UI/kontrakty/SQLite v9/dependencies/lockfile/provider/vault/workflow/uprawnienia bez zmian.
- [ ] Nowe CI i ponowny audyt poprawki; brak PASS CLOSED.
- [ ] Dopiero później krok 3: Working Context w chat.preflight/send.
- [ ] Wcześniejsze odbiory live API/systemowy vault/Windows/Tauri/WebView pozostają otwarte.

---

## Historyczne kryteria bazowego kroku 2

# 4B-3 krok 2 — trwały Working Context i panel, 21.10.2026

Edward potwierdził zielone CI kroku 1 i upoważnił krok 2. Nowy krok ma lokalne PASS, wymaga CI/audytu. Nie dostarczono numerów/platform wcześniejszego przebiegu ani nowego tekstu audytu.

- [x] Ręczna aktualizacja L2 z tekstów wiadomości jednej rozmowy; niezależny Scribe lub Primary w roli proponującej.
- [x] Decyzja Primary z rzeczywistego wywołania, nie UI; ACCEPT/MODIFY/REJECT, pomiar/hash finalnej treści.
- [x] Trwałe propozycje, receipts, próby, wersje i typed wynik operacji; atomowa aktywacja.
- [x] Pełny snapshot/CAS przed POST i commit; zmiany klucza/modelu/źródła/polityki nie zatwierdzają starego wyniku.
- [x] Fizyczne retry/naprawy/decyzje dzielą 4 POST/180 s, trwała rezerwacja przed siecią.
- [x] STOP, restart, rollback, idempotencja i izolacja profili; stara wersja zachowana przy porażce.
- [x] Pojemność 300/presety/własna/unlimited, jawne okna obu modeli, EN/PL, faktyczny wynik i próby.
- [x] Szkic ustawień i pełny zakres zachowane; niepewny update rozliczany z tym samym ID po zmianie rozmowy.
- [x] SQLite v8→v9 i zachowanie poprzednich danych; stare kontrakty/dependencies/lockfile/CI/uprawnienia bez zmian.
- [x] 228/228 Rust, 130/130 frontend, build, 23/23 granice, 193/193 P2.
- [ ] Nowe CI/audyt kroku 2.
- [ ] Krok 3: aktywny L2 w chat.preflight/send; domknięcie 4B-3.
- [ ] Późniejsze warstwy/pozostałe przeznaczenia importu/wyszukiwanie/pełna historia i restore — 4B-4.
- [ ] Rzeczywiste OS vault/live API/Windows/Tauri/WebView.

Panel wyraźnie mówi, że w tym kroku zapisany L2 nie jest jeszcze wysyłany w czacie. Scribe czyta tekst wiadomości, bez zawartości dokumentów i obrazów.

---

## Historyczne kryteria wcześniejszych kroków

# 4B-3 krok 1 — silnik protokołu, 21.10.2026

Poprawka vision ma potwierdzenie zielonego CI Edwarda; brak identyfikatora/platform i tekstu nowego audytu. Za jego upoważnieniem wykonano pierwszy krok 4B-3.

- [x] Pure Rust: propozycja Scribe albo Primary, osobna decyzja Primary.
- [x] ACCEPT/MODIFY/REJECT; pomiar/hash faktycznej treści po MODIFY.
- [x] Domyślne 300, presety/własna wartość i unlimited bez cichego skracania.
- [x] Dwie próby pojemności, jedna osobna naprawa formatu, najwyżej cztery żądania logiczne.
- [x] Źródła w danych, profile/hash i język zadania niezależny od UI.
- [x] 200/200 Rust headless (29 nowych), 115/115 frontend, build, 22/22 granice, 193/193 P2.
- [x] SQLite/IPC/P2/provider/vault/dependencies/lockfile/workflow/capabilities bez zmian.
- [ ] Nowe CI i audyt 4B-3 kroku 1.
- [ ] Trwały koordynator, fizyczny budżet, receipts, atomowy zapis/STOP/restart i panel pamięci — krok 2.
- [ ] Dołączenie L2 do preflight/send i pełny odbiór 4B-3.
- [ ] Rzeczywiste OS vault/live API/Windows/Tauri/WebView.

Silnik nie jest IPC ani aktywną pamięcią. Wynik to kandydatura; nowe rekordy i aktywację może wykonać dopiero natywny koordynator po ponownej walidacji zakresu.

---

## Historyczne kryteria poprzednich etapów

# 4B-2A krok 3 — korekta audytu, 21.10.2026

Odebrany werdykt: DO KOREKTY. Nie otrzymano PASS poprawki.

- [x] Wymagany pełny scopeId z vision.get jest przesyłany w vision.set i porównywany w transakcji obok routeRevision/policyRevision.
- [x] Stary zapis po zmianie klucza + udanym re-teście odrzucony bez aktywacji.
- [x] Stary zapis po zmianie endpointu + nowym kluczu + udanym re-teście odrzucony bez aktywacji.
- [x] Panel odświeża unknown po konflikcie; nowa decyzja dotyczy wyłącznie świeżo odczytanego zakresu.
- [x] CSP produkcyjne/deweloperskie jawnie dopuszcza Blob wyłącznie dla obrazów; kontrola statyczna PASS.
- [x] 171/171 Rust headless; 115/115 frontend; build; 21/21 granice; 193/193 P2.
- [ ] Nowe CI i ponowny niezależny audyt poprawki.
- [ ] Rzeczywisty WebView i podgląd/CSP w desktopie; OS vault i live API — nadal otwarte.

Przed przejściem do 4B-3 wymagany odbiór poprawki. Poniżej zachowane historyczne przekazanie kroku 3.

# 4B-2A krok 3 — 21.10.2026, kandydat do CI i audytu

- [x] Edward potwierdził zielony wynik poprzedniego kroku 2 i kontynuację.
- [x] Import i podgląd statycznego PNG/JPEG/WebP/GIF; sygnatura, rozszerzenie, dekoder, rozmiar, wymiary, hash; animacje jawnie odrzucone.
- [x] Obraz ready bez fikcyjnego tekstu/OCR; dokumenty i obrazy w jednej wiadomości.
- [x] SourceRefs i snapshoty odtwarzają te same bajty w kolejnych turach i po restarcie.
- [x] Jawna deklaracja vision, CAS, izolacja i unieważnianie po zmianie trasy/połączenia/klucza; unknown i unsupported blokują także obrazy w historii.
- [x] Szacowany kontekst obrazów, limity dostawców, brak cichego usuwania/konwersji/obcinania; STOP i idempotencja.
- [x] SQLite v7 → v8; 169/169 Rust headless, 113/113 frontend, build, 21/21 granice, 193/193 P2 PASS.
- [ ] Nowe CI i niezależny audyt kroku 3.
- [ ] Rzeczywisty vault, live API oraz native desktop/Windows/WebView — odbiory nadal otwarte.

Całe 4B-2A pozostaje otwarte. Nie dodano OCR, RAG ani generowania obrazów. Następny etap funkcjonalny po odbiorze: 4B-3 — Scribe i Working Context. Wyniki początkowych nieudanych uruchomień zachowano w docs/evidence/4b2a-step-3. Poniższe odbiory są historyczne.

# 4B-2A krok 2 — 21.10.2026, kandydat do CI i audytu

- [x] Edward potwierdził zielone CI kroku 1; brak nowego werdyktu audytu i numeru przebiegu.
- [x] Wybór dokumentów, jawne wysyłanie tekstu/pochodzenia do dostawcy, oryginały lokalne.
- [x] Wspólny preflight i send, pełna historia i dokumenty, bez cichego skracania.
- [x] Trwałe SourceRefs i snapshoty wersji, atomowy bind/receipt/op, izolacja i walidacja hashy.
- [x] Migracja v6 → v7, restart bez POST, idempotencja, STOP i ochrona nowszego szkicu/wyboru.
- [x] 153/153 Rust headless; 109/109 frontend; build; 21/21 granice; 193/193 P2.
- [ ] Nowy wynik CI i niezależny audyt kroku 2.
- [ ] Natywny desktop/WebView/Windows oraz rzeczywisty OS vault i live API — nadal osobne odbiory.
- [ ] Obrazy/vision — kolejny krok; OCR i retrieval nie są częścią tego przekazania.

4B-2A pozostaje otwarte. Szczegóły i początkowe błędy fixture/asercji w raporcie kroku 2 oraz docs/evidence/4b2a-step-2. Poniżej historyczne odbiory; ich wyniki dotyczą wcześniejszych drzew.

# 4B-2A krok 1 — 20.10.2026, kandydat

- [x] TXT/MD/DOCX/tekstowy PDF: lokalny wybór, przechwycenie oryginału i ekstrakcja.
- [x] Profil/rozmowa, autor osobno od importera, P2 DTO, atomowy zapis i idempotencja.
- [x] Trwała operacja, STOP, restart/interrupted, oryginały i hash.
- [x] Panel EN/PL, podgląd tekstu, pobranie oryginału i odłączenie z CAS.
- [x] Jawny lokalny zakres: pliki nie są jeszcze wysyłane przez chat.send.
- [x] 146/146 Rust headless, 103/103 frontend, build, 21/21 granice, 193/193 P2.
- [ ] Nowe CI i audyt tej paczki.
- [ ] Rzeczywisty natywny dialog, pobieranie i Windows/Tauri/WebView.
- [ ] Podłączenie dokumentów do modelu — kolejny krok 4B-2A; potem obrazy.
- [ ] OCR, wiedza profilu i pamięć — późniejsze etapy.

Edward potwierdził zielone CI poprzedniej korekty 4B-2 kroku 2. Nie dostarczono PASS jej ponownego audytu. 4B-2A pozostaje otwarte. Wyniki testów i wcześniejszy timing FAIL są jawne w raporcie oraz logach. Poniżej historyczne odbiory.

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
