# AI Home 4B-6 krok 4 AUDIT FIX 3 — raport

Data: 23 października 2026.

## Zakres

Poprawka dotyczy wyłącznie błędu testowego ujawnionego przez GitHub Actions po AUDIT FIX 2. Produkcyjna logika archiwizacji/permanent deletion nie została zmieniona.

## Przyczyna

Nowy test `permanent_conversation_delete_redacts_memory_run_sources_and_cancels_active_memory_operation` ręcznie wstawiał rekord `operations.body` niezgodny z frozen DTO `Operation`. Fixture `memory_update` nie zawierał wymaganych pól `capacityAttemptCount`, `limits` i `recordRefs`. `conversation.archive` prawidłowo odczytywał operację przez standardowe `get()`, walidacja DTO odrzucała wadliwy rekord i dispatcher zwracał `STORAGE_ERROR`.

## Poprawka

- uzupełniono fixture do kształtu używanego przez `memory_runtime::accept()`;
- dodano `capacityAttemptCount: 0`;
- dodano `expectedRecordRevision: 1`;
- dodano `limits.maxPhysicalCalls` i `limits.maxElapsedMs`;
- dodano `recordRefs` wskazujące testowy `memory_record`;
- przed INSERT-em fixture wykonuje `assert!(schema::dto("Operation", &op))`, dzięki czemu przyszła niezgodność kontraktu zostanie zgłoszona bezpośrednio w miejscu tworzenia fixture, a nie jako wtórny `STORAGE_ERROR`.

## Kontrole lokalne

- P2: 193/193 PASS;
- boundaries: 29/29 PASS;
- Rust source-text normalization: PASS;
- schema expectation sweep: PASS, SQLite v12;
- brak zmian produkcyjnych i brak migracji.

Pełny Cargo/Vitest/typecheck/build pozostaje do GitHub Actions.
