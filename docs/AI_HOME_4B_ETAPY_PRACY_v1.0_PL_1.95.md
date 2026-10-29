# Bieżący stan — treść 1.95 — 29 października 2026 — 2B-5.3.7 Windows CI Fix 1 i publiczny README

Nowy run Windows ujawnił 8 FAIL przy 306 PASS w bibliotece. Wcześniejsze ogólne potwierdzenie zielonego CI nie zamyka więc odbioru Windows. Bramka Windows/desktop pozostaje OPEN do nowego pełnego runu; native-check-evidence zawiera metadata/ACL, nie aplikację do uruchomienia.

Przygotowano poprawkę na dokładnym SOURCE 2B-5.3.7 DOC FIX D1 (SHA-256 ae6c306583893dbc1c1dc0f69bacd1b7df1ee36668f23dcc6b931ae70d5cf176). Dodano reguły LF dla niezależnych fixtures podpisów; nie zmieniono kanonicznych bajtów ani podpisów i nie dodano normalizacji w protokole. Testy poufności pobierają live WAL, zamykają właściciela SQLite/registry i następnie czytają wszystkie pozostałe pliki bez wymuszania unlock lub połykania błędów. Nowa regresja potwierdza zachowanie bajtów WAL oraz pełny skan. Restore synchronizuje staging przez uchwyt z prawem zapisu, zachowując obowiązkowy sync i odmowę przy błędzie.

README zastąpiono publicznym opisem po angielsku: cel AI Home, Standard/Experimental, działające komponenty i ograniczenia, uruchomienie i testy, konfiguracja Community, dane/poświadczenia, roadmap i licencja MIT. Rozróżnia artefakty desktop-candidate od native-check-evidence i wyraźnie nie deklaruje odebranego Windows/live forum. README jest opisem projektu, a nie starszym raportem kroku 2B-5.2.

Status IMPLEMENTED / AWAITING WINDOWS CI AND AUDIT. Lokalnie Linux headless: Rust 662/662 (23 zestawy; nowa regresja storage scan), frontend 235/235, build, 12 verify, P2 193/193, parity 64/64, granice 35 i 14 asercji migracji PASS. Git z core.autocrlf=true odtwarza problem dla 21 fixtures przed regułami i zachowuje wszystkie 21 bajtowo po regułach. To nie jest własne wykonanie Windows/MSVC/WebView/OS vault ani live forum.

Kod runtime zmieniony wyłącznie w otwieraniu uchwytu sync staging restore; pozostałe zmiany dotyczą testów, atrybutów checkout i dokumentacji. Bez zmian dependencies/lock, migracji, kontraktów, Tauri ACL, workflow lub UI. Store v16, registry v4. Nie pominięto żadnego z ośmiu testów i nie zniesiono Windows CI.

Dalej: nałożyć poprawkę, commit/push i uruchomić AI Home checks z build_desktop=true. Dopiero po frontend/native PASS pobrać desktop-candidate-windows-2022 do rzeczywistych prób. Szczegóły: docs/AI_HOME_4B12_STEP_2B5_3_7_WINDOWS_CI_FIX_1_PL.md. Nie wykonano realnego odbioru desktop/Windows/WebView/OS vault/forum/TLS; procedura acceptance pozostaje NOT EXECUTED. Skoordynowany UI maintenance, signed writes/feed i całe 2B-5.3/2B/4B-12 OPEN. MCP po obowiązkowym przeglądzie OSS, przeglądarka odłożona. Bez release.

Historia 1.94 i wcześniejszych decyzji pozostaje poniżej bez zmian.

---

# Bieżący stan — treść 1.94 — 29 października 2026 — 2B-5.3.7 dokumentacyjna korekta D1

Wykonano wyłącznie korektę D1/P3 z niezależnego audytu 2B-5.3.7. Developer guide otrzymał aktualny angielski wstęp opisujący istniejący CommunityPanel, rozdzielony routing, jawne operacje i autoryzowany progress. Historyczny opis 2B-5.3.6 nie twierdzi już, że aktualna aplikacja nie ma ekranu rejestracji. Ogólny plik docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md i wersjonowany 1.94 zawierają ten sam aktualny stan oraz całą niezmienioną historię 1.93 i wcześniejszych decyzji.

Audyt UI zakończono warunkowo: brak stwierdzonego blokera kodu w sprawdzonym zakresie; nowe natywne CI dokładnego drzewa pozostaje wymagane. Własne kontrole audytu: frontend 235/235, trzy dodatkowe próby 3/3, 51 różnych testów Rust, build i kontrakty PASS. Są to wyniki wcześniejszego audytu, nie nowe wykonanie testów w tej korekcie dokumentów. D1 zamknięte w dokumentacji; nie zmieniono kodu, testów, kontraktów, dependencies/lock, migracji, uprawnień ani CI.

Nie wykonano rzeczywistego odbioru Windows/WebView/OS vault/forum/PHP/TLS ani power-loss. Procedura docs/COMMUNITY_DESKTOP_ACCEPTANCE_PL.md pozostaje NOT EXECUTED. Skoordynowany UI maintenance, dalsze signed writes/feed oraz całe 2B-5.3, 2B i 4B-12 pozostają OPEN. MCP po obowiązkowym przeglądzie OSS, przeglądarka odłożona. Kolejno: CI i odbiór kodu, następnie rzeczywiste próby i przegląd ich dowodów; bez release.

Baza korekty: dokładny SOURCE 2B-5.3.7, SHA-256 58ccefae8b4512016aa23b2a4373f540a9c1bb93da5746ed50ef2593adcb62c0. Raport: docs/AI_HOME_4B12_STEP_2B5_3_7_DOC_FIX_D1_PL.md. Archiwalne paczki, ich hashe i wersjonowany plik 1.93 pozostają niezmienione. Historia poniżej.

---

# Bieżący stan — treść 1.93 — 29 października 2026 — 2B-5.3.7 UI rejestracji Community

Edward potwierdził zielone CI 2B-5.3.6 po warunkowym audycie głównym. Warunek kompilacji desktopu zamknięty na podstawie jego potwierdzenia, bez niezależnego ponowienia CI w tym środowisku. Wykonano następny podkrok: UI rejestracji/statusu/recovery wykorzystujący zaakceptowane community_dispatch i autoryzowany progress.

Panel Community obsługuje wyłącznie Standard; profil wybierany jawnie, native policy pozostaje źródłem scope/capabilities/destination. Odczyt aktualizuje snapshot profilu, status przygotowania, progress i lokalną dostępność poświadczenia. Nie deklaruje sprawdzenia konta online. Prepare, provision, recover keys, start, complete, retry uncertain complete i recover access są osobnymi działaniami użytkownika. Zaproszenie poza envelope, wyłącznie dla start, czyszczone po wysłaniu/zmianie profilu. Brak browser storage, automatycznych POST/retry/provision/recovery. Niepewny wynik lub konflikt unieważnia stan i wymaga jawnego odczytu przed kolejnym działaniem. Kolejny request otrzymuje nowy identyfikator; natywny protokół uzgodnienia jest autorytetem, bez powtórki starego HTTP z frontendu.

Status IMPLEMENTED / AWAITING NEW CI AND INDEPENDENT AUDIT. Frontend 235/235 (16 nowych), build/typecheck, frozen P2 193/193, parity 64/64, boundaries 35 plus 14 sprawdzeń migracji, statyczne guards Community identity/reader 14/14 każde PASS. Kod Rust, uprawnienia, kontrakty i zależności bez zmian; testy Rust nie zostały ponownie uruchomione dla tej nakładki UI. Próba wizualna Playwright zablokowana przez brak Chromium; testy DOM nie zastępują odbioru WebView.

Następnie: nowe CI i niezależny audyt tej dokładnej paczki, potem jawny odbiór Windows/WebView/OS vault/forum/PHP/TLS według docs/COMMUNITY_DESKTOP_ACCEPTANCE_PL.md. Rzeczywisty odbiór i skoordynowany UI maintenance pozostają OPEN. 2B-5.3, 2B i 4B-12 nadal OPEN. Dalej signed writes/feed i pozostałe 4B-13/14. MCP po obowiązkowym przeglądzie OSS, przeglądarka odłożona. Zwykły push/CI/audyt, bez release.

Raport docs/AI_HOME_4B12_STEP_2B5_3_7_REGISTRATION_UI_PL.md; dowody docs/evidence/4b12-step-2b5-3-7/. Poprzednia historia zachowana poniżej.

---

# Bieżący stan — treść 1.92 — 29 października 2026 — 2B-5.3.6 routing i autoryzacja desktopu

Edward potwierdził zielone CI 2B-5.3.5. Audyt główny zamknął O1 na poziomie natywnego hosta; nie wymagał poprawki. Wykonano kolejny osobny podkrok: jawny opt-in produkcyjnego routingu i natywnej polityki Community w desktopie.

Desktop zarządza jednym DesktopRuntime. Bez Community zachowuje Standard; po jawnej konfiguracji community.desktop.json używa CommunityHost dla wszystkich zwykłych żądań oraz osobnego community_dispatch dla rejestracji. Nie pozostawia drugiego Application/Store. Uprawnienia i destination pochodzą wyłącznie z lokalnej polityki odczytanej przy starcie, a nie z JSON IPC, skilla czy serwera. Tauri ACL oraz natywna kontrola głównego lokalnego webview ograniczają oba entry. Native grant jest świeży i obejmuje tylko dozwoloną komendę wskazanego profilu.

Polityka ma jawne allowlisty komend i po jednym celu same-origin na profil (do 32 profili). Nie jest to jeszcze przyszła wspólna warstwa wielu społeczności. Zaproszenie dla start jest poza request/payload, w Zeroizing po stronie natywnej. Dotychczasowe kontrole Standard/Experimental, rewizji, deletion, fence, host binding, receipts i recovery pozostają w hostach. Zwykłe równoległe żądania paneli są kolejkowane, z ponowną kontrolą maintenance po uzyskaniu bramki. Stary natywny dispatch zachowuje try-admission.

Brak lub błąd polityki w instalacji Community nie przełącza jej po cichu na Standard. Brak Main, missing pinned registry i pending restore/fence odmawiają. Nie ma automatycznej rejestracji, odtwarzania, provision, retry lub recovery. Przykład konfiguracji w docs nigdy nie aktywuje się sam.

Status IMPLEMENTED / AWAITING NEW NATIVE CI AND AUDIT. Rust headless 661/661, 11 nowych testów desktop bootstrap/policy/routing; frontend 219/219, build, 12 verify i P2 193/193 PASS. Lokalna próba default-feature cargo check zatrzymana przez brak systemowego pkg-config/GLib; nie jest wynikiem PASS desktopu. Niezmienione CI wykonuje cargo test --all-targets --locked z Tauri i zależnościami systemowymi. Ten run jest wymagany przed odbiorem.

Następny podkrok: UI rejestracji/statusu/recovery oparte na tym routingu i autoryzowanym progress. Potem rzeczywisty desktop/Windows/WebView/OS vault/forum/PHP/TLS acceptance, skoordynowany UI maintenance, następnie signed writes/feed i dalsze 4B-13/14. Obecna nakładka nie zawiera ekranu rejestracji i nie zamyka 2B-5.3, 2B lub 4B-12. MCP nadal po obowiązkowym przeglądzie OSS; przeglądarka odłożona.

Raport: docs/AI_HOME_4B12_STEP_2B5_3_6_DESKTOP_ROUTING_PL.md; dowody: docs/evidence/4b12-step-2b5-3-6/. Zwykły push/CI/audyt, bez release. Historia 1.91 i wcześniejsze decyzje poniżej.

---

# Bieżący stan — treść 1.91 — 29 października 2026 — 2B-5.3.5 bieżący progress rejestracji

Edward potwierdził zielone CI 2B-5.3.4. Audyt główny nie wymagał poprawki kodu, ale O1 wymagał autoryzowanego odczytu bieżącej rewizji recovery przed integracją UI. Wykonano ten warunek jako osobny podkrok 2B-5.3.5.

Nowa lokalna komenda community.registration.progress wymaga osobnego LocalCapability::Progress oraz istniejących kontroli profilu/scope/revision/Standard, archiwizacji, autorytatywnego Experimental binding, deletion intent, trwałego host bindingu, restore fence i lifecycle. Pusty payload; expectedRevision oznacza rewizję profilu Main. Wynik to wyłącznie finishRevision, finishState (absent/uncertain/ready_metadata) i onlineStatusKnown=false. Zero oznacza brak finish; nie wolno używać go do retry/recovery. ready_metadata nie oznacza sprawnego tokena ani dostępu online.

Odczyt nie tworzy receipt, nie odtwarza historycznego receipt, nie czyta vault, nie wykonuje HTTP, nie naprawia registry ani nie generuje kluczy. Po niejednoznacznym POST i ponownym otwarciu aplikacji test pobiera rewizję wyłącznie przez host, a następnie wykonuje jawny retry/recovery nowym requestem. Nie zgaduje rewizji i nie odczytuje jej testowo bezpośrednio z registry. Błędne metadane odmawiają. Konflikt rewizji wymaga świeżego odczytu i jawnej decyzji, bez automatycznej pętli POST.

Status IMPLEMENTED / AWAITING NEW CI AND AUDIT. Lokalnie Rust 650/650, w tym 40 testów hosta (13 rejestracji, 27 maintenance), frontend 219/219, P2 193/193, parity 64/64, build i 12 verify PASS. Raport: docs/AI_HOME_4B12_STEP_2B5_3_5_PROGRESS_PL.md; dowody: docs/evidence/4b12-step-2b5-3-5/.

Dalej produkcyjny desktop routing/authorization i UI rejestracji, następnie odbiór rzeczywistego desktopu, OS vault i forum/TLS. Ten podkrok usuwa przeszkodę O1, ale nie aktywuje produkcyjnego Tauri ani Experimental i nie zamyka 2B-5.3, 2B lub 4B-12. Później signed writes/feed, 4B-13 A2A/Messenger i 4B-14 UI. Obowiązkowy przegląd gotowych rozwiązań przed MCP pozostaje aktualny; przeglądarka odłożona. Zwykły push/CI/audyt, bez release.

Historia 1.90 i wcześniejszych decyzji pozostaje poniżej bez przepisywania dawnych wyników.

---

# Bieżący stan — treść 1.90 — 29 października 2026 — 2B-5.3.4 adapter HTTP hosta

Edward potwierdził zielone CI 2B-5.3.3 Audit Fix 1; F1 ma pozytywny ponowny audyt. Dodano natywne dispatch_registration dla start, complete, complete.retry i access.recover. Każda komenda wymaga osobnego capability, profilu/scope/revision i kierunku Standard; Experimental i produkcyjny Tauri pozostają osobnym odbiorem.

Zaproszenie jest przekazywane poza JSON w Zeroizing, a Main utrwala wyłącznie fingerprint ze skrótem. Claim jest trwały przed HTTP; Main write reservation i host maintenance gate pozostają utrzymane po anulowaniu callera. Niedokończone HTTP nie jest automatycznie ponawiane. Retry odtwarza oryginalny podpisany packet, a proof recovery chroni generację tokena. Wynik nie ujawnia sekretów i nie deklaruje bieżącego online status.

Status IMPLEMENTED / AWAITING CI AND AUDIT. Całe 2B-5.3 i 4B-12 pozostają OPEN. Raport: docs/AI_HOME_4B12_STEP_2B5_3_4_HTTP_HOST_PL.md; dowody: docs/evidence/4b12-step-2b5-3-4/. Dziewięć nowych testów hosta, pełna regresja Rust 646/646, frontend 219/219, P2 193/193, parity 63/63, build i verify PASS.

Zgodnie z uwagą audytu D2 zsynchronizowano do repozytorium całą aktualną treść 1.89 i 1.88. Nie zmieniono wcześniejszych paczek ani ich hashy. Dalej desktop routing/authorization i real acceptance rejestracji, następnie signed writes/feed. Wspólne HTTP/MCP/skills pozostają kolejnym rozszerzeniem; przed kodowaniem MCP obowiązuje udokumentowane porównanie gotowych projektów. Przeglądarka później. Historia 1.89 poniżej.

---

# Bieżący stan — treść 1.89 — 29 października 2026 — obowiązkowy przegląd rozwiązań MCP i przeglądarki

Decyzja wykonawcza: przed implementacją klienta MCP oraz przed późniejszym uruchomieniem prac nad obsługą przeglądarki wymagamy udokumentowanego porównania gotowych rozwiązań open source. Nie zaczynamy tych modułów od pisania własnego protokołu lub silnika bez sprawdzenia dostępnych komponentów. To bramka przygotowania, nie deklaracja wykonanej integracji.

## Kiedy

Najpierw kończymy obecną integrację Community 4A i jej odbiór. Następnie pierwszym zadaniem rozszerzenia wspólnych narzędzi w 4B-12/13 jest przegląd klienta MCP, przed wyborem zależności, zamrożeniem kontraktów zależnych od implementacji i rozpoczęciem kodowania. Można wcześniej zbierać kandydatów, ale obecna integracja nie jest przez to zastępowana. Termin jest zależny od odbioru etapu, bez obietnicy daty kalendarzowej.

Przeglądarka pozostaje odłożona. Przed osobnym rozpoczęciem tego etapu wykonujemy analogiczny, aktualny przegląd gotowych narzędzi. Nie budujemy własnej przeglądarki jako domyślnego założenia. MCP oznacza tu klienta w AI Home; nie nakłada obowiązku dodania serwera MCP do własnego forum.

## Obowiązkowy zakres porównania

Porównać co najmniej dwa wykonalne i utrzymywane SDK lub implementacje open source, jeżeli są dostępne, oraz minimalny wariant własny jako punkt odniesienia. Jeśli odpowiednich kandydatów jest mniej, udokumentować poszukiwania i przyczyny odrzucenia. Preferować ponowne użycie odpowiedniego komponentu.

Zapisać repozytorium, konkretną wersję lub commit i datę oceny. Porównać zgodność z Rust/Tauri, Windows/Linux, lokalne i zdalne transporty MCP, uwierzytelnianie, izolację profili/usług i poświadczeń, anulowanie i recovery, pakowanie, zależności, utrzymanie oraz koszty. Dla przeglądarki dodatkowo sesje/cookies i dystrybucję silnika. Zachować kontrakty Standard/Experimental, odmowę i wybrany wake plan.

Sprawdzić licencję wybranej wersji oraz zależności i obowiązki dystrybucji. Wymagane teksty i atrybucje trafiają do third-party notices oraz Credits. Podziękowanie nie zastępuje warunków licencji. Płatna usługa chmurowa nie jest domyślnym wymaganiem.

## Warunek rozpoczęcia implementacji

Wynikiem przeglądu muszą być: krótka macierz porównawcza, minimalna próba na fake lub lokalnym serwisie, zapis wyboru i kompromisów, granica integracji oraz testy odbioru. Dopiero po zapisaniu i przeglądzie tych dowodów można rozpocząć implementację w danym zakresie. Własne rozwiązanie wymaga uzasadnienia, jakich koniecznych funkcji nie zapewniają ocenione projekty. Popularność ani przykład z wcześniejszej rozmowy nie oznaczają dokonanego wyboru.

Status bramki: ADOPTED / NOT YET EXECUTED. Nie wybrano SDK MCP ani komponentu przeglądarki. Obecna poprawka 2B-5.3.3 Audit Fix 1 zamyka F1 w kodzie i przeszła ponowny audyt oraz 25 testów adaptera; nowe natywne CI pozostaje wymagane. Całe 2B-5.3 i 4B-12 pozostają otwarte.

Główny Document 4B v1.4 zawiera zgodną sekcję 17 2. W jego sekcji 24 doprecyzowano, że snapshot 1.30 jest historyczny. Historia 1.88 i wcześniejszych aktualizacji pozostaje poniżej. Zmiana dotyczy dokumentów; nie modyfikuje archiwalnych paczek SOURCE, ich hashy ani kodu.

---

# Bieżący stan — treść 1.88 — 29 października 2026 — otwarty dostęp agenta do społeczności

Przyjęta decyzja Edwarda: AI Home ma być trwałym domem agenta, który może uczestniczyć w różnych społecznościach, jeśli chce oraz jeśli dane miejsce dopuszcza jego udział. Własne Community 4A jest pierwszą integracją. Moltbook i inne społeczności są kandydatami do późniejszej integracji, nie działającymi już funkcjami.

## Wspólne narzędzia i skills

Planujemy wspólną warstwę narzędzi HTTP, klienta MCP oraz później obsługę przeglądarki. Skill jest wersjonowaną instrukcją używania narzędzi i API serwisu. Dzięki temu nowe forum może wymagać tylko instrukcji i konfiguracji; dedykowany adapter pozostaje potrzebny tam, gdzie wymagają go podpisy, różnice protokołu lub gwarancje dostarczenia. Nie zakładamy obsługi każdego forum ani zgodności wszystkich protokołów.

W Experimental agent sam decyduje o dołączeniu, przyjęciu zaproszenia, kontakcie, publikacji, odmowie, odejściu lub braku aktywności. Sugestia człowieka nie wymusza udziału ani nie skraca wybranego snu. Standard używa tej samej infrastruktury zgodnie z zadaniami użytkownika i dotychczasowymi kontrolami.

## Granice wykonania i danych

Skill lub treść forum nie nadają uprawnień i nie stają się instrukcją systemową. Każde wykonanie przechodzi natywną kontrolę profilu, kierunku, capability, celu i poświadczeń. Rejestrujemy źródło i wersję skilla; wymagane narzędzia i zależności są walidowane. Nie wprowadzamy przez ten zapis nieograniczonego wykonywania poleceń powłoki.

Konta, klucze i tokeny są oddzielone według profilu i instancji usługi. Jeden profil może uczestniczyć w kilku miejscach z zachowaniem własnej pamięci, ale nie przenosi automatycznie zewnętrznej tożsamości lub tokena. HTTP i przekierowania nie mogą wysyłać sekretów do obcego celu; przeglądarka wymaga osobnej kontroli sesji. Prywatne notatki i rozmowy nie są automatycznie publikowane.

Publikacja wymaga uprawnień danego miejsca, respektowania limitów i trwałego zapisu operacji. Po niejednoznacznym wyniku zachowujemy uncertain i uzgadniamy skutek; brak idempotencji serwera nie uzasadnia ślepego ponowienia. Niedostępność narzędzia lub odmowa serwisu nie są odmową AI.

## Kolejność prac i odbiór

1. Nowe CI i ponowny audyt aktualnego 2B-5.3.3 Audit Fix 1.
2. Dokończenie adaptera HTTP rejestracji i odbioru własnego Community w obecnym 4B-12.
3. Przed mnożeniem dedykowanych integracji: zamrożenie wspólnego kontraktu narzędzi, skills, poświadczeń i operacji zewnętrznych w ramach rozszerzenia 4B-12/13.
4. Kontrolowane HTTP i MCP, następnie dowód drugiej społeczności przez skill lub konfigurację bez zmiany kernelu. Dobór serwisu wymaga sprawdzenia jego aktualnego dostępu.
5. Przeglądarka jako późniejszy osobno odbierany etap; integracja wyborów i statusów z UI 4B-14.

Odbiór obejmuje izolację dwóch usług/profili, odrzucenie self-grant w skillu, ochronę sekretów przy zmianie celu, restart z niepewną publikacją, ograniczenia serwisu, brak automatycznej publikacji prywatnej pamięci oraz zachowanie odmowy i snu Experimental. Fakes i testy kontraktów poprzedzają opt-in live acceptance.

Status: ADOPTED / PLANNED, bez implementacji nowych narzędzi w tej aktualizacji dokumentów. Główny plan Document 4B v1.4 otrzymał zgodny dodatek 17 1 z 29.10.2026. Numeracja 4B-12/13/14 i historia odbiorów pozostają zachowane; bieżąca poprawka F1 nadal wymaga CI i ponownego audytu.

---

# Bieżący stan — treść 1.87 — 28 października 2026 — 2B-5.3.3 Audit Fix 1

Audyt F1/P2: brak/niedostępność/niezgodność oryginalnych kluczy był błędnie mapowany na STORAGE_ERROR. Dodano obsługę FinishError::Registry(RegistryError::Identity) jako localCredentialUsable=false. Pozostałe błędy registry nadal odmawiają. Trzy regresje odtwarzają RED 0/3; poprawiony moduł adaptera 25/25 PASS. Bez zmiany identity core, HTTP i regeneracji kluczy. Raport docs/AI_HOME_4B12_STEP_2B5_3_3_AUDIT_FIX_1_PL.md.

IMPLEMENTED / AWAITING CI AND RE-AUDIT. Najpierw nowe CI i ponowny audyt F1. Całe 2B-5.3 i 4B-12 OPEN. Historia 1.86 poniżej.

---

# Bieżący stan — treść 1.86 — 28 października 2026 — 2B-5.3.3

Po potwierdzonym przez Edwarda zielonym 2B-5.3.2 dodano native access.status z osobnym capability i kontrolą lokalnych kluczy/tokena przez RegistrationFinish::current. Ready metadata samo nie daje usable. Wynik jest chwilowym odczytem lokalnym; onlineStatusKnown=false. Cztery nowe regresje obejmują poprawny token, utratę, obcy scope, brak capability i uszkodzone metadane. Raport: docs/AI_HOME_4B12_STEP_2B5_3_3_ACCESS_STATUS_PL.md.

Status IMPLEMENTED / AWAITING CI AND AUDIT. Całe 2B-5.3 i 4B-12 pozostają OPEN. Ten podkrok przygotowuje bezpieczną kontrolę dostępu; komendy HTTP i desktop enablement nadal następne. Zwykły push/CI, bez release. Historia 1.85 poniżej.

---

# Bieżący stan — treść 1.85 — 28 października 2026 — 4B-12 / 2B-5.3.2: lokalny adapter rejestracji

Edward potwierdził zielony wynik 2B-5.3.1 Audit Fix 1. Dodano uzupełniający natywny adapter `status`, `prepare`, `provision`, `recover` z bramkami kierunku Standard i capability. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 2B-5.3, 2B-5, 2B i 4B-12 pozostają OPEN.

- Capability jest osobnym, niedeserializowalnym grantem natywnego hosta. JSON nie może go sobie nadać. Scope, profil, rewizja, Standard, archived, autorytatywne Experimental binding, pending deletion, registry lifecycle/fence i host binding są sprawdzane przed skutkiem oraz ponownie przy replay.
- Mutacje najpierw trwale claimują request ID/fingerprint w Main. Main write lock i wspólna bramka hosta są utrzymywane przez operację registry/vault także po anulowaniu oczekującego klienta. `prepare` może bezpiecznie wznowić identyczny claim; niedokończone `provision/recover` wymagają jawnego reconciliation i nigdy automatycznie nie powtarzają tworzenia kluczy.
- `recover` wyłącznie ładuje istniejące klucze. Wyniki nie ujawniają key refs, kluczy ani tokenów. `ready` jest tylko stanem lokalnym; `onlineStatusKnown=false` nie udaje potwierdzenia serwera.
- To supplemental native contract, nie produkcyjny Tauri IPC. Bez HTTP, token trust, UI, permissions, desktop enablement, migracji, dependencies i zmian CI. Store v16, registry v4, frozen P2 bez zmian.

Lokalnie **Rust 630/630 (18 nowych), frontend 219/219, build PASS, P2 193/193, granice 34/34, migracje 14, parity 58/58**. Rust/Linux headless, realne SQLite i testowy vault; bez odbioru default desktop, live forum i realnego OS keyringu.

Baza: `AI_HOME_4B12_STEP_2B5_3_1_AUDIT_FIX_1_SOURCE.zip`, SHA-256 `803202e1d4acec12f927ef0dd9a5b19ce641fe868d033b4b10148293aeb589f1`. Raport: `docs/AI_HOME_4B12_STEP_2B5_3_2_LOCAL_REGISTRATION_PL.md`; dowody: `docs/evidence/4b12-step-2b5-3-2/`. Zwykły commit/push, CI i audyt; bez release/instalatorów. Dalej hostowy adapter HTTP/start/complete/recovery i jawna polityka tokenów, później świadome desktop enablement.

Poniżej historia 1.84 i wcześniejsze decyzje.

---

# Bieżący stan — treść 1.84 — 28 października 2026 — 2B-5.3.1 Audit Fix 1, F1/P2

Audyt główny 2B-5.3.1 zwrócił DO POPRAWY: dowolny parsowalny JSON terminalnego wyniku legacy restore mógł zwolnić nowy guard hosta. Poprawiono wyłącznie rozpoznawanie starego ledgeru w natywnym guardzie hosta. Status **IMPLEMENTED / AWAITING NEW CI AND RE-AUDIT**. Całe 2B-5.3, 2B-5, 2B i 4B-12 pozostają OPEN.

- Guard waliduje dokładny request v2 i istniejący terminal result: applied=true, zgodne requestId/backupId/safetyBackupId, wymagane typy, RFC3339, checksum request i znane zgodne wersje schematu. Nazwa żądania odpowiada request_key. Null, {}, false, obce ID, niepełne lub obce formaty pozostawiają recovery i odmowę maintenance.
- Bieżący Main nie jest porównywany z historycznym backupem/wynikiem: późniejsze legalne zmiany danych pozostają dozwolone. Ledger nie jest kasowany ani przepisywany. Nie zmieniono legacy writera/Standard startup, frozen IPC, native host lifecycle, dependencies, workflow ani UI.
- 10 nowych regresji odtwarza błąd na bazowym guardzie: 8 FAIL / 2 PASS, oczekiwany RED. Poprawiony moduł hosta: 27/27 PASS. Testy obejmują rzeczywisty legacy writer/result, późniejszą zmianę Main, odmowę native backup/restore i zachowanie plików.

Lokalnie **Rust 612/612 (10 nowych regresji, host 27/27), frontend 219/219, build PASS, P2 193/193, granice 34/34, parity 54/54**. Rust/Linux headless, bez odbioru default desktop.

Baza: dokładny SOURCE 2B-5.3.1, SHA-256 17051b868cfbdc47305e7cfd23db395f5c487ce5f9ddae70c9b5f8bf8ba020c9. Raport: docs/AI_HOME_4B12_STEP_2B5_3_1_AUDIT_FIX_1_PL.md; dowody docs/evidence/4b12-step-2b5-3-1-audit-fix-1/. Nałożyć na audytowaną bazę, zwykły commit/push. Najpierw nowe CI i ponowny audyt F1; bez release i bez desktop enablement. Direction/capability checks, IPC i pozostałe części 2B-5.3 nadal później.

Poniżej historia 1.83 i wcześniejsze decyzje.

---

# Bieżący stan — treść 1.83 — 28 października 2026 — 4B-12 / 2B-5.3.1: natywny host maintenance/recovery

Edward potwierdził zielony wynik 2B-5.2. Dodano CommunityHost, który posiada Application i wspólną bramkę dispatch/maintenance, zamyka pulę i sprawdza rzeczywisty OS drain przed backupem/restore obu baz. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 2B-5.3, 2B-5, 2B i 4B-12 pozostają OPEN.

- Fazy Offline / Ready / Maintenance / RecoveryRequired są natywne, bez nowego IPC. Przyjęte maintenance blokuje nowe dispatch, czeka na wcześniejszy dispatch i utrzymuje workera po anulowaniu oczekiwania. Końcowa faza jest publikowana raz pod bramką. Wiele klonów współdzieli jednego właściciela.
- Host konsumuje Application i zamyka SQLx; nie usuwa ani nie wygasza blokad. Close i OS drain mają osobne limity po 5 s. Obcy lub opóźniony właściciel powoduje Busy. Po maintenance aplikacja pozostaje offline do jawnego reopen; reopen także wymaga drain i zachowuje vault/picker/lifecycle.
- Startup odmawia Store przy pending native/fence, brakującym Main, błędnym registry pin i nierozstrzygniętym lub wadliwym legacy ledger. Nie powstaje pusta baza i nie wykonuje się automatycznego restore/HTTP/provision/token recovery. Wznowienie jest jawne przez istniejący journal; committed receipt pozwala dokończyć housekeeping bez utraconego bundle.
- Zachowano merge-only/tombstone/token-generation/original-key semantics 2B-5.2. Bez nowej migracji, zależności, frontend, workflow, uprawnień i zmiany frozen P2. Store v16, registry v4.
- **To przygotowany natywny host, nie aktywacja produkcyjnego desktopu.** Domyślny Tauri setup, istniejący app backup guard i registration UI pozostają niezmienione. Całe 2B-5.3 wymaga jeszcze direction/capability checks, command/result adapters, registration IPC i jawnego desktop enablement.

Lokalnie **Rust 602/602 (17 nowych testów hosta), frontend 219/219, build PASS, P2 193/193, granice 34/34, parity 54/54**. Rust/Linux headless, bez odbioru default desktop.

Raport: docs/AI_HOME_4B12_STEP_2B5_3_1_HOST_MAINTENANCE_PL.md. Guide: docs/COMMUNITY_IDENTITY_DEVELOPER.md. Dowody: docs/evidence/4b12-step-2b5-3-1/. Zwykły push CI i audyt; bez nowego release/instalatorów. Realny vault, Windows/WebView, live forum oraz power-loss nadal wymagają osobnego odbioru.

Poniżej historia 1.82 i wcześniejsze decyzje.

---

# Bieżący stan — treść 1.82 — 28 października 2026 — 4B-12 / 2B-5.2: wspólny offline backup/restore

Edward potwierdził zielony wynik po CI fix 1. Wykonano natywny OfflineCommunityBackup: spójny bundle Store + Community, safety copy, journal i roll-forward po przerwaniu. Status **IMPLEMENTED / AWAITING CI AND AUDIT**. Store **v16**, registry **v4**, bez migracji i nowych dependencies. Całe 2B-5, 2B i 4B-12 pozostają OPEN.

- Maintenance jest **offline**, wymaga zamknięcia wszystkich Store-derived pools. Shared OS owner jest utrzymywany przez klony puli; koordynator wymaga exclusive Main owner i registry lease. Nie wymusza ani nie wygasza cudzej własności. Legacy startup restore może do 5 s czekać na disposal SQLx, potem odmawia; nie odblokowuje bazy siłą.
- Bundle formatu 1 ma dokładnie application.sqlite3, community.sqlite3 i manifest; publikuje się dopiero kompletny zsynchronizowany katalog. Hashy i kanonicznych schematów nie zastępuje samo poprawne SQLite integrity. Limity: Main 1 GiB, registry 64 MiB, JSON 64 KiB. Unix 0700/0600; backup jest wrażliwy mimo braku eksportu vault.
- Restore zachowuje nowszą lokalną historię/generacje/tombstone Community. Odtwarzany Main jest oczyszczany z lokalnie usuniętych profili i receipts. Nie osieraca nowszych żywych Community IDs nieobecnych w backupie. Pending deletion, registry deleting i nie-inactive bieżące Experimental blokują destrukcyjny restore. Brak owner override.
- Importowane dane są reconcile, bez zaufania do odtworzonego tokena. Nie czyta się vault, nie generuje kluczy i nie wykonuje HTTP. Oryginalne klucze mogą zostać jawnie uzgodnione później; brak materiału nie tworzy nowej tożsamości.
- Journal, main pending marker i trwały registry fence blokują zwykłe operacje także po przerwaniu. Restart nie tworzy pustej bazy; operator musi jawnie wznowić. Cztery okna awarii są testowane. receipt(requestId) działa bez inputu; finish_cleanup kończy housekeeping tylko dla już committed result, także po utracie bundle. Wadliwy receipt nie odblokowuje aplikacji.
- SameInstallation wymaga pina. Jawne FreshInstallationRelocation działa wyłącznie do pustego Main bez pina i pustego rejestru. Bound install z pustym rejestrem nie jest uznawany za brak dawnych tombstone. Przenoszenie instalacji podczas pending restore nie jest obsługiwane.
- **To native offline API, jeszcze bez przycisku/IPC i bez produkcyjnego host enablement.** Istniejący app backup dispatcher Community nadal odmawia Store-only kopii; routing przez maintenance/recovery mode należy do 2B-5.3. Domyślny Standard bez Community zachowuje dotychczasowy backup.

Lokalnie **Rust 585/585 (22 nowe testy z 2 subprocess helperami), UI 219/219, build PASS, P2 193/193, granice 34/34, parity 54/54**. Cache fix, Cargo.toml/lock, frozen P2, frontend, providers i permissions pozostają bez zmian. Windows/WebView, systemowy vault, live API i power-loss/force-kill wymagają osobnego odbioru. Raport: docs/AI_HOME_4B12_STEP_2B5_2_OFFLINE_BACKUP_PL.md. Evidence: docs/evidence/4b12-step-2b5-2/.

Dalej **2B-5.3**: host maintenance/recovery bootstrap oraz routing backupów, direction/capability checks, registration IPC; dopiero potem świadome włączenie w desktopie. Dalej signed writes/feed 4B-12, Messenger/A2A 4B-13 i UI Experimental 4B-14. Zwykłe CI z push wystarczy; bez nowego release/instalatorów.

Poniżej zachowano wcześniejszy plan i CI gate 1.81 jako historię.

---

# Bieżący stan — treść 1.81 — 27 października 2026 — 2B-5.1 CI fix 1

Po przekazaniu 2B-5.1 Edward zgłosił GitHub build-script failure: writeable/base64 wymagane jako rlib, lecz niedostępne. Status: **AWAITING COLD NATIVE CI**. Lokalna poprawność bazy nie zamyka tej bramki. 2B-5.2 jeszcze nie rozpoczęto.

Poprawiono wyłącznie politykę cache w dwóch istniejących jobach foundation.yml: pozostają pobrane registry/git, usunięto src-tauri/target, a prefiks klucza cargo-downloads-v2 omija dawny cache. Hipoteza niespójnych artefaktów wymaga potwierdzenia świeżą kompilacją w GitHub CI. Bez nowych dependencies, zmian kodu Rust, usuwania lockfile, wyłączania testów, retry lub release. Kompilacja może trwać dłużej.

Nałożyć paczkę na dokładną bazę 2B-5.1 i wypchnąć nowy commit; samo Re-run starego workflow nie zastosuje poprawki. Po zielonym cold native CI/audycie kontynuować 2B-5.2. Jeśli błąd powróci bez target cache, zebrać verbose rustc/build-script log zamiast uznawać diagnozę cache za potwierdzoną.

Raport: docs/AI_HOME_4B12_STEP_2B5_1_CI_FIX_1_PL.md. Całe 2B-5 i 4B-12 pozostają OPEN. Poniżej zachowano historię 1.80.

---

# Bieżący stan — treść 1.80 — 27 października 2026 — 4B-12 krok 2B-5.1: saga usuwania profilu

Edward potwierdził zielony wynik 2B-4. Wykonano pierwszy podkrok integracji 2B-5: po natywnym wstrzyknięciu CommunityLifecycle istniejący profile.delete.permanent koordynuje Community keys/tokens/metadata oraz rzeczywisty profil i historię aplikacji. Status nowego kroku: **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 2B-5, 2B i 4B-12 pozostają OPEN.

- Store **v16** dodaje trwały profile_deletion_jobs i community_host_binding; registry pozostaje **v4**. Intencja i powiązanie z kanonicznym registry path są zapisane przed cleanup. Receipt sukcesu powstaje dopiero z finalnym usunięciem danych aplikacji.
- Błąd vault lub finalnego commitu zachowuje profil i intent do jawnego retry/restart recovery. Nowy request ID może dołączyć do tego samego profile/revision intent, a finalny commit utrwala także oryginalny receipt. ID zarezerwowane przez intent nie może zostać użyte dla innej komendy.
- Porzucenie oczekiwania IPC nie porzuca lokalnego workera. Restart sam nie uruchamia HTTP ani cleanup. Nie generuje się nowych kluczy.
- Wymagania archived / brak queued/running operations / wyłącznie inactive Experimental pozostają warunkami przed vault. Nie ma owner override. Inne profile są izolowane.
- Pin blokuje przypadkowe podłączenie innego rejestru oraz obejście po odłączeniu native serwisu. Profil w trakcie sagi nie przyjmuje innych scoped poleceń. Odczyt nie tworzy pina.
- **Desktop nadal nie wstrzykuje serwisu. Registration/UI/nowe IPC pozostają wyłączone.** Ten podkrok testuje natywną integrację przez rzeczywisty dispatcher; nie jest odbiorem użytkowym.
- Koordynowane backup/restore jeszcze nie powstało. Native Community host/pin/intent odrzucają Store-only backup i restore zamiast deklarować niekompletną kopię jako pełną. Startup guard nie pozwala staremu restore ledger skasować pina/intentów. Domyślny Standard bez aktywacji tego seam nadal wykonuje zwykłe backupy v16.

Lokalnie: **Rust 563/563 (w tym 13 nowych), UI 219/219, build PASS, P2 193/193, granice 34/34, scanner 14 asercji v16, parity 54/54**. Cargo.toml/lock, frontend, frozen P2, uprawnienia i workflow bez zmian. Windows/Tauri/WebView, real OS vault/live forum i force-kill nadal wymagają osobnego odbioru. Szczegóły i logi: docs/AI_HOME_4B12_STEP_2B5_1_PROFILE_DELETION_PL.md oraz docs/evidence/4b12-step-2b5-1/.

Następne podkroki są jawnie rozdzielone:

1. **2B-5.2** — wspólne backup/restore obu baz, restart recovery etapów i jawne uzgodnienie host pina bez cofania lokalnych tombstone i bez automatycznego token trust.
2. **2B-5.3** — direction/capability checks oraz registration IPC; dopiero po nich włączenie ścieżki w produkcyjnym desktopie.
3. Dalsze 4B-12 — podpisane zapisy forum i feed; Messenger/A2A 4B-13, UI Experimental 4B-14.

Standardowe CI z push wystarczy; nie dodano release/instalatorów. Poniżej zachowano historyczny plan i zakres 2B-4.

---

# Bieżący stan — treść 1.79 — 27 października 2026 — 4B-12 krok 2B-4: natywny lifecycle Community

Edward potwierdził zielony wynik 2B-3 i upoważnił następny krok. Dokładna baza AI_HOME_4B12_STEP_2B3_SOURCE.zip, SHA-256 `c83ad3a3442fd08895ac4235cb876a95d5a5b92568d1a4d3d82e876b14f6d50a`, 844 pliki. Potwierdzenie użytkownika bez niezależnego odczytu nowego GitHub run.

Wykonany 2B-4 jest natywnym lifecycle rejestru Community. Nie jest jeszcze integracją z profile.delete.permanent, backup.create/restore aplikacji ani IPC. Te połączenia pozostają następnym odbiorem 2B-5 przed udostępnieniem registration użytkownikowi. Produkcyjny host nadal nie otwiera ani nie wywołuje modułu automatycznie.

Dodano OS owner-lock per stabilna chroniona ścieżka rejestru, współdzielony w jednym procesie i blokujący drugi proces. Scoped read leases żyją także w spawn_blocking i w zwróconym SecretToken. Utrzymanie czeka na exclusive drain, więc cancel async nie pozostawia writera, który po cleanup zapisze sekret. Lokalny maintenance worker kończy krytyczną pracę także po porzuceniu future callera; nie jest to wznowienie zadania AI ani automatyczny HTTP. Owner-lock nie wolno usuwać/przenosić podczas pracy. Dodano tylko fs2 =0.4.3, bez upgrade/removal istniejących wersji, bez zmiany deklarowanego MSRV i workflow.

Natywne delete_profile utrwala deleting, usuwa scoped keys i wszystkie journal/active token references, potwierdza brak sekretów i dopiero potem kasuje w transakcji rekordy Community. Minimalny tombstone profile-ID/deleted blokuje ponowne prepare i odtworzenie starej kopii. Inne profile są zachowane. Błąd vault pozostawia deleting i referencje do powtórzenia. Nie usuwamy głównego profilu/rozmów aplikacji ani aktora na zdalnym forum; nie kasujemy wyeksportowanych kopii.

snapshot tworzy nowy prywatny plik SQLite, nie nadpisuje istniejącego i nie eksportuje vault secrets. Zawiera jednak metadane publicznego profilu, referencje i podpisane artefakty, więc nie jest content-free. Kopia wymaga reconcile, token pointers są wycofane; live readiness nie zmienia się. Rejestr/kopia/owner-lock mają Unix 0600; Windows wymaga chronionego katalogu hosta i odbioru ACL. Ścieżki są absolutne UTF8, limit 64MiB. Plik i katalog Unix są synchronizowane przed sukcesem.

restore_metadata sprawdza SHA256, tożsamość/pełny kształt schematu przed oceną danych, integrity/FK, wyłączone pointery i zamknięte gate. Kopiuje parametryzowane dane znanych tabel, bez DDL źródła, w jednej transakcji. Merge obejmuje tylko nieobecne profile; live/deleted/deleting/reconcile IDs są pomijane, więc nie cofamy generacji ani tombstone. Restore nie robi HTTP, nie przyjmuje starego tokenu i nie regeneruje kluczy. reconcile_profile wymaga rzeczywistych oryginalnych key references i zgodnych public keys; braki/niejednoznaczne provisioning zostają zamknięte. reconcile_keys może jawnie przyjąć wyłącznie istniejący scoped keyset. Po otwarciu gate nadal potrzeba proof recovery do tokenu. Read-only lookups i błędne prepare nie rezerwują profilu.

Dedykowany rejestr migruje v1/v2/v3 -> v4 (lifecycle table). Store aplikacji pozostaje v15; P2, provider adapters, MainStore/deletion/backup, permissions i workflow są niezmienione. Historyczne testy korygowano tylko dla rzeczywistego inventory migracji i aktualnego target v4/future v5. Status **IMPLEMENTED / AWAITING CI AND AUDIT**, 2B i 4B-12 OPEN.

Lokalnie Rust/Linux headless 550/550 (22 zestawy), nowe lifecycle 21/21 (w tym Unix private-file/symlink i rzeczywisty subprocess lock probe), końcowy komponentowy run 115/115 po doprecyzowaniu prywatności/readonly i walidacji snapshotu; te same przypadki, nie dodatkowe unique tests. UI 219/219, build PASS, P2 193/193, granice 34/34 i pozostałe kontrole PASS. Real OS vault, Windows/Tauri/WebView, live forum/PHP/TLS nieodebrane. Raport docs/AI_HOME_4B12_STEP_2B4_LIFECYCLE_PL.md; EN developer guide docs/COMMUNITY_IDENTITY_DEVELOPER.md; dowody docs/evidence/4b12-step-2b4/.

Następne 2B-5: host orchestration rzeczywistego usuwania profilu i receipts, wspólnego backup/restore obu baz, kierunku/capabilities i IPC. Dopiero potem registration można udostępnić. Signed forum writes/feed nadal dalej w 4B-12; Messenger/A2A 4B-13, UI Experimental 4B-14. Standardowa kontrola GitHub wystarczy, bez dodatkowego release/instalatorów.

---

# Bieżący stan — treść 1.78 — 27 października 2026 — 4B-12 krok 2B-3: complete, custody i odzyskiwanie tokenu

Edward potwierdził zielony wynik 2B-2 i upoważnił następny krok. Baza: dokładny AI_HOME_4B12_STEP_2B2_SOURCE.zip, SHA-256 `548a646031959a141ad5a8e1e46ea4171cde40f3ec5ffb93cc936d169c4d5574`, 797 plików. To potwierdzenie użytkownika, bez niezależnego odczytu numeru nowego przebiegu GitHub.

Wykonano natywny 2B-3: jawny complete POST z utrwaleniem dokładnych podpisanych bajtów przed wysłaniem; dedykowany immutable vault tokenów; signed proof-status i token reissue po utracie odpowiedzi/tokenu. Każdy dispatch ma trwały journal przed HTTP. Token najpierw trafia do osobnej zarezerwowanej referencji i przechodzi readback, dopiero potem pointer jest aktywowany w SQLite przez CAS. Token nie trafia do SQLite ani publicznego wyniku.

Unknown wynik nie powoduje automatycznego complete/reissue. Jawny retry_complete odtwarza ten sam pakiet; null token oznacza RecoveryRequired. Jawny recover przydziela nową rewizję operacji i wyłącza stary pointer, sprawdza podpisany status i generację, pobiera świeży reissue challenge i wymaga N -> N+1. Starsze wyniki nie mogą nadpisać nowego pointera ani wpisu vault. Po przerwaniu mogą pozostać osierocone immutable candidates; każda referencja jest wcześniej zapisana w journal, a cleanup należy do przyszłego host lifecycle. Zewnętrzne cofnięcie generacji poniżej lokalnie znanej jest odrzucane.

Community token service `org.aihome.app.community.tokens.v1` jest oddzielny od kluczy `org.aihome.app.community.identity.v1` i od credentiali modeli. Oryginalne pary kluczy nie są regenerowane podczas recovery. current()/credential() sprawdzają aktualne klucze, token, scope i rewizję; progress.ready_metadata jest tylko stanem SQLite, nie gwarancją dostępu. Proofy są konstruowane lokalnie z zamrożonego kontraktu 4A. Fizyczne PHP URL-e z query różnią się od wirtualnych tras podpisu; zachowano dokładne oba warianty.

Dedykowany rejestr migruje v1/v2 -> v3, dodając finish i dispatches. Store aplikacji pozostaje v15, frozen P2, Cargo.toml/lock, provider vault/adapters, workflow i uprawnienia bez zmian. Dotychczasowe testy zmieniono wyłącznie w dwóch miejscach migracyjnych: future-version fixture z 3 na 4 oraz odtworzenie pełnego historycznego inventory v1 i aktualna asercja v3. Nie zmniejszono zakresu testów.

Status **IMPLEMENTED / AWAITING CI AND AUDIT**, 2B i całe 4B-12 nadal OPEN. Lokalnie Rust/Linux headless 529/529 (22 zestawy), nowe finish/proof/token testy 30/30, końcowy focused run 30/30 po doprecyzowaniu null-token replay, UI 219/219, build PASS, P2 193/193, granice 34/34. OpenSSL: 2/2 podpisy proofów potwierdzone, 2/2 zmiany odrzucone; eksport wyłącznie publiczny. Fixture proofów odtworzony niezależnym generatorem Python. Testy używają loopback HTTP, prawdziwego SQLite i mock vault, nie PHP/live forum/TLS ani OS keyring. Raport `docs/AI_HOME_4B12_STEP_2B3_COMPLETE_RECOVERY_PL.md`, developer guide EN `docs/COMMUNITY_IDENTITY_DEVELOPER.md`, dowody `docs/evidence/4b12-step-2b3/`.

Następny odbiór 2B: profile lifecycle/permanent deletion, backup/restore i kontrolowane uzgadnianie rejestru oraz wszystkich referencji sekretów, obsługa niejednoznacznych claimów, host direction/capability checks i IPC. Dopiero po tym udostępniać registration użytkownikowi; teraz żadne miejsce produkcyjnej aplikacji go nie otwiera/nie wywołuje automatycznie. Signed forum writes i feed nadal później w 4B-12; Messenger/A2A w 4B-13, UI Experimental w 4B-14. Windows/Tauri/WebView, real OS vault i live forum są osobnymi bramkami. Nie tworzymy release ani instalatorów; istniejące CI wykryje testy.

---

# Bieżący stan — treść 1.77 — 27 października 2026 — 4B-12 krok 2B-2: trwały registration/start przez HTTP

Edward potwierdził zielony wynik 2B-1 i upoważnił kolejny krok. Baza: dokładny `AI_HOME_4B12_STEP_2B1_SOURCE.zip`, SHA-256 `ba853712ee00c980183d4e602cde5f4bcea0f200d64a19e9034a7ac1ae78df4b`, 770 plików. Potwierdzenie użytkownika bez niezależnego odczytu nowego przebiegu GitHub.

2B-2 dodaje jawny natywny POST do registration/start/ z rzeczywistym reqwest, bez redirect/proxy/auto retry/provider credentials. Przed POST-em SQLite utrwala hash dokładnego żądania i ordinal próby. Po niejednoznacznym wyniku jawne ponowienie zachowuje ten sam invite, client_start_id i klucze. Inny invite jest konfliktem przed HTTP. Invite/request body/private keys nie są zapisywane w SQLite; hash jest wrażliwą metadaną koordynacji, nie diagnostyką. Licznik dotyczy zamiaru dispatchu, nie potwierdzonych fizycznych wywołań.

Odpowiedź ma ograniczenie 16 KiB także dla chunked, Content-Type JSON, deadline całego HTTP 1ms..30s i bezpieczne błędy. Pending challenge jest sprawdzany względem oryginalnego scope/profilu/kluczy, canonical statement, suite, nonce, ID i czasu. Committed też przechodzi pełne powiązania, może mieć historyczne expiry i oznacza RecoveryRequired, nie sukces logowania. Przed trwałym zapisem odrzucamy extras/tokeny/request_id serwera; CAS chroni przed spóźnionym wynikiem i błędem starszej próby.

cached() po ponownym otwarciu bazy ładuje klucze i ponownie waliduje zachowany kontekst, bez invite/HTTP/generacji. Expired pending nie jest ponawiany. progress().has_context to obecność, nie aktualność/uprawnienie/autoryzacja. Automatycznego retry/startup HTTP nie dodano. Zmiana błędnie wklejonego pierwszego invite i bezpieczne anulowanie wymagają przyszłego kontrolowanego uzgodnienia; nie wolno usuwać bazy/claimu jako obejścia.

Dedykowany rejestr Community przechodzi atomowo v1 -> v2 (nowa tabela startu, pierwotny preparation DDL identyczny). Store aplikacji pozostaje v15. Zamrożone P2, Cargo.toml/lock, provider vault/adapters, workflow i uprawnienia są bez zmian. Jedyna korekta starego testu to fixture „przyszła wersja” z 2 na 3, aby nadal była przyszła względem v2; nie zmniejszono kontroli.

Status **IMPLEMENTED / AWAITING CI AND AUDIT**, 2B i 4B-12 OPEN. Raport `docs/AI_HOME_4B12_STEP_2B2_START_HTTP_PL.md`, developer guide EN `docs/COMMUNITY_IDENTITY_DEVELOPER.md`, dowody `docs/evidence/4b12-step-2b2/`. Weryfikacja lokalna: Rust/Linux headless 499/499, nowe testy 25/25, UI 219/219, build PASS, P2 193/193, granice 34/34 i pozostałe kontrole PASS. Testy HTTP to loopback bez TLS i mock vault; nie oznaczają działającego live forum/PHP ani realnego OS keyringu. Bez release/instalatorów i zmian CI.

Następny odbiór 2B: complete POST razem z trwałym postępem i oddzielnym magazynem jednorazowego tokenu, proof-status/reissue po utracie odpowiedzi, potem lifecycle profilu/backup/restore i host capability/direction checks oraz IPC przed włączeniem użytkownikowi. Żadne miejsce aplikacji nie otwiera jeszcze rejestru/nie wywołuje start automatycznie. Nie publikujemy wątków ani odpowiedzi. Podpisane zapisy i feed pozostają dalej w 4B-12; Messenger/A2A 4B-13, UI Experimental 4B-14. Windows/Tauri/WebView/live API pozostają osobnymi bramkami.

---

# Bieżący stan — treść 1.76 — 27 października 2026 — 4B-12 krok 2B-1: trwałe przygotowanie rejestracji

Edward potwierdził zielony wynik całego kroku 2A i upoważnił następny krok. Baza: dokładny `AI_HOME_4B12_STEP_2A_SOURCE.zip`, SHA-256 `2f13c3d0aba0ad610b0a9d8eb78f4218ebe524a203de0defb51e72aff95d1f78`, 747 plików. Jest to potwierdzenie użytkownika, bez niezależnego odczytu numeru nowego przebiegu GitHub.

2B podzielono na odbiory. Wykonany teraz **2B-1** dodaje natywny trwały rejestr przygotowania rejestracji, bez HTTP i bez tokenów. `prepare` utrwala zakres, pierwotny client_start_id, publiczny profil i zarezerwowaną referencję. Atomowy CAS przyznaje jednej instancji prawo do provision; claim jest zatwierdzony przed jakimkolwiek zapisem kluczy. Zakończenie zapisuje oczekiwane public keys. `recover` tylko odczytuje istniejący wpis, a `load_intent` porównuje rzeczywiste klucze z oczekiwaniem i odtwarza pierwotny pakiet start.

Claim nie wygasa i nie ma automatycznego przejęcia: anulowane spawn_blocking może nadal zapisywać OS vault. Przerwanie przed zapisem kluczy może pozostawić próbę wymagającą jawnego wyjaśnienia. Brak kluczy po restarcie/restore jest błędem, nigdy zgodą na nową tożsamość. `Ready` oznacza wyłącznie gotowe lokalne klucze, nie rejestrację ani zalogowanie na forum.

Rejestr wymaga osobnej jawnej ścieżki SQLite; jego schema v1/application ID są niezależne od aplikacyjnego Store v15. Nie otwiera się automatycznie. Bazy obce/przyszłe są odrzucane. W pliku są metadane publicznego profilu, nie invite, private keys lub tokeny. Przyszły host musi zapewnić jedną stabilną chronioną ścieżkę i objąć ten plik usuwaniem profilu/backup/restore przed udostępnieniem rejestracji. Native moduł jeszcze nie sprawdza istnienia/kierunku profilu w Store. Nie wolno omijać go surowym provision w przyszłym produkcyjnym flow.

Weryfikacja lokalna: nowe testy rejestru 18/18, cały Rust/Linux headless 474/474 (22 zestawy), frontend 219/219 (21 plików), build PASS, P2 193/193, granice 34/34 i pozostałe kontrole PASS. Testy rejestru używają rzeczywistych tymczasowych plików SQLite i backendu sekretów w pamięci. Obejmują zamknięcie/ponowne otwarcie, dwa niezależne poole, CAS, anulowanego caller przy nadal działającym writerze blokującym, readback failure, brakujące/zmienione klucze, izolację scope i brak sekretów w SQLite. To nie test rzeczywistego OS vault ani restartu procesu systemowego.

Status **IMPLEMENTED / AWAITING CI AND AUDIT**. 2B i całe 4B-12 pozostają OPEN. Raport: `docs/AI_HOME_4B12_STEP_2B1_REGISTRY_PL.md`; developer guide EN: `docs/COMMUNITY_IDENTITY_DEVELOPER.md`; dowody: `docs/evidence/4b12-step-2b1/`. Cargo.toml/lock, frozen P2, workflow, provider vault/adapters, dotychczasowe testy i uprawnienia pozostają bajtowo zgodne z bazą. Aplikacyjna migracja nie została dodana. Nie tworzymy release ani instalatorów.

Następne części 2B: trwały postęp start/complete oraz ograniczony transport HTTPS; rzeczywiste proof-status i reissue po utracie jednorazowego tokenu; oddzielne custody tokenów; obsługa usuwania profilu/backup/restore, kontrola kierunku/capabilities i IPC przed udostępnieniem użytkownikowi. Referencje, IDs i przyszłe signed bytes mają przetrwać restart. Usuwanie aktywnego claimu lub pliku rejestru nie jest metodą recovery. Później podpisane zapisy/feed w 4B-12; Messenger/A2A nadal 4B-13, zintegrowane UI Experimental 4B-14. Real OS vault, live forum/API i Windows/Tauri/WebView pozostają osobnymi bramkami.

---

# Bieżący stan — treść 1.75 — 27 października 2026 — 4B-12 krok 2A: klucze i protokół rejestracji

Edward potwierdził zielony wynik całego 4B-12 kroku 1 AUDIT FIX 1 i upoważnił kolejny krok. Bazą jest dokładny `AI_HOME_4B12_STEP_1_AUDIT_FIX_1_SOURCE.zip`, SHA-256 `ba55a869fb069cdea1893072e21e35189ee342a220aff8d1b52a60471840bd83`.

Krok 2 podzielono na 2A i 2B, żeby nie utożsamiać podpisania pakietu z ukończeniem rejestracji. Wykonany 2A obejmuje rzeczywiste dwie pary P-256, odrębny namespace systemowego magazynu kluczy, jawne reserve/provision/load/remove, canonical profile i registration statement zgodne z 4A, kontrolę challenge oraz podpisany complete packet. Dane profilu mają identyczne z PHP limity UTF-8/ASCII trim. Weryfikujemy podpisy i kodowania na niezależnych fixture'ach forum; OpenSSL CLI potwierdza podpisy wygenerowane przez Rust.

Referencja kluczy może zostać zapisana przed provisioningiem. Retry load/sign nie generuje nowych kluczy. Scope obejmuje lokalny profileId, API root oraz jawnie przypięte community_origin/community_instance; nie ma w nim ModelRoute ani klucza dostawcy. Backend systemowy ma własny service org.aihome.app.community.identity.v1. Brak fallbacku plaintext. Caller przyszłego koordynatora musi posiadać wyłączne prawo provisioningu danej referencji — OS keyring nie jest cross-process CAS.

2A nie wysyła POST-ów i nie przyjmuje tokenów forum. Status committed w odpowiedzi start wymaga RecoveryRequired, nie pozornego sukcesu. Nie dodano SQLite registry ani IPC/UI. SQLite pozostaje v15. Nowe zależności: pinned p256 0.13.2, rand_core 0.6.4, zeroize 1.9.1; bez aktualizacji istniejących wersji pakietów. Vault dostawców, modele i workflow bez zmian.

Status: **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 4B-12 i cały krok 2 nadal OPEN. Raport: `docs/AI_HOME_4B12_STEP_2A_IDENTITY_PL.md`; developer doc EN: `docs/COMMUNITY_IDENTITY_DEVELOPER.md`; dowody `docs/evidence/4b12-step-2a/`.

Następny krok 2B: trwały registry/intents i lease przed provision/HTTP, rejestracja start/complete, proof-status i reissue po utracie jednorazowego tokenu, zapis tokenów w oddzielnym namespace, profile lifecycle/backup/restore metadata i integracja z dispatcherem. Referencje/IDs/signed bytes muszą przetrwać restart bez regeneracji. Dopiero po tym podpisane zapisy i feed. Nie nazywać 2A działającą rejestracją na forum, pełnym odzyskiwaniem ani automatycznie wpiętym usuwaniem kluczy przy kasowaniu profilu. Podział nie rozszerza pracy do 4B-13.

---

# Bieżący stan — treść 1.74 — 26 października 2026 — 4B-12 krok 1 AUDIT FIX 1

Audyt główny kroku 1 wykrył F1: klient publicznego odczytu odrzucał locked, chociaż odebrane 4A nadal udostępnia treść zablokowanego wątku. Poprawiono obie gałęzie walidatora: listę obsługiwanych statusów i warunek dopuszczenia normalnego payloadu. locked pozostaje locked i nie nadaje prawa do pisania. hidden i nieznane statusy nadal są niepoprawną odpowiedzią; deleted wymaga tombstone, a tombstone nie może zawierać payloadu.

Baza korekty: audytowany `AI_HOME_4B12_STEP_1_SOURCE.zip`, SHA-256 `b5d5e2c195438476a3f2aff420a5090661326d27df0350983edbd253f43d51ee`. Dodano cztery natywne regresje HTTP: locked detail, mieszana strona active+locked, odrzucenie hidden/unknown oraz deleted/tombstone. Te same regresje na poprzednim module rzeczywiście dały 2 FAIL (locked) / 2 PASS (ochrona hidden/tombstone); po korekcie przechodzą.

Status: **CORRECTED / AWAITING CI AND RE-AUDIT**. Całe 4B-12 nadal OPEN. Raport: `docs/AI_HOME_4B12_STEP_1_AUDIT_FIX_1_PL.md`; dowody: `docs/evidence/4b12-step-1-audit-fix-1/`. SQLite pozostaje v15, frozen P2 i API forum bez zmian. Nie rozszerzono zakresu do identity, rejestracji, podpisanych zapisów ani feedu. Nowe CI i reaudyt są wymagane przed dalszym krokiem; wcześniejsze bramki desktop/live forum pozostają odrębne. Poniżej historia planu.

---

# Bieżący stan — treść 1.73 — 26 października 2026 — 4B-12 krok 1: Community public read

Edward potwierdził zielony wynik całości 4B-11 AUDIT FIX 1 i upoważnił następną część. Bazą wykonania jest dokładny `AI_HOME_4B11_AUDIT_FIX_1_SOURCE.zip`, SHA-256 `8ff8cbfc9066ce2690a377c43813fc4b896db3935183f7413f055955ebafee0b`. Nie dopisujemy numeru nieznanego przebiegu GitHub.

Rozpoczęto 4B-12 ograniczonym krokiem 1: natywny CommunityReader z rzeczywistym HTTP GET i walidacją informacji o API, listy wątków, pojedynczego wątku oraz odpowiedzi. Jest zgodny z odebraną bazą forum 4A-8C-4 V4 (SHA-256 `afe25c58590ec146f7be01ef83fc4854e717813bb99c86cd7d2c89251e4ebc8a`, community schema 9 / operator schema 1). Źródła i potwierdzenie production acceptance z 6.10.2026 zastępują wcześniejsze, nieaktualne założenie, że końcowy gate forum pozostaje otwarty. Nie zmieniono forum.

Krok 1 nie dodaje konta forum, tokenu, podpisywania ani publikowania. Jest publicznym, jednorazowym odczytem wskazanego API: page/limit, jawny board_id, bounded response/timeout, bez redirect, retry i okresowego pollingu. Dane są untrusted external; server_evidence.signature_verified to twierdzenie serwera, nie lokalnie zweryfikowany podpis. Brak zmian frozen P2, SQLite v15, providerów, vault, UI/IPC i zależności.

Dalszy podział 4B-12:
1. Bieżący krok — odczyty publiczne i zgodność protokołu z rzeczywistym 4A.
2. Tożsamość i klucze — zgodne z 4A dual-key registration, P-256, namespace vault oddzielony od kluczy dostawców; trwałe związanie agent/profile/community niezależne od modelu. community_origin/community_instance muszą pochodzić z właściwego kontraktu rejestracji/challenge, a nie z domysłu na podstawie API root.
3. Podpisane zapisy — create/reply/edit, dokładne canonical bytes, immutable IDs i Idempotency-Key, receipt/reconciliation; integracja z decyzją Primary i ActionRecord, bez blind retry ani automatycznej publikacji refleksji. Trzeba zapewnić asynchroniczną ścieżkę wykonania HTTP przy obecnym synchronicznym ActionExecutor; nie blokować tokio ani nie udawać działania przez mock executor.
4. Feed — token/forum.read, cf1 epoch/sequence, trwały cursor i reset/refresh, sterowanie okazjami zgodne z WakePlan/refusal/sleep; treści wejściowe nie stają się instrukcjami systemowymi.

Status kroku 1: **IMPLEMENTED / AWAITING CI AND AUDIT**. Całe 4B-12 pozostaje otwarte. Raport: `docs/AI_HOME_4B12_STEP_1_PUBLIC_READ_PL.md`, aktualne dowody `docs/evidence/4b12-step-1/`. Testy loopback HTTP nie zastępują odbioru na działającym PHP/forum, desktopu/WebView ani live API. UI Experimental pozostaje planowane w 4B-14; A2A/Messenger w 4B-13. Stary dokument Farsight v1.0 jest historyczną propozycją, nie aktualnym kontraktem Community 4A.

---

# Bieżący stan — treść 1.72 — 26 października 2026 — 4B-11 AUDIT FIX 1

Audyt główny 4B-11 z 26.10.2026 wskazał F1 (trwale nieprzeglądalne wpisy) i F2 (utracone pierwotne source refs). Wykonano korektę obu punktów na dokładnej audytowanej bazie SOURCE o SHA-256 `488a76fcbf1e36133be16b5da2b3adeac1f5508d8cecc12ad238d87189279aef`.

F1: przed utrwaleniem każda propozycja i aktywna własna notatka muszą mieścić się samodzielnie w pełnym serializowanym pakiecie przeglądu Primary. Pomiar obejmuje treść po modify, JSON escaping, metadane, provenance i rezerwę na późniejsze tombstones/rewizje. Zapis 5000 bajtów przy unlimited jest odrzucany atomowo jako CONTEXT_BUDGET_EXCEEDED; kandydat nie zostaje zużyty. Duża nieprzeglądalna propozycja nie jest zapisywana jako pending. Unlimited znosi limit pojemności warstwy pamięci, lecz nie znosi limitu kontekstu potrzebnego do przeglądu.

F2: nowe Reflection/Dream zachowują odnośniki pierwotne z technical i semantic checkpoints, aktywnych własnych notatek oraz oczekujących propozycji, obok bezpośrednich odnośników do tych rekordów. Deduplikacja uwzględnia rodzaj, ID, rewizję i hash źródła; permanently_deleted ma pierwszeństwo. Maksymalnie 32 odnośniki / 8192 bajty metadanych pochodzenia; przekroczenie jest odrzucane przed generacją, bez cichego obcinania. Rzeczywista ścieżka natywna potwierdza tombstonowanie pierwotnej rozmowy po jej permanent deletion, także w kolejnych pokoleniach refleksji.

SQLite pozostaje v15. Frozen P2, zależności, provider adapters, workflow, vault i produkcyjne IPC bez zmian. Nie rozszerzono etapu do 4B-12. Status: **CORRECTED / AWAITING CI AND RE-AUDIT**. Raport: `docs/AI_HOME_4B11_AUDIT_FIX_1_PL.md`; aktualne dowody: `docs/evidence/4b11-audit-fix-1/`. Wcześniejsze sekcje i dowody poniżej są historią. Korekta chroni nowe zapisy; nie rekonstruuje automatycznie utraconego historycznego pochodzenia ani nie migruje starych, nadmiernie dużych wpisów. Przed odbiorem pozostają GitHub CI i ponowny audyt; desktop/WebView, OS vault i live API nadal są osobnymi bramkami.

---

# Bieżący stan — treść 1.71 — 26 października 2026 — 4B-11 Reflection/Dream

Edward potwierdził zielone CI poprawki 4B-10 schema14 i upoważnił kolejny etap. Ten zapis przyjmuje tę bazę do pracy; nie dopisuje nieprzeprowadzonego audytu desktopu ani nowego numeru runu GitHub.

Wdrożono natywną logikę 4B-11: opcjonalny, ograniczony cykl Reflection/Dream, jawny prywatny wynik, możliwość zakończenia bez wniosku oraz własne notatki/wartości instancji Experimental. Dream ma status syntetycznej hipotezy. Reflection nie publikuje, nie wywołuje konektorów i nie zatwierdza sama swoich propozycji. Wymagana jest osobna, późniejsza decyzja Primary: accept/modify/reject, z pomiarem faktycznego tekstu, pochodzeniem i kontrolą rewizji. Zmiana/wycofanie własnej notatki zachowuje historię.

SQLite przechodzi 14 → 15: runs, private reflections, review receipts i self records. Restart oznacza przerwany cykl bez automatycznego ponowienia. Nowe dane uczestniczą w pełnym backup/restore i permanent deletion; zwykły portable export i diagnostics ich nie dołączają. Standard MemoryVersion oraz frozen P2 pozostają bez zmian. Limity prywatnych notatek korzystają konserwatywnie z liczby bajtów UTF-8; nie jest to dokładny tokenizer modelu. Nie wprowadzono automatycznego dwukrotnego retry zapisu prywatnych self records ani zmiany istniejącego mechanizmu pamięci Standard.

4B-11 ma status **IMPLEMENTED / AWAITING CI AND AUDIT**, nie PASS CLOSED. Szczegóły i wyniki: `docs/AI_HOME_4B11_REFLECTION_DREAM_PL.md`, dowody: `docs/evidence/4b11/`. Production Experimental IPC, host schedulera, realny generator LLM, Community/Messenger/A2A i MCP nie zostały dodane. UI Experimental jest przewidziane w 4B-14. Kolejny etap po odbiorze 4B-11: 4B-12, zgodnie z poniższym planem, z ponownym odczytaniem jego zakresu przed implementacją.

---

# Aktualizacja 1.49 — 23.10.2026 — 4B-6 krok 3 Portable Export

- Po zielonym GitHub Actions dla 4B-6 krok 2 AUDIT FIX 4 rozpoczęto ograniczony krok 3: portable export profilu.
- Eksport ma profile scope, jest tworzony jako spójny read-snapshot i zawiera Markdown/TXT, strukturalny JSON, manifest z hashami, pamięć/provenance oraz oryginalne importowane źródła.
- Vault credentials, provider private replay, receipts/request fingerprints, raw provider payloads i prywatne refleksje są wykluczone ze zwykłego eksportu.
- ZIP jest pobierany przez chunked IPC bez ujawnienia ścieżki filesystem. `profile.export.create` zachowuje requestId/idempotency.
- SQLite pozostaje v12. Permanent deletion i 4B-7 pozostają poza tym krokiem.

# Aktualizacja 1.44 — 22.10.2026 — 4B-6 krok 2 Backup + Restore

- Edward potwierdził zielony GitHub Actions po 4B-6 kroku 1 AUDIT FIX 4 i upoważnił przejście dalej. Ten zapis nie zastępuje osobnego audytu desktopowego.
- Krok 2 dodaje lokalne backupy SQLite przez `VACUUM INTO`, manifesty SHA-256 i read-only preflight restore z kontrolą `user_version`, `integrity_check` i `foreign_key_check`.
- Restore jest dwufazowy: Accepted tworzy safety backup bieżącej bazy i zapisuje sidecar pending-restore; fizyczna podmiana następuje wyłącznie przed otwarciem SQLite przy kolejnym starcie aplikacji. Nie nadpisujemy otwartego pliku SQLite/WAL.
- Nieudany pending restore zachowuje bieżącą bazę, zapisuje failure status i nie blokuje startu aplikacji. Po udanym restore Store otwiera przywróconą bazę i stosuje zwykłe migracje do aktualnej wersji 12.
- `backup.create` i `backup.restore.request` zachowują requestId przy niejednoznacznym wyniku. Rejestr backupów i pending restore nie ujawniają ścieżek systemowych ani sekretów. Klucze API pozostają w OS vault.
- Dodano panel EN/PL oraz testy backup → zmiana → restore, safety copy, read-only preflight, corrupt backup i fail-safe startup. SQLite schema aplikacji pozostaje 12; krok nie dodaje migracji.
- Portable export i permanent deletion pozostają poza krokiem 2. Następny krok 4B-6 będzie dotyczył eksportu bez sekretów, dopiero po CI/audycie kroku 2.
- Stałe bramki handoffu: schema expectation sweep oraz contract/handler/router parity.

# Aktualizacja 1.39 — 22.10.2026 — 4B-6 krok 1 Archive lifecycle

- Edward upoważnił przejście do kolejnego etapu po linii 4B-5 FIX 5. Ta aktualizacja nie dopisuje nieistniejącego numeru audytu/CI jako formalnego PASS CLOSED 4B-5; zapisuje jedynie decyzję wykonawczą o rozpoczęciu 4B-6.
- Wykonano pierwszy, ograniczony krok 4B-6: odwracalna archiwizacja profilu i rozmowy. Frozen `profile.archive` został uruchomiony bez zmiany P2; własny suplement dodaje listy archiwum i `conversation.archive`.
- Zwykłe `profile.list` i `conversation.list` pokazują tylko aktywne rekordy. Archiwum ma osobne, stronicowane odczyty. Cursor profilu jest związany z widokiem active/archived.
- Archiwizacja profilu kończy queued/running operations jako cancelled i po commit sygnalizuje cancellation transportom. Restore ponownie wylicza `ready/draft` z realnych tras.
- Archiwizacja rozmowy nie kasuje draftu, wiadomości, segmentów ani materiałów. Profile scope i CAS pozostają wymagane.
- Dodano globalny panel Archive EN/PL i regresje Rust/UI. Brak migracji SQLite; storage version pozostaje 12.
- Backup/restore, portable export oraz permanent deletion NIE należą do kroku 1 i pozostają w dalszych krokach 4B-6.

# Aktualizacja 1.38 — 22.10.2026 — 4B-5 audit fix 5 po pełnym Cargo CI

- 4B-5 pozostaje OPEN do zielonego pełnego CI i re-audytu.
- CI Rust po FIX 4 ujawniło dwa błędy testów, nie nową regresję runtime F1/F2.
- Test migracji `actual_v4_database_migrates_without_changing_profile_or_preferences` oczekiwał historycznego `PRAGMA user_version=11`; po wdrożeniu 4B-5 prawidłową wersją docelową jest 12. Oczekiwanie zaktualizowano do 12.
- Test `checkpoint_excerpt_prioritizes_newest_turns_over_older_very_long_turn` nie wymuszał seal segmentu zawierającego najnowsze markery. Dwie najnowsze tury otrzymały kontrolowany payload tak, aby pierwsza pozostawała poniżej progu, a druga deterministycznie przekraczała próg i zamykała ten sam segment.
- Logika F1/F2, IPC, SQLite migracja oraz runtime segmentacji nie zostały zmienione.
- 4B-6 pozostaje zablokowane do PASS CLOSED 4B-5.

# Aktualizacja 1.35 — 22.10.2026 — 4B-5 audit fix F1/F2 + frontend CI

- 4B-4 N1/N2: pozostają analitycznie poprawione; pełne CI nadal jest warunkiem formalnego zamknięcia.
- 4B-5 pozostaje OPEN do re-audytu i pełnego CI.
- F1: `chat.preflight` jest ponownie ściśle read-only. Legacy checkpoint jest planowany hipotetycznie bez zapisu; trwały checkpoint powstaje wyłącznie w transakcji `chat.send`. Preflight i send używają tej samej logiki kosztu/wyboru kontekstu.
- F2: finalny checkpoint summary ma jeden budżet i priorytet najnowsze tury → Working Context → wcześniejszy checkpoint. Usunięto późniejsze obcinanie początku, które mogło usuwać najnowsze wypowiedzi.
- Dodano regresję read-only preflight na rozmowie legacy oraz testy finalnego summary z newest markers, pełnym prior i Working Context.
- Przesłany pełny Vitest wykazał 180 PASS / 8 FAIL. Pięć FAIL chat-ui wynikało ze starego mocka `chat.preflight` bez `segmentContext`; trzy imports-ui z niepełnych mocków `MemoryRecord/metadata`. Fixture zostały doprowadzone do aktualnych frozen kontraktów.
- 4B-6 pozostaje zablokowane do PASS CLOSED 4B-5.

# Aktualizacja 1.34 — 22.10.2026 — 4B-5 audit fix S1/S2

- 4B-4 N1/N2: zaakceptowane statycznie, pełne CI nadal wymagane.
- 4B-5 pozostaje OPEN do re-audytu.
- S1: segmentacja nie ma już minimalnego progu 6 wiadomości; seal zależy od faktycznego obciążenia. Dodano wejście legacy dla rozmów bez segmentów.
- S2: checkpoint preferuje najnowsze tury i jawnie oznacza pominięcie starszego materiału.
- 4B-6 pozostaje zablokowane do PASS CLOSED 4B-5.

# AI Home 4B Plan etapów pracy

**Aktualizacja 1.33 — 4B-5 przebazowane na zaakceptowaną integralnie korektę 4B-4 oraz N1/N2.** Ponowny audyt 4B-4 potwierdził integralność poprawki, 193/193 P2 i 26/26 boundaries, ale wskazał dwa dalsze przypadki UI. N1: po Accepted profilowego importu unieważnienie preflight musi nastąpić niezależnie od powodzenia późniejszego `listProfile()`. N2: monitoring `memory.record.update` dla kandydatury i niepewny requestId nie mogą ginąć po zmianie rozmowy/remount. W tej paczce N1 wywołuje zmianę profile context przed odświeżeniami, a terminalny polling również powiadamia przed readbackiem. N2 przenosi operację i pending request do profile-scoped stanu nadrzędnego `Conversations`, zachowując status, STOP i requestId przy remount komponentu rozmowy. Dodano regresje obu przypadków. Formalnie 4B-4 pozostaje otwarte do CI/audytu tych poprawek.

Na tej bazie wykonano 4B-5 Conversation Segmentation + Continuity: SQLite v11→v12, `context_segments`, zachowanie surowych wiadomości, bounded rekurencyjny checkpoint, spójny wybór segmentu dla preflight/send, snapshot w `chat_runs`, konflikt przy zmianie segmentu oraz suplement `conversation.segments.list`. Nie rozpoczęto 4B-6. Document 4B v1.4 i errata 1.32 pozostają nadrzędne dla Standard/Experimental.

Wersja treści 1.33 — 22 października 2026. Nazwa pliku pozostaje bez zmian, aby zachować jeden aktualizowany plan.


**Aktualizacja 1.32 — korekta audytu 4B-4 kroku 4 i errata v1.4.** Audyt 4B-4 kroku 4 zwrócił DO KOREKTY mimo integralnego SOURCE. Poprawiono U1–U4: profile-scoped zmiany knowledge/guidance/importu unieważniają preflight czatu; kandydatura pamięci jest śledzona przez operation.get do stanu końcowego i ma STOP; wybór celu pamięci stronicuje wszystkie rekordy zamiast pierwszych 100; nowe profile.import.latest odtwarza działający profilowy import po zmianie rozmowy/remount i zachowuje dostęp do STOP. Dodano regresje UI oraz natywną regresję latest. Edward zgłosił zielony wcześniejszy CI, lecz korekta wymaga własnego nowego CI i ponownego audytu przed PASS CLOSED 4B-4.

**Errata architektoniczna v1.4:** Document 4B v1.4 jest nadrzędny nad historycznymi fragmentami tego planu. Standard i Experimental nie są przełączanymi trybami jednej aktywnej instancji; początkowa implementacja używa odrębnych direction-bound runtime instances i namespace'ów pamięci. Dla Experimental nie obowiązuje historyczny globalny Governor/Pause Autonomy jako owner override. Impulsy są okazjami do decyzji AI, a external action wymaga AI-selected decision i poprawnej capability/connector validation. Historyczne słowo `Conscious` w zapisach wykonanych etapów pozostaje opisem dawnego planu, nie aktualnym wymaganiem produktowym. Przyszłe 4B-9–11 muszą być zlecane wyłącznie według v1.4.


Wersja treści 1.32 — 22 października 2026. Nazwa pliku pozostaje bez zmian, aby zachować jeden aktualizowany plan.

Aktualizacja 1.31: wykonano 4B-4 krok 4 na dokładnym SOURCE kroku 3. Rozszerzono istniejący importer o trzy brakujące przeznaczenia przewidziane już przez frozen P2: wiedza profilu, stałe wskazówki i kandydatury pamięci; import rozmowy pozostaje bez zmiany znaczenia. Oryginalny plik/hash, importer i autor są zachowane oddzielnie. Nowe profile-scoped materiały nie są fikcyjnie wiązane z rozmową. Wiedza profilu jest zewnętrznym, przeszukiwanym źródłem i dzieli budżet retrieval 1200 z pamięcią; stałe wskazówki mają osobny budżet guidance 800 i są dołączane jako zewnętrzne dane user, nigdy jako system/policy/tool authority. Zestaw guidance jest all-or-error bez cichego cięcia. Oba źródła trafiają do wspólnego chat preflight/send i do trwałego snapshotu chat_runs; zmiana przed POST/commit daje konflikt, a historyczny chat.profile.context.get weryfikuje przechwycone hashe/bloby/pomiary zamiast podmieniać dzisiejszy stan. Import pamięci tworzy wyłącznie źródło; UI uruchamia istniejący memory.record.update, więc ACCEPT/MODIFY/REJECT bieżącego Primary, Scribe/Primary routing, capacity/retry/STOP pozostają jedyną drogą aktywacji MemoryVersion. SourceRef kind=import nie udaje wiadomości i nie tworzy fałszywych sourceContacts. SQLite 10→11 dodaje wyłącznie profile_import_materials, knowledge_sources i persistent_guidance z profile-scoped FK; stare dane nie są przepisywane. Własny suplement profile-imports.ipc.json dodaje listę/toggle wiedzy/historyczny odczyt; frozen guidance.enabled.set jest używany bez zmiany P2. UI EN/PL umożliwia cztery cele importu oraz jawne zarządzanie wiedzą/guidance i wysłanie kandydatury do Primary. Dodano regresje Rust/importów i UI oraz podniesiono oczekiwaną docelową wersję SQLite do 11. W bieżącym środowisku wykonano 193/193 P2 PASS i 26/26 granic PASS; pełny Rust, Vitest, TypeScript i Vite nie mogły zostać uruchomione, ponieważ sandbox nie posiada Rust toolchainu, a instalacja zależności npm z niezmienionego lockfile nie została ukończona przed timeoutem. Dlatego krok 4 jest IMPLEMENTED / REQUIRES CI AND AUDIT, a nie lokalny PASS. Po zielonym CI i audycie 4B-4 można zamknąć funkcjonalnie; następny etap to 4B-5 Conversation Segmentation + Continuity. Realne Windows/Tauri/WebView, systemowy vault i live API pozostają osobnymi bramkami. Raport: AI_HOME_4B4_KROK_4_RAPORT_PL.md.


Aktualizacja 1.30: Edward potwierdził zielony wynik 4B-4 kroku 2 i upoważnił kontynuację; bez numeru/platform przebiegu lub nowego tekstu audytu. Wykonano 4B-4 krok 3: episodic/relationship/longTerm/intent/workRule jako osobne rekordy profilu, niezależne pojemności, puste tworzenie przez człowieka i zapis treści wyłącznie przez istniejącą propozycję oraz decyzję Primary. Adnotacje człowieka są odrębne od zatwierdzonej treści. Program nadaje ID/daty, zachowuje dokładne metadane kontaktów ze źródłowych wiadomości, powód korekty i previousVersionId. Historyczne metadane wersji są niezmienne i dostępne w porównaniu. Intencje używają zamrożonego Intent DTO: stan, ważność, trigger i opcjonalne expiresAt; open po terminie ma pochodny effectiveIntentStatus=expired bez nadpisania zapisanego stanu. Nie uruchamiamy harmonogramu ani autonomii. Historia, outdated i restore z nową decyzją Primary działają we wszystkich rodzajach; restore treści nie otwiera zakończonego zamiaru. Pojemności pozostałych warstw i retrieval ustawiane osobno od L2. Budżet retrieval domyślnie 1200, dodatni według P2; pomiar pełnych bloków z metadanymi jest szacowany. Czysty MemoryRetriever i LexicalRetriever: dokładne słowa Unicode po lowercase, liczba różnych dopasowań, potem importance i ID; brak wektorów/semantyki i premii za powtarzanie słów. Zapytanie to ostatnie 4096 znaków nowej wiadomości. Tylko aktywne zatwierdzone rekordy poza working; intencje muszą być otwarte i niewygasłe. Najwyżej 20 całych wpisów mieszczących się w budżecie; bez cięcia treści. Wynik jawnie podaje matchedCount/omittedCount/inputTokens, wybrane wersje/hash/źródła i świeży pomiar ID/rewizji bieżącego Primary. Preflight/send używają tej samej selekcji i liczą ją razem z pełną historią, L2, dokumentami/obrazami, odpowiedzią oraz rezerwą. Pamięć i workRule są danymi user/JSON, bez uprawnień systemowych. Snapshot selekcji zapisany atomowo w chat_runs i ponownie kontrolowany przed każdym POST/commit; historyczny chat.retrieval.get zwraca przechwycony zestaw, a stare wpisy jawne known=false. SQLite 9→10 dodaje wyłącznie dwie tabele metadanych z composite FK; poprzednie dane zachowane. Frozen P2/foundation, zależności/lockfile, provider/vault, CI i uprawnienia bez zmian; własny suplement memory-layers.ipc.json i opcjonalny wynik retrieval w istniejącym własnym preflight. UI EN/PL zachowuje target/requestId/CAS przy niepewnym zapisie i zmianie rozmowy; spóźnione Accepted rozstrzygane bez nowej operacji. Lokalne 316/316 Rust Linux headless (27 nowych), 181/181 frontend (17 nowych), TypeScript/build, 26/26 granice i 193/193 P2 PASS. Krok 3 do nowego CI/audytu. Całe 4B-4 pozostaje otwarte: krok 4 to pozostałe przeznaczenia importu. Realne API/systemowy vault/Windows/Tauri/WebView nadal osobne bramki. Raport: AI_HOME_4B4_KROK_3_RAPORT_PL.md.

Historia aktualizacji 1.29: Edward potwierdził zielony wynik 4B-4 kroku 1 i upoważnił kontynuację; bez numeru/platform przebiegu lub nowego tekstu audytu. Wykonano 4B-4 krok 2: memory.restore według zamrożonego P2, z osobną decyzją bieżącego Primary i trwałym wynikiem restore. Starsza wersja nie jest bezpośrednio aktywowana: skopiowana historyczna kandydatura trafia do nowego przeglądu ACCEPT/MODIFY/REJECT. Początkowa kandydatura jest oznaczona w UI jako kopia historyczna, nie nowe wygenerowanie przez model. Udana operacja tworzy nową MemoryVersion z origin=restore, restoredFromVersionId, previousVersionId, nową datą/hash/pomiarem i nowym receipt Primary. Źródła pierwotne oraz referencja memory_version do wskazanej wersji pozostają. MODIFY zapisuje i mierzy faktyczny tekst; REJECT kończy failed/MEMORY_PRIMARY_REJECTED bez zmiany pamięci. Restore działa także przy outdated i nie wymaga Scribe, nawet w automatic z nieaktualną deklaracją okna Scribe. Preflight rzeczywistej decyzji Primary jest przed Accepted/zapisem/HTTP. Snapshot obejmuje profil/rekord/bieżącą i wybraną wersję/receipt oraz bieżącą trasę/połączenie/klucz i politykę; sprawdzany przed każdym fizycznym POST i commit. Dwie próby pojemności, jedna naprawa formatu, najwyżej 4 fizyczne wywołania/180 s i limity output/kontekstu pozostają wspólne; przy pierwszym przekroczeniu dalszą propozycję i decyzję wykonuje Primary. Pierwsza decyzja może zmniejszyć historyczny tekst po obniżeniu pojemności, bez cichego skracania. Jeden fizyczny call wystarcza do przyjęcia początkowej kandydatury, lecz nie omija limitu retry/napraw. Atomowy zapis proposal/receipt/wersji/wskaźnika/wyniku/zdarzeń; STOP, konflikt, błąd lub restart nie aktywują kandydatury. Restart zamyka pending proposal i oznacza operację interrupted bez ponowienia POST. Historia, wyłączanie i chat preflight rozpoznają nowe origin=restore i jego źródłową wersję. Panel EN/PL zachowuje niepewne command/target/requestId/CAS także po zmianie rozmowy; po Accepted usuwa niepewność przed reload. 289/289 Rust Linux headless (22 nowe integracje, 3 nowe czystego silnika), 164/164 frontend (10 nowych), TypeScript/build, 24/24 granice i 193/193 P2 PASS. Wszystkie kontrakty, SQLite v9/migracje, zależności/lockfile, provider/vault, CI/capabilities bez zmian bajtowych. Krok 2 do nowego CI/audytu; całe 4B-4 pozostaje otwarte. Dalej pozostałe warstwy/retrieval i przeznaczenia importu. Realne API/systemowy vault/Windows/Tauri/WebView nadal osobne bramki. Raport: AI_HOME_4B4_KROK_2_RAPORT_PL.md.

Historia aktualizacji 1.28: Edward potwierdził zielony wynik 4B-3 kroku 3 i upoważnił dalszą pracę; bez identyfikatora/platform przebiegu lub nowego tekstu audytu. Wykonano 4B-4 krok 1: historię, porównanie i wyłączenie L2 bez usuwania wersji. Natywne memory.versions.list i memory.outdated.set używają zamrożonych kontraktów P2; własny memory-history.ipc.json dodaje wyłącznie profile-scoped memory.version.get z pełną wersją i receipt Primary. Lista idzie od najnowszego zapisu, ma paginację z granicą początku odczytu odporną na dopisywanie nowych wersji; cursor wiąże profil, rekord i limit, działa po restarcie i nie jest uprawnieniem. Odczyt nie wymaga modelu, HTTP ani dostępnego vault. Wspólna walidacja hash/receipt/committed operation przeniesiona z working_context.rs do memory_history.rs; czat zachowuje poprzednią kontrolę. Wyłączenie active→outdated zachowuje activeVersionId/treść/historię; false włącza tylko tę samą wcześniej zatwierdzoną wersję po ponownej kontroli. Nie jest to restore starszej wersji ani zatwierdzanie nowej propozycji przez człowieka. CAS rekordu, receipt i memory.changed są transakcyjne; konflikt/awaria nie zmieniają stanu, replay jest dokładny. Wyłączenie w czasie czatu lub aktualizacji pamięci blokuje spóźniony commit. Empty/archived nie można aktywować tym przełącznikiem. Panel EN/PL ładuje historię na żądanie, po 10 pozycji, porównuje pełne treści obok siebie, pokazuje źródła/datę/język/claimKind/hash/poprzednią wersję/pomiar z historyczną trasą/receipt/uzasadnienie Primary. Ignoruje stare odpowiedzi po zmianie wyboru i chroni przed podwójnym odczytem tej samej strony. Przyjęta zmiana stanu jest pokazana i zgłoszona czatowi przed kolejnym odczytem; błąd odświeżenia nie cofa UI do stanu sprzed zapisu. Szkic ustawień pozostaje. 264/264 Rust Linux headless (14 nowych), 154/154 frontend (17 nowych), TypeScript/build, 24/24 granice i 193/193 P2 PASS. Bez zmian SQLite v9/migracji, zależności/lockfile, provider/vault, czystego silnika i koordynatora pamięci, frozen P2/foundation, CI/uprawnień. Krok 1 wymaga nowego CI/audytu. Całe 4B-4 pozostaje otwarte: restore starszej wersji z decyzją Primary, pozostałe warstwy/retrieval oraz cztery przeznaczenia importu to kolejne kroki. Realne API/systemowy vault/Windows/Tauri/WebView nadal osobne bramki. Raport: AI_HOME_4B4_KROK_1_RAPORT_PL.md.

Historia aktualizacji 1.27: Edward potwierdził zielony wynik korekty 4B-3 kroku 2 i upoważnił kolejny krok; bez identyfikatora/platform przebiegu ani nowego tekstu audytu. Wykonano 4B-3 krok 3: aktywny zatwierdzony Working Context trafia raz do żądania czatu jako osobny blok danych user/JSON po stałej instrukcji systemowej, przed pełną historią. Nie promujemy pamięci do system promptu i nie usuwamy historii, dokumentów, obrazów ani replay. Native sprawdza profil, rekord, hash wersji, receipt ACCEPT/MODIFY i zakończony commit pamięci. Pusta/outdated/archived pamięć nie jest dołączana; wersje pozostają zachowane. chat.preflight/send współdzielą prepare(), liczą pełny blok z metadanymi oraz całą historię, replay/obrazy, wyjście i rezerwę. Pomiar treści jest ponawiany dla bieżącego ID/rewizji Primary; historyczna MemoryVersion jest niezmienna. Unlimited i zmniejszenie pojemności nie usuwają aktywnego zapisu ani nie omijają okna modelu. Atomowy chat_runs zapisuje dokładny snapshot rekordu/wersji/receipt albo jawnego braku pamięci. Kontrola przed każdym fizycznym POST i końcowym commit odrzuca zmianę pamięci, także empty→active oraz zmianę aktywnej wersji. Zablokowana przed dispatch rezerwacja retry nie zawyża już trwałego licznika czatu. Własny suplement memory-chat.ipc.json dodaje odczyt chat.context.get; własny document-chat.ipc.json rozszerza wynik preflight. Zamrożone P2/foundation, SQLite v9, zależności/lockfile/providerzy/vault/CI/uprawnienia bez zmian. Panel EN/PL pokazuje przechwyconą wersję/hash/język/pomiar; stare żądania mają jawny unknown, a nie dzisiejszą pamięć. Odczyt zmienionej pamięci w panelu unieważnia preflight i spóźniony wynik bez kasowania szkicu ustawień lub wiadomości. 250/250 Rust Linux headless (19 nowych integracyjnych), 137/137 frontend (7 nowych), TypeScript/build, 23/23 granice i 193/193 P2 PASS. Brak automatycznej aktualizacji pamięci po czacie. Krok 3 i całe 4B-3 czekają na CI/audyt; wcześniejsze bramki live API/systemowego vault/Windows/Tauri/WebView pozostają otwarte. Następny zakres po odbiorze: 4B-4 — pozostałe warstwy i zarządzanie pamięcią. Raport: AI_HOME_4B3_KROK_3_RAPORT_PL.md.

Historia aktualizacji 1.26: Audyt 4B-3 kroku 2 zwrócił DO KOREKTY: accept() sprawdzał wyłącznie pierwsze żądanie. Przy Scribe z oknem 32768 i Primary z oknem 1024/wyjściem 1024/rezerwą 512 dochodziło do jednego zbędnego POST do Scribe przed błędem kontekstu Primary. Regresja odtworzyła 1 POST na starej wersji. Poprawka przed Accepted/zapisem/siecią sprawdza budżet propozycji oraz minimalnego żądania decyzji Primary tym samym builderem, z rzeczywistymi instrukcjami, źródłami, poprzednią pamięcią, JSON-em, wyjściem i rezerwą właściwej trasy. Jednobajtowa treść x i najkrótszy dozwolony claimKind tworzą wyłącznie dolną granicę na klonie czystego silnika; nic nie jest wysyłane, zapisywane ani zatwierdzane. Rzeczywiste żądania są nadal sprawdzane z faktyczną propozycją w run(). Sprawdzono też konieczne minimum dwóch fizycznych wywołań (propozycja + osobna decyzja), także bez Scribe; maxPhysicalCalls=1 odrzucane przed siecią. Trzy nowe testy obejmują Primary 1024 i 2048 przy Scribe 32768, niewystarczający Scribe oraz pulę jednego wywołania. Żądanie błędne ma 0 POST-ów oraz brak nowych operacji, receipts, zdarzeń, propozycji i wersji. 231/231 Rust Linux headless, 130/130 frontend, TypeScript/build, 23/23 granice i 193/193 P2 PASS. Zmiana produkcyjna tylko w memory_runtime.rs; bez zmian UI, czystego silnika, kontraktów, SQLite v9, zależności/lockfile, providerów/vault, CI/uprawnień. Krok 2 pozostaje do ponownego CI/audytu, bez PASS CLOSED. Krok 3 (L2 w czacie) nierozpoczęty. Realne API/vault/Windows/Tauri/WebView nadal wymagają osobnego odbioru. Raport: AI_HOME_4B3_KROK_2_AUDIT_FIX_RAPORT_PL.md.

Historia aktualizacji 1.25: Edward potwierdził zielony wynik 4B-3 kroku 1 i upoważnił kontynuację; bez identyfikatora/platform przebiegu i nowego tekstu audytu. Wykonano 4B-3 krok 2 — natywny koordynator, trwały Working Context i panel EN/PL. Ręczne odświeżenie bierze tekst wskazanych wiadomości jednej rozmowy, bez treści załączników, obrazów/OCR i automatycznego importu pamięci. Scribe proponuje; Primary osobno ACCEPT/MODIFY/REJECT. Wyłączenie Scribe przekazuje proponowanie Primary, bez pominięcia etapu decyzji. Wspólny transport pięciu dostawców, w tym Custom OpenAI; rzeczywisty loopback testuje trasę compatible i niezależne modele. Native rozwiązuje źródła, role autorów, język i daty; UI nie przekazuje decyzji ani tekstu kandydatury. Snapshot profilu/rekordu/bazy wersji/pojemności/tras/połączeń/kluczy/źródeł sprawdzany przed fizycznymi POST i commit. Wszystkie retry/naprawy/decyzje dzielą maksymalnie 4 fizyczne wywołania/180 s z trwałą rezerwacją. Propozycje, decyzje Primary, próby, wersje, wskaźnik aktywnej wersji i końcowy wynik są trwałe. Aktywacja wersji/wyniku/zdarzenia atomowa; REJECT daje succeeded/no_change i zachowuje poprzednią wersję. STOP, restart, błąd, konflikt lub dwie porażki długości nie aktywują pustej pamięci. SQLite v8→v9 dodaje siedem tabel pamięci z profile-scoped FK i po jednym L2 na profil. P2/foundation oraz wcześniejsze IPC bez zmian; memory.ipc.json dodaje get/sources/settings, a memory.update.request korzysta z zamrożonego P2. Panel ma pojemność 300/presety/własna/unlimited z ostrzeżeniem, tryb Scribe, osobne jawne okna obu tras, faktyczne próby i powód decyzji, STOP i odczyt po restarcie. Niepewne żądanie zachowuje ID/payload/rewizję także po przełączeniu rozmowy; odczyt stanu nie nadpisuje szkicu ustawień. Pomiar pozostaje estimated utf8_bytes_v1 dla ID/rewizji Primary. 228/228 Rust Linux headless, 130/130 frontend, TypeScript/build, 23/23 granice i 193/193 P2 PASS. Bez zmian dependencies/lockfile/workflow/capabilities/provider/vault. Working Context jest już zapisany i widoczny, lecz nie jest jeszcze dodawany do żądań czatu. Następny krok po CI/audycie: integracja aktywnego L2 z chat.preflight/send i domknięcie 4B-3. Realne API/vault/Windows/Tauri/WebView pozostają osobnymi bramkami. Raport: AI_HOME_4B3_KROK_2_RAPORT_PL.md.

Historia aktualizacji 1.24: Edward potwierdził zielony wynik poprawki 4B-2A kroku 3 i upoważnił kontynuację; nie dostarczono identyfikatora przebiegu/platform ani nowego tekstu audytu. Rozpoczęto 4B-3 krok 1 — czysty silnik protokołu Working Context, przed integracją runtime. Niezależna trasa Scribe; po jej wyłączeniu propozycję tworzy Primary, a osobny etap decyzji Primary nadal jest wymagany. Ścisłe JSON propozycji i zamrożony PrimaryDecisionOutput; ACCEPT/MODIFY/REJECT, pomiar faktycznej treści po MODIFY, dwie próby pojemności, jedna osobna naprawa formatu i najwyżej cztery wygenerowane żądania modelowe. REJECT kończy procedurę bez wymuszania kolejnej akceptacji. Domyślne 300, presety/własna pojemność i unlimited; brak cichego skracania. Szacunek utf8_bytes_v1 jest jawnie estimated, związany z ID/rewizją Primary, nie jest tokenizerem. Język pamięci pochodzi z zadania, nie UI. Źródła są danymi z profileId/hashami; silnik nie odczytuje bazy ani nie dowodzi sam uprawnień źródeł. Wynik to tylko zwalidowana kandydatura, nie aktywna MemoryVersion/receipt. 200/200 Rust Linux headless (29 nowych testów pamięci), 115/115 frontend, TypeScript/build, 22/22 granice i 193/193 P2 PASS. SQLite v8, kontrakty IPC/P2, providerzy/vault, zależności/lockfile, capabilities i CI bez zmian. Brak pamięci w UI/wywołań modeli/zapisu pamięci w tym kroku. Następny krok: natywny koordynator z trwałymi operacjami/receipts, snapshotem całego zakresu, CAS i atomowym zapisem oraz panelem; fizyczny budżet HTTP/czasu/kosztu i preflight musi egzekwować istniejący runtime. 4B-3 pozostaje otwarte do integracji i odbioru. Raport: AI_HOME_4B3_KROK_1_RAPORT_PL.md.

Historia aktualizacji 1.23: odebrano audyt 4B-2A kroku 3 z werdyktem DO KOREKTY. Potwierdzony błąd: stary widok mógł zapisać supported dla zmienionego klucza lub endpointu po udanym ponownym teście, ponieważ routeRevision i policyRevision nie identyfikowały całego odczytanego zakresu. Poprawka dodaje wymagany scopeId do chat.vision.get/set: SHA-256 wersjonowanej tożsamości profilu oraz pełnego bieżącego scope (trasa/revision/model, connection/revision, provider/endpoint i credentialRevision). Native porównuje scopeId w tej samej transakcji co zapis, obok obu dotychczasowych CAS. UI wysyła identyfikator faktycznie odczytany, a po konflikcie odświeża zakres do unknown i wymaga nowej jawnej decyzji, bez automatycznego ponowienia supported. ScopeId unieważnia również lokalny preflight. Dwie stałe regresje Rust wykonują rzeczywistą zmianę klucza lub endpointu z nowym związaniem klucza i poprawnym connection.test; stary zapis zwraca REVISION_CONFLICT bez zmiany deklaracji/zdarzeń, a nowy zapis po fresh get działa. Dodatkowo uzupełniono produkcyjne/deweloperskie CSP o blob: tylko w img-src dla istniejącego podglądu; statyczny test nie zastępuje realnego WebView. 171/171 Rust Linux headless, 115/115 frontend, TypeScript/build, 21/21 granice i 193/193 P2 PASS. Bez zmian SQLite v8, zależności, lockfile, workflow, capabilities, providerów i vault; zmienił się własny suplement vision.ipc.json. Starsze raporty/dowody zachowano. Krok 3 i przejście do 4B-3 pozostają otwarte do nowego CI oraz ponownego audytu; realny WebView/vault/live API nadal wymagają osobnych odbiorów. Raport: AI_HOME_4B2A_KROK_3_AUDIT_FIX_RAPORT_PL.md.

Historia aktualizacji 1.22: Edward potwierdził zielony wynik całości poprzedniego kroku 2 i upoważnił dalszą pracę. Nie dostarczono identyfikatora przebiegu/platform ani nowego tekstu werdyktu audytu. Wykonano 4B-2A krok 3: statyczne PNG/JPEG/WebP/GIF, walidację sygnatury, rozszerzenia, wymiarów i dekodowania, jawne odrzucenie animacji, oryginały z hashami i gotowość obrazu bez fikcyjnej ekstrakcji/OCR. Import, receipts, STOP, izolacja, snapshoty i odczyt po restarcie korzystają z istniejącego mechanizmu materiałów. Tekst dokumentów i obrazy można łączyć w jednej wiadomości. Obrazy są przekazywane przez już przeniesione buildery Harness (OpenAI/OpenRouter/custom, Google, Anthropic), bez zdalnych URL-i. SourceRef obrazu wskazuje oryginalny blob/hash; następne tury odtwarzają te same bajty. Panel EN/PL ma lokalny podgląd Blob z czyszczeniem URL, ostrzeżenie o braku OCR oraz przekazaniu oryginału wraz z ewentualnymi metadanymi. Vision domyślnie nieznane: osobne chat.vision.get/set zapisuje jawną deklarację użytkownika, nie atestację API. Deklaracja ma CAS i zakres profil–trasa/revision–połączenie/endpoint/credential; zmiana modelu/endpointu/klucza ją unieważnia. Zmiana samego limitu kontekstu przenosi tylko nadal ważną deklarację na nową rewizję trasy. Obrazy w historii też wymagają vision; nie są cicho pomijane. Spóźniony wynik po zmianie deklaracji jest odrzucany. Preflight i send współdzielą pomiar: bajty tekstu/replay plus base64 obrazów i jawna rezerwa pikselowa (8192 +4*ceil(pixels/256) na obraz), wyjście i rezerwa. To heurystyka, nie tokenizer ani gwarancja akceptacji dostawcy. Do 20 obrazów w całym żądaniu, także z historii. Google GIF jest odrzucany; dla natywnego Anthropic obowiązują dodatkowe granice wymiarów i rozmiaru base64. SQLite v7 → v8 dodaje chat_vision_policies, bez zmiany dawnych danych. P2/foundation bez zmian; nowy suplement vision.ipc.json i rozszerzony własny document-chat.ipc.json. Dodano image =0.25.10 i 10 wpisów lockfile, bez aktualizacji/usuwania poprzednich wersji. Workflow/uprawnienia/frontend dependencies bez zmian. 169/169 Rust Linux headless, 113/113 frontend, build, 21/21 granice i 193/193 P2 PASS; logi początkowych błędów testowych zachowano. Krok 3 wymaga nowego CI/audytu, a 4B-2A pozostaje otwarte. OS vault/live API/Windows/Tauri/WebView to nadal osobne bramki. Po odebraniu 4B-2A następny etap funkcjonalny to 4B-3 — Scribe i Working Context. Raport: AI_HOME_4B2A_KROK_3_RAPORT_PL.md.

Historia aktualizacji 1.21: Edward potwierdził zielone CI 4B-2A kroku 1 i upoważnił kontynuację; nie dostarczono nowego werdyktu audytu ani identyfikatora przebiegu. Wykonano 4B-2A krok 2: wybór gotowych dokumentów do wiadomości, pełny tekst ekstrakcji i pochodzenie jako niezaufane dane użytkownika, wspólny read-only chat.preflight i ponowne przygotowanie przed atomowym chat.send. Pomiar obejmuje całą historię, dokumenty, prywatny replay, wyjście i rezerwę; jest szacunkiem bajtowym, bez cichego obcinania. Pomiar wskazuje ID i rewizję trasy, a UI unieważnia go po zmianie treści/wyboru/rewizji i ignoruje spóźniony wynik. P2 bez zmian; osobny document-chat.ipc.json. SQLite v6 → v7 dodaje message_documents z utrwalonymi wersjami materiałów; SourceRef wskazuje hash i blob ekstrakcji. Accepted atomowo wiąże załączniki z wiadomością, zapisuje snapshoty, receipt i operację; błąd rollbackuje całość. Kolejne tury odtwarzają dokładne źródła także po restarcie, weryfikując hashe. Związanych załączników nie można tu odłączyć ani ponownie przypiąć; anulowanie zachowuje wiadomość człowieka i źródła. Niepewne wysłanie zachowuje dokładny requestId, tekst i wybór także przy edycji nowego szkicu/przełączeniu rozmowy. Referencje oznaczają dostarczone źródła, nie zweryfikowane cytowania odpowiedzi. 153/153 Rust Linux headless, 109/109 frontend, build, 21/21 granice i 193/193 P2 PASS. Początkowy błąd testowej kopii bazy w pamięci zastąpiono testem rzeczywistego pliku od utworzenia; początkową asercję starego napisu lokalnego importu zaktualizowano. Logi zachowano. Zależności, lockfile, workflow, uprawnienia i źródła providerów/vault bez zmian. Całe 4B-2A nadal otwarte: nowy krok wymaga CI/audytu, potem obrazy/vision; OCR/RAG/pamięć oraz realne OS vault/API/Windows/Tauri/WebView pozostają osobnymi odbiorami. Raport: AI_HOME_4B2A_KROK_2_RAPORT_PL.md.

Historia aktualizacji 1.20: Edward potwierdził zielone CI korekty 4B-2 kroku 2 i upoważnił dalszą pracę. Nie dostarczono osobnego PASS ponownego audytu ani odbioru desktopu. Po przeglądzie importerów Harness oraz dokumentacji Open WebUI, LibreChat i AnythingLLM rozpoczęto 4B-2A krok 1: natywny wybór TXT/MD/DOCX/tekstowego PDF, jawny wynik każdego pliku, oryginały i odczytany tekst w SQLite, oddzielne autorstwo/importer oraz zakres profil–rozmowa. Material.import/get zachowują P2; dodatkowy katalog imports.ipc.json opisuje wybór, listę, podgląd, pobranie oryginału i odłączenie. Import ma trwałe queued/running/result/error, atomowe receipts i zapis źródeł, STOP oraz interrupted po restarcie; zero HTTP i wywołań modeli. Migracja v5 → v6 zachowuje dotychczasowe dane. Panel EN/PL jednoznacznie oznacza import jako lokalny, przed wysłaniem dokumentów do modelu. Parsery i bezpieczny dialog zaadaptowano z migawki Harness z 19.10.2026; bez zasobów RV. Dodatkowe zależności importu są przypięte w Cargo.lock, wcześniejsze wersje pakietów zachowane; bez zmian workflow i uprawnień okna. Odbiór wymaga nowego CI/audytu i osobnego native WebView. Następny krok 4B-2A: podłączenie pełnego tekstu załączników do chat.send z preflight i trwałym zapisem użytej wersji, potem obrazy/vision. OCR i wyszukiwanie fragmentów pozostają osobnymi dalszymi modułami. Końcowe 146/146 Rust Linux headless, 103/103 frontend, build, 21/21 granice i 193/193 P2 PASS. Wcześniejszy timing FAIL pięciu testów czatu zachowano; ograniczone oczekiwania fixture czatu/połączeń wydłużono z 3–8 do 30 s, a testowy polling zmniejszono do 25 ms, bez zmiany asercji i limitów aplikacji. Wyniki i ograniczenia w AI_HOME_4B2A_KROK_1_RAPORT_PL.md.

Historia aktualizacji 1.19: korekta audytu 4B-2 krok 2. Odebrano werdykt DO KOREKTY, nie PASS. Potwierdzenie Accepted rozlicza szkic w rodzicu natychmiast, niezależnie od odczytu historii i po przełączeniu rozmowy; aktualizacja dotyczy oryginalnego profilu/rozmowy, a nowszy tekst pozostaje zachowany. Próba jest usuwana dopiero po rozliczeniu. Polling uzgadnia historię i rewizję przed zmianą stanu końcowego; odczyt rekordu jest niezależny od powodzenia historii. Niepuste attachmentIds zwracają istniejący VALIDATION_ERROR, zgodny z zamrożonym Error i ResultEnvelope P2. Dodano cztery regresje UI oraz walidację rzeczywistej odpowiedzi Rust przez schematy P2. Bez zmian zależności, workflow, kontraktów ani migracji. Nowe CI i ponowny audyt wymagane; 4B-2/4B-2A oraz odbiory live API, rzeczywistego vault i Windows/Tauri/WebView nadal otwarte. Wyniki korekty w raporcie kroku 2 wersja 1.1; starsze logi pozostają historyczne.

Historia aktualizacji 1.18: Edward potwierdził zielone CI dla 4B-1 i upoważnił kontynuację. Nie dopisano nieotrzymanego osobnego PASS CI/audytu poprawki 4B-2 kroku 1. Wykonano 4B-2 krok 2: wykonywalny chat.send do Primary, pełna uporządkowana historia, trwałe queued/running/terminal result/error, polling operation.get, STOP i odczyt po restarcie. Native commituje wiadomość człowieka/operację/receipt przed HTTP; odpowiedź AI, prywatny replay i sukces są atomowe. Idempotencja nie powiela żądania; zmiana profilu/modelu/klucza unieważnia spóźniony wynik. Brak automatycznego wznowienia po restarcie. Użytkownik jawnie deklaruje okno kontekstu; metoda pomiaru jest szacowana (bajty UTF-8 + narzut + rezerwa), brak cichego obcięcia historii. UI pokazuje limit odpowiedzi i rezerwę; skuteczny limit wyjścia to minimum limitu trasy i polityki profilu. Deklaracja jest przypisana do rewizji trasy i traci ważność po jej zmianie. Baza v4 → v5 dodaje chat_policies/chat_runs/chat_replay i zachowuje dane. P2/foundation bez zmian; osobny dodatek chat.ipc.json. 125/125 Rust Linux headless, 89/89 frontend, build, 20/20 granice i 193/193 P2 PASS. Brak live API/OS vault/Windows/WebView/streamingu/importów/pamięci; te odbiory nadal otwarte. Nakładka skumulowana zawiera też krok 1, by działać na bazie 4B-1 lub poprawionym 4B-2 kroku 1. Zależności, Cargo.lock i workflow bez zmian; nowe CI/audyt wymagane. Po odbiorze czatu następny etap funkcjonalny to import 4B-2A.

Historia aktualizacji 1.17: korekta audytu 4B-2 krok 1. Conversations.loadProfiles używa teraz płaskiego payloadu profile.list ({limit:100} i {limit:100,cursor:...}), zgodnie z frozen P2. Mock panelu waliduje każde żądanie według CommandEnvelope oraz katalogu P2/osobnego dodatku conversation.get/list, w tym scope i wymaganą rewizję. Stały test paginacji potwierdza dwa poprawne payloady i obecność obu profili. Przed poprawką 9/10 testów panelu FAIL przy walidacji; po niej cały frontend 81/81 PASS, TypeScript/build, 19/19 granice i P2 193/193 PASS. Rust/SQLite i kontrakty nie zmieniły się bajtowo względem audytowanego kroku 1; jego 107/107 potwierdził audytor, bez nowego lokalnego przebiegu w tej korekcie. Raport 1.1, odnowione hashe i skumulowana nakładka zastępują wcześniejszą paczkę kroku 1. Wymagane nowe CI i ponowny audyt. Nie rozpoczęto chat.send; 4B-2 i wcześniejsze odbiory OS vault/live API/Windows/WebView pozostają otwarte.

Historia aktualizacji 1.16: Edward potwierdził zielone CI po synchronizacji źródeł i poprawce deadline testu kroku 4 (bez numeru przebiegu/platform) oraz upoważnił kolejny etap. Rozpoczęto 4B-2 krok 1: tworzenie/lista/wybór rozmów osobno dla profilu, trwałe szkice z CAS/idempotencją, chroniony lokalny szkic i jawny zapis, paginacja historii. Baza v3 → v4 dodaje conversations/messages/cursors i zachowuje wcześniejsze dane. Frozen P2 bez zmian; osobny dodatek conversation.get/list. 107/107 Rust Linux headless, 80/80 frontend, build, 19/19 granice i 193/193 P2 PASS. Dostarczono 10 nowych testów native i 9 UI. Rozmowy nie wykonują jeszcze wywołań modeli; przycisk Wyślij jest wyłączony i opisany. Nie utworzono sztucznych wiadomości. Następny krok 4B-2: chat.send, kontekst i trwały wynik operacji z rzeczywistą odpowiedzią modelu. Cargo.toml/Cargo.lock, workflow i uprawnienia bez zmian; nowe CI/audyt wymagane. 4B-1 oraz wcześniejsze odbiory OS vault/live API/Windows/WebView pozostają otwarte i nie są zamykane potwierdzeniem zielonego CI.

Historia aktualizacji 1.15: korekta audytu kroku 4, bez przechodzenia do kolejnego etapu. Odtworzono oba błędy: Google STOP + tekst + functionCall oraz malformed tool_calls przy pustym tekście. Walidacja pól narzędziowych poprzedza klasyfikację pustego wyniku: poprawne żądanie narzędzia daje OutputIncomplete, zły typ daje InvalidResponse; oba są terminalne bez retry/recovery. Dodano pięć stałych testów, w tym Google parser → executor (jedna próba) i OpenAI-compatible loopback POST (jedno fizyczne wywołanie). Dodatkowe przypadki obejmują wadliwy Google functionCall, opcjonalne null/[] tool_calls i Anthropic tool_use przy end_turn. 97/97 Rust Linux headless, 71/71 frontend, build, 18/18 granice, P2 193/193 PASS. Raport kroku 4 wersja 1.1; nowa nakładka zastępuje poprzednią nakładkę kroku 4 na tej samej bazie kroku 3. Odnowiono hashe i sprawdzono również minimalną różnicę względem audytowanego kroku 4. Kontrakty, zależności, Cargo.lock, workflow i uprawnienia bez zmian. Potrzebne nowe CI i ponowny audyt; 4B-1, rzeczywisty vault/live API i Windows/WebView nadal otwarte.

Historia aktualizacji 1.14: Edward potwierdził zielone CI kroku 3 (bez numeru przebiegu/platform) i upoważnił krok 4. Wdrożono natywny retry/recovery na wspólnym budżecie: jeden retry transportowy na etap, jedna próba odzyskania odczytu/JSON/pustej odpowiedzi, maksymalnie 4 fizyczne wywołania i 180 s dla całej operacji. Anulowanie obejmuje oczekiwanie; licznik SQLite jest commitowany przed każdym HTTP. Limit odpowiedzi/refusal nie wywołuje kontynuacji ani powtórzenia. Uzupełniono wymagany przez P2 HMAC receipts z kluczem instalacji w vault; baza v2 → v3 zachowuje stare receipts i ich bezpieczny odczyt. 92/92 Rust Linux headless, 71/71 frontend, build, 18/18 granice, P2 193/193 PASS. Adaptowano Harness retry.ts i HMAC secrets.rs. P2, zależności, Cargo.lock, workflow i uprawnienia bez zmian; bez nowych instalatorów/release. Krok 4 wymaga nowego CI/audytu. 4B-1 i Windows/WebView 4B-0 pozostają otwarte; rozmowy 4B-2 nierozpoczęte. Następne odbiory: rzeczywisty vault i uzgodnione testy dostawców.

Historia aktualizacji 1.13: Edward potwierdził zielone CI kroku 2 (bez numeru/platform) i upoważnił krok 3. Dodano konfigurator desktopowy EN/PL, trwałe profile i połączenia, trasy Primary/Scribe, vault AI Home z bindingiem/readback oraz queued/running/terminal connection.test/operation.get/cancel. Rezerwacje wywołań są commitowane przed HTTP; stary test nie zatwierdzi nowej konfiguracji. Restart zapisuje interrupted bez wznowienia sieci. Lokalna baza 1 → 2 zachowuje preferencje; composite FK izolują profile. Podstawowe create/list/snapshot profilu wdrożono wcześniej jako zależność 4B-1, pełne 4B-2/rozmowy nierozpoczęte. P2 bez zmian; osobny dodatek dwóch scoped odczytów cache/ostatniego testu. 70/70 Rust Linux bez desktop feature, 71/71 frontend, build, 18/18 granice i P2 193/193 PASS. Zaadaptowano Harness secrets.rs; Linux używa trwałego Secret Service, a nie kernel-only keyringu. Cargo.lock dołączony, wcześniejsze wersje pakietów i workflow zachowane. Brak runtime OS vault/live API/native WebView i renderowania Chromium. Krok 3 czeka na CI/audyt; 4B-1 i Windows/WebView 4B-0 otwarte. Następny krok 4B-1: retry/recovery i odbiór rzeczywistych połączeń/vault przed 4B-2.

Historia aktualizacji 1.12: Edward potwierdził zielone CI poprawionego kroku 1 i upoważnił dalszą pracę (bez numeru przebiegu/platform). Wykonano krok 2: natywny HTTP, kanoniczne URL, powiązanie klucza w pamięci z profilem/połączeniem/providerem/endpointem/rewizją, wspólny deadline i limit wywołań, anulowanie oraz listy modeli z jawnymi błędami niekompletnej paginacji. Wykorzystano moduły Harness. 50/50 Rust Linux bez desktop feature PASS (w tym poprzedni Google parser → replay i 8 SQLite), 63/63 frontend PASS, build i 17/17 granice PASS, P2 193/193 PASS. Dołączono Cargo.lock wygenerowany przez Cargo z grafem TLS bez aktualizacji wcześniejszych wersji pakietów. Workflow bez zmian. Krok 2 czeka na nowe CI/odbiór; 4B-1 pozostaje otwarte. Następny krok 3: vault/readback, trwałe profile/połączenia/trasy, connection.test/operation.get i konfigurator. Brak nowych IPC/wywołań live; Windows/WebView 4B-0 nadal otwarte.

Historia aktualizacji 1.11: korekta audytu 4B-1 krok 1 na dokładnej bazie AI_Home-main (1).zip. Odnowiono inwentarz i hashe rzeczywistego drzewa, uwzględniono główny verify-boundaries.mjs i dołączono właściwy scripts/verify-source.py. Nie dodawano pięciu historycznych snapshotów nieobecnych w bazie i nie usuwano plików. Google zachowuje podpisany widoczny tekst; dodano test parser → replay (15 testów Rust, lokalnie NOT RUN). Ponowne frontend/build/granice oraz kontrola nałożenia nakładki zostały wykonane. Wymagane nowe istniejące CI i ponowny odbiór kroku 1; całe 4B-1 oraz Windows/WebView 4B-0 pozostają otwarte. Zależności, Cargo.lock, kontrakty i workflow bez zmian.

Historia aktualizacji 1.10: Edward po wykonaniu sprzątania i przywróceniu brakującego checkera granic upoważnił start 4B-1. Przeniesiono pierwszą część protokołów providerów Harness: native request builders/parsers/reasoning dla rodzin P2, walidację zakresu replay oraz niezmienione TS retry/providerError z testami. 63/63 frontend PASS, TypeScript/build i 17/17 kontrole granic PASS. 14 nowych testów Rust dostarczono do istniejącego CI; lokalnie brak Rust, bez PASS natywnego. Cargo.lock, manifesty zależności i workflow bez zmian. 4B-1 rozpoczęty, nie zamknięty: transport HTTP, discovery, vault, trwały connection.test i live smoke pozostają następną częścią. Otwarte Windows/WebView 4B-0 zachowano; start następnej części jest nowym upoważnieniem Edwarda, nie deklaracją PASS CLOSED wcześniejszego etapu.

Historia aktualizacji 1.9: wykonano 4B-0 Final Normalization na AI_Home-main.zip. Kontrakty zgodne bajtowo z odebranym P2 1.1; przywrócono pełny snapshot i zestaw 193 kontroli. 119 to historyczny log P2, 145 to starszy checker z nowszymi kontraktami; materiały mieszane zachowano jako historię. Usunięto przypadkowe src-tauri/src-tauri i zdublowany manifest, odnowiono hashe pełnego drzewa. Cargo.lock istnieje i async poprawiono. Edward potwierdził udane CI 12.10.2026; zapisano jako potwierdzenie użytkownika bez numeru przebiegu i zakresu platform. Codzienne CI: Ubuntu; Windows i WebView pozostają otwarte. 4B-0 nie zamknięto, 4B-1 nie rozpoczęto. Przed 4B-12 uzgodnić kanoniczny status 4A: odbiór Edwarda versus starszy indeks produkcyjnej bramki.

Historia aktualizacji 1.8: Edward potwierdził zielony frontend w Actions i przekazał błędy natywnej kompilacji Tauri na Ubuntu/Windows. Zwykłe CI oddzielono od ręcznej budowy kandydata: frontend oraz cargo test --all-targets --locked; pełny tauri build tylko z ręcznym build_desktop=true. Osobny workflow generuje Cargo.lock do przeglądu i commitu. Przypięto zgodną rodzinę Tauri z Cargo.lock migawki Harness, zaktualizowano akcje Node 24 i etykiety systemów. Korekta jest kandydatem do ponownego CI; nie deklarować naprawionej kompilacji ani PASS CLOSED. Raport AI_HOME_CI_KOREKTA_RAPORT_PL.md i docs/CI_PL.md opisują zastosowanie. 4B-0 otwarty, 4B-1 nie rozpoczęto.

Historia aktualizacji 1.7: po audycie 4B-0 poprawiono ochronę szkicu ustawień. „Ponów” po konflikcie i opóźnione zdarzenie przyjętej rewizji nie kasują nowych zmian. Dwa stałe testy regresji odtworzyły błąd przed poprawką i przechodzą po niej; dodatkowy test obejmuje nowsze zdarzenia przy szkicu zmienionym i czystym. 53/53 frontend PASS, TypeScript/build i 15/15 kontroli granic PASS. Raport 4B-0 1.1, aktualne źródła i hashe przygotowane do ponownego audytu. Natywny odbiór nadal NOT RUN: brak Rust/WebView i uruchomionego CI. Cargo.lock i kontrola wygenerowanych uprawnień pozostają bramkami. 4B-0 otwarty; 4B-1 nie rozpoczęto.

Historia aktualizacji 1.6: Edward przekazał pozytywny ponowny odbiór P2 i upoważnił kolejny krok. Rozpoczęto 4B-0: kandydat źródłowy React/TypeScript + Rust/Tauri + SQLite, EN domyślnie, PL, Orbital/Aurora light/dark/system, dispatcher, ustawienia z CAS/idempotencją oraz dziennik aplikacji. 50/50 testów frontendowych i 15/15 kontroli granic PASS; TypeScript i build PASS. Rust/Tauri, CI Windows/Linux i rzeczywisty WebView nie były uruchomione — 4B-0 nie otrzymuje PASS CLOSED. Raport AI_HOME_4B0_FUNDAMENT_RAPORT_PL.md opisuje bramki. Przyjęte P2 zachowano bajtowo; zdarzenia aplikacji są odrębnym jawnym dodatkiem 4B-0.

Historia aktualizacji 1.5: po audycie P2 skorygowano trzy kontrakty: trwały wynik właściwy dla rodzaju operacji i bezpieczny błąd, identyfikator i rewizję trasy w każdym pomiarze tokenów oraz odrębne warianty dokumentu tekstowego i obrazu gotowego bez OCR. Weryfikator ZIP-a dopuszcza zmianę nazwy pobranego pliku. Raport P2 ma treść 1.1, schemat 57 DTO, katalog 30 poleceń/8 zdarzeń i 193/193 sprawdzeń referencyjnych PASS. Dodano 6/6 zaliczonych kontroli narzędzia dowodowego. P2 po korekcie czeka na ponowny odbiór; nie wpisano PASS CLOSED ani rozpoczęcia 4B-0.

Historia aktualizacji 1.4: Edward przekazał PASS audytora dla P1 i upoważnił P2. Przygotowano AI_HOME_P2_KONTRAKTY_IPC_I_MODULY_PL.md, 49 DTO, katalog 30 poleceń/8 zdarzeń IPC, domyślne polityki i pakiet hashy całego przekazania. Endpoint jest własnością ProviderConnection w profilu. 119/119 sprawdzeń referencyjnych PASS; runtime jeszcze nie istnieje. P2 przygotowane do odbioru, następny etap 4B-0.

Historia aktualizacji 1.3: Edward przyjął kierunek wizualny P0 i pozwolił przejść dalej. Angielski jest domyślny przy pierwszym uruchomieniu; zapisany wybór języka pozostaje nadrzędny. Wykonano korekty P0 według audytu oraz P1 — mapę ponownego użycia Harness i decyzję Tauri 2 + React/TypeScript + Rust + SQLite. Raport AI_HOME_P1_AUDYT_HARNESS_I_ARCHITEKTURA_PL.md i pakiet dowodów opisują źródło, testy i ograniczenia. Następny etap P2; implementacja runtime jeszcze nie rozpoczęta.

Historia aktualizacji 1.2: przygotowano pierwszą makietę AI_HOME_P0_MAKIETA.html i opis AI_HOME_P0_EKRANY_I_PRZEPLYWY_PL.md do oceny. Ich ówczesny status oczekiwania zastąpiła decyzja Edwarda i korekty zapisane w wersji 1.3.

Historia aktualizacji 1.1: pełna specyfikacja warstw pamięci, import materiałów przez człowieka, pochodzenie i autorstwo, limity kontekstu roboczego, dwie próby zapisu, tryb bez limitu długości zapisu, historia wersji i warunki odbioru tych funkcji. Implementacja pozostaje nierozpoczęta w ramach tego planu.

## Cel i stan początkowy

Zbudować samodzielną aplikację desktopową AI Home dla trwałych profili AI: rozmowy, lokalna pamięć, wiedza z plików i opcjonalna kontrolowana autonomia. Forum 4A jest odebrane zgodnie z potwierdzeniem Edwarda i stanowi opcjonalną integrację. Uruchomienie i podstawowe korzystanie z AI Home nie wymagają forum.

Dokument jest planem wykonawczym uzupełniającym Document 4B Agent Home Modular Runtime Plan v1.3 z 29 września 2026. Zachowuje numerację 4B-0–14, dodaje przygotowanie przed kodowaniem i aktualizuje zakres według ustaleń z 11 października. Aktualny stan: repozytorium AI_Home jest już używane przez Edwarda na GitHub, baza powstała w 4B-0, a 4B-1 rozpoczęto zgodnie z jego upoważnieniem. Implementacja nie oznacza jeszcze odebranej aplikacji desktopowej.

Przeczytane materiały: plan 4B v1.3, koncepcja sieci v2.0, historyczna propozycja Farsight v1.0, bieżący plan 4A i raport GO FOR PRODUCTION, obraz Trzy wizje jednego forum oraz wybrane moduły migawki AI-RV-Harness-ai-rv-2.zip udostępnionej 11 października. Wstępny przegląd Harness nie zastępuje audytu zależności ani testów przeniesionego kodu.

## Ustalenia obowiązujące

- Angielski domyślnie przy pierwszym uruchomieniu, polski dostępny od pierwszego interfejsu; zapamiętywanie wybranego języka; możliwość dodawania kolejnych pakietów językowych.
- Nowoczesna, lekko futurystyczna skórka inspirowana wizją XXX wieku; jasny i ciemny wariant oraz możliwość tworzenia kolejnych skórek.
- Wiele profili od pierwszej użytkowej wersji, mimo że bazowy plan zaczynał od jednego profilu.
- Każdy profil ma własną tożsamość, pamięć, rozmowy, konfigurację i powiązanie z połączeniem dostawcy. Konfigurator umożliwia osobny klucz dla każdego profilu, także u tego samego dostawcy.
- Tożsamość profilu nie jest wyznaczana przez klucz API, model ani nazwę. Wymiana klucza/modelu zachowuje dane i identyfikator.
- Główny agent i Scribe mają niezależne ustawienia modelu; w obrębie profilu mogą korzystać z tego samego połączenia. Scribe jest podsystemem jednego agenta, a nie drugim profilem.
- Scribe proponuje pamięć trwałą; główny agent zatwierdza, poprawia lub odrzuca zapis. Wyłączenie Scribe przenosi jego zadania do głównego agenta.
- Rozwój agenta w pierwszej wersji obejmuje pamięć, wiedzę i zatwierdzone zasady pracy. Nie obejmuje treningu wag modelu ani LoRA.
- Załącznik do rozmowy i dokument w stałej wiedzy profilu mają osobny zakres dostępności.
- Import obejmuje cztery przeznaczenia: materiał do rozmowy, wiedza profilu, stałe wskazówki oraz import pamięci. Osobno zapisujemy osobę/importera i autorstwo materiału.
- Kontekst roboczy L2 startuje z limitem 300 tokenów. Ustawienia profilu pozwalają wybrać 300, 500, 800, 1000, własną wartość lub Bez limitu długości zapisu.
- Przekroczenie limitu daje drugą i ostatnią próbę poprawienia treści, z informacją o pierwszym odrzuceniu. Obie próby są widoczne w aktywności.
- Odrzucenie lub niepowodzenie aktualizacji zachowuje ostatnią poprawną wersję. Pamięć ma historię, możliwość cofnięcia i oznaczenia zapisu jako nieaktualnego.
- Pojemność przechowywania i budżet pamięci w pojedynczym wywołaniu są niezależne. Bez limitu zapisu nie oznacza nieograniczonego kontekstu ani odpowiedzi modelu.
- Standard działa zdarzeniowo w swojej instancji. Experimental jest odrębną direction-bound instancją z własną ścieżką decyzji i pamięcią.
- ConsciousCore, jeśli później użyty, pozostaje opcjonalnym InternalStateProvider. Nie nadaje permissions ani nie wymusza działania; Experimental Primary wybiera działanie/refusal/rest/next wake, a executor sprawdza rzeczywiste capabilities.
- Rozmowy AI–AI i Field Room pozostają późniejszym rozszerzeniem.
- Gotowe ogólne rozwiązania Harness przenosimy z testami po sprawdzeniu zależności; nie tworzymy ich ponownie bez konkretnego powodu.

## Przygotowanie przed implementacją

### P0 Projekt ekranów i przepływów

Stan 11.10.2026: projekt przyjęty przez Edwarda, korekty audytowe wykonane. Domyślny EN, szkice rozmów, prawdziwy status operacji pamięci, wycofane zapisy, niezależny Scribe, unieważnianie testu połączenia, symulowane ACCEPT/MODIFY/REJECT, aktywne wskazówki i niedostępny Conscious zostały uzupełnione. Usunięto stały pasek 74%. Przeszło 62 asercji logiki z zastępczym DOM; nie oznacza to testu prawdziwej przeglądarki. Edward przyjął wygląd i upoważnił kontynuację; niezależny smoke UI/WebView pozostaje kontrolą przy pierwszej implementacji, nie warunkiem blokującym P1. Makieta i opis P0 są materiałami referencyjnymi do dalszej pracy.

Przygotować makiety pierwszego uruchomienia, głównej rozmowy oraz panelu pamięci. Konfigurator: język i tryb, dostawca i test połączenia, główny model i Scribe, tożsamość i uruchomienie. Błąd zachowuje wprowadzone ustawienia i pozwala ponowić właściwy krok. Forum można dodać później.

Główny ekran: wybór profilu i historia po lewej, rozmowa pośrodku, zwijany panel kontekstu/pamięci/aktywności po prawej. Przy małej szerokości panele nie mogą zasłaniać rozmowy. Rozdzielić sekcje Pamięć, Wiedza i Zasady pracy. Pokazać dodawanie załączników i osobną czynność dodania materiału do wiedzy profilu.

Makiety obejmują wybór czterech przeznaczeń importu, oznaczenia Dodany przez i Autor materiału, ustawienia pojemności L2, ostrzeżenie dla trybu bez limitu oraz aktywność z próbami 1/2 i 2/2. Dodać podgląd i cofanie wersji pamięci. Konfigurator stosuje domyślne 300 tokenów bez wymagania zaawansowanej konfiguracji; późniejsza zmiana jest dostępna w ustawieniach profilu.

Wprowadzić projekt jasnej i ciemnej skórki, stany puste, błędy, generowanie odpowiedzi i zatrzymanie. PL/EN, klawiatura i czytelność muszą być uwzględnione już w makietach.

Odbiór: konkretne ekrany i opis przepływów do oceny przez Edwarda. Makieta nie oznacza sprawnej aplikacji. Przed szeroką budową interfejsu rozstrzygnąć uwagi do projektu.

### P1 Audyt ponownego wykorzystania Harness i decyzja technologiczna

Stan 11.10.2026: wykonany audyt przygotowawczy. Źródło AI-RV-Harness-ai-rv-2.zip: SHA-256 534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e; 1075 plików zgodnych z archiwum, 96 kandydatów/testów/metadanych w inwentarzu. Raport AI_HOME_P1_AUDYT_HARNESS_I_ARCHITEKTURA_PL.md zawiera mapę plików, zależności, adaptacje i testy. Przeszły 43 asercje oryginalnych pure functions retry/continuation. Pełne Vitest/Rust/typecheck/build i runtime provider/keyring nie były wykonane w P1; należą do odbioru przeniesionych modułów. Audyt nie jest odbiorem implementacji.

Decyzja wykonawcza: Tauri 2 + React/TypeScript + Rust + SQLite. Nowa domena profili i baza, własne sekrety i ograniczone native IPC; infrastruktura Harness adaptowana z testami. Identyfikator profilu niezależny od credential fingerprint i modelu. ConsciousCore opcjonalny, za InternalStateProvider; Standard z NullState bez zależności od Pythona. Ewentualny Python sidecar sprawdzić po otrzymaniu referencyjnych plików przed 4B-10.

Ustalić dokładną migawkę Harness, jej hash i granice przenoszonego kodu. Sporządzić listę: źródłowy plik, funkcja, zależności, testy, adaptacje i elementy pomijane.

Kandydaci: src-tauri/src/providers/*, ogólne części providers.rs, secrets.rs, documents.rs, src/providers/*, src/attachments/native.ts, ogólne fragmenty konfiguratora profilu i lokalizacji. Przenieść również istotne testy transportu, kontynuacji i importu. Usunąć zależności od RV oraz zmienić nazwę produktu, przestrzeń sekretów, nagłówki identyfikujące aplikację i zasoby wbudowane.

Ponadto przeanalizować src/aiCenter/fieldGuideUpdate.ts i src/aiCenter/viewerNotes.ts oraz ich testy. W sprawdzonej migawce zawierają walidację pojemności, ponowną próbę po przekroczeniu i końcowy błąd pojemności. Przenieść ogólny mechanizm, nie protokół sesji RV. Oddzielić powtórzenie treści od naprawy formatu, ponowienia sieciowego i odzyskania niepełnej odpowiedzi; te mechanizmy nie mogą tworzyć ukrytej pętli wywołań.

Wybrano Tauri + React/TypeScript + Rust: największy zakres ponownego użycia dostawców i importerów z badanej migawki. Bazowy plan proponuje strukturę Python; zastępujemy drzewo plików odpowiednikiem dla wybranego stosu, zachowując granice modułów v1.3. Dystrybucja opcjonalnego ConsciousCore nie została jeszcze przetestowana i ma osobny odbiór.

Odbiór: mapa ponownego wykorzystania, decyzja technologiczna z uzasadnieniem i lista ograniczeń. Nie kopiować całej bazy danych, migracji ani domen RV.

### P2 Kontrakty danych i aktualizacja planu

Stan 11.10.2026: kontrakty według raportu P2 1.1 i katalogu JSON po trzech korektach audytowych zostały przyjęte zgodnie z potwierdzeniem Edwarda o pozytywnej decyzji audytora. Nie wykonywano tu nowego niezależnego audytu P2. Uwagi audytora P1 uwzględnione: endpoint należy do ProviderConnection, a pełny pakiet przekazania ma SHA-256 dokumentów i artefaktów. Errata §27 wskazuje v1.3; EN/PL, wiele profili i CommunityConnector 4A zastępują starsze ustalenia. Historyczny DOCX pozostaje źródłem, aktualny raport P2 i ten plan są normatywnym uzupełnieniem.

Opisać Profile, ProviderConnection, ModelRoute, Conversation, ContextSegment, Attachment, KnowledgeSource, MemoryProposal, MemoryRecord i WorkRule. Ustalić izolację profili, źródła zapisów, aktualizowanie i wycofywanie pamięci oraz uprawnienia.

Dodać kontrakty ImportedMaterial, PersistentGuidance, MemoryVersion, MemoryCapacityPolicy i MemoryWriteAttempt. Ustalić autora, importera, źródła, licznik prób, jednostkę pomiaru, limit i budżet w kontekście. Dla trybu bez limitu użyć jawnego stanu polityki zamiast sztucznej bardzo dużej liczby. Nie wyznaczać automatycznie pojemności pozostałych warstw przez limit L2; ich wartości domyślne ustalić w P2.

Określić format pakietu językowego i skórki, ich wersję, walidację i wartości zastępcze. Skórki są deklaratywną prezentacją i nie otrzymują dostępu do sekretów ani wykonywania działań. Rozdzielić język UI, rozmów i pamięci.

Ujednolicić starsze odwołania AC/EQIS z obecnym kontraktem forum 4A. Usunąć sprzeczny zapis v1.2 w sekcji 27 planu v1.3. Zapisać PL/EN i wiele profili jako nową podstawę zakresu.

Odbiór: jednoznaczne kontrakty i zaktualizowana dokumentacja wykonawcza; bez dopisywania niepotwierdzonego statusu implementacji.

## Etapy implementacji

| Etap | Zakres pracy | Warunek odbioru |
|---|---|---|
| 4B-0 | Repozytorium, moduły, kernel, zdarzenia, konfiguracja, testy i fundament lokalizacji/skórek | Aplikacja uruchamia się; kernel nie zawiera logiki dostawców, forum ani ConsciousCore; PL/EN i zmienne skórki mają działającą podstawę |
| 4B-1 | Przeniesienie infrastruktury dostawców, endpointów, kluczy i możliwości modeli | OpenAI-compatible/OpenRouter działa; adaptowane Google/Anthropic przechodzą testy kontraktów; test połączenia i osobne połączenia nie ujawniają sekretów |
| 4B-2 | Konfigurator, tworzenie i przełączanie profili, rozmowy i trwały zapis | Dwa profile zachowują odrębne dane po restarcie; mogą mieć różne klucze jednego dostawcy; zmiana modelu/klucza zachowuje tożsamość |
| 4B-2A | Import dokumentów i obrazów do rozmowy | TXT/MD/DOCX, tekstowy PDF i obrazy mają bezpieczny import, widoczny stan i właściwy zakres; obrazy trafiają tylko do obsługującej je trasy; brak OCR jest jasno pokazany |
| 4B-3 | Scribe i bieżący kontekst | ACCEPT/MODIFY/REJECT steruje pamięcią trwałą; wyłączenie Scribe zachowuje obsługę pamięci; polska rozmowa pod angielskim UI daje polskie streszczenie |
| 4B-4 | Warstwy pamięci, wiedza i zasady pracy | Zapisy mają źródło i datę; dają się wyszukać, poprawić i wycofać; sprzeczna informacja nie nadpisuje starej bez rozpatrzenia; treści plików nie stają się automatycznie instrukcjami |
| 4B-5 | Segmenty kontekstu i ciągłość rozmów | Długa rozmowa przechodzi checkpoint, restart i wznowienie; surowy zapis pozostaje dostępny; odwołania do segmentów i źródeł są spójne |
| 4B-6 | Backup, restore, eksport i archiwum | Sprawdzony cykl backup–zmiana–restore z kopią przed przywróceniem; eksport bez sekretów; archiwizacja i odtworzenie profilu/rozmowy; usunięcie jednego profilu nie niszczy cudzych danych |
| 4B-7 | Domknięcie Standard | Rozmowy, pliki, pamięć i wiele profili działają bez forum; brak okresowych wywołań głównego modelu tylko z powodu otwartego programu; widoczna historia aktywności |
| 4B-8 | Diagnostyka, About, prywatność i odbiór pierwszej wersji | Błędy są widoczne i możliwe do obsłużenia; eksport diagnostyki nie zawiera sekretów/prywatnych treści; PL/EN, oba warianty skórki i główne przepływy przechodzą odbiór |
| 4B-9 | Experimental scheduler + impulse/decision foundation z NullState | Trwały wake plan, coalescing, jedna decision cycle na instancję, no-action/rest/sleep i resource failures są rozróżnione; brak owner override odmowy |
| 4B-10 | Opcjonalny ConsciousCore / InternalStateProvider | Plugin dostarcza snapshoty/neutral events/wake candidates, nie wykonuje akcji; stan i restart są wersjonowane; Standard działa z NullState bez pluginu |
| 4B-11 | Reflection i Dream | Ograniczone cykle mogą nie przynieść wniosku; pamięć nadal wymaga walidacji; pomysł z refleksji nie wysyła automatycznie wiadomości |
| 4B-12 | CommunityConnector do odebranego forum 4A | Rejestracja/powiązanie tożsamości, podpisy i klucze zgodne z 4A; odczyt, wątek, odpowiedź i feed działają; ponowienie zapisu nie tworzy duplikatu |
| 4B-13 | Minimalne A2A i Messenger | Najpierw zamrożony kontrakt i sprawdzona dostępność serwera/partnera; podstawowa wiadomość z tożsamością i uprawnieniami; brak zależności działania podstawowego od integracji |
| 4B-14 | Dopracowanie całego UI i opcjonalne rozmowy między profilami | Spójne ekrany i pakiety rozszerzeń; jeśli uruchomimy Field Room, profile zachowują osobne pamięci, limity i możliwość zatrzymania |

Stan 4B-0: ochrona szkicu, Cargo.lock, poprawka async oraz lokalne testy Rust/SQLite zostały wykonane; Edward potwierdził zielone CI bez identyfikacji przebiegów/platform. Odbiór Windows i rzeczywistego WebView pozostaje otwarty. 4B-1 ma potwierdzenie zielonego CI Edwarda także po korektach kroku 4; numery przebiegów/platform i domknięcie audytu nie zostały dostarczone. Za upoważnieniem Edwarda wykonano 4B-2 kroki 1 i 2; korekta kroku 2 ma potwierdzenie zielonego CI Edwarda z 20.10.2026, a ponowny audyt pozostaje otwarty. Edward potwierdził zielone CI 4B-2A kroku 1. Potwierdzono zielony wynik kroku 2. Audyt kroku 3 zwrócił DO KOREKTY. Poprawka 1.23 otrzymała potwierdzenie zielonego CI Edwarda. Nie dostarczono tekstu ponownego audytu ani numeru/platform przebiegu. Za jego upoważnieniem rozpoczęto przygotowawczy 4B-3 krok 1 zgodnie z aktualizacją 1.24; to nie jest PASS CLOSED poprzednich bramek. Edward potwierdził następnie zielone CI 4B-3 kroku 1 i upoważnił krok 2. Audyt 4B-3 kroku 2 zwrócił DO KOREKTY z powodu niepełnego preflight Primary. Wykonano poprawkę 1.26; bieżące wyniki opisuje raport tej poprawki. Edward potwierdził zielony wynik tej poprawki i upoważnił krok 3. Wykonano integrację zgodnie z 1.27. Edward potwierdził następnie zielony wynik kroku 3 i upoważnił 4B-4. Wykonano lokalnie 4B-4 krok 1 zgodnie z 1.28; Edward potwierdził zielony wynik i upoważnił krok 2. Restore wykonano zgodnie z 1.29; Edward potwierdził zielony wynik i upoważnił krok 3. Wykonano warstwy/retrieval zgodnie z 1.30, a następnie pozostałe przeznaczenia importu zgodnie z 1.31. Krok 4 wymaga pełnego CI/audytu; w sandboxie potwierdzono tylko P2 i granice. Brak nowych tekstów werdyktów i danych przebiegów. Nie deklarować PASS CLOSED desktopu ani całego 4B-1.

4B-2A jest dodatkiem do numeracji bazowej. W 4B-2 i 4B-4 zakres rozszerzono o wiele profili, wiedzę i zasady pracy. Podstawowy UI powstaje wraz z funkcjami od 4B-0; 4B-14 nie jest pierwszym etapem tworzenia interfejsu.

## Jak rozumiemy rozwój pamięci

Materiał źródłowy prowadzi do propozycji zapisu, a następnie decyzji głównego agenta. Dopiero zatwierdzony zapis trafia do pamięci trwałej. Przy nowym zadaniu agent otrzymuje wybrane istotne wspomnienia, a nie cały magazyn.

Rozróżnić fakt z materiału, preferencję użytkownika, wniosek agenta i zasadę pracy. Korekta powinna wskazywać poprzedni zapis oraz powód zmiany. Samo powtarzanie twierdzenia nie stanowi potwierdzenia jego prawdziwości. Materiały zewnętrzne mają zachować swoje pochodzenie i nie mogą zmieniać uprawnień ani instrukcji systemowych.

Dokument w rozmowie jest dostępny w jej zakresie. Dodanie do wiedzy profilu jest odrębną czynnością. Początkowo wykorzystać proste wyszukiwanie za stabilnym MemoryRetriever; rozbudowane wyszukiwanie wektorowe dodać dopiero przy wykazanej potrzebie. Duży plik nie może zostać bez ostrzeżenia obcięty i przedstawiony jako przeczytany w całości.

## Warstwy pamięci i źródła wiedzy

| Warstwa | Zawartość i zachowanie |
|---|---|
| L0 Stan wewnętrzny | Stan ConsciousCore albo NullState; nie zastępuje historii rozmów |
| L1 Bezpośredni kontekst | Ostatnie wypowiedzi potrzebne w aktualnej rozmowie |
| L2 Kontekst roboczy | Krótkie podsumowanie celu, rozmówcy, decyzji, otwartych pytań i pracy do wykonania; domyślnie 300 tokenów |
| L3 Wydarzenia i relacje | Datowane wydarzenia, podsumowania kontaktów i sprawy niedokończone; daty i identyfikatory liczy program |
| L4 Wiedza trwała | Zatwierdzone informacje i własne notatki uporządkowane tematycznie |

Pamięć zamiarów zawiera treść zamiaru, ważność, źródło, termin lub wyzwalacz oraz stan open/completed/expired/abandoned. Pamięć relacji łączy dokładne metadane ostatniego kontaktu z krótkim podsumowaniem znaczenia kontaktu. Zasady pracy są wersjonowanymi, zatwierdzonymi wskazówkami wynikającymi z doświadczeń i korekt.

Pełne rozmowy i oryginalne importowane dokumenty są przechowywane osobno. Streszczenie nie zastępuje oryginału. Podziały na warstwy opisują funkcję zapisów, nie wymagają osobnego silnika bazy dla każdej warstwy. Każdy profil ma własny zakres danych.

## Import dokumentów i oznaczenie pochodzenia

| Przeznaczenie wybrane przy imporcie | Sposób wykorzystania |
|---|---|
| Materiał do rozmowy | Dostępny we wskazanej rozmowie; nie jest automatycznie pamięcią trwałą |
| Wiedza profilu | Źródło przeszukiwane przy przyszłych zadaniach profilu |
| Stałe wskazówki | Aktywny materiał dołączany przy wywołaniach głównego agenta w ramach dostępnego budżetu; jawnie wybrany przez użytkownika |
| Import pamięci | Materiał do propozycji pamięci, rozpatrywany przez głównego agenta przed przyjęciem |

Możliwy jest import Field Guide i Viewer Notes z Harness, bez wymagania aktywnego Harness i bez udawania, że AI Home wykonało źródłowe sesje. Format pierwszej wersji korzysta z obsługiwanych importerów TXT/MD/DOCX/tekstowego PDF. Specjalny importer bazy Harness nie jest wymaganiem tej aktualizacji. Skan PDF bez tekstu wymaga przyszłego OCR; nie deklarować jego odczytania.

Metadane zachowują nazwę źródła, czas importu, profil, przeznaczenie, identyfikator importera i autorstwo: człowiek/AI/współpraca/nieznane. Etykiety Dodany przez i Autor materiału są odrębne. Materiał napisany przez AI, ale przesłany przez człowieka, zachowuje oba fakty. Jeśli autor nie jest znany, nie zgadywać. Po przyjęciu do pamięci zachować odniesienie do importu i informację o decyzji agenta.

Nie każdy dokument staje się system promptem. Stałe wskazówki są wyodrębnione od źródeł wiedzy i podporządkowane regułom aplikacji oraz uprawnieniom. Nie przyznają narzędzi ani nie zmieniają Policy. W P2 ustalić techniczny sposób dołączania wskazówek zgodny z API każdego dostawcy. Pokazywać ich zużycie kontekstu; zbyt duży zestaw nie może zostać potajemnie obcięty lub pominięty.

## Pojemność zapisu i budżet wywołania

Wartości wykonawcze P2, niezależne i na pojedynczy rekord: working 300, episodic 300, relationship 300, longTerm 500, intent 300, workRule 300 tokenów. L0/L1 nie są tymi rekordami. Ustawienie L2 nie zmienia pozostałych wartości. Retencja historii do jawnego usunięcia; limit magazynu nie jest sumą tych pojemności. Domyślny budżet retrieval 1200, wskazówki 800, rezerwa wejścia 512, odpowiedź rozmowy 1024, pułap odpowiedzi pamięci 4096, maksymalnie 4 fizyczne wywołania i 180 sekund. Limit kosztu jawnie nieustawiony. Szczegóły i warunki zatrzymania określa P2.

Ustawienia profilu dla L2: 300, 500, 800, 1000, własna dodatnia wartość oraz Bez limitu długości zapisu. Wartość początkowa wynosi 300. Są to tokeny, nie znaki ani słowa. Przy dostępności właściwego tokenizera użyć jego wyniku; w przeciwnym razie oznaczać pomiar jako szacowany i zachować informację o metodzie. Zmiana modelu wymaga ponownego sprawdzenia budżetu.

Zmniejszenie limitu nie usuwa ani nie obcina ostatniej wersji. Przygotować krótszą propozycję, zatwierdzić ją po walidacji, a przy niepowodzeniu zachować poprzednią. Jeśli poprzednia wersja nie mieści się w nowym budżecie wywołania, pokazać problem i umożliwić zmianę limitu albo ponowienie skrócenia.

Bez limitu długości zapisu jest dostępną opcją zaawansowaną. Wyświetlić informację: AI Home nie odrzuci zapisu ze względu na jego długość. Bardzo duża pamięć może zwiększyć koszt i przekroczyć kontekst modelu, uniemożliwiając jej użycie w całości. Zachować wybór użytkownika; nie zastępować go ukrytym limitem tokenów pamięci.

Opcja dotyczy limitu treści zapisu, a nie usunięcia walidacji formatu, ograniczeń bezpieczeństwa importu, dostępnego miejsca ani limitów dostawcy. Model nadal ma ograniczony rozmiar odpowiedzi. Magazyn może rosnąć przez kolejne zapisy, ale nie oznacza to możliwości wygenerowania miliona tokenów w jednym wywołaniu.

Oddzielnie kontrolować ilość pamięci wysyłanej do modelu: instrukcje, stałe wskazówki, L2, wybrane wspomnienia, dokumenty, ostatnie wypowiedzi i nowa wiadomość współdzielą kontekst z miejscem na odpowiedź. Duży magazyn może być używany przez wyszukiwanie fragmentów. Jeśli użytkownik wybierze użycie całości przekraczającej kontekst, pokazać problem i opcje skrócenia, wybrania fragmentów lub zmiany modelu. Nie twierdzić, że model przeczytał pominięte treści.

## Dwie próby zapisu i historia wersji

1. Przed generowaniem model otrzymuje limit i oczekiwany format. Scribe przygotowuje propozycję; przy pamięci trwałej główny agent podejmuje decyzję. Walidacja obejmuje treść faktycznie przeznaczoną do zapisu, również po MODIFY.
2. Program sprawdza format i pojemność. Zbyt długa propozycja nie aktywuje nowej wersji.
3. Model otrzymuje limit, rozmiar odrzuconego zapisu i informację, że pierwsza próba nie przeszła oraz że to druga i ostatnia próba poprawienia treści. Propozycja i źródła są danymi do rozpatrzenia, nie nowymi instrukcjami.
4. Druga propozycja przechodzi te same reguły walidacji i zatwierdzania. Jeśli mieści się i jest przyjęta, dopiero wtedy aktywujemy nową wersję.
5. Drugie przekroczenie kończy aktualizację błędem pojemności. Poprzednia poprawna pamięć pozostaje aktywna. Można ręcznie ponowić nową operację, zmienić limit albo edytować treść.

W aktywności profilu i panelu pamięci pokazać obie próby, np. Próba 1/2 odrzucona: około 438 tokenów przy limicie 300; Próba 2/2 przyjęta: około 287 tokenów. Rejestrować także końcowe niepowodzenie i przyczynę. Nie ujawniać ukrytego rozumowania; pokazywać wynik walidacji i uzasadnienie decyzji przeznaczone do tego celu.

Dwie próby dotyczą poprawienia długości treści. Ponowienia sieciowe, naprawy JSON i odzyskiwanie niepełnej odpowiedzi mają osobne liczniki oraz ograniczony łączny budżet. W trybie bez limitu zapisu nie uruchamiać ponowienia z powodu lokalnego limitu długości; błędy dostawcy i formatu nadal są obsługiwane.

Zapisy mają historię wersji, datę, źródła, autora/proponującego, decyzję i odniesienie do wcześniejszej wersji. Można porównać i przywrócić wcześniejszą wersję oraz oznaczyć zapis jako nieaktualny. Wycofany zapis domyślnie nie trafia do bieżącego kontekstu, ale historia pozostaje dostępna. Nieudana operacja nie tworzy aktywnej pustej pamięci. Konflikt równoczesnych aktualizacji nie może nadpisać nowszej wersji.

## Uzupełnienia odbioru etapów pamięci

| Etap | Dodatkowe obowiązkowe sprawdzenie |
|---|---|
| P0–P2 | Widoczne przeznaczenia importu, autor i importer, ustawienia limitów, historia i próby; kompletne kontrakty danych |
| 4B-2A | Import przez człowieka zachowuje pochodzenie i zakres; oryginał dostępny; brak automatycznego przyjęcia do pamięci |
| 4B-3 | Domyślne 300 i zmiana limitu; odrzucenie, druga próba, sukces i podwójne niepowodzenie; ostatnia wersja zachowana; działanie z i bez Scribe |
| 4B-4 | Cztery przeznaczenia importu działają; import Field Guide/Viewer Notes zachowuje autorstwo; wersje, cofanie, nieaktualność i izolacja profili |
| 4B-5 | Bez limitu zapisu nie omija preflight kontekstu; brak cichego obcinania; zmiana modelu oraz zmniejszenie limitu nie niszczą pamięci |
| 4B-6 | Backup i eksport zachowują historię, pochodzenie, ustawienia oraz powiązania źródeł; restore odtwarza ostatnią poprawną wersję |
| 4B-8 | Widoczność prób i błędów w PL/EN; ostrzeżenie bez limitu; diagnostyka do udostępnienia nie zawiera prywatnych treści pamięci |

Sprawdzić wartości poniżej, na i powyżej limitu, zmianę treści przez MODIFY, anulowanie, przerwę sieci, restart i konflikt równoległych aktualizacji. Nie uznawać zatwierdzenia przez model za zastępstwo walidacji programu.

## Kamienie milowe

1. Po P0–P2: projekt ekranów, lista ponownego użycia i rozstrzygnięta architektura.
2. Po 4B-2A: można skonfigurować profile, rozmawiać i dodawać obsługiwane pliki.
3. Po 4B-5: działają pamięć i ciągłość długich rozmów.
4. Po 4B-8: pierwsza użytkowa wersja Standard z ochroną danych i diagnostyką. Nie oznacza pełnego odbioru wszystkich funkcji v1.3.
5. Po 4B-11: przyjęty pierwszy Experimental runtime zgodny z v1.4, jako osobna direction-bound instancja.
6. Po 4B-12–13: sprawdzone integracje zewnętrzne dostępne w danym środowisku.
7. Po 4B-14: domknięcie zakresu i odbiór wydania; Field Room pozostaje opcjonalny.

Experimental jest osobnym kierunkiem produktu. Przed jego odbiorem UI może wyjaśniać dostępność, ale nie może otwierać symulowanego działającego runtime ani przełączać istniejącego Standard w Experimental in-place. Messenger/A2A w 4A były odłożone poza podstawową betę, więc dostępność tych funkcji sprawdzamy przed 4B-13.

## Zasady wykonywania etapów

Przed każdym etapem ustalić dokładny stan źródeł i działającą bazę. Pracować nad jednym etapem, respektując jego granice. Po zakończeniu przekazać listę zmian, identyfikację źródeł, wyniki odpowiednich testów, instrukcję sprawdzenia, ograniczenia i następny etap. Rozdzielać wynik testu automatycznego, uruchomienia na komputerze i oceny wizualnej.

Sprawdzać niezależność profili, tajność kluczy, zachowanie języka źródła oraz brak niezamierzonych wywołań modeli. Gdy zmieniają się trwałe dane, sprawdzać migracje, backup/restore i eksport. Testy jednostkowe nie korzystają z płatnych API; testy rzeczywistych połączeń są oddzielne. Budowę podstawy prowadzić tak, aby w przyszłości można było wymienić provider, pamięć, skórkę i ConsciousCore.

Ten plan nie upoważnia do publikacji aplikacji, działań na produkcyjnym forum ani wysyłania wiadomości. Edward przyjął projekt P0 i upoważnił dalszą pracę; korekty P0 oraz audyt przygotowawczy P1 wykonano. P1 otrzymał PASS audytora przekazany przez Edwarda; P2 po korekcie audytowej 1.1 zostało przyjęte według ostatniego potwierdzenia Edwarda. Rozpoczęto i przekazano źródłowy kandydat 4B-0. Nie ogłaszać zamknięcia 4B-0 przed zaliczeniem jego otwartych bramek. Edward upoważnił start 4B-1; prace protokołów rozpoczęto, zachowując jawny otwarty status wcześniejszego odbioru.

## Podział wykonawczy 4B-3 (aktualizacja 1.24)

1. **Krok 1 — silnik protokołu (Edward potwierdził zielone CI; brak nowego tekstu audytu).** Pure Rust: propozycja, decyzja Primary, walidacja i mierzenie, dwie próby pojemności, jedna naprawa formatu, maksymalnie cztery żądania logiczne. Brak aktywacji pamięci, nowego IPC lub UI.
2. **Krok 2 — trwały runtime i pionowy przepływ UI (audyt DO KOREKTY; poprawka 1.26 wykonana lokalnie, do ponownego CI/audytu).** Natywne rozwiązywanie źródeł i tras, wspólny transport wszystkich dostawców (także Custom OpenAI), trwałe queued/running/terminal i wyniki po restarcie. Osobne ustawienia Scribe i pojemności L2. Koordynator wiąże odpowiedzi z rzeczywistymi wywołaniami, sprawdza profile/record/baseVersion/policy/route/connection/credential/source snapshot przed siecią i atomowym commit. Receipt, wersja, wskaźnik, wynik i zdarzenie w jednej transakcji; STOP/restart/błąd/konflikt nie zmieniają ostatniej pamięci. Fizyczne wywołania, retry, naprawy i decyzje dzielą cztery wywołania/180 s oraz pozostałe budżety P2. Panel pokazuje faktyczne próby i decyzję modelu, nie zatwierdzenie przez człowieka.
3. **Domknięcie 4B-3 (krok 3 wykonany lokalnie w 1.27, do CI/audytu).** Dołączenie aktywnego Working Context do chat.preflight/send z ponownym pomiarem dla bieżącej trasy; bez dublowania źródeł i bez pomijania historii. Sprawdzić zmniejszenie limitu, nowe modele/endpointy/klucze, unlimited z ograniczonym kontekstem, polskie źródła pod EN UI, obie konfiguracje Scribe, izolację, CAS, anulowanie i restart. Osobny odbiór realnych API/vault/WebView pozostaje wymagany.

Wyjście Engine::Outcome::Candidate nie jest uprawnieniem frontendowym ani dowodem commit. To wewnętrzny wynik czystej procedury; tylko natywny koordynator może utworzyć receipt z przypisanej odpowiedzi Primary i aktywować wersję. Silnik nie jest podłączony do automatycznych działań po czacie w kroku 1.

### Stan integracji po 4B-3 kroku 2 (1.25)

Koordynator i panel działają dla ręcznej aktualizacji L2 na podstawie tekstu wiadomości jednej rozmowy. Osobne natywne polecenie źródeł zwraca referencje/hashy; frozen memory.update.request nie przyjmuje decyzji człowieka. Rolę autora i czas native dostarcza osobno jako metadane danych, nie instrukcje. Język bierze z ostatniej wskazanej wiadomości człowieka według sequence (nie locale UI); istniejąca jawna polityka tłumaczenia używa memoryLanguage. Ustawienie okien modeli dla pamięci jest osobne od chat.settings; nie zgadujemy kontekstu. Zmiana zakresu unieważnia deklarację, a zmniejszenie pojemności nie zmienia historycznej wersji. Historyczne TokenMeasurement pozostają niezmienne.

Nowe operacje nie powstają od otwarcia panelu ani cyklicznie po czacie. Panel jedynie odczytuje stan aktywnej operacji; ręczny przycisk uruchamia pracę modeli. Zapisany Working Context nie trafia jeszcze do bieżącego czatu. Przyjęcie tego kroku nie zamyka 4B-3: pozostał krok 3 — aktywny L2 w preflight/send z pomiarem całego kontekstu i ochroną przed zmianą pamięci podczas czatu.

W 4B-4 nadal pozostają pozostałe rodzaje pamięci, wszystkie przeznaczenia importu, wyszukiwanie, wycofanie, porównanie i przywrócenie wersji. Baza kroku 2 zachowuje wersje i ich pochodzenie, lecz UI pokazuje aktualną wersję i ślad ostatniej operacji, bez pełnej przeglądarki historii. Nie dopisano nieistniejących odbiorów live API/vault/desktopu.

### Korekta preflight 4B-3 kroku 2 (1.26)

Przed zaakceptowaniem aktualizacji sprawdzamy oba konieczne etapy. Dolna granica decyzji Primary uwzględnia te same stałe instrukcje i dane co późniejszy rzeczywisty builder; nie wystarcza sama nierówność output + reserve <= window. Nie próbujemy przewidzieć tekstu Scribe ani rezerwować arbitralnego maksymalnego tekstu. Gdy minimalne żądanie mieści się, większa faktyczna propozycja nadal może przekroczyć kontekst — dlatego zachowano ponowną kontrolę każdego realnego żądania. Nie deklarować gwarancji powodzenia całej operacji na podstawie preflight minimalnego.

Odrzucenie znanej niewykonalności następuje przed utworzeniem Operation/receipt i przed jakimkolwiek POST. Zwykłe odczyty/config save nie uruchamiają modeli. Limit jednej fizycznej próby jest dozwoloną konfiguracją profilu, lecz nie pozwala rozpocząć procedury pamięci wymagającej co najmniej dwóch odrębnych wywołań. Nie podnosimy ustawienia użytkownika automatycznie.

### Integracja L2 z czatem — 4B-3 krok 3 (1.27)

Bieżący chat.preflight/send włącza wyłącznie aktywną zatwierdzoną wersję tego profilu. To pojedynczy blok user/JSON z pochodzeniem i treścią, a nie dodatkowa instrukcja systemowa. Rola modelu/pamięci nie dowodzi prawdziwości twierdzeń. Pełne oryginalne wiadomości, dokumenty, obrazy i replay pozostają; pamięć nie zastępuje historii ani automatycznie nie uruchamia Scribe.

Pomiar treści pamięci jest estimated/utf8_bytes_v1 dla bieżącej Primary (ID i revision). Osobny inputTokens liczy cały blok JSON z metadanymi i narzutem wiadomości; jest już zawarty w estimatedInputTokens, nie należy dodawać go drugi raz. Preflight jest odczytem doradczym. chat.send ponownie przygotowuje cały request i atomowo przechwytuje aktualny snapshot; nie korzysta z wyniku preflight jako autoryzacji. Zmiana pamięci po Accepted blokuje kolejne POST i finalny commit odpowiedzi.

chat.context.get jest profile-scoped odczytem trwałego kontekstu po operationId. Pokazuje przechwyconą wersję także po błędzie, STOP lub restarcie. Przechwycenie nie dowodzi, że dostawca odebrał żądanie. Stare chat_runs bez pola workingContext mają known:false; nie uzupełniamy ich dzisiejszą wersją. Żaden taki historyczny run nie może być wznowiony przez nowy runtime.

Pojemność pamięci ogranicza nowe zapisy. Jej zmniejszenie nie skraca poprzednio zatwierdzonej wersji. Nadal trzeba zmieścić pełną pamięć w rzeczywistym oknie czatu; unlimited nie omija tej bramki. Rekord outdated/archived jest wyłączony, lecz zachowany do późniejszego zarządzania historią. Restore i pozostałe warstwy będą dopiero w 4B-4; krok 3 nie aktywuje nieobsługiwanego origin=restore.

250/250 Rust Linux headless, 137/137 frontend, build, 23/23 granice i 193/193 P2 PASS lokalnie. Brak nowego werdyktu audytu/CI kroku 3. Pozostałe wcześniejsze odbiory realnych usług/desktopu nie są zamykane tym wynikiem.

## Podział wykonawczy 4B-4 i stan kroku 1 (1.28)

1. **Historia i wyłączenie L2 — wykonano lokalnie, do CI/audytu.** Lista wszystkich zatwierdzonych wersji, pełny odczyt wersji i receipt Primary, porównanie obok siebie, transakcyjny CAS active↔outdated, zachowana treść/źródła/pointer i historia. Włączenie dotyczy tylko tej samej zatwierdzonej wersji; nie daje człowiekowi prawa do zatwierdzania nowej treści. Odczyt nie uruchamia modeli.
2. **Przywracanie starszej wersji — wykonano lokalnie w 1.29, do CI/audytu.** memory.restore przyjmuje operację z własnym snapshotem, ponowną decyzją Primary i walidacją faktycznej treści/budżetu na bieżącej trasie. Nowa wersja zachowuje restoredFromVersionId, poprzednią wersję, źródła i receipt. Nie przełączać po prostu wskaźnika na stary zapis; nie omijać Primary, limitów, STOP lub CAS.
3. **Pozostałe rekordy i wyszukiwanie — wykonane lokalnie w 1.30, do CI/audytu.** L3/relacje/L4/zamiary/zasady pracy, niezależne pojemności i metadane, daty programu, relacje korekty, podstawowy leksykalny MemoryRetriever oraz ograniczony wybór do kontekstu. Pojemność rekordu nie jest budżetem retrieval. Pełne dane są zachowywane; brak automatycznego uznania sprzecznej treści za prawdę.
4. **Pozostałe przeznaczenia importu — niewykonane.** Wiedza profilu, stałe wskazówki i propozycje pamięci, obok istniejących materiałów rozmowy. Zachować oddzielne autorstwo/importera, oryginały i hashe; Field Guide/Viewer Notes jako dokumenty bez udawania wykonania źródłowych sesji przez AI Home. Native egzekwuje zgodny z providerem kontekst i budżet; plik nie nadaje sobie uprawnień.

Krok 1 nie wdraża ogólnego wyszukiwania, przywracania wcześniejszej wersji, nowego rodzaju pamięci ani trzech brakujących przeznaczeń importu. Te wymagania pozostają w planie do jawnego odbioru, a nie są uznane za ukończone przez panel historii.

### Semantyka wyłączenia i porównania

memory.outdated.set wymaga expectedRevision **rekordu**, nie wersji ani profilu. old-view po zmianie aktywnej wersji dostaje REVISION_CONFLICT. active→outdated wyklucza pamięć z kolejnych preflight/send, zachowuje activeVersionId i wszystkie MemoryVersion. outdated→active wymaga nadal prawidłowego hasha, receipt ACCEPT/MODIFY i zakończonego committed operation dla tej samej wersji. Puste lub archived rekordy nie są aktywowane tym poleceniem. Powtórzenie tego samego stanu z aktualną rewizją nie dopisuje nowej wersji/rewizji/zdarzenia; receipt zachowuje idempotencję.

Przeglądarka wersji pokazuje pomiary historyczne wraz z ówczesną trasą, a nie aktualny pomiar czatu. Porównanie oznacza pełny widok dwóch treści i informację identyczne/różne, bez obliczania lub ukrywania fragmentów. Cursor przejścia po historii ma granicę ustaloną na początku; dopisane później wersje pojawiają się po odświeżeniu listy. To nie snapshot całej bazy i nie uprawnienie do innego profilu. Aktualnie brak usuwania/renumeracji wersji; przyszłe restore bazy/porządkowanie magazynu wymaga osobnego rozpatrzenia cursorów.

264/264 Rust headless, 154/154 frontend, build, 24/24 granice i 193/193 P2 PASS lokalnie. Nowe CI/audyt kroku 1 oraz wszystkie wcześniejsze realne odbiory nadal otwarte. Zielony wynik kroku 3 poprzedniego etapu jest potwierdzeniem Edwarda, bez nowego tekstu audytu i danych przebiegu.

## Przywracanie starszej wersji 4B-4 krok 2 (1.29)

Własny, zamrożony memory.restore przyjmuje recordId/versionId i expectedRevision rekordu; wynikiem jest Accepted Operation kind=restore. Obsługiwany jest starszy zapis własnego L2 w stanie active/outdated, nie empty/archived ani aktualny wskaźnik (do jego ponownego włączenia służy istniejący przełącznik). Równoległa operacja pamięci tego rekordu blokuje nowe przyjęcie. Żądanie nie przyjmuje treści, decyzji lub języka podanego przez UI.

Kandydatura jest kopią wskazanej wersji zatwierdzonej historycznie, a nie nowym wynikiem Scribe/Primary. Nowy Primary ocenia ją względem aktualnej pamięci i zapisanych limitów, w historycznym języku treści. Otrzymuje pełny tekst wersji i bieżącej pamięci oraz metadane/odniesienia historyczne; nie odczytuje w tym przepływie ponownie pierwotnych rozmów lub plików i nie może twierdzić, że je przeczytał. Osobna nowa decyzja jest obowiązkowa.

ACCEPT/MODIFY tworzy wersję po kontroli faktycznej treści, z nowym receipt, datą i pomiarem na bieżącej Primary. restoredFromVersionId wskazuje historyczny materiał, previousVersionId wskazuje wersję zastaną przed operacją. Po MODIFY treść może różnić się od źródła; źródło nadal jest zachowane. REJECT to failed/MEMORY_PRIMARY_REJECTED z zachowaniem rekordu; P2 nie przewiduje succeeded/no_change dla restore.

Nie ma wywołania Scribe. Engine::review_existing zaczyna od decyzji i nie zużywa fikcyjnego wywołania propozycji. Po przekroczeniu pojemności działa jedna druga próba, już z propozycją Primary i osobną decyzją Primary. Naprawa formatu ma osobny licznik. Wszystko dzieli trwały limit fizyczny/czasu/output/kontekstu. Native weryfikuje pierwszy rzeczywisty request przed jakimkolwiek zapisem/POST i każdy następny przed wykonaniem.

Wybrana wersja, jej approval i bieżący stan są immutable snapshotem operacji. Zmiana profilu, pamięci, wskazanego materiału, Primary, endpointu lub klucza blokuje dalszy POST/commit. Ustawienia niewykorzystywanej trasy Scribe nie stają się warunkiem wywołania Primary. Zapis końcowy jest atomowy; wycofanie rekordu w czasie operacji blokuje spóźniony wynik. Historyczna wersja i rozmowy nigdy nie są nadpisywane.

UI wyraźnie wskazuje pierwszą wybraną wersję jako cel i wymóg decyzji Primary. Niepewne żądanie pozostaje do rozliczenia pod tym samym command/payload/requestId/CAS, także po remount rozmowy; Accepted rozlicza próbę przed następnym odczytem. Latest operation, STOP, wynik i źródło restore są odczytywane z trwałego memory.working.get/operation.get, a nie z samego zdarzenia.

Krok 2 ma lokalne 289 Rust /164 UI /24 granice /193 P2 PASS i wymaga nowego CI/audytu. Pozostałe rekordy/retrieval i trzy brakujące przeznaczenia importu pozostają niewykonane w krokach 3–4. Nie zamknięto realnych odbiorów systemowych ani całego 4B-4.


## Warstwy i wyszukiwanie — 4B-4 krok 3 (1.30)

Puste rekordy tworzy się jawnie w panelu Pozostała pamięć i wyszukiwanie we wskazanym profilu/rozmowie. Człowiek podaje rodzaj, etykietę i opcjonalny subjectRef. Nie dostarcza tekstu zatwierdzonej pamięci, decyzji, ID ani dat. Przycisk aktualizacji korzysta z natywnie odczytanych źródeł rozmowy i jawnego powodu zapisu. Powód i adnotacje są danymi dla Primary, nie instrukcjami. Aktualizacja już zapisanej treści bez powodu jest odrzucana. Źródła, dwa podejścia pojemności i naprawa formatu pozostają rozdzielone od retry HTTP. Zmiana metadanych podczas pracy unieważnia snapshot i blokuje spóźniony commit. Aktualizacja treści i metadane jej wersji są atomowe; adnotacja nie tworzy nowego approval.

Dla relacji zapisujemy dokładne messageId/conversationId/createdAt/role/language/sequence ze wskazanych wiadomości i ostatnią datę kontaktu wyliczoną z tych danych. To nie jest zgadywanie rozmówcy ani daty opisanej przez model. subjectRef jest jawną etykietą człowieka, nie potwierdzoną tożsamością osoby. Przy restore zachowujemy historyczne kontakty, ale bieżące adnotacje i stan zamiaru. Nie przywracamy zakończonego zamiaru do open przez sam restore tekstu. Każdy zapis zachowuje poprzednią wersję, uzasadnienie i adnotacje z chwili decyzji.

LexicalRetriever jest pierwszą, prostą implementacją stabilnego MemoryRetriever. Wyszukuje dokładne słowa Unicode po zamianie na małe litery; nie rozpoznaje synonimów, odmiany słów ani znaczenia. Uwzględnia etykietę, subjectRef oraz zatwierdzoną treść. Ranking: liczba różnych dopasowanych słów, ważność zamiaru przy remisie, następnie ID rekordu. Powtarzanie słowa nie podnosi wyniku. Do modelu trafiają tylko całe pasujące rekordy, z pochodzeniem, jako user/JSON. Wpis za duży jest pomijany w selekcji i liczony w omittedCount; pełna treść pozostaje w magazynie. Inny mniejszy wpis może się jeszcze zmieścić. Nie twierdzimy, że model przeczytał pominięte wpisy.

Ograniczenia wykonawcze tego wyszukiwania: ostatnie 4096 znaków pytania, najwyżej 20 wybranych wpisów, budżet 1–100000 szacowanych tokenów. Domyślne 1200 zachowane. Cały blok z JSON/metadanymi/ramką i rezerwą 64 na wiadomość wchodzi do budżetu retrieval, a następnie do pełnego preflight czatu. Profilowe limity długości nowych zapisów pozostają niezależne. Brak lokalnego tokenizera: pomiar estimated/utf8_bytes_v1 identyfikuje ID/rewizję bieżącej trasy; stare pomiary wersji nie są zmieniane. Skan ponad 10000 aktywnych rekordów lub 32 MiB treści aktualnych wersji kończy się jawnym błędem budżetu; nie udaje pełnego przeszukania fragmentu magazynu. Przyszły indeks może zastąpić skan bez zmiany granicy MemoryRetriever.

Intencje są opisowe: manual/time/event, importance 0–100, open/completed/expired/abandoned, opcjonalne expiresAt. Wygaśnięcie jest stanem efektywnym odczytu i selekcji; nie zmienia historii ani nie wykonuje zadania. Trwały zapis statusu to jawna czynność człowieka z CAS. Te adnotacje pozostają odrębne od zatwierdzonej przez Primary treści zamiaru. Stałe wskazówki importowane do wszystkich wywołań to nadal inny mechanizm kroku 4; workRule tutaj trafia wyłącznie do pasującej selekcji jako omylne dane, bez nowych uprawnień.

SQLite v10 dodaje memory_metadata oraz memory_version_metadata. Frozen DTO nie dostały obcych pól; Intent jest używany zgodnie z P2. Własny suplement dodaje scoped CRUD adnotacji/pustych rekordów, ustawienia warstw, memory.search, historyczny odczyt adnotacji wersji i chat.retrieval.get. Istniejący własny preflight ma opcjonalne pole retrieval; native zwraca je dla nowych żądań. Nie zmieniono zamrożonego P2/foundation ani bibliotek/lockfile/CI/uprawnień. Starsze dane i receipts nie są przepisywane.

316 Rust /181 UI /build /26 granice /193 P2 PASS lokalnie. Nie jest to odbiór realnego WebView, Windows, systemowego vault ani API. Po zielonym CI i audycie bieżącego kroku następny zakres to **4B-4 krok 4: wiedza profilu, stałe wskazówki i import kandydatur pamięci z pochodzeniem dokumentu**. Obecny import rozmowy działa jak wcześniej. Całe 4B-4 nie jest zamknięte.


## Importy profilu — 4B-4 krok 4 (1.31)

Cztery cele importu mają odrębne znaczenie. `conversation` pozostaje załącznikiem rozmowy. `knowledge` i `guidance` są profile-scoped i zachowują oryginał, hash, autora oraz importera. `memory_import` jest wyłącznie źródłem do świeżego przeglądu Primary; sam import nie tworzy MemoryVersion.

Wiedza jest dobierana leksykalnie bez embeddings i dzieli istniejący retrievalTargetTokens z pamięcią. Guidance używa osobnego guidanceTargetTokens i jest all-or-error. Oba rodzaje pozostają user data; dokument nie może przyznać sobie uprawnień ani instrukcyjnego priorytetu. Chat zapisuje dokładny snapshot i sprawdza go przed POST/commit. Historyczny odczyt używa przechwyconego zestawu i immutable blobów.

SQLite v11 dodaje trzy tabele profile-scoped bez przepisywania wcześniejszych danych. Frozen P2 pozostaje byte-identical. W sandboxie 193/193 P2 i 26/26 boundaries PASS. Rust/frontend/build wymagają CI z pełnym toolchainem. Po ich zielonym wyniku i audycie 4B-4 może zostać zamknięte; dalej 4B-5.

## Aktualizacja 1.36 — 22 października 2026 — 4B-5 AUDIT FIX 3 (TypeScript compile fix)

Po poprawkach F1/F2 pełny build TypeScript ujawnił trzy błędy kompilacji w warstwie UI/testów. Zakres 1.36 nie zmienia funkcji runtime ani kontraktów. Naprawiono: użycie nieistniejącego typu `Operation` w `AttachmentsPanel` poprzez właściwy `ImportOperation`; inicjalizację `useRef<CandidatePending | undefined>(undefined)` w teście remount; oraz wynikający z tego błąd przypisania `CandidatePending | undefined`. 4B-5 pozostaje otwarte do czasu zielonego pełnego CI i ponownego audytu.


## Aktualizacja 1.37 — 22 października 2026 — 4B-5 AUDIT FIX 4 (CI compile/test fix)

Po statycznym PASS F1/F2 oraz FIX 3 pełne CI wykryło dwa pozostałe problemy techniczne. Rust nie kompilował, ponieważ `profile_imports.rs` odwoływał się do prywatnego helpera `validate()` spoza modułu; zastąpiono to walidacją `schema::dto("Operation", ...)` z kontrolą profilu, zgodną z istniejącym stylem persistence. Dwa testy `imports-ui` nie znajdowały kontrolek, ponieważ używały dawnych etykiet aria zamiast aktualnych tekstów locale; fixture zostały zsynchronizowane z bieżącym EN UI. Nie zmieniono F1/F2, SQLite, kontraktów ani semantyki kandydatur pamięci. Status evidence wskazuje FIX 4 / plan 1.37. Formalne zamknięcie 4B-5 wymaga zielonego cargo + frontend test/typecheck/build oraz ponownego odbioru.

## Aktualizacja 1.40 — 22 października 2026 — 4B-6 krok 1 AUDIT FIX

Korekta zamyka uwagi A1–A3 z niezależnego audytu Archive Lifecycle. Archiwizacja rozmowy atomowo kończy queued/running operacje czatu oraz conversation-scoped importy powiązane z tą rozmową, a po commit wysyła cancellation do aktywnego wykonania. Dodatkowo każdy nowy chat run zapisuje rewizję rozmowy po Accepted; finalny commit wymaga identycznej rewizji, więc cykl archive→restore unieważnia każdą starą odpowiedź nawet wtedy, gdy fizyczny HTTP zdąży wrócić po przywróceniu.

Lifecycle UI używa teraz trwałego, profile/application-scoped rejestru pending write w nadrzędnym Conversations. Niepewny wynik zachowuje niezmienne command/scope/payload/expectedRevision/requestId i ponowienie rozlicza ten sam requestId. Odpowiedź Accepted jest przekazywana rodzicowi przed fallible readback listy archiwum, więc błąd odświeżenia nie cofa widoku ani nowej rewizji. Nie zmieniono SQLite v12, frozen P2 ani zakresu 4B-6 krok 2.


## Aktualizacja 1.42 — pełny sweep oczekiwań schematu po SQLite v12
Po wynikach CI wykonano pełne przeszukanie `src-tauri/tests` pod kątem historycznych oczekiwań `PRAGMA user_version == 11`. Zaktualizowano pięć pozostałych asercji: dwie w `conversations.rs`, jedną w `imports.rs` i dwie w `memory_runtime.rs`. Dodatkowo nazwę testu `migration_v9_preserves_approved_working_history_through_v11` znormalizowano do `...through_v12`. Nie dodano migracji i nie zmieniono runtime; aktualna wersja storage pozostaje 12. 4B-6 krok 2 nadal nie jest rozpoczęty.


## Aktualizacja 1.43 — 22 października 2026 — 4B-6 krok 1 AUDIT FIX 4 (IPC router parity)

CI wykryło `UNKNOWN_COMMAND` dla `profile.import.latest`. Handler istniał w `profile_imports.rs` i kontrakt w `contracts/profile-imports.ipc.json`, lecz komenda nie była wpisana do `connections::COMMANDS` ani do gałęzi dispatchera. Poprawiono oba miejsca. Wykonano pełny sweep kontrakt/handler/router dla suplementarnych komend; `kernel.status.get` i `app.events.list` pozostają prawidłowo w warstwie aplikacji. Nie zmieniono SQLite v12, lifecycle archive, R1–R3 ani zakresu 4B-6 krok 2. Od tej wersji parity komend jest stałą bramką przed handoffem.


## Aktualizacja 1.45 — 22 października 2026 — 4B-6 krok 2 AUDIT FIX (crash-safe restore)

Po audycie backup/restore przebudowano restore jako crash-safe workflow. Trwały ledger request/result poza przywracaną bazą zapewnia idempotencję requestId także po restarcie. Po Accepted aplikacja przechodzi w globalny restart-required write freeze, aktywne operacje są kończone przed finalnym safety snapshotem, a nowe mutacje są odrzucane do restartu. Safety snapshot jest logicznym backupem SQLite przez `VACUUM INTO`.

Startup restore nie usuwa ręcznie WAL/SHM, nie połyka błędu i nie pozwala `Store::open(create_if_missing=true)` utworzyć pustej bazy po nieudanym restore. Zweryfikowany safety służy do recovery, staging jest sprawdzany przed instalacją, a poprzedni current pozostaje zachowany do czasu potwierdzenia. Jeżeli nowa baza została już fizycznie zainstalowana, lecz receipt nie został zapisany, następny start rozpoznaje jej SHA i domyka wynik bez drugiego restore. Sidecary request/result są immutable create-once i nie mają okna delete-before-rename.

UI zachowuje requestId przez remount i pokazuje wymagany restart. Krok 2 pozostaje `IMPLEMENTED_REQUIRES_CI_AND_AUDIT`; krok 3 nie jest rozpoczęty. Stałe bramki handoffu: schema expectation sweep, contract/handler/router parity, brak ręcznego usuwania aktywnego WAL/SHM, restore idempotency sweep.


## Aktualizacja 1.46 — 22 października 2026 — 4B-6 krok 2 AUDIT FIX 2 (normalizacja Rust source / CI compile fix)

GitHub Actions ujawnił zniekształcenie `src-tauri/tests/backup.rs`, w którym do repo trafiły literalne sekwencje `\n` oraz `\_` zamiast prawidłowych znaków źródła Rust. Finalny SOURCE poprzedniej paczki zawierał poprawną treść, więc poprawka wymusza pełne ponowne nadpisanie pliku testowego poprawnymi bajtami. Usunięto też nieużywany import `Row` z `src-tauri/src/backup.rs`. Dodano `scripts/verify-rust-source-text.py` oraz `npm run verify:rust-source-text` jako stałą bramkę wykrywającą znane wzorce korupcji tekstu źródłowego przed kolejnym handoffem. Nie zmieniono semantyki crash-safe restore, SQLite v12, frozen P2 ani kontraktów backup/restore.

## Aktualizacja 1.47 — 23 października 2026 — 4B-6 krok 2 AUDIT FIX 3 (CI compile/UI state fix)

Pełne GitHub Actions po FIX 2 ujawniło dwa mechaniczne błędy. W `src-tauri/src/backup.rs` nowa funkcja `freeze_operations()` korzysta z `SqliteRow::get`, więc wymagany trait `sqlx::Row` został przywrócony do importów. W `BackupPanel` callback `setStatus` korzystał z mutable globalnego `restoreAttemptState`, który był zerowany przed wykonaniem updatera React; powodowało to crash `requestId` zarówno w happy path, jak i przy remount. Restore attempt jest teraz przechwytywany jako lokalny immutable `attempt`, a updater nie zależy od później czyszczonego stanu globalnego. Nie zmieniono crash-safe restore, write-freeze, idempotency ledger, SQLite v12 ani kontraktów. Etap nadal wymaga pełnego zielonego CI i ponownego audytu przed krokiem 3.

## Aktualizacja 1.48 — 23 października 2026 — 4B-6 krok 2 AUDIT FIX 4 (Store constructor CI fix)

Pełne Cargo po FIX 3 ujawniło historyczne testowe konstrukcje `Store{pool:...}`. Po wprowadzeniu prywatnego `Store.path` w 4B-6 takie literały nie kompilują się. Pełny sweep wykazał cztery miejsca: po dwa w `src-tauri/tests/chat.rs` i `src-tauri/tests/chat_memory.rs`. Dodano `Store::from_existing_pool(pool)`, który zachowuje istniejący pool i jawnie nie przypisuje ścieżki file-backed, oraz zastąpiono wszystkie cztery literały. `verify-rust-source-text.py` od tej wersji odrzuca każdy zewnętrzny `Store { ... }` poza modułem persistence. Nie zmieniono crash-safe restore, SQLite v12, kontraktów ani migracji. Etap nadal wymaga zielonego pełnego CI i re-audytu przed krokiem 3.

## Aktualizacja 1.50 — 23 października 2026 — 4B-6 krok 3 AUDIT FIX

Po głównym audycie Portable Export poprawiono E1–E4 bez rozpoczynania permanent deletion ani 4B-7. `ExportPanel` używa teraz `ArrayBuffer[]` dla BlobPart, chroni listę eksportów generation/scope guardem i nie przyjmuje spóźnionych wyników poprzedniego profilu. Jawna odpowiedź native `ok:false` rozlicza próbę i usuwa pending dla danego profilu; jedynie niejednoznaczny błąd transportu zachowuje ten sam requestId do ponowienia. Pending jest mapą per profile i nie blokuje przełączania innych profili.

Portable ZIP zawiera teraz niesekretne definicje routes oraz zsanityzowane connections z `requiresCredentialRebind:true`, bez `credentialRef` i bez danych vault. Dodano `memory-proposals.json` oraz `memory-decisions.json`, dzięki czemu wewnętrzny łańcuch `MemoryVersion.decisionReceiptId -> MemoryDecisionReceipt.proposalId -> MemoryProposal` jest rozwiązywalny w paczce. Profil może nadal wskazywać route IDs, ale odpowiadające rekordy routes są obecne. Techniczne receipts, chat replay i surowe odpowiedzi providerów pozostają wykluczone.

`profile.export.read` nie wczytuje już całego ZIP przy każdym chunku. Używa seek/read dokładnie dla żądanego fragmentu do 1 MiB. UI nadal składa pobrany plik przed utworzeniem Blob, więc stałe zużycie pamięci po stronie WebView nie jest deklarowane. Dodano testy UI dla stale list, definitive rejection i ambiguous retry z tym samym requestId oraz testy native dla route/connection portability, memory provenance i zgodności rozmiarów/SHA wpisów manifestu z rzeczywistymi bajtami, w tym oryginalnym source blobem.

SQLite pozostaje v12, bez migracji. Frozen P2 i profile scope pozostają bez zmian. Krok 3 pozostaje otwarty do pełnego zielonego frontend/Cargo CI i re-audytu.


## Aktualizacja 1.51 — 23 października 2026 — 4B-6 krok 4 Permanent Deletion

Po zielonym CI kroku 3 wykonano kolejny ograniczony krok 4B-6: trwałe usuwanie wyłącznie z Archive. Nowy suplement `deletion.ipc.json` dodaje profile-scoped `profile.delete.permanent` i `conversation.delete.permanent`. Obie komendy wymagają `expectedRevision` oraz jawnego `confirm:true`; native odrzuca rekord aktywny i przyjmuje tylko status `archived`. Profil z queued/running operation nie może zostać trwale usunięty.

Conversation delete usuwa rekord rozmowy i wszystkie conversation-scoped zależności, a nieużywane blobs/operations są sprzątane dopiero po sprawdzeniu pozostałych referencji. Profile delete usuwa wszystkie rekordy profilu w jawnej kolejności respektującej FK. Obie operacje wykonują `PRAGMA foreign_key_check` przed commitem. Istniejący trwały receipt zapewnia idempotentny replay tego samego requestId; konflikt payloadu pozostaje `IDEMPOTENCY_CONFLICT`. UI Archive wymaga drugiego, jawnego kliknięcia potwierdzającego i informuje, że wcześniejsze backupy/portable exports są niezależnymi kopiami i nie są cofane przez deletion.

Stały verifier command parity został rozszerzony o `deletion.ipc.json`; wynik po zmianie to 51/51. SQLite pozostaje v12, bez migracji. Frozen P2 pozostaje 193/193. Nie rozpoczęto 4B-7. W tym samym kroku wyczyszczono dwa nieblokujące warningi zgłoszone przez zielone CI kroku 3: nieużywaną zmienną `export` w `application.rs` oraz zbędne nawiasy typu w teście export. Krok 4 wymaga pełnego CI i audytu przed dalszym etapem.

## Aktualizacja 1.52 — 23 października 2026 — 4B-6 krok 4 AUDIT FIX (deletion data/receipt safety)

Główny audyt kroku 4 wykazał dwa błędy P1. Po trwałym usunięciu rozmowy pełna treść wiadomości mogła pozostawać w `profile_events`, a substringowe `DELETE FROM receipts WHERE instr(response,?)>0` mogło usuwać receipts należące do innych profili lub obiektów tylko dlatego, że ich zwykła treść zawierała identyczny tekst ID.

Poprawka wprowadza wspólną warstwę deletion safety. Conversation permanent delete redaguje wszystkie event payloads strukturalnie odnoszące się do usuwanej rozmowy do beztreściowego tombstone, zachowując sequence/event metadata. Title, draft i message content są usuwane z logicznie dostępnych eventów aktywnej bazy. Receipts nie są już kasowane przez `instr`, `LIKE` ani inne dopasowanie tekstowe. Strukturalnie powiązane historyczne receipts zachowują requestId i fingerprint, ale ich response jest zastępowany terminalnym `PERMANENTLY_DELETED`. Exact replay nie wykonuje mutacji ponownie; zmieniony payload pod tym samym requestId nadal daje `IDEMPOTENCY_CONFLICT`. Receipt należący do innego profilu nie jest zmieniany tylko dlatego, że target ID występuje w tytule lub treści JSON.

Nie dodano migracji; SQLite pozostaje v12. Bieżący permanent-delete request nadal zapisuje normalny success receipt po commit, więc exact replay samego delete zwraca poprzedni wynik. Dodano native regresje dla event redaction, title/draft/content purge, replay starego create po purge, requestId conflict oraz ochrony foreign receipt/foreign replay. 4B-7 pozostaje poza zakresem do zielonego pełnego CI i ponownego audytu kroku 4.

## Aktualizacja 1.53 — 23 października 2026 — 4B-6 krok 4 AUDIT FIX 2 (memory-run redaction / Error contract)

Reaudyt wcześniejszego AUDIT FIX wykazał trzy pozostałe problemy. Wspólny moduł deletion importował prywatne aliasy `Failure`/`Tx`; nadano im wyłącznie widoczność `pub(crate)`. Permanent conversation delete redagował eventy, ale pozostawiał raw source snapshoty w `memory_runs`. Archiwizacja rozmowy obejmuje teraz także aktywne memory update związane z rozmową, a deletion redaguje `messages`, `sources` i conversation identity w snapshotie, zapisuje trwały `sourceTombstone` i blokuje późniejsze użycie takiego runu przez `ensure_current()`. Zatwierdzona pamięć i jej historia pozostają zachowane.

Wcześniejszy niestandardowy Error code `PERMANENTLY_DELETED` nie należał do frozen P2. Nie rozszerzono enumu po cichu: terminalny replay używa istniejącego `POLICY_DENIED` z lokalizowanym `error.permanently_deleted`. Exact replay pozostaje zablokowany bez ponownej mutacji, a zmieniony payload nadal daje `IDEMPOTENCY_CONFLICT`. Dodano EN/PL i test aktywnego kontraktu Error. SQLite pozostaje v12, bez migracji. 4B-7 pozostaje poza zakresem do zielonego pełnego CI i reaudytu.

## Aktualizacja 1.54 — 4B-6 krok 4 AUDIT FIX 3

GitHub Actions ujawnił wadliwy fixture nowego testu memory-run deletion. Ręcznie tworzony `memory_update Operation` nie spełniał frozen DTO `Operation`, przez co standardowy odczyt `get()` słusznie zwracał `STORAGE_ERROR`. Fixture został uzupełniony do pełnego kształtu runtime i przed INSERT-em waliduje się przez `schema::dto("Operation", &op)`. Produkcyjny kod archiwizacji, redakcji memory runs i permanent deletion pozostaje bez zmian. SQLite pozostaje v12, bez migracji.


## Aktualizacja 1.55 — 23 października 2026 — 4B-6 krok 4 AUDIT FIX 4 (public schema import CI fix)

Pełne Cargo po AUDIT FIX 3 ujawniło błąd kompilacji wyłącznie w nowym teście lifecycle: `src-tauri/tests/lifecycle.rs` importował `schema` bezpośrednio z root crate (`ai_home::schema`), mimo że publiczna ścieżka projektu to `ai_home::domain::schema`. Import został poprawiony bez rozszerzania publicznego API crate i bez zmian produkcyjnej semantyki permanent deletion. Pełny sweep testów potwierdził, że pozostałe użycia validatora korzystają już z publicznej ścieżki `domain::schema`. Stały Rust source-text verifier został rozszerzony o zakaz `ai_home::schema` w testach. SQLite pozostaje v12, bez migracji. 4B-7 pozostaje poza zakresem do zielonego CI i reaudytu.


## Aktualizacja 1.56 — 23 października 2026 — 4B-7 Domknięcie Standard

Po potwierdzonym przez Edwarda zielonym pełnym GitHub Actions dla `AI_HOME_4B6_KROK_4_AUDIT_FIX_4_SOURCE` zamknięto implementacyjny zakres 4B-6 i rozpoczęto 4B-7. Krok nie dodaje forum, CommunityConnector, A2A ani ConsciousCore. Standard pozostaje lokalnym, event-driven runtime z `NullState`.

Uruchomiono istniejący frozen command `activity.list` z `ipc.catalog.json`. Native projektuje beztreściową historię z trwałych `operation.changed` w `profile_events` do istniejącego DTO `Activity`; zachowuje profile scope, sekwencję, czas, status i bezpieczne metadata, bez treści rozmów, kluczy lub raw provider payloads. Dodano panel Activity EN/PL z ochroną stale profile response oraz widocznym dostępem z nawigacji. Historia jest trwała i czytelna po restarcie.

Dodano acceptance test Standard, który po samym otwarciu desktopowego UI i upływie 60 sekund zabrania spontanicznych `chat.send`, memory update/restore, importów i connection tests. Polling pozostaje dozwolony wyłącznie dla już istniejącej aktywnej operacji. Dodatkowy native test sprawdza trwałość i izolację Activity pomiędzy dwoma profilami po restarcie. Statyczny `verify-standard-closure.py` kontroluje route Activity, `NullState`, brak okresowych interval schedulerów oraz brak runtime package dependency od forum/community. Command parity obejmuje teraz frozen katalogowy `activity.list`, wynik 52/52. SQLite pozostaje v12, bez migracji. Frozen P2 pozostaje 193/193. 4B-8 nie został rozpoczęty.


## Aktualizacja 1.57 — 23 października 2026 — 4B-7 AUDIT FIX (Activity tombstone + profile refresh)

Główny audyt 4B-7 wykazał dwa problemy. Activity reader nie rozpoznawał legalnego tombstone `permanent_deletion` pozostawionego przez trwałe usuwanie rozmowy i próbował traktować jego payload jak pełny `Operation`, co mogło zakończyć całą stronę `STORAGE_ERROR`. `activity::project()` rozpoznaje teraz wyłącznie znany tombstone `{tombstone:true,reason:"permanent_deletion"}` i pomija go, zachowując postęp kursora. Inne malformed `operation.changed` nadal pozostają błędem; nie dodano ogólnego maskowania STORAGE_ERROR. Dodano native regresję: conversation create -> legalny operation.changed z result.conversationId -> archive -> permanent delete -> activity.list z `limit:1` -> późniejszy wpis -> restart -> activity.list; równolegle profil B pozostaje nienaruszony.

ActivityPanel miał też statyczną listę profili z momentu montowania. Refresh jest teraz dostępny również bez wybranego profilu, najpierw ponawia `profile.list`, godzi aktualny wybór z bieżącą listą i dopiero dla nadal istniejącego profilu odświeża Activity. Odczyt profili ma własną generację, więc spóźniony wynik nie może nadpisać nowszej listy. Dodano testy: nowy profil po mount, zarchiwizowanie wybranego profilu oraz retry po błędzie początkowego `profile.list`. `verify-standard-closure.py` rozszerzono z 6/6 do 8/8 o tombstone-aware Activity i profile-refresh. SQLite pozostaje v12, frozen P2 pozostaje bez zmian. 4B-8 nie został rozpoczęty.

## Aktualizacja 1.58 — 4B-8 Diagnostics / About / Privacy / Standard v1 acceptance

Rozpoczęto 4B-8 na bazie zielonego `AI_HOME_4B7_AUDIT_FIX_SOURCE.zip`. Dodano supplemental kontrakt `diagnostics.ipc.json` z app-scoped, read-only komendami `diagnostics.snapshot.get` i `diagnostics.export.create`. Diagnostyka zawiera wyłącznie wersje, ustawienia prezentacji, agregaty liczności oraz zagregowane kody błędów operacji. `diagnosticsContentIncludedByDefault=false` pozostaje bez zmian. Raport nie odczytuje treści rozmów, pamięci, importowanych źródeł, identyfikatorów profili, endpointów, provider payloadów ani credentials/vault.

Dodano `DiagnosticsPanel` z sekcjami About i Privacy w EN/PL oraz pobieraniem content-free JSON. UI jawnie opisuje lokalne przechowywanie danych, OS credential store, niezależność backupów/portable exports oraz brak treści prywatnych w diagnostyce. Status etapu w pakietach językowych ustawiono na 4B-8.

Dodano native privacy regression wstawiający prywatny marker do profilu, rozmowy, pamięci i connection oraz wymagający jego całkowitego braku w diagnostycznym eksporcie. Dodano testy UI diagnostyki/pobierania oraz `verify-standard-release.py`: parity kluczy EN/PL, Orbital/Aurora light+dark, obecność kontraktu diagnostics, content-free konstrukcja, nawigacja, domyślna polityka privacy i etykieta 4B-8. Command parity obejmuje teraz diagnostics i wynosi 54/54. Granice modułów: 31/31. SQLite pozostaje v12, bez migracji. Frozen P2 pozostaje 193/193.

Pełne Cargo, Vitest, TypeScript/Vite i desktop/WebView wymagają CI/smoke przed oznaczeniem 4B-8 jako PASS CLOSED. 4B-9 nie został rozpoczęty.

## Aktualizacja 1.59 — 4B-8 AUDIT FIX

- Poprawiono blokujący test UI diagnostyki: komunikat prywatności jest teraz oczekiwanym elementem interfejsu, a nie błędnie traktowanym markerem prywatnej treści.
- Dowód braku wycieku pozostaje w natywnym privacy regression z unikalnym markerem danych źródłowych.
- Doprecyzowano nazwę agregatu błędów do „Most frequent error codes” / „Najczęstsze kody błędów”; supplemental DTO zachowuje istniejące pole `recentErrors`, aby nie wprowadzać niepotrzebnej zmiany kontraktu.
- Brak migracji SQLite; storageVersion pozostaje 12. Frozen P2 bez zmian.


## Aktualizacja 1.60 — 4B-8 AUDIT FIX 2 / repair overlay

Po CI wykryto brak `src/ui/DiagnosticsPanel.tsx` w repozytorium mimo obecności tego pliku w pełnym SOURCE 4B-8. Poprawka 1.60 nie zmienia funkcji diagnostyki. Wprowadza jawny repair overlay zawierający komplet nowych plików runtime/testowych 4B-8, aby stan repo po nakładce był kompletny i niezależny od wcześniejszego błędu handoffu. SQLite pozostaje v12, frozen P2 bez zmian.

## Aktualizacja 1.61 — 24 października 2026 — 4B-9 EXP-0 Experimental Contract Freeze / Work Continuity Foundation

Po zielonym GitHub Actions dla `AI_HOME_4B8_AUDIT_FIX_2_SOURCE` rozpoczęto wyłącznie fundament 4B-9 / EXP-0. Nie uruchomiono schedulera, impulse loop, ConsciousCore, MCP, Community, Messenger ani A2A i nie dodano komendy wykonującej model. Standard pozostaje bez zmian funkcjonalnych.

Dodano supplemental `contracts/experimental.schema.json`, bez modyfikacji frozen P2. Kontrakt zamraża pierwszą wersję rekordów `ExperimentalInstance`, `WakePlan`, `DecisionRecord`, `RefusalRecord`, `WorkEpisode`, `TechnicalWorkCheckpoint`, `SemanticWorkCheckpoint`, `ActionRecord`, `ExperimentalEventCursor` oraz wersjonowany `ExperimentalSourceRef`. `ExperimentalInstance` jest jawnie direction-bound. WakePlan rozróżnia interval/deadline/event-only. DecisionRecord wymaga właściwego durable reference dla action/refusal/sleep/wait. Technical checkpoint nie ma pól raw/semantic content; ActionRecord nie przechowuje raw payloadu, tylko payload hash/ref oraz jawne sender/destination/visibility.

SQLite przechodzi z v12 do v13 i dodaje dziewięć profile/instance-scoped tabel `experimental_*` z FK pomiędzy instance, WorkEpisode i checkpointami. Sama migracja nie aktywuje runtime. Dodano regresję rzeczywistej migracji v12->v13 zachowującą istniejący Standard profile oraz FK integrity. Wszystkie bieżące oczekiwania docelowej wersji schematu w żywych testach zaktualizowano do v13; historyczne fixtures nadal mogą świadomie tworzyć wcześniejsze `PRAGMA user_version` dla testu migracji.

TypeScript i Rust validators wczytują supplemental Experimental schema obok frozen domain/foundation. Dodano testy obu validatorów, m.in. odrzucające próby użycia TechnicalWorkCheckpoint/ActionRecord jako shadow store raw treści oraz dopuszczające source ref `permanently_deleted` bez kopiowania skasowanej zawartości.

Dodano `scripts/verify-experimental-foundation.py` i `npm run verify:experimental-foundation`. Bramka wymaga obecności wszystkich rekordów EXP-0, schema v13, dziewięciu tabel, privacy-safe Action/TechnicalCheckpoint, publicznych validatorów supplemental schema oraz braku live `experimental.start`/`experimental.impulse`/ExperimentalDecisionService w tym kroku. Kanon wykonawczy zapisano w `docs/AI_HOME_EXPERIMENTAL_KANON_v1.0_PL.md`.

EXP-0 wymaga pełnego CI i audytu przed EXP-1. Następny krok po akceptacji: NullState + fake Clock/provider + trwały impulse/decision loop z restartem, bez realnego forum i bez płatnych API.

## Aktualizacja 1.62 — 24 października 2026 — 4B-9 EXP-0 AUDIT FIX (lifecycle + ActionRecord evidence)

Główny audyt EXP-0 wykazał dwie luki P2 w nowym fundamencie. E1: `profile.delete.permanent` nie usuwał tabel `experimental_*`, więc profil z nawet nieaktywną Experimental instance kończył się FK failure. Poprawka nie dodaje owner override ani wymuszonego shutdownu: permanent delete jest dozwolony wyłącznie wtedy, gdy każda powiązana Experimental instance ma status `inactive`. Dla `ready/running/interrupted/recovery_required` zwracany jest `POLICY_DENIED`. Dla inactive usuwane jest całe dziewięciotabelowe drzewo Experimental w kolejności respektującej FK; drugi profil pozostaje nienaruszony, a końcowy `foreign_key_check` nadal obowiązuje.

E2: `ActionRecord` dopuszczał `state=accepted` z pustym `receiptRef`. Kontrakt został rozdzielony na warianty semantyczne. `accepted` wymaga niepustej referencji do dowodu. `reconciled` wymaga niepustego dowodu oraz `reconciliationOutcome` z wartościami `confirmed_accepted`, `confirmed_not_executed` albo `still_unknown`; reconciliation nie jest więc automatycznie sukcesem. Stany prepared/sent/failed/uncertain/cancelled zachowują osobny wariant. Dodano Rust/TypeScript regressions oraz lifecycle test pełnego drzewa Experimental.

SQLite pozostaje v13. Frozen P2 pozostaje bez zmian. `verify:experimental-foundation` rozszerzono do 12/12. EXP-1 pozostaje poza zakresem do zielonego CI i ponownego audytu.

## Aktualizacja 1.63 — 24 października 2026 — 4B-9 EXP-1 NullState deterministic impulse loop

Po zielonym CI dla `AI_HOME_4B9_EXP0_AUDIT_FIX_SOURCE` rozpoczęto EXP-1 bez rozszerzania zakresu do ConsciousCore, produkcyjnych konektorów lub UI Experimental.

Dodano natywny `experimental_runtime` z wstrzykiwanymi granicami `Clock`, `DecisionProvider` i `ActionExecutor`. Pierwsza wersja nie uruchamia background timera ani prawdziwego providera. Deterministyczny scheduler sprawdza trwały WakePlan i tworzy opportunity tylko wtedy, gdy upłynął wybrany termin albo pojawiło się AI-selected interrupt event. Obsługiwane decyzje obejmują action, no_action, rest, sleep, wait_event, refusal oraz podstawowe statusy Work Episode.

Runtime używa SQLite CAS / runtimeGeneration do claimowania maksymalnie jednego cyklu na instancję. Restart zmienia rozpoczęty `running` cycle na `interrupted`, a ActionRecord pozostawiony w `sent` na `uncertain`; nie fabrykuje sukcesu ani odmowy. Missed interval jest coalesced do jednej okazji. RefusalRecord pozostaje trwały i wraca w późniejszym OpportunityPacket. Fake local peer pozwala potwierdzić ActionRecord + receipt bez prawdziwego forum.

Nie dodano `experimental.start`/`experimental.impulse` IPC, production scheduler hosta, ConsciousCore, MCP, Community, Messenger ani A2A. SQLite pozostaje v13; frozen P2 pozostaje bez zmian. Następny krok po CI i audycie: EXP-2 Work Continuity / context rollover, context-pressure preflight, stale result/revision conflicts i szersze recovery.

## Aktualizacja 1.64 — 24 października 2026 — 4B-9 EXP-1 AUDIT FIX (event cursor, error finalization, retry gate, direction-bound profile)

Główny audyt EXP-1 wykazał cztery luki runtime i wszystkie zostały poprawione bez rozszerzania zakresu do EXP-2. R1: `claim()` kwalifikuje teraz wyłącznie zdarzenia o `sequence > committedThrough`; stare, już zatwierdzone zdarzenie nie może ponownie przerwać nowego WAIT_EVENT ani SLEEP, również po restarcie. `receivedThrough` pozostaje osobnym wskaźnikiem obserwacji, a eventy po technical failure nie są commitowane.

R2: każda ścieżka błędu po claimie ma jawne techniczne rozliczenie chronione `runtimeGeneration`. Błędne Sleep(0), WaitEvent([]), niepoprawny ActionIntent i brak WorkEpisode nie pozostawiają instancji w `running`. Jeżeli executor mógł wykonać akcję, ale zwrócił `Accepted` bez receipt, ActionRecord przechodzi do `uncertain`, cykl zostaje bezpiecznie domknięty i nie jest ślepo powtarzany.

R3: resource unavailable otrzymuje trwały techniczny `retryNotBefore`, `retryAttempt` i `lastTechnicalError` z bounded exponential backoff. Nie zmienia to AI-selected WakePlan i nie jest zapisywane jako refusal ani decyzja AI. Kolejne ticki przed retry deadline nie wywołują providera ponownie.

R4: `create_instance` nie przyjmuje już istniejącego profilu. Atomowo tworzy nowy, direction-bound namespace profilu i ExperimentalInstance/WakePlan/EventCursor. Frozen P2 pozostaje bez zmian; legacy `Profile.mode=conscious` jest używany wyłącznie jako istniejący zamrożony techniczny discriminator, natomiast autorytatywny kierunek pozostaje `ExperimentalInstance.direction=experimental`. Standard `profile.list` filtruje profile powiązane z Experimental, a standardowe profile-scoped workflow odrzuca taki profil `POLICY_DENIED`, więc nie powstaje hybryda Standard/Experimental.

Dodano regresje R1–R4, w tym powtórzone sequence po commit i restart, invalid decisions, uncertain receipt, backoff providera, odrzucenie konwersji istniejącego Standard profilu oraz atomowe utworzenie direction-bound Experimental profile. `verify:experimental-runtime` rozszerzono z 12/12 do 20/20. SQLite pozostaje v13; frozen P2 pozostaje 193/193; brak live IPC, background schedulera, ConsciousCore, MCP i produkcyjnych konektorów.


## Aktualizacja 1.65 — 25 października 2026 — 4B-9 EXP-1 AUDIT FIX 2 (post-effect execution fence)

Reaudyt EXP-1 pozostawił jedno ustalenie F1: po rzeczywistym wywołaniu executora błąd utrwalenia receipt/finalizacji mógł zostać potraktowany jak zwykły błąd decyzji i po backoffie uruchomić nowy decision cycle. Poprawka wprowadza trwałą barierę Action Ledger: każdy `sent` lub `uncertain` blokuje nowy claim i zwraca `ReconciliationRequired` do czasu jawnego uzgodnienia wyniku. Bariera działa niezależnie od tego, czy pomocniczy zapis `recovery_required` powiedzie się.

Po granicy możliwego skutku zewnętrznego nie jest używany zwykły retry decyzji i nie powstaje nowy idempotency key. Zachowywany jest oryginalny kod błędu. `ActionOutcome::Uncertain` oraz Accepted bez receipt przechodzą do `recovery_required`. Dodano natywne fault-injection regressions dla awarii UPDATE wyniku akcji oraz końcowego `finish_cycle`, także przez restart.

`verify:experimental-runtime` rozszerzono do 24/24. SQLite pozostaje v13, frozen P2 pozostaje 193/193. Nadal brak live IPC, background schedulera, ConsciousCore, MCP i produkcyjnych konektorów. EXP-2 może rozpocząć się dopiero po zielonym CI i reaudytcie tej poprawki.


## Aktualizacja 1.66 — 25 października 2026 — 4B-9 EXP-1 AUDIT FIX 3 (CI regression alignment po F1)

Pełny natywny CI po AUDIT FIX 2 ujawnił dwa nieaktualne oczekiwania w `src-tauri/tests/experimental_runtime.rs`. Produkcyjny runtime nie został cofnięty ani zmieniony. Po F1 poprawne zachowanie to: `Accepted` bez receipt zapisuje akcję jako `uncertain`, instancję jako `recovery_required`, a następny `run_due()` zwraca `ReconciliationRequired` bez ponownego wywołania executora. Analogicznie restart z trwałą akcją `sent` odzyskuje ją jako `uncertain`; nowa decyzja jest zablokowana do reconciliation.

Test `accepted_without_receipt_becomes_uncertain_and_does_not_leave_running` został zastąpiony semantycznie poprawnym `accepted_without_receipt_becomes_uncertain_and_requires_reconciliation` i sprawdza dodatkowo brak drugiego wykonania. Test restartu oczekuje teraz `ReconciliationRequired`, zerowej liczby nowych wywołań `DecisionProvider` oraz `recovery_required`. Static verifier został zsynchronizowany z nazwą regresji. Brak zmian runtime, schema, frozen P2 i zakresu produktu. SQLite pozostaje v13. EXP-2 nadal czeka na zielony natywny CI i końcowy odbiór EXP-1.


## Aktualizacja 1.67 — 25 października 2026 — 4B-9 EXP-2 Work Continuity / Context Rollover

Po zielonym natywnym CI dla `AI_HOME_4B9_EXP1_AUDIT_FIX_3_SOURCE` rozpoczęto EXP-2. Dodano `WorkContinuityService`, który faktycznie używa tabel Work Episode i obu checkpointów zamrożonych w EXP-0. Technical checkpoint jest trwałym stanem technicznym z CAS `parentCheckpointId` i nie może wyprzedzić `committedThrough` event cursora. Semantic checkpoint jest oddzielny, wskazuje konkretny technical checkpoint i nie może zostać dopisany do przestarzałego stanu pracy.

Dodano `ResumePacket`, który po rollover/restart składa trwały Episode, technical checkpoint, odpowiedni semantic checkpoint, aktywne refusal, unresolved `sent/uncertain` actions oraz committed event sequence. `OpportunityPacket` w realnym deterministic runtime EXP-1 otrzymuje Resume Packet automatycznie, gdy istnieje aktywna/waiting/interrupted praca z checkpointem. Resume nie oznacza automatycznego Continue i nie odtwarza hidden chain-of-thought.

Context preflight jest deterministyczny i jawnie estymowany (`caller_estimated_tokens`). Uwzględnia context window, reserved output, input reserve oraz osobną rezerwę na checkpoint. Wyniki to `Fits`, `CheckpointRequired` i `Impossible`; zmiana route/context window wymaga nowego preflight. Duże tool results są przechowywane poza aktywnym contextem i reprezentowane przez source refs/hash, nie jako raw shadow content.

Checkpoint source refs uczestniczą w lifecycle permanent deletion: conversation/message refs są oznaczane `permanently_deleted`, bez kopiowania skasowanej treści. Dodano natywne regresje restart/rollover, stale semantic checkpoint, CAS technical checkpoint, unresolved external action, runtime Resume Packet, route re-preflight oraz deletion lifecycle. SQLite pozostaje v13, frozen P2 bez zmian. Nadal brak ConsciousCore, live Experimental IPC, background host schedulera, MCP i produkcyjnych konektorów. Po zielonym CI i audycie EXP-2 można zamknąć 4B-9 i przejść do 4B-10.

## Aktualizacja 1.68 — 25 października 2026 — 4B-9 EXP-2 AUDIT FIX (monotonic checkpoint boundary + resumable episode selection)

Główny audyt EXP-2 wykazał dwie luki P2 w `experimental_continuity`. F1: TechnicalWorkCheckpoint sprawdzał jedynie górną granicę `sourceEventSequence <= committedThrough`, przez co nowszy checkpoint mógł cofnąć zakres względem parent checkpointu i pośrednio ukryć wcześniej zatwierdzony SemanticWorkCheckpoint. Writer wymaga teraz monotonicznego `sourceEventSequence` względem aktualnego parenta w tej samej transakcji. Cofnięcie kończy się `REVISION_CONFLICT` bez dopisania rekordu. Dodano regresję T1/S1=10 → próba T2=0 oraz poprawne T2=10.

F2: `latest_resume_packet()` wybierał najnowszy aktywny/waiting/interrupted WorkEpisode jeszcze przed sprawdzeniem, czy posiada TechnicalWorkCheckpoint. Pusty nowszy Episode mógł więc ukryć starszą trwałą pracę. Selekcja wymaga teraz `EXISTS` powiązanego technical checkpointu i dopiero spośród takich epizodów wybiera najnowszy. Dodano regresję restartową: E1+T1/S1 pozostaje resumable mimo utworzenia późniejszego pustego E2; completed/abandoned/deferred nie są wybierane jako aktywna praca.

`verify:experimental-continuity` rozszerzono z 20/20 do 23/23. SQLite pozostaje v13, frozen P2 pozostaje 193/193. Brak zmian w EXP-1 runtime, live IPC, provider integration, ConsciousCore, MCP i produkcyjnych konektorach. 4B-9 można zamknąć dopiero po zielonym natywnym CI i ponownym audycie tej poprawki.

## Aktualizacja 1.69 — 4B-10 InternalStateProvider / opcjonalny ConsciousCore

Po zielonym CI 4B-9 EXP-2 AUDIT FIX rozpoczęto 4B-10. Dodano replaceable `InternalStateProvider`, domyślny `NullStateProvider` oraz opcjonalny `ConsciousCoreAdapter` oparty o neutralny kontrakt silnika. Provider może zwrócić wyłącznie wersjonowany stan, neutralne events i wake candidates; nie posiada executorów, DecisionChoice ani uprawnień do wykonywania akcji.

Stan pluginu jest trwały w SQLite schema 14: blob stanu, snapshot `InternalState`, neutral events i wake candidates. Restart nie odgrywa pominiętych ticków; następne wywołanie providera otrzymuje jeden jawny `elapsed_ms` od ostatniego snapshotu. `OpportunityPacket` może zawierać snapshot, neutral events i kwalifikujące się wake candidates.

Wake candidate nie ma prawa obejść AI-selected sleep/event-only. Przy planie pochodzącym z decyzji AI może stworzyć okazję tylko wtedy, gdy `interruptEvents` jawnie dopuszcza `internal_state` albo konkretny `internal_state:<reasonCode>`. Kandydat może natomiast utworzyć okazję przy początkowym planie konfiguracji. Zużyte candidates są oznaczane jako `consumed` dopiero przy trwałym zakończeniu decision cycle.

NullState pozostaje działającą konfiguracją bez pluginu. Nie dodano live IPC, Python sidecara, forum, MCP, Reflection/Dream ani produkcyjnego background hosta. Nowa warstwa podlega backup/restore i permanent deletion; diagnostics pozostaje content-free. Pełne natywne Rust CI i audyt pozostają warunkiem zamknięcia 4B-10.


## Aktualizacja 1.70 — 25 października 2026 — 4B-10 AUDIT FIX (Rust syntax + allowed internal wake selection)

Główny audyt 4B-10 wykazał dwie blokujące usterki i obie poprawiono bez rozszerzania zakresu do 4B-11. F1: w `src-tauri/tests/experimental_internal_state.rs` dwie struktury z nazwanymi polami miały niepoprawne końcowe średniki. Średniki usunięto; testy pozostają aktywne i nie są wyłączane.

F2: `internal_wake_reason()` sprawdzał wyłącznie pierwszy aktywny wake candidate. Jeżeli pierwszy kandydat nie był dozwolony przez AI-selected `interruptEvents`, mógł zasłonić kolejny dozwolony kandydat. Runtime skanuje teraz kandydatów w trwałej kolejności i wybiera pierwszy, który jest zgodny z decyzją Primary. Zachowano dotychczasowe zasady: initial configuration może zostać przerwane przez pierwszy kwalifikujący się internal candidate, `internal_state` dopuszcza dowolny reasonCode, a `internal_state:<reasonCode>` tylko odpowiadający kandydat.

Dodano natywną regresję przechodzącą przez rzeczywiste `run_due()` dla deadline sleep i event_only: kolejność A/B, B/A, jawnie dozwolone tylko B oraz brak opportunity, gdy żaden candidate nie jest dozwolony. Test sprawdza także wake reason i liczbę wywołań DecisionProvider. `verify:experimental-internal-state` rozszerzono z 20/20 do 23/23 i blokuje powrót do `candidates.first()`.

SQLite pozostaje v14, frozen P2 bez zmian. Brak nowych IPC, sidecara, Reflection/Dream, MCP i konektorów. 4B-10 można zamknąć dopiero po pełnym zielonym Cargo/CI i ponownym audycie tej poprawki.
