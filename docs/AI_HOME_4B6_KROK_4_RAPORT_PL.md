# AI Home 4B-6 krok 4 — Permanent Deletion z Archive

Data: 23 października 2026.

## Zakres

Krok 4 dodaje trwałe usuwanie wyłącznie rekordów znajdujących się już w Archive. Nie rozpoczyna 4B-7 i nie dodaje migracji SQLite; storage pozostaje v12.

## Implementacja

- Dodano suplement `contracts/deletion.ipc.json` z `profile.delete.permanent` oraz `conversation.delete.permanent`.
- Obie komendy wymagają `expectedRevision` oraz jawnego `confirm:true`.
- Aktywnego profilu ani aktywnej rozmowy nie można usunąć trwale. Native wymaga statusu `archived`.
- Profile delete dodatkowo odrzuca żądanie, jeżeli istnieje queued/running operation.
- Conversation delete usuwa message documents, replay, attachments, chat runs, segments, messages, conversation-scoped import materials, cursors oraz nieużywane już blobs/operations.
- Profile delete usuwa zależne rekordy profilu w kolejności respektującej FK: rozmowy/importy/pamięć/routes/connections/operations/events i dopiero profil.
- Przed commitem wykonywany jest `PRAGMA foreign_key_check`; naruszenie integralności kończy operację błędem i rollbackiem.
- Istniejący receipt/idempotency mechanizm daje replay tego samego permanent-delete requestId bez drugiego usuwania. Inny payload pod tym samym requestId pozostaje konfliktem idempotencji.
- Cursory związane z usuwanym zakresem są invalidowane.
- UI Archive ma dwustopniowe potwierdzenie: `Delete permanently` -> `Confirm permanent delete` / `Cancel`.
- UI jawnie informuje, że permanent deletion dotyczy aktywnej bazy AI Home; wcześniejsze backupy i portable exports pozostają niezależnymi kopiami.

## Warning cleanup z zielonego CI kroku 3

- Usunięto nieużywaną lokalną zmienną `export` w `application.rs` w ścieżce, która jej nie potrzebuje.
- W `src-tauri/tests/export.rs` usunięto zbędne nawiasy wokół typu `Vec<String>`.

## Testy i bramki

Dodano regresje native dla: aktywny rekord -> POLICY_DENIED, stale CAS -> REVISION_CONFLICT, archived delete, idempotent replay i zniknięcie rekordu z odczytów. Dodano regresje UI wymagające osobnego potwierdzenia dla profilu i rozmowy.

Lokalnie wykonano:
- `python3 check_contracts.py` -> 193/193 PASS,
- `node scripts/verify-boundaries.mjs` -> 29/29 PASS,
- `python3 scripts/verify-command-parity.py` -> 51/51 PASS,
- `python3 scripts/verify-rust-source-text.py` -> PASS,
- schema expectation sweep -> PASS, SQLite v12,
- overlay reconstruction -> wymagane przed handoffem.

Pełne Cargo/Vitest/typecheck/Vite/Tauri/WebView pozostają do GitHub Actions. Nie deklaruje się lokalnego runtime PASS bez toolchainu Rust i pełnych zależności frontendowych.
