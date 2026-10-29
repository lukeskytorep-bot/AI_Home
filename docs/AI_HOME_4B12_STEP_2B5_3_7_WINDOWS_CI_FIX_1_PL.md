# AI Home — 2B-5.3.7 Windows CI Fix 1 oraz publiczny README

Data: 29.10.2026. Status: IMPLEMENTED / AWAITING WINDOWS CI AND AUDIT.

## Baza i wejście

Dokładny SOURCE po DOC FIX D1: SHA-256 ae6c306583893dbc1c1dc0f69bacd1b7df1ee36668f23dcc6b931ae70d5cf176. Zachowano UI, D1 i całą historię planu. Wejściowy log Windows: biblioteka 306 PASS / 8 FAIL. Dostarczone native-check-evidence Ubuntu/Windows zawierają Cargo.lock, wygenerowane schemas, ACL i materiały build-script, bez executable. community_dispatch występuje w obu wygenerowanych konfiguracjach. Lockfile różni się CRLF/LF; po normalizacji jest identyczny, nie stanowi to podstawy do zmiany zależności.

## Trzy poprawki

### 1. Kanoniczne fixtures i checkout Windows

Cztery awarie dotyczą niezależnych bajtów profilu/statement/proof oraz podpisów. Log pokazuje różnicę LF (10) versus CRLF (13,10). Dodano text eol=lf dla obu katalogów fixtures community-identity i community-completion. Wszystkie 21 plików, w tym podpisy, jest identyczne bajtowo z bazowym ZIP-em. Nie zastąpiono wektorów, nie zmieniono verify_signature, nie dodano normalizacji, trimowania signed text lub akceptacji obu kodowań w protokole.

Własna próba używa rzeczywistego Git z core.autocrlf=true. Bez reguł odtworzono zmianę 21 plików podczas checkout; po dodaniu reguł wszystkie 21 odtwarza się bajtowo. Osobno sprawdzono, że już istniejący CRLF working tree jest normalizowany do oryginalnego index po git add pod nowymi regułami. Skrypt i wynik w evidence. Dla aplikowania ZIP na Windows nakładka dodatkowo zawiera te same, niezmienione kanoniczne fixtures, aby zastąpić wcześniejszy CRLF working copy przed commitem.

### 2. Kontrola danych na dysku bez odczytu aktywnych blokad

Trzy testy poufności czytały całe katalogi podczas otwartego SQLite i registry owner-lock. Windows zgłaszał error 33 na aktywnych blokadach. Zmieniono tylko sposób obserwacji w testach: helper test_storage::bytes_after_close bierze maintenance lease, zapisuje dokładne bajty istniejącego live WAL, zamyka registry/pool, zwalnia lease naturalnie, a potem czyta wszystkie pozostałe pliki. Zachowano oryginalne listy forbidden strings i asercje. Nie pomija się plików lock/SHM, nie wymusza unlock, nie usuwa WAL/SHM ręcznie i nie ignoruje błędów odczytu. Brak WAL jest jedynym dopuszczonym NotFound podczas pobrania live WAL.

Utrwalenie live WAL przed close zapobiega osłabieniu testu przez checkpoint usuwający wcześniejsze bajty. Nowa regresja zapisuje sentinel w WAL, potwierdza identyczną przechwyconą zawartość i odczyt każdego pozostałego pliku, w tym Main registry, owner-lock oraz dodatkowego pliku. Kod helpera jest wyłącznie cfg(test), nie jest API produkcyjnym.

### 3. Rzeczywisty błąd sync w staging restore

copy_verified_source otwierał skopiowany plik przez File::open (read-only), a następnie sync_all. Windows FlushFileBuffers wymaga dostępu do zapisu, co wyjaśnia restore staging sync failed. Zastąpiono otwarcie przez OpenOptions read+write. Sync pozostaje obowiązkowy, a błąd jest propagowany przed publikacją staged database. Nie ma cfg(windows) pomijającego sync, ignorowania błędów ani zmian safety backup/journal/checksum/recovery policy.

Źródło wymagania Windows: [Microsoft FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers). Sprawdzono oficjalny opis dostępu GENERIC_WRITE. Nie zmieniono semantyki podpisów ani protokołu Community.

## Osiem pierwotnych przypadków

| Test | Zakres poprawki |
|---|---|
| legacy_actual_writer_receipt_stays_valid_after_later_main_changes | write-capable staging sync |
| proofs_match_independent_python_contract_fixtures_and_signatures | kanoniczny checkout LF |
| independent_signature_vectors_verify_for_both_registration_roles | kanoniczny checkout LF |
| p256_generic_fixture_verifies_and_p384_is_rejected | kanoniczny checkout LF |
| profile_and_statement_match_independent_php_registration_fixtures_byte_for_byte | kanoniczny checkout LF |
| sqlite_and_public_progress_do_not_contain_bearer_tokens | live WAL + natural close + pełny odczyt |
| unvalidated_extra_response_fields_and_server_request_id_are_not_persisted | live WAL + natural close + pełny odczyt |
| registry_contains_no_invite_or_private_key_bytes | live WAL + natural close + pełny odczyt |

Wszystkie osiem przechodzi w wykonanej regresji Linux po poprawce. To nie jest wynik Windows; pełny Windows run nadal jest obowiązkowy.

## README do publicznego repozytorium

Nowy README po angielsku zastępuje historyczny nagłówek i raport backupu 2B-5.2. Opisuje cel aplikacji i dwa kierunki, bieżące funkcje i komponenty niebędące jeszcze pełnym UI, granice Community i Experimental, budowanie/uruchomienie, browser preview, dane i poświadczenia, testy, strukturę projektu, roadmap i istniejącą licencję MIT. Zawiera instrukcję pobrania desktop-candidate z CI i odróżnia go od native-check-evidence bez aplikacji. Odsyła do oficjalnych prerequisites Tauri 2 i istniejącej procedury acceptance.

Nie deklaruje pełnego odbioru Windows ani uniwersalnego dostępu do dowolnego forum. Nie zmieniono licencji, ustawień prywatności repozytorium ani package.json private. README jest gotowy do publikacji razem ze źródłem; ten krok nie stanowi deklaracji przeprowadzenia osobnego przeglądu wszystkich historycznych plików pod kątem publikacji. Nie publikowano repozytorium ani release.

## Wykonana weryfikacja

- Rust Linux headless: 662/662 PASS, 23 zestawy, --all-targets --no-default-features --locked --offline. Jedna nowa regresja WAL/full-scan; żadnych pominiętych testów.
- Frontend: 235/235, 23 pliki PASS. TypeScript/Vite build PASS.
- Wszystkie 12 istniejących npm verify PASS; boundaries 35, 14 migration assertions, parity 64/64; P2 193/193 PASS.
- Rzeczywisty Git autocrlf roundtrip: RED (21 zmienionych fixtures bez reguł), GREEN (21/21 exact po regułach), zachowany index przy istniejącym CRLF working tree.
- Linki lokalne README sprawdzone; wskazują istniejące pliki. Oficjalne wymagania systemowe: https://v2.tauri.app/start/prerequisites/.
- SHA bazy, niezmienione fixtures/protokół/lock/dependencies, brak usunięć, pełne manifesty i BASE + overlay = SOURCE bajtowo zweryfikowane podczas pakowania.

Rust 1.99.1 / Cargo 1.99.1, Node 24.22.1, npm 11.9.0. Wcześniejsze ostrzeżenie unused InternalStateProvider pochodzi z niezmienionego testu. Nie wykonywano Windows/MSVC, default-feature Tauri, WebView, rzeczywistego OS vault, live forum/TLS lub power-loss. Nie wolno na podstawie Linux PASS zapisać Windows PASS.

## Wdrożenie i dalszy odbiór

Nałożyć CHANGED_FILES na dokładną bazę 2B-5.3.7 DOC FIX D1, albo użyć pełnego SOURCE z tej poprawki. Nie tworzyć AI_Home/AI_Home i nie mieszać dwóch alternatywnych źródeł. Nakładka celowo zawiera także byte-identical fixtures; nie usuwać ich z niej podczas aplikowania na stary Windows checkout. Zaktualizowano kompletny manifest i SHA256SUMS.

Wykonać commit/push. Uruchomić AI Home checks z build_desktop=true, aby ponownie wykonać Windows i zbudować executable dopiero po pełnym PASS. Pobierać desktop-candidate-windows-2022, nie native-check-evidence-windows-2022. Materiały z nieudanego starego runu nie dowodzą poprawności nowej wersji.

Plan 1.95 odnotowuje rzeczywisty Windows FAIL i ponownie otwartą bramkę. D1, historia 1.94 i wcześniejsze decyzje zachowane. Po nowym CI i audycie dopiero realny odbiór według COMMUNITY_DESKTOP_ACCEPTANCE_PL.md. Całe 2B-5.3/2B/4B-12 pozostają OPEN; maintenance UI, signed writes/feed i dalsze etapy nie są w tym kroku wykonane. MCP po obowiązkowym przeglądzie OSS, przeglądarka odłożona. Bez release.
