# AI Home — 4B-1 krok 4: ograniczony native retry/recovery

Wersja 1.1 — korekta po audycie, 12 października 2026. Status: kandydat do istniejącego CI i audytu; całe 4B-1 pozostaje otwarte.

## Korekta audytu

Dwa dodatkowe testy odtworzyły oba błędy na pierwotnym completion.rs (17 testów executor PASS, 2 nowe FAIL). Po poprawce oba przypadki należą do stałego zestawu. Google STOP + widoczny tekst + functionCall zwraca OutputIncomplete i executor kończy po jednej próbie, bez sieci w tym teście: endpoint Google jest stały i nie został osłabiony dla loopback. Wadliwe tool_calls przy pustym tekście zwraca InvalidResponse; rzeczywisty loopback execute_chat wykonuje jeden POST, bez recovery. Dodano też testy niepoprawnego functionCall/args, null i pustej tablicy OpenAI-compatible oraz Anthropic tool_use przy end_turn.

Walidacja obecności i typu pól narzędziowych poprzedza rozpoznawanie pustej odpowiedzi. Google functionCall wymaga obiektu, nazwy i poprawnego typu opcjonalnego args. OpenAI-compatible tool_calls dopuszcza brak/null/pustą tablicę; inny typ lub element niebędący obiektem daje InvalidResponse. Niepusta tablica obiektów i legacy function_call wymagają dalszej obsługi aplikacji, więc dają OutputIncomplete. Anthropic tool_use jest wykrywany także przy end_turn. Ten etap nie wykonuje narzędzi. Żadne z tych terminalnych błędów nie jest kwalifikowane do retry/recovery.

Nowa nakładka jest skumulowanym, poprawionym krokiem 4 na bazie kroku 3 i zastępuje poprzednią paczkę. Kontrola dodatkowej różnicy względem pierwotnego SOURCE kroku 4 potwierdza, że kod produkcyjny zmienił się tylko w completion.rs; poza nim zmieniono testy, bieżące dokumenty i dowody. Logi pierwotnego kroku 4 (92 testy) pozostają historią; aktualne 97 testów i regresje przed poprawką są w docs/evidence/4b1-step-4-post-audit.

## Baza i zakres

Edward potwierdził zielone CI kroku 3 i polecił kolejny krok. Nie przekazano identyfikatora przebiegu ani platform. Nakładkę przygotowano na dokładnym poprzednim SOURCE.zip, SHA-256 `2933625c6d7800f2adcda33f065e0695b3b35bc7b4c7fba15a3b80ae1c2fa87a`. Nie usunięto plików repozytorium. Workflow, Cargo.toml, Cargo.lock, package.json/package-lock, uprawnienia, kontrakty P2/foundation/connections i snapshoty są identyczne bajtowo z bazą. Nie dodano zależności, jobów, release’ów ani instalatorów.

## Zachowanie

Native executor adaptuje klasyfikację i backoff Harness retry.ts do bezpiecznych enumów. HTTP 408/425/429/500/502/503/504/524/529 oraz Network/Timeout mogą otrzymać jeden retry transportowy na etap. BodyRead, InvalidJson i jawny EmptyResponse mogą otrzymać jedną próbę recovery, powtarzając niezmienione żądanie i otwierając nowy etap retry. Drugi taki błąd kończy wykonanie. Recovery nie jest semantyczną naprawą JSON ani kontynuacją tekstu.

Wszystkie fizyczne wywołania używają tego samego CallBudget: maksymalnie 4 i 180 s, a niższy limit użytkownika pozostaje obowiązujący. Każda strona discovery ma swój etap wykonawcy, ale ten sam globalny licznik/deadline; paginacja ich nie resetuje. Sequence 503 → invalid JSON → 503 → sukces ma dokładnie cztery rezerwacje. Backoff ma exponential full jitter 500 ms–8 s; Retry-After najwyżej 30 s. Oczekiwanie przekraczające pozostały deadline zostaje odrzucone, a anulowanie przerywa oczekiwanie.

CallRecorder nadal commituje rezerwację w SQLite przed wysłaniem HTTP. Native connection.test rzeczywiście używa bounded wykonawcy dla list modeli. Testy IPC potwierdzają durable physicalCallCount, zdarzenia i jeden końcowy wynik/cache. Publiczne execute_chat jest przygotowane dla dalszej implementacji rozmów; fizyczny chat jest prywatny dla modułu. Nie ma streamingu, callbacków z częściowym tekstem ani ponawiania już zwróconego sukcesu. Powtórzenie żądania po utracie odpowiedzi nie gwarantuje exactly-once po stronie zewnętrznego dostawcy.

Przed raw parserem sprawdzane są finish/stop reasons. Limit wyjścia, tool use i pause_turn dają OutputIncomplete; refusal/content_filter/safety dają PolicyBlocked. Unknown lub wadliwe metadata dają InvalidResponse. Brak/null reason zachowuje wcześniejsze zaakceptowane formaty. Tylko jawne obsługiwane puste kształty kwalifikują się do recovery; błędny schemat nie staje się pustą odpowiedzią. Limity/refusal nie są ponawiane, nie zwiększamy tokenów ani nie dopisujemy kontynuacji. Surowy Google parser i jego zachowanie podpisanego tekstu pozostają niezmienione; poprzednia regresja parser → replay nadal przechodzi.

## Korekta P2: HMAC credential receipts

Przegląd P2 ujawnił wymóg installation HMAC. Krok 3 usuwał sekret z metadanych fingerprintu i porównywał ponowienie z oryginalnym immutable wpisem vault; nie implementował HMAC. Krok 4 uzupełnia ten wymóg, adaptując HMAC-SHA256 z Harness secrets.rs bez dodatkowego crate; potwierdzono wektor RFC 4231.

Nowy receipt connection.create/connection.credential.set zapisuje `hmac-v1:<digest>` z całego polecenia, profilu, rewizji i payloadu. Lokalny 32-bajtowy klucz jest generowany z dwóch UUIDv4 pochodzących z CSPRNG, zapisany w oddzielnym immutable wpisie systemowego vault i odczytany ponownie. SQLite zawiera tylko opaque credential_ref. Klucz instalacji i API secret nie są zwracane do UI ani zapisywane w SQLite/manifestach. Baza v2 → v3 dodaje tabelę installation_hmac, zachowując wcześniejsze dane.

Stare receipts z kroku 3 są obsługiwane tylko odczytowo: zgodność metadata i dokładnego sekretu z oryginalnym bound wpisem vault jest wymagana; polecenie nie wykonuje się ponownie i receipt nie jest przepisywany. Jeśli zapisany master reference istnieje, ale vault nie zawiera klucza, system zwraca bezpieczny błąd zamiast regenerować klucz. Nieudany commit może pozostawić nieużyty immutable wpis vault; nie jest fallbackiem i nie czyścimy go automatycznie. Instalacja z istniejącymi danymi wymaga zachowania jej vault; kopiowanie samego SQLite nie jest pełną migracją sekretów.

## Wyniki lokalne

| Kontrola | Wynik i zakres |
| --- | --- |
| Rust Linux | 97/97 PASS: 65 unit/provider/vault, 24 connections SQLite/IPC, 8 foundation |
| Nowe testy Rust względem kroku 3 | 27: 5 korekt audytowych oraz 22 wcześniejsze: 17 executor/stop reasons, 1 HMAC RFC, 4 integracje retry/migracja/receipt |
| Frontend | 71/71 PASS, jsdom; kod UI bez zmian |
| TypeScript/Vite | PASS |
| Granice modułów | 18/18 PASS |
| Zamrożone P2 | 193/193 PASS, kontrakty bez zmian |
| Inwentarz i nałożenie | katalog/ZIP oraz odtworzenie na dokładnej bazie; log dostarczony osobno |

Rust: `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --locked --offline`, rustc 1.99.0. Aktualne logi znajdują się w docs/evidence/4b1-step-4-post-audit; wcześniejsze docs/evidence/4b1-step-4 są historyczne. Testy sieci używają wyłącznie loopback; vault jest wstrzykniętym magazynem testowym. Nie wywoływano płatnych API ani kluczy Edwarda.

## Pozostałe odbiory

Nowe CI/audyt kroku 4 są wymagane. Lokalnie nie uruchomiono Windows, desktop feature/Tauri/WebView, rzeczywistego OS vault ani live API. Dotychczasowe zielone CI nie jest dowodem tych nowych modułów. Windows/WebView 4B-0 i całe 4B-1 pozostają otwarte. Kolejne odbiory obejmują rzeczywisty vault oraz uzgodniony smoke dostawców; rozmowy/pamięć 4B-2 i późniejsze etapy pozostają nierozpoczęte.

## Źródła

Harness ZIP SHA-256 `534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e`: src/providers/retry.ts oraz src-tauri/src/secrets.rs; hashe/adaptacje w docs/reuse. MIT notice zachowany.

FinishReason Google: https://ai.google.dev/api/generate-content
Stop reasons Anthropic: https://platform.claude.com/docs/en/build-with-claude/handling-stop-reasons
P2: snapshots/p2-v1.1 (wymóg HMAC, retry i budżetu).

Function calling Google (sprawdzone przy korekcie): https://ai.google.dev/gemini-api/docs/function-calling
