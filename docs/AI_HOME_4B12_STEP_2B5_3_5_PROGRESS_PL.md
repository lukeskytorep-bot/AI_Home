# AI Home 4B-12 — 2B-5.3.5 — autoryzowany bieżący progress rejestracji

Data: 29.10.2026. Status: IMPLEMENTED / AWAITING NEW CI AND AUDIT.

## Wynik i powód

Zrealizowano O1 z audytu 2B-5.3.4: recovery po niejednoznacznym POST/restart może pobrać aktualną rewizję przez autoryzowany host. Dotychczasowy test sięgał bezpośrednio do RegistrationFinish::progress. Nowa ścieżka umożliwia przygotowanie desktopu bez zgadywania rewizji i bez używania historycznego wyniku jako aktualnego stanu.

Baza: dokładny AI_HOME_4B12_STEP_2B5_3_4_SOURCE.zip, SHA-256 8f11ccba75968b738620015379df4f783706827d503350e8cbb63d8ce8fbc4c2. Edward potwierdził zielone CI; zewnętrznego run/commitu nie sprawdzano niezależnie. Audyt nie znalazł blokera kodu 2B-5.3.4. Obecne zmiany wymagają własnego nowego CI i audytu.

## Kontrakt

Komenda community.registration.progress, entry CommunityHost::dispatch_local, grant LocalAccess z osobnym LocalCapability::Progress. Pusty payload. expectedRevision to rewizja profilu Main. Grant jest natywny, nie deserializowany z JSON. Nie wolno wyprowadzać go z samego żądania, reklamy capability serwera ani skilla.

| Pole wyniku | Znaczenie |
|---|---|
| finishRevision | 0 przy absent; dodatnia bieżąca rewizja finish przy pozostałych stanach |
| finishState | absent, uncertain lub ready_metadata |
| onlineStatusKnown | zawsze false |

Brak registration ID, agent ID, generation, kluczy, key/token reference, zaproszenia, podpisanego pakietu i surowego payloadu serwera. ready_metadata opisuje lokalny zapis protokołu; nie potwierdza sprawności poświadczenia ani aktualnej akceptacji serwera. access.status nadal służy do świeżej kontroli lokalnej dostępności kluczy i tokena; późniejszy HTTP ponownie autoryzuje i ładuje poświadczenie.

## Autoryzacja i skutki

Wspólne kontrole obejmują profil/scope, Main revision, Standard, archiwizację, autorytatywny experimental_instances, pending deletion, trwały host binding, restore fence i registry lifecycle. Nie dodano owner override. Komenda lokalna jest odrzucana w dispatch_registration. Brak capability i self-grant JSON odmawiają. Host używa dotychczasowej bramki Ready/maintenance.

Odczyt jest świeży, bez receipt/replay. Nie czyta key/token vault, nie zmienia registry, nie zapisuje poświadczeń, nie wykonuje HTTP i nie naprawia uszkodzonych danych. Brak preparation lub finish daje absent; błędny zapis finish daje ograniczony STORAGE_ERROR. Gotowe metadane mogą pozostać ready_metadata po utracie sekretów, przy access.status=false.

Po niejednoznacznym complete/retry/recovery lub restarcie caller odczytuje progress. Następnie jawnie wysyła nowy complete.retry/access.recover z dodatnią finishRevision. Zero nie jest poprawną rewizją tych mutacji. Odczyt nie jest blokadą na przyszłość: jeżeli stan zmieni się przed mutacją, protokół zwraca konflikt. Caller ponownie odczytuje stan i podejmuje jawną decyzję. Nie dodano automatycznego ponowienia POST. registration.status pozostaje osobnym źródłem preparation registrationRevision.

## Weryfikacja

- Rust: 650/650 PASS, 23 zestawy, --all-targets --no-default-features --locked --offline; dodatkowy finalny filtr host_tests: 40/40. Pełny przebieg obejmuje finalny kod produkcyjny; końcowe wzmocnienie fixture restartu zweryfikowano oddzielnie w 40/40 testach hosta. Po rozpoczęciu pełnej regresji nie zmieniano kodu produkcyjnego ani kontraktu.
- Cztery nowe testy progress; wcześniejsze testy utraconej odpowiedzi, finalnego zapisu receipt i anulowania rozszerzone o docelową ścieżkę odczytu.
- O1: utrata odpowiedzi complete, historyczny replay bez ponownego POST, odtworzenie hosta oraz obu połączeń baz z dysku, progress przez host, jawny retry, ponowne odtworzenie hosta/baz, świeży progress przez host i proof recovery. Rejestrowe połączenie jest zamykane, wrappery vault odtwarzane; przeżywa jedynie backend testowego vault i symulowany serwer. W tym teście brak bezpośredniego odczytu rewizji registry. To kontrolowany restart właścicieli zasobów, bez symulacji force-kill procesu.
- Fresh read: ten sam requestId po zmianie stanu daje nowy wynik; brak dodatkowych receipts/HTTP/secret writes; ustawione testowe flagi błędu odczytu vault nie zostają zużyte przez progress.
- Denials: brak lub obcy grant, self-grant JSON, obcy profil, stale Main revision, niewłaściwy entry, archived/conscious, autorytatywny Experimental binding, deletion intent, restore fence i reconcile.
- Uszkodzony finish odmawia bez naprawy i bez HTTP. Finalny błąd zapisu Main po POST nie blokuje świeżego odczytu lokalnego finish; maintenance odmawia progress podczas trwającego workera.
- Frontend: 219/219; typecheck/Vite build PASS. P2 193/193, parity 64/64, boundaries 34/34, migration assertions 14 przy v16; wszystkie 12 istniejących npm verify PASS.

Pierwsze uruchomienie nowego testu polityki ujawniło błędny fixture: użyto nieistniejącego trybu experimental zamiast kontraktowego conscious, co prawidłowo dało STORAGE_ERROR. Skorygowano fixture, nie zmieniano polityki produkcyjnej. Finalne logi zawierają wynik po tej korekcie. Znane ostrzeżenie unused InternalStateProvider dotyczy niezmienionego wcześniejszego testu.

Rust 1.99.1, Node 24.22.1, npm 11.9.0. SQLite rzeczywiste, key/token stores testowe, HTTP loopback bez TLS. To nie jest odbiór Windows/WebView, default desktop, realnego OS vault, forum/PHP/TLS, force-kill ani power-loss/fsync failure.

## Zakres i następny krok

Zmiana runtime ograniczona do community_host/local.rs, supplemental community.local.ipc.json i testów hosta. Bez migracji, dependencies/lock, frontend, Tauri setup/permissions, workflow i frozen P2. Store v16, registry v4. Nie zmieniono protokołu HTTP ani zasad generacji tokena i oryginalnych kluczy. Plan 1.91 zachowuje całą historię 1.90/1.89/1.88.

Najpierw nałożyć CHANGED_FILES na zieloną bazę 2B-5.3.4, wykonać zwykły commit/push, nowe natywne CI i audyt. SOURCE to pełne alternatywne źródło. AUDIT_PACKAGE zawiera dokładną bazę, source, overlay, dowody i sumy; BASE + overlay = SOURCE bez usunięć. Nie nakładać obu ZIP-ów źródłowych jeden na drugi. Bez release/instalatorów.

Po odbiorze: produkcyjny desktop routing/authorization oraz UI rejestracji, realny desktop/vault/forum acceptance, następnie signed writes/feed. Całe 2B-5.3, 2B i 4B-12 pozostają otwarte. MCP dopiero po obowiązkowym porównaniu OSS; przeglądarka później.
