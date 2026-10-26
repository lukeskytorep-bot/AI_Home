# AI Home — audyt 4B-12 krok 1: publiczny odczyt Community

Data: 26.10.2026. Werdykt: **DO KOREKTY** — jedna niezgodność z kontraktem odczytu odebranego forum 4A.

Podstawa: AUDIT_PACKAGE, osobny CHANGED_FILES, raport, plan 1.73, instrukcja, VERIFY i OVERLAY_PROOF oraz dołączony pełny referencyjny kod forum. Audyt nie zmienia implementacji.

## F1 — P2: publiczne wątki locked są odrzucane jako niepoprawna odpowiedź

Lokalizacja klienta: `src-tauri/src/community_connector/mod.rs`, `content()`.

Klient dopuszcza tylko statusy active/deleted, a po obsłudze tombstone dodatkowo wymaga statusu active. Tymczasem referencyjne 4A obsługuje również locked jako stan treści: `storage/sqlite3-repository.php::setContentStatus()` dopuszcza go jawnie. `includes/content-service.php::requirePublicVisibility()` odrzuca hidden, lecz nie locked; `publicContent()` zwraca dla locked normalny publiczny payload ze statusem locked. Repozytorium listy wątków wyklucza hidden, nie locked. Zablokowanie pisania nie oznacza ukrycia publicznego odczytu.

Skutek:

- `thread(id)` odrzuci poprawny publiczny zablokowany wątek;
- `threads(board,page,limit)` odrzuci całą stronę z InvalidResponse, jeżeli choć jeden jej element ma locked. Użytkownik utraci również pozostałe poprawne wątki na tej stronie.

Dowód stanowi porównanie rzeczywistego kodu PHP z oboma warunkami walidacji Rust. W pełnym referencyjnym archiwum test `tests/forum-signed-revisions-self-test.php` korzysta z setContentStatus(..., locked, ...), więc nie jest to wymyślony stan przyszłego API. Nie uruchomiono PHP ani tego scenariusza w natywnym kliencie Rust.

Wymagana poprawka: odzwierciedlić publiczną semantykę locked w walidatorze; zmienić zarówno listę dopuszczonych statusów, jak i późniejszy warunek active dla normalnego payloadu. Zachować ochronę przed hidden, nieznanymi statusami i payloadem w tombstone. Nie zmieniać automatycznie locked na active.

Regresje: rzeczywisty reqwest/loopback z payloadem zgodnym z publicContent — locked thread detail, mieszana strona active+locked, nadal odrzucany hidden/nieznany status, nadal poprawnie obsłużony deleted/tombstone. Sprawdzić, że normalny locked zachowuje treść oraz oryginalny status.

## Ocena zakresu

Publiczny odczyt jest oddzielnym, natywnym serwisem. Nie otrzymuje tokenów ani executora, nie zapisuje automatycznie treści w pamięci i nie jest jeszcze IPC/UI. Odpowiedzi pozostają UntrustedExternal z SignatureValidation::NotPerformed; deklaracja podpisu po stronie serwera nie jest utożsamiana z weryfikacją lokalną.

Konfiguracja HTTPS/subpath, lokalne budowanie tras, odcięcie redirect/retry/proxy, limity body i czasu oraz bezpieczne błędy są spójne z zadeklarowanym zakresem. Brak live forum, rejestracji, podpisanych zapisów i feedu jest jawną granicą kroku 1, nie wadą tej paczki. Całe 4B-12 pozostaje otwarte.

## Integralność

Niezależnie porównano rzeczywiste ZIP-y: dołączona baza 4B-11 AUDIT FIX 1 + embedded overlay = SOURCE bajtowo. Osobny załączony overlay jest identyczny z embedded overlay. CRC i unikalność nazw porównywanych archiwów: PASS.

- SOURCE SHA256: `b5d5e2c195438476a3f2aff420a5090661326d27df0350983edbd253f43d51ee`.
- CHANGED_FILES SHA256: `0afa6b62a5a92b36d3da008a155987113fcb6c75daf9f6692882eb1b21952932`.
- verify-source przed instalacją/buildem: PASS — 667 manifest hashes / 668 SHA256SUMS entries.

## Niezależnie wykonane kontrole

- npm ci: PASS; Vitest 219/219, 21 plików: PASS; TypeScript i Vite build: PASS.
- P2 reference checks: 193/193 PASS.
- Community reader static guard: 12/12 PASS.
- Experimental foundation/runtime/continuity/internal state/reflection: 12/12, 24/24, 23/23, 23/23, 16/16 PASS.
- Command parity: 54/54; boundaries: 34/34 i 13 asercji schematu v15; Standard closure/release: 8/8 i 8/8; Rust source-text: PASS.
- Próba Cargo tests offline: NIEURUCHOMIONE — odnaleziony cargo jest launcherem rustup; brak zainstalowanego toolchainu i wersji domyślnej. Polecenie zakończyło się przed kompilacją. Wyniku 431/431 z raportu autora nie przypisuję własnemu wykonaniu.
- PHP/live Community HTTPS, desktop Tauri/WebView, OS vault i live model API: NOT RUN.

## Dalszy odbiór

Poprawić F1, dodać regresje protokołu locked, uruchomić pełny natywny CI dla nowej paczki i przekazać ją do reaudytu. Nie zatwierdzam jeszcze kroku 1 jako zamkniętego. Poprawka dotyczy zgodności odczytu; nie wymaga rozszerzania zakresu do identity ani zapisów.
