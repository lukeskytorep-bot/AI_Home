# AI Home 4B-6 krok 4 AUDIT FIX 4 — raport

Data: 23 października 2026.

## Zakres

Poprawka dotyczy wyłącznie błędu kompilacji testu `src-tauri/tests/lifecycle.rs` ujawnionego przez pełne Cargo po AUDIT FIX 3. Produkcyjna logika permanent deletion, memory-run redaction, receipts i archive lifecycle nie została zmieniona.

## Przyczyna

Test importował `schema` jako `ai_home::schema`, ale crate nie eksportuje modułu `schema` z root. Publiczna ścieżka używana przez istniejące testy to `ai_home::domain::schema`, ponieważ `domain` jest modułem publicznym. Rust słusznie zgłaszał `E0432 unresolved import ai_home::schema`.

## Poprawka

- `src-tauri/tests/lifecycle.rs` importuje teraz `domain::schema`;
- nie upubliczniono żadnego dodatkowego modułu ani aliasu tylko na potrzeby testu;
- pełny sweep `src-tauri/tests` potwierdził brak innych importów `ai_home::schema`;
- `verify-rust-source-text.py` rozszerzono o kontrolę zakazującą root importu `ai_home::schema` w testach.

## Kontrole lokalne

- P2: 193/193 PASS;
- boundaries: 29/29 PASS;
- command parity: 51/51 PASS;
- Rust source-text normalization: PASS;
- schema expectation sweep: PASS, SQLite v12;
- brak zmian produkcyjnych i brak migracji.

Pełny Cargo/Vitest/typecheck/build pozostaje do GitHub Actions.
