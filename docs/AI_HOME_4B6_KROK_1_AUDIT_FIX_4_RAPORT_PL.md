# AI Home 4B-6 krok 1 — AUDIT FIX 4 — raport

Data: 22 października 2026.

## Zakres

Poprawka usuwa błąd CI `UNKNOWN_COMMAND` dla `profile.import.latest`. Implementacja komendy była obecna w `src-tauri/src/profile_imports.rs`, ale natywny router `src-tauri/src/connections.rs` nie deklarował jej w `COMMANDS` i nie kierował do `profile_imports::dispatch`.

## Zmiana

- dodano `profile.import.latest` do `connections::COMMANDS`;
- dodano `profile.import.latest` do gałęzi dispatch `profile_imports::dispatch`;
- wykonano pełny sweep kontrakt/handler/router; nie znaleziono innych braków tego samego typu w ścieżce connections;
- `kernel.status.get` i `app.events.list` są obsługiwane przez warstwę application i nie są brakami routera connections;
- pełny sweep asercji schematu nadal nie zawiera oczekiwań `user_version == 11`;
- SQLite pozostaje v12, bez migracji.

## Status

IMPLEMENTED — REQUIRES CI AND RE-AUDIT. Nie rozpoczyna 4B-6 kroku 2.
