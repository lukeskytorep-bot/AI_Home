# AI Home — fundament 4B-0

Stan 12 października 2026: baza aplikacji z React/TypeScript + Rust/Tauri + SQLite. EN domyślnie, PL, Orbital/Aurora, light/dark/system. Profile, AI API i pamięć należą do kolejnych etapów. Forum nie jest wymagane.

## Uruchomienie i kontrole

Node.js 24; `npm ci`, `npm test`, `npm run verify:boundaries`, `npm run build`.
Browser preview: `npm run dev`, http://127.0.0.1:1420. Preview ma własny localStorage; nie sprawdza SQLite/WebView.
Desktop: Rust stable i wymagania systemowe Tauri, https://v2.tauri.app/start/prerequisites/ . `npm run tauri dev`. `src-tauri/Cargo.lock` jest już zachowany w repozytorium. Testy natywne: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked`.

Zwykłe CI: frontend i Rust/SQLite/Tauri na Ubuntu, bez artefaktów do pobrania. Ręcznie `check_windows=true` dodaje Windows, a `build_desktop=true` dodaje Windows i tworzy pliki do sprawdzenia. Nie publikuje release ani instalatorów. Prepare Cargo lockfile jest potrzebne wyłącznie do uzasadnionych zmian zależności, nie przy każdym etapie.

## Jedna wersja P2

`snapshots/p2-v1.1` zawiera komplet odebranego przekazania P2 1.1, w tym build_audit_fixtures.py, własne kontrakty i oryginalny SHA256SUMS.txt. `python3 check_contracts.py` sprawdza zgodność kontraktów aplikacji z tym snapshotem i uruchamia 193 kontrole. 119 dotyczy historycznej wersji P2, 145 wynikało ze starszego skryptu uruchamianego na nowych kontraktach. Nie są to wyniki tego samego zestawu testów. Materiał mieszany przeniesiono do snapshots/p2-legacy-mixed; jego hashe nie oznaczają zgodności obecnej aplikacji.

## Dowody i status

`docs/evidence/manifest.sha256.json` i główny `SHA256SUMS.txt` identyfikują drzewa tego przekazania. Manifest obejmuje wszystkie pliki źródłowe i dowodowe poza sobą i SHA256SUMS; SHA256SUMS obejmuje także manifest, poza samym sobą. `python3 scripts/verify-source.py .` sprawdza kompletność i hashe; ten sam skrypt przyjmuje pełny ZIP. Nazwa pobranego ZIP-a jest dowolna. Po kolejnych zmianach manifest odświeżać przy następnym przekazaniu; nie przedstawiać dawnych hashy jako dowodów nowego stanu.

Edward potwierdził udane CI po poprawce async. Brakuje identyfikatora przebiegu i zakresu platform, dlatego zapisujemy potwierdzenie użytkownika, nie niezależnie zweryfikowany PASS wszystkich systemów. Windows, wygenerowane uprawnienia i rzeczywisty WebView pozostają do odbioru. 4B-0 otwarty; 4B-1 rozpoczęty od przeniesienia biblioteki protokołów, bez zamknięcia całego etapu. Szczegóły: docs/ACCEPTANCE_PL.md i docs/AI_HOME_4B0_FUNDAMENT_RAPORT_PL.md.

## Rozpoczęte 4B-1

Pierwszy krok przenosi bibliotekę formatów żądań/odpowiedzi oraz retry z Harness. Nie wykonuje jeszcze HTTP ani nie przechowuje kluczy. `docs/AI_HOME_4B1_KROK_1_RAPORT_PL.md` opisuje zakres, testy i następny krok. Cargo.lock i codzienne CI pozostają bez zmian.

## 4B-1 krok 1 — korekta audytowa

Korekta na dokładnej bazie AI_Home-main (1).zip: zgodny pełny inwentarz, dołączony weryfikator i zachowany podpisany tekst Google. Dodano test parser → replay; 15 testów Rust czeka na wykonanie w istniejącym CI. Szczegóły i hash bazy: docs/AI_HOME_4B1_KROK_1_RAPORT_PL.md. Całe 4B-1 pozostaje otwarte.
