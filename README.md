# AI Home

Stan 21.10.2026: kandydat 4B-2A krok 2 — rozmowa z Primary o zaimportowanych dokumentach. Edward potwierdził zielone CI kroku 1; krok 2 wymaga nowego CI i audytu.

Aplikacja ma osobne profile, połączenia i trasy Primary/Scribe, trwałe rozmowy i szkice, historię, operacje oraz STOP. Import TXT/MD/DOCX/tekstowego PDF zachowuje oryginały, odczytany tekst i osobne autorstwo/importera. Zaznaczone gotowe dokumenty są wysyłane w całości jako niezaufane dane użytkownika wraz z metadanymi pochodzenia do skonfigurowanego dostawcy. Oryginały pozostają lokalne. Dostarczone dokumenty zostają związane z wiadomością; kolejne tury uwzględniają je z historii także po restarcie. Referencje są dowodem przekazania źródeł, nie zweryfikowanymi cytowaniami odpowiedzi. Nie ma automatycznej promocji do pamięci.

EN jest domyślny; PL i skórki Orbital/Aurora pozostają dostępne. AI Home działa niezależnie od forum. Browser preview nie wykonuje połączeń dostawców ani natywnego importu. Obrazy/vision, OCR, RAG, pamięć, streaming i Conscious są dalszymi etapami.

## Uruchomienie i kontrole

Node.js 24: `npm ci`, `npm test`, `npm run verify:boundaries`, `npm run build`.
Browser preview: `npm run dev`, http://127.0.0.1:1420.
Desktop: Rust stable i wymagania Tauri https://v2.tauri.app/start/prerequisites/ ; `npm run tauri dev`.
Rust/SQLite/Tauri: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.
Headless: ten sam command z `--no-default-features`; nie sprawdza desktop feature/WebView.

Istniejące CI na push sprawdza frontend i Rust/SQLite/Tauri na Ubuntu. Workflow i Cargo.lock pozostają bez zmian. Nie są potrzebne Prepare Cargo lockfile, nowy release ani instalatory. Ręczne `check_windows=true` dodaje kontrolę Windows; `build_desktop=true` służy oddzielnemu odbiorowi desktopu.

Klucze korzystają z OS vault, bez fallbacku do pliku/localStorage. Linux wymaga odblokowanego Secret Service. Test połączenia pobiera modele; użycie konkretnego modelu wymaga jeszcze sprawdzenia czatu. Testy używają wstrzykiwanego vault i loopback HTTP, bez płatnych API.

## Kontekst i trwałość

Użytkownik jawnie deklaruje okno kontekstu dla ID/rewizji trasy Primary. `chat.preflight` pokazuje szacunek wejścia oraz limit wyjścia i rezerwę. `chat.send` powtarza dokładnie tę samą walidację przed zapisem/HTTP. Liczone są wszystkie wiadomości, pełny tekst dokumentów, metadane i prywatny replay. Pomiar to bajty UTF-8 z narzutem, nie tokenizer modelu. Przekroczenie limitu zatrzymuje wysyłkę; nic nie jest cicho obcinane.

SQLite v7 dodaje `message_documents` do v6. Snapshoty metadanych materiału zawierają rewizję, blob i hash ekstrakcji; Message.sourceRefs pozostają zgodne z P2. Odczyt historii weryfikuje zgodność snapshotów i hashy. Związanych załączników nie można odłączyć tym panelem ani przypiąć ponownie. Niewysłane można odłączyć, zachowując oryginał. SQLite przechowuje treści jawnie; szyfrowanie i permanentne usuwanie nie są tutaj implementowane.

Niepewna próba wysyłania zachowuje ID, tekst i wybór do rozliczenia; nowszy szkic i wybór nie zmieniają starej próby. STOP zachowuje wiadomość człowieka i źródła; restart nie ponawia sieci. Istniejące ograniczenia retry/budżetu pozostają bez zmian.

## Dowody i stan

153/153 Rust Linux headless, 109/109 frontend, TypeScript/Vite, 21/21 granice i 193/193 kontrole referencyjne P2 PASS. Pełny Rust jest wykonany w domyślnym trybie równoległym. Wcześniejsze nieudane uruchomienia i poprawki fixture/asercji zachowano w dowodach. Natywny WebView/Windows, rzeczywisty vault i live API pozostają nieodebrane.

Raport: `docs/AI_HOME_4B2A_KROK_2_RAPORT_PL.md`; aktualny plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md` (treść 1.21); odbiór: `docs/ACCEPTANCE_PL.md`; logi: `docs/evidence/4b2a-step-2/`. Starsze raporty i logi są historyczne.

## Kontrakty i przekazanie

Zamrożone P2/foundation i dotychczasowe katalogi pozostają bajtowo zgodne z bazą. `contracts/document-chat.ipc.json` dodaje tylko odczyt `chat.preflight`; `chat.send` wykorzystuje istniejące P2 attachmentIds. Providerzy, vault, zależności, lockfile i uprawnienia są bez zmian. Parsery importu i ich MIT pochodzą z adaptacji Harness w kroku 1; szczegóły w docs/reuse.

`snapshots/p2-v1.1` przechowuje pełne P2; `python3 check_contracts.py` wykonuje 193 kontrole. 119 i 145 są wynikami innych historycznych rewizji, nie tego samego zestawu. Migawki nie są odnawiane wraz z kodem aplikacji.

Aktualne `docs/evidence/manifest.sha256.json` i `SHA256SUMS.txt` opisują komplet tego przekazania. Opcjonalnie: Windows `py -3 scripts/verify-source.py .`, Linux `python3 scripts/verify-source.py .`. Skrypt akceptuje również pełny SOURCE.zip pod dowolną nazwą. Po własnych zmianach stare hashe nie są dowodem nowego drzewa.
