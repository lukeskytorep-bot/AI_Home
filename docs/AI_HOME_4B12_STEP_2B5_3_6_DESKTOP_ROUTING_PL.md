# AI Home 4B-12 / 2B-5.3.6 — routing i autoryzacja desktopowa

Data: 29.10.2026. Status: IMPLEMENTED / AWAITING NEW NATIVE CI AND AUDIT.

## Wynik

Dodano jawny opt-in routingu desktopowego Community. Produkcyjny setup zarządza jednym DesktopRuntime: dotychczasowym Standard lub CommunityHost z lokalną polityką. Zwykłe home_dispatch i osobne community_dispatch używają tego samego właściciela aplikacji. UI rejestracji nie jest jeszcze częścią tej nakładki.

Baza: dokładny AI_HOME_4B12_STEP_2B5_3_5_SOURCE.zip, SHA-256 90b81cf092a32e9c7807125f4df4251d76ac8c339a373fee7fb495ce7953d3b3. Edward potwierdził zielone CI. Audyt 2B-5.3.5 zamknął O1; zewnętrznego run/commitu nie sprawdzano niezależnie.

## Natywna polityka i caller

Plik community.desktop.json w Tauri app_data_dir jest jawną lokalną konfiguracją właściciela instalacji. Nie pochodzi z IPC. Jest ładowany raz przy starcie, więc zmiana/revocation pliku wymaga restartu. Każdy request otrzymuje nowy grant z aktualnej polityki procesu, a stan profilu jest ponownie kontrolowany przez host. Serwer, skill i JSON request nie nadają capabilities. To konfiguracja zaufanej lokalnej aplikacji, bez nowego uprawnienia dla Experimental.

Ścisły format: schemaVersion=1 i targets; 1..32 różnych profili, jeden przypięty endpoint/origin/instance na profil, jawne nazwy dozwolonych komend z dziesięciu supplemental registration commands. Brak domyślnych grants. Unknown/duplicate commands, unknown fields, duplicate profiles, nieprawidłowy scope i API poza origin odmawiają. Maksimum 64 KiB, odczyt ograniczony także przy wzroście pliku. Plik musi być regularny; Unix odrzuca symlinki, hardlinki oraz group/other write. Windows polega na ACL katalogu aplikacji. Nie jest to projektowany później wspólny mechanizm wielu społeczności na profil.

Przykład docs/community-desktop.example.json jest instrukcją, nigdy automatycznie ładowanym plikiem. Najpierw uruchomić aplikację bez opt-in, utworzyć/migrować Main i profil Standard, zamknąć aplikację, a potem jawnie skonfigurować zweryfikowany forum endpoint, origin i instance. Nie wpisywać sekretów do konfiguracji. Docelowy UI konfiguracyjny pozostaje osobnym etapem.

Tauri permission allow-community-dispatch dodano wyłącznie do capability okna main, bez remote. Oba entry sprawdzają również label i URL z natywnie wstrzykniętego WebviewWindow. Dopuszczone bundled origins: tauri://localhost oraz http://tauri.localhost, bez obcego portu i userinfo. Wyłącznie build debug dopuszcza dokładny dev server http://127.0.0.1:1420. Test osobno sprawdza regułę release, niezależnie od cfg testowego. JSON nie może podać tego callera.

## Routing i trwałość

community_dispatch przyjmuje request: CommandEnvelope i osobne invite: string|null. Zaproszenie jest przenoszone do Zeroizing od początku handlera; nie jest polem payload/request ani receipt. Nie dodano logowania argumentów. Start jako jedyny pozwala na invite i nadal ma ograniczenie długości/formatu. Żądania lokalne z invite odmawiają. Transport IPC i pamięć frontendu nie są obietnicą braku wszystkich kopii sekretu; przyszły UI musi czyścić pole i nie zapisywać go w storage.

Polityka wybiera cel po profileId i sprawdza konkretną komendę przed replay lub HTTP. Lokalny status/progress/preparation przechodzi dispatch_local; start/complete/retry/access.recover przechodzą dispatch_registration. Zwykły home_dispatch nie rozpoznaje supplemental registration commands, więc nie stanowi obejścia.

Nie zmieniono kontroli Standard, autorytatywnego experimental_instances, archiwizacji, deletion, expectedRevision, host binding, registry lifecycle/fence, durable claims i terminal replay. expectedRevision to Main revision; preparation registrationRevision i finishRevision nadal są osobnymi wartościami. UI ma używać status/progress, bez odczytu surowego registry. Historii receipt nie wolno uznać za bieżący grant. Późniejsze HTTP ponownie autoryzuje i ładuje poświadczenie.

Desktopowe zwykłe żądania są kolejkowane, aby równoległe panele nie dostawały Busy przy starcie. Nowe dispatch_queued współdzieli całą implementację z dotychczasowym try-admission; po zdobyciu bramki ponownie sprawdza phase. Maintenance nadal odmawia nowych wejść, a działający worker zachowuje właściciela po anulowaniu oczekiwania. Registration entry zachowuje dotychczasową odmowę Busy — nie dodano automatycznej pętli retry/POST.

## Startup i fail closed

Bez polityki i śladów Community Standard zachowuje dotychczasowy startup/legacy restore. Z polityką Main musi już istnieć; nie wykonuje się legacy restore ani automatycznej rejestracji, vault read, key creation lub HTTP. Pending native marker, registry fence i unresolved legacy restore odmawiają przed otwarciem registry. Missing pinned registry nie jest zastępowany nową pustą bazą. Host nadal sprawdza rzeczywisty pin i owner drain.

Usunięcie polityki nie pozwala wrócić po cichu do Standard, również gdy pozostał tylko trwały binding w Main. Binding jest sprawdzany przed legacy restore i ponownie po otwarciu Store. Niespójności, uszkodzona polityka i brak możliwości odczytu odmawiają zamiast fallback. Odbudowa instalacji/recovery nie jest automatycznie wykonywana przez startup.

Dotychczasowy guard skoordynowanego hosta nadal odmawia Store-only backup/restore z UI. Nie dodano jeszcze desktopowego UI maintenance/recovery. Obsługa natywnego odzyskiwania z wcześniejszych etapów pozostaje osobnym narzędziem, a pełny odbiór UX backupu jest nadal otwarty.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust all-targets, no-default-features, locked/offline | 661/661, 23 zestawy PASS |
| Finalny filtr desktop | 11/11 PASS na Linux |
| Frontend Vitest | 219/219, 21 plików PASS |
| TypeScript/Vite | PASS |
| P2 / parity / boundaries | 193/193, 64/64, 34/34 PASS |
| Migration assertions | 14 przy Store v16 PASS |
| Wszystkie 12 npm verify | PASS |
| Default-feature cargo check z Tauri | BLOCKED — brak systemowego pkg-config/GLib; brak wyniku PASS |

Siedem nowych testów policy/caller/bootstrap obejmuje strict format, unknown/duplicate grants, origin, limity, domyślny Standard, brak Main, uszkodzoną konfigurację, odmowę fallback po usunięciu polityki, marker/fence i missing pinned registry, a na Unix także symlink i write permissions. Cztery testy właściwego routingu obejmują startowe równoległe panele, complete/replay/current status, utracony POST z odtworzeniem obu baz/hosta i recovery przez autoryzowany desktop route oraz denial obcego callera/profilu/self-grant i replay po odebraniu grantu. Dokładne testy i logi znajdują się w SOURCE.

Loopback HTTP i testowy vault nie dowodzą TLS ani rzeczywistego OS vault. Bootstrap z systemowymi wrapperami nie wykonuje odczytu sekretu. Restart to kontrolowane odtworzenie właścicieli i połączeń z dysku, bez force-kill/power-loss. W fixture pinned registry początkowo użyto krótkiego napisu odrzucanego przez istniejący CHECK length=64; poprawiono dane testowe, nie schemat.

Rust 1.99.1, Node 24.22.1, npm 11.9.0. Tauri API entry, macro/permission generation i pełny default-feature build wymagają nowego CI. Lokalna próba zatrzymała się przed kompilacją aplikacji na glib-sys/pkg-config; próba instalacji zależności napotkała ograniczenia systemowe/blokadę zarządcy pakietów. Nie obchodzono blokady i nie deklaruje się kompilacji Tauri jako zaliczonej. Niezmieniony workflow instaluje Linux dependencies i wykonuje cargo test --all-targets --locked z domyślnymi features; Windows check można uruchomić jego istniejącą opcją.

## Zakres i odbiór

Zmieniono bootstrap Tauri, dodano native desktop policy/router i testy, wspólną ścieżkę queued dispatch oraz jedną scoped permission i jej dokładny guard. Nie zmieniono dependencies, Cargo.lock, migracji, frozen P2, frontendu ani workflow. Store v16 i registry v4. Pełna historia planu 1.91 pozostaje pod nowym wpisem 1.92.

Nałożyć CHANGED_FILES na zieloną bazę 2B-5.3.5 i wykonać zwykły commit/push, nowe natywne CI oraz audyt. SOURCE jest pełną alternatywą, AUDIT_PACKAGE zawiera dokładną bazę, overlay, source i dowody. BASE + overlay = SOURCE bajtowo, bez usunięć. Nie nakładać obu źródeł jednocześnie; bez release/instalatorów.

Kolejny etap: ekran rejestracji/statusu/recovery korzystający z tej ścieżki, następnie realny desktop/Windows/WebView/OS vault/forum/PHP/TLS acceptance i UI skoordynowanego maintenance. Dopiero dalej signed writes/feed i dalsze 4B-13/14. Całe 2B-5.3, 2B i 4B-12 pozostają OPEN. MCP po obowiązkowym przeglądzie OSS; przeglądarka później.
