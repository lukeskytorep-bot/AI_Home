# AI Home — 4B-12 krok 2B-4 — natywny lifecycle Community

Data: 27.10.2026. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Plan treść 1.79. Całe 2B i 4B-12 pozostają OPEN.

Baza: AI_HOME_4B12_STEP_2B3_SOURCE.zip, SHA-256 `c83ad3a3442fd08895ac4235cb876a95d5a5b92568d1a4d3d82e876b14f6d50a`, 844 pliki. Edward potwierdził zielony wynik; nie pobierano niezależnie nowego numeru GitHub CI ani audytu.

## Wynik i zakres

Dodano natywny lifecycle rejestru Community: wyłącznego właściciela procesu, draining operacji i blocking writers, scoped usuwanie danych/sekretów, tombstone, prywatny snapshot, merge restore i jawne uzgadnianie oryginalnych kluczy. To nie jest jeszcze podłączenie do profile.delete.permanent, backup.create/restore całej aplikacji ani IPC. Produkcyjny host nie otwiera/nie wywołuje modułu automatycznie. Następny 2B-5 musi połączyć te prymitywy z rzeczywistymi profilami/receipts, oboma magazynami danych, direction/capabilities i IPC przed udostępnieniem registration.

## Właściciel i draining

Jeden chroniony absolutny UTF8 path rejestru ma trwały plik .owner-lock i OS lock fs2. W jednym procesie handle rejestru współdzielą owner; drugi współpracujący proces jest odrzucany do czasu zwolnienia ownera. Nie wolno usuwać/przenosić pliku rejestru/owner-lock podczas pracy ani uruchamiać kilku żywych kopii na tych samych vault references. Raw SQLite i niskopoziomowe vault API są uprzywilejowanymi prymitywami i nie mogą omijać tego protokołu.

Zwykłe registry/start/finish korzystają z read lease. Lease jest klonowana do spawn_blocking; cancel async nie udaje zakończenia OS writera. Zwrócony SecretToken także utrzymuje lease do Drop. Maintenance czeka na exclusive lease, więc nie usunie sekretu, pod którym stary writer dopiero kończy zapis. Nie ma timeoutu ani zgadywania wygaśnięcia ownership. Credential należy trzymać tylko przez czas natywnego requestu, nie bezterminowo cache'ować; caller nie powinien czekać na maintenance, samemu utrzymując taki credential.

Lokalne delete/snapshot/restore/reconcile kończą krytyczną pracę również po porzuceniu future callera i utrzymują owner. To lokalne utrzymanie storage, nie wznowienie zadania AI ani zgoda na publikowanie. Zawieszony vault może zatrzymać maintenance zamiast dopuścić unsafe cleanup. OS zwalnia lock po zakończeniu procesu; testy potwierdzają wykluczenie innego procesu i zwolnienie po normalnym close, nie force-kill/real OS-vault shutdown.

Dodano dokładnie jedną przypiętą zależność fs2 =0.4.3. Istniejących wersji nie podnoszono ani nie usuwano; jedyny zmieniony stary node Cargo.lock to dependency edge ai-home -> fs2. Deklarowany MSRV, dependency pins modeli/Tauri i workflow bez zmian. Cargo.toml i Cargo.lock trzeba przekazywać razem.

## Usuwanie profilu Community

lifecycle::delete_profile najpierw utrwala deleting. Usuwa oryginalne key references oraz sumę journaled i aktywnych token references z kontrolą scope i potwierdzeniem braku wpisu. Scope-only cleanup obsługuje orphan token bez utrwalonego agent/generation receipt. Dopiero po potwierdzonym usunięciu sekretów kasuje w transakcji rekordy Community, zostawiając minimalny profile-ID/deleted tombstone. Nie zostają display/about/referencje tego profilu. Tombstone blokuje prepare oraz odtworzenie starej kopii w tej instalacji.

Błąd vault pozostawia deleting, dane i referencje do idempotentnego retry; zwykłe operacje są zablokowane. Inne profile nie są ruszane. Operacja nie kasuje głównego profilu/rozmów aplikacji, aktora zdalnego forum ani wyeksportowanych backupów. Nie stanowi globalnej obietnicy wymazania wszystkich nośników; jest Community częścią przyszłej sagi permanent deletion.

## Snapshot i restore

snapshot działa po drain, tworzy nowy plik i nigdy nie nadpisuje istniejącego. Rodzic musi istnieć i być chroniony. Ścieżki są absolutne UTF8; apostrofy i natywne backslashe zachowują poprawną semantykę. Nowe i rozpoznane pliki rejestru/WAL/SHM oraz snapshot mają Unix 0600; Windows dziedziczy ACL katalogu hosta i wymaga osobnego odbioru. Plik jest synchronizowany write-capable handle, a katalog Unix dodatkowo fsync przed sukcesem.

Kopia SQLite zawiera metadane profilu, refs i ograniczone podpisane artefakty — jest wrażliwa mimo braku eksportu vault. secrets_included=false dotyczy vault keys/tokens, nie analizy DLP tekstu wpisanego przez właściciela. Aktywne gate w kopii przechodzą do reconcile, finish pointers są wycofane, a live readiness pozostaje bez zmian. deleting blokuje snapshot. Export/import ma limit 64MiB. Jest to kopia dedykowanego rejestru, nie kompletny backup Store aplikacji.

restore_metadata przyjmuje oczekiwany SHA256 i czyta immutable/read-only source. Przed oceną tabel sprawdza tożsamość i pełny kształt schematu względem trusted target — dodatkowe view/trigger/DDL są odrzucane. Następnie kontroluje integrity/FK, gate i nieaktywne finish pointers, a przed commit ponownie hash. Kopiuje parametryzowane dane znanych tabel, nigdy DDL źródła. Import jest jedną transakcją.

Merge dotyczy wyłącznie nieobecnych Community IDs. Live/reconcile/deleting/deleted są pomijane: nie cofamy istniejącej historii/generacji, nie usuwamy live danych i nie nadpisujemy lokalnego tombstone. Imported tombstone zostaje deleted. Pozostałe dane są reconcile: bez provision/start/complete/credential. Read-only odczyty i błędne prepare nie tworzą profilu ani nie blokują późniejszego importu.

reconcile_profile sprawdza oryginalne key refs i expected public keys. Brak klucza blokuje, nie generuje nowego. Restored reserved/provisioning także zostają zamknięte; reconcile_keys może jawnie przyjąć tylko istniejący poprawny scoped keyset. Po otwarciu gate nadal nie ufamy staremu tokenowi: potrzebne jest jawne proof recovery. active oznacza możliwość wykonania operacji, nie autoryzację; current() pozostaje kontrolą realnego credentiala. Żaden restore/reconcile nie wykonuje HTTP.

## Schemat i weryfikacja

Rejestr Community migruje v1/v2/v3 -> v4, dodając jedną lifecycle table. Preparation/start/finish/journal DDL są zachowane. Store aplikacji nadal v15. Frozen P2, provider adapters, application/deletion/backup code, uprawnienia i workflow są bajtowo zgodne z bazą. Stare testy dostosowano tylko w fixture future v5 i inventory/target migracji v1/v2; testów i asercji nie zmniejszono. Koordynatorowy inventory JSON opisuje aktualny v4; historyczny inventory 2A zachowuje swój jawny scope.

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless all-targets locked/offline | **550/550 PASS**, 22 zestawy |
| Nowe lifecycle testy, część powyższych 550 | **21/21 PASS** |
| Końcowy komponentowy run po prywatności/readonly/shape/path doprecyzowaniach | **115/115 PASS**, powtórzone przypadki |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| P2 / granice / asercje Store | **193/193 / 34/34 / 13 przy v15 PASS** |
| Command parity | **54/54 PASS** |
| Standard closure/release | **8/8 i 8/8 PASS** |
| Experimental foundation/runtime/continuity/internal/reflection | **12/24/23/23/16 PASS** |
| Community reader / historical identity static | **14/14 i 14/14 PASS** |
| Zależności | **1 nowy pakiet, 0 upgrade, 0 removal** |
| Rust source text, manifest, ZIP, baza + nakładka = SOURCE | **PASS**, osobne dowody w pakiecie |

21 nowych przypadków obejmuje scoped delete z orphan candidates, idempotencję/reopen i tombstone, izolację innego profilu, błąd vault/retry, cancel podczas blocking provision, utrzymywany credential i anulowanego callera maintenance, live-vs-copy readiness, restore/reconcile i brak nowych kluczy, anti-resurrection/anti-rollback merge, readonly/hash failure, unsafe/zmieniony schemat snapshotu, create-once i escaped filenames, missing reserved/adopting existing keys, foreign token-ref odmowę, migrację v3, Unix mode/symlink i realny subprocess owner-lock probe. Child probe jest częścią jednego przypadku; nie dodajemy jego wewnętrznych uruchomień do liczby unikalnych testów. Unix-only case nie stanowi wyniku Windows.

Testy to prawdziwy SQLite, rzeczywiste kernel file locks/subprocess i backendy sekretów w pamięci. Nie wykonywano real OS keyring, Windows/Tauri/WebView, live forum/PHP/TLS ani API modeli. Nie wykonano nowego OpenSSL: kryptografia i protokół proof są zachowane i przechodzą istniejące testy; ich wcześniejsze dowody zostają w bazie. W trakcie końcowej kontroli usunięto wyłącznie odtwarzalny cache buildów po wyczerpaniu scratch space, a końcowy run ponowiono. Pozostaje historyczny warning unused InternalStateProvider w niezmienionym teście; nowe moduły nie dodają warningów.

## Przekazanie

Nakładka jest na dokładnej bazie 2B-3 i niczego nie usuwa. Audyt zawiera tę bazę, nakładkę, pełny SOURCE, raport, plan 1.79, EN developer guide, logi, dependency diff i dowód zgodności. Istniejący cargo all-targets CI wykrywa nowe przypadki bez zmiany jobów. Nie jest potrzebny dodatkowy release/instalator.

Następny 2B-5: rzeczywisty host profile deletion/receipts, backup/restore obu baz, direction/capability checks i IPC. Dopiero wtedy udostępnić registration. Signed forum writes/feed pozostają dalej w 4B-12; całość 2B oraz 4B-12 nadal OPEN.
