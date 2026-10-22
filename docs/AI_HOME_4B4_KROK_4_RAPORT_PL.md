# AI Home — 4B-4 krok 4: przeznaczenia importu

22 października 2026. Plan treść 1.31. **IMPLEMENTED / REQUIRES CI AND AUDIT.** Bazą jest dokładny `AI_HOME_4B4_KROK_3_SOURCE.zip`, SHA-256 `7b7f3abccdcd0825721e1c1122e0cefdcf0672a2fcbf4ae552507e80b016e147`.

## Zakres

Domknięto cztery znaczenia przewidziane przez frozen `ImportRequest`: `conversation`, `knowledge`, `guidance`, `memory_import`. Istniejący import rozmowy zachowuje dotychczasowe zachowanie. Trzy nowe cele są profile-scoped i nie tworzą fikcyjnej więzi z rozmową. Oryginalny blob/hash, nazwa, importer oraz deklarowane autorstwo pozostają oddzielne.

`knowledge` tworzy `KnowledgeSource` po poprawnym parsowaniu dokumentu. Źródła są dobierane deterministycznym `LexicalRetriever` według ostatnich 4096 znaków nowej wiadomości. Wiedza dzieli `retrievalTargetTokens` z już wybraną pamięcią: najpierw pamięć, potem wiedza wykorzystuje pozostały budżet. Całe bloki są włączane albo pomijane; treść nie jest cięta. Pierwsza implementacja nie używa embeddings ani vector DB.

`guidance` tworzy `PersistentGuidance` i korzysta z `guidanceTargetTokens` (domyślnie 800 z frozen P2). Wszystkie aktywne wskazówki są liczone jako pełne zewnętrzne bloki user/JSON. Zestaw jest all-or-error: jeśli przekracza budżet, zwracany jest `CONTEXT_BUDGET_EXCEEDED` przed fizycznym POST. Guidance nie staje się system promptem, policy, capability ani tool authorization. Stała instrukcja systemowa i osobna etykieta danych mówią modelowi, że jest to materiał zewnętrzny i omylny.

`memory_import` tworzy tylko gotowy materiał źródłowy. Nie tworzy `MemoryVersion`, proposal ani receipt. UI przekazuje SourceRef `kind=import` do istniejącego `memory.record.update`; dalej obowiązują Scribe/Primary routing, osobny Primary ACCEPT/MODIFY/REJECT, capacity attempts, format repair, fizyczny call budget, STOP i snapshot. Importowe SourceRef nie są przedstawiane jako wiadomości i nie tworzą fałszywych `sourceContacts`.

## Chat i snapshot

`chat.preflight` i `chat.send` współdzielą ten sam `prepare()`. Po Working Context i pamięci przechwytywane są profile knowledge i persistent guidance, a ich pełny koszt wchodzi do rzeczywistego okna modelu razem z historią, dokumentami/obrazami, outputem i reserve. `chat_runs` zapisuje `profileContext`. Przed każdym POST/commit bieżąca selekcja jest odtwarzana i porównywana; zmiana aktywacji, rewizji, trasy lub danych powoduje `REVISION_CONFLICT`.

Nowe `chat.profile.context.get` zwraca dokładny przechwycony zestaw. Historyczny odczyt weryfikuje DTO, scope profilu, immutable extracted blob/hash, koszty pełnych bloków oraz measurement ID/revision trasy. Stare runy bez `profileContext` zwracają `known=false`; nie są uzupełniane dzisiejszym stanem.

## Storage i IPC

SQLite `10→11` dodaje tylko:

- `profile_import_materials`,
- `knowledge_sources`,
- `persistent_guidance`.

Tabele używają profile-scoped FK. Dotychczasowe rozmowy, `import_materials`, attachments, memory records/versions i receipts nie są przepisywane. Pending profile import po restarcie jest zamykany jako failed/`OPERATION_INTERRUPTED`, zachowując oryginalny blob.

Nowy własny suplement `contracts/profile-imports.ipc.json` dodaje `profile.imports.list`, `knowledge.enabled.set` i `chat.profile.context.get`. Zamrożone `guidance.enabled.set` jest używane bez zmiany P2. `document-chat.ipc.json` dodaje opcjonalne wyniki knowledge/guidance w preflight. Frozen P2/foundation nie zmieniono.

## UI EN/PL

Panel plików pozwala wybrać: materiał rozmowy, wiedzę profilu, stałe wskazówki lub kandydaturę pamięci. Profile imports pokazują osobno autora materiału i lokalnego importera. Wiedzę i guidance można włączać/wyłączać przez CAS. Kandydatura pamięci wymaga wybranego rekordu i jawnego powodu; przycisk uruchamia istniejący `memory.record.update`, bez ręcznego pola udającego decyzję Primary.

## Testy i weryfikacja w tym środowisku

Dodano regresje Rust importu dla profile knowledge, braku bezpośredniej aktywacji memory import oraz izolacji profili. Test UI sprawdza także, że `knowledge` nie wysyła `conversationId`. Oczekiwana docelowa wersja SQLite w dotychczasowych testach migracji została podniesiona do 11.

W sandboxie wykonano:

- frozen P2/reference checks: **193/193 PASS**,
- module boundaries: **26/26 PASS**,
- JSON syntax nowych/zmienionych kontraktów: **PASS**.

Nie udało się wykonać pełnego Rust/Vitest/TypeScript/Vite. Sandbox nie zawiera `cargo`/`rustc`, a `npm ci --ignore-scripts` na niezmienionym lockfile nie ukończył pobrania zależności przed timeoutem. Próba doinstalowania toolchainu także nie ukończyła się w dostępnym środowisku. Nie przedstawiam więc tego kroku jako lokalnego PASS. Pełne CI jest wymagane przed akceptacją.

Nie wykonano Windows/Tauri/WebView, systemowego vault ani live provider API. Nie zmieniono workflow, providerów, vault, package-lock/Cargo.lock ani zależności.

## Dalej

Po zielonym pełnym CI i niezależnym audycie krok 4 domyka funkcjonalnie 4B-4. Następny etap to **4B-5 Conversation Segmentation + Continuity**. Nie należy rozpoczynać 4B-5 w tym samym patchu.
