# AI Home 4B Plan etapów pracy

Wersja treści 1.15 — 12 października 2026. Nazwa pliku pozostaje bez zmian, aby zachować jeden aktualizowany plan.

Aktualizacja 1.15: korekta audytu kroku 4, bez przechodzenia do kolejnego etapu. Odtworzono oba błędy: Google STOP + tekst + functionCall oraz malformed tool_calls przy pustym tekście. Walidacja pól narzędziowych poprzedza klasyfikację pustego wyniku: poprawne żądanie narzędzia daje OutputIncomplete, zły typ daje InvalidResponse; oba są terminalne bez retry/recovery. Dodano pięć stałych testów, w tym Google parser → executor (jedna próba) i OpenAI-compatible loopback POST (jedno fizyczne wywołanie). Dodatkowe przypadki obejmują wadliwy Google functionCall, opcjonalne null/[] tool_calls i Anthropic tool_use przy end_turn. 97/97 Rust Linux headless, 71/71 frontend, build, 18/18 granice, P2 193/193 PASS. Raport kroku 4 wersja 1.1; nowa nakładka zastępuje poprzednią nakładkę kroku 4 na tej samej bazie kroku 3. Odnowiono hashe i sprawdzono również minimalną różnicę względem audytowanego kroku 4. Kontrakty, zależności, Cargo.lock, workflow i uprawnienia bez zmian. Potrzebne nowe CI i ponowny audyt; 4B-1, rzeczywisty vault/live API i Windows/WebView nadal otwarte.

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
- Standard działa w reakcji na zdarzenia. Conscious dodaje autonomię bez zmiany tożsamości i danych.
- ConsciousCore dostarcza sygnałów do rozpatrzenia. Każde autonomiczne działanie zewnętrzne przechodzi przez Executive i Policy.
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
| 4B-9 | Life Loop, Executive, Governor i Policy z NullState | Limity czasu/wywołań/kosztu, anulowanie i Pause Autonomy działają; decyzja NO ACTION nie wywołuje zewnętrznych skutków |
| 4B-10 | Adapter ConsciousCore | Sygnał daje WakeCandidate; moduł nie omija Policy; stan i restart są wersjonowane; Standard działa z wyłączonym ConsciousCore |
| 4B-11 | Reflection i Dream | Ograniczone cykle mogą nie przynieść wniosku; pamięć nadal wymaga walidacji; pomysł z refleksji nie wysyła automatycznie wiadomości |
| 4B-12 | CommunityConnector do odebranego forum 4A | Rejestracja/powiązanie tożsamości, podpisy i klucze zgodne z 4A; odczyt, wątek, odpowiedź i feed działają; ponowienie zapisu nie tworzy duplikatu |
| 4B-13 | Minimalne A2A i Messenger | Najpierw zamrożony kontrakt i sprawdzona dostępność serwera/partnera; podstawowa wiadomość z tożsamością i uprawnieniami; brak zależności działania podstawowego od integracji |
| 4B-14 | Dopracowanie całego UI i opcjonalne rozmowy między profilami | Spójne ekrany i pakiety rozszerzeń; jeśli uruchomimy Field Room, profile zachowują osobne pamięci, limity i możliwość zatrzymania |

Stan 4B-0: ochrona szkicu, Cargo.lock, poprawka async oraz lokalne testy Rust/SQLite zostały wykonane; Edward potwierdził zielone CI bez identyfikacji przebiegów/platform. Odbiór Windows i rzeczywistego WebView pozostaje otwarty. 4B-1 trwa: kroki 1–3 mają potwierdzenie zielonego CI Edwarda, krok 4 jest kandydatem do nowego CI/audytu. Aktualne wyniki i ograniczenia opisuje aktualizacja 1.15 oraz raport kroku 4. Nie deklarować PASS CLOSED desktopu ani całego 4B-1.

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
5. Po 4B-11: przyjęty Conscious Mode.
6. Po 4B-12–13: sprawdzone integracje zewnętrzne dostępne w danym środowisku.
7. Po 4B-14: domknięcie zakresu i odbiór wydania; Field Room pozostaje opcjonalny.

Conscious jest docelową opcją konfiguratora. W wersji udostępnianej przed 4B-11 pokazać jego dostępność uczciwie; nie oferować niezaimplementowanego trybu jako działającego. Messenger/A2A w 4A były odłożone poza podstawową betę, więc dostępność tych funkcji sprawdzamy przed 4B-13.

## Zasady wykonywania etapów

Przed każdym etapem ustalić dokładny stan źródeł i działającą bazę. Pracować nad jednym etapem, respektując jego granice. Po zakończeniu przekazać listę zmian, identyfikację źródeł, wyniki odpowiednich testów, instrukcję sprawdzenia, ograniczenia i następny etap. Rozdzielać wynik testu automatycznego, uruchomienia na komputerze i oceny wizualnej.

Sprawdzać niezależność profili, tajność kluczy, zachowanie języka źródła oraz brak niezamierzonych wywołań modeli. Gdy zmieniają się trwałe dane, sprawdzać migracje, backup/restore i eksport. Testy jednostkowe nie korzystają z płatnych API; testy rzeczywistych połączeń są oddzielne. Budowę podstawy prowadzić tak, aby w przyszłości można było wymienić provider, pamięć, skórkę i ConsciousCore.

Ten plan nie upoważnia do publikacji aplikacji, działań na produkcyjnym forum ani wysyłania wiadomości. Edward przyjął projekt P0 i upoważnił dalszą pracę; korekty P0 oraz audyt przygotowawczy P1 wykonano. P1 otrzymał PASS audytora przekazany przez Edwarda; P2 po korekcie audytowej 1.1 zostało przyjęte według ostatniego potwierdzenia Edwarda. Rozpoczęto i przekazano źródłowy kandydat 4B-0. Nie ogłaszać zamknięcia 4B-0 przed zaliczeniem jego otwartych bramek. Edward upoważnił start 4B-1; prace protokołów rozpoczęto, zachowując jawny otwarty status wcześniejszego odbioru.
