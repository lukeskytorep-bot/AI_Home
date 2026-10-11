# Fundament 4B-0: moduły i granice

| Moduł | Odpowiedzialność |
|---|---|
| `src/ui` | Ekran pierwszego uruchomienia, szkic ustawień, zapisywanie i komunikaty. Bez bezpośrednich native invoke i fetch. |
| `src/ipc` | Jeden klient i dwa jawne transporty: desktop oraz demonstracyjny browser. Brak fallbacku po błędzie desktopu. |
| `src/domain` | Typy, walidacja używanych słów kluczowych schematów i pakietów, EN fallback, kolory skórki. |
| `src-tauri/src/kernel.rs` | Cykl życia Booting/Ready/Failed/Stopping, Standard i NullState. Brak SQL, dostawców, forum i ConsciousCore. |
| `src-tauri/src/application.rs` | Dispatcher, walidacja IPC, walidacja paczek, koordynacja zmiany ustawień. |
| `src-tauri/src/persistence.rs` | SQLite: migracja wersji 1, ustawienia, receipts i dziennik zdarzeń. |
| `src-tauri/src/lib.rs` | Composition root Tauri, katalog aplikacji, jedna komenda natywna i publikacja zatwierdzonego zdarzenia. |
| `contracts` | Niezmienione cztery JSON-y P2 oraz jawny dodatek zakresu aplikacji. |

## Rzeczywiście zaimplementowane polecenia

`app.preferences.get`, `app.preferences.set`, `app.pack.validate` pochodzą z P2. `kernel.status.get` i `app.events.list` są osobnym rozszerzeniem 4B-0. Pozostałe polecenia P2 zwracają `UNKNOWN_COMMAND`, a interfejs nie przedstawia ich jako działających.

P2 zawiera zdarzenia profilu. Ustawienia aplikacji nie mają `profileId`, dlatego dodano odrębne `AppEvent`, `EventPage` i `KernelStatus` w foundation.schema.json oraz foundation.ipc.json. To rozszerzenie wymaga przeglądu w audycie 4B-0; nie przepisano zaakceptowanych kontraktów P2. Hashy pilnuje skrypt granic modułów.

## Spójność zmiany ustawień

UI wysyła `expectedRevision` i unikalny `requestId`. Dispatcher przyjmuje tylko wymienione pola. Natywny zapis obejmuje w jednej transakcji: nową rewizję, wynik dla tego requestId oraz zdarzenie z rosnącą sekwencją. Ponowienie identycznego żądania zwraca zapisany wynik bez drugiego zdarzenia; zmiana treści pod tym samym ID jest konfliktem idempotencji. Stara rewizja nie nadpisuje nowej.

Emit następuje po commit. Błąd emit nie cofa zapisu. Odbiorca po wykryciu luki pobiera aktualne ustawienia; `app.events.list` pozwala odtworzyć zdarzenia po utracie transmisji lub restarcie. Subskrypcja UI jest zakładana przed odczytem; najpierw pobierana jest sekwencja, potem ustawienia, a starsza rewizja nie zastępuje nowszej. Dziennik natywny nie jest czyszczony w 4B-0. Preview przechowuje ostatnie 100 zdarzeń i jawnie zwraca EVENT_GAP przy starszym kursorze.

UI zachowuje szkic po konflikcie rewizji i po kliknięciu „Ponów”, pobiera nową rewizję i umożliwia jawne ponowienie zapisu. Szkic ma osobny znacznik niezapisanych zmian i licznik edycji. Nowe zatwierdzone ustawienia zawsze aktualizują rewizję, lecz zmieniają formularz tylko bez własnych zmian użytkownika. Zdarzenia dotyczące już obsłużonej lub starszej rewizji nie zmieniają szkicu. Potwierdzenie zapisu czyści znacznik tylko dla tej samej generacji edycji; starsza odpowiedź nie obniża przyjętej rewizji. Odrzucony zapis nie emituje zdarzenia. Uszkodzony zapis nie jest automatycznie zastępowany domyślnymi ustawieniami.

## IPC i pakiety

Tauri rejestruje tylko `home_dispatch`. Lista komend w AppManifest ogranicza dostęp do niej przez capability głównego, lokalnego okna. Capability pozwala wyłącznie na dispatcher oraz listen/unlisten. Nie przyznaje surowego SQL, filesystemu, emit z UI ani dostępu do okien zdalnych. CSP wydania różni się od CSP deweloperskiego dla Vite. Sprawdzenie wygenerowanych uprawnień podczas native build jest otwartą bramką.

EN/PL są kompletne. Pakiet dodatkowy z brakującymi znanymi kluczami dostaje ostrzeżenia `fallback.en:<key>`; nieznane klucze są błędem. Kolory skórek mają format HEX; pakiet nie może przekazać wykonywalnego CSS. Obecne ustawienia pozwalają wybrać tylko wbudowane EN/PL i Orbital/Aurora. Instalacja własnych pakietów nie jest jeszcze częścią 4B-0.

Własne walidatory TS/Rust obsługują podzbiór JSON Schema występujący w zamrożonych plikach. Nie są uniwersalnym silnikiem JSON Schema. W TS uruchomiono wszystkie pozytywne przykłady P2 i przykłady odrzucania. Analogiczny test w Rust dostarczono, ale jeszcze nie uruchomiono. Reguły semantyczne przyszłych profili/operacji będą implementowane w ich etapach, nie przez samo przyjęcie DTO.

## Ponowne użycie Harness

Wykorzystano sprawdzony w P1 wybór stosu, rzeczywiste wersje zależności frontendowych oraz wzorzec Tauri/React. npm lockfile dostosowano do mniejszej listy zależności AI Home i sprawdzono przez `npm ci`. Wersje bezpośrednich crate'ów Rust pochodzą z lockfile migawki Harness. Nie przeniesiono repozytorium, migracji, danych RV, sekretów ani jego domenowych ustawień. Nowa domena kernel/ustawienia powstała dla AI Home.

Plan przeniesienia gotowych providerów, retry/continuation, sekretów, importerów i mechanizmów notatek zachowuje P1. Faktyczna adaptacja providerów należy do 4B-1, importerów do 4B-2A, pamięci do 4B-3/4. Nie deklarujemy wykonania tych przenosin w tym pakiecie.

## Korekta CI po pierwszej kompilacji

Logi Edwarda ujawniły niezgodną rodzinę zależności Tauri. Przypięto runtime/makra/build na zestaw migawki Harness; poprawka czeka na CI. `docs/CI_PL.md` opisuje generację i zachowanie Cargo.lock, zwykłe testy pełnego desktop feature oraz oddzielną ręczną kompilację kandydata. Nie ma publikacji wydania po każdym etapie.
