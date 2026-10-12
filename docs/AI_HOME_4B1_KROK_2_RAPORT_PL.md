# AI Home — 4B-1 krok 2: natywny HTTP i listy modeli

Wersja treści 1.0 · 12 października 2026 · Kandydat do istniejącego CI

**Wykonano transport HTTP i discovery w module natywnym. Całe 4B-1 pozostaje otwarte.** Interfejs nie korzysta jeszcze z tych funkcji; nie jest to gotowy konfigurator ani trwały connection.test.

## Podstawa i status kroku 1

Edward potwierdził 12.10.2026, że poprawiony krok 1 przeszedł CI na zielono, i upoważnił kolejny krok. Nie przekazano numeru przebiegu ani nowego ZIP-a repozytorium. Bazą tej nakładki jest dokładne pełne źródło poprzedniej poprawki: AI_HOME_4B1_KROK_1_SOURCE.zip, SHA-256 `0e4228fa2e51cf7e5602b1203b760c8439530e3208e9379429b6e54380b71568`. Zakładamy brak innych zmian po tej poprawce. Paczka kroku 2 zawiera wyłącznie różnice względem tej bazy. Sprawdzono odtworzenie pełnego drzewa przez nałożenie paczki na bazę.

## Implementacja

- HttpTransport.chat łączy rzeczywisty natywny POST z odebranymi builderami/parserami kroku 1. Klucz jest wysyłany w nagłówku: Bearer dla OpenAI/OpenRouter/Custom, x-goog-api-key dla Google, x-api-key oraz anthropic-version dla Anthropic. Nagłówki kluczy są sensitive; Google nie umieszcza klucza w URL. OpenRouter otrzymuje nazwę AI Home, bez adresu strony Harness.
- BoundCredential przechowuje klucz wyłącznie w pamięci natywnej, bez Debug, Serialize ani publicznego getteru. Sprawdza profil, połączenie, provider, kanoniczny endpoint i rewizję klucza przed wywołaniem. Primary i Scribe mogą używać tego samego połączenia. To walidacja obiektu utworzonego przez natywnego wywołującego — jeszcze nie weryfikacja rekordu keyringu ani zapis sekretu.
- Client weryfikuje HTTPS przy użyciu rustls. HTTP dopuszczono dla lokalnych endpointów. URL jest parsowany i normalizowany; odrzuca userinfo, query, fragment, znaki sterujące oraz port 0. Model i trasa nadal przechodzą kontrolę kroku 1.
- Maksymalnie 30 sekund na nawiązanie połączenia; wspólny deadline operacji domyślnie 180 sekund, maksymalnie 4 rezerwacje fizycznego wywołania. Sklonowany CallBudget dzieli licznik i deadline; kolejne strony discovery nie resetują limitów. Licznik jest konserwatywny: rezerwacja bezpośrednio przed execute może zostać zużyta przy anulowaniu, zanim bajty zostaną wysłane. Nie jest to jeszcze trwały licznik Operation P2.
- Sticky Cancellation działa przed rozpoczęciem, w oczekiwaniu na nagłówki i podczas czytania body. Anulowanie porzuca lokalny future HTTP; nie gwarantuje anulowania pracy ani rozliczenia po stronie dostawcy. Nie powstał globalny rejestr ani nowa komenda IPC.
- Odpowiedź maksymalnie 8 MiB; sprawdzany Content-Length oraz każdy chunk. Żądanie maksymalnie 16 MiB po kodowaniu JSON. To limity transportu, odrębne od pojemności pamięci i budżetu tokenów. Nie ma cichego obcinania.
- Brak automatycznych redirectów, systemowego proxy i ukrytych retry reqwest. Proxy i streaming nie są obecnie obsługiwane. HTTP error body nie jest odczytywane ani ujawniane; błędy mają statyczne kategorie oraz bezpieczny status/liczbowy Retry-After, bez URL, surowej treści ani komunikatu reqwest. Retry-After obsługuje sekundy i IMF-fixdate z pułapem 30 sekund. Transport sam nie ponawia płatnego POST; executor retry/recovery jest następną integracją.
- discover_models wykonuje GET models, waliduje strukturę i zwraca id/label. Google filtruje supportedGenerationMethods=generateContent i obsługuje nextPageToken. Anthropic obsługuje has_more/last_id oraz after_id. OpenAI-compatible obsługuje ewentualne has_more/after. Brak strony, duplikat, powtarzany cursor lub przekroczenie budżetu kończy się jawnym błędem. Nie zwraca niepełnej listy jako pełnej. Pusta poprawna lista pozostaje pustą listą; nie oznacza testu połączenia. Limit wynosi 4000 pozycji i wspólny budżet wywołań. Nazwa modelu nie służy do zgadywania vision/reasoning.

## Wykorzystanie Harness

Zaadaptowano flow endpoint/auth/discovery z providers.rs, normalizację URL i nagłówki adapters.rs oraz zasady client/timeout/Retry-After z transport.rs. Nie skopiowano globalnego rejestru anulowania, domeny RV, keyringu, raw error strings ani nieograniczonego odczytu response.text. docs/reuse/4B1_HTTP_REUSE.json zawiera hashe źródeł i adaptowanych plików. MIT notice zachowano. Źródło Harness identyczne z migawką P1: SHA-256 534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e.

## Weryfikacja

| Kontrola | Wynik |
|---|---|
| Rust Linux bez desktopu, --all-targets --locked --offline | 50/50 PASS: 15 testów protokołów kroku 1, 27 nowych testów HTTP/discovery oraz 8 istniejących testów fundamentu/SQLite |
| Regresja podpisanego tekstu Google parser → replay | PASS w rzeczywistym Rust |
| Lokalny serwer HTTP na loopback | PASS: request/body/auth, HTTP 429 i Retry-After, redirect, invalid JSON, truncated/chunked/oversized body, request limit, anulowanie przed i w trakcie, wolne nagłówki/body, współdzielony limit i discovery |
| Format nowych modułów Rust | PASS rustfmt --check |
| Vitest | 63/63 PASS |
| TypeScript/build | PASS |
| Granice modułów | 17/17 PASS |
| Kontrakty P2 | 193/193 PASS; kontrakty i snapshoty bez zmian |
| Weryfikacja katalogu, pełnego ZIP i odtworzonej nakładki | PASS; hashe odnowione na faktycznym drzewie |
| Windows/Tauri z desktop feature/WebView | NOT RUN lokalnie; wymagane odpowiednie CI i późniejszy odbiór WebView |
| Zewnętrzne API, rzeczywiste klucze i keyring | NOT RUN; żadnych płatnych wywołań |

Narzędzia: rustc 1.99.0 (b940084d7 2026-09-28). Użyto cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets --locked --offline. Pominięto desktop feature, więc wynik nie dowodzi kompilacji ani działania Tauri/WebView. Testy Google/Anthropic obejmują formaty/parsery/nagłówki; prawdziwy lokalny HTTP obejmuje Custom OpenAI-compatible, nie połączenie z usługami tych dostawców. Logi: docs/evidence/4b1-step-2.

Frontend używa wcześniej zainstalowanych zależności przy bajtowo identycznym package-lock.json. Nie wykonywano nowej instalacji npm. Rust początkowo pobrał zależności, a końcowe wykonanie było offline z --locked.

Dodano bezpośrednie reqwest =0.13.5 z json/rustls i cechy time/net/io-util Tokio. Cargo.lock został wygenerowany przez Cargo na istniejącym lockfile; wersje wszystkich wcześniejszych pakietów zachowano, dodano graf TLS. Zestawienie: docs/evidence/4b1-step-2/dependency-change.json. Nie edytowano ręcznie lockfile. Zależności JS, workflow, uprawnienia i komendy IPC bez zmian. Nie potrzeba Prepare Cargo lockfile ani instalatora.

## Następny krok

4B-1 krok 3: vault AI Home z readback i bindingiem, trwałe profile/ProviderConnection/ModelRoute, integracja native HTTP z connection.test/operation.get oraz konfigurator. Retry executor i cancellation operacji muszą korzystać z jednego wspólnego CallBudget. Dopiero po tej integracji i live smoke będzie można odebrać działające połączenia. 4B-2 (rozmowy) nierozpoczęte. Otwarte Windows/WebView z 4B-0 zachowano.

Dokumentacja użyta przy implementacji: https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html, https://docs.rs/reqwest/0.13.5/reqwest/retry/fn.never.html, https://ai.google.dev/api/models, https://platform.claude.com/docs/en/api/models/list. Dokumentacja nie stanowi dowodu działania live API.
