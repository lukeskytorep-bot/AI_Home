# AI Home — 4B-1, krok 1: protokoły dostawców

Wersja treści 1.1 · 12 października 2026 · Korekta po audycie · Kandydat źródłowy do zwykłego CI

**Pierwsza część 4B-1 wykonana. Całe 4B-1 pozostaje otwarte.** Nie ogłaszamy gotowych połączeń z API, zapisu kluczy ani działającego konfiguratora. Edward upoważnił kolejny krok; otwarte Windows/WebView z 4B-0 pozostają w planie.

## Co rzeczywiście powstało

Natywny moduł providers przygotowuje endpoint i JSON żądania oraz odczytuje JSON odpowiedzi dla OpenAI, OpenRouter, Google, Anthropic i Custom OpenAI. Jest biblioteką protokołów, nie transportem HTTP. Nie przyjmuje klucza API, nie wykonuje sieci, nie zapisuje bazy i nie dodaje komend IPC. Kod kerne­la pozostaje bez logiki dostawców.

Przeniesiono request_builders.rs, response_parsers.rs i reasoning.rs z Harness. Ograniczono rodziny dostawców do P2, dodano fasadę AI Home i własne identyfikatory zakresu. Parser zachowuje rozdzielenie widocznego tekstu i danych dostawcy; dla podpisanej odpowiedzi Google i Anthropic nie zmienia widocznego tekstu. Surowy komunikat błędu dostawcy może zawierać klucz lub prywatny prompt, dlatego publiczna fasada zwraca bezpieczną kategorię InvalidResponse, bez surowego payloadu. Sumowanie usage jest odporne na przepełnienie u64.

Dla continuation dodano zgodność profileId, connectionId, routeId, rewizji trasy i credential oraz modelu/endpointu. Nie pozwala to przenieść stanu między profilem, Primary/Scribe lub zmienioną trasą. Google zachowuje kolejność części i podpis przy części widocznej; zmiana widocznego tekstu jest odrzucana. Lokalny guard wielkości replay to 2 MiB. Nie jest to jeszcze pełna walidacja wszystkich formatów continuation z Harness ani gotowy mechanizm chat/recovery: ścisłe walidatory podpisów, formatów i transportu pozostają w następnej integracji 4B-1.

Własne endpointy mają ograniczony podzbiór ASCII URL: HTTPS lub HTTP tylko na loopback, bez userinfo/query/fragment i niepoprawnego portu. Dostawcy nazwani korzystają z domyślnych adresów. To etap przed pełnym parserem URL w transporcie; nie deklarujemy obsługi wszystkich poprawnych URI ani dowolnych alternatywnych endpointów nazwanych providerów. Nie wywnioskowano możliwości modelu z jego nazwy i nie narzucono reasoning domyślnie.

Gotowe TS retry.ts i providerError.ts przeniesiono **bajtowo bez zmian** razem z testami retry. Polityka obejmuje transient HTTP, Retry-After, odrębne single recovery, anulowanie oraz brak retry po rozpoczęciu semantycznej odpowiedzi. Same funkcje nie wykonują ponowień: executor z łącznym budżetem P2 będzie je wywoływał w dalszej części etapu.

Zachowano MIT notice Harness. docs/reuse/4B1_PROTOCOL_REUSE.json identyfikuje pliki źródłowe/dostępne oraz adaptacje; HARNESS_PROVIDER_LICENSE.txt zachowuje oryginalny notice. Źródło: migawka z audytu P1, SHA-256 534b7c96458895625fd969949a1dfe5ac12cbc6f9716b56474d1493f9c375f6e.

## Wyniki i granice

| Kontrola | Wynik |
|---|---|
| Vitest | 63/63 PASS: 53 dotychczasowe + 10 oryginalnych testów retry Harness |
| TypeScript / produkcyjny build | PASS |
| Granice modułów i kontrakty P2 | 17/17 PASS; dodano dwa pliki TS do istniejącej kontroli |
| Retry/providerError względem Harness | Identyczne bajtowo |
| Cargo.toml, Cargo.lock, package*.json, workflow i IPC P2 | Bez zmian |
| Nowe testy Rust | 15 dostarczonych, NOT RUN lokalnie; obejmują cztery protokoły, custom endpointy, formaty obrazów, usage, safe errors i zakres replay |
| Rzeczywiste HTTP/API, keyring i WebView | NOT RUN; poza tą pierwszą częścią |

Świeże npm ci offline nie ukończyło instalacji z powodu brakującego elementu cache xmlchars. Kontrole wykonano z wcześniej zainstalowanymi zależnościami AI Home i identycznym package-lock.json. Zwykłe CI nadal wykonuje własne npm ci. Nie aktualizowano bibliotek ani lockfile.

Podstawa tej korekty: dokładnie dostarczony przez Edwarda AI_Home-main (1).zip, SHA-256 `85b4d928e9e2131f1c9169531ab1bdd161664ece0cc7f61f30a0ea6fa7f9e3c4`. Na tę bazę nałożono cały krok 1 i poniższe korekty. Pakiet zmian jest samodzielną nakładką na tę bazę, nie wymaga wcześniejszej paczki kroku 1. Zachowano wszystkie pliki bazy, w tym główny verify-boundaries.mjs; nie wykonywano sprzątania. Kod profili i połączeń nie jest kopiowany z domeny RV.

## Korekty dwóch uwag audytora

1. Manifesty odtworzono z faktycznego drzewa tej konkretnej bazy i korekty. Pięć historycznych plików snapshotów wymienionych w poprzednim manifeście nie istnieje w załączonej bazie — nie odtwarzano ich ani nie deklarowano ich obecności. Snapshot P2 1.1 zachowano bez zmian. Główny verify-boundaries.mjs jest objęty nowymi hashami. Nakładka zawiera scripts/verify-source.py: katalog dopuszcza wyłącznie równoważność UTF-8 CRLF/LF, archiwum jest sprawdzane bajtowo. Zweryfikowano katalog, pełny ZIP i niezależne nałożenie nakładki na świeżo rozpakowaną bazę.
2. Google z natywnym stanem continuation zachowuje oryginalny widoczny tekst, także dosłowne znaczniki <think>. Dodany test google_signed_literal_text_survives_parser_to_replay buduje continuation z rzeczywistego wyniku parsera i przekazuje go do prepare_chat. Sprawdza równość widocznego tekstu oraz całej kolejności części, podpisów i pól thought w kolejnym żądaniu, zarówno dla podpisu przy widocznej części, jak i dodatkowej części ukrytej. To 15. test Rust; lokalnie **NOT RUN**, ponieważ cargo/rustc nie są dostępne. Wymagany nowy wynik istniejącego CI dla poprawionego modułu; zielona baza nie stanowi tego dowodu.

Zalecenie zachowania części i podpisów Google: https://ai.google.dev/gemini-api/docs/generate-content/thinking.

## Następna część 4B-1

Natywny HTTP i discovery, ograniczenia czasu/rozmiaru i anulowanie; osobna przestrzeń sekretów AI Home z bindingiem dostawca/endpoint/połączenie i readback; trwałe operacje connection.test/operation.get oraz integracja z profilami/trasami P2. Dopiero potem możemy ogłosić działający test połączenia i przeprowadzić live smoke OpenAI-compatible/OpenRouter. Google/Anthropic wymagają natywnych testów kontraktów i potwierdzenia obsługiwanych modeli/parametrów. Scribe, rozmowy, import i pamięć mają własne etapy.

Pakiet nie wymaga nowych jobów CI, instalatora, ponownego Prepare Cargo lockfile ani ręcznego usuwania plików. 4B-0 nie otrzymuje PASS CLOSED. Nie rozpoczęto 4B-2.

Sprawdzona dokumentacja bazowych formatów API: https://developers.openai.com/api/reference/resources/chat, https://ai.google.dev/api/generate-content, https://docs.anthropic.com/en/api/messages, https://openrouter.ai/docs/api-reference/overview. Dokumentacja i testy protokołu nie zastępują wywołania API konkretnego modelu.
