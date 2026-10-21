# Bieżący krok: 4B-3 krok 2 — trwała pamięć i panel

W wybranej rozmowie jest panel Working Context. Zapisz pojemność oraz jawne okna kontekstu modeli i wybierz Scribe albo wyłącz go, aby Primary proponował oraz osobno decydował. Przycisk Update memory from this conversation uruchamia modele na tekstach wiadomości. Domyślne L2: 300 szacowanych tokenów; presety/własna wartość/unlimited z ostrzeżeniem. Panel pokazuje próby, decyzję i ostatnią zapisaną wersję.

**Pamięć jest już trwale zapisywana, ale jeszcze nie jest dołączana do żądań czatu — to krok 3.** Treści plików i obrazy nie są czytane przez Scribe w tym kroku. Nie ma automatycznych wywołań modeli po otwarciu panelu. STOP/restart/błąd/konflikt nie zastępuje starej pamięci pustym zapisem. Decyzja jest odpowiedzią Primary, nie zatwierdzeniem przez człowieka.

SQLite migruje automatycznie v8→v9. Nie uruchamiaj cleanup ani Prepare Cargo lockfile. Zależności, zamrożone kontrakty, workflow i uprawnienia pozostają bez zmian. Wystarczy istniejące CI na push; bez nowego release/instalatorów.

228/228 Rust Linux headless, 130/130 frontend, TypeScript/Vite, 23/23 granice i 193/193 P2 PASS. Raport: docs/AI_HOME_4B3_KROK_2_RAPORT_PL.md; logi docs/evidence/4b3-step-2; plan treść 1.25. Nowe CI/audyt kroku pozostają wymagane. Realne API/vault/Windows/WebView to osobne odbiory.

Poniżej historyczny opis bazy kroku 1 oraz wcześniejszych etapów. Aktualny stan określa nagłówek powyżej, raport i plan 1.25.

---

# Bieżący krok: 4B-3 krok 1 — protokół pamięci

Dodano czysty silnik Rust dla propozycji Scribe i decyzji Primary (ACCEPT/MODIFY/REJECT), dwóch prób pojemności i jednej naprawy formatu. Gdy Scribe jest wyłączony, Primary proponuje i podejmuje osobną decyzję. Obsługiwane 300/500/800/1000/własna pojemność i unlimited. Pomiar jest szacunkiem UTF-8 bytes, związanym z ID/rewizją Primary; nie jest tokenizerem.

**To krok przygotowawczy: pamięć nie jest jeszcze podłączona do UI, HTTP ani trwałej bazy.** Nie zmieniono dotychczasowego działania czatu/vision. Silnik daje tylko kandydaturę, nie aktywną wersję. Trwałe operacje, atomowy commit i panel powstaną w kroku 2; później aktywny L2 wejdzie do preflight/send.

200/200 Rust Linux headless (29 nowych testów pamięci), 115/115 frontend, TypeScript/Vite, 22/22 granice, 193/193 P2 PASS. Raport: docs/AI_HOME_4B3_KROK_1_RAPORT_PL.md; logi docs/evidence/4b3-step-1; plan treść 1.24. Kontrakty, SQLite v8, zależności/lockfile, workflow i capabilities bez zmian. Wystarcza istniejące CI; bez nowych release/instalatorów. Realne API/vault/Windows/WebView pozostają osobnymi odbiorami.

Poniżej opis funkcjonalnej bazy 4B-2A; wcześniejsze wyniki/liczniki dotyczą tamtego przekazania.

---

# AI Home

Stan 21.10.2026: korekta audytu 4B-2A kroku 3. Audyt zwrócił DO KOREKTY z błędem spóźnionego zapisu vision po zmianie połączenia. Poprawka wymaga nowego CI i ponownego audytu.

Aplikacja ma osobne profile, połączenia i trasy Primary/Scribe, trwałe rozmowy i szkice, historię, operacje oraz STOP. Import TXT/MD/DOCX/tekstowego PDF zachowuje oryginały, odczytany tekst i osobne autorstwo/importera. Zaznaczone gotowe dokumenty są wysyłane w całości jako niezaufane dane użytkownika wraz z metadanymi pochodzenia do skonfigurowanego dostawcy. Oryginały dokumentów pozostają lokalne. Dla obrazów wysyłane są oryginalne bajty (wraz z ewentualnymi metadanymi), bez lokalnej konwersji lub OCR. Dostarczone dokumenty zostają związane z wiadomością; kolejne tury uwzględniają je z historii także po restarcie. Referencje są dowodem przekazania źródeł, nie zweryfikowanymi cytowaniami odpowiedzi. Nie ma automatycznej promocji do pamięci.

EN jest domyślny; PL i skórki Orbital/Aurora pozostają dostępne. AI Home działa niezależnie od forum. Browser preview nie wykonuje połączeń dostawców ani natywnego importu. Import i podgląd statycznych PNG/JPEG/WebP/GIF oraz wysyłanie vision są dostępne. Animacje są odrzucane jawnie. OCR, RAG, pamięć, streaming i Conscious są dalszymi etapami.

## Uruchomienie i kontrole

Node.js 24: `npm ci`, `npm test`, `npm run verify:boundaries`, `npm run build`.
Browser preview: `npm run dev`, http://127.0.0.1:1420.
Desktop: Rust stable i wymagania Tauri https://v2.tauri.app/start/prerequisites/ ; `npm run tauri dev`.
Rust/SQLite/Tauri: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.
Headless: ten sam command z `--no-default-features`; nie sprawdza desktop feature/WebView.

Istniejące CI na push sprawdza frontend i Rust/SQLite/Tauri na Ubuntu. Workflow pozostaje bez zmian. Cargo.lock zawiera 10 nowych wpisów kodeków obrazu, bez zmiany wcześniejszych wersji pakietów. Nie są potrzebne Prepare Cargo lockfile, nowy release ani instalatory. Ręczne `check_windows=true` dodaje kontrolę Windows; `build_desktop=true` służy oddzielnemu odbiorowi desktopu.

Klucze korzystają z OS vault, bez fallbacku do pliku/localStorage. Linux wymaga odblokowanego Secret Service. Test połączenia pobiera modele; użycie konkretnego modelu wymaga jeszcze sprawdzenia czatu. Testy używają wstrzykiwanego vault i loopback HTTP, bez płatnych API.

## Kontekst i trwałość

Użytkownik jawnie deklaruje okno kontekstu dla ID/rewizji trasy Primary. `chat.preflight` pokazuje szacunek wejścia oraz limit wyjścia i rezerwę. `chat.send` powtarza dokładnie tę samą walidację przed zapisem/HTTP. Liczone są wszystkie wiadomości, pełny tekst dokumentów, metadane i prywatny replay. Pomiar to bajty UTF-8 z narzutem, nie tokenizer modelu. Przekroczenie limitu zatrzymuje wysyłkę; nic nie jest cicho obcinane.

SQLite v8 dodaje `chat_vision_policies` do v7. Poprzednia migracja v6 → v7 dodała `message_documents`. Snapshoty metadanych materiału zawierają rewizję, blob i hash ekstrakcji dokumentu albo oryginału obrazu; Message.sourceRefs pozostają zgodne z P2. Odczyt historii weryfikuje zgodność snapshotów i hashy. Związanych załączników nie można odłączyć tym panelem ani przypiąć ponownie. Niewysłane można odłączyć, zachowując oryginał. SQLite przechowuje treści jawnie; szyfrowanie i permanentne usuwanie nie są tutaj implementowane.

Niepewna próba wysyłania zachowuje ID, tekst i wybór do rozliczenia; nowszy szkic i wybór nie zmieniają starej próby. STOP zachowuje wiadomość człowieka i źródła; restart nie ponawia sieci. Istniejące ograniczenia retry/budżetu pozostają bez zmian.

## Dowody i stan

171/171 Rust Linux headless, 115/115 frontend, TypeScript/Vite, 21/21 granice i 193/193 kontrole referencyjne P2 PASS. Pełny Rust jest wykonany w domyślnym trybie równoległym. Wcześniejsze nieudane uruchomienia i poprawki fixture/asercji zachowano w dowodach. Natywny WebView/Windows, rzeczywisty vault i live API pozostają nieodebrane.

Raport: `docs/AI_HOME_4B2A_KROK_3_AUDIT_FIX_RAPORT_PL.md`; aktualny plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` (treść 1.23); odbiór: `docs/ACCEPTANCE_PL.md`; logi: `docs/evidence/4b2a-step-3-correction/`. Starsze raporty i logi są historyczne.

## Kontrakty i przekazanie

Zamrożone P2/foundation i wcześniejsze katalogi poza własnym vision.ipc.json pozostają bajtowo zgodne z bazą kroku 3. document-chat.ipc.json rozszerza wynik preflight o obrazy; vision.ipc.json dodaje osobne polecenia deklaracji. `contracts/document-chat.ipc.json` dodaje tylko odczyt `chat.preflight`; `chat.send` wykorzystuje istniejące P2 attachmentIds. Szablony żądań providerów i vault pozostają bez zmian; walidator protokołu odrzuca obce MIME/role i wadliwe base64. Dodano image =0.25.10 z kodekami PNG/JPEG/GIF/WebP. Zależności frontendu i uprawnienia są bez zmian. Parsery importu i ich MIT pochodzą z adaptacji Harness w kroku 1; szczegóły w docs/reuse.

`snapshots/p2-v1.1` przechowuje pełne P2; `python3 check_contracts.py` wykonuje 193 kontrole. 119 i 145 są wynikami innych historycznych rewizji, nie tego samego zestawu. Migawki nie są odnawiane wraz z kodem aplikacji.

Aktualne `docs/evidence/manifest.sha256.json` i `SHA256SUMS.txt` opisują komplet tego przekazania. Opcjonalnie: Windows `py -3 scripts/verify-source.py .`, Linux `python3 scripts/verify-source.py .`. Skrypt akceptuje również pełny SOURCE.zip pod dowolną nazwą. Po własnych zmianach stare hashe nie są dowodem nowego drzewa.

## Vision

Vision domyślnie ma stan unknown. `chat.vision.get/set` przechowuje jawną deklarację użytkownika dla dokładnego modelu, rewizji trasy, połączenia, endpointu i rewizji klucza. To nie jest atestacja API; sprawdź możliwości wybranego modelu przed deklaracją. Zmiana tego zakresu unieważnia deklarację. Zmiana samego okna kontekstu przenosi nadal ważną deklarację na nową rewizję trasy. Zmiana deklaracji w trakcie żądania blokuje spóźniony wynik. Obrazy zachowane w historii również wymagają vision.

Import: do 10 MiB/obraz, 16 384 px/bok i 40 mln pikseli, dodatkowo limity dekodera. Obrazy są przechowywane bez tekstu ekstrakcji; statyczne GIF są obsługiwane poza Google. APNG, animowany GIF i animowany WebP są odrzucane. SVG/HEIC/HEIF nie są importowane. Brak automatycznej konwersji, kompresji, korekty orientacji lub usuwania EXIF. Podgląd korzysta z lokalnego Blob URL i zwalnia go po zamknięciu lub zmianie rozmowy.

Preflight obrazu ma metodę `utf8_bytes_and_image_reserve_estimate`: długość base64 +8192 +4*ceil(piksele/256) dla każdego obrazu, plus istniejący tekst/replay/wyjście/rezerwa. To heurystyka, nie licznik dostawcy ani gwarancja pojemności modelu. Pomiar wskazuje ID i rewizję trasy oraz rewizję deklaracji vision. W całym żądaniu jest najwyżej 20 obrazów, także z historii; nic nie jest po cichu pomijane. Google GIF jest odrzucany przed POST. Natywny Anthropic ma dodatkowy limit 8000 px/bok i 10 000 000 bajtów base64. Inne ograniczenia konkretnego modelu/endpointu mogą wymagać osobnego odbioru.

Po CI/audycie kroku 3 kolejnym etapem funkcjonalnym jest 4B-3 — Scribe i Working Context. Nie zamknięto bramek realnego vault/API ani desktopu.

## Korekta deklaracji vision

vision.get zwraca wymagany scopeId — niejawny w UI, publiczny identyfikator pełnego bieżącego zakresu. vision.set wymaga go wraz z policyRevision i expectedRevision trasy. Hash obejmuje wersjonowaną tożsamość profilu, trasy/modelu, połączenia/endpointu i rewizji klucza; nie zawiera surowego klucza. Native porównuje identyfikator w transakcji przed zapisem. Stary panel po zmianie połączenia/klucza i udanym ponownym teście dostaje REVISION_CONFLICT; nie może przenieść deklaracji na nowy zakres. Po konflikcie UI odświeża deklarację i wymaga nowej decyzji. Stary payload bez scopeId jest niepoprawny; nie zgadujemy aktualnego zakresu. Baza, dane i starsze snapshoty nie wymagają migracji.

W CSP i devCsp dodano blob: tylko do img-src dla podglądu obrazu. Test sprawdza konfigurację; nie dowodzi renderowania w realnym WebView. Nie rozszerzono script-src/connect-src ani capabilities. Raporty/logi z pierwotnego kroku 3 pozostają historyczne.
