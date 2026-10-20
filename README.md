# AI Home

Stan 21.10.2026: kandydat 4B-2A krok 3 — dokumenty i statyczne obrazy w czacie Primary. Edward potwierdził zielony wynik kroku 2; krok 3 wymaga nowego CI i audytu.

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

169/169 Rust Linux headless, 113/113 frontend, TypeScript/Vite, 21/21 granice i 193/193 kontrole referencyjne P2 PASS. Pełny Rust jest wykonany w domyślnym trybie równoległym. Wcześniejsze nieudane uruchomienia i poprawki fixture/asercji zachowano w dowodach. Natywny WebView/Windows, rzeczywisty vault i live API pozostają nieodebrane.

Raport: `docs/AI_HOME_4B2A_KROK_3_RAPORT_PL.md`; aktualny plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` (treść 1.22); odbiór: `docs/ACCEPTANCE_PL.md`; logi: `docs/evidence/4b2a-step-3/`. Starsze raporty i logi są historyczne.

## Kontrakty i przekazanie

Zamrożone P2/foundation i wcześniejsze katalogi poza własnym document-chat.ipc.json pozostają bajtowo zgodne z bazą. document-chat.ipc.json rozszerza wynik preflight o obrazy; vision.ipc.json dodaje osobne polecenia deklaracji. `contracts/document-chat.ipc.json` dodaje tylko odczyt `chat.preflight`; `chat.send` wykorzystuje istniejące P2 attachmentIds. Szablony żądań providerów i vault pozostają bez zmian; walidator protokołu odrzuca obce MIME/role i wadliwe base64. Dodano image =0.25.10 z kodekami PNG/JPEG/GIF/WebP. Zależności frontendu i uprawnienia są bez zmian. Parsery importu i ich MIT pochodzą z adaptacji Harness w kroku 1; szczegóły w docs/reuse.

`snapshots/p2-v1.1` przechowuje pełne P2; `python3 check_contracts.py` wykonuje 193 kontrole. 119 i 145 są wynikami innych historycznych rewizji, nie tego samego zestawu. Migawki nie są odnawiane wraz z kodem aplikacji.

Aktualne `docs/evidence/manifest.sha256.json` i `SHA256SUMS.txt` opisują komplet tego przekazania. Opcjonalnie: Windows `py -3 scripts/verify-source.py .`, Linux `python3 scripts/verify-source.py .`. Skrypt akceptuje również pełny SOURCE.zip pod dowolną nazwą. Po własnych zmianach stare hashe nie są dowodem nowego drzewa.

## Vision

Vision domyślnie ma stan unknown. `chat.vision.get/set` przechowuje jawną deklarację użytkownika dla dokładnego modelu, rewizji trasy, połączenia, endpointu i rewizji klucza. To nie jest atestacja API; sprawdź możliwości wybranego modelu przed deklaracją. Zmiana tego zakresu unieważnia deklarację. Zmiana samego okna kontekstu przenosi nadal ważną deklarację na nową rewizję trasy. Zmiana deklaracji w trakcie żądania blokuje spóźniony wynik. Obrazy zachowane w historii również wymagają vision.

Import: do 10 MiB/obraz, 16 384 px/bok i 40 mln pikseli, dodatkowo limity dekodera. Obrazy są przechowywane bez tekstu ekstrakcji; statyczne GIF są obsługiwane poza Google. APNG, animowany GIF i animowany WebP są odrzucane. SVG/HEIC/HEIF nie są importowane. Brak automatycznej konwersji, kompresji, korekty orientacji lub usuwania EXIF. Podgląd korzysta z lokalnego Blob URL i zwalnia go po zamknięciu lub zmianie rozmowy.

Preflight obrazu ma metodę `utf8_bytes_and_image_reserve_estimate`: długość base64 +8192 +4*ceil(piksele/256) dla każdego obrazu, plus istniejący tekst/replay/wyjście/rezerwa. To heurystyka, nie licznik dostawcy ani gwarancja pojemności modelu. Pomiar wskazuje ID i rewizję trasy oraz rewizję deklaracji vision. W całym żądaniu jest najwyżej 20 obrazów, także z historii; nic nie jest po cichu pomijane. Google GIF jest odrzucany przed POST. Natywny Anthropic ma dodatkowy limit 8000 px/bok i 10 000 000 bajtów base64. Inne ograniczenia konkretnego modelu/endpointu mogą wymagać osobnego odbioru.

Po CI/audycie kroku 3 kolejnym etapem funkcjonalnym jest 4B-3 — Scribe i Working Context. Nie zamknięto bramek realnego vault/API ani desktopu.
