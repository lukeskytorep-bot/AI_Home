# AI Home — 4B-12 krok 2B-2 — trwały registration/start przez HTTP

Data: 27.10.2026. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 2B i 4B-12 pozostają OPEN. Plan treść 1.77.

Baza: `AI_HOME_4B12_STEP_2B1_SOURCE.zip`, SHA-256 `ba853712ee00c980183d4e602cde5f4bcea0f200d64a19e9034a7ac1ae78df4b`, 770 plików. Edward potwierdził zielony wynik; nowego numeru GitHub CI ani dodatkowego raportu audytora nie pobierano. Zachowano dokładne źródła/reference forum 4A z poprzedniego przekazania.

## Co działa

Dodano natywny `community_identity::registry::start::RegistrationStart`. Jest to rzeczywisty reqwest POST do API root + registration/start/, z obsługą podkatalogu. Nie jest to mock transportu w kodzie produkcyjnym. W testach prywatny override, kompilowany wyłącznie z cfg(test), kieruje reqwest na loopback HTTP. Produkcja wymaga jawnego HTTPS CommunityScope.

start() wymaga gotowego wpisu 2B-1, ładuje i sprawdza faktyczne klucze oraz rekonstruuje pierwotny RegistrationIntent. Następnie zapisuje hash dokładnego request body i ordinal zamiaru dispatchu w SQLite przed wysłaniem. Invite i request body pozostają w pamięci; klucze i start ID nie zmieniają się przy ponowieniu. Konflikt innego invite kończy się przed kolejnym POST-em. Hash kompletnego żądania jest wrażliwą metadaną koordynacji, nie treścią diagnostyczną i nie zaszyfrowanym invite.

Po utracie odpowiedzi możliwe jest jawne ponowienie z tym samym invite: serwer 4A deduplikuje client_start_id i powiązane wejście. Nie ma automatycznego ponowienia, nowych kluczy ani HTTP podczas otwarcia bazy/startu aplikacji. Ordinal liczy trwały zamiar wysłania; cancel między zapisem markera a HTTP może oznaczać zero fizycznych POST-ów dla danego ordinalu. Nie jest to licznik płatnych wywołań providera.

Zweryfikowany challenge jest projektowany na zamknięty zbiór pól publicznego kontekstu i zapisywany przed zwróceniem wyniku. Arbitrary extras, request_id serwera, echo invite i pola tokenów nie są utrwalane. UPDATE ma CAS po ordinalu: spóźniona pending response nie nadpisuje nowszego committed; spóźniony błąd nie kasuje sukcesu. Błąd zapisu wyniku po POST pozostawia trwały stan niejednoznaczny i nie zwraca fałszywego sukcesu.

cached() nie bierze invite i nie robi HTTP. Ładuje aktualne klucze i ponownie sprawdza oryginalny profil/scope oraz czas. Zapisany expired pending zwraca Expired bez ponowienia. progress() daje licznik zamiarów, obecność kontekstu i lokalny kod ostatniego błędu. has_context oznacza wyłącznie obecność, nie aktualność, autoryzację ani zezwolenie na publikację.

## Walidacja i transport

Odpowiedź pending musi odpowiadać oryginalnemu profile hash, scope, kluczom, lokalnie skonstruowanemu AGENT-REGISTRATION/1, suite, formatom, canonical nonce/ID i dacie UTC. Czas walidacji jest pobierany po odpowiedzi, nie przed rozpoczęciem HTTP. Challenge nie może być przeterminowany ani wykraczać ponad 24h. Istniejąca kryptografia 2A pozostaje bez zmian.

Committed start również musi przejść pełną kontrolę kontekstu. Historyczna data expiry może być w przeszłości, bo to informacja o zakończonej rejestracji, nie nowy challenge do podpisania. Wynikiem jest wyłącznie RecoveryRequired(registration_id). Nie przyjmujemy tokenu, nie ogłaszamy logowania i nie eksponujemy committed jako pending. Publiczne validate_challenge 2A zachowuje poprzednią semantykę; nowy koordynator korzysta z wewnętrznej pełnej walidacji także dla committed.

Dedykowany klient nie podąża za redirect, nie wykonuje auto retry, nie używa proxy ani credentiali providera/cookies/bearer. Endpoint nie pochodzi z odpowiedzi serwera. Cały HTTP send/headers/body jest objęty jednym deadline 1ms..30s. Wymagany jest status 200 i MIME application/json. Limit 16384 bajtów jest egzekwowany zarówno po Content-Length, jak i podczas czytania chunked. 429 zwraca jedynie liczbowy Retry-After do 86400. Błędy nie zawierają raw reqwest error string, URL-a, body lub komunikatu serwera.

Zeroizing oryginalnego StartRequest i bufora odpowiedzi nie obiecuje usunięcia wszystkich kopii: reqwest otrzymuje własny bufor żądania, serde_json/platforma także mogą tworzyć kopie. Nie dodano plaintext fallbacku dla prywatnych kluczy ani przechowywania invite w SQLite.

## Migracja i ograniczenia

Dedykowana baza rejestru Community przechodzi atomowo schema 1 -> 2. Nowa tabela community_registration_starts wiąże scope, request SHA256, ordinal, projekcję kontekstu i lokalny kod błędu. Pierwotny CREATE TABLE preparation jest identyczny z 2B-1, co sprawdza dowód źródłowy. App Store pozostaje v15. Test migracji zachowuje dane przygotowania/kluczy i nie wykonuje HTTP. Jedyna zmiana starego testu to fixture przyszłej wersji z 2 na 3, żeby nadal sprawdzała odrzucenie wersji wyższej niż obecna v2. Reszta poprzednich testów jest niezmieniona.

Pierwsze poprawnie zbudowane żądanie wiąże invite przez hash. Nawet gdy invite był błędnie wklejony, nie można podstawić innego pod ten sam start ID. Kontrolowane anulowanie/uzgodnienie nie jest jeszcze zaimplementowane; nie należy usuwać bazy ani generować nowej tożsamości jako obejścia. Claim provision nadal nie wygasa automatycznie, aby nie ścigać się z opóźnionym OS-vault writerem.

Aplikacja nadal nie otwiera automatycznie rejestru i nie wywołuje start. Brakuje host profile direction/capability checks, integracji profile.delete.permanent, backup/restore, cleanup i IPC/UI; przed udostępnieniem rejestracji użytkownikowi są obowiązkowe. Pełny backup samego Store nie obejmuje osobnej bazy rejestru ani sekretów Community. Raw low-level provision wymaga exclusivity i nie może omijać koordynatora.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless all-targets, locked/offline | **499/499 PASS**, 22 zestawy |
| Nowe testy startu HTTP (w powyższym przebiegu) | **25/25 PASS** |
| Końcowa kontrola 25 testów po wzmocnieniu byte equality | **25/25 PASS** — te same przypadki, nie dodatkowe 25 |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| P2 reference / granice | **193/193 i 34/34 PASS** |
| Asercje migracji aplikacji | **13, aktualny Store v15** |
| Command parity | **54/54 PASS** |
| Standard closure / release | **8/8 i 8/8 PASS** |
| Experimental foundation/runtime/continuity/internal/reflection | **12/24/23/23/16 PASS** |
| Community reader / identity static guards | **14/14 i 14/14 PASS** |
| Rust source-text, manifest, ZIP i rekonstrukcja nakładki | **PASS** — dowody w pakiecie |

Nowe testy obejmują dokładną trasę i bajty POST, brak credentiali w headers, zachowany podpisywalny challenge, zamknięcie/ponowne otwarcie bez invite/HTTP, utratę odpowiedzi z anulowaniem i jawny retry z identycznymi bajtami, konflikty invite, usuwanie extras, committed/expired oraz złe scope/statement/hash, korupcję cache, JSON/MIME, redirect, 429, Content-Length/chunked, deadline nagłówków, brak kluczy, błąd zapisu przed/po POST, spóźniony sukces/błąd i migrację v1.

To realny reqwest na loopback, prawdziwe tymczasowe pliki SQLite i mock backend sekretów. Nie uruchamiano PHP ani żywego forum, OS vault, TLS deploymentu, Windows/Tauri/WebView czy API modeli. Close/reopen nie jest restartem procesu systemowego. Nie wykonywano nowego OpenSSL CLI; historyczne dowody i niezależne fixtures 2A pozostają w bazie, a ich testy Rust przechodzą ponownie. Pozostaje wcześniejszy warning unused InternalStateProvider w niezmienionym teście.

## Przekazanie i następny krok

Nakładka jest policzona na dokładnej bazie 2B-1 i niczego nie usuwa. Pakiet audytowy zawiera tę bazę, pełne forum reference, nakładkę, wynikowy SOURCE, raport, plan 1.77, developer guide i logi. Dokładne baza + nakładka = SOURCE oraz kompletne hashe są zweryfikowane. Cargo.toml/lock, package lock, frozen P2, workflow, provider vault/adapters i uprawnienia są bajtowo zgodne z bazą. Dotychczasowe cargo all-targets CI wykrywa testy bez zmiany jobów; nie tworzymy release ani instalatorów.

Następny odbiór 2B musi połączyć complete POST z trwałym postępem, oddzielnym custody jednorazowego tokenu, proof-status i reissue po utracie odpowiedzi. Nie wolno wysyłać complete osobnym prowizorycznym kodem bez odzyskiwania. Potem lifecycle/backup/restore i host/IPC przed włączeniem użytkownikowi; podpisane zapisy i feed dalej w 4B-12. Ten etap nie zamyka 2B ani całego 4B-12.
