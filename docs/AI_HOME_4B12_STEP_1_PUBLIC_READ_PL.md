# AI Home — 4B-12 krok 1 — publiczny odczyt Community

Data: 26.10.2026. Status: IMPLEMENTED / AWAITING CI AND AUDIT. Całe 4B-12 pozostaje otwarte.
Baza: zielony według Edwarda `AI_HOME_4B11_AUDIT_FIX_1_SOURCE.zip`, SHA-256 `8ff8cbfc9066ce2690a377c43813fc4b896db3935183f7413f055955ebafee0b`, 637 plików. Plan treść 1.73.

## Wykonany zakres

Dodano niezależny, natywny `community_connector::CommunityReader`, korzystający z rzeczywistego reqwest HTTP GET. Publiczne metody Rust to `new`, `discover`, `threads`, `thread`, `replies`. Nie są to jeszcze polecenia IPC ani ekran desktopowy. Każdy wynik posiada lokalnie przypisany profile_id i api_root, oznaczenie UntrustedExternal i SignatureValidation::NotPerformed. Ani treść forum, ani capability discovery nie inicjują modelu, narzędzi lub publikacji.

Wskazany jawnie URL API musi używać HTTPS i kończyć się /api/v1/. Obsługiwany jest podkatalog wdrożenia. Odrzucane są credentials w URL, query/fragment, znaki kontrolne, kodowane i względne dot-segments. Żądania są budowane wyłącznie z lokalnych stałych tras i walidowanych identyfikatorów. Brak przekierowań, automatycznych powtórzeń, cookies, tokenów i dziedziczenia proxy ze środowiska. Nie podążamy za żadnymi URL-ami zwróconymi przez forum. Konfiguracja nie jest automatycznie odkrywana z treści wiadomości.

Każde wywołanie pobiera jedną stronę; limit 1–50, page 1–10000, bez automatycznego przewijania. Domyślnie 4 MiB odpowiedzi i 15 sekund na całe wywołanie; konfigurowalna granica timeout do 30 sekund. Limit rozmiaru działa zarówno z Content-Length, jak i podczas pobierania bez zadeklarowanej długości. Deadline obejmuje oczekiwanie na nagłówki oraz odczyt body. 429 zwraca oddzielny bezpieczny błąd z opcjonalnym liczbowym Retry-After do 86400 sekund, bez samoczynnego retry. Inne błędy nie kopiują treści odpowiedzi serwera ani URL-a do komunikatu.

Sprawdzamy JSON envelope, Content-Type, request_id, wersję API/schema, identyfikatory i związanie odpowiedzi z wybraną tablicą/wątkiem, paginację, podstawowe typy, rozmiary tekstu, daty, tombstones i duplikaty rekordów. Tytuł, body i Unicode są zachowywane jako dane. Ta walidacja nie dowodzi prawdziwości treści ani integralności podpisu. API publicznych odczytów nie zwraca pełnego materiału potrzebnego do lokalnej weryfikacji podpisu; server_evidence.signature_verified jest wyłącznie deklaracją serwera.

## Zgodność z odebranym forum 4A

Użyto `AI_Community_4A8C4_FINAL_COMPLETE_SOURCE_CANDIDATE_V4.zip`, SHA-256 `afe25c58590ec146f7be01ef83fc4854e717813bb99c86cd7d2c89251e4ebc8a`. To tożsamość wskazana w `4A-8C-4_PRODUCTION_ACCEPTED_FINAL_CLOSURE.md` z 6.10.2026: produkcja odebrana, community schema 9, operator schema 1, API root przez root.php / DirectoryIndex root.php index.php. Przeczytano rzeczywiste PHP, health, route rewrites i kształt publicContent, nie oparto klienta na starej propozycji Farsight/Vimeo.

| Odczyt | Trasa względem API root | Parametry/odpowiedź |
|---|---|---|
| Discovery | GET / | api_version v1, sqlite3 schema 9, migration_state current, capabilities |
| Lista wątków | GET threads/ | board_id, page, limit; items z preview zamiast body |
| Wątek | GET threads/{msg_id} | thread, zgodny content_id/thread_id |
| Odpowiedzi | GET threads/{msg_id}/replies | page, limit; thread_id i items z body |

Nie wymyślono endpointu /boards — przyjęta wersja 4A nie ma go w tym API. General ma po stronie 4A domyślne ID brd_00000000000000000000000000000001, lecz klient wymaga jawnego board_id. /events wymaga Bearer i forum.read, dlatego nie jest udawany jako publiczny odczyt ani implementowany w tym kroku.

W `contracts/community.read.v1.json` zapisano inwentarz lokalnej usługi i tras, nie nowy standard serwerowy/IPC. `docs/evidence/4b12-step-1/forum-reference/` zawiera tożsamość ZIP-a i dokładne kopie użytych PHP/route/storage plików z hashami. Statyczny guard weryfikuje ich niezmienność; nie zastępuje uruchomienia forum.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux, cargo test --no-default-features --all-targets --locked --offline | **431/431 PASS**, 22 zestawy |
| Nowe testy CommunityReader (zawarte powyżej) | **17/17 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| Frozen P2 reference checks | **193/193 PASS** |
| Granice i schema expectation sweep | **34/34 PASS**, 13 asercji do v15 |
| Command parity | **54/54 PASS** |
| Community reader static/source guard | **12/12 PASS** |
| Experimental foundation / runtime / continuity / internal state / reflection | **12/12 / 24/24 / 23/23 / 23/23 / 16/16 PASS** |
| Standard closure / release, Rust source-text | **8/8 / 8/8 / PASS** |

Polecenia i pełne logi są w docs/evidence/4b12-step-1. Środowisko: Rust 1.99.1, Node 24.22.1, Linux headless. Brak testu desktop feature/Tauri, PHP, systemowego vault i realnego serwera Community. Wcześniejsze ostrzeżenie unused import w teście experimental_internal_state pozostaje bez zmian. W początkowym roboczym teście poprawiono mutowalność odbiornika oneshot; finalny zestaw wykonano w całości po korekcie.


17 nowych testów obejmuje: konfigurację HTTPS/subpath i limity, discovery bez credentials, dokładne GET i płaskie query, Unicode i nieufne treści, scope odpowiedzi, odrzucenie niepoprawnego żądania przed siecią, redirect, 429/401/503, timeout nagłówków i body, ograniczenie długości deklarowanej i strumieniowanej, HTML/błędny JSON/błędny envelope, niezgodne discovery, duplikaty, paginację, tombstones, typy metadanych i limity body. Testy HTTP używają lokalnego serwera TCP i rzeczywistego reqwest. HTTP loopback konstruuje klienta wyłącznie wewnątrz cfg(test); produkcyjny konstruktor nie ma przełącznika pomijającego HTTPS.

## Granice i dalsza praca

To pierwszy działający adapter publicznego odczytu, nie ukończony CommunityConnector. Nie uruchamiano PHP ani działającego forum, nie wysyłano na produkcję żadnych zapisów i nie używano prawdziwych tokenów. Wierność protokołu sprawdzono względem źródeł PHP i kontrolowanych odpowiedzi HTTP. Nie potwierdzono HTTPS/TLS na rzeczywistym hostingu, Windows/Tauri/WebView, OS vault i live model API.

Brak nowej trwałej warstwy: wyniki istnieją w RAM, nie są automatycznie zapisywane w pamięci ani historii. Backup/restore, diagnostyka i deletion nie zyskują nowego magazynu surowej treści. Frozen P2, SQLite v15, istniejące testy pamięci, profile, vault, provider adapters, zależności/lockfiles, capabilities i workflow pozostają identyczne z bazą. Zmiany w istniejącym kodzie runtime to wyłącznie eksport nowego modułu w lib.rs; package.json dostaje komendę dodatkowego guarda. Dotychczasowe cargo test --all-targets automatycznie obejmuje nowe testy.

Następny krok: dual-key identity/registration i namespace sekretów Community, oddzielny od kluczy modelu. Następnie podpisane write/receipts/idempotency i bezpieczne podłączenie do decyzji Primary, a później feed/cursor/restart. Kształt i canonical bytes podpisów będą zaczerpnięte z odebranego 4A. Root discovery nie jest dowodem tożsamości ani uprawnieniem do pisania. Messenger/A2A to osobny etap 4B-13, z niezależnym kontraktem; API 4A deklaruje je obecnie jako false.

## Przekazanie

CHANGED_FILES jest nakładką na dokładną zieloną bazę 4B-11 AUDIT FIX 1. Rozpakować do głównego katalogu AI_Home (package.json), nadpisać pliki, niczego ręcznie nie usuwać. Windows: `py -3 scripts/verify-source.py .`, potem commit/push i dotychczasowy CI. Nie potrzeba release ani instalatorów. SOURCE to komplet źródeł. AUDIT_PACKAGE zawiera również dokładną bazę AI Home, raport, plan i dowody, a OVERLAY_PROOF odtwarza rzeczywiste BASE+OVERLAY=SOURCE=working directory. Nowy krok wymaga własnego CI i audytu.
