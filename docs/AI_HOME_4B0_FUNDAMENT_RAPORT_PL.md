# AI Home 4B-0 — raport fundamentu

Wersja treści 1.1 · 11 października 2026

**Status: kandydat źródłowy do audytu. Frontend sprawdzony. Brak PASS CLOSED dla 4B-0: kompilacja natywna i rzeczywisty WebView pozostają do wykonania.**

## Korekta po audycie 4B-0

Audytor potwierdził wcześniejsze 50/50 testów, build, hashy i bazę P2, ale odtworzył kasowanie szkicu przy „Ponów” po konflikcie i przy opóźnionym zdarzeniu już zapisanej rewizji. 4B-0 pozostaje otwarty.

Oba przypadki odtworzono w nowych stałych testach: przed poprawką 2 FAIL, po poprawce PASS. Oddzielono własność szkicu od zatwierdzonych ustawień. Synchronizacja aktualizuje rewizję, chroni niezapisane zmiany i ignoruje rewizje już obsłużone. Potwierdzenie zapisu czyści szkic wyłącznie dla tej samej generacji edycji; starsza odpowiedź nie obniża przyjętej rewizji. Dodano też test nowszego zdarzenia: chroni zmieniony szkic i aktualizuje czysty formularz po zapisie. Łącznie 53/53 testy frontendowe PASS. Kontrole granic i build nadal PASS.

W workflow dodano archiwizowanie wygenerowanych schematów/uprawnień razem z binarnym kandydatem i Cargo.lock, aby natywny odbiór miał materiał do weryfikacji. Workflow nadal nie został uruchomiony. Środowisko ponownie sprawdzono: brak cargo/rustc i brak runnera Windows/WebView. Odbiór Rust/SQLite, kompilacja Windows/Linux i UI natywne nie są zaliczone. Nie rozpoczęto 4B-1.

## Podstawa decyzji

Edward potwierdził pozytywny odbiór audytora dla skorygowanego P2 i upoważnił kolejny krok. Jest to przyjęcie przekazane przez użytkownika, nie nowy audyt wykonany w tym raporcie. Wykorzystano P2 1.1, plan zaktualizowano do 1.7. Cztery kontrakty P2 pozostają identyczne bajtowo; ich sumy kontroluje skrypt granic modułów.

## Co wykonano

- Lokalny szkielet repozytorium AI Home z Tauri 2, React/TypeScript i Rust/SQLite. Konfiguracja Vite, npm lockfile, osobne warstwy UI, IPC, domeny, aplikacji, persistence i kernela.
- Ekran pierwszego uruchomienia: EN domyślnie, PL, Orbital/Aurora, jasny/ciemny/systemowy wariant. Ustawienia mają szkic i jawny zapis; komunikaty są lokalizowane. Kod zawiera układ dla wąskiego okna, ale jego renderowania nie potwierdzono.
- Natywny kandydat kernela Standard/NullState, bez zależności od Pythona, forum, dostawców ani domeny RV. Startup inicjalizuje bazę przed gotowym kernelem; zamknięcie obiektu aplikacji ustawia Stopping.
- Dispatcher pięciu poleceń: app.preferences.get/set, app.pack.validate, kernel.status.get, app.events.list. Walidacja wejścia, bezpieczne błędy, kontrola rewizji oraz rozdzielenie desktopu i jawnego preview.
- Kod SQLite: migracja wersji 1; atomowy zapis ustawień, receipt i zdarzenia; idempotencja; journal do odtworzenia po restarcie lub utracie emit. Te zachowania natywne mają dostarczone testy Rust, lecz nie były wykonane w tym środowisku.
- Osobny browser preview z testowaną trwałością localStorage. Nie udaje natywnej bazy i nie jest fallbackiem po błędzie Tauri.
- Deklaratywne paczki języka i skórek z walidacją; kompletne EN/PL, fallback EN dla brakujących znanych kluczy, odmowa nieznanych kluczy i wykonywalnego CSS. Instalowanie własnych paczek pozostaje późniejszą funkcją.
- Jawne uzupełnienie P2 dla zdarzeń aplikacji: foundation.schema.json i foundation.ipc.json. Nie nadano ustawieniom aplikacji fikcyjnego profileId. Ten dodatek podlega audytowi 4B-0.
- CI dla frontend oraz Rust/Tauri na Windows/Linux, instrukcja uruchomienia i lista otwartych bramek. Zainicjowano lokalny Git; bez commitów, remote, publikacji lub zmian na forum.

## Wyniki wykonanych kontroli

| Kontrola | Wynik i granice |
|---|---|
| npm ci z lockfile | PASS; dostępne zależności z lokalnego cache |
| Vitest/React/jsdom | 53/53 PASS w 3 plikach; obejmuje przykłady P2, pakiety, trwałość preview, CAS, idempotencję, błędy, UI EN/PL, zmianę skórki i zachowanie szkicu |
| TypeScript | PASS |
| Produkcyjny build Vite | PASS |
| Granice modułów i hashy P2 | 15/15 PASS; kontrole statyczne nie są pełnym audytem architektury |
| Kontrole referencyjne P2 | 193/193 PASS, ponownie uruchomione na bazie P2 |
| Kontrast kolorów | PASS w testach dla obu wariantów obu skórek; nie zastępuje oceny wizualnej |
| HTTP smoke produkcyjnego preview | PASS: serwer i HTML/CSS/JS zwracają HTTP 200; bez wykonania JS w przeglądarce |
| Testy Rust i SQLite | NOT RUN — brak cargo/rustc; 8 testów dostarczonych, bez deklaracji zaliczenia |
| Kompilacja Tauri/Windows/Linux | NOT RUN; workflow przygotowany, nie uruchomiony |
| Rzeczywista przeglądarka/WebView | NOT RUN — brak silnika Chromium/WebView |
| Keyring i rzeczywiste API | Poza zakresem 4B-0; nie były testowane |

Logi wykonanych kontroli są w docs/evidence. Testy z jsdom sprawdzają logikę i DOM, nie rozmiary renderowanych elementów, Tauri IPC ani zachowanie na rzeczywistym ekranie.

## Ponowne wykorzystanie Harness

Baza P1: AI-RV-Harness-ai-rv-2.zip, SHA-256 534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e. Użyto decyzji technologicznej, rzeczywistych wersji zależności i ogólnego wzorca Tauri/React; npm lockfile zaadaptowano i sprawdzono. Nie kopiowano migracji i danych Harness. W 4B-0 nie przenoszono providerów, retry, importerów ani pamięci — ich adaptacja zgodnie z mapą P1 następuje w właściwych etapach. Ustalenie wykorzystania gotowców pozostaje w planie.

## Istotne ograniczenia przed zamknięciem

Nie ma jeszcze gotowego instalatora ani potwierdzonego startu natywnej aplikacji. W tym środowisku brak toolchain Rust i silnika przeglądarki. Kod natywny wymaga kompilacji oraz wykonania dostarczonych testów i sprawdzenia uprawnień. Bezpośrednie wersje crate'ów przypięto na podstawie Harness, ale zależności przechodnie nie zostały rozstrzygnięte: Cargo.lock ma powstać w pierwszym CI, zostać zachowany i włączony przed zamknięciem etapu. Następnie należy używać --locked.

Nie zaimplementowano providerów, kluczy API, profili, rozmów, plików, pamięci, Scribe ani Conscious. Ekran oznacza kolejne funkcje jako przyszłe. Nie przedstawia symulacji jako działania AI.

Do odbioru: uruchomienie CI, utworzenie lockfile Rust, start i restart aplikacji natywnej, EN/PL, skórki light/dark/system, klawiatura, wąskie okno, SQLite i recovery dziennika. Dokładna lista w docs/ACCEPTANCE_PL.md. Natywny gate pozostaje otwarty; potem następny etap funkcjonalny to 4B-1, czyli adaptacja providerów Harness.

## Identyfikacja przekazania

AI_HOME_4B0_SOURCE.zip zawiera kod, README, plan 1.7, ten raport, bazowy raport P2 1.1, logi i manifest wszystkich plików poza samym manifestem. Archiwum nie zawiera node_modules, dist, target ani metadanych Git. Zewnętrzny plik SHA256 identyfikuje całe archiwum. Weryfikator akceptuje archiwum niezależnie od jego nazwy (np. dopisek pobrania „(1)”). Osobne pliki raportu i planu są identyczne bajtowo z ich kopiami w ZIP-ie.

Źródła techniczne sprawdzone podczas przygotowania: https://v2.tauri.app/security/capabilities/ oraz https://v2.tauri.app/reference/config/ . Konfiguracja deklaruje AppManifest.commands i capability main; skuteczność wygenerowanej konfiguracji wymaga native build.
