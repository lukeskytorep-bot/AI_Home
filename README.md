# AI Home — profiles, conversations and saved drafts

Stan 12.10.2026: kandydat 4B-2 krok 1 po korekcie audytu. Desktop dodaje rozmowy osobne dla każdego profilu i trwały zapis szkiców. Konfigurator, połączenia, klucze i Primary/Scribe pozostają dostępne. Przycisk Wyślij jest jeszcze wyłączony; odpowiedzi modelu należą do następnej części. AI Home nie wymaga forum.

EN jest domyślny przy pierwszym uruchomieniu; zapisany język jest zachowany. Skórki Orbital/Aurora obsługują light/dark/system. Browser preview nie zapisuje kluczy i nie wykonuje połączeń dostawców. Conscious pozostaje niedostępny.

## Uruchomienie i kontrole

Node.js 24: `npm ci`, `npm test`, `npm run verify:boundaries`, `npm run build`.
Browser preview: `npm run dev`, http://127.0.0.1:1420.
Desktop: Rust stable i wymagania systemowe Tauri, https://v2.tauri.app/start/prerequisites/ ; `npm run tauri dev`.
Natywne testy wraz z Tauri: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.
Headless Rust/SQLite: ten sam command z `--no-default-features`; nie sprawdza WebView ani desktop feature.

Cargo.lock jest dołączony i należy go commitować. Zwykłe CI sprawdza frontend i Rust/SQLite/Tauri na Ubuntu, bez tworzenia instalatorów. Istniejące ręczne `check_windows=true` dodaje Windows, a `build_desktop=true` przygotowuje pliki do odbioru desktopu. Nie publikuje release. Ta nakładka nie wymaga Prepare Cargo lockfile.

Linuxowy vault wymaga dostępnego i odblokowanego Secret Service; Windows używa Credential Manager, macOS Keychain. Błąd magazynu nie uruchamia fallbacku do pliku ani localStorage. Test połączenia pobiera modele; nie potwierdza jeszcze działania konkretnego modelu w czacie.

Natywny wykonawca ma ograniczony retry/recovery na jednym budżecie (4 wywołania/180 s). Test połączenia używa go przy pobieraniu modeli. Biblioteka czatu odrzuca odpowiedź uciętą limitem lub refusal; nie dopisuje kontynuacji. Organizacja rozmów i zapis szkiców są dostępne; wysyłanie do modelu jest kolejną częścią.

## Aktualne dowody

Rust Linux 107/107 potwierdzony dla wcześniejszej paczki przez audytora; kod native niezmieniony w korekcie. Ponownie lokalnie: 81/81 frontend, TypeScript/build, 19/19 granice i P2 193/193 PASS. Rzeczywisty OS vault, płatne API, Windows/Tauri/WebView i renderowanie Chromium pozostają niewykonane lokalnie. Wynik 4B-2 kroku 1 wymaga nowego CI oraz audytu. Całe 4B-1 i otwarte bramki Windows/WebView 4B-0 pozostają otwarte.

Raport: `docs/AI_HOME_4B2_KROK_1_RAPORT_PL.md`; plan: `docs/AI_HOME_4B_ETAPY_PRACY_v1.0_PL.md`; bramki: `docs/ACCEPTANCE_PL.md`. Dawne raporty 4B-1 kroków 1–4 i logi w `docs/evidence` zachowują stan swoich historycznych przekazań. Edward potwierdził zielone CI poprzednich kroków; nie przekazał identyfikatorów przebiegów ani zakresu platform.

## Kontrakty i inwentarz

P2 i foundation zachowano bajtowo. contracts/conversations.ipc.json dodaje tylko scoped odczyty conversation.get/list. Baza v3 → v4 dodaje rozmowy, wiadomości i cursory, zachowując wcześniejsze dane. `contracts/connections.ipc.json` dodaje tylko scoped odczyty cache modeli i ostatniego testu. Lokalna baza 1 → 2 dodaje konfigurację i trwałe operacje; 2 → 3 dodaje wyłącznie referencję klucza instalacji HMAC. Preferencje, profile, połączenia i receipts są zachowane. Nowe polecenia zapisujące klucz mają HMAC z lokalnym kluczem w vault; stare receipts są sprawdzane z oryginalnym sekretem i nie wykonują zapisu ponownie. Źródła Harness i adaptacje identyfikują pliki w `docs/reuse`, z zachowanym MIT notice.

`snapshots/p2-v1.1` jest pełnym zamrożonym przekazaniem P2 1.1. `python3 check_contracts.py` wykonuje jego 193 kontrole; 119 jest wynikiem historycznego P2, a 145 wynikało z wcześniejszego checkera na późniejszych kontraktach. Nie są to wyniki jednego zestawu. `snapshots/p2-legacy-mixed` jest historią.

`docs/evidence/manifest.sha256.json` i `SHA256SUMS.txt` opisują komplet aktualnego przekazania. W Windows: `py -3 scripts/verify-source.py .`; Linux: `python3 scripts/verify-source.py .`. Weryfikator przyjmuje także pełny SOURCE.zip pod dowolną nazwą. Po zmianach odnowić manifest przy kolejnym przekazaniu; dawne hashe nie są dowodem nowego stanu.
