# AI Home — 4B12 / 2B-5.3.7 — UI rejestracji Community

Data: 29.10.2026. Status: IMPLEMENTED / AWAITING NEW CI AND INDEPENDENT AUDIT.

## Baza i zakres

Dokładna baza: SOURCE 2B-5.3.6, SHA-256 `63d556a2cc2332fa8b6085dbe080074c27b963f54d83ad6dd48b274a0c077c26`. Edward potwierdził zielone CI bazy. Nie uruchomiono ponownie tego zewnętrznego CI.

Dodano panel w głównym oknie AI Home, adapter supplemental IPC i opcjonalne communityDispatch w Transport. Native transport kieruje te operacje do community_dispatch; zwykły ruch pozostaje w home_dispatch. Browser preview nie symuluje rejestracji. Kontrakty frozen P2 i supplemental JSON, Rust, Tauri ACL, migracje, zależności i workflow pozostają byte-identical do bazy.

## Zachowanie

- Lista profili Standard bez archived; osobne odświeżenie listy. Experimental nie jest przez ten panel uruchamiany ani przestawiany. Natywny host nadal sprawdza autorytatywny binding i wszystkie uprawnienia.
- Odczyt bieżącego stanu jest jawny. Pobiera świeżą rewizję Main, preparation status, finish progress i access.status. Nie wywołuje HTTP rejestracji. Odczyt po udanej operacji też dotyczy tylko tych danych lokalnych.
- Każdy zapis jest osobnym kliknięciem. Synchronizowany ref i disabled controls zapobiegają podwójnemu kliknięciu; nie ma timerów ani automatycznych ponowień.
- Zaproszenie jest osobnym argumentem IPC, wyłącznie dla start, z limitem 200 bajtów UTF-8 i zakazem CR/LF/NUL. Nie trafia do envelope ani browser storage. Pole jest czyszczone przed oczekiwaniem na wynik, po zmianie profilu i po odświeżeniu listy. JavaScript nie daje gwarancji wyzerowania pamięci; po stronie natywnej pozostaje Zeroizing.
- Po transporcie niepewnym lub dowolnym błędzie stan jest unieważniany. Najpierw jawny odczyt; potem użytkownik wybiera uzgodnienie. Nowy request ID przy nowej jawnej operacji, nigdy ślepe ponowienie starego POST.
- Po restarcie uncertain odczytuje dodatnią finishRevision przez autoryzowany progress. Retry complete używa tej rewizji. ready_metadata nie oznacza usable ani online. Zero nie nadaje się do retry/recover access.
- Odpowiedzi walidowane według istniejącego supplemental kontraktu, zgodności requestId i inwariantów revision/state. Błędy pokazują lokalizowane komunikaty; surowe wyjątki nie trafiają do UI.
- Docelowy serwis i capability nadal wynikają wyłącznie z lokalnego community.desktop.json. Panel nie ma pola do nadawania uprawnień ani zmiany celu. Brak konfiguracji lub zakaz pojedynczej komendy daje odmowę, bez fallbacku.

## Weryfikacja wykonana tutaj

- Frontend: 235/235, 23 pliki; 16 nowych testów (15 UI/adapter i jeden native transport).
- Build i TypeScript PASS.
- Frozen P2: 193/193; boundaries 35 i 14 sprawdzeń migracji; parity 64/64; Rust source-text PASS.
- Statyczne guards Community identity i reader: po 14/14 PASS.
- Nowe testy obejmują: browser/no transport, brak automatycznych zapisów/odczytów przy wyborze, Standard filtering, poprawne rewizje, out-of-band invite, transport failure, restart/uncertain/retry, metadata versus usable, zmianę profilu, prepare bez provision, revision conflict, błędne odpowiedzi, late unmount, odświeżenie listy, sprzeczne metadane, walidację zaproszenia, double click oraz rozdzielenie routingu Tauri.

## Ograniczenia i otwarte bramki

To raport wykonania, nie niezależny audyt własnego kodu. Testy DOM używają fake transportu. Rust nie został ponownie uruchomiony: pliki backendu i kontraktów nie zmieniły się. Nowe CI dla kompletnego SOURCE pozostaje wymagane.

Próba uruchomienia wizualnego Playwright była BLOCKED: brak executable Chromium w środowisku. Nie deklarujemy odbioru wyglądu ani WebView. Nie wykonano realnego Windows/OS vault/live forum/TLS ani power-loss. Szczegółowa procedura w COMMUNITY_DESKTOP_ACCEPTANCE_PL.md. Nie włączono Community na cudzej instalacji, nie wysłano zaproszenia, nie utworzono konta ani kluczy poza testami fake. UI maintenance pozostaje oddzielnym zadaniem; istniejące odmowy backup/restore instalacji Community zachowane.

## Przekazanie

Nakładka wymaga dokładnej bazy 2B-5.3.6. Zalecane podmienić pełne SOURCE, aby uniknąć wcześniejszego problemu mieszanego drzewa. Zwykły commit/push i CI, bez release. Sprawdzić verify-source.py na paczce/directory, niezależny audyt, potem odbiór rzeczywisty. Nie zamyka to 2B-5.3 ani 4B-12.
