# AI Home 4B-6 krok 4 — AUDIT FIX 2 F1–F3

Data: 23 października 2026.

## Zakres

Ograniczona poprawka reaudytu permanent deletion. Nie rozpoczyna 4B-7, nie zmienia frozen P2 i nie dodaje migracji. SQLite pozostaje v12.

## F1 — widoczność typów Rust

- `connections::Failure` i `connections::Tx` mają teraz widoczność `pub(crate)`.
- Nie rozszerzono publicznego API biblioteki poza crate.
- `deletion.rs` może legalnie współdzielić wewnętrzne aliasy transakcji/błędu z koordynatorem.

## F2 — raw source snapshots w memory_runs

- Archiwizacja rozmowy obejmuje teraz także queued/running `memory_update` powiązany z rozmową przez `memory_runs.body.conversationId`.
- Taka aktywna operacja jest terminalnie anulowana i pending memory proposals są zamykane tak samo jak dla innych lifecycle cancellations.
- `conversation.delete.permanent` redaguje powiązane `memory_runs`: `messages=[]`, `sources=[]`, `conversationId=null`, `conversationLanguage="und"`.
- Snapshot otrzymuje `sourceTombstone={reason:"permanent_deletion",resumable:false}`.
- `memory_runtime::ensure_current()` odrzuca run z `sourceTombstone`, więc stare źródło nie może być później użyte jako nadal dostępne do wznowienia.
- `memory_records`, zatwierdzone `memory_versions`, decyzje i historia pamięci nie są przez tę korektę usuwane.

## F3 — kontrakt Error i locale

- Nie rozszerzano frozen enum Error.
- Terminalny tombstone receipt używa istniejącego kodu `POLICY_DENIED` oraz `messageKey=error.permanently_deleted`.
- Dispatcher zwraca zapisany tombstone receipt bez ponownego wykonywania historycznej mutacji.
- Zmieniony payload pod tym samym requestId nadal kończy się `IDEMPOTENCY_CONFLICT` przed odczytem tombstone.
- Dodano lokalizacje EN/PL dla `error.permanently_deleted`.
- Dodano test validator/locale dla pełnego kształtu Error.

## Regresje

Do `src-tauri/tests/lifecycle.rs` dodano przypadek aktywnego `memory_update` powiązanego z rozmową: archive anuluje operację, permanent delete usuwa raw marker ze snapshotu, ustawia source tombstone i zachowuje rekord pamięci. Historyczne replay tests oczekują teraz dozwolonego `POLICY_DENIED` z właściwym messageKey.

Do `tests/deletion-error.test.ts` dodano test kontraktu Error i tłumaczeń EN/PL.

## Kontrole lokalne

- P2: 193/193 PASS.
- boundaries: 29/29 PASS.
- contract/handler/router parity: 51/51 PASS.
- Rust source-text normalization: PASS.
- schema expectation sweep: storage target v12, brak docelowych oczekiwań v11.
- substring receipt purge sweep: PASS.
- `PERMANENTLY_DELETED` custom code sweep: PASS, brak użycia poza dokumentacją historyczną.
- Frontend Vitest/typecheck/build: wymagają CI; lokalnie brak `node_modules` / `vitest`.
- Cargo/native runtime: wymaga CI; lokalny toolchain Rust niedostępny.
