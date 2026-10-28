# AI Home — 4B-12 / 2B-5.2 — wspólny backup i offline restore

Data: 28.10.2026. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Plan treść **1.82**. Całe 2B-5, 2B i 4B-12 pozostają OPEN.

Baza: `AI_HOME_4B12_STEP_2B5_1_CI_FIX_1_SOURCE.zip`, SHA-256 `9582976a308e78b4c45000f02215295e5714a836c616e352eecdd2acc93cadb2`, 901 plików. Edward potwierdził zielony wynik po poprawce cache. Nie pobierano niezależnie numeru run ani nowego werdyktu audytu.

## Wynik

Dodano natywny `community_backup::OfflineCommunityBackup`: spójną kopię Store + Community oraz odtwarzanie z trwałym journalem, kopią bezpieczeństwa i wznowieniem po przerwaniu. To operacja **offline**: wszystkie Store-derived pools muszą zostać zamknięte i zwolnione. Nie dodano jeszcze przycisku ani poleceń IPC. Produkcyjny desktop nadal nie włącza registration ani tego maintenance API. Istniejące app backup commands dla Community hosta nadal odmawiają wykonania niepełnej kopii; ich kierowanie przez maintenance/recovery bootstrap należy do 2B-5.3.

Store pozostaje **v16**, registry **v4**. Nie ma nowej migracji, zależności, zmian Cargo.lock, workflow, uprawnień ani frozen P2. Zachowano download-only cache z odebranej poprawki CI. Nowe manifesty i journal JSON są natywnym formatem lokalnego backupu, nie rozszerzeniem zamrożonego IPC.

## Wspólna własność i spójność

Store::open uzyskuje współdzielony OS lock w trwałym pliku `.app-owner-lock` obok danej bazy. Pula SQLx utrzymuje lock także po porzuceniu samego wrappera Store, jeśli nadal istnieje clone puli. Native offline maintenance wymaga wyłącznego locka głównej bazy i exclusive lease rejestru. Otwarte Store powoduje Busy; nie wymusza się zamknięcia, nie wygasza ownership timerem i nie usuwa lockfile. Dwa pliki baz są czytane przy obu blokadach; Community writers i zwrócone credentials muszą wcześniej zwolnić lease.

Dotychczasowy startup restore Standard także uzyskuje exclusive owner. Może maksymalnie 5 sekund czekać na techniczne zakończenie disposal zadań SQLx po Drop. Ten timeout tylko odmawia operacji, nigdy nie zwalnia cudzego locka. Usunięto w ten sposób wyścig ujawniony przez istniejący test backupu; wszystkie jego asercje pozostają zachowane. Raw SQL, raw keyring oraz nieuczestniczące procesy są uprzywilejowane i nie mogą omijać protokołu. Chroniony lokalny katalog i działające blokady plików są warunkiem hosta.

## Format i publikacja backupu

Wynik jest katalogiem zawierającym dokładnie `application.sqlite3`, `community.sqlite3` i `manifest.json`. Main jest logicznym VACUUM snapshotem, Community korzysta z istniejącego sanitize snapshot: aktywne gate w kopii stają się reconcile, finish token pointers są wycofane. Live readiness i generacje pozostają bez zmian. Nie czyta się ani nie eksportuje vault keys/tokens.

Oba pliki i manifest są synchronizowane. Kompletny katalog jest publikowany dopiero przez rename prywatnego katalogu tymczasowego; awaria przed publikacją nie daje gotowego backupu. Istniejącego katalogu docelowego nie nadpisuje się. Ten sam request ID może odczytać zweryfikowaną oryginalną kopię; inny request nie przejmuje tego miejsca. Osierocone prywatne katalogi tymczasowe po awarii nie są automatycznie kasowane.

Manifest wiąże oba SHA-256, schematy 16/4, request ID, timestamp, liczby profili, źródłowy root binding i registry binding. Limity formatu 1: Main do 1 GiB, Community do 64 MiB, JSON do 64 KiB. Wymagany jest dokładny kanoniczny kształt bieżącego schematu, oceniony przed zapisem/evaluacją nieznanego DDL; dodatkowe triggery/views są odrzucane, również po przeliczeniu hashy przez nadawcę. Native format 1 nie zastępuje migracji historycznych backupów Standard. Symlinki komponentów i dodatkowe pliki w bundle nie są przyjmowane.

Backup jest **wrażliwy**: zawiera historię, pamięci, prywatne rekordy i referencje. vaultSecretsIncluded=false oznacza brak eksportu vault, nie analizę DLP treści użytkownika. Nowe katalogi mają Unix 0700, pliki 0600. Windows dziedziczy ACL chronionego katalogu i wymaga osobnego odbioru. Ścieżki muszą być absolutne UTF8; quoting VACUUM zachowuje apostrofy i natywne separatory.

## Restore, tombstone i oryginalna tożsamość

Przed pracą sprawdza się oba komponenty, hash/schema/integrity/FK, pin oraz spójność referencji profili. Oczekująca saga deletion, registry deleting i nie-inactive bieżące Experimental uniemożliwiają destrukcyjny restore. Nie ma owner override ani wymuszenia decyzji AI. Bieżący żywy profil Community nieobecny w Main z backupu powoduje odmowę — restore nie może osierocić nowej tożsamości.

Następnie utrwala się journal żądania, marker blokujący Store::open, fence rejestru i kompletny safety bundle przed importem. Wszelkie zwykłe operacje registry/start/finish oraz publiczny lifecycle/reconcile respektują fence. Obejmuje to okres między przerwaniem workera a wznowieniem, więc nie można w tym oknie dopisać tożsamości lub tombstone pod już przygotowanym Main snapshotem. Native privileged maintenance używa tego samego exclusive lease bez reentrant deadlock.

Community jest **merge-only**: istniejące live/reconcile/deleted IDs są zachowane, generacje i journal nie są cofane. Brakujące wpisy z kopii są importowane jako reconcile, bez gotowego tokena. Przed instalacją kopii Main usuwa się z niej wszystkie profile oznaczone obecnie jako deleted w rejestrze, wraz z ich drzewem danych i odpowiednimi starymi receipts. To pruning przygotowywanej kopii profilu już usuniętego, nie STOP żywego Experimental. Używa tej samej funkcji usuwania drzewa co saga 2B-5.1.

Oryginalne klucze nie są regenerowane. Import nie czyta vault i nie wykonuje HTTP; jawne reconcile sprawdza rzeczywiste oryginalne key references. Brak kluczy pozostawia gate zamknięty. Gotowość live istniejącego wpisu pozostaje niezależna od importu; odtworzony wpis nie zyskuje dostępu przez samą obecność metadanych.

SameInstallation wymaga właściwego registry binding. FreshInstallationRelocation jest jawną opcją wyłącznie dla pustego Main bez pina i pustego rejestru. Dopiero wtedy pin w przygotowywanej kopii jest związany z nową lokalizacją; profil, klucze i podpisane scope nie są zmieniane. Nie można w ten sposób przejąć niepustej instalacji. Związana instalacja z pustym rejestrem jest odrzucana przy restore: nie da się zgadnąć, czy zaginęły tombstone. Ochrona przed wskrzeszeniem wymaga zachowania aktualnego rejestru; nie jest obietnicą odzyskania historii utraconej na wszystkich nośnikach.

## Przerwanie, restart i wynik

Prywatne kopie wejścia i staging są sprawdzane przed zapisem. Journal wiąże request ID, bundle path/hash, policy, target binding i safety path. Gotowy Main ma osobny trwały hash/receipt, stary Main zostaje zachowany. Przed swap ponownie sprawdza się źródło. Po przerwaniu nie usuwa się ręcznie WAL/SHM i nie tworzy pustego Main. Startup Store i stary restore odmawiają działania przy oczekującym native markerze.

Powtórzenie tego samego żądania kontynuuje roll-forward: merge jest idempotentny, przerwany swap kończy się z przygotowanej kopii, a commit rezultatu i housekeeping można wznowić. Inne żądanie nie przejmuje pending job. Publiczne create/restore/finish_cleanup kończą lokalną pracę również po porzuceniu future oczekującego callera; nie jest to uruchomienie zadania AI. Bundle musi pozostać w niezmienionym miejscu podczas nieukończonego importu.

Końcowy receipt jest dostępny przez `receipt(requestId)` bez oryginalnego bundle. Gdy dane zostały już zastosowane i receipt zapisano, `finish_cleanup(requestId)` może zakończyć sprzątanie markerów nawet po utracie plików wejściowych. Nie stosuje danych ponownie i nie uznaje braku receipt za sukces. Wadliwy końcowy JSON nie odblokowuje Store ani rejestru. Również replay zakończonego żądania sprawdza hash zastosowanego Main, dopóki istnieje pending marker; brak lub zmiana pliku nie usuwa zabezpieczeń i nie tworzy pustej bazy. Usunięcie main markera następuje po durable result; jeśli proces umrze przed usunięciem fence, Main jest już spójny, a Community pozostaje zamknięte do housekeeping.

Nie przenosić ani nie podmieniać plików owner/fence/journal podczas pracy. Nie ma automatycznego rollbacku generacji Community, automatycznego proof reissue ani zaufania do starego tokena. Safety i previous Main są zachowane do audytu; polityka retencji/GUI recovery zostaje po stronie przyszłego hosta.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all-targets, locked/offline | **585/585 PASS**, 23 zestawy |
| Nowy moduł backupu, część powyższych | **22/22 PASS**, w tym 2 helpery subprocess |
| Dotychczasowy backup Standard / host deletion | **7/7 i 13/13 PASS** w pełnym zestawie |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript/Vite build | **PASS** |
| P2 / granice / migration assertions | **193/193 / 34/34 / 14 przy v16 PASS** |
| Command parity | **54/54 PASS** |
| Standard closure/release | **8/8 i 8/8 PASS** |
| Experimental foundation/runtime/continuity/internal/reflection | **12/24/23/23/16 PASS** |
| Reader / historical identity static guards | **14/14 i 14/14 PASS** |
| Rust source text | **PASS** |

Testy obejmują oba rzeczywiste SQLite stores, owner między procesami i utrzymanie go przez clone pool, porzucenie oczekiwania, symlinki/private modes, walidację po rehash dodatkowego triggera, nowszą generację tokena, tombstone pruning, scope/policy, missing/original keys, cztery okna awarii, prawdziwe wznowienie w nowym procesie po normalnym zamknięciu ownera, receipts bez inputu i wadliwy końcowy receipt. Failpointy są wyłącznie cfg(test), lokalne dla instancji koordynatora; produkcja nie odczytuje testowych env ani flag.

Nie wykonano force-kill/power-loss/fsync-failure na sprzęcie, realnego OS vault, Windows/Tauri/WebView, live forum/API/TLS ani ponownego OpenSSL. Sekrety w testach mają backend pamięciowy; native capture/restore nie wywołuje jego odczytu. Subprocess tests są normalnym zamknięciem/restartem i sprawdzeniem blokady, nie dowodem odporności każdego filesystemu na utratę zasilania. Pozostaje stary niewykorzystany import InternalStateProvider w niezmienionym teście. Lokalny PASS nie jest nowym wynikiem GitHub CI ani audytu.

## Przekazanie i następny krok

Overlay ma ścieżki względem dokładnej bazy 2B-5.1 CI fix 1, bez usunięć. Pełny SOURCE, baza, raport, plan, guide EN, logi i dowód odtworzenia drzewa są w audit package. Wszystkie archiwalne fixture, zależności, provider adapters, frontend i zamrożone kontrakty pozostają zgodne bajtowo z bazą.

Dalej **2B-5.3**: maintenance/recovery bootstrap i routing backupów w hoście, direction/capability checks, registration IPC oraz świadome podłączenie desktopu. Obecny default desktop nadal nie otwiera Community automatycznie. Standardowy push CI wystarczy; release/instalatory nie są potrzebne. Signed forum writes/feed pozostają dalszą częścią 4B-12; Messenger/A2A 4B-13, UI Experimental 4B-14.
