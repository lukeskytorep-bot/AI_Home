# AI Home — profiles, conversations and saved drafts

Stan 20.10.2026: kandydat 4B-2A krok 1 — lokalny import dokumentów, przed nowym CI/audytem. Profile i rozmowy mają trwałe szkice, wykonywalny czat tekstowy z modelem Primary, historię, status operacji i STOP. Po wybraniu Primary/zweryfikowanego połączenia użytkownik musi zadeklarować okno kontekstu modelu. Dokumenty TXT/MD/DOCX/tekstowy PDF można importować i przeglądać lokalnie w rozmowie. Treści plików NIE są jeszcze dołączane do wywołań modelu. Obrazy, OCR, pamięć i streaming pozostają kolejnymi etapami. AI Home nie wymaga forum.

EN jest domyślny przy pierwszym uruchomieniu; zapisany język jest zachowany. Skórki Orbital/Aurora obsługują light/dark/system. Browser preview nie zapisuje kluczy i nie wykonuje połączeń dostawców. Conscious pozostaje niedostępny.

## Uruchomienie i kontrole

Node.js 24: `npm ci`, `npm test`, `npm run verify:boundaries`, `npm run build`.
Browser preview: `npm run dev`, http://127.0.0.1:1420.
Desktop: Rust stable i wymagania systemowe Tauri, https://v2.tauri.app/start/prerequisites/ ; `npm run tauri dev`.
Natywne testy wraz z Tauri: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.
Headless Rust/SQLite: ten sam command z `--no-default-features`; nie sprawdza WebView ani desktop feature.

Cargo.lock jest dołączony i należy go commitować. Zwykłe CI sprawdza frontend i Rust/SQLite/Tauri na Ubuntu, bez tworzenia instalatorów. Istniejące ręczne `check_windows=true` dodaje Windows, a `build_desktop=true` przygotowuje pliki do odbioru desktopu. Nie publikuje release. Ta nakładka nie wymaga Prepare Cargo lockfile.

Linuxowy vault wymaga dostępnego i odblokowanego Secret Service; Windows używa Credential Manager, macOS Keychain. Błąd magazynu nie uruchamia fallbacku do pliku ani localStorage. Test połączenia pobiera modele; nie potwierdza jeszcze działania konkretnego modelu w czacie.

Natywny wykonawca ma ograniczony retry/recovery na jednym budżecie (4 wywołania/180 s). Test połączenia używa go przy pobieraniu modeli. Biblioteka czatu odrzuca odpowiedź uciętą limitem lub refusal; nie dopisuje kontynuacji. Organizacja rozmów i zapis szkiców są dostępne; wysyłanie do modelu jest dostępne w kroku 2.

## Aktualne dowody

Aktualny 4B-2A krok 1: Rust Linux 146/146 PASS bez desktop feature; frontend 103/103 PASS, TypeScript/build, 21/21 granice i P2 193/193 PASS. Rzeczywisty OS vault, płatne API, Windows/Tauri/WebView i renderowanie Chromium pozostają niewykonane lokalnie. Edward potwierdził CI korekty 4B-2 kroku 2; ponowny audyt nadal otwarty. Nowy krok 4B-2A wymaga CI i audytu. Całe 4B-1 i otwarte bramki Windows/WebView 4B-0 pozostają otwarte.

Raport: `docs/AI_HOME_4B2_KROK_2_RAPORT_PL.md`; plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md`; bramki: `docs/ACCEPTANCE_PL.md`. Dawne raporty 4B-1 kroków 1–4 i logi w `docs/evidence` zachowują stan swoich historycznych przekazań. Edward potwierdził zielone CI poprzednich kroków; nie przekazał identyfikatorów przebiegów ani zakresu platform.

Nowa baza v4 → v5 dodaje trwałe operacje czatu i prywatny replay. contracts/chat.ipc.json dodaje jawne ustawienia kontekstu i odczyt ostatniego chat. Pomiar kontekstu jest szacowany, limit zadeklarowany przez użytkownika. Cała historia jest uwzględniana albo żądanie odrzucone; nic nie jest cicho skracane. Efektywne wyjście to minimum limitu trasy i profilu (domyślnie 1024), widoczne w panelu. Restart nie ponawia płatnych wywołań.

## Kontrakty i inwentarz

P2 i foundation zachowano bajtowo. contracts/conversations.ipc.json dodaje tylko scoped odczyty conversation.get/list. Baza v3 → v4 dodaje rozmowy, wiadomości i cursory, zachowując wcześniejsze dane. `contracts/connections.ipc.json` dodaje tylko scoped odczyty cache modeli i ostatniego testu. Lokalna baza 1 → 2 dodaje konfigurację i trwałe operacje; 2 → 3 dodaje wyłącznie referencję klucza instalacji HMAC. Preferencje, profile, połączenia i receipts są zachowane. Nowe polecenia zapisujące klucz mają HMAC z lokalnym kluczem w vault; stare receipts są sprawdzane z oryginalnym sekretem i nie wykonują zapisu ponownie. Źródła Harness i adaptacje identyfikują pliki w `docs/reuse`, z zachowanym MIT notice.

`snapshots/p2-v1.1` jest pełnym zamrożonym przekazaniem P2 1.1. `python3 check_contracts.py` wykonuje jego 193 kontrole; 119 jest wynikiem historycznego P2, a 145 wynikało z wcześniejszego checkera na późniejszych kontraktach. Nie są to wyniki jednego zestawu. `snapshots/p2-legacy-mixed` jest historią.

`docs/evidence/manifest.sha256.json` i `SHA256SUMS.txt` opisują komplet aktualnego przekazania. W Windows: `py -3 scripts/verify-source.py .`; Linux: `python3 scripts/verify-source.py .`. Weryfikator przyjmuje także pełny SOURCE.zip pod dowolną nazwą. Po zmianach odnowić manifest przy kolejnym przekazaniu; dawne hashe nie są dowodem nowego stanu.

Korekta audytu: Accepted rozlicza szkic także po przełączeniu rozmowy lub błędzie historii; polling zachowuje aktualną rewizję. Załączniki pozostają niedostępne i zwracają kontraktowy VALIDATION_ERROR. Aktualne logi: `docs/evidence/4b2-step-2-correction/`.

Import dokumentów: natywny wybór, oryginał i tekst w SQLite v6, osobne autorstwo/importer, hash, podgląd, pobranie, odłączenie i trwała operacja ze STOP/restartem. Raport: `docs/AI_HOME_4B2A_KROK_1_RAPORT_PL.md`; aktualne logi: `docs/evidence/4b2a-step-1/`. Zależności importu i Cargo.lock dodano bez usuwania wcześniejszych wersji pakietów. Dotychczasowy workflow pozostaje bez zmian.
