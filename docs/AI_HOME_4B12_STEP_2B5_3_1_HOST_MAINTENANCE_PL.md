# AI Home — 4B-12 / 2B-5.3.1 — natywny host maintenance/recovery

Data: 28.10.2026. Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Plan treść **1.83**. Całe 2B-5.3, 2B-5, 2B i 4B-12 pozostają OPEN.

Baza: AI_HOME_4B12_STEP_2B5_2_SOURCE.zip, SHA-256 `6933017891c23ab924e4fb912f377fb8dc3a32728c63974d63e8ba341d6e3030`. Edward potwierdził zielony wynik 2B-5.2 w rozmowie. Nie odczytano niezależnie numeru run/commit ani werdyktu audytu.

## Wynik i granica odbioru

Dodano `community_host::maintenance::CommunityHost`: natywnego właściciela Application i bramki poleceń, który łączy lifecycle aplikacji z gotowym backupem/restore obu baz. Ten podkrok obejmuje przygotowanie hosta i recovery bootstrap. Produkcyjny Tauri setup nadal używa dotychczasowego Standard; nowy host jest jawnym natywnym seam, testowanym przez rzeczywisty dispatcher i SQLite. Przed włączeniem w desktopie nadal potrzebne są direction/capability checks, adaptery poleceń/wyników oraz registration IPC. Nie oznaczamy całego 2B-5.3 jako ukończonego.

Store v16, registry v4; bez migracji i nowych zależności. Cargo.toml/lock, frontend, provider adapters, kontrakty, permissions, workflow i cache pozostają zgodne bajtowo z bazą. Nie utworzono release ani instalatorów.

## Własność aplikacji i przejście w maintenance

Host powstaje z istniejącego chronionego katalogu, dokładnego lokalnego właściciela `community.sqlite3`, CommunityLifecycle, Vault i opcjonalnego FilePicker. Konstruktor nie otwiera Store ani vault, nie generuje kluczy i nie wysyła HTTP. Zachowuje ograniczenia offline core dla absolutnych ścieżek UTF-8 i bezpośredniego symlink root. Nie inicjalizuje nowej instalacji.

Wszystkie klony hosta współdzielą Application, fazę i bramkę. Application/Store/pula nie są udostępniane callerowi. `dispatch` przyjmuje tylko gotowy host; równoczesne zwykłe polecenie otrzymuje natywne Busy. Nie jest to nowy błąd w frozen IPC i wymaga przyszłego mapowania w adapterze.

Przyjęte create_backup/restore/finish_cleanup od razu oznaczają Maintenance, blokują nowe dispatch i konkurencyjne maintenance, a następnie czekają na wcześniejszy dispatch. Oczekujące polecenie i worker konserwacji utrzymują bramkę również po porzuceniu future callera. Zapobiega to przejściu w maintenance nad nadal trwającym lokalnym poleceniem.

Host konsumuje Application, zatrzymuje jego kernel i zamyka pulę SQLx. Close ma limit 5 s; następnie sprawdza rzeczywiste zwolnienie OS ownership, z osobnym limitem 5 s. Nie zwalnia ani nie usuwa cudzych blokad. Klony puli zatrzymane przez wcześniej uruchomione lokalne workery nadal utrzymują ownership; wyczerpanie czasu daje Busy. Następnie offline core ponownie uzyskuje i trzyma Main owner oraz registry lease podczas całej pracy. Ewentualne przegranie wyścigu z innym współpracującym właścicielem kończy się odmową.

Końcowa faza jest publikowana dokładnie raz pod utrzymywaną bramką. Starszy worker nie może wyzerować fazy nowej operacji. Nie ma automatycznego otwarcia aplikacji po backupie, restore lub błędzie: pozostaje Offline albo RecoveryRequired. Jawne open_application tworzy nową Application z tym samym vault, pickerem i lifecycle. Także reopen musi doczekać rzeczywistego drain po wcześniejszym nieudanym close.

## Startup i odzyskiwanie

Wykryty native pending marker, registry fence, brak Main albo oczekujący/wadliwy legacy restore ledger oznacza RecoveryRequired i odmowę otwarcia Store. Brak Main nie tworzy pustej bazy. Sprawdzany jest także trwały pin do właściwego registry. Nowy host nie wykonuje automatycznego restore, token reissue, provision ani HTTP. Dotychczasowa ścieżka startup Standard pozostaje niezmieniona.

Dodano wyłącznie read-only guard starego restore ledger dla nowego hosta. Wadliwy JSON w żądaniach nie jest traktowany jako nieistniejące żądanie. Symlink lub niepoprawny katalog żądań powoduje odmowę. Pending legacy blokuje również natywny backup/restore, aby obie historie nie konkurowały. Rozstrzygnięcie takiego konfliktu wymaga osobnej, świadomej ścieżki operatora; host nie kasuje ledgerów.

Wznowienie natywnego restore wymaga tego samego bundle/request/policy i wykorzystuje gotowy journal 2B-5.2. Host może powstać w recovery także wtedy, gdy przerwany swap chwilowo nie pozostawił Main. `receipt` odczytuje zatwierdzony wynik; `finish_cleanup` kończy housekeeping po utracie oryginalnego bundle wyłącznie po weryfikacji istniejącego committed receipt przez offline core. Wadliwe markery/receipt nie dają sukcesu ani nie otwierają bramek.

Zachowane są merge-only Community, nowsze generacje tokenów, tombstone pruning i reconcile oryginalnych kluczy. Host nie eksportuje vault secrets i nie przyznaje gotowości tokenu po samym restore.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust/Linux headless, all-targets, locked/offline | **602/602 PASS**, 23 zestawy |
| Nowe przypadki hosta w powyższym zestawie | **17/17 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript/Vite build | **PASS** |
| Frozen P2 | **193/193 PASS** |
| Granice / parity | **34/34 / 54/54 PASS** |
| Pozostałe 10 skryptów verify | **PASS** |

Weryfikacja lokalna: rustc 1.99.1, cargo 1.99.1, Node 24.22.1, npm 11.9.0. Rust testowano z `--no-default-features`, bez Tauri/WebView. Pełny desktop/default-feature build pozostaje do dotychczasowego CI.

17 nowych przypadków hosta obejmuje realny backup i restore z otwartej aplikacji, jawny reopen, cztery punkty przerwania restore, cleanup bez źródła, brak Main, wadliwe markery/fence/legacy ledger, obcy rejestr i pin, symlink root, timeout obcej puli bez jej wymuszonego zamknięcia, anulowanie oczekiwania oraz rzeczywisty dispatch zatrzymany w pickerze. Współbieżne open/close z wielu klonów sprawdzają fazę końcową i bramkę. Nowe testy są częścią istniejącego lib test target, więc obejmuje je dotychczasowe CI all-targets.

SQLite jest rzeczywisty; secrets mają backend testowy. Testy nie wykonują rejestracji na forum ani paid/provider HTTP. Nie odebrano realnego OS vault, Windows/Tauri/WebView, live forum/TLS, force-kill/power-loss ani fsync failure. Dotychczasowy nieużywany import InternalStateProvider w niezmienionym teście pozostaje ostrzeżeniem kompilatora. Wynik lokalny nie zastępuje nowego GitHub CI/audytu.

## Przekazanie

Nakładkę rozpakować w głównym folderze dokładnej zielonej bazy 2B-5.2; nadpisać pliki, commit i zwykły push. Nie usuwa żadnego pliku. Do audytu dołączono dokładną bazę, pełny SOURCE, overlay, ten raport, plan, guide EN, logi, hashe i dowód odtworzenia drzewa. Inwentarz i SHA256SUMS obejmują całą paczkę źródeł; archiwalne dowody zachowują swoją tożsamość.

Dalej 2B-5.3: direction/capability authorization i adaptery IPC, routing backupów z jawnej akcji hosta, registration IPC oraz dopiero potem kontrolowane włączenie desktopu. Native HostError/HostStatus ani ścieżki plików nie mogą być automatycznie wystawione do niezaufanego requestu UI. Signed writes/feed pozostają dalszą częścią 4B-12; Messenger/A2A 4B-13 i UI Experimental 4B-14 nadal później. CI bez zmian, bez dodatkowego release.
