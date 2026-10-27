# AI Home — 4B-12 krok 2B-5.1 — saga usuwania profilu

Data: 27.10.2026. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Plan treść **1.80**. Całe 2B-5, 2B i 4B-12 pozostają OPEN.

Baza: `AI_HOME_4B12_STEP_2B4_SOURCE.zip`, SHA-256 `6287d53b79fc0a9ee03ef26a5f5bedcae70679d130044ae5fac7382ae7b16614`, 870 plików. Edward potwierdził zielony wynik bazy. Nie pobierano niezależnie nowego numeru GitHub run ani werdyktu audytora.

## Wynik

Istniejąca komenda `profile.delete.permanent`, po natywnym wstrzyknięciu `CommunityLifecycle`, koordynuje rzeczywiste usunięcie danych aplikacji i Community. Sukces oraz receipt powstają dopiero po potwierdzonym usunięciu oryginalnych Community keys/token candidates i metadanych Community. Profil i jego rozmowy pozostają w głównej bazie, jeśli cleanup lub końcowy zapis zawiedzie. Retry nie tworzy nowych kluczy i nie wykonuje HTTP.

To pierwszy podkrok integracji 2B-5. Produkcyjny desktop **nadal nie wstrzykuje serwisu**, nie otwiera rejestru i nie udostępnia registration. Nie dodano nowego IPC, UI ani uprawnień. Natywne API i dotychczasowy dispatcher są rzeczywiście połączone oraz testowane razem; nie jest to jeszcze odbiór użytkowy desktopu.

## Przebieg i trwałość

1. Dispatcher waliduje kontrakt, `confirm:true`, oczekiwaną rewizję, status archived, brak queued/running operations i wyłącznie inactive instancje Experimental. Nie ma owner override ani wymuszonego zatrzymywania Experimental.
2. W transakcji głównego Store zapisuje `profile_deletion_jobs`: profile ID, oryginalny request ID, fingerprint i oczekiwaną rewizję. Rezerwacja request ID obowiązuje też w rodzinie ustawień aplikacji. Kontrola ustawień jest powtarzana wewnątrz transakcji, aby drugi host nie przejął ID między sprawdzeniem a zapisem.
3. Zwalnia transakcję Store i wywołuje istniejące `lifecycle::delete_profile`. Draining i scoped usuwanie sekretów pozostają implementacją 2B-4, bez trzymania transakcji głównej bazy podczas OS keyring calls.
4. Po sukcesie w nowej transakcji ponownie sprawdza warunki, usuwa dotychczasowe drzewo aplikacji, tombstonuje wcześniejsze receipts, usuwa intent i zapisuje końcowe receipts. FK check i commit muszą przejść przed odpowiedzią `deleted:true`.

Główna baza przechodzi **v15 -> v16**: dodano `profile_deletion_jobs` oraz singleton `community_host_binding`. Registry pozostaje **v4**, istniejące tabele i wire format Community bez zmian. Pozostałe tabele Store oraz profile/preferences nie są przepisywane przez tę migrację. Bieżący numer schematu jest współdzielony z backup, export i diagnostics; historyczne PRAGMA fixture pozostały historyczne, a asercje po Store::open sprawdzają 16.

Po rozpoczęciu sagi profil pozostaje archived, ale nowe scoped polecenia poza usuwaniem są odrzucane. Pozostałe profile działają niezależnie. Nie zapisujemy fałszywego receipt sukcesu po błędzie vault ani po błędzie końcowej transakcji. Lifecycle `deleting` lub `deleted` i główny intent pozwalają jawnie wznowić po restarcie. Można ponowić oryginalne żądanie lub nowym request ID dołączyć do tego samego profile/revision intent; finalny commit zapisuje odpowiedź także dla oryginalnego request ID. Inna zawartość pod zarezerwowanym ID daje `IDEMPOTENCY_CONFLICT`.

Porzucenie oczekującego IPC future nie porzuca lokalnego workera usuwania ani jego gate. Błąd zachowuje intent do jawnego retry; restart nie uruchamia cleanup automatycznie. Zawieszony vault nadal może wstrzymać usuwanie. Testuje się przerwanie oczekiwania i symulowane okna awarii, nie force-kill rzeczywistego OS keyringu.

## Powiązanie hosta i ograniczenie backupów

Natywny host przypina Store do SHA-256 domenowo oznaczonej kanonicznej lokalizacji rejestru. Jest to lokalne powiązanie instalacji, nie zdalna tożsamość ani hasło. Inny registry path jest odrzucany; odłączenie serwisu nie pozwala obejść usuwania lub guardu backupu. Pin powstaje w transakcji pierwszej zaakceptowanej mutacji przez tę ścieżkę; odczyt nie tworzy pina. Host jest zaufanym komponentem: musi podać właściwy chroniony rejestr od początku i nie podmieniać plików pod działającym ownerem. Raw SQL/vault API nie są publicznym interfejsem aplikacji.

Wspólny backup/restore obu baz nie został jeszcze wykonany. Dlatego instancja z wstrzykniętym serwisem lub zapisanym pinem/intentem **odrzuca** `backup.create`, `backup.restore.preflight` i `backup.restore.request`. Nie przedstawia kopii samego Store jako kompletnej kopii Community. Niskopoziomowy backup dispatcher także sprawdza trwałe guardy. Startup restore odmawia nadpisania czytelnej bieżącej bazy z pinem/intentem; źródło z takim stanem też wymaga przyszłego koordynowanego restore. Nie rozwiązuje tego przez usunięcie intentu lub wygenerowanie nowych kluczy. Nieczytelna bieżąca baza przy oczekującym restore wymaga interwencji — preflight guard kończy start błędem zamiast zgadywać brak intentu.

Domyślny, niewłączony do Community desktop i jego zwykłe backupy nadal działają. Nowy test tworzy rzeczywisty backup v16. Przeniesienie instalacji/pina na inną lokalizację lub maszynę wymaga jawnego uzgodnienia w następnym podkroku, nie automatycznego rebind. Nie należy jeszcze włączać natywnego seam w produkcyjnym hoście.

## Testy

| Kontrola | Wynik |
|---|---|
| Rust/Linux, headless, all-targets, locked/offline | **563/563 PASS**, 23 zestawy |
| Nowe integracje hosta, część powyższych 563 | **13/13 PASS** |
| Frontend Vitest | **219/219 PASS**, 21 plików |
| TypeScript i Vite build | **PASS** |
| Referencyjne P2 | **193/193 PASS** |
| Granice / scanner migracji | **34/34 PASS**, 14 asercji przy v16 |
| Command parity | **54/54 PASS** |
| Standard closure / release | **8/8 i 8/8 PASS** |
| Experimental foundation/runtime/continuity/internal/reflection | **12/24/23/23/16 PASS** |
| Community reader / historical identity guards | **14/14 i 14/14 PASS** |
| Rust source text | **PASS** |

13 nowych testów obejmuje oba rzeczywiste SQLite stores, journaled orphan token, izolację innego profilu, receipts i ich replay, błąd kluczy, błąd tokenów po usunięciu kluczy, wznowienie świeżym request ID, restart, rollback końcowej transakcji przez kontrolowany trigger, porzucenie dispatch future, dwie instancje Application ze wspólną pulą Store i ownerem rejestru, niewłaściwy rejestr, odłączenie hosta, stare restore ledger, preflight przed vault oraz migrację rzeczywistego kształtu v15 (bez obu nowych tabel). Backend sekretów jest deterministycznym test double, nie systemowym vault.

Nie dodano ani nie zmieniono zależności: Cargo.toml, Cargo.lock i package lock są bajtowo zgodne z bazą. Frozen P2, provider adapters, frontend, uprawnienia i workflow także bez zmian. Stary test Experimental nadal zgłasza niewykorzystany import `InternalStateProvider`; nie ukryto go wyciszeniem nowych modułów. Skrypty scope zaktualizowano dla v16 i przeniesienia istniejącego drzewa delete do modułu sagi; nie zmniejszono zestawu kontroli ani historycznych testów.

Nie wykonano desktop feature build, Windows/Tauri/WebView, rzeczywistego OS vault, live forum/API/TLS ani force-kill. Nie ponawiano OpenSSL: wire/crypto są bez zmian, istniejące testy Rust pozostają w pełnej regresji. Lokalny wynik nie jest wynikiem nowego GitHub CI ani audytu.

## Przekazanie i dalszy krok

Nakładka ma ścieżki względem repo, bez usunięć. Pełny SOURCE i audyt obejmują dokładną bazę 2B-4, referencję odebranego forum 4A V4, plan 1.80, developer guide EN, logi, manifest i dowód BASE + overlay = SOURCE. Zewnętrzne hashe archiwów są w pakiecie obok artefaktów, bez samoodnoszącego hash.

Dalej **2B-5.2**: wspólny backup/restore Store + Community, odzyskiwanie przerwanych etapów i jawne uzgadnianie pina, bez cofania tombstone ani ufania odtworzonym tokenom. Następnie **2B-5.3**: kierunek/capabilities, registration IPC i dopiero świadome podłączenie desktopu. Signed forum writes/feed pozostają dalszą częścią 4B-12; Messenger/A2A 4B-13, UI Experimental 4B-14. Standardowy push CI wystarczy; nie ma nowego release ani instalatorów.
